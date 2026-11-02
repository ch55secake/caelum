import type { usePullRequestReview } from "./usePullRequestReview.js";

export function PageStatus({
  resource,
  label,
}: {
  resource: ReturnType<typeof usePullRequestReview>;
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
