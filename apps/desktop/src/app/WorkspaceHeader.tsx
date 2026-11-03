import {
  LoaderCircle,
  MessageCircleQuestion,
  PanelLeftOpen,
  PanelRight,
  Search,
  Settings,
  ShieldAlert,
} from "lucide-react";
import type { WorkspaceController } from "../application/workspace/controller.js";
import type { ApprovalPrompt } from "../application/workspace/useWorkspace.js";
import type { QuestionPrompt } from "../domain/sessions/questionState.js";
import type { WorkspaceSurfaces } from "./useWorkspaceSurfaces.js";

export function WorkspaceHeader({
  app,
  title,
  surfaces,
  activeApprovals,
  activeQuestions,
  showSession,
  focusSearch,
}: {
  app: WorkspaceController;
  title: string;
  surfaces: WorkspaceSurfaces;
  activeApprovals: ApprovalPrompt[];
  activeQuestions: QuestionPrompt[];
  showSession: () => void;
  focusSearch: () => void;
}) {
  const { activePullRequest, collapsed, changesOpen } = surfaces;
  return (
    <header className="topbar" data-tauri-drag-region>
      <div className="breadcrumbs">
        {collapsed && (
          <button
            className="icon-button"
            aria-label="Expand sidebar"
            onClick={() => surfaces.setCollapsed(false)}
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
              surfaces.setSettingsOpen(true);
            }}
          >
            <Settings size={16} />
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
              surfaces.showSession();
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
            onClick={surfaces.toggleChanges}
          >
            <PanelRight size={16} />
          </button>
        )}
      </div>
    </header>
  );
}
