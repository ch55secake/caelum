import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { isTauri } from "@tauri-apps/api/core";
import {
  FolderOpen,
  LoaderCircle,
  PanelLeftOpen,
  PanelRight,
  Search,
  ShieldAlert,
  MessageCircleQuestion,
  Settings as SettingsIcon,
  X,
} from "lucide-react";
import { ProjectInspector, type InspectorRequest } from "./ProjectInspector.js";
import { FileNavigation } from "./FileNavigation.js";
import type { FileLocation } from "./fileContext.js";
import { useWorkspace } from "./useWorkspace.js";
import { useSidebarView } from "./useSidebarView.js";
import { Sidebar } from "./Sidebar.js";
import { ApprovalCard, Conversation } from "./Conversation.js";
import { Composer } from "./Composer.js";
import { QuestionCard } from "./QuestionCard.js";
import { SessionWorktree } from "./Worktrees.js";
import { questionKey } from "./questionState.js";
import { projectHasActiveTurn, sessionTitle } from "./workspaceState.js";
import { lastUserMessage } from "./shellMessages.js";
import { Settings } from "./Settings.js";
import { useSettings } from "./useSettings.js";
import { ContextMenuProvider } from "./ContextMenu.js";
import {
  PullRequestPreviewProvider,
  PullRequestScreen,
  type PullRequestTarget,
} from "./PullRequestPreview.js";

export default function App() {
  const preferences = useSettings();
  const app = useWorkspace(preferences.settings);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState<number | null>(() => {
    try {
      const width = Number(localStorage.getItem("caelum.sidebar.width"));
      return Number.isFinite(width) && width > 0 ? width : null;
    } catch {
      return null;
    }
  });
  useEffect(() => {
    if (sidebarWidth === null) return;
    try {
      localStorage.setItem("caelum.sidebar.width", String(sidebarWidth));
    } catch {
      /* Keep the preferred width for this window, including after collapse. */
    }
  }, [sidebarWidth]);
  const [pane, setPane] = useState<"files" | "changes" | null>("changes");
  const [expanded, setExpanded] = useState(false);
  const [fileRequest, setFileRequest] = useState<InspectorRequest>();
  const fileRequestId = useRef(0);
  const [pullRequest, setPullRequest] = useState<
    (PullRequestTarget & { scope: string }) | null
  >(null);
  const sidebarView = useSidebarView(app.project?.path ?? "");
  const searchRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [focusRequest, setFocusRequest] = useState<{
    target: "search" | "composer";
  } | null>(null);
  useLayoutEffect(() => {
    if (!focusRequest) return;
    const target =
      focusRequest.target === "search"
        ? searchRef.current
        : textareaRef.current;
    target?.focus();
    if (focusRequest.target === "search") target?.select();
  }, [focusRequest]);
  const questionDockRef = useRef<HTMLDivElement>(null);
  const sessionKey = `${app.project?.path}\0${app.provider}\0${app.selectedId ?? "new"}`;
  const activePullRequest =
    pullRequest?.scope === sessionKey ? pullRequest : null;
  const currentPullRequest = useRef(activePullRequest);
  currentPullRequest.current = activePullRequest;
  const changesOpen = !activePullRequest && pane === "changes";
  const currentSessionKey = useRef(sessionKey);
  currentSessionKey.current = sessionKey;
  const title = app.selected
    ? sessionTitle(
        app.providerWorkspace?.sessions.find(
          (item) => item.id === app.selectedId,
        ) ?? app.selected.session.thread,
      )
    : "New session";
  function toggleChanges() {
    setPullRequest(null);
    setPane((value) =>
      !activePullRequest && value === "changes" ? null : "changes",
    );
    setFileRequest(undefined);
    setExpanded(false);
  }
  const projectPath = app.project?.path;
  const openFile = useCallback(
    (location: FileLocation) => {
      if (!projectPath) return;
      setPullRequest(null);
      setPane("files");
      setFileRequest({
        project: projectPath,
        id: ++fileRequestId.current,
        location,
      });
    },
    [projectPath],
  );
  const fileNavigation = useMemo(
    () => (projectPath ? { projectPath, open: openFile } : null),
    [projectPath, openFile],
  );
  function newSession() {
    setPullRequest(null);
    app.newSession();
    sidebarView.update({ search: "", searchOpen: false });
    setFocusRequest({ target: "composer" });
  }
  function showSession() {
    setPullRequest(null);
    setExpanded(false);
    // Leaving a PR already restores focus through the session effect below.
    if (!activePullRequest) setFocusRequest({ target: "composer" });
  }
  function focusSearch() {
    setCollapsed(false);
    sidebarView.update({ searchOpen: true });
    setFocusRequest({ target: "search" });
  }
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;
      if (document.querySelector("dialog[open]")) return;
      if (event.key === ",") {
        event.preventDefault();
        setSettingsOpen(true);
        return;
      }
      if (settingsOpen) return;
      if (
        app.project &&
        (event.key.toLowerCase() === "p" ||
          (event.shiftKey && event.key.toLowerCase() === "f"))
      ) {
        event.preventDefault();
        const id = ++fileRequestId.current;
        setPullRequest(null);
        setPane("files");
        setFileRequest({
          project: app.project.path,
          id,
          search: {
            id,
            mode: event.key.toLowerCase() === "p" ? "files" : "content",
          },
        });
      }
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
    setPullRequest(null);
    setExpanded(false);
    setFileRequest(undefined);
  }, [sessionKey]);
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
  const activeQuestionKey = activeQuestions[0]
    ? questionKey(activeQuestions[0])
    : null;
  useEffect(() => {
    if (activePullRequest) return;
    const previousFocus = document.activeElement;
    const frame = requestAnimationFrame(() => {
      if (document.querySelector("dialog[open]")) return;
      // Respect a newer keyboard or pointer selection made before this frame.
      if (
        document.activeElement !== previousFocus &&
        document.activeElement !== document.body
      )
        return;
      const target = activeQuestionKey
        ? questionDockRef.current?.querySelector<HTMLElement>(".question-card")
        : textareaRef.current;
      target?.focus({ preventScroll: true });
    });
    return () => cancelAnimationFrame(frame);
  }, [sessionKey, activeQuestionKey, Boolean(activePullRequest)]);
  const sessionError = app.selected?.error ?? app.providerWorkspace?.error;
  const mac = isTauri() && /Macintosh|Mac OS X/.test(navigator.userAgent);
  return (
    <FileNavigation.Provider value={fileNavigation}>
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
          { text: "Settings…", action: () => setSettingsOpen(true) },
        ]}
      >
        {!collapsed && (
          <PullRequestPreviewProvider
            scope={`${sessionKey}\0${settingsOpen}\0${pane}\0${expanded}\0${activePullRequest?.pr.url}`}
            activeUrl={activePullRequest?.pr.url ?? null}
            onOpen={(target) =>
              setPullRequest({ ...target, scope: sessionKey })
            }
          >
            <Sidebar
              app={app}
              preferredWidth={sidebarWidth}
              setPreferredWidth={setSidebarWidth}
              collapse={() => setCollapsed(true)}
              view={sidebarView}
              searchRef={searchRef}
              focusSearch={focusSearch}
              newSession={newSession}
              changesOpen={changesOpen}
              filesOpen={!activePullRequest && pane === "files"}
              sessionVisible={!activePullRequest && (!expanded || !pane)}
              openFiles={() => {
                setPullRequest(null);
                setPane("files");
                setExpanded(false);
                setFileRequest(undefined);
              }}
              showSession={showSession}
              toggleChanges={toggleChanges}
              openSettings={() => setSettingsOpen(true)}
            />
          </PullRequestPreviewProvider>
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
              <span title={activePullRequest ? undefined : app.project?.path}>
                {activePullRequest
                  ? "Pull requests"
                  : (app.rootProject?.name ?? "Caelum")}
              </span>
              <span className="crumb-divider">/</span>
              {!activePullRequest &&
                app.currentWorktree &&
                !app.currentWorktree.main && (
                  <>
                    <span title={app.project?.path}>
                      {app.currentWorktree.branch ?? "Detached HEAD"}
                    </span>
                    <span className="crumb-divider">/</span>
                  </>
                )}
              <strong title={activePullRequest?.pr.title ?? title}>
                {activePullRequest ? `#${activePullRequest.pr.number}` : title}
              </strong>
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
              {app.selected && (
                <span className="connection-status">
                  {app.selected.connection === "connecting" ? (
                    "Connecting…"
                  ) : app.selected.connection === "disconnected" ? (
                    "Disconnected"
                  ) : activeApprovals.length ? (
                    "Approval required"
                  ) : activeQuestions.length ? (
                    "Waiting for your answer"
                  ) : app.selected.isWorking ? (
                    <LoaderCircle
                      size={14}
                      className="spinning"
                      role="img"
                      aria-label="Agent running"
                    />
                  ) : (
                    ""
                  )}
                </span>
              )}
              {app.approvals.length > 0 && (
                <button
                  className="approval-shortcut"
                  aria-label={`${app.approvals.length} pending approvals`}
                  onClick={() => {
                    showSession();
                    app.viewApproval(
                      app.approvals.find(
                        (request) => !activeApprovals.includes(request),
                      ) ?? app.approvals[0],
                    );
                  }}
                >
                  <ShieldAlert size={14} />
                  {app.approvals.length}
                </button>
              )}
              {app.questions.length > 0 && (
                <button
                  className="question-shortcut"
                  aria-label={`${app.questions.length} pending questions`}
                  onClick={() => {
                    setPullRequest(null);
                    setExpanded(false);
                    app.viewQuestion(
                      app.questions.find(
                        (request) => !activeQuestions.includes(request),
                      ) ?? app.questions[0],
                    );
                    requestAnimationFrame(() =>
                      document
                        .querySelector<HTMLElement>(
                          ".question-card input, .question-card textarea, .question-card button",
                        )
                        ?.focus(),
                    );
                  }}
                >
                  <MessageCircleQuestion size={14} />
                  {app.questions.length}
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
                  aria-label={changesOpen ? "Hide changes" : "Show changes"}
                  aria-expanded={changesOpen}
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
          {app.checkoutUnavailable && (
            <div className="connection-banner" role="status">
              This worktree is unavailable. Choose another checkout from the
              sidebar.
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
          {activePullRequest && (
            <PullRequestScreen
              key={activePullRequest.pr.url}
              target={activePullRequest}
              back={showSession}
              session={
                app.selected && app.selectedId
                  ? {
                      name: title,
                      disabledReason:
                        app.checkoutUnavailable ||
                        Boolean(
                          app.rootProject &&
                            app.worktreeBusy[app.rootProject.path],
                        )
                          ? "This checkout is unavailable or busy."
                          : app.selected.connection !== "connected"
                            ? "Reconnect this session to address feedback."
                            : app.selected.isWorking ||
                                app.selected.isSubmitting
                              ? "Wait for the current turn to finish."
                              : activeQuestions.length || activeApprovals.length
                                ? "Answer the session's pending request first."
                                : app.providerWorkspace?.archivingSessionIds.includes(
                                      app.selectedId,
                                    )
                                  ? "This session is being archived."
                                  : null,
                      address: async (prompt) => {
                        const sent = await app.submit(prompt);
                        if (
                          sent &&
                          currentSessionKey.current === sessionKey &&
                          currentPullRequest.current?.pr.url ===
                            activePullRequest.pr.url
                        )
                          showSession();
                        return sent;
                      },
                    }
                  : undefined
              }
            />
          )}
          <div
            className={`session-layout${pane && app.project ? " with-changes" : ""}`}
            hidden={Boolean(activePullRequest)}
          >
            <div
              className="session-workspace"
              hidden={expanded && Boolean(pane)}
            >
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
                        app.selected?.historyLoaded && app.selected.nextCursor,
                      ),
                    )
                  }
                  approvals={activeApprovals}
                  resolving={Boolean(app.resolving)}
                  addToDraft={(text) => {
                    const draft = app.selected?.draft ?? "";
                    app.setDraft(draft ? `${draft}\n\n${text}` : text);
                    textareaRef.current?.focus();
                  }}
                  resolve={(request, decision) =>
                    void app.resolveApproval(request, decision)
                  }
                />
              ) : (
                <div className="session-empty">
                  <h1>New session</h1>
                  <p>What would you like to work on?</p>
                  <SessionWorktree app={app} />
                </div>
              )}
              {activeQuestions.length > 0 ? (
                <div
                  ref={questionDockRef}
                  className="composer-wrap question-dock"
                  aria-label="Agent questions"
                >
                  <span
                    className="sr-only"
                    role="status"
                    aria-label="Agent activity"
                  >
                    Waiting for your answer
                  </span>
                  {activeQuestions.map((request) => {
                    const key = questionKey(request);
                    return (
                      <QuestionCard
                        key={key}
                        request={request}
                        draft={app.questionDraft(request)}
                        setDraft={(draft) =>
                          app.setQuestionDraft(request, draft)
                        }
                        submitting={Boolean(app.questionEdits[key]?.submitting)}
                        error={app.questionEdits[key]?.error}
                        resolve={(answer) =>
                          void app.resolveQuestion(request, answer)
                        }
                      />
                    );
                  })}
                </div>
              ) : (
                app.project && (
                  <Composer
                    key={`composer-${sessionKey}`}
                    projectPath={app.project.path}
                    textareaRef={textareaRef}
                    draft={
                      app.selected?.draft ?? app.providerWorkspace?.draft ?? ""
                    }
                    setDraft={app.setDraft}
                    fileContext={
                      app.selected?.fileContext ??
                      app.providerWorkspace?.fileContext ??
                      []
                    }
                    removeFileContext={app.removeFileContext}
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
                      app.checkoutUnavailable ||
                      Boolean(
                        app.rootProject &&
                          app.worktreeBusy[app.rootProject.path],
                      ) ||
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
                    newSession={newSession}
                    cancel={() => void app.cancel()}
                    choices={app.providerWorkspace?.choices ?? null}
                    choicesError={app.providerWorkspace?.choicesError ?? null}
                    model={app.options.model}
                    modelPending={Boolean(app.selected?.model)}
                    effort={app.options.effort}
                    modelChoice={app.options.modelChoice}
                    contextUsage={app.selected?.contextUsage ?? null}
                    favouriteModels={
                      preferences.settings.favoriteModels[app.provider]
                    }
                    toggleFavouriteModel={(id) =>
                      preferences.toggleModelFavourite(app.provider, id)
                    }
                    effortPending={app.selected?.effort != null}
                    agent={app.options.agent}
                    currentAgent={app.selected?.session.agent ?? null}
                    setOption={app.setOption}
                    retryChoices={() =>
                      void app.loadChoices(app.project!, app.provider)
                    }
                  />
                )
              )}
            </div>
            {pane && app.project && (
              <ProjectInspector
                key={app.project.path}
                project={app.project}
                mode={pane}
                changeMode={(mode) => {
                  setPane(mode);
                  setFileRequest(undefined);
                }}
                expanded={expanded}
                setExpanded={setExpanded}
                request={fileRequest}
                canAttach={
                  !app.selected?.isSubmitting &&
                  !(!app.selectedId && app.providerWorkspace?.isCreating)
                }
                attach={(file, ask) => {
                  if (currentSessionKey.current !== sessionKey) return;
                  app.addFileContext(file, ask);
                  setExpanded(false);
                  requestAnimationFrame(() => {
                    if (currentSessionKey.current === sessionKey)
                      textareaRef.current?.focus();
                  });
                }}
                working={Boolean(
                  app.workspace && projectHasActiveTurn(app.workspace),
                )}
                close={() => {
                  setPane(null);
                  setExpanded(false);
                }}
              />
            )}
          </div>
        </section>
        {settingsOpen && (
          <Settings
            app={app}
            preferences={preferences}
            close={() => setSettingsOpen(false)}
          />
        )}
      </ContextMenuProvider>
    </FileNavigation.Provider>
  );
}
