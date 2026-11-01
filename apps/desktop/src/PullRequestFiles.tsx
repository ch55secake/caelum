import {
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  ChevronDown,
  ExternalLink,
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

const FilePatch = memo(function FilePatch({
  file,
  collapsed,
  toggle,
  feedback,
  renderFeedback,
}: {
  file: PullRequestFile;
  collapsed: boolean;
  toggle: (path: string) => void;
  feedback: ReviewFeedback[];
  renderFeedback: RenderFeedback;
}) {
  const id = useId();
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
  const annotations = new Map(
    [...placed].map(([index, items]) => [
      index,
      items.map((item) => renderFeedback(item, true)),
    ]),
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
      <div id={id} className="pr-file-body" hidden={collapsed}>
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
      </div>
    </article>
  );
});

export function PullRequestFiles({
  projectPath,
  pr,
  open,
  active,
  feedback,
  renderFeedback,
  reviewActions,
  threadStatus,
  children,
}: {
  projectPath: string;
  pr: PullRequestDetails;
  open: (url: string) => void;
  active: boolean;
  feedback: ReviewFeedback[];
  renderFeedback: RenderFeedback;
  reviewActions: ReactNode;
  threadStatus: ReactNode;
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
  const { byFile, otherThreads } = useMemo(() => {
    const paths = new Set(files.map((file) => file.path));
    const renamed = new Map(
      files
        .filter((file) => file.previousPath)
        .map((file) => [file.previousPath!, file.path]),
    );
    const byFile = new Map<string, ReviewFeedback[]>();
    const otherThreads: ReviewFeedback[] = [];
    for (const item of feedback) {
      const path = item.thread!.path;
      const current = paths.has(path) ? path : renamed.get(path);
      if (current) byFile.set(current, [...(byFile.get(current) ?? []), item]);
      else otherThreads.push(item);
    }
    return { byFile, otherThreads };
  }, [files, feedback]);
  const [collapsed, setCollapsed] = useState(() => new Map<string, boolean>());
  const [collapseAll, setCollapseAll] = useState(false);
  const [showFiles, setShowFiles] = useState(true);
  const root = useRef<HTMLElement>(null);
  const toolbar = useRef<HTMLDivElement>(null);
  const fileList = useRef<HTMLElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const fileListId = useId();
  const retryMore = useRef(false);
  const pendingJump = useRef<string | null>(null);
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
    const path = pendingJump.current;
    if (!path) return;
    pendingJump.current = null;
    const screen = root.current?.closest<HTMLElement>(".pr-screen");
    const card = [
      ...(root.current?.querySelectorAll<HTMLElement>("[data-diff-path]") ??
        []),
    ].find((element) => element.dataset.diffPath === path);
    if (screen && card)
      screen.scrollTo({
        top:
          screen.scrollTop +
          card.getBoundingClientRect().top -
          screen.getBoundingClientRect().top -
          (toolbar.current?.offsetHeight ?? 0) -
          12,
      });
    setSelected(path);
  }, [collapsed]);

  const jump = (path: string) => {
    pendingJump.current = path;
    setCollapsed((current) => new Map(current).set(path, false));
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
          {files.map((file) => (
            <button
              type="button"
              key={file.path}
              onClick={() => jump(file.path)}
              aria-current={
                file.path === (selected ?? files[0]?.path) ? "true" : undefined
              }
              title={file.path}
            >
              <FileDiff size={13} />
              <span>{file.path}</span>
              <span className="lines-added">+{file.additions}</span>
              <span className="lines-deleted">−{file.deletions}</span>
            </button>
          ))}
        </nav>
        <div className="pr-file-patches">
          {files.map((file) => (
            <FilePatch
              key={file.path}
              file={file}
              collapsed={isCollapsed(file.path)}
              toggle={toggle}
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
            <button
              className="pr-external-action"
              type="button"
              onClick={() => open(`${pr.url}/files`)}
            >
              View full diff on GitHub <ExternalLink size={12} />
            </button>
          </div>
          {threadStatus}
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
