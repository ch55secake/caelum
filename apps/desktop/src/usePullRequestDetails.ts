import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  checkStatus,
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
};
type Entry = {
  snapshot: Snapshot;
  checkedAt: number;
  revision: number;
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
};
const freshness = 60_000;

function entryFor(url: string) {
  let entry = cache.get(url);
  if (!entry) {
    for (const [key, value] of cache) {
      if (cache.size < 100) break;
      if (
        !value.listeners.size &&
        !value.snapshot.loading &&
        !value.snapshot.merging
      )
        cache.delete(key);
    }
    entry = {
      snapshot: { ...empty, loading: false },
      checkedAt: 0,
      revision: 0,
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
      publish(entry, {
        ...entry.snapshot,
        value,
        error: null,
        loading: false,
        mergeError: value.state === "MERGED" ? null : entry.snapshot.mergeError,
      });
    })
    .catch((reason) => {
      if (entry.revision === revision)
        publish(entry, {
          ...entry.snapshot,
          error: String(reason),
          loading: false,
        });
    })
    .finally(() => {
      if (entry.revision === revision) entry.checkedAt = Date.now();
    });
}

async function merge(projectPath: string, url: string, method: MergeMethod) {
  const entry = entryFor(url);
  const { value, loading, merging } = entry.snapshot;
  if (loading || merging || !value || !mergeStatus(value).enabled) return;
  entry.revision++;
  publish(entry, {
    ...entry.snapshot,
    merging: true,
    mergeError: null,
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
        refresh(projectPath, url, pending ? 15_000 : freshness);
    };
    update();
    const timer = window.setInterval(update, live ? 15_000 : freshness);
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
  };
}
