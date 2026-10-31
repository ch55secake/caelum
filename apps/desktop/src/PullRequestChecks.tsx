import {
  Check,
  CircleHelp,
  ExternalLink,
  LoaderCircle,
  Minus,
  X,
} from "lucide-react";
import {
  checkStatus,
  checkSummary,
  type StatusCheck,
} from "./pullRequestState.js";

export function CheckStatusIcon({
  tone,
  size = 16,
}: {
  tone: string;
  size?: number;
}) {
  const Icon =
    tone === "success"
      ? Check
      : tone === "failure"
        ? X
        : tone === "pending"
          ? LoaderCircle
          : tone === "neutral"
            ? Minus
            : CircleHelp;
  return (
    <Icon
      size={size}
      aria-hidden="true"
      className={tone === "pending" ? "spinning" : undefined}
    />
  );
}

export function PullRequestChecks({
  checks,
  open,
}: {
  checks: StatusCheck[] | null;
  open: (url: string) => void;
}) {
  const summary = checkSummary(checks);
  return (
    <section className="pr-checks" aria-label="Pull request checks">
      <header className="pr-checks-heading">
        <h2>
          Checks <span>{checks?.length ?? 0}</span>
        </h2>
        <div className="pr-status" data-tone={summary.tone} role="status">
          <CheckStatusIcon tone={summary.tone} size={14} />
          <span>{summary.text}</span>
        </div>
      </header>
      {checks?.length ? (
        <ul className="pr-check-list">
          {checks.map((check, index) => {
            const result = checkStatus(check);
            return (
              <li className="pr-check" key={index}>
                <span className="pr-check-icon" data-tone={result.tone}>
                  <CheckStatusIcon tone={result.tone} />
                </span>
                <span className="pr-check-name">
                  {result.url ? (
                    <a
                      href={result.url}
                      onClick={(event) => {
                        event.preventDefault();
                        open(result.url!);
                      }}
                    >
                      {result.name}
                      <ExternalLink size={12} aria-hidden="true" />
                    </a>
                  ) : (
                    result.name
                  )}
                </span>
                <span className="pr-check-result" data-tone={result.tone}>
                  {result.text}
                </span>
              </li>
            );
          })}
        </ul>
      ) : (
        <p className="pr-check-empty">
          Checks will appear here when reported by GitHub.
        </p>
      )}
    </section>
  );
}
