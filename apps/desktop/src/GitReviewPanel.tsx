import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { GitBranch, RefreshCw } from "lucide-react";

type ProjectInfo = { name: string; path: string };
type GitFileStatus = { path: string; indexStatus: string; worktreeStatus: string };
type GitRepositoryStatus = {
  projectPath: string;
  repositoryRoot: string | null;
  branch: string | null;
  changes: GitFileStatus[];
};
type GitFileDiff = { path: string; staged: string; unstaged: string; untracked: string };

export function GitReviewPanel({ project }: { project: ProjectInfo | null }) {
  const [status, setStatus] = useState<GitRepositoryStatus | null>(null);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [isLoadingStatus, setIsLoadingStatus] = useState(false);
  const [selectedPath, setSelectedPath] = useState<string | null>(null);
  const [diff, setDiff] = useState<GitFileDiff | null>(null);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const [refreshKey, setRefreshKey] = useState(0);
  const diffRequest = useRef(0);

  useEffect(() => {
    let disposed = false;
    setStatus(null);
    setStatusError(null);
    setSelectedPath(null);
    setDiff(null);
    setDiffError(null);

    if (!project) {
      setIsLoadingStatus(false);
      return () => {
        disposed = true;
      };
    }
    if (!isTauri()) {
      setIsLoadingStatus(false);
      setStatusError("Git inspection is available in the Caelum desktop app.");
      return () => {
        disposed = true;
      };
    }

    setIsLoadingStatus(true);
    void invoke<GitRepositoryStatus>("git_repository_status", { projectPath: project.path })
      .then((snapshot) => {
        if (!disposed) setStatus(snapshot);
      })
      .catch((reason: unknown) => {
        if (!disposed) setStatusError(String(reason));
      })
      .finally(() => {
        if (!disposed) setIsLoadingStatus(false);
      });

    return () => {
      disposed = true;
      diffRequest.current += 1;
    };
  }, [project?.path, refreshKey]);

  async function selectFile(path: string) {
    if (!project) return;
    const request = ++diffRequest.current;
    setSelectedPath(path);
    setDiff(null);
    setDiffError(null);
    setIsLoadingDiff(true);
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
  }

  if (!project) {
    return (
      <section className="git-review" aria-labelledby="git-review-title">
        <div className="git-review-heading"><GitBranch size={17} /><div><h1 id="git-review-title">Changes</h1><p>Open a project to inspect Git status and diffs.</p></div></div>
      </section>
    );
  }

  return (
    <section className="git-review" aria-labelledby="git-review-title">
      <header className="git-review-heading">
        <GitBranch size={17} />
        <div><h1 id="git-review-title" title={project.path}>Changes</h1></div>
        <button className="git-refresh" onClick={() => setRefreshKey((key) => key + 1)} disabled={isLoadingStatus} aria-label="Refresh Git status" title="Refresh Git status">
          <RefreshCw size={14} className={isLoadingStatus ? "spinning" : undefined} />
          <span>Refresh</span>
        </button>
      </header>

      {statusError && <div className="error-banner" role="alert">Could not read Git status: {statusError}</div>}
      {isLoadingStatus && !status && <div className="git-empty" role="status">Checking repository status…</div>}
      {status && status.repositoryRoot === null && (
        <div className="git-empty"><strong>Not a Git repository</strong><span>This project folder is not inside a Git working tree.</span></div>
      )}
      {status?.repositoryRoot && (
        <>
          <div className="git-repository-meta">
            <div title={status.repositoryRoot}><GitBranch size={13} /><strong>{status.branch ?? "Detached HEAD"}</strong></div>
            <div><strong>{status.changes.length}</strong><span>changed files</span></div>
          </div>
          {status.changes.length === 0 ? (
            <div className="git-empty"><strong>Working tree clean</strong><span>There are no staged, unstaged, or untracked changes.</span></div>
          ) : (
            <div className="git-review-grid">
              <div className="git-changes-list" aria-label="Changed files">
                <div className="git-list-heading">CHANGED FILES <span>{status.changes.length}</span></div>
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
                      <span className={`git-status-code${code === "??" ? " untracked" : ""}`} aria-hidden="true">{code.replaceAll(" ", "·")}</span>
                      <span className="git-file-path">{change.path}</span>
                    </button>
                  );
                })}
              </div>
              <div className="git-diff-panel" aria-label="Selected file diff" aria-live="polite">
                {!selectedPath && <div className="git-empty">Select a changed file to inspect its diff.</div>}
                {selectedPath && <div className="git-diff-heading">{selectedPath}</div>}
                {isLoadingDiff && <div className="git-empty" role="status">Loading diff…</div>}
                {diffError && <div className="error-banner" role="alert">Could not read this diff: {diffError}</div>}
                {diff && (
                  <div className="git-diff-content">
                    <DiffSection title="Unstaged" content={diff.unstaged} />
                    <DiffSection title="Staged" content={diff.staged} />
                    <DiffSection title="Untracked" content={diff.untracked} />
                    {!diff.staged && !diff.unstaged && !diff.untracked && (
                      <div className="git-empty">Git reports this file as changed, but there is no text diff to display.</div>
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
  if (change.indexStatus === "?" && change.worktreeStatus === "?") return "Untracked";
  const label = (status: string) => ({
    M: "modified",
    A: "added",
    D: "deleted",
    R: "renamed",
    C: "copied",
    U: "unmerged",
  }[status] ?? "changed");
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
      <pre>{lines.map((line, index) => {
        const lineClass = line.startsWith("+") && !line.startsWith("+++")
          ? "added"
          : line.startsWith("-") && !line.startsWith("---")
            ? "removed"
            : line.startsWith("@@")
              ? "hunk"
              : "";
        return <span className={lineClass} key={`${index}-${line}`}>{line}{index < lines.length - 1 ? "\n" : ""}</span>;
      })}</pre>
    </section>
  );
}
