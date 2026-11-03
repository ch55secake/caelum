import { useEffect, useRef, useState } from "react";
import {
  activityEvent,
  activityKey,
  applyActivity,
  emptyActivity,
  readMarkers,
  savedMarkers,
  READ_MARKERS_KEY,
  type PromptRun,
  type SessionActivity,
} from "../../domain/sessions/activityState.js";
import type {
  ProviderKind,
  SessionSummary,
} from "../../domain/sessions/workspaceState.js";

export type CompletionEvent = {
  key: string;
  run: PromptRun;
  background: boolean;
};

export function useSessionActivity() {
  const [initial] = useState(() => {
    try {
      return readMarkers(
        JSON.parse(localStorage.getItem(READ_MARKERS_KEY) ?? "null"),
      );
    } catch {
      return new Map<string, SessionActivity>();
    }
  });
  const records = useRef(initial);
  const [, render] = useState(0);
  const [storageError, setStorageError] = useState<string | null>(null);
  const viewport = useRef<string | null>(null);
  const foreground = useRef(document.hasFocus() && !document.hidden);
  const timer = useRef<number | null>(null);
  const dirty = useRef(false);
  const completions = useRef(new Set<(event: CompletionEvent) => void>());
  const listeningSince = useRef(Date.now());
  function flush() {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
    if (!dirty.current) return;
    try {
      localStorage.setItem(
        READ_MARKERS_KEY,
        JSON.stringify(savedMarkers(records.current)),
      );
      dirty.current = false;
      setStorageError(null);
    } catch {
      setStorageError(
        "Could not save session read markers. Unread indicators will reset after restart.",
      );
    }
  }
  function write(key: string, next: SessionActivity) {
    const previous = records.current.get(key);
    if (next === previous) return;
    records.current.set(key, next);
    if (previous?.unread !== next.unread || previous?.run !== next.run)
      render((value) => value + 1);
    dirty.current = true;
    if (timer.current === null) timer.current = window.setTimeout(flush, 750);
  }
  function markViewed(key: string) {
    const record = records.current.get(key);
    if (!record?.unread) return;
    write(key, {
      ...record,
      unread: false,
      readAt: Math.max(Date.now(), record.updatedAt),
    });
  }
  function isViewed(key: string) {
    return viewport.current === key && foreground.current && !document.hidden;
  }
  useEffect(() => {
    const focus = () => {
      foreground.current = !document.hidden;
      if (viewport.current && foreground.current) markViewed(viewport.current);
    };
    const blur = () => {
      foreground.current = false;
      flush();
    };
    const visibility = () => {
      if (document.hidden) blur();
      else if (document.hasFocus()) focus();
    };
    window.addEventListener("focus", focus);
    window.addEventListener("blur", blur);
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("pagehide", flush);
    return () => {
      window.removeEventListener("focus", focus);
      window.removeEventListener("blur", blur);
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("pagehide", flush);
      flush();
    };
  }, []);

  function receive(
    path: string,
    provider: ProviderKind,
    method: string,
    data: Record<string, unknown>,
    envelope?: { id?: string; created?: number },
  ) {
    if (method === "codex/processExited" || method === "server.disconnected") {
      for (const [key, record] of records.current) {
        if (
          key.startsWith(activityKey(path, provider)) &&
          record.run?.finishedAt === null
        )
          write(key, {
            ...record,
            run: {
              ...record.run,
              finishedAt: Date.now(),
              outcome: "disconnected",
            },
          });
      }
      return;
    }
    const event = activityEvent(provider, method, data, envelope);
    if (!event) return;
    const key = activityKey(path, provider, event.sessionId);
    const viewed = isViewed(key);
    const result = applyActivity(
      records.current.get(key),
      event,
      viewed,
      Date.now(),
    );
    write(key, result.record);
    if (
      result.completion &&
      (event.at == null || event.at >= listeningSince.current)
    )
      for (const listener of completions.current)
        listener({ key, run: result.completion, background: !viewed });
  }
  function observeSessions(
    path: string,
    provider: ProviderKind,
    sessions: SessionSummary[],
  ) {
    for (const session of sessions) {
      const key = activityKey(path, provider, session.id);
      const previous = records.current.get(key);
      const updatedAt = session.updatedAt ?? 0;
      const viewed = isViewed(key);
      if (previous && updatedAt <= previous.updatedAt) continue;
      write(key, {
        ...(previous ?? emptyActivity()),
        updatedAt,
        unread: Boolean(
          previous &&
            (previous.unread || updatedAt > previous.readAt) &&
            !viewed,
        ),
        readAt: viewed
          ? Math.max(updatedAt, Date.now())
          : (previous?.readAt ?? updatedAt),
      });
    }
  }
  return {
    begin: (
      key: string,
      submissionId: string,
      startedAt = Date.now(),
      queuedMs?: number,
    ) => {
      const record = records.current.get(key) ?? emptyActivity();
      if (record.run?.finishedAt === null) return;
      write(key, {
        ...record,
        run: {
          id: submissionId,
          submissionId,
          startedAt,
          queuedMs,
          awaitingStart: true,
          finishedAt: null,
          outcome: null,
        },
      });
    },
    fail: (key: string, submissionId: string) => {
      const record = records.current.get(key);
      if (
        record?.run?.submissionId === submissionId &&
        record.run.finishedAt === null
      )
        write(key, {
          ...record,
          run: { ...record.run, finishedAt: Date.now(), outcome: "failed" },
        });
    },
    reconcile: (key: string, working: boolean) => {
      const record = records.current.get(key);
      // A fresh idle snapshot recovers a missed terminal quietly. Its arrival
      // time is not the execution's completion time, so do not report a duration.
      if (!working && record?.run?.finishedAt === null)
        write(key, {
          ...record,
          run: { ...record.run, finishedAt: Date.now(), outcome: "reconciled" },
        });
    },
    delivered: (key: string, id: string, queuedAt?: number) => {
      const record = records.current.get(key) ?? emptyActivity();
      const now = Date.now();
      const run =
        record.run?.finishedAt === null
          ? record.run
          : {
              id,
              submissionId: id,
              startedAt: now,
              finishedAt: null,
              outcome: null,
              awaitingStart: true,
            };
      write(key, {
        ...record,
        run: {
          ...run,
          queuedMs: queuedAt == null ? undefined : Math.max(0, now - queuedAt),
        },
      });
    },
    receive,
    observeSessions,
    get: (key: string) => records.current.get(key),
    setViewport: (key: string, visible: boolean) => {
      if (visible) {
        viewport.current = key;
        if (isViewed(key)) markViewed(key);
      } else if (viewport.current === key) viewport.current = null;
    },
    subscribe: (listener: (event: CompletionEvent) => void) => {
      completions.current.add(listener);
      return () => {
        completions.current.delete(listener);
      };
    },
    storageError,
    retrySave: flush,
  };
}
