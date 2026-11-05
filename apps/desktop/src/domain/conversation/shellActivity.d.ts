import type { ConversationEntry } from "../sessions/workspaceState.js";
export type BackgroundShell = {
  id: string;
  command: string;
  startedAt: number;
};
export type ShellDetails = {
  command: string;
  cwd?: string;
  exit?: number;
  signal?: string;
  background?: boolean;
  independent?: boolean;
  outputTruncated?: boolean;
  timedOut?: boolean;
};
export function shellDetails(
  name: string | undefined,
  input: unknown,
  metadata?: Record<string, unknown>,
): ShellDetails | undefined;
export function shellEntry(
  shell: unknown,
  output?: unknown,
): ConversationEntry | null;
export function recentShellOutput(value: unknown): string;
