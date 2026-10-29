import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { addOpenProject, closeOpenProject } from "./projectWorkspace.js";
import { historyEntries, mergeHistory } from "./conversationState.js";
import {
  applyCodexEvent,
  applyOpenCodeEvent,
  createProjectWorkspace,
  createSessionWorkspace,
  restoreProjectWorkspace,
  updateProjectWorkspace,
  updateProviderWorkspace,
  updateSessionWorkspace,
  type ProjectInfo,
  type ProviderKind,
  type ProviderChoices,
  type ProviderWorkspaceState,
  type SessionWorkspaceState,
  type SessionSummary,
  type SessionDetails,
  type WorkspaceMap,
  type WorkspaceSessionInfo,
} from "./workspaceState.js";

export type ApprovalPrompt = {
  provider: string;
  projectPath: string;
  sessionId?: string | null;
  requestId: string;
  summary: string;
  description: string;
};
type PersistedProject = Parameters<typeof restoreProjectWorkspace>[0] & {
  project: ProjectInfo;
};
type ArchivedSession = {
  project: ProjectInfo;
  provider: ProviderKind;
  thread: SessionSummary;
  cached?: SessionWorkspaceState;
  details?: SessionDetails;
  restoreNavigation: number | null;
};
type HistoryPage = {
  data: Record<string, unknown>[];
  nextCursor: string | null;
  isWorking?: boolean | null;
  model?: string | null;
  effort?: string | null;
};
type OpenCodeInfo = {
  project: ProjectInfo;
  session: {
    id: string;
    title: string | null;
    updatedAt: number | null;
    isWorking?: boolean;
    model?: string | null;
    effort?: string | null;
  };
};
const providerInfo = (
  provider: ProviderKind,
  value: Omit<WorkspaceSessionInfo, "provider"> | OpenCodeInfo,
): WorkspaceSessionInfo =>
  "thread" in value
    ? { ...value, provider }
    : {
        project: value.project,
        provider,
        model: value.session.model,
        effort: value.session.effort,
        thread: {
          id: value.session.id,
          name: value.session.title,
          preview: "",
          updatedAt: value.session.updatedAt,
          isWorking: value.session.isWorking,
        },
      };
const keyFor = (path: string, provider: ProviderKind, id = "") =>
  `${path}\0${provider}\0${id}`;

function generationOptions(
  provider: ProviderKind,
  workspace?: ProviderWorkspaceState,
  session?: SessionWorkspaceState | null,
) {
  const model = session
    ? (session.model ?? session.session.model ?? "")
    : (workspace?.model ?? "");
  const modelChoice = workspace?.choices?.models.find(
    (choice) => choice.id === (model || workspace.choices?.defaultModel),
  );
  const changedModel = Boolean(
    session?.model && session.model !== session.session.model,
  );
  const pendingEffort = session ? session.effort : workspace?.effort || null;
  const effort =
    pendingEffort ?? (changedModel ? "" : (session?.session.effort ?? ""));
  return {
    model,
    modelChoice,
    effort,
    requestedEffort:
      pendingEffort !== null || changedModel
        ? effort ||
          (provider === "codex" ? (modelChoice?.defaultEffort ?? null) : "")
        : null,
  };
}

export function useWorkspace() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceMap>({});
  const state = useRef(workspaces);
  const [view, setView] = useState<"session" | "files">("session");
  const [error, setError] = useState<string | null>(null);
  const [restoring, setRestoring] = useState(isTauri());
  const [ready, setReady] = useState(!isTauri());
  const [selecting, setSelecting] = useState(false);
  const [approvals, setApprovals] = useState<ApprovalPrompt[]>([]);
  const [resolving, setResolving] = useState<string | null>(null);
  const [archivedSession, setArchivedSession] =
    useState<ArchivedSession | null>(null);
  const [undoingArchive, setUndoingArchive] = useState(false);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const locks = useRef(new Set<string>());
  const historyRefreshes = useRef(new Set<string>());
  const detailsRefreshes = useRef(new Set<string>());
  const detailsChecked = useRef(new Map<string, number>());
  // Archive/Undo invalidates snapshots already in flight, including snapshots
  // for the same session ID after it has been restored.
  const revisions = useRef(new Map<string, number>());
  const navigation = useRef(0);
  const workspace = project ? workspaces[project.path] : undefined;
  const provider = workspace?.provider ?? "codex";
  const providerWorkspace = workspace?.providers[provider];
  const selectedId = providerWorkspace?.selectedSessionId;
  const selected = selectedId
    ? providerWorkspace?.sessionsById[selectedId]
    : undefined;
  const options = generationOptions(provider, providerWorkspace, selected);

  function update(fn: (current: WorkspaceMap) => WorkspaceMap) {
    state.current = fn(state.current);
    setWorkspaces(state.current);
  }
  function mutateProvider(
    path: string,
    kind: ProviderKind,
    fn: Parameters<typeof updateProviderWorkspace>[3],
  ) {
    update((current) => updateProviderWorkspace(current, path, kind, fn));
  }
  function mutateSession(
    path: string,
    kind: ProviderKind,
    id: string,
    fn: Parameters<typeof updateSessionWorkspace>[4],
  ) {
    update((current) => updateSessionWorkspace(current, path, kind, id, fn));
  }
  function persist(command: string, args: Record<string, unknown>) {
    if (!isTauri()) return Promise.resolve();
    queue.current = queue.current
      .then(() => invoke(command, args))
      .catch((reason: unknown) =>
        setError(`Could not save workspace: ${String(reason)}`),
      );
    return queue.current;
  }
  const revision = (key: string) => revisions.current.get(key) ?? 0;
  function invalidateSession(path: string, kind: ProviderKind, id: string) {
    for (const key of [keyFor(path, kind), keyFor(path, kind, id)])
      revisions.current.set(key, revision(key) + 1);
  }
  function unavailable(path: string, kind: ProviderKind, id: string) {
    const current = state.current[path]?.providers[kind];
    return (
      !current ||
      current.archivingSessionIds.includes(id) ||
      current.archivedSessionIds.includes(id)
    );
  }

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const stops: (() => void)[] = [];
    let approvalRevision = 0;
    const subscribe = async <T>(
      name: string,
      callback: (payload: T) => void,
    ) => {
      const stop = await listen<T>(name, ({ payload }) => {
        if (!disposed) callback(payload);
      });
      if (disposed) stop();
      else stops.push(stop);
    };
    void Promise.all([
      subscribe<Parameters<typeof applyCodexEvent>[1]>(
        "codex-event",
        (event) => {
          update((current) => applyCodexEvent(current, event));
          if (
            event.method === "item/completed" &&
            (event.params.item as { type?: string } | undefined)?.type ===
              "contextCompaction" &&
            typeof event.params.threadId === "string"
          )
            void loadHistory(
              event.projectPath,
              "codex",
              event.params.threadId,
            );
          if (
            event.method === "turn/completed" &&
            typeof event.params.threadId === "string"
          )
            void loadSessionDetails(
              event.projectPath,
              "codex",
              event.params.threadId,
              true,
            );
        },
      ),
      subscribe<Parameters<typeof applyOpenCodeEvent>[1]>(
        "opencode-event",
        (event) => {
          update((current) => applyOpenCodeEvent(current, event));
          if (
            [
              "session.compaction.started",
              "session.compaction.ended",
              "session.compaction.failed",
            ].includes(event.eventType) &&
            typeof event.data.sessionID === "string"
          )
            void loadHistory(
              event.projectPath,
              "opencode",
              event.data.sessionID,
            );
          if (
            [
              "session.execution.succeeded",
              "session.execution.failed",
              "session.execution.interrupted",
            ].includes(event.eventType) &&
            typeof event.data.sessionID === "string"
          ) {
            void loadHistory(
              event.projectPath,
              "opencode",
              event.data.sessionID,
            );
            void loadSessionDetails(
              event.projectPath,
              "opencode",
              event.data.sessionID,
              true,
            );
          }
        },
      ),
      subscribe<ApprovalPrompt[]>("approval-requests", (payload) => {
        approvalRevision++;
        setApprovals(payload);
      }),
    ])
      .then(async () => {
        if (disposed) return;
        setReady(true);
        const revision = approvalRevision;
        const snapshot = await invoke<ApprovalPrompt[]>("pending_approvals");
        if (!disposed) {
          if (revision === approvalRevision) setApprovals(snapshot);
        }
      })
      .catch((reason: unknown) => {
        if (!disposed) setError(String(reason));
      });
    return () => {
      disposed = true;
      stops.forEach((stop) => stop());
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    void Promise.all([
      invoke<{
        projects: PersistedProject[];
        activeProjectPath: string | null;
      }>("restore_workspace").catch((reason: unknown) => {
        setError(String(reason));
        return { projects: [], activeProjectPath: null };
      }),
      invoke<ProjectInfo | null>("startup_project"),
    ])
      .then(async ([snapshot, startup]) => {
        const entries = [...snapshot.projects];
        if (
          startup &&
          !entries.some((entry) => entry.project.path === startup.path)
        )
          entries.unshift({
            project: startup,
            provider: "codex",
            codexSessionId: null,
            opencodeSessionId: null,
            codexSessionIds: [],
            opencodeSessionIds: [],
          });
        const available = await Promise.all(
          entries.map(async (entry) => {
            try {
              return {
                ...entry,
                project: await invoke<ProjectInfo>("open_project", {
                  path: entry.project.path,
                }),
              };
            } catch {
              return null;
            }
          }),
        );
        if (disposed) return;
        const restored = available.filter(
          (entry): entry is PersistedProject => entry !== null,
        );
        const nextProjects = restored.map((entry) => entry.project);
        update(() =>
          Object.fromEntries(
            restored.map((entry) => [
              entry.project.path,
              restoreProjectWorkspace(entry),
            ]),
          ),
        );
        setProjects(nextProjects);
        const active =
          nextProjects.find(
            (item) =>
              item.path === (startup?.path ?? snapshot.activeProjectPath),
          ) ??
          nextProjects[0] ??
          null;
        setProject(active);
        if (startup) void persist("remember_project", { project: startup });
        void persist("set_active_project", {
          projectPath: active?.path ?? null,
        });
        if (restored.length !== entries.length)
          setError("Some saved project folders are unavailable.");
      })
      .catch((reason: unknown) => {
        if (!disposed) setError(String(reason));
      })
      .finally(() => {
        if (!disposed) setRestoring(false);
      });
    return () => {
      disposed = true;
    };
  }, []);

  function activateProject(target: ProjectInfo | null) {
    navigation.current++;
    if (target)
      update((current) =>
        current[target.path]
          ? current
          : { ...current, [target.path]: createProjectWorkspace() },
      );
    setProject(target);
    setView("session");
    void persist("set_active_project", { projectPath: target?.path ?? null });
  }
  async function chooseProject() {
    if (!isTauri()) {
      setError("Open the desktop app to choose a local project.");
      return;
    }
    setSelecting(true);
    try {
      const path = await open({
        directory: true,
        multiple: false,
        title: "Open project",
      });
      if (typeof path !== "string") return;
      const opened = await invoke<ProjectInfo>("open_project", { path });
      void persist("remember_project", { project: opened });
      setProjects((current) => addOpenProject(current, opened));
      activateProject(opened);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setSelecting(false);
    }
  }
  function closeProject(path: string) {
    const next = closeOpenProject(projects, project?.path ?? null, path);
    setProjects(next.projects);
    void persist("close_project_record", {
      projectPath: path,
      activeProjectPath: next.activeProject?.path ?? null,
    });
    if (path === project?.path) activateProject(next.activeProject);
  }
  function changeProvider(kind: ProviderKind, fresh = true) {
    if (!project) return;
    navigation.current++;
    update((current) =>
      updateProjectWorkspace(current, project.path, (value) => ({
        ...value,
        provider: kind,
      })),
    );
    if (fresh) {
      mutateProvider(project.path, kind, (value) => ({
        ...value,
        selectedSessionId: null,
      }));
      void persist("select_project_session", {
        projectPath: project.path,
        provider: kind,
        sessionId: null,
      });
    }
    void persist("set_project_provider", {
      projectPath: project.path,
      provider: kind,
    });
  }
  function newSession() {
    if (!project) return;
    navigation.current++;
    setView("session");
    mutateProvider(project.path, provider, (value) => ({
      ...value,
      selectedSessionId: null,
      error: null,
    }));
    void persist("select_project_session", {
      projectPath: project.path,
      provider,
      sessionId: null,
    });
  }
  async function loadChoices(target: ProjectInfo, kind: ProviderKind) {
    const key = `choices:${keyFor(target.path, kind)}`;
    if (locks.current.has(key) || !isTauri()) return;
    locks.current.add(key);
    try {
      const choices = await invoke<ProviderChoices>("provider_choices", {
        projectPath: target.path,
        provider: kind,
      });
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        choices,
        choicesError: null,
      }));
    } catch (reason) {
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        choicesError: String(reason),
      }));
    } finally {
      locks.current.delete(key);
    }
  }
  function setOption(key: "model" | "agent" | "effort", value: string) {
    if (!project) return;
    if (key !== "agent" && selectedId)
      mutateSession(project.path, provider, selectedId, (current) => ({
        ...current,
        ...(key === "model"
          ? {
              model: value || null,
              effort:
                (value || current.session.model) ===
                (current.model ?? current.session.model)
                  ? current.effort
                  : null,
            }
          : { effort: value }),
      }));
    else
      mutateProvider(project.path, provider, (current) => ({
        ...current,
        [key]: value,
        ...(key === "model" && value !== current.model ? { effort: "" } : {}),
      }));
  }
  async function loadSessions(target: ProjectInfo, kind: ProviderKind) {
    const scope = keyFor(target.path, kind);
    const version = revision(scope);
    const key = `list:${keyFor(target.path, kind)}`;
    if (locks.current.has(key) || !isTauri()) return;
    locks.current.add(key);
    mutateProvider(target.path, kind, (value) => ({
      ...value,
      isLoadingSessions: true,
      error: null,
    }));
    try {
      const result = await invoke<{
        threads?: SessionSummary[];
        sessions?: OpenCodeInfo["session"][];
      }>(`${kind}_list_sessions`, { projectPath: target.path });
      if (revision(scope) !== version) return;
      const sessions = (
        result.threads ??
        result.sessions?.map((item) => ({
          id: item.id,
          name: item.title,
          preview: "",
          updatedAt: item.updatedAt,
          isWorking: item.isWorking,
        })) ??
        []
      )
        .filter(
          (item) =>
            !state.current[target.path].providers[
              kind
            ].archivedSessionIds.includes(item.id),
        )
        .map((item) => ({
          ...item,
          updatedAt:
            item.updatedAt && item.updatedAt < 1e12
              ? item.updatedAt * 1000
              : item.updatedAt,
        }));
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        sessions: [
          ...sessions.map((item) => {
            const local = value.sessions.find(
              (current) => current.id === item.id,
            );
            return local && (local.updatedAt ?? 0) > (item.updatedAt ?? 0)
              ? { ...item, ...local, name: item.name ?? local.name }
              : item;
          }),
          ...value.sessions.filter(
            (item) =>
              !sessions.some((fresh) => fresh.id === item.id) &&
              value.sessionsById[item.id],
          ),
        ].sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0)),
        sessionsLoaded: true,
      }));
      void persist("sync_provider_sessions", {
        projectPath: target.path,
        provider: kind,
        sessionIds: state.current[target.path].providers[kind].sessions.map(
          (item) => item.id,
        ),
      });
      for (const { id } of state.current[target.path].providers[kind]
        .sessions)
        if (
          detailsChecked.current.has(
            `details:${keyFor(target.path, kind, id)}`,
          )
        )
          void loadSessionDetails(target.path, kind, id, true);
    } catch (reason) {
      if (revision(scope) === version)
        mutateProvider(target.path, kind, (value) => ({
          ...value,
          error: String(reason),
        }));
    } finally {
      locks.current.delete(key);
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        isLoadingSessions: false,
      }));
      if (revision(scope) !== version) void loadSessions(target, kind);
    }
  }
  async function loadSessionDetails(
    path: string,
    kind: ProviderKind,
    id: string,
    force = false,
  ) {
    if (
      !isTauri() ||
      unavailable(path, kind, id) ||
      !state.current[path].providers[kind].sessions.some(
        (item) => item.id === id,
      )
    )
      return;
    const scope = keyFor(path, kind, id);
    const key = `details:${scope}`;
    if (locks.current.has(key)) {
      if (force) detailsRefreshes.current.add(key);
      return;
    }
    if (
      !force &&
      Date.now() - (detailsChecked.current.get(key) ?? 0) < 15_000
    )
      return;
    const version = revision(scope);
    locks.current.add(key);
    try {
      const details = await invoke<SessionDetails>("session_details", {
        projectPath: path,
        provider: kind,
        sessionId: id,
      });
      if (revision(scope) !== version || unavailable(path, kind, id)) return;
      mutateProvider(path, kind, (value) => ({
        ...value,
        sessionDetails: { ...value.sessionDetails, [id]: details },
      }));
    } catch {
      // Optional sidebar context must not block opening a session.
    } finally {
      locks.current.delete(key);
      if (revision(scope) === version)
        detailsChecked.current.set(key, Date.now());
      if (detailsRefreshes.current.delete(key) || revision(scope) !== version)
        void loadSessionDetails(path, kind, id, true);
    }
  }
  async function archiveSession(thread: SessionSummary, kind: ProviderKind) {
    if (!project || unavailable(project.path, kind, thread.id)) return;
    const target = project;
    const current = state.current[target.path].providers[kind];
    const cached = current.sessionsById[thread.id];
    if (
      (cached?.isWorking ?? thread.isWorking) ||
      cached?.isSubmitting ||
      cached?.connection === "connecting" ||
      approvals.some(
        (request) =>
          request.projectPath === target.path &&
          request.provider === kind &&
          request.sessionId === thread.id,
      )
    )
      return;
    const nav = navigation.current;
    mutateProvider(target.path, kind, (value) => ({
      ...value,
      archivingSessionIds: [...value.archivingSessionIds, thread.id],
    }));
    try {
      await invoke("set_session_archived", {
        projectPath: target.path,
        provider: kind,
        sessionId: thread.id,
        archived: true,
      });
      const latest = state.current[target.path].providers[kind];
      const wasSelected = latest.selectedSessionId === thread.id;
      const restoreNavigation =
        wasSelected && navigation.current === nav
          ? ++navigation.current
          : null;
      const notice: ArchivedSession = {
        project: target,
        provider: kind,
        thread:
          latest.sessions.find((item) => item.id === thread.id) ?? thread,
        cached: latest.sessionsById[thread.id],
        details: latest.sessionDetails[thread.id],
        restoreNavigation,
      };
      invalidateSession(target.path, kind, thread.id);
      mutateProvider(target.path, kind, (value) => {
        const sessionsById = { ...value.sessionsById };
        const sessionDetails = { ...value.sessionDetails };
        delete sessionsById[thread.id];
        delete sessionDetails[thread.id];
        return {
          ...value,
          sessionsById,
          sessionDetails,
          sessions: value.sessions.filter((item) => item.id !== thread.id),
          archivedSessionIds: [...value.archivedSessionIds, thread.id],
          selectedSessionId: wasSelected ? null : value.selectedSessionId,
        };
      });
      setArchivedSession(notice);
      void persist("forget_project_session", {
        projectPath: target.path,
        provider: kind,
        sessionId: thread.id,
      });
    } catch (reason) {
      setError(`Could not archive session: ${String(reason)}`);
    } finally {
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        archivingSessionIds: value.archivingSessionIds.filter(
          (id) => id !== thread.id,
        ),
      }));
    }
  }
  async function undoArchive() {
    if (!archivedSession || locks.current.has("undo-archive")) return;
    const notice = archivedSession;
    const { project: target, provider: kind, thread } = notice;
    locks.current.add("undo-archive");
    setUndoingArchive(true);
    try {
      await invoke("set_session_archived", {
        projectPath: target.path,
        provider: kind,
        sessionId: thread.id,
        archived: false,
      });
      invalidateSession(target.path, kind, thread.id);
      const restoreSelection =
        notice.restoreNavigation === navigation.current;
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        archivedSessionIds: value.archivedSessionIds.filter(
          (id) => id !== thread.id,
        ),
        sessions: [
          thread,
          ...value.sessions.filter((item) => item.id !== thread.id),
        ].sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0)),
        selectedSessionId: restoreSelection
          ? thread.id
          : value.selectedSessionId,
        sessionDetails: notice.details
          ? { ...value.sessionDetails, [thread.id]: notice.details }
          : value.sessionDetails,
        sessionsById: {
          ...value.sessionsById,
          [thread.id]: {
            ...(notice.cached ??
              createSessionWorkspace({
                project: target,
                provider: kind,
                thread,
              })),
            connection: "disconnected",
            isLoadingHistory: false,
            isWorking: false,
            isSubmitting: false,
            activity: null,
            error: null,
          },
        },
      }));
      void persist("sync_provider_sessions", {
        projectPath: target.path,
        provider: kind,
        sessionIds: state.current[target.path].providers[kind].sessions.map(
          (item) => item.id,
        ),
      });
      if (restoreSelection) {
        void persist("select_project_session", {
          projectPath: target.path,
          provider: kind,
          sessionId: thread.id,
        });
        void connectSession(target, kind, thread);
      }
      void loadSessionDetails(target.path, kind, thread.id, true);
      setArchivedSession((value) => (value === notice ? null : value));
    } catch (reason) {
      setError(`Could not restore session: ${String(reason)}`);
    } finally {
      locks.current.delete("undo-archive");
      setUndoingArchive(false);
    }
  }
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
      if (!older) historyRefreshes.current.add(key);
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
      mutateSession(path, kind, id, (value) => {
        const overlap = entries.some((entry) =>
          value.conversation.some((live) => live.id === entry.id),
        );
        const advance =
          older ||
          !value.historyLoaded ||
          (!overlap && Boolean(page.nextCursor));
        // A snapshot can race a newer event or an optimistic prompt. Only
        // reconcile activity when nothing changed since the request began.
        const fresh =
          !older &&
          !current.isSubmitting &&
          !value.isSubmitting &&
          current.eventRevision === value.eventRevision &&
          current.followRevision === value.followRevision;
        const isWorking = fresh
          ? (page.isWorking ?? value.isWorking)
          : value.isWorking;
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
        return {
          ...value,
          session: fresh
            ? {
                ...value.session,
                ...(page.model ? { model: page.model } : {}),
                ...(page.effort != null ? { effort: page.effort } : {}),
              }
            : value.session,
          isWorking,
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
      if (historyRefreshes.current.delete(key))
        void loadHistory(path, kind, id);
    }
  }
  async function connectSession(
    target: ProjectInfo,
    kind: ProviderKind,
    thread: SessionSummary,
  ) {
    if (unavailable(target.path, kind, thread.id)) return;
    const scope = keyFor(target.path, kind, thread.id);
    const version = revision(scope);
    const key = `connect:${keyFor(target.path, kind, thread.id)}`;
    if (locks.current.has(key)) return;
    const cached =
      state.current[target.path]?.providers[kind].sessionsById[thread.id];
    if (cached?.connection === "connected") {
      await loadHistory(target.path, kind, thread.id);
      return;
    }
    locks.current.add(key);
    mutateProvider(target.path, kind, (value) => ({
      ...value,
      sessionsById: {
        ...value.sessionsById,
        [thread.id]: {
          ...(value.sessionsById[thread.id] ??
            createSessionWorkspace({
              project: target,
              provider: kind,
              thread,
            })),
          connection: "connecting",
          error: null,
        },
      },
    }));
    try {
      const result = await invoke<
        Omit<WorkspaceSessionInfo, "provider"> | OpenCodeInfo
      >(`${kind}_resume_session`, {
        projectPath: target.path,
        threadId: thread.id,
        sessionId: thread.id,
      });
      if (revision(scope) !== version) return;
      const info = providerInfo(kind, result);
      mutateSession(target.path, kind, thread.id, (value) => ({
        ...value,
        session: info,
        connection: "connected",
        isWorking:
          value.eventRevision === (cached?.eventRevision ?? 0)
            ? (info.thread.isWorking ?? value.isWorking)
            : value.isWorking,
      }));
      void loadHistory(target.path, kind, thread.id);
    } catch (reason) {
      if (revision(scope) === version)
        mutateSession(target.path, kind, thread.id, (value) => ({
          ...value,
          connection: "disconnected",
          error: String(reason),
        }));
    } finally {
      locks.current.delete(key);
    }
  }
  function selectSession(thread: SessionSummary, kind = provider) {
    if (!project || unavailable(project.path, kind, thread.id)) return;
    navigation.current++;
    setView("session");
    if (kind !== provider) changeProvider(kind, false);
    mutateProvider(project.path, kind, (value) => ({
      ...value,
      selectedSessionId: thread.id,
    }));
    void persist("select_project_session", {
      projectPath: project.path,
      provider: kind,
      sessionId: thread.id,
    });
    void connectSession(project, kind, thread);
  }
  useEffect(() => {
    if (!project || !providerWorkspace || !ready || restoring || !isTauri())
      return;
    if (!providerWorkspace.sessionsLoaded && !providerWorkspace.error)
      void loadSessions(project, provider);
    if (!providerWorkspace.choices && !providerWorkspace.choicesError)
      void loadChoices(project, provider);
    if (selectedId && !selected) {
      const thread = providerWorkspace.sessions.find(
        (item) => item.id === selectedId,
      );
      if (thread) void connectSession(project, provider, thread);
    }
  }, [
    project?.path,
    provider,
    providerWorkspace?.sessionsLoaded,
    providerWorkspace?.error,
    selectedId,
    ready,
    restoring,
  ]);
  useEffect(() => {
    if (project && selectedId && selected?.unread && view === "session")
      mutateSession(project.path, provider, selectedId, (value) => ({
        ...value,
        unread: false,
      }));
  }, [project?.path, provider, selectedId, selected?.unread, view]);

  useEffect(() => {
    if (!project || !selectedId || !ready || !isTauri()) return;
    const path = project.path,
      kind = provider,
      id = selectedId;
    let refreshed = Date.now();
    const refresh = () => {
      const session = state.current[path]?.providers[kind].sessionsById[id];
      if (
        document.hidden ||
        session?.connection !== "connected" ||
        session.isSubmitting ||
        session.isLoadingHistory
      )
        return;
      refreshed = Date.now();
      void loadHistory(path, kind, id);
    };
    refresh();
    const timer = window.setInterval(() => {
      const session = state.current[path]?.providers[kind].sessionsById[id];
      if (session?.isWorking || Date.now() - refreshed >= 15_000) refresh();
    }, 5_000);
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [project?.path, provider, selectedId, ready]);

  function setDraft(draft: string) {
    if (!project) return;
    if (selectedId)
      mutateSession(project.path, provider, selectedId, (value) => ({
        ...value,
        draft,
      }));
    else
      mutateProvider(project.path, provider, (value) => ({
        ...value,
        draft,
      }));
  }
  async function submit() {
    if (!project) return;
    const target = project,
      kind = provider;
    const snapshot = state.current[target.path].providers[kind];
    let id = snapshot.selectedSessionId;
    const cached = id ? snapshot.sessionsById[id] : null;
    const model = cached?.model ?? null;
    const { requestedEffort: effort } = generationOptions(
      kind,
      snapshot,
      cached,
    );
    const text = (cached?.draft ?? snapshot.draft).trim();
    const key = `send:${keyFor(target.path, kind, id ?? "new")}`;
    if (
      !text ||
      (id && unavailable(target.path, kind, id)) ||
      locks.current.has(key) ||
      cached?.isWorking ||
      cached?.isSubmitting ||
      (id && (!cached || cached.connection !== "connected"))
    )
      return;
    locks.current.add(key);
    const nav = navigation.current;
    const messageId = `msg_${crypto.randomUUID().replaceAll("-", "")}`;
    const optimisticId =
      kind === "opencode" ? `opencode-${messageId}` : `local-${messageId}`;
    try {
      if (!id) {
        mutateProvider(target.path, kind, (value) => ({
          ...value,
          isCreating: true,
          error: null,
        }));
        const result = await invoke<
          Omit<WorkspaceSessionInfo, "provider"> | OpenCodeInfo
        >(`${kind}_start_session`, {
          projectPath: target.path,
          model: snapshot.model || null,
          agent: snapshot.agent || null,
        });
        const started = providerInfo(kind, result);
        started.model ??= snapshot.model || null;
        id = started.thread.id;
        mutateProvider(target.path, kind, (value) => ({
          ...value,
          draft: value.draft.trim() === text ? "" : value.draft,
          selectedSessionId:
            navigation.current === nav
              ? started.thread.id
              : value.selectedSessionId,
          sessions: [
            { ...started.thread, preview: text, updatedAt: Date.now() },
            ...value.sessions.filter((item) => item.id !== started.thread.id),
          ],
          sessionsById: {
            ...value.sessionsById,
            [started.thread.id]: {
              ...createSessionWorkspace(started),
              effort,
              connection: "connected",
              historyLoaded: true,
              draft: text,
            },
          },
        }));
        if (navigation.current === nav)
          void persist("select_project_session", {
            projectPath: target.path,
            provider: kind,
            sessionId: id,
          });
      }
      const sessionId = id;
      mutateSession(target.path, kind, sessionId, (value) => ({
        ...value,
        draft: "",
        error: null,
        isSubmitting: true,
        isWorking: true,
        activity: { label: "Working…" },
        lastOutcome: null,
        scrollTop: null,
        windowEndId: null,
        followRevision: value.followRevision + 1,
        conversation: [
          ...value.conversation,
          { id: optimisticId, kind: "user", text, optimistic: true },
        ],
      }));
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        sessions: value.sessions
          .map((item) =>
            item.id === sessionId
              ? {
                  ...item,
                  preview: item.preview || text,
                  updatedAt: Date.now(),
                }
              : item,
          )
          .sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0)),
      }));
      await invoke(`${kind}_send_prompt`, {
        projectPath: target.path,
        threadId: sessionId,
        sessionId,
        prompt: text,
        messageId,
        model,
        effort,
      });
      if (model || effort !== null)
        mutateSession(target.path, kind, sessionId, (value) => ({
          ...value,
          session: {
            ...value.session,
            ...(model ? { model } : {}),
            ...(effort !== null ? { effort } : {}),
          },
          model: value.model === model ? null : value.model,
          effort:
            value.effort === (cached?.effort ?? effort) ? null : value.effort,
        }));
    } catch (reason) {
      if (id)
        mutateSession(target.path, kind, id, (value) => ({
          ...value,
          draft: value.draft ? `${text}\n\n${value.draft}` : text,
          isWorking: false,
          activity: null,
          error: String(reason),
          conversation: value.conversation.filter(
            (entry) => entry.id !== optimisticId,
          ),
        }));
      else
        mutateProvider(target.path, kind, (value) => ({
          ...value,
          error: String(reason),
        }));
    } finally {
      locks.current.delete(key);
      if (id)
        mutateSession(target.path, kind, id, (value) => ({
          ...value,
          isSubmitting: false,
        }));
      mutateProvider(target.path, kind, (value) => ({
        ...value,
        isCreating: false,
      }));
    }
  }
  async function cancel() {
    if (!project || !selectedId) return;
    const target = project,
      kind = provider,
      id = selectedId;
    try {
      await invoke(`${kind}_cancel_turn`, {
        projectPath: target.path,
        threadId: id,
        sessionId: id,
      });
    } catch (reason) {
      mutateSession(target.path, kind, id, (value) => ({
        ...value,
        error: String(reason),
      }));
    }
  }
  async function resolveApproval(
    request: ApprovalPrompt,
    decision: "allowOnce" | "deny",
  ) {
    if (resolving) return;
    setResolving(request.requestId);
    try {
      await invoke("resolve_approval", {
        provider: request.provider,
        projectPath: request.projectPath,
        requestId: request.requestId,
        decision,
      });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setResolving(null);
    }
  }
  function viewApproval(request: ApprovalPrompt) {
    if (request.provider !== "codex" && request.provider !== "opencode")
      return;
    const target = projects.find(
      (item) => item.path === request.projectPath,
    ) ?? {
      path: request.projectPath,
      name:
        request.projectPath.split(/[\\/]/).filter(Boolean).at(-1) ??
        request.projectPath,
    };
    if (!projects.some((item) => item.path === target.path)) {
      setProjects((current) => addOpenProject(current, target));
      void persist("remember_project", { project: target });
    }
    activateProject(target);
    const kind = request.provider;
    update((current) =>
      updateProjectWorkspace(current, target.path, (value) => ({
        ...value,
        provider: kind,
      })),
    );
    void persist("set_project_provider", {
      projectPath: target.path,
      provider: kind,
    });
    if (!request.sessionId) return;
    const id = request.sessionId;
    if (unavailable(target.path, kind, id)) return;
    const thread = state.current[target.path].providers[kind].sessions.find(
      (item) => item.id === id,
    ) ?? { id, name: null, preview: "Approval required", updatedAt: null };
    mutateProvider(target.path, kind, (value) => ({
      ...value,
      selectedSessionId: id,
      sessions: value.sessions.some((item) => item.id === id)
        ? value.sessions
        : [thread, ...value.sessions],
    }));
    void persist("select_project_session", {
      projectPath: target.path,
      provider: kind,
      sessionId: id,
    });
    void connectSession(target, kind, thread);
  }
  return {
    projects,
    project,
    workspaces,
    workspace,
    provider,
    providerWorkspace,
    options,
    selected,
    selectedId,
    view,
    setView,
    error,
    setError,
    restoring,
    selecting,
    approvals,
    resolving,
    archivedSession,
    undoingArchive,
    dismissArchive: () => setArchivedSession(null),
    archiveSession,
    undoArchive,
    activateProject,
    chooseProject,
    closeProject,
    changeProvider,
    newSession,
    selectSession,
    loadSessions,
    loadSessionDetails,
    loadHistory,
    connectSession,
    setDraft,
    submit,
    cancel,
    resolveApproval,
    mutateSession,
    setOption,
    loadChoices,
    viewApproval,
  };
}
