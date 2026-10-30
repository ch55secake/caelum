import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import {
  FolderOpen,
  LoaderCircle,
  PanelLeftOpen,
  PanelRight,
  Search,
  ShieldAlert,
  Settings as SettingsIcon,
  X,
} from "lucide-react";
import { FilesPanel } from "./FilesPanel.js";
import { GitReviewPanel } from "./GitReviewPanel.js";
import { useWorkspace } from "./useWorkspace.js";
import { Sidebar, sessionTitle } from "./Sidebar.js";
import { ApprovalCard, Conversation } from "./Conversation.js";
import { Composer } from "./Composer.js";
import { projectHasActiveTurn } from "./workspaceState.js";
import { lastUserMessage } from "./shellMessages.js";
import { Settings } from "./Settings.js";
import { useSettings } from "./useSettings.js";

export default function App() {
  const preferences = useSettings();
  const app = useWorkspace(preferences.settings);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [changesOpen, setChangesOpen] = useState(true);
  const [search, setSearch] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const sessionKey = `${app.project?.path}\0${app.provider}\0${app.selectedId ?? "new"}`;
  const title =
    app.view === "files"
      ? "Files"
      : app.selected
        ? sessionTitle(
            app.providerWorkspace?.sessions.find(
              (item) => item.id === app.selectedId,
            ) ?? app.selected.session.thread,
          )
        : "New session";
  function toggleChanges() {
    if (app.view !== "session") {
      app.setView("session");
      setChangesOpen(true);
    } else setChangesOpen((value) => !value);
  }
  function newSession() {
    app.newSession();
    setSearch("");
    requestAnimationFrame(() => textareaRef.current?.focus());
  }
  function focusSearch() {
    setCollapsed(false);
    requestAnimationFrame(() => {
      searchRef.current?.focus();
      searchRef.current?.select();
    });
  }
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;
      if (event.key === ",") {
        event.preventDefault();
        setSettingsOpen(true);
        return;
      }
      if (settingsOpen) return;
      if (event.key.toLowerCase() === "n") {
        event.preventDefault();
        newSession();
      }
      if (event.key.toLowerCase() === "k") {
        event.preventDefault();
        focusSearch();
      }
      if (event.key === "\\") {
        event.preventDefault();
        setCollapsed((value) => !value);
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });
  useEffect(() => {
    if (app.view === "session") textareaRef.current?.focus();
  }, [sessionKey, app.view]);
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
  const sessionError = app.selected?.error ?? app.providerWorkspace?.error;
  const mac = isTauri() && /Macintosh|Mac OS X/.test(navigator.userAgent);
  return (
    <main
      className={`app-shell${mac ? " macos-titlebar-overlay" : ""}${collapsed ? " sidebar-collapsed" : ""}`}
    >
      {!collapsed && (
        <Sidebar
          app={app}
          collapse={() => setCollapsed(true)}
          search={search}
          setSearch={setSearch}
          searchRef={searchRef}
          newSession={newSession}
          changesOpen={changesOpen && app.view === "session"}
          toggleChanges={toggleChanges}
          openSettings={() => setSettingsOpen(true)}
        />
      )}
      <section className="main-panel">
        <header className="topbar" data-tauri-drag-region>
          <div className="breadcrumbs">
            {collapsed && (
              <button
                className="icon-button"
                aria-label="Expand sidebar"
                onClick={() => setCollapsed(false)}
              >
                <PanelLeftOpen size={16} />
              </button>
            )}
            <span title={app.project?.path}>
              {app.project?.name ?? "Caelum"}
            </span>
            <span className="crumb-divider">/</span>
            <strong title={title}>{title}</strong>
          </div>
          <div className="topbar-actions">
            {collapsed && (
              <button
                className="icon-button"
                aria-label="Settings"
                title="Settings (⌘,)"
                onClick={(event) => {
                  event.currentTarget.focus({ preventScroll: true });
                  setSettingsOpen(true);
                }}
              >
                <SettingsIcon size={16} />
              </button>
            )}
            {app.selected && app.view === "session" && (
              <span className="connection-status">
                {app.selected.connection === "connecting"
                  ? "Connecting…"
                  : app.selected.connection === "disconnected"
                    ? "Disconnected"
                    : activeApprovals.length
                      ? "Approval required"
                      : app.selected.isWorking
                        ? "Running"
                        : ""}
              </span>
            )}
            {app.approvals.length > 0 && (
              <button
                className="approval-shortcut"
                aria-label={`${app.approvals.length} pending approvals`}
                onClick={() =>
                  app.viewApproval(
                    app.approvals.find(
                      (request) => !activeApprovals.includes(request),
                    ) ?? app.approvals[0],
                  )
                }
              >
                <ShieldAlert size={14} />
                {app.approvals.length}
              </button>
            )}
            <button
              className="icon-button"
              aria-label="Search sessions"
              title="Search sessions (⌘K)"
              onClick={focusSearch}
            >
              <Search size={15} />
            </button>
            {app.project && (
              <button
                className="icon-button changes-toggle"
                aria-label={
                  changesOpen && app.view === "session"
                    ? "Hide changes"
                    : "Show changes"
                }
                aria-expanded={changesOpen && app.view === "session"}
                aria-controls="changes-pane"
                title="Toggle changes pane"
                onClick={toggleChanges}
              >
                <PanelRight size={16} />
              </button>
            )}
          </div>
        </header>
        {app.error && (
          <div className="error-banner" role="alert">
            <span>{app.error}</span>
            <button
              className="icon-button"
              aria-label="Dismiss error"
              onClick={() => app.setError(null)}
            >
              <X size={13} />
            </button>
          </div>
        )}
        {app.archivedSession && (
          <div
            className="archive-notice"
            role="status"
            aria-label="Session archived"
          >
            <span title={sessionTitle(app.archivedSession.thread)}>
              Archived{" "}
              <strong>{sessionTitle(app.archivedSession.thread)}</strong>
            </span>
            <button
              onClick={() => void app.undoArchive()}
              disabled={app.undoingArchive}
            >
              {app.undoingArchive ? "Restoring…" : "Undo"}
            </button>
            <button
              className="icon-button"
              aria-label="Dismiss archived session notice"
              onClick={app.dismissArchive}
            >
              <X size={13} />
            </button>
          </div>
        )}
        {app.view === "files" ? (
          <div className="panel-content">
            <FilesPanel
              project={app.project}
              theme={preferences.resolvedTheme}
              accentColor={preferences.settings.accentColor}
            />
          </div>
        ) : (
          <div
            className={`session-layout${changesOpen && app.project ? " with-changes" : ""}`}
          >
            <div className="session-workspace">
              {sessionError && (
                <div className="error-banner" role="alert">
                  <span>{sessionError}</span>
                  {!app.selected && app.project && (
                    <button
                      onClick={() =>
                        void app.loadSessions(app.project!, app.provider)
                      }
                    >
                      Retry connection
                    </button>
                  )}
                </div>
              )}
              {app.selected?.connection === "disconnected" && (
                <div className="connection-banner">
                  <span>
                    Connection lost. Reconnect to continue this session.
                  </span>
                  <button
                    onClick={() => {
                      if (app.project && app.selected)
                        void app.connectSession(
                          app.project,
                          app.provider,
                          app.selected.session.thread,
                        );
                    }}
                  >
                    Reconnect
                  </button>
                </div>
              )}
              {projectApprovals.map((request) => (
                <ApprovalCard
                  key={request.requestId}
                  request={request}
                  resolving={Boolean(app.resolving)}
                  resolve={(decision) =>
                    void app.resolveApproval(request, decision)
                  }
                />
              ))}
              {!app.project ? (
                <div className="session-empty">
                  {app.restoring ? (
                    <>
                      <LoaderCircle size={22} className="spinning" />
                      <span>Restoring workspace…</span>
                    </>
                  ) : (
                    <button
                      className="open-project-button"
                      onClick={() => void app.chooseProject()}
                      disabled={app.selecting}
                    >
                      <FolderOpen size={18} />
                      Open project
                    </button>
                  )}
                </div>
              ) : app.selected ? (
                <Conversation
                  key={sessionKey}
                  session={app.selected}
                  saveView={(value) =>
                    app.mutateSession(
                      app.project!.path,
                      app.provider,
                      app.selectedId!,
                      (current) => ({ ...current, ...value }),
                    )
                  }
                  loadOlder={() =>
                    app.loadHistory(
                      app.project!.path,
                      app.provider,
                      app.selectedId!,
                      true,
                    )
                  }
                  retryHistory={() =>
                    void app.loadHistory(
                      app.project!.path,
                      app.provider,
                      app.selectedId!,
                      Boolean(
                        app.selected?.historyLoaded &&
                        app.selected.nextCursor,
                      ),
                    )
                  }
                  approvals={activeApprovals}
                  resolving={Boolean(app.resolving)}
                  resolve={(request, decision) =>
                    void app.resolveApproval(request, decision)
                  }
                />
              ) : (
                <div className="session-empty">
                  <h1>New session</h1>
                  <p>What would you like to work on?</p>
                </div>
              )}
              {app.project && (
                <Composer
                  key={`composer-${sessionKey}`}
                  textareaRef={textareaRef}
                  draft={
                    app.selected?.draft ?? app.providerWorkspace?.draft ?? ""
                  }
                  setDraft={app.setDraft}
                  lastMessage={lastUserMessage(
                    app.selected?.conversation ?? [],
                  )}
                  provider={app.provider}
                  changeProvider={app.changeProvider}
                  isNew={!app.selectedId}
                  working={app.selected?.isWorking ?? false}
                  activity={app.selected?.activity?.label}
                  awaitingApproval={activeApprovals.length > 0}
                  submitting={
                    app.selected?.isSubmitting ||
                    (!app.selectedId && app.providerWorkspace?.isCreating) ||
                    false
                  }
                  disabled={
                    Boolean(
                      app.selectedId &&
                      (!app.selected ||
                        app.providerWorkspace?.archivingSessionIds.includes(
                          app.selectedId,
                        ) ||
                        app.selected.connection !== "connected"),
                    ) ||
                    Boolean(
                      !app.selectedId && app.providerWorkspace?.isCreating,
                    )
                  }
                  submit={() => void app.submit()}
                  cancel={() => void app.cancel()}
                  choices={app.providerWorkspace?.choices ?? null}
                  choicesError={app.providerWorkspace?.choicesError ?? null}
                  model={app.options.model}
                  modelPending={Boolean(app.selected?.model)}
                  effort={app.options.effort}
                  modelChoice={app.options.modelChoice}
                  favouriteModels={
                    preferences.settings.favoriteModels[app.provider]
                  }
                  toggleFavouriteModel={(id) =>
                    preferences.toggleModelFavourite(app.provider, id)
                  }
                  effortPending={app.selected?.effort != null}
                  agent={app.options.agent}
                  setOption={app.setOption}
                  retryChoices={() =>
                    void app.loadChoices(app.project!, app.provider)
                  }
                />
              )}
            </div>
            {changesOpen && app.project && (
              <aside
                id="changes-pane"
                className="changes-pane"
                aria-label="Project changes"
              >
                <GitReviewPanel
                  key={app.project.path}
                  project={app.project}
                  working={Boolean(
                    app.workspace && projectHasActiveTurn(app.workspace),
                  )}
                  close={() => setChangesOpen(false)}
                />
              </aside>
            )}
          </div>
        )}
      </section>
      {settingsOpen && (
        <Settings
          app={app}
          preferences={preferences}
          close={() => setSettingsOpen(false)}
        />
      )}
    </main>
  );
}
