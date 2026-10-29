// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
//! Device control between paired computers, modelled on Spotify Connect.
//!
//! The players live in the web view; this module only checks the messages. Every message is
//! rebuilt from the fields listed here, so nothing else (a file path, a local file id) can travel
//! or arrive. Songs are named by content id only.
//!
//! A device can leave the group to play on its own ("detached"). While detached it sends only
//! `connect-leave` and refusals, takes in no reports, and answers every control or transfer with a
//! refusal, so it neither steers nor follows the others. Detaching lasts until the device rejoins or
//! the app restarts.

use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::time::{Duration, Instant};

pub const VERSION: u64 = 1;
pub const KINDS: [&str; 5] = ["connect-state", "connect-cmd", "connect-ack", "connect-transfer", "connect-leave"];
/// The refusal a detached device sends for controls and transfers.
pub const DETACHED_REASON: &str = "connect.detachedThere";
pub const MAX_REFS: usize = 2000;
pub const MAX_STATE_QUEUE: usize = 200;
pub const MAX_PAST: usize = 200;
const MAX_TEXT: usize = 512;
const MAX_DONE: usize = 16;
const MAX_POSITION: f64 = 10_000_000.0;
/// Commands, acks and transfers one device may send per second.
pub const COMMAND_RATE: u32 = 20;
/// State reports one device may send per second.
pub const STATE_RATE: u32 = 10;

pub fn is_content_id(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub fn is_device_id(text: &str) -> bool {
    !text.is_empty() && text.len() <= 200 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_token(text: &str) -> bool {
    !text.is_empty() && text.len() <= 64 && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn is_reason(text: &str) -> bool {
    !text.is_empty() && text.len() <= 80 && text.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
}

fn text(value: Option<&Value>, max: usize) -> String {
    value
        .and_then(Value::as_str)
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}

/// A label only: the last part of anything that looks like a path.
fn label(value: Option<&Value>) -> String {
    let full = text(value, MAX_TEXT);
    full.split(['/', '\\']).filter(|part| !part.is_empty()).last().unwrap_or("").chars().take(200).collect()
}

fn number(value: Option<&Value>, low: f64, high: f64) -> Option<f64> {
    let n = value?.as_f64()?;
    (n.is_finite() && n >= low && n <= high).then_some(n)
}

fn index(value: Option<&Value>) -> Option<u64> {
    value?.as_u64().filter(|n| *n <= 1_000_000)
}

fn content_id(value: Option<&Value>) -> Option<String> {
    let id = value?.as_str()?;
    is_content_id(id).then(|| id.to_string())
}

fn content_ids(value: Option<&Value>, min: usize, max: usize) -> Option<Vec<String>> {
    let items = value?.as_array()?;
    if items.len() < min || items.len() > max {
        return None;
    }
    items.iter().map(|item| content_id(Some(item))).collect()
}

fn order(value: Option<&Value>) -> Option<&'static str> {
    match value?.as_str()? {
        "manual" => Some("manual"),
        "listShuffle" => Some("listShuffle"),
        "libraryRandom" => Some("libraryRandom"),
        _ => None,
    }
}

fn repeat(value: Option<&Value>) -> Option<&'static str> {
    match value?.as_str()? {
        "off" => Some("off"),
        "list" => Some("list"),
        "one" => Some("one"),
        _ => None,
    }
}

fn context_kind(value: Option<&Value>) -> &'static str {
    const KNOWN: [&str; 13] = ["library", "liked", "recent", "added", "random", "playlist", "artist", "album", "folder", "device", "search", "problems", "remote"];
    let kind = value.and_then(Value::as_str).unwrap_or("library");
    KNOWN.iter().find(|item| **item == kind).copied().unwrap_or("library")
}

fn meta(value: &Value) -> Option<Value> {
    let id = content_id(value.get("contentId"))?;
    let duration = value.get("durationMs").and_then(Value::as_u64).filter(|n| *n <= 86_400_000);
    Some(json!({
        "contentId": id,
        "title": text(value.get("title"), MAX_TEXT),
        "artist": text(value.get("artist"), MAX_TEXT),
        "album": text(value.get("album"), MAX_TEXT),
        "durationMs": duration,
    }))
}

fn sanitize_state(body: &Value) -> Option<Value> {
    let queue = body.get("queue")?;
    let future: Vec<Value> = queue
        .get("future")
        .and_then(Value::as_array)
        .map(|items| items.iter().take(MAX_STATE_QUEUE).filter_map(meta).collect())
        .unwrap_or_default();
    let track = match body.get("track") {
        None | Some(Value::Null) => Value::Null,
        Some(item) => meta(item)?,
    };
    let mut done: Vec<Value> = body
        .get("done")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(Value::as_str).filter(|id| is_token(id)).map(|id| json!(id)).collect())
        .unwrap_or_default();
    if done.len() > MAX_DONE {
        done.drain(..done.len() - MAX_DONE);
    }
    let mut out = json!({
        "v": VERSION,
        "claim": body.get("claim").and_then(Value::as_u64).unwrap_or(0),
        "playing": body.get("playing")?.as_bool()?,
        "positionSec": number(body.get("positionSec"), 0.0, MAX_POSITION)?,
        "volume": number(body.get("volume"), 0.0, 1.0)?,
        "muted": body.get("muted").and_then(Value::as_bool).unwrap_or(false),
        "track": track,
        "queue": {
            "future": future,
            "order": order(queue.get("order"))?,
            "repeat": repeat(queue.get("repeat"))?,
            "contextKind": context_kind(queue.get("contextKind")),
            "contextName": label(queue.get("contextName")),
        },
        "done": done,
    });
    // Added by the sending backend; kept on the way in so stale reports can be told apart.
    for key in ["epoch", "seq"] {
        if let Some(n) = body.get(key).and_then(Value::as_u64) {
            out[key] = json!(n);
        }
    }
    Some(out)
}

fn sanitize_command(body: &Value) -> Option<Value> {
    let id = body.get("id")?.as_str().filter(|id| is_token(id))?;
    let op = body.get("op")?.as_str()?;
    let mut out = Map::new();
    out.insert("v".into(), json!(VERSION));
    out.insert("id".into(), json!(id));
    out.insert("op".into(), json!(op));
    match op {
        "toggle" | "play" | "pause" | "next" | "prev" | "clear" | "shuffle" | "shuffleLibrary" => {}
        "seek" => {
            out.insert("sec".into(), json!(number(body.get("sec"), 0.0, MAX_POSITION)?));
        }
        "skip" => {
            out.insert("delta".into(), json!(number(body.get("delta"), -3600.0, 3600.0)?));
        }
        "volume" => {
            out.insert("level".into(), json!(number(body.get("level"), 0.0, 1.0)?));
        }
        "mute" => {
            out.insert("muted".into(), json!(body.get("muted")?.as_bool()?));
        }
        "repeat" => {
            out.insert("mode".into(), json!(repeat(body.get("mode"))?));
        }
        "playList" => {
            let refs = content_ids(body.get("refs"), 1, MAX_REFS)?;
            let at = index(body.get("index"))?;
            if at as usize >= refs.len() {
                return None;
            }
            out.insert("refs".into(), json!(refs));
            out.insert("index".into(), json!(at));
            out.insert("shuffle".into(), json!(body.get("shuffle").and_then(Value::as_bool).unwrap_or(false)));
            out.insert("contextKind".into(), json!(context_kind(body.get("contextKind"))));
            out.insert("contextName".into(), json!(label(body.get("contextName"))));
        }
        "add" | "playNext" => {
            out.insert("refs".into(), json!(content_ids(body.get("refs"), 1, MAX_REFS)?));
        }
        "insert" => {
            out.insert("at".into(), json!(index(body.get("at"))?));
            out.insert("refs".into(), json!(content_ids(body.get("refs"), 1, MAX_REFS)?));
        }
        "remove" | "jump" => {
            out.insert("at".into(), json!(index(body.get("at"))?));
            out.insert("contentId".into(), json!(content_id(body.get("contentId"))?));
        }
        "move" => {
            out.insert("from".into(), json!(index(body.get("from"))?));
            out.insert("to".into(), json!(index(body.get("to"))?));
            out.insert("contentId".into(), json!(content_id(body.get("contentId"))?));
        }
        "transfer" => {
            let to = body.get("to")?.as_str().filter(|id| is_device_id(id))?;
            out.insert("to".into(), json!(to));
        }
        _ => return None,
    }
    Some(Value::Object(out))
}

fn sanitize_ack(body: &Value) -> Option<Value> {
    let id = body.get("id")?.as_str().filter(|id| is_token(id))?;
    let ok = body.get("ok")?.as_bool()?;
    let mut out = json!({"v": VERSION, "id": id, "ok": ok});
    if let Some(reason) = body.get("reason").and_then(Value::as_str).filter(|reason| is_reason(reason)) {
        out["reason"] = json!(reason);
    }
    Some(out)
}

fn sanitize_transfer(body: &Value) -> Option<Value> {
    let id = body.get("id")?.as_str().filter(|id| is_token(id))?;
    let queue = body.get("queue")?;
    let current = match queue.get("current") {
        None | Some(Value::Null) => Value::Null,
        Some(item) => json!(content_id(Some(item))?),
    };
    Some(json!({
        "v": VERSION,
        "id": id,
        "positionSec": number(body.get("positionSec"), 0.0, MAX_POSITION)?,
        "playing": body.get("playing")?.as_bool()?,
        "queue": {
            "past": content_ids(queue.get("past"), 0, MAX_PAST)?,
            "current": current,
            "future": content_ids(queue.get("future"), 0, MAX_REFS)?,
            "order": order(queue.get("order"))?,
            "repeat": repeat(queue.get("repeat"))?,
            "contextKind": context_kind(queue.get("contextKind")),
            "contextName": label(queue.get("contextName")),
        },
    }))
}

/// "This device stopped taking part"; it carries nothing else.
fn sanitize_leave(_body: &Value) -> Option<Value> {
    Some(json!({"v": VERSION}))
}

/// The message rebuilt from its allowed fields, or `None` when it is malformed or another version.
pub fn sanitize(kind: &str, body: &Value) -> Option<Value> {
    if body.get("v").and_then(Value::as_u64) != Some(VERSION) {
        return None;
    }
    let mut out = match kind {
        "connect-state" => sanitize_state(body)?,
        "connect-cmd" => sanitize_command(body)?,
        "connect-ack" => sanitize_ack(body)?,
        "connect-transfer" => sanitize_transfer(body)?,
        "connect-leave" => sanitize_leave(body)?,
        _ => return None,
    };
    out["kind"] = json!(kind);
    Some(out)
}

/// What happens to a message that arrives from another device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inbound {
    /// Handed to the player.
    Forward,
    /// Dropped without an answer.
    Drop,
    /// Dropped and answered with a refusal, so the sender is not left waiting.
    Refuse,
}

pub fn inbound(detached: bool, kind: &str) -> Inbound {
    if !KINDS.contains(&kind) {
        return Inbound::Drop;
    }
    if !detached {
        return Inbound::Forward;
    }
    match kind {
        "connect-cmd" | "connect-transfer" => Inbound::Refuse,
        // A leave or an answer to an earlier control changes nothing here but is still good to know.
        "connect-leave" | "connect-ack" => Inbound::Forward,
        _ => Inbound::Drop,
    }
}

/// Whether this computer may send a message of `kind`. A detached computer only says it left and
/// answers controls with refusals.
pub fn outbound_allowed(detached: bool, kind: &str) -> bool {
    KINDS.contains(&kind) && (!detached || matches!(kind, "connect-leave" | "connect-ack"))
}

/// The refusal sent for a control or transfer that reached a detached device.
pub fn refusal(kind: &str, body: &Value) -> Option<Value> {
    if !matches!(kind, "connect-cmd" | "connect-transfer") || body.get("v").and_then(Value::as_u64) != Some(VERSION) {
        return None;
    }
    let id = body.get("id")?.as_str().filter(|id| is_token(id))?;
    Some(json!({"v": VERSION, "kind": "connect-ack", "id": id, "ok": false, "reason": DETACHED_REASON}))
}

/// Messages counted per device and kind in one-second windows.
#[derive(Debug, Clone, Copy)]
pub struct RateWindow {
    start: Instant,
    count: u32,
}

pub fn allow(windows: &mut HashMap<String, RateWindow>, key: &str, now: Instant, max: u32) -> bool {
    let window = windows.entry(key.to_string()).or_insert(RateWindow { start: now, count: 0 });
    if now.duration_since(window.start) >= Duration::from_secs(1) {
        *window = RateWindow { start: now, count: 0 };
    }
    if window.count >= max {
        return false;
    }
    window.count += 1;
    true
}

/// True for a report newer than the last one seen from that device's current run.
pub fn fresh_state(seen: &mut HashMap<String, (u64, u64)>, device_id: &str, epoch: u64, seq: u64) -> bool {
    if let Some((last_epoch, last_seq)) = seen.get(device_id)
        && *last_epoch == epoch
        && seq <= *last_seq
    {
        return false;
    }
    seen.insert(device_id.to_string(), (epoch, seq));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONG: &str = "1111111111111111111111111111111111111111111111111111111111111111";

    fn state() -> Value {
        json!({
            "v": 1, "claim": 3, "playing": true, "positionSec": 12.5, "volume": 0.4, "muted": false,
            "track": {"contentId": SONG, "title": "T", "artist": "A", "album": "B", "durationMs": 1000, "absolutePath": "C:\\secret.mp3"},
            "queue": {"future": [{"contentId": SONG, "title": "x", "localFileId": "L1"}], "order": "manual", "repeat": "off", "contextKind": "folder", "contextName": "C:\\Users\\me\\Music"},
            "done": ["a1", "bad id!"],
            "absolutePath": "/home/me/x.mp3",
        })
    }

    #[test]
    fn state_keeps_only_listed_fields() {
        let out = sanitize("connect-state", &state()).unwrap();
        let text = out.to_string();
        assert!(!text.contains("secret") && !text.contains("absolutePath") && !text.contains("localFileId") && !text.contains("Users"));
        assert_eq!(out["queue"]["contextName"], "Music");
        assert_eq!(out["done"], json!(["a1"]));
        assert_eq!(out["kind"], "connect-state");
        assert_eq!(out["track"]["contentId"], SONG);
    }

    #[test]
    fn wrong_version_or_kind_is_refused() {
        let mut body = state();
        body["v"] = json!(2);
        assert!(sanitize("connect-state", &body).is_none());
        assert!(sanitize("position", &state()).is_none());
    }

    #[test]
    fn state_values_must_be_in_range() {
        for (key, value) in [("volume", json!(1.5)), ("positionSec", json!(-1)), ("playing", json!("yes"))] {
            let mut body = state();
            body[key] = value;
            assert!(sanitize("connect-state", &body).is_none(), "{key}");
        }
        let mut body = state();
        body["track"]["contentId"] = json!("C:\\x.mp3");
        assert!(sanitize("connect-state", &body).is_none());
    }

    #[test]
    fn state_queue_is_capped() {
        let mut body = state();
        body["queue"]["future"] = json!(vec![json!({"contentId": SONG}); 500]);
        let out = sanitize("connect-state", &body).unwrap();
        assert_eq!(out["queue"]["future"].as_array().unwrap().len(), MAX_STATE_QUEUE);
    }

    #[test]
    fn commands_are_checked_per_operation() {
        let ok = |body: Value| sanitize("connect-cmd", &body).is_some();
        assert!(ok(json!({"v": 1, "id": "c1", "op": "toggle"})));
        assert!(ok(json!({"v": 1, "id": "c1", "op": "seek", "sec": 30.0})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "seek", "sec": -3})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "volume", "level": 2})));
        assert!(ok(json!({"v": 1, "id": "c1", "op": "playList", "refs": [SONG], "index": 0})));
        let list = sanitize("connect-cmd", &json!({"v": 1, "id": "c1", "op": "playList", "refs": [SONG], "index": 0, "contextKind": "folder", "contextName": "D:\\Music\\Jazz"})).unwrap();
        assert_eq!(list["contextName"], "Jazz");
        assert_eq!(list["contextKind"], "folder");
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "playList", "refs": [SONG], "index": 1})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "playList", "refs": ["/etc/passwd"], "index": 0})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "add", "refs": []})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "add", "refs": vec![SONG; MAX_REFS + 1]})));
        assert!(ok(json!({"v": 1, "id": "c1", "op": "move", "from": 0, "to": 2, "contentId": SONG})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "remove", "at": 0})));
        assert!(ok(json!({"v": 1, "id": "c1", "op": "transfer", "to": "ab01"})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "transfer", "to": "../x"})));
        assert!(!ok(json!({"v": 1, "id": "bad id", "op": "toggle"})));
        assert!(!ok(json!({"v": 1, "id": "c1", "op": "format-disk"})));
        let out = sanitize("connect-cmd", &json!({"v": 1, "id": "c1", "op": "pause", "path": "C:\\x"})).unwrap();
        assert!(out.get("path").is_none());
    }

    #[test]
    fn transfers_and_acks_are_rebuilt() {
        let transfer = json!({"v": 1, "id": "t1", "positionSec": 3.0, "playing": true,
            "queue": {"past": [SONG], "current": SONG, "future": [SONG], "order": "listShuffle", "repeat": "one", "contextKind": "liked", "contextName": ""}});
        let out = sanitize("connect-transfer", &transfer).unwrap();
        assert_eq!(out["queue"]["order"], "listShuffle");
        let mut empty = transfer.clone();
        empty["queue"]["current"] = Value::Null;
        assert!(sanitize("connect-transfer", &empty).is_some());
        let mut bad = transfer;
        bad["queue"]["future"] = json!(["x"]);
        assert!(sanitize("connect-transfer", &bad).is_none());
        let ack = sanitize("connect-ack", &json!({"v": 1, "id": "t1", "ok": false, "reason": "notice.offlineTrack"})).unwrap();
        assert_eq!(ack["reason"], "notice.offlineTrack");
        let odd = sanitize("connect-ack", &json!({"v": 1, "id": "t1", "ok": false, "reason": "<script>"})).unwrap();
        assert!(odd.get("reason").is_none());
    }

    #[test]
    fn leave_carries_nothing_else() {
        let out = sanitize("connect-leave", &json!({"v": 1, "track": {"contentId": SONG}, "path": "C:\\x"})).unwrap();
        assert_eq!(out, json!({"v": 1, "kind": "connect-leave"}));
        assert!(sanitize("connect-leave", &json!({"v": 2})).is_none());
    }

    #[test]
    fn joined_device_takes_every_kind() {
        for kind in KINDS {
            assert_eq!(inbound(false, kind), Inbound::Forward, "{kind}");
            assert!(outbound_allowed(false, kind), "{kind}");
        }
        assert_eq!(inbound(false, "position"), Inbound::Drop);
        assert!(!outbound_allowed(false, "position"));
    }

    #[test]
    fn detached_device_neither_follows_nor_steers() {
        assert_eq!(inbound(true, "connect-state"), Inbound::Drop);
        assert_eq!(inbound(true, "connect-cmd"), Inbound::Refuse);
        assert_eq!(inbound(true, "connect-transfer"), Inbound::Refuse);
        assert_eq!(inbound(true, "connect-leave"), Inbound::Forward);
        assert_eq!(inbound(true, "connect-ack"), Inbound::Forward);
        assert!(!outbound_allowed(true, "connect-state"));
        assert!(!outbound_allowed(true, "connect-cmd"));
        assert!(!outbound_allowed(true, "connect-transfer"));
        assert!(outbound_allowed(true, "connect-leave"));
        assert!(outbound_allowed(true, "connect-ack"));
    }

    #[test]
    fn refusals_name_the_command() {
        let out = refusal("connect-transfer", &json!({"v": 1, "id": "t9", "positionSec": 1})).unwrap();
        assert_eq!(out["ok"], false);
        assert_eq!(out["id"], "t9");
        assert_eq!(out["reason"], DETACHED_REASON);
        let ack = sanitize("connect-ack", &out).unwrap();
        assert_eq!(ack["reason"], DETACHED_REASON);
        assert!(refusal("connect-cmd", &json!({"v": 1, "id": "bad id"})).is_none());
        assert!(refusal("connect-state", &json!({"v": 1, "id": "s1"})).is_none());
        assert!(refusal("connect-cmd", &json!({"v": 2, "id": "c1"})).is_none());
    }

    #[test]
    fn rate_limit_resets_each_second() {
        let mut windows = HashMap::new();
        let start = Instant::now();
        for _ in 0..3 {
            assert!(allow(&mut windows, "d:cmd", start, 3));
        }
        assert!(!allow(&mut windows, "d:cmd", start, 3));
        assert!(allow(&mut windows, "other", start, 3));
        assert!(allow(&mut windows, "d:cmd", start + Duration::from_millis(1001), 3));
    }

    #[test]
    fn stale_reports_are_dropped_until_a_new_run() {
        let mut seen = HashMap::new();
        assert!(fresh_state(&mut seen, "d", 1, 5));
        assert!(!fresh_state(&mut seen, "d", 1, 5));
        assert!(!fresh_state(&mut seen, "d", 1, 4));
        assert!(fresh_state(&mut seen, "d", 1, 6));
        assert!(fresh_state(&mut seen, "d", 2, 1));
    }
}
