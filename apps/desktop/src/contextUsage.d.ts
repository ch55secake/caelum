import type { ContextUsage, ConversationEntry } from "./workspaceState.js";

export function contextWindow(value: unknown): number | null;
export function openCodeModel(model: unknown): string | null;
export function codexContextUsage(usage: unknown): ContextUsage | null;
export function openCodeContextUsage(
  tokens: unknown,
  model: string | null,
  timestamp?: number | null,
): ContextUsage | null;
export function openCodeHistoryUsage(
  messages: Record<string, unknown>[],
  since?: number,
): ContextUsage | null | undefined;
export function contextUsageSince(
  usage: ContextUsage | null,
  conversation: ConversationEntry[],
): number;
