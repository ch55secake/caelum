import {
  useEffect,
  useId,
  useLayoutEffect,
  useRef,
  useState,
  type KeyboardEvent,
  type RefObject,
} from "react";
import type { useSessionPullRequests } from "../../application/github/useSessionPullRequests.js";
import { readPullRequestDetails } from "../../application/github/usePullRequestDetails.js";
import {
  createPullRequestContext,
  type PullRequestContext,
} from "../../domain/pull-requests/pullRequestContext.js";

export type SessionPullRequests = ReturnType<typeof useSessionPullRequests>;

export function usePullRequestMentions({
  source,
  draft,
  setDraft,
  enabled,
  input,
  attach,
}: {
  source: SessionPullRequests;
  draft: string;
  setDraft: (text: string) => void;
  enabled: boolean;
  input: RefObject<HTMLTextAreaElement | null>;
  attach: (pr: PullRequestContext) => boolean;
}) {
  const id = useId();
  const list = useRef<HTMLDivElement>(null);
  const revision = useRef(0);
  const mounted = useRef(false);
  const latest = useRef({ draft, enabled, attach, source });
  latest.current = { draft, enabled, attach, source };
  const [caret, setCaret] = useState<number | null>(null);
  const [focused, setFocused] = useState(false);
  const [dismissed, setDismissed] = useState(false);
  const [active, setActive] = useState(0);
  const [reading, setReading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const pendingCaret = useRef<number | null>(null);
  const query =
    caret === null
      ? null
      : (draft.slice(0, caret).match(/(?:^|\s)#([^\s#"`]*)$/)?.[1] ?? null);
  const open = enabled && focused && !dismissed && query !== null;
  const matches = [...source.entries.values()]
    .filter(({ pr }) =>
      `${pr.number} ${pr.title}`
        .toLocaleLowerCase()
        .includes((query ?? "").toLocaleLowerCase()),
    )
    .sort((a, b) => b.pr.number - a.pr.number);
  const selected = Math.min(active, Math.max(0, matches.length - 1));
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      revision.current++;
    };
  }, []);
  useEffect(() => {
    revision.current++;
    setReading(false);
  }, [enabled]);
  useLayoutEffect(() => {
    if (pendingCaret.current === null) return;
    input.current?.setSelectionRange(
      pendingCaret.current,
      pendingCaret.current,
    );
    pendingCaret.current = null;
  }, [draft, input]);
  useEffect(() => {
    list.current
      ?.querySelector('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" });
  }, [selected, query, source.entries.size]);
  function changed() {
    revision.current++;
    setReading(false);
    setError(null);
    setDismissed(false);
    setActive(0);
    const area = input.current;
    setCaret(
      area && area.selectionStart === area.selectionEnd
        ? area.selectionStart
        : null,
    );
  }
  function dismiss() {
    revision.current++;
    setReading(false);
    setDismissed(true);
  }
  async function choose(index: number) {
    const entry = matches[index];
    if (!open || reading || !entry || caret === null || query === null) return;
    const version = ++revision.current;
    const start = caret - query.length - 1;
    const end =
      caret + (draft.slice(caret).match(/^[^\s#"`]*/)?.[0].length ?? 0);
    const current = () =>
      mounted.current &&
      version === revision.current &&
      latest.current.enabled &&
      latest.current.draft === draft &&
      latest.current.source.entries.has(entry.pr.url);
    setReading(true);
    setError(null);
    try {
      const details = await readPullRequestDetails(
        entry.projectPath,
        entry.pr.url,
      );
      if (!current()) return;
      if (details.url !== entry.pr.url || details.number !== entry.pr.number)
        throw new Error(
          "The pull request changed. Refresh and select it again.",
        );
      if (!latest.current.attach(createPullRequestContext(details)))
        throw new Error(
          "Could not attach this pull request. Attach up to 16 pull requests per message.",
        );
      const suffix = draft.slice(end);
      const replacement = `#${entry.pr.number}${suffix.startsWith(" ") ? "" : " "}`;
      pendingCaret.current =
        start + replacement.length + (suffix.startsWith(" ") ? 1 : 0);
      setCaret(pendingCaret.current);
      setDraft(draft.slice(0, start) + replacement + suffix);
      dismiss();
      input.current?.focus({ preventScroll: true });
    } catch (reason) {
      if (current()) setError(String(reason));
    } finally {
      if (current()) setReading(false);
    }
  }
  return {
    reading,
    controls: open ? id : undefined,
    activeId: open && matches[selected] ? `${id}-${selected}` : undefined,
    changed,
    focus: () => {
      setFocused(true);
      changed();
    },
    blur: () => {
      setFocused(false);
      dismiss();
    },
    select: () => {
      const area = input.current;
      const next =
        area && area.selectionStart === area.selectionEnd
          ? area.selectionStart
          : null;
      if (next !== caret) changed();
    },
    keyDown(event: KeyboardEvent<HTMLTextAreaElement>) {
      if (
        !open ||
        event.nativeEvent.isComposing ||
        event.shiftKey ||
        event.metaKey ||
        event.ctrlKey ||
        event.altKey
      )
        return false;
      if (event.key === "Escape") {
        event.preventDefault();
        event.stopPropagation();
        dismiss();
        return true;
      }
      if (["ArrowUp", "ArrowDown", "Enter", "Tab"].includes(event.key)) {
        event.preventDefault();
        if (event.key === "ArrowUp" || event.key === "ArrowDown")
          setActive(
            Math.max(
              0,
              Math.min(
                matches.length - 1,
                selected + (event.key === "ArrowDown" ? 1 : -1),
              ),
            ),
          );
        else if (matches[selected]) void choose(selected);
        return true;
      }
      return false;
    },
    menu: open && (
      <div className="slash-menu">
        <div className="slash-heading">
          Session pull requests <span>Tab / Enter Attach</span>
        </div>
        {(source.loading || reading) && (
          <div className="slash-note" role="status">
            {reading
              ? "Attaching PR and checks…"
              : "Finding session pull requests…"}
          </div>
        )}
        {(error || source.error) && (
          <div className="slash-note" role="alert">
            {error || source.error}{" "}
            <button
              type="button"
              onMouseDown={(event) => event.preventDefault()}
              onClick={() => {
                if (error) void choose(selected);
                else source.retry();
              }}
            >
              Retry pull requests
            </button>
          </div>
        )}
        <div
          ref={list}
          id={id}
          className="slash-list"
          role="listbox"
          aria-label="Pull request mentions"
          aria-busy={source.loading || reading}
        >
          {matches.map(({ pr }, index) => (
            <button
              type="button"
              role="option"
              key={pr.url}
              id={`${id}-${index}`}
              aria-selected={index === selected}
              disabled={reading}
              onMouseDown={(event) => event.preventDefault()}
              onMouseEnter={() => setActive(index)}
              onClick={() => void choose(index)}
            >
              <strong>#{pr.number}</strong>
              <span>{pr.title}</span>
            </button>
          ))}
        </div>
        {!source.loading && !error && !source.error && !matches.length && (
          <div className="slash-note">
            No matching pull requests for this session.
          </div>
        )}
      </div>
    ),
  };
}
