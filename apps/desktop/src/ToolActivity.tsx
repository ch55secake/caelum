import { Check, ChevronRight, CircleAlert, Wrench } from "lucide-react";
import { displayValue } from "./conversationState.js";
import type { ConversationEntry } from "./workspaceState.js";
import { CopyButton } from "./MessageContent.js";
import { useEffect, useState } from "react";
import { splitShellMessages } from "./shellMessages.js";

function ShellIdentity({ id }: { id: string }) {
  return (
    <div className="tool-execution-id">
      <span>Shell</span>
      <code>{id}</code>
      <CopyButton text={id} label="Copy shell ID" />
    </div>
  );
}

function ToolValue({
  label,
  value,
  parseShell = true,
}: {
  label: string;
  value: unknown;
  parseShell?: boolean;
}) {
  const [full, setFull] = useState(false);
  const text = displayValue(value);
  if (!text) return null;
  const parts =
    label === "Output" && parseShell ? splitShellMessages(text) : [];
  if (parts.some((part) => part.type === "shell"))
    return (
      <div className="tool-value">
        {parts.map((part, index) =>
          part.type === "text" ? (
            <ToolValue
              key={index}
              label="Output"
              value={part.text}
              parseShell={false}
            />
          ) : (
            <section
              key={index}
              className="shell-output"
              aria-label={`Shell ${part.id}`}
            >
              <div className="shell-output-heading">
                <strong>Run</strong>
                <span>{part.state}</span>
              </div>
              <ShellIdentity id={part.id} />
              <ToolValue
                label="Input"
                value={part.command}
                parseShell={false}
              />
              <ToolValue
                label="Output"
                value={part.output}
                parseShell={false}
              />
            </section>
          ),
        )}
      </div>
    );
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
  const running = entry.status === "running";
  const [observedStart] = useState(Date.now);
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    if (!running) return;
    setNow(Date.now());
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [running]);
  const seconds = Math.max(
    0,
    Math.floor((now - (entry.startedAt ?? observedStart)) / 1000),
  );
  const elapsed =
    seconds >= 60
      ? `${Math.floor(seconds / 60)}m ${seconds % 60}s`
      : `${seconds}s`;
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
    : entry.status === "completed"
      ? Check
      : Wrench;
  return (
    <div
      className={`tool-activity${failed ? " failed" : ""}`}
      data-running={running}
    >
      <button
        className="tool-summary"
        onClick={toggle}
        aria-expanded={expanded}
      >
        <ChevronRight size={13} className={expanded ? "expanded" : ""} />
        <Icon size={14} />
        <strong>{entry.name ?? "Tool"}</strong>
        <span className="tool-preview">{summary}</span>
        <span className="tool-status">
          {failed
            ? "Failed"
            : entry.status === "running"
              ? `Running · ${elapsed}`
              : entry.durationMs != null
                ? `${(entry.durationMs / 1000).toFixed(1)}s`
                : entry.status === "interrupted"
                  ? "Interrupted"
                  : ""}
        </span>
      </button>
      {expanded && (
        <div className="tool-details">
          {entry.executionId && <ShellIdentity id={entry.executionId} />}
          <ToolValue label="Input" value={entry.input} />
          <ToolValue label="Output" value={entry.output} />
          <ToolValue label="Error" value={entry.error} />
        </div>
      )}
    </div>
  );
}
