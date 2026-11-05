import type {
  ProviderKind,
  SessionSummary,
  WorkspaceMap,
} from "./workspaceState.js";
export type PromptRun = {
  id: string;
  startedAt: number;
  finishedAt: number | null;
  outcome: string | null;
  awaitingStart?: boolean;
  submissionId?: string;
  queuedMs?: number;
};
export type SessionActivity = {
  unread: boolean;
  updatedAt: number;
  readAt: number;
  observedAt: number;
  run: PromptRun | null;
  settled: string[];
};
export type ActivityEvent = {
  sessionId: string;
  type: "started" | "completed" | "content";
  executionId?: string;
  eventId?: string;
  at?: number;
  outcome?: string;
  quiet?: boolean;
};
export type AttentionRequest = {
  provider: string;
  projectPath: string;
  sessionId?: string | null;
};
export type ActivityTarget = {
  key: string;
  path: string;
  provider: ProviderKind;
  session: SessionSummary;
};
export const READ_MARKERS_KEY: string;
export const MAX_READ_MARKERS: number;
export function activityKey(
  path: string,
  provider: string,
  id?: string,
): string;
export function emptyActivity(): SessionActivity;
export function activityEvent(
  provider: ProviderKind,
  method: string,
  data: Record<string, unknown>,
  envelope?: { id?: string; created?: number },
): ActivityEvent | null;
export function applyActivity(
  record: SessionActivity | undefined,
  event: ActivityEvent,
  viewed: boolean,
  now: number,
): { record: SessionActivity; completion?: PromptRun };
export function readMarkers(value: unknown): Map<string, SessionActivity>;
export function savedMarkers(
  records: Map<string, SessionActivity>,
): [string, Pick<SessionActivity, "unread" | "updatedAt" | "readAt">][];
export function projectActivity(
  workspaces: WorkspaceMap,
  member: (path: string) => boolean,
  lookup: (key: string) => SessionActivity | undefined,
  requests: AttentionRequest[],
): {
  working: ActivityTarget[];
  waiting: ActivityTarget[];
  unread: ActivityTarget[];
  attention: AttentionRequest[];
};
