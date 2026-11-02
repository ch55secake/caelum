const widths = [68, 42, 82, 56, 74, 48, 62, 36, 78, 52, 66, 40];

export function FileTreeSkeleton() {
  return (
    <div className="pr-file-tree-skeleton" aria-hidden="true">
      <span className="pr-file-skeleton-bar pr-file-skeleton-summary" />
      {widths.slice(0, 6).map((width) => (
        <div className="pr-file-skeleton-tree-row" key={width}>
          <span className="pr-file-skeleton-bar pr-file-skeleton-icon" />
          <span
            className="pr-file-skeleton-bar"
            style={{ width: `${width}%` }}
          />
        </div>
      ))}
    </div>
  );
}

export function FileDiffSkeleton() {
  return (
    <div role="status" aria-label="Loading files">
      <span className="sr-only">Loading files…</span>
      {[12, 8].map((lines) => (
        <div className="pr-file-diff-skeleton" aria-hidden="true" key={lines}>
          <div className="pr-file-skeleton-heading">
            <span className="pr-file-skeleton-bar pr-file-skeleton-icon" />
            <span className="pr-file-skeleton-bar" style={{ width: "38%" }} />
          </div>
          <div className="pr-file-skeleton-code">
            {widths.slice(0, lines).map((width) => (
              <div className="pr-file-skeleton-line" key={width}>
                <span className="pr-file-skeleton-gutter">
                  <span className="pr-file-skeleton-bar" />
                </span>
                <span className="pr-file-skeleton-gutter">
                  <span className="pr-file-skeleton-bar" />
                </span>
                <span className="pr-file-skeleton-source">
                  <span
                    className="pr-file-skeleton-bar"
                    style={{ width: `${width}%` }}
                  />
                </span>
              </div>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}
