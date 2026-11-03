import type { WorkspaceController } from "../application/workspace/controller.js";
import type { useSettings } from "../application/useSettings.js";
import type { useSessionPullRequests } from "../application/github/useSessionPullRequests.js";
import { lastUserMessage } from "../domain/conversation/shellMessages.js";
import { Composer } from "../ui/composer/Composer.js";
import { SessionWorktree } from "../ui/projects/Worktrees.js";
import type { WorkspaceFocus } from "./useWorkspaceFocus.js";
import type { WorkspaceSurfaces } from "./useWorkspaceSurfaces.js";

export function SessionComposer({
  app,
  preferences,
  pullRequests,
  sessionKey,
  focus,
  surfaces,
  awaitingApproval,
  newSession,
}: {
  app: WorkspaceController;
  preferences: ReturnType<typeof useSettings>;
  pullRequests: ReturnType<typeof useSessionPullRequests>;
  sessionKey: string;
  focus: WorkspaceFocus;
  surfaces: WorkspaceSurfaces;
  awaitingApproval: boolean;
  newSession: () => void;
}) {
  if (!app.project) return null;
  return (
    <Composer
      worktreePicker={<SessionWorktree app={app} />}
      key={`composer-${sessionKey}`}
      projectPath={app.project.path}
      session={app.selected ?? null}
      loadTimeline={() =>
        app.loadHistory(
          app.project!.path,
          app.provider,
          app.selectedId!,
          Boolean(app.selected?.historyLoaded && app.selected.nextCursor),
        )
      }
      revealMessage={(id) => focus.conversationRef.current?.reveal(id)}
      onLayoutChange={() => focus.conversationRef.current?.restoreScroll()}
      textareaRef={focus.textareaRef}
      draft={app.selected?.draft ?? app.providerWorkspace?.draft ?? ""}
      setDraft={app.setDraft}
      fileContext={
        app.selected?.fileContext ?? app.providerWorkspace?.fileContext ?? []
      }
      removeFileContext={app.removeFileContext}
      pullRequests={pullRequests}
      pullRequestContext={app.selected?.pullRequestContext ?? []}
      removePullRequestContext={app.removePullRequestContext}
      addPullRequestContext={(pr) =>
        surfaces.isCurrentSession() && app.addPullRequestContext(pr)
      }
      addFileContext={(file) =>
        surfaces.isCurrentSession() && app.addFileContext(file)
      }
      lastMessage={lastUserMessage(app.selected?.conversation ?? [])}
      provider={app.provider}
      changeProvider={app.changeProvider}
      isNew={!app.selectedId}
      working={app.selected?.isWorking ?? false}
      activity={app.selected?.activity?.label}
      awaitingApproval={awaitingApproval}
      submitting={
        app.selected?.isSubmitting ||
        (!app.selectedId && app.providerWorkspace?.isCreating) ||
        false
      }
      disabled={
        app.checkoutUnavailable ||
        Boolean(app.rootProject && app.worktreeBusy[app.rootProject.path]) ||
        Boolean(
          app.selectedId &&
            (!app.selected ||
              app.providerWorkspace?.archivingSessionIds.includes(
                app.selectedId,
              ) ||
              app.selected.connection !== "connected"),
        ) ||
        Boolean(!app.selectedId && app.providerWorkspace?.isCreating)
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
      favouriteModels={preferences.settings.favoriteModels[app.provider]}
      toggleFavouriteModel={(id) =>
        preferences.toggleModelFavourite(app.provider, id)
      }
      effortPending={app.selected?.effort != null}
      agent={app.options.agent}
      currentAgent={app.selected?.session.agent ?? null}
      setOption={app.setOption}
      retryChoices={() => void app.loadChoices(app.project!, app.provider)}
    />
  );
}
