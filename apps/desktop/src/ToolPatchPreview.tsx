import { useMemo } from "react";
import { DiffCode } from "./DiffSection.js";
import { toolPatchPreview } from "./toolPatch.js";

export function ToolPatchPreview({
  input,
  expanded,
  toggle,
}: {
  input: unknown;
  expanded: boolean;
  toggle: () => void;
}) {
  const preview = useMemo(() => toolPatchPreview(input), [input]);
  if (!preview.files.length) return null;
  return (
    <div className="tool-patch-preview" role="group" aria-label="Patch preview">
      {preview.files.map((file, index) => {
        const label = file.moveTo ? `${file.path} → ${file.moveTo}` : file.path;
        return (
          <section className="tool-patch-file" aria-label={label} key={index}>
            <div className="tool-patch-heading">
              <span className="tool-patch-path" title={label}>
                {label}
              </span>
              <span className="tool-patch-kind">
                {file.moveTo
                  ? "Rename"
                  : file.kind === "add"
                    ? "Add"
                    : file.kind === "delete"
                      ? "Delete"
                      : "Edit"}
              </span>
              {file.additions > 0 && (
                <span
                  className="tool-patch-added"
                  aria-label={`${file.additions} additions`}
                >
                  +{file.additions}
                </span>
              )}
              {file.deletions > 0 && (
                <span
                  className="tool-patch-removed"
                  aria-label={`${file.deletions} deletions`}
                >
                  −{file.deletions}
                </span>
              )}
            </div>
            {file.lines.length > 0 && (
              <DiffCode path={file.moveTo ?? file.path} lines={file.lines} />
            )}
            {(file.omittedLines > 0 || file.shortened) && (
              <div className="tool-patch-note">
                {file.omittedLines > 0
                  ? `… ${file.omittedLines} more changed lines`
                  : "Long lines shortened"}
              </div>
            )}
          </section>
        );
      })}
      <div className="tool-patch-footer">
        {preview.omittedFiles > 0 && (
          <span>
            {preview.omittedFiles} more{" "}
            {preview.omittedFiles === 1 ? "file" : "files"}
          </span>
        )}
        <button
          className="text-button"
          onClick={toggle}
          aria-expanded={expanded}
        >
          {expanded ? "Hide full patch input" : "Show full patch input"}
        </button>
      </div>
    </div>
  );
}
