import { invoke } from "@tauri-apps/api/core";
import type { GitFileStatus } from "./gitReviewState.js";

export type GitRepositoryStatus = {
  projectPath: string;
  repositoryRoot: string | null;
  branch: string | null;
  changes: GitFileStatus[];
};
type Result =
  | { status: GitRepositoryStatus; error: null }
  | { status: null; error: string };
const pending = new Map<string, Promise<GitRepositoryStatus>>();
const listeners = new Map<string, Set<(result: Result) => void>>();

export function subscribeGitStatus(
  path: string,
  listener: (result: Result) => void,
) {
  const set = listeners.get(path) ?? new Set();
  set.add(listener);
  listeners.set(path, set);
  return () => {
    set.delete(listener);
    if (!set.size) listeners.delete(path);
  };
}

export function readGitStatus(
  projectPath: string,
): Promise<GitRepositoryStatus> {
  const existing = pending.get(projectPath);
  if (existing) return existing;
  const publish = (result: Result) =>
    listeners.get(projectPath)?.forEach((listener) => listener(result));
  const request = invoke<GitRepositoryStatus>("git_repository_status", {
    projectPath,
  })
    .then(
      (status) => {
        publish({ status, error: null });
        return status;
      },
      (error: unknown) => {
        publish({ status: null, error: String(error) });
        throw error;
      },
    )
    .finally(() => pending.delete(projectPath));
  pending.set(projectPath, request);
  return request;
}
