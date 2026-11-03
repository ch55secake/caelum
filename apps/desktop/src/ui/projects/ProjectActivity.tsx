import { ShieldAlert } from "lucide-react";

export function ProjectActivity({
  working,
  unread,
  attention,
  openUnread,
  openAttention,
}: {
  working: number;
  unread: number;
  attention: number;
  openUnread: () => void;
  openAttention: () => void;
}) {
  if (!working && !unread && !attention) return null;
  return (
    <div className="project-activity" aria-label="Project activity">
      {working > 0 && (
        <span
          className="project-activity-badge"
          title={`${working} working ${working === 1 ? "session" : "sessions"}`}
          aria-label={`${working} working ${working === 1 ? "session" : "sessions"}`}
        >
          <span className="running-dot" aria-hidden="true" />
          {working}
          <span>working</span>
        </span>
      )}
      {unread > 0 && (
        <button
          className="project-activity-badge unread"
          aria-label={`${unread} unread ${unread === 1 ? "session" : "sessions"}. Open next unread`}
          title="Open next unread session"
          onClick={openUnread}
        >
          <span className="unread-dot" aria-hidden="true" />
          {unread}
          <span>unread</span>
        </button>
      )}
      {attention > 0 && (
        <button
          className="project-activity-badge attention"
          aria-label={`${attention} ${attention === 1 ? "session needs" : "sessions need"} attention`}
          title="Open a session needing approval or an answer"
          onClick={openAttention}
        >
          <ShieldAlert size={12} aria-hidden="true" />
          {attention}
        </button>
      )}
    </div>
  );
}
