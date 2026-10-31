import {
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";
import { invoke } from "@tauri-apps/api/core";
import type { SlashCommand } from "./slashCommands.js";

export function useSlashCommands({
  project,
  enabled,
  draft,
  setDraft,
  run,
  input,
}: {
  project: string;
  enabled: boolean;
  draft: string;
  setDraft: (text: string) => void;
  run: (text: string) => void;
  input: RefObject<HTMLTextAreaElement | null>;
}) {
  const id = useId();
  const list = useRef<HTMLDivElement>(null);
  const [commands, setCommands] = useState<SlashCommand[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const [active, setActive] = useState(0);
  const [focused, setFocused] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const query = draft.startsWith("//")
    ? null
    : (draft.match(/^\/([^\s]*)$/)?.[1] ?? null);
  const open = enabled && focused && !dismissed && query !== null;
  useEffect(() => {
    if (!open) return;
    let disposed = false;
    setLoading(true);
    setError(null);
    setCommands([]);
    void invoke<SlashCommand[]>("opencode_commands", { projectPath: project })
      .then((value) => {
        if (!disposed) setCommands(value);
      })
      .catch((reason) => {
        if (!disposed) setError(String(reason));
      })
      .finally(() => {
        if (!disposed) setLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [project, open, refresh]);
  const matches = [
    {
      name: "usage",
      description:
        "View OpenCode Go allowances, Console budgets and session context",
    },
    ...commands.filter((command) => command.name !== "usage"),
  ].filter((command) =>
    command.name.toLowerCase().includes((query ?? "").toLowerCase()),
  );
  const selected = Math.min(active, Math.max(0, matches.length - 1));
  useEffect(() => {
    const option = list.current?.querySelector<HTMLElement>(
      '[aria-selected="true"]',
    );
    if (option && list.current) {
      const bounds = list.current.getBoundingClientRect();
      const row = option.getBoundingClientRect();
      if (row.top < bounds.top) list.current.scrollTop -= bounds.top - row.top;
      else if (row.bottom > bounds.bottom)
        list.current.scrollTop += row.bottom - bounds.bottom;
    }
  }, [selected, query, commands]);
  function choose(command: SlashCommand) {
    setDraft(`/${command.name} `);
    setDismissed(true);
    input.current?.focus({ preventScroll: true });
  }
  return {
    controls: open ? id : undefined,
    activeId: open && matches[selected] ? `${id}-${selected}` : undefined,
    focus: () => setFocused(true),
    blur: () => setFocused(false),
    changed: () => {
      setDismissed(false);
      setActive(0);
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
        setDismissed(true);
        return true;
      }
      if (["ArrowUp", "ArrowDown"].includes(event.key)) {
        event.preventDefault();
        setActive(
          Math.max(
            0,
            Math.min(
              matches.length - 1,
              selected + (event.key === "ArrowDown" ? 1 : -1),
            ),
          ),
        );
        return true;
      }
      if ((event.key === "Tab" || event.key === "Enter") && matches[selected]) {
        event.preventDefault();
        if (event.key === "Tab") choose(matches[selected]);
        else {
          const text = `/${matches[selected].name}`;
          setDraft(text);
          setDismissed(true);
          run(text);
        }
        return true;
      }
      return false;
    },
    menu: open && (
      <div className="slash-menu">
        <div className="slash-heading">
          Commands{" "}
          <span>↑↓ Navigate · Tab Complete · Enter Run · Esc Close</span>
        </div>
        {loading && (
          <div className="slash-note" role="status">
            Loading OpenCode commands…
          </div>
        )}
        {error && (
          <div className="slash-note" role="alert">
            {error}{" "}
            <button
              type="button"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => setRefresh((value) => value + 1)}
            >
              Retry commands
            </button>
          </div>
        )}
        <div
          id={id}
          ref={list}
          className="slash-list"
          role="listbox"
          aria-label="Slash commands"
        >
          {matches.map((command, index) => (
            <button
              type="button"
              role="option"
              key={command.name}
              id={`${id}-${index}`}
              aria-selected={index === selected}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => setActive(index)}
              onClick={() => choose(command)}
            >
              <strong>/{command.name}</strong>
              <span>{command.description}</span>
            </button>
          ))}
        </div>
        {!loading && !error && !matches.length && (
          <div className="slash-note">No matching commands.</div>
        )}
      </div>
    ),
  };
}
