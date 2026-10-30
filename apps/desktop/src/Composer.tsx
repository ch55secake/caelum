import { useLayoutEffect, useRef, useState, type RefObject } from "react";
import { ArrowUp, Gauge, Layers, Square } from "lucide-react";
import type {
  ProviderKind,
  ProviderChoices,
  ModelChoice,
  ContextUsage,
} from "./workspaceState.js";
import { ChoicePicker } from "./ChoicePicker.js";
import { ModelPicker } from "./ModelPicker.js";
import { ContextUsageIndicator } from "./ContextUsageIndicator.js";
import { AgentPicker } from "./AgentPicker.js";
import { FileContextChips } from "./FileNavigation.js";
import type { FileContext } from "./fileContext.js";
import { parseSlashCommand } from "./slashCommands.js";
import { useSlashCommands } from "./SlashCommandPicker.js";
import { UsageDialog } from "./UsageDialog.js";

export function Composer({
  projectPath,
  draft,
  setDraft,
  lastMessage,
  provider,
  changeProvider,
  isNew,
  working,
  activity,
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
  contextUsage,
  favouriteModels,
  toggleFavouriteModel,
  agent,
  currentAgent,
  setOption,
  choicesError,
  retryChoices,
  awaitingApproval,
  fileContext,
  removeFileContext,
}: {
  projectPath: string;
  draft: string;
  setDraft: (text: string) => void;
  lastMessage: string | null;
  provider: ProviderKind;
  changeProvider: (kind: ProviderKind) => void;
  isNew: boolean;
  working: boolean;
  activity?: string;
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
  contextUsage: ContextUsage | null;
  favouriteModels: string[];
  toggleFavouriteModel: (id: string) => void;
  agent: string;
  currentAgent: string | null;
  setOption: (key: "model" | "agent" | "effort", value: string) => void;
  choicesError: string | null;
  retryChoices: () => void;
  awaitingApproval: boolean;
  fileContext: FileContext[];
  removeFileContext: (id: string) => void;
}) {
  const measurement = useRef<HTMLTextAreaElement>(null);
  const recalled = useRef(false);
  const [usageOpen, setUsageOpen] = useState(false);
  const [commandError, setCommandError] = useState<string | null>(null);
  const command = provider === "opencode" ? parseSlashCommand(draft) : null;
  const slash = useSlashCommands({
    project: projectPath,
    enabled: provider === "opencode" && !disabled && !submitting,
    draft,
    setDraft,
    run: send,
    input: textareaRef,
  });
  function send(text = draft) {
    if (disabled || submitting) return;
    const command = provider === "opencode" ? parseSlashCommand(text) : null;
    if (command?.name === "usage") {
      if (command.arguments.trim()) {
        setCommandError("/usage does not take arguments.");
        return;
      }
      setCommandError(null);
      setDraft("");
      setUsageOpen(true);
    } else if (!working || command) submit();
  }
  const busy = working || submitting || awaitingApproval;
  const agents = choices?.agents ?? [];
  const canCycleAgent =
    provider === "opencode" &&
    !disabled &&
    !submitting &&
    agents.some((choice) => choice.id !== agent);
  const effortName = (name: string) =>
    name === "xhigh"
      ? "Extra high"
      : name.charAt(0).toUpperCase() + name.slice(1);
  function resizeInput() {
    const area = textareaRef.current;
    const measure = measurement.current;
    if (!area || !measure) return;
    // Measuring outside the flow avoids briefly expanding the transcript and
    // clamping its scroll position on every keystroke in a multiline draft.
    measure.style.width = `${area.getBoundingClientRect().width}px`;
    area.style.height = `${Math.min(180, Math.max(56, measure.scrollHeight))}px`;
  }
  useLayoutEffect(resizeInput, [draft, textareaRef]);
  useLayoutEffect(() => {
    if (!recalled.current) return;
    recalled.current = false;
    textareaRef.current?.setSelectionRange(draft.length, draft.length);
  }, [draft, textareaRef]);
  useLayoutEffect(() => {
    const area = textareaRef.current;
    if (!area) return;
    let width = area.getBoundingClientRect().width;
    const observer = new ResizeObserver(() => {
      const nextWidth = area.getBoundingClientRect().width;
      if (nextWidth === width) return;
      width = nextWidth;
      resizeInput();
    });
    observer.observe(area);
    return () => observer.disconnect();
  }, [textareaRef]);
  return (
    <div className="composer-wrap">
      {busy && (
        <span className="sr-only" role="status" aria-label="Agent activity">
          {awaitingApproval
            ? "Approval required"
            : (activity ?? "Agent working")}
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
          send();
        }}
      >
        {slash.menu}
        <FileContextChips
          files={fileContext}
          remove={removeFileContext}
          disabled={submitting}
        />
        <textarea
          ref={measurement}
          className="composer-measure"
          aria-hidden="true"
          tabIndex={-1}
          readOnly
          value={draft}
        />
        <textarea
          ref={textareaRef}
          aria-label="Message"
          aria-autocomplete={provider === "opencode" ? "list" : undefined}
          aria-haspopup={provider === "opencode" ? "listbox" : undefined}
          aria-controls={slash.controls}
          aria-activedescendant={slash.activeId}
          aria-keyshortcuts={canCycleAgent ? "Shift+Tab" : undefined}
          title={!draft && lastMessage ? "↑ Recall last message" : undefined}
          placeholder={
            working
              ? "Draft your next message…"
              : provider === "opencode"
                ? "Ask anything, or type / for commands…"
                : "Ask anything, or describe a change…"
          }
          value={draft}
          disabled={disabled}
          onFocus={slash.focus}
          onBlur={slash.blur}
          onChange={(event) => {
            setDraft(event.target.value);
            setCommandError(null);
            slash.changed();
          }}
          onKeyDown={(event) => {
            if (slash.keyDown(event)) return;
            if (
              event.key === "Tab" &&
              event.shiftKey &&
              !event.altKey &&
              !event.metaKey &&
              !event.ctrlKey &&
              !event.nativeEvent.isComposing &&
              canCycleAgent
            ) {
              event.preventDefault();
              if (!event.repeat) {
                const index = agents.findIndex((choice) => choice.id === agent);
                setOption("agent", agents[(index + 1) % agents.length].id);
              }
              return;
            }
            if (
              event.key === "ArrowUp" &&
              !event.nativeEvent.isComposing &&
              !event.shiftKey &&
              !event.altKey &&
              !event.metaKey &&
              !event.ctrlKey &&
              !submitting &&
              !draft &&
              lastMessage
            ) {
              event.preventDefault();
              recalled.current = true;
              setDraft(lastMessage);
              return;
            }
            if (
              event.key === "Enter" &&
              !event.nativeEvent.isComposing &&
              (!event.shiftKey || event.metaKey || event.ctrlKey)
            ) {
              event.preventDefault();
              send();
            }
          }}
        />
        {commandError && (
          <p className="composer-command-error" role="alert">
            {commandError}
          </p>
        )}
        {provider === "opencode" && (
          <AgentPicker
            value={agent}
            current={currentAgent}
            choices={agents}
            isNew={isNew}
            working={working}
            disabled={disabled || submitting}
            onChange={(value) => setOption("agent", value)}
          />
        )}
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
              <ModelPicker
                label="Model"
                provider={provider}
                value={model}
                disabled={submitting || disabled}
                onChange={(value) => setOption("model", value)}
                models={choices?.models ?? []}
                favouriteModels={favouriteModels}
                toggleFavourite={toggleFavouriteModel}
                unavailableDescription="Current selection · currently unavailable"
                defaultChoice={{
                  name: isNew ? "Default model" : "Session model",
                  description: isNew
                    ? "Use provider settings"
                    : "Keep this session’s configured model",
                }}
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
                  ? activity === "Compacting context…"
                    ? activity
                    : "Draft while the agent works"
                  : !draft && lastMessage
                    ? "↑ Recall · ↵ Send"
                    : "↵ Send · ⇧↵ Newline"}
          </span>
          {!isNew && (
            <ContextUsageIndicator usage={contextUsage} choices={choices} />
          )}
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
          ) : null}
          {(!working || command) && (
            <button
              key="send"
              type="button"
              className="send-button"
              onClick={() => send()}
              disabled={
                disabled ||
                submitting ||
                !draft.trim() ||
                (provider === "opencode" && draft.trim() === "/")
              }
              aria-label={command ? "Run command" : "Send message"}
              title={command ? "Run command" : "Send message"}
            >
              <ArrowUp size={17} />
            </button>
          )}
        </div>
      </form>
      {usageOpen && (
        <UsageDialog
          project={projectPath}
          usage={contextUsage}
          choices={choices}
          close={() => setUsageOpen(false)}
        />
      )}
    </div>
  );
}
