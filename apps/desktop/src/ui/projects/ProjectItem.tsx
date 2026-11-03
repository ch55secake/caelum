import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { FolderGit2, GitBranch, X } from "lucide-react";
import {
  readGitStatus,
  subscribeGitStatus,
  type GitRepositoryStatus,
} from "../../application/gitStatus.js";
import type { ProjectInfo } from "../../domain/sessions/workspaceState.js";
import { SidebarDetails } from "../primitives/SidebarDetails.js";
import {
  useContextMenu,
  copyAction,
  revealAction,
  type ContextAction,
} from "../primitives/ContextMenu.js";

export function ProjectItem({
  project,
  checkout,
  subtitle,
  selected,
  workingCount,
  approval,
  activate,
  close,
  worktreeActions,
  disclosure,
  activity,
}: {
  project: ProjectInfo;
  checkout: ProjectInfo;
  subtitle: string;
  selected: boolean;
  workingCount: number;
  approval: boolean;
  activate: () => void;
  close: () => void;
  worktreeActions: ContextAction[];
  disclosure?: ReactNode;
  activity: ReactNode;
}) {
  const detailsId = useId();
  const metadataId = useId();
  const contextMenu = useContextMenu();
  const row = useRef<HTMLDivElement>(null);
  const [status, setStatus] = useState<GitRepositoryStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const working = workingCount > 0;
  const workingLabel = `${workingCount} working ${workingCount === 1 ? "session" : "sessions"}`;
  useEffect(() => {
    if (!isTauri()) return;
    const unsubscribe = subscribeGitStatus(checkout.path, (result) => {
      setStatus(result.status);
      setError(result.error);
    });
    let visible = false;
    const refresh = () => {
      if (visible && !document.hidden)
        void readGitStatus(checkout.path).catch(() => {});
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
  }, [checkout.path, selected, working]);
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
      {disclosure}
      <SidebarDetails
        id={detailsId}
        className="project-item"
        details={
          <span className="project-details">
            <span className="project-path">{checkout.path}</span>
            <span className="project-session-detail">{subtitle}</span>
            {working && <span>Running session</span>}
            {approval && <span>Approval required</span>}
          </span>
        }
      >
        <button
          className="sidebar-row-select"
          aria-label={project.name}
          aria-describedby={`${detailsId} ${metadataId}`}
          aria-current={selected ? "page" : undefined}
          onClick={activate}
        >
          <span className="project-heading">
            {!disclosure && <FolderGit2 size={14} aria-hidden="true" />}
            <span className="project-name">{project.name}</span>
            {working && (
              <span
                className="project-working"
                title={workingLabel}
                aria-label={workingLabel}
              >
                <span className="running-dot" aria-hidden="true" />
                {workingCount}
                <span>working</span>
              </span>
            )}
          </span>
          <span className="project-session">{subtitle}</span>
          <span id={metadataId} className="project-context">
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
                  <GitBranch size={11} aria-hidden="true" />
                  <span>{branch}</span>
                </span>
                <span className={`project-changes${count ? " dirty" : ""}`}>
                  {count ? `${count} changed` : "Clean"}
                </span>
              </>
            )}
          </span>
        </button>
        {activity}
      </SidebarDetails>
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
