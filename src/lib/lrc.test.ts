import assert from "node:assert/strict";
import { test } from "node:test";
import { activeLine, parseLyrics } from "./lrc.ts";

test("timed lines are sorted and repeated stamps expand", () => {
  const lyrics = parseLyrics("[ti:Song]\n[00:12.50]Second\n[00:01.00][00:20]First and again\n");
  assert.equal(lyrics.timed, true);
  assert.deepEqual(lyrics.lines.map((line) => [line.time, line.text]), [
    [1, "First and again"],
    [12.5, "Second"],
    [20, "First and again"],
  ]);
});

test("offset tag shifts every line", () => {
  const lyrics = parseLyrics("[offset:500]\n[00:02.00]Hi");
  assert.equal(lyrics.lines[0].time, 1.5);
});

test("plain lyrics keep their blank lines inside but trim the edges", () => {
  const lyrics = parseLyrics("\n\nLine one\n\nLine two\n\n");
  assert.equal(lyrics.timed, false);
  assert.deepEqual(lyrics.lines.map((line) => line.text), ["Line one", "", "Line two"]);
});

test("active line follows position", () => {
  const { lines } = parseLyrics("[00:01]a\n[00:05]b\n[00:09]c");
  assert.equal(activeLine(lines, 0), -1);
  assert.equal(activeLine(lines, 1), 0);
  assert.equal(activeLine(lines, 6), 1);
  assert.equal(activeLine(lines, 100), 2);
});
