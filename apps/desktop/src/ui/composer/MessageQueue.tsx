import { ListOrdered, X } from "lucide-react";
import type { QueuedMessage } from "../../domain/composer/messageQueueState.js";
import { splitPromptContext } from "../../domain/composer/promptContext.js";
import { visibleMessageText } from "../../domain/conversation/shellMessages.js";
import { FileContextChips } from "../files/FileNavigation.js";
import { PullRequestContextChips } from "../pull-requests/PullRequestContextChips.js";
import { elapsedTime, useClock } from "../../application/useClock.js";

export function MessageQueue({
  messages,
  paused,
  working,
  disabled,
  remove,
  resume,
}: {
  messages: QueuedMessage[];
  paused: boolean;
  working: boolean;
  disabled: boolean;
  remove: (id: string) => void;
  resume: () => void;
}) {
  const now = useClock(messages.some((item) => item.queuedAt != null));
  if (!messages.length) return null;
  return (
    <section
      className="message-queue composer-wrap"
      aria-label="Queued messages"
    >
      <header>
        <ListOrdered size={13} />
        <strong>Queued · {messages.length}</strong>
        <span>{paused ? "Paused" : "Runs after the current turn"}</span>
        {!working && (paused || messages[0].remote) && (
          <button
            type="button"
            className="text-button"
            disabled={disabled}
            onClick={resume}
          >
            Resume queue
          </button>
        )}
      </header>
      <ol>
        {messages.map((item) => {
          const { text, files, pullRequests } = splitPromptContext(item.prompt);
          return (
            <li key={item.id}>
              <div>
                <p title={visibleMessageText(text)}>
                  {visibleMessageText(text)}
                </p>
                <FileContextChips files={files} />
                <PullRequestContextChips pullRequests={pullRequests} />
                {item.pending && <small>Sending…</small>}
                {item.queuedAt != null && (
                  <small className="queue-wait">
                    Waiting {elapsedTime(now - item.queuedAt)}
                  </small>
                )}
                {item.error && (
                  <small className="queue-error" role="alert">
                    {item.error}
                  </small>
                )}
              </div>
              <button
                type="button"
                className="icon-button"
                aria-label={`Remove queued message: ${visibleMessageText(text)}`}
                disabled={disabled || item.pending}
                onClick={() => remove(item.id)}
              >
                <X size={13} />
              </button>
            </li>
          );
        })}
      </ol>
    </section>
  );
}
