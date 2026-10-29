import { useLayoutEffect, type RefObject } from "react";
import { ArrowUp, Bot, Cpu, Gauge, Layers, Square } from "lucide-react";
import type {
  ProviderKind,
  ProviderChoices,
  ModelChoice,
} from "./workspaceState.js";
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
  modelPending,
  effort,
  effortPending,
  modelChoice,
  agent,
  setOption,
  choicesError,
  retryChoices,
  awaitingApproval,
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
  modelPending: boolean;
  effort: string;
  effortPending: boolean;
  modelChoice?: ModelChoice;
  agent: string;
  setOption: (key: "model" | "agent" | "effort", value: string) => void;
  choicesError: string | null;
  retryChoices: () => void;
  awaitingApproval: boolean;
}) {
  const busy = working || submitting || awaitingApproval;
  const effortName = (name: string) =>
    name === "xhigh"
      ? "Extra high"
      : name.charAt(0).toUpperCase() + name.slice(1);
  useLayoutEffect(() => {
    const area = textareaRef.current;
    if (area) {
      area.style.height = "0px";
      area.style.height = `${Math.min(180, Math.max(56, area.scrollHeight))}px`;
    }
  }, [draft]);
  return (
    <div className="composer-wrap">
      {busy && (
        <span className="sr-only" role="status" aria-label="Agent activity">
          {awaitingApproval ? "Approval required" : "Agent working"}
        </span>
      )}
      {awaitingApproval && (
        <div className="composer-approval">Approval required</div>
      )}
      <form
        className="composer"
        aria-label="Message composer"
        data-working={busy}
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
            ) : (
              <span className="provider-label">
                {provider === "codex" ? "Codex" : "OpenCode"}
              </span>
            )}
            {(Boolean(choices?.models.length) || Boolean(model)) && (
              <ChoicePicker
                label="Model"
                icon={<Cpu size={13} />}
                value={model}
                disabled={submitting || disabled}
                searchable
                onChange={(value) => setOption("model", value)}
                choices={[
                  {
                    id: "",
                    name: isNew ? "Default model" : "Session model",
                    description: isNew
                      ? "Use provider settings"
                      : "Keep this session’s configured model",
                  },
                  ...(model &&
                  !choices?.models.some((choice) => choice.id === model)
                    ? [
                        {
                          id: model,
                          name: model,
                          description: "Current selection",
                        },
                      ]
                    : []),
                  ...(choices?.models ?? []).map((choice) => ({
                    ...choice,
                    description: choice.id,
                  })),
                ]}
              />
            )}
            {Boolean(modelChoice?.efforts.length) && (
              <ChoicePicker
                label="Effort"
                icon={<Gauge size={13} />}
                value={effort}
                disabled={submitting || disabled}
                onChange={(value) => setOption("effort", value)}
                choices={[
                  {
                    id: "",
                    name: "Default effort",
                    description: modelChoice?.defaultEffort
                      ? `Use ${effortName(modelChoice.defaultEffort).toLowerCase()} effort`
                      : "Use model settings",
                  },
                  ...(effort &&
                  !modelChoice!.efforts.some((choice) => choice.id === effort)
                    ? [
                        {
                          id: effort,
                          name: effortName(effort),
                          description: "Current selection",
                        },
                      ]
                    : []),
                  ...modelChoice!.efforts.map((choice) => ({
                    ...choice,
                    name: effortName(choice.name),
                  })),
                ]}
              />
            )}
            {isNew && Boolean(choices?.agents.length) && (
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
          </div>
          <span className="composer-hint">
            {effortPending
              ? "Settings apply to next message"
              : modelPending
                ? "Model applies to next message"
                : working
                  ? "Draft while the agent works"
                  : "↵ Send · ⇧↵ Newline"}
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
              <ArrowUp size={17} />
            </button>
          )}
        </div>
      </form>
    </div>
  );
}
