import type { FileContext } from "./fileContext.js";

export type SlashCommand = { name: string; description?: string | null };
export function parseSlashCommand(
  text: string,
): { name: string; arguments: string } | null;
export function commandFiles(
  files: FileContext[],
): { uri: string; name: string; description: string }[];
