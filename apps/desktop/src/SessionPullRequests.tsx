import { useEffect, useId, useRef, useState } from "react";
import { GitPullRequest, GitPullRequestDraft } from "lucide-react";
import { copyAction, useContextMenu } from "./ContextMenu.js";
import { usePullRequestPreview } from "./PullRequestPreview.js";
import {
  refreshBranchPullRequests,
  useBranchPullRequests,
} from "./useBranchPullRequests.js";
import type { PullRequest } from "./pullRequestState.js";
import type { ProviderKind, SessionSummary } from "./workspaceState.js";
import type { WorkspaceController } from "./Sidebar.js";

type Session = SessionSummary & { provider: ProviderKind; title: string };

export function useSessionPullRequests(
  app: WorkspaceController,
  sessions: Session[],
) {
  const path = app.project?.path ?? "";
  const sessionKey = JSON.stringify(
    sessions.map(({ id, provider, updatedAt }) => [provider, id, updatedAt]),
  );
  const load = useRef(app.loadSessionDetails);
  load.current = app.loadSessionDetails;
  const [loadingDetails, setLoadingDetails] = useState(false);
  useEffect(() => {
    let disposed = false;
    const pending = [...sessions];
    // Include off-screen and unselected sessions without resuming conversations.
    const collect = async () => {
      while (!disposed && pending.length) {
        const session = pending.shift()!;
        await load.current(path, session.provider, session.id);
      }
    };
    setLoadingDetails(true);
    void Promise.all(Array.from({ length: 4 }, collect)).then(() => {
      if (!disposed) setLoadingDetails(false);
    });
    return () => {
      disposed = true;
    };
  }, [path, sessionKey]);
  const branches = sessions.flatMap((session) => {
    const branch =
      app.workspace?.providers[session.provider].sessionDetails[session.id]
        ?.branch;
    return branch ? [branch] : [];
  });
  const requests = useBranchPullRequests(path, branches);
  const entries = new Map<string, { pr: PullRequest; sessions: Session[] }>();
  for (const session of sessions) {
    const branch =
      app.workspace?.providers[session.provider].sessionDetails[session.id]
        ?.branch;
    const pr = branch ? requests[branch]?.value : null;
    if (!pr) continue;
    const entry = entries.get(pr.url) ?? { pr, sessions: [] };
    entry.sessions.push(session);
    entries.set(pr.url, entry);
  }
  const loading =
    loadingDetails ||
    branches.some((branch) => !requests[branch] || requests[branch].loading);
  const error = branches.map((branch) => requests[branch]?.error).find(Boolean);
  return { path, entries, loading, error };
}

export function SessionPullRequests({
  result,
}: {
  result: ReturnType<typeof useSessionPullRequests>;
}) {
  const { path, entries, loading, error } = result;
  return (
    <section className="session-pr-section" aria-label="Pull requests">
      <div className="session-pr-list">
        {entries.size
          ? [...entries.values()].map((entry) => (
              <SessionPullRequest
                key={`${path}\0${entry.pr.url}`}
                projectPath={path}
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
            <button
              type="button"
              disabled={loading}
              onClick={() => refreshBranchPullRequests(path)}
            >
              {loading ? "Retrying…" : "Retry"}
            </button>
          </div>
        )}
      </div>
    </section>
  );
}

function SessionPullRequest({
  projectPath,
  pr,
  sessions,
}: {
  projectPath: string;
  pr: PullRequest;
  sessions: Session[];
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
