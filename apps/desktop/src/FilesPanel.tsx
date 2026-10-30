import {
  Suspense,
  lazy,
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
} from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { GitStatusIcon } from "./GitStatusIcon.js";
import { readGitStatus, type GitRepositoryStatus } from "./gitStatus.js";
import { useContextMenu, fileActions, copyAction } from "./ContextMenu.js";
import { FileSearch, type FileSearchRequest } from "./FileSearch.js";
import { useFileWorkspace, fileTabKey } from "./fileWorkspace.js";
import {
  createFileContext,
  type FileContext,
  type FileLocation,
} from "./fileContext.js";
import {
  ChevronDown,
  ChevronRight,
  FileCode2,
  FileText,
  Folder,
  FolderOpen,
  GitBranch,
  RefreshCw,
  X,
} from "lucide-react";

const FilePreview = lazy(() => import("./FilePreview.js"));

type ProjectInfo = { name: string; path: string };
type EntryKind = "file" | "directory" | "symlink";
type ProjectFileEntry = {
  name: string;
  path: string;
  kind: EntryKind;
  sizeBytes: number | null;
};
type DirectoryListing = {
  path: string;
  entries: ProjectFileEntry[];
  total: number;
  nextOffset: number | null;
};
type DirectoryState = DirectoryListing & {
  loading: boolean;
  error: string | null;
};
type ProjectFileContent = {
  path: string;
  content: string;
  sizeBytes: number;
  truncated: boolean;
};
type GitFileStatus = {
  path: string;
  indexStatus: string;
  worktreeStatus: string;
};
type GitFileVersions = {
  path: string;
  original: string;
  modified: string;
  truncated: boolean;
};
type TreeNode =
  | { kind: "entry"; entry: ProjectFileEntry; depth: number }
  | { kind: "more"; path: string; depth: number };
type EditorView =
  | { kind: "empty" }
  | { kind: "loading"; path: string; mode: "file" | "diff" }
  | { kind: "file"; path: string; content: ProjectFileContent }
  | { kind: "diff"; path: string; content: GitFileVersions }
  | { kind: "error"; path: string; mode: "file" | "diff"; message: string };

const PAGE_SIZE = 200;
const EMPTY_DIRECTORY: DirectoryState = {
  path: "",
  entries: [],
  total: 0,
  nextOffset: null,
  loading: false,
  error: null,
};

export function FilesPanel({
  project,
  working,
  request,
  attach,
  canAttach,
}: {
  project: ProjectInfo;
  working: boolean;
  request?: { id: number; location?: FileLocation; search?: FileSearchRequest };
  attach: (file: FileContext, ask?: boolean) => void;
  canAttach: boolean;
}) {
  const contextMenu = useContextMenu();
  const projectPath = project.path;
  const saved = useFileWorkspace(projectPath);
  const [directories, setDirectories] = useState<
    Record<string, DirectoryState>
  >({});
  const [expandedPaths, setExpandedPaths] = useState<Set<string>>(
    () => new Set(saved.value.expanded),
  );
  const [focusedPath, setFocusedPath] = useState("");
  const [activeSource, setActiveSource] = useState<
    "files" | "changes" | "recent"
  >("files");
  const [attachmentError, setAttachmentError] = useState<string | null>(null);
  const [jump, setJump] = useState<
    (FileLocation & { id: number; focusFrom: Element | null }) | undefined
  >();
  const treeScroll = useRef<HTMLDivElement>(null);
  const treeRestored = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  const [status, setStatus] = useState<GitRepositoryStatus | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [statusError, setStatusError] = useState<string | null>(null);
  const [editor, setEditor] = useState<EditorView>({ kind: "empty" });
  const [refreshKey, setRefreshKey] = useState(0);
  const requestGeneration = useRef(0);
  const requestSequence = useRef(0);
  const directoryRequests = useRef(new Map<string, number>());
  const editorRequest = useRef(0);
  const itemRefs = useRef(new Map<string, HTMLButtonElement>());

  const loadDirectory = useCallback(
    async (
      path: string,
      offset = 0,
      append = false,
      generation = requestGeneration.current,
    ) => {
      if (!projectPath) return;
      if (!isTauri()) {
        setDirectories((current) => ({
          ...current,
          [path]: {
            ...(current[path] ?? { ...EMPTY_DIRECTORY, path }),
            loading: false,
            error: "Project files are available in the Caelum desktop app.",
          },
        }));
        return;
      }

      const requestId = ++requestSequence.current;
      directoryRequests.current.set(path, requestId);
      setDirectories((current) => ({
        ...current,
        [path]: {
          ...(current[path] ?? { ...EMPTY_DIRECTORY, path }),
          loading: true,
          error: null,
        },
      }));

      try {
        const result = await invoke<DirectoryListing>(
          "list_project_directory",
          {
            projectPath,
            path,
            offset,
            limit: PAGE_SIZE,
          },
        );
        // Refresh every page the reader opened, so polling does not collapse a
        // large directory back to its first page or lose the tree's scroll position.
        const count = saved.current.current.loaded[path] ?? PAGE_SIZE;
        while (
          !append &&
          result.nextOffset !== null &&
          result.entries.length < count
        ) {
          if (
            generation !== requestGeneration.current ||
            directoryRequests.current.get(path) !== requestId
          )
            return;
          const next = await invoke<DirectoryListing>(
            "list_project_directory",
            {
              projectPath,
              path,
              offset: result.nextOffset,
              limit: PAGE_SIZE,
            },
          );
          result.entries.push(...next.entries);
          result.nextOffset = next.nextOffset;
          result.total = next.total;
        }
        if (
          generation !== requestGeneration.current ||
          directoryRequests.current.get(path) !== requestId
        )
          return;
        saved.update((value) => ({
          ...value,
          loaded: {
            ...value.loaded,
            [path]: append
              ? (value.loaded[path] ?? 0) + result.entries.length
              : result.entries.length,
          },
        }));
        setDirectories((current) => {
          const previous = current[path] ?? { ...EMPTY_DIRECTORY, path };
          return {
            ...current,
            [path]: {
              ...result,
              entries: append
                ? [...previous.entries, ...result.entries]
                : result.entries,
              loading: false,
              error: null,
            },
          };
        });
      } catch (reason) {
        if (
          generation !== requestGeneration.current ||
          directoryRequests.current.get(path) !== requestId
        )
          return;
        setDirectories((current) => ({
          ...current,
          [path]: {
            ...(current[path] ?? { ...EMPTY_DIRECTORY, path }),
            loading: false,
            error: String(reason),
          },
        }));
      }
    },
    [projectPath],
  );

  useEffect(() => {
    const generation = ++requestGeneration.current;
    setStatusLoading(true);

    if (!projectPath) {
      setStatusLoading(false);
      return () => {
        requestGeneration.current += 1;
      };
    }
    if (!isTauri()) {
      setStatusLoading(false);
      setStatusError("Git changes are available in the Caelum desktop app.");
      void loadDirectory("", 0, false, generation);
      return () => {
        requestGeneration.current += 1;
      };
    }

    void loadDirectory("", 0, false, generation);
    for (const path of saved.current.current.expanded)
      void loadDirectory(path, 0, false, generation);
    void readGitStatus(projectPath)
      .then((snapshot) => {
        if (generation === requestGeneration.current) {
          setStatus(snapshot);
          setStatusError(null);
        }
      })
      .catch((reason: unknown) => {
        if (generation === requestGeneration.current)
          setStatusError(String(reason));
      })
      .finally(() => {
        if (generation === requestGeneration.current) setStatusLoading(false);
      });

    return () => {
      requestGeneration.current += 1;
    };
  }, [projectPath, refreshKey, loadDirectory]);

  useEffect(() => {
    const refresh = () => {
      if (!document.hidden) setRefreshKey((key) => key + 1);
    };
    const timer = window.setInterval(refresh, working ? 2500 : 10000);
    window.addEventListener("focus", refresh);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", refresh);
    };
  }, [working]);

  const activeTab = saved.value.tabs.find(
    (tab) => fileTabKey(tab) === saved.value.active,
  );
  useEffect(() => {
    const version = ++editorRequest.current;
    if (!activeTab) {
      setEditor({ kind: "empty" });
      return;
    }
    const { path, mode } = activeTab;
    setEditor((current) =>
      current.kind === mode && current.path === path
        ? current
        : { kind: "loading", path, mode },
    );
    const load =
      mode === "file"
        ? invoke<ProjectFileContent>("read_project_file", {
            projectPath,
            path,
          }).then((content): EditorView => ({ kind: "file", path, content }))
        : invoke<GitFileVersions>("git_file_versions", {
            projectPath,
            filePath: path,
          }).then((content): EditorView => ({ kind: "diff", path, content }));
    void load
      .then((value) => {
        if (version === editorRequest.current) setEditor(value);
      })
      .catch((reason) => {
        if (version === editorRequest.current)
          setEditor({ kind: "error", path, mode, message: String(reason) });
      });
    return () => {
      editorRequest.current++;
    };
  }, [projectPath, saved.value.active, refreshKey]);

  useEffect(() => {
    if (request?.location) openFile(request.location.path, request.location);
  }, [request?.id]);

  const treeNodes = useMemo(
    () => flattenTree(directories, expandedPaths),
    [directories, expandedPaths],
  );
  const changedPaths = useMemo(
    () => new Set(status?.changes.map((change) => change.path) ?? []),
    [status],
  );
  const selectedPath = activeTab?.path ?? null;
  useLayoutEffect(() => {
    if (activeSource !== "files") {
      treeRestored.current = false;
      return;
    }
    if (
      treeRestored.current ||
      !treeScroll.current ||
      !directories[""] ||
      directories[""].loading ||
      [...expandedPaths].some(
        (path) => !directories[path] || directories[path].loading,
      )
    )
      return;
    treeScroll.current.scrollTop = saved.current.current.treeScroll;
    treeRestored.current = true;
  }, [directories, activeSource, expandedPaths]);

  function focusTreeItem(path: string) {
    setFocusedPath(path);
    requestAnimationFrame(() => itemRefs.current.get(path)?.focus());
  }

  function toggleDirectory(path: string) {
    const isExpanded = expandedPaths.has(path);
    const next = new Set(expandedPaths);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    setExpandedPaths(next);
    saved.update((value) => ({ ...value, expanded: [...next] }));
    if (!isExpanded && !directories[path]) void loadDirectory(path);
  }

  function openFile(path: string, location?: FileLocation) {
    saved.open({ path, mode: "file" });
    setJump(
      location
        ? { ...location, id: Date.now(), focusFrom: document.activeElement }
        : undefined,
    );
  }

  function openDiff(path: string) {
    setJump(undefined);
    saved.open({ path, mode: "diff" });
  }

  async function attachFile(path: string) {
    setAttachmentError(null);
    try {
      const file = await invoke<ProjectFileContent>("read_project_file", {
        projectPath,
        path,
      });
      if (mounted.current)
        attach(
          createFileContext(path, file.content, undefined, file.truncated),
        );
    } catch (reason) {
      if (mounted.current) setAttachmentError(String(reason));
    }
  }

  function activateEntry(entry: ProjectFileEntry) {
    if (entry.kind === "directory") {
      toggleDirectory(entry.path);
      return;
    }
    if (entry.kind === "symlink") {
      setEditor({
        kind: "error",
        path: entry.path,
        mode: "file",
        message: "Symbolic links are not opened by the project file browser.",
      });
      return;
    }
    openFile(entry.path);
  }

  function handleTreeKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const currentIndex = treeNodes.findIndex(
      (node) => treeNodeId(node) === focusedPath,
    );
    const currentNode = currentIndex >= 0 ? treeNodes[currentIndex] : undefined;
    const entry = currentNode?.kind === "entry" ? currentNode.entry : undefined;

    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      const nextIndex =
        event.key === "ArrowDown"
          ? Math.min(treeNodes.length - 1, currentIndex + 1)
          : Math.max(0, currentIndex - 1);
      const node = treeNodes[nextIndex];
      if (node) focusTreeItem(treeNodeId(node));
    } else if (event.key === "Home" || event.key === "End") {
      event.preventDefault();
      const node = event.key === "Home" ? treeNodes[0] : treeNodes.at(-1);
      if (node) focusTreeItem(treeNodeId(node));
    } else if (event.key === "ArrowRight" && entry?.kind === "directory") {
      event.preventDefault();
      if (!expandedPaths.has(entry.path)) {
        toggleDirectory(entry.path);
      } else {
        const child = treeNodes[currentIndex + 1];
        if (
          child?.kind === "entry" &&
          currentNode &&
          child.depth > currentNode.depth
        )
          focusTreeItem(child.entry.path);
      }
    } else if (event.key === "ArrowLeft" && entry) {
      event.preventDefault();
      if (entry.kind === "directory" && expandedPaths.has(entry.path)) {
        toggleDirectory(entry.path);
      } else {
        const parent = [...treeNodes.slice(0, currentIndex)]
          .reverse()
          .find(
            (node) =>
              node.kind === "entry" && node.depth < (currentNode?.depth ?? 0),
          );
        if (parent?.kind === "entry") focusTreeItem(parent.entry.path);
      }
    } else if ((event.key === "Enter" || event.key === " ") && entry) {
      event.preventDefault();
      activateEntry(entry);
    }
  }

  const root = directories[""];
  const isChangedPath = selectedPath !== null && changedPaths.has(selectedPath);

  return (
    <section className="files-panel" aria-labelledby="files-title">
      <header className="files-panel-heading">
        <FileCode2 size={18} />
        <div>
          <h1 id="files-title" title={project.path}>
            {project.name}
          </h1>
        </div>
        <button
          className="git-refresh"
          onClick={() => setRefreshKey((key) => key + 1)}
          title="Refresh project files and Git status"
          aria-label="Refresh project files and Git status"
        >
          <RefreshCw size={14} /> <span>Refresh</span>
        </button>
      </header>

      <FileSearch
        project={project.path}
        request={request?.search}
        openFile={(location) => openFile(location.path, location)}
      />
      {saved.error && (
        <div className="file-browser-error" role="alert">
          {saved.error}
          <button onClick={saved.save}>Retry</button>
        </div>
      )}
      {attachmentError && (
        <div className="file-browser-error" role="alert">
          {attachmentError}
        </div>
      )}

      <div className="file-browser-layout">
        <aside
          className="file-browser-sidebar"
          aria-label="Project files and changes"
        >
          <div
            className="file-source-tabs"
            role="group"
            aria-label="Browse files or changes"
          >
            <button
              aria-pressed={activeSource === "files"}
              className={activeSource === "files" ? "selected" : ""}
              onClick={() => setActiveSource("files")}
            >
              All
            </button>
            <button
              aria-pressed={activeSource === "changes"}
              className={activeSource === "changes" ? "selected" : ""}
              onClick={() => setActiveSource("changes")}
            >
              Changes{" "}
              <span>
                {status?.changes.length ?? (statusLoading ? "…" : "0")}
              </span>
            </button>
            <button
              aria-pressed={activeSource === "recent"}
              className={activeSource === "recent" ? "selected" : ""}
              onClick={() => setActiveSource("recent")}
            >
              Recent
            </button>
          </div>

          {activeSource === "files" ? (
            <div
              className="file-tree-scroll"
              ref={treeScroll}
              onScroll={(event) => {
                if (!treeRestored.current) return;
                const treeScroll = event.currentTarget.scrollTop;
                saved.update((value) => ({ ...value, treeScroll }));
              }}
            >
              {root?.loading && root.entries.length === 0 && (
                <div className="file-browser-state" role="status">
                  Loading project files…
                </div>
              )}
              {root?.error && (
                <div className="file-browser-error" role="alert">
                  <strong>Could not load this directory</strong>
                  <span>{root.error}</span>
                  <button onClick={() => void loadDirectory("")}>
                    Try again
                  </button>
                </div>
              )}
              {root && !root.loading && !root.error && root.total === 0 && (
                <div className="file-browser-state">
                  This project folder is empty.
                </div>
              )}
              <div
                className="file-tree"
                role="tree"
                aria-label={`${project.name} project files`}
                aria-busy={root?.loading || undefined}
                onKeyDown={handleTreeKeyDown}
              >
                {treeNodes.map((node) => {
                  if (node.kind === "more") {
                    const directory = directories[node.path];
                    return (
                      <div className="file-tree-more" key={`more:${node.path}`}>
                        <button
                          ref={(element) => {
                            const id = treeNodeId(node);
                            if (element) itemRefs.current.set(id, element);
                            else itemRefs.current.delete(id);
                          }}
                          className="file-tree-item file-tree-more-item"
                          role="treeitem"
                          aria-level={node.depth + 1}
                          aria-label={`Load more files in ${node.path || project.name}`}
                          tabIndex={
                            focusedPath === treeNodeId(node) ||
                            (!focusedPath &&
                              treeNodeId(treeNodes[0]) === treeNodeId(node))
                              ? 0
                              : -1
                          }
                          style={
                            { "--tree-depth": node.depth } as CSSProperties
                          }
                          onFocus={() => setFocusedPath(treeNodeId(node))}
                          onClick={() =>
                            void loadDirectory(
                              node.path,
                              directory?.nextOffset ?? 0,
                              true,
                            )
                          }
                          disabled={directory?.loading}
                        >
                          {directory?.loading
                            ? "Loading more…"
                            : `Load more in ${node.path.split("/").at(-1) ?? "project"}`}
                        </button>
                      </div>
                    );
                  }
                  const { entry, depth } = node;
                  const expanded = expandedPaths.has(entry.path);
                  const directoryState = directories[entry.path];
                  const isDirectory = entry.kind === "directory";
                  const isSelected = selectedPath === entry.path;
                  const Icon = isDirectory
                    ? expanded
                      ? FolderOpen
                      : Folder
                    : entry.kind === "symlink"
                      ? FileText
                      : FileCode2;
                  return (
                    <div className="file-tree-node" key={entry.path}>
                      <button
                        ref={(element) => {
                          if (element)
                            itemRefs.current.set(entry.path, element);
                          else itemRefs.current.delete(entry.path);
                        }}
                        className={`file-tree-item${isSelected ? " selected" : ""}${entry.kind === "symlink" ? " symlink" : ""}`}
                        style={{ "--tree-depth": depth } as CSSProperties}
                        role="treeitem"
                        aria-expanded={isDirectory ? expanded : undefined}
                        aria-selected={isSelected}
                        aria-level={depth + 1}
                        aria-busy={
                          isDirectory && directoryState?.loading
                            ? true
                            : undefined
                        }
                        tabIndex={
                          focusedPath === entry.path ||
                          (!focusedPath &&
                            treeNodes[0]?.kind === "entry" &&
                            treeNodes[0].entry.path === entry.path)
                            ? 0
                            : -1
                        }
                        title={
                          entry.kind === "symlink"
                            ? `${entry.path} (symbolic links are not opened)`
                            : entry.path
                        }
                        aria-label={`${entry.name}${isDirectory ? `, folder${expanded ? ", expanded" : ", collapsed"}${directoryState?.loading ? ", loading" : ""}${directoryState?.error ? ", failed to load" : ""}` : entry.kind === "symlink" ? ", symbolic link, not opened" : ", file"}`}
                        onFocus={() => setFocusedPath(entry.path)}
                        onClick={() => activateEntry(entry)}
                        onContextMenu={(event) =>
                          contextMenu(
                            event,
                            fileActions(project.path, entry.path, [
                              {
                                text: isDirectory
                                  ? expanded
                                    ? "Collapse folder"
                                    : "Expand folder"
                                  : "Open file",
                                enabled: entry.kind !== "symlink",
                                action: () => activateEntry(entry),
                              },
                              ...(entry.kind === "file"
                                ? [
                                    {
                                      text: "Add to prompt",
                                      enabled: canAttach,
                                      action: () => attachFile(entry.path),
                                    },
                                  ]
                                : []),
                              ...(changedPaths.has(entry.path)
                                ? [
                                    {
                                      text: "Review changes",
                                      action: () => openDiff(entry.path),
                                    },
                                  ]
                                : []),
                            ]),
                          )
                        }
                      >
                        {isDirectory ? (
                          expanded ? (
                            <ChevronDown size={13} />
                          ) : (
                            <ChevronRight size={13} />
                          )
                        ) : (
                          <span className="tree-chevron-space" />
                        )}
                        <Icon size={14} />
                        <span className="file-tree-name">{entry.name}</span>
                        {status?.changes.find(
                          (change) => change.path === entry.path,
                        ) && (
                          <GitStatusIcon
                            change={
                              status.changes.find(
                                (change) => change.path === entry.path,
                              )!
                            }
                          />
                        )}
                        {isDirectory && directoryState?.loading && (
                          <span
                            className="tree-loading-dot"
                            aria-label="Loading folder"
                          />
                        )}
                        {isDirectory && directoryState?.error && (
                          <span
                            className="tree-error-mark"
                            title={directoryState.error}
                            aria-label="Could not load folder"
                          >
                            !
                          </span>
                        )}
                      </button>
                      {isDirectory && expanded && directoryState?.error && (
                        <div
                          className="tree-inline-error"
                          style={{ "--tree-depth": depth + 1 } as CSSProperties}
                        >
                          <span>{directoryState.error}</span>
                          <button
                            onClick={() => void loadDirectory(entry.path)}
                          >
                            Retry
                          </button>
                        </div>
                      )}
                      {isDirectory &&
                        expanded &&
                        directoryState &&
                        !directoryState.loading &&
                        !directoryState.error &&
                        directoryState.total === 0 && (
                          <div
                            className="tree-empty-child"
                            style={
                              { "--tree-depth": depth + 1 } as CSSProperties
                            }
                          >
                            Empty folder
                          </div>
                        )}
                    </div>
                  );
                })}
              </div>
              {root?.nextOffset !== null && root?.nextOffset !== undefined && (
                <button
                  className="file-tree-root-more"
                  onClick={() => void loadDirectory("", root.nextOffset!, true)}
                  disabled={root.loading}
                >
                  {root.loading ? "Loading more files…" : "Load more files"}
                </button>
              )}
            </div>
          ) : activeSource === "recent" ? (
            <div className="changed-files-scroll">
              {!saved.value.recent.length && (
                <div className="file-browser-state">
                  Opened files appear here.
                </div>
              )}
              {saved.value.recent.map((path) => (
                <button
                  key={path}
                  className="changed-file-item"
                  onClick={() => openFile(path)}
                  onContextMenu={(event) =>
                    contextMenu(
                      event,
                      fileActions(project.path, path, [
                        { text: "Open file", action: () => openFile(path) },
                        {
                          text: "Add to prompt",
                          enabled: canAttach,
                          action: () => attachFile(path),
                        },
                      ]),
                    )
                  }
                >
                  <FileCode2 size={13} />
                  <span>{path}</span>
                </button>
              ))}
            </div>
          ) : (
            <div className="changed-files-scroll">
              {statusLoading && (
                <div className="file-browser-state" role="status">
                  Checking Git changes…
                </div>
              )}
              {statusError && (
                <div className="file-browser-error" role="alert">
                  <strong>Could not load changes</strong>
                  <span>{statusError}</span>
                </div>
              )}
              {status && status.repositoryRoot === null && (
                <div className="file-browser-state">
                  This project is not inside a Git repository.
                </div>
              )}
              {status?.repositoryRoot && status.changes.length === 0 && (
                <div className="file-browser-state">No changed files.</div>
              )}
              {status?.changes.map((change) => (
                <button
                  key={change.path}
                  className={`changed-file-item${selectedPath === change.path ? " selected" : ""}`}
                  onClick={() => openDiff(change.path)}
                  onContextMenu={(event) =>
                    contextMenu(
                      event,
                      fileActions(project.path, change.path, [
                        {
                          text: "Review changes",
                          action: () => openDiff(change.path),
                        },
                      ]),
                    )
                  }
                  title={`${change.path}: ${describeStatus(change)}`}
                  aria-label={`${change.path}, ${describeStatus(change)}`}
                >
                  <GitStatusIcon change={change} />
                  <span>{change.path}</span>
                </button>
              ))}
            </div>
          )}
        </aside>

        <main
          className="file-editor-panel"
          aria-label="File preview and code review"
        >
          {saved.value.tabs.length > 0 && (
            <div className="file-tabs" role="tablist" aria-label="Open files">
              {saved.value.tabs.map((tab) => {
                const key = fileTabKey(tab);
                return (
                  <div
                    className="file-tab"
                    key={key}
                    data-selected={key === saved.value.active}
                    onContextMenu={(event) =>
                      contextMenu(
                        event,
                        fileActions(project.path, tab.path, [
                          { text: "Close tab", action: () => saved.close(key) },
                          {
                            text: "Add to prompt",
                            enabled: canAttach,
                            action: () => attachFile(tab.path),
                          },
                        ]),
                      )
                    }
                  >
                    <button
                      role="tab"
                      aria-selected={key === saved.value.active}
                      tabIndex={key === saved.value.active ? 0 : -1}
                      title={tab.path}
                      onClick={() => {
                        setJump(undefined);
                        saved.open(tab);
                      }}
                      onKeyDown={(event) => {
                        const tabs = saved.value.tabs;
                        const index = tabs.findIndex(
                          (item) => fileTabKey(item) === key,
                        );
                        if (
                          ["ArrowLeft", "ArrowRight", "Home", "End"].includes(
                            event.key,
                          )
                        ) {
                          event.preventDefault();
                          const next =
                            event.key === "Home"
                              ? 0
                              : event.key === "End"
                                ? tabs.length - 1
                                : (index +
                                    (event.key === "ArrowRight" ? 1 : -1) +
                                    tabs.length) %
                                  tabs.length;
                          setJump(undefined);
                          saved.open(tabs[next]);
                          event.currentTarget
                            .closest('[role="tablist"]')
                            ?.querySelectorAll<HTMLButtonElement>(
                              '[role="tab"]',
                            )
                            [next]?.focus();
                        }
                      }}
                    >
                      {tab.path.split("/").at(-1)}
                      {tab.mode === "diff" ? " · Diff" : ""}
                    </button>
                    <button
                      className="icon-button"
                      aria-label={`Close ${tab.path}${tab.mode === "diff" ? " diff" : ""}`}
                      onClick={() => saved.close(key)}
                    >
                      <X size={12} />
                    </button>
                  </div>
                );
              })}
            </div>
          )}
          {editor.kind === "empty" && (
            <div className="file-editor-empty">
              <FileCode2 size={24} />
              <strong>Select a file to preview</strong>
              <span>
                Browse project files or choose a changed file to open a
                side-by-side diff.
              </span>
            </div>
          )}
          {editor.kind === "loading" && (
            <div className="file-editor-empty" role="status">
              {editor.mode === "diff"
                ? "Loading code diff…"
                : "Loading file preview…"}
            </div>
          )}
          {editor.kind === "error" && (
            <div className="file-editor-empty" role="alert">
              <strong>Unable to open {editor.path}</strong>
              <span>{editor.message}</span>
              <button onClick={() => setRefreshKey((key) => key + 1)}>
                Retry
              </button>
            </div>
          )}
          {editor.kind === "file" && (
            <>
              <div
                className="file-editor-toolbar"
                onContextMenu={(event) =>
                  contextMenu(
                    event,
                    fileActions(project.path, editor.path, [
                      {
                        text: "Add to prompt",
                        enabled: canAttach,
                        action: () =>
                          attach(
                            createFileContext(
                              editor.path,
                              editor.content.content,
                              undefined,
                              editor.content.truncated,
                            ),
                          ),
                      },
                      copyAction(
                        editor.content.truncated
                          ? "Copy file preview"
                          : "Copy file contents",
                        editor.content.content,
                      ),
                    ]),
                  )
                }
              >
                <div>
                  <span className="file-editor-path" title={editor.path}>
                    {editor.path}
                  </span>
                  <span className="file-editor-size">
                    {formatBytes(editor.content.sizeBytes)}
                    {editor.content.truncated ? " · preview truncated" : ""}
                  </span>
                </div>
                <button
                  className="review-file-button"
                  disabled={!canAttach}
                  onClick={() =>
                    attach(
                      createFileContext(
                        editor.path,
                        editor.content.content,
                        undefined,
                        editor.content.truncated,
                      ),
                    )
                  }
                >
                  Add to prompt
                </button>
                {isChangedPath && (
                  <button
                    className="review-file-button"
                    onClick={() => openDiff(editor.path)}
                  >
                    <GitBranch size={13} /> Review changes
                  </button>
                )}
              </div>
              {editor.content.truncated && (
                <div className="file-truncation-notice" role="status">
                  Showing the first 1 MiB of this file to keep large previews
                  responsive.
                </div>
              )}
              <Suspense
                fallback={
                  <div className="file-editor-empty" role="status">
                    Loading code preview…
                  </div>
                }
              >
                <FilePreview
                  key={`file:${editor.path}`}
                  path={editor.path}
                  content={editor.content.content}
                  viewState={saved.value.views[`file:${editor.path}`] ?? null}
                  saveView={(view) =>
                    saved.update((value) => ({
                      ...value,
                      views: { ...value.views, [`file:${editor.path}`]: view },
                    }))
                  }
                  jump={jump?.path === editor.path ? jump : undefined}
                  attach={attach}
                  canAttach={canAttach}
                />
              </Suspense>
            </>
          )}
          {editor.kind === "diff" && (
            <>
              <div
                className="file-editor-toolbar"
                onContextMenu={(event) =>
                  contextMenu(event, fileActions(project.path, editor.path))
                }
              >
                <div>
                  <span className="file-editor-path" title={editor.path}>
                    {editor.path}
                  </span>
                  <span className="file-editor-size">HEAD ↔ Working tree</span>
                </div>
                <span className="diff-badge">READ ONLY</span>
              </div>
              {editor.content.truncated && (
                <div className="file-truncation-notice" role="status">
                  One or both versions exceed the 1 MiB preview limit. The
                  available preview is shown.
                </div>
              )}
              <Suspense
                fallback={
                  <div className="file-editor-empty" role="status">
                    Loading diff preview…
                  </div>
                }
              >
                <FilePreview
                  key={`diff:${editor.path}`}
                  path={editor.path}
                  content={editor.content.modified}
                  original={editor.content.original}
                  viewState={saved.value.views[`diff:${editor.path}`] ?? null}
                  saveView={(view) =>
                    saved.update((value) => ({
                      ...value,
                      views: { ...value.views, [`diff:${editor.path}`]: view },
                    }))
                  }
                  attach={attach}
                  canAttach={canAttach}
                />
              </Suspense>
            </>
          )}
        </main>
      </div>
    </section>
  );
}

function flattenTree(
  directories: Record<string, DirectoryState>,
  expandedPaths: Set<string>,
): TreeNode[] {
  const root = directories[""];
  if (!root) return [];
  const nodes: TreeNode[] = [];
  const stack: TreeNode[] = [...root.entries]
    .reverse()
    .map((entry) => ({ kind: "entry", entry, depth: 0 }));

  while (stack.length > 0) {
    const node = stack.pop()!;
    nodes.push(node);
    if (
      node.kind !== "entry" ||
      node.entry.kind !== "directory" ||
      !expandedPaths.has(node.entry.path)
    )
      continue;
    const directory = directories[node.entry.path];
    if (!directory) continue;
    if (directory.nextOffset !== null) {
      stack.push({
        kind: "more",
        path: node.entry.path,
        depth: node.depth + 1,
      });
    }
    for (let index = directory.entries.length - 1; index >= 0; index -= 1) {
      stack.push({
        kind: "entry",
        entry: directory.entries[index],
        depth: node.depth + 1,
      });
    }
  }
  return nodes;
}

function treeNodeId(node: TreeNode | undefined): string {
  if (!node) return "";
  return node.kind === "entry" ? node.entry.path : `more:${node.path}`;
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function describeStatus(change: GitFileStatus): string {
  if (change.indexStatus === "?" && change.worktreeStatus === "?")
    return "Untracked";
  const entries = [
    change.indexStatus !== " " && `Staged ${statusWord(change.indexStatus)}`,
    change.worktreeStatus !== " " &&
      `Unstaged ${statusWord(change.worktreeStatus)}`,
  ].filter(Boolean);
  return entries.join("; ") || "Changed";
}

function statusWord(status: string): string {
  return (
    (
      {
        M: "modified",
        A: "added",
        D: "deleted",
        R: "renamed",
        C: "copied",
        U: "unmerged",
      } as Record<string, string>
    )[status] ?? "changed"
  );
}
