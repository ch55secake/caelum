import type { FileContext } from "./fileContext.js";

export type ProviderKind = "codex" | "opencode";
export type SessionSummary = {
  id: string;
  name: string | null;
  preview: string;
  updatedAt: number | null;
  isWorking?: boolean;
};
export function sessionTitle(session: SessionSummary): string;
export type SessionDetails = {
  branch: string | null;
  branchIsCurrent: boolean;
  changes: { additions: number; deletions: number } | null;
};
export type ConversationEntry = {
  images?: import("./chatMedia.js").ChatImageSource[];
  sourceType?: "system" | "synthetic";
  fileContext?: FileContext[];
  id: string;
  kind: "user" | "assistant" | "tool" | "status" | "reasoning" | "compaction";
  text: string;
  optimistic?: boolean;
  complete?: boolean;
  historyText?: string;
  streamText?: string;
  messageId?: string;
  reasoning?: {
    summary: Record<
      number,
      { text: string; historyText?: string; streamText?: string }
    >;
    content: Record<
      number,
      { text: string; historyText?: string; streamText?: string }
    >;
  };
  historyOutput?: string;
  streamOutput?: string;
  name?: string;
  executionId?: string;
  status?: "running" | "completed" | "failed" | "interrupted";
  input?: unknown;
  output?: unknown;
  error?: string;
  durationMs?: number;
  startedAt?: number;
  compactionReason?: "auto" | "manual";
};
export type ProjectInfo = { name: string; path: string };
export type WorkspaceSessionInfo = {
  project: ProjectInfo;
  thread: SessionSummary;
  provider: ProviderKind;
  model?: string | null;
  effort?: string | null;
  agent?: string | null;
};
export type ModelChoice = {
  id: string;
  name: string;
  provider?: { id: string; name: string } | null;
  contextWindow?: number | null;
  efforts: { id: string; name: string }[];
  defaultEffort?: string | null;
};
export type ProviderChoices = {
  models: ModelChoice[];
  agents: { id: string; name: string }[];
  defaultModel?: string | null;
};

export type SessionWorkspaceState = {
  session: WorkspaceSessionInfo;
  /** Pending selection for the next prompt; null keeps the provider's session model. */
  model: string | null;
  /** Null keeps the session effort; an empty string selects the model default. */
  effort: string | null;
  /** Pending agent for the next prompt; null keeps the native session agent. */
  agent: string | null;
  conversation: ConversationEntry[];
  isWorking: boolean;
  isSubmitting: boolean;
  error: string | null;
  draft: string;
  fileContext: FileContext[];
  connection: "connecting" | "connected" | "disconnected";
  historyError: string | null;
  historyLoaded: boolean;
  isLoadingHistory: boolean;
  nextCursor: string | null;
  historyAnchor: string | null;
  unread: boolean;
  expandedTools: Record<string, boolean>;
  scrollTop: number | null;
  windowEndId: string | null;
  eventRevision: number;
  followRevision: number;
  activity: { label: string; detail?: string } | null;
  lastOutcome: string | null;
  contextUsage: ContextUsage | null;
  contextResetAt: number | null;
  contextStep: {
    id: string;
    model: string | null;
    timestamp: number | null;
  } | null;
};

export type ContextUsage = {
  usedTokens: number;
  contextWindow: number | null;
  model: string | null;
  timestamp: number | null;
};

export type ProviderWorkspaceState = {
  sessions: SessionSummary[];
  sessionDetails: Record<string, SessionDetails>;
  archivingSessionIds: string[];
  archivedSessionIds: string[];
  sessionsLoaded: boolean;
  isLoadingSessions: boolean;
  selectedSessionId: string | null;
  sessionsById: Record<string, SessionWorkspaceState>;
  error: string | null;
  draft: string;
  fileContext: FileContext[];
  isCreating: boolean;
  choices: ProviderChoices | null;
  choicesError: string | null;
  isLoadingChoices: boolean;
  /** Null inherits app defaults; an empty string uses the provider default. */
  model: string | null;
  effort: string | null;
  agent: string | null;
};

export type ProjectWorkspaceState = {
  provider: ProviderKind;
  providers: Record<ProviderKind, ProviderWorkspaceState>;
};

export type WorkspaceMap = Record<string, ProjectWorkspaceState>;

export function createProjectWorkspace(): ProjectWorkspaceState;
export function createSessionWorkspace(
  session: WorkspaceSessionInfo,
): SessionWorkspaceState;
export function restoreProjectWorkspace(persisted: {
  provider: ProviderKind;
  codexSessionId: string | null;
  opencodeSessionId: string | null;
  codexSessionIds: string[];
  opencodeSessionIds: string[];
}): ProjectWorkspaceState;
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
export function applyCodexEvent(
  workspaces: WorkspaceMap,
  event: {
    projectPath: string;
    method: string;
    params: Record<string, unknown>;
    requestId?: unknown;
  },
): WorkspaceMap;
export function applyOpenCodeEvent(
  workspaces: WorkspaceMap,
  event: {
    projectPath: string;
    eventType: string;
    id?: string;
    created?: number;
    data: Record<string, unknown>;
    location?: unknown;
  },
): WorkspaceMap;
