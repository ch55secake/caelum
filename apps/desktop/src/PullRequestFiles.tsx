import {
  memo,
  useCallback,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import {
  ChevronDown,
  ExternalLink,
  FileDiff,
  PanelLeft,
  RefreshCw,
} from "lucide-react";
import { DiffSection } from "./DiffSection.js";
import { PageStatus } from "./PullRequestReview.js";
import { usePullRequestReview } from "./usePullRequestReview.js";
import type { PullRequestFile } from "./pullRequestReviewState.js";
import type { PullRequestDetails } from "./pullRequestState.js";

const FilePatch = memo(function FilePatch({
  file,
  collapsed,
  toggle,
}: {
  file: PullRequestFile;
  collapsed: boolean;
  toggle: (path: string) => void;
}) {
  const id = useId();
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
          />
        ) : (
          <p className="pr-muted pr-file-unavailable">
            GitHub did not provide a text patch for this file. It may be binary,
            too large, or renamed without changes.
          </p>
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
}: {
  projectPath: string;
  pr: PullRequestDetails;
  open: (url: string) => void;
  active: boolean;
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
  const [collapsed, setCollapsed] = useState(() => new Map<string, boolean>());
  const [collapseAll, setCollapseAll] = useState(false);
  const [showFiles, setShowFiles] = useState(true);
  const root = useRef<HTMLElement>(null);
  const toolbar = useRef<HTMLDivElement>(null);
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
        "--pr-files-viewport-height",
        `${screen.clientHeight}px`,
      );
    };
    measure();
    const observer = new ResizeObserver(measure);
    observer.observe(bar);
    observer.observe(screen);
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
    if (!active || !showFiles) return;
    const list = root.current?.querySelector<HTMLElement>(".pr-file-list");
    const current = list?.querySelector<HTMLElement>('[aria-current="true"]');
    if (!list || !current) return;
    const bounds = list.getBoundingClientRect();
    const row = current.getBoundingClientRect();
    if (row.top < bounds.top) list.scrollTop -= bounds.top - row.top;
    else if (row.bottom > bounds.bottom)
      list.scrollTop += row.bottom - bounds.bottom;
  }, [active, selected, showFiles]);

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
      <div ref={toolbar} className="pr-review-toolbar pr-files-toolbar">
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
          <span className="lines-added">+{pr.additions.toLocaleString()}</span>
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
      {!resource.loading && !resource.error && !files.length && (
        <p className="pr-muted">No changed files.</p>
      )}
      <div
        className={`pr-file-layout${showFiles ? "" : " pr-file-layout-wide"}`}
      >
        <nav
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
        </div>
      </div>
    </section>
  );
}
