import {
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type ReactNode,
} from "react";
import {
  ChevronDown,
  FileDiff,
  MessageSquare,
  PanelLeft,
  RefreshCw,
} from "lucide-react";
import { DiffSection } from "./DiffSection.js";
import { PageStatus } from "./PullRequestReviewStatus.js";
import { usePullRequestReview } from "./usePullRequestReview.js";
import { diffLines } from "./diffHighlighting.js";
import type {
  PullRequestFile,
  ReviewFeedback,
} from "./pullRequestReviewState.js";
import type { PullRequestDetails } from "./pullRequestState.js";

type RenderFeedback = (item: ReviewFeedback, inline?: boolean) => ReactNode;
const noFeedback: ReviewFeedback[] = [];

const FilePatchContent = memo(function FilePatchContent({
  file,
  feedback,
  renderFeedback,
}: {
  file: PullRequestFile;
  feedback: ReviewFeedback[];
  renderFeedback: RenderFeedback;
}) {
  const { placed, unplaced } = useMemo(() => {
    const oldLines = new Map<number, number>();
    const newLines = new Map<number, number>();
    diffLines(file.patch ?? "").forEach((line, index) => {
      if (line.oldLine !== undefined) oldLines.set(line.oldLine, index);
      if (line.newLine !== undefined) newLines.set(line.newLine, index);
    });
    const placed = new Map<number, ReviewFeedback[]>();
    const unplaced: ReviewFeedback[] = [];
    for (const item of feedback) {
      const thread = item.thread!;
      // Original line numbers belong to earlier revisions, never the current
      // patch. Keep outdated and unavailable context explicit beside the file.
      const index =
        !thread.isOutdated && thread.line != null
          ? (thread.diffSide === "LEFT"
              ? oldLines
              : thread.diffSide === "RIGHT"
                ? newLines
                : undefined
            )?.get(thread.line)
          : undefined;
      if (index === undefined) unplaced.push(item);
      else placed.set(index, [...(placed.get(index) ?? []), item]);
    }
    return { placed, unplaced };
  }, [file.patch, feedback]);
  const annotations = useMemo(
    () =>
      new Map(
        [...placed].map(([index, items]) => [
          index,
          items.map((item) => renderFeedback(item, true)),
        ]),
      ),
    [placed, renderFeedback],
  );
  return (
    <>
      {file.previousPath && (
        <p className="pr-file-rename">
          Renamed from <code>{file.previousPath}</code>
        </p>
      )}
      {file.patch ? (
        <DiffSection
          path={file.path}
          title={file.path}
          content={file.patch}
          numbered
          hideHeading
          annotations={annotations}
        />
      ) : (
        <p className="pr-muted pr-file-unavailable">
          GitHub did not provide a text patch for this file. It may be binary,
          too large, or renamed without changes.
        </p>
      )}
      {unplaced.length > 0 && (
        <section
          className="pr-file-comments"
          aria-label={`Additional comments on ${file.path}`}
        >
          <p className="pr-muted">
            File comments and threads outside the current patch
          </p>
          {unplaced.map((item) => renderFeedback(item))}
        </section>
      )}
    </>
  );
});

const FilePatch = memo(function FilePatch({
  file,
  collapsed,
  toggle,
  ready,
  feedback,
  renderFeedback,
}: {
  file: PullRequestFile;
  collapsed: boolean;
  toggle: (path: string) => void;
  ready: boolean;
  feedback: ReviewFeedback[];
  renderFeedback: RenderFeedback;
}) {
  const id = useId();
  const height = useMemo(
    () =>
      Math.max(64, (file.patch?.split("\n").length ?? 0) * 20.4) +
      (file.previousPath ? 40 : 0) +
      feedback.reduce(
        (height, item) => height + (item.thread?.isResolved ? 60 : 220),
        0,
      ),
    [file.patch, file.previousPath, feedback],
  );
  return (
    <article
      className="pr-file-diff"
      data-diff-path={file.path}
      aria-label={`Diff for ${file.path}`}
    >
      <header className="pr-file-heading" data-collapsed={collapsed}>
        <h2>
          <button
            type="button"
            onClick={() => toggle(file.path)}
            aria-expanded={!collapsed}
            aria-controls={id}
            aria-label={`${collapsed ? "Expand" : "Collapse"} diff for ${file.path}`}
          >
            <ChevronDown size={14} className="pr-file-chevron" />
            <FileDiff size={14} />
            <span>{file.path}</span>
          </button>
        </h2>
        <span className="pr-file-status">{file.status}</span>
        {feedback.length > 0 && (
          <span
            className="pr-file-comment-count"
            title={`${feedback.length} review threads`}
          >
            <MessageSquare size={12} />
            {feedback.length}
          </span>
        )}
        <span className="lines-added">+{file.additions}</span>
        <span className="lines-deleted">−{file.deletions}</span>
      </header>
      <div
        id={id}
        className="pr-file-body"
        hidden={collapsed}
        style={{ "--pr-file-height": `${height}px` } as CSSProperties}
      >
        {ready ? (
          <FilePatchContent
            file={file}
            feedback={feedback}
            renderFeedback={renderFeedback}
          />
        ) : (
          <div
            className="pr-file-placeholder"
            aria-label={`Diff for ${file.path} renders when in view`}
          />
        )}
      </div>
    </article>
  );
});

type Jump = { path?: string; threadId?: string; section?: string };

function ThreadLinks({
  items,
  path,
  jump,
}: {
  items: ReviewFeedback[];
  path?: string;
  jump: (target: Jump) => void;
}) {
  if (!items.length) return null;
  return (
    <div
      className="pr-file-thread-list"
      role="group"
      aria-label={`Comments on ${path ?? items[0].thread!.path}`}
    >
      {items.map((item) => {
        const thread = item.thread!;
        const line = thread.line ?? thread.originalLine;
        const location = line
          ? `${thread.diffSide === "LEFT" ? "Old" : "New"} ${thread.startLine ? `lines ${thread.startLine}–${line}` : `line ${line}`}`
          : "File comment";
        const state = thread.isResolved
          ? "Resolved"
          : thread.isOutdated
            ? "Outdated"
            : "Unresolved";
        const preview = thread.isResolved
          ? ""
          : `${item.comments[0]?.author?.login ?? "Deleted user"}: ${item.comments[0]?.body.trim().split("\n", 1)[0].slice(0, 100) || "Comment"}`;
        const status = `${state}${thread.isResolved && thread.isOutdated ? " · Outdated" : ""}`;
        return (
          <button
            type="button"
            className="pr-thread-link"
            key={item.id}
            data-state={state.toLowerCase()}
            onClick={() => jump({ path, threadId: item.id })}
            aria-label={`Go to comment: ${location}, ${status.toLowerCase()}${preview ? `. ${preview}` : ""}`}
            title={preview || `${location} · ${status}`}
          >
            <span className="pr-thread-location">
              <MessageSquare size={12} />
              {location}
              <span>{status}</span>
            </span>
            {preview && <span className="pr-thread-preview">{preview}</span>}
          </button>
        );
      })}
    </div>
  );
}

function ThreadCount({ items }: { items: ReviewFeedback[] }) {
  if (!items.length) return null;
  const unresolved = items.filter(
    (item) => !item.thread!.isResolved && !item.thread!.isOutdated,
  ).length;
  const label = `${unresolved} unresolved of ${items.length} review threads`;
  return (
    <span
      className="pr-file-thread-count"
      data-unresolved={unresolved > 0}
      aria-label={label}
      title={label}
    >
      <MessageSquare size={12} />
      {items.length}
    </span>
  );
}

export function PullRequestFiles({
  projectPath,
  pr,
  active,
  feedback,
  renderFeedback,
  reviewActions,
  threadStatus,
  reviewLinks,
  children,
}: {
  projectPath: string;
  pr: PullRequestDetails;
  active: boolean;
  feedback: ReviewFeedback[];
  renderFeedback: RenderFeedback;
  reviewActions: ReactNode;
  threadStatus: ReactNode;
  reviewLinks: { id: string; title: string; count: number }[];
  children: ReactNode;
}) {
  const resource = usePullRequestReview({
    projectPath,
    url: pr.url,
    headOid: pr.headRefOid,
    kind: "files",
  });
  const files = useMemo(
    () => [
      ...new Map(
        resource.pages
          .flatMap((page) => page.files ?? [])
          .map((file) => [file.path, file]),
      ).values(),
    ],
    [resource.pages],
  );
  const [selected, setSelected] = useState<string>();
  const { byFile, otherThreads, otherFiles } = useMemo(() => {
    const paths = new Set(files.map((file) => file.path));
    const renamed = new Map(
      files
        .filter((file) => file.previousPath)
        .map((file) => [file.previousPath!, file.path]),
    );
    const byFile = new Map<string, ReviewFeedback[]>();
    const otherThreads: ReviewFeedback[] = [];
    const otherFiles = new Map<string, ReviewFeedback[]>();
    for (const item of feedback) {
      const path = item.thread!.path;
      const current = paths.has(path) ? path : renamed.get(path);
      if (current) byFile.set(current, [...(byFile.get(current) ?? []), item]);
      else {
        otherThreads.push(item);
        otherFiles.set(path, [...(otherFiles.get(path) ?? []), item]);
      }
    }
    return { byFile, otherThreads, otherFiles };
  }, [files, feedback]);
  const [collapsed, setCollapsed] = useState(() => new Map<string, boolean>());
  const [collapseAll, setCollapseAll] = useState(false);
  const [showFiles, setShowFiles] = useState(true);
  const [rendered, setRendered] = useState(() => new Set<string>());
  const root = useRef<HTMLElement>(null);
  const toolbar = useRef<HTMLDivElement>(null);
  const fileList = useRef<HTMLElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const fileListId = useId();
  const retryMore = useRef(false);
  const pendingJump = useRef<Jump | null>(null);
  const isCollapsed = (path: string) => collapsed.get(path) ?? collapseAll;
  const toggle = useCallback(
    (path: string) =>
      setCollapsed((current) =>
        new Map(current).set(path, !(current.get(path) ?? collapseAll)),
      ),
    [collapseAll],
  );
  const more = () => {
    retryMore.current = true;
    resource.more();
  };
  const refresh = () => {
    retryMore.current = false;
    resource.refresh();
  };
  const loadMore = useRef(more);
  loadMore.current = more;

  useLayoutEffect(() => {
    const element = root.current;
    const bar = toolbar.current;
    const screen = element?.closest<HTMLElement>(".pr-screen");
    if (!active || !element || !bar || !screen) return;
    const measure = () => {
      element.style.setProperty(
        "--pr-files-toolbar-height",
        `${bar.offsetHeight}px`,
      );
      element.style.setProperty(
        "--pr-file-list-top",
        `${Math.min(bar.offsetHeight + 12, screen.clientHeight - (fileList.current?.offsetHeight ?? 0) - 12)}px`,
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(bar);
    observer.observe(screen);
    if (fileList.current) observer.observe(fileList.current);
    return () => observer.disconnect();
  }, [active]);

  useEffect(() => {
    const element = root.current;
    const screen = element?.closest<HTMLElement>(".pr-screen");
    if (!active || !element || !screen || !files.length) return;
    const cards = [
      ...element.querySelectorAll<HTMLElement>("[data-diff-path]"),
    ];
    let frame = 0;
    const update = () => {
      frame = 0;
      const top =
        screen.getBoundingClientRect().top +
        (toolbar.current?.offsetHeight ?? 0) +
        16;
      // Card positions are ordered, so long PRs need only a few layout reads
      // per frame to keep the navigator in sync.
      let first = 0;
      let last = cards.length - 1;
      while (first < last) {
        const middle = Math.ceil((first + last) / 2);
        if (cards[middle].getBoundingClientRect().top <= top) first = middle;
        else last = middle - 1;
      }
      let current = cards[first];
      if (
        screen.scrollTop > 0 &&
        screen.scrollTop + screen.clientHeight >= screen.scrollHeight - 4
      )
        current = cards.at(-1)!;
      setSelected(current?.dataset.diffPath);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(update);
    };
    update();
    screen.addEventListener("scroll", schedule, { passive: true });
    const observer = new ResizeObserver(schedule);
    observer.observe(element);
    return () => {
      screen.removeEventListener("scroll", schedule);
      observer.disconnect();
      cancelAnimationFrame(frame);
    };
  }, [active, files]);

  // Keep file headers and estimated space in the document, but only create and
  // highlight patches near the viewport. Once visited, their DOM stays mounted.
  useEffect(() => {
    const element = root.current;
    const screen = element?.closest(".pr-screen");
    if (!active || !element || !screen) return;
    const observer = new IntersectionObserver(
      (entries) => {
        const paths = entries
          .filter((entry) => entry.isIntersecting)
          .map((entry) => (entry.target as HTMLElement).dataset.diffPath!);
        if (!paths.length) return;
        setRendered((current) => {
          const added = paths.filter((path) => !current.has(path));
          return added.length ? new Set([...current, ...added]) : current;
        });
      },
      { root: screen, rootMargin: "800px 0px" },
    );
    for (const card of element.querySelectorAll<HTMLElement>(
      "[data-diff-path]",
    )) {
      if (!(collapsed.get(card.dataset.diffPath!) ?? collapseAll))
        observer.observe(card);
    }
    return () => observer.disconnect();
  }, [active, files, collapsed, collapseAll]);

  useEffect(() => {
    const sentinel = end.current;
    const screen = root.current?.closest(".pr-screen");
    if (
      !active ||
      !sentinel ||
      !screen ||
      !resource.hasMore ||
      resource.loading ||
      resource.error
    )
      return;
    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((entry) => entry.isIntersecting)) loadMore.current();
      },
      { root: screen, rootMargin: "0px 0px 600px 0px" },
    );
    observer.observe(sentinel);
    return () => observer.disconnect();
  }, [
    active,
    resource.hasMore,
    resource.loading,
    resource.error,
    resource.pages,
  ]);

  useLayoutEffect(() => {
    const target = pendingJump.current;
    if (!target) return;
    pendingJump.current = null;
    const screen = root.current?.closest<HTMLElement>(".pr-screen");
    const attribute = target.threadId
      ? "threadId"
      : target.section
        ? "feedbackGroup"
        : "diffPath";
    const selector = target.threadId
      ? "[data-thread-id]"
      : target.section
        ? "[data-feedback-group]"
        : "[data-diff-path]";
    const card = [
      ...(root.current?.querySelectorAll<HTMLElement>(selector) ?? []),
    ].find(
      (element) =>
        element.dataset[attribute] ===
        (target.threadId ?? target.section ?? target.path),
    );
    if (screen && card) {
      // Reveal skipped layout before measuring a comment deep inside a patch.
      const body = card.closest<HTMLElement>(".pr-file-body");
      if (body) body.style.contentVisibility = "visible";
      const heading = target.threadId
        ? card
            .closest(".pr-file-diff")
            ?.querySelector<HTMLElement>(".pr-file-heading")
        : null;
      screen.scrollTo({
        top:
          screen.scrollTop +
          card.getBoundingClientRect().top -
          screen.getBoundingClientRect().top -
          (toolbar.current?.offsetHeight ?? 0) -
          (heading?.offsetHeight ?? 0) -
          12,
      });
      if (target.threadId || target.section)
        card.focus({ preventScroll: true });
      if (body) body.style.removeProperty("content-visibility");
    }
    if (target.path) setSelected(target.path);
  }, [collapsed, rendered]);

  const jump = (target: Jump) => {
    pendingJump.current = target;
    if (target.path)
      setRendered((current) => new Set(current).add(target.path!));
    setCollapsed((current) =>
      target.path ? new Map(current).set(target.path, false) : new Map(current),
    );
  };

  return (
    <section ref={root} className="pr-files" aria-label="Pull request files">
      <div ref={toolbar} className="pr-files-toolbar">
        <div className="pr-review-toolbar pr-files-controls">
          <div className="pr-files-summary">
            <button
              type="button"
              className="icon-button"
              aria-label={showFiles ? "Hide file list" : "Show file list"}
              aria-expanded={showFiles}
              aria-controls={fileListId}
              onClick={() => setShowFiles((value) => !value)}
            >
              <PanelLeft size={15} />
            </button>
            <strong>
              {files.length === pr.changedFiles
                ? `${files.length} files changed`
                : `${files.length} of ${pr.changedFiles} files`}
            </strong>
            <span className="lines-added">
              +{pr.additions.toLocaleString()}
            </span>
            <span className="lines-deleted">
              −{pr.deletions.toLocaleString()}
            </span>
          </div>
          <div className="pr-files-actions">
            <button
              type="button"
              disabled={!files.length}
              onClick={() => {
                setCollapseAll(!collapseAll);
                setCollapsed(new Map());
              }}
            >
              {collapseAll ? "Expand all" : "Collapse all"}
            </button>
            <button type="button" onClick={refresh} disabled={resource.loading}>
              <RefreshCw size={13} />
              Refresh files
            </button>
          </div>
        </div>
        {reviewActions}
      </div>
      {!resource.loading && !resource.error && !files.length && (
        <p className="pr-muted">No changed files.</p>
      )}
      <div className="pr-thread-status">{threadStatus}</div>
      <div
        className={`pr-file-layout${showFiles ? "" : " pr-file-layout-wide"}`}
      >
        <nav
          ref={fileList}
          id={fileListId}
          className="pr-file-list"
          aria-label="Changed files"
          hidden={!showFiles}
        >
          <div className="pr-file-list-summary">
            <MessageSquare size={13} />
            {feedback.length} loaded review threads
          </div>
          {files.map((file) => (
            <div key={file.path}>
              <button
                className="pr-file-link"
                type="button"
                onClick={() => jump({ path: file.path })}
                aria-current={
                  file.path === (selected ?? files[0]?.path)
                    ? "true"
                    : undefined
                }
                title={file.path}
              >
                <FileDiff size={13} />
                <span>{file.path}</span>
                <ThreadCount items={byFile.get(file.path) ?? noFeedback} />
                <span className="lines-added">+{file.additions}</span>
                <span className="lines-deleted">−{file.deletions}</span>
              </button>
              <ThreadLinks
                items={byFile.get(file.path) ?? noFeedback}
                path={file.path}
                jump={jump}
              />
            </div>
          ))}
          {otherFiles.size > 0 && (
            <div className="pr-file-list-summary">Other commented files</div>
          )}
          {[...otherFiles].map(([path, items]) => (
            <div key={path}>
              <button
                type="button"
                className="pr-file-link"
                onClick={() => jump({ threadId: items[0].id })}
              >
                <FileDiff size={13} />
                <span>{path}</span>
                <ThreadCount items={items} />
              </button>
              <ThreadLinks items={items} jump={jump} />
            </div>
          ))}
          <div className="pr-file-review-links">
            {reviewLinks.map((link) => (
              <button
                type="button"
                key={link.id}
                onClick={() => jump({ section: link.id })}
              >
                <MessageSquare size={13} />
                <span>{link.title}</span>
                <span>{link.count}</span>
              </button>
            ))}
          </div>
        </nav>
        <div className="pr-file-patches">
          {files.map((file, index) => (
            <FilePatch
              key={file.path}
              file={file}
              collapsed={isCollapsed(file.path)}
              toggle={toggle}
              ready={index === 0 || rendered.has(file.path)}
              feedback={byFile.get(file.path) ?? noFeedback}
              renderFeedback={renderFeedback}
            />
          ))}
          <div ref={end} className="pr-files-pagination">
            <PageStatus
              resource={{
                ...resource,
                more,
                refresh: () => (retryMore.current ? more() : refresh()),
              }}
              label="files"
            />
            {!resource.loading &&
              !resource.error &&
              !resource.hasMore &&
              files.length < pr.changedFiles && (
                <p className="pr-muted">GitHub returned a partial file list.</p>
              )}
          </div>
          {otherThreads.length > 0 && (
            <section
              className="pr-feedback-group"
              aria-label="Comments on other files"
            >
              <h2>Comments on other files</h2>
              <p className="pr-muted">
                These files are not in the loaded diff.
              </p>
              {otherThreads.map((item) => renderFeedback(item))}
            </section>
          )}
          {children}
        </div>
      </div>
    </section>
  );
}
