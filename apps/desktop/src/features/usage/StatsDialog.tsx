import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BarChart3, X } from "lucide-react";
import "./stats.css";

type Tokens = {
  input: number;
  output: number;
  reasoning: number;
  cache: { read: number; write: number };
};
type Stats = {
  range: { from: number; to: number };
  sessions: number;
  subagents: number;
  prompts: number;
  steps: number;
  tokens: Tokens;
  cost: number;
  activeDays: number;
  streak: number;
  activity: { date: string; steps: number }[];
  models: {
    model: { id: string; providerID: string; variant?: string };
    steps: number;
    tokens: Tokens;
    cost: number;
  }[];
};
type Project = { id: string; name?: string; canonical: string };
const count = (value: number) => value.toLocaleString();
const total = (tokens: Tokens) =>
  tokens.input +
  tokens.output +
  tokens.reasoning +
  tokens.cache.read +
  tokens.cache.write;
const money = (value: number) =>
  value.toLocaleString(undefined, {
    style: "currency",
    currency: "USD",
    maximumFractionDigits: 4,
  });
function boundary(date: string, nextDay = false) {
  if (!date) return null;
  const value = new Date(`${date}T00:00:00`);
  if (nextDay) value.setDate(value.getDate() + 1);
  return value.getTime();
}

export function StatsDialog({
  project,
  close,
}: {
  project: string;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [scope, setScope] = useState("");
  const [from, setFrom] = useState("");
  const [to, setTo] = useState("");
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsError, setProjectsError] = useState<string | null>(null);
  const [result, setResult] = useState<{ key: string; data: Stats } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refresh, setRefresh] = useState(0);
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const start = boundary(from),
    end = boundary(to, true);
  const invalid =
    Boolean(from && to && from > to) ||
    (start !== null && (!Number.isFinite(start) || start < 0)) ||
    (end !== null && (!Number.isFinite(end) || end < 0));
  const key = JSON.stringify([project, scope, start, end, timezone]);
  const data = !invalid && result?.key === key ? result.data : null;
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
    setProjectsError(null);
    void invoke<Project[]>("opencode_stats_projects", {
      projectPath: project,
    }).then(
      (value) => {
        if (!disposed) setProjects(value);
      },
      (reason) => {
        if (!disposed) setProjectsError(String(reason));
      },
    );
    return () => {
      disposed = true;
    };
  }, [project, refresh]);
  useEffect(() => {
    if (invalid) {
      setLoading(false);
      return;
    }
    let disposed = false;
    setLoading(true);
    setError(null);
    void invoke<Stats>("opencode_stats", {
      projectPath: project,
      filter: { from: start, to: end, project: scope || null, timezone },
    })
      .then(
        (value) => {
          if (!disposed) setResult({ key, data: value });
        },
        (reason) => {
          if (!disposed) setError(String(reason));
        },
      )
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [key, invalid, refresh]);
  const maxActivity = Math.max(
    1,
    ...(data?.activity.map((day) => day.steps) ?? []),
  );
  return (
    <dialog
      ref={dialog}
      className="settings-dialog stats-dialog"
      aria-labelledby="stats-title"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <header className="settings-heading usage-heading">
        <span className="usage-icon">
          <BarChart3 size={20} />
        </span>
        <div>
          <h1 id="stats-title">OpenCode statistics</h1>
          <p>
            Recorded usage on this OpenCode server. Codex usage isn’t included.
          </p>
        </div>
        <button
          className="icon-button"
          aria-label="Close statistics"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="usage-content stats-content">
        <div className="stats-filters">
          <label>
            Project
            <select
              aria-label="Statistics project"
              value={scope}
              onChange={(event) => setScope(event.target.value)}
            >
              <option value="">All OpenCode projects</option>
              {projects.map((item) => (
                <option key={item.id} value={item.id}>
                  {item.name ? `${item.name} — ` : ""}
                  {item.canonical}
                </option>
              ))}
            </select>
          </label>
          <label>
            From
            <input
              type="date"
              min="1970-01-01"
              aria-label="Statistics from"
              value={from}
              onChange={(event) => setFrom(event.target.value)}
            />
          </label>
          <label>
            Through
            <input
              type="date"
              min={from || "1970-01-01"}
              aria-label="Statistics through"
              value={to}
              onChange={(event) => setTo(event.target.value)}
            />
          </label>
          <button
            onClick={() => {
              setFrom("");
              setTo("");
            }}
          >
            All time
          </button>
        </div>
        <p className="usage-caption">
          {scope
            ? "Selected OpenCode project, including its linked worktrees."
            : "All recorded OpenCode projects and sessions."}{" "}
          Dates use {timezone}. Blank dates include all recorded history.
        </p>
        {projectsError && (
          <p className="usage-error" role="alert">
            Couldn’t load project filters: {projectsError}{" "}
            <button onClick={() => setRefresh((value) => value + 1)}>
              Retry projects
            </button>
          </p>
        )}
        {invalid && (
          <p className="usage-error" role="alert">
            Choose a valid date range with the end on or after the start.
          </p>
        )}
        {!invalid && error && (
          <div className="usage-error" role="alert">
            Couldn’t load statistics: {error}
            {data && (
              <p>Showing the last reported statistics for these filters.</p>
            )}
          </div>
        )}
        {loading && <p role="status">Loading statistics…</p>}
        {data && (
          <>
            <p className="usage-caption">
              Reported range: {new Date(data.range.from).toLocaleDateString()} –{" "}
              {new Date(
                Math.max(data.range.from, data.range.to - 1),
              ).toLocaleDateString()}
            </p>
            {!data.steps && (
              <p className="usage-empty">
                No recorded model usage in this range.
              </p>
            )}
            <dl className="stats-summary" aria-label="Usage totals">
              {[
                ["Total tokens", count(total(data.tokens))],
                ["Recorded cost (USD)", money(data.cost)],
                ["Sessions", count(data.sessions)],
                ["Subagent sessions", count(data.subagents)],
                ["Prompts", count(data.prompts)],
                ["Model steps", count(data.steps)],
              ].map(([label, value]) => (
                <div key={label}>
                  <dt>{label}</dt>
                  <dd>{value}</dd>
                </div>
              ))}
            </dl>
            <section aria-label="Token breakdown">
              <h2>Token breakdown</h2>
              <dl className="stats-tokens">
                {[
                  ["Input", data.tokens.input],
                  ["Output", data.tokens.output],
                  ["Reasoning", data.tokens.reasoning],
                  ["Cache read", data.tokens.cache.read],
                  ["Cache write", data.tokens.cache.write],
                ].map(([label, value]) => (
                  <div key={label}>
                    <dt>{label}</dt>
                    <dd>{count(value as number)}</dd>
                  </div>
                ))}
              </dl>
            </section>
            <p className="usage-caption">
              Totals sum input, output, reasoning and cached tokens reported by
              OpenCode. Recorded cost may be zero for subscription models or
              missing pricing; it is not a billing statement.
            </p>
            <section aria-label="Usage by model">
              <h2>Models</h2>
              <div className="stats-table-wrap">
                <table>
                  <thead>
                    <tr>
                      <th>Model / provider</th>
                      <th>Steps</th>
                      <th>Tokens</th>
                      <th>Input</th>
                      <th>Output</th>
                      <th>Reasoning</th>
                      <th>Cache read</th>
                      <th>Cache write</th>
                      <th>USD</th>
                    </tr>
                  </thead>
                  <tbody>
                    {data.models.map((item) => (
                      <tr key={JSON.stringify(item.model)}>
                        <th scope="row">
                          {item.model.id}
                          <small>
                            {item.model.providerID}
                            {item.model.variant
                              ? ` · ${item.model.variant}`
                              : ""}
                          </small>
                        </th>
                        <td>{count(item.steps)}</td>
                        <td>{count(total(item.tokens))}</td>
                        <td>{count(item.tokens.input)}</td>
                        <td>{count(item.tokens.output)}</td>
                        <td>{count(item.tokens.reasoning)}</td>
                        <td>{count(item.tokens.cache.read)}</td>
                        <td>{count(item.tokens.cache.write)}</td>
                        <td>{money(item.cost)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
            <section aria-label="Daily activity">
              <h2>
                Activity · {count(data.activeDays)} active days ·{" "}
                {count(data.streak)} day streak
              </h2>
              <ol className="stats-activity">
                {data.activity.map((day) => (
                  <li key={day.date}>
                    <time>{day.date}</time>
                    <meter
                      aria-label={`${day.date} model steps`}
                      value={day.steps}
                      min={0}
                      max={maxActivity}
                    />
                    <span>{count(day.steps)} steps</span>
                  </li>
                ))}
              </ol>
            </section>
          </>
        )}
      </div>
      <footer className="settings-footer usage-footer">
        <span>OpenCode native statistics</span>
        <button
          disabled={loading || invalid}
          onClick={() => setRefresh((value) => value + 1)}
        >
          {error ? "Retry statistics" : "Refresh statistics"}
        </button>
        <button onClick={close}>Done</button>
      </footer>
    </dialog>
  );
}
