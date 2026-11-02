import { useId } from "react";
import { GitPullRequest, GitPullRequestDraft } from "lucide-react";
import { copyAction, useContextMenu } from "../primitives/ContextMenu.js";
import { usePullRequestPreview } from "./PullRequestPreview.js";
import type { useSessionPullRequests, PullRequestSession } from "../../application/github/useSessionPullRequests.js";
import type { PullRequest } from "../../domain/pull-requests/pullRequestState.js";

export function SessionPullRequests({
  result,
}: {
  result: ReturnType<typeof useSessionPullRequests>;
}) {
  const { path, entries, loading, error, retry } = result;
  return (
    <div className="session-pr-section">
      <div className="session-pr-list">
        {entries.size
          ? [...entries.values()].map((entry) => (
              <SessionPullRequest
                key={`${path}\0${entry.pr.url}`}
                {...entry}
              />
            ))
          : !error && (
              <div className="empty-sessions">
                {loading
                  ? "Finding pull requests…"
                  : "No pull requests for open sessions"}
              </div>
            )}
        {error && (
          <div className="session-pr-error" role="status" title={error}>
            <span>
              {entries.size
                ? "Could not refresh all PRs. Showing last known results."
                : "Could not load pull requests."}
            </span>
            <button type="button" disabled={loading} onClick={retry}>
              {loading ? "Retrying…" : "Retry"}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function SessionPullRequest({
  projectPath,
  pr,
  sessions,
}: {
  projectPath: string;
  pr: PullRequest;
  sessions: PullRequestSession[];
}) {
  const preview = usePullRequestPreview();
  const menu = useContextMenu();
  const id = useId();
  const Icon = pr.isDraft ? GitPullRequestDraft : GitPullRequest;
  return (
    <button
      type="button"
      className="session-pr-item"
      data-draft={pr.isDraft}
      aria-label={`View ${pr.isDraft ? "draft " : ""}pull request #${pr.number}: ${pr.title}`}
      aria-describedby={id}
      aria-current={preview.activeUrl === pr.url ? "page" : undefined}
      title={`${pr.title}\n${sessions.map((session) => session.title).join("\n")}`}
      onClick={() => preview.open({ projectPath, pr })}
      onContextMenu={(event) =>
        menu(event, [
          copyAction("Copy pull request URL", pr.url),
          copyAction("Copy pull request title", pr.title),
        ])
      }
    >
      <Icon size={14} aria-hidden="true" />
      <span className="session-pr-text">
        <span className="session-pr-title">{pr.title}</span>
        <span id={id} className="session-pr-context">
          #{pr.number} ·{" "}
          {sessions.length === 1
            ? sessions[0].title
            : `${sessions.length} sessions`}
        </span>
      </span>
    </button>
  );
}
