import { useEffect, useId, useMemo, useRef, useState } from "react";
import { CornerDownLeft, History, Paperclip, Search, X } from "lucide-react";
import type { SessionWorkspaceState } from "../../domain/sessions/workspaceState.js";
import { conversationPresentation } from "../../domain/conversation/shellMessages.js";
import "./timeline.css";

export function TimelineDialog({
  session,
  loadHistory,
  reveal,
  close,
  restoreFocus,
}: {
  session: SessionWorkspaceState;
  loadHistory: () => Promise<void>;
  reveal: (id: string) => void;
  close: () => void;
  restoreFocus: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const picked = useRef(false);
  const requested = useRef<string | null>(null);
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const id = useId();
  const messages = useMemo(
    () =>
      conversationPresentation(session.conversation)
        .filter((entry) => entry.kind === "user" && !entry.sourceType)
        .reverse(),
    [session.conversation],
  );
  const matches = messages.filter((entry) =>
    entry.text.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()),
  );
  const selected = Math.min(active, Math.max(0, matches.length - 1));
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    search.current?.focus({ preventScroll: true });
    return () => {
      element.close();
      if (!picked.current) restoreFocus();
    };
  }, []);
  useEffect(() => {
    if (session.isLoadingHistory || session.historyError) return;
    const cursor = session.historyLoaded ? session.nextCursor : "initial";
    if (!cursor) return;
    if (requested.current === cursor) return;
    requested.current = cursor;
    void loadHistory();
  }, [
    session.historyLoaded,
    session.nextCursor,
    session.isLoadingHistory,
    session.historyError,
    loadHistory,
  ]);
  useEffect(() => {
    list.current
      ?.querySelector('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [selected, query]);
  function choose(index: number) {
    const message = matches[index];
    if (!message) return;
    picked.current = true;
    dialog.current?.close();
    close();
    reveal(message.id);
  }
  const error = session.historyError;
  return (
    <dialog
      ref={dialog}
      className="settings-dialog timeline-dialog"
      aria-label="Session timeline"
      onKeyDownCapture={(event) => {
        if (
          event.nativeEvent.isComposing ||
          event.metaKey ||
          event.ctrlKey ||
          event.altKey ||
          event.shiftKey
        )
          return;
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          close();
        } else if (
          ["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)
        ) {
          event.preventDefault();
          if (event.key === "Home" || event.key === "End")
            setActive(
              event.key === "Home" ? 0 : Math.max(0, matches.length - 1),
            );
          else
            setActive(
              Math.max(
                0,
                Math.min(
                  matches.length - 1,
                  selected + (event.key === "ArrowDown" ? 1 : -1),
                ),
              ),
            );
          search.current?.focus({ preventScroll: true });
        } else if (event.key === "Enter") {
          const target = event.target as HTMLElement;
          if (target.closest('button:not([role="option"])')) return;
          event.preventDefault();
          choose(selected);
        }
      }}
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <header className="timeline-heading">
        <span className="timeline-icon">
          <History size={18} aria-hidden="true" />
        </span>
        <div>
          <h1>Session timeline</h1>
        </div>
        <button
          className="icon-button"
          aria-label="Close timeline"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="timeline-search">
        <label className="timeline-search-field">
          <Search size={16} aria-hidden="true" />
          <input
            ref={search}
            type="search"
            aria-label="Search user messages"
            placeholder="Search your messages…"
            value={query}
            aria-controls={id}
            aria-haspopup="listbox"
            aria-autocomplete="list"
            aria-activedescendant={
              matches[selected] ? `${id}-${selected}` : undefined
            }
            onChange={(event) => {
              setQuery(event.target.value);
              setActive(0);
            }}
          />
        </label>
        <div className="timeline-summary">
          <span role="status">
            {matches.length} messages
            {!error && (session.isLoadingHistory || session.nextCursor)
              ? " · Loading earlier history…"
              : ""}
          </span>
        </div>
        {error && (
          <div className="inline-error" role="alert">
            Couldn’t load all history: {error}
            <button
              onClick={() => {
                requested.current = null;
                void loadHistory();
              }}
            >
              Retry timeline
            </button>
          </div>
        )}
      </div>
      <div
        ref={list}
        id={id}
        className="timeline-list"
        role="listbox"
        tabIndex={0}
        aria-activedescendant={
          matches[selected] ? `${id}-${selected}` : undefined
        }
        aria-label="User messages"
      >
        {matches.map((entry, index) => (
          <button
            id={`${id}-${index}`}
            type="button"
            role="option"
            aria-selected={selected === index}
            tabIndex={-1}
            key={entry.id}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => choose(index)}
            onPointerMove={(event) => {
              if (event.movementX || event.movementY) setActive(index);
            }}
          >
            <span className="timeline-rail" aria-hidden="true">
              <span />
            </span>
            <span className="timeline-entry">
              {Boolean(
                entry.fileContext?.length || entry.pullRequestContext?.length,
              ) && (
                <span className="timeline-entry-label">
                  {Boolean(entry.fileContext?.length) && (
                    <small>
                      <Paperclip size={11} aria-hidden="true" />
                      {entry.fileContext!.length} files
                    </small>
                  )}
                  {entry.pullRequestContext?.map((pr) => (
                    <small key={pr.id}>#{pr.number}</small>
                  ))}
                </span>
              )}
              <span className="timeline-excerpt">
                <Excerpt
                  text={
                    entry.text.trim() ||
                    (entry.fileContext?.length
                      ? "File context"
                      : "Image message")
                  }
                  query={query.trim()}
                />
              </span>
            </span>
            <CornerDownLeft
              className="timeline-jump"
              size={15}
              aria-hidden="true"
            />
          </button>
        ))}
        {!matches.length && (
          <p className="timeline-empty">
            <Search size={22} aria-hidden="true" />
            {session.isLoadingHistory
              ? "Finding user messages…"
              : "No matching user messages."}
          </p>
        )}
      </div>
    </dialog>
  );
}

function Excerpt({ text, query }: { text: string; query: string }) {
  const match = query
    ? text.toLocaleLowerCase().indexOf(query.toLocaleLowerCase())
    : -1;
  const start = Math.max(0, match - 80);
  const end = Math.min(
    text.length,
    Math.max(start + 280, match + query.length),
  );
  return (
    <>
      {start > 0 && "…"}
      {match < 0 ? (
        text.slice(start, end)
      ) : (
        <>
          {text.slice(start, match)}
          <mark>{text.slice(match, match + query.length)}</mark>
          {text.slice(match + query.length, end)}
        </>
      )}
      {end < text.length && "…"}
    </>
  );
}
