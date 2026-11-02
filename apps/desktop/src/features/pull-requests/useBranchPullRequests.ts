import { useEffect, useState, type RefObject } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { PullRequest } from "./pullRequestState.js";

type Snapshot = {
  value: PullRequest | null;
  loading: boolean;
  error: string | null;
};
type Entry = {
  snapshot: Snapshot;
  checkedAt: number;
  revision: number;
  listeners: Set<(snapshot: Snapshot) => void>;
};
const cache = new Map<string, Entry>();
const mergedUrls = new Set<string>();
const refreshInterval = 60_000;
const retryInterval = 15_000;
const queue: (() => Promise<void>)[] = [];
let active = 0;

export function markPullRequestMerged(url: string) {
  mergedUrls.add(url);
  for (const entry of cache.values()) {
    if (entry.snapshot.value?.url !== url) continue;
    entry.snapshot = { ...entry.snapshot, value: null };
    entry.listeners.forEach((listener) => listener(entry.snapshot));
  }
}

function drain() {
  while (active < 4 && queue.length) {
    const task = queue.shift()!;
    active++;
    void task().finally(() => {
      active--;
      drain();
    });
  }
}

// Indicators and the session PR list share discovery by checkout and branch.
function refresh(
  projectPath: string,
  branch: string,
  entry: Entry,
  force = false,
) {
  if (force) {
    entry.checkedAt = 0;
    entry.revision++;
  }
  if (
    entry.snapshot.loading ||
    Date.now() - entry.checkedAt <
      (entry.snapshot.error ? retryInterval : refreshInterval)
  )
    return;
  const publish = (snapshot: Snapshot) => {
    entry.snapshot = snapshot;
    entry.listeners.forEach((listener) => listener(snapshot));
  };
  publish({ ...entry.snapshot, loading: true });
  queue.push(async () => {
    if (!entry.listeners.size) {
      publish({ ...entry.snapshot, loading: false });
      return;
    }
    const revision = entry.revision;
    try {
      const value = await invoke<PullRequest | null>(
        "github_open_pull_request",
        { projectPath, branch },
      );
      if (entry.revision === revision) {
        entry.checkedAt = Date.now();
        publish({
          value: value && !mergedUrls.has(value.url) ? value : null,
          loading: false,
          error: null,
        });
      }
    } catch (reason) {
      if (entry.revision === revision) {
        entry.checkedAt = Date.now();
        publish({ ...entry.snapshot, loading: false, error: String(reason) });
      }
    } finally {
      if (entry.revision !== revision) {
        // Explicit refresh during an older request must discard its result and
        // perform a new lookup, still using the shared four-request queue.
        publish({ ...entry.snapshot, loading: false });
        refresh(projectPath, branch, entry);
      }
    }
  });
  drain();
}

export function refreshBranchPullRequests(
  projectPath: string,
  branches?: string[],
) {
  const prefix = `${projectPath}\0`;
  for (const [key, entry] of cache) {
    if (!key.startsWith(prefix)) continue;
    const branch = key.slice(prefix.length);
    if (!branches || branches.includes(branch))
      refresh(projectPath, branch, entry, true);
  }
}

export function useBranchPullRequests(
  projectPath: string,
  branches: string[],
  element?: RefObject<HTMLElement | null>,
) {
  const result = useScopedBranchPullRequests(
    branches.map((branch) => ({ projectPath, branch })),
    element,
  );
  return Object.fromEntries(
    branches.map((branch) => [
      branch,
      result[branchPullRequestKey(projectPath, branch)],
    ]),
  );
}

export const branchPullRequestKey = (projectPath: string, branch: string) =>
  `${projectPath}\0${branch}`;

export function useScopedBranchPullRequests(
  scopes: { projectPath: string; branch: string }[],
  element?: RefObject<HTMLElement | null>,
) {
  const scopeKey = JSON.stringify(
    [
      ...new Set(
        scopes
          .filter(({ projectPath }) => projectPath)
          .map(({ projectPath, branch }) =>
            branchPullRequestKey(projectPath, branch),
          ),
      ),
    ].sort(),
  );
  const [result, setResult] = useState<{
    scopeKey: string;
    values: Record<string, Snapshot>;
  }>();
  useEffect(() => {
    if (!isTauri()) return;
    let schedule = () => {};
    const keys: string[] = JSON.parse(scopeKey);
    const subscriptions = keys.map((key) => {
      const [projectPath, branch] = key.split("\0");
      const entry = cache.get(key) ?? {
        snapshot: { value: null, loading: false, error: null },
        checkedAt: 0,
        revision: 0,
        listeners: new Set(),
      };
      cache.set(key, entry);
      const receive = (snapshot: Snapshot) => {
        setResult((current) => ({
          scopeKey,
          values: {
            ...(current?.scopeKey === scopeKey ? current.values : {}),
            [key]: snapshot,
          },
        }));
        schedule();
      };
      entry.listeners.add(receive);
      receive(entry.snapshot);
      return { projectPath, branch, entry, receive };
    });
    let visible = !element;
    let timer: number | undefined;
    const update = () => {
      if (visible && !document.hidden)
        for (const { projectPath, branch, entry } of subscriptions)
          refresh(projectPath, branch, entry);
      schedule();
    };
    schedule = () => {
      window.clearTimeout(timer);
      if (!visible || document.hidden) return;
      const due = subscriptions
        .filter(({ entry }) => !entry.snapshot.loading)
        .map(
          ({ entry }) =>
            entry.checkedAt +
            (entry.snapshot.error ? retryInterval : refreshInterval),
        );
      if (due.length)
        timer = window.setTimeout(
          update,
          Math.max(1000, Math.min(...due) - Date.now()),
        );
    };
    const observer = element
      ? new IntersectionObserver(
          (entries) => {
            visible = entries.some((entry) => entry.isIntersecting);
            update();
          },
          { root: element.current?.closest(".session-list, .project-list") },
        )
      : null;
    // The branch row has a layout box even when its optional icon is absent.
    if (element?.current?.parentElement)
      observer?.observe(element.current.parentElement);
    update();
    window.addEventListener("focus", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      for (const { entry, receive } of subscriptions)
        entry.listeners.delete(receive);
      observer?.disconnect();
      window.clearTimeout(timer);
      window.removeEventListener("focus", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, [scopeKey, element]);
  return result?.scopeKey === scopeKey ? result.values : {};
}
