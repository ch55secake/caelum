import { useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  historyEntries,
  mergeHistory,
} from "../../domain/conversation/conversationState.js";
import {
  contextUsageSince,
  openCodeHistoryUsage,
} from "../../domain/usage/contextUsage.js";
import type { ProviderKind } from "../../domain/sessions/workspaceState.js";
import { keyFor, type WorkspaceStore } from "./store.js";

type HistoryPage = {
  data: Record<string, unknown>[];
  nextCursor: string | null;
  isWorking?: boolean | null;
  model?: string | null;
  effort?: string | null;
  agent?: string | null;
};

export function useSessionHistory(
  { state, locks, unavailable, revision, mutateSession }: WorkspaceStore,
  loadSessionDetails: (
    path: string,
    kind: ProviderKind,
    id: string,
  ) => Promise<void>,
  reconcileActivity: (key: string, working: boolean) => void,
) {
  const refreshes = useRef(new Set<string>());
  async function loadHistory(
    path: string,
    kind: ProviderKind,
    id: string,
    older = false,
  ) {
    if (unavailable(path, kind, id)) return;
    const scope = keyFor(path, kind, id);
    const version = revision(scope);
    const key = `history:${scope}`;
    const current = state.current[path]?.providers[kind].sessionsById[id];
    if (!current) return;
    if (locks.current.has(key)) {
      if (!older) refreshes.current.add(key);
      return;
    }
    locks.current.add(key);
    mutateSession(path, kind, id, (value) => ({
      ...value,
      isLoadingHistory: true,
      historyError: null,
    }));
    try {
      const page = await invoke<HistoryPage>(`${kind}_session_history`, {
        projectPath: path,
        threadId: id,
        sessionId: id,
        cursor: older ? current.nextCursor : null,
      });
      if (revision(scope) !== version) return;
      const entries = historyEntries(kind, page.data);
      if (older && page.nextCursor && page.nextCursor === current.nextCursor)
        throw new Error(
          "History pagination did not advance. Retry loading earlier messages.",
        );
      mutateSession(path, kind, id, (value) => {
        const overlap = entries.some((entry) =>
          value.conversation.some((live) => live.id === entry.id),
        );
        const advance =
          older ||
          !value.historyLoaded ||
          (!overlap && Boolean(page.nextCursor));
        // Only reconcile activity when no event or optimistic prompt raced the snapshot.
        const fresh =
          !older &&
          !current.isSubmitting &&
          !value.isSubmitting &&
          current.eventRevision === value.eventRevision &&
          current.followRevision === value.followRevision;
        const isWorking = fresh
          ? (page.isWorking ?? value.isWorking)
          : value.isWorking;
        if (fresh && page.isWorking != null)
          reconcileActivity(scope, isWorking);
        const conversation = mergeHistory(
          value.conversation,
          entries,
          older || !value.historyLoaded ? "prepend" : "refresh",
          older ? value.historyAnchor : null,
        );
        const compacting = conversation.some(
          (entry) =>
            entry.kind === "compaction" &&
            entry.status === "running" &&
            !entry.complete,
        );
        const contextUsage =
          fresh && kind === "opencode"
            ? openCodeHistoryUsage(
                page.data,
                Math.max(
                  value.contextResetAt ?? 0,
                  contextUsageSince(value.contextUsage, conversation),
                ),
              )
            : undefined;
        return {
          ...value,
          contextUsage:
            contextUsage === undefined ? value.contextUsage : contextUsage,
          session: fresh
            ? {
                ...value.session,
                ...(page.model ? { model: page.model } : {}),
                ...(page.effort != null ? { effort: page.effort } : {}),
                ...(page.agent ? { agent: page.agent } : {}),
              }
            : value.session,
          isWorking,
          lastOutcome: isWorking ? null : value.lastOutcome,
          activity: !isWorking
            ? null
            : fresh && compacting
              ? { label: "Compacting context…" }
              : fresh && value.activity?.label === "Compacting context…"
                ? { label: "Working…" }
                : value.activity,
          conversation,
          historyLoaded: true,
          nextCursor: advance ? page.nextCursor : value.nextCursor,
          historyAnchor: advance
            ? (entries[0]?.id ?? value.historyAnchor)
            : value.historyAnchor,
        };
      });
      if (!older) void loadSessionDetails(path, kind, id);
    } catch (reason) {
      if (revision(scope) === version)
        mutateSession(path, kind, id, (value) => ({
          ...value,
          historyError: String(reason),
        }));
    } finally {
      locks.current.delete(key);
      if (revision(scope) === version)
        mutateSession(path, kind, id, (value) => ({
          ...value,
          isLoadingHistory: false,
        }));
      if (refreshes.current.delete(key)) void loadHistory(path, kind, id);
    }
  }
  return loadHistory;
}
