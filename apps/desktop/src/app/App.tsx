import { isTauri } from "@tauri-apps/api/core";
import { useWorkspace } from "../application/workspace/useWorkspace.js";
import { useSessionPullRequests } from "../application/github/useSessionPullRequests.js";
import { useSettings } from "../application/useSettings.js";
import { questionKey } from "../domain/sessions/questionState.js";
import { sessionTitle } from "../domain/sessions/workspaceState.js";
import { useSidebarView } from "../ui/sidebar/useSidebarView.js";
import { Sidebar } from "../ui/sidebar/Sidebar.js";
import { Settings } from "../ui/settings/Settings.js";
import { useEscapeCancellation } from "../ui/composer/useEscapeCancellation.js";
import { FileNavigation } from "../ui/files/FileNavigation.js";
import { ContextMenuProvider } from "../ui/primitives/ContextMenu.js";
import { PullRequestPreviewProvider } from "../ui/pull-requests/PullRequestPreview.js";
import { useWorkspaceSurfaces } from "./useWorkspaceSurfaces.js";
import { useWorkspaceFocus } from "./useWorkspaceFocus.js";
import { useWorkspaceShortcuts } from "./useWorkspaceShortcuts.js";
import { WorkspaceHeader } from "./WorkspaceHeader.js";
import { WorkspaceNotices } from "./WorkspaceNotices.js";
import { WorkspaceInspector } from "./WorkspaceInspector.js";
import { WorkspacePullRequest } from "./WorkspacePullRequest.js";
import { SessionWorkspace } from "./SessionWorkspace.js";

export default function App() {
  const preferences = useSettings();
  const app = useWorkspace(preferences.settings);
  const sessionKey = `${app.project?.path}\0${app.provider}\0${app.selectedId ?? "new"}`;
  const surfaces = useWorkspaceSurfaces(app.project?.path, sessionKey);
  const { activePullRequest, settingsOpen, collapsed, pane, expanded } =
    surfaces;
  const sidebarView = useSidebarView(app.rootProject?.path ?? "");
  const sessionPullRequests = useSessionPullRequests(
    app,
    app.projectSessions
      .filter(
        (session) =>
          session.project.path === app.project?.path &&
          session.provider === app.provider &&
          session.id === app.selectedId,
      )
      .map((session) => ({ ...session, title: sessionTitle(session) })),
  );
  const activeApprovals = app.approvals.filter(
    (request) =>
      request.projectPath === app.project?.path &&
      request.provider === app.provider &&
      request.sessionId === app.selectedId,
  );
  const projectApprovals = app.approvals.filter(
    (request) =>
      request.projectPath === app.project?.path && !request.sessionId,
  );
  const activeQuestions = app.questions.filter(
    (request) =>
      request.projectPath === app.project?.path &&
      request.provider === app.provider &&
      request.sessionId === app.selectedId,
  );
  const focus = useWorkspaceFocus(
    sessionKey,
    activeQuestions[0] ? questionKey(activeQuestions[0]) : null,
    Boolean(activePullRequest),
  );
  function newSession() {
    surfaces.dismissPullRequest();
    app.newSession();
    sidebarView.update({ search: "", searchOpen: false });
    focus.request("composer");
  }
  function showSession() {
    surfaces.showSession();
    // Leaving a PR already restores focus through the session-focus effect.
    if (!activePullRequest) focus.request("composer");
  }
  function focusSearch() {
    surfaces.setCollapsed(false);
    sidebarView.update({ searchOpen: true });
    focus.request("search");
  }
  useWorkspaceShortcuts({
    hasProject: Boolean(app.project),
    surfaces,
    newSession,
    focusSearch,
  });
  useEscapeCancellation({
    scope: sessionKey,
    enabled:
      Boolean(app.selected?.isWorking) &&
      !settingsOpen &&
      !activePullRequest &&
      !(expanded && pane),
    cancel: app.cancel,
  });
  const title = app.selected
    ? sessionTitle(
        app.providerWorkspace?.sessions.find(
          (item) => item.id === app.selectedId,
        ) ?? app.selected.session.thread,
      )
    : "New session";
  const mac = isTauri() && /Macintosh|Mac OS X/.test(navigator.userAgent);
  return (
    <FileNavigation.Provider value={surfaces.fileNavigation}>
      <ContextMenuProvider
        className={`app-shell${mac ? " macos-titlebar-overlay" : ""}${collapsed ? " sidebar-collapsed" : ""}`}
        scope={`${sessionKey}\0${pane}\0${expanded}\0${settingsOpen}\0${activePullRequest?.pr.url}`}
        reportError={app.setError}
        actions={[
          {
            text: "New session",
            enabled: Boolean(app.project),
            action: newSession,
          },
          {
            text: "Open project…",
            enabled: !app.selecting && !app.restoring,
            action: app.chooseProject,
          },
          { item: "Separator" },
          { text: "Settings…", action: () => surfaces.setSettingsOpen(true) },
        ]}
      >
        {!collapsed && (
          <PullRequestPreviewProvider
            scope={`${sessionKey}\0${settingsOpen}\0${pane}\0${expanded}\0${activePullRequest?.pr.url}`}
            activeUrl={activePullRequest?.pr.url ?? null}
            onOpen={surfaces.openPullRequest}
          >
            <Sidebar
              app={app}
              preferredWidth={surfaces.sidebarWidth}
              setPreferredWidth={surfaces.setSidebarWidth}
              collapse={() => surfaces.setCollapsed(true)}
              view={sidebarView}
              searchRef={focus.searchRef}
              focusSearch={focusSearch}
              newSession={newSession}
              changesOpen={surfaces.changesOpen}
              filesOpen={!activePullRequest && pane === "files"}
              sessionVisible={!activePullRequest && (!expanded || !pane)}
              openFiles={() => surfaces.openFiles()}
              showSession={showSession}
              toggleChanges={surfaces.toggleChanges}
              openSettings={() => surfaces.setSettingsOpen(true)}
            />
          </PullRequestPreviewProvider>
        )}
        <section className="main-panel">
          <WorkspaceHeader
            app={app}
            title={title}
            surfaces={surfaces}
            activeApprovals={activeApprovals}
            activeQuestions={activeQuestions}
            showSession={showSession}
            focusSearch={focusSearch}
          />
          <WorkspaceNotices app={app} />
          <WorkspacePullRequest
            app={app}
            title={title}
            surfaces={surfaces}
            pendingInput={Boolean(
              activeApprovals.length || activeQuestions.length,
            )}
            showSession={showSession}
          />
          <div
            className={`session-layout${pane && app.project ? " with-changes" : ""}`}
            hidden={Boolean(activePullRequest)}
          >
            <SessionWorkspace
              app={app}
              preferences={preferences}
              pullRequests={sessionPullRequests}
              sessionKey={sessionKey}
              surfaces={surfaces}
              focus={focus}
              activeApprovals={activeApprovals}
              projectApprovals={projectApprovals}
              activeQuestions={activeQuestions}
              newSession={newSession}
            />
            <WorkspaceInspector app={app} surfaces={surfaces} focus={focus} />
          </div>
        </section>
        {settingsOpen && (
          <Settings
            app={app}
            preferences={preferences}
            close={() => surfaces.setSettingsOpen(false)}
          />
        )}
      </ContextMenuProvider>
    </FileNavigation.Provider>
  );
}
