import {
  Archive,
  FileCode2,
  GitBranch,
  LoaderCircle,
  MessageSquarePlus,
  Orbit,
  PanelLeftClose,
  Plus,
  RefreshCw,
  Search,
  ShieldAlert,
  Settings,
  X,
} from "lucide-react";
import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type RefObject,
} from "react";
import type {
  ProviderKind,
  SessionSummary,
  SessionDetails,
} from "./workspaceState.js";
import type { useWorkspace } from "./useWorkspace.js";
import { ProjectWorktrees } from "./Worktrees.js";
import { PullRequestIndicator } from "./PullRequestIndicator.js";
import {
  SessionPullRequests,
  useSessionPullRequests,
} from "./SessionPullRequests.js";
import type { SidebarView } from "./useSidebarView.js";
import { usePullRequestPreview } from "./PullRequestPreview.js";
import { useContextMenu, copyAction } from "./ContextMenu.js";
export type WorkspaceController = ReturnType<typeof useWorkspace>;
const MIN_SIDEBAR_WIDTH = 180;
const MAX_SIDEBAR_WIDTH = 480;
export function sessionTitle(item: SessionSummary) {
  return item.name || item.preview || "Untitled session";
}

function SessionContext({
  projectPath,
  details,
  load,
  revision,
}: {
  projectPath: string;
  details?: SessionDetails;
  load: () => void;
  revision: number | null;
}) {
  const element = useRef<HTMLSpanElement>(null);
  const loadRef = useRef(load);
  loadRef.current = load;
  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) loadRef.current();
      },
      { root: element.current?.closest(".session-list") },
    );
    if (element.current) observer.observe(element.current);
    return () => observer.disconnect();
  }, [revision]);
  return (
    <span className="session-context" ref={element}>
      {details?.branch && (
        <span
          className="session-branch"
          title={`${details.branchIsCurrent ? "Current working-directory branch" : "Session branch"}: ${details.branch}`}
        >
          <GitBranch size={10} aria-hidden="true" />
          <span>{details.branch}</span>
        </span>
      )}
      {details?.branch && (
        <PullRequestIndicator
          projectPath={projectPath}
          branch={details.branch}
        />
      )}
      {details?.changes && (
        <span
          className="session-lines"
          title="Lines changed in the latest turn"
          aria-label={`Latest turn: ${details.changes.additions} added, ${details.changes.deletions} deleted`}
        >
          <span className="lines-added">
            +{details.changes.additions.toLocaleString()}
          </span>
          <span className="lines-deleted">
            −{details.changes.deletions.toLocaleString()}
          </span>
        </span>
      )}
    </span>
  );
}

export function Sidebar({
  app,
  preferredWidth,
  setPreferredWidth,
  collapse,
  view,
  searchRef,
  focusSearch,
  newSession,
  changesOpen,
  filesOpen,
  sessionVisible,
  openFiles,
  showSession,
  toggleChanges,
  openSettings,
}: {
  app: WorkspaceController;
  preferredWidth: number | null;
  setPreferredWidth: (width: number) => void;
  collapse: () => void;
  view: SidebarView;
  searchRef: RefObject<HTMLInputElement | null>;
  focusSearch: () => void;
  newSession: () => void;
  changesOpen: boolean;
  filesOpen: boolean;
  sessionVisible: boolean;
  openFiles: () => void;
  showSession: () => void;
  toggleChanges: () => void;
  openSettings: () => void;
}) {
  const contextMenu = useContextMenu();
  const { activeUrl } = usePullRequestPreview();
  const selectSession = (item: SessionSummary, provider: ProviderKind) => {
    showSession();
    app.selectSession(item, provider);
  };
  const sidebar = useRef<HTMLElement>(null);
  const [layoutWidth, setLayoutWidth] = useState(window.innerWidth);
  const drag = useRef<{ x: number; width: number } | null>(null);
  const activityId = useId();
  const sessionList = useRef<HTMLDivElement>(null);
  const requestList = useRef<HTMLDivElement>(null);
  const searchButton = useRef<HTMLButtonElement>(null);
  const restoration = useRef<{
    path: string;
    lists: Record<"sessions" | "requests", { top: number; pending: boolean }>;
  } | null>(null);
  const { search, searchOpen } = view;
  const path = app.project?.path ?? "";
  const closeSearch = () => {
    view.update({ search: "", searchOpen: false });
    searchButton.current?.focus();
  };
  useLayoutEffect(() => {
    const container = sidebar.current?.parentElement;
    if (!container) return;
    const measure = () => setLayoutWidth(container.clientWidth);
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(container);
    return () => observer.disconnect();
  }, []);
  const minimumWorkspaceWidth =
    app.project && sessionVisible && (filesOpen || changesOpen) ? 580 : 320;
  const maximum = Math.min(
    MAX_SIDEBAR_WIDTH,
    Math.max(MIN_SIDEBAR_WIDTH, layoutWidth - minimumWorkspaceWidth),
  );
  const width = Math.max(
    MIN_SIDEBAR_WIDTH,
    Math.min(maximum, preferredWidth ?? (layoutWidth <= 980 ? 214 : 238)),
  );
  const resize = (value: number) =>
    setPreferredWidth(Math.max(MIN_SIDEBAR_WIDTH, Math.min(maximum, value)));
  const allSessions = (["codex", "opencode"] as ProviderKind[])
    .flatMap((provider) =>
      (app.workspace?.providers[provider].sessions ?? []).map((session) => ({
        ...session,
        provider,
      })),
    )
    .sort((a, b) => (b.updatedAt ?? 0) - (a.updatedAt ?? 0));
  const sessions = allSessions.filter((item) =>
    `${sessionTitle(item)} ${item.provider} ${app.workspace?.providers[item.provider].sessionDetails[item.id]?.branch ?? ""}`
      .toLowerCase()
      .includes(search.toLowerCase()),
  );
  const pullRequests = useSessionPullRequests(
    app,
    allSessions.map((session) => ({
      ...session,
      title: sessionTitle(session),
    })),
  );
  useLayoutEffect(() => {
    if (restoration.current?.path !== path) {
      restoration.current = {
        path,
        lists: {
          sessions: {
            top: view.positions.current.get(`${path}\0sessions`) ?? 0,
            pending: true,
          },
          requests: {
            top: view.positions.current.get(`${path}\0requests`) ?? 0,
            pending: true,
          },
        },
      };
    }
    for (const name of ["sessions", "requests"] as const) {
      const restore = restoration.current.lists[name];
      const list =
        name === "sessions" ? sessionList.current : requestList.current;
      if (!restore.pending || !list) continue;
      list.scrollTop = restore.top;
      // Wait for discovery to rehydrate before accepting an empty list's clamp.
      const loading =
        name === "sessions"
          ? app.providerWorkspace?.isLoadingSessions
          : pullRequests.loading;
      if (Math.abs(list.scrollTop - restore.top) < 1 || !loading)
        restore.pending = false;
    }
  }, [
    path,
    sessions.length,
    pullRequests.entries.size,
    pullRequests.loading,
    app.providerWorkspace?.isLoadingSessions,
  ]);
  return (
    <aside
      ref={sidebar}
      id="workspace-sidebar"
      className="sidebar"
      aria-label="Projects and sessions"
      style={{ "--sidebar-width": `${width}px` } as CSSProperties}
    >
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
        {app.projectGroups.map((project) => (
          <ProjectWorktrees
            key={project.path}
            app={app}
            project={project}
            showSession={showSession}
          />
        ))}
      </nav>
      <nav className="side-nav" aria-label="Workspace">
        <button
          className="new-session"
          onClick={newSession}
          disabled={!app.project}
          title="New session (⌘N)"
        >
          <MessageSquarePlus size={15} />
          <span>New session</span>
        </button>
        <button
          className={`icon-button${filesOpen ? " active" : ""}`}
          aria-label="Files"
          title="Files (⌘P)"
          aria-pressed={filesOpen}
          disabled={!app.project}
          onClick={openFiles}
        >
          <FileCode2 size={15} />
        </button>
        <button
          className={`icon-button${changesOpen ? " active" : ""}`}
          aria-label="Changes"
          title="Changes"
          disabled={!app.project}
          onClick={toggleChanges}
          aria-pressed={changesOpen}
        >
          <GitBranch size={15} />
        </button>
      </nav>
      <div className="sidebar-session-lists">
        <section
          className="sidebar-session-section"
          aria-labelledby={`${activityId}-sessions-heading`}
        >
          <div className="sidebar-section-heading">
            <h2 id={`${activityId}-sessions-heading`}>Sessions</h2>
            <span className="sidebar-section-count">
              {search
                ? `${sessions.length} / ${allSessions.length}`
                : allSessions.length}
            </span>
            <div className="sidebar-list-tools">
              <button
                ref={searchButton}
                type="button"
                className="icon-button"
                aria-label="Search sessions"
                title="Search sessions (⌘K)"
                aria-expanded={searchOpen}
                aria-controls={`${activityId}-search`}
                disabled={!app.project}
                onClick={focusSearch}
              >
                <Search size={13} />
              </button>
              <button
                className="icon-button"
                title="Refresh sessions"
                aria-label="Refresh sessions"
                disabled={
                  !app.project || app.providerWorkspace?.isLoadingSessions
                }
                onClick={() => void app.refreshSessions()}
              >
                <RefreshCw
                  size={13}
                  className={
                    app.providerWorkspace?.isLoadingSessions ? "spinning" : ""
                  }
                />
              </button>
            </div>
          </div>
          <label
            id={`${activityId}-search`}
            className="session-search"
            hidden={!searchOpen}
          >
            <Search size={13} />
            <input
              ref={searchRef}
              placeholder="Search sessions…"
              aria-label="Search sessions"
              value={search}
              onChange={(event) => view.update({ search: event.target.value })}
              onKeyDown={(event) => {
                if (event.key === "Escape") {
                  event.preventDefault();
                  closeSearch();
                }
              }}
            />
            <button
              type="button"
              className="icon-button"
              aria-label="Close session search"
              onClick={closeSearch}
            >
              <X size={12} />
            </button>
          </label>
          <div
            ref={sessionList}
            id={`${activityId}-sessions`}
            className="session-list"
            tabIndex={0}
            onScroll={(event) => {
              if (!restoration.current?.lists.sessions.pending)
                view.positions.current.set(
                  `${path}\0sessions`,
                  event.currentTarget.scrollTop,
                );
            }}
          >
            {app.providerWorkspace?.isLoadingSessions &&
              sessions.length === 0 && (
                <div className="empty-sessions">Connecting…</div>
              )}
            {sessions.length === 0 &&
              !app.providerWorkspace?.isLoadingSessions && (
                <div className="empty-sessions">
                  {search ? "No matching sessions" : "No sessions yet"}
                </div>
              )}
            {sessions.map((item) => {
              const workspace = app.workspace?.providers[item.provider];
              const state = workspace?.sessionsById[item.id];
              const archiving = workspace?.archivingSessionIds.includes(
                item.id,
              );
              const working = state?.isWorking ?? item.isWorking;
              const approval = app.approvals.some(
                (request) =>
                  request.projectPath === app.project?.path &&
                  request.provider === item.provider &&
                  request.sessionId === item.id,
              );
              return (
                <div
                  key={`${app.project?.path}-${item.provider}-${item.id}`}
                  className={`session-row${!activeUrl && app.provider === item.provider && app.selectedId === item.id ? " selected" : ""}`}
                  onContextMenu={(event) =>
                    contextMenu(event, [
                      {
                        text: "Open session",
                        enabled: !archiving,
                        action: () => selectSession(item, item.provider),
                      },
                      copyAction("Copy session title", sessionTitle(item)),
                      copyAction("Copy session ID", item.id),
                      { item: "Separator" },
                      {
                        text: "Archive session",
                        enabled: !(
                          archiving ||
                          working ||
                          state?.isSubmitting ||
                          state?.connection === "connecting" ||
                          approval
                        ),
                        action: () => app.archiveSession(item, item.provider),
                      },
                    ])
                  }
                >
                  <div className="session-item">
                    <button
                      className="sidebar-row-select"
                      title={`${sessionTitle(item)} · ${item.provider === "codex" ? "Codex" : "OpenCode"}`}
                      aria-label={sessionTitle(item)}
                      aria-current={
                        !activeUrl &&
                        app.provider === item.provider &&
                        app.selectedId === item.id
                          ? "page"
                          : undefined
                      }
                      disabled={archiving}
                      onClick={() => selectSession(item, item.provider)}
                    >
                      <span className="session-heading">
                        <span
                          className="session-status"
                          title={approval ? "Approval required" : undefined}
                        >
                          {approval ? (
                            <ShieldAlert
                              size={13}
                              className="approval-indicator"
                              aria-label="Approval required"
                            />
                          ) : working ? (
                            <span
                              className="running-dot"
                              aria-label="Running"
                              title="Running"
                            />
                          ) : state?.unread ? (
                            <span
                              className="unread-dot"
                              aria-label="Unread"
                              title="Unread"
                            />
                          ) : state?.connection === "disconnected" ? (
                            <span
                              className="disconnected-dot"
                              aria-label="Disconnected"
                              title="Disconnected"
                            />
                          ) : null}
                        </span>
                        <span className="session-title">
                          {sessionTitle(item)}
                        </span>
                      </span>
                    </button>
                    <SessionContext
                      projectPath={app.project!.path}
                      details={workspace?.sessionDetails[item.id]}
                      revision={item.updatedAt}
                      load={() => {
                        if (app.project)
                          void app.loadSessionDetails(
                            app.project.path,
                            item.provider,
                            item.id,
                          );
                      }}
                    />
                  </div>
                  <button
                    className="icon-button session-archive"
                    aria-label="Archive session"
                    title={
                      working || state?.isSubmitting || approval
                        ? "Stop the running turn before archiving"
                        : `Archive ${sessionTitle(item)}`
                    }
                    disabled={
                      archiving ||
                      working ||
                      state?.isSubmitting ||
                      state?.connection === "connecting" ||
                      approval
                    }
                    onClick={() => void app.archiveSession(item, item.provider)}
                  >
                    {archiving ? (
                      <LoaderCircle size={13} className="spinning" />
                    ) : (
                      <Archive size={13} />
                    )}
                  </button>
                </div>
              );
            })}
          </div>
        </section>
        <section
          className="sidebar-request-section"
          aria-labelledby={`${activityId}-requests-heading`}
        >
          <div className="sidebar-section-heading">
            <h2 id={`${activityId}-requests-heading`}>Pull requests</h2>
            <span className="sidebar-section-count">
              {pullRequests.entries.size}
            </span>
          </div>
          <div
            ref={requestList}
            id={`${activityId}-requests`}
            className="sidebar-request-panel"
            tabIndex={0}
            onScroll={(event) => {
              if (!restoration.current?.lists.requests.pending)
                view.positions.current.set(
                  `${path}\0requests`,
                  event.currentTarget.scrollTop,
                );
            }}
          >
            <SessionPullRequests result={pullRequests} />
          </div>
        </section>
      </div>
      <button
        className="nav-item sidebar-settings"
        aria-label="Settings"
        onClick={(event) => {
          event.currentTarget.focus({ preventScroll: true });
          openSettings();
        }}
        title="Settings (⌘,)"
      >
        <Settings size={15} /> Settings <kbd>⌘,</kbd>
      </button>
      <div
        className="sidebar-resize"
        role="separator"
        aria-label="Resize sidebar"
        aria-orientation="vertical"
        aria-controls="workspace-sidebar"
        aria-valuemin={MIN_SIDEBAR_WIDTH}
        aria-valuemax={maximum}
        aria-valuenow={Math.round(width)}
        tabIndex={0}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          drag.current = { x: event.clientX, width };
          event.currentTarget.setPointerCapture(event.pointerId);
          event.currentTarget.focus({ preventScroll: true });
          event.preventDefault();
        }}
        onPointerMove={(event) => {
          if (drag.current)
            resize(drag.current.width + event.clientX - drag.current.x);
        }}
        onPointerUp={() => {
          drag.current = null;
        }}
        onLostPointerCapture={() => {
          drag.current = null;
        }}
        onKeyDown={(event) => {
          if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) {
            event.preventDefault();
            resize(
              event.key === "Home"
                ? MIN_SIDEBAR_WIDTH
                : event.key === "End"
                  ? maximum
                  : width + (event.key === "ArrowRight" ? 32 : -32),
            );
          }
        }}
      />
    </aside>
  );
}
