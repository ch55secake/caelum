import type { WorkspaceController } from "../application/workspace/controller.js";
import { PullRequestScreen } from "../ui/pull-requests/PullRequestPreview.js";
import type { WorkspaceSurfaces } from "./useWorkspaceSurfaces.js";

export function WorkspacePullRequest({
  app,
  title,
  surfaces,
  pendingInput,
  showSession,
}: {
  app: WorkspaceController;
  title: string;
  surfaces: WorkspaceSurfaces;
  pendingInput: boolean;
  showSession: () => void;
}) {
  const target = surfaces.activePullRequest;
  if (!target) return null;
  const disabledReason =
    app.checkoutUnavailable ||
    Boolean(app.rootProject && app.worktreeBusy[app.rootProject.path])
      ? "This checkout is unavailable or busy."
      : app.selected?.connection !== "connected"
        ? "Reconnect this session to address feedback."
        : app.selected.isWorking || app.selected.isSubmitting
          ? "Wait for the current turn to finish."
          : pendingInput
            ? "Answer the session's pending request first."
            : app.selectedId &&
                app.providerWorkspace?.archivingSessionIds.includes(
                  app.selectedId,
                )
              ? "This session is being archived."
              : null;
  return (
    <PullRequestScreen
      key={target.pr.url}
      target={target}
      back={showSession}
      session={
        app.selected && app.selectedId
          ? {
              name: title,
              disabledReason,
              address: async (prompt) => {
                const sent = await app.submit(prompt);
                if (
                  sent &&
                  surfaces.isCurrentSession() &&
                  surfaces.currentPullRequest.current?.pr.url === target.pr.url
                )
                  showSession();
                return sent;
              },
            }
          : undefined
      }
    />
  );
}
