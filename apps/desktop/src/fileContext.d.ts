export type FileContext = {
  id: string;
  path: string;
  content: string;
  startLine?: number;
  endLine?: number;
  truncated: boolean;
};
export type FileLocation = { path: string; line?: number; column?: number };
export const MAX_FILE_CONTEXT: number;
export function fileContextLabel(file: FileContext): string;
export function createFileContext(
  path: string,
  content: string,
  range?: { startLine: number; endLine: number },
  truncated?: boolean,
): FileContext;
export function promptWithFileContext(
  text: string,
  files: FileContext[],
): string;
export function splitFileContext(prompt: string): {
  text: string;
  files: FileContext[];
};
export function fileReference(
  value: string,
  project: string,
): FileLocation | null;
