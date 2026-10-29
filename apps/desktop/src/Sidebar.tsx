import {
  FileCode2,
  FolderGit2,
  GitBranch,
  LoaderCircle,
  MessageSquare,
  MessageSquarePlus,
  Orbit,
  PanelLeftClose,
  Plus,
  RefreshCw,
  Search,
  ShieldAlert,
  X,
} from "lucide-react";
import type { RefObject } from "react";
import {
  projectHasActiveTurn,
  type ProviderKind,
  type SessionSummary,
} from "./workspaceState.js";
import type { useWorkspace } from "./useWorkspace.js";
export type WorkspaceController = ReturnType<typeof useWorkspace>;
export function sessionTitle(item: SessionSummary) {
  return item.name || item.preview || "Untitled session";
}

export function Sidebar({
  app,
  collapse,
  search,
  setSearch,
  searchRef,
  newSession,
}: {
  app: WorkspaceController;
  collapse: () => void;
  search: string;
  setSearch: (text: string) => void;
  searchRef: RefObject<HTMLInputElement | null>;
  newSession: () => void;
}) {
  const sessions = (["codex", "opencode"] as ProviderKind[])
    .flatMap((provider) =>
      (app.workspace?.providers[provider].sessions ?? []).map((session) => ({
        ...session,
        provider,
      })),
    )
    .sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0))
    .filter((item) =>
      `${sessionTitle(item)} ${item.provider}`
        .toLowerCase()
        .includes(search.toLowerCase()),
    );
  return (
    <aside className="sidebar">
      <div className="brand-row">
        <Orbit size={18} />
        <strong>caelum</strong>
        <button
          className="icon-button"
          aria-label="Collapse sidebar"
          title="Collapse sidebar"
          onClick={collapse}
        >
          <PanelLeftClose size={16} />
        </button>
      </div>
      <div className="section-heading">
        <span>Projects</span>
        <button
          className="icon-button"
          aria-label="Open project"
          onClick={() => void app.chooseProject()}
          disabled={app.selecting || app.restoring}
        >
          <Plus size={14} />
        </button>
      </div>
      <nav className="project-list" aria-label="Open projects">
        {app.restoring && <span className="empty-sessions">Restoring…</span>}
        {app.projects.map((project) => (
          <div
            key={project.path}
            className={`project-row${app.project?.path === project.path ? " selected" : ""}`}
          >
            <button
              className="project-item"
              title={project.path}
              aria-current={
                app.project?.path === project.path ? "page" : undefined
              }
              onClick={() => app.activateProject(project)}
            >
              <FolderGit2 size={14} />
              <span>{project.name}</span>
              {app.approvals.some(
                (request) => request.projectPath === project.path,
              ) ? (
                <ShieldAlert
                  size={13}
                  className="approval-indicator"
                  aria-label="Approval required"
                />
              ) : app.workspaces[project.path] &&
                projectHasActiveTurn(app.workspaces[project.path]) ? (
                <span className="running-dot" aria-label="Running" />
              ) : null}
            </button>
            <button
              className="icon-button project-close"
              aria-label={`Close ${project.name}`}
              onClick={() => app.closeProject(project.path)}
            >
              <X size={12} />
            </button>
          </div>
        ))}
      </nav>
      <button
        className="new-session"
        onClick={newSession}
        disabled={!app.project}
      >
        <MessageSquarePlus size={15} />
        <span>New session</span>
        <kbd>⌘N</kbd>
      </button>
      <nav className="side-nav" aria-label="Workspace">
        <button
          className={`nav-item${app.view === "session" ? " active" : ""}`}
          onClick={() => app.setView("session")}
        >
          <MessageSquare size={15} />
          Sessions
        </button>
        <button
          className={`nav-item${app.view === "files" ? " active" : ""}`}
          disabled={!app.project}
          onClick={() => app.setView("files")}
        >
          <FileCode2 size={15} />
          Files
        </button>
        <button
          className={`nav-item${app.view === "changes" ? " active" : ""}`}
          disabled={!app.project}
          onClick={() => app.setView("changes")}
        >
          <GitBranch size={15} />
          Changes
        </button>
      </nav>
      <div className="section-heading sessions-heading">
        <span>Sessions</span>
        <button
          className="icon-button"
          title="Refresh sessions"
          aria-label="Refresh sessions"
          disabled={!app.project || app.providerWorkspace?.isLoadingSessions}
          onClick={() => {
            if (app.project) void app.loadSessions(app.project, app.provider);
          }}
        >
          <RefreshCw
            size={13}
            className={
              app.providerWorkspace?.isLoadingSessions ? "spinning" : ""
            }
          />
        </button>
      </div>
      <label className="session-search">
        <Search size={13} />
        <input
          ref={searchRef}
          placeholder="Search sessions…"
          aria-label="Search sessions"
          value={search}
          onChange={(event) => setSearch(event.target.value)}
        />
        <kbd>⌘K</kbd>
      </label>
      <div className="session-list">
        {app.providerWorkspace?.isLoadingSessions && sessions.length === 0 && (
          <div className="empty-sessions">Connecting…</div>
        )}
        {sessions.length === 0 && !app.providerWorkspace?.isLoadingSessions && (
          <div className="empty-sessions">
            {search ? "No matching sessions" : "No sessions yet"}
          </div>
        )}
        {sessions.map((item) => {
          const state =
            app.workspace?.providers[item.provider].sessionsById[item.id];
          const approval = app.approvals.some(
            (request) =>
              request.projectPath === app.project?.path &&
              request.provider === item.provider &&
              request.sessionId === item.id,
          );
          return (
            <button
              key={`${item.provider}-${item.id}`}
              className={`session-item${app.provider === item.provider && app.selectedId === item.id && app.view === "session" ? " selected" : ""}`}
              title={sessionTitle(item)}
              onClick={() => app.selectSession(item, item.provider)}
            >
              <span className="session-provider" title={item.provider}>
                {item.provider === "codex" ? "C" : "O"}
              </span>
              <span className="session-title">{sessionTitle(item)}</span>
              {approval ? (
                <ShieldAlert
                  size={13}
                  className="approval-indicator"
                  aria-label="Approval required"
                />
              ) : state?.isWorking ? (
                <LoaderCircle
                  size={13}
                  className="spinning"
                  aria-label="Running"
                />
              ) : state?.unread ? (
                <span className="unread-dot" aria-label="Unread" />
              ) : state?.connection === "disconnected" ? (
                <span className="disconnected-dot" aria-label="Disconnected" />
              ) : null}
            </button>
          );
        })}
      </div>
    </aside>
  );
}
