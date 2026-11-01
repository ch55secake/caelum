import { useId, useState } from "react";
import { CircleCheck, GitMerge, LoaderCircle, RefreshCw } from "lucide-react";
import { ChoicePicker } from "./ChoicePicker.js";
import {
  mergeStatus,
  canUpdateBranch,
  type MergeMethod,
  type PullRequestDetails,
} from "./pullRequestState.js";

const methods: {
  id: MergeMethod;
  name: string;
  action: string;
  description: string;
}[] = [
  {
    id: "merge",
    name: "Create a merge commit",
    action: "Merge pull request",
    description: "Keep all commits and add a merge commit.",
  },
  {
    id: "squash",
    name: "Squash and merge",
    action: "Squash and merge",
    description: "Combine all commits into one commit.",
  },
  {
    id: "rebase",
    name: "Rebase and merge",
    action: "Rebase and merge",
    description: "Replay each commit onto the base branch.",
  },
];

export function PullRequestMerge({
  pr,
  ready,
  loading,
  merging,
  error,
  lastMethod,
  merge,
  updating,
  updateError,
  updateBranch,
}: {
  pr: PullRequestDetails;
  ready: boolean;
  loading: boolean;
  merging: boolean;
  error: string | null;
  lastMethod: MergeMethod | null;
  merge: (method: MergeMethod) => Promise<void>;
  updating: boolean;
  updateError: string | null;
  updateBranch: () => Promise<void>;
}) {
  const [method, setMethod] = useState<MergeMethod>(lastMethod ?? "merge");
  const id = useId();
  const status = mergeStatus(pr);
  const terminal = pr.state === "MERGED" || pr.state === "CLOSED";
  const checking = loading && !terminal;
  const selected = methods.find((item) => item.id === method)!;
  return (
    <section
      className="pr-merge"
      aria-label="Merge pull request"
      data-merged={pr.state === "MERGED"}
      data-ready={ready}
    >
      <div className="pr-merge-status" role="status">
        {merging || updating || checking ? (
          <LoaderCircle size={16} className="spinning" aria-hidden="true" />
        ) : ready ? (
          <CircleCheck size={22} aria-hidden="true" />
        ) : (
          <GitMerge size={16} aria-hidden="true" />
        )}
        <span id={id}>
          {merging
            ? "Merging pull request…"
            : updating
              ? `Updating branch from ${pr.baseRefName}…`
              : checking
                ? "Checking merge readiness…"
                : status.text}
        </span>
      </div>
      {!terminal && (
        <div className="pr-merge-controls">
          <button
            className="pr-update-branch"
            type="button"
            disabled={!canUpdateBranch(pr) || loading || merging || updating}
            title={`Merge the latest changes from ${pr.baseRefName} into ${pr.headRefName} on GitHub`}
            onClick={() => void updateBranch()}
          >
            <RefreshCw
              size={14}
              className={updating ? "spinning" : undefined}
              aria-hidden="true"
            />
            {updating ? "Updating…" : "Update branch"}
          </button>
          <div
            className="pr-merge-button"
            role="group"
            aria-label="Merge actions"
          >
            <button
              className="pr-merge-action"
              type="button"
              disabled={!ready}
              aria-describedby={id}
              onClick={() => void merge(method)}
            >
              {merging ? (
                <LoaderCircle
                  size={14}
                  className="spinning"
                  aria-hidden="true"
                />
              ) : (
                <GitMerge size={14} aria-hidden="true" />
              )}
              {merging ? "Merging…" : selected.action}
            </button>
            <ChoicePicker
              label="Merge method"
              icon={null}
              triggerName=""
              value={method}
              choices={methods}
              align="end"
              disabled={merging || updating}
              onChange={(value) => setMethod(value as MergeMethod)}
            />
          </div>
        </div>
      )}
      {error && (
        <p className="pr-error" role="alert">
          Could not merge this pull request. {error}
        </p>
      )}
      {updateError && (
        <p className="pr-error" role="alert">
          Could not update this branch. {updateError}
        </p>
      )}
    </section>
  );
}
