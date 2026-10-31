import type { ProjectInfo } from "./workspaceState.js";

export type GitWorktree = {
  path: string;
  project: ProjectInfo;
  branch: string | null;
  main: boolean;
  locked: string | null;
  prunable: string | null;
  available: boolean;
};

export type GitWorktrees = {
  project: ProjectInfo;
  storageDirectory: string;
  branches: string[];
  defaultBase: string;
  worktrees: GitWorktree[];
};

export const worktreeLabel = (entry: GitWorktree) =>
  entry.main
    ? "Original checkout"
    : (entry.branch ?? entry.path.split("/").at(-1) ?? "Detached HEAD");
