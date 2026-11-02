import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ArrowLeft,
  ArrowUp,
  LoaderCircle,
  MessageCircleQuestion,
  Pencil,
} from "lucide-react";
import { CopyButton, MessageContent } from "./MessageContent.js";
import {
  usePullRequestMentions,
  type SessionPullRequests,
} from "../composer/PullRequestMentionPicker.js";
import { useFileMentions } from "../composer/FileMentionPicker.js";
import { FileContextChips } from "../files/FileNavigation.js";
import { PullRequestContextChips } from "../pull-requests/PullRequestContextChips.js";
import {
  MAX_FILE_CONTEXT,
  type FileContext,
} from "../../domain/files/fileContext.js";
import {
  MAX_PULL_REQUEST_CONTEXT,
  type PullRequestContext,
} from "../../domain/pull-requests/pullRequestContext.js";
import { promptWithContext } from "../../domain/composer/promptContext.js";
import "./sideQuestion.css";

export function SideQuestionPanel({
  project,
  sessionId,
  initial,
  initialFiles,
  initialPullRequests,
  pullRequests,
  available,
  minHeight,
  close,
}: {
  project: string;
  sessionId: string;
  initial: string;
  initialFiles: FileContext[];
  initialPullRequests: PullRequestContext[];
  pullRequests: SessionPullRequests;
  available: boolean;
  minHeight: number;
  close: () => void;
}) {
  const id = useId();
  const input = useRef<HTMLTextAreaElement>(null);
  const measurement = useRef<HTMLTextAreaElement>(null);
  const panel = useRef<HTMLElement>(null);
  const mounted = useRef(false);
  const started = useRef(false);
  const pending = useRef(false);
  const [question, setQuestion] = useState(initial);
  const [editing, setEditing] = useState(!initial.trim());
  const [asked, setAsked] = useState(initial);
  const [answer, setAnswer] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [files, setFiles] = useState(initialFiles);
  const [prs, setPrs] = useState(initialPullRequests);
  const lastPrompt = useRef("");
  const mentions = useFileMentions({
    project,
    draft: question,
    setDraft: setQuestion,
    input,
    enabled: available && editing && !loading,
    attach: (file) => {
      const rest = files.filter(
        (item) =>
          item.path !== file.path ||
          item.startLine !== file.startLine ||
          item.endLine !== file.endLine,
      );
      if (rest.length >= MAX_FILE_CONTEXT) return false;
      setFiles([...rest, file]);
      return true;
    },
  });
  const prMentions = usePullRequestMentions({
    source: pullRequests,
    draft: question,
    setDraft: setQuestion,
    input,
    enabled: available && editing && !loading,
    attach: (pr) => {
      const rest = prs.filter((item) => item.url !== pr.url);
      if (rest.length >= MAX_PULL_REQUEST_CONTEXT) return false;
      setPrs([...rest, pr]);
      return true;
    },
  });
  async function ask(
    text = question,
    prompt = promptWithContext(text, files, prs),
  ) {
    if (
      pending.current ||
      !available ||
      !text.trim() ||
      mentions.reading ||
      prMentions.reading
    )
      return;
    pending.current = true;
    setEditing(false);
    setAsked(text);
    setLoading(true);
    setError(null);
    setAnswer(null);
    lastPrompt.current = prompt;
    try {
      const reply = await invoke<string>("opencode_generate", {
        projectPath: project,
        sessionId,
        prompt,
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
  useLayoutEffect(() => {
    if (editing) input.current?.focus({ preventScroll: true });
    else panel.current?.focus({ preventScroll: true });
  }, [editing]);
  function resizeInput() {
    const area = input.current;
    const measure = measurement.current;
    if (!area || !measure) return;
    measure.style.width = `${area.getBoundingClientRect().width}px`;
    area.style.height = `${Math.min(180, Math.max(56, measure.scrollHeight))}px`;
  }
  useLayoutEffect(resizeInput, [question, editing]);
  useLayoutEffect(() => {
    const area = input.current;
    if (!area) return;
    let width = area.getBoundingClientRect().width;
    const observer = new ResizeObserver(() => {
      const next = area.getBoundingClientRect().width;
      if (next === width) return;
      width = next;
      resizeInput();
    });
    observer.observe(area);
    return () => observer.disconnect();
  }, [editing]);
  return (
    <section
      ref={panel}
      tabIndex={-1}
      className="composer side-question-panel"
      style={{ minHeight }}
      aria-labelledby={`${id}-title`}
      onKeyDown={(event) => {
        if (event.key === "Escape" && !event.nativeEvent.isComposing) {
          event.preventDefault();
          event.stopPropagation();
          close();
        }
      }}
    >
      <header className="side-question-heading">
        <button
          type="button"
          className="icon-button"
          aria-label="Back to message"
          title="Back to message · Esc"
          onClick={close}
        >
          <ArrowLeft size={16} />
        </button>
        <MessageCircleQuestion size={15} aria-hidden="true" />
        <h2 id={`${id}-title`}>Side question</h2>
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
            }}
          >
            <Pencil size={13} />
          </button>
        )}
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
              ref={measurement}
              className="composer-measure"
              aria-hidden="true"
              tabIndex={-1}
              readOnly
              value={question}
            />
            <textarea
              ref={input}
              id={`${id}-input`}
              value={question}
              disabled={!available}
              aria-autocomplete="list"
              aria-haspopup="listbox"
              aria-controls={prMentions.controls ?? mentions.controls}
              aria-activedescendant={prMentions.activeId ?? mentions.activeId}
              placeholder="Ask a quick question…"
              onFocus={() => {
                mentions.focus();
                prMentions.focus();
              }}
              onBlur={() => {
                mentions.blur();
                prMentions.blur();
              }}
              onSelect={() => {
                mentions.select();
                prMentions.select();
              }}
              onChange={(event) => {
                setQuestion(event.target.value);
                mentions.changed();
                prMentions.changed();
              }}
              onKeyDown={(event) => {
                if (prMentions.keyDown(event) || mentions.keyDown(event))
                  return;
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
            {mentions.menu}
            {prMentions.menu}
            <FileContextChips
              files={files}
              remove={(id) => setFiles(files.filter((file) => file.id !== id))}
            />
            <PullRequestContextChips
              pullRequests={prs}
              remove={(id) => setPrs(prs.filter((pr) => pr.id !== id))}
            />
            <div className="side-question-actions">
              <span>Uses this session’s context</span>
              <button
                type="submit"
                className="send-button"
                aria-label="Ask side question"
                title="Ask side question"
                disabled={
                  loading ||
                  !available ||
                  !question.trim() ||
                  mentions.reading ||
                  prMentions.reading
                }
              >
                <ArrowUp size={17} />
              </button>
            </div>
          </form>
        ) : (
          <>
            <p className="side-question-prompt">{asked}</p>
            <FileContextChips files={files} />
            <PullRequestContextChips pullRequests={prs} />
          </>
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
              onClick={() => void ask(asked, lastPrompt.current)}
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
