import {
  useLayoutEffect,
  useRef,
  useState,
  type RefObject,
  type ReactNode,
} from "react";
import { ArrowUp, Gauge, Layers, Square } from "lucide-react";
import type {
  ProviderKind,
  ProviderChoices,
  ModelChoice,
  ContextUsage,
  SessionWorkspaceState,
} from "../../domain/sessions/workspaceState.js";
import { ChoicePicker } from "../primitives/ChoicePicker.js";
import { ModelPicker } from "./ModelPicker.js";
import { ContextUsageIndicator } from "../usage/ContextUsageIndicator.js";
import { AgentPicker } from "./AgentPicker.js";
import { FileContextChips } from "../files/FileNavigation.js";
import type { FileContext } from "../../domain/files/fileContext.js";
import { parseSlashCommand } from "../../domain/composer/slashCommands.js";
import { useSlashCommands } from "./SlashCommandPicker.js";
import { UsageDialog } from "../usage/UsageDialog.js";
import { useFileMentions } from "./FileMentionPicker.js";
import { StatsDialog } from "../usage/StatsDialog.js";
import { TimelineDialog } from "../conversation/TimelineDialog.js";
import { SideQuestionPanel } from "../conversation/SideQuestionPanel.js";

export function Composer({
  projectPath,
  session,
  loadTimeline,
  revealMessage,
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
  newSession,
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
  addFileContext,
  worktreePicker,
}: {
  projectPath: string;
  session: SessionWorkspaceState | null;
  loadTimeline: () => Promise<void>;
  revealMessage: (id: string) => void;
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
  newSession: () => void;
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
  addFileContext: (file: FileContext) => boolean;
  worktreePicker?: ReactNode;
}) {
  const measurement = useRef<HTMLTextAreaElement>(null);
  const recalled = useRef(false);
  const [usageOpen, setUsageOpen] = useState(false);
  const [statsOpen, setStatsOpen] = useState(false);
  const [timelineOpen, setTimelineOpen] = useState(false);
  const [sideQuestion, setSideQuestion] = useState<{
    initial: string;
    id: number;
  } | null>(null);
  const sideQuestionId = useRef(0);
  const closeSideQuestion = () => {
    setSideQuestion(null);
    textareaRef.current?.focus({ preventScroll: true });
  };
  const [commandError, setCommandError] = useState<string | null>(null);
  const parsed = parseSlashCommand(draft);
  const mentions = useFileMentions({
    project: projectPath,
    draft,
    setDraft,
    enabled: !disabled && !submitting,
    input: textareaRef,
    attach: addFileContext,
  });
  const command =
    provider === "opencode" ||
    ["new", "timeline", "btw"].includes(parsed?.name ?? "")
      ? parsed
      : null;
  const slash = useSlashCommands({
    project: projectPath,
    provider,
    enabled: !disabled && !submitting,
    draft,
    setDraft,
    run: send,
    input: textareaRef,
  });
  function send(text = draft) {
    if (disabled || submitting || mentions.reading) return;
    const command = parseSlashCommand(text);
    if (command?.name === "new") {
      if (command.arguments.trim()) {
        setCommandError("/new does not take arguments.");
        return;
      }
      setCommandError(null);
      setDraft("");
      newSession();
    } else if (command?.name === "timeline") {
      if (!session || command.arguments.trim()) {
        setCommandError(
          !session
            ? "Open a session to view its timeline."
            : "/timeline does not take arguments.",
        );
        return;
      }
      setCommandError(null);
      setDraft("");
      setTimelineOpen(true);
    } else if (provider === "opencode" && command?.name === "usage") {
      if (command.arguments.trim()) {
        setCommandError("/usage does not take arguments.");
        return;
      }
      setCommandError(null);
      setDraft("");
      setUsageOpen(true);
    } else if (provider === "opencode" && command?.name === "stats") {
      if (command.arguments.trim()) {
        setCommandError(
          "/stats does not take arguments. Use the filters in the statistics dialog.",
        );
        return;
      }
      setCommandError(null);
      setDraft("");
      setStatsOpen(true);
    } else if (command?.name === "btw") {
      if (provider !== "opencode" || !session) {
        setCommandError(
          "Open an OpenCode session to ask a temporary side question.",
        );
        return;
      }
      setCommandError(null);
      setDraft("");
      setSideQuestion({
        initial: command.arguments,
        id: ++sideQuestionId.current,
      });
    } else submit();
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
      {sideQuestion && session && (
        <SideQuestionPanel
          key={sideQuestion.id}
          project={projectPath}
          sessionId={session.session.thread.id}
          initial={sideQuestion.initial}
          available={!disabled}
          resumeDraft={() =>
            textareaRef.current?.focus({ preventScroll: true })
          }
          close={closeSideQuestion}
        />
      )}
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
        {mentions.menu}
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
          aria-autocomplete="list"
          aria-haspopup="listbox"
          aria-controls={mentions.controls ?? slash.controls}
          aria-activedescendant={mentions.activeId ?? slash.activeId}
          aria-keyshortcuts={canCycleAgent ? "Shift+Tab" : undefined}
          title={!draft && lastMessage ? "↑ Recall last message" : undefined}
          placeholder={
            working
              ? "Queue your next message…"
              : "Ask anything, / for commands, @ for files…"
          }
          value={draft}
          disabled={disabled}
          onFocus={() => {
            slash.focus();
            mentions.focus();
          }}
          onBlur={() => {
            slash.blur();
            mentions.blur();
          }}
          onSelect={mentions.select}
          onChange={(event) => {
            setDraft(event.target.value);
            setCommandError(null);
            slash.changed();
            mentions.changed();
          }}
          onKeyDown={(event) => {
            if (mentions.keyDown(event) || slash.keyDown(event)) return;
            if (event.key === "Escape" && sideQuestion) {
              event.preventDefault();
              closeSideQuestion();
              return;
            }
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
            {isNew && worktreePicker}
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
                    : "↵ Queue · ⇧↵ Newline"
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
          {
            <button
              key="send"
              type="button"
              className="send-button"
              onClick={() => send()}
              disabled={
                disabled ||
                submitting ||
                mentions.reading ||
                !draft.trim() ||
                draft.trim() === "/"
              }
              aria-label={
                command
                  ? "Run command"
                  : working
                    ? "Queue message"
                    : "Send message"
              }
              title={
                command
                  ? "Run command"
                  : working
                    ? "Queue message"
                    : "Send message"
              }
            >
              <ArrowUp size={17} />
            </button>
          }
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
      {statsOpen && (
        <StatsDialog project={projectPath} close={() => setStatsOpen(false)} />
      )}
      {timelineOpen && session && (
        <TimelineDialog
          session={session}
          loadHistory={loadTimeline}
          reveal={revealMessage}
          close={() => setTimelineOpen(false)}
        />
      )}
    </div>
  );
}
