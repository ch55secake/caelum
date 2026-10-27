import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke, isTauri } from "@tauri-apps/api/core";
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
} from "lucide-react";

type AppInfo = { name: string; version: string };
type ProjectInfo = { name: string; path: string };

export default function App() {
  const hasMacTitlebarOverlay = isTauri() && /Macintosh|Mac OS X/.test(navigator.userAgent);
  const [appInfo, setAppInfo] = useState<AppInfo | null>(null);
  const [project, setProject] = useState<ProjectInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isSelecting, setIsSelecting] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    void invoke<AppInfo>("app_info").then(setAppInfo).catch((reason: unknown) => {
      setError(String(reason));
    });
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
    } catch (reason) {
      setError(String(reason));
    } finally {
      setIsSelecting(false);
    }
  }

  return (
    <main className={`app-shell${hasMacTitlebarOverlay ? " macos-titlebar-overlay" : ""}`}>
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

        <button className="new-session" disabled={!project} title={!project ? "Choose a project first" : "New session (coming soon)"}>
          <MessageSquarePlus size={16} /><span>New session</span><kbd>⌘ N</kbd>
        </button>

        <div className="section-heading"><span>WORKSPACE</span></div>
        <nav className="side-nav" aria-label="Workspace">
          <button className="nav-item active"><Layers3 size={16} /><span>Overview</span></button>
          <button className="nav-item" disabled><FileCode2 size={16} /><span>Files</span><span className="nav-soon">Soon</span></button>
          <button className="nav-item" disabled><GitBranch size={16} /><span>Changes</span><span className="nav-soon">Soon</span></button>
          <button className="nav-item" disabled><Activity size={16} /><span>Terminal</span><span className="nav-soon">Soon</span></button>
        </nav>

        <div className="section-heading sessions-heading"><span>RECENT SESSIONS</span><button className="icon-button quiet" aria-label="Add session" disabled><Plus size={14} /></button></div>
        <div className="empty-sessions">Your conversations<br />will show up here.</div>

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
            <h1>{project ? `Ready to work in ${project.name}.` : "A clearer sky for your code."}</h1>
            <p>{project ? "Your project is open. Choose an agent to start a focused coding session." : "Bring your coding agents and projects together in one native workspace."}</p>
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
              <div className="card-copy"><span className="card-kicker">COMING NEXT</span><strong>Connect an agent</strong><small>Codex and OpenCode integrations are in progress</small></div>
              <span className="provider-badge">CODEX · OPENCODE</span>
            </div>
          </div>

          <div className="starter-panel">
            <div className="starter-heading"><div className="starter-orbit"><Orbit size={17} /></div><div><strong>{project ? "Start with a clean slate" : "Designed for your flow"}</strong><p>{project ? "Agent sessions are the next milestone." : "A calm, focused home for AI-assisted development."}</p></div></div>
            <div className="starter-divider" />
            <div className="feature-row"><span className="feature-check">01</span><span>Native macOS shell, powered by Tauri and Rust</span><span className="feature-state">READY</span></div>
            <div className="feature-row"><span className="feature-check">02</span><span>Shared core for desktop and terminal workflows</span><span className="feature-state">IN PROGRESS</span></div>
            <div className="feature-row"><span className="feature-check">03</span><span>Codex and OpenCode provider sessions</span><span className="feature-state planned">NEXT</span></div>
          </div>

          <div className="prompt-dock">
            <textarea aria-label="Prompt" placeholder={project ? "Agent prompts will be available in a future milestone…" : "Open a project to begin…"} disabled />
            <div className="prompt-footer"><span><span className="tiny-agent-dot" /> No agent connected</span><button disabled aria-label="Send prompt"><ArrowUp size={16} /></button></div>
          </div>
          <div className="bottom-note">{project ? `Working locally · ${project.path}` : "Your projects stay on your machine."}</div>
        </div>
      </section>
    </main>
  );
}
