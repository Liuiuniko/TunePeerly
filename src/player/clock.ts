// Original work © 2026 zhebuilds. AGPLv3 plus ADDITIONAL_TERMS.md.
type Clock = { position: number; duration: number };
type Transport = { playing: boolean };

/**
 * The player engine writes this computer's playback here. While another device is the active
 * playback device, `show` puts that device's playback on screen instead, so every progress bar and
 * play button follows the device that is really playing.
 */
function createStore<T>(initial: T) {
  let local = initial;
  let shown: T | null = null;
  const listeners = new Set<() => void>();
  const notify = () => listeners.forEach((listener) => listener());
  return {
    get: () => shown ?? local,
    set: (next: T) => {
      local = next;
      if (shown === null) notify();
    },
    show: (next: T | null) => {
      if (next === null && shown === null) return;
      shown = next;
      notify();
    },
    subscribe: (listener: () => void) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}

export const clock = createStore<Clock>({ position: 0, duration: 0 });
export const transport = createStore<Transport>({ playing: false });
