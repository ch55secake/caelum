import type { ProfilerOnRenderCallback } from "react";

// This module is only imported by the opt-in profiling build. Keep measurements
// bounded and retain counts/bytes, never prompt contents or command arguments.
export function startProfiling(): ProfilerOnRenderCallback {
  type Metric = { count: number; total: number; max: number; samples: number[] };
  const metric = (): Metric => ({ count: 0, total: 0, max: 0, samples: [] });
  const add = (target: Metric, value: number) => {
    target.count++;
    target.total += value;
    target.max = Math.max(target.max, value);
    // Fixed-size ring; percentiles describe the most recent 4096 observations.
    target.samples[(target.count - 1) % 4096] = value;
  };
  const describe = (target: Metric) => {
    const sorted = [...target.samples].sort((a, b) => a - b);
    const percentile = (fraction: number) =>
      sorted[Math.max(0, Math.ceil(sorted.length * fraction) - 1)] ?? null;
    return {
      count: target.count, total: target.total, max: target.max,
      p50: percentile(0.5), p95: percentile(0.95), sampled: sorted.length,
    };
  };
  const fresh = () => ({
    started: performance.now(),
    renders: metric(), frames: metric(), longTasks: metric(),
    commands: new Map<string, { count: number; requestBytes: number; responseBytes: number; failures: number; duration: Metric }>(),
    events: new Map<string, { count: number; bytes: number }>(),
  });
  let state = fresh();
  const encoder = new TextEncoder();
  const bytes = (value: unknown) => {
    try { return encoder.encode(JSON.stringify(value) ?? "").byteLength; }
    catch { return 0; }
  };
  const internals = (window as unknown as {
    __TAURI_INTERNALS__?: {
      invoke: (command: string, args?: unknown, options?: unknown) => Promise<unknown>;
      transformCallback: (callback: (value: any) => void, once?: boolean) => number;
    };
  }).__TAURI_INTERNALS__;
  if (internals) {
    const invoke = internals.invoke.bind(internals);
    internals.invoke = async (command, args, options) => {
      const started = performance.now();
      const key = state.commands.has(command) || state.commands.size < 128 ? command : "other";
      let record = state.commands.get(key);
      if (!record) {
        record = { count: 0, requestBytes: 0, responseBytes: 0, failures: 0, duration: metric() };
        state.commands.set(key, record);
      }
      record.count++;
      record.requestBytes += bytes(args);
      try {
        const result = await invoke(command, args, options);
        record.responseBytes += bytes(result);
        return result;
      } catch (error) {
        record.failures++;
        throw error;
      } finally {
        add(record.duration, performance.now() - started);
      }
    };
    const transform = internals.transformCallback.bind(internals);
    internals.transformCallback = (callback, once) => transform((value) => {
      if (value && typeof value.event === "string" && "payload" in value) {
        const method = value.payload?.method ?? value.payload?.eventType;
        const name = method ? `${value.event}:${method}` : value.event;
        const key = state.events.has(name) || state.events.size < 128 ? name : "other";
        const record = state.events.get(key) ?? { count: 0, bytes: 0 };
        record.count++;
        record.bytes += bytes(value.payload);
        state.events.set(key, record);
      }
      callback(value);
    }, once);
  }
  const longTasksSupported = PerformanceObserver.supportedEntryTypes?.includes("longtask") ?? false;
  if (longTasksSupported) {
    new PerformanceObserver((entries) => {
      for (const entry of entries.getEntries())
        if (entry.startTime >= state.started) add(state.longTasks, entry.duration);
    }).observe({ entryTypes: ["longtask"] });
  }
  let lastFrame: number | null = null;
  const frame = (time: number) => {
    if (lastFrame !== null) add(state.frames, time - lastFrame);
    lastFrame = time;
    requestAnimationFrame(frame);
  };
  requestAnimationFrame(frame);
  Object.assign(window, {
    __CAELUM_PROFILE__: {
      reset() { state = fresh(); lastFrame = null; },
      snapshot() {
        return {
          elapsedMs: performance.now() - state.started,
          navigationMs: performance.now(),
          renders: describe(state.renders),
          frameGapMs: describe(state.frames),
          longTasks: longTasksSupported ? describe(state.longTasks) : null,
          commands: Object.fromEntries([...state.commands].map(([key, value]) =>
            [key, { ...value, duration: describe(value.duration) }])),
          events: Object.fromEntries(state.events),
          domNodes: document.getElementsByTagName("*").length,
          transcriptEntries: document.querySelectorAll(".conversation-entry").length,
        };
      },
    },
  });
  return (_id, _phase, duration) => add(state.renders, duration);
}
