import { useClock } from "../../application/useClock.js";
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
  checkTiming,
  type StatusCheck,
} from "../../domain/pull-requests/pullRequestState.js";

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
  const now = useClock(
    (checks ?? []).some((check) => checkTiming(check, Date.now())?.running),
  );
  const timings = (checks ?? []).map((check) => checkTiming(check, now));
  const summary = checkSummary(checks);
  return (
    <section className="pr-checks" aria-label="Pull request checks">
      <header className="pr-checks-heading">
        <h2>
          <span className="pr-checks-icon" data-tone={summary.tone}>
            <CheckStatusIcon tone={summary.tone} size={14} />
          </span>
          Checks <span className="pr-checks-count">{checks?.length ?? 0}</span>
        </h2>
        <div className="pr-checks-summary" role="status">
          {summary.text}
        </div>
      </header>
      {checks?.length ? (
        <ul className="pr-check-list">
          {checks.map((check, index) => {
            const result = checkStatus(check);
            const timing = timings[index];
            return (
              <li className="pr-check" key={index}>
                <span className="pr-check-icon" data-tone={result.tone}>
                  <CheckStatusIcon tone={result.tone} size={14} />
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
                {timing ? (
                  <time
                    className="pr-check-duration"
                    dateTime={`PT${timing.seconds}S`}
                    title={`${timing.running ? "Elapsed time" : "Duration"}: ${timing.text}`}
                    aria-label={`${timing.running ? "Elapsed time" : "Duration"}: ${timing.text}`}
                  >
                    {timing.text}
                  </time>
                ) : (
                  <span
                    className="pr-check-duration"
                    title="Timing unavailable"
                    aria-label="Timing unavailable"
                  >
                    —
                  </span>
                )}
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
