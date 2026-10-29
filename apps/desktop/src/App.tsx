import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { FilesPanel } from "./FilesPanel.js";
import { GitReviewPanel } from "./GitReviewPanel.js";
import { addOpenProject, closeOpenProject, selectOpenProject } from "./projectWorkspace.js";
import {
  applyCodexEvent,
  applyOpenCodeEvent,
  createProjectWorkspace,
  projectHasActiveTurn,
  restoreProjectWorkspace,
  updateProjectWorkspace,
  updateProviderWorkspace,
  updateSessionWorkspace,
  type ConversationEntry,
  type ProviderKind,
  type SessionSummary,
  type WorkspaceMap,
  type WorkspaceSessionInfo,
} from "./workspaceState.js";
import {
  Activity,
  ArrowUp,
  CircleHelp,
  Command,
  FileCode2,
  FolderGit2,
  FolderOpen,
  Gauge,
  GitBranch,
  Layers3,
  MessageSquarePlus,
  Orbit,
  PanelLeftClose,
  Plus,
  Settings2,
  Sparkles,
  Square,
  X,
} from "lucide-react";

type AppInfo = { name: string; version: string };
type ProjectInfo = { name: string; path: string };
type ApprovalPrompt = {
  provider: string;
  projectPath: string;
  sessionId?: string | null;
  requestId: string;
  summary: string;
  description: string;
};
type CodexSessionInfo = { project: ProjectInfo; thread: SessionSummary };
type CodexSessionList = { project: ProjectInfo; threads: SessionSummary[] };
type CodexEvent = { projectPath: string; method: string; params: Record<string, unknown>; requestId: unknown };
type OpenCodeSession = { id: string; title: string | null; updatedAt: number | null };
type OpenCodeSessionInfo = { project: ProjectInfo; session: OpenCodeSession };
type OpenCodeSessionList = { project: ProjectInfo; sessions: OpenCodeSession[] };
type OpenCodeHistoryEntry = { id: string; kind: string; text: string };
type OpenCodeEvent = { projectPath: string; eventType: string; data: Record<string, unknown>; location: unknown };
type PersistedProject = {
  project: ProjectInfo;
  provider: ProviderKind;
  codexSessionId: string | null;
  opencodeSessionId: string | null;
  codexSessionIds: string[];
  opencodeSessionIds: string[];
};
type WorkspaceSnapshot = { projects: PersistedProject[]; activeProjectPath: string | null };

export default function App() {
  const hasMacTitlebarOverlay = isTauri() && /Macintosh|Mac OS X/.test(navigator.userAgent);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [openProjects, setOpenProjects] = useState<ProjectInfo[]>([]);
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [workspaces, setWorkspaces] = useState<WorkspaceMap>({});
  const [workspaceView, setWorkspaceView] = useState<"overview" | "files" | "changes">("overview");
  const [isRestoringWorkspace, setIsRestoringWorkspace] = useState(isTauri());
  const [appError, setAppError] = useState<string | null>(null);
  const [persistenceError, setPersistenceError] = useState<string | null>(null);
  const [isSelecting, setIsSelecting] = useState(false);
  const [approvals, setApprovals] = useState<ApprovalPrompt[]>([]);
  const [isResolvingApproval, setIsResolvingApproval] = useState(false);
  const requestIds = useRef(new Map<string, number>());
  const persistenceQueue = useRef<Promise<void>>(Promise.resolve());
  const workspace = project ? workspaces[project.path] : undefined;
  const provider = workspace?.provider ?? "codex";
  const providerWorkspace = workspace?.providers[provider];
  const selectedSessionId = providerWorkspace?.selectedSessionId ?? null;
  const selectedSession = selectedSessionId ? providerWorkspace?.sessionsById[selectedSessionId] : undefined;
  const session = selectedSession?.session ?? null;
  const conversation = selectedSession?.conversation ?? [];
  const sessions = providerWorkspace?.sessions ?? [];
  const prompt = workspace?.prompt ?? "";
  const isLoadingSessions = providerWorkspace?.isLoadingSessions ?? false;
  const isSubmitting = selectedSession?.isSubmitting ?? false;
  const isWorking = selectedSession?.isWorking ?? false;
  const error = selectedSession?.error ?? providerWorkspace?.error ?? appError;

  function mutateWorkspace(projectPath: string, update: Parameters<typeof updateProjectWorkspace>[2]) {
    setWorkspaces((current) => updateProjectWorkspace(current, projectPath, update));
  }

  function mutateProvider(
    projectPath: string,
    selectedProvider: ProviderKind,
    update: Parameters<typeof updateProviderWorkspace>[3],
  ) {
    setWorkspaces((current) => updateProviderWorkspace(current, projectPath, selectedProvider, update));
  }

  function mutateSession(
    projectPath: string,
    selectedProvider: ProviderKind,
    sessionId: string,
    update: Parameters<typeof updateSessionWorkspace>[4],
  ) {
    setWorkspaces((current) => updateSessionWorkspace(current, projectPath, selectedProvider, sessionId, update));
  }

  function persist(command: string, args: Record<string, unknown>): Promise<unknown> {
    if (!isTauri()) return Promise.resolve();
    const request = persistenceQueue.current.then(() => invoke(command, args));
    persistenceQueue.current = request.then(() => undefined, () => undefined);
    void request.then(() => setPersistenceError(null), () => undefined);
    void request.catch((reason: unknown) => {
      setPersistenceError(`Local workspace metadata could not be saved: ${String(reason)}`);
    });
    return request;
  }

  function requestKey(projectPath: string, selectedProvider: ProviderKind) {
    return `${projectPath}\0${selectedProvider}`;
  }

  function beginWorkspaceRequest(projectPath: string, selectedProvider: ProviderKind) {
    const key = requestKey(projectPath, selectedProvider);
    const id = (requestIds.current.get(key) ?? 0) + 1;
    requestIds.current.set(key, id);
    return id;
  }

  function isCurrentWorkspaceRequest(projectPath: string, selectedProvider: ProviderKind, id: number) {
    return requestIds.current.get(requestKey(projectPath, selectedProvider)) === id;
  }

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<AppInfo>("app_info").then(setAppInfo).catch((reason: unknown) => {
      setAppError(String(reason));
    });
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    const workspaceRestore = invoke<WorkspaceSnapshot>("restore_workspace").then(
      (snapshot) => ({ snapshot, error: null as unknown }),
      (reason: unknown) => ({
        snapshot: { projects: [], activeProjectPath: null } satisfies WorkspaceSnapshot,
        error: reason,
      }),
    );
    void Promise.all([
      workspaceRestore,
      invoke<ProjectInfo | null>("startup_project"),
    ]).then(async ([{ snapshot, error: restoreError }, startupProject]) => {
      if (restoreError) {
        setPersistenceError(`Local workspace persistence is unavailable: ${String(restoreError)}`);
        if (!startupProject) {
          setIsRestoringWorkspace(false);
          return;
        }
      }
      let entries = snapshot.projects;
      if (startupProject) {
        if (!entries.some((entry) => entry.project.path === startupProject.path)) {
          entries = [{
            project: startupProject,
            provider: "codex",
            codexSessionId: null,
            opencodeSessionId: null,
            codexSessionIds: [],
            opencodeSessionIds: [],
          }, ...entries];
        }
        if (!restoreError) {
          try {
            await persist("remember_project", { project: startupProject });
            await persist("set_active_project", { projectPath: startupProject.path });
          } catch {
            // Keep the requested project open even when workspace persistence is unavailable.
          }
        }
      }
      const restored = await Promise.all(entries.map(async (entry) => {
        try {
          const opened = await invoke<ProjectInfo>("open_project", { path: entry.project.path });
          return { entry, project: opened };
        } catch {
          return null;
        }
      }));
      if (disposed) return;

      const available = restored.filter((entry): entry is NonNullable<typeof entry> => entry !== null);
      const restoredWorkspaces: WorkspaceMap = {};
      for (const { entry, project: restoredProject } of available) {
        restoredWorkspaces[restoredProject.path] = restoreProjectWorkspace(entry);
      }

      const projects = available.map(({ project: restoredProject }) => restoredProject);
      const activePath = startupProject?.path ?? snapshot.activeProjectPath;
      const active = projects.find((item) => item.path === activePath) ?? projects[0] ?? null;
      setWorkspaces(restoredWorkspaces);
      setOpenProjects(projects);
      setProject(active);
      setIsRestoringWorkspace(false);

      if (active?.path !== activePath) {
        persist("set_active_project", { projectPath: active?.path ?? null });
      }
      if (available.length !== entries.length) {
        setAppError("One or more saved projects are unavailable. Reopen them when their folders are accessible.");
      }
    }).catch((reason: unknown) => {
      if (disposed) return;
      setIsRestoringWorkspace(false);
      setPersistenceError(String(reason));
    });

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;

    let disposed = false;
    let unlisten: (() => void) | undefined;
    let snapshotLoaded = false;
    let deferredUpdates: ApprovalPrompt[] | undefined;

    void listen<ApprovalPrompt[]>("approval-requests", (event) => {
      if (snapshotLoaded) setApprovals(event.payload);
      else deferredUpdates = event.payload;
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    }).catch((reason: unknown) => {
      if (!disposed) setAppError(String(reason));
    });
    void invoke<ApprovalPrompt[]>("pending_approvals").then((snapshot) => {
      if (disposed) return;
      setApprovals(deferredUpdates ?? snapshot);
      snapshotLoaded = true;
    }).catch((reason: unknown) => {
      if (disposed) return;
      if (deferredUpdates) setApprovals(deferredUpdates);
      snapshotLoaded = true;
      setAppError(String(reason));
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function chooseProject() {
    if (!isTauri()) {
      setAppError("Open Caelum as a desktop app to choose a local project.");
      return;
    }

    setIsSelecting(true);
    setAppError(null);
    try {
      const selected = await open({ directory: true, multiple: false, title: "Open a project" });
      if (typeof selected !== "string") return;
      const opened = await invoke<ProjectInfo>("open_project", { path: selected });
      await persist("remember_project", { project: opened }).catch(() => undefined);
      setOpenProjects((current) => addOpenProject(current, opened));
      activateProject(opened);
    } catch (reason) {
      setAppError(String(reason));
    } finally {
      setIsSelecting(false);
    }
  }

  function activateProject(target: ProjectInfo | null, saveActiveProject = true) {
    if (target?.path === project?.path) return;
    setWorkspaceView("overview");
    if (target) {
      setWorkspaces((current) => current[target.path]
        ? current
        : { ...current, [target.path]: createProjectWorkspace() });
    }
    setProject(target);
    setAppError(null);
    if (saveActiveProject) {
      persist("set_active_project", { projectPath: target?.path ?? null });
    }
    if (!target) return;
  }

  function closeProject(path: string) {
    const next = closeOpenProject(openProjects, project?.path ?? null, path);
    persist("close_project_record", {
      projectPath: path,
      activeProjectPath: next.activeProject?.path ?? null,
    });
    setOpenProjects(next.projects);
    if (next.activeProject?.path !== project?.path) {
      activateProject(next.activeProject, false);
    }
  }

  async function loadSessions(target: ProjectInfo, selectedProvider: ProviderKind) {
    const requestId = beginWorkspaceRequest(target.path, selectedProvider);
    mutateProvider(target.path, selectedProvider, (current) => ({ ...current, isLoadingSessions: true, error: null }));
    try {
      let sessions: SessionSummary[];
      if (selectedProvider === "codex") {
        const result = await invoke<CodexSessionList>("codex_list_sessions", {
          projectPath: target.path,
        });
        sessions = result.threads;
      } else {
        const result = await invoke<OpenCodeSessionList>("opencode_list_sessions", {
          projectPath: target.path,
        });
        sessions = result.sessions.map((item) => ({
          id: item.id,
          name: item.title,
          preview: "",
          updatedAt: item.updatedAt,
        }));
      }
      if (isCurrentWorkspaceRequest(target.path, selectedProvider, requestId)) {
        mutateProvider(target.path, selectedProvider, (current) => ({
          ...current,
          sessions,
          sessionsLoaded: true,
          error: null,
        }));
        persist("sync_provider_sessions", {
          projectPath: target.path,
          provider: selectedProvider,
          sessionIds: sessions.map((item) => item.id),
        });
      }
    } catch (reason) {
      if (isCurrentWorkspaceRequest(target.path, selectedProvider, requestId)) {
        mutateProvider(target.path, selectedProvider, (current) => ({ ...current, error: String(reason) }));
      }
    } finally {
      if (isCurrentWorkspaceRequest(target.path, selectedProvider, requestId)) {
        mutateProvider(target.path, selectedProvider, (current) => ({ ...current, isLoadingSessions: false }));
      }
    }
  }

  function changeProvider(nextProvider: ProviderKind) {
    if (!project || provider === nextProvider) return;
    const target = project;
    mutateWorkspace(target.path, (current) => ({ ...current, provider: nextProvider }));
    persist("set_project_provider", { projectPath: target.path, provider: nextProvider });
    setAppError(null);
  }

  async function startSession() {
    if (!project || isLoadingSessions) return;
    const requestId = beginWorkspaceRequest(project.path, provider);
    const currentProject = project;
    const currentProvider = provider;
    mutateProvider(currentProject.path, currentProvider, (current) => ({ ...current, isLoadingSessions: true, error: null }));
    try {
      let started: WorkspaceSessionInfo;
      if (currentProvider === "codex") {
        const result = await invoke<CodexSessionInfo>("codex_start_session", {
          projectPath: currentProject.path,
        });
        started = { ...result, provider: currentProvider };
      } else {
        const result = await invoke<OpenCodeSessionInfo>("opencode_start_session", {
          projectPath: currentProject.path,
        });
        started = {
          project: result.project,
          thread: { id: result.session.id, name: result.session.title, preview: "", updatedAt: result.session.updatedAt },
          provider: currentProvider,
        };
      }
      if (!isCurrentWorkspaceRequest(currentProject.path, currentProvider, requestId)) return;
      mutateProvider(currentProject.path, currentProvider, (current) => ({
        ...current,
        sessions: [started.thread, ...current.sessions.filter((item) => item.id !== started.thread.id)],
        sessionsLoaded: true,
        isLoadingSessions: false,
        selectedSessionId: started.thread.id,
        sessionsById: {
          ...current.sessionsById,
          [started.thread.id]: {
            session: started,
            conversation: [],
            isWorking: false,
            isSubmitting: false,
            error: null,
          },
        },
        error: null,
      }));
      persist("select_project_session", {
        projectPath: currentProject.path,
        provider: currentProvider,
        sessionId: started.thread.id,
      });
    } catch (reason) {
      if (isCurrentWorkspaceRequest(currentProject.path, currentProvider, requestId)) {
        mutateProvider(currentProject.path, currentProvider, (current) => ({ ...current, error: String(reason) }));
      }
    } finally {
      if (isCurrentWorkspaceRequest(currentProject.path, currentProvider, requestId)) {
        mutateProvider(currentProject.path, currentProvider, (current) => ({ ...current, isLoadingSessions: false }));
      }
    }
  }

  async function resumeSession(thread: SessionSummary) {
    if (!project || isLoadingSessions) return;
    const currentWorkspace = workspaces[project.path] ?? createProjectWorkspace();
    const currentProviderWorkspace = currentWorkspace.providers[provider];
    if (currentProviderWorkspace.sessionsById[thread.id]) {
      mutateProvider(project.path, provider, (current) => ({ ...current, selectedSessionId: thread.id, error: null }));
      persist("select_project_session", {
        projectPath: project.path,
        provider,
        sessionId: thread.id,
      });
      return;
    }

    const requestId = beginWorkspaceRequest(project.path, provider);
    const currentProject = project;
    const currentProvider = provider;
    mutateProvider(currentProject.path, currentProvider, (current) => ({ ...current, isLoadingSessions: true, error: null }));
    try {
      let resumed: WorkspaceSessionInfo;
      let conversation: ConversationEntry[];
      if (currentProvider === "codex") {
        const result = await invoke<CodexSessionInfo>("codex_resume_session", {
          projectPath: currentProject.path,
          threadId: thread.id,
        });
        resumed = { ...result, provider: currentProvider };
        conversation = thread.preview ? [{ id: `preview-${thread.id}`, kind: "status", text: thread.preview }] : [];
      } else {
        const result = await invoke<OpenCodeSessionInfo>("opencode_resume_session", {
          projectPath: currentProject.path,
          sessionId: thread.id,
        });
        const history = await invoke<OpenCodeHistoryEntry[]>("opencode_session_history", {
          projectPath: currentProject.path,
          sessionId: thread.id,
        });
        resumed = {
          project: result.project,
          thread: { id: result.session.id, name: result.session.title, preview: "", updatedAt: result.session.updatedAt },
          provider: currentProvider,
        };
        conversation = history.map((entry) => ({
          id: entry.id,
          kind: (["user", "assistant", "activity", "status"].includes(entry.kind) ? entry.kind : "status") as ConversationEntry["kind"],
          text: entry.text,
        }));
      }
      if (!isCurrentWorkspaceRequest(currentProject.path, currentProvider, requestId)) return;
      mutateProvider(currentProject.path, currentProvider, (current) => ({
        ...current,
        sessions: [resumed.thread, ...current.sessions.filter((item) => item.id !== resumed.thread.id)],
        sessionsLoaded: true,
        isLoadingSessions: false,
        selectedSessionId: resumed.thread.id,
        sessionsById: {
          ...current.sessionsById,
          [resumed.thread.id]: {
            session: resumed,
            conversation,
            isWorking: false,
            isSubmitting: false,
            error: null,
          },
        },
        error: null,
      }));
      persist("select_project_session", {
        projectPath: currentProject.path,
        provider: currentProvider,
        sessionId: resumed.thread.id,
      });
    } catch (reason) {
      if (isCurrentWorkspaceRequest(currentProject.path, currentProvider, requestId)) {
        mutateProvider(currentProject.path, currentProvider, (current) => ({ ...current, error: String(reason) }));
      }
    } finally {
      if (isCurrentWorkspaceRequest(currentProject.path, currentProvider, requestId)) {
        mutateProvider(currentProject.path, currentProvider, (current) => ({ ...current, isLoadingSessions: false }));
      }
    }
  }

  useEffect(() => {
    if (!project || !providerWorkspace) return;
    if (!providerWorkspace.sessionsLoaded) {
      if (!providerWorkspace.isLoadingSessions && !providerWorkspace.error) {
        void loadSessions(project, provider);
      }
      return;
    }

    if (!selectedSessionId || providerWorkspace.isLoadingSessions) return;
    if (providerWorkspace.sessionsById[selectedSessionId]) return;

    const savedSession = providerWorkspace.sessions.find((item) => item.id === selectedSessionId);
    if (savedSession) {
      void resumeSession(savedSession);
    } else {
      mutateProvider(project.path, provider, (current) => ({ ...current, selectedSessionId: null }));
      persist("select_project_session", {
        projectPath: project.path,
        provider,
        sessionId: null,
      });
    }
  }, [project?.path, provider, providerWorkspace, selectedSessionId]);

  async function submitPrompt() {
    const currentProject = project;
    const currentProvider = provider;
    const currentSession = session;
    const text = prompt.trim();
    if (!currentProject || !currentSession || !text || isSubmitting || isWorking) return;
    const sessionId = currentSession.thread.id;
    mutateWorkspace(currentProject.path, (current) => ({ ...current, prompt: "" }));
    mutateProvider(currentProject.path, currentProvider, (current) => ({ ...current, error: null }));
    mutateSession(currentProject.path, currentProvider, sessionId, (current) => ({
      ...current,
      isSubmitting: true,
      isWorking: true,
      error: null,
      conversation: [...current.conversation, {
        id: `user-${Date.now()}`,
        kind: "user" as const,
        text,
      }].slice(-300),
    }));
    try {
      if (currentSession.provider === "codex") {
        await invoke("codex_send_prompt", {
          projectPath: currentProject.path,
          threadId: currentSession.thread.id,
          prompt: text,
        });
      } else {
        await invoke("opencode_send_prompt", {
          projectPath: currentProject.path,
          sessionId: currentSession.thread.id,
          prompt: text,
        });
      }
    } catch (reason) {
      mutateSession(currentProject.path, currentProvider, sessionId, (current) => ({
        ...current,
        isWorking: false,
        isSubmitting: false,
        error: String(reason),
      }));
    } finally {
      mutateSession(currentProject.path, currentProvider, sessionId, (current) => ({ ...current, isSubmitting: false }));
    }
  }

  async function cancelTurn() {
    if (!project || !session || !isWorking) return;
    const currentProject = project;
    const currentProvider = provider;
    const sessionId = session.thread.id;
    mutateSession(currentProject.path, currentProvider, sessionId, (current) => ({ ...current, error: null }));
    try {
      if (currentProvider === "codex") {
        await invoke("codex_cancel_turn", {
          projectPath: currentProject.path,
          threadId: sessionId,
        });
      } else {
        await invoke("opencode_cancel_turn", {
          projectPath: currentProject.path,
          sessionId,
        });
      }
    } catch (reason) {
      mutateSession(currentProject.path, currentProvider, sessionId, (current) => ({ ...current, error: String(reason) }));
    }
  }

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<CodexEvent>("codex-event", (event) => {
      if (!disposed) setWorkspaces((current) => applyCodexEvent(current, event.payload));
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    }).catch((reason: unknown) => {
      if (!disposed) setAppError(String(reason));
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<OpenCodeEvent>("opencode-event", (event) => {
      if (!disposed) setWorkspaces((current) => applyOpenCodeEvent(current, event.payload));
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    }).catch((reason: unknown) => {
      if (!disposed) setAppError(String(reason));
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function resolveApproval(decision: "allowOnce" | "deny") {
    const request = approvals[0];
    if (!request || isResolvingApproval) return;

    setIsResolvingApproval(true);
    setAppError(null);
    try {
      await invoke("resolve_approval", {
        provider: request.provider,
        projectPath: request.projectPath,
        requestId: request.requestId,
        decision,
      });
    } catch (reason) {
      const message = String(reason);
      if (request.provider === "codex" || request.provider === "opencode") {
        if (request.sessionId && workspaces[request.projectPath]?.providers[request.provider].sessionsById[request.sessionId]) {
          mutateSession(request.projectPath, request.provider, request.sessionId, (current) => ({ ...current, error: message }));
        } else {
          mutateProvider(request.projectPath, request.provider, (current) => ({ ...current, error: message }));
        }
      } else {
        setAppError(message);
      }
    } finally {
      setIsResolvingApproval(false);
    }
  }

  const providerName = provider === "codex" ? "Codex" : "OpenCode";
  const providerDetails = provider === "codex"
    ? "Local Codex CLI · project-scoped sessions"
    : "OpenCode local server · HTTP/SSE sessions";
  const approvalProject = approvals[0]
    ? selectOpenProject(openProjects, approvals[0].projectPath)
    : null;

  return (
    <main className={`app-shell${hasMacTitlebarOverlay ? " macos-titlebar-overlay" : ""}`}>
      {approvals[0] && (
        <div className="approval-backdrop">
          <section className="approval-dialog" role="dialog" aria-modal="true" aria-labelledby="approval-title">
            <span className="card-kicker">{approvals.length > 1 ? `APPROVAL 1 OF ${approvals.length}` : "APPROVAL REQUIRED"}</span>
            <h2 id="approval-title">{approvals[0].summary}</h2>
            <p>{approvals[0].description}</p>
            <small>
              Requested by {approvals[0].provider} · {approvalProject?.name ?? approvals[0].projectPath}
              {approvals[0].sessionId ? ` · session ${approvals[0].sessionId}` : ""}. Approval applies to this request only.
            </small>
            <div className="approval-actions">
              {approvalProject && approvalProject.path !== project?.path && (
                <button className="approval-project" onClick={() => activateProject(approvalProject)}>
                  View project
                </button>
              )}
              <button className="approval-deny" onClick={() => void resolveApproval("deny")} disabled={isResolvingApproval}>
                Deny
              </button>
              <button className="approval-allow" onClick={() => void resolveApproval("allowOnce")} disabled={isResolvingApproval}>
                Allow once
              </button>
            </div>
          </section>
        </div>
      )}
      <aside className="sidebar">
        <div className="brand-row">
          <div className="brand-mark"><Orbit size={17} strokeWidth={1.8} /></div>
          <div className="brand-copy"><span>caelum</span><small>YOUR CODE. NEW HORIZONS.</small></div>
          <button className="icon-button quiet" aria-label="Collapse sidebar"><PanelLeftClose size={16} /></button>
        </div>

        <div className="section-heading projects-heading">
          <span>PROJECTS</span>
          <button className="icon-button quiet" aria-label="Add project" title="Add project" onClick={() => void chooseProject()} disabled={isSelecting}>
            <Plus size={14} />
          </button>
        </div>
        <nav className="project-list" aria-label="Open projects">
          {isRestoringWorkspace && <div className="empty-sessions">Restoring workspace…</div>}
          {openProjects.length === 0 && !isRestoringWorkspace && (
            <div className="empty-projects">
              <span>No projects open</span>
              <button onClick={() => void chooseProject()} disabled={isSelecting}>Open a project</button>
            </div>
          )}
          {openProjects.map((item) => (
            <div className={`project-row${project?.path === item.path ? " selected" : ""}`} key={item.path}>
              <button
                className="project-item"
                aria-current={project?.path === item.path ? "page" : undefined}
                title={item.path}
                onClick={() => activateProject(selectOpenProject(openProjects, item.path))}
              >
                <FolderGit2 size={14} />
                <span><strong>{item.name}</strong><small>{item.path}</small></span>
                {workspaces[item.path] && projectHasActiveTurn(workspaces[item.path]) && (
                  <span className="project-turn-indicator" title="Agent turn active" aria-label="Agent turn active" />
                )}
              </button>
              <button
                className="project-close"
                aria-label={`Close ${item.name}`}
                title={`Close ${item.name}`}
                onClick={() => closeProject(item.path)}
              >
                <X size={13} />
              </button>
            </div>
          ))}
        </nav>

        <button className="new-session" onClick={() => void startSession()} disabled={!project || isLoadingSessions} title={!project ? "Choose a project first" : `Start an ${providerName} session`}>
          <MessageSquarePlus size={16} /><span>New session</span><kbd>⌘ N</kbd>
        </button>

        <div className="section-heading"><span>WORKSPACE</span></div>
        <nav className="side-nav" aria-label="Workspace">
          <button className={`nav-item${workspaceView === "overview" ? " active" : ""}`} onClick={() => setWorkspaceView("overview")}><Layers3 size={16} /><span>Overview</span></button>
          <button className={`nav-item${workspaceView === "files" ? " active" : ""}`} onClick={() => setWorkspaceView("files")} disabled={!project} title={!project ? "Choose a project first" : "Browse project files"}><FileCode2 size={16} /><span>Files</span></button>
          <button className={`nav-item${workspaceView === "changes" ? " active" : ""}`} onClick={() => setWorkspaceView("changes")} disabled={!project} title={!project ? "Choose a project first" : "Inspect Git status and diffs"}><GitBranch size={16} /><span>Changes</span></button>
          <button className="nav-item" disabled><Activity size={16} /><span>Terminal</span><span className="nav-soon">Soon</span></button>
        </nav>

        <div className="section-heading sessions-heading"><span>RECENT SESSIONS</span><button className="icon-button quiet" aria-label="Add session" onClick={() => void startSession()} disabled={!project || isLoadingSessions}><Plus size={14} /></button></div>
        <div className="session-list">
          {!project && <div className="empty-sessions">Choose a project to see its {providerName} conversations.</div>}
          {project && isLoadingSessions && sessions.length === 0 && <div className="empty-sessions">Connecting to {providerName}…</div>}
          {project && !isLoadingSessions && sessions.length === 0 && <div className="empty-sessions">No {providerName} conversations yet.</div>}
          {sessions.map((item) => (
            <button className={`session-item${selectedSessionId === item.id ? " selected" : ""}`} key={item.id} onClick={() => void resumeSession(item)} disabled={isLoadingSessions}>
              <MessageSquarePlus size={13} /><span><strong>{item.name || item.preview || `New ${providerName} session`}</strong><small>{providerWorkspace?.sessionsById[item.id]?.isWorking ? "Working…" : item.preview || "No messages yet"}</small></span>
              {providerWorkspace?.sessionsById[item.id]?.isWorking && <span className="session-turn-indicator" title="Agent turn active" aria-label="Agent turn active" />}
            </button>
          ))}
        </div>

        <div className="sidebar-bottom">
          <button className="nav-item" disabled><Settings2 size={16} /><span>Settings</span></button>
          <div className="profile-card"><div className="avatar">C</div><div><strong>Local workspace</strong><small>{appInfo ? `Caelum ${appInfo.version}` : "No account connected"}</small></div><CircleHelp size={15} className="muted-icon" /></div>
        </div>
      </aside>

      <section className="main-panel">
        <header className="topbar">
          <div className="breadcrumbs"><span>Workspace</span><span className="crumb-divider">/</span><strong>{workspaceView === "changes" ? "Changes" : workspaceView === "files" ? "Files" : project?.name ?? "Overview"}</strong></div>
          <div className="topbar-actions"><button className="command-button"><Command size={13} /><span>Quick actions</span><kbd>⌘ K</kbd></button><button className="icon-button" aria-label="Performance"><Gauge size={16} /></button></div>
        </header>

        <div className={`content-scroll${workspaceView === "changes" ? " changes-content" : workspaceView === "files" ? " files-content" : ""}`}>
          {workspaceView === "changes" ? (
            <GitReviewPanel project={project} />
          ) : workspaceView === "files" ? (
            <FilesPanel project={project} />
          ) : (
            <>
          <div className="welcome-block">
            <div className="eyebrow"><span className="status-dot" /> LOCAL-FIRST AI WORKSPACE</div>
            <h1>{session ? session.thread.name || `Working in ${project?.name}.` : project ? `Ready to work in ${project.name}.` : "A clearer sky for your code."}</h1>
            <p>{session ? `${providerName} session · ${session.thread.id}` : project ? `Your project is open. Start or resume a ${providerName} session to begin.` : "Bring your coding agents and projects together in one native workspace."}</p>
          </div>

          {persistenceError && <div className="error-banner" role="alert">{persistenceError}</div>}
          {error && <div className="error-banner" role="alert">{error}</div>}

          <div className="setup-grid">
            <button className="setup-card project-card" onClick={chooseProject} disabled={isSelecting}>
              <div className="card-icon blue"><FolderOpen size={18} /></div>
              <div className="card-copy"><span className="card-kicker">FIRST STEP</span><strong>{project ? "Add another project" : "Open a project"}</strong><small>{project ? project.path : "Select a local folder to get started"}</small></div>
              <span className="card-arrow">↗</span>
            </button>
            <div className="setup-card provider-card">
              <div className="card-icon violet"><Sparkles size={18} /></div>
              <div className="card-copy"><span className="card-kicker">PROVIDER</span><strong>{provider === "codex" ? "Codex app-server" : "OpenCode server"}</strong><small>{project ? providerDetails : `Open a project to connect ${providerName}`}</small>
                <select className="provider-select" aria-label="AI provider" value={provider} disabled={isLoadingSessions || isSubmitting || isWorking} onChange={(event) => void changeProvider(event.target.value as ProviderKind)}>
                  <option value="codex">Codex</option>
                  <option value="opencode">OpenCode</option>
                </select>
              </div>
              <span className="provider-badge">{provider === "codex" ? "CODEX" : "OPENCODE"}</span>
            </div>
          </div>

          {session ? (
            <div className="conversation" aria-live="polite" aria-label={`${providerName} conversation`}>
              {conversation.map((entry) => (
                <article className={`conversation-entry ${entry.kind}`} key={entry.id}>
                  {entry.kind !== "user" && entry.kind !== "assistant" && <span className="entry-kicker">{entry.kind === "activity" ? "ACTIVITY" : provider.toUpperCase()}</span>}
                  <p>{entry.text}</p>
                </article>
              ))}
              {isWorking && <div className="working-indicator"><span className="status-dot" /> {providerName} is working…</div>}
            </div>
          ) : (
            <div className="starter-panel">
              <div className="starter-heading"><div className="starter-orbit"><Orbit size={17} /></div><div><strong>{project ? "Your project is ready" : "Designed for your flow"}</strong><p>{project ? `Start a ${providerName} session to work with this project.` : "A calm, focused home for AI-assisted development."}</p></div></div>
              <div className="starter-divider" />
              <div className="feature-row"><span className="feature-check">01</span><span>Native macOS shell, powered by Tauri and Rust</span><span className="feature-state">READY</span></div>
              <div className="feature-row"><span className="feature-check">02</span><span>Project-scoped {providerName} sessions</span><span className="feature-state">READY</span></div>
              <div className="feature-row"><span className="feature-check">03</span><span>Streamed responses, tool activity, and approvals</span><span className="feature-state">READY</span></div>
            </div>
          )}

          <div className="prompt-dock">
            <textarea aria-label="Prompt" placeholder={session ? `Ask ${providerName} to work on your project…` : "Start or resume a session to send a prompt…"} value={prompt} disabled={!session || isWorking} onChange={(event) => { if (project) mutateWorkspace(project.path, (current) => ({ ...current, prompt: event.target.value })); }} onKeyDown={(event) => { if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) { event.preventDefault(); void submitPrompt(); } }} />
            <div className="prompt-footer"><span><span className={`tiny-agent-dot${session ? " connected" : ""}`} /> {session ? isWorking ? `${providerName} is working` : `${providerName} connected` : "No session selected"}</span>
              {isWorking ? <button onClick={() => void cancelTurn()} aria-label={`Cancel ${providerName} turn`} title="Cancel turn"><Square size={13} /></button> : <button onClick={() => void submitPrompt()} disabled={!session || !prompt.trim() || isSubmitting} aria-label="Send prompt"><ArrowUp size={16} /></button>}
            </div>
          </div>
          <div className="bottom-note">{project ? `${session ? `${providerName} · ` : "Working locally · "}${project.path}` : "Your projects stay on your machine."}</div>
            </>
          )}
        </div>
      </section>
    </main>
  );
}
