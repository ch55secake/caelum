import { useRef } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import {
  sessionBranches,
  type ProviderKind,
  type SessionDetails,
} from "../../domain/sessions/workspaceState.js";
import { refreshBranchPullRequests } from "../github/useBranchPullRequests.js";
import { keyFor, type WorkspaceStore } from "./store.js";

export function useSessionMetadata({
  state,
  unavailable,
  revision,
  mutateProvider,
}: WorkspaceStore) {
  const refreshes = useRef(new Set<string>());
  const loads = useRef(new Map<string, Promise<void>>());
  const checked = useRef(new Map<string, number>());
  const metadataKey = (path: string, kind: ProviderKind, id: string) =>
    `details:${keyFor(path, kind, id)}`;
  const hasSessionDetails = (path: string, kind: ProviderKind, id: string) => {
    const key = metadataKey(path, kind, id);
    return checked.current.has(key) || loads.current.has(key);
  };
  async function loadSessionDetails(
    path: string,
    kind: ProviderKind,
    id: string,
    force = false,
  ): Promise<void> {
    if (
      !isTauri() ||
      unavailable(path, kind, id) ||
      !state.current[path].providers[kind].sessions.some(
        (item) => item.id === id,
      )
    )
      return;
    const scope = keyFor(path, kind, id);
    const key = metadataKey(path, kind, id);
    const pending = loads.current.get(key);
    if (pending) {
      if (force) refreshes.current.add(key);
      return pending;
    }
    if (!force && Date.now() - (checked.current.get(key) ?? 0) < 15_000) return;
    const version = revision(scope);
    const request = (async () => {
      try {
        const details = await invoke<SessionDetails>("session_details", {
          projectPath: path,
          provider: kind,
          sessionId: id,
        });
        if (
          revision(scope) !== version ||
          unavailable(path, kind, id) ||
          refreshes.current.has(key)
        )
          return;
        mutateProvider(path, kind, (value) => ({
          ...value,
          sessionDetails: { ...value.sessionDetails, [id]: details },
        }));
      } catch {
        // Optional sidebar context must not block opening a session.
      } finally {
        loads.current.delete(key);
        if (revision(scope) === version) checked.current.set(key, Date.now());
        // Execution-triggered discovery waits for the queued fresh metadata.
        if (refreshes.current.delete(key) || revision(scope) !== version)
          await loadSessionDetails(path, kind, id, true);
      }
    })();
    loads.current.set(key, request);
    return request;
  }
  async function refreshSessionPullRequest(
    path: string,
    kind: ProviderKind,
    id: string,
  ) {
    await loadSessionDetails(path, kind, id, true);
    refreshBranchPullRequests(
      path,
      sessionBranches(state.current[path]?.providers[kind].sessionDetails[id]),
    );
  }
  function receiveBranches({
    projectPath: path,
    provider: kind,
    sessionId: id,
    branch,
    branches,
  }: {
    projectPath: string;
    provider: ProviderKind;
    sessionId: string;
    branch: string | null;
    branches?: string[];
  }) {
    if (unavailable(path, kind, id)) return;
    const current = state.current[path].providers[kind];
    if (!current.sessions.some((session) => session.id === id)) return;
    const history =
      branches ??
      sessionBranches({
        branch,
        branches: sessionBranches(current.sessionDetails[id]),
      });
    const key = metadataKey(path, kind, id);
    // Older provider reads must not restore a branch observed before live Git.
    if (loads.current.has(key)) refreshes.current.add(key);
    mutateProvider(path, kind, (value) => ({
      ...value,
      sessionDetails: {
        ...value.sessionDetails,
        [id]: {
          changes: value.sessionDetails[id]?.changes ?? null,
          branch,
          branches: history,
          branchIsCurrent: false,
        },
      },
    }));
    refreshBranchPullRequests(path, history);
  }
  return {
    loadSessionDetails,
    hasSessionDetails,
    refreshSessionPullRequest,
    receiveBranches,
  };
}
