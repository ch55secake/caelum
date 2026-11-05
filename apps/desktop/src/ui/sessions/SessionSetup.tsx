import { useEffect, useLayoutEffect, useRef, useState } from "react";
import {
  ArrowRight,
  FolderOpen,
  GitBranch,
  Layers,
  MessageSquarePlus,
  Plus,
  X,
} from "lucide-react";
import type { WorkspaceController } from "../../application/workspace/controller.js";
import type {
  ProjectInfo,
  ProviderKind,
} from "../../domain/sessions/workspaceState.js";
import { worktreeLabel } from "../../domain/projects/worktreeState.js";
import { ChoicePicker } from "../primitives/ChoicePicker.js";
import { WorktreeDialog } from "../projects/Worktrees.js";

export function SessionSetup({
  app,
  defaultProvider,
  close,
  started,
}: {
  app: WorkspaceController;
  defaultProvider: ProviderKind;
  close: () => void;
  started: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const pending = useRef(false);
  const [root, setRoot] = useState(app.rootProject);
  const [checkout, setCheckout] = useState(app.project?.path ?? "");
  const [provider, setProvider] = useState(
    app.project ? app.provider : defaultProvider,
  );
  const [creating, setCreating] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const catalog = root ? app.worktreeCatalogs[root.path] : null;
  const worktrees = catalog?.worktrees ?? [];
  const checkoutEntry = worktrees.find(
    (entry) => entry.project.path === checkout,
  );
  const checkoutError = root ? app.worktreeErrors[root.path] : null;
  const unavailable = Boolean(catalog && !checkoutEntry?.available);
  const loading = Boolean(root && catalog === undefined && !checkoutError);
  useLayoutEffect(() => {
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
    const rootPath = root && app.projectRoots[root.path];
    const canonical =
      rootPath && app.projectGroups.find((item) => item.path === rootPath);
    if (canonical && canonical.path !== root?.path) setRoot(canonical);
  }, [root, app.projectRoots, app.projectGroups]);

  function selectProject(target: ProjectInfo) {
    const path = app.groupWorktree(target)?.project.path ?? target.path;
    setRoot(target);
    setCheckout(path);
    setProvider(app.workspaces[path]?.provider ?? defaultProvider);
    setError("");
  }
  async function perform(action: () => Promise<void>) {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (reason) {
      setError(String(reason));
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }
  return (
    <dialog
      ref={dialog}
      className="worktree-dialog session-setup"
      aria-label="New session setup"
      onCancel={(event) => {
        event.preventDefault();
        if (!busy) close();
      }}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          if (!root || unavailable || loading || checkoutError) return;
          void perform(async () => {
            await app.startSessionDraft(root, checkout, provider);
            started();
          });
        }}
      >
        <header className="settings-heading worktree-dialog-heading">
          <span className="worktree-dialog-icon" aria-hidden="true">
            <MessageSquarePlus size={18} />
          </span>
          <div>
            <h1>New session</h1>
            <p>Choose where you want to work.</p>
          </div>
          <button
            type="button"
            className="icon-button"
            aria-label="Close session setup"
            disabled={busy}
            onClick={close}
          >
            <X size={16} />
          </button>
        </header>
        <div className="worktree-dialog-body">
          <div className="session-setup-field">
            <span>Project</span>
            <ChoicePicker
              label="Session project"
              value={root?.path ?? ""}
              icon={<FolderOpen size={15} />}
              triggerName={root?.name ?? "Choose a project"}
              searchable
              disabled={busy || app.projectGroups.length === 0}
              choices={app.projectGroups.map((item) => ({
                id: item.path,
                name: item.name,
                description: item.path,
              }))}
              onChange={(path) => {
                const target = app.projectGroups.find(
                  (item) => item.path === path,
                );
                if (target) selectProject(target);
              }}
            />
            <button
              type="button"
              className="setup-secondary"
              disabled={busy || app.selecting}
              onClick={() =>
                void perform(async () => {
                  const opened = await app.pickProject();
                  if (opened) selectProject(opened);
                })
              }
            >
              <FolderOpen size={13} />
              Open folder…
            </button>
          </div>
          <div className="session-setup-field">
            <span>Checkout</span>
            <ChoicePicker
              label="Session checkout"
              value={checkout}
              icon={<GitBranch size={15} />}
              triggerName={
                loading
                  ? "Loading checkouts…"
                  : checkoutEntry
                    ? worktreeLabel(checkoutEntry)
                    : "Original checkout"
              }
              searchable
              disabled={busy || !root || loading}
              choices={
                catalog
                  ? worktrees.map((entry) => ({
                      id: entry.project.path,
                      name: worktreeLabel(entry),
                      description: `${entry.available ? (entry.branch ?? "Detached HEAD") : "Unavailable"} · ${entry.project.path}`,
                      disabled: !entry.available,
                    }))
                  : root
                    ? [
                        {
                          id: root.path,
                          name: "Original checkout",
                          description: root.path,
                        },
                      ]
                    : []
              }
              onChange={(path) => {
                setCheckout(path);
                setError("");
              }}
            />
            {catalog && (
              <button
                type="button"
                className="setup-secondary"
                disabled={busy || app.worktreeBusy[root!.path]}
                onClick={() => setCreating(true)}
              >
                <Plus size={13} />
                Create worktree…
              </button>
            )}
            {checkoutError && (
              <div className="worktree-error" role="alert">
                {checkoutError}
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => void app.refreshWorktrees(root!)}
                >
                  Retry checkouts
                </button>
              </div>
            )}
            {unavailable && (
              <p className="worktree-error" role="alert">
                Choose an available checkout to continue.
              </p>
            )}
          </div>
          <div className="session-setup-field">
            <span>Provider</span>
            <ChoicePicker
              label="Session provider"
              value={provider}
              icon={<Layers size={15} />}
              disabled={busy}
              choices={[
                { id: "codex", name: "Codex" },
                { id: "opencode", name: "OpenCode" },
              ]}
              onChange={(value) => setProvider(value as ProviderKind)}
            />
          </div>
          <p className="worktree-field-hint">
            Your session starts when you send the first message.
          </p>
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
            className="worktree-create"
            disabled={
              busy || !root || loading || unavailable || Boolean(checkoutError)
            }
          >
            {busy ? "Opening…" : "Start writing"}
            <ArrowRight size={14} />
          </button>
        </footer>
      </form>
      {creating && root && (
        <WorktreeDialog
          app={app}
          project={root}
          created={(entry) => setCheckout(entry.project.path)}
          close={() => setCreating(false)}
        />
      )}
    </dialog>
  );
}
