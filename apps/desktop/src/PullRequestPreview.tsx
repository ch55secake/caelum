import {
  createContext,
  useContext,
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
  type RefObject,
} from "react";
import { createPortal } from "react-dom";
import {
  ArrowRight,
  Check,
  CircleDot,
  ExternalLink,
  FileDiff,
  GitBranch,
  GitMerge,
  GitPullRequest,
  GitPullRequestClosed,
  GitPullRequestDraft,
  RefreshCw,
  X,
} from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import {
  checkStatus,
  checkSummary,
  pullRequestState,
  reviewStatus,
  type PullRequest,
} from "./pullRequestState.js";
import { usePullRequestDetails } from "./usePullRequestDetails.js";
import { copyAction, useContextMenu } from "./ContextMenu.js";

type Target = {
  anchor: HTMLButtonElement;
  projectPath: string;
  pr: PullRequest;
};
type Preview = Target & { scope: string; expanded: boolean };
const PreviewContext = createContext<{
  anchor: HTMLButtonElement | null;
  id: string;
  show: (target: Target, immediate?: boolean) => void;
  open: (target: Target) => void;
  leave: (anchor: HTMLButtonElement) => void;
  release: (anchor: HTMLButtonElement) => void;
  enter: () => void;
} | null>(null);
export const usePullRequestPreview = () => useContext(PreviewContext)!;

export function PullRequestPreviewProvider({
  children,
  scope,
}: {
  children: ReactNode;
  scope: string;
}) {
  const id = useId();
  const [preview, setPreview] = useState<Preview | null>(null);
  const current = useRef<Preview | null>(null);
  const pending = useRef<Target | null>(null);
  const openTimer = useRef<number | undefined>(undefined);
  const closeTimer = useRef<number | undefined>(undefined);
  const restoring = useRef(false);
  const popup = useRef<HTMLDivElement>(null);
  const latestScope = useRef(scope);
  latestScope.current = scope;
  const clearTimers = () => {
    window.clearTimeout(openTimer.current);
    window.clearTimeout(closeTimer.current);
    pending.current = null;
  };
  const dismiss = (restore = false) => {
    clearTimers();
    const target = current.current;
    current.current = null;
    setPreview(null);
    if (restore && target?.anchor.isConnected) {
      restoring.current = true;
      target.anchor.focus({ preventScroll: true });
      restoring.current = false;
    }
  };
  const show = (target: Target, immediate = false) => {
    if (restoring.current || current.current?.expanded) return;
    clearTimers();
    if (current.current?.anchor === target.anchor) return;
    current.current = null;
    setPreview(null);
    pending.current = target;
    const open = () => {
      pending.current = null;
      if (
        !target.anchor.isConnected ||
        latestScope.current !== scope ||
        document.querySelector("dialog[open]")
      )
        return;
      const next = { ...target, scope, expanded: false };
      current.current = next;
      setPreview(next);
    };
    if (immediate) open();
    else openTimer.current = window.setTimeout(open, 250);
  };
  const hold = () => {
    window.clearTimeout(closeTimer.current);
  };
  const leave = (anchor: HTMLButtonElement) => {
    if (pending.current?.anchor === anchor) {
      window.clearTimeout(openTimer.current);
      pending.current = null;
    }
    if (current.current?.anchor !== anchor || current.current.expanded) return;
    hold();
    closeTimer.current = window.setTimeout(() => {
      if (
        popup.current?.contains(document.activeElement) ||
        (anchor === document.activeElement && anchor.matches(":focus-visible"))
      )
        return;
      dismiss();
    }, 180);
  };
  useLayoutEffect(() => {
    dismiss();
    return clearTimers;
  }, [scope]);
  useEffect(() => {
    if (!preview || preview.expanded) return;
    const outside = (event: Event) => {
      const node = event.target as Node;
      if (!preview.anchor.contains(node) && !popup.current?.contains(node))
        dismiss();
    };
    const scroll = (event: Event) => {
      if (
        event.target === document ||
        (event.target instanceof Element &&
          event.target.contains(preview.anchor))
      )
        dismiss();
    };
    const keyboard = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || document.querySelector('[role="menu"]'))
        return;
      event.preventDefault();
      event.stopPropagation();
      dismiss(Boolean(popup.current?.contains(document.activeElement)));
    };
    const blur = () => dismiss();
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    document.addEventListener("scroll", scroll, true);
    document.addEventListener("keydown", keyboard, true);
    window.addEventListener("blur", blur);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      document.removeEventListener("scroll", scroll, true);
      document.removeEventListener("keydown", keyboard, true);
      window.removeEventListener("blur", blur);
    };
  }, [preview]);
  const visible = preview?.scope === scope ? preview : null;
  return (
    <PreviewContext.Provider
      value={{
        anchor: visible?.anchor ?? null,
        id,
        show,
        open: (target) => {
          if (
            !target.anchor.isConnected ||
            document.querySelector("dialog[open]")
          )
            return;
          clearTimers();
          const next = { ...target, scope, expanded: true };
          current.current = next;
          setPreview(next);
        },
        leave,
        release: (anchor) => {
          if (
            current.current?.anchor === anchor ||
            pending.current?.anchor === anchor
          )
            dismiss();
        },
        enter: () =>
          popup.current?.querySelector<HTMLButtonElement>("button")?.focus(),
      }}
    >
      {children}
      {visible &&
        (visible.expanded ? (
          <PullRequestDialog
            key={visible.pr.url}
            id={id}
            target={visible}
            close={() => dismiss(true)}
          />
        ) : (
          createPortal(
            <PreviewCard
              key={`${visible.projectPath}\0${visible.pr.url}`}
              id={id}
              target={visible}
              popup={popup}
              hold={hold}
              leave={() => leave(visible.anchor)}
              exit={(backward) => {
                if (backward) return dismiss(true);
                const controls = Array.from(
                  document.querySelectorAll<HTMLElement>(
                    "button, a[href], input, select, textarea, [tabindex]",
                  ),
                ).filter(
                  (element) =>
                    element.tabIndex >= 0 &&
                    !element.matches(":disabled") &&
                    element.getClientRects().length > 0 &&
                    !popup.current?.contains(element),
                );
                const next = controls[controls.indexOf(visible.anchor) + 1];
                dismiss();
                next?.focus({ preventScroll: true });
              }}
              expand={() => {
                clearTimers();
                const next = { ...visible, expanded: true };
                current.current = next;
                setPreview(next);
              }}
            />,
            document.body,
          )
        ))}
    </PreviewContext.Provider>
  );
}

function PreviewCard({
  id,
  target,
  popup,
  hold,
  leave,
  exit,
  expand,
}: {
  id: string;
  target: Target;
  popup: RefObject<HTMLDivElement | null>;
  hold: () => void;
  leave: () => void;
  exit: (backward: boolean) => void;
  expand: () => void;
}) {
  useLayoutEffect(() => {
    const element = popup.current!;
    const place = () => {
      const anchor = target.anchor.getBoundingClientRect();
      const bounds = element.getBoundingClientRect();
      const right = anchor.right + 8;
      element.style.left = `${Math.max(12, Math.min(right + bounds.width <= window.innerWidth - 12 ? right : anchor.left - bounds.width - 8, window.innerWidth - bounds.width - 12))}px`;
      element.style.top = `${Math.max(12, Math.min(anchor.top - 12, window.innerHeight - bounds.height - 12))}px`;
    };
    place();
    const observer = new ResizeObserver(place);
    observer.observe(element);
    window.addEventListener("resize", place);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", place);
    };
  }, [target.anchor, popup]);
  return (
    <div
      ref={popup}
      id={id}
      role="dialog"
      aria-label="Pull request preview"
      className="pr-preview"
      onPointerEnter={hold}
      onPointerLeave={leave}
      onFocus={hold}
      onKeyDown={(event) => {
        if (event.key !== "Tab") return;
        const controls = Array.from(
          event.currentTarget.querySelectorAll<HTMLElement>(
            "button:not(:disabled), a[href]",
          ),
        );
        const at = controls.indexOf(document.activeElement as HTMLElement);
        if (at < 0) return;
        event.preventDefault();
        const next = controls[at + (event.shiftKey ? -1 : 1)];
        if (next) next.focus({ preventScroll: true });
        else exit(event.shiftKey);
      }}
    >
      <PullRequestContent target={target} expand={expand} />
    </div>
  );
}

function PullRequestDialog({
  id,
  target,
  close,
}: {
  id: string;
  target: Target;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const dismiss = () => {
    dialog.current?.close();
    close();
  };
  useEffect(() => {
    const element = dialog.current!;
    element.showModal();
    return () => element.close();
  }, []);
  return createPortal(
    <dialog
      id={id}
      ref={dialog}
      className="pr-dialog"
      aria-label={`Pull request #${target.pr.number}`}
      onCancel={(event) => {
        event.preventDefault();
        dismiss();
      }}
    >
      <div className="pr-dialog-heading">
        <span>Pull request</span>
        <button
          type="button"
          className="icon-button"
          aria-label="Close pull request"
          onClick={dismiss}
        >
          <X size={16} />
        </button>
      </div>
      <PullRequestContent target={target} expanded />
    </dialog>,
    document.body,
  );
}

function PullRequestContent({
  target,
  expanded = false,
  expand,
}: {
  target: Target;
  expanded?: boolean;
  expand?: () => void;
}) {
  const details = usePullRequestDetails(target.projectPath, target.pr.url);
  const primaryAction = useRef<HTMLButtonElement>(null);
  const [linkError, setLinkError] = useState("");
  const menu = useContextMenu();
  const pr = details.value ?? target.pr;
  const state = pullRequestState(pr);
  const Icon =
    state === "Merged"
      ? GitMerge
      : state === "Closed"
        ? GitPullRequestClosed
        : state === "Draft"
          ? GitPullRequestDraft
          : GitPullRequest;
  const checks = checkSummary(details.value?.statusCheckRollup ?? null);
  const description = details.value?.body
    .replace(/<!--[\s\S]*?-->/g, "")
    .trim();
  const open = (url: string) => {
    setLinkError("");
    void openUrl(url).catch(() =>
      setLinkError(
        "Could not open the link. Copy its address from the context menu.",
      ),
    );
  };
  return (
    <div
      className="pr-content"
      onContextMenu={(event) =>
        menu(event, [
          copyAction("Copy pull request URL", pr.url),
          copyAction("Copy pull request title", pr.title),
        ])
      }
    >
      <div className="pr-meta">
        <span className="pr-state" data-state={state.toLowerCase()}>
          <Icon size={13} />
          {state}
        </span>
        <span>#{pr.number}</span>
        {details.value?.author && (
          <span className="pr-author">by {details.value.author.login}</span>
        )}
      </div>
      <h2>{pr.title}</h2>
      {details.value ? (
        <>
          <div className="pr-branches">
            <GitBranch size={12} />
            <code title={details.value.headRefName}>
              {details.value.headRefName}
            </code>
            <ArrowRight size={12} />
            <code title={details.value.baseRefName}>
              {details.value.baseRefName}
            </code>
          </div>
          <div
            className={`pr-description markdown${expanded ? "" : " pr-excerpt"}`}
          >
            {description ? (
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
                    expanded && href && /^https?:\/\//i.test(href) ? (
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
                {expanded ? description : description.slice(0, 1200)}
              </Markdown>
            ) : (
              <p className="pr-muted">No description provided.</p>
            )}
          </div>
          <div
            className="pr-changes"
            aria-label={`${details.value.additions} lines added, ${details.value.deletions} lines deleted, ${details.value.changedFiles} files changed`}
          >
            <span className="lines-added">
              +{details.value.additions.toLocaleString()}
            </span>
            <span className="lines-deleted">
              −{details.value.deletions.toLocaleString()}
            </span>
            <span>
              <FileDiff size={13} />
              {details.value.changedFiles.toLocaleString()}{" "}
              {details.value.changedFiles === 1 ? "file" : "files"} changed
            </span>
          </div>
          <div className="pr-status" data-tone={checks.tone}>
            <CircleDot size={13} />
            <span>{checks.text}</span>
          </div>
          <div
            className="pr-status"
            data-tone={
              details.value.reviewDecision === "APPROVED"
                ? "success"
                : details.value.reviewDecision === "CHANGES_REQUESTED"
                  ? "failure"
                  : "neutral"
            }
          >
            <Check size={13} />
            <span>{reviewStatus(details.value.reviewDecision)}</span>
          </div>
          {expanded && Boolean(details.value.statusCheckRollup?.length) && (
            <section className="pr-checks" aria-label="Pull request checks">
              <h3>Checks</h3>
              {details.value.statusCheckRollup!.map((check, index) => {
                const result = checkStatus(check);
                return (
                  <div className="pr-check" key={index}>
                    <span>{result.name}</span>
                    <span data-tone={result.tone}>{result.text}</span>
                  </div>
                );
              })}
            </section>
          )}
        </>
      ) : (
        details.loading && (
          <p className="pr-muted" role="status">
            Loading pull request details…
          </p>
        )
      )}
      {details.error && (
        <div className="pr-error" role="alert">
          <p>
            {details.value
              ? "Could not refresh this pull request. Showing the last loaded details."
              : "Could not load pull request details."}
          </p>
          <p>{details.error}</p>
          <button
            type="button"
            onClick={() => {
              primaryAction.current?.focus({ preventScroll: true });
              details.refresh();
            }}
            disabled={details.loading}
          >
            Retry
          </button>
        </div>
      )}
      {linkError && (
        <p className="pr-error" role="alert">
          {linkError}
        </p>
      )}
      <footer className="pr-actions">
        {expanded ? (
          <button
            ref={primaryAction}
            type="button"
            disabled={details.loading}
            onClick={details.refresh}
          >
            <RefreshCw
              size={13}
              className={details.loading ? "spinning" : undefined}
            />
            Refresh
          </button>
        ) : (
          <button ref={primaryAction} type="button" onClick={expand}>
            View pull request
            <ArrowRight size={13} />
          </button>
        )}
        <a
          href={pr.url}
          onClick={(event) => {
            event.preventDefault();
            open(pr.url);
          }}
        >
          GitHub
          <ExternalLink size={12} />
        </a>
      </footer>
    </div>
  );
}
