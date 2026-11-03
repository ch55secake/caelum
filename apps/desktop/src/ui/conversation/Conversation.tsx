import {
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type Ref,
} from "react";
import {
  ArrowDown,
  ChevronDown,
  BrainCircuit,
  LoaderCircle,
  ShieldCheck,
  Sparkles,
  Minimize2,
} from "lucide-react";
import type {
  ConversationEntry,
  SessionWorkspaceState,
} from "../../domain/sessions/workspaceState.js";
import type { ApprovalPrompt } from "../../application/workspace/useWorkspace.js";
import { CopyButton, MessageContent } from "./MessageContent.js";
import { ToolActivity } from "./ToolActivity.js";
import { conversationPresentation } from "../../domain/conversation/shellMessages.js";
import { useContextMenu, copyAction } from "../primitives/ContextMenu.js";
import { FileContextChips } from "../files/FileNavigation.js";
import { PullRequestContextChips } from "../pull-requests/PullRequestContextChips.js";
import { ChatImages } from "./ChatImage.js";
import { useLatestView } from "./useLatestView.js";

function CompactionNotice({
  entry,
  working,
  expanded,
  toggle,
}: {
  entry: ConversationEntry;
  working: boolean;
  expanded: boolean;
  toggle: () => void;
}) {
  const status =
    entry.status === "running" && !working ? "interrupted" : entry.status;
  const label =
    status === "running"
      ? "Compacting context…"
      : status === "failed"
        ? "Compaction failed"
        : status === "interrupted"
          ? "Compaction interrupted"
          : "Context compacted";
  const hasDetails = Boolean(entry.text || entry.error);
  const heading = (
    <>
      {status === "running" ? (
        <LoaderCircle size={13} className="spinning" aria-hidden="true" />
      ) : (
        <Minimize2 size={13} aria-hidden="true" />
      )}
      <span role="status">{label}</span>
      {hasDetails && (
        <ChevronDown
          size={12}
          className={expanded ? "expanded" : undefined}
          aria-hidden="true"
        />
      )}
    </>
  );
  return (
    <section
      className="compaction-notice"
      aria-label="Context compaction"
      data-status={status}
    >
      {hasDetails ? (
        <button
          className="compaction-heading"
          aria-label={label}
          aria-expanded={expanded}
          onClick={toggle}
        >
          {heading}
        </button>
      ) : (
        <div className="compaction-heading">{heading}</div>
      )}
      {hasDetails && expanded && (
        <div className="compaction-details">
          {entry.error && (
            <p className="compaction-error" role="alert">
              {entry.error}
            </p>
          )}
          {entry.text && <MessageContent text={entry.text} />}
        </div>
      )}
    </section>
  );
}

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

export type ConversationHandle = {
  reveal: (id: string) => void;
  restoreScroll: () => void;
};

export function Conversation({
  ref,
  session,
  saveView,
  loadOlder,
  retryHistory,
  approvals,
  resolving,
  resolve,
  addToDraft,
  reportLatestView,
}: {
  ref?: Ref<ConversationHandle>;
  session: SessionWorkspaceState;
  saveView: (value: Partial<SessionWorkspaceState>) => void;
  loadOlder: () => Promise<void>;
  retryHistory: () => void;
  approvals: ApprovalPrompt[];
  resolving: boolean;
  resolve: (request: ApprovalPrompt, decision: "allowOnce" | "deny") => void;
  addToDraft: (text: string) => void;
  reportLatestView: (visible: boolean) => void;
}) {
  const contextMenu = useContextMenu();
  const scroll = useRef<HTMLDivElement>(null);
  const content = useRef<HTMLDivElement>(null);
  const following = useRef(session.scrollTop === null);
  const position = useRef(session.scrollTop);
  const dimensions = useRef({ height: 0, content: 0 });
  const [atBottom, setAtBottom] = useState(following.current);
  // A sliding 100-entry window bounds Markdown and tool DOM, while keeping all records in memory.
  const [end, setEnd] = useState<string | null>(session.windowEndId);
  const [target, setTarget] = useState<{ id: string } | null>(null);
  const endRef = useRef(end);
  endRef.current = end;
  const save = useRef(saveView);
  save.current = saveView;
  function followResize() {
    const container = scroll.current!;
    const next = {
      height: container.clientHeight,
      content: container.scrollHeight,
    };
    const previous = dimensions.current;
    if (next.height === previous.height && next.content === previous.content)
      return false;
    dimensions.current = next;
    if (following.current && endRef.current === null)
      container.scrollTop = container.scrollHeight;
    return true;
  }
  const entries = useMemo(
    () => conversationPresentation(session.conversation),
    [session.conversation],
  );
  const latestUserId = useMemo(
    () => [...entries].reverse().find((entry) => entry.kind === "user")?.id,
    [entries],
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
  useImperativeHandle(
    ref,
    () => ({
      restoreScroll() {
        const container = scroll.current!;
        // Replacing the composer can briefly clamp WebKit's scroll position,
        // even when both modes finish at the same height (no resize event).
        container.scrollTop =
          following.current && endRef.current === null
            ? container.scrollHeight
            : (position.current ?? container.scrollTop);
      },
      reveal(id) {
        const index = entries.findIndex((entry) => entry.id === id);
        if (index < 0) return;
        anchor.current = null;
        following.current = false;
        setAtBottom(false);
        setEnd(
          entries[Math.min(entries.length, Math.max(100, index + 50)) - 1].id,
        );
        setTarget({ id });
      },
    }),
    [entries],
  );
  useLayoutEffect(() => {
    const container = scroll.current!;
    container.scrollTop = session.scrollTop ?? container.scrollHeight;
    followResize();
    const observer = new ResizeObserver(followResize);
    observer.observe(container);
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
    if (session.isLoadingHistory && !following.current) rememberAnchor();
  }, [session.isLoadingHistory]);
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
  useLayoutEffect(() => {
    if (!target) return;
    const container = scroll.current!;
    const item = Array.from(
      container.querySelectorAll<HTMLElement>("[data-entry-id]"),
    ).find((node) => node.dataset.entryId === target.id);
    if (!item) return;
    container.scrollTop = Math.max(0, item.offsetTop - 24);
    position.current = container.scrollTop;
    item.focus({ preventScroll: true });
    rememberAnchor();
  }, [target]);
  const followRevision = useRef(session.followRevision);
  useLayoutEffect(() => {
    if (followRevision.current === session.followRevision) return;
    followRevision.current = session.followRevision;
    anchor.current = null;
    jump();
  }, [session.followRevision]);
  useLatestView(
    scroll,
    session.historyLoaded && windowEnd === entries.length,
    reportLatestView,
  );
  function jump() {
    anchor.current = null;
    setTarget(null);
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
          // Layout-induced scroll events can arrive before ResizeObserver.
          // Preserve the follow intent through composer and content resizes.
          if (followResize()) return;
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
              {session.isLoadingHistory ? "Loading…" : "Load earlier messages"}
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
              tabIndex={-1}
              data-timeline-target={target?.id === entry.id || undefined}
              className={`conversation-entry ${entry.kind}${entry.id === latestUserId ? " latest-user" : ""}`}
              onContextMenu={
                entry.kind === "tool"
                  ? undefined
                  : (event) =>
                      contextMenu(event, [
                        copyAction(
                          entry.kind === "reasoning"
                            ? "Copy thinking"
                            : "Copy message",
                          entry.text,
                        ),
                        {
                          text: "Add to draft",
                          enabled:
                            Boolean(entry.text) &&
                            session.connection === "connected",
                          action: () => addToDraft(entry.text),
                        },
                      ])
              }
            >
              {entry.kind === "tool" ? (
                <ToolActivity
                  entry={entry}
                  expanded={
                    session.expandedTools[entry.id] ?? entry.status === "failed"
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
              ) : entry.kind === "compaction" ? (
                <CompactionNotice
                  entry={entry}
                  working={session.isWorking}
                  expanded={
                    session.expandedTools[entry.id] ?? entry.status === "failed"
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
              ) : entry.kind === "status" ? (
                <div className="turn-status">{entry.text}</div>
              ) : (
                <>
                  {entry.kind === "user" ? (
                    <CopyButton text={entry.text} label="Copy message" />
                  ) : (
                    <div className="message-heading">
                      <span>
                        <Sparkles size={13} aria-hidden="true" />
                        {session.session.provider === "codex"
                          ? "Codex"
                          : "OpenCode"}
                      </span>
                      <CopyButton text={entry.text} label="Copy message" />
                    </div>
                  )}
                  {entry.kind === "user" ? (
                    <>
                      <div className="user-text">{entry.text}</div>
                      {entry.fileContext && (
                        <FileContextChips files={entry.fileContext} />
                      )}
                      {entry.pullRequestContext && (
                        <PullRequestContextChips
                          pullRequests={entry.pullRequestContext}
                        />
                      )}
                    </>
                  ) : (
                    <MessageContent text={entry.text} />
                  )}
                  <ChatImages images={entry.images} />
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
              {!session.isWorking &&
                session.lastOutcome &&
                !["completed", "succeeded", "idle"].includes(
                  session.lastOutcome,
                ) && (
                  <div className="turn-status" role="status">
                    {session.lastOutcome === "interrupted"
                      ? "Turn cancelled"
                      : session.lastOutcome === "paused"
                        ? "Execution paused"
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
