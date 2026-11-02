import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BarChart3, FolderOpen, LoaderCircle, RefreshCw, X } from "lucide-react";
import type { Stats } from "../../domain/usage/statistics.js";
import { ChoicePicker } from "../primitives/ChoicePicker.js";
import { StatsOverview } from "./StatsOverview.js";
import "./stats.css";

type Project = { id: string; name?: string; canonical: string };

export function StatsDialog({
  project,
  close,
}: {
  project: string;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const [scope, setScope] = useState("");
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsError, setProjectsError] = useState<string | null>(null);
  const [result, setResult] = useState<{ key: string; data: Stats } | null>(
    null,
  );
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [refresh, setRefresh] = useState(0);
  const timezone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const key = JSON.stringify([project, scope, timezone]);
  const data = result?.key === key ? result.data : null;
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
    let disposed = false;
    setLoading(true);
    setError(null);
    void invoke<Stats>("opencode_stats", {
      projectPath: project,
      filter: { from: null, to: null, project: scope || null, timezone },
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
  }, [key, refresh]);
  return (
    <dialog
      ref={dialog}
      className="settings-dialog stats-dialog"
      aria-labelledby="stats-title"
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.nativeEvent.isComposing) {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
      }}
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
          <p>Tokens, cost and activity across your sessions</p>
        </div>
        <button
          className="icon-button"
          aria-label="Close statistics"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="stats-filter-bar">
        <div className="stats-filters">
          <ChoicePicker
            label="Project"
            icon={<FolderOpen size={14} />}
            value={scope}
            onChange={setScope}
            searchable
            choices={[
              { id: "", name: "All projects" },
              ...projects.map((item) => ({
                id: item.id,
                name:
                  item.name ||
                  item.canonical.split(/[\\/]/).filter(Boolean).at(-1) ||
                  item.canonical,
                description: item.canonical,
              })),
            ]}
          />
        </div>
        <p className="stats-range">
          {data
            ? `${new Date(data.range.from).toLocaleDateString()} – ${new Date(Math.max(data.range.from, data.range.to - 1)).toLocaleDateString()}`
            : "Recorded usage"}{" "}
          · {timezone}
        </p>
      </div>
      <div className="usage-content stats-content" aria-busy={loading}>
        {projectsError && (
          <p className="usage-error" role="alert">
            Couldn’t load project filters: {projectsError}{" "}
            <button onClick={() => setRefresh((value) => value + 1)}>
              Retry projects
            </button>
          </p>
        )}
        {error && (
          <div className="usage-error" role="alert">
            Couldn’t load statistics: {error}
            {data && (
              <p>Showing the last reported statistics for these filters.</p>
            )}
          </div>
        )}
        {loading && (
          <p className="stats-loading" role="status">
            <LoaderCircle size={14} className="spinning" />
            Loading statistics…
          </p>
        )}
        {loading && !data && (
          <div className="stats-skeleton" aria-hidden="true">
            <div />
            <div />
            <div />
            <div />
          </div>
        )}
        {data && <StatsOverview data={data} />}
      </div>
      <footer className="settings-footer usage-footer">
        <span>OpenCode server</span>
        <button
          disabled={loading}
          onClick={() => setRefresh((value) => value + 1)}
        >
          <RefreshCw size={12} />
          {error ? "Retry statistics" : "Refresh statistics"}
        </button>
        <button onClick={close}>Done</button>
      </footer>
    </dialog>
  );
}
