mod catalog;
mod media;
mod config;
mod db;
mod error;
mod frames;
mod identity;
mod ids;
mod net;
mod relay;
mod scan;
mod sync_logic;
mod tls;

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::identity::IdentityMaterial;

struct AppState {
    db: Arc<Mutex<Db>>,
    net: net::Network,
    material: IdentityMaterial,
    app_dir: std::path::PathBuf,
}

const NETWORK_KEYS: [&str; 9] = [
    "discovery",
    "syncEnabled",
    "audioShare",
    "incognito",
    "relayHost",
    "relayPort",
    "relayDeviceId",
    "relayToken",
    "displayName",
];

fn online_ids(state: &AppState) -> Vec<String> {
    let live = state.net.live.inner.lock().expect("live");
    live.links.iter().filter(|link| link.state == "online").map(|link| link.device_id.clone()).collect()
}

#[tauri::command(async)]
fn bootstrap(state: State<'_, AppState>) -> AppResult<Value> {
    let online = online_ids(&state);
    let db = state.db.lock().expect("db");
    let live = state.net.live.inner.lock().expect("live");
    let pending = db.setting_json("pendingSnapshots")?;
    let pending = pending
        .as_object()
        .map(|map| {
            map.iter()
                .map(|(feature, item)| json!({"feature": feature, "source": item.get("source").cloned().unwrap_or(Value::Null)}))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({
        "deviceId": state.material.device_id,
        "certId": state.material.cert_id,
        "deviceName": db.setting_string("displayName").unwrap_or_default(),
        "pairingText": identity::pairing_text(
            &state.material.identity.identity_key().serialize(),
            &state.material.cert_der,
            &db.setting_string("displayName").unwrap_or_default(),
        ),
        "settings": db.device_settings_map()?,
        "theme": db.synced("theme")?,
        "language": db.synced("language")?,
        "grants": db.grants()?,
        "playlists": db.playlists()?,
        "player": db.player_json()?,
        "history": db.history_keys()?,
        "pending": pending,
        "tracks": catalog::library(&db, &online)?,
        "config": config::load_config(&state.app_dir)?,
        "listenPort": live.listen_port,
        "nearby": live.nearby,
        "links": live.links,
        "relay": live.relay,
        "offer": live.offer,
        "download": live.download,
        "manual": live.manual,
    }))
}

#[tauri::command(async)]
fn import_paths(app: AppHandle, state: State<'_, AppState>, paths: Vec<String>, kind: String) -> AppResult<Value> {
    let mut summaries = Vec::new();
    for path in &paths {
        let summary = scan::import_path(&state.db, Path::new(path), &kind)?;
        if !allow_grant(&app, &summary.kind, path) {
            let _ = state.db.lock().expect("db").delete_grant(summary.grant_id);
            return Err(AppError::Message("file.unreadable".into()));
        }
        summaries.push(summary);
    }
    let _ = state.net.tx.send(net::Cmd::Publish);
    let _ = app.emit("library-updated", json!({}));
    Ok(json!(summaries))
}

#[tauri::command(async)]
fn rescan(app: AppHandle, state: State<'_, AppState>) -> AppResult<()> {
    let grants = state.db.lock().expect("db").grants()?;
    for grant in grants {
        if Path::new(&grant.path).exists() {
            let _ = scan::import_path(&state.db, Path::new(&grant.path), &grant.kind);
        }
    }
    scan::refresh_access(&state.db.lock().expect("db"))?;
    let _ = state.net.tx.send(net::Cmd::Publish);
    let _ = app.emit("library-updated", json!({}));
    Ok(())
}

fn allow_grant(app: &AppHandle, kind: &str, path: &str) -> bool {
    let scope = app.asset_protocol_scope();
    if kind == "directory" {
        scope.allow_directory(path, true).is_ok()
    } else {
        scope.allow_file(path).is_ok()
    }
}

#[tauri::command(async)]
fn remove_grant(app: AppHandle, state: State<'_, AppState>, id: i64) -> AppResult<()> {
    let db = state.db.lock().expect("db");
    if let Some(grant) = db.delete_grant(id)? {
        let scope = app.asset_protocol_scope();
        if grant.kind == "directory" {
            let _ = scope.forbid_directory(grant.path, true);
        } else {
            let _ = scope.forbid_file(grant.path);
        }
        for kept in db.grants()? {
            if kept.kind == "file" {
                let _ = scope.allow_file(kept.path);
            }
        }
    }
    drop(db);
    let _ = state.net.tx.send(net::Cmd::Publish);
    let _ = app.emit("library-updated", json!({}));
    Ok(())
}

#[tauri::command(async)]
fn library(state: State<'_, AppState>) -> AppResult<Value> {
    let online = online_ids(&state);
    let db = state.db.lock().expect("db");
    catalog::library(&db, &online)
}

#[tauri::command(async)]
fn search(state: State<'_, AppState>, query: String) -> AppResult<Value> {
    let online = online_ids(&state);
    let db = state.db.lock().expect("db");
    catalog::search(&db, &query, &online)
}

#[tauri::command(async)]
fn lyrics(state: State<'_, AppState>, local_file_id: String) -> AppResult<String> {
    state.db.lock().expect("db").lyrics(&local_file_id)
}

#[tauri::command(async)]
fn save_player(state: State<'_, AppState>, state_json: Value) -> AppResult<()> {
    let db = state.db.lock().expect("db");
    let before = future_ids(&db.player_json()?);
    db.save_player_json(&state_json)?;
    let after = future_ids(&state_json);
    // Advancing to the next track only drops the head; any other change is a local queue edit.
    let advanced = before.get(1..).is_some_and(|rest| rest == after.as_slice());
    let changed = before != after && !advanced;
    if changed {
        db.mark_dirty("queue", true)?;
    }
    drop(db);
    if changed {
        let _ = state.net.tx.send(net::Cmd::AutoSync("queue".into()));
    }
    Ok(())
}

fn future_ids(state: &Value) -> Vec<String> {
    state
        .get("future")
        .and_then(|items| items.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("contentId").and_then(|id| id.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

#[tauri::command(async)]
fn save_progress(state: State<'_, AppState>, position_sec: f64) -> AppResult<()> {
    state.db.lock().expect("db").save_progress(position_sec)
}

#[tauri::command(async)]
fn record_history(state: State<'_, AppState>, content_id: Option<String>, local_file_id: Option<String>) -> AppResult<()> {
    let db = state.db.lock().expect("db");
    db.record_history_local(content_id.as_deref(), local_file_id.as_deref())?;
    if db.setting_string("historySyncMode").unwrap_or_else(|_| "utc".into()) != "utc" && content_id.is_some() {
        db.mark_dirty("history", true)?;
    }
    drop(db);
    let _ = state.net.tx.send(net::Cmd::AutoSync("history".into()));
    Ok(())
}

#[tauri::command(async)]
fn show_in_folder(state: State<'_, AppState>, local_file_id: String) -> AppResult<()> {
    let path: String = state.db.lock().expect("db").conn.query_row(
        "SELECT absolute_path FROM local_files WHERE local_file_id = ?1",
        rusqlite::params![local_file_id],
        |row| row.get(0),
    )?;
    if !Path::new(&path).exists() {
        return Err(AppError::Message("file.missing".into()));
    }
    tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|err| AppError::Message(err.to_string()))
}

#[tauri::command(async)]
fn playback_result(
    state: State<'_, AppState>,
    local_file_id: Option<String>,
    content_id: Option<String>,
    status: String,
    reason: Option<String>,
) -> AppResult<()> {
    let db = state.db.lock().expect("db");
    if let Some(id) = &local_file_id {
        db.conn.execute(
            "UPDATE local_files SET playback_status = ?1, playback_reason = ?2 WHERE local_file_id = ?3",
            rusqlite::params![status, reason, id],
        )?;
    }
    if status == "playable" {
        let _ = db.clear_transport_notes("local");
        if let Some(content_id) = &content_id {
            db.clear_unplayable(content_id)?;
        }
    } else if matches!(reason.as_deref(), Some("playback.decodeFailed") | Some("playback.unsupported") | Some("playback.video")) {
        if let Some(content_id) = &content_id {
            let class_name = if reason.as_deref() == Some("playback.unsupported") || reason.as_deref() == Some("playback.video") {
                "unsupported"
            } else {
                "decode"
            };
            db.note_failure(Some("local"), Some(content_id), local_file_id.as_deref(), class_name, reason.as_deref().unwrap_or(""), "local")?;
        }
    } else if status == "failed" && reason.as_deref() == Some("file.missing") {
        if let Some(id) = &local_file_id {
            db.conn.execute(
                "UPDATE local_files SET access_status = 'missing', access_reason = 'file.missing' WHERE local_file_id = ?1",
                rusqlite::params![id],
            )?;
        }
    }
    Ok(())
}

#[tauri::command(async)]
fn remote_failure(state: State<'_, AppState>, content_id: String, device_id: String, class: String, reason: String) -> AppResult<()> {
    state
        .db
        .lock()
        .expect("db")
        .note_failure(Some(&device_id), Some(&content_id), None, &class, &reason, &device_id)
}

#[tauri::command(async)]
fn toggle_favorite(state: State<'_, AppState>, content_id: Option<String>, local_file_id: Option<String>) -> AppResult<bool> {
    let db = state.db.lock().expect("db");
    let added = db.toggle_favorite(content_id.as_deref(), local_file_id.as_deref())?;
    db.mark_dirty("favorites", true)?;
    drop(db);
    let _ = state.net.tx.send(net::Cmd::AutoSync("favorites".into()));
    Ok(added)
}

fn playlists_changed(state: &AppState) {
    if let Ok(db) = state.db.lock() {
        let _ = db.mark_dirty("playlists", true);
    }
    let _ = state.net.tx.send(net::Cmd::AutoSync("playlists".into()));
}

#[tauri::command(async)]
fn create_playlist(state: State<'_, AppState>, name: String) -> AppResult<String> {
    if name.trim().is_empty() {
        return Err(AppError::Message("playlist.nameEmpty".into()));
    }
    let id = state.db.lock().expect("db").create_playlist(name.trim())?;
    playlists_changed(&state);
    Ok(id)
}

#[tauri::command(async)]
fn rename_playlist(state: State<'_, AppState>, id: String, name: String) -> AppResult<()> {
    if name.trim().is_empty() {
        return Err(AppError::Message("playlist.nameEmpty".into()));
    }
    state.db.lock().expect("db").rename_playlist(&id, name.trim())?;
    playlists_changed(&state);
    Ok(())
}

#[tauri::command(async)]
fn delete_playlist(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.lock().expect("db").delete_playlist(&id)?;
    playlists_changed(&state);
    Ok(())
}

#[tauri::command(async)]
fn add_to_playlist(state: State<'_, AppState>, playlist_id: String, content_id: Option<String>, local_file_id: Option<String>) -> AppResult<()> {
    state
        .db
        .lock()
        .expect("db")
        .add_to_playlist(&playlist_id, content_id.as_deref(), local_file_id.as_deref())?;
    playlists_changed(&state);
    let _ = state.net.tx.send(net::Cmd::Publish);
    Ok(())
}

#[tauri::command(async)]
fn remove_playlist_member(
    state: State<'_, AppState>,
    playlist_id: String,
    content_id: Option<String>,
    local_file_id: Option<String>,
) -> AppResult<()> {
    state
        .db
        .lock()
        .expect("db")
        .remove_member(&playlist_id, content_id.as_deref(), local_file_id.as_deref())?;
    playlists_changed(&state);
    Ok(())
}

#[tauri::command(async)]
fn move_playlist_member(state: State<'_, AppState>, playlist_id: String, from: usize, to: usize) -> AppResult<()> {
    state.db.lock().expect("db").move_member(&playlist_id, from, to)?;
    playlists_changed(&state);
    Ok(())
}

#[tauri::command(async)]
fn set_exposure(state: State<'_, AppState>, scope: String, id: String, value: String) -> AppResult<()> {
    let db = state.db.lock().expect("db");
    if scope == "grant" {
        db.set_grant_exposure(id.parse().unwrap_or(0), &value)?;
    } else if scope == "playlist" {
        db.set_playlist_exposure(&id, &value)?;
    } else if scope == "global" {
        db.set_device_setting("exposeDefault", json!(value))?;
    }
    Ok(())
}

#[tauri::command(async)]
fn expose_members(state: State<'_, AppState>, scope: String, id: String, expose: bool) -> AppResult<()> {
    let db = state.db.lock().expect("db");
    if scope == "grant" {
        db.expose_grant_members(id.parse().unwrap_or(0), expose)?;
    } else {
        db.expose_playlist_members(&id, expose)?;
    }
    drop(db);
    let _ = state.net.tx.send(net::Cmd::Publish);
    Ok(())
}

#[tauri::command(async)]
fn set_track_exposure(state: State<'_, AppState>, local_file_id: String, expose: bool) -> AppResult<()> {
    state.db.lock().expect("db").set_track_exposure(&local_file_id, expose)?;
    let _ = state.net.tx.send(net::Cmd::Publish);
    Ok(())
}

#[tauri::command(async)]
fn update_setting(state: State<'_, AppState>, key: String, value: Value) -> AppResult<()> {
    let db = state.db.lock().expect("db");
    if key == "theme" || key == "language" {
        let text = value.as_str().unwrap_or("").to_string();
        db.set_synced(&key, &text)?;
        db.mark_dirty(&key, true)?;
        drop(db);
        let _ = state.net.tx.send(net::Cmd::AutoSync(key));
        return Ok(());
    }
    db.set_device_setting(&key, value)?;
    if key == "historyCap" {
        db.trim_history()?;
    }
    drop(db);
    if NETWORK_KEYS.contains(&key.as_str()) {
        let _ = state.net.tx.send(net::Cmd::Apply);
    }
    Ok(())
}

#[tauri::command(async)]
fn update_config(state: State<'_, AppState>, database_path: String, listen_port: u16) -> AppResult<()> {
    config::validate_database_path(&database_path)?;
    let config = config::StartupConfig { database_path, listen_port };
    config::save_config(&state.app_dir, &config)?;
    let _ = state.net.tx.send(net::Cmd::Apply);
    Ok(())
}

#[tauri::command(async)]
fn network_command(state: State<'_, AppState>, action: String, payload: Value) -> AppResult<()> {
    let text = || payload.as_str().unwrap_or("").to_string();
    let cmd = match action.as_str() {
        "pair" => net::Cmd::Pair(text()),
        "manual" => net::Cmd::Manual {
            host: payload.get("host").and_then(|v| v.as_str()).unwrap_or("").trim().to_string(),
            port: payload.get("port").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
        },
        "confirm" => net::Cmd::Confirm(text()),
        "reject" => net::Cmd::Reject(text()),
        "remove" => net::Cmd::Remove(text()),
        "paste" => net::Cmd::Paste(text()),
        "sync" => net::Cmd::Sync {
            feature: payload.get("feature").and_then(|v| v.as_str()).unwrap_or("favorites").to_string(),
            refresh: payload.get("refresh").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "resolve" => net::Cmd::Resolve {
            feature: payload.get("feature").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            take: payload.get("take").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "download" => net::Cmd::Download(text()),
        "activity" => net::Cmd::Activity {
            playing: payload.get("playing").and_then(|v| v.as_bool()).unwrap_or(false),
            local_file_id: payload.get("localFileId").and_then(|v| v.as_str()).map(str::to_string),
            buffer_enough: payload.get("bufferEnough").and_then(|v| v.as_bool()).unwrap_or(false),
        },
        "position" => net::Cmd::Position {
            content_id: payload.get("contentId").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            seconds: payload.get("seconds").and_then(|v| v.as_f64()).unwrap_or(0.0),
        },
        "close-route" => net::Cmd::CloseRoutes,
        _ => net::Cmd::Apply,
    };
    state.net.tx.send(cmd).map_err(|_| AppError::Message("network stopped".into()))?;
    Ok(())
}

#[tauri::command(async)]
fn remote_url(state: State<'_, AppState>, content_id: String, exclude: Option<Vec<String>>) -> AppResult<Value> {
    if state.db.lock().expect("db").setting_bool("incognito")? {
        return Err(AppError::Message("notice.offlineTrack".into()));
    }
    let mut holders: Vec<String> = {
        let db = state.db.lock().expect("db");
        let mut stmt = db.conn.prepare("SELECT DISTINCT device_id FROM holdings WHERE content_id = ?1 ORDER BY device_id")?;
        let rows = stmt.query_map(rusqlite::params![content_id], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let exclude = exclude.unwrap_or_default();
    holders.retain(|holder| !exclude.contains(holder));
    let online = state.net.live.inner.lock().expect("live").links.clone();
    let device_id = holders
        .into_iter()
        .find(|holder| online.iter().any(|link| &link.device_id == holder && link.state == "online"))
        .ok_or_else(|| AppError::Message(if exclude.is_empty() { "notice.offlineTrack" } else { "playback.allSourcesFailed" }.into()))?;
    Ok(json!({"url": state.net.bus.open_route(&content_id, &device_id), "deviceId": device_id}))
}

#[tauri::command(async)]
fn playlist_members(state: State<'_, AppState>, id: String) -> AppResult<Vec<Value>> {
    let refs = state.db.lock().expect("db").content_member_refs("playlist", Some(&id))?;
    Ok(refs.into_iter().map(|item| json!({"contentId": item.content_id, "localFileId": item.local_file_id})).collect())
}

#[tauri::command(async)]
fn move_membership(state: State<'_, AppState>, previous: String, next: String, accept: bool) -> AppResult<()> {
    if !accept {
        return Ok(());
    }
    let db = state.db.lock().expect("db");
    let already: Option<i64> = db
        .conn
        .query_row("SELECT id FROM favorites WHERE content_id = ?1", rusqlite::params![next], |row| row.get(0))
        .ok();
    if already.is_some() {
        db.conn.execute("DELETE FROM favorites WHERE content_id = ?1", rusqlite::params![previous])?;
    } else {
        db.conn.execute(
            "UPDATE favorites SET content_id = ?1 WHERE content_id = ?2",
            rusqlite::params![next, previous],
        )?;
    }
    db.conn.execute(
        "UPDATE playlist_tracks SET content_id = ?1 WHERE content_id = ?2",
        rusqlite::params![next, previous],
    )?;
    db.mark_dirty("favorites", true)?;
    db.mark_dirty("playlists", true)?;
    drop(db);
    let _ = state.net.tx.send(net::Cmd::AutoSync("favorites".into()));
    let _ = state.net.tx.send(net::Cmd::AutoSync("playlists".into()));
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data = app.path().data_dir().expect("data dir");
            let cache = app.path().app_cache_dir().expect("cache dir");
            let app_dir = config::app_dir(&data);
            let startup = config::load_config(&app_dir).expect("config");
            let database = config::resolve_database_path(&app_dir, &startup);
            let db = Db::open(&database, app_dir.clone(), cache.clone()).expect("database");
            for grant in db.grants().unwrap_or_default() {
                if Path::new(&grant.path).exists() {
                    allow_grant(app.handle(), &grant.kind, &grant.path);
                }
            }
            let material = identity::load_or_create(&db).expect("identity");
            let db = Arc::new(Mutex::new(db));
            let net = net::spawn(app.handle().clone(), db.clone(), material.clone_for_net())?;
            let _ = net.tx.send(net::Cmd::Apply);
            let worker = BackgroundWork {
                app: app.handle().clone(),
                db: db.clone(),
                live: net.live.clone(),
                tx: net.tx.clone(),
            };
            std::thread::spawn(move || worker.run());
            app.manage(AppState { db, net, material, app_dir });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap,
            import_paths,
            rescan,
            remove_grant,
            library,
            search,
            lyrics,
            save_player,
            save_progress,
            record_history,
            playback_result,
            remote_failure,
            toggle_favorite,
            create_playlist,
            rename_playlist,
            delete_playlist,
            add_to_playlist,
            remove_playlist_member,
            move_playlist_member,
            set_exposure,
            expose_members,
            set_track_exposure,
            update_setting,
            update_config,
            network_command,
            remote_url,
            playlist_members,
            move_membership,
            show_in_folder
        ])
        .run(tauri::generate_context!())
        .expect("error while running Multi-Device Music Player");
}

/// Access checks, covers and content hashing after the window is up. Playback keeps the disk first.
struct BackgroundWork {
    app: AppHandle,
    db: Arc<Mutex<Db>>,
    live: net::Live,
    tx: tokio::sync::mpsc::UnboundedSender<net::Cmd>,
}

impl BackgroundWork {
    fn run(self) {
        if let Ok(guard) = self.db.lock() {
            if scan::refresh_access(&guard).is_ok() {
                let _ = self.app.emit("library-updated", json!({}));
            }
        }
        let mut changed = false;
        let mut hashed = false;
        let mut last_emit = Instant::now();
        loop {
            let (playing, serving, current, buffer_enough) = self
                .live
                .inner
                .lock()
                .map(|guard| (guard.playing, guard.serving, guard.current_local.clone(), guard.buffer_enough))
                .unwrap_or((false, false, None, false));
            let covered = self.cover_step(playing);
            let hash = self.hash_step(playing, serving, current.as_deref(), buffer_enough);
            let did_work = covered || hash;
            changed |= did_work;
            hashed |= hash;
            if changed && (!did_work || last_emit.elapsed() > Duration::from_secs(3)) {
                let _ = self.app.emit("library-updated", json!({}));
                let _ = self.tx.send(net::Cmd::Publish);
                if hashed {
                    let _ = self.tx.send(net::Cmd::AutoSync("favorites".into()));
                    let _ = self.tx.send(net::Cmd::AutoSync("playlists".into()));
                }
                last_emit = Instant::now();
                changed = false;
                hashed = false;
            }
            if !did_work {
                std::thread::sleep(Duration::from_millis(1500));
            }
        }
    }

    fn cover_step(&self, playing: bool) -> bool {
        let job = self.db.lock().ok().and_then(|guard| scan::next_cover_job(&guard).ok().flatten());
        let Some(job) = job else { return false };
        let saved = scan::extract_cover(&job);
        if let Ok(guard) = self.db.lock() {
            let _ = scan::finish_cover(&guard, &job.local_file_id, saved.as_deref());
        }
        if playing {
            std::thread::sleep(Duration::from_millis(120));
        }
        true
    }

    fn hash_step(&self, playing: bool, serving: bool, current: Option<&str>, buffer_enough: bool) -> bool {
        if playing && (!buffer_enough || serving) {
            return false;
        }
        let job = self
            .db
            .lock()
            .ok()
            .and_then(|guard| scan::next_hash_job(&guard, current, playing).ok().flatten());
        let Some(job) = job else { return false };
        match scan::compute_content_id(&job.path) {
            Ok(content_id) => {
                if let Ok(guard) = self.db.lock() {
                    if scan::finish_hash(&guard, &job, &content_id).unwrap_or(false) {
                        if let Ok(Some(change)) = scan::take_id_change(&guard, &job.local_file_id, &content_id) {
                            let _ = self.app.emit("content-moved", change);
                        }
                    }
                }
            }
            Err(_) => {
                if let Ok(guard) = self.db.lock() {
                    let _ = scan::mark_unreadable(&guard, &job.local_file_id);
                }
            }
        }
        true
    }
}

impl IdentityMaterial {
    fn clone_for_net(&self) -> Self {
        Self {
            device_id: self.device_id.clone(),
            identity: self.identity,
            registration_id: self.registration_id,
            cert_der: self.cert_der.clone(),
            cert_key_pkcs8: self.cert_key_pkcs8.clone(),
            cert_id: self.cert_id.clone(),
            distribution_id: self.distribution_id,
        }
    }
}
