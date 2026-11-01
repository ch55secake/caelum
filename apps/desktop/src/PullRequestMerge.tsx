import { useId, useState } from "react";
import { GitMerge, LoaderCircle } from "lucide-react";
import { ChoicePicker } from "./ChoicePicker.js";
import {
  mergeStatus,
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
  loading,
  merging,
  error,
  lastMethod,
  merge,
}: {
  pr: PullRequestDetails;
  loading: boolean;
  merging: boolean;
  error: string | null;
  lastMethod: MergeMethod | null;
  merge: (method: MergeMethod) => Promise<void>;
}) {
  const [method, setMethod] = useState<MergeMethod>(lastMethod ?? "merge");
  const id = useId();
  const status = mergeStatus(pr);
  const terminal = pr.state === "MERGED" || pr.state === "CLOSED";
  const selected = methods.find((item) => item.id === method)!;
  return (
    <section
      className="pr-merge"
      aria-label="Merge pull request"
      data-merged={pr.state === "MERGED"}
      data-ready={status.enabled}
    >
      <div className="pr-merge-status" role="status">
        {merging ? (
          <LoaderCircle size={16} className="spinning" aria-hidden="true" />
        ) : (
          <GitMerge size={16} aria-hidden="true" />
        )}
        <span id={id}>{merging ? "Merging pull request…" : status.text}</span>
      </div>
      {!terminal && (
        <div
          className="pr-merge-button"
          role="group"
          aria-label="Merge actions"
        >
          <button
            className="pr-merge-action"
            type="button"
            disabled={!status.enabled || loading || merging}
            aria-describedby={id}
            onClick={() => void merge(method)}
          >
            {merging ? (
              <LoaderCircle size={14} className="spinning" aria-hidden="true" />
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
            disabled={merging}
            onChange={(value) => setMethod(value as MergeMethod)}
          />
        </div>
      )}
      {error && (
        <p className="pr-error" role="alert">
          Could not merge this pull request. {error}
        </p>
      )}
    </section>
  );
}
