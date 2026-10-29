import { useLayoutEffect, useMemo, useRef, useState } from "react";
import {
  ArrowDown,
  ChevronDown,
  BrainCircuit,
  LoaderCircle,
  ShieldCheck,
} from "lucide-react";
import type {
  ConversationEntry,
  SessionWorkspaceState,
} from "./workspaceState.js";
import type { ApprovalPrompt } from "./useWorkspace.js";
import { CopyButton, MessageContent } from "./MessageContent.js";
import { ToolActivity } from "./ToolActivity.js";
import { conversationPresentation } from "./shellMessages.js";

function ThinkingTrace({
  entry,
  active,
  onExpand,
}: {
  entry: ConversationEntry;
  active: boolean;
  onExpand: () => void;
}) {
  const preview = entry.text
    .replace(/[#*_`>\[\]]/g, "")
    .replace(/\s+/g, " ")
    .trim();
  return (
    <details
      className="reasoning"
      onToggle={(event) => {
        if (event.currentTarget.open) onExpand();
      }}
    >
      <summary aria-label="Thinking trace">
        <BrainCircuit size={13} />
        <span>
          {entry.status === "interrupted"
            ? "Thinking interrupted"
            : active
              ? "Thinking…"
              : "Thought"}
        </span>
        <span className="reasoning-preview">
          {preview.slice(0, 160)}
          {preview.length > 160 ? "…" : ""}
        </span>
        <ChevronDown size={12} className="reasoning-chevron" />
      </summary>
      <div className="reasoning-content">
        {entry.text ? (
          <MessageContent text={entry.text} />
        ) : (
          <p>No thinking text was provided yet.</p>
        )}
      </div>
    </details>
  );
}

export function ApprovalCard({
  request,
  resolving,
  resolve,
}: {
  request: ApprovalPrompt;
  resolving: boolean;
  resolve: (decision: "allowOnce" | "deny") => void;
}) {
  return (
    <section className="approval-card" aria-label="Approval required">
      <div className="approval-title">
        <ShieldCheck size={16} />
        <strong>{request.summary}</strong>
      </div>
      <p>{request.description}</p>
      <div className="approval-actions">
        <span>One-time permission</span>
        <button disabled={resolving} onClick={() => resolve("deny")}>
          Deny
        </button>
        <button
          className="primary-button"
          disabled={resolving}
          onClick={() => resolve("allowOnce")}
        >
          Allow once
        </button>
      </div>
    </section>
  );
}

export function Conversation({
  session,
  saveView,
  loadOlder,
  retryHistory,
  approvals,
  resolving,
  resolve,
}: {
  session: SessionWorkspaceState;
  saveView: (value: Partial<SessionWorkspaceState>) => void;
  loadOlder: () => Promise<void>;
  retryHistory: () => void;
  approvals: ApprovalPrompt[];
  resolving: boolean;
  resolve: (request: ApprovalPrompt, decision: "allowOnce" | "deny") => void;
}) {
  const scroll = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const following = useRef(session.scrollTop === null);
  const position = useRef(session.scrollTop);
  const [atBottom, setAtBottom] = useState(following.current);
  // A sliding 100-entry window bounds Markdown and tool DOM, while keeping all records in memory.
  const [end, setEnd] = useState<string | null>(session.windowEndId);
  const endRef = useRef(end);
  endRef.current = end;
  const save = useRef(saveView);
  save.current = saveView;
  const entries = useMemo(
    () => conversationPresentation(session.conversation),
    [session.conversation],
  );
  const endIndex =
    end === null ? -1 : entries.findIndex((entry) => entry.id === end);
  const pinnedEnd = endIndex < 0 ? entries.length : endIndex + 1;
  const start = Math.max(0, pinnedEnd - 100);
  // Keep appending while there is room in the window, even when the reader
  // has expanded a trace/tool. Pinning only prevents older entries being evicted.
  const windowEnd =
    endIndex < 0 ? entries.length : Math.min(entries.length, start + 100);
  const anchor = useRef<{ id: string; offset: number } | null>(null);
  const rememberAnchor = () => {
    const container = scroll.current;
    if (!container) return;
    const item = Array.from(
      container.querySelectorAll<HTMLElement>("[data-entry-id]"),
    ).find((node) => node.offsetTop >= container.scrollTop);
    if (item)
      anchor.current = {
        id: item.dataset.entryId!,
        offset: item.offsetTop - container.scrollTop,
      };
  };
  useLayoutEffect(() => {
    const container = scroll.current!;
    container.scrollTop = session.scrollTop ?? container.scrollHeight;
    const observer = new ResizeObserver(() => {
      if (following.current && endRef.current === null)
        container.scrollTop = container.scrollHeight;
    });
    if (content.current) observer.observe(content.current);
    return () => {
      observer.disconnect();
      save.current({
        scrollTop: following.current ? null : position.current,
        windowEndId: endRef.current,
      });
    };
  }, []);
  useLayoutEffect(() => {
    const container = scroll.current!;
    if (anchor.current) {
      const item = Array.from(
        container.querySelectorAll<HTMLElement>("[data-entry-id]"),
      ).find((node) => node.dataset.entryId === anchor.current?.id);
      if (item) container.scrollTop = item.offsetTop - anchor.current.offset;
      anchor.current = null;
    } else if (following.current && end === null)
      container.scrollTop = container.scrollHeight;
  }, [entries, end, approvals.length]);
  const followRevision = useRef(session.followRevision);
  useLayoutEffect(() => {
    if (followRevision.current === session.followRevision) return;
    followRevision.current = session.followRevision;
    anchor.current = null;
    jump();
  }, [session.followRevision]);
  function jump() {
    following.current = true;
    setAtBottom(true);
    setEnd(null);
    position.current = null;
    requestAnimationFrame(() => {
      if (scroll.current)
        scroll.current.scrollTop = scroll.current.scrollHeight;
    });
  }
  async function older() {
    rememberAnchor();
    following.current = false;
    setAtBottom(false);
    if (start > 0) {
      setEnd(entries[Math.max(100, windowEnd - 80) - 1].id);
      return;
    }
    setEnd(entries[windowEnd - 1]?.id ?? null);
    await loadOlder();
  }
  return (
    <div className="transcript-wrap">
      <div
        ref={scroll}
        className="transcript"
        aria-label="Conversation"
        onScroll={() => {
          const node = scroll.current!;
          const near =
            node.scrollHeight - node.scrollTop - node.clientHeight < 64 &&
            windowEnd === entries.length;
          following.current = near;
          position.current = node.scrollTop;
          setAtBottom(near);
          if (!near && end === null)
            setEnd(entries[entries.length - 1]?.id ?? null);
          if (near && end !== null) setEnd(null);
        }}
      >
        <div ref={content} className="transcript-content">
          {(start > 0 || session.nextCursor) && (
            <button
              className="history-button"
              onClick={() => void older()}
              disabled={session.isLoadingHistory}
            >
              {session.isLoadingHistory
                ? "Loading…"
                : "Load earlier messages"}
            </button>
          )}
          {session.historyError && (
            <div className="inline-error" role="alert">
              <span>History could not be loaded: {session.historyError}</span>
              <button onClick={retryHistory}>Retry</button>
            </div>
          )}
          {!session.historyLoaded && session.isLoadingHistory && (
            <div className="session-empty">
              <LoaderCircle className="spinning" size={18} />
              <span>Loading conversation…</span>
            </div>
          )}
          {entries.slice(start, windowEnd).map((entry) => (
            <article
              key={entry.id}
              data-entry-id={entry.id}
              className={`conversation-entry ${entry.kind}`}
            >
              {entry.kind === "tool" ? (
                <ToolActivity
                  entry={entry}
                  expanded={
                    session.expandedTools[entry.id] ??
                    entry.status === "failed"
                  }
                  toggle={() => {
                    following.current = false;
                    setAtBottom(false);
                    setEnd(entries[windowEnd - 1]?.id ?? null);
                    saveView({
                      expandedTools: {
                        ...session.expandedTools,
                        [entry.id]: !(
                          session.expandedTools[entry.id] ??
                          entry.status === "failed"
                        ),
                      },
                    });
                  }}
                />
              ) : entry.kind === "reasoning" ? (
                <ThinkingTrace
                  entry={entry}
                  active={
                    session.isWorking &&
                    !entry.complete &&
                    entry.status !== "interrupted"
                  }
                  onExpand={() => {
                    following.current = false;
                    setAtBottom(false);
                    setEnd(entries[windowEnd - 1]?.id ?? null);
                  }}
                />
              ) : entry.kind === "status" ? (
                <div className="turn-status">{entry.text}</div>
              ) : (
                <>
                  <div className="message-heading">
                    <span>
                      {entry.kind === "user"
                        ? "You"
                        : session.session.provider === "codex"
                          ? "Codex"
                          : "OpenCode"}
                    </span>
                    <CopyButton text={entry.text} label="Copy message" />
                  </div>
                  {entry.kind === "user" ? (
                    <div className="user-text">{entry.text}</div>
                  ) : (
                    <MessageContent text={entry.text} />
                  )}
                </>
              )}
            </article>
          ))}
          {windowEnd < entries.length && (
            <button
              className="history-button"
              onClick={() => {
                rememberAnchor();
                setEnd(
                  entries[Math.min(entries.length, windowEnd + 80) - 1].id,
                );
              }}
            >
              Newer messages <ChevronDown size={13} />
            </button>
          )}
          {windowEnd === entries.length && (
            <>
              {approvals.map((request) => (
                <ApprovalCard
                  key={request.requestId}
                  request={request}
                  resolving={resolving}
                  resolve={(decision) => resolve(request, decision)}
                />
              ))}
              {!session.isWorking && session.lastOutcome && (
                <div className="turn-status" role="status">
                  {["completed", "succeeded", "idle"].includes(
                    session.lastOutcome,
                  )
                    ? "Done"
                    : session.lastOutcome === "interrupted"
                      ? "Interrupted"
                      : "Turn ended"}
                </div>
              )}
            </>
          )}
        </div>
      </div>
      {(!atBottom || windowEnd < entries.length) && (
        <button className="jump-latest" onClick={jump}>
          <ArrowDown size={13} />
          Jump to latest{approvals.length > 0 ? " · Approval required" : ""}
        </button>
      )}
    </div>
  );
}
