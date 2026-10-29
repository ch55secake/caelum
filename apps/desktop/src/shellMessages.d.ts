import type { ConversationEntry } from "./workspaceState.js";
export type ShellMessagePart =
  | { type: "text"; text: string }
  | {
      type: "shell";
      id: string;
      command: string;
      state: string;
      output: string;
    };
export function splitShellMessages(text: string): ShellMessagePart[];
export function conversationPresentation(
  entries: ConversationEntry[],
): ConversationEntry[];
export function lastUserMessage(entries: ConversationEntry[]): string | null;
