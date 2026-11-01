import { useEffect, useId, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { FolderGit2, GitBranch, ShieldAlert, X } from "lucide-react";
import {
  readGitStatus,
  subscribeGitStatus,
  type GitRepositoryStatus,
} from "./gitStatus.js";
import type { ProjectInfo } from "./workspaceState.js";
import {
  useContextMenu,
  copyAction,
  revealAction,
  type ContextAction,
} from "./ContextMenu.js";

export function ProjectItem({
  project,
  selected,
  working,
  approval,
  activate,
  close,
  worktreeActions,
}: {
  project: ProjectInfo;
  selected: boolean;
  working: boolean;
  approval: boolean;
  activate: () => void;
  close: () => void;
  worktreeActions: ContextAction[];
}) {
  const detailsId = useId();
  const contextMenu = useContextMenu();
  const row = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<GitRepositoryStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    if (!isTauri()) return;
    const unsubscribe = subscribeGitStatus(project.path, (result) => {
      setStatus(result.status);
      setError(result.error);
    });
    let visible = false;
    const refresh = () => {
      if (visible && !document.hidden)
        void readGitStatus(project.path).catch(() => {});
    };
    const observer = new IntersectionObserver(
      (entries) => {
        visible = entries.some((entry) => entry.isIntersecting);
        if (visible) refresh();
      },
      { root: row.current?.closest(".project-list") },
    );
    if (row.current) observer.observe(row.current);
    const timer = window.setInterval(
      refresh,
      working ? 2500 : selected ? 10000 : 30000,
    );
    window.addEventListener("focus", refresh);
    document.addEventListener("visibilitychange", refresh);
    return () => {
      unsubscribe();
      observer.disconnect();
      window.clearInterval(timer);
      window.removeEventListener("focus", refresh);
      document.removeEventListener("visibilitychange", refresh);
    };
  }, [project.path, selected, working]);
  const branch = status?.branch ?? "Detached HEAD";
  const count = status?.changes.length ?? 0;
  return (
    <div
      ref={row}
      className={`project-row${selected ? " selected" : ""}`}
      onContextMenu={(event) =>
        contextMenu(event, [
          { text: "Open project", action: activate },
          ...worktreeActions,
          { item: "Separator" },
          copyAction("Copy project path", project.path),
          revealAction(project.path),
          { item: "Separator" },
          { text: "Close project", action: close },
        ])
      }
    >
      <div className="project-item">
        <button
          className="sidebar-row-select"
          title={project.path}
          aria-label={project.name}
          aria-describedby={detailsId}
          aria-current={selected ? "page" : undefined}
          onClick={activate}
        >
          <span className="project-heading">
            <FolderGit2 size={14} aria-hidden="true" />
            <span className="project-name">{project.name}</span>
            {working && (
              <span
                className="running-dot"
                title="Running session"
                aria-label="Running session"
              />
            )}
            {approval && (
              <ShieldAlert
                size={13}
                className="approval-indicator"
                aria-label="Approval required"
              />
            )}
          </span>
        </button>
        <span id={detailsId} className="project-details">
          <span className="project-path">{project.path}</span>
          <span className="project-context">
            {error ? (
              <span title={error}>Git unavailable</span>
            ) : !status ? (
              <span>{isTauri() ? "Checking Git…" : "Local folder"}</span>
            ) : !status.repositoryRoot ? (
              <span>Not a Git repository</span>
            ) : (
              <>
                <span
                  className="project-branch"
                  title={`Current branch: ${branch}`}
                >
                  <GitBranch size={10} aria-hidden="true" />
                  <span>{branch}</span>
                </span>
                <span
                  className={`project-changes${count ? " dirty" : ""}`}
                  title={
                    count
                      ? `${count} changed ${count === 1 ? "file" : "files"} in this project, including staged, unstaged, and untracked changes`
                      : "Working tree clean"
                  }
                >
                  {count ? `${count} changed` : "Clean"}
                </span>
              </>
            )}
          </span>
          {working && <span className="sr-only">Running session</span>}
          {approval && <span className="sr-only">Approval required</span>}
        </span>
      </div>
      <button
        className="icon-button project-close"
        aria-label={`Close ${project.name}`}
        onClick={close}
      >
        <X size={12} />
      </button>
    </div>
  );
}
