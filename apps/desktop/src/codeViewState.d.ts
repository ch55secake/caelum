export type CodePosition = { line: number; column: number };
export type ReadingPosition = {
  line: number;
  offset: number;
  left: number;
  anchor: CodePosition;
  head: CodePosition;
};
export type FileViewState = {
  main: ReadingPosition;
  original?: ReadingPosition;
};
export function restoreFileView(value: unknown): FileViewState | null;
export function selectedFileText(
  content: string,
  start: CodePosition,
  end: CodePosition,
): string;
