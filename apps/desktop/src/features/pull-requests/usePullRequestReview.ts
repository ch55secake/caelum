import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { ReviewPage } from "./pullRequestReviewState.js";

type Kind = "files" | "discussion" | "reviews" | "threads" | "replies";
type Target = {
  projectPath: string;
  url: string;
  headOid: string;
  kind: Kind;
  threadId?: string;
  cursor?: string;
};
type Snapshot = {
  pages: ReviewPage[];
  version: number;
  loading: boolean;
  error: string | null;
};
type Entry = {
  snapshot: Snapshot;
  revision: number;
  checkedAt: number;
  listeners: Set<(value: Snapshot) => void>;
};
const cache = new Map<string, Entry>();
const empty: Snapshot = { pages: [], version: 0, loading: false, error: null };
const tasks: (() => Promise<void>)[] = [];
let active = 0;
function drain() {
  while (active < 4 && tasks.length) {
    active++;
    void tasks.shift()!().finally(() => {
      active--;
      drain();
    });
  }
}
function entryFor(key: string) {
  let entry = cache.get(key);
  if (!entry) {
    for (const [key, value] of cache) {
      if (cache.size < 60) break;
      if (!value.listeners.size && !value.snapshot.loading) cache.delete(key);
    }
    entry = {
      snapshot: empty,
      revision: 0,
      checkedAt: 0,
      listeners: new Set(),
    };
    cache.set(key, entry);
  }
  return entry;
}
function publish(entry: Entry, value: Snapshot) {
  entry.snapshot = value;
  entry.listeners.forEach((listener) => listener(value));
}
function load(target: Target, entry: Entry, more = false, force = false) {
  if (force) {
    entry.revision++;
    entry.checkedAt = 0;
  }
  if (entry.snapshot.loading) return;
  const last = entry.snapshot.pages.at(-1);
  if (more && !(last?.nextPage || last?.nextCursor)) return;
  if (!more && Date.now() - entry.checkedAt < 60_000) return;
  const revision = entry.revision;
  publish(entry, { ...entry.snapshot, loading: true, error: null });
  tasks.push(async () => {
    try {
      if (entry.revision !== revision) return;
      const { kind, cursor, ...args } = target;
      const value = await invoke<ReviewPage>(
        kind === "files"
          ? "github_pull_request_files"
          : "github_pull_request_feedback",
        {
          ...args,
          ...(kind === "files"
            ? { page: more ? last!.nextPage : 1 }
            : { kind, cursor: more ? last!.nextCursor : (cursor ?? null) }),
        },
      );
      if (entry.revision !== revision) return;
      if (
        more &&
        value.baseOid &&
        value.baseOid !== entry.snapshot.pages[0]?.baseOid
      )
        throw new Error(
          "The pull request base changed. Refresh the files before loading more.",
        );
      entry.checkedAt = Date.now();
      publish(entry, {
        pages: more ? [...entry.snapshot.pages, value] : [value],
        version: entry.snapshot.version + (more ? 0 : 1),
        loading: false,
        error: null,
      });
    } catch (reason) {
      if (entry.revision === revision) {
        entry.checkedAt = Date.now();
        publish(entry, {
          ...entry.snapshot,
          loading: false,
          error: String(reason),
        });
      }
    } finally {
      if (entry.revision !== revision) {
        publish(entry, { ...entry.snapshot, loading: false });
        load(target, entry);
      }
    }
  });
  drain();
}

export function usePullRequestReview(target: Target, enabled = true) {
  const key = JSON.stringify([
    target.url,
    target.headOid,
    target.kind,
    target.threadId,
    target.cursor,
  ]);
  const [result, setResult] = useState<{ key: string; snapshot: Snapshot }>();
  useEffect(() => {
    if (!enabled) return;
    const entry = entryFor(key);
    const receive = (snapshot: Snapshot) => setResult({ key, snapshot });
    entry.listeners.add(receive);
    receive(entry.snapshot);
    load(target, entry, false, target.kind === "replies");
    return () => {
      entry.listeners.delete(receive);
    };
  }, [key, enabled, target.projectPath]);
  const snapshot =
    result?.key === key ? result.snapshot : { ...empty, loading: enabled };
  return {
    ...snapshot,
    more: () => load(target, entryFor(key), true),
    refresh: () => load(target, entryFor(key), false, true),
    hasMore: Boolean(
      snapshot.pages.at(-1)?.nextPage || snapshot.pages.at(-1)?.nextCursor,
    ),
  };
}
