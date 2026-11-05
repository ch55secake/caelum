import { FolderOpen, GitBranch, Layers, MessageSquarePlus } from "lucide-react";
import type { WorkspaceController } from "../../application/workspace/controller.js";
import { worktreeLabel } from "../../domain/projects/worktreeState.js";

export function NewSessionWelcome({
  app,
  setup,
}: {
  app: WorkspaceController;
  setup: () => void;
}) {
  return (
    <div className="session-empty new-session-welcome">
      <span className="new-session-emblem" aria-hidden="true">
        <MessageSquarePlus size={23} />
      </span>
      <h1>New session</h1>
      <p>What would you like to work on?</p>
      <button
        className="new-session-context"
        onClick={(event) => {
          event.currentTarget.focus({ preventScroll: true });
          setup();
        }}
        aria-label="Change session setup"
        aria-haspopup="dialog"
        disabled={app.providerWorkspace?.isCreating}
      >
        <span title={app.project?.path}>
          <FolderOpen size={12} />
          {app.rootProject?.name ?? app.project?.name}
        </span>
        {app.currentWorktree && (
          <span>
            <GitBranch size={12} />
            {app.currentWorktree.branch ?? worktreeLabel(app.currentWorktree)}
          </span>
        )}
        <span>
          <Layers size={12} />
          {app.provider === "codex" ? "Codex" : "OpenCode"}
        </span>
      </button>
    </div>
  );
}
