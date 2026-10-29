import { useLayoutEffect, type RefObject } from "react";
import { ArrowUp, Bot, Cpu, Layers, LoaderCircle, Square } from "lucide-react";
import type { ProviderKind, ProviderChoices } from "./workspaceState.js";
import { ChoicePicker } from "./ChoicePicker.js";

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
                <ChoicePicker
                  label="Provider"
                  icon={<Layers size={13} />}
                  value={provider}
                  onChange={(value) => changeProvider(value as ProviderKind)}
                  disabled={submitting}
                  choices={[
                    { id: "codex", name: "Codex" },
                    { id: "opencode", name: "OpenCode" },
                  ]}
                />
                {Boolean(choices?.models.length) && (
                  <ChoicePicker
                    label="Model"
                    icon={<Cpu size={13} />}
                    value={model}
                    disabled={submitting}
                    searchable
                    onChange={(value) => setOption("model", value)}
                    choices={[
                      {
                        id: "",
                        name: "Default model",
                        description: "Use provider settings",
                      },
                      ...choices!.models.map((choice) => ({
                        ...choice,
                        description: choice.id,
                      })),
                    ]}
                  />
                )}
                {Boolean(choices?.agents.length) && (
                  <ChoicePicker
                    label="Agent"
                    icon={<Bot size={13} />}
                    value={agent}
                    disabled={submitting}
                    onChange={(value) => setOption("agent", value)}
                    choices={[
                      { id: "", name: "Default agent" },
                      ...choices!.agents,
                    ]}
                  />
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
