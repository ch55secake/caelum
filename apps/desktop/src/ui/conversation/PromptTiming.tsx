import { Timer } from "lucide-react";
import type { PromptRun } from "../../domain/sessions/activityState.js";
import { elapsedTime, useClock } from "../../application/useClock.js";

export function PromptTiming({
  run,
  waiting,
}: {
  run: PromptRun;
  waiting: boolean;
}) {
  const active = run.finishedAt === null;
  const now = useClock(active);
  const label = active
    ? waiting
      ? "Waiting for input"
      : "Working"
    : ({
        completed: "Completed",
        failed: "Failed",
        interrupted: "Interrupted",
        disconnected: "Disconnected",
        reconciled: "Finished",
      }[run.outcome ?? ""] ?? "Finished");
  return (
    <div
      className="prompt-timing composer-wrap"
      aria-label="Prompt timing"
      title="Elapsed time for the whole prompt, including tools and time waiting for input"
    >
      <Timer size={16} aria-hidden="true" />
      <span>
        {label} ·{" "}
        {run.outcome === "reconciled"
          ? "timing unavailable"
          : elapsedTime((run.finishedAt ?? now) - run.startedAt)}
      </span>
    </div>
  );
}

export function PromptQueueWait({ queuedMs }: { queuedMs: number }) {
  return (
    <div className="prompt-wait composer-wrap" aria-label="Prompt queue time">
      Queued for {elapsedTime(queuedMs)}
    </div>
  );
}
