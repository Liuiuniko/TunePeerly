use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::sync_logic::{self, HistoryEntry, MemberRef, QueueItem};

pub const FEATURES: [&str; 6] = [
    "favorites",
    "theme",
    "language",
    "playlists",
    "queue",
    "history",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackStub {
    pub local_file_id: Option<String>,
    pub content_id: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub folder: String,
    pub file_name: String,
    pub added_at: String,
    pub favorite: bool,
    pub exposed: bool,
    pub badge: String,
    pub access_status: String,
    pub metadata_status: String,
    pub playback_status: String,
    pub reason_key: Option<String>,
    pub video: bool,
    pub cover_url: Option<String>,
    pub absolute_path: Option<String>,
    pub holder_ids: Vec<String>,
    pub playable: bool,
    pub dimmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantRow {
    pub id: i64,
    pub kind: String,
    pub path: String,
    pub exposure: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistRow {
    pub id: String,
    pub name: String,
    pub exposure: String,
    pub count: i64,
}

pub struct Db {
    pub conn: Connection,
    pub app_dir: PathBuf,
    pub cache_dir: PathBuf,
}

impl Db {
    pub fn open(database: &Path, app_dir: PathBuf, cache_dir: PathBuf) -> AppResult<Self> {
        if let Some(parent) = database.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::create_dir_all(cache_dir.join("covers"))?;
        let conn = Connection::open(database)?;
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let db = Self {
            conn,
            app_dir,
            cache_dir,
        };
        db.migrate()?;
        db.seed()?;
        Ok(db)
    }

    fn migrate(&self) -> AppResult<()> {
        self.conn.execute_batch(SCHEMA)?;
        self.add_column_if_missing("local_files", "cover_checked", "INTEGER NOT NULL DEFAULT 0")?;
        Ok(())
    }

    fn add_column_if_missing(&self, table: &str, column: &str, definition: &str) -> AppResult<()> {
        let mut stmt = self.conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<Result<Vec<_>, _>>()?;
        if !names.iter().any(|name| name == column) {
            self.conn
                .execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"))?;
        }
        Ok(())
    }

    fn seed(&self) -> AppResult<()> {
        for (key, value) in device_defaults() {
            self.conn.execute(
                "INSERT INTO device_settings(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO NOTHING",
                params![key, value.to_string()],
            )?;
        }
        for (key, value) in [("theme", "light"), ("language", "en")] {
            self.conn.execute(
                "INSERT INTO synced_settings(key, value) VALUES(?1, ?2)
                 ON CONFLICT(key) DO NOTHING",
                params![key, value],
            )?;
        }
        for feature in FEATURES {
            self.conn.execute(
                "INSERT INTO feature_revisions(feature, revision) VALUES(?1, 0)
                 ON CONFLICT(feature) DO NOTHING",
                params![feature],
            )?;
        }
        let name = self.setting_string("displayName")?;
        if name.is_empty() {
            let host = whoami::fallible::hostname().unwrap_or_else(|_| "This computer".into());
            self.set_device_setting("displayName", json!(host))?;
        }
        Ok(())
    }

    pub fn setting_raw(&self, key: &str) -> AppResult<String> {
        self.conn
            .query_row(
                "SELECT value FROM device_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::Message(format!("Missing setting {key}")))
    }

    pub fn setting_string(&self, key: &str) -> AppResult<String> {
        let raw = self.setting_raw(key)?;
        Ok(serde_json::from_str::<String>(&raw).unwrap_or(raw))
    }

    pub fn setting_bool(&self, key: &str) -> AppResult<bool> {
        Ok(serde_json::from_str(&self.setting_raw(key)?).unwrap_or(false))
    }

    pub fn setting_i64(&self, key: &str) -> AppResult<i64> {
        Ok(serde_json::from_str(&self.setting_raw(key)?).unwrap_or(0))
    }

    pub fn setting_json(&self, key: &str) -> AppResult<Value> {
        Ok(serde_json::from_str(&self.setting_raw(key)?).unwrap_or(Value::Null))
    }

    pub fn set_device_setting(&self, key: &str, value: Value) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO device_settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )?;
        Ok(())
    }

    pub fn synced(&self, key: &str) -> AppResult<String> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM synced_settings WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .unwrap_or_default())
    }

    pub fn set_synced(&self, key: &str, value: &str) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO synced_settings(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn device_settings_map(&self) -> AppResult<Value> {
        let mut out = serde_json::Map::new();
        let mut stmt = self.conn.prepare("SELECT key, value FROM device_settings")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (key, value) = row?;
            out.insert(key, serde_json::from_str(&value).unwrap_or(Value::String(value)));
        }
        Ok(Value::Object(out))
    }

    pub fn signal_blob(&self, key: &str) -> AppResult<Option<Vec<u8>>> {
        Ok(self
            .conn
            .query_row(
                "SELECT value FROM signal_state WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn set_signal_blob(&self, key: &str, value: &[u8]) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO signal_state(key, value) VALUES(?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn grants(&self) -> AppResult<Vec<GrantRow>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, kind, path, exposure FROM grants ORDER BY path")?;
        let rows = stmt.query_map([], |row| {
            Ok(GrantRow {
                id: row.get(0)?,
                kind: row.get(1)?,
                path: row.get(2)?,
                exposure: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn insert_grant(&self, kind: &str, path: &str) -> AppResult<i64> {
        self.conn.execute(
            "INSERT INTO grants(kind, path, exposure) VALUES(?1, ?2, 'follow')
             ON CONFLICT(path) DO UPDATE SET kind = excluded.kind",
            params![kind, path],
        )?;
        let id = self.conn.query_row(
            "SELECT id FROM grants WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn delete_grant(&self, id: i64) -> AppResult<Option<GrantRow>> {
        let grant = self
            .conn
            .query_row(
                "SELECT id, kind, path, exposure FROM grants WHERE id = ?1",
                params![id],
                |row| {
                    Ok(GrantRow {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        path: row.get(2)?,
                        exposure: row.get(3)?,
                    })
                },
            )
            .optional()?;
        if grant.is_some() {
            let ids: Vec<String> = {
                let mut stmt = self
                    .conn
                    .prepare("SELECT local_file_id FROM local_files WHERE grant_id = ?1")?;
                let rows = stmt.query_map(params![id], |row| row.get(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            for file_id in ids {
                self.delete_local_file_record(&file_id)?;
            }
            self.conn
                .execute("DELETE FROM grants WHERE id = ?1", params![id])?;
        }
        Ok(grant)
    }

    fn delete_local_file_record(&self, local_file_id: &str) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM favorites WHERE local_file_id = ?1",
            params![local_file_id],
        )?;
        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE local_file_id = ?1",
            params![local_file_id],
        )?;
        self.conn.execute(
            "DELETE FROM library_fts WHERE local_file_id = ?1",
            params![local_file_id],
        )?;
        self.conn.execute(
            "DELETE FROM local_files WHERE local_file_id = ?1",
            params![local_file_id],
        )?;
        Ok(())
    }

    pub fn feature_revision(&self, feature: &str) -> AppResult<i64> {
        Ok(self
            .conn
            .query_row(
                "SELECT revision FROM feature_revisions WHERE feature = ?1",
                params![feature],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    pub fn bump_feature_revision(&self, feature: &str) -> AppResult<i64> {
        let next = self.feature_revision(feature)? + 1;
        self.conn.execute(
            "INSERT INTO feature_revisions(feature, revision) VALUES(?1, ?2)
             ON CONFLICT(feature) DO UPDATE SET revision = excluded.revision",
            params![feature, next],
        )?;
        Ok(next)
    }

    pub fn mark_dirty(&self, feature: &str, dirty: bool) -> AppResult<()> {
        let mut map = self.setting_json("dirty")?;
        if !map.is_object() {
            map = json!({});
        }
        map[feature] = json!(dirty);
        self.set_device_setting("dirty", map)
    }

    pub fn is_dirty(&self, feature: &str) -> AppResult<bool> {
        Ok(self
            .setting_json("dirty")?
            .get(feature)
            .and_then(|value| value.as_bool())
            .unwrap_or(false))
    }

    pub fn block_flags(&self, feature: &str) -> AppResult<BlockFlags> {
        let all = self.setting_json("syncBlocks")?;
        let item = all.get(feature).cloned().unwrap_or(json!({}));
        Ok(BlockFlags {
            manual: item.get("manual").and_then(|v| v.as_bool()).unwrap_or(false),
            source: item.get("source").and_then(|v| v.as_bool()).unwrap_or(false),
            timestamp: item.get("timestamp").and_then(|v| v.as_bool()).unwrap_or(false),
            all: item.get("all").and_then(|v| v.as_bool()).unwrap_or(false),
        })
    }

    pub fn sync_source(&self, feature: &str) -> AppResult<Option<String>> {
        let map = self.setting_json("syncSources")?;
        Ok(map
            .get(feature)
            .and_then(|value| value.as_str())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string()))
    }

    pub fn exposure_for_new_folder_file(&self, grant_id: i64) -> AppResult<bool> {
        let exposure: String = self.conn.query_row(
            "SELECT exposure FROM grants WHERE id = ?1",
            params![grant_id],
            |row| row.get(0),
        )?;
        self.resolve_exposure(&exposure)
    }

    pub fn resolve_exposure(&self, exposure: &str) -> AppResult<bool> {
        let mode = if exposure == "follow" {
            self.setting_string("exposeDefault")?
        } else {
            exposure.to_string()
        };
        Ok(mode == "expose")
    }

    pub fn set_grant_exposure(&self, id: i64, exposure: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE grants SET exposure = ?1 WHERE id = ?2",
            params![exposure, id],
        )?;
        Ok(())
    }

    pub fn expose_grant_members(&self, id: i64, expose: bool) -> AppResult<()> {
        self.conn.execute(
            "UPDATE local_files SET exposed = ?1 WHERE grant_id = ?2",
            params![expose, id],
        )?;
        Ok(())
    }

    pub fn playlists(&self) -> AppResult<Vec<PlaylistRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT p.id, p.name, p.exposure,
                    (SELECT COUNT(*) FROM playlist_tracks t WHERE t.playlist_id = p.id)
             FROM playlists p ORDER BY p.position, p.name",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PlaylistRow {
                id: row.get(0)?,
                name: row.get(1)?,
                exposure: row.get(2)?,
                count: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn create_playlist(&self, name: &str) -> AppResult<String> {
        let id = Uuid::new_v4().to_string();
        let position: i64 = self
            .conn
            .query_row("SELECT COALESCE(MAX(position), 0) + 1 FROM playlists", [], |row| {
                row.get(0)
            })?;
        self.conn.execute(
            "INSERT INTO playlists(id, name, position, exposure, revision) VALUES(?1, ?2, ?3, 'follow', 0)",
            params![id, name, position],
        )?;
        Ok(id)
    }

    pub fn rename_playlist(&self, id: &str, name: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE playlists SET name = ?1 WHERE id = ?2",
            params![name, id],
        )?;
        Ok(())
    }

    pub fn delete_playlist(&self, id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id])?;
        self.conn
            .execute("DELETE FROM playlists WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn add_to_playlist(
        &self,
        playlist_id: &str,
        content_id: Option<&str>,
        local_file_id: Option<&str>,
    ) -> AppResult<()> {
        let already: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM playlist_tracks
             WHERE playlist_id = ?1 AND ((?2 IS NOT NULL AND content_id = ?2) OR (?3 IS NOT NULL AND local_file_id = ?3))",
            params![playlist_id, content_id, local_file_id],
            |row| row.get(0),
        )?;
        if already > 0 {
            return Err(crate::error::AppError::Message("playlist.already".into()));
        }
        let position: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(position), 0) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
            params![playlist_id],
            |row| row.get(0),
        )?;
        self.conn.execute(
            "INSERT INTO playlist_tracks(playlist_id, position, content_id, local_file_id)
             VALUES(?1, ?2, ?3, ?4)",
            params![playlist_id, position, content_id, local_file_id],
        )?;
        if let Some(local_file_id) = local_file_id {
            let exposure: String = self.conn.query_row(
                "SELECT exposure FROM playlists WHERE id = ?1",
                params![playlist_id],
                |row| row.get(0),
            )?;
            let exposed = self.resolve_exposure(&exposure)?;
            self.conn.execute(
                "UPDATE local_files SET exposed = ?1 WHERE local_file_id = ?2",
                params![exposed, local_file_id],
            )?;
        }
        Ok(())
    }

    /// Moves one member to `to` (0-based) and renumbers the list.
    pub fn move_member(&self, playlist_id: &str, from: usize, to: usize) -> AppResult<()> {
        let rows: Vec<(Option<String>, Option<String>)> = {
            let mut stmt = self.conn.prepare(
                "SELECT content_id, local_file_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position",
            )?;
            let rows = stmt.query_map(params![playlist_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        if from >= rows.len() || to >= rows.len() || from == to {
            return Ok(());
        }
        let mut rows = rows;
        let item = rows.remove(from);
        rows.insert(to, item);
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![playlist_id])?;
        for (index, (content_id, local_file_id)) in rows.iter().enumerate() {
            tx.execute(
                "INSERT INTO playlist_tracks(playlist_id, position, content_id, local_file_id) VALUES(?1, ?2, ?3, ?4)",
                params![playlist_id, index as i64 + 1, content_id, local_file_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn remove_member(
        &self,
        playlist_id: &str,
        content_id: Option<&str>,
        local_file_id: Option<&str>,
    ) -> AppResult<()> {
        if let Some(content_id) = content_id {
            self.conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND content_id = ?2",
                params![playlist_id, content_id],
            )?;
        }
        if let Some(local_file_id) = local_file_id {
            self.conn.execute(
                "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND local_file_id = ?2",
                params![playlist_id, local_file_id],
            )?;
        }
        Ok(())
    }

    pub fn set_playlist_exposure(&self, id: &str, exposure: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE playlists SET exposure = ?1 WHERE id = ?2",
            params![exposure, id],
        )?;
        Ok(())
    }

    pub fn expose_playlist_members(&self, id: &str, expose: bool) -> AppResult<()> {
        let ids = self.playlist_local_ids(id)?;
        for local_file_id in ids {
            self.conn.execute(
                "UPDATE local_files SET exposed = ?1 WHERE local_file_id = ?2",
                params![expose, local_file_id],
            )?;
        }
        Ok(())
    }

    fn playlist_local_ids(&self, id: &str) -> AppResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT local_file_id FROM playlist_tracks
             WHERE playlist_id = ?1 AND local_file_id IS NOT NULL",
        )?;
        let rows = stmt.query_map(params![id], |row| row.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn set_track_exposure(&self, local_file_id: &str, expose: bool) -> AppResult<()> {
        self.conn.execute(
            "UPDATE local_files SET exposed = ?1 WHERE local_file_id = ?2",
            params![expose, local_file_id],
        )?;
        Ok(())
    }

    pub fn toggle_favorite(
        &self,
        content_id: Option<&str>,
        local_file_id: Option<&str>,
    ) -> AppResult<bool> {
        let existing: Option<i64> = if let Some(content_id) = content_id {
            self.conn
                .query_row(
                    "SELECT id FROM favorites WHERE content_id = ?1",
                    params![content_id],
                    |row| row.get(0),
                )
                .optional()?
        } else if let Some(local_file_id) = local_file_id {
            self.conn
                .query_row(
                    "SELECT id FROM favorites WHERE local_file_id = ?1",
                    params![local_file_id],
                    |row| row.get(0),
                )
                .optional()?
        } else {
            None
        };
        if let Some(id) = existing {
            self.conn
                .execute("DELETE FROM favorites WHERE id = ?1", params![id])?;
            Ok(false)
        } else {
            self.conn.execute(
                "INSERT INTO favorites(content_id, local_file_id) VALUES(?1, ?2)",
                params![content_id, local_file_id],
            )?;
            Ok(true)
        }
    }

    pub fn is_favorite(&self, content_id: Option<&str>, local_file_id: Option<&str>) -> AppResult<bool> {
        let count: i64 = if let Some(content_id) = content_id {
            self.conn.query_row(
                "SELECT COUNT(*) FROM favorites WHERE content_id = ?1",
                params![content_id],
                |row| row.get(0),
            )?
        } else if let Some(local_file_id) = local_file_id {
            self.conn.query_row(
                "SELECT COUNT(*) FROM favorites WHERE local_file_id = ?1",
                params![local_file_id],
                |row| row.get(0),
            )?
        } else {
            0
        };
        Ok(count > 0)
    }

    pub fn favorite_content_ids(&self) -> AppResult<Vec<String>> {
        let mut stmt = self
            .conn
            .prepare("SELECT content_id FROM favorites WHERE content_id IS NOT NULL")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn our_holdings_revision(&self) -> AppResult<i64> {
        self.setting_i64("holdingsRevision")
    }

    pub fn set_holdings_revision(&self, revision: i64) -> AppResult<()> {
        self.set_device_setting("holdingsRevision", json!(revision))
    }

    pub fn replace_peer_holdings(
        &self,
        device_id: &str,
        revision: i64,
        rows: &[HoldingInput],
    ) -> AppResult<bool> {
        let applied: i64 = self
            .conn
            .query_row(
                "SELECT revision FROM holding_revisions WHERE device_id = ?1",
                params![device_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0);
        if revision <= applied {
            return Ok(false);
        }
        self.conn.execute(
            "DELETE FROM holdings WHERE device_id = ?1",
            params![device_id],
        )?;
        for row in rows {
            self.conn.execute(
                "INSERT INTO holdings(device_id, revision, content_id, title, artist, album, track_number, duration_ms, codec)
                 VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    device_id,
                    revision,
                    row.content_id,
                    row.title,
                    row.artist,
                    row.album,
                    row.track_number,
                    row.duration_ms,
                    row.codec
                ],
            )?;
        }
        self.conn.execute(
            "INSERT INTO holding_revisions(device_id, revision) VALUES(?1, ?2)
             ON CONFLICT(device_id) DO UPDATE SET revision = excluded.revision",
            params![device_id, revision],
        )?;
        Ok(true)
    }

    pub fn delete_peer_holdings(&self, device_id: &str) -> AppResult<()> {
        self.conn
            .execute("DELETE FROM holdings WHERE device_id = ?1", params![device_id])?;
        self.conn.execute(
            "DELETE FROM holding_revisions WHERE device_id = ?1",
            params![device_id],
        )?;
        Ok(())
    }

    pub fn record_history_local(&self, content_id: Option<&str>, local_file_id: Option<&str>) -> AppResult<()> {
        if self.setting_bool("incognito")? {
            return Ok(());
        }
        let now = Utc::now().to_rfc3339();
        if let Some(content_id) = content_id {
            self.conn.execute(
                "DELETE FROM play_history WHERE content_id = ?1",
                params![content_id],
            )?;
            self.conn.execute(
                "INSERT INTO play_history(content_id, local_file_id, played_at) VALUES(?1, NULL, ?2)",
                params![content_id, now],
            )?;
        } else if let Some(local_file_id) = local_file_id {
            self.conn.execute(
                "DELETE FROM play_history WHERE local_file_id = ?1 AND content_id IS NULL",
                params![local_file_id],
            )?;
            self.conn.execute(
                "INSERT INTO play_history(content_id, local_file_id, played_at) VALUES(NULL, ?1, ?2)",
                params![local_file_id, now],
            )?;
        }
        self.trim_history()?;
        Ok(())
    }

    pub fn trim_history(&self) -> AppResult<()> {
        let cap = self.setting_i64("historyCap")?.max(1);
        self.conn.execute(
            "DELETE FROM play_history WHERE id NOT IN (
                SELECT id FROM play_history ORDER BY played_at DESC LIMIT ?1
             )",
            params![cap],
        )?;
        Ok(())
    }

    pub fn history_for_sync(&self) -> AppResult<Vec<HistoryEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT content_id, played_at FROM play_history
             WHERE content_id IS NOT NULL ORDER BY played_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (content_id, played_at) = row?;
            if let Ok(at) = DateTime::parse_from_rfc3339(&played_at) {
                out.push(HistoryEntry {
                    content_id,
                    at: at.with_timezone(&Utc),
                });
            }
        }
        Ok(out)
    }

    pub fn replace_synced_history(&self, entries: &[HistoryEntry]) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM play_history WHERE content_id IS NOT NULL",
            [],
        )?;
        for entry in entries {
            self.conn.execute(
                "INSERT INTO play_history(content_id, local_file_id, played_at) VALUES(?1, NULL, ?2)",
                params![entry.content_id, entry.at.to_rfc3339()],
            )?;
        }
        Ok(())
    }

    pub fn player_json(&self) -> AppResult<Value> {
        let text: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM player_state WHERE key = 'state'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(text
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .map(|value| match value.get("queue") {
                Some(queue) if queue.is_object() => {
                    let mut flat = queue.clone();
                    if let Some(position) = value.get("positionSec") {
                        flat["positionSec"] = position.clone();
                    }
                    flat
                }
                _ => value,
            })
            .unwrap_or_else(|| json!({
                "order": "manual",
                "repeat": "off",
                "past": [],
                "current": null,
                "future": [],
                "positionSec": 0,
                "contextId": "library"
            })))
    }

    pub fn save_player_json(&self, value: &Value) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO player_state(key, value) VALUES('state', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![value.to_string()],
        )?;
        Ok(())
    }

    pub fn save_progress(&self, position_sec: f64) -> AppResult<()> {
        let mut state = self.player_json()?;
        state["positionSec"] = json!(position_sec);
        self.save_player_json(&state)
    }

    pub fn queue_items_from_state(&self) -> AppResult<Vec<QueueItem>> {
        let state = self.player_json()?;
        Ok(parse_queue_items(state.get("future")))
    }

    pub fn current_content_from_state(&self) -> AppResult<Option<String>> {
        let state = self.player_json()?;
        Ok(state
            .get("current")
            .and_then(|item| item.get("contentId"))
            .and_then(|value| value.as_str())
            .map(str::to_string))
    }

    pub fn history_keys(&self) -> AppResult<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(content_id, local_file_id) FROM play_history
             WHERE COALESCE(content_id, local_file_id) IS NOT NULL
             ORDER BY played_at DESC",
        )?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn applied_revision(&self, feature: &str, source_device_id: &str) -> AppResult<i64> {
        Ok(self
            .conn
            .query_row(
                "SELECT revision FROM snapshots WHERE feature = ?1 AND source_device_id = ?2",
                params![feature, source_device_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or(0))
    }

    pub fn record_snapshot(&self, feature: &str, source_device_id: &str, revision: i64, payload: &str) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO snapshots(feature, source_device_id, revision, payload, signature)
             VALUES(?1, ?2, ?3, ?4, x'')
             ON CONFLICT(feature, source_device_id) DO UPDATE SET revision = excluded.revision, payload = excluded.payload",
            params![feature, source_device_id, revision, payload],
        )?;
        Ok(())
    }

    pub fn playlist_snapshot(&self) -> AppResult<Value> {
        let lists = self.playlists()?;
        let mut out = Vec::new();
        for list in lists {
            let members = self
                .content_member_refs("playlist", Some(&list.id))?
                .into_iter()
                .filter_map(|member| member.content_id)
                .collect::<Vec<_>>();
            out.push(json!({"id": list.id, "name": list.name, "members": members}));
        }
        Ok(json!(out))
    }

    /// Replaces playlists with the snapshot. Unhashed local rows and this computer's exposure defaults stay.
    pub fn apply_playlist_snapshot(&self, items: &Value) -> AppResult<()> {
        let Some(lists) = items.as_array() else { return Ok(()) };
        let incoming: Vec<(String, String, Vec<String>)> = lists
            .iter()
            .filter_map(|item| {
                let id = item.get("id")?.as_str()?.to_string();
                let name = item.get("name")?.as_str()?.to_string();
                let members = item
                    .get("members")?
                    .as_array()?
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect();
                Some((id, name, members))
            })
            .collect();
        let keep: Vec<&str> = incoming.iter().map(|(id, _, _)| id.as_str()).collect();
        for existing in self.playlists()? {
            if !keep.contains(&existing.id.as_str()) {
                self.delete_playlist(&existing.id)?;
            }
        }
        for (position, (id, name, members)) in incoming.iter().enumerate() {
            self.conn.execute(
                "INSERT INTO playlists(id, name, position, exposure, revision) VALUES(?1, ?2, ?3, 'follow', 0)
                 ON CONFLICT(id) DO UPDATE SET name = excluded.name, position = excluded.position",
                params![id, name, position as i64 + 1],
            )?;
            let existing = self.content_member_refs("playlist", Some(id))?;
            let before: Vec<String> = existing.iter().filter_map(|member| member.content_id.clone()).collect();
            let kept = sync_logic::apply_member_snapshot(&existing, members);
            self.conn
                .execute("DELETE FROM playlist_tracks WHERE playlist_id = ?1", params![id])?;
            for (index, member) in kept.iter().enumerate() {
                self.conn.execute(
                    "INSERT INTO playlist_tracks(playlist_id, position, content_id, local_file_id) VALUES(?1, ?2, ?3, ?4)",
                    params![id, index as i64 + 1, member.content_id, member.local_file_id],
                )?;
            }
            let exposure: String = self.conn.query_row(
                "SELECT exposure FROM playlists WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )?;
            let exposed = self.resolve_exposure(&exposure)?;
            for content_id in members.iter().filter(|content_id| !before.contains(content_id)) {
                self.conn.execute(
                    "UPDATE local_files SET exposed = ?1 WHERE content_id = ?2",
                    params![exposed, content_id],
                )?;
            }
        }
        Ok(())
    }

    pub fn content_member_refs(&self, table: &str, owner: Option<&str>) -> AppResult<Vec<MemberRef>> {
        let sql = if table == "favorites" {
            "SELECT content_id, local_file_id FROM favorites".to_string()
        } else {
            "SELECT content_id, local_file_id FROM playlist_tracks WHERE playlist_id = ?1 ORDER BY position"
                .to_string()
        };
        let mut stmt = self.conn.prepare(&sql)?;
        let map = |row: &rusqlite::Row<'_>| {
            Ok(MemberRef {
                content_id: row.get(0)?,
                local_file_id: row.get(1)?,
            })
        };
        let rows = if let Some(owner) = owner {
            stmt.query_map(params![owner], map)?
                .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map([], map)?.collect::<Result<Vec<_>, _>>()?
        };
        Ok(rows)
    }

    pub fn note_failure(
        &self,
        source_device_id: Option<&str>,
        content_id: Option<&str>,
        local_file_id: Option<&str>,
        class_name: &str,
        reason: &str,
        attempt_id: &str,
    ) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO playback_notes(source_device_id, content_id, local_file_id, class, reason, attempt_id)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)",
            params![source_device_id, content_id, local_file_id, class_name, reason, attempt_id],
        )?;
        if matches!(class_name, "decode" | "unsupported") {
            if let Some(content_id) = content_id {
                self.conn.execute(
                    "UPDATE content_items SET unplayable = 1, unplayable_reason = ?1 WHERE content_id = ?2",
                    params![reason, content_id],
                )?;
            }
        }
        Ok(())
    }

    pub fn clear_transport_notes(&self, attempt_id: &str) -> AppResult<()> {
        self.conn.execute(
            "DELETE FROM playback_notes WHERE class = 'transport' AND attempt_id = ?1",
            params![attempt_id],
        )?;
        Ok(())
    }

    pub fn clear_unplayable(&self, content_id: &str) -> AppResult<()> {
        self.conn.execute(
            "UPDATE content_items SET unplayable = 0, unplayable_reason = NULL WHERE content_id = ?1",
            params![content_id],
        )?;
        self.conn.execute(
            "DELETE FROM playback_notes WHERE content_id = ?1 AND class IN ('decode', 'unsupported')",
            params![content_id],
        )?;
        Ok(())
    }

    pub fn content_unplayable(&self, content_id: &str) -> AppResult<bool> {
        Ok(self
            .conn
            .query_row(
                "SELECT unplayable FROM content_items WHERE content_id = ?1",
                params![content_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or(0)
            == 1)
    }

    /// Returns each matching local file with a lyrics excerpt when the lyrics column matched.
    pub fn search_hits(&self, query: &str, limit: i64) -> AppResult<std::collections::HashMap<String, Option<String>>> {
        let match_query = fts_query(query);
        if match_query.is_empty() {
            return Ok(Default::default());
        }
        let mut stmt = self.conn.prepare(
            "SELECT local_file_id, snippet(library_fts, 4, char(2), char(3), '…', 10)
             FROM library_fts WHERE library_fts MATCH ?1 LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![match_query, limit], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
        })?;
        let mut out = std::collections::HashMap::new();
        for row in rows {
            let (id, snippet) = row?;
            out.insert(id, snippet.filter(|text| text.contains('\u{2}')));
        }
        Ok(out)
    }

    pub fn lyrics(&self, local_file_id: &str) -> AppResult<String> {
        Ok(self
            .conn
            .query_row(
                "SELECT COALESCE(lyrics, '') FROM local_files WHERE local_file_id = ?1",
                params![local_file_id],
                |row| row.get(0),
            )
            .optional()?
            .unwrap_or_default())
    }

    pub fn peers(&self) -> AppResult<Vec<PeerRow>> {
        let mut stmt = self.conn.prepare(
            "SELECT device_id, identity_public, cert_der, cert_id, display_name, safety_number FROM peers",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PeerRow {
                device_id: row.get(0)?,
                identity_public: row.get(1)?,
                cert_der: row.get(2)?,
                cert_id: row.get(3)?,
                display_name: row.get(4)?,
                safety_number: row.get(5)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn upsert_peer(&self, peer: &PeerRow) -> AppResult<()> {
        self.conn.execute(
            "INSERT INTO peers(device_id, identity_public, cert_der, cert_id, display_name, safety_number)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(device_id) DO UPDATE SET
               identity_public = excluded.identity_public,
               cert_der = excluded.cert_der,
               cert_id = excluded.cert_id,
               display_name = excluded.display_name,
               safety_number = excluded.safety_number",
            params![
                peer.device_id,
                peer.identity_public,
                peer.cert_der,
                peer.cert_id,
                peer.display_name,
                peer.safety_number
            ],
        )?;
        Ok(())
    }

    pub fn rename_peer(&self, device_id: &str, name: &str) -> AppResult<bool> {
        let changed = self.conn.execute(
            "UPDATE peers SET display_name = ?2 WHERE device_id = ?1 AND display_name <> ?2",
            params![device_id, name],
        )?;
        Ok(changed > 0)
    }

    pub fn remove_peer(&self, device_id: &str) -> AppResult<()> {
        self.delete_peer_holdings(device_id)?;
        self.conn
            .execute("DELETE FROM peers WHERE device_id = ?1", params![device_id])?;
        self.conn.execute(
            "DELETE FROM peer_list_prefs WHERE device_id = ?1",
            params![device_id],
        )?;
        let mut sources = self.setting_json("syncSources")?;
        if let Some(map) = sources.as_object_mut() {
            for value in map.values_mut() {
                if value.as_str() == Some(device_id) {
                    *value = Value::Null;
                }
            }
        }
        self.set_device_setting("syncSources", sources)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PeerRow {
    pub device_id: String,
    pub identity_public: Vec<u8>,
    pub cert_der: Vec<u8>,
    pub cert_id: String,
    pub display_name: String,
    pub safety_number: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingInput {
    pub content_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_number: Option<i64>,
    pub duration_ms: Option<i64>,
    pub codec: String,
}

#[derive(Debug, Clone, Copy)]
pub struct BlockFlags {
    pub manual: bool,
    pub source: bool,
    pub timestamp: bool,
    pub all: bool,
}

pub fn parse_queue_items(value: Option<&Value>) -> Vec<QueueItem> {
    let Some(items) = value.and_then(|item| item.as_array()) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            if let Some(id) = item.get("contentId").and_then(|value| value.as_str()).filter(|id| !id.is_empty()) {
                return Some(QueueItem::Content(id.to_string()));
            }
            if let Some(id) = item.get("localFileId").and_then(|value| value.as_str()).filter(|id| !id.is_empty()) {
                return Some(QueueItem::Local(id.to_string()));
            }
            let kind = item.get("kind")?.as_str()?;
            let id = item.get("id")?.as_str()?;
            match kind {
                "content" => Some(QueueItem::Content(id.to_string())),
                "local" => Some(QueueItem::Local(id.to_string())),
                _ => None,
            }
        })
        .collect()
}

pub fn fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .filter(|token| !token.is_empty())
        .map(|token| {
            let clean: String = token
                .chars()
                .filter(|ch| ch.is_alphanumeric())
                .collect();
            if clean.is_empty() {
                String::new()
            } else {
                format!("\"{clean}\"*")
            }
        })
        .filter(|token| !token.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn device_defaults() -> Vec<(&'static str, Value)> {
    vec![
        ("showUnavailable", json!(true)),
        ("historyCap", json!(100)),
        ("historySyncMode", json!("utc")),
        ("incognito", json!(false)),
        ("syncSources", json!({})),
        ("dirty", json!({})),
        ("syncBlocks", json!({})),
        ("exposeDefault", json!("hide")),
        ("positionSync", json!(false)),
        ("downloadFolder", json!("")),
        ("volume", json!(1.0)),
        ("window", json!({})),
        ("relayHost", json!("")),
        ("relayPort", json!(22067)),
        ("relayDeviceId", json!("")),
        ("relayToken", json!("")),
        ("discovery", json!(false)),
        ("syncEnabled", json!(false)),
        ("audioShare", json!(false)),
        ("displayName", json!("")),
        ("holdingsRevision", json!(0)),
        ("pendingSnapshots", json!({})),
        ("manualAddresses", json!([])),
    ]
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS content_items (
    content_id TEXT PRIMARY KEY,
    title TEXT,
    artist TEXT,
    album TEXT,
    track_number INTEGER,
    duration_ms INTEGER,
    codec TEXT,
    sample_rate INTEGER,
    bit_depth INTEGER,
    video INTEGER NOT NULL DEFAULT 0,
    unplayable INTEGER NOT NULL DEFAULT 0,
    unplayable_reason TEXT
);
CREATE TABLE IF NOT EXISTS grants (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    exposure TEXT NOT NULL DEFAULT 'follow'
);
CREATE TABLE IF NOT EXISTS local_files (
    local_file_id TEXT PRIMARY KEY,
    grant_id INTEGER,
    relative_path TEXT NOT NULL,
    absolute_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    lyrics TEXT,
    metadata_status TEXT NOT NULL,
    metadata_reason TEXT,
    access_status TEXT NOT NULL,
    access_reason TEXT,
    playback_status TEXT NOT NULL,
    playback_reason TEXT,
    exposed INTEGER NOT NULL,
    added_at TEXT NOT NULL,
    mtime_ms INTEGER,
    size_bytes INTEGER,
    content_id TEXT,
    duration_ms INTEGER,
    title TEXT,
    artist TEXT,
    album TEXT,
    track_number INTEGER,
    codec TEXT,
    sample_rate INTEGER,
    bit_depth INTEGER,
    video INTEGER NOT NULL DEFAULT 0,
    cover_path TEXT,
    cover_checked INTEGER NOT NULL DEFAULT 0,
    UNIQUE(grant_id, relative_path)
);
CREATE TABLE IF NOT EXISTS favorites (
    id INTEGER PRIMARY KEY,
    content_id TEXT UNIQUE,
    local_file_id TEXT UNIQUE
);
CREATE TABLE IF NOT EXISTS holdings (
    device_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    content_id TEXT NOT NULL,
    title TEXT,
    artist TEXT,
    album TEXT,
    track_number INTEGER,
    duration_ms INTEGER,
    codec TEXT,
    PRIMARY KEY (device_id, content_id)
);
CREATE TABLE IF NOT EXISTS holding_revisions (
    device_id TEXT PRIMARY KEY,
    revision INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS playback_notes (
    id INTEGER PRIMARY KEY,
    source_device_id TEXT,
    content_id TEXT,
    local_file_id TEXT,
    class TEXT NOT NULL,
    reason TEXT,
    attempt_id TEXT
);
CREATE TABLE IF NOT EXISTS snapshots (
    feature TEXT NOT NULL,
    source_device_id TEXT NOT NULL,
    revision INTEGER NOT NULL,
    payload TEXT NOT NULL,
    signature BLOB NOT NULL,
    PRIMARY KEY (feature, source_device_id)
);
CREATE TABLE IF NOT EXISTS applied_ops (
    op_id TEXT PRIMARY KEY,
    applied_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS mailbox (
    id INTEGER PRIMARY KEY,
    sender_id TEXT NOT NULL,
    n INTEGER NOT NULL,
    ciphertext BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE(sender_id, n)
);
CREATE TABLE IF NOT EXISTS mailbox_acks (
    mailbox_id INTEGER NOT NULL,
    device_id TEXT NOT NULL,
    PRIMARY KEY (mailbox_id, device_id)
);
CREATE TABLE IF NOT EXISTS playlists (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    position INTEGER NOT NULL,
    exposure TEXT NOT NULL DEFAULT 'follow',
    revision INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS playlist_tracks (
    playlist_id TEXT NOT NULL,
    position INTEGER NOT NULL,
    content_id TEXT,
    local_file_id TEXT,
    PRIMARY KEY (playlist_id, position)
);
CREATE TABLE IF NOT EXISTS play_history (
    id INTEGER PRIMARY KEY,
    content_id TEXT,
    local_file_id TEXT,
    played_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS device_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS synced_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS peer_list_prefs (
    device_id TEXT PRIMARY KEY,
    exposure TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS peers (
    device_id TEXT PRIMARY KEY,
    identity_public BLOB NOT NULL,
    cert_der BLOB NOT NULL,
    cert_id TEXT NOT NULL,
    display_name TEXT NOT NULL,
    safety_number TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS signal_state (
    key TEXT PRIMARY KEY,
    value BLOB NOT NULL
);
CREATE TABLE IF NOT EXISTS player_state (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS feature_revisions (
    feature TEXT PRIMARY KEY,
    revision INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS pending_content_move (
    local_file_id TEXT PRIMARY KEY,
    previous_content_id TEXT NOT NULL
);
CREATE VIRTUAL TABLE IF NOT EXISTS library_fts USING fts5(
    local_file_id,
    content_id,
    title,
    artist,
    lyrics,
    folder
);
"#;

pub fn apply_queue_state(state: &mut Value, future: &[QueueItem]) {
    if !state.is_object() {
        *state = json!({});
    }
    state["future"] = json!(future
        .iter()
        .map(|item| match item {
            QueueItem::Content(id) => json!({"key": id, "contentId": id, "localFileId": null}),
            QueueItem::Local(id) => json!({"key": id, "contentId": null, "localFileId": id}),
        })
        .collect::<Vec<_>>());
}

pub fn merge_history_into_db(db: &Db, remote: Vec<HistoryEntry>) -> AppResult<Vec<HistoryEntry>> {
    let local = db.history_for_sync()?;
    let cap = db.setting_i64("historyCap")?.max(1) as usize;
    let merged = sync_logic::merge_history(local, remote, Utc::now(), cap);
    db.replace_synced_history(&merged)?;
    Ok(merged)
}
