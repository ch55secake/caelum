import {
  Check,
  ChevronRight,
  CircleAlert,
  LoaderCircle,
  Wrench,
} from "lucide-react";
import { displayValue } from "./conversationState.js";
import type { ConversationEntry } from "./workspaceState.js";
import { CopyButton } from "./MessageContent.js";
import { useState } from "react";

function ToolValue({ label, value }: { label: string; value: unknown }) {
  const [full, setFull] = useState(false);
  const text = displayValue(value);
  if (!text) return null;
  return (
    <div className="tool-value">
      <div className="tool-value-heading">
        <span>{label}</span>
        <CopyButton text={text} label={`Copy ${label.toLowerCase()}`} />
      </div>
      <pre>{full ? text : text.slice(0, 12000)}</pre>
      {text.length > 12000 && (
        <button className="text-button" onClick={() => setFull(!full)}>
          {full
            ? "Show less"
            : `Show full output (${text.length.toLocaleString()} characters)`}
        </button>
      )}
    </div>
  );
}
export function ToolActivity({
  entry,
  expanded,
  toggle,
}: {
  entry: ConversationEntry;
  expanded: boolean;
  toggle: () => void;
}) {
  let input = entry.input;
  if (typeof input === "string") {
    try {
      input = JSON.parse(input);
    } catch {
      /* Input can still be streaming. */
    }
  }
  const fields =
    input && typeof input === "object"
      ? (input as Record<string, unknown>)
      : {};
  const summary = displayValue(
    fields.command ??
      fields.filePath ??
      fields.path ??
      fields.pattern ??
      fields.query ??
      input,
  ).split("\n")[0];
  const failed = entry.status === "failed" || Boolean(entry.error);
  const Icon = failed
    ? CircleAlert
    : entry.status === "running"
      ? LoaderCircle
      : entry.status === "completed"
        ? Check
        : Wrench;
  return (
    <div className={`tool-activity${failed ? " failed" : ""}`}>
      <button
        className="tool-summary"
        onClick={toggle}
        aria-expanded={expanded}
      >
        <ChevronRight size={13} className={expanded ? "expanded" : ""} />
        <Icon
          size={14}
          className={entry.status === "running" ? "spinning" : ""}
        />
        <strong>{entry.name ?? "Tool"}</strong>
        <span className="tool-preview">{summary}</span>
        <span className="tool-status">
          {failed
            ? "Failed"
            : entry.status === "running"
              ? "Running"
              : entry.durationMs != null
                ? `${(entry.durationMs / 1000).toFixed(1)}s`
                : entry.status === "interrupted"
                  ? "Interrupted"
                  : ""}
        </span>
      </button>
      {expanded && (
        <div className="tool-details">
          <ToolValue label="Input" value={entry.input} />
          <ToolValue label="Output" value={entry.output} />
          <ToolValue label="Error" value={entry.error} />
        </div>
      )}
    </div>
  );
}
