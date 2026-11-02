import { useEffect, useRef, useState } from "react";
import { restoreFileView, type FileViewState } from "./codeViewState.js";

export type FileTab = { path: string; mode: "file" | "diff" };
export const fileTabKey = (tab: FileTab) => `${tab.mode}:${tab.path}`;
type FileWorkspace = {
  tabs: FileTab[];
  active: string | null;
  expanded: string[];
  recent: string[];
  views: Record<string, FileViewState | null>;
  treeWidth: number | null;
  treeScroll: number;
  loaded: Record<string, number>;
};
const cache = new Map<string, FileWorkspace>();
const storageKey = (project: string) =>
  `caelum.files.v1:${encodeURIComponent(project)}`;
const validPath = (path: unknown): path is string =>
  typeof path === "string" &&
  path.length > 0 &&
  path.length < 4096 &&
  !path.includes("\0") &&
  !path.split(/[\\/]/).some((part) => !part || part === "." || part === "..");
function restore(project: string): FileWorkspace {
  const empty: FileWorkspace = {
    tabs: [],
    active: null,
    expanded: [],
    recent: [],
    views: {},
    treeWidth: null,
    treeScroll: 0,
    loaded: {},
  };
  try {
    const stored = JSON.parse(
      localStorage.getItem(storageKey(project)) ?? "null",
    );
    if (!stored || !Array.isArray(stored.tabs)) return empty;
    const tabs: FileTab[] = [
      ...new Map<string, FileTab>(
        stored.tabs
          .filter(
            (tab: FileTab) =>
              tab && validPath(tab.path) && ["file", "diff"].includes(tab.mode),
          )
          .map((tab: FileTab) => [fileTabKey(tab), tab] as const),
      ).values(),
    ].slice(-24);
    const keys = new Set(tabs.map(fileTabKey));
    return {
      tabs,
      active: keys.has(stored.active)
        ? stored.active
        : tabs[0]
          ? fileTabKey(tabs[0])
          : null,
      expanded: Array.isArray(stored.expanded)
        ? stored.expanded.filter(validPath).slice(0, 300)
        : [],
      recent: Array.isArray(stored.recent)
        ? stored.recent.filter(validPath).slice(0, 30)
        : [],
      views: Object.fromEntries(
        Object.entries(stored.views ?? {})
          .filter(([key]) => keys.has(key))
          .map(([key, view]) => [key, restoreFileView(view)]),
      ),
      treeWidth:
        Number.isFinite(stored.treeWidth) && stored.treeWidth > 0
          ? stored.treeWidth
          : null,
      treeScroll: Number.isFinite(stored.treeScroll)
        ? Math.max(0, stored.treeScroll)
        : 0,
      loaded: Object.fromEntries(
        Object.entries(stored.loaded ?? {})
          .filter(
            ([path, count]) =>
              (path === "" || validPath(path)) &&
              typeof count === "number" &&
              Number.isSafeInteger(count) &&
              count > 0 &&
              count <= 50_000,
          )
          .slice(0, 301),
      ) as Record<string, number>,
    };
  } catch {
    return empty;
  }
}

export function useFileWorkspace(project: string) {
  const [value, setValue] = useState(
    () => cache.get(project) ?? restore(project),
  );
  const current = useRef(value);
  const [error, setError] = useState<string | null>(null);
  const save = () => {
    try {
      localStorage.setItem(
        storageKey(project),
        JSON.stringify(current.current),
      );
      setError(null);
    } catch {
      setError("Could not save file tabs on this device.");
    }
  };
  useEffect(() => {
    const timer = window.setTimeout(save, 200);
    return () => window.clearTimeout(timer);
  }, [value, project]);
  useEffect(() => {
    const flush = () => {
      try {
        localStorage.setItem(
          storageKey(project),
          JSON.stringify(current.current),
        );
      } catch {
        /* Keep the in-memory workspace. */
      }
    };
    window.addEventListener("pagehide", flush);
    return () => {
      window.removeEventListener("pagehide", flush);
      flush();
    };
  }, [project]);
  function update(change: (value: FileWorkspace) => FileWorkspace) {
    current.current = change(current.current);
    cache.set(project, current.current);
    setValue(current.current);
  }
  function open(tab: FileTab) {
    update((value) => {
      const tabs = value.tabs.some(
        (item) => fileTabKey(item) === fileTabKey(tab),
      )
        ? value.tabs
        : [...value.tabs, tab].slice(-24);
      const keys = new Set(tabs.map(fileTabKey));
      return {
        ...value,
        tabs,
        active: fileTabKey(tab),
        views: Object.fromEntries(
          Object.entries(value.views).filter(([key]) => keys.has(key)),
        ),
        recent: [
          tab.path,
          ...value.recent.filter((path) => path !== tab.path),
        ].slice(0, 30),
      };
    });
  }
  function close(key: string) {
    update((value) => {
      const index = value.tabs.findIndex((tab) => fileTabKey(tab) === key);
      const tabs = value.tabs.filter((tab) => fileTabKey(tab) !== key);
      const next = tabs[Math.min(index, tabs.length - 1)];
      const views = { ...value.views };
      delete views[key];
      return {
        ...value,
        tabs,
        views,
        active:
          value.active === key
            ? next
              ? fileTabKey(next)
              : null
            : value.active,
      };
    });
  }
  return { value, current, update, open, close, error, save };
}
