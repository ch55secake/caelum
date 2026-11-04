import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { InspectorRequest } from "../ui/files/ProjectInspector.js";
import type { FileLocation } from "../domain/files/fileContext.js";
import type { PullRequestTarget } from "../ui/pull-requests/PullRequestPreview.js";

export function useWorkspaceSurfaces(
  projectPath: string | undefined,
  sessionKey: string,
) {
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const [sidebarWidth, setSidebarWidth] = useState<number | null>(() => {
    try {
      const width = Number(localStorage.getItem("caelum.sidebar.width"));
      return Number.isFinite(width) && width > 0 ? width : null;
    } catch {
      return null;
    }
  });
  useEffect(() => {
    if (sidebarWidth === null) return;
    try {
      localStorage.setItem("caelum.sidebar.width", String(sidebarWidth));
    } catch {
      /* Keep the preferred width for this window, including after collapse. */
    }
  }, [sidebarWidth]);
  const [pane, setPane] = useState<"files" | "changes" | null>("changes");
  const [expanded, setExpanded] = useState(false);
  const [fileRequest, setFileRequest] = useState<InspectorRequest>();
  const fileRequestId = useRef(0);
  const [pullRequest, setPullRequest] = useState<
    (PullRequestTarget & { scope: string }) | null
  >(null);
  const activePullRequest =
    pullRequest?.scope === sessionKey ? pullRequest : null;
  const currentPullRequest = useRef(activePullRequest);
  currentPullRequest.current = activePullRequest;
  const currentSessionKey = useRef(sessionKey);
  currentSessionKey.current = sessionKey;
  const changesOpen = !activePullRequest && pane === "changes";

  useEffect(() => {
    setPullRequest(null);
    setExpanded(false);
    setFileRequest(undefined);
  }, [sessionKey]);

  function toggleChanges() {
    setPullRequest(null);
    setPane((value) =>
      !activePullRequest && value === "changes" ? null : "changes",
    );
    setFileRequest(undefined);
    setExpanded(false);
  }
  const openFile = useCallback(
    (location: FileLocation) => {
      if (!projectPath) return;
      setPullRequest(null);
      setPane("files");
      setFileRequest({
        project: projectPath,
        id: ++fileRequestId.current,
        location,
      });
    },
    [projectPath],
  );
  const fileNavigation = useMemo(
    () => (projectPath ? { projectPath, open: openFile } : null),
    [projectPath, openFile],
  );
  function openFiles(search?: "files" | "content") {
    setPullRequest(null);
    setPane("files");
    if (search && projectPath) {
      const id = ++fileRequestId.current;
      setFileRequest({
        project: projectPath,
        id,
        search: { id, mode: search },
      });
    } else {
      setExpanded(false);
      setFileRequest(undefined);
    }
  }
  function showSession() {
    setPullRequest(null);
    setExpanded(false);
  }
  return {
    settingsOpen,
    setSettingsOpen,
    collapsed,
    setCollapsed,
    sidebarWidth,
    setSidebarWidth,
    pane,
    expanded,
    setExpanded,
    fileRequest,
    activePullRequest,
    currentPullRequest,
    changesOpen,
    fileNavigation,
    openFiles,
    toggleChanges,
    showSession,
    isCurrentSession: () => currentSessionKey.current === sessionKey,
    openPullRequest: (target: PullRequestTarget) =>
      setPullRequest({ ...target, scope: sessionKey }),
    dismissPullRequest: () => setPullRequest(null),
    changePane: (mode: "files" | "changes") => {
      setPane(mode);
      setFileRequest(undefined);
    },
    closeInspector: () => {
      setPane(null);
      setExpanded(false);
    },
  };
}

export type WorkspaceSurfaces = ReturnType<typeof useWorkspaceSurfaces>;
