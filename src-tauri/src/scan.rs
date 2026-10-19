use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey};
use rusqlite::params;
use serde::Serialize;
use uuid::Uuid;

use crate::db::Db;
use crate::error::{AppError, AppResult};

const AUDIO_EXT: &[&str] = &[
    "flac", "wav", "wave", "mp3", "ogg", "opus", "m4a", "aac", "aiff", "aif", "wma", "alac",
    "ape", "wv", "dsf", "dff", "m4b", "mp4", "caf", "tta", "spx", "oga",
];
const VIDEO_EXT: &[&str] = &["mkv", "avi", "webm", "wmv", "flv", "mpg", "mpeg", "mov", "m4v"];
const TAG_WORKERS: usize = 4;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdChange {
    pub local_file_id: String,
    pub previous_content_id: String,
    pub next_content_id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub kind: String,
    pub path: String,
    pub grant_id: i64,
    pub added: usize,
    pub updated: usize,
    pub missing: usize,
}

struct ExistingRow {
    local_file_id: String,
    content_id: Option<String>,
    mtime_ms: Option<i64>,
    size_bytes: Option<i64>,
    exposed: i64,
}

struct Prepared {
    relative: String,
    absolute: PathBuf,
    mtime_ms: i64,
    size: i64,
    tags: Option<TagRead>,
}

/// Walks a grant and reads tags without holding the database, then writes the batch in one transaction.
pub fn import_path(db: &Arc<Mutex<Db>>, path: &Path, kind: &str) -> AppResult<ImportSummary> {
    let kind = match kind {
        "auto" if path.is_dir() => "directory",
        "auto" if path.is_file() => "file",
        "file" if path.is_file() => "file",
        "directory" if path.is_dir() => "directory",
        _ => return Err(AppError::Message("file.unreadable".into())),
    };
    if !path.exists() {
        return Err(AppError::Message("file.missing".into()));
    }
    let path_text = path.to_string_lossy().to_string();
    let (grant_id, existing) = {
        let guard = db.lock().expect("db");
        let grant_id = guard.insert_grant(kind, &path_text)?;
        (grant_id, existing_rows(&guard, grant_id)?)
    };
    let mut files = Vec::new();
    if kind == "file" {
        if is_audio_candidate(path) {
            files.push(path.to_path_buf());
        }
    } else {
        collect_audio(path, &mut files);
    }
    let root_is_dir = kind == "directory";
    let prepared = prepare_files(path, root_is_dir, &files, &existing);

    let guard = db.lock().expect("db");
    guard.conn.execute_batch("BEGIN")?;
    let written = write_prepared(&guard, grant_id, &prepared, &existing);
    match written {
        Ok((added, updated)) => {
            let seen: HashSet<&str> = prepared.iter().map(|item| item.relative.as_str()).collect();
            let mut missing = 0;
            for (relative, row) in &existing {
                if !seen.contains(relative.as_str()) {
                    guard.conn.execute(
                        "UPDATE local_files SET access_status = 'missing', access_reason = 'file.missing'
                         WHERE local_file_id = ?1",
                        params![row.local_file_id],
                    )?;
                    missing += 1;
                }
            }
            guard.conn.execute_batch("COMMIT")?;
            Ok(ImportSummary { kind: kind.into(), path: path_text, grant_id, added, updated, missing })
        }
        Err(err) => {
            let _ = guard.conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

fn existing_rows(db: &Db, grant_id: i64) -> AppResult<HashMap<String, ExistingRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT relative_path, local_file_id, content_id, mtime_ms, size_bytes, exposed
         FROM local_files WHERE grant_id = ?1",
    )?;
    let rows = stmt.query_map(params![grant_id], |row| {
        Ok((
            row.get::<_, String>(0)?,
            ExistingRow {
                local_file_id: row.get(1)?,
                content_id: row.get(2)?,
                mtime_ms: row.get(3)?,
                size_bytes: row.get(4)?,
                exposed: row.get(5)?,
            },
        ))
    })?;
    Ok(rows.collect::<Result<HashMap<_, _>, _>>()?)
}

fn prepare_files(
    root: &Path,
    root_is_dir: bool,
    files: &[PathBuf],
    existing: &HashMap<String, ExistingRow>,
) -> Vec<Prepared> {
    let chunk = files.len().div_ceil(TAG_WORKERS).max(1);
    let mut out = Vec::with_capacity(files.len());
    std::thread::scope(|scope| {
        let workers: Vec<_> = files
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    part.iter()
                        .filter_map(|file| prepare_one(root, root_is_dir, file, existing))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        for worker in workers {
            if let Ok(items) = worker.join() {
                out.extend(items);
            }
        }
    });
    out
}

fn prepare_one(
    root: &Path,
    root_is_dir: bool,
    file: &Path,
    existing: &HashMap<String, ExistingRow>,
) -> Option<Prepared> {
    let meta = fs::metadata(file).ok()?;
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|time| time.as_millis() as i64)
        .unwrap_or(0);
    let size = meta.len() as i64;
    let relative = if root_is_dir {
        file.strip_prefix(root).unwrap_or(file).to_string_lossy().to_string()
    } else {
        file.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string_lossy().to_string())
    };
    let unchanged = existing
        .get(&relative)
        .is_some_and(|row| row.mtime_ms == Some(mtime_ms) && row.size_bytes == Some(size));
    Some(Prepared {
        relative,
        absolute: file.to_path_buf(),
        mtime_ms,
        size,
        tags: if unchanged { None } else { Some(read_tags(file)) },
    })
}

fn write_prepared(
    db: &Db,
    grant_id: i64,
    prepared: &[Prepared],
    existing: &HashMap<String, ExistingRow>,
) -> AppResult<(usize, usize)> {
    let mut added = 0;
    let mut updated = 0;
    let new_exposed = if db.exposure_for_new_folder_file(grant_id)? { 1 } else { 0 };
    for item in prepared {
        let previous = existing.get(&item.relative);
        let Some(read) = &item.tags else {
            if let Some(row) = previous {
                db.conn.execute(
                    "UPDATE local_files SET absolute_path = ?1, access_status = 'available', access_reason = NULL
                     WHERE local_file_id = ?2",
                    params![item.absolute.to_string_lossy(), row.local_file_id],
                )?;
            }
            continue;
        };
        let file = item.absolute.as_path();
        let video = read.video || VIDEO_EXT.contains(&extension(file).as_str());
        let file_name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "audio".into());
        let title = if read.title.is_empty() { file_name.clone() } else { read.title.clone() };
        let now = chrono::Utc::now().to_rfc3339();
        let bytes_changed = previous.is_some();
        let local_file_id = previous
            .map(|row| row.local_file_id.clone())
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let exposed = previous.map(|row| row.exposed).unwrap_or(new_exposed);
        let folder = file.parent().map(|path| path.to_string_lossy().to_string()).unwrap_or_default();
        db.conn.execute(
            "INSERT INTO local_files(
                local_file_id, grant_id, relative_path, absolute_path, file_name, lyrics,
                metadata_status, metadata_reason, access_status, access_reason,
                playback_status, playback_reason, exposed, added_at, mtime_ms, size_bytes,
                content_id, duration_ms, title, artist, album, track_number, codec,
                sample_rate, bit_depth, video, cover_path, cover_checked
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'available',NULL,'untried',NULL,?9,?10,?11,?12,NULL,?13,?14,?15,?16,?17,?18,?19,?20,?21,NULL,0)
             ON CONFLICT(local_file_id) DO UPDATE SET
                absolute_path = excluded.absolute_path,
                file_name = excluded.file_name,
                lyrics = excluded.lyrics,
                metadata_status = excluded.metadata_status,
                metadata_reason = excluded.metadata_reason,
                access_status = 'available',
                access_reason = NULL,
                playback_status = 'untried',
                playback_reason = NULL,
                mtime_ms = excluded.mtime_ms,
                size_bytes = excluded.size_bytes,
                content_id = NULL,
                duration_ms = excluded.duration_ms,
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                track_number = excluded.track_number,
                codec = excluded.codec,
                sample_rate = excluded.sample_rate,
                bit_depth = excluded.bit_depth,
                video = excluded.video,
                cover_path = NULL,
                cover_checked = 0",
            params![
                local_file_id,
                grant_id,
                item.relative,
                file.to_string_lossy(),
                file_name,
                read.lyrics,
                read.metadata_status,
                read.metadata_reason,
                exposed,
                now,
                item.mtime_ms,
                item.size,
                read.duration_ms,
                title,
                read.artist,
                read.album,
                read.track_number,
                read.codec,
                read.sample_rate,
                read.bit_depth,
                video,
            ],
        )?;
        index_fts(db, &local_file_id, None, &title, &read.artist, &read.lyrics, &folder)?;
        if let Some(previous_content) = previous.and_then(|row| row.content_id.clone()) {
            db.conn.execute(
                "INSERT INTO pending_content_move(local_file_id, previous_content_id)
                 VALUES(?1, ?2)
                 ON CONFLICT(local_file_id) DO UPDATE SET previous_content_id = excluded.previous_content_id",
                params![local_file_id, previous_content],
            )?;
        }
        if bytes_changed { updated += 1 } else { added += 1 }
    }
    Ok((added, updated))
}

fn collect_audio(dir: &Path, out: &mut Vec<PathBuf>) {
    // `Path::is_dir` follows links.  Recursing that way can walk a junction
    // back to an ancestor forever (and could pull unrelated files into a
    // granted folder), so walk only real directories iteratively.
    let mut pending = vec![dir.to_path_buf()];
    while let Some(next) = pending.pop() {
        let Ok(entries) = fs::read_dir(next) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                let path = entry.path();
                if is_audio_candidate(&path) && !is_download_temp(&path) {
                    out.push(path);
                }
            }
        }
    }
}

fn is_download_temp(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with(".device-music-download-") || name.ends_with(".part"))
        .unwrap_or(false)
}

fn is_audio_candidate(path: &Path) -> bool {
    let ext = extension(path);
    AUDIO_EXT.contains(&ext.as_str()) || VIDEO_EXT.contains(&ext.as_str())
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn index_fts(
    db: &Db,
    local_file_id: &str,
    content_id: Option<&str>,
    title: &str,
    artist: &str,
    lyrics: &str,
    folder: &str,
) -> AppResult<()> {
    db.conn.execute(
        "DELETE FROM library_fts WHERE local_file_id = ?1",
        params![local_file_id],
    )?;
    db.conn.execute(
        "INSERT INTO library_fts(local_file_id, content_id, title, artist, lyrics, folder)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
        params![local_file_id, content_id.unwrap_or(""), title, artist, strip_lrc(lyrics), folder],
    )?;
    Ok(())
}

pub fn strip_lrc(lyrics: &str) -> String {
    lyrics
        .lines()
        .map(|line| {
            let mut rest = line;
            while rest.starts_with('[') {
                if let Some(end) = rest.find(']') {
                    rest = &rest[end + 1..];
                } else {
                    break;
                }
            }
            rest.trim()
        })
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

struct TagRead {
    title: String,
    artist: String,
    album: String,
    track_number: Option<i64>,
    duration_ms: Option<i64>,
    codec: String,
    sample_rate: Option<i64>,
    bit_depth: Option<i64>,
    lyrics: String,
    metadata_status: String,
    metadata_reason: Option<String>,
    video: bool,
}

fn read_tags(path: &Path) -> TagRead {
    let mut read = TagRead {
        title: String::new(),
        artist: String::new(),
        album: String::new(),
        track_number: None,
        duration_ms: None,
        codec: extension(path),
        sample_rate: None,
        bit_depth: None,
        lyrics: sidecar_lyrics(path),
        metadata_status: "ready".into(),
        metadata_reason: None,
        video: container_has_video(path),
    };
    let parsed = Probe::open(path).and_then(|probe| {
        probe
            .options(ParseOptions::new().read_properties(true))
            .read()
    });
    let Ok(tagged) = parsed else {
        read.metadata_status = "failed".into();
        read.metadata_reason = Some("metadata.failed".into());
        return read;
    };
    let properties = tagged.properties();
    let duration = properties.duration().as_millis();
    read.duration_ms = if duration > 0 { duration.try_into().ok() } else { None };
    read.sample_rate = properties.sample_rate().map(|value| value as i64);
    read.bit_depth = properties.bit_depth().map(|value| value as i64);
    read.codec = format!("{:?}", tagged.file_type()).to_ascii_lowercase();
    if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
        read.title = tag.title().unwrap_or_default().trim().to_string();
        read.artist = tag.artist().unwrap_or_default().trim().to_string();
        read.album = tag.album().unwrap_or_default().trim().to_string();
        read.track_number = tag.track().map(|value| value as i64);
        if read.lyrics.is_empty() {
            read.lyrics = tag
                .get_string(&ItemKey::Lyrics)
                .unwrap_or_default()
                .to_string();
        }
    }
    read
}

fn sidecar_lyrics(path: &Path) -> String {
    let mut lrc = path.to_path_buf();
    lrc.set_extension("lrc");
    fs::read_to_string(lrc).unwrap_or_default()
}

fn container_has_video(path: &Path) -> bool {
    let ext = extension(path);
    if VIDEO_EXT.contains(&ext.as_str()) {
        return true;
    }
    if ext != "mp4" && ext != "m4a" && ext != "m4b" {
        return false;
    }
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    let mut buf = vec![0u8; 512 * 1024];
    let Ok(n) = file.read(&mut buf) else {
        return false;
    };
    let sample = &buf[..n];
    contains_handler(sample, b"vide") && !audio_only_handler(sample)
}

fn contains_handler(sample: &[u8], kind: &[u8]) -> bool {
    sample.windows(kind.len()).any(|window| window == kind)
}

fn audio_only_handler(sample: &[u8]) -> bool {
    contains_handler(sample, b"soun") && !contains_handler(sample, b"vide")
}

pub struct HashJob {
    pub local_file_id: String,
    pub path: PathBuf,
    pub mtime_ms: i64,
    pub size: i64,
}

/// The preferred file is the one the player just used; other files wait their turn by import time.
pub fn next_hash_job(db: &Db, prefer: Option<&str>, only_prefer: bool) -> AppResult<Option<HashJob>> {
    let map = |row: &rusqlite::Row<'_>| {
        Ok(HashJob {
            local_file_id: row.get(0)?,
            path: PathBuf::from(row.get::<_, String>(1)?),
            mtime_ms: row.get::<_, Option<i64>>(2)?.unwrap_or(0),
            size: row.get::<_, Option<i64>>(3)?.unwrap_or(0),
        })
    };
    if let Some(prefer) = prefer {
        let job = db
            .conn
            .query_row(
                "SELECT local_file_id, absolute_path, mtime_ms, size_bytes FROM local_files
                 WHERE local_file_id = ?1 AND content_id IS NULL AND access_status = 'available'",
                params![prefer],
                map,
            )
            .ok();
        if job.is_some() || only_prefer {
            return Ok(job);
        }
    }
    Ok(db
        .conn
        .query_row(
            "SELECT local_file_id, absolute_path, mtime_ms, size_bytes FROM local_files
             WHERE content_id IS NULL AND access_status = 'available'
             ORDER BY added_at LIMIT 1",
            [],
            map,
        )
        .ok())
}

/// Reads the whole file in 1 MiB blocks. Runs without the database lock.
pub fn compute_content_id(path: &Path) -> std::io::Result<String> {
    let mut hasher = blake3::Hasher::new();
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

pub fn finish_hash(db: &Db, job: &HashJob, content_id: &str) -> AppResult<bool> {
    let still: Option<(Option<i64>, Option<i64>, Option<String>)> = db
        .conn
        .query_row(
            "SELECT mtime_ms, size_bytes, content_id FROM local_files WHERE local_file_id = ?1",
            params![job.local_file_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok();
    let Some((mtime, size, existing)) = still else { return Ok(false) };
    if mtime != Some(job.mtime_ms) || size != Some(job.size) || existing.is_some() {
        return Ok(false);
    }
    bind_content(db, &job.local_file_id, content_id)?;
    Ok(true)
}

pub fn mark_unreadable(db: &Db, local_file_id: &str) -> AppResult<()> {
    db.conn.execute(
        "UPDATE local_files SET access_status = 'unreadable', access_reason = 'file.unreadable'
         WHERE local_file_id = ?1",
        params![local_file_id],
    )?;
    Ok(())
}

pub fn take_id_change(db: &Db, local_file_id: &str, next_content_id: &str) -> AppResult<Option<IdChange>> {
    let previous: Option<String> = db
        .conn
        .query_row(
            "SELECT previous_content_id FROM pending_content_move WHERE local_file_id = ?1",
            params![local_file_id],
            |row| row.get(0),
        )
        .ok();
    db.conn.execute(
        "DELETE FROM pending_content_move WHERE local_file_id = ?1",
        params![local_file_id],
    )?;
    let Some(previous) = previous else {
        return Ok(None);
    };
    if previous == next_content_id {
        return Ok(None);
    }
    let in_use: i64 = db.conn.query_row(
        "SELECT (SELECT COUNT(*) FROM favorites WHERE content_id = ?1)
              + (SELECT COUNT(*) FROM playlist_tracks WHERE content_id = ?1)",
        params![previous],
        |row| row.get(0),
    )?;
    if in_use == 0 {
        return Ok(None);
    }
    let title: String = db
        .conn
        .query_row(
            "SELECT title FROM local_files WHERE local_file_id = ?1",
            params![local_file_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|_| String::new());
    Ok(Some(IdChange {
        local_file_id: local_file_id.to_string(),
        previous_content_id: previous,
        next_content_id: next_content_id.to_string(),
        title,
    }))
}

fn bind_content(db: &Db, local_file_id: &str, content_id: &str) -> AppResult<()> {
    let (title, artist, album, track, duration, codec, sample_rate, bit_depth, video, playback, reason): (
        String,
        String,
        String,
        Option<i64>,
        Option<i64>,
        String,
        Option<i64>,
        Option<i64>,
        i64,
        String,
        Option<String>,
    ) = db.conn.query_row(
        "SELECT title, artist, album, track_number, duration_ms, codec, sample_rate, bit_depth, video, playback_status, playback_reason
         FROM local_files WHERE local_file_id = ?1",
        params![local_file_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
                row.get(6)?,
                row.get(7)?,
                row.get(8)?,
                row.get(9)?,
                row.get(10)?,
            ))
        },
    )?;
    let failed = playback == "failed"
        && matches!(reason.as_deref(), Some("playback.decodeFailed") | Some("playback.unsupported") | Some("playback.video"));
    db.conn.execute(
        "INSERT INTO content_items(content_id, title, artist, album, track_number, duration_ms, codec, sample_rate, bit_depth, video, unplayable, unplayable_reason)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
         ON CONFLICT(content_id) DO UPDATE SET
            title = CASE WHEN excluded.title = '' THEN content_items.title ELSE excluded.title END,
            artist = CASE WHEN excluded.artist = '' THEN content_items.artist ELSE excluded.artist END,
            album = CASE WHEN excluded.album = '' THEN content_items.album ELSE excluded.album END,
            unplayable = MAX(content_items.unplayable, excluded.unplayable),
            unplayable_reason = COALESCE(content_items.unplayable_reason, excluded.unplayable_reason)",
        params![
            content_id,
            title,
            artist,
            album,
            track,
            duration,
            codec,
            sample_rate,
            bit_depth,
            video,
            if failed { 1 } else { 0 },
            if failed { reason } else { None }
        ],
    )?;
    db.conn.execute(
        "UPDATE local_files SET content_id = ?1 WHERE local_file_id = ?2",
        params![content_id, local_file_id],
    )?;
    let merged_favorite: Option<i64> = db
        .conn
        .query_row("SELECT id FROM favorites WHERE content_id = ?1", params![content_id], |row| row.get(0))
        .ok();
    if merged_favorite.is_some() {
        db.conn.execute(
            "DELETE FROM favorites WHERE local_file_id = ?1 AND content_id IS NULL",
            params![local_file_id],
        )?;
    } else {
        db.conn.execute(
            "UPDATE favorites SET content_id = ?1, local_file_id = NULL
             WHERE local_file_id = ?2 AND content_id IS NULL",
            params![content_id, local_file_id],
        )?;
    }
    db.conn.execute(
        "UPDATE playlist_tracks SET content_id = ?1
         WHERE local_file_id = ?2 AND content_id IS NULL",
        params![content_id, local_file_id],
    )?;
    db.conn.execute(
        "DELETE FROM play_history WHERE content_id = ?1 AND played_at < (
            SELECT COALESCE(MAX(played_at), '') FROM play_history WHERE local_file_id = ?2 AND content_id IS NULL)",
        params![content_id, local_file_id],
    )?;
    db.conn.execute(
        "UPDATE play_history SET content_id = ?1
         WHERE local_file_id = ?2 AND content_id IS NULL
           AND NOT EXISTS (SELECT 1 FROM play_history other WHERE other.content_id = ?1)",
        params![content_id, local_file_id],
    )?;
    db.conn.execute(
        "DELETE FROM play_history WHERE local_file_id = ?1 AND content_id IS NULL",
        params![local_file_id],
    )?;
    db.conn.execute(
        "UPDATE library_fts SET content_id = ?1 WHERE local_file_id = ?2",
        params![content_id, local_file_id],
    )?;
    Ok(())
}

pub fn refresh_access(db: &Db) -> AppResult<()> {
    let files: Vec<(String, String)> = {
        let mut stmt = db
            .conn
            .prepare("SELECT local_file_id, absolute_path FROM local_files")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, path) in files {
        let status = if Path::new(&path).exists() {
            "available"
        } else {
            "missing"
        };
        let reason = if status == "missing" { Some("file.missing") } else { None };
        db.conn.execute(
            "UPDATE local_files SET access_status = ?1, access_reason = ?2
             WHERE local_file_id = ?3 AND (access_status != ?1 OR access_status IS NULL)",
            params![status, reason, id],
        )?;
    }
    Ok(())
}

pub struct CoverJob {
    pub local_file_id: String,
    pub path: PathBuf,
    pub dest: PathBuf,
}

pub fn next_cover_job(db: &Db) -> AppResult<Option<CoverJob>> {
    Ok(db
        .conn
        .query_row(
            "SELECT local_file_id, absolute_path FROM local_files
             WHERE cover_checked = 0 AND access_status = 'available' AND video = 0
             ORDER BY added_at LIMIT 1",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .ok()
        .map(|(id, path)| CoverJob {
            dest: db.cache_dir.join("covers").join(format!("{id}.jpg")),
            local_file_id: id,
            path: PathBuf::from(path),
        }))
}

/// Decodes the embedded picture and writes a JPEG no larger than 600 px. Runs without the database lock.
pub fn extract_cover(job: &CoverJob) -> Option<String> {
    let tagged = Probe::open(&job.path).and_then(|probe| probe.read()).ok()?;
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
    let picture = tag
        .pictures()
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| tag.pictures().first())?;
    let image = image::load_from_memory(picture.data()).ok()?;
    let (width, height) = (image.width(), image.height());
    let long = width.max(height).max(1);
    let scale = if long > 600 { 600.0 / long as f32 } else { 1.0 };
    let resized = image.resize(
        ((width as f32) * scale).max(1.0) as u32,
        ((height as f32) * scale).max(1.0) as u32,
        image::imageops::FilterType::Triangle,
    );
    let rgb = image::DynamicImage::ImageRgb8(resized.to_rgb8());
    rgb.save_with_format(&job.dest, image::ImageFormat::Jpeg).ok()?;
    Some(job.dest.to_string_lossy().to_string())
}

pub fn finish_cover(db: &Db, local_file_id: &str, cover: Option<&str>) -> AppResult<()> {
    db.conn.execute(
        "UPDATE local_files SET cover_path = ?1, cover_checked = 1 WHERE local_file_id = ?2",
        params![cover, local_file_id],
    )?;
    Ok(())
}
