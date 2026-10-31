import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { GitPullRequest, GitPullRequestDraft } from "lucide-react";

type PullRequest = {
  number: number;
  title: string;
  url: string;
  isDraft: boolean;
};
type Entry = {
  value: PullRequest | null;
  checkedAt: number;
  pending: boolean;
  listeners: Set<(value: PullRequest | null) => void>;
};
const cache = new Map<string, Entry>();
const refreshInterval = 60_000;

// A branch shared by project/session rows is queried only once per minute.
function refresh(projectPath: string, branch: string, entry: Entry) {
  if (entry.pending || Date.now() - entry.checkedAt < refreshInterval) return;
  entry.pending = true;
  entry.checkedAt = Date.now();
  void invoke<PullRequest | null>("github_open_pull_request", {
    projectPath,
    branch,
  })
    .catch(() => null)
    .then((value) => {
      entry.value = value;
      entry.pending = false;
      entry.listeners.forEach((listener) => listener(value));
    });
}

export function PullRequestIndicator({
  projectPath,
  branch,
}: {
  projectPath: string;
  branch: string;
}) {
  const element = useRef<HTMLSpanElement>(null);
  const key = `${projectPath}\0${branch}`;
  const [snapshot, setSnapshot] = useState<{
    key: string;
    value: PullRequest | null;
  }>();
  const pr = snapshot?.key === key ? snapshot.value : null;
  useEffect(() => {
    if (!isTauri()) return;
    const entry = cache.get(key) ?? {
      value: null,
      checkedAt: 0,
      pending: false,
      listeners: new Set(),
    };
    cache.set(key, entry);
    const receive = (value: PullRequest | null) => setSnapshot({ key, value });
    entry.listeners.add(receive);
    receive(
      Date.now() - entry.checkedAt < refreshInterval ? entry.value : null,
    );
    let visible = false;
    let timer: number | undefined;
    const update = () => {
      window.clearTimeout(timer);
      if (!visible || document.hidden) return;
      refresh(projectPath, branch, entry);
      timer = window.setTimeout(
        update,
        Math.max(1000, entry.checkedAt + refreshInterval - Date.now()),
      );
    };
    const observer = new IntersectionObserver(
      (entries) => {
        visible = entries.some((entry) => entry.isIntersecting);
        update();
      },
      { root: element.current?.closest(".session-list, .project-list") },
    );
    // Observe the branch row even while its optional icon has no layout box.
    if (element.current?.parentElement)
      observer.observe(element.current.parentElement);
    window.addEventListener("focus", update);
    document.addEventListener("visibilitychange", update);
    return () => {
      entry.listeners.delete(receive);
      observer.disconnect();
      window.clearTimeout(timer);
      window.removeEventListener("focus", update);
      document.removeEventListener("visibilitychange", update);
    };
  }, [projectPath, branch, key]);
  const label = pr
    ? `${pr.isDraft ? "Draft" : "Open"} pull request #${pr.number}: ${pr.title}`
    : undefined;
  const Icon = pr?.isDraft ? GitPullRequestDraft : GitPullRequest;
  return (
    <span ref={element} className="branch-pr" data-draft={pr?.isDraft}>
      {pr && (
        <span role="img" aria-label={label} title={label}>
          <Icon size={12} aria-hidden="true" />
        </span>
      )}
    </span>
  );
}
