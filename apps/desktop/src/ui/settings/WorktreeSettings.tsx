import { FolderOpen } from "lucide-react";
import { open } from "@tauri-apps/plugin-dialog";
import type { Preferences } from "../../application/useSettings.js";
import type { WorkspaceController } from "../../application/workspace/controller.js";

export function WorktreeSettings({
  app,
  preferences,
}: {
  app: WorkspaceController;
  preferences: Preferences;
}) {
  return (
    <>
      <h2>Worktrees</h2>
      <p className="settings-description">
        New checkouts are grouped by project in this folder.
      </p>
      <label className="worktree-folder-setting">
        Worktree folder
        <input
          value={preferences.settings.worktreeDirectory}
          placeholder="~/.caelum/worktrees"
          onChange={(event) =>
            preferences.update((current) => ({
              ...current,
              worktreeDirectory: event.target.value,
            }))
          }
        />
      </label>
      <button
        className="settings-refresh"
        onClick={() => {
          void open({
            directory: true,
            multiple: false,
            title: "Choose worktree folder",
          })
            .then((path) => {
              if (typeof path === "string")
                preferences.update((current) => ({
                  ...current,
                  worktreeDirectory: path,
                }));
            })
            .catch((reason) => app.setError(String(reason)));
        }}
      >
        <FolderOpen size={14} /> Choose folder
      </button>
      <p className="settings-note">
        Leave empty to use ~/.caelum/worktrees. Changes apply to newly created
        worktrees; existing checkouts keep their locations.
      </p>
    </>
  );
}
