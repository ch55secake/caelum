import { useSyncExternalStore } from "react";

const listeners = new Set<() => void>();
let now = Date.now();
let timer: ReturnType<typeof setInterval> | undefined;
const tick = () => {
  now = Date.now();
  for (const listener of listeners) listener();
};
const visibility = () => {
  clearInterval(timer);
  timer = undefined;
  if (!document.hidden && listeners.size) {
    tick();
    timer = setInterval(tick, 1000);
  }
};
const subscribe = (listener: () => void) => {
  listeners.add(listener);
  if (listeners.size === 1) {
    document.addEventListener("visibilitychange", visibility);
    visibility();
  }
  return () => {
    listeners.delete(listener);
    if (!listeners.size) {
      clearInterval(timer);
      timer = undefined;
      document.removeEventListener("visibilitychange", visibility);
    }
  };
};
const idle = () => () => {};
const snapshot = () => now;

// One ticker for every visible timer, with no polling while the app is hidden
// or all executions have finished. Ticks never update workspace state.
export function useClock(active: boolean) {
  const time = useSyncExternalStore(active ? subscribe : idle, snapshot);
  return active ? Math.max(time, Date.now()) : Date.now();
}

export function elapsedTime(milliseconds: number) {
  const seconds = Math.max(0, Math.floor(milliseconds / 1000));
  if (seconds >= 3600)
    return `${Math.floor(seconds / 3600)}h ${Math.floor((seconds % 3600) / 60)}m`;
  return seconds >= 60
    ? `${Math.floor(seconds / 60)}m ${seconds % 60}s`
    : `${seconds}s`;
}
