import { useEffect, useId, useMemo, useRef, useState } from "react";
import { X } from "lucide-react";
import type { SessionWorkspaceState } from "../sessions/workspaceState.js";
import { conversationPresentation } from "./shellMessages.js";
import "./timeline.css";

export function TimelineDialog({
  session,
  loadHistory,
  reveal,
  close,
}: {
  session: SessionWorkspaceState;
  loadHistory: () => Promise<void>;
  reveal: (id: string) => void;
  close: () => void;
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
    const previous = document.activeElement;
    element.showModal();
    search.current?.focus({ preventScroll: true });
    return () => {
      element.close();
      if (
        !picked.current &&
        previous instanceof HTMLElement &&
        previous.isConnected
      )
        previous.focus({ preventScroll: true });
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
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <header className="settings-heading">
        <div>
          <h1>Session timeline</h1>
          <p>Jump to an earlier user message · newest first</p>
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
        <input
          ref={search}
          type="search"
          aria-label="Search user messages"
          placeholder="Search your messages…"
          value={query}
          aria-controls={id}
          aria-autocomplete="list"
          aria-activedescendant={
            matches[selected] ? `${id}-${selected}` : undefined
          }
          onChange={(event) => {
            setQuery(event.target.value);
            setActive(0);
          }}
          onKeyDown={(event) => {
            if (
              event.nativeEvent.isComposing ||
              event.metaKey ||
              event.ctrlKey ||
              event.altKey ||
              event.shiftKey
            )
              return;
            if (event.key === "ArrowDown" || event.key === "ArrowUp") {
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
            } else if (event.key === "Enter") {
              event.preventDefault();
              choose(selected);
            }
          }}
        />
        <span role="status">
          {matches.length} messages
          {error
            ? " · Earlier history unavailable"
            : session.isLoadingHistory || session.nextCursor
              ? " · Loading earlier history…"
              : " · All history loaded"}
        </span>
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
        aria-label="User messages"
      >
        {matches.map((entry, index) => (
          <button
            id={`${id}-${index}`}
            type="button"
            role="option"
            aria-selected={selected === index}
            key={entry.id}
            onMouseDown={(event) => event.preventDefault()}
            onClick={() => choose(index)}
            onMouseEnter={() => setActive(index)}
          >
            <span>
              {entry.text.trim().slice(0, 320) ||
                (entry.fileContext?.length ? "File context" : "Image message")}
              {entry.text.trim().length > 320 ? "…" : ""}
            </span>
            {Boolean(entry.fileContext?.length) && (
              <small>{entry.fileContext!.length} attached snapshots</small>
            )}
          </button>
        ))}
        {!matches.length && (
          <p>
            {session.isLoadingHistory
              ? "Finding user messages…"
              : "No matching user messages."}
          </p>
        )}
      </div>
      <footer className="settings-footer">
        <span>↑↓ Navigate · Enter Jump · Esc Close</span>
        <button onClick={close}>Done</button>
      </footer>
    </dialog>
  );
}
