export type OrderMode = "manual" | "listShuffle" | "libraryRandom";
export type RepeatMode = "off" | "list" | "one";

export type TrackRef = {
  key: string;
  contentId: string | null;
  localFileId: string | null;
};

export type QueueState = {
  order: OrderMode;
  repeat: RepeatMode;
  contextId: string;
  past: TrackRef[];
  current: TrackRef | null;
  future: TrackRef[];
  positionSec: number;
};

export type FailureClass = "transport" | "decode" | "unsupported";

export type PlaySource = {
  deviceId: string;
  pathId: string;
  remote: boolean;
};

export function fisherYates<T>(items: T[], random: () => number): T[] {
  const copy = items.slice();
  for (let index = copy.length - 1; index > 0; index -= 1) {
    const swap = Math.floor(random() * (index + 1));
    const current = copy[index];
    copy[index] = copy[swap];
    copy[swap] = current;
  }
  return copy;
}

export function sameTrack(left: TrackRef | null, right: TrackRef | null): boolean {
  if (!left || !right) return false;
  return left.key === right.key;
}

export function playFromList(
  state: QueueState,
  tracks: TrackRef[],
  index: number,
  shuffle: boolean,
  random: () => number,
): QueueState {
  const current = tracks[index] ?? null;
  const after = tracks.slice(index + 1);
  const future = shuffle || state.order === "listShuffle" ? fisherYates(after, random) : after;
  return {
    ...state,
    order: shuffle ? "listShuffle" : state.order === "libraryRandom" ? "manual" : state.order,
    past: state.current ? [...state.past, state.current] : state.past,
    current,
    future,
    positionSec: 0,
  };
}

export function startLibraryRandom(tracks: TrackRef[], random: () => number, contextId: string): QueueState {
  const shuffled = fisherYates(tracks, random);
  return {
    order: "libraryRandom",
    repeat: "off",
    contextId,
    past: [],
    current: shuffled[0] ?? null,
    future: shuffled.slice(1),
    positionSec: 0,
  };
}

export function onEnded(
  state: QueueState,
  rebuild: TrackRef[],
  random: () => number,
): { state: QueueState; increment: boolean } {
  if (state.repeat === "one" && state.current) {
    return { state: { ...state, positionSec: 0 }, increment: true };
  }
  if (state.future.length > 0) {
    const [current, ...future] = state.future;
    return {
      increment: true,
      state: {
        ...state,
        past: state.current ? [...state.past, state.current] : state.past,
        current,
        future,
        positionSec: 0,
      },
    };
  }
  if (state.repeat === "list") {
    const pool = state.order === "manual" ? rebuild : fisherYates(rebuild, random);
    const [current, ...future] = pool;
    return {
      increment: Boolean(current),
      state: {
        ...state,
        past: state.current ? [...state.past, state.current] : state.past,
        current: current ?? null,
        future,
        positionSec: 0,
      },
    };
  }
  return { state, increment: false };
}

export function previous(state: QueueState, positionSec: number): QueueState {
  if (positionSec > 3) {
    return { ...state, positionSec: 0 };
  }
  const prior = state.past[state.past.length - 1];
  if (!prior) return { ...state, positionSec: 0 };
  return {
    ...state,
    past: state.past.slice(0, -1),
    current: prior,
    future: state.current ? [state.current, ...state.future] : state.future,
    positionSec: 0,
  };
}

export function next(state: QueueState): QueueState {
  if (state.future.length === 0) return state;
  const [current, ...future] = state.future;
  return {
    ...state,
    past: state.current ? [...state.past, state.current] : state.past,
    current,
    future,
    positionSec: 0,
  };
}

export function playNext(state: QueueState, tracks: TrackRef | TrackRef[]): QueueState {
  const items = Array.isArray(tracks) ? tracks : [tracks];
  return { ...state, future: [...items, ...state.future] };
}

export function addToQueue(state: QueueState, tracks: TrackRef | TrackRef[]): QueueState {
  const items = Array.isArray(tracks) ? tracks : [tracks];
  return { ...state, future: [...state.future, ...items] };
}

/** Removes one queue entry by position, so other copies of the same song stay. */
export function removeAt(state: QueueState, index: number): QueueState {
  if (index < 0 || index >= state.future.length) return state;
  return { ...state, future: state.future.filter((_, at) => at !== index) };
}

export function moveInQueue(state: QueueState, from: number, to: number): QueueState {
  if (from === to || from < 0 || from >= state.future.length) return state;
  const future = state.future.slice();
  const [item] = future.splice(from, 1);
  future.splice(Math.max(0, Math.min(to, future.length)), 0, item);
  return { ...state, future };
}

/** Jumps to a queued entry; everything before it moves to `past`. */
export function jumpTo(state: QueueState, index: number): QueueState {
  if (index < 0 || index >= state.future.length) return state;
  const skipped = state.future.slice(0, index);
  return {
    ...state,
    past: [...state.past, ...(state.current ? [state.current] : []), ...skipped],
    current: state.future[index],
    future: state.future.slice(index + 1),
    positionSec: 0,
  };
}

export function toggleShuffle(state: QueueState, context: TrackRef[], random: () => number): QueueState {
  if (state.order === "manual") {
    return { ...state, order: "listShuffle", future: fisherYates(state.future, random) };
  }
  const index = context.findIndex((track) => sameTrack(track, state.current));
  const future = index >= 0 ? context.slice(index + 1) : context;
  return { ...state, order: "manual", future };
}

export function cycleRepeat(mode: RepeatMode): RepeatMode {
  if (mode === "off") return "list";
  if (mode === "list") return "one";
  return "off";
}

export function acceptGeneration(current: number, eventGeneration: number): boolean {
  return current === eventGeneration;
}

export function shouldIncrementGeneration(reason: "track" | "device" | "repeat" | "path"): boolean {
  return reason !== "path";
}

export function selectSource(
  sources: PlaySource[],
  failure: FailureClass,
  failed: PlaySource | null,
  incognito: boolean,
): PlaySource | null {
  if (!failed) return sources[0] ?? null;
  if (failure === "decode" || failure === "unsupported") return null;
  if (incognito && failed.remote) return null;
  const start = sources.findIndex(
    (source) => source.deviceId === failed.deviceId && source.pathId === failed.pathId,
  );
  const rest = sources.slice(start >= 0 ? start + 1 : 0);
  const sameDevice = rest.find((source) => source.deviceId === failed.deviceId);
  if (sameDevice) return sameDevice;
  if (incognito) return null;
  return rest.find((source) => source.deviceId !== failed.deviceId) ?? null;
}
