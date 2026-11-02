import { applyConversationEvent } from "./conversationState.js";

const PROVIDERS = ["codex", "opencode"];
export function sessionTitle(session) {
  return session.name || session.preview || "Untitled session";
}
export function sessionBranches(details) {
  return [
    ...new Set([
      ...(details?.branches ?? []),
      ...(details?.branch ? [details.branch] : []),
    ]),
  ];
}
export function sessionListError(reason) {
  return {
    kind:
      reason?.kind === "notInstalled" && typeof reason.message === "string"
        ? "notInstalled"
        : "failed",
    message:
      typeof reason?.message === "string" ? reason.message : String(reason),
  };
}
function emptyProviderWorkspace() {
  return {
    sessions: [],
    sessionDetails: {},
    archivingSessionIds: [],
    archivedSessionIds: [],
    sessionsLoaded: false,
    isLoadingSessions: false,
    sessionsError: null,
    selectedSessionId: null,
    sessionsById: {},
    draft: "",
    fileContext: [],
    isCreating: false,
    error: null,
    choices: null,
    choicesError: null,
    isLoadingChoices: false,
    model: null,
    effort: null,
    agent: null,
  };
}
export function createProjectWorkspace() {
  return {
    provider: "codex",
    providers: {
      codex: emptyProviderWorkspace(),
      opencode: emptyProviderWorkspace(),
    },
  };
}
export function createSessionWorkspace(session) {
  return {
    session,
    model: null,
    effort: null,
    agent: null,
    conversation: [],
    queuedMessages: [],
    queuePaused: false,
    inboxRevision: 0,
    settledInboxIds: [],
    draft: "",
    fileContext: [],
    isWorking: session.thread.isWorking ?? false,
    isSubmitting: false,
    connection: "connecting",
    error: null,
    historyError: null,
    historyLoaded: false,
    isLoadingHistory: false,
    nextCursor: null,
    historyAnchor: null,
    unread: false,
    expandedTools: {},
    scrollTop: null,
    windowEndId: null,
    eventRevision: 0,
    followRevision: 0,
    activity: null,
    lastOutcome: null,
    contextUsage: null,
    contextResetAt: null,
    contextStep: null,
  };
}
export function restoreProjectWorkspace(persisted) {
  const workspace = createProjectWorkspace();
  workspace.provider =
    persisted.provider === "opencode" ? "opencode" : "codex";
  for (const provider of PROVIDERS) {
    workspace.providers[provider].selectedSessionId =
      persisted[`${provider}SessionId`] ?? null;
    workspace.providers[provider].sessions = (
      persisted[`${provider}SessionIds`] ?? []
    ).map((id) => ({ id, name: null, preview: "", updatedAt: null }));
  }
  return workspace;
}
export function updateProjectWorkspace(workspaces, projectPath, update) {
  const current = workspaces[projectPath] ?? createProjectWorkspace();
  const next = update(current);
  return next === current
    ? workspaces
    : { ...workspaces, [projectPath]: next };
}
export function updateProviderWorkspace(
  workspaces,
  projectPath,
  provider,
  update,
) {
  return updateProjectWorkspace(workspaces, projectPath, (workspace) => {
    const current = workspace.providers[provider];
    const next = update(current);
    return next === current
      ? workspace
      : {
          ...workspace,
          providers: { ...workspace.providers, [provider]: next },
        };
  });
}
export function updateSessionWorkspace(
  workspaces,
  projectPath,
  provider,
  sessionId,
  update,
) {
  return updateProviderWorkspace(
    workspaces,
    projectPath,
    provider,
    (workspace) => {
      const current = workspace.sessionsById[sessionId];
      if (!current) return workspace;
      const next = update(current);
      return next === current
        ? workspace
        : {
            ...workspace,
            sessionsById: { ...workspace.sessionsById, [sessionId]: next },
          };
    },
  );
}
export function projectHasActiveTurn(workspace) {
  return PROVIDERS.some((provider) => {
    const state = workspace.providers[provider];
    return (
      Object.values(state.sessionsById).some(
        (session) => session.isWorking || session.isSubmitting,
      ) ||
      state.sessions.some(
        (session) =>
          state.sessionsById[session.id]?.isWorking ?? session.isWorking,
      )
    );
  });
}
function applyEvent(
  workspaces,
  projectPath,
  provider,
  method,
  data,
  disconnected,
  event,
) {
  return updateProviderWorkspace(
    workspaces,
    projectPath,
    provider,
    (workspace) => {
      if (disconnected)
        return {
          ...workspace,
          sessions: workspace.sessions.map((session) => ({
            ...session,
            isWorking: false,
          })),
          sessionsById: Object.fromEntries(
            Object.entries(workspace.sessionsById).map(([id, session]) => [
              id,
              {
                ...session,
                connection: "disconnected",
                isWorking: false,
                isSubmitting: false,
                activity: null,
                eventRevision: session.eventRevision + 1,
                conversation: session.conversation.map((entry) =>
                  entry.kind === "tool" && entry.status === "running"
                    ? { ...entry, status: "interrupted" }
                    : ["reasoning", "compaction"].includes(entry.kind) &&
                        !entry.complete
                      ? { ...entry, status: "interrupted" }
                      : entry,
                ),
              },
            ]),
          ),
        };
      const id =
        data.threadId ??
        data.thread?.id ??
        data.sessionID ??
        data.session?.id ??
        data.info?.id;
      if (!id) return workspace;
      const current = workspace.sessionsById[id];
      const summary = workspace.sessions.find((session) => session.id === id);
      // Unopened sessions still receive activity updates. Keep only their
      // summary until the user opens the conversation.
      const seed =
        current ??
        (summary
          ? createSessionWorkspace({ provider, thread: summary })
          : null);
      const applied = seed
        ? applyConversationEvent(seed, provider, method, data, event)
        : null;
      const next = current ? applied : null;
      const name =
        data.thread?.name ??
        data.session?.title ??
        data.info?.title ??
        (method === "thread/name/updated" ? data.threadName : null) ??
        ([
          "session.updated",
          "session.title.updated",
          "session.renamed",
        ].includes(method)
          ? data.title
          : null);
      const changed = applied && applied !== seed;
      const sessions = workspace.sessions
        .map((item) =>
          item.id === id
            ? {
                ...item,
                name: name ?? item.name,
                ...(applied ? { isWorking: applied.isWorking } : {}),
                updatedAt: changed ? Date.now() : item.updatedAt,
              }
            : item,
        )
        .sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0));
      return {
        ...workspace,
        sessions,
        sessionsById: next
          ? {
              ...workspace.sessionsById,
              [id]: {
                ...next,
                eventRevision:
                  (current.eventRevision ?? 0) + (changed ? 1 : 0),
                unread: changed ? true : next.unread,
              },
            }
          : workspace.sessionsById,
      };
    },
  );
}
export function applyCodexEvent(workspaces, event) {
  return applyEvent(
    workspaces,
    event.projectPath,
    "codex",
    event.method,
    event.params ?? {},
    event.method === "codex/processExited",
  );
}
export function applyOpenCodeEvent(workspaces, event) {
  return applyEvent(
    workspaces,
    event.projectPath,
    "opencode",
    event.eventType,
    event.data ?? {},
    event.eventType === "server.disconnected",
    event,
  );
}
