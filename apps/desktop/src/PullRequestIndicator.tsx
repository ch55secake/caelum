import { useLayoutEffect, useRef } from "react";
import { GitPullRequest, GitPullRequestDraft } from "lucide-react";
import { usePullRequestPreview } from "./PullRequestPreview.js";
import { useBranchPullRequests } from "./useBranchPullRequests.js";

export function PullRequestIndicator({
  projectPath,
  branch,
}: {
  projectPath: string;
  branch: string;
}) {
  const element = useRef<HTMLSpanElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const preview = usePullRequestPreview();
  const previewRef = useRef(preview);
  previewRef.current = preview;
  const key = `${projectPath}\0${branch}`;
  const requests = useBranchPullRequests(projectPath, [branch], element);
  const pr = requests[branch]?.value;
  useLayoutEffect(() => {
    const anchor = trigger.current;
    return () => {
      if (anchor) previewRef.current.release(anchor);
    };
  }, [key, pr?.url]);
  const label = pr
    ? `${pr.isDraft ? "Draft" : "Open"} pull request #${pr.number}: ${pr.title}`
    : undefined;
  const Icon = pr?.isDraft ? GitPullRequestDraft : GitPullRequest;
  return (
    <span ref={element} className="branch-pr" data-draft={pr?.isDraft}>
      {pr && (
        <button
          ref={trigger}
          type="button"
          className="branch-pr-trigger"
          aria-label={label}
          aria-haspopup="dialog"
          aria-expanded={
            preview.anchor === trigger.current && Boolean(trigger.current)
          }
          aria-controls={
            preview.anchor === trigger.current ? preview.id : undefined
          }
          onPointerEnter={(event) => {
            if (event.pointerType !== "touch")
              preview.show({ anchor: event.currentTarget, projectPath, pr });
          }}
          onPointerLeave={(event) => preview.leave(event.currentTarget)}
          onFocus={(event) =>
            preview.show({ anchor: event.currentTarget, projectPath, pr }, true)
          }
          onBlur={(event) => preview.leave(event.currentTarget)}
          onClick={(event) => {
            event.stopPropagation();
            preview.show(
              { anchor: event.currentTarget, projectPath, pr },
              true,
            );
          }}
          onKeyDown={(event) => {
            if (
              (event.key === "Tab" && !event.shiftKey) ||
              event.key === "ArrowDown"
            ) {
              if (preview.anchor === trigger.current) {
                event.preventDefault();
                preview.enter();
              }
            }
          }}
        >
          <Icon size={12} aria-hidden="true" />
        </button>
      )}
    </span>
  );
}
