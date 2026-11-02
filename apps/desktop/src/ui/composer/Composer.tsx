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
import {
  MAX_FILE_CONTEXT,
  type FileContext,
} from "../../domain/files/fileContext.js";
import { parseSlashCommand } from "../../domain/composer/slashCommands.js";
import { useSlashCommands } from "./SlashCommandPicker.js";
import { UsageDialog } from "../usage/UsageDialog.js";
import { useFileMentions } from "./FileMentionPicker.js";
import { StatsDialog } from "../usage/StatsDialog.js";
import { TimelineDialog } from "../conversation/TimelineDialog.js";
import { SideQuestionPanel } from "../conversation/SideQuestionPanel.js";
import {
  usePullRequestMentions,
  type SessionPullRequests,
} from "./PullRequestMentionPicker.js";
import {
  MAX_PULL_REQUEST_CONTEXT,
  type PullRequestContext,
} from "../../domain/pull-requests/pullRequestContext.js";
import { PullRequestContextChips } from "../pull-requests/PullRequestContextChips.js";

export function Composer({
  projectPath,
  session,
  loadTimeline,
  revealMessage,
  onLayoutChange,
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
  pullRequests,
  pullRequestContext,
  addPullRequestContext,
  removePullRequestContext,
  worktreePicker,
}: {
  projectPath: string;
  session: SessionWorkspaceState | null;
  loadTimeline: () => Promise<void>;
  revealMessage: (id: string) => void;
  onLayoutChange: () => void;
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
  pullRequests: SessionPullRequests;
  pullRequestContext: PullRequestContext[];
  addPullRequestContext: (pr: PullRequestContext) => boolean;
  removePullRequestContext: (id: string) => void;
  worktreePicker?: ReactNode;
}) {
  const measurement = useRef<HTMLTextAreaElement>(null);
  const recalled = useRef(false);
  const [usageOpen, setUsageOpen] = useState(false);
  const [statsOpen, setStatsOpen] = useState(false);
  const [timelineOpen, setTimelineOpen] = useState(false);
  const [sideQuestion, setSideQuestion] = useState<{
    initial: string;
    files: FileContext[];
    pullRequests: PullRequestContext[];
    minHeight: number;
    id: number;
  } | null>(null);
  const sideQuestionId = useRef(0);
  const mainDraft = useRef(draft);
  const wasSideQuestion = useRef(false);
  useLayoutEffect(() => {
    if (!draft.startsWith("/")) mainDraft.current = draft;
  }, [draft]);
  useLayoutEffect(() => {
    if (wasSideQuestion.current && !sideQuestion)
      textareaRef.current?.focus({ preventScroll: true });
    wasSideQuestion.current = Boolean(sideQuestion);
  }, [sideQuestion, textareaRef]);
  const closeSideQuestion = () => {
    setSideQuestion(null);
  };
  const [commandError, setCommandError] = useState<string | null>(null);
  const parsed = parseSlashCommand(draft);
  const preparingSideQuestion =
    provider === "opencode" && parsed?.name === "btw";
  const [sideFiles, setSideFiles] = useState<FileContext[]>([]);
  const [sidePrs, setSidePrs] = useState<PullRequestContext[]>([]);
  useLayoutEffect(() => {
    if (!preparingSideQuestion) {
      setSideFiles([]);
      setSidePrs([]);
    }
  }, [preparingSideQuestion]);
  const mentions = useFileMentions({
    project: projectPath,
    draft,
    setDraft,
    enabled: !disabled && !submitting && !sideQuestion,
    input: textareaRef,
    attach: (file) => {
      if (!preparingSideQuestion) return addFileContext(file);
      const rest = sideFiles.filter(
        (item) =>
          item.path !== file.path ||
          item.startLine !== file.startLine ||
          item.endLine !== file.endLine,
      );
      if (rest.length >= MAX_FILE_CONTEXT) return false;
      setSideFiles([...rest, file]);
      return true;
    },
  });
  const prMentions = usePullRequestMentions({
    source: pullRequests,
    draft,
    setDraft,
    enabled: Boolean(session) && !disabled && !submitting && !sideQuestion,
    input: textareaRef,
    attach: (pr) => {
      if (!preparingSideQuestion) return addPullRequestContext(pr);
      const rest = sidePrs.filter((item) => item.url !== pr.url);
      if (rest.length >= MAX_PULL_REQUEST_CONTEXT) return false;
      setSidePrs([...rest, pr]);
      return true;
    },
  });
  const command =
    provider === "opencode" ||
    ["new", "timeline", "btw", "usage"].includes(parsed?.name ?? "")
      ? parsed
      : null;
  const slash = useSlashCommands({
    project: projectPath,
    provider,
    enabled: !disabled && !submitting && !sideQuestion,
    draft,
    setDraft,
    run: send,
    input: textareaRef,
  });
  function send(text = draft) {
    if (disabled || submitting || mentions.reading || prMentions.reading)
      return;
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
    } else if (command?.name === "usage") {
      if (provider !== "opencode") {
        setCommandError("/usage is only available with OpenCode.");
        return;
      }
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
          "/stats does not take arguments. Choose a project in the statistics dialog.",
        );
        return;
      }
      setCommandError(null);
      setDraft("");
      setStatsOpen(true);
    } else if (command?.name === "btw") {
      if (provider !== "opencode" || !session) {
        setCommandError(
          "Open an OpenCode session to ask a side question.",
        );
        return;
      }
      setCommandError(null);
      setDraft(mainDraft.current);
      setSideQuestion({
        initial: command.arguments,
        files: sideFiles,
        pullRequests: sidePrs,
        minHeight:
          textareaRef.current?.closest("form")?.getBoundingClientRect().height ??
          0,
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
  useLayoutEffect(resizeInput, [draft, textareaRef, sideQuestion]);
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
  }, [textareaRef, sideQuestion]);
  useLayoutEffect(() => {
    onLayoutChange();
  }, [sideQuestion]);
  return (
    <div className="composer-wrap">
      {sideQuestion && session && (
        <SideQuestionPanel
          key={sideQuestion.id}
          project={projectPath}
          sessionId={session.session.thread.id}
          initial={sideQuestion.initial}
          initialFiles={sideQuestion.files}
          initialPullRequests={sideQuestion.pullRequests}
          pullRequests={pullRequests}
          available={!disabled}
          minHeight={sideQuestion.minHeight}
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
      {awaitingApproval && !sideQuestion && (
        <div className="composer-approval">Approval required</div>
      )}
      {!sideQuestion && (
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
          {prMentions.menu}
          <FileContextChips
            files={preparingSideQuestion ? sideFiles : fileContext}
            remove={
              preparingSideQuestion
                ? (id) =>
                    setSideFiles(sideFiles.filter((file) => file.id !== id))
                : removeFileContext
            }
            disabled={submitting}
          />
          <PullRequestContextChips
            pullRequests={preparingSideQuestion ? sidePrs : pullRequestContext}
            remove={
              preparingSideQuestion
                ? (id) => setSidePrs(sidePrs.filter((pr) => pr.id !== id))
                : removePullRequestContext
            }
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
            aria-controls={
              prMentions.controls ?? mentions.controls ?? slash.controls
            }
            aria-activedescendant={
              prMentions.activeId ?? mentions.activeId ?? slash.activeId
            }
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
              prMentions.focus();
            }}
            onBlur={() => {
              slash.blur();
              mentions.blur();
              prMentions.blur();
            }}
            onSelect={() => {
              mentions.select();
              prMentions.select();
            }}
            onChange={(event) => {
              setDraft(event.target.value);
              setCommandError(null);
              slash.changed();
              mentions.changed();
              prMentions.changed();
            }}
            onKeyDown={(event) => {
              if (
                prMentions.keyDown(event) ||
                mentions.keyDown(event) ||
                slash.keyDown(event)
              )
                return;
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
                  const index = agents.findIndex(
                    (choice) => choice.id === agent,
                  );
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
                title="Stop generation · Esc Esc"
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
                  prMentions.reading ||
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
      )}
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
          restoreFocus={() =>
            textareaRef.current?.focus({ preventScroll: true })
          }
        />
      )}
    </div>
  );
}
