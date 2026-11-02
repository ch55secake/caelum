import { useEffect, useId, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LoaderCircle, MessageCircleQuestion, Pencil, X } from "lucide-react";
import { CopyButton, MessageContent } from "./MessageContent.js";
import "./sideQuestion.css";

export function SideQuestionPanel({
  project,
  sessionId,
  initial,
  available,
  resumeDraft,
  close,
}: {
  project: string;
  sessionId: string;
  initial: string;
  available: boolean;
  resumeDraft: () => void;
  close: () => void;
}) {
  const id = useId();
  const input = useRef<HTMLTextAreaElement>(null);
  const mounted = useRef(false);
  const started = useRef(false);
  const pending = useRef(false);
  const [question, setQuestion] = useState(initial);
  const [editing, setEditing] = useState(!initial.trim());
  const [asked, setAsked] = useState(initial);
  const [answer, setAnswer] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function ask(text = question) {
    if (pending.current || !available || !text.trim()) return;
    pending.current = true;
    setEditing(false);
    setAsked(text);
    setLoading(true);
    setError(null);
    setAnswer(null);
    resumeDraft();
    try {
      const reply = await invoke<string>("opencode_generate", {
        projectPath: project,
        sessionId,
        prompt: text,
      });
      if (mounted.current) setAnswer(reply);
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    } finally {
      pending.current = false;
      if (mounted.current) {
        setLoading(false);
      }
    }
  }
  useEffect(() => {
    mounted.current = true;
    if (!initial.trim()) input.current?.focus({ preventScroll: true });
    // A slash argument asks once, including through StrictMode effect replay.
    if (!started.current && initial.trim()) {
      started.current = true;
      void ask(initial);
    }
    return () => {
      mounted.current = false;
    };
  }, []);
  return (
    <section
      className="side-question-panel"
      aria-labelledby={`${id}-title`}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
      }}
    >
      <header className="side-question-heading">
        <MessageCircleQuestion size={15} aria-hidden="true" />
        <h2 id={`${id}-title`}>Side question</h2>
        <span title="Uses this session’s context without adding to its history">
          Temporary
        </span>
        {!editing && answer !== null && (
          <CopyButton text={answer} label="Copy side answer" />
        )}
        {!editing && (
          <button
            type="button"
            className="icon-button side-question-edit"
            aria-label="Edit side question"
            title="Edit question"
            disabled={loading}
            onClick={() => {
              setEditing(true);
              requestAnimationFrame(() =>
                input.current?.focus({ preventScroll: true }),
              );
            }}
          >
            <Pencil size={13} />
          </button>
        )}
        <button
          className="icon-button"
          aria-label="Dismiss side question"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="side-question-content">
        {editing ? (
          <form
            onSubmit={(event) => {
              event.preventDefault();
              void ask();
            }}
          >
            <label className="sr-only" htmlFor={`${id}-input`}>
              Question
            </label>
            <textarea
              ref={input}
              id={`${id}-input`}
              value={question}
              readOnly={loading}
              placeholder="Ask a quick question…"
              onChange={(event) => setQuestion(event.target.value)}
              onKeyDown={(event) => {
                if (
                  event.key === "Enter" &&
                  !event.nativeEvent.isComposing &&
                  (!event.shiftKey || event.metaKey || event.ctrlKey)
                ) {
                  event.preventDefault();
                  void ask();
                }
              }}
            />
            <div className="side-question-actions">
              <button
                type="submit"
                disabled={loading || !available || !question.trim()}
              >
                Ask side question
              </button>
            </div>
          </form>
        ) : (
          <p className="side-question-prompt">{asked}</p>
        )}
        {!available && (
          <p className="inline-error" role="status">
            Reconnect this session to ask another question.
          </p>
        )}
        {!editing && loading && (
          <p className="side-question-loading" role="status">
            <LoaderCircle className="spinning" size={15} /> Answering
            independently of the main turn…
          </p>
        )}
        {!editing && error && (
          <p className="usage-error" role="alert">
            Couldn’t answer: {error}
            <button
              type="button"
              onClick={() => void ask(asked)}
              disabled={!available}
            >
              Retry side question
            </button>
          </p>
        )}
        {!editing && answer !== null && (
          <section className="side-question-answer" aria-label="Side answer">
            {answer ? (
              <MessageContent text={answer} />
            ) : (
              <p>OpenCode returned an empty answer.</p>
            )}
          </section>
        )}
      </div>
    </section>
  );
}
