import { useEffect, useRef, useState } from "react";
import {
  refreshBranchPullRequests,
  useScopedBranchPullRequests,
  branchPullRequestKey,
} from "./useBranchPullRequests.js";
import type { PullRequest } from "../../domain/pull-requests/pullRequestState.js";
import {
  sessionBranches,
  type ProjectSession,
} from "../../domain/sessions/workspaceState.js";
import type { WorkspaceController } from "../workspace/controller.js";

export type PullRequestSession = ProjectSession & { title: string };

export function useSessionPullRequests(
  app: Pick<
    WorkspaceController,
    "rootProject" | "loadSessionDetails" | "workspaces" | "isLoadingSessions"
  >,
  sessions: PullRequestSession[],
) {
  const path = app.rootProject?.path ?? "";
  // Membership drives discovery. Execution boundaries, Git events and explicit
  // refreshes already refresh metadata; token timestamps must not restart it.
  const sessionKey = JSON.stringify(
    sessions.map(({ key, available }) => [key, available]).sort(),
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
        await load.current(session.project.path, session.provider, session.id);
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
  const branchesFor = (session: PullRequestSession) =>
    sessionBranches(
      app.workspaces[session.project.path]?.providers[session.provider]
        .sessionDetails[session.id],
    );
  const scopes = sessions.flatMap((session) =>
    session.available
      ? branchesFor(session).map((branch) => ({
          projectPath: session.project.path,
          branch,
        }))
      : [],
  );
  const requests = useScopedBranchPullRequests(scopes, undefined, true);
  const associated = new Set<string>();
  const entries = new Map<
    string,
    { projectPath: string; pr: PullRequest; sessions: PullRequestSession[] }
  >();
  for (const session of sessions) {
    if (!session.available) continue;
    const pending = branchesFor(session);
    const seen = new Set<string>();
    while (pending.length) {
      const branch = pending.shift()!;
      if (seen.has(branch)) continue;
      seen.add(branch);
      const key = branchPullRequestKey(session.project.path, branch);
      associated.add(key);
      const snapshot = requests[key];
      // GitHub's explicit base relationship recovers switches missed between
      // local Git samples. Never infer ownership from unrelated repository PRs.
      if (snapshot?.baseRefName) pending.push(snapshot.baseRefName);
      const pr = snapshot?.value;
      if (!pr) continue;
      const entry = entries.get(pr.url) ?? {
        projectPath: session.project.path,
        pr,
        sessions: [],
      };
      if (!entry.sessions.some(({ key }) => key === session.key))
        entry.sessions.push(session);
      entries.set(pr.url, entry);
    }
  }
  const snapshots = [...associated].map((key) => requests[key]);
  const loading =
    app.isLoadingSessions ||
    loadingDetails ||
    snapshots.some((request) => !request || request.loading);
  const error = snapshots.map((request) => request?.error).find(Boolean);
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
