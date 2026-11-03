import { FolderOpen, LoaderCircle } from "lucide-react";
import type { WorkspaceController } from "../application/workspace/controller.js";
import type { ApprovalPrompt } from "../application/workspace/useWorkspace.js";
import type { useSettings } from "../application/useSettings.js";
import type { useSessionPullRequests } from "../application/github/useSessionPullRequests.js";
import {
  questionKey,
  type QuestionPrompt,
} from "../domain/sessions/questionState.js";
import { ApprovalCard, Conversation } from "../ui/conversation/Conversation.js";
import { MessageQueue } from "../ui/composer/MessageQueue.js";
import { QuestionCard } from "../ui/sessions/QuestionCard.js";
import { NewSessionWelcome } from "../ui/sessions/NewSessionWelcome.js";
import { SessionComposer } from "./SessionComposer.js";
import type { WorkspaceFocus } from "./useWorkspaceFocus.js";
import type { WorkspaceSurfaces } from "./useWorkspaceSurfaces.js";

export function SessionWorkspace({
  app,
  preferences,
  pullRequests,
  sessionKey,
  surfaces,
  focus,
  activeApprovals,
  projectApprovals,
  activeQuestions,
  newSession,
  setupSession,
}: {
  app: WorkspaceController;
  preferences: ReturnType<typeof useSettings>;
  pullRequests: ReturnType<typeof useSessionPullRequests>;
  sessionKey: string;
  surfaces: WorkspaceSurfaces;
  focus: WorkspaceFocus;
  activeApprovals: ApprovalPrompt[];
  projectApprovals: ApprovalPrompt[];
  activeQuestions: QuestionPrompt[];
  newSession: () => void;
  setupSession: () => void;
}) {
  const sessionError = app.selected
    ? app.selected.error
    : app.providerWorkspace?.error;
  return (
    <div
      className="session-workspace"
      hidden={surfaces.expanded && Boolean(surfaces.pane)}
    >
      {sessionError && (
        <div className="error-banner" role="alert">
          <span>{sessionError}</span>
          {!app.selected && app.project && (
            <button
              disabled={app.providerWorkspace?.isCreating}
              onClick={() => void app.submit()}
            >
              Retry
            </button>
          )}
        </div>
      )}
      {app.selected?.connection === "disconnected" && (
        <div className="connection-banner">
          <span>Connection lost. Reconnect to continue this session.</span>
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
          resolve={(decision) => void app.resolveApproval(request, decision)}
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
          ref={focus.conversationRef}
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
              Boolean(app.selected?.historyLoaded && app.selected.nextCursor),
            )
          }
          approvals={activeApprovals}
          resolving={Boolean(app.resolving)}
          addToDraft={(text) => {
            const draft = app.selected?.draft ?? "";
            app.setDraft(draft ? `${draft}\n\n${text}` : text);
            focus.textareaRef.current?.focus();
          }}
          resolve={(request, decision) =>
            void app.resolveApproval(request, decision)
          }
        />
      ) : (
        <NewSessionWelcome app={app} setup={setupSession} />
      )}
      {app.selected && (
        <MessageQueue
          messages={app.selected.queuedMessages}
          paused={app.selected.queuePaused}
          working={app.selected.isWorking}
          disabled={
            app.selected.isSubmitting || app.selected.connection !== "connected"
          }
          remove={(id) => void app.removeQueued(id)}
          resume={() => void app.resumeQueue()}
        />
      )}
      {activeQuestions.length > 0 ? (
        <div
          ref={focus.questionDockRef}
          className="composer-wrap question-dock"
          aria-label="Agent questions"
        >
          <span className="sr-only" role="status" aria-label="Agent activity">
            Waiting for your answer
          </span>
          {activeQuestions.map((request) => {
            const key = questionKey(request);
            return (
              <QuestionCard
                key={key}
                request={request}
                draft={app.questionDraft(request)}
                setDraft={(draft) => app.setQuestionDraft(request, draft)}
                submitting={Boolean(app.questionEdits[key]?.submitting)}
                error={app.questionEdits[key]?.error}
                resolve={(answer) => void app.resolveQuestion(request, answer)}
              />
            );
          })}
        </div>
      ) : (
        <SessionComposer
          app={app}
          preferences={preferences}
          pullRequests={pullRequests}
          sessionKey={sessionKey}
          focus={focus}
          surfaces={surfaces}
          awaitingApproval={activeApprovals.length > 0}
          newSession={newSession}
        />
      )}
    </div>
  );
}
