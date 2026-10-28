import { useEffect, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Activity,
  ArrowUp,
  ChevronDown,
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
} from "lucide-react";

type AppInfo = { name: string; version: string };
type ProjectInfo = { name: string; path: string };
type ApprovalPrompt = {
  provider: string;
  requestId: string;
  summary: string;
  description: string;
};
type CodexThreadInfo = { id: string; name: string | null; preview: string; updatedAt: number | null };
type CodexSessionInfo = { project: ProjectInfo; thread: CodexThreadInfo };
type CodexSessionList = { project: ProjectInfo; threads: CodexThreadInfo[] };
type CodexEvent = { projectPath: string; method: string; params: Record<string, unknown>; requestId: unknown };
type ConversationEntry = { id: string; kind: "user" | "assistant" | "activity" | "status"; text: string };

function valueString(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

function itemActivity(item: unknown): string | undefined {
  if (!item || typeof item !== "object") return undefined;
  const value = item as Record<string, unknown>;
  const type = valueString(value.type) ?? "tool";
  if (type === "commandExecution") {
    const command = Array.isArray(value.command) ? value.command.join(" ") : valueString(value.command);
    return command ? `Running command: ${command}` : "Running a command";
  }
  if (type === "fileChange") return "Preparing file changes";
  if (type === "mcpToolCall") return `Using tool: ${valueString(value.tool) ?? "MCP tool"}`;
  if (type === "webSearch") return `Searching the web: ${valueString(value.query) ?? ""}`.trim();
  if (type === "reasoning") return undefined;
  return `Codex activity: ${type.replace(/([A-Z])/g, " $1").toLowerCase()}`;
}

export default function App() {
  const hasMacTitlebarOverlay = isTauri() && /Macintosh|Mac OS X/.test(navigator.userAgent);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSelecting, setIsSelecting] = useState(false);
  const [approvals, setApprovals] = useState<ApprovalPrompt[]>([]);
  const [isResolvingApproval, setIsResolvingApproval] = useState(false);
  const [sessions, setSessions] = useState<CodexThreadInfo[]>([]);
  const [session, setSession] = useState<CodexSessionInfo | null>(null);
  const [conversation, setConversation] = useState<ConversationEntry[]>([]);
  const [prompt, setPrompt] = useState("");
  const [isLoadingSessions, setIsLoadingSessions] = useState(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [isWorking, setIsWorking] = useState(false);
  const projectRef = useRef(project);
  const sessionRef = useRef(session);
  projectRef.current = project;
  sessionRef.current = session;

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<AppInfo>("app_info").then(setAppInfo).catch((reason: unknown) => {
      setError(String(reason));
    });
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
      if (!disposed) setError(String(reason));
    });
    void invoke<ApprovalPrompt[]>("pending_approvals").then((snapshot) => {
      if (disposed) return;
      setApprovals(deferredUpdates ?? snapshot);
      snapshotLoaded = true;
    }).catch((reason: unknown) => {
      if (disposed) return;
      if (deferredUpdates) setApprovals(deferredUpdates);
      snapshotLoaded = true;
      setError(String(reason));
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  async function chooseProject() {
    if (!isTauri()) {
      setError("Open Caelum as a desktop app to choose a local project.");
      return;
    }

    setIsSelecting(true);
    setError(null);
    try {
      const selected = await open({ directory: true, multiple: false, title: "Open a project" });
      if (typeof selected !== "string") return;
      const opened = await invoke<ProjectInfo>("open_project", { path: selected });
      setProject(opened);
      setSession(null);
      setConversation([]);
      setSessions([]);
      setPrompt("");
      setIsWorking(false);
      await loadSessions(opened);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsSelecting(false);
    }
  }

  async function loadSessions(target: ProjectInfo) {
    setIsLoadingSessions(true);
    setError(null);
    try {
      const result = await invoke<CodexSessionList>("codex_list_sessions", {
        projectPath: target.path,
      });
      setSessions(result.threads);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsLoadingSessions(false);
    }
  }

  async function startSession() {
    if (!project || isLoadingSessions) return;
    setError(null);
    setIsLoadingSessions(true);
    try {
      const started = await invoke<CodexSessionInfo>("codex_start_session", {
        projectPath: project.path,
      });
      setSession(started);
      setConversation([]);
      setIsWorking(false);
      setSessions((current) => [started.thread, ...current.filter((item) => item.id !== started.thread.id)]);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsLoadingSessions(false);
    }
  }

  async function resumeSession(thread: CodexThreadInfo) {
    if (!project || isLoadingSessions) return;
    setError(null);
    setIsLoadingSessions(true);
    try {
      const resumed = await invoke<CodexSessionInfo>("codex_resume_session", {
        projectPath: project.path,
        threadId: thread.id,
      });
      setSession(resumed);
      setConversation(thread.preview ? [{ id: `preview-${thread.id}`, kind: "status", text: thread.preview }] : []);
      setIsWorking(false);
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsLoadingSessions(false);
    }
  }

  async function submitPrompt() {
    const currentProject = project;
    const currentSession = session;
    const text = prompt.trim();
    if (!currentProject || !currentSession || !text || isSubmitting || isWorking) return;
    setPrompt("");
    setIsSubmitting(true);
    setIsWorking(true);
    setConversation((current) => [...current, {
      id: `user-${Date.now()}`,
      kind: "user",
      text,
    }]);
    setError(null);
    try {
      await invoke("codex_send_prompt", {
        projectPath: currentProject.path,
        threadId: currentSession.thread.id,
        prompt: text,
      });
    } catch (reason) {
      setIsWorking(false);
      setError(String(reason));
    } finally {
      setIsSubmitting(false);
    }
  }

  async function cancelTurn() {
    if (!project || !session || !isWorking) return;
    setError(null);
    try {
      await invoke("codex_cancel_turn", {
        projectPath: project.path,
        threadId: session.thread.id,
      });
    } catch (reason) {
      setError(String(reason));
    }
  }

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen<CodexEvent>("codex-event", (event) => {
      const update = event.payload;
      if (disposed) return;
      const currentProject = projectRef.current;
      const currentSession = sessionRef.current;
      if (!currentProject || update.projectPath !== currentProject.path) return;
      const params = update.params && typeof update.params === "object" ? update.params : {};
      if (update.method === "codex/processExited") {
        if (currentSession) setIsWorking(false);
        setError("The Codex app-server stopped. Start or resume a session to reconnect.");
        return;
      }
      if (!currentSession) return;
      const eventThreadId = valueString(params.threadId)
        ?? valueString((params.thread as Record<string, unknown> | undefined)?.id);
      if (!eventThreadId || eventThreadId !== currentSession.thread.id) return;

      if (update.method === "turn/started") setIsWorking(true);
      if (update.method === "turn/completed") {
        setIsWorking(false);
        const turn = params.turn as Record<string, unknown> | undefined;
        const status = turn && typeof turn.status === "object" && turn.status
          ? valueString((turn.status as Record<string, unknown>).type)
          : valueString(turn?.status);
        setConversation((current): ConversationEntry[] => [...current, {
          id: `turn-${String(turn?.id ?? Date.now())}`,
          kind: "status" as const,
          text: status === "interrupted" ? "Turn cancelled" : status === "failed" ? "Codex could not complete this turn" : "Turn complete",
        }].slice(-300));
      }
      if (update.method === "item/agentMessage/delta") {
        const delta = valueString(params.delta);
        if (!delta) return;
        const itemId = valueString(params.itemId) ?? `stream-${currentSession.thread.id}`;
        setConversation((current): ConversationEntry[] => {
          const existing = current.findIndex((entry) => entry.id === `assistant-${itemId}`);
          if (existing < 0) return [...current, { id: `assistant-${itemId}`, kind: "assistant" as const, text: delta }].slice(-300);
          const next = [...current];
          next[existing] = { ...next[existing], text: next[existing].text + delta };
          return next;
        });
      }
      if (update.method === "item/started") {
        const item = params.item as Record<string, unknown> | undefined;
        const activity = itemActivity(item);
        if (activity) {
          const id = valueString(item?.id) ?? `${Date.now()}`;
          setConversation((current): ConversationEntry[] => [...current, { id: `activity-${id}`, kind: "activity" as const, text: activity }].slice(-300));
        }
      }
      if (update.method === "item/commandExecution/outputDelta") {
        const id = valueString(params.itemId);
        const delta = valueString(params.delta);
        if (!id || !delta) return;
        setConversation((current): ConversationEntry[] => {
          const entryId = `activity-${id}`;
          const existing = current.findIndex((entry) => entry.id === entryId);
          if (existing < 0) {
            return [...current, { id: entryId, kind: "activity" as const, text: `Command output:\n${delta}`.slice(-12000) }].slice(-300);
          }
          const next = [...current];
          next[existing] = { ...next[existing], text: `${next[existing].text}\n${delta}`.slice(-12000) };
          return next;
        });
      }
      if (update.method === "item/completed") {
        const item = params.item as Record<string, unknown> | undefined;
        const id = valueString(item?.id);
        const status = valueString(item?.status);
        if (id && status) {
          setConversation((current) => current.map((entry) => {
            if (entry.id !== `activity-${id}`) return entry;
            const finalStatus = status === "completed" ? "complete" : status;
            return { ...entry, text: `${entry.text} (${finalStatus})` };
          }));
        }
      }
    }).then((stopListening) => {
      if (disposed) stopListening();
      else unlisten = stopListening;
    }).catch((reason: unknown) => {
      if (!disposed) setError(String(reason));
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
    setError(null);
    try {
      await invoke("resolve_approval", {
        provider: request.provider,
        requestId: request.requestId,
        decision,
      });
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsResolvingApproval(false);
    }
  }

  return (
    <main className={`app-shell${hasMacTitlebarOverlay ? " macos-titlebar-overlay" : ""}`}>
      {approvals[0] && (
        <div className="approval-backdrop">
          <section className="approval-dialog" role="dialog" aria-modal="true" aria-labelledby="approval-title">
            <span className="card-kicker">{approvals.length > 1 ? `APPROVAL 1 OF ${approvals.length}` : "APPROVAL REQUIRED"}</span>
            <h2 id="approval-title">{approvals[0].summary}</h2>
            <p>{approvals[0].description}</p>
            <small>Requested by {approvals[0].provider}. Approval applies to this request only.</small>
            <div className="approval-actions">
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

        <button className="workspace-picker" onClick={chooseProject} disabled={isSelecting}>
          <span className="workspace-icon"><FolderGit2 size={16} /></span>
          <span className="workspace-label"><strong>{project?.name ?? "Select a project"}</strong><small>{project ? "Local workspace" : "Choose a folder to begin"}</small></span>
          <ChevronDown size={14} className="muted-icon" />
        </button>

        <button className="new-session" onClick={() => void startSession()} disabled={!project || isLoadingSessions} title={!project ? "Choose a project first" : "Start a Codex session"}>
          <MessageSquarePlus size={16} /><span>New session</span><kbd>⌘ N</kbd>
        </button>

        <div className="section-heading"><span>WORKSPACE</span></div>
        <nav className="side-nav" aria-label="Workspace">
          <button className="nav-item active"><Layers3 size={16} /><span>Overview</span></button>
          <button className="nav-item" disabled><FileCode2 size={16} /><span>Files</span><span className="nav-soon">Soon</span></button>
          <button className="nav-item" disabled><GitBranch size={16} /><span>Changes</span><span className="nav-soon">Soon</span></button>
          <button className="nav-item" disabled><Activity size={16} /><span>Terminal</span><span className="nav-soon">Soon</span></button>
        </nav>

        <div className="section-heading sessions-heading"><span>RECENT SESSIONS</span><button className="icon-button quiet" aria-label="Add session" onClick={() => void startSession()} disabled={!project || isLoadingSessions}><Plus size={14} /></button></div>
        <div className="session-list">
          {!project && <div className="empty-sessions">Choose a project to see its Codex conversations.</div>}
          {project && isLoadingSessions && sessions.length === 0 && <div className="empty-sessions">Connecting to Codex…</div>}
          {project && !isLoadingSessions && sessions.length === 0 && <div className="empty-sessions">No Codex conversations yet.</div>}
          {sessions.map((item) => (
            <button className={`session-item${session?.thread.id === item.id ? " selected" : ""}`} key={item.id} onClick={() => void resumeSession(item)} disabled={isLoadingSessions}>
              <MessageSquarePlus size={13} /><span><strong>{item.name || item.preview || "New Codex session"}</strong><small>{item.preview || "No messages yet"}</small></span>
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
          <div className="breadcrumbs"><span>Workspace</span><span className="crumb-divider">/</span><strong>{project?.name ?? "Overview"}</strong></div>
          <div className="topbar-actions"><button className="command-button"><Command size={13} /><span>Quick actions</span><kbd>⌘ K</kbd></button><button className="icon-button" aria-label="Performance"><Gauge size={16} /></button></div>
        </header>

        <div className="content-scroll">
          <div className="welcome-block">
            <div className="eyebrow"><span className="status-dot" /> LOCAL-FIRST AI WORKSPACE</div>
            <h1>{session ? session.thread.name || `Working in ${project?.name}.` : project ? `Ready to work in ${project.name}.` : "A clearer sky for your code."}</h1>
            <p>{session ? `Codex session · ${session.thread.id}` : project ? "Your project is open. Start or resume a Codex session to begin." : "Bring your coding agents and projects together in one native workspace."}</p>
          </div>

          {error && <div className="error-banner" role="alert">{error}</div>}

          <div className="setup-grid">
            <button className="setup-card project-card" onClick={chooseProject} disabled={isSelecting}>
              <div className="card-icon blue"><FolderOpen size={18} /></div>
              <div className="card-copy"><span className="card-kicker">FIRST STEP</span><strong>{project ? "Switch project" : "Open a project"}</strong><small>{project ? project.path : "Select a local folder to get started"}</small></div>
              <span className="card-arrow">↗</span>
            </button>
            <div className="setup-card provider-card">
              <div className="card-icon violet"><Sparkles size={18} /></div>
              <div className="card-copy"><span className="card-kicker">PROVIDER</span><strong>Codex app-server</strong><small>{project ? "Local Codex CLI · project-scoped sessions" : "Open a project to connect Codex"}</small></div>
              <span className="provider-badge">CODEX</span>
            </div>
          </div>

          {session ? (
            <div className="conversation" aria-live="polite" aria-label="Codex conversation">
              {conversation.map((entry) => (
                <article className={`conversation-entry ${entry.kind}`} key={entry.id}>
                  {entry.kind !== "user" && entry.kind !== "assistant" && <span className="entry-kicker">{entry.kind === "activity" ? "ACTIVITY" : "CODEX"}</span>}
                  <p>{entry.text}</p>
                </article>
              ))}
              {isWorking && <div className="working-indicator"><span className="status-dot" /> Codex is working…</div>}
            </div>
          ) : (
            <div className="starter-panel">
              <div className="starter-heading"><div className="starter-orbit"><Orbit size={17} /></div><div><strong>{project ? "Your project is ready" : "Designed for your flow"}</strong><p>{project ? "Start a Codex session to work with this project." : "A calm, focused home for AI-assisted development."}</p></div></div>
              <div className="starter-divider" />
              <div className="feature-row"><span className="feature-check">01</span><span>Native macOS shell, powered by Tauri and Rust</span><span className="feature-state">READY</span></div>
              <div className="feature-row"><span className="feature-check">02</span><span>Project-scoped Codex app-server sessions</span><span className="feature-state">READY</span></div>
              <div className="feature-row"><span className="feature-check">03</span><span>Streamed responses, tool activity, and approvals</span><span className="feature-state">READY</span></div>
            </div>
          )}

          <div className="prompt-dock">
            <textarea aria-label="Prompt" placeholder={session ? "Ask Codex to work on your project…" : project ? "Start or resume a session to send a prompt…" : "Open a project to begin…"} value={prompt} disabled={!session || isWorking} onChange={(event) => setPrompt(event.target.value)} onKeyDown={(event) => { if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) { event.preventDefault(); void submitPrompt(); } }} />
            <div className="prompt-footer"><span><span className={`tiny-agent-dot${session ? " connected" : ""}`} /> {session ? isWorking ? "Codex is working" : "Codex connected" : "No session selected"}</span>
              {isWorking ? <button onClick={() => void cancelTurn()} aria-label="Cancel Codex turn" title="Cancel turn"><Square size={13} /></button> : <button onClick={() => void submitPrompt()} disabled={!session || !prompt.trim() || isSubmitting} aria-label="Send prompt"><ArrowUp size={16} /></button>}
            </div>
          </div>
          <div className="bottom-note">{project ? `${session ? "Codex · " : "Working locally · "}${project.path}` : "Your projects stay on your machine."}</div>
        </div>
      </section>
    </main>
  );
}
