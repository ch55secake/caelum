import { useEffect, useRef } from "react";
import type { WorkspaceSurfaces } from "./useWorkspaceSurfaces.js";

export function useWorkspaceShortcuts(actions: {
  hasProject: boolean;
  surfaces: WorkspaceSurfaces;
  newSession: () => void;
  focusSearch: () => void;
  toggleSidebar: () => void;
}) {
  const latest = useRef(actions);
  latest.current = actions;
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey) || event.altKey) return;
      if (document.querySelector("dialog[open]")) return;
      const { hasProject, surfaces, newSession, focusSearch } = latest.current;
      if (event.key === ",") {
        event.preventDefault();
        surfaces.setSettingsOpen(true);
        return;
      }
      if (surfaces.settingsOpen) return;
      const key = event.key.toLowerCase();
      if (hasProject && (key === "p" || (event.shiftKey && key === "f"))) {
        event.preventDefault();
        surfaces.openFiles(key === "p" ? "files" : "content");
      }
      if (key === "n") {
        event.preventDefault();
        newSession();
      }
      if (key === "k") {
        event.preventDefault();
        focusSearch();
      }
      if (event.key === "\\") {
        event.preventDefault();
        latest.current.toggleSidebar();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}
