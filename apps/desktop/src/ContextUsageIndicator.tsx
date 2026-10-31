import { useId } from "react";
import { contextWindow } from "./contextUsage.js";
import type { ContextUsage, ProviderChoices } from "./workspaceState.js";

const compactNumber = new Intl.NumberFormat(undefined, {
  notation: "compact",
  maximumFractionDigits: 1,
});

export function ContextUsageIndicator({
  usage,
  choices,
}: {
  usage: ContextUsage | null;
  choices: ProviderChoices | null;
}) {
  const descriptionId = useId();
  // The pending composer model may differ from the model that measured this
  // context. Only use the reported model's capacity for the percentage.
  const model = choices?.models.find((model) => model.id === usage?.model);
  const limit =
    contextWindow(usage?.contextWindow) ??
    contextWindow(model?.contextWindow);
  const percent =
    usage && limit ? Math.round((usage.usedTokens / limit) * 100) : null;
  const count = usage
    ? `${usage.usedTokens.toLocaleString()}${limit ? ` / ${limit.toLocaleString()}` : ""} tokens`
    : "Usage has not been reported by this provider yet.";

  return (
    <div
      className="context-usage"
      role={percent === null ? "group" : "meter"}
      aria-label="Context usage"
      aria-describedby={descriptionId}
      aria-valuemin={percent === null ? undefined : 0}
      aria-valuemax={percent === null ? undefined : limit!}
      aria-valuenow={
        percent === null ? undefined : Math.min(usage!.usedTokens, limit!)
      }
      aria-valuetext={
        percent === null ? undefined : `${percent}% used · ${count}`
      }
      tabIndex={0}
    >
      <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden="true">
        <circle
          className="context-ring-track"
          cx="8"
          cy="8"
          r="6"
          fill="none"
          strokeWidth="2"
        />
        {percent !== null && (
          <circle
            className="context-ring-fill"
            cx="8"
            cy="8"
            r="6"
            fill="none"
            strokeWidth="2"
            pathLength="100"
            strokeDasharray={`${Math.min(percent, 100)} 100`}
            transform="rotate(-90 8 8)"
          />
        )}
      </svg>
      <span>
        Context{" "}
        {percent !== null
          ? `${percent}%`
          : usage
            ? compactNumber.format(usage.usedTokens)
            : "—"}
      </span>
      <span
        id={descriptionId}
        className="context-usage-details"
        role="tooltip"
      >
        <strong>Context window</strong>
        <span>{count}</span>
        {usage && (
          <span>
            Latest reported model request, including cached input and output.
            Updates after each step.
          </span>
        )}
        {usage && !limit && <span>Context window size unavailable.</span>}
        {model && (
          <span>
            {model.name} · {model.id}
          </span>
        )}
      </span>
    </div>
  );
}
