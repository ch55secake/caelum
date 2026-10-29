import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { GitBranch, RefreshCw, X } from "lucide-react";
import type { ProjectInfo } from "./workspaceState.js";

type GitFileStatus = {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
};
type GitRepositoryStatus = {
  projectPath: string;
  repositoryRoot: string | null;
  branch: string | null;
  changes: GitFileStatus[];
};
type GitFileDiff = {
  path: string;
  staged: string;
  unstaged: string;
  untracked: string;
};

export function GitReviewPanel({
  project,
  working,
  close,
}: {
  project: ProjectInfo;
  working: boolean;
  close: () => void;
}) {
  const [status, setStatus] = useState<GitRepositoryStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [isLoadingStatus, setIsLoadingStatus] = useState(false);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [diff, setDiff] = useState<GitFileDiff | null>(null);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const diffRequest = useRef(0);
  const selection = useRef<string | null>(null);
  const refresh = useRef<() => void>(() => {});

  const selectFile = useCallback(
    async (path: string | null) => {
      const request = ++diffRequest.current;
      if (selection.current !== path) {
        setDiff(null);
        setSelectedPath(path);
        selection.current = path;
      }
      setDiffError(null);
      setIsLoadingDiff(Boolean(path));
      if (!path) return;
      try {
        const result = await invoke<GitFileDiff>("git_file_diff", {
          projectPath: project.path,
          filePath: path,
        });
        if (request === diffRequest.current) setDiff(result);
      } catch (reason) {
        if (request === diffRequest.current) setDiffError(String(reason));
      } finally {
        if (request === diffRequest.current) setIsLoadingDiff(false);
      }
    },
    [project.path],
  );

  useEffect(() => {
    let disposed = false;
    let pending = false;
    if (!isTauri()) {
      setIsLoadingStatus(false);
      setStatusError("Git inspection is available in the Caelum desktop app.");
      return () => {
        disposed = true;
      };
    }

    async function readChanges() {
      if (pending || disposed || document.hidden) return;
      pending = true;
      setIsLoadingStatus(true);
      try {
        const snapshot = await invoke<GitRepositoryStatus>(
          "git_repository_status",
          { projectPath: project.path },
        );
        if (disposed) return;
        setStatus(snapshot);
        setStatusError(null);
        const path = snapshot.changes.some(
          (change) => change.path === selection.current,
        )
          ? selection.current
          : (snapshot.changes[0]?.path ?? null);
        await selectFile(path);
      } catch (reason) {
        if (!disposed) setStatusError(String(reason));
      } finally {
        pending = false;
        if (!disposed) setIsLoadingStatus(false);
      }
    }
    refresh.current = () => void readChanges();
    void readChanges();
    // Check while tools run and after a turn finishes, including background sessions.
    const timer = window.setInterval(readChanges, working ? 2500 : 10000);
    const onFocus = () => void readChanges();
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onFocus);

    return () => {
      disposed = true;
      diffRequest.current += 1;
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onFocus);
    };
  }, [project.path, working, selectFile]);

  return (
    <section className="git-review" aria-labelledby="git-review-title">
      <header className="git-review-heading">
        <GitBranch size={17} />
        <div>
          <h1 id="git-review-title" title={project.path}>
            Changes
          </h1>
        </div>
        <button
          className="icon-button"
          onClick={() => refresh.current()}
          disabled={isLoadingStatus}
          aria-label="Refresh Git status"
          title="Refresh Git status"
        >
          <RefreshCw
            size={14}
            className={isLoadingStatus ? "spinning" : undefined}
          />
        </button>
        <button
          className="icon-button"
          onClick={close}
          aria-label="Close changes"
          title="Close changes"
        >
          <X size={14} />
        </button>
      </header>

      {statusError && (
        <div className="error-banner" role="alert">
          Could not read Git status: {statusError}
        </div>
      )}
      {isLoadingStatus && !status && (
        <div className="git-empty" role="status">
          Checking repository status…
        </div>
      )}
      {status && status.repositoryRoot === null && (
        <div className="git-empty">
          <strong>Not a Git repository</strong>
          <span>This project folder is not inside a Git working tree.</span>
        </div>
      )}
      {status?.repositoryRoot && (
        <>
          <div className="git-repository-meta">
            <div title={status.repositoryRoot}>
              <GitBranch size={13} />
              <strong>{status.branch ?? "Detached HEAD"}</strong>
            </div>
            <span>
              {status.changes.length}{" "}
              {status.changes.length === 1 ? "file" : "files"}
            </span>
          </div>
          {status.changes.length === 0 ? (
            <div className="git-empty">
              <strong>Working tree clean</strong>
              <span>There are no staged, unstaged, or untracked changes.</span>
            </div>
          ) : (
            <div className="git-review-grid">
              <div className="git-changes-list" aria-label="Changed files">
                {status.changes.map((change) => {
                  const code = `${change.indexStatus}${change.worktreeStatus}`;
                  const statusDescription = describeChange(change);
                  return (
                    <button
                      key={change.path}
                      className={`git-file-item${selectedPath === change.path ? " selected" : ""}`}
                      onClick={() => void selectFile(change.path)}
                      aria-pressed={selectedPath === change.path}
                      title={`${change.path} — ${statusDescription}`}
                      aria-label={`${change.path}, ${statusDescription}`}
                    >
                      <span
                        className={`git-status-code${code === "??" ? " untracked" : ""}`}
                        aria-hidden="true"
                      >
                        {code.replaceAll(" ", "·")}
                      </span>
                      <span className="git-file-path">{change.path}</span>
                    </button>
                  );
                })}
              </div>
              <div
                className="git-diff-panel"
                aria-label="Selected file diff"
                aria-live="polite"
              >
                {!selectedPath && (
                  <div className="git-empty">
                    Select a changed file to inspect its diff.
                  </div>
                )}
                {selectedPath && (
                  <div className="git-diff-heading">{selectedPath}</div>
                )}
                {isLoadingDiff && !diff && (
                  <div className="git-empty" role="status">
                    Loading diff…
                  </div>
                )}
                {diffError && (
                  <div className="error-banner" role="alert">
                    Could not read this diff: {diffError}
                  </div>
                )}
                {diff && (
                  <div className="git-diff-content">
                    <DiffSection title="Unstaged" content={diff.unstaged} />
                    <DiffSection title="Staged" content={diff.staged} />
                    <DiffSection title="Untracked" content={diff.untracked} />
                    {!diff.staged && !diff.unstaged && !diff.untracked && (
                      <div className="git-empty">
                        Git reports this file as changed, but there is no text
                        diff to display.
                      </div>
                    )}
                  </div>
                )}
              </div>
            </div>
          )}
        </>
      )}
    </section>
  );
}

function describeChange(change: GitFileStatus) {
  if (change.indexStatus === "?" && change.worktreeStatus === "?")
    return "Untracked";
  const label = (status: string) =>
    ({
      M: "modified",
      A: "added",
      D: "deleted",
      R: "renamed",
      C: "copied",
      U: "unmerged",
    })[status] ?? "changed";
  const states = [
    change.indexStatus !== " " && `Staged ${label(change.indexStatus)}`,
    change.worktreeStatus !== " " && `Unstaged ${label(change.worktreeStatus)}`,
  ].filter(Boolean);
  return states.join("; ") || "Changed";
}

function DiffSection({ title, content }: { title: string; content: string }) {
  if (!content) return null;
  const lines = content.split("\n");
  return (
    <section className="git-diff-section" aria-label={`${title} diff`}>
      <h2>{title}</h2>
      <pre>
        {lines.map((line, index) => {
          const lineClass =
            line.startsWith("+") && !line.startsWith("+++")
              ? "added"
              : line.startsWith("-") && !line.startsWith("---")
                ? "removed"
                : line.startsWith("@@")
                  ? "hunk"
                  : "";
          return (
            <span className={lineClass} key={`${index}-${line}`}>
              {line}
              {index < lines.length - 1 ? "\n" : ""}
            </span>
          );
        })}
      </pre>
    </section>
  );
}
