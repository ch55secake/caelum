import { ChevronDown, MessageSquarePlus } from "lucide-react";

export function NewSessionButton({
  start,
  setup,
  disabled,
  compact = false,
}: {
  start: () => void;
  setup: () => void;
  disabled: boolean;
  compact?: boolean;
}) {
  return (
    <div className={`new-session-split${compact ? " compact" : ""}`}>
      <button
        className="new-session"
        aria-label="New session"
        title="New session (⌘N)"
        onClick={start}
        disabled={disabled}
      >
        <MessageSquarePlus size={14} />
        {!compact && <span>New session</span>}
      </button>
      <button
        className="new-session-setup"
        aria-label="Session setup"
        title="Choose project, checkout and provider"
        aria-haspopup="dialog"
        disabled={disabled}
        onClick={(event) => {
          event.currentTarget.focus({ preventScroll: true });
          setup();
        }}
      >
        <ChevronDown size={13} />
      </button>
    </div>
  );
}
