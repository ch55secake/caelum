import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { LoaderCircle, MessageCircleQuestion, X } from "lucide-react";
import { CopyButton, MessageContent } from "./MessageContent.js";
import "./sideQuestion.css";

export function SideQuestionDialog({
  project,
  sessionId,
  initial,
  available,
  close,
}: {
  project: string;
  sessionId: string;
  initial: string;
  available: boolean;
  close: () => void;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const mounted = useRef(false);
  const started = useRef(false);
  const pending = useRef(false);
  const [question, setQuestion] = useState(initial);
  const [answer, setAnswer] = useState<{
    question: string;
    text: string;
  } | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  async function ask(text = question) {
    if (pending.current || !available || !text.trim()) return;
    pending.current = true;
    setLoading(true);
    setError(null);
    setAnswer(null);
    try {
      const reply = await invoke<string>("opencode_generate", {
        projectPath: project,
        sessionId,
        prompt: text,
      });
      if (mounted.current) setAnswer({ question: text, text: reply });
    } catch (reason) {
      if (mounted.current) setError(String(reason));
    } finally {
      pending.current = false;
      if (mounted.current) {
        setLoading(false);
        input.current?.focus({ preventScroll: true });
      }
    }
  }
  useEffect(() => {
    mounted.current = true;
    const element = dialog.current!;
    const previous = document.activeElement;
    element.showModal();
    input.current?.focus({ preventScroll: true });
    // A slash argument asks once, including through StrictMode effect replay.
    if (!started.current && initial.trim()) {
      started.current = true;
      void ask(initial);
    }
    return () => {
      mounted.current = false;
      element.close();
      if (previous instanceof HTMLElement && previous.isConnected)
        previous.focus({ preventScroll: true });
    };
  }, []);
  return (
    <dialog
      ref={dialog}
      className="settings-dialog side-question-dialog"
      aria-labelledby="side-question-title"
      onCancel={(event) => {
        event.preventDefault();
        close();
      }}
    >
      <header className="settings-heading usage-heading">
        <span className="usage-icon">
          <MessageCircleQuestion size={20} />
        </span>
        <div>
          <h1 id="side-question-title">Side question</h1>
          <p>Temporary OpenCode answer from this session’s context.</p>
        </div>
        <button
          className="icon-button"
          aria-label="Dismiss side question"
          onClick={close}
        >
          <X size={16} />
        </button>
      </header>
      <div className="side-question-content">
        <p className="usage-caption">
          Each question uses the main session’s context. Questions and answers
          stay out of its history and are cleared when dismissed.
        </p>
        <form
          onSubmit={(event) => {
            event.preventDefault();
            void ask();
          }}
        >
          <label htmlFor="side-question-input">Question</label>
          <textarea
            ref={input}
            id="side-question-input"
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
            <span>Enter Ask · Shift+Enter Newline</span>
            <button
              type="submit"
              disabled={loading || !available || !question.trim()}
            >
              {loading
                ? "Answering…"
                : error
                  ? "Retry side question"
                  : "Ask side question"}
            </button>
          </div>
        </form>
        {!available && (
          <p className="inline-error" role="status">
            Reconnect this session to ask another question.
          </p>
        )}
        {loading && (
          <p className="side-question-loading" role="status">
            <LoaderCircle className="spinning" size={15} /> Answering
            independently of the main turn…
          </p>
        )}
        {error && (
          <p className="usage-error" role="alert">
            Couldn’t answer: {error}
          </p>
        )}
        {answer && (
          <section className="side-question-answer" aria-label="Side answer">
            <header>
              <h2>Answer</h2>
              <CopyButton text={answer.text} label="Copy side answer" />
            </header>
            <p className="side-question-prompt">{answer.question}</p>
            {answer.text ? (
              <MessageContent text={answer.text} />
            ) : (
              <p>OpenCode returned an empty answer.</p>
            )}
          </section>
        )}
      </div>
      <footer className="settings-footer">
        <span>Temporary · OpenCode</span>
        <button onClick={close}>Done</button>
      </footer>
    </dialog>
  );
}
