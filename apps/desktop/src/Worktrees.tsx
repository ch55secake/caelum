import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GitBranch, Plus, RefreshCw, Trash2, X } from "lucide-react";
import { ProjectItem } from "./ProjectItem.js";
import { projectHasActiveTurn, type ProjectInfo } from "./workspaceState.js";
import type { WorkspaceController } from "./Sidebar.js";
import { worktreeLabel, type GitWorktree } from "./worktreeState.js";
import { copyAction, revealAction, useContextMenu } from "./ContextMenu.js";

function WorktreeDialog({
  app,
  project,
  removing,
  close,
}: {
  app: WorkspaceController;
  project: ProjectInfo;
  removing?: GitWorktree;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const catalog = app.worktreeCatalogs[project.path];
  const [branch, setBranch] = useState("");
  const [base, setBase] = useState(catalog?.defaultBase ?? "HEAD");
  const [destination, setDestination] = useState("");
  const [previewError, setPreviewError] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const submitting = useRef(false);
  const source = app.worktreeSource(project);
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
    setDestination("");
    setPreviewError("");
    const timer = window.setTimeout(() => {
      if (!branch || removing) return;
      void invoke<string>("worktree_destination", {
        projectPath: source.path,
        branch,
        baseDirectory: app.worktreeDirectory,
      })
        .then((path) => {
          if (!disposed) setDestination(path);
        })
        .catch((reason) => {
          if (!disposed) setPreviewError(String(reason));
        });
    }, 150);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, [source.path, branch, app.worktreeDirectory, removing]);
  const title = removing ? "Remove worktree" : "Create worktree";
  return (
    <dialog
      ref={dialog}
      className="worktree-dialog"
      aria-label={title}
      onCancel={(event) => {
        event.preventDefault();
        if (!busy) close();
      }}
    >
      <form
        onSubmit={async (event) => {
          event.preventDefault();
          if (submitting.current) return;
          submitting.current = true;
          setBusy(true);
          setError("");
          try {
            if (removing) await app.removeWorktree(project, removing);
            else await app.createWorktree(project, branch, base);
            close();
          } catch (reason) {
            setError(String(reason));
          } finally {
            setBusy(false);
            submitting.current = false;
          }
        }}
      >
        <header className="settings-heading">
          <h1>{title}</h1>
          <button
            type="button"
            className="icon-button"
            aria-label="Close worktree dialog"
            disabled={busy}
            onClick={close}
          >
            <X size={16} />
          </button>
        </header>
        <div className="worktree-dialog-body">
          {removing ? (
            <>
              <p>
                Remove <strong>{worktreeLabel(removing)}</strong> from{" "}
                {project.name}? Its branch and commits will be kept.
              </p>
              <code className="worktree-destination">{removing.path}</code>
            </>
          ) : (
            <>
              <label>
                New branch
                <input
                  autoFocus
                  required
                  autoComplete="off"
                  spellCheck={false}
                  value={branch}
                  disabled={busy}
                  placeholder="feature/search"
                  onChange={(event) => setBranch(event.target.value)}
                />
              </label>
              <label>
                Start from
                <select
                  value={base}
                  disabled={busy}
                  onChange={(event) => setBase(event.target.value)}
                >
                  {[
                    ...new Set([
                      catalog?.defaultBase ?? "HEAD",
                      ...(catalog?.branches ?? []),
                    ]),
                  ].map((name) => (
                    <option key={name} value={name}>
                      {name}
                    </option>
                  ))}
                </select>
              </label>
              <div>
                <span className="worktree-location-label">Location</span>
                <code className="worktree-destination">
                  {destination ||
                    catalog?.storageDirectory ||
                    "Choose a branch name"}
                </code>
              </div>
              {previewError && (
                <p className="worktree-error" role="alert">
                  {previewError}
                </p>
              )}
            </>
          )}
          {error && (
            <p className="worktree-error" role="alert">
              {error}
            </p>
          )}
        </div>
        <footer className="settings-footer">
          <button type="button" onClick={close} disabled={busy}>
            Cancel
          </button>
          <button
            type="submit"
            className={removing ? "worktree-remove" : "question-submit"}
            disabled={
              busy ||
              (!removing && (!destination || Boolean(previewError))) ||
              Boolean(removing && app.worktreeInUse(removing))
            }
          >
            {busy ? "Working…" : title}
          </button>
        </footer>
      </form>
    </dialog>
  );
}

export function ProjectWorktrees({
  app,
  project,
}: {
  app: WorkspaceController;
  project: ProjectInfo;
}) {
  const [dialog, setDialog] = useState<"create" | GitWorktree | null>(null);
  const catalog = app.worktreeCatalogs[project.path];
  const error = app.worktreeErrors[project.path];
  const busy = app.worktreeBusy[project.path];
  const menu = useContextMenu();
  const member = (path: string) =>
    (app.projectRoots[path] ?? path) === project.path;
  return (
    <div className="project-group">
      <ProjectItem
        project={project}
        selected={app.rootProject?.path === project.path}
        working={Object.entries(app.workspaces).some(
          ([path, workspace]) =>
            member(path) && projectHasActiveTurn(workspace),
        )}
        approval={app.approvals.some((request) => member(request.projectPath))}
        activate={() => app.activateGroup(project)}
        close={() => app.closeProject(project.path)}
      />
      {catalog && (
        <div
          className="worktree-list"
          aria-label={`Worktrees for ${project.name}`}
        >
          <div className="worktree-heading">
            <span>Worktrees</span>
            <button
              className="icon-button"
              aria-label={`Refresh worktrees for ${project.name}`}
              disabled={busy}
              onClick={() => void app.refreshWorktrees(project)}
            >
              <RefreshCw size={11} />
            </button>
            <button
              className="icon-button"
              aria-label={`Create worktree for ${project.name}`}
              disabled={busy}
              onClick={() => setDialog("create")}
            >
              <Plus size={12} />
            </button>
          </div>
          {catalog.worktrees.map((entry) => (
            <div
              className="worktree-row"
              key={entry.path}
              onContextMenu={(event) =>
                menu(event, [
                  copyAction("Copy worktree path", entry.path),
                  revealAction(entry.path),
                ])
              }
            >
              <button
                className={`worktree-item${app.project?.path === entry.project.path ? " selected" : ""}`}
                aria-label={`Switch to worktree ${worktreeLabel(entry)}`}
                aria-current={
                  app.project?.path === entry.project.path ? "page" : undefined
                }
                disabled={!entry.available || busy}
                title={`${entry.branch ?? "Detached HEAD"}\n${entry.project.path}${entry.locked != null ? `\nLocked: ${entry.locked}` : ""}`}
                onClick={() => void app.openWorktree(project, entry)}
              >
                <GitBranch size={12} />
                <span>{worktreeLabel(entry)}</span>
                {!entry.available ? (
                  <small>Missing</small>
                ) : app.workspaces[entry.project.path] &&
                  projectHasActiveTurn(app.workspaces[entry.project.path]) ? (
                  <i className="running-dot" aria-label="Running session" />
                ) : null}
              </button>
              {!entry.main && (
                <button
                  className="icon-button worktree-delete"
                  aria-label={`Remove worktree ${worktreeLabel(entry)}`}
                  title={
                    app.worktreeInUse(entry)
                      ? "Stop running sessions before removing"
                      : entry.locked != null
                        ? "Worktree is locked"
                        : "Remove worktree"
                  }
                  disabled={
                    busy || app.worktreeInUse(entry) || entry.locked != null
                  }
                  onClick={() => setDialog(entry)}
                >
                  <Trash2 size={11} />
                </button>
              )}
            </div>
          ))}
        </div>
      )}
      {error && (
        <div className="worktree-load-error">
          <span title={error}>Worktrees unavailable</span>
          <button onClick={() => void app.refreshWorktrees(project)}>
            Retry
          </button>
        </div>
      )}
      {dialog && (
        <WorktreeDialog
          app={app}
          project={project}
          removing={dialog === "create" ? undefined : dialog}
          close={() => setDialog(null)}
        />
      )}
    </div>
  );
}

export function SessionWorktree({ app }: { app: WorkspaceController }) {
  const root = app.rootProject;
  const catalog = root ? app.worktreeCatalogs[root.path] : null;
  if (!root || !catalog) return null;
  return (
    <label className="session-worktree">
      Worktree
      <select
        aria-label="Session worktree"
        value={app.project?.path}
        disabled={app.worktreeBusy[root.path]}
        onChange={(event) => {
          const entry = catalog.worktrees.find(
            (entry) => entry.project.path === event.target.value,
          );
          if (entry) void app.openWorktree(root, entry, true);
        }}
      >
        {catalog.worktrees.map((entry) => (
          <option
            key={entry.path}
            value={entry.project.path}
            disabled={!entry.available}
          >
            {worktreeLabel(entry)}
          </option>
        ))}
        {!app.currentWorktree && (
          <option value={app.project?.path} disabled>
            Unavailable checkout
          </option>
        )}
      </select>
    </label>
  );
}
