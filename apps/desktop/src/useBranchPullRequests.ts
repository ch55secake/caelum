import { useEffect, useState, type RefObject } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { PullRequest } from "./pullRequestState.js";

type Snapshot = { value: PullRequest | null; loading: boolean };
type Entry = {
  snapshot: Snapshot;
  checkedAt: number;
  listeners: Set<(snapshot: Snapshot) => void>;
};
const cache = new Map<string, Entry>();
const mergedUrls = new Set<string>();
const refreshInterval = 60_000;
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
function refresh(projectPath: string, branch: string, entry: Entry) {
  if (entry.snapshot.loading || Date.now() - entry.checkedAt < refreshInterval)
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
    entry.checkedAt = Date.now();
    const value = await invoke<PullRequest | null>("github_open_pull_request", {
      projectPath,
      branch,
    }).catch(() => null);
    publish({
      value: value && !mergedUrls.has(value.url) ? value : null,
      loading: false,
    });
  });
  drain();
}

export function useBranchPullRequests(
  projectPath: string,
  branches: string[],
  element?: RefObject<HTMLElement | null>,
) {
  const branchKey = JSON.stringify([...new Set(branches)].sort());
  const [result, setResult] = useState<{
    projectPath: string;
    values: Record<string, Snapshot>;
  }>();
  useEffect(() => {
    if (!projectPath || !isTauri()) return;
    const names: string[] = JSON.parse(branchKey);
    const subscriptions = names.map((branch) => {
      const key = `${projectPath}\0${branch}`;
      const entry = cache.get(key) ?? {
        snapshot: { value: null, loading: false },
        checkedAt: 0,
        listeners: new Set(),
      };
      cache.set(key, entry);
      const receive = (snapshot: Snapshot) =>
        setResult((current) => ({
          projectPath,
          values: {
            ...(current?.projectPath === projectPath ? current.values : {}),
            [branch]: snapshot,
          },
        }));
      entry.listeners.add(receive);
      receive(
        Date.now() - entry.checkedAt < refreshInterval
          ? entry.snapshot
          : { value: null, loading: true },
      );
      return { branch, entry, receive };
    });
    let visible = !element;
    let timer: number | undefined;
    const update = () => {
      window.clearTimeout(timer);
      if (!visible || document.hidden) return;
      for (const { branch, entry } of subscriptions)
        refresh(projectPath, branch, entry);
      if (subscriptions.length)
        timer = window.setTimeout(
          update,
          Math.max(
            1000,
            Math.min(...subscriptions.map(({ entry }) => entry.checkedAt)) +
              refreshInterval -
              Date.now(),
          ),
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
  }, [projectPath, branchKey, element]);
  return result?.projectPath === projectPath ? result.values : {};
}
