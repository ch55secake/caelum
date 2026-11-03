import { Timer } from "lucide-react";
import type { PromptRun } from "../../domain/sessions/activityState.js";
import { elapsedTime } from "../../application/useClock.js";

export function PromptTiming({ run }: { run: PromptRun }) {
  if (run.finishedAt === null) return null;
  const duration =
    run.outcome === "reconciled"
      ? "timing unavailable"
      : elapsedTime(run.finishedAt - run.startedAt);
  const label =
    {
      failed: "Failed",
      interrupted: "Interrupted",
      disconnected: "Disconnected",
    }[run.outcome ?? ""] ?? "Finished";
  return (
    <div
      className="prompt-timing composer-wrap"
      aria-label="Prompt timing"
      title="Elapsed time for the whole prompt, including tools and time waiting for input"
    >
      <Timer size={16} aria-hidden="true" />
      <span>
        {run.outcome === "completed"
          ? `Worked for ${duration}`
          : `${label} · ${duration}`}
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
