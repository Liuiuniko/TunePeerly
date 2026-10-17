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

export function groupTracks(tracks: Track[], field: "artist" | "album" | "folder"): Group[] {
  const groups = new Map<string, Group>();
  for (const track of tracks) {
    const name = track[field] || "";
    if (field === "folder" && !name) continue;
    let group = groups.get(name);
    if (!group) {
      group = { name, tracks: [], cover: null };
      groups.set(name, group);
    }
    group.tracks.push(track);
    if (!group.cover && track.coverUrl) group.cover = track.coverUrl;
  }
  return [...groups.values()].sort((a, b) => {
    if (!a.name) return 1;
    if (!b.name) return -1;
    return collator.compare(a.name, b.name);
  });
}

export function covers(tracks: Track[], limit = 4): string[] {
  const out: string[] = [];
  for (const track of tracks) {
    if (track.coverUrl && !out.includes(track.coverUrl)) out.push(track.coverUrl);
    if (out.length >= limit) break;
  }
  return out;
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
