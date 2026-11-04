import { useEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { BackgroundShell } from "../../domain/conversation/shellActivity.js";
import type { applyOpenCodeEvent } from "../../domain/sessions/workspaceState.js";

type OpenCodeEvent = Parameters<typeof applyOpenCodeEvent>[1];
const empty = { shells: [] as BackgroundShell[], unavailable: false };

// Jobs outlive model turns and tool calls. Read the provider's active jobs,
// independently of history pagination and the session's working flag.
export function useBackgroundShells(
  projectPath: string | undefined,
  sessionId: string | null | undefined,
  enabled: boolean,
) {
  const scope = useMemo(
    () => ({ projectPath, sessionId, enabled }),
    [projectPath, sessionId, enabled],
  );
  const [snapshot, setSnapshot] = useState({ scope, ...empty });
  const receiver = useRef<(event: OpenCodeEvent) => void>(() => {});

  useEffect(() => {
    if (!enabled || !projectPath || !sessionId || !isTauri()) return;
    let active = true;
    let version = 0;
    let pending = false;
    let dirty = false;
    let current = empty;
    const publish = (value: typeof empty) => {
      current = value;
      setSnapshot({ scope, ...value });
    };
    const refresh = async () => {
      if (!active || document.hidden) return;
      if (pending) {
        dirty = true;
        return;
      }
      pending = true;
      dirty = false;
      const started = version;
      try {
        const shells = await invoke<BackgroundShell[]>(
          "opencode_background_shells",
          { projectPath, sessionId },
        );
        if (active && version === started)
          publish({ shells, unavailable: false });
      } catch {
        if (active && version === started)
          publish({ shells: [], unavailable: true });
      } finally {
        pending = false;
        if (dirty && active) void refresh();
      }
    };
    receiver.current = (event) => {
      if (!active || event.projectPath !== projectPath) return;
      const { eventType, data } = event;
      if (eventType === "server.disconnected") {
        active = false;
        publish(empty);
        return;
      }
      const shellEvent = [
        "shell.created",
        "shell.exited",
        "shell.deleted",
      ].includes(eventType);
      const sessionEvent =
        eventType.startsWith("session.shell.") ||
        eventType.startsWith("session.execution.") ||
        ["session.tool.success", "session.tool.failed"].includes(eventType);
      if (!shellEvent && (data.sessionID !== sessionId || !sessionEvent))
        return;
      // Discard snapshots taken before a lifecycle change; coalesce another read
      // when an event arrives during a request instead of losing the refresh.
      version++;
      if (
        ["shell.exited", "shell.deleted", "session.shell.ended"].includes(eventType)
      ) {
        const id =
          eventType === "session.shell.ended"
            ? (data.shell as { id?: string } | undefined)?.id
            : data.id;
        publish({
          ...current,
          shells: current.shells.filter((shell) => shell.id !== id),
        });
      }
      void refresh();
    };
    publish(empty);
    void refresh();
    const timer = window.setInterval(() => void refresh(), 5_000);
    const focus = () => void refresh();
    window.addEventListener("focus", focus);
    document.addEventListener("visibilitychange", focus);
    return () => {
      active = false;
      receiver.current = () => {};
      window.clearInterval(timer);
      window.removeEventListener("focus", focus);
      document.removeEventListener("visibilitychange", focus);
    };
  }, [scope]);

  const value = enabled && snapshot.scope === scope ? snapshot : empty;
  return {
    shells: value.shells,
    unavailable: value.unavailable,
    receive: (event: OpenCodeEvent) => receiver.current(event),
  };
}
