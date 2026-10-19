// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
//! Snapshot and history rules. These functions do not read the network.

use chrono::{DateTime, Duration, Utc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    pub content_id: String,
    pub at: DateTime<Utc>,
}

pub fn apply_member_snapshot(existing: &[MemberRef], snapshot: &[String]) -> Vec<MemberRef> {
    let mut kept: Vec<MemberRef> = existing
        .iter()
        .filter(|member| member.content_id.is_none())
        .cloned()
        .collect();
    for content_id in snapshot {
        kept.push(MemberRef {
            content_id: Some(content_id.clone()),
            local_file_id: None,
        });
    }
    kept
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberRef {
    pub content_id: Option<String>,
    pub local_file_id: Option<String>,
}

pub fn merge_history(
    local: Vec<HistoryEntry>,
    remote: Vec<HistoryEntry>,
    now: DateTime<Utc>,
    cap: usize,
) -> Vec<HistoryEntry> {
    let mut merged = Vec::new();
    for entry in local.into_iter().chain(remote) {
        let at = clamp_future(entry.at, now);
        if let Some(existing) = merged
            .iter_mut()
            .find(|item: &&mut HistoryEntry| item.content_id == entry.content_id)
        {
            if at > existing.at {
                existing.at = at;
            }
        } else {
            merged.push(HistoryEntry {
                content_id: entry.content_id,
                at,
            });
        }
    }
    merged.sort_by(|a, b| b.at.cmp(&a.at).then_with(|| a.content_id.cmp(&b.content_id)));
    merged.truncate(cap);
    merged
}

fn clamp_future(at: DateTime<Utc>, now: DateTime<Utc>) -> DateTime<Utc> {
    if at > now + Duration::minutes(5) {
        now
    } else {
        at
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// Playback tests build a local candidate. The running player does not.
    #[cfg_attr(not(test), allow(dead_code))]
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaySource {
    pub device_id: String,
    pub path_id: String,
    pub kind: SourceKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureClass {
    Transport,
    Decode,
    Unsupported,
}

/// Decode and unsupported stop the whole `content_id`. Anything else can try another holder.
pub fn failure_class(reason: &str) -> FailureClass {
    match reason {
        "playback.decodeFailed" => FailureClass::Decode,
        "playback.unsupported" | "playback.video" => FailureClass::Unsupported,
        _ => FailureClass::Transport,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceChoice {
    Play(PlaySource),
    Stop,
}

/// Transport failures try another still-exposed path on the same device, then the next device.
/// Decode and unsupported stay on the original bytes.
pub fn select_source(
    sources: &[PlaySource],
    failure: FailureClass,
    failed: Option<&PlaySource>,
    incognito: bool,
) -> SourceChoice {
    let Some(failed) = failed else {
        return sources
            .first()
            .cloned()
            .map(SourceChoice::Play)
            .unwrap_or(SourceChoice::Stop);
    };
    match failure {
        FailureClass::Decode | FailureClass::Unsupported => SourceChoice::Stop,
        FailureClass::Transport => {
            if incognito && failed.kind == SourceKind::Remote {
                return SourceChoice::Stop;
            }
            let start = sources
                .iter()
                .position(|source| {
                    source.device_id == failed.device_id && source.path_id == failed.path_id
                })
                .map(|index| index + 1)
                .unwrap_or(0);
            let rest = &sources[start..];
            if let Some(next) = rest
                .iter()
                .find(|source| source.device_id == failed.device_id)
            {
                return SourceChoice::Play(next.clone());
            }
            if incognito {
                return SourceChoice::Stop;
            }
            rest.iter()
                .find(|source| source.device_id != failed.device_id)
                .cloned()
                .map(SourceChoice::Play)
                .unwrap_or(SourceChoice::Stop)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_snapshot_keeps_unhashed_rows() {
        let existing = vec![
            MemberRef {
                content_id: Some("old".into()),
                local_file_id: None,
            },
            MemberRef {
                content_id: None,
                local_file_id: Some("file".into()),
            },
        ];
        let next = apply_member_snapshot(&existing, &["new".into()]);
        assert_eq!(next.len(), 2);
        assert!(next.iter().any(|item| item.local_file_id.as_deref() == Some("file")));
        assert!(next.iter().any(|item| item.content_id.as_deref() == Some("new")));
        assert!(!next.iter().any(|item| item.content_id.as_deref() == Some("old")));
    }

    #[test]
    fn history_merges_by_latest_utc_and_clamps_the_future() {
        let now = DateTime::parse_from_rfc3339("2026-10-18T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let local = vec![HistoryEntry {
            content_id: "a".into(),
            at: now - Duration::minutes(10),
        }];
        let remote = vec![
            HistoryEntry {
                content_id: "a".into(),
                at: now - Duration::minutes(2),
            },
            HistoryEntry {
                content_id: "b".into(),
                at: now + Duration::minutes(30),
            },
        ];
        let merged = merge_history(local, remote, now, 100);
        assert_eq!(merged[0].content_id, "b");
        assert_eq!(merged[0].at, now);
        assert_eq!(merged[1].content_id, "a");
        assert_eq!(merged[1].at, now - Duration::minutes(2));
    }

    #[test]
    fn equal_history_timestamps_keep_one_order_without_error() {
        let now = DateTime::parse_from_rfc3339("2026-10-18T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let same = now - Duration::minutes(5);
        let merged = merge_history(
            vec![
                HistoryEntry { content_id: "b".into(), at: same },
                HistoryEntry { content_id: "a".into(), at: same },
            ],
            vec![HistoryEntry { content_id: "a".into(), at: same }],
            now,
            100,
        );
        assert_eq!(merged.len(), 2);
        assert_eq!(merged[0].content_id, "a");
        assert_eq!(merged[1].content_id, "b");
        assert_eq!(merged[0].at, same);
    }

    #[test]
    fn transport_tries_same_device_then_next_device() {
        let sources = vec![
            PlaySource {
                device_id: "a".into(),
                path_id: "1".into(),
                kind: SourceKind::Remote,
            },
            PlaySource {
                device_id: "a".into(),
                path_id: "2".into(),
                kind: SourceKind::Remote,
            },
            PlaySource {
                device_id: "b".into(),
                path_id: "3".into(),
                kind: SourceKind::Remote,
            },
        ];
        let first = select_source(&sources, FailureClass::Transport, Some(&sources[0]), false);
        assert_eq!(first, SourceChoice::Play(sources[1].clone()));
        let second = select_source(&sources, FailureClass::Transport, Some(&sources[1]), false);
        assert_eq!(second, SourceChoice::Play(sources[2].clone()));
    }

    #[test]
    fn decode_failure_does_not_change_device() {
        let sources = vec![PlaySource {
            device_id: "a".into(),
            path_id: "1".into(),
            kind: SourceKind::Local,
        }];
        assert_eq!(
            select_source(&sources, FailureClass::Unsupported, Some(&sources[0]), false),
            SourceChoice::Stop
        );
    }

    #[test]
    fn incognito_remote_failure_stops() {
        let sources = vec![
            PlaySource {
                device_id: "a".into(),
                path_id: "1".into(),
                kind: SourceKind::Remote,
            },
            PlaySource {
                device_id: "b".into(),
                path_id: "2".into(),
                kind: SourceKind::Remote,
            },
        ];
        assert_eq!(
            select_source(&sources, FailureClass::Transport, Some(&sources[0]), true),
            SourceChoice::Stop
        );
    }
}
