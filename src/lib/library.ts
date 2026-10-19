import type { MemberRef, Track } from "../api";
import type { TrackRef } from "../player/advance";

export function trackKey(track: Pick<Track, "contentId" | "localFileId">): string {
  return track.contentId ?? track.localFileId ?? "";
}

export function refOf(track: Track): TrackRef {
  return { key: trackKey(track), contentId: track.contentId, localFileId: track.localFileId };
}

/** Finds tracks by content id first, then by the local file id they had before hashing. */
export class TrackIndex {
  private byContent = new Map<string, Track>();
  private byLocal = new Map<string, Track>();

  constructor(tracks: Track[]) {
    for (const track of tracks) {
      if (track.contentId) this.byContent.set(track.contentId, track);
      if (track.localFileId) this.byLocal.set(track.localFileId, track);
    }
  }

  get(ref: { contentId?: string | null; localFileId?: string | null; key?: string } | null | undefined): Track | undefined {
    if (!ref) return undefined;
    return (
      (ref.contentId ? this.byContent.get(ref.contentId) : undefined) ??
      (ref.localFileId ? this.byLocal.get(ref.localFileId) : undefined) ??
      (ref.key ? this.byContent.get(ref.key) ?? this.byLocal.get(ref.key) : undefined)
    );
  }

  members(refs: MemberRef[]): Track[] {
    const out: Track[] = [];
    for (const ref of refs) {
      const track = this.get(ref);
      if (track) out.push(track);
    }
    return out;
  }
}

export type SortKey = "order" | "title" | "artist" | "album" | "folder" | "addedAt" | "favorite" | "duration" | "trackNumber";
export type Sort = { key: SortKey; dir: 1 | -1 };

const collator = new Intl.Collator(undefined, { sensitivity: "base", numeric: true });

export function displayTitle(track: Track, untitled: string): string {
  return track.title || track.fileName || untitled;
}

/** Moves one visible row, leaving hidden rows in their places. `from` and `to` count only visible rows. */
export function reorderVisible<T>(items: T[], visible: (item: T) => boolean, from: number, to: number): T[] {
  const slots: number[] = [];
  items.forEach((item, index) => {
    if (visible(item)) slots.push(index);
  });
  if (from < 0 || to < 0 || from >= slots.length || to >= slots.length || from === to) return items;
  const shown = slots.map((index) => items[index]);
  const [moved] = shown.splice(from, 1);
  shown.splice(to, 0, moved);
  const next = items.slice();
  slots.forEach((index, at) => {
    next[index] = shown[at];
  });
  return next;
}

export function sortTracks(tracks: Track[], sort: Sort): Track[] {
  if (sort.key === "order") return sort.dir === 1 ? tracks : tracks.slice().reverse();
  const value = (track: Track): string | number => {
    switch (sort.key) {
      case "favorite":
        return track.favorite ? 0 : 1;
      case "duration":
        return track.durationMs ?? 0;
      case "trackNumber":
        return track.trackNumber ?? Number.MAX_SAFE_INTEGER;
      case "addedAt":
        return track.addedAt;
      case "title":
      case "artist":
      case "album":
      case "folder":
        return track[sort.key] || track.fileName;
      default:
        return 0;
    }
  };
  return tracks.slice().sort((left, right) => {
    const a = value(left);
    const b = value(right);
    const primary = typeof a === "number" && typeof b === "number" ? a - b : collator.compare(String(a), String(b));
    if (primary !== 0) return primary * sort.dir;
    return collator.compare(left.title, right.title);
  });
}

export function filterText(tracks: Track[], text: string): Track[] {
  const tokens = text.toLowerCase().split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return tracks;
  return tracks.filter((track) => {
    const haystack = `${track.title} ${track.artist} ${track.album} ${track.fileName}`.toLowerCase();
    return tokens.every((token) => haystack.includes(token));
  });
}

export type Group = { name: string; tracks: Track[]; cover: string | null };

let coverSeed = Math.floor(Math.random() * 0x7fffffff) || 1;

/** The next refresh of the interface picks a different cover for each collection. */
export function reshuffleCovers() {
  coverSeed = Math.floor(Math.random() * 0x7fffffff) || 1;
}

function mix(seed: number, value: string): number {
  let hash = seed >>> 0;
  for (let index = 0; index < value.length; index += 1) hash = Math.imul(hash ^ value.charCodeAt(index), 0x5bd1e995);
  return hash >>> 0;
}

export function groupTracks(tracks: Track[], field: "artist" | "album" | "folder"): Group[] {
  const groups = new Map<string, Group & { rank: number }>();
  for (const track of tracks) {
    const name = track[field] || "";
    if (field === "folder" && !name) continue;
    let group = groups.get(name);
    if (!group) {
      group = { name, tracks: [], cover: null, rank: 0 };
      groups.set(name, group);
    }
    group.tracks.push(track);
    if (!track.coverUrl) continue;
    const rank = mix(coverSeed, `${name}\0${track.coverUrl}`);
    if (!group.cover || rank < group.rank) {
      group.cover = track.coverUrl;
      group.rank = rank;
    }
  }
  return [...groups.values()].sort((a, b) => {
    if (!a.name) return 1;
    if (!b.name) return -1;
    return collator.compare(a.name, b.name);
  });
}

/** Up to `limit` different covers. `salt` keeps two lists from picking the same ones. The choice changes when `reshuffleCovers` runs. */
export function pickCovers(urls: (string | null | undefined)[], limit = 4, salt = ""): string[] {
  const unique: string[] = [];
  for (const url of urls) {
    if (url && !unique.includes(url)) unique.push(url);
  }
  if (unique.length <= limit) return unique;
  return unique
    .map((url) => ({ url, rank: mix(coverSeed, `${salt}\0${url}`) }))
    .sort((a, b) => a.rank - b.rank || (a.url < b.url ? -1 : 1))
    .slice(0, limit)
    .map((item) => item.url);
}

export function covers(tracks: Track[], limit = 4, salt = ""): string[] {
  return pickCovers(tracks.map((track) => track.coverUrl), limit, salt);
}

export function folderName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function problemGroups(tracks: Track[]): { access: Track[]; playback: Track[] } {
  const local = tracks.filter((track) => track.localFileId);
  return {
    access: local.filter((track) => track.accessStatus !== "available"),
    playback: local.filter((track) => track.accessStatus === "available" && (track.playbackStatus === "failed" || track.video)),
  };
}

/** Can this track be put into the library-wide shuffle right now. */
export function shufflable(track: Track): boolean {
  return track.playable && !track.video && track.badge !== "unavailable" && track.playbackStatus !== "failed";
}
