export type ProviderKind = "codex" | "opencode";
export type SessionSummary = { id: string; name: string | null; preview: string; updatedAt: number | null };
export type ConversationEntry = { id: string; kind: "user" | "assistant" | "activity" | "status"; text: string };
export type ProjectInfo = { name: string; path: string };
export type WorkspaceSessionInfo = { project: ProjectInfo; thread: SessionSummary; provider: ProviderKind };

export type SessionWorkspaceState = {
  session: WorkspaceSessionInfo;
  conversation: ConversationEntry[];
  isWorking: boolean;
  isSubmitting: boolean;
  error: string | null;
};

export type ProviderWorkspaceState = {
  sessions: SessionSummary[];
  sessionsLoaded: boolean;
  isLoadingSessions: boolean;
  selectedSessionId: string | null;
  sessionsById: Record<string, SessionWorkspaceState>;
  error: string | null;
};

export type ProjectWorkspaceState = {
  provider: ProviderKind;
  prompt: string;
  providers: Record<ProviderKind, ProviderWorkspaceState>;
};

export type WorkspaceMap = Record<string, ProjectWorkspaceState>;

export function createProjectWorkspace(): ProjectWorkspaceState;
export function updateProjectWorkspace(
  workspaces: WorkspaceMap,
  projectPath: string,
  update: (workspace: ProjectWorkspaceState) => ProjectWorkspaceState,
): WorkspaceMap;
export function updateProviderWorkspace(
  workspaces: WorkspaceMap,
  projectPath: string,
  provider: ProviderKind,
  update: (workspace: ProviderWorkspaceState) => ProviderWorkspaceState,
): WorkspaceMap;
export function updateSessionWorkspace(
  workspaces: WorkspaceMap,
  projectPath: string,
  provider: ProviderKind,
  sessionId: string,
  update: (session: SessionWorkspaceState) => SessionWorkspaceState,
): WorkspaceMap;
export function projectHasActiveTurn(workspace: ProjectWorkspaceState): boolean;
export function applyCodexEvent(workspaces: WorkspaceMap, event: {
  projectPath: string; method: string; params: Record<string, unknown>; requestId?: unknown;
}): WorkspaceMap;
export function applyOpenCodeEvent(workspaces: WorkspaceMap, event: {
  projectPath: string; eventType: string; data: Record<string, unknown>; location?: unknown;
}): WorkspaceMap;
