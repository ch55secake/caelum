import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RefreshCw, X } from "lucide-react";
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
const money = (value: number) =>
  new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 2,
  }).format(value);
const resetTime = (value: number) => new Date(value).toLocaleString();

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
  const [limits, setLimits] = useState<UsageLimits | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [refresh, setRefresh] = useState(0);
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
    setLimits(null);
    void invoke<UsageLimits | null>("opencode_usage_limits", {
      projectPath: project,
    })
      .then((value) => {
        if (!disposed) setLimits(value);
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
  return (
    <dialog
      className="settings-dialog usage-dialog"
      ref={dialog}
      aria-labelledby="usage-title"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <header className="settings-heading">
        <h1 id="usage-title">OpenCode usage</h1>
        <button
          className="icon-button"
          aria-label="Close usage"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="settings-content usage-content">
        <section aria-label="Session context">
          <h2>Session context</h2>
          <p>
            {usage
              ? `${usage.usedTokens.toLocaleString()}${capacity ? ` / ${capacity.toLocaleString()}` : ""} tokens`
              : "No context usage reported for this session yet."}
          </p>
          {usage && (
            <p className="settings-note">
              Latest model request, including cached input and output.
              {!capacity && " Context window size unavailable."}
            </p>
          )}
          {usage?.model && (
            <p className="settings-note">{model?.name ?? usage.model}</p>
          )}
        </section>
        <section aria-label="OpenCode allowances and budgets">
          <h2>Allowances and budgets</h2>
          {loading ? (
            <p role="status">Loading usage…</p>
          ) : error ? (
            <p role="alert" className="settings-provider-error">
              {error}
            </p>
          ) : !limits ? (
            <p className="settings-note">
              No OpenCode Console allowances reported. Limits for other
              providers aren’t exposed by this API.
            </p>
          ) : (
            <>
              <p>{limits.workspace.name ?? limits.workspace.id}</p>
              {limits.go &&
                Object.entries(limits.go).map(([name, meter]) => (
                  <div className="usage-meter" key={name}>
                    <div>
                      <strong>
                        {name.charAt(0).toUpperCase() + name.slice(1)} allowance
                      </strong>
                      <span>
                        {meter.percent}% used
                        {meter.limited ? " · Limit reached" : ""}
                      </span>
                    </div>
                    <progress
                      aria-label={`${name} allowance used`}
                      max={100}
                      value={meter.percent}
                    />
                    <small>Resets {resetTime(meter.resetsAt)}</small>
                  </div>
                ))}
              {limits.budgets.map((budget) => (
                <div className="usage-budget" key={budget.id}>
                  <strong>
                    {budget.coverage === "all"
                      ? "All models"
                      : [
                          ...budget.models.map((item) => item.name),
                          ...budget.vendors,
                        ].join(", ") || "Model budget"}
                  </strong>
                  <p>
                    {money(budget.spent)} / {money(budget.limit)}
                  </p>
                  <small>
                    {budget.shared ? "Shared" : "Personal"} budget · Resets{" "}
                    {resetTime(budget.resetsAt)}
                  </small>
                  {budget.per === "model" &&
                    budget.models
                      .filter((item) => item.spent != null)
                      .map((item) => (
                        <p
                          className="settings-note"
                          key={`${item.providerID}/${item.modelID}`}
                        >
                          {item.name}: {money(item.spent!)}
                        </p>
                      ))}
                </div>
              ))}
              {!limits.go && !limits.budgets.length && (
                <p className="settings-note">
                  No allowances or budgets reported.
                </p>
              )}
            </>
          )}
        </section>
      </div>
      <footer className="settings-footer">
        <button
          disabled={loading}
          onClick={() => setRefresh((value) => value + 1)}
        >
          <RefreshCw size={13} /> {error ? "Retry usage" : "Refresh usage"}
        </button>
        <button onClick={close}>Done</button>
      </footer>
    </dialog>
  );
}
