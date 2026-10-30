import type { DiffLine } from "./diffHighlighting.js";

export type ToolPatchFile = {
  path: string;
  moveTo?: string;
  kind: "add" | "update" | "delete";
  lines: DiffLine[];
  additions: number;
  deletions: number;
  omittedLines: number;
  shortened: boolean;
};

export function toolPatchPreview(input: unknown): {
  files: ToolPatchFile[];
  omittedFiles: number;
};
