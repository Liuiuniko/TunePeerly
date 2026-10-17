export type LyricLine = { time: number; text: string };
export type Lyrics = { timed: boolean; lines: LyricLine[] };

const stamp = /\[(\d{1,3}):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;
const tag = /^\[(ar|ti|al|by|offset|length|re|ve|au|#):(.*)\]$/i;

/** Parses LRC text. Lines without a time tag become plain lyrics when nothing is timed. */
export function parseLyrics(source: string): Lyrics {
  const timed: LyricLine[] = [];
  const plain: string[] = [];
  let offset = 0;
  for (const raw of source.replace(/\r\n?/g, "\n").split("\n")) {
    const line = raw.trim();
    const meta = tag.exec(line);
    if (meta) {
      if (meta[1].toLowerCase() === "offset") offset = Number(meta[2]) / 1000 || 0;
      continue;
    }
    const times: number[] = [];
    let match: RegExpExecArray | null;
    stamp.lastIndex = 0;
    let rest = line;
    while ((match = stamp.exec(line)) !== null) {
      const fraction = match[3] ? Number(match[3].padEnd(3, "0")) / 1000 : 0;
      times.push(Number(match[1]) * 60 + Number(match[2]) + fraction);
      rest = line.slice(stamp.lastIndex);
    }
    if (times.length > 0) {
      const text = rest.replace(stamp, "").trim();
      for (const time of times) timed.push({ time: Math.max(0, time - offset), text });
    } else {
      plain.push(raw.trimEnd());
    }
  }
  if (timed.length > 0) {
    timed.sort((a, b) => a.time - b.time);
    return { timed: true, lines: timed };
  }
  while (plain.length > 0 && plain[0] === "") plain.shift();
  while (plain.length > 0 && plain[plain.length - 1] === "") plain.pop();
  return { timed: false, lines: plain.map((text) => ({ time: 0, text })) };
}

/** Index of the line being sung at `position`, or -1 before the first line. */
export function activeLine(lines: LyricLine[], position: number): number {
  let low = 0;
  let high = lines.length - 1;
  let found = -1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    if (lines[middle].time <= position + 0.15) {
      found = middle;
      low = middle + 1;
    } else {
      high = middle - 1;
    }
  }
  return found;
}
