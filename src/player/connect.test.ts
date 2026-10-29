// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
import assert from "node:assert/strict";
import test from "node:test";
import {
  ACK_TIMEOUT_MS,
  acceptsInbound,
  forgetDevice,
  pickAction,
  STALE_MS,
  addOverlay,
  claimBeats,
  contextIdFrom,
  contextKindOf,
  extrapolate,
  isContentId,
  nextClaim,
  pickActive,
  receiveState,
  safeName,
  settleOverlay,
  throttle,
  viewOf,
  windowAround,
  type ConnectState,
  type RemoteEntry,
} from "./connect.ts";

const A = "aa".repeat(33);
const B = "bb".repeat(33);
const C = "cc".repeat(33);
const SONG = "1".repeat(64);
const OTHER = "2".repeat(64);

function report(partial: Partial<ConnectState> = {}): ConnectState {
  return {
    v: 1,
    epoch: 7,
    seq: 1,
    claim: 1,
    playing: true,
    positionSec: 10,
    volume: 0.5,
    muted: false,
    track: { contentId: SONG, title: "t", artist: "a", album: "b", durationMs: 200_000 },
    queue: { future: [], order: "manual", repeat: "off", contextKind: "library", contextName: "" },
    done: [],
    ...partial,
  };
}

function entry(deviceId: string, partial: Partial<ConnectState> = {}, now = 0): RemoteEntry {
  return receiveState(undefined, deviceId, report(partial), now);
}

test("claims compare by clock, then by device id", () => {
  assert.equal(claimBeats(2, A, 1, B), true);
  assert.equal(claimBeats(1, B, 2, A), false);
  assert.equal(claimBeats(3, B, 3, A), true);
  assert.equal(claimBeats(3, A, 3, B), false);
});

test("position advances only while playing and stops at the end", () => {
  assert.equal(extrapolate({ positionSec: 10, playing: true, at: 1000, track: null }, 3000), 12);
  assert.equal(extrapolate({ positionSec: 10, playing: false, at: 1000, track: null }, 9000), 10);
  const track = { contentId: SONG, title: "", artist: "", album: "", durationMs: 11_000 };
  assert.equal(extrapolate({ positionSec: 10, playing: true, at: 0, track }, 5000), 11);
});

test("an older report from the same run is ignored, a new run is taken", () => {
  const first = entry(B, { seq: 5, positionSec: 50 });
  assert.equal(receiveState(first, B, report({ seq: 4, positionSec: 1 }), 10), first);
  assert.equal(receiveState(first, B, report({ seq: 5, positionSec: 1 }), 10), first);
  const restarted = receiveState(first, B, report({ epoch: 8, seq: 1, positionSec: 1 }), 10);
  assert.equal(restarted.positionSec, 1);
});

test("a report within one second of the extrapolated position keeps the old anchor", () => {
  const first = entry(B, { seq: 1, positionSec: 10 }, 0);
  const close = receiveState(first, B, report({ seq: 2, positionSec: 12.4 }), 2000);
  assert.equal(close.positionSec, 10);
  assert.equal(close.at, 0);
  assert.equal(close.receivedAt, 2000);
  const far = receiveState(first, B, report({ seq: 3, positionSec: 40 }), 2000);
  assert.equal(far.positionSec, 40);
  assert.equal(far.at, 2000);
  const otherSong = receiveState(first, B, report({ seq: 4, positionSec: 12, track: { contentId: OTHER, title: "", artist: "", album: "", durationMs: null } }), 2000);
  assert.equal(otherSong.at, 2000);
  const paused = receiveState(first, B, report({ seq: 5, positionSec: 12, playing: false }), 2000);
  assert.equal(paused.at, 2000);
});

test("the active device is the highest claim among online, fresh devices that claimed", () => {
  const online = new Set([B, C]);
  assert.equal(pickActive(A, 0, {}, online, 0), A);
  assert.equal(pickActive(A, 0, { [B]: entry(B, { claim: 0 }) }, online, 0), A);
  assert.equal(pickActive(A, 0, { [B]: entry(B, { claim: 1 }) }, online, 0), B);
  assert.equal(pickActive(A, 5, { [B]: entry(B, { claim: 4 }), [C]: entry(C, { claim: 3 }) }, online, 0), A);
  assert.equal(pickActive(A, 2, { [B]: entry(B, { claim: 4 }), [C]: entry(C, { claim: 4 }) }, online, 0), C);
  assert.equal(pickActive(A, 0, { [B]: entry(B, { claim: 4 }) }, new Set(), 0), A);
  assert.equal(pickActive(A, 0, { [B]: entry(B, { claim: 4 }) }, online, STALE_MS + 1), A);
});

test("a new claim beats every claim seen", () => {
  assert.equal(nextClaim(0, {}), 1);
  assert.equal(nextClaim(3, { [B]: entry(B, { claim: 9 }) }), 10);
});

test("the optimistic overlay lasts until the device reports the newest command or times out", () => {
  let overlay = addOverlay(null, "c1", { playing: false }, 100);
  overlay = addOverlay(overlay, "c2", { volume: 0.2 }, 200);
  assert.deepEqual(overlay.ids, ["c1", "c2"]);
  assert.equal(overlay.playing, false);
  assert.equal(settleOverlay(overlay, ["c1"], 300), overlay);
  assert.equal(settleOverlay(overlay, ["x", "c2"], 300), null);
  assert.equal(settleOverlay(overlay, [], 200 + ACK_TIMEOUT_MS + 1), null);
  const view = viewOf(entry(B, { playing: true, volume: 0.9 }), overlay);
  assert.equal(view.playing, false);
  assert.equal(view.volume, 0.2);
  const seek = viewOf(entry(B), addOverlay(null, "s", { positionSec: 99 }, 500));
  assert.equal(seek.positionSec, 99);
  assert.equal(seek.at, 500);
});

test("throttle sends the first value at once and the latest one after the wait", () => {
  let now = 0;
  const queued: (() => void)[] = [];
  const sent: number[] = [];
  const push = throttle<number>((value) => sent.push(value), 150, { set: (fn) => queued.push(fn), now: () => now });
  push(1);
  push(2);
  push(3);
  assert.deepEqual(sent, [1]);
  now = 150;
  queued.shift()?.();
  assert.deepEqual(sent, [1, 3]);
  now = 400;
  push(4);
  assert.deepEqual(sent, [1, 3, 4]);
});

test("only portable contexts keep their id; labels never carry a path", () => {
  assert.equal(contextKindOf("folder:C:\\Music\\Jazz"), "folder");
  assert.equal(contextKindOf("weird"), "library");
  assert.equal(contextIdFrom("liked"), "liked");
  assert.equal(contextIdFrom("folder"), "remote");
  assert.equal(contextIdFrom("playlist", "Road trip"), "remote:Road trip");
  assert.equal(contextIdFrom("folder", "C:\\Music\\Jazz"), "remote:Jazz");
  assert.equal(safeName("C:\\Music\\Jazz"), "Jazz");
  assert.equal(safeName("/home/me/Music/"), "Music");
});

test("content ids are 64 lowercase hex characters", () => {
  assert.equal(isContentId(SONG), true);
  assert.equal(isContentId("C:\\x.mp3"), false);
  assert.equal(isContentId(SONG.toUpperCase().replace(/1/g, "A")), false);
});

test("a long list is cut to a window that still holds the chosen song", () => {
  const items = Array.from({ length: 5000 }, (_, at) => at);
  const cut = windowAround(items, 4000, 2000);
  assert.equal(cut.items.length, 2000);
  assert.equal(cut.items[cut.index], 4000);
  assert.deepEqual(windowAround([1, 2, 3], 1, 2000), { items: [1, 2, 3], index: 1 });
});

test("a detached device ignores reports, controls and transfers but reads answers and leaves", () => {
  for (const kind of ["connect-state", "connect-cmd", "connect-ack", "connect-transfer", "connect-leave"]) {
    assert.equal(acceptsInbound(false, kind), true, kind);
  }
  assert.equal(acceptsInbound(true, "connect-state"), false);
  assert.equal(acceptsInbound(true, "connect-cmd"), false);
  assert.equal(acceptsInbound(true, "connect-transfer"), false);
  assert.equal(acceptsInbound(true, "connect-ack"), true);
  assert.equal(acceptsInbound(true, "connect-leave"), true);
  assert.equal(acceptsInbound(false, "position"), false);
});

test("a device that left is forgotten, so it can no longer be the active one", () => {
  const now = 1000;
  const remotes: Record<string, RemoteEntry> = {
    [B]: entry(B, { claim: 9 }, now),
    [C]: entry(C, { claim: 4 }, now),
  };
  const online = new Set([B, C]);
  assert.equal(pickActive(A, 2, remotes, online, now), B);
  const after = forgetDevice(remotes, B);
  assert.equal(B in after, false);
  assert.equal(B in remotes, true, "the old record is not changed");
  assert.equal(pickActive(A, 2, after, online, now), C);
  assert.equal(pickActive(A, 2, forgetDevice(after, C), online, now), A);
  assert.equal(forgetDevice(after, "nobody"), after);
});

test("the device picker while joined moves playback", () => {
  const base = { self: A, detached: false, detachedPeers: new Set<string>(), hasTrack: true, playing: true };
  assert.deepEqual(pickAction({ ...base, target: B, active: A }), { kind: "transfer", to: B });
  assert.deepEqual(pickAction({ ...base, target: A, active: A }), { kind: "none" });
  assert.deepEqual(pickAction({ ...base, target: C, active: B }), { kind: "remoteTransfer", via: B, to: C });
  assert.deepEqual(pickAction({ ...base, target: A, active: B }), { kind: "remoteTransfer", via: B, to: A });
  assert.deepEqual(pickAction({ ...base, target: B, active: B }), { kind: "none" });
  assert.deepEqual(pickAction({ ...base, target: C, active: A, detachedPeers: new Set([C]) }), { kind: "unavailable" });
});

test("the device picker while detached rejoins", () => {
  const base = { self: A, active: A, detached: true, detachedPeers: new Set<string>(), hasTrack: true, playing: false };
  assert.deepEqual(pickAction({ ...base, target: A }), { kind: "rejoin", claim: false });
  assert.deepEqual(pickAction({ ...base, target: A, playing: true }), { kind: "rejoin", claim: true });
  assert.deepEqual(pickAction({ ...base, target: B, playing: true }), { kind: "rejoinTransfer", to: B });
  assert.deepEqual(pickAction({ ...base, target: B }), { kind: "rejoin", claim: false }, "nothing playing here: just join");
  assert.deepEqual(pickAction({ ...base, target: B, playing: true, hasTrack: false }), { kind: "rejoin", claim: false });
  assert.deepEqual(pickAction({ ...base, target: B, detachedPeers: new Set([B]) }), { kind: "unavailable" });
  assert.deepEqual(pickAction({ ...base, self: "", target: B }), { kind: "none" });
});

test("a claim stays above devices that left and were forgotten", () => {
  const remotes: Record<string, RemoteEntry> = { [B]: entry(B, { claim: 9 }) };
  assert.equal(nextClaim(3, remotes), 10);
  const forgotten = forgetDevice(remotes, B);
  assert.equal(nextClaim(3, forgotten), 4, "without the floor the old claim would win again");
  assert.equal(nextClaim(3, forgotten, 9), 10);
  assert.equal(nextClaim(12, forgotten, 9), 13);
});
