export type GitFileStatus = {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
  modifiedAt: string | null;
  revision: string;
};
export type GitFileDiff = {
  path: string;
  staged: string;
  unstaged: string;
  untracked: string;
};
export type DiffSectionKey = "unstaged" | "staged" | "untracked";
export type DiffHighlights = Record<DiffSectionKey, number[]>;
export const diffSections: DiffSectionKey[];
export function latestChangedFile(
  previous: GitFileStatus[] | null,
  changes: GitFileStatus[],
): GitFileStatus | null;
export function changedDiffLines(
  previous: GitFileDiff | null,
  next: GitFileDiff,
): DiffHighlights;
