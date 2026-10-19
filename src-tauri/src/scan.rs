// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
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
    "ape", "wv", "dsf", "dff", "m4b", "mp4", "caf", "tta", "spx", "oga", "audio",
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
    lyrics_stamp: Option<String>,
}

struct Prepared {
    relative: String,
    absolute: PathBuf,
    mtime_ms: i64,
    size: i64,
    tags: Option<TagRead>,
    /// Set when only the lyrics file changed. The song itself is untouched.
    lyrics: Option<String>,
    lyrics_stamp: String,
    /// The directory entry exists, but its size and time could not be read.
    unreadable: bool,
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
        adopt_orphans(&guard, grant_id, path, kind == "directory")?;
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
    guard.conn.execute_batch("BEGIN IMMEDIATE")?;
    let written = (|| {
        let (added, updated) = write_prepared(&guard, grant_id, &prepared, &existing)?;
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
        Ok(ImportSummary { kind: kind.into(), path: path_text, grant_id, added, updated, missing })
    })();
    match written {
        Ok(summary) => match guard.conn.execute_batch("COMMIT") {
            Ok(()) => Ok(summary),
            Err(error) => {
                let _ = guard.conn.execute_batch("ROLLBACK");
                Err(error.into())
            }
        },
        Err(error) => {
            let _ = guard.conn.execute_batch("ROLLBACK");
            Err(error)
        }
    }
}

/// Songs left behind by a removed folder rejoin the folder they lie under. They keep their ids,
/// so favorites, playlists, history, tags and covers are still attached when the files are seen again.
fn adopt_orphans(db: &Db, grant_id: i64, root: &Path, root_is_dir: bool) -> AppResult<()> {
    let orphans: Vec<(String, String)> = {
        let mut stmt = db
            .conn
            .prepare("SELECT local_file_id, absolute_path FROM local_files WHERE grant_id IS NULL")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    if orphans.is_empty() {
        return Ok(());
    }
    let mut taken: HashSet<String> = {
        let mut stmt = db.conn.prepare("SELECT relative_path FROM local_files WHERE grant_id = ?1")?;
        let rows = stmt.query_map(params![grant_id], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<HashSet<_>, _>>()?
    };
    for (local_file_id, absolute) in orphans {
        let file = Path::new(&absolute);
        let relative = if root_is_dir {
            relative_under(root, file)
        } else if same_path(root, file) {
            file.file_name().map(|name| name.to_string_lossy().to_string())
        } else {
            None
        };
        let Some(relative) = relative else { continue };
        if taken.contains(&relative) {
            // The folder already has this file under another id.
            db.delete_local_file_record(&local_file_id)?;
            continue;
        }
        db.conn.execute(
            "UPDATE local_files SET grant_id = ?1, relative_path = ?2 WHERE local_file_id = ?3",
            params![grant_id, relative, local_file_id],
        )?;
        taken.insert(relative);
    }
    Ok(())
}

fn path_parts(path: &Path) -> Vec<String> {
    path.components().map(|part| part.as_os_str().to_string_lossy().to_lowercase()).collect()
}

fn same_path(a: &Path, b: &Path) -> bool {
    path_parts(a) == path_parts(b)
}

/// The part of `file` below `root`. Windows paths ignore letter case.
fn relative_under(root: &Path, file: &Path) -> Option<String> {
    let root_parts = path_parts(root);
    let mut file_parts = file.components();
    for part in &root_parts {
        let next = file_parts.next()?;
        if &next.as_os_str().to_string_lossy().to_lowercase() != part {
            return None;
        }
    }
    let rest: PathBuf = file_parts.collect();
    if rest.as_os_str().is_empty() {
        None
    } else {
        Some(rest.to_string_lossy().to_string())
    }
}

fn existing_rows(db: &Db, grant_id: i64) -> AppResult<HashMap<String, ExistingRow>> {
    let mut stmt = db.conn.prepare(
        "SELECT relative_path, local_file_id, content_id, mtime_ms, size_bytes, exposed, lyrics_stamp
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
                lyrics_stamp: row.get(6)?,
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
    let relative = if root_is_dir {
        file.strip_prefix(root).unwrap_or(file).to_string_lossy().to_string()
    } else {
        file.file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| file.to_string_lossy().to_string())
    };
    // A sharing violation is not "the file is gone". Dropping it here would make
    // the import mark a song that is still on disk as missing.
    let meta = match fs::metadata(file) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(_) => {
            return Some(Prepared {
                relative,
                absolute: file.to_path_buf(),
                mtime_ms: 0,
                size: 0,
                tags: None,
                lyrics: None,
                lyrics_stamp: String::new(),
                unreadable: true,
            });
        }
    };
    let mtime_ms = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|time| time.as_millis() as i64)
        .unwrap_or(0);
    let size = meta.len() as i64;
    let previous = existing.get(&relative);
    let unchanged = previous.is_some_and(|row| row.mtime_ms == Some(mtime_ms) && row.size_bytes == Some(size));
    // The `.lrc` is a separate file, so the song's own time and size say nothing about it.
    let lyrics_stamp = sidecar_stamp(file);
    let lyrics_stale = unchanged && previous.is_some_and(|row| row.lyrics_stamp.as_deref().unwrap_or("") != lyrics_stamp);
    Some(Prepared {
        relative,
        absolute: file.to_path_buf(),
        mtime_ms,
        size,
        tags: if unchanged { None } else { Some(read_tags(file)) },
        lyrics: if lyrics_stale { Some(read_lyrics(file)) } else { None },
        lyrics_stamp,
        unreadable: false,
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
        if item.unreadable {
            if let Some(row) = previous {
                db.conn.execute(
                    "UPDATE local_files SET absolute_path = ?1, access_status = 'unreadable', access_reason = 'file.unreadable'
                     WHERE local_file_id = ?2",
                    params![item.absolute.to_string_lossy(), row.local_file_id],
                )?;
            }
            continue;
        }
        let Some(read) = &item.tags else {
            if let Some(row) = previous {
                db.conn.execute(
                    "UPDATE local_files SET absolute_path = ?1, access_status = 'available', access_reason = NULL
                     WHERE local_file_id = ?2",
                    params![item.absolute.to_string_lossy(), row.local_file_id],
                )?;
                if let Some(lyrics) = &item.lyrics {
                    db.conn.execute(
                        "UPDATE local_files SET lyrics = ?1, lyrics_stamp = ?2 WHERE local_file_id = ?3",
                        params![lyrics, item.lyrics_stamp, row.local_file_id],
                    )?;
                    let (title, artist): (String, String) = db.conn.query_row(
                        "SELECT COALESCE(title, ''), COALESCE(artist, '') FROM local_files WHERE local_file_id = ?1",
                        params![row.local_file_id],
                        |found| Ok((found.get(0)?, found.get(1)?)),
                    )?;
                    let folder = item.absolute.parent().map(|dir| dir.to_string_lossy().to_string()).unwrap_or_default();
                    index_fts(db, &row.local_file_id, row.content_id.as_deref(), &title, &artist, lyrics, &folder)?;
                    updated += 1;
                }
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
                sample_rate, bit_depth, video, cover_path, cover_checked, lyrics_stamp
             ) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,'available',NULL,'untried',NULL,?9,?10,?11,?12,NULL,?13,?14,?15,?16,?17,?18,?19,?20,?21,NULL,0,?22)
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
                cover_checked = 0,
                lyrics_stamp = excluded.lyrics_stamp",
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
                item.lyrics_stamp,
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
    // Directory junctions and symlinks can point at an ancestor. Following them
    // walks the same tree forever, so each real directory is entered once.
    let mut pending = vec![dir.to_path_buf()];
    let mut seen = HashSet::new();
    while let Some(next) = pending.pop() {
        let key = fs::canonicalize(&next).unwrap_or_else(|_| next.clone());
        if !seen.insert(key) {
            continue;
        }
        let Ok(entries) = fs::read_dir(&next) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() && !file_type.is_symlink() && !is_reparse_point(&entry) {
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

/// A Windows directory junction is not reported as a symlink, but it is a reparse point.
#[cfg(windows)]
fn is_reparse_point(entry: &fs::DirEntry) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    entry
        .metadata()
        .map(|meta| meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0)
        .unwrap_or(false)
}

#[cfg(not(windows))]
fn is_reparse_point(_entry: &fs::DirEntry) -> bool {
    false
}

fn is_download_temp(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with(".tunepeerly-download-") || name.ends_with(".part"))
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

/// Turns a stored codec or probe name into an extension this library will scan.
/// A value that is already a scanned extension is kept as it is, so `.mp4` stays `.mp4`.
pub fn file_extension_for_codec(codec: &str) -> String {
    let codec = codec.trim().to_ascii_lowercase();
    if AUDIO_EXT.contains(&codec.as_str()) {
        return codec;
    }
    // Names that older versions stored from the probe instead of from the file name.
    let mapped = match codec.as_str() {
        "mpeg" | "mp2" => "mp3",
        "vorbis" => "ogg",
        "wavpack" => "wv",
        "speex" => "spx",
        _ => "audio",
    };
    mapped.to_string()
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
    // Keep a real filename extension. The probe's type name (mpeg, mp4, vorbis)
    // is not an extension the library scan will accept later.
    let original = extension(path);
    read.codec = if AUDIO_EXT.contains(&original.as_str()) || VIDEO_EXT.contains(&original.as_str()) {
        original
    } else {
        file_extension_for_codec(&format!("{:?}", tagged.file_type()).to_ascii_lowercase())
    };
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

fn sidecar_path(path: &Path) -> PathBuf {
    let mut lrc = path.to_path_buf();
    lrc.set_extension("lrc");
    lrc
}

fn sidecar_lyrics(path: &Path) -> String {
    let Ok(bytes) = fs::read(sidecar_path(path)) else { return String::new() };
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(&bytes);
    String::from_utf8(bytes.to_vec()).unwrap_or_default()
}

/// Modified time and size of the `.lrc` next to a song. Empty when there is none.
fn sidecar_stamp(path: &Path) -> String {
    let Ok(meta) = fs::metadata(sidecar_path(path)) else { return String::new() };
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|time| time.as_millis())
        .unwrap_or(0);
    format!("{mtime}:{}", meta.len())
}

/// The `.lrc` next to the song, or else the lyrics inside its tag.
fn read_lyrics(path: &Path) -> String {
    let side = sidecar_lyrics(path);
    if !side.is_empty() {
        return side;
    }
    Probe::open(path)
        .and_then(|probe| probe.read())
        .ok()
        .and_then(|tagged| {
            let tag = tagged.primary_tag().or_else(|| tagged.first_tag())?;
            tag.get_string(&ItemKey::Lyrics).map(|text| text.to_string())
        })
        .unwrap_or_default()
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
    mp4_has_video_track(&mut file).unwrap_or(false)
}

/// Walks the top-level boxes to `moov` and looks at each track's handler type.
/// Text inside tags or audio data can never be mistaken for a video track this way.
fn mp4_has_video_track(file: &mut File) -> Option<bool> {
    use std::io::{Seek as _, SeekFrom};
    const MOOV_LIMIT: u64 = 16 * 1024 * 1024;
    let total = file.metadata().ok()?.len();
    let mut pos = 0u64;
    for _ in 0..4096 {
        if pos + 8 > total {
            return None;
        }
        file.seek(SeekFrom::Start(pos)).ok()?;
        let mut head = [0u8; 16];
        file.read_exact(&mut head[..8]).ok()?;
        let mut size = u32::from_be_bytes(head[..4].try_into().ok()?) as u64;
        let mut header = 8u64;
        if size == 1 {
            file.read_exact(&mut head[8..16]).ok()?;
            size = u64::from_be_bytes(head[8..16].try_into().ok()?);
            header = 16;
        } else if size == 0 {
            size = total - pos;
        }
        if size < header {
            return None;
        }
        if &head[4..8] == b"moov" {
            let len = size - header;
            if len > MOOV_LIMIT || pos + size > total {
                return None;
            }
            let mut buf = vec![0u8; len as usize];
            file.read_exact(&mut buf).ok()?;
            return Some(moov_has_video(&buf));
        }
        pos += size;
    }
    None
}

fn mp4_boxes(buf: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let mut out = Vec::new();
    let mut at = 0usize;
    while at + 8 <= buf.len() {
        let size = u32::from_be_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]) as usize;
        let kind = [buf[at + 4], buf[at + 5], buf[at + 6], buf[at + 7]];
        if size < 8 || at + size > buf.len() {
            break;
        }
        out.push((kind, &buf[at + 8..at + size]));
        at += size;
    }
    out
}

fn moov_has_video(moov: &[u8]) -> bool {
    mp4_boxes(moov)
        .into_iter()
        .filter(|(kind, _)| kind == b"trak")
        .flat_map(|(_, trak)| mp4_boxes(trak))
        .filter(|(kind, _)| kind == b"mdia")
        .flat_map(|(_, mdia)| mp4_boxes(mdia))
        .filter(|(kind, _)| kind == b"hdlr")
        // version and flags, a reserved word, then the four-letter handler type
        .any(|(_, hdlr)| hdlr.len() >= 12 && &hdlr[8..12] == b"vide")
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
            // Songs of a removed folder stay unavailable, whether or not their files still exist.
            .prepare("SELECT local_file_id, absolute_path FROM local_files WHERE grant_id IS NOT NULL")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    for (id, path) in files {
        // `Path::exists` is false both when a file is gone and when it cannot be checked.
        let (status, reason) = match fs::metadata(&path) {
            Ok(_) => ("available", None),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => ("missing", Some("file.missing")),
            Err(_) => ("unreadable", Some("file.unreadable")),
        };
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("tunepeerly-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn open_db() -> (Arc<Mutex<Db>>, PathBuf) {
        let root = temp("db");
        let db = Db::open(&root.join("library.db"), root.clone(), root.join("cache")).unwrap();
        (Arc::new(Mutex::new(db)), root)
    }

    fn lyrics_of(db: &Arc<Mutex<Db>>) -> (String, String, String) {
        db.lock()
            .unwrap()
            .conn
            .query_row(
                "SELECT local_file_id, COALESCE(lyrics, ''), access_status FROM local_files",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap()
    }

    #[test]
    fn removed_folder_keeps_songs_and_readding_restores_them() {
        let (db, _keep) = open_db();
        let music = temp("music");
        fs::write(music.join("a.mp3"), b"not really audio").unwrap();
        let first = import_path(&db, &music, "directory").unwrap();
        let (id, _, status) = lyrics_of(&db);
        assert_eq!(status, "available");

        db.lock().unwrap().delete_grant(first.grant_id, true).unwrap();
        let (kept, _, status) = lyrics_of(&db);
        assert_eq!(kept, id);
        assert_eq!(status, "missing");
        scan_refresh(&db);
        assert_eq!(lyrics_of(&db).2, "missing");

        import_path(&db, &music, "directory").unwrap();
        let (again, _, status) = lyrics_of(&db);
        assert_eq!(again, id);
        assert_eq!(status, "available");
    }

    #[test]
    fn lyrics_added_later_are_picked_up_by_a_rescan() {
        let (db, _keep) = open_db();
        let music = temp("lyrics");
        fs::write(music.join("a.mp3"), b"not really audio").unwrap();
        import_path(&db, &music, "directory").unwrap();
        assert_eq!(lyrics_of(&db).1, "");

        fs::write(music.join("a.lrc"), "[00:01.00]hello").unwrap();
        import_path(&db, &music, "directory").unwrap();
        assert_eq!(lyrics_of(&db).1, "[00:01.00]hello");

        fs::write(music.join("a.lrc"), "[00:01.00]hello again").unwrap();
        import_path(&db, &music, "directory").unwrap();
        assert_eq!(lyrics_of(&db).1, "[00:01.00]hello again");

        fs::remove_file(music.join("a.lrc")).unwrap();
        import_path(&db, &music, "directory").unwrap();
        assert_eq!(lyrics_of(&db).1, "");
    }

    fn scan_refresh(db: &Arc<Mutex<Db>>) {
        refresh_access(&db.lock().unwrap()).unwrap();
    }
}
