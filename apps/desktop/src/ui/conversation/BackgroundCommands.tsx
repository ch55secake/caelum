import { CircleAlert, LoaderCircle } from "lucide-react";
import type { BackgroundShell } from "../../domain/conversation/shellActivity.js";
import { elapsedTime, useClock } from "../../application/useClock.js";

export function BackgroundCommands({
  shells,
  unavailable,
  busy,
}: {
  shells: BackgroundShell[];
  unavailable: boolean;
  busy: boolean;
}) {
  const now = useClock(!unavailable && shells.length > 0);
  if (!unavailable && !shells.length) return null;
  const label = unavailable
    ? "Background command status unavailable"
    : busy
      ? `${shells.length} background command${shells.length === 1 ? "" : "s"} running`
      : shells.length === 1
        ? "Waiting for background command"
        : `Waiting for ${shells.length} background commands`;
  return (
    <section
      className="background-commands composer-wrap"
      aria-label="Background commands"
    >
      <div className="background-command-status" role="status">
        {unavailable ? (
          <CircleAlert size={14} aria-hidden="true" />
        ) : (
          <LoaderCircle size={14} className="spinning" aria-hidden="true" />
        )}
        <span>{label}</span>
      </div>
      {shells.length > 0 && (
        <ul>
          {shells.map((shell) => (
            <li key={shell.id}>
              <code title={shell.command}>{shell.command}</code>
              <span aria-label="Command elapsed time">
                {elapsedTime(now - shell.startedAt)}
              </span>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
