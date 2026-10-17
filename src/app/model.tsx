import { listen } from "@tauri-apps/api/event";
import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { api, errorText, pickFiles, pickFolder, type Bootstrap, type ContentMove, type MemberRef, type Settings, type Track } from "../api";
import i18n from "../i18n";
import { filterText, refOf, shufflable, trackKey, TrackIndex } from "../lib/library";
import {
  addToQueue,
  cycleRepeat,
  jumpTo,
  moveInQueue,
  next as nextState,
  onEnded,
  playFromList,
  playNext,
  previous as previousState,
  removeAt,
  startLibraryRandom,
  toggleShuffle,
  type QueueState,
  type TrackRef,
} from "../player/advance";
import { PlaybackEngine, type EngineEvents } from "../player/engine";

export type RouteName =
  | "home"
  | "search"
  | "library"
  | "liked"
  | "recent"
  | "artists"
  | "albums"
  | "folders"
  | "playlist"
  | "artist"
  | "album"
  | "folder"
  | "device"
  | "devices"
  | "settings"
  | "problems"
  | "now";
export type Route = { name: RouteName; id?: string };

export type MenuState = { x: number; y: number; tracks: Track[]; playlistId?: string; queueIndex?: number; playlistMenu?: string; play?: () => void } | null;

export type DialogState =
  | { kind: "confirm"; title: string; body: string; action: string; danger?: boolean; run: () => void | Promise<void> }
  | { kind: "prompt"; title: string; label: string; value: string; action: string; run: (value: string) => void | Promise<void> }
  | null;

export type Toast = { id: number; text: string };

const emptyQueue: QueueState = { order: "manual", repeat: "off", contextId: "library", past: [], current: null, future: [], positionSec: 0 };
const PAST_LIMIT = 200;

function asRef(value: unknown): TrackRef | null {
  if (!value || typeof value !== "object") return null;
  const item = value as Record<string, unknown>;
  const contentId = typeof item.contentId === "string" ? item.contentId : null;
  const localFileId = typeof item.localFileId === "string" ? item.localFileId : null;
  const key = typeof item.key === "string" ? item.key : contentId ?? localFileId;
  return key ? { key, contentId, localFileId } : null;
}

function refs(value: unknown): TrackRef[] {
  return Array.isArray(value) ? value.map(asRef).filter((item): item is TrackRef => item !== null) : [];
}

function normalizeQueue(raw: Record<string, unknown> | undefined): QueueState {
  if (!raw) return emptyQueue;
  const order = raw.order === "listShuffle" || raw.order === "libraryRandom" ? raw.order : "manual";
  const repeat = raw.repeat === "list" || raw.repeat === "one" ? raw.repeat : "off";
  return {
    order,
    repeat,
    contextId: typeof raw.contextId === "string" ? raw.contextId : "library",
    past: refs(raw.past).slice(-PAST_LIMIT),
    current: asRef(raw.current),
    future: refs(raw.future),
    positionSec: Number(raw.positionSec) > 0 ? Number(raw.positionSec) : 0,
  };
}

function sameRoute(left: Route, right: Route): boolean {
  return left.name === right.name && (left.id ?? "") === (right.id ?? "");
}

function useModel() {
  const { t } = useTranslation();
  const [boot, setBoot] = useState<Bootstrap | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [members, setMembers] = useState<Record<string, MemberRef[]>>({});
  const [queue, setQueueState] = useState<QueueState>(emptyQueue);
  const [route, setRoute] = useState<Route>({ name: "home" });
  const [navigation, setNavigation] = useState({ canBack: false, canForward: false });
  const [queueOpen, setQueueOpen] = useState(false);
  const [menu, setMenu] = useState<MenuState>(null);
  const [dialog, setDialog] = useState<DialogState>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [notice, setNoticeState] = useState<string | null>(null);
  const [moves, setMoves] = useState<ContentMove[]>([]);
  const [volume, setVolumeState] = useState(1);
  const [muted, setMuted] = useState(false);
  const [lyrics, setLyrics] = useState<{ key: string; text: string } | null>(null);
  const [failure, setFailure] = useState<{ key: string; reason: string } | null>(null);
  const [search, setSearch] = useState("");

  const queueRef = useRef(queue);
  const bootRef = useRef(boot);
  bootRef.current = boot;
  const membersRef = useRef(members);
  membersRef.current = members;
  const restored = useRef(false);
  const context = useRef<{ id: string; tracks: Track[] | null }>({ id: "library", tracks: null });
  const nav = useRef<{ stack: Route[]; index: number }>({ stack: [{ name: "home" }], index: 0 });
  const toastId = useRef(0);
  const noticeTimer = useRef<number | undefined>(undefined);
  const volumeTimer = useRef<number | undefined>(undefined);
  const refreshTimer = useRef<number | undefined>(undefined);

  const tracks = useMemo(() => boot?.tracks ?? [], [boot]);
  const index = useMemo(() => new TrackIndex(tracks), [tracks]);
  const indexRef = useRef(index);
  indexRef.current = index;
  const settings: Settings = boot?.settings ?? {};
  const showUnavailable = settings.showUnavailable !== false;

  const library = useMemo(() => (showUnavailable ? tracks : tracks.filter((track) => track.badge !== "unavailable")), [tracks, showUnavailable]);
  const liked = useMemo(() => tracks.filter((track) => track.favorite), [tracks]);
  const recent = useMemo(() => {
    const seen = new Set<string>();
    const out: Track[] = [];
    for (const key of boot?.history ?? []) {
      const track = index.get({ key });
      if (track && !seen.has(trackKey(track))) {
        seen.add(trackKey(track));
        out.push(track);
      }
    }
    return out;
  }, [boot?.history, index]);
  const current = index.get(queue.current);

  const handlers = useRef<EngineEvents | null>(null);
  const engineRef = useRef<PlaybackEngine | null>(null);
  if (!engineRef.current) {
    engineRef.current = new PlaybackEngine({
      ended: () => handlers.current?.ended(),
      failed: (track, info) => handlers.current?.failed(track, info),
      heard: (track) => handlers.current?.heard(track),
      paused: (track, position) => handlers.current?.paused(track, position),
      seeked: (track, position, paused) => handlers.current?.seeked(track, position, paused),
      progress: (position) => handlers.current?.progress(position),
      activity: (playing, track, enough) => handlers.current?.activity(playing, track, enough),
      playable: (track) => handlers.current?.playable(track),
    });
  }
  const engine = engineRef.current;

  const toast = useCallback((text: string) => {
    const id = ++toastId.current;
    setToasts((items) => [...items.slice(-2), { id, text }]);
    window.setTimeout(() => setToasts((items) => items.filter((item) => item.id !== id)), 2600);
  }, []);

  const setNotice = useCallback((text: string | null) => {
    window.clearTimeout(noticeTimer.current);
    setNoticeState(text);
    if (text) noticeTimer.current = window.setTimeout(() => setNoticeState(null), 8000);
  }, []);

  const say = useCallback((error: unknown) => {
    const text = errorText(error);
    setNotice(i18n.exists(text) ? t(text) : text);
  }, [setNotice, t]);

  const applyLook = useCallback((theme: string, language: string) => {
    document.documentElement.dataset.theme = theme === "dark" ? "dark" : "light";
    const lang = language === "zh" ? "zh" : "en";
    document.documentElement.lang = lang === "zh" ? "zh-CN" : "en";
    if (i18n.language !== lang) void i18n.changeLanguage(lang);
  }, []);

  const loadMembers = useCallback(async (playlists: Bootstrap["playlists"]) => {
    const rows = await Promise.all(
      playlists.map((playlist) => api.playlistMembers(playlist.id).then((items) => [playlist.id, items] as const).catch(() => [playlist.id, []] as const)),
    );
    setMembers(Object.fromEntries(rows));
  }, []);

  const refresh = useCallback(async () => {
    try {
      const next = await api.bootstrap();
      setBoot(next);
      setLoadError(null);
      applyLook(next.theme, next.language);
      void loadMembers(next.playlists);
      if (!restored.current) {
        restored.current = true;
        const state = normalizeQueue(next.player);
        queueRef.current = state;
        setQueueState(state);
        const saved = Number(next.settings.volume);
        const level = Number.isFinite(saved) ? Math.min(1, Math.max(0, saved)) : 1;
        setVolumeState(level);
        engine.setVolume(level, false);
        context.current = { id: state.contextId, tracks: null };
        engine.cue(new TrackIndex(next.tracks).get(state.current) ?? null, state.positionSec);
      }
    } catch (error) {
      setLoadError(errorText(error));
    }
  }, [applyLook, engine, loadMembers]);

  const scheduleRefresh = useCallback(() => {
    window.clearTimeout(refreshTimer.current);
    refreshTimer.current = window.setTimeout(() => void refresh(), 150);
  }, [refresh]);

  useEffect(() => {
    void refresh();
    const stops = Promise.all([
      listen("library-updated", scheduleRefresh),
      listen("network-updated", scheduleRefresh),
      listen<unknown>("queue-updated", (event) => {
        const future = refs(event.payload);
        const state = { ...queueRef.current, future };
        queueRef.current = state;
        setQueueState(state);
      }),
      listen<ContentMove>("content-moved", (event) => setMoves((items) => [...items, event.payload])),
    ]).catch(() => [] as (() => void)[]);
    return () => {
      void stops.then((items) => items.forEach((stop) => stop()));
    };
  }, [refresh, scheduleRefresh]);

  const commit = useCallback((state: QueueState, position: number = engine.position) => {
    const trimmed = state.past.length > PAST_LIMIT ? { ...state, past: state.past.slice(-PAST_LIMIT) } : state;
    queueRef.current = trimmed;
    setQueueState(trimmed);
    void api.savePlayer({ ...trimmed, positionSec: position }).catch(() => {});
  }, [engine]);

  const contextTracks = useCallback((id: string): Track[] => {
    const all = bootRef.current?.tracks ?? [];
    const visible = bootRef.current?.settings.showUnavailable !== false ? all : all.filter((track) => track.badge !== "unavailable");
    const [kind, ...rest] = id.split(":");
    const value = rest.join(":");
    switch (kind) {
      case "liked":
        return all.filter((track) => track.favorite);
      case "playlist":
        return indexRef.current.members(membersRef.current[value] ?? []);
      case "artist":
        return visible.filter((track) => track.artist === value);
      case "album":
        return visible.filter((track) => track.album === value).sort((a, b) => (a.trackNumber ?? 0) - (b.trackNumber ?? 0));
      case "folder":
        return visible.filter((track) => track.folder === value);
      case "device":
        return all.filter((track) => track.holderIds.includes(value));
      case "random":
        return all.filter(shufflable);
      default:
        return visible;
    }
  }, []);

  const rebuildPool = useCallback((state: QueueState): Track[] => {
    if (state.order === "libraryRandom") return (bootRef.current?.tracks ?? []).filter(shufflable);
    const list = context.current.id === state.contextId && context.current.tracks ? context.current.tracks : contextTracks(state.contextId);
    return list.filter((track) => track.playable);
  }, [contextTracks]);

  const loadLyrics = useCallback((track: Track | undefined) => {
    if (!track?.localFileId) {
      setLyrics(null);
      return;
    }
    const key = trackKey(track);
    void api.lyrics(track.localFileId).then((text) => setLyrics({ key, text })).catch(() => setLyrics({ key, text: "" }));
  }, []);

  const leave = useCallback(() => {
    const playing = engine.current;
    if (!playing?.contentId || !bootRef.current?.settings.positionSync) return;
    void api.network("position", { contentId: playing.contentId, seconds: engine.position }).catch(() => {});
  }, [engine]);

  const startCurrent = useCallback((state: QueueState) => {
    const track = indexRef.current.get(state.current);
    setFailure(null);
    if (!track) {
      engine.cue(null, 0);
      setLyrics(null);
      return;
    }
    void engine.start(track, state.positionSec, true);
    loadLyrics(track);
  }, [engine, loadLyrics]);

  const playList = useCallback((list: Track[], at: number, contextId: string, shuffle = false) => {
    const chosen = list[at];
    if (!chosen) return;
    if (!chosen.playable) {
      setNotice(t(chosen.reasonKey ?? "notice.offlineTrack"));
      return;
    }
    const pool = list.filter((track, position) => position === at || track.playable);
    context.current = { id: contextId, tracks: pool };
    leave();
    const state = playFromList({ ...queueRef.current, contextId }, pool.map(refOf), pool.indexOf(chosen), shuffle, Math.random);
    commit(state, 0);
    startCurrent(state);
  }, [commit, leave, setNotice, startCurrent, t]);

  const playAll = useCallback((list: Track[], contextId: string, shuffle = false) => {
    const playable = list.filter((track) => track.playable);
    if (playable.length === 0) {
      setNotice(t("notice.nothingPlayable"));
      return;
    }
    const start = shuffle ? list.indexOf(playable[Math.floor(Math.random() * playable.length)]) : list.indexOf(playable[0]);
    playList(list, start, contextId, shuffle);
  }, [playList, setNotice, t]);

  const shuffleLibrary = useCallback(() => {
    const pool = (bootRef.current?.tracks ?? []).filter(shufflable);
    if (pool.length === 0) {
      setNotice(t("notice.nothingPlayable"));
      return;
    }
    context.current = { id: "random", tracks: pool };
    leave();
    const state = { ...startLibraryRandom(pool.map(refOf), Math.random, "random"), repeat: queueRef.current.repeat };
    commit(state, 0);
    startCurrent(state);
  }, [commit, leave, setNotice, startCurrent, t]);

  const next = useCallback(() => {
    const state = queueRef.current;
    let target: QueueState | null = null;
    if (state.future.length > 0) target = nextState(state);
    else if (state.repeat !== "off" && state.current) {
      const result = onEnded({ ...state, repeat: "list" }, rebuildPool(state).map(refOf), Math.random);
      target = result.increment ? { ...result.state, repeat: state.repeat } : null;
    }
    if (!target) return;
    leave();
    commit(target, 0);
    startCurrent(target);
  }, [commit, leave, rebuildPool, startCurrent]);

  const previous = useCallback(() => {
    const state = queueRef.current;
    if (!state.current) return;
    const target = previousState(state, engine.position);
    if (target.current?.key === state.current.key && target.past.length === state.past.length) {
      engine.seek(0);
      return;
    }
    leave();
    commit(target, 0);
    startCurrent(target);
  }, [commit, engine, leave, startCurrent]);

  const toggle = useCallback(() => {
    const state = queueRef.current;
    if (!state.current) {
      if (state.future.length > 0) next();
      else playAll(library, "library");
      return;
    }
    const track = indexRef.current.get(state.current);
    if (!engine.hasSource && track) {
      // Retry with the freshest row: the file may have come back or another device may hold it now.
      const resume = engine.current && trackKey(engine.current) === trackKey(track) ? engine.position : state.positionSec;
      setFailure(null);
      engine.cue(track, resume);
    }
    engine.toggle();
  }, [engine, library, loadLyrics, next, playAll]);

  const jump = useCallback((at: number) => {
    const target = jumpTo(queueRef.current, at);
    leave();
    commit(target, 0);
    startCurrent(target);
  }, [commit, leave, startCurrent]);

  const queueActions = useMemo(() => ({
    playNext: (items: Track[]) => {
      commit(playNext(queueRef.current, items.map(refOf)));
      toast(items.length === 1 ? t("toast.playNext") : t("toast.playNextMany", { count: items.length }));
    },
    add: (items: Track[]) => {
      commit(addToQueue(queueRef.current, items.map(refOf)));
      toast(items.length === 1 ? t("toast.queued") : t("toast.queuedMany", { count: items.length }));
    },
    insert: (at: number, items: Track[]) => {
      const state = queueRef.current;
      const position = Math.max(0, Math.min(at, state.future.length));
      commit({ ...state, future: [...state.future.slice(0, position), ...items.map(refOf), ...state.future.slice(position)] });
      toast(items.length === 1 ? t("toast.queued") : t("toast.queuedMany", { count: items.length }));
    },
    remove: (at: number) => commit(removeAt(queueRef.current, at)),
    move: (from: number, to: number) => commit(moveInQueue(queueRef.current, from, to)),
    clear: () => commit({ ...queueRef.current, future: [] }),
    jump,
  }), [commit, jump, t, toast]);

  const toggleShuffleMode = useCallback(() => {
    const state = queueRef.current;
    const pool = context.current.id === state.contextId && context.current.tracks ? context.current.tracks : contextTracks(state.contextId);
    commit(toggleShuffle(state, pool.filter((track) => track.playable).map(refOf), Math.random));
  }, [commit, contextTracks]);

  const cycleRepeatMode = useCallback(() => {
    commit({ ...queueRef.current, repeat: cycleRepeat(queueRef.current.repeat) });
  }, [commit]);

  const setVolume = useCallback((value: number) => {
    const level = Math.min(1, Math.max(0, value));
    setVolumeState(level);
    setMuted(false);
    engine.setVolume(level, false);
    window.clearTimeout(volumeTimer.current);
    volumeTimer.current = window.setTimeout(() => void api.updateSetting("volume", level).catch(() => {}), 400);
  }, [engine]);

  const toggleMute = useCallback(() => {
    setMuted((value) => {
      engine.setVolume(volume, !value);
      return !value;
    });
  }, [engine, volume]);

  const patchBoot = useCallback((patch: (value: Bootstrap) => Bootstrap) => {
    setBoot((value) => (value ? patch(value) : value));
  }, []);

  const setSetting = useCallback(async (key: string, value: unknown) => {
    const before = bootRef.current;
    if (!before) return;
    if (key === "theme" || key === "language") {
      patchBoot((item) => ({ ...item, [key]: value }));
      applyLook(key === "theme" ? String(value) : before.theme, key === "language" ? String(value) : before.language);
    } else {
      patchBoot((item) => ({ ...item, settings: { ...item.settings, [key]: value } }));
    }
    if (key === "incognito" && value === true && engine.remoteDevice) {
      engine.halt();
      setNotice(t("notice.incognitoStopped"));
    }
    try {
      await api.updateSetting(key, value);
      if (key === "incognito") void api.network("close-route").catch(() => {});
      if (key === "showUnavailable" || key === "historyCap" || key === "incognito") scheduleRefresh();
    } catch (error) {
      setBoot(before);
      applyLook(before.theme, before.language);
      say(error);
    }
  }, [applyLook, engine, patchBoot, say, scheduleRefresh, setNotice, t]);

  const importPaths = useCallback(async (paths: string[], kind: "directory" | "file" | "auto") => {
    if (paths.length === 0) return;
    try {
      const summaries = await api.importPaths(paths, kind);
      const added = summaries.reduce((sum, item) => sum + item.added, 0);
      toast(added > 0 ? t("toast.imported", { count: added }) : t("toast.importedNone"));
      await refresh();
    } catch (error) {
      say(error);
    }
  }, [refresh, say, t, toast]);

  const addMusic = useCallback(async (kind: "directory" | "file") => {
    try {
      const paths = kind === "directory" ? [await pickFolder()].filter((item): item is string => Boolean(item)) : await pickFiles();
      await importPaths(paths, kind);
    } catch (error) {
      say(error);
    }
  }, [importPaths, say]);

  const like = useCallback(async (track: Track) => {
    const key = trackKey(track);
    const flip = (value: boolean) =>
      patchBoot((item) => ({ ...item, tracks: item.tracks.map((row) => (trackKey(row) === key ? { ...row, favorite: value } : row)) }));
    flip(!track.favorite);
    try {
      const added = await api.toggleFavorite(track.contentId, track.localFileId);
      flip(added);
      toast(added ? t("toast.liked") : t("toast.unliked"));
    } catch (error) {
      flip(track.favorite);
      say(error);
    }
  }, [patchBoot, say, t, toast]);

  const addToPlaylist = useCallback(async (playlistId: string, items: Track[]) => {
    const playlist = bootRef.current?.playlists.find((item) => item.id === playlistId);
    let added = 0;
    let already = 0;
    for (const track of items) {
      try {
        await api.addToPlaylist(playlistId, track.contentId, track.localFileId);
        added += 1;
      } catch (error) {
        if (errorText(error) === "playlist.already") already += 1;
        else {
          say(error);
          break;
        }
      }
    }
    if (added > 0) toast(t("toast.added", { name: playlist?.name ?? "" }));
    else if (already > 0) toast(t("toast.already", { name: playlist?.name ?? "" }));
    scheduleRefresh();
  }, [say, scheduleRefresh, t, toast]);

  const navigate = useCallback((next: Route) => {
    const state = nav.current;
    if (sameRoute(state.stack[state.index], next)) return;
    state.stack = [...state.stack.slice(0, state.index + 1), next].slice(-60);
    state.index = state.stack.length - 1;
    setRoute(next);
    setMenu(null);
    setNavigation({ canBack: state.index > 0, canForward: false });
  }, []);

  const step = useCallback((delta: number) => {
    const state = nav.current;
    const target = state.index + delta;
    if (target < 0 || target >= state.stack.length) return;
    state.index = target;
    setRoute(state.stack[target]);
    setMenu(null);
    setNavigation({ canBack: target > 0, canForward: target < state.stack.length - 1 });
  }, []);

  const createPlaylist = useCallback((items: Track[] = [], open = false) => {
    setDialog({
      kind: "prompt",
      title: t("playlist.create"),
      label: t("playlist.name"),
      value: t("playlist.defaultName", { count: (bootRef.current?.playlists.length ?? 0) + 1 }),
      action: t("common.create"),
      run: async (name) => {
        try {
          const id = await api.createPlaylist(name);
          for (const track of items) await api.addToPlaylist(id, track.contentId, track.localFileId).catch(() => {});
          toast(items.length > 0 ? t("toast.added", { name }) : t("toast.created", { name }));
          await refresh();
          if (open) navigate({ name: "playlist", id });
        } catch (error) {
          say(error);
        }
      },
    });
  }, [navigate, refresh, say, t, toast]);

  const renamePlaylist = useCallback((id: string) => {
    const playlist = bootRef.current?.playlists.find((item) => item.id === id);
    if (!playlist) return;
    setDialog({
      kind: "prompt",
      title: t("playlist.rename"),
      label: t("playlist.name"),
      value: playlist.name,
      action: t("common.save"),
      run: async (name) => {
        try {
          await api.renamePlaylist(id, name);
          await refresh();
        } catch (error) {
          say(error);
        }
      },
    });
  }, [refresh, say, t]);

  const deletePlaylist = useCallback((id: string) => {
    const playlist = bootRef.current?.playlists.find((item) => item.id === id);
    if (!playlist) return;
    setDialog({
      kind: "confirm",
      title: t("playlist.deleteTitle", { name: playlist.name }),
      body: t("playlist.deleteBody"),
      action: t("common.delete"),
      danger: true,
      run: async () => {
        try {
          await api.deletePlaylist(id);
          if (route.name === "playlist" && route.id === id) navigate({ name: "home" });
          toast(t("toast.deleted", { name: playlist.name }));
          await refresh();
        } catch (error) {
          say(error);
        }
      },
    });
  }, [navigate, refresh, route, say, t, toast]);

  const removeFromPlaylist = useCallback(async (playlistId: string, items: Track[]) => {
    try {
      for (const track of items) await api.removePlaylistMember(playlistId, track.contentId, track.localFileId);
      const playlist = bootRef.current?.playlists.find((item) => item.id === playlistId);
      toast(t("toast.removedFrom", { name: playlist?.name ?? "" }));
      await refresh();
    } catch (error) {
      say(error);
    }
  }, [refresh, say, t, toast]);

  const movePlaylistMember = useCallback(async (playlistId: string, from: number, to: number) => {
    setMembers((value) => {
      const list = (value[playlistId] ?? []).slice();
      const [item] = list.splice(from, 1);
      if (item) list.splice(to, 0, item);
      return { ...value, [playlistId]: list };
    });
    try {
      await api.movePlaylistMember(playlistId, from, to);
    } catch (error) {
      say(error);
    }
    scheduleRefresh();
  }, [say, scheduleRefresh]);

  handlers.current = {
    ended: () => {
      const state = queueRef.current;
      const track = indexRef.current.get(state.current);
      if (state.repeat === "one" && track) {
        void engine.start(track, 0, true);
        return;
      }
      const result = onEnded(state, rebuildPool(state).map(refOf), Math.random);
      if (result.increment && result.state.current) {
        commit(result.state, 0);
        startCurrent(result.state);
      } else {
        commit({ ...state, positionSec: 0 }, 0);
        engine.cue(track ?? null, 0);
      }
    },
    failed: (track, info) => {
      setFailure({ key: trackKey(track), reason: info.reason });
      setNotice(t(info.reason, { defaultValue: t("notice.playbackFailed") }));
      if (track.localFileId && info.kind !== "offline" && info.kind !== "transport") {
        void api.playbackResult(track.localFileId, track.contentId, "failed", info.reason).then(scheduleRefresh).catch(() => {});
      }
    },
    heard: (track) => {
      if (bootRef.current?.settings.incognito) return;
      const key = trackKey(track);
      patchBoot((item) => ({ ...item, history: [key, ...item.history.filter((value) => value !== key)] }));
      void api.recordHistory(track.contentId, track.localFileId).catch(() => {});
    },
    paused: (track, position) => {
      void api.saveProgress(position).catch(() => {});
      if (track.contentId && bootRef.current?.settings.positionSync) {
        void api.network("position", { contentId: track.contentId, seconds: position }).catch(() => {});
      }
    },
    seeked: (track, position, paused) => {
      void api.saveProgress(position).catch(() => {});
      if (paused && track.contentId && bootRef.current?.settings.positionSync) {
        void api.network("position", { contentId: track.contentId, seconds: position }).catch(() => {});
      }
    },
    progress: (position) => void api.saveProgress(position).catch(() => {}),
    activity: (playing, track, bufferEnough) =>
      void api.network("activity", { playing, localFileId: track?.localFileId ?? null, bufferEnough }).catch(() => {}),
    playable: (track) => {
      if (track.playbackStatus !== "playable") void api.playbackResult(track.localFileId, track.contentId, "playable", null).catch(() => {});
    },
  };

  useEffect(() => {
    const flush = () => {
      void api.saveProgress(engine.position).catch(() => {});
      leave();
    };
    window.addEventListener("beforeunload", flush);
    return () => window.removeEventListener("beforeunload", flush);
  }, [engine, leave]);

  useEffect(() => {
    if (!current) {
      setLyrics(null);
      return;
    }
    if (lyrics?.key !== trackKey(current)) loadLyrics(current);
  }, [current?.contentId, current?.localFileId]);

  const findTracks = useCallback((text: string, scope: "all" | "queue" | string) => {
    if (scope === "queue") {
      const state = queueRef.current;
      return filterText([state.current, ...state.future].map((item) => indexRef.current.get(item)).filter((item): item is Track => Boolean(item)), text);
    }
    if (scope.startsWith("playlist:")) return filterText(indexRef.current.members(membersRef.current[scope.slice(9)] ?? []), text);
    return null;
  }, []);

  return {
    boot,
    loadError,
    settings,
    tracks,
    library,
    liked,
    recent,
    index,
    members,
    queue,
    current,
    engine,
    volume,
    muted,
    lyrics,
    failure,
    route,
    navigation,
    queueOpen,
    menu,
    dialog,
    toasts,
    notice,
    moves,
    search,
    setSearch,
    setQueueOpen,
    setMenu,
    setDialog,
    setNotice,
    setMoves,
    toast,
    say,
    refresh,
    scheduleRefresh,
    navigate,
    back: () => step(-1),
    forward: () => step(1),
    playList,
    playAll,
    shuffleLibrary,
    toggle,
    next,
    previous,
    queueActions,
    toggleShuffle: toggleShuffleMode,
    cycleRepeat: cycleRepeatMode,
    setVolume,
    toggleMute,
    setSetting,
    importPaths,
    addMusic,
    like,
    addToPlaylist,
    createPlaylist,
    renamePlaylist,
    deletePlaylist,
    removeFromPlaylist,
    movePlaylistMember,
    contextTracks,
    findTracks,
  };
}

export type AppModel = ReturnType<typeof useModel>;

const AppContext = createContext<AppModel | null>(null);

export function AppProvider({ children }: { children: ReactNode }) {
  const model = useModel();
  return <AppContext.Provider value={model}>{children}</AppContext.Provider>;
}

export function useApp(): AppModel {
  const value = useContext(AppContext);
  if (!value) throw new Error("AppProvider missing");
  return value;
}
