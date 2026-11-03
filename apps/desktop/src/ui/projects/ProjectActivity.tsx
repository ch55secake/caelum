import { ShieldAlert } from "lucide-react";

export function ProjectActivity({
  attention,
  openAttention,
}: {
  attention: number;
  openAttention: () => void;
}) {
  if (!attention) return null;
  return (
    <div className="project-activity" aria-label="Project activity">
      <button
        className="project-activity-badge attention"
        aria-label={`${attention} ${attention === 1 ? "session needs" : "sessions need"} attention`}
        title="Open a session needing approval or an answer"
        onClick={openAttention}
      >
        <ShieldAlert size={12} aria-hidden="true" />
        {attention}
      </button>
    </div>
  );
}
