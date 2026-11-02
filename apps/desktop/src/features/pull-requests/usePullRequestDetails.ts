import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  checkStatus,
  canUpdateBranch,
  mergeStatus,
  type MergeMethod,
  type PullRequestDetails,
} from "./pullRequestState.js";
import { markPullRequestMerged } from "./useBranchPullRequests.js";

type Snapshot = {
  value: PullRequestDetails | null;
  error: string | null;
  loading: boolean;
  merging: boolean;
  mergeError: string | null;
  mergeMethod: MergeMethod | null;
  updating: boolean;
  updateError: string | null;
};
type Entry = {
  snapshot: Snapshot;
  checkedAt: number;
  revision: number;
  update: { headOid: string; startedAt: number } | null;
  listeners: Set<(snapshot: Snapshot) => void>;
};
// The canonical URL includes the host and repository, and is shared by worktrees.
const cache = new Map<string, Entry>();
const empty: Snapshot = {
  value: null,
  error: null,
  loading: true,
  merging: false,
  mergeError: null,
  mergeMethod: null,
  updating: false,
  updateError: null,
};
const freshness = 60_000;
const updateTimeout = 90_000;
const updateTimeoutMessage =
  "GitHub has not confirmed the branch update yet. Refresh the pull request to check its status before trying again.";

function entryFor(url: string) {
  let entry = cache.get(url);
  if (!entry) {
    for (const [key, value] of cache) {
      if (cache.size < 100) break;
      if (
        !value.listeners.size &&
        !value.snapshot.loading &&
        !value.snapshot.merging &&
        !value.snapshot.updating
      )
        cache.delete(key);
    }
    entry = {
      snapshot: { ...empty, loading: false },
      checkedAt: 0,
      revision: 0,
      update: null,
      listeners: new Set(),
    };
    cache.set(url, entry);
  }
  return entry;
}

function publish(entry: Entry, snapshot: Snapshot) {
  entry.snapshot = snapshot;
  entry.listeners.forEach((listener) => listener(snapshot));
}

function refresh(projectPath: string, url: string, maxAge = freshness) {
  const entry = entryFor(url);
  if (
    entry.snapshot.loading ||
    entry.snapshot.merging ||
    (entry.snapshot.updating && !entry.update) ||
    Date.now() - entry.checkedAt < maxAge
  )
    return;
  const revision = ++entry.revision;
  publish(entry, { ...entry.snapshot, error: null, loading: true });
  void invoke<PullRequestDetails>("github_pull_request_details", {
    projectPath,
    url,
  })
    .then((value) => {
      if (entry.revision !== revision) return;
      // A confirmed merge cannot be undone by an eventually consistent read.
      if (entry.snapshot.value?.state === "MERGED")
        value = { ...value, state: "MERGED" };
      const waiting =
        entry.update &&
        value.state === "OPEN" &&
        value.headRefOid === entry.update.headOid;
      const timedOut =
        waiting && Date.now() - entry.update!.startedAt >= updateTimeout;
      if (!waiting || timedOut) entry.update = null;
      publish(entry, {
        ...entry.snapshot,
        value,
        error: null,
        loading: false,
        mergeError: value.state === "MERGED" ? null : entry.snapshot.mergeError,
        updating: Boolean(entry.update),
        updateError: timedOut
          ? updateTimeoutMessage
          : entry.snapshot.updateError,
      });
    })
    .catch((reason) => {
      if (entry.revision === revision) {
        const timedOut =
          entry.update && Date.now() - entry.update.startedAt >= updateTimeout;
        if (timedOut) entry.update = null;
        publish(entry, {
          ...entry.snapshot,
          error: String(reason),
          loading: false,
          updating: Boolean(entry.update),
          updateError: timedOut
            ? updateTimeoutMessage
            : entry.snapshot.updateError,
        });
      }
    })
    .finally(() => {
      if (entry.revision === revision) entry.checkedAt = Date.now();
    });
}

async function merge(projectPath: string, url: string, method: MergeMethod) {
  const entry = entryFor(url);
  const { value, loading, merging, updating } = entry.snapshot;
  if (loading || merging || updating || !value || !mergeStatus(value).enabled)
    return;
  entry.revision++;
  publish(entry, {
    ...entry.snapshot,
    merging: true,
    mergeError: null,
    updateError: null,
    mergeMethod: method,
  });
  try {
    await invoke("github_merge_pull_request", {
      projectPath,
      url,
      method,
      headOid: value.headRefOid,
    });
    entry.checkedAt = Date.now();
    markPullRequestMerged(url);
    publish(entry, {
      ...entry.snapshot,
      value: { ...value, state: "MERGED" },
      error: null,
      merging: false,
    });
  } catch (reason) {
    publish(entry, {
      ...entry.snapshot,
      merging: false,
      mergeError: String(reason),
    });
    refresh(projectPath, url, 0);
  }
}

async function updateBranch(projectPath: string, url: string) {
  const entry = entryFor(url);
  const { value, loading, merging, updating } = entry.snapshot;
  if (loading || merging || updating || !value || !canUpdateBranch(value))
    return;
  entry.revision++;
  publish(entry, {
    ...entry.snapshot,
    updating: true,
    updateError: null,
    mergeError: null,
  });
  try {
    await invoke("github_update_pull_request_branch", {
      projectPath,
      url,
      headOid: value.headRefOid,
    });
    // The API queues the update. Keep both mutation controls locked until a
    // fresh detail response observes the new head (or the bounded wait expires).
    entry.update = { headOid: value.headRefOid, startedAt: Date.now() };
  } catch (reason) {
    publish(entry, {
      ...entry.snapshot,
      updating: false,
      updateError: String(reason),
    });
  }
  refresh(projectPath, url, 0);
}

export function usePullRequestDetails(
  projectPath: string,
  url: string,
  live = false,
) {
  const [result, setResult] = useState<{ url: string; snapshot: Snapshot }>();
  useEffect(() => {
    const entry = entryFor(url);
    const receive = (snapshot: Snapshot) => setResult({ url, snapshot });
    entry.listeners.add(receive);
    receive(entry.snapshot);
    const update = () => {
      const value = entry.snapshot.value;
      const pending =
        live &&
        value?.state === "OPEN" &&
        (value.mergeable === "UNKNOWN" ||
          value.mergeStateStatus === "UNKNOWN" ||
          value.statusCheckRollup?.some(
            (check) => checkStatus(check).tone === "pending",
          ));
      if (!document.hidden)
        refresh(
          projectPath,
          url,
          entry.update ? 5_000 : pending ? 15_000 : freshness,
        );
    };
    update();
    const timer = window.setInterval(update, live ? 5_000 : freshness);
    window.addEventListener("focus", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      entry.listeners.delete(receive);
      window.clearInterval(timer);
      window.removeEventListener("focus", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, [projectPath, url, live]);
  return {
    ...(result?.url === url ? result.snapshot : empty),
    refresh: () => refresh(projectPath, url, 0),
    merge: (method: MergeMethod) => merge(projectPath, url, method),
    updateBranch: () => updateBranch(projectPath, url),
  };
}
