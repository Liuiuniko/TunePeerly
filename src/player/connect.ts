// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
// Device control between paired computers, modelled on Spotify Connect.
// 
// Exactly one device is the active playback device. Every device reports its own player as
// `connect-state`; the device with the highest claim (a Lamport clock, ties broken by device id)
// is active. A device that starts playing takes a new claim, so every other device pauses and
// shows "Playing on X". Other devices control the active one with `connect-cmd`, and moving
// playback is a `connect-transfer` that the target answers with `connect-ack`.
import type { OrderMode, RepeatMode } from "./advance.ts";

export const CONNECT_VERSION = 1;
/** Commands without an answer after this long are given up and the optimistic view is dropped. */
export const ACK_TIMEOUT_MS = 3000;
/** A remote report that differs from the extrapolated position by less than this keeps the old anchor. */
export const SNAP_THRESHOLD_SEC = 1;
/** Reports are repeated this often, so a silent device is noticed. */
export const HEARTBEAT_PLAYING_MS = 5000;
export const HEARTBEAT_PAUSED_MS = 15000;
/** A device that sent nothing for this long no longer counts as active. */
export const STALE_MS = 35000;
/** Seek and volume drags send at most one command per this many milliseconds. */
export const THROTTLE_MS = 150;
export const MAX_REFS = 2000;
export const MAX_STATE_QUEUE = 200;
export const MAX_PAST = 200;

export type ConnectMeta = { contentId: string; title: string; artist: string; album: string; durationMs: number | null };

export type ConnectQueue = {
  future: ConnectMeta[];
  order: OrderMode;
  repeat: RepeatMode;
  contextKind: string;
  contextName: string;
};

/** What a device reports about its own player. `epoch` and `seq` are added by the sending backend. */
export type ConnectState = {
  v: number;
  epoch?: number;
  seq?: number;
  claim: number;
  playing: boolean;
  positionSec: number;
  volume: number;
  muted: boolean;
  track: ConnectMeta | null;
  queue: ConnectQueue;
  /** Ids of the latest commands this device carried out, newest last. */
  done: string[];
};

export type ConnectCommand =
  | { op: "toggle" | "play" | "pause" | "next" | "prev" | "clear" | "shuffle" | "shuffleLibrary" }
  | { op: "seek"; sec: number }
  | { op: "skip"; delta: number }
  | { op: "volume"; level: number }
  | { op: "mute"; muted: boolean }
  | { op: "repeat"; mode: RepeatMode }
  | { op: "playList"; refs: string[]; index: number; shuffle: boolean; contextKind?: string; contextName?: string }
  | { op: "add" | "playNext"; refs: string[] }
  | { op: "insert"; at: number; refs: string[] }
  | { op: "remove" | "jump"; at: number; contentId: string }
  | { op: "move"; from: number; to: number; contentId: string }
  | { op: "transfer"; to: string };

export type ConnectTransfer = {
  v: number;
  id: string;
  positionSec: number;
  playing: boolean;
  queue: { past: string[]; current: string | null; future: string[]; order: OrderMode; repeat: RepeatMode; contextKind: string; contextName: string };
};

export type ConnectAck = { v: number; id: string; ok: boolean; reason?: string };

/** A remote device's last report, anchored to this computer's clock when it arrived. */
export type RemoteEntry = ConnectState & { deviceId: string; at: number; receivedAt: number };

/** Claim order: higher clock wins, then the larger device id. */
export function claimBeats(clock: number, device: string, otherClock: number, otherDevice: string): boolean {
  if (clock !== otherClock) return clock > otherClock;
  return device > otherDevice;
}

/** Playback position of a remote device now, advanced from its last report. */
export function extrapolate(entry: { positionSec: number; playing: boolean; at: number; track: ConnectMeta | null }, now: number): number {
  const elapsed = entry.playing ? Math.max(0, (now - entry.at) / 1000) : 0;
  const raw = Math.max(0, entry.positionSec + elapsed);
  const duration = entry.track?.durationMs ? entry.track.durationMs / 1000 : 0;
  return duration > 0 ? Math.min(raw, duration) : raw;
}

/**
 * Folds a new report into what is known about that device. Older reports from the same run are
 * dropped. A playing song whose position is within the threshold of the extrapolated one keeps the
 * old anchor, so the progress bar does not jitter with every report.
 */
export function receiveState(previous: RemoteEntry | undefined, deviceId: string, state: ConnectState, now: number): RemoteEntry {
  if (previous && previous.epoch === state.epoch && (state.seq ?? 0) <= (previous.seq ?? 0)) return previous;
  const next: RemoteEntry = { ...state, deviceId, at: now, receivedAt: now };
  if (
    previous &&
    previous.playing &&
    state.playing &&
    previous.track?.contentId === state.track?.contentId &&
    Math.abs(extrapolate(previous, now) - state.positionSec) < SNAP_THRESHOLD_SEC
  ) {
    next.positionSec = previous.positionSec;
    next.at = previous.at;
  }
  return next;
}

export function isFresh(entry: RemoteEntry, now: number): boolean {
  return now - entry.receivedAt < STALE_MS;
}

/**
 * The active device: the highest claim among this computer and the online remotes that ever claimed.
 * A remote that never started playing (claim 0) cannot be active.
 */
export function pickActive(selfId: string, selfClaim: number, remotes: Record<string, RemoteEntry>, online: ReadonlySet<string>, now: number): string {
  let best = selfId;
  let bestClaim = selfClaim;
  for (const entry of Object.values(remotes)) {
    if (entry.deviceId === selfId || entry.claim <= 0 || !online.has(entry.deviceId) || !isFresh(entry, now)) continue;
    if (claimBeats(entry.claim, entry.deviceId, bestClaim, best)) {
      best = entry.deviceId;
      bestClaim = entry.claim;
    }
  }
  return best;
}

/** The next claim for this computer: above every claim it has seen. */
export function nextClaim(selfClaim: number, remotes: Record<string, RemoteEntry>): number {
  let max = selfClaim;
  for (const entry of Object.values(remotes)) max = Math.max(max, entry.claim);
  return max + 1;
}

/** Fields a controller shows before the active device confirms a command. */
export type Overlay = { ids: string[]; at: number; playing?: boolean; positionSec?: number; trackIndex?: number; volume?: number; muted?: boolean };

export function addOverlay(overlay: Overlay | null, id: string, patch: Omit<Overlay, "ids" | "at">, now: number): Overlay {
  return { ...(overlay ?? {}), ...patch, ids: [...(overlay?.ids ?? []), id].slice(-16), at: now };
}

/** The overlay ends once the device reports that it carried out the newest command, or after the timeout. */
export function settleOverlay(overlay: Overlay | null, done: readonly string[], now: number): Overlay | null {
  if (!overlay) return null;
  if (now - overlay.at > ACK_TIMEOUT_MS) return null;
  const last = overlay.ids[overlay.ids.length - 1];
  if (last && done.includes(last)) return null;
  return overlay;
}

/** What a controller shows for the active remote device. */
export type RemoteView = {
  deviceId: string;
  playing: boolean;
  /** Position at `at`, so `extrapolate` gives the position now. */
  positionSec: number;
  at: number;
  volume: number;
  muted: boolean;
  track: ConnectMeta | null;
  queue: ConnectQueue;
};

export function viewOf(entry: RemoteEntry, overlay: Overlay | null): RemoteView {
  const view: RemoteView = {
    deviceId: entry.deviceId,
    playing: entry.playing,
    positionSec: entry.positionSec,
    at: entry.at,
    volume: entry.volume,
    muted: entry.muted,
    track: entry.track,
    queue: entry.queue,
  };
  if (!overlay) return view;
  if (overlay.playing !== undefined) view.playing = overlay.playing;
  if (overlay.positionSec !== undefined) {
    view.positionSec = overlay.positionSec;
    view.at = overlay.at;
  }
  if (overlay.volume !== undefined) view.volume = overlay.volume;
  if (overlay.muted !== undefined) view.muted = overlay.muted;
  return view;
}

/** Calls `send` at most once per `wait` ms, always delivering the latest value at the end. */
export function throttle<T>(send: (value: T) => void, wait = THROTTLE_MS, timers: { set: (fn: () => void, ms: number) => unknown; now: () => number } = { set: (fn, ms) => setTimeout(fn, ms), now: () => Date.now() }) {
  let last = -Infinity;
  let pending: { value: T } | null = null;
  let scheduled = false;
  const flush = () => {
    scheduled = false;
    if (!pending) return;
    const { value } = pending;
    pending = null;
    last = timers.now();
    send(value);
  };
  return (value: T) => {
    pending = { value };
    const since = timers.now() - last;
    if (since >= wait && !scheduled) {
      flush();
    } else if (!scheduled) {
      scheduled = true;
      timers.set(flush, Math.max(0, wait - since));
    }
  };
}

const CONTENT_ID = /^[0-9a-f]{64}$/;

export function isContentId(value: unknown): value is string {
  return typeof value === "string" && CONTENT_ID.test(value);
}

/** Context kinds whose id carries no data and can be rebuilt on any device. */
const PORTABLE_CONTEXTS = new Set(["library", "liked", "recent", "added", "random"]);

/** The context kind sent over the wire; never a folder path or other private detail. */
export function contextKindOf(contextId: string): string {
  const kind = contextId.split(":")[0] ?? "library";
  return ["library", "liked", "recent", "added", "random", "playlist", "artist", "album", "folder", "device", "search", "problems", "remote"].includes(kind) ? kind : "library";
}

export function isPortableContext(kind: string): boolean {
  return PORTABLE_CONTEXTS.has(kind);
}

/** The context id a receiving device uses: portable ones are kept, others play as the sent list under its label. */
export function contextIdFrom(kind: string, name = ""): string {
  if (PORTABLE_CONTEXTS.has(kind)) return kind;
  const label = safeName(name);
  return label ? `remote:${label}` : "remote";
}

/** A label only: the last part of a folder path, never the path itself. */
export function safeName(name: string): string {
  const last = name.split(/[\\/]/).filter(Boolean).pop() ?? "";
  return last.slice(0, 200);
}

/** A window of at most `max` items around `index`, so a long list still starts at the chosen song. */
export function windowAround<T>(items: T[], index: number, max = MAX_REFS): { items: T[]; index: number } {
  if (items.length <= max) return { items, index };
  const start = Math.max(0, Math.min(index - Math.floor(max / 4), items.length - max));
  return { items: items.slice(start, start + max), index: index - start };
}

let counter = 0;
export function commandId(): string {
  counter = (counter + 1) % 1_000_000;
  return `${Date.now().toString(36)}-${counter.toString(36)}-${Math.floor(Math.random() * 1e9).toString(36)}`;
}
