import { useEffect, useId, useRef, useState } from "react";
import { GitPullRequest, GitPullRequestDraft } from "lucide-react";
import { copyAction, useContextMenu } from "./ContextMenu.js";
import { usePullRequestPreview } from "./PullRequestPreview.js";
import {
  refreshBranchPullRequests,
  useScopedBranchPullRequests,
  branchPullRequestKey,
} from "./useBranchPullRequests.js";
import type { PullRequest } from "./pullRequestState.js";
import type { ProjectSession } from "./workspaceState.js";
import type { WorkspaceController } from "./Sidebar.js";

type Session = ProjectSession & { title: string };

export function useSessionPullRequests(
  app: WorkspaceController,
  sessions: Session[],
) {
  const path = app.rootProject?.path ?? "";
  const sessionKey = JSON.stringify(
    sessions.map(({ key, updatedAt, available }) => [
      key,
      updatedAt,
      available,
    ]),
  );
  const load = useRef(app.loadSessionDetails);
  load.current = app.loadSessionDetails;
  const [loadingDetails, setLoadingDetails] = useState(false);
  useEffect(() => {
    let disposed = false;
    const pending = sessions.filter((session) => session.available);
    // Include off-screen and unselected sessions without resuming conversations.
    const collect = async () => {
      while (!disposed && pending.length) {
        const session = pending.shift()!;
        await load.current(
          session.project.path,
          session.provider,
          session.id,
        );
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
  const branchFor = (session: Session) =>
    app.workspaces[session.project.path]?.providers[session.provider]
      .sessionDetails[session.id]?.branch;
  const scopes = sessions.flatMap((session) => {
    const branch = branchFor(session);
    return branch && session.available
      ? [{ projectPath: session.project.path, branch }]
      : [];
  });
  const requests = useScopedBranchPullRequests(scopes);
  const entries = new Map<
    string,
    { projectPath: string; pr: PullRequest; sessions: Session[] }
  >();
  for (const session of sessions) {
    if (!session.available) continue;
    const branch = branchFor(session);
    const pr = branch
      ? requests[branchPullRequestKey(session.project.path, branch)]?.value
      : null;
    if (!pr) continue;
    const entry = entries.get(pr.url) ?? {
      projectPath: session.project.path,
      pr,
      sessions: [],
    };
    entry.sessions.push(session);
    entries.set(pr.url, entry);
  }
  const loading =
    app.isLoadingSessions ||
    loadingDetails ||
    scopes.some(
      ({ projectPath, branch }) =>
        !requests[branchPullRequestKey(projectPath, branch)] ||
        requests[branchPullRequestKey(projectPath, branch)].loading,
    );
  const error = Object.values(requests)
    .map((request) => request?.error)
    .find(Boolean);
  return {
    path,
    entries,
    loading,
    error,
    retry: () => {
      for (const projectPath of new Set(
        scopes.map((scope) => scope.projectPath),
      ))
        refreshBranchPullRequests(projectPath);
    },
  };
}

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
