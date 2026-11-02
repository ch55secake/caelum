import type { SessionWorkspaceState } from "../sessions/workspaceState.js";
export type QueuedMessage = {
  id: string;
  prompt: string;
  remote: boolean;
  pending: boolean;
  model: string | null;
  effort: string | null;
  agent: string | null;
  error?: string;
};
export function inboxMessages(items: unknown): QueuedMessage[];
export function reconcileInbox(
  current: QueuedMessage[],
  snapshot: QueuedMessage[],
): QueuedMessage[];
export function queueEvent(
  session: SessionWorkspaceState,
  method: string,
  data: Record<string, unknown>,
): SessionWorkspaceState;
