import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { PullRequestDetails } from "./pullRequestState.js";

type Snapshot = {
  value: PullRequestDetails | null;
  error: string | null;
  loading: boolean;
};
type Entry = {
  snapshot: Snapshot;
  checkedAt: number;
  listeners: Set<(snapshot: Snapshot) => void>;
};
// The canonical URL includes the host and repository, and is shared by worktrees.
const cache = new Map<string, Entry>();
const empty: Snapshot = { value: null, error: null, loading: true };
const freshness = 60_000;

function entryFor(url: string) {
  let entry = cache.get(url);
  if (!entry) {
    for (const [key, value] of cache) {
      if (cache.size < 100) break;
      if (!value.listeners.size && !value.snapshot.loading) cache.delete(key);
    }
    entry = {
      snapshot: { ...empty, loading: false },
      checkedAt: 0,
      listeners: new Set(),
    };
    cache.set(url, entry);
  }
  return entry;
}

function refresh(projectPath: string, url: string, force = false) {
  const entry = entryFor(url);
  if (
    entry.snapshot.loading ||
    (!force && Date.now() - entry.checkedAt < freshness)
  )
    return;
  const publish = (snapshot: Snapshot) => {
    entry.snapshot = snapshot;
    entry.listeners.forEach((listener) => listener(snapshot));
  };
  publish({ ...entry.snapshot, error: null, loading: true });
  void invoke<PullRequestDetails>("github_pull_request_details", {
    projectPath,
    url,
  })
    .then((value) => publish({ value, error: null, loading: false }))
    .catch((reason) =>
      publish({ ...entry.snapshot, error: String(reason), loading: false }),
    )
    .finally(() => {
      entry.checkedAt = Date.now();
    });
}

export function usePullRequestDetails(projectPath: string, url: string) {
  const [result, setResult] = useState<{ url: string; snapshot: Snapshot }>();
  useEffect(() => {
    const entry = entryFor(url);
    const receive = (snapshot: Snapshot) => setResult({ url, snapshot });
    entry.listeners.add(receive);
    receive(entry.snapshot);
    const update = () => {
      if (!document.hidden) refresh(projectPath, url);
    };
    update();
    const timer = window.setInterval(update, freshness);
    window.addEventListener("focus", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      entry.listeners.delete(receive);
      window.clearInterval(timer);
      window.removeEventListener("focus", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, [projectPath, url]);
  return {
    ...(result?.url === url ? result.snapshot : empty),
    refresh: () => refresh(projectPath, url, true),
  };
}
