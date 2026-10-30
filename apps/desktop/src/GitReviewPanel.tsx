import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { GitBranch, RefreshCw, X } from "lucide-react";
import { GitStatusIcon } from "./GitStatusIcon.js";
import { DiffSection } from "./DiffSection.js";
import { readGitStatus, type GitRepositoryStatus } from "./gitStatus.js";
import type { ProjectInfo } from "./workspaceState.js";
import {
  changedDiffLines,
  diffSections,
  latestChangedFile,
  type DiffHighlights,
  type GitFileDiff,
  type GitFileStatus,
} from "./gitReviewState.js";

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
  const [latestPath, setLatestPath] = useState<string | null>(null);
  const [diff, setDiff] = useState<GitFileDiff | null>(null);
  const [highlights, setHighlights] = useState<DiffHighlights | null>(null);
  const [reveal, setReveal] = useState(0);
  const [diffError, setDiffError] = useState<string | null>(null);
  const [isLoadingDiff, setIsLoadingDiff] = useState(false);
  const diffRequest = useRef(0);
  const selection = useRef<string | null>(null);
  const manualSelection = useRef(0);
  const previousStatus = useRef<GitFileStatus[] | null>(null);
  const diffCache = useRef(
    new Map<
      string,
      {
        diff: GitFileDiff;
        highlights: DiffHighlights;
      }
    >(),
  );
  const diffPanel = useRef<HTMLDivElement>(null);
  const fileList = useRef<HTMLDivElement>(null);
  const refresh = useRef<() => void>(() => {});

  const selectFile = useCallback(
    async (path: string | null) => {
      const request = ++diffRequest.current;
      const switched = selection.current !== path;
      if (switched) {
        setDiff(null);
        setHighlights(null);
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
        if (request !== diffRequest.current) return;
        const cached = diffCache.current.get(path);
        const changed =
          !cached ||
          diffSections.some(
            (section) => cached.diff[section] !== result[section],
          );
        const marked = changed
          ? changedDiffLines(cached?.diff ?? null, result)
          : cached.highlights;
        diffCache.current.set(path, { diff: result, highlights: marked });
        setDiff(result);
        setHighlights(marked);
        if (switched || changed) setReveal((value) => value + 1);
      } catch (reason) {
        if (request === diffRequest.current) setDiffError(String(reason));
      } finally {
        if (request === diffRequest.current) setIsLoadingDiff(false);
      }
    },
    [project.path],
  );

  useLayoutEffect(() => {
    const panel = diffPanel.current;
    if (!panel) return;
    const target = panel.querySelector<HTMLElement>("[data-latest-change]");
    const heading = panel.querySelector(".git-diff-heading");
    // Scroll only the review pane; scrollIntoView can move the conversation
    // or the entire workspace when the composer has keyboard focus.
    panel.scrollTop = target
      ? panel.scrollTop +
        target.getBoundingClientRect().top -
        panel.getBoundingClientRect().top -
        (heading?.clientHeight ?? 0) -
        24
      : 0;
    const list = fileList.current;
    const selected = list?.querySelector<HTMLElement>(".selected");
    if (list && selected) {
      const row = selected.getBoundingClientRect();
      const bounds = list.getBoundingClientRect();
      if (row.top < bounds.top) list.scrollTop += row.top - bounds.top;
      else if (row.bottom > bounds.bottom)
        list.scrollTop += row.bottom - bounds.bottom;
    }
  }, [reveal]);

  useEffect(() => {
    let disposed = false;
    let pending = false;
    if (!isTauri()) {
      setIsLoadingStatus(false);
      setStatusError(
        "Git inspection is available in the Caelum desktop app.",
      );
      return () => {
        disposed = true;
      };
    }

    async function readChanges() {
      if (pending || disposed || document.hidden) return;
      pending = true;
      const interaction = manualSelection.current;
      setIsLoadingStatus(true);
      try {
        const snapshot = await readGitStatus(project.path);
        if (disposed) return;
        setStatus(snapshot);
        setStatusError(null);
        const latest = latestChangedFile(
          previousStatus.current,
          snapshot.changes,
        );
        previousStatus.current = snapshot.changes;
        const newest =
          latestChangedFile(null, snapshot.changes)?.path ?? null;
        setLatestPath(
          (current) =>
            latest?.path ??
            (snapshot.changes.some((change) => change.path === current)
              ? current
              : newest),
        );
        const paths = new Set(snapshot.changes.map((change) => change.path));
        for (const path of diffCache.current.keys())
          if (!paths.has(path)) diffCache.current.delete(path);
        const path =
          latest && interaction === manualSelection.current
            ? latest.path
            : snapshot.changes.some(
                  (change) => change.path === selection.current,
                )
              ? selection.current
              : newest;
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
              <span>
                There are no staged, unstaged, or untracked changes.
              </span>
            </div>
          ) : (
            <div className="git-review-grid">
              <div
                ref={fileList}
                className="git-changes-list"
                aria-label="Changed files"
              >
                {status.changes.map((change) => {
                  const statusDescription = describeChange(change);
                  return (
                    <button
                      key={change.path}
                      className={`git-file-item${selectedPath === change.path ? " selected" : ""}${latestPath === change.path ? " latest" : ""}`}
                      onClick={() => {
                        manualSelection.current += 1;
                        void selectFile(change.path);
                      }}
                      aria-pressed={selectedPath === change.path}
                      title={`${change.path} — ${statusDescription}${latestPath === change.path ? " — Latest change" : ""}`}
                      aria-label={`${change.path}, ${statusDescription}`}
                    >
                      <GitStatusIcon change={change} />
                      <span className="git-file-path">{change.path}</span>
                    </button>
                  );
                })}
              </div>
              <div
                ref={diffPanel}
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
                    {diffSections.map((section) => (
                      <DiffSection
                        key={section}
                        path={diff.path}
                        title={
                          section.charAt(0).toUpperCase() + section.slice(1)
                        }
                        content={diff[section]}
                        highlights={
                          selectedPath === latestPath
                            ? highlights?.[section]
                            : undefined
                        }
                      />
                    ))}
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
    change.worktreeStatus !== " " &&
      `Unstaged ${label(change.worktreeStatus)}`,
  ].filter(Boolean);
  return states.join("; ") || "Changed";
}
