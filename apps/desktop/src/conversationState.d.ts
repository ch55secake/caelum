import type {
  ConversationEntry,
  ProviderKind,
  SessionWorkspaceState,
} from "./workspaceState.js";
export function displayValue(value: unknown): string;
export function historyEntries(
  provider: ProviderKind,
  data: Record<string, unknown>[],
): ConversationEntry[];
export function mergeHistory(
  live: ConversationEntry[],
  history: ConversationEntry[],
  mode?: "prepend" | "refresh",
  beforeId?: string | null,
): ConversationEntry[];
export function upsertEntry(
  entries: ConversationEntry[],
  entry: ConversationEntry,
): ConversationEntry[];
export function applyConversationEvent(
  session: SessionWorkspaceState,
  provider: ProviderKind,
  method: string,
  data: Record<string, unknown>,
): SessionWorkspaceState;
