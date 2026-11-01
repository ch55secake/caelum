import { memo, useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  Check,
  ChevronRight,
  ExternalLink,
  RefreshCw,
  Send,
} from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { DiffSection } from "./DiffSection.js";
import { PullRequestFiles } from "./PullRequestFiles.js";
import { PageStatus } from "./PullRequestReviewStatus.js";
import { usePullRequestReview } from "./usePullRequestReview.js";
import {
  feedbackSelectedByDefault,
  reviewPrompt,
  type ReviewComment,
  type ReviewFeedback,
  type ReviewThread,
} from "./pullRequestReviewState.js";
import type { PullRequestDetails } from "./pullRequestState.js";

export type ReviewSession = {
  name: string;
  disabledReason: string | null;
  address: (prompt: string) => Promise<boolean>;
};
type Props = {
  projectPath: string;
  pr: PullRequestDetails;
  open: (url: string) => void;
};
const unique = <T extends { id: string }>(items: T[]) => [
  ...new Map(items.map((item) => [item.id, item])).values(),
];

const Comment = memo(function Comment({
  comment,
  open,
}: {
  comment: ReviewComment;
  open: Props["open"];
}) {
  const date = new Date(comment.createdAt);
  return (
    <article className="pr-comment">
      <header>
        <strong>{comment.author?.login ?? "Deleted user"}</strong>
        {comment.state && (
          <span>{comment.state.toLowerCase().replaceAll("_", " ")}</span>
        )}
        {Number.isFinite(date.getTime()) && (
          <time dateTime={comment.createdAt} title={date.toLocaleString()}>
            {date.toLocaleDateString()}
          </time>
        )}
        {/^https:\/\//i.test(comment.url) && (
          <a
            href={comment.url}
            onClick={(event) => {
              event.preventDefault();
              open(comment.url);
            }}
            aria-label={`Open comment by ${comment.author?.login ?? "deleted user"} on GitHub`}
          >
            <ExternalLink size={12} />
          </a>
        )}
      </header>
      <div className="markdown pr-comment-body">
        <Markdown
          remarkPlugins={[remarkGfm]}
          skipHtml
          components={{
            img: ({ alt }) => <span>{alt}</span>,
            table: ({ children }) => (
              <div className="markdown-table">
                <table>{children}</table>
              </div>
            ),
            a: ({ href, children }) =>
              href && /^https?:\/\//i.test(href) ? (
                <a
                  href={href}
                  onClick={(event) => {
                    event.preventDefault();
                    open(href);
                  }}
                >
                  {children}
                </a>
              ) : (
                <span>{children}</span>
              ),
          }}
        >
          {comment.body || "No review summary."}
        </Markdown>
      </div>
    </article>
  );
});

function ThreadReplies({
  thread,
  projectPath,
  pr,
  received,
  enabled,
  enable,
}: Props & {
  thread: ReviewThread;
  received: (comments: ReviewComment[], nextCursor: string | null) => void;
  enabled: boolean;
  enable: () => void;
}) {
  const replies = usePullRequestReview(
    {
      projectPath,
      url: pr.url,
      headOid: pr.headRefOid,
      kind: "replies",
      threadId: thread.id,
      cursor: thread.nextCursor ?? undefined,
    },
    enabled,
  );
  const callback = useRef(received);
  callback.current = received;
  useEffect(() => {
    if (enabled && replies.pages.length)
      callback.current(
        replies.pages.flatMap((page) => page.comments ?? []),
        replies.loading || replies.error
          ? thread.nextCursor
          : (replies.pages.at(-1)?.nextCursor ?? null),
      );
  }, [
    enabled,
    replies.pages,
    replies.loading,
    replies.error,
    thread.nextCursor,
  ]);
  if (!enabled)
    return (
      <button type="button" onClick={enable}>
        Load remaining replies
      </button>
    );
  return <PageStatus resource={replies} label="replies" />;
}

export function PullRequestReview({
  projectPath,
  pr,
  open,
  session,
  active,
}: Props & { session?: ReviewSession; active: boolean }) {
  const target = { projectPath, url: pr.url, headOid: pr.headRefOid };
  const discussion = usePullRequestReview({ ...target, kind: "discussion" });
  const reviews = usePullRequestReview({ ...target, kind: "reviews" });
  const threads = usePullRequestReview({ ...target, kind: "threads" });
  const reviewThreads = useMemo(
    () => unique(threads.pages.flatMap((page) => page.threads ?? [])),
    [threads.pages],
  );
  const [choices, setChoices] = useState<Record<string, boolean>>({});
  const [replies, setReplies] = useState<
    Record<
      string,
      {
        thread: ReviewThread;
        version: number;
        comments: ReviewComment[];
        nextCursor: string | null;
      }
    >
  >({});
  const [sending, setSending] = useState(false);
  const sendingRef = useRef(false);
  const [error, setError] = useState("");
  const [refreshId, setRefreshId] = useState(0);
  const [requestedReplies, setRequestedReplies] = useState(
    () => new Set<string>(),
  );
  const resources = [discussion, reviews, threads];
  const discussionItems = useMemo(
    () =>
      unique(discussion.pages.flatMap((page) => page.comments ?? [])).map(
        (comment): ReviewFeedback => ({
          id: comment.id,
          kind: "discussion",
          comments: [comment],
        }),
      ),
    [discussion.pages],
  );
  const reviewItems = useMemo(
    () =>
      unique(reviews.pages.flatMap((page) => page.comments ?? [])).map(
        (comment): ReviewFeedback => ({
          id: comment.id,
          kind: "review",
          comments: [comment],
        }),
      ),
    [reviews.pages],
  );
  const threadItems = useMemo(
    () =>
      reviewThreads.map((thread): ReviewFeedback => {
        const cached = replies[thread.id];
        const extra =
          cached?.thread === thread && cached.version === threads.version
            ? cached
            : undefined;
        const comments = unique([
          ...thread.comments,
          ...(extra?.comments ?? []),
        ]);
        return {
          id: thread.id,
          kind: "thread",
          comments,
          thread: {
            ...thread,
            comments,
            nextCursor: extra ? extra.nextCursor : thread.nextCursor,
          },
        };
      }),
    [reviewThreads, replies, threads.version],
  );
  const groups = [
    {
      id: "discussion",
      title: "Discussion",
      label: "discussion comments",
      resource: discussion,
      items: discussionItems,
    },
    {
      id: "reviews",
      title: "Reviews",
      label: "reviews",
      resource: reviews,
      items: reviewItems,
    },
  ];
  const selected = useMemo(
    () =>
      [...discussionItems, ...reviewItems, ...threadItems].filter(
        (item) =>
          !item.thread?.nextCursor &&
          item.comments.some((comment) => comment.body.trim()) &&
          (choices[item.id] ?? feedbackSelectedByDefault(item)),
      ),
    [discussionItems, reviewItems, threadItems, choices],
  );
  const loading = resources.some(
    (resource) =>
      resource.loading || (!resource.pages.length && !resource.error),
  );
  const stale = resources.some((resource) => resource.error);
  const refresh = () => {
    resources.forEach((resource) => resource.refresh());
    setChoices({});
    setReplies({});
    setRequestedReplies(new Set());
    setRefreshId((current) => current + 1);
    setError("");
  };
  const address = async () => {
    if (
      sendingRef.current ||
      !session ||
      session.disabledReason ||
      loading ||
      stale ||
      !selected.length
    )
      return;
    sendingRef.current = true;
    setSending(true);
    setError("");
    try {
      if (!(await session.address(reviewPrompt(pr, selected))))
        throw new Error(
          "The current session is not ready. Reconnect or wait for its turn to finish.",
        );
    } catch (reason) {
      setError(`Could not send review feedback. ${String(reason)}`);
    } finally {
      sendingRef.current = false;
      setSending(false);
    }
  };
  const renderFeedback = useCallback(
    (item: ReviewFeedback, inline = false) => {
      const thread = item.thread;
      const original =
        thread && reviewThreads.find((value) => value.id === thread.id);
      const replyKey = `${refreshId}:${threads.version}:${original?.id}:${original?.nextCursor}`;
      const label = thread
        ? `${thread.path}${thread.line || thread.originalLine ? `:${thread.line ?? thread.originalLine}` : ""}`
        : `comment by ${item.comments[0]?.author?.login ?? "deleted user"}`;
      const isSelected = selected.some((value) => value.id === item.id);
      const content = (
        <>
          <div className="pr-feedback-heading">
            <label>
              <input
                type="checkbox"
                checked={isSelected}
                disabled={
                  Boolean(thread?.nextCursor) ||
                  sending ||
                  !item.comments.some((comment) => comment.body.trim())
                }
                onChange={(event) =>
                  setChoices((current) => ({
                    ...current,
                    [item.id]: event.target.checked,
                  }))
                }
                aria-label={`Include ${label}`}
              />
              <span>{label}</span>
            </label>
            {thread && (
              <>
                <span>
                  {thread.diffSide === "LEFT" ? "Original" : "New"}{" "}
                  {thread.startLine
                    ? `lines ${thread.startLine}–${thread.line ?? thread.originalLine}`
                    : "side"}
                </span>
                <span className="pr-thread-state">
                  {thread.isResolved ? "Resolved" : "Unresolved"}
                </span>
                {thread.isOutdated && (
                  <span className="pr-thread-state">Outdated</span>
                )}
              </>
            )}
          </div>
          {thread && !inline && item.comments[0]?.diffHunk && (
            <details className="pr-comment-context">
              <summary>Diff context</summary>
              <DiffSection
                title="Review context"
                path={thread.path}
                content={item.comments[0].diffHunk}
                numbered
              />
            </details>
          )}
          {item.comments.map((comment) => (
            <Comment key={comment.id} comment={comment} open={open} />
          ))}
          {original?.nextCursor && thread?.nextCursor && (
            <ThreadReplies
              key={replyKey}
              thread={original}
              projectPath={projectPath}
              pr={pr}
              open={open}
              enabled={requestedReplies.has(replyKey)}
              enable={() =>
                setRequestedReplies((current) => new Set(current).add(replyKey))
              }
              received={(comments, nextCursor) =>
                setReplies((current) => ({
                  ...current,
                  [original.id]: {
                    thread: original,
                    version: threads.version,
                    comments,
                    nextCursor,
                  },
                }))
              }
            />
          )}
        </>
      );
      return (
        <div
          className="pr-feedback"
          key={item.id}
          data-thread-id={thread?.id}
          tabIndex={-1}
        >
          {thread?.isResolved ? (
            <details className="pr-resolved-thread">
              <summary>
                <ChevronRight size={13} className="pr-resolved-chevron" />
                <Check size={13} className="pr-resolved-check" />
                <strong>Resolved</strong>
                <span className="pr-resolved-location">{label}</span>
                {thread.isOutdated && (
                  <span className="pr-thread-state">Outdated</span>
                )}
                {isSelected && (
                  <span className="pr-thread-state">Selected</span>
                )}
              </summary>
              {content}
            </details>
          ) : (
            content
          )}
        </div>
      );
    },
    [
      reviewThreads,
      refreshId,
      threads.version,
      selected,
      sending,
      open,
      projectPath,
      pr,
      requestedReplies,
    ],
  );
  return (
    <PullRequestFiles
      projectPath={projectPath}
      pr={pr}
      active={active}
      feedback={threadItems}
      renderFeedback={renderFeedback}
      reviewLinks={groups.map((group) => ({
        id: group.id,
        title: group.title,
        count: group.items.length,
      }))}
      threadStatus={<PageStatus resource={threads} label="review threads" />}
      reviewActions={
        <>
          <div className="pr-address-comments">
            <div>
              <strong>
                {session
                  ? `Send to ${session.name}`
                  : "Select a session to address feedback"}
              </strong>
              <p>
                {session?.disabledReason ??
                  `${selected.length} selected · unresolved, current threads selected by default`}
              </p>
            </div>
            <button
              type="button"
              onClick={refresh}
              disabled={loading || sending}
            >
              <RefreshCw size={13} /> Refresh comments
            </button>
            <button
              type="button"
              onClick={() => void address()}
              disabled={
                !session ||
                Boolean(session.disabledReason) ||
                !selected.length ||
                loading ||
                Boolean(stale) ||
                sending
              }
            >
              <Send size={13} />
              {sending ? "Sending…" : "Address selected comments"}
            </button>
          </div>
          {error && (
            <p className="pr-error" role="alert">
              {error}
            </p>
          )}
          {resources.some((resource) => resource.hasMore) && (
            <p className="pr-muted">
              More feedback is available. The action includes only the selected,
              loaded comments.
            </p>
          )}
        </>
      }
    >
      {groups.map((group) => (
        <section
          className="pr-feedback-group"
          key={group.title}
          aria-label={group.title}
          data-feedback-group={group.id}
          tabIndex={-1}
        >
          <h2>
            {group.title} <span>{group.items.length}</span>
          </h2>
          {!group.resource.loading &&
            !group.resource.error &&
            !group.items.length && (
              <p className="pr-muted">No {group.label}.</p>
            )}
          {group.items.map((item) => renderFeedback(item))}
          <PageStatus resource={group.resource} label={group.label} />
        </section>
      ))}
    </PullRequestFiles>
  );
}
