import { useId, useState } from "react";
import { GitMerge, LoaderCircle } from "lucide-react";
import {
  mergeStatus,
  type MergeMethod,
  type PullRequestDetails,
} from "./pullRequestState.js";

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
  return (
    <section
      className="pr-merge"
      aria-label="Merge pull request"
      data-merged={pr.state === "MERGED"}
    >
      <div className="pr-merge-status" role="status">
        {merging ? (
          <LoaderCircle size={18} className="spinning" aria-hidden="true" />
        ) : (
          <GitMerge size={18} aria-hidden="true" />
        )}
        <span id={id}>{merging ? "Merging pull request…" : status.text}</span>
      </div>
      {!terminal && (
        <div className="pr-merge-actions">
          <label>
            Merge method
            <select
              aria-label="Merge method"
              value={method}
              disabled={merging}
              onChange={(event) => setMethod(event.target.value as MergeMethod)}
            >
              <option value="merge">Merge commit</option>
              <option value="squash">Squash and merge</option>
              <option value="rebase">Rebase and merge</option>
            </select>
          </label>
          <button
            className="primary-button"
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
            {merging ? "Merging…" : "Merge pull request"}
          </button>
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
