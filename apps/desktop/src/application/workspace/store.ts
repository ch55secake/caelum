import { useRef, useState } from "react";
import {
  updateProviderWorkspace,
  updateSessionWorkspace,
  type ProviderKind,
  type WorkspaceMap,
} from "../../domain/sessions/workspaceState.js";

export const keyFor = (path: string, provider: ProviderKind, id = "") =>
  `${path}\0${provider}\0${id}`;

// Shared scoped mutations and request lifetimes. Controllers receive this port,
// never a UI component or another controller's entire return value.
export function useWorkspaceStore() {
  const [workspaces, setWorkspaces] = useState<WorkspaceMap>({});
  const state = useRef(workspaces);
  const removedDirectories = useRef(new Set<string>());
  const checkoutVersions = useRef(new Map<string, number>());
  const revisions = useRef(new Map<string, number>());
  const locks = useRef(new Set<string>());
  function update(fn: (current: WorkspaceMap) => WorkspaceMap) {
    state.current = fn(state.current);
    setWorkspaces(state.current);
  }
  function mutateProvider(
    path: string,
    kind: ProviderKind,
    fn: Parameters<typeof updateProviderWorkspace>[3],
  ) {
    if (!removedDirectories.current.has(path))
      update((current) => updateProviderWorkspace(current, path, kind, fn));
  }
  function mutateSession(
    path: string,
    kind: ProviderKind,
    id: string,
    fn: Parameters<typeof updateSessionWorkspace>[4],
  ) {
    if (!removedDirectories.current.has(path))
      update((current) => updateSessionWorkspace(current, path, kind, id, fn));
  }
  const revision = (key: string) => revisions.current.get(key) ?? 0;
  function invalidateSession(path: string, kind: ProviderKind, id: string) {
    for (const key of [keyFor(path, kind), keyFor(path, kind, id)])
      revisions.current.set(key, revision(key) + 1);
  }
  function unavailable(path: string, kind: ProviderKind, id: string) {
    const current = state.current[path]?.providers[kind];
    return (
      removedDirectories.current.has(path) ||
      !current ||
      current.archivingSessionIds.includes(id) ||
      current.archivedSessionIds.includes(id)
    );
  }
  return {
    workspaces,
    state,
    removedDirectories,
    checkoutVersions,
    locks,
    update,
    mutateProvider,
    mutateSession,
    revision,
    invalidateSession,
    unavailable,
  };
}

export type WorkspaceStore = ReturnType<typeof useWorkspaceStore>;
