import { useEffect, useRef } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { BackgroundShell } from "../../domain/conversation/shellActivity.js";
import type { applyOpenCodeEvent } from "../../domain/sessions/workspaceState.js";
import { keyFor, type WorkspaceStore } from "./store.js";

type OpenCodeEvent = Parameters<typeof applyOpenCodeEvent>[1];

// Keep shell lifetimes separate from model turns, including after navigation.
export function useBackgroundShells(
  { state, mutateSession, revision, unavailable }: WorkspaceStore,
  projectPath: string | undefined,
  sessionId: string | null | undefined,
  enabled: boolean,
) {
  const pending = useRef(new Map<string, { dirty: boolean }>());
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  async function refresh(path: string, id: string) {
    const session = state.current[path]?.providers.opencode.sessionsById[id];
    if (
      !mounted.current || document.hidden ||
      unavailable(path, "opencode", id) || session?.connection !== "connected"
    ) return;
    const key = keyFor(path, "opencode", id);
    const existing = pending.current.get(key);
    if (existing) {
      existing.dirty = true;
      return;
    }
    const request = { dirty: false };
    pending.current.set(key, request);
    const lifetime = revision(key);
    const started = session.shellRevision;
    try {
      const shells = await invoke<BackgroundShell[]>(
        "opencode_background_shells", { projectPath: path, sessionId: id },
      );
      if (
        mounted.current && !unavailable(path, "opencode", id) &&
        revision(key) === lifetime
      )
        mutateSession(path, "opencode", id, (current) =>
          current.connection === "connected" && current.shellRevision === started
            ? { ...current, backgroundShells: shells }
            : current,
        );
    } catch {
      // Keep confirmed activity until a lifecycle event or a fresh read settles
      // it. Optional status refreshes should not become a permanent error banner.
    } finally {
      pending.current.delete(key);
      if (request.dirty) void refresh(path, id);
    }
  }

  useEffect(() => {
    if (!isTauri()) return;
    const refreshWatched = () => {
      if (document.hidden) return;
      for (const [path, workspace] of Object.entries(state.current)) {
        for (const [id, session] of Object.entries(
          workspace.providers.opencode.sessionsById,
        )) {
          if (
            session.backgroundShells.length ||
            (enabled && path === projectPath && id === sessionId)
          )
            void refresh(path, id);
        }
      }
    };
    refreshWatched();
    const timer = window.setInterval(refreshWatched, 5_000);
    window.addEventListener("focus", refreshWatched);
    document.addEventListener("visibilitychange", refreshWatched);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refreshWatched);
      document.removeEventListener("visibilitychange", refreshWatched);
    };
  }, [projectPath, sessionId, enabled]);

  return {
    receive: ({ projectPath: path, eventType, data }: OpenCodeEvent) => {
      const shellEvent = [
        "shell.created", "shell.exited", "shell.deleted",
      ].includes(eventType);
      const sessionEvent =
        eventType.startsWith("session.shell.") ||
        eventType.startsWith("session.execution.") ||
        ["session.tool.success", "session.tool.failed"].includes(eventType);
      if (!shellEvent && !sessionEvent) return;
      const ended = [
        "shell.exited", "shell.deleted", "session.shell.ended",
      ].includes(eventType);
      const shellId =
        eventType === "session.shell.ended"
          ? (data.shell as { id?: string } | undefined)?.id
          : data.id;
      const sessions = state.current[path]?.providers.opencode.sessionsById ?? {};
      for (const [id, session] of Object.entries(sessions)) {
        if (
          session.connection !== "connected" ||
          (!shellEvent && data.sessionID !== id)
        ) continue;
        // A terminal without sessionID must also invalidate in-flight snapshots
        // whose shell IDs have not arrived yet.
        mutateSession(path, "opencode", id, (current) => ({
          ...current,
          shellRevision: current.shellRevision + 1,
          backgroundShells: ended
            ? current.backgroundShells.filter((shell) => shell.id !== shellId)
            : current.backgroundShells,
        }));
        void refresh(path, id);
      }
    },
  };
}
