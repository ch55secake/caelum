import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Gauge, RefreshCw, X } from "lucide-react";
import { contextWindow } from "./contextUsage.js";
import type { ContextUsage, ProviderChoices } from "./workspaceState.js";

type UsageMeter = { percent: number; limited: boolean; resetsAt: number };
type UsageLimits = {
  workspace: { id: string; name?: string; usageURL: string };
  go?: { rolling: UsageMeter; weekly: UsageMeter; monthly: UsageMeter };
  budgets: {
    id: string;
    limit: number;
    spent: number;
    shared: boolean;
    resetsAt: number;
    scope: "each" | "combined";
    per: "set" | "model";
    coverage: "all" | "models";
    models: {
      providerID: string;
      modelID: string;
      name: string;
      spent?: number;
    }[];
    vendors: string[];
  }[];
};
const currency = new Intl.NumberFormat(undefined, {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 2,
});
const percent = new Intl.NumberFormat(undefined, { maximumFractionDigits: 1 });
const windows = [
  { id: "rolling", name: "Rolling window" },
  { id: "weekly", name: "Weekly" },
  { id: "monthly", name: "Monthly" },
] as const;

function ResetTime({ at, now }: { at: number; now: number }) {
  const date = new Date(at);
  const minutes = Math.ceil((at - now) / 60_000);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);
  const remaining =
    minutes <= 0
      ? "Reset time passed"
      : minutes < 60
        ? `Resets in ${minutes}m`
        : hours < 24
          ? `Resets in ${hours}h${minutes % 60 ? ` ${minutes % 60}m` : ""}`
          : days < 7
            ? `Resets in ${days}d${hours % 24 ? ` ${hours % 24}h` : ""}`
            : `Resets ${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })}`;
  return (
    <time
      className="usage-reset"
      dateTime={date.toISOString()}
      title={date.toLocaleString()}
      aria-label={`Reset time: ${date.toLocaleString()}`}
    >
      {remaining}
    </time>
  );
}

function GoAllowance({
  id,
  name,
  meter,
  now,
}: {
  id: string;
  name: string;
  meter: UsageMeter;
  now: number;
}) {
  return (
    <div className="usage-allowance" data-limited={meter.limited}>
      <h3>{name}</h3>
      <div className="usage-amount">
        <strong>
          {percent.format(meter.percent)}
          <span>%</span>
        </strong>
        <span>used</span>
      </div>
      <progress
        className="usage-progress"
        aria-label={`OpenCode Go ${id} allowance used`}
        max={100}
        value={meter.percent}
      />
      <p className="usage-remaining">
        {meter.limited
          ? "Limit reached"
          : `${percent.format(Math.max(0, 100 - meter.percent))}% remaining`}
      </p>
      <ResetTime at={meter.resetsAt} now={now} />
    </div>
  );
}

function ConsoleBudget({
  budget,
  now,
}: {
  budget: UsageLimits["budgets"][number];
  now: number;
}) {
  const name =
    budget.coverage === "all"
      ? "All models"
      : [...budget.models.map((item) => item.name), ...budget.vendors].join(
          ", ",
        ) || "Model budget";
  return (
    <div className="usage-budget">
      <div className="usage-budget-heading">
        <h3>{name}</h3>
        <span className="usage-badge">
          {budget.shared ? "Shared" : "Personal"}
        </span>
      </div>
      <div className="usage-budget-details">
        <p>
          <strong>{currency.format(budget.spent)}</strong> /{" "}
          {currency.format(budget.limit)}
        </p>
        <ResetTime at={budget.resetsAt} now={now} />
      </div>
      {budget.limit > 0 && (
        <progress
          className="usage-progress"
          aria-label={`${name} Console budget spent`}
          max={budget.limit}
          value={Math.min(budget.spent, budget.limit)}
          aria-valuetext={`${currency.format(budget.spent)} of ${currency.format(budget.limit)} spent`}
        />
      )}
      {budget.per === "model" &&
        budget.models
          .filter((item) => item.spent != null)
          .map((item) => (
            <div
              className="usage-budget-model"
              key={`${item.providerID}/${item.modelID}`}
            >
              <span title={`${item.providerID}/${item.modelID}`}>
                {item.name}
              </span>
              <span>{currency.format(item.spent!)}</span>
            </div>
          ))}
    </div>
  );
}

export function UsageDialog({
  project,
  usage,
  choices,
  close,
}: {
  project: string;
  usage: ContextUsage | null;
  choices: ProviderChoices | null;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [result, setResult] = useState<{
    project: string;
    limits: UsageLimits | null;
    updatedAt: number;
  } | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(timer);
  }, []);
  useEffect(() => {
    const element = dialog.current!;
    const previous = document.activeElement;
    element.showModal();
    return () => {
      element.close();
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus({ preventScroll: true });
    };
  }, []);
  useEffect(() => {
    let disposed = false;
    setLoading(true);
    setError(null);
    void invoke<UsageLimits | null>("opencode_usage_limits", {
      projectPath: project,
    })
      .then((value) => {
        if (!disposed) {
          const updatedAt = Date.now();
          setResult({ project, limits: value, updatedAt });
          setNow(updatedAt);
        }
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [project, refresh]);
  const model = choices?.models.find((item) => item.id === usage?.model);
  const capacity =
    contextWindow(usage?.contextWindow) ?? contextWindow(model?.contextWindow);
  const snapshot = result?.project === project ? result : null;
  const limits = snapshot?.limits;
  return (
    <dialog
      className="settings-dialog usage-dialog"
      ref={dialog}
      aria-labelledby="usage-title"
      aria-describedby="usage-source"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <header className="settings-heading usage-heading">
        <span className="usage-icon" aria-hidden="true">
          <Gauge size={20} />
        </span>
        <div>
          <h1 id="usage-title">OpenCode Go usage</h1>
          <p id="usage-source">
            Subscription allowances. Codex usage isn’t included.
          </p>
        </div>
        <button
          className="icon-button"
          aria-label="Close usage"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="usage-content">
        <section aria-label="OpenCode Go allowances" aria-busy={loading}>
          <div className="usage-section-heading">
            <h2>Plan allowances</h2>
            {limits && (
              <span
                className="usage-workspace"
                title={limits.workspace.name ?? limits.workspace.id}
              >
                {limits.workspace.name ?? limits.workspace.id}
              </span>
            )}
          </div>
          {error && (
            <div className="usage-error" role="alert">
              <strong>
                Couldn’t {snapshot ? "refresh" : "load"} OpenCode Go usage.
              </strong>
              <p>{error}</p>
              {snapshot && <p>Showing the last reported usage.</p>}
            </div>
          )}
          {!snapshot && loading ? (
            <div className="usage-empty" role="status">
              <RefreshCw size={15} className="spinning" /> Loading OpenCode Go
              usage…
            </div>
          ) : (
            snapshot &&
            (limits?.go ? (
              <div className="usage-allowances">
                {windows.map(({ id, name }) => (
                  <GoAllowance
                    key={id}
                    id={id}
                    name={name}
                    meter={limits.go![id]}
                    now={now}
                  />
                ))}
              </div>
            ) : (
              <div className="usage-empty">
                <p>No OpenCode Go allowances reported.</p>
                <span>
                  OpenCode hasn’t returned Go subscription limits for this
                  workspace.
                </span>
              </div>
            ))
          )}
        </section>
        {Boolean(limits?.budgets.length) && (
          <section aria-label="OpenCode Console budgets">
            <div className="usage-section-heading">
              <h2>OpenCode Console budgets</h2>
              <span>Workspace spending</span>
            </div>
            {limits!.budgets.map((budget) => (
              <ConsoleBudget key={budget.id} budget={budget} now={now} />
            ))}
          </section>
        )}
        <section
          className="usage-context"
          aria-label="OpenCode session context"
        >
          <div className="usage-section-heading">
            <h2>Session context</h2>
            {usage?.model && (
              <span className="usage-model" title={usage.model}>
                {model?.name ?? usage.model}
              </span>
            )}
          </div>
          {usage ? (
            <>
              <div className="usage-context-summary">
                <p>
                  <strong>{usage.usedTokens.toLocaleString()}</strong>
                  {capacity ? ` / ${capacity.toLocaleString()}` : ""} tokens
                </p>
                {capacity && (
                  <span className="usage-badge">
                    {percent.format((usage.usedTokens / capacity) * 100)}% of
                    context
                  </span>
                )}
              </div>
              <p className="usage-caption">
                Latest OpenCode model request, including cached input and
                output.{!capacity && " Context window size unavailable."}
              </p>
            </>
          ) : (
            <p className="usage-caption">
              No context usage reported for this session yet.
            </p>
          )}
        </section>
      </div>
      <footer className="settings-footer usage-footer">
        <span>
          {snapshot && (
            <>
              Updated{" "}
              <time dateTime={new Date(snapshot.updatedAt).toISOString()}>
                {new Date(snapshot.updatedAt).toLocaleTimeString(undefined, {
                  hour: "numeric",
                  minute: "2-digit",
                })}
              </time>
            </>
          )}
        </span>
        <button
          disabled={loading}
          onClick={() => setRefresh((value) => value + 1)}
        >
          <RefreshCw size={13} className={loading ? "spinning" : undefined} />{" "}
          {loading ? "Refreshing…" : error ? "Retry usage" : "Refresh usage"}
        </button>
        <button onClick={close}>Done</button>
      </footer>
    </dialog>
  );
}
