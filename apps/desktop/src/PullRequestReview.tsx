import { useEffect, useRef, useState } from "react";
import {
  ExternalLink,
  FileDiff,
  MessageSquare,
  RefreshCw,
  Send,
} from "lucide-react";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { DiffSection } from "./DiffSection.js";
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
type Resource = ReturnType<typeof usePullRequestReview>;
const unique = <T extends { id: string }>(items: T[]) => [
  ...new Map(items.map((item) => [item.id, item])).values(),
];

function PageStatus({
  resource,
  label,
}: {
  resource: Resource;
  label: string;
}) {
  return (
    <>
      {resource.loading && (
        <p className="pr-muted" role="status">
          Loading {label}…
        </p>
      )}
      {resource.error && (
        <div className="pr-error" role="alert">
          <p>
            {resource.pages.length
              ? `Could not refresh ${label}. Showing the last loaded results.`
              : `Could not load ${label}.`}
          </p>
          <p>{resource.error}</p>
          <button
            type="button"
            disabled={resource.loading}
            onClick={resource.refresh}
          >
            Retry {label}
          </button>
        </div>
      )}
      {resource.hasMore && (
        <button
          type="button"
          disabled={resource.loading}
          onClick={resource.more}
        >
          Load more {label}
        </button>
      )}
    </>
  );
}

export function PullRequestFiles({ projectPath, pr, open }: Props) {
  const resource = usePullRequestReview({
    projectPath,
    url: pr.url,
    headOid: pr.headRefOid,
    kind: "files",
  });
  const files = [
    ...new Map(
      resource.pages
        .flatMap((page) => page.files ?? [])
        .map((file) => [file.path, file]),
    ).values(),
  ];
  const [selected, setSelected] = useState<string>();
  const file = files.find((file) => file.path === selected) ?? files[0];
  return (
    <section className="pr-files" aria-label="Pull request files">
      <div className="pr-review-toolbar">
        <span>
          {files.length} of {pr.changedFiles} files ·{" "}
          <code>{pr.headRefOid.slice(0, 7)}</code>
        </span>
        <button
          type="button"
          onClick={resource.refresh}
          disabled={resource.loading}
        >
          <RefreshCw size={13} />
          Refresh files
        </button>
      </div>
      <PageStatus resource={resource} label="files" />
      {!resource.loading && !resource.error && !files.length && (
        <p className="pr-muted">No changed files.</p>
      )}
      {files.length > 0 && (
        <div className="pr-file-layout">
          <nav className="pr-file-list" aria-label="Changed files">
            {files.map((entry) => (
              <button
                type="button"
                key={entry.path}
                onClick={() => setSelected(entry.path)}
                aria-current={entry.path === file?.path ? "true" : undefined}
                title={entry.path}
              >
                <FileDiff size={13} />
                <span>{entry.path}</span>
                <span className="lines-added">+{entry.additions}</span>
                <span className="lines-deleted">−{entry.deletions}</span>
              </button>
            ))}
          </nav>
          {file && (
            <div className="pr-file-diff">
              <div className="pr-file-heading">
                <strong>{file.path}</strong>
                <span>{file.status}</span>
                {file.previousPath && (
                  <span>Renamed from {file.previousPath}</span>
                )}
              </div>
              {file.patch ? (
                <DiffSection
                  path={file.path}
                  title="Changes"
                  content={file.patch}
                />
              ) : (
                <p className="pr-muted">
                  GitHub did not provide a text patch for this file. It may be
                  binary, too large, or renamed without changes.
                </p>
              )}
            </div>
          )}
        </div>
      )}
      {!resource.hasMore &&
        files.length > 0 &&
        files.length < pr.changedFiles && (
          <p className="pr-muted">GitHub returned a partial file list.</p>
        )}
      <button
        className="pr-external-action"
        type="button"
        onClick={() => open(`${pr.url}/files`)}
      >
        View full diff on GitHub <ExternalLink size={12} />
      </button>
    </section>
  );
}

function Comment({
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
}

function ThreadReplies({
  thread,
  projectPath,
  pr,
  received,
}: Props & {
  thread: ReviewThread;
  received: (comments: ReviewComment[], nextCursor: string | null) => void;
}) {
  const [enabled, setEnabled] = useState(false);
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
      <button type="button" onClick={() => setEnabled(true)}>
        Load remaining replies
      </button>
    );
  return <PageStatus resource={replies} label="replies" />;
}

export function PullRequestComments({
  projectPath,
  pr,
  open,
  session,
}: Props & { session?: ReviewSession }) {
  const target = { projectPath, url: pr.url, headOid: pr.headRefOid };
  const discussion = usePullRequestReview({ ...target, kind: "discussion" });
  const reviews = usePullRequestReview({ ...target, kind: "reviews" });
  const threads = usePullRequestReview({ ...target, kind: "threads" });
  const reviewThreads = unique(
    threads.pages.flatMap((page) => page.threads ?? []),
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
  const resources = [discussion, reviews, threads];
  const groups = [
    {
      title: "Discussion",
      label: "discussion comments",
      resource: discussion,
      items: unique(
        discussion.pages.flatMap((page) => page.comments ?? []),
      ).map(
        (comment): ReviewFeedback => ({
          id: comment.id,
          kind: "discussion",
          comments: [comment],
        }),
      ),
    },
    {
      title: "Reviews",
      label: "reviews",
      resource: reviews,
      items: unique(reviews.pages.flatMap((page) => page.comments ?? [])).map(
        (comment): ReviewFeedback => ({
          id: comment.id,
          kind: "review",
          comments: [comment],
        }),
      ),
    },
    {
      title: "Inline comments",
      label: "review threads",
      resource: threads,
      items: reviewThreads.map((thread): ReviewFeedback => {
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
    },
  ];
  const selected = groups
    .flatMap((group) => group.items)
    .filter(
      (item) =>
        !item.thread?.nextCursor &&
        item.comments.some((comment) => comment.body.trim()) &&
        (choices[item.id] ?? feedbackSelectedByDefault(item)),
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
  return (
    <section className="pr-comments" aria-label="Pull request comments">
      <div className="pr-review-toolbar">
        <span>
          <MessageSquare size={14} /> Review feedback
        </span>
        <button type="button" onClick={refresh} disabled={loading || sending}>
          <RefreshCw size={13} />
          Refresh comments
        </button>
      </div>
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
          More feedback is available below. The action includes only the
          selected, loaded comments.
        </p>
      )}
      {groups.map((group) => (
        <section
          className="pr-feedback-group"
          key={group.title}
          aria-label={group.title}
        >
          <h2>
            {group.title} <span>{group.items.length}</span>
          </h2>
          {!group.resource.loading &&
            !group.resource.error &&
            !group.items.length && (
              <p className="pr-muted">No {group.label}.</p>
            )}
          {group.items.map((item) => {
            const thread = item.thread;
            const original =
              thread && reviewThreads.find((value) => value.id === thread.id);
            const label = thread
              ? `${thread.path}${thread.line || thread.originalLine ? `:${thread.line ?? thread.originalLine}` : ""}`
              : `comment by ${item.comments[0]?.author?.login ?? "deleted user"}`;
            return (
              <div className="pr-feedback" key={item.id}>
                <div className="pr-feedback-heading">
                  <label>
                    <input
                      type="checkbox"
                      checked={selected.some((value) => value.id === item.id)}
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
                {thread && item.comments[0]?.diffHunk && (
                  <details className="pr-comment-context">
                    <summary>Diff context</summary>
                    <DiffSection
                      title="Review context"
                      path={thread.path}
                      content={item.comments[0].diffHunk}
                    />
                  </details>
                )}
                {item.comments.map((comment) => (
                  <Comment key={comment.id} comment={comment} open={open} />
                ))}
                {original?.nextCursor && (
                  <ThreadReplies
                    key={`${refreshId}:${threads.version}:${original.id}:${original.nextCursor}`}
                    thread={original}
                    projectPath={projectPath}
                    pr={pr}
                    open={open}
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
              </div>
            );
          })}
          <PageStatus resource={group.resource} label={group.label} />
        </section>
      ))}
    </section>
  );
}
