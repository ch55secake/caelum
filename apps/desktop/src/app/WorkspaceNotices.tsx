import { X } from "lucide-react";
import type { WorkspaceController } from "../application/workspace/controller.js";
import { sessionTitle } from "../domain/sessions/workspaceState.js";

export function WorkspaceNotices({ app }: { app: WorkspaceController }) {
  return (
    <>
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
      {app.activity.storageError && (
        <div className="error-banner" role="alert">
          <span>{app.activity.storageError}</span>
          <button onClick={app.activity.retrySave}>Retry</button>
        </div>
      )}
      {app.archivedSession && (
        <div
          className="archive-notice"
          role="status"
          aria-label="Session archived"
        >
          <span title={sessionTitle(app.archivedSession.thread)}>
            Archived <strong>{sessionTitle(app.archivedSession.thread)}</strong>
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
    </>
  );
}
