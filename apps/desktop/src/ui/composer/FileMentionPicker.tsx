import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  createFileContext,
  type FileContext,
} from "../../domain/files/fileContext.js";

export function useFileMentions({
  project,
  draft,
  setDraft,
  enabled,
  input,
  attach,
}: {
  project: string;
  draft: string;
  setDraft: (text: string) => void;
  enabled: boolean;
  input: RefObject<HTMLTextAreaElement | null>;
  attach: (file: FileContext) => boolean;
}) {
  const id = useId();
  const list = useRef<HTMLDivElement>(null);
  const revision = useRef(0);
  const mounted = useRef(false);
  const latest = useRef({ draft, enabled, attach });
  latest.current = { draft, enabled, attach };
  const [caret, setCaret] = useState<number | null>(null);
  const [focused, setFocused] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [active, setActive] = useState(0);
  const [refresh, setRefresh] = useState(0);
  const [result, setResult] = useState<{
    key: string;
    paths: string[];
    truncated: boolean;
  } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [reading, setReading] = useState(false);
  const pendingCaret = useRef<number | null>(null);
  const token =
    caret === null
      ? null
      : draft.slice(0, caret).match(/(?:^|\s)@([^\s@"`]*)$/);
  const query = token?.[1] ?? null;
  const open = enabled && focused && !dismissed && query !== null;
  const key = JSON.stringify([project, query, refresh]);
  const paths = open && result?.key === key ? result.paths : [];
  const loading = open && result?.key !== key && !error;
  const selected = Math.min(active, Math.max(0, paths.length - 1));
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      revision.current++;
    };
  }, []);
  useEffect(() => {
    revision.current++;
    setReading(false);
  }, [enabled]);
  useLayoutEffect(() => {
    if (pendingCaret.current === null) return;
    input.current?.setSelectionRange(
      pendingCaret.current,
      pendingCaret.current,
    );
    pendingCaret.current = null;
  }, [draft, input]);
  useEffect(() => {
    if (!open) return;
    let disposed = false;
    setError(null);
    const timer = window.setTimeout(() => {
      void invoke<{ matches: { path: string }[]; truncated: boolean }>(
        "search_project_files",
        {
          projectPath: project,
          query,
          mode: "files",
          includeIgnored: false,
        },
      ).then(
        (value) => {
          if (!disposed)
            setResult({
              key,
              paths: value.matches.slice(0, 40).map((match) => match.path),
              truncated: value.truncated || value.matches.length > 40,
            });
        },
        (reason) => {
          if (!disposed) setError(String(reason));
        },
      );
    }, 160);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, [open, key, project, query]);
  useEffect(() => {
    list.current
      ?.querySelector('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [selected, result]);
  function changed() {
    revision.current++;
    setReading(false);
    setError(null);
    setDismissed(false);
    setActive(0);
    const area = input.current;
    setCaret(
      area && area.selectionStart === area.selectionEnd
        ? area.selectionStart
        : null,
    );
  }
  function dismiss() {
    revision.current++;
    setReading(false);
    setDismissed(true);
  }
  async function choose(path: string) {
    if (!open || reading || caret === null || query === null) return;
    const version = ++revision.current;
    const start = caret - query.length - 1;
    const end =
      caret + (draft.slice(caret).match(/^[^\s@"`]*/)?.[0].length ?? 0);
    setReading(true);
    setError(null);
    const current = () =>
      mounted.current &&
      version === revision.current &&
      latest.current.enabled &&
      latest.current.draft === draft;
    try {
      const file = await invoke<{
        path: string;
        content: string;
        truncated: boolean;
      }>("read_project_file", { projectPath: project, path });
      if (!current()) return;
      if (
        !latest.current.attach(
          createFileContext(file.path, file.content, undefined, file.truncated),
        )
      )
        throw new Error(
          "Could not attach this file. Attach up to 16 files or selections per message.",
        );
      const mention = `@${/\s/.test(path) ? JSON.stringify(path) : path}`;
      const suffix = draft.slice(end);
      const replacement = mention + (suffix.startsWith(" ") ? "" : " ");
      pendingCaret.current =
        start + replacement.length + (suffix.startsWith(" ") ? 1 : 0);
      setCaret(pendingCaret.current);
      setDraft(draft.slice(0, start) + replacement + suffix);
      dismiss();
      input.current?.focus({ preventScroll: true });
    } catch (reason) {
      if (current()) setError(String(reason));
    } finally {
      if (current()) setReading(false);
    }
  }
  return {
    reading,
    controls: open ? id : undefined,
    activeId: open && paths[selected] ? `${id}-${selected}` : undefined,
    changed,
    focus: () => {
      setFocused(true);
      changed();
    },
    blur: () => {
      setFocused(false);
      dismiss();
    },
    select: () => {
      const area = input.current;
      const next =
        area && area.selectionStart === area.selectionEnd
          ? area.selectionStart
          : null;
      if (next !== caret) changed();
    },
    keyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
      if (
        !open ||
        event.nativeEvent.isComposing ||
        event.shiftKey ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey
      )
        return false;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        dismiss();
        return true;
      }
      if (["ArrowUp", "ArrowDown", "Enter", "Tab"].includes(event.key)) {
        event.preventDefault();
        if (event.key === "ArrowUp" || event.key === "ArrowDown")
          setActive(
            Math.max(
              0,
              Math.min(
                paths.length - 1,
                selected + (event.key === "ArrowDown" ? 1 : -1),
              ),
            ),
          );
        else if (paths[selected]) void choose(paths[selected]);
        return true;
      }
      return false;
    },
    menu: open && (
      <div className="slash-menu">
        <div className="slash-heading">
          Files <span>↑↓ Navigate · Tab / Enter Attach · Esc Close</span>
        </div>
        {(loading || reading) && (
          <div className="slash-note" role="status">
            {reading ? "Attaching snapshot…" : "Finding files…"}
          </div>
        )}
        {error && (
          <div className="slash-note" role="alert">
            {error}{" "}
            <button
              type="button"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => {
                setError(null);
                setRefresh((value) => value + 1);
              }}
            >
              Retry files
            </button>
          </div>
        )}
        <div
          id={id}
          ref={list}
          className="slash-list"
          role="listbox"
          aria-label="File mentions"
          aria-busy={loading || reading}
        >
          {paths.map((path, index) => (
            <button
              type="button"
              role="option"
              key={path}
              id={`${id}-${index}`}
              aria-selected={index === selected}
              disabled={reading}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => setActive(index)}
              onClick={() => void choose(path)}
            >
              <strong>{path}</strong>
            </button>
          ))}
        </div>
        {!loading && !error && !paths.length && (
          <div className="slash-note">No matching files.</div>
        )}
      </div>
    ),
  };
}
