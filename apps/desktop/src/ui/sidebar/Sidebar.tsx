import {
  Archive,
  FileCode2,
  GitBranch,
  LoaderCircle,
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
  ProjectSession,
  SessionDetails,
} from "../../domain/sessions/workspaceState.js";
import { sessionTitle } from "../../domain/sessions/workspaceState.js";
import type { WorkspaceController } from "../../application/workspace/controller.js";
import { ProjectWorktrees } from "../projects/Worktrees.js";
import { SettingsTrigger } from "../settings/SettingsTrigger.js";
import { NewSessionButton } from "../sessions/NewSessionButton.js";
import { PullRequestIndicator } from "../pull-requests/PullRequestIndicator.js";
import { SessionPullRequests } from "../pull-requests/SessionPullRequests.js";
import { useSessionPullRequests } from "../../application/github/useSessionPullRequests.js";
import type { SidebarView } from "./useSidebarView.js";
import { usePullRequestPreview } from "../pull-requests/PullRequestPreview.js";
import { useContextMenu, copyAction } from "../primitives/ContextMenu.js";
const MIN_SIDEBAR_WIDTH = 180;
const MAX_SIDEBAR_WIDTH = 480;
function SessionContext({
  projectPath,
  checkoutLabel,
  details,
  load,
  revision,
}: {
  projectPath: string;
  checkoutLabel?: string;
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
      {checkoutLabel && (
        <span className="session-checkout" title={projectPath}>
          {checkoutLabel}
        </span>
      )}
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
  setupSession,
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
  setupSession: () => void;
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
  const selectSession = (item: ProjectSession) => {
    showSession();
    app.selectSession(item, item.provider, item.project);
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
  const path = app.rootProject?.path ?? "";
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
  const allSessions = app.projectSessions;
  const sessions = allSessions.filter((item) =>
    `${sessionTitle(item)} ${item.provider} ${item.checkoutLabel} ${item.project.path} ${app.workspaces[item.project.path]?.providers[item.provider].sessionDetails[item.id]?.branch ?? ""}`
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
        name === "sessions" ? app.isLoadingSessions : pullRequests.loading;
      if (Math.abs(list.scrollTop - restore.top) < 1 || !loading)
        restore.pending = false;
    }
  }, [
    path,
    sessions.length,
    pullRequests.entries.size,
    pullRequests.loading,
    app.isLoadingSessions,
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
        <NewSessionButton
          start={newSession}
          setup={setupSession}
          disabled={app.restoring || Boolean(app.providerWorkspace?.isCreating)}
        />
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
                disabled={!app.project || app.isLoadingSessions}
                onClick={() => void app.refreshSessions()}
              >
                <RefreshCw
                  size={13}
                  className={app.isLoadingSessions ? "spinning" : ""}
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
            {app.sessionErrors.length > 0 && (
              <div
                className="session-discovery-error"
                role="status"
                title={app.sessionErrors.join("\n")}
              >
                <span>Could not load all sessions.</span>
                <button
                  type="button"
                  disabled={app.isLoadingSessions}
                  onClick={() => void app.refreshSessions()}
                >
                  Retry
                </button>
              </div>
            )}
            {app.isLoadingSessions && sessions.length === 0 && (
              <div className="empty-sessions">Connecting…</div>
            )}
            {sessions.length === 0 &&
              !app.isLoadingSessions &&
              !app.sessionErrors.length && (
                <div className="empty-sessions">
                  {search ? "No matching sessions" : "No sessions yet"}
                </div>
              )}
            {sessions.map((item) => {
              const workspace =
                app.workspaces[item.project.path]?.providers[item.provider];
              const state = workspace?.sessionsById[item.id];
              const archiving = workspace?.archivingSessionIds.includes(
                item.id,
              );
              const working = state?.isWorking ?? item.isWorking;
              const approval = [...app.approvals, ...app.questions].some(
                (request) =>
                  request.projectPath === item.project.path &&
                  request.provider === item.provider &&
                  request.sessionId === item.id,
              );
              return (
                <div
                  key={item.key}
                  data-project-path={item.project.path}
                  data-provider={item.provider}
                  data-session-id={item.id}
                  className={`session-row${!activeUrl && app.project?.path === item.project.path && app.provider === item.provider && app.selectedId === item.id ? " selected" : ""}`}
                  onContextMenu={(event) =>
                    contextMenu(event, [
                      {
                        text: "Open session",
                        enabled: !archiving && item.available,
                        action: () => selectSession(item),
                      },
                      copyAction("Copy session title", sessionTitle(item)),
                      copyAction("Copy session ID", item.id),
                      { item: "Separator" },
                      {
                        text: "Archive session",
                        enabled: !(
                          !item.available ||
                          archiving ||
                          working ||
                          state?.queuedMessages.length ||
                          state?.isSubmitting ||
                          state?.connection === "connecting" ||
                          approval
                        ),
                        action: () =>
                          app.archiveSession(item, item.provider, item.project),
                      },
                    ])
                  }
                >
                  <div className="session-item">
                    <button
                      className="sidebar-row-select"
                      title={`${sessionTitle(item)} · ${item.provider === "codex" ? "Codex" : "OpenCode"}\n${item.checkoutLabel} · ${item.project.path}${item.available ? "" : " · Unavailable"}`}
                      aria-label={sessionTitle(item)}
                      aria-current={
                        !activeUrl &&
                        app.project?.path === item.project.path &&
                        app.provider === item.provider &&
                        app.selectedId === item.id
                          ? "page"
                          : undefined
                      }
                      disabled={archiving || !item.available}
                      onClick={() => selectSession(item)}
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
                          ) : null}
                          {working && (
                            <span
                              className="running-dot"
                              aria-label="Running"
                              title="Running"
                            />
                          )}
                          {app.activity.get(item.key)?.unread && (
                            <span
                              className="unread-dot"
                              aria-label="Unread"
                              title="Unread"
                            />
                          )}
                          {!working && state?.connection === "disconnected" ? (
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
                      projectPath={item.project.path}
                      checkoutLabel={
                        app.projectCheckouts.length > 1
                          ? `${item.checkoutLabel}${item.available ? "" : " · Missing"}`
                          : undefined
                      }
                      details={workspace?.sessionDetails[item.id]}
                      revision={item.updatedAt}
                      load={() => {
                        if (item.available)
                          void app.loadSessionDetails(
                            item.project.path,
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
                      !item.available ||
                      archiving ||
                      working ||
                      Boolean(state?.queuedMessages.length) ||
                      state?.isSubmitting ||
                      state?.connection === "connecting" ||
                      approval
                    }
                    onClick={() =>
                      void app.archiveSession(item, item.provider, item.project)
                    }
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
      <SettingsTrigger
        className="nav-item sidebar-settings"
        open={openSettings}
      >
        <Settings size={15} /> Settings <kbd>⌘,</kbd>
      </SettingsTrigger>
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
