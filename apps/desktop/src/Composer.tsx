import { useLayoutEffect, type RefObject } from "react";
import { ArrowUp, LoaderCircle, Square } from "lucide-react";
import type { ProviderKind, ProviderChoices } from "./workspaceState.js";

export function Composer({
  draft,
  setDraft,
  provider,
  changeProvider,
  isNew,
  working,
  submitting,
  disabled,
  textareaRef,
  submit,
  cancel,
  choices,
  model,
  agent,
  setOption,
  choicesError,
  retryChoices,
}: {
  draft: string;
  setDraft: (text: string) => void;
  provider: ProviderKind;
  changeProvider: (kind: ProviderKind) => void;
  isNew: boolean;
  working: boolean;
  submitting: boolean;
  disabled: boolean;
  textareaRef: RefObject<HTMLTextAreaElement | null>;
  submit: () => void;
  cancel: () => void;
  choices: ProviderChoices | null;
  model: string;
  agent: string;
  setOption: (key: "model" | "agent", value: string) => void;
  choicesError: string | null;
  retryChoices: () => void;
}) {
  useLayoutEffect(() => {
    const area = textareaRef.current;
    if (area) {
      area.style.height = "0px";
      area.style.height = `${Math.min(180, Math.max(56, area.scrollHeight))}px`;
    }
  }, [draft]);
  return (
    <div className="composer-wrap">
      <form
        className="composer"
        onSubmit={(event) => {
          event.preventDefault();
          submit();
        }}
      >
        <textarea
          ref={textareaRef}
          aria-label="Message"
          placeholder={
            working
              ? "Draft your next message…"
              : "Ask anything, or describe a change…"
          }
          value={draft}
          disabled={disabled}
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (
              event.key === "Enter" &&
              !event.nativeEvent.isComposing &&
              (!event.shiftKey || event.metaKey || event.ctrlKey)
            ) {
              event.preventDefault();
              if (!working && !submitting) submit();
            }
          }}
        />
        <div className="composer-footer">
          <div className="composer-controls">
            {isNew ? (
              <>
                <select
                  aria-label="Provider"
                  value={provider}
                  onChange={(event) =>
                    changeProvider(event.target.value as ProviderKind)
                  }
                  disabled={submitting}
                >
                  <option value="codex">Codex</option>
                  <option value="opencode">OpenCode</option>
                </select>
                {Boolean(choices?.models.length) && (
                  <select
                    aria-label="Model"
                    value={model}
                    disabled={submitting}
                    onChange={(event) => setOption("model", event.target.value)}
                  >
                    <option value="">Default model</option>
                    {choices!.models.map((choice) => (
                      <option key={choice.id} value={choice.id}>
                        {choice.name}
                      </option>
                    ))}
                  </select>
                )}
                {Boolean(choices?.agents.length) && (
                  <select
                    aria-label="Agent"
                    value={agent}
                    disabled={submitting}
                    onChange={(event) => setOption("agent", event.target.value)}
                  >
                    <option value="">Default agent</option>
                    {choices!.agents.map((choice) => (
                      <option key={choice.id} value={choice.id}>
                        {choice.name}
                      </option>
                    ))}
                  </select>
                )}
                {choicesError && (
                  <button
                    type="button"
                    className="text-button choices-retry"
                    title={choicesError}
                    onClick={retryChoices}
                  >
                    Load models
                  </button>
                )}
              </>
            ) : (
              <span className="provider-label">
                {provider === "codex" ? "Codex" : "OpenCode"}
              </span>
            )}
          </div>
          <span className="composer-hint">
            {working ? "Draft while the agent works" : "↵ Send · ⇧↵ Newline"}
          </span>
          {working ? (
            <button
              key="stop"
              type="button"
              className="send-button stop-button"
              onClick={(event) => {
                event.preventDefault();
                cancel();
              }}
              aria-label="Stop generation"
              title="Stop generation"
            >
              <Square size={13} />
            </button>
          ) : (
            <button
              key="send"
              type="button"
              className="send-button"
              onClick={submit}
              disabled={disabled || submitting || !draft.trim()}
              aria-label="Send message"
              title="Send message"
            >
              {submitting ? (
                <LoaderCircle size={15} className="spinning" />
              ) : (
                <ArrowUp size={17} />
              )}
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
