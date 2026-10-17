import assert from "node:assert/strict";
import test from "node:test";
import {
  acceptGeneration,
  addToQueue,
  fisherYates,
  jumpTo,
  moveInQueue,
  next,
  onEnded,
  playNext,
  previous,
  removeAt,
  selectSource,
  shouldIncrementGeneration,
  type QueueState,
  type TrackRef,
} from "./advance.ts";

function track(key: string): TrackRef {
  return { key, contentId: key, localFileId: null };
}

function state(partial: Partial<QueueState> = {}): QueueState {
  return {
    order: "manual",
    repeat: "off",
    contextId: "library",
    past: [],
    current: track("a"),
    future: [track("b"), track("c")],
    positionSec: 0,
    ...partial,
  };
}

test("manual order and independent repeat", () => {
  const ended = onEnded(state(), [track("a"), track("b")], () => 0);
  assert.equal(ended.increment, true);
  assert.equal(ended.state.current?.key, "b");
  assert.equal(ended.state.future[0]?.key, "c");
});

test("shuffle queue is a permutation", () => {
  const items = ["a", "b", "c", "d"];
  let cursor = 0;
  const values = [0.9, 0.1, 0.4];
  const shuffled = fisherYates(items, () => values[cursor++] ?? 0);
  assert.deepEqual(shuffled.slice().sort(), items.slice().sort());
});

test("previous after three seconds restarts", () => {
  const result = previous(state({ past: [track("z")], positionSec: 4 }), 4);
  assert.equal(result.current?.key, "a");
  assert.equal(result.positionSec, 0);
  assert.equal(result.past.length, 1);
});

test("previous within three seconds uses history", () => {
  const result = previous(state({ past: [track("z")] }), 2);
  assert.equal(result.current?.key, "z");
  assert.equal(result.future[0]?.key, "a");
});

test("next returns to the song just left", () => {
  const left = previous(state({ past: [track("z")] }), 1);
  const back = next(left);
  assert.equal(back.current?.key, "a");
});

test("play next and add to queue", () => {
  assert.equal(playNext(state(), track("x")).future[0]?.key, "x");
  assert.equal(addToQueue(state(), track("y")).future.at(-1)?.key, "y");
});

test("removing a duplicate keeps the other copy", () => {
  const queued = addToQueue(state(), [track("b"), track("x")]);
  const result = removeAt(queued, 0);
  assert.deepEqual(result.future.map((item) => item.key), ["c", "b", "x"]);
});

test("moving a queued entry", () => {
  const result = moveInQueue(state({ future: [track("b"), track("c"), track("d")] }), 0, 2);
  assert.deepEqual(result.future.map((item) => item.key), ["c", "d", "b"]);
});

test("jumping into the queue moves skipped entries to past", () => {
  const result = jumpTo(state({ future: [track("b"), track("c"), track("d")] }), 1);
  assert.equal(result.current?.key, "c");
  assert.deepEqual(result.past.map((item) => item.key), ["a", "b"]);
  assert.deepEqual(result.future.map((item) => item.key), ["d"]);
});

test("repeat list rebuilds without keeping the first song in both places", () => {
  const result = onEnded(state({ repeat: "list", future: [] }), [track("a"), track("b"), track("c")], () => 0);
  assert.equal(result.state.current?.key, "a");
  assert.deepEqual(result.state.future.map((item) => item.key), ["b", "c"]);
});

test("repeat one does not rebuild the list", () => {
  const result = onEnded(state({ repeat: "one" }), [track("nope")], () => 0);
  assert.equal(result.state.current?.key, "a");
  assert.equal(result.state.future.length, 2);
  assert.equal(result.increment, true);
});

test("stale generation events are dropped", () => {
  assert.equal(acceptGeneration(3, 3), true);
  assert.equal(acceptGeneration(4, 3), false);
});

test("transport failure tries the next device and does not skip the song", () => {
  const sources = [
    { deviceId: "a", pathId: "1", remote: true },
    { deviceId: "a", pathId: "2", remote: true },
    { deviceId: "b", pathId: "3", remote: true },
  ];
  assert.equal(selectSource(sources, "transport", sources[0], false)?.pathId, "2");
  assert.equal(selectSource(sources, "transport", sources[1], false)?.deviceId, "b");
});

test("decode or unsupported does not change device", () => {
  const sources = [
    { deviceId: "local", pathId: "file", remote: false },
    { deviceId: "peer", pathId: "copy", remote: true },
  ];
  assert.equal(selectSource(sources, "decode", sources[0], false), null);
  assert.equal(selectSource(sources, "unsupported", sources[0], false), null);
});

test("a lan and relay path switch does not increment generation", () => {
  assert.equal(shouldIncrementGeneration("path"), false);
  assert.equal(shouldIncrementGeneration("device"), true);
  assert.equal(shouldIncrementGeneration("repeat"), true);
});
