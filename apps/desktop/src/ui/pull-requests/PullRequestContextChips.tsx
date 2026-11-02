import { useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import { GitPullRequest, X } from "lucide-react";
import type { PullRequestContext } from "../../domain/pull-requests/pullRequestContext.js";
import "./pullRequestContext.css";

export function PullRequestContextChips({
  pullRequests,
  remove,
  disabled = false,
}: {
  pullRequests: PullRequestContext[];
  remove?: (id: string) => void;
  disabled?: boolean;
}) {
  const [failed, setFailed] = useState<string | null>(null);
  if (!pullRequests.length) return null;
  return (
    <div
      className="pr-context-chips"
      aria-label={
        remove ? "Attached pull requests" : "Sent pull request context"
      }
    >
      {pullRequests.map((pr) => (
        <span className="pr-context-chip" key={pr.id}>
          <a
            href={pr.url}
            target="_blank"
            rel="noreferrer"
            title={
              failed === pr.url
                ? "Could not open link. Copy the address to your browser."
                : `${pr.title}\n${pr.head} → ${pr.base} · ${pr.commit.slice(0, 7)}\n${pr.checks === null ? "Checks unavailable" : pr.checks.map((check) => `${check.name}: ${check.status}`).join("\n") || "No checks"}`
            }
            onClick={(event) => {
              if (!isTauri()) return;
              event.preventDefault();
              void openUrl(pr.url).catch(() => setFailed(pr.url));
            }}
          >
            <GitPullRequest size={12} />
            <span>
              #{pr.number} · {pr.title}
            </span>
          </a>
          {remove && (
            <button
              type="button"
              aria-label={`Remove #${pr.number} context`}
              disabled={disabled}
              onClick={() => remove(pr.id)}
            >
              <X size={11} />
            </button>
          )}
        </span>
      ))}
    </div>
  );
}
