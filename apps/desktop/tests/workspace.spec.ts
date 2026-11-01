import { expect, test, type Page } from "@playwright/test";
import type { QuestionPrompt } from "../src/questionState.js";
import type { GitWorktree } from "../src/worktreeState.js";

async function desktop(
  page: Page,
  selected: string | null = "thread-a",
  questionOptions: {
    initialQuestions?: QuestionPrompt[];
    holdQuestions?: boolean;
    worktrees?: boolean;
  } = {},
) {
  await page.addInitScript(
    ({ selected, initialQuestions, holdQuestions, worktrees }) => {
      const callbacks = new Map<number, (event: unknown) => void>();
      const listeners = new Map<string, number[]>();
      const project = { name: "caelum", path: "/work/caelum" };
      const otherProject = { name: "other", path: "/work/other" };
      const fileKey = (project: string, path: string) => `${project}\0${path}`;
      const projectFiles = new Map<string, string | null>([
        [fileKey(project.path, "src/App.tsx"), "export const ready = true;\n"],
        [fileKey(project.path, "src/styles.css"), "body { color: violet; }\n"],
        [
          fileKey(project.path, "README.md"),
          "# Caelum\nProject documentation.\n",
        ],
        [
          fileKey(project.path, "vendor/generated.ts"),
          "export const generated = true;\n",
        ],
        [
          fileKey(otherProject.path, "src/App.tsx"),
          "export const other = true;\n",
        ],
        ...JSON.parse(sessionStorage.getItem("project-files") ?? "[]"),
      ]);
      const archived = new Set<string>(
        JSON.parse(sessionStorage.getItem("archived") ?? "[]"),
      );
      const sessionKey = (path: string, provider: string, id: string) =>
        `${path}:${provider}:${id}`;
      const gates = new Map<
        string,
        { promise: Promise<void>; release: () => void }
      >();
      const sessionDetails = new Map<string, any>();
      let questions =
        initialQuestions ??
        JSON.parse(sessionStorage.getItem("questions") ?? "[]");
      if (holdQuestions) {
        let release = () => {};
        const promise = new Promise<void>((resolve) => {
          release = resolve;
        });
        gates.set("pending_questions", { promise, release });
      }
      let failNextArchive = false;
      const threads = [
        {
          id: "thread-a",
          name: "Refine the session workspace",
          preview: "",
          updatedAt: 123,
        },
        {
          id: "thread-b",
          name: "Review the provider adapters",
          preview: "",
          updatedAt: 122,
        },
      ];
      const calls: { command: string; args: Record<string, unknown> }[] = [];
      let checkouts: GitWorktree[] = JSON.parse(
        sessionStorage.getItem("worktrees") ?? "null",
      ) ?? [
        {
          path: project.path,
          project,
          branch: "main",
          main: true,
          available: true,
          locked: null,
          prunable: null,
        },
      ];
      const savedWorkspace = JSON.parse(
        sessionStorage.getItem("worktree-workspace") ?? "null",
      );
      const savedProjects = new Map<string, any>(
        savedWorkspace?.projects?.map((entry: any) => [
          entry.project.path,
          entry,
        ]) ?? [
          [
            project.path,
            {
              project,
              rootPath: null,
              provider: "codex",
              codexSessionId: selected,
              opencodeSessionId: null,
              codexSessionIds: threads.map((entry) => entry.id),
              opencodeSessionIds: [],
            },
          ],
        ],
      );
      let activeWorktree = savedWorkspace?.activeProjectPath ?? project.path;
      const saveWorktrees = () => {
        sessionStorage.setItem("worktrees", JSON.stringify(checkouts));
        sessionStorage.setItem(
          "worktree-workspace",
          JSON.stringify({
            projects: [...savedProjects.values()],
            activeProjectPath: activeWorktree,
          }),
        );
      };
      const checkoutProject = (path: string) => ({
        name: path === otherProject.path ? "other" : "caelum",
        path,
      });
      const rememberCheckout = (path: string) => {
        if (!savedProjects.has(path))
          savedProjects.set(path, {
            project: checkoutProject(path),
            rootPath:
              path === project.path || path === otherProject.path
                ? null
                : project.path,
            provider: "codex",
            codexSessionId: null,
            opencodeSessionId: null,
            codexSessionIds: [],
            opencodeSessionIds: [],
          });
        saveWorktrees();
      };
      const addWorktree = (
        path: string,
        branch: string | null,
        options: Partial<GitWorktree> = {},
      ) => {
        const entry = {
          path,
          project: checkoutProject(path),
          branch,
          main: false,
          available: true,
          locked: null,
          prunable: null,
          ...options,
        };
        checkouts = [...checkouts.filter((item) => item.path !== path), entry];
        projectFiles.set(
          fileKey(path, "src/App.tsx"),
          `export const checkout = ${JSON.stringify(branch)};\n`,
        );
        projectFiles.set(fileKey(path, "README.md"), `# ${branch}\n`);
        saveWorktrees();
        return entry;
      };
      let clipboard = "";
      let copyError = false;
      let contextEvent: Event | null = null;
      document.addEventListener(
        "contextmenu",
        (event) => {
          contextEvent = event;
        },
        true,
      );
      const models = new Map(
        ["thread-a", "thread-b", "ses-a"].map((id) => [id, "provider/model"]),
      );
      const efforts = new Map(
        ["thread-a", "thread-b", "ses-a"].map((id) => [id, "medium"]),
      );
      let failNextSend = false;
      const commandErrors = new Map<string, string>();
      const slashCommands = new Map<
        string,
        { name: string; description?: string }[]
      >();
      let usageLimits: unknown = null;
      let commandResult: {
        text: string;
        working: boolean;
        agent?: string;
        model?: string;
      } | null = null;
      const choicesErrors = new Map<string, string>();
      const choiceAgents = [
        { id: "build", name: "Build" },
        { id: "plan", name: "Plan" },
        { id: "reviewer", name: "Reviewer" },
      ];
      const agents = new Map<string, string | null>(
        JSON.parse(sessionStorage.getItem("session-agents") ?? "[]"),
      );
      const agentFor = (path: string, id: string) =>
        agents.has(`${path}:${id}`) ? agents.get(`${path}:${id}`) : "build";
      const setAgent = (path: string, id: string, agent: string | null) => {
        agents.set(`${path}:${id}`, agent);
        sessionStorage.setItem("session-agents", JSON.stringify([...agents]));
      };
      let agentError = false;
      const choiceModels = new Map<string, any[]>();
      const codexUsage = new Map<string, unknown>(
        JSON.parse(sessionStorage.getItem("codex-context") ?? "[]"),
      );
      let sequence = 0;
      let historyGate: Promise<void> | null = null;
      let startGate: Promise<void> | null = null;
      let releaseStart: () => void = () => {};
      type GitChange = {
        path: string;
        indexStatus: string;
        worktreeStatus: string;
        modifiedAt?: string | null;
        revision?: string;
      };
      const gitProjects = new Map<string, GitChange[]>([
        [
          project.path,
          [
            {
              path: "src/App.tsx",
              indexStatus: " ",
              worktreeStatus: "M",
              modifiedAt: "200",
              revision: "app-1",
            },
            {
              path: "src/styles.css",
              indexStatus: "M",
              worktreeStatus: " ",
              modifiedAt: "100",
              revision: "css-1",
            },
          ],
        ],
      ]);
      const gitDiffs = new Map<string, Record<string, string>>();
      const gitVersions = new Map<
        string,
        { original: string; modified: string }
      >();
      const gitMetadata = new Map<
        string,
        { branch: string | null; repositoryRoot: string | null }
      >();
      const gitErrors = new Map<string, string>();
      const pullRequests = new Map<string, unknown>();
      let diffVersion = 1;
      const codexHistory = new Map<
        string,
        { data: unknown[]; isWorking: boolean }
      >();
      let openHistory: unknown[] = [
        {
          id: "msg-history",
          type: "assistant",
          content: [{ type: "text", text: "Restored OpenCode conversation" }],
        },
      ];
      let openWorking: boolean | undefined;
      let release: () => void = () => {};
      const emit = (event: string, payload: unknown) =>
        (listeners.get(event) ?? []).forEach((id) =>
          callbacks.get(id)?.({ event, id, payload }),
        );
      Object.assign(window, {
        isTauri: true,
        fixture: {
          calls,
          addWorktree,
          forgetWorktree: (path: string) => {
            checkouts = checkouts.filter((entry) => entry.path !== path);
            saveWorktrees();
          },
          emit,
          setQuestions: (value: QuestionPrompt[], notify = true) => {
            questions = value;
            sessionStorage.setItem("questions", JSON.stringify(value));
            if (notify) emit("question-requests", value);
          },
          contextPrevented: () => contextEvent?.defaultPrevented ?? false,
          clipboard: () => clipboard,
          setClipboard: (text: string) => {
            clipboard = text;
          },
          failCopy: () => {
            copyError = true;
          },
          setAgent,
          setProjectFile: (
            path: string,
            content: string | null,
            projectPath = project.path,
          ) => {
            projectFiles.set(fileKey(projectPath, path), content);
            sessionStorage.setItem(
              "project-files",
              JSON.stringify([...projectFiles]),
            );
          },
          setGitVersions: (
            path: string,
            original: string,
            modified: string,
          ) => {
            gitVersions.set(path, { original, modified });
          },
          failAgent: () => {
            agentError = true;
          },
          setCodexUsage: (id: string, usage: unknown) => {
            codexUsage.set(id, usage);
            sessionStorage.setItem(
              "codex-context",
              JSON.stringify([...codexUsage]),
            );
          },
          setModels: (provider: string, models: any[] | null) => {
            if (models) choiceModels.set(provider, models);
            else choiceModels.delete(provider);
          },
          setChoicesError: (provider: string, error: string | null) => {
            if (error) choicesErrors.set(provider, error);
            else choicesErrors.delete(provider);
          },
          hold: (command: string) => {
            let release = () => {};
            const promise = new Promise<void>((resolve) => {
              release = resolve;
            });
            gates.set(command, { promise, release });
          },
          release: (command: string) => {
            const gate = gates.get(command);
            gates.delete(command);
            gate?.release();
          },
          failArchive: () => {
            failNextArchive = true;
          },
          setSessionDetails: (provider: string, id: string, value: unknown) => {
            sessionDetails.set(`${provider}:${id}`, value);
          },
          addSession: (id: string, name: string) => {
            threads.push({ id, name, preview: "", updatedAt: 1 });
          },
          setGitFiles: (files: GitChange[], path = project.path) => {
            gitProjects.set(path, files);
          },
          setGitMetadata: (
            value: { branch: string | null; repositoryRoot: string | null },
            path = project.path,
          ) => {
            gitMetadata.set(path, value);
          },
          setGitError: (error: string | null, path = project.path) => {
            if (error) gitErrors.set(path, error);
            else gitErrors.delete(path);
          },
          setPullRequest: (
            branch: string,
            value: unknown,
            path = project.path,
          ) => {
            pullRequests.set(fileKey(path, branch), value);
          },
          setPullRequestDetails: (url: string, value: unknown) => {
            pullRequests.set(url, value);
          },
          setGitDiff: (
            file: string,
            diff: Record<string, string>,
            path = project.path,
          ) => {
            gitDiffs.set(`${path}:${file}`, diff);
          },
          setDiffVersion: (version: number) => {
            diffVersion = version;
          },
          failSend: () => {
            failNextSend = true;
          },
          setCommandError: (command: string, error: string | null) => {
            if (error) commandErrors.set(command, error);
            else commandErrors.delete(command);
          },
          setCommands: (
            path: string,
            commands: { name: string; description?: string }[],
          ) => {
            slashCommands.set(path, commands);
          },
          setUsageLimits: (value: unknown) => {
            usageLimits = value;
          },
          setCommandResult: (value: typeof commandResult) => {
            commandResult = value;
          },
          holdHistory: () => {
            historyGate = new Promise<void>((resolve) => {
              release = resolve;
            });
          },
          releaseHistory: () => {
            release();
            historyGate = null;
          },
          setOpenHistory: (data: unknown[]) => {
            openHistory = data;
          },
          setCodexHistory: (
            data: unknown[],
            isWorking: boolean,
            id = "thread-a",
          ) => {
            codexHistory.set(id, { data, isWorking });
          },
          setOpenWorking: (working: boolean) => {
            openWorking = working;
          },
          holdStart: () => {
            startGate = new Promise<void>((resolve) => {
              releaseStart = resolve;
            });
          },
          releaseStart: () => {
            releaseStart();
            startGate = null;
          },
        },
        __TAURI_EVENT_PLUGIN_INTERNALS__: {
          unregisterListener: (_event: string, id: number) =>
            callbacks.delete(id),
        },
        __TAURI_INTERNALS__: {
          metadata: { currentWindow: { label: "main" } },
          transformCallback: (callback: (event: unknown) => void) => {
            const id = ++sequence;
            callbacks.set(id, callback);
            return id;
          },
          invoke: async (command: string, args: Record<string, any> = {}) => {
            calls.push({ command, args });
            if (command === "plugin:clipboard-manager|read_text") {
              await gates.get(command)?.promise;
              return clipboard;
            }
            if (command === "plugin:clipboard-manager|write_text") {
              if (copyError) {
                copyError = false;
                throw new Error("Clipboard unavailable");
              }
              clipboard = args.text;
              return;
            }
            if (command === "plugin:event|listen") {
              listeners.set(args.event, [
                ...(listeners.get(args.event) ?? []),
                args.handler,
              ]);
              return args.handler;
            }
            if (command === "plugin:event|unlisten") return;
            if (
              [
                "plugin:window|set_theme",
                "plugin:window|set_background_color",
              ].includes(command)
            )
              return;
            if (command === "git_worktrees") {
              if (!worktrees || args.projectPath === otherProject.path)
                return null;
              if (
                checkouts.find(
                  (entry) => entry.project.path === args.projectPath,
                )?.available === false
              )
                throw new Error("Project folder unavailable");
              const error = commandErrors.get(command);
              const snapshot = {
                project,
                storageDirectory: `${args.baseDirectory || "/home/oscar/.caelum/worktrees"}/caelum-a1b2`,
                branches: ["main", "release"],
                defaultBase: "main",
                worktrees: structuredClone(checkouts),
              };
              await gates.get(command)?.promise;
              if (error) throw new Error(error);
              return snapshot;
            }
            if (command === "worktree_destination") {
              if (!args.branch || /\s|\.\./.test(args.branch))
                throw new Error("Enter a valid new Git branch name.");
              return `${args.baseDirectory || "/home/oscar/.caelum/worktrees"}/caelum-a1b2/${args.branch.replaceAll("/", "-")}`;
            }
            if (
              ["create_worktree", "open_worktree", "remove_worktree"].includes(
                command,
              )
            ) {
              const error = commandErrors.get(command);
              await gates.get(command)?.promise;
              if (error) throw new Error(error);
              if (command === "create_worktree") {
                const path = `${args.baseDirectory || "/home/oscar/.caelum/worktrees"}/caelum-a1b2/${args.branch.replaceAll("/", "-")}`;
                if (
                  checkouts.some(
                    (entry) =>
                      entry.path === path || entry.branch === args.branch,
                  )
                )
                  throw new Error("Branch or destination already exists");
                const entry = addWorktree(path, args.branch);
                rememberCheckout(path);
                return entry;
              }
              const entry = checkouts.find(
                (entry) => entry.path === args.worktreePath,
              );
              if (!entry) throw new Error("Worktree unavailable");
              if (command === "open_worktree") {
                rememberCheckout(project.path);
                rememberCheckout(entry.project.path);
                return entry.project;
              }
              checkouts = checkouts.filter(
                (entry) => entry.path !== args.worktreePath,
              );
              savedProjects.delete(entry.project.path);
              if (activeWorktree === entry.project.path)
                activeWorktree = project.path;
              saveWorktrees();
              return;
            }
            if (command === "restore_workspace" && worktrees)
              return {
                projects: [...savedProjects.values()],
                activeProjectPath: activeWorktree,
              };
            if (command === "restore_workspace")
              return {
                projects: [
                  {
                    project,
                    provider: "codex",
                    codexSessionId:
                      selected &&
                      !archived.has(sessionKey(project.path, "codex", selected))
                        ? selected
                        : null,
                    opencodeSessionId: null,
                    codexSessionIds: threads
                      .filter(
                        (item) =>
                          !archived.has(
                            sessionKey(project.path, "codex", item.id),
                          ),
                      )
                      .map((item) => item.id),
                    opencodeSessionIds: [],
                  },
                ],
                activeProjectPath: project.path,
              };
            if (command === "startup_project") return null;
            if (command === "open_project" && worktrees) {
              if (
                checkouts.find((entry) => entry.project.path === args.path)
                  ?.available === false
              )
                throw new Error("Project folder unavailable");
              return {
                ...checkoutProject(args.path),
                name: args.path.split("/").at(-1),
              };
            }
            if (command === "open_project")
              return args.path === otherProject.path ? otherProject : project;
            if (command === "plugin:dialog|open") return otherProject.path;
            if (command === "session_details") {
              const key = `${args.provider}:${args.sessionId}`;
              const details = sessionDetails.has(key)
                ? sessionDetails.get(key)
                : {
                    branch:
                      args.projectPath === otherProject.path
                        ? "other-project"
                        : args.provider === "opencode"
                          ? "main"
                          : args.sessionId === "thread-a"
                            ? "feat/session-ui-redesign"
                            : "fix/provider-adapters",
                    branchIsCurrent: args.provider === "opencode",
                    changes: {
                      additions: args.sessionId === "thread-a" ? 42 : 7,
                      deletions: 3,
                    },
                  };
              await gates.get(command)?.promise;
              if (details === null) throw new Error("Metadata unavailable");
              return details;
            }
            if (command === "set_session_archived") {
              await gates.get(command)?.promise;
              if (failNextArchive) {
                failNextArchive = false;
                throw new Error("Archive unavailable");
              }
              const key = sessionKey(
                args.projectPath,
                args.provider,
                args.sessionId,
              );
              if (args.archived) archived.add(key);
              else archived.delete(key);
              sessionStorage.setItem("archived", JSON.stringify([...archived]));
              return;
            }
            if (command === "pending_approvals") return [];
            if (command === "pending_questions") {
              const snapshot = questions;
              await gates.get(command)?.promise;
              return snapshot;
            }
            if (command === "refresh_questions") {
              emit("question-requests", questions);
              return;
            }
            if (command === "resolve_question") {
              const error = commandErrors.get(command);
              await gates.get(command)?.promise;
              if (error) throw new Error(error);
              questions = questions.filter(
                (request: QuestionPrompt) =>
                  !["provider", "projectPath", "sessionId", "requestId"].every(
                    (key) => request[key as keyof QuestionPrompt] === args[key],
                  ),
              );
              sessionStorage.setItem("questions", JSON.stringify(questions));
              emit("question-requests", questions);
              return;
            }
            if (command === "plugin:opener|open_url") return;
            if (command === "plugin:opener|reveal_item_in_dir") return;
            if (command === "provider_choices") {
              const error = choicesErrors.get(args.provider);
              await gates.get(`${command}:${args.provider}`)?.promise;
              if (error) throw new Error(error);
              return {
                models:
                  choiceModels.get(args.provider) ??
                  [
                    {
                      id: "provider/model",
                      name: "Test model",
                      efforts: ["low", "medium", "high"].map((id) => ({
                        id,
                        name: id,
                      })),
                      defaultEffort:
                        args.provider === "codex" ? "medium" : null,
                    },
                    {
                      id: "provider/fast",
                      name: "Fast model",
                      efforts: [{ id: "low", name: "low" }],
                      defaultEffort: args.provider === "codex" ? "low" : null,
                    },
                    {
                      id: "provider/plain",
                      name: "Simple model",
                      efforts: [],
                    },
                  ].map((model) => ({
                    ...model,
                    ...(args.provider === "opencode"
                      ? {
                          provider: { id: "provider", name: "Provider One" },
                          contextWindow:
                            model.id === "provider/fast" ? 64000 : 200000,
                        }
                      : {}),
                  })),
                defaultModel: "provider/model",
                agents: args.provider === "opencode" ? choiceAgents : [],
              };
            }
            if (
              [
                "opencode_commands",
                "opencode_usage_limits",
                "opencode_run_command",
              ].includes(command)
            ) {
              const error = commandErrors.get(command);
              const commands = slashCommands.get(args.projectPath) ?? [
                { name: "init", description: "Set up project instructions" },
                { name: "review", description: "Review changes" },
                {
                  name: "goal",
                  description:
                    "Work until an objective is met [pause|resume|clear]",
                },
                { name: "team/review", description: "Custom team review" },
              ];
              const limits = usageLimits;
              await gates.get(command)?.promise;
              if (error) throw new Error(error);
              if (command === "opencode_commands") return commands;
              if (command === "opencode_usage_limits") return limits;
              if (args.model) models.set(args.sessionId, args.model);
              if (args.effort != null) efforts.set(args.sessionId, args.effort);
              if (commandResult) {
                openWorking = commandResult.working;
                openHistory = [
                  ...openHistory,
                  {
                    id: `msg-command-${sequence++}`,
                    type: "user",
                    text: commandResult.text,
                  },
                ];
                if (commandResult.agent)
                  setAgent(
                    args.projectPath,
                    args.sessionId,
                    commandResult.agent,
                  );
                if (commandResult.model)
                  models.set(args.sessionId, commandResult.model);
              }
              return;
            }
            if (command === "codex_list_sessions") {
              const snapshot = {
                project,
                threads: (worktrees
                  ? (
                      savedProjects.get(args.projectPath)?.codexSessionIds ?? []
                    ).map(
                      (id: string) =>
                        threads.find((entry) => entry.id === id) ?? {
                          id,
                          name: "Worktree session",
                          preview: "",
                          updatedAt: 124,
                        },
                    )
                  : threads
                ).filter(
                  (item) =>
                    !archived.has(
                      sessionKey(args.projectPath, "codex", item.id),
                    ),
                ),
              };
              await gates.get(command)?.promise;
              return snapshot;
            }
            if (command === "opencode_list_sessions") {
              const snapshot = {
                project,
                sessions: (worktrees
                  ? (
                      savedProjects.get(args.projectPath)?.opencodeSessionIds ??
                      []
                    ).map((id: string) => ({
                      id,
                      title: "OpenCode session",
                      updatedAt: 124,
                    }))
                  : [{ id: "ses-a", title: "OpenCode session", updatedAt: 123 }]
                ).filter(
                  (item) =>
                    !archived.has(
                      sessionKey(args.projectPath, "opencode", item.id),
                    ),
                ),
              };
              await gates.get(command)?.promise;
              return snapshot;
            }
            if (command === "codex_resume_session") {
              if (codexUsage.has(args.threadId))
                emit("codex-event", {
                  projectPath: args.projectPath,
                  method: "thread/tokenUsage/updated",
                  params: {
                    threadId: args.threadId,
                    turnId: "restored",
                    tokenUsage: codexUsage.get(args.threadId),
                  },
                });
              return {
                project,
                model: models.get(args.threadId),
                effort: efforts.get(args.threadId),
                thread: threads.find((item) => item.id === args.threadId) ?? {
                  id: args.threadId,
                  name: "Fresh session",
                },
              };
            }
            if (command === "opencode_resume_session") {
              const snapshot = {
                project,
                session: {
                  id: args.sessionId,
                  title: "OpenCode session",
                  isWorking: openWorking,
                  model: models.get(args.sessionId),
                  effort: efforts.get(args.sessionId),
                  agent: agentFor(args.projectPath, args.sessionId),
                },
              };
              await gates.get(command)?.promise;
              return snapshot;
            }
            if (command === "codex_session_history") {
              const snapshot = codexHistory.get(args.threadId);
              if (historyGate) await historyGate;
              if (snapshot && !args.cursor)
                return { nextCursor: null, ...snapshot };
              if (args.cursor)
                return {
                  nextCursor: null,
                  data: [
                    {
                      id: "older",
                      status: "completed",
                      items: Array.from({ length: 120 }, (_, index) => ({
                        id: `older-${index}`,
                        type: "agentMessage",
                        text: `Earlier message ${index}`,
                      })),
                    },
                  ],
                };
              return {
                nextCursor: "older",
                data: [
                  {
                    id: "turn-1",
                    status: "completed",
                    items: [
                      {
                        id: `${args.threadId}-user`,
                        type: "userMessage",
                        content: [
                          {
                            type: "text",
                            text: "Make the workspace darker and easier to use.",
                          },
                        ],
                      },
                      {
                        id: `${args.threadId}-tool`,
                        type: "commandExecution",
                        command: "pnpm test",
                        status: "completed",
                        aggregatedOutput: "14 tests passed",
                        durationMs: 420,
                      },
                      {
                        id: `${args.threadId}-answer`,
                        type: "agentMessage",
                        text: "## A focused workspace\n\nThe session now has **more room**. See the [documentation](https://example.com/docs).\n\n- A compact sidebar\n- Structured tool output\n\n```ts\nconst theme = 'near-black';\n```\n\n| Surface | Color |\n| --- | --- |\n| Background | #0B0C0E |",
                      },
                    ],
                  },
                ],
              };
            }
            if (command === "opencode_session_history") {
              const snapshot = {
                nextCursor: null,
                data: openHistory,
                isWorking: openWorking,
                model: models.get(args.sessionId),
                effort: efforts.get(args.sessionId),
                agent: agentFor(args.projectPath, args.sessionId),
              };
              if (historyGate) await historyGate;
              return snapshot;
            }
            if (command.endsWith("_start_session")) {
              if (startGate) await startGate;
              const id =
                (command.startsWith("codex") ? "thread-new" : "ses-new") +
                (worktrees ? `-${sequence++}` : "");
              const model = args.model || "provider/model";
              models.set(id, model);
              if (command.startsWith("opencode"))
                setAgent(args.projectPath, id, args.agent || "build");
              return command.startsWith("codex")
                ? {
                    project: worktrees
                      ? checkoutProject(args.projectPath)
                      : project,
                    model,
                    thread: { id, name: null, preview: "" },
                  }
                : {
                    project: worktrees
                      ? checkoutProject(args.projectPath)
                      : project,
                    session: {
                      id,
                      title: null,
                      model,
                      agent: agentFor(args.projectPath, id),
                    },
                  };
            }
            if (command === "opencode_select_agent") {
              await gates.get(command)?.promise;
              if (agentError) {
                agentError = false;
                throw new Error("Agent switch failed");
              }
              setAgent(args.projectPath, args.sessionId, args.agent);
              emit("opencode-event", {
                projectPath: args.projectPath,
                eventType: "session.agent.selected",
                data: { sessionID: args.sessionId, agent: args.agent },
              });
              return;
            }
            if (command.endsWith("_send_prompt")) {
              await gates.get(command)?.promise;
              if (failNextSend) {
                failNextSend = false;
                throw new Error("Connection refused");
              }
              const id = args.threadId ?? args.sessionId;
              if (args.model) models.set(id, args.model);
              if (args.effort != null) efforts.set(id, args.effort);
              if (command.startsWith("codex"))
                emit("codex-event", {
                  projectPath: worktrees ? args.projectPath : project.path,
                  method: "turn/started",
                  params: {
                    threadId: args.threadId,
                    turn: { id: "turn-new" },
                  },
                });
              return;
            }
            if (command.endsWith("_cancel_turn")) {
              if (command.startsWith("opencode")) {
                openWorking = false;
                emit("opencode-event", {
                  projectPath: worktrees ? args.projectPath : project.path,
                  eventType: "session.execution.interrupted",
                  data: { sessionID: args.sessionId, reason: "abort" },
                });
              } else
                emit("codex-event", {
                  projectPath: worktrees ? args.projectPath : project.path,
                  method: "turn/completed",
                  params: {
                    threadId: args.threadId,
                    turn: { id: "turn-new", status: "interrupted" },
                  },
                });
              return;
            }
            if (command === "resolve_approval") {
              emit("approval-requests", []);
              return;
            }
            if (command === "github_open_pull_request") {
              const result =
                pullRequests.get(fileKey(args.projectPath, args.branch)) ??
                null;
              const error = commandErrors.get(command);
              await (
                gates.get(`${command}:${args.projectPath}:${args.branch}`) ??
                gates.get(command)
              )?.promise;
              if (error) throw new Error(error);
              return result;
            }
            if (command === "github_pull_request_details") {
              const result = pullRequests.get(args.url);
              const error = commandErrors.get(command);
              await (gates.get(`${command}:${args.url}`) ?? gates.get(command))
                ?.promise;
              if (error) throw new Error(error);
              if (!result)
                throw new Error("No pull request details in fixture");
              return result;
            }
            if (command === "github_merge_pull_request") {
              const error = commandErrors.get(command);
              await (gates.get(`${command}:${args.url}`) ?? gates.get(command))
                ?.promise;
              if (error) throw new Error(error);
              const pr = pullRequests.get(args.url) as any;
              if (!pr || pr.headRefOid !== args.headOid)
                throw new Error("Head branch was modified (HTTP 409)");
              pullRequests.set(args.url, { ...pr, state: "MERGED" });
              for (const [key, value] of pullRequests) {
                if (key !== args.url && (value as any)?.url === args.url)
                  pullRequests.delete(key);
              }
              return;
            }
            if (command === "git_repository_status") {
              const error = gitErrors.get(args.projectPath);
              const snapshot = {
                projectPath: args.projectPath,
                repositoryRoot: args.projectPath,
                branch: "main",
                changes: gitProjects.get(args.projectPath) ?? [],
                ...gitMetadata.get(args.projectPath),
              };
              await (
                gates.get(`${command}:${args.projectPath}`) ??
                gates.get(command)
              )?.promise;
              if (error) throw new Error(error);
              return snapshot;
            }
            if (command === "list_project_directory") {
              const entries = new Map<string, any>();
              const prefix = `${args.projectPath}\0${args.path ? `${args.path}/` : ""}`;
              for (const [key, content] of projectFiles) {
                if (content === null || !key.startsWith(prefix)) continue;
                const rest = key.slice(prefix.length);
                const name = rest.split("/")[0];
                const path = args.path ? `${args.path}/${name}` : name;
                entries.set(path, {
                  name,
                  path,
                  kind: rest.includes("/") ? "directory" : "file",
                  sizeBytes: rest.includes("/") ? null : content.length,
                });
              }
              const sorted = [...entries.values()].sort(
                (a, b) =>
                  (a.kind === "directory" ? 0 : 1) -
                    (b.kind === "directory" ? 0 : 1) ||
                  a.name.localeCompare(b.name),
              );
              const end = args.offset + args.limit;
              return {
                path: args.path,
                entries: sorted.slice(args.offset, end),
                total: entries.size,
                nextOffset: end < entries.size ? end : null,
              };
            }
            if (command === "search_project_files") {
              const matches: any[] = [];
              for (const [key, content] of projectFiles) {
                if (
                  content === null ||
                  !key.startsWith(`${args.projectPath}\0`)
                )
                  continue;
                const path = key.slice(args.projectPath.length + 1);
                if (!args.includeIgnored && path.startsWith("vendor/"))
                  continue;
                if (args.mode === "files") {
                  if (path.toLowerCase().includes(args.query.toLowerCase()))
                    matches.push({
                      path,
                      line: null,
                      column: null,
                      preview: null,
                    });
                } else if (args.query.trim()) {
                  content.split("\n").forEach((line, index) => {
                    const column = line
                      .toLowerCase()
                      .indexOf(args.query.toLowerCase());
                    if (column >= 0)
                      matches.push({
                        path,
                        line: index + 1,
                        column: column + 1,
                        preview: line,
                      });
                  });
                }
              }
              await (
                gates.get(`${command}:${args.query}`) ?? gates.get(command)
              )?.promise;
              return { matches, truncated: false, skipped: 0 };
            }
            if (command === "read_project_file") {
              const content = projectFiles.get(
                fileKey(args.projectPath, args.path),
              );
              if (content == null)
                throw new Error(`File not found: ${args.path}`);
              await (gates.get(`${command}:${args.path}`) ?? gates.get(command))
                ?.promise;
              return {
                path: args.path,
                content,
                sizeBytes: content.length,
                truncated: false,
              };
            }
            if (command === "git_file_versions")
              return {
                path: args.filePath,
                original: "export const ready = false;\n",
                modified: "export const ready = true;\n",
                truncated: false,
                ...gitVersions.get(args.filePath),
              };
            if (command === "git_file_diff") {
              const snapshot = {
                path: args.filePath,
                staged: args.filePath.endsWith(".css")
                  ? `diff --git a/${args.filePath} b/${args.filePath}\n@@ -1 +1 @@\n-background: white;\n+background: #0b0c0e; /* revision ${diffVersion} */`
                  : "",
                unstaged: args.filePath.endsWith(".tsx")
                  ? `diff --git a/${args.filePath} b/${args.filePath}\n@@ -1 +1 @@\n-const layout = 'page';\n+const layout = 'split'; // revision ${diffVersion}`
                  : "",
                untracked: "",
                ...gitDiffs.get(`${args.projectPath}:${args.filePath}`),
              };
              await gates.get(command)?.promise;
              return snapshot;
            }
            if (
              [
                "remember_project",
                "set_active_project",
                "set_project_provider",
                "select_project_session",
                "sync_provider_sessions",
                "forget_project_session",
                "close_project_record",
              ].includes(command)
            ) {
              if (worktrees) {
                if (command === "remember_project")
                  rememberCheckout(args.project.path);
                if (command === "set_active_project") {
                  activeWorktree = args.projectPath;
                  const record = savedProjects.get(args.projectPath);
                  if (record)
                    savedProjects.get(
                      record.rootPath ?? args.projectPath,
                    ).selectedWorktreePath = args.projectPath;
                }
                const record = savedProjects.get(args.projectPath);
                if (record && command === "set_project_provider")
                  record.provider = args.provider;
                if (record && command === "select_project_session") {
                  record[`${args.provider}SessionId`] = args.sessionId;
                  if (
                    args.sessionId &&
                    !record[`${args.provider}SessionIds`].includes(
                      args.sessionId,
                    )
                  )
                    record[`${args.provider}SessionIds`].push(args.sessionId);
                }
                if (command === "close_project_record") {
                  for (const [path, entry] of savedProjects)
                    if (
                      path === args.projectPath ||
                      entry.rootPath === args.projectPath
                    )
                      savedProjects.delete(path);
                  activeWorktree = args.activeProjectPath;
                }
                saveWorktrees();
              }
              return;
            }
            throw new Error(`Unhandled fixture command: ${command}`);
          },
        },
      });
    },
    { selected, ...questionOptions },
  );
  await page.goto("/");
  await expect(
    page
      .getByRole("textbox", { name: "Message", exact: true })
      .or(page.getByRole("button", { name: "Cancel questions", exact: true }))
      .first(),
  ).toBeEnabled();
}

async function emit(
  page: Page,
  method: string,
  params: Record<string, unknown>,
  provider = "codex",
  metadata: { id?: string; created?: number } = {},
) {
  await page.evaluate(
    ({ method, params, provider, metadata }) =>
      (window as any).fixture.emit(
        `${provider}-event`,
        provider === "codex"
          ? { projectPath: "/work/caelum", method, params }
          : {
              projectPath: "/work/caelum",
              eventType: method,
              data: params,
              ...metadata,
            },
      ),
    { method, params, provider, metadata },
  );
}

async function choose(page: Page, label: string, option: string) {
  await page.getByRole("button", { name: label, exact: true }).click();
  await page
    .getByRole(
      label === "Model" || label === "Default model" ? "button" : "option",
      { name: option, exact: true },
    )
    .click();
}

async function setHistory(
  page: Page,
  provider: string,
  data: unknown[],
  isWorking: boolean,
) {
  await page.evaluate(
    ({ provider, data, isWorking }) => {
      const fixture = (window as any).fixture;
      if (provider === "codex") fixture.setCodexHistory(data, isWorking);
      else {
        fixture.setOpenHistory(data);
        fixture.setOpenWorking(isWorking);
      }
    },
    { provider, data, isWorking },
  );
}

function sessionRow(page: Page, title: string) {
  return page
    .locator(".session-row")
    .filter({ has: page.getByRole("button", { name: title, exact: true }) });
}

async function openSettings(page: Page, section = "Appearance") {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: section, exact: true }).click();
  return dialog;
}

async function findProjectFile(page: Page, path: string) {
  await page.keyboard.press("Meta+p");
  const search = page.getByRole("combobox", { name: "Find project file" });
  await expect(search).toBeFocused();
  await search.fill(path);
  await expect(
    page.getByRole("option", { name: path, exact: true }),
  ).toBeVisible();
  await search.press("Enter");
  await expect(page.locator(".file-editor-path")).toHaveText(path);
  await expect(page.locator(".code-viewer .cm-editor").first()).toBeVisible();
}

const attachedContext = (page: Page) =>
  page.locator('.composer [aria-label="Attached context"]');

async function createCheckout(page: Page, branch = "feature/search") {
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Create worktree");
  const dialog = page.getByRole("dialog", {
    name: "Create worktree",
    exact: true,
  });
  await dialog
    .getByRole("textbox", { name: "New branch", exact: true })
    .fill(branch);
  await dialog
    .getByRole("combobox", { name: "Start from", exact: true })
    .selectOption("release");
  await expect(dialog.locator(".worktree-destination")).toContainText(
    branch.replaceAll("/", "-"),
  );
  const destination = await dialog
    .locator(".worktree-destination")
    .textContent();
  await dialog
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("combobox", { name: "Session worktree" }),
  ).toHaveValue(destination!);
  return destination!;
}

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} worktrees group checkouts, isolate drafts and files, support multiple sessions and restore selection`, async ({
    page,
  }) => {
    await desktop(page, null, { worktrees: true });
    await expect(page.locator(".worktree-list")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Create worktree for caelum" }),
    ).toHaveCount(0);
    await expect(page.locator(".worktree-row")).toHaveCount(0);
    await expect(
      page.getByRole("combobox", { name: "Session worktree" }),
    ).toHaveCount(0);
    const message = page.getByRole("textbox", { name: "Message", exact: true });
    await message.fill("Original checkout draft");
    const settings = await openSettings(page, "Worktrees");
    await settings
      .getByRole("textbox", { name: "Worktree folder" })
      .fill("/custom/checkouts");
    await settings.getByRole("button", { name: "Done", exact: true }).click();
    const path = await createCheckout(page);
    expect(path).toBe("/custom/checkouts/caelum-a1b2/feature-search");
    await expect(page.locator(".worktree-row")).toHaveCount(1);
    await expect(
      page
        .getByRole("combobox", { name: "Session worktree" })
        .locator("option"),
    ).toHaveText(["feature/search"]);
    await expect(page.locator(".project-group")).toHaveCount(1);
    await expect(page.locator(".breadcrumbs")).toContainText("feature/search");
    await findProjectFile(page, "src/App.tsx");
    await expect(page.locator(".code-viewer")).toContainText(
      'checkout = "feature/search"',
    );
    await page.evaluate((path) => {
      const fixture = (window as any).fixture;
      fixture.setGitFiles(
        [{ path: "src/App.tsx", indexStatus: " ", worktreeStatus: "M" }],
        path,
      );
      fixture.setGitDiff(
        "src/App.tsx",
        { unstaged: "@@ -1 +1 @@\n-original checkout\n+worktree change" },
        path,
      );
    }, path);
    await page.getByRole("button", { name: "Changes", exact: true }).click();
    await expect(
      page.getByRole("region", { name: "Unstaged diff", exact: true }),
    ).toContainText("worktree change");
    await page
      .getByRole("button", {
        name: "caelum",
        exact: true,
      })
      .click();
    await expect(message).toHaveValue("Original checkout draft");
    await findProjectFile(page, "src/App.tsx");
    await expect(page.locator(".code-viewer")).toContainText("ready = true");
    await page
      .getByRole("combobox", { name: "Session worktree" })
      .selectOption(path);
    await expect(message).toHaveValue("");
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await message.fill("First worktree task");
    await message.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", {
        name: "Remove worktree feature/search",
        exact: true,
      }),
    ).toBeDisabled();
    await message.fill("First session draft");
    await page.getByRole("button", { name: "New session" }).click();
    await message.fill("Second worktree task");
    await message.press("Enter");
    await expect
      .poll(() =>
        page.evaluate(
          (provider) =>
            (window as any).fixture.calls.filter(
              (call: any) => call.command === `${provider}_start_session`,
            ).length,
          provider,
        ),
      )
      .toBe(2);
    const calls = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls.filter(
          (call: any) => call.command === `${provider}_send_prompt`,
        ),
      provider,
    );
    expect(calls).toHaveLength(2);
    expect(calls.map((call: any) => call.args.projectPath)).toEqual([
      path,
      path,
    ]);
    expect(calls[0].args.threadId ?? calls[0].args.sessionId).not.toBe(
      calls[1].args.threadId ?? calls[1].args.sessionId,
    );
    await page
      .getByRole("button", {
        name: "caelum",
        exact: true,
      })
      .click();
    await expect(message).toHaveValue("Original checkout draft");
    await page
      .getByRole("button", {
        name: "Switch to worktree feature/search",
        exact: true,
      })
      .click();
    await expect(page.locator(".provider-label")).toHaveText(
      provider === "codex" ? "Codex" : "OpenCode",
    );
    await page.reload();
    await expect(message).toBeEnabled();
    await expect(page.locator(".project-group")).toHaveCount(1);
    await expect(page.locator(".breadcrumbs > span").first()).toHaveText(
      "caelum",
    );
    await expect(
      page.getByRole("button", {
        name: "Switch to worktree feature/search",
        exact: true,
      }),
    ).toHaveAttribute("aria-current", "page");
    await expect(page.locator(".provider-label")).toHaveText(
      provider === "codex" ? "Codex" : "OpenCode",
    );
    const restored = await openSettings(page, "Worktrees");
    await expect(
      restored.getByRole("textbox", { name: "Worktree folder" }),
    ).toHaveValue("/custom/checkouts");
  });
}

test("worktrees discover external checkouts, reject removal failures and ignore stale inventory after removal", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const path = await createCheckout(page, "fix/layout");
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Keep this draft until removal succeeds");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.addWorktree("/external/locked", "locked", {
      locked: "another task",
    });
    fixture.addWorktree("/external/missing", null, {
      available: false,
      prunable: "directory missing",
    });
  });
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await expect(
    page.getByRole("button", { name: "Remove worktree locked", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree missing",
      exact: true,
    }),
  ).toBeDisabled();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "remove_worktree",
      "Git: contains modified or untracked files",
    ),
  );
  await page
    .getByRole("button", { name: "Remove worktree fix/layout", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Remove worktree",
    exact: true,
  });
  await expect(dialog).toContainText(path);
  await dialog
    .getByRole("button", { name: "Remove worktree", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText(
    "modified or untracked",
  );
  await expect(message).toHaveValue("Keep this draft until removal succeeds");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("remove_worktree", null);
    fixture.hold("git_worktrees");
  });
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await page
    .getByRole("button", { name: "Remove worktree fix/layout", exact: true })
    .click();
  await dialog
    .getByRole("button", { name: "Remove worktree", exact: true })
    .click();
  await page.evaluate(() => (window as any).fixture.release("git_worktrees"));
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree fix/layout",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await expect(message).toBeEnabled();
  await page.evaluate((path) => {
    const fixture = (window as any).fixture;
    fixture.emit("codex-event", {
      projectPath: path,
      method: "thread/name/updated",
      params: { threadId: "late", threadName: "Late result" },
    });
    fixture.addWorktree(path, "fix/layout");
  }, path);
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await page
    .getByRole("button", { name: "Switch to worktree fix/layout", exact: true })
    .click();
  await expect(message).toHaveValue("");
  await page
    .getByRole("button", { name: "Remove worktree fix/layout", exact: true })
    .click();
  await dialog
    .getByRole("button", { name: "Remove worktree", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  await page.reload();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree fix/layout",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
});

test("worktrees recover creation failures and delayed selection cannot steal another project", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "create_worktree",
      "Git: branch already exists",
    ),
  );
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Create worktree");
  const dialog = page.getByRole("dialog", {
    name: "Create worktree",
    exact: true,
  });
  const branch = await dialog
    .getByRole("textbox", { name: "New branch" })
    .inputValue();
  expect(branch).not.toBe("");
  await expect(
    dialog.getByRole("combobox", { name: "Start from" }),
  ).toHaveValue("main");
  await dialog
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText(
    "branch already exists",
  );
  await expect(dialog.getByRole("textbox", { name: "New branch" })).toHaveValue(
    branch,
  );
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("create_worktree", null),
  );
  await dialog
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === "create_worktree")
          .at(-1).args,
    ),
  ).toMatchObject({ branch, base: "main" });
  await page
    .getByRole("button", { name: "Create worktree for caelum" })
    .click();
  await expect(
    dialog.getByRole("textbox", { name: "New branch" }),
  ).not.toHaveValue(branch);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await page
    .getByRole("button", {
      name: "caelum",
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await page.evaluate(() => (window as any).fixture.hold("open_worktree"));
  await page
    .getByRole("button", {
      name: `Switch to worktree ${branch}`,
      exact: true,
    })
    .click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await page.evaluate(() => (window as any).fixture.release("open_worktree"));
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await page.evaluate(() => (window as any).fixture.hold("open_worktree"));
  await page
    .getByRole("button", {
      name: `Switch to worktree ${branch}`,
      exact: true,
    })
    .click();
  await page.getByRole("button", { name: "Close caelum", exact: true }).click();
  await expect(page.locator(".project-group")).toHaveCount(1);
  await page.evaluate(() => (window as any).fixture.release("open_worktree"));
  await expect
    .poll(() =>
      page.evaluate(() =>
        JSON.parse(sessionStorage.getItem("worktree-workspace")!).projects.map(
          (entry: any) => entry.project.path,
        ),
      ),
    )
    .toEqual(["/work/other"]);
  await page.reload();
  await expect(
    page.getByRole("button", {
      name: `Switch to worktree ${branch}`,
      exact: true,
    }),
  ).toHaveCount(0);
});

test("worktrees keep drafts when the active checkout is removed externally", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const path = await createCheckout(page);
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Retain the draft");
  await page.evaluate(
    (path) => (window as any).fixture.forgetWorktree(path),
    path,
  );
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await expect(message).toBeDisabled();
  await expect(
    page.getByText(
      "This worktree is unavailable. Choose another checkout from the sidebar.",
    ),
  ).toBeVisible();
  await page
    .getByRole("button", {
      name: "caelum",
      exact: true,
    })
    .click();
  await expect(message).toBeEnabled();
  await page.evaluate(
    (path) => (window as any).fixture.addWorktree(path, "feature/search"),
    path,
  );
  await expect(page.locator(".worktree-list")).toHaveCount(0);
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Refresh worktrees");
  await page
    .getByRole("button", {
      name: "Switch to worktree feature/search",
      exact: true,
    })
    .click();
  await expect(message).toHaveValue("Retain the draft");
});

test("worktrees restore their project group when the original project directory is missing", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  await createCheckout(page);
  await page.evaluate(() =>
    (window as any).fixture.addWorktree("/work/caelum", "main", {
      main: true,
      available: false,
    }),
  );
  await page.reload();
  await expect(page.locator(".project-group")).toHaveCount(1);
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree Original checkout",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree feature/search",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await expect(page.locator(".breadcrumbs > span").first()).toHaveText(
    "caelum",
  );
  await createCheckout(page, "feature/second");
  await page.evaluate(() =>
    (window as any).fixture.addWorktree("/external/checkout", "external"),
  );
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree external",
      exact: true,
    }),
  ).toBeEnabled();
  for (const branch of ["feature/second", "feature/search"]) {
    await page
      .getByRole("button", { name: `Remove worktree ${branch}`, exact: true })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Remove worktree",
      exact: true,
    });
    await dialog
      .getByRole("button", { name: "Remove worktree", exact: true })
      .click();
    await expect(dialog).toHaveCount(0);
  }
  await page
    .getByRole("button", { name: "Switch to worktree external", exact: true })
    .click();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree external",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
});

const question = (overrides: Partial<QuestionPrompt> = {}): QuestionPrompt => ({
  provider: "codex",
  projectPath: "/work/caelum",
  sessionId: "thread-a",
  requestId: "41",
  title: "Implementation choices",
  fields: [
    {
      key: "approach",
      type: "string",
      title: "Approach",
      description: "How should we proceed?",
      required: true,
      custom: true,
      options: [
        { value: "Small", label: "Small", description: "Focused change" },
        { value: "Large", label: "Large", description: "Full rewrite" },
      ],
    },
    { key: "detail", type: "string", title: "Details", required: true },
  ],
  ...overrides,
});

async function setQuestions(
  page: Page,
  requests: QuestionPrompt[],
  notify = true,
) {
  await page.evaluate(
    ({ requests, notify }) =>
      (window as any).fixture.setQuestions(requests, notify),
    { requests, notify },
  );
}

test("Codex questions preserve drafts across sessions and retry the original request", async ({
  page,
}) => {
  await desktop(page);
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Next message draft");
  await setQuestions(page, [question()]);
  const card = page.getByRole("form", { name: "Implementation choices" });
  await expect(card).toBeVisible();
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveCount(0);
  await expect(card).toBeFocused();
  await expect(card.locator("input:checked")).toHaveCount(0);
  await page.keyboard.press("Tab");
  await expect(
    card.getByRole("radio", { name: "Small Focused change" }),
  ).toBeFocused();
  await expect(page.locator(".connection-status")).toHaveText(
    "Waiting for your answer",
  );
  await card.getByRole("button", { name: "Submit answers" }).click();
  await expect(
    card.getByRole("radio", { name: "Small Focused change" }),
  ).toBeFocused();
  await page.keyboard.press("Space");
  await page.keyboard.press("ArrowDown");
  await expect(
    card.getByRole("radio", { name: "Large Full rewrite" }),
  ).toBeChecked();
  const customAnswer = card.getByRole("textbox", { name: "Approach" });
  await customAnswer.click();
  await expect(
    card.getByRole("radio", { name: "Type your own answer" }),
  ).toBeChecked();
  await customAnswer.pressSequentially("Incremental refactor");
  const singleLineHeight = (await customAnswer.boundingBox())!.height;
  await customAnswer.press("Shift+Enter");
  await customAnswer.pressSequentially("Preserve the existing API.");
  await expect(customAnswer).toHaveValue(
    "Incremental refactor\nPreserve the existing API.",
  );
  expect((await customAnswer.boundingBox())!.height).toBeGreaterThan(
    singleLineHeight,
  );
  await customAnswer.fill("Incremental refactor");
  expect((await customAnswer.boundingBox())!.height).toBe(singleLineHeight);
  await card.getByRole("radio", { name: "Small Focused change" }).check();
  await expect(customAnswer).toHaveValue("Incremental refactor");
  await card.getByRole("radio", { name: "Type your own answer" }).check();
  await expect(customAnswer).toBeFocused();
  await card
    .getByRole("textbox", { name: "Details" })
    .fill("Keep compatibility");
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await expect(card).toHaveCount(0);
  await expect(message).toBeVisible();
  await page.getByRole("button", { name: "1 pending questions" }).click();
  await expect(message).toHaveCount(0);
  await expect(card.getByRole("textbox", { name: "Details" })).toHaveValue(
    "Keep compatibility",
  );
  await expect(card.getByRole("textbox", { name: "Approach" })).toHaveValue(
    "Incremental refactor",
  );
  await customAnswer.focus();
  await customAnswer.dispatchEvent("keydown", {
    key: "Enter",
    isComposing: true,
  });
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "resolve_question",
      ),
    ),
  ).toHaveLength(0);
  await page.evaluate(() => {
    (window as any).fixture.setCommandError(
      "resolve_question",
      "Connection unavailable",
    );
    (window as any).fixture.hold("resolve_question");
  });
  await customAnswer.press("Enter");
  await expect(card.getByRole("button", { name: "Sending…" })).toBeDisabled();
  await expect(card.getByRole("textbox", { name: "Details" })).toBeDisabled();
  await page.evaluate(() =>
    (window as any).fixture.release("resolve_question"),
  );
  await expect(card.getByRole("alert")).toContainText("Connection unavailable");
  await expect(message).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("resolve_question", null),
  );
  await card.getByRole("textbox", { name: "Details" }).press("Enter");
  await expect(card).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toHaveValue("Next message draft");
  await expect(message).toBeFocused();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "resolve_question",
    ),
  );
  expect(calls).toHaveLength(2);
  expect(calls[1].args).toEqual({
    provider: "codex",
    projectPath: "/work/caelum",
    sessionId: "thread-a",
    requestId: "41",
    answer: { approach: "Incremental refactor", detail: "Keep compatibility" },
  });
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "codex_send_prompt",
      ),
    ),
  ).toHaveLength(0);
  await setQuestions(page, [question()]);
  await card.getByRole("button", { name: "Cancel questions" }).press("Enter");
  await expect(message).toHaveValue("Next message draft");
  await expect(message).toBeFocused();
});

test("OpenCode questions submit multiple and custom answers with typed conditional fields", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await setQuestions(page, [
    question({
      provider: "opencode",
      sessionId: "ses-a",
      requestId: "frm_1",
      fields: [
        {
          key: "q0",
          type: "multiselect",
          title: "Targets",
          required: true,
          custom: true,
          options: [
            { value: "Mac", label: "macOS" },
            { value: "Linux", label: "Linux" },
          ],
        },
        {
          key: "enabled",
          type: "boolean",
          title: "Enable rollout",
          required: true,
          default: false,
        },
        {
          key: "count",
          type: "integer",
          title: "Batch size",
          required: true,
          minimum: 1,
          default: 2,
          when: [{ key: "enabled", op: "eq", value: true }],
        },
        { key: "metadata", type: "string", hidden: true, default: "preserved" },
      ],
    }),
  ]);
  const card = page.getByRole("form", { name: "Implementation choices" });
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveCount(0);
  await card.getByRole("checkbox", { name: "macOS" }).check();
  await card.getByRole("checkbox", { name: "Linux" }).focus();
  await page.keyboard.press("Space");
  const customAnswers = card.getByRole("textbox", {
    name: "Targets: custom answers",
  });
  await customAnswers.fill("Windows");
  await customAnswers.press("Shift+Enter");
  await customAnswers.pressSequentially("FreeBSD");
  await expect(
    card.getByRole("spinbutton", { name: "Batch size" }),
  ).toHaveCount(0);
  await card.getByRole("radio", { name: "Yes", exact: true }).check();
  await card.getByRole("spinbutton", { name: "Batch size" }).fill("3");
  await customAnswers.press("Enter");
  await expect(card).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === "resolve_question")
          .at(-1).args,
    ),
  ).toEqual({
    provider: "opencode",
    projectPath: "/work/caelum",
    sessionId: "ses-a",
    requestId: "frm_1",
    answer: {
      q0: ["Mac", "Linux", "Windows", "FreeBSD"],
      enabled: true,
      count: 3,
      metadata: "preserved",
    },
  });
});

test("pending questions recover on reconnect and stay scoped across projects and providers", async ({
  page,
}) => {
  const first = question();
  const second = question({ provider: "opencode", sessionId: "ses-a" });
  const third = question({ projectPath: "/work/other", sessionId: "thread-a" });
  await desktop(page, "thread-a", { initialQuestions: [first, second, third] });
  const card = page.getByRole("form", { name: "Implementation choices" });
  await expect(card).toHaveCount(1);
  await card.getByRole("button", { name: "Cancel questions" }).click();
  await expect(card).toHaveCount(0);
  await page.getByRole("button", { name: "2 pending questions" }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("OpenCode session");
  await expect(card).toHaveCount(1);
  await setQuestions(page, [third]); // answered by another client
  await expect(card).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  await emit(page, "server.disconnected", {}, "opencode");
  await setQuestions(page, [second, third], false); // form predates the new connection
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(card).toHaveCount(1);
  await page.getByRole("button", { name: "2 pending questions" }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await card.getByRole("button", { name: "Cancel questions" }).click();
  const replies = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "resolve_question",
    ),
  );
  expect(replies.map((call: any) => call.args)).toEqual([
    {
      provider: "codex",
      projectPath: "/work/caelum",
      sessionId: "thread-a",
      requestId: "41",
      answer: null,
    },
    {
      provider: "codex",
      projectPath: "/work/other",
      sessionId: "thread-a",
      requestId: "41",
      answer: null,
    },
  ]);
});

test("a stale pending-question snapshot cannot resurrect an externally cancelled question", async ({
  page,
}) => {
  await desktop(page, "thread-a", {
    initialQuestions: [question()],
    holdQuestions: true,
  });
  await setQuestions(page, []);
  await page.evaluate(() =>
    (window as any).fixture.release("pending_questions"),
  );
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(page.locator(".question-card")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "1 pending questions" }),
  ).toHaveCount(0);
});

test("Files searches filenames and contents beside the conversation, handles ignored files and rejects stale results", async ({
  page,
}) => {
  await desktop(page);
  const navigation = page.getByRole("navigation", {
    name: "Workspace",
    exact: true,
  });
  await expect(
    navigation.getByRole("button", { name: "Sessions", exact: true }),
  ).toHaveClass(/active/);
  await expect(
    navigation.getByRole("button", { name: "Changes", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.evaluate(() =>
    (window as any).fixture.setProjectFile(
      "src/App.tsx",
      "// header\nexport const ready = true;\n// footer\n",
    ),
  );
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft visible");
  await findProjectFile(page, "src/App.tsx");
  await expect(
    navigation.getByRole("button", { name: "Sessions", exact: true }),
  ).toHaveClass(/active/);
  await expect(
    navigation.getByRole("button", { name: "Sessions", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    navigation.getByRole("button", { name: "Files", exact: true }),
  ).toHaveClass(/active/);
  await expect(
    navigation.getByRole("button", { name: "Files", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    navigation.getByRole("button", { name: "Changes", exact: true }),
  ).toHaveAttribute("aria-pressed", "false");
  await expect(input).toBeVisible();
  await expect(input).toHaveValue("Keep this draft visible");
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
  await page.keyboard.press("Meta+Shift+f");
  const contents = page.getByRole("combobox", {
    name: "Search project contents",
  });
  await expect(contents).toBeFocused();
  await contents.fill("ready");
  await expect(
    page.getByRole("option", { name: /src\/App.tsx:2/ }),
  ).toBeVisible();
  await contents.press("Enter");
  await expect(page.locator(".cm-activeLineGutter")).toHaveText("2");
  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:src/styles.css"),
  );
  await page.keyboard.press("Meta+p");
  const filenames = page.getByRole("combobox", { name: "Find project file" });
  await filenames.fill("src/styles.css");
  await expect(
    page.getByRole("option", { name: "src/styles.css", exact: true }),
  ).toBeVisible();
  await filenames.press("Enter");
  await expect(
    page.getByText("Loading file preview…", { exact: true }),
  ).toBeVisible();
  const tabs = page.getByRole("tablist", { name: "Open files" });
  await expect(tabs.getByRole("tab")).toHaveCount(2);
  await tabs.getByRole("tab", { name: "styles.css", exact: true }).focus();
  await page.evaluate(() =>
    (window as any).fixture.release("read_project_file:src/styles.css"),
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          JSON.parse(
            localStorage.getItem("caelum.files.v1:%2Fwork%2Fcaelum") ?? "{}",
          ).views?.["file:src/styles.css"]?.main,
      ),
    )
    .toBeTruthy();
  await expect(
    tabs.getByRole("tab", { name: "styles.css", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await expect(
    tabs.getByRole("tab", { name: "App.tsx", exact: true }),
  ).toBeFocused();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");

  await page.keyboard.press("Meta+p");
  const search = page.getByRole("combobox", { name: "Find project file" });
  await search.fill("generated");
  await expect(
    page.getByText("No matching files.", { exact: true }),
  ).toBeVisible();
  await page.getByRole("checkbox", { name: "Include ignored" }).check();
  await expect(
    page.getByRole("option", { name: "vendor/generated.ts" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.hold("search_project_files:README"),
  );
  await search.fill("README");
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).fixture.calls.some(
          (call: any) =>
            call.command === "search_project_files" &&
            call.args.query === "README",
        ),
      ),
    )
    .toBe(true);
  await search.fill("styles");
  await expect(
    page.getByRole("option", { name: "src/styles.css" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.release("search_project_files:README"),
  );
  await expect(page.getByRole("option", { name: "README.md" })).toHaveCount(0);
  await search.press("Escape");
  await page.getByRole("button", { name: "Recent", exact: true }).click();
  await expect(
    page
      .locator(".changed-files-scroll")
      .getByRole("button", { name: "src/App.tsx", exact: true }),
  ).toBeVisible();
  await tabs
    .getByRole("button", { name: "Close src/styles.css", exact: true })
    .click();
  await expect(tabs.getByRole("tab")).toHaveCount(1);
});

test("Files restores tabs, expanded folders and reading positions, refreshes in place and keeps project workspaces separate", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await desktop(page);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setProjectFile(
      "src/App.tsx",
      Array.from(
        { length: 220 },
        (_, index) => `export const line${index + 1} = ${index + 1};`,
      ).join("\n"),
    );
    for (let index = 0; index < 260; index++)
      fixture.setProjectFile(`src/extra-${index}.ts`, "export {};\n");
  });
  await findProjectFile(page, "src/App.tsx");
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click();
  await page
    .getByRole("treeitem", { name: "Load more files in src", exact: true })
    .click();
  await expect(
    page.getByRole("treeitem", { name: /extra-.*file/ }),
  ).toHaveCount(260);
  const tree = page.locator(".file-tree-scroll");
  await tree.evaluate((node) => {
    node.scrollTop = 700;
  });
  const code = page.locator(".code-viewer .cm-editor").first();
  const editorInput = page.getByRole("textbox", {
    name: "Read-only preview of src/App.tsx",
    exact: true,
  });
  await editorInput.focus();
  await editorInput.press("PageDown");
  await editorInput.press("PageDown");
  const stored = () =>
    page.evaluate(() =>
      JSON.parse(
        localStorage.getItem("caelum.files.v1:%2Fwork%2Fcaelum") ?? "{}",
      ),
    );
  await expect
    .poll(
      async () => (await stored()).views?.["file:src/App.tsx"]?.main?.line ?? 0,
    )
    .toBeGreaterThan(20);
  const view = (await stored()).views["file:src/App.tsx"].main;
  const firstVisibleLine = () =>
    code.locator(".cm-line").evaluateAll((nodes) => {
      const bounds = nodes[0]?.closest(".cm-scroller")?.getBoundingClientRect();
      return nodes.find(
        (node) => node.getBoundingClientRect().bottom > (bounds?.top ?? 0),
      )?.textContent;
    });
  const line = await firstVisibleLine();
  await findProjectFile(page, "README.md");
  await page.getByRole("tab", { name: "App.tsx", exact: true }).click();
  await expect.poll(firstVisibleLine).toBe(line);
  const updated =
    Array.from(
      { length: 220 },
      (_, index) => `export const line${index + 1} = ${index + 1};`,
    ).join("\n") + `\n// refreshed ${"x".repeat(512)}\n`;
  await page.evaluate((updated) => {
    (window as any).fixture.setProjectFile("src/App.tsx", updated);
    window.dispatchEvent(new Event("focus"));
  }, updated);
  await expect(page.locator(".file-editor-size")).toHaveText(
    `${(updated.length / 1024).toFixed(1)} KiB`,
  );
  await expect(
    page.getByRole("treeitem", { name: /extra-.*file/ }),
  ).toHaveCount(260);
  await expect
    .poll(async () => (await stored()).views?.["file:src/App.tsx"]?.main)
    .toEqual(view);
  await expect.poll(firstVisibleLine).toBe(line);
  await page.getByRole("button", { name: "Close files", exact: true }).click();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect.poll(firstVisibleLine).toBe(line);
  await expect.poll(() => tree.evaluate((node) => node.scrollTop)).toBe(700);
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(
    page.getByRole("tablist", { name: "Open files" }).getByRole("tab"),
  ).toHaveCount(2);
  await expect(
    page.getByRole("treeitem", { name: /extra-.*file/ }),
  ).toHaveCount(260);
  await expect.poll(firstVisibleLine).toBe(line);
  await expect.poll(() => tree.evaluate((node) => node.scrollTop)).toBe(700);

  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".file-tabs")).toHaveCount(0);
  await findProjectFile(page, "src/App.tsx");
  await expect(code).toContainText("other");
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(
    page.getByRole("tablist", { name: "Open files" }).getByRole("tab"),
  ).toHaveCount(2);
  await expect.poll(firstVisibleLine).toBe(line);
});

test("Files inspector resizes and expands while keeping the conversation mounted and handles missing files and late reads", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 });
  await desktop(page);
  await findProjectFile(page, "src/App.tsx");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my place");
  await page
    .locator(".transcript")
    .evaluate((node) => node.setAttribute("data-mounted", "yes"));
  const pane = page.getByRole("complementary", {
    name: "Project files",
    exact: true,
  });
  const width = (await pane.boundingBox())!.width;
  const resize = page.getByRole("separator", {
    name: "Resize project inspector",
  });
  await resize.focus();
  await resize.press("ArrowLeft");
  await expect
    .poll(async () => (await pane.boundingBox())!.width)
    .toBeGreaterThan(width);
  const divider = (await resize.boundingBox())!;
  await page.mouse.move(divider.x + divider.width / 2, divider.y + 100);
  await page.mouse.down();
  await page.mouse.move(divider.x - 80, divider.y + 100);
  await page.mouse.up();
  await expect
    .poll(async () => (await pane.boundingBox())!.width)
    .toBeGreaterThan(width + 60);
  const tree = pane.getByRole("complementary", {
    name: "Project files and changes",
  });
  const treeResize = page.getByRole("separator", { name: "Resize file tree" });
  const treeWidth = async () => (await tree.boundingBox())!.width;
  const initialTreeWidth = await treeWidth();
  const inspectorWidth = (await pane.boundingBox())!.width;
  const preview = page.locator(".code-viewer");
  await preview.evaluate((node) => node.setAttribute("data-mounted", "yes"));
  await treeResize.focus();
  await treeResize.press("ArrowRight");
  await expect.poll(treeWidth).toBe(initialTreeWidth + 32);
  const treeDivider = (await treeResize.boundingBox())!;
  await page.mouse.move(
    treeDivider.x + treeDivider.width / 2,
    treeDivider.y + 100,
  );
  await page.mouse.down();
  await page.mouse.move(
    treeDivider.x + treeDivider.width / 2 + 60,
    treeDivider.y + 100,
  );
  await page.mouse.up();
  await expect.poll(treeWidth).toBe(initialTreeWidth + 92);
  const preferredTreeWidth = await treeWidth();
  await expect
    .poll(async () => (await pane.boundingBox())!.width)
    .toBe(inspectorWidth);
  await expect(preview).toHaveAttribute("data-mounted", "yes");
  await page.getByRole("button", { name: "Expand project inspector" }).click();
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
  await expect(input).toBeHidden();
  await expect(
    page.getByRole("button", { name: "Sessions", exact: true }),
  ).not.toHaveClass(/active/);
  await expect(
    page.getByRole("button", { name: "Files", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  await page
    .getByRole("button", { name: "Restore conversation layout" })
    .click();
  await expect(input).toHaveValue("Keep my place");
  await expect(
    page.getByRole("button", { name: "Sessions", exact: true }),
  ).toHaveClass(/active/);
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  await page.screenshot({ path: "test-results/files-inspector-dark.png" });
  await page
    .getByRole("button", { name: "Load earlier messages", exact: true })
    .click();
  const transcript = page.locator(".transcript");
  await transcript.evaluate((node) => {
    node.scrollTop = 250;
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Expand project inspector" }).click();
  await page
    .getByRole("button", { name: "Restore conversation layout" })
    .click();
  await expect
    .poll(() => transcript.evaluate((node) => node.scrollTop))
    .toBe(250);

  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:README.md"),
  );
  await page.keyboard.press("Meta+p");
  const search = page.getByRole("combobox", { name: "Find project file" });
  await search.fill("README.md");
  await page.getByRole("option", { name: "README.md", exact: true }).click();
  await expect(
    page.getByText("Loading file preview…", { exact: true }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "App.tsx", exact: true }).click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
  await page.evaluate(() =>
    (window as any).fixture.release("read_project_file:README.md"),
  );
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
  await page.evaluate(() => {
    (window as any).fixture.setProjectFile("src/App.tsx", null);
    window.dispatchEvent(new Event("focus"));
  });
  await expect(
    page.getByRole("alert").filter({ hasText: "File not found: src/App.tsx" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.setProjectFile("src/App.tsx", "// restored\n"),
  );
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(page.locator(".code-viewer .cm-editor").first()).toContainText(
    "restored",
  );
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page.screenshot({ path: "test-results/files-inspector-light.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(input).toBeVisible();
  await expect.poll(treeWidth).toBeLessThan(preferredTreeWidth);
  await expect
    .poll(
      async () =>
        (await page.locator(".file-editor-panel").boundingBox())!.width,
    )
    .toBeGreaterThanOrEqual(160);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/files-inspector-compact.png" });
  await page.setViewportSize({ width: 1500, height: 900 });
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect.poll(treeWidth).toBe(initialTreeWidth);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
});

for (const provider of ["codex", "opencode"]) {
  test(`${provider} Files context sends exact file and selection snapshots, removes chips and restores failed sends`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1500, height: 900 });
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
    }
    await page.evaluate(() =>
      (window as any).fixture.setProjectFile(
        "src/App.tsx",
        "export const selected = true;\nexport const untouched = false;\n",
      ),
    );
    await findProjectFile(page, "src/App.tsx");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await attachedContext(page)
      .getByRole("button", { name: "Remove src/App.tsx context", exact: true })
      .click();
    await expect(attachedContext(page)).toHaveCount(0);
    const editorInput = page.getByRole("textbox", {
      name: "Read-only preview of src/App.tsx",
      exact: true,
    });
    await editorInput.focus();
    await editorInput.press("ControlOrMeta+Home");
    await editorInput.press("Shift+ArrowDown");
    await page
      .getByRole("button", { name: "Ask about selection", exact: true })
      .click();
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Explain the selected code.");
    await expect(attachedContext(page)).toContainText("src/App.tsx:1");
    await findProjectFile(page, "README.md");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(
      page.getByRole("alert").filter({ hasText: "Connection refused" }),
    ).toBeVisible();
    await expect(
      attachedContext(page).locator(".file-context-chip"),
    ).toHaveCount(2);
    await expect(input).toHaveValue("Explain the selected code.");
    await page.getByRole("button", { name: /New session/ }).click();
    await expect(attachedContext(page)).toHaveCount(0);
    await page
      .getByRole("button", {
        name:
          provider === "codex"
            ? "Refine the session workspace"
            : "OpenCode session",
        exact: true,
      })
      .click();
    await expect(
      attachedContext(page).locator(".file-context-chip"),
    ).toHaveCount(2);
    await input.press("Enter");
    await expect(attachedContext(page)).toHaveCount(0);
    await expect(
      page.locator('[aria-label="Sent file context"] .file-context-chip'),
    ).toHaveCount(2);
    await expect(page.locator(".user-text").last()).toHaveText(
      "Explain the selected code.",
    );
    const prompts = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === `${provider}_send_prompt`)
          .map((call: any) => call.args.prompt),
      provider,
    );
    expect(prompts).toHaveLength(2);
    expect(prompts[1]).toBe(prompts[0]);
    const payload = JSON.parse(
      prompts[1]
        .split("<caelum-file-context>\n")[1]
        .split("\n</caelum-file-context>")[0],
    );
    expect(payload.files).toEqual([
      expect.objectContaining({
        path: "src/App.tsx",
        content: "export const selected = true;\n",
        startLine: 1,
        endLine: 1,
        truncated: false,
      }),
      expect.objectContaining({
        path: "README.md",
        content: "# Caelum\nProject documentation.\n",
        truncated: false,
      }),
    ]);
    await input.press("ArrowUp");
    await expect(input).toHaveValue("Explain the selected code.");
    await page.screenshot({
      path: `test-results/${provider}-file-context.png`,
    });
  });

  test(`${provider} Files context transfers on first send and isolates new-session provider drafts`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await findProjectFile(page, "src/App.tsx");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Use this snapshot");
    await choose(page, "Provider", provider === "codex" ? "OpenCode" : "Codex");
    await expect(attachedContext(page)).toHaveCount(0);
    await choose(page, "Provider", provider === "codex" ? "Codex" : "OpenCode");
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await expect(input).toHaveValue("Use this snapshot");
    await page.evaluate(() => {
      (window as any).fixture.holdStart();
      (window as any).fixture.failSend();
    });
    await input.press("Enter");
    await expect(input).toBeDisabled();
    await page
      .getByRole("button", {
        name: "Refine the session workspace",
        exact: true,
      })
      .click();
    await expect(attachedContext(page)).toHaveCount(0);
    await page.evaluate(() => (window as any).fixture.releaseStart());
    await expect(
      page.getByRole("button", { name: "Use this snapshot", exact: true }),
    ).toBeVisible();
    await expect(page.locator(".breadcrumbs strong")).toHaveText(
      "Refine the session workspace",
    );
    await page
      .getByRole("button", { name: "Use this snapshot", exact: true })
      .click();
    await expect(
      page.getByRole("alert").filter({ hasText: "Connection refused" }),
    ).toBeVisible();
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await input.press("Enter");
    await expect(
      page.locator('[aria-label="Sent file context"]'),
    ).toContainText("src/App.tsx");
    await expect(attachedContext(page)).toHaveCount(0);
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    expect(
      calls.filter((call: any) => call.command === `${provider}_start_session`),
    ).toHaveLength(1);
    expect(
      calls.filter((call: any) => call.command === `${provider}_send_prompt`),
    ).toHaveLength(2);
    await page.getByRole("button", { name: /New session/ }).click();
    await expect(attachedContext(page)).toHaveCount(0);
    await expect(input).toHaveValue("");
  });
}

test("Files attachment callbacks and failed sends stay scoped across session and project navigation", async ({
  page,
}) => {
  await desktop(page);
  await findProjectFile(page, "src/App.tsx");
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:src/App.tsx"),
  );
  await page
    .getByRole("treeitem", { name: "App.tsx, file", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Add to prompt");
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.release("read_project_file:src/App.tsx"),
  );
  await expect(attachedContext(page)).toHaveCount(0);
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(attachedContext(page)).toHaveCount(0);
  await page
    .locator(".file-editor-toolbar")
    .getByRole("button", { name: "Add to prompt", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Scoped retry");
  await page.evaluate(() => {
    (window as any).fixture.hold("codex_send_prompt");
    (window as any).fixture.failSend();
  });
  await input.press("Enter");
  await expect(attachedContext(page)).toHaveCount(0);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(attachedContext(page)).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("codex_send_prompt"),
  );
  await expect(input).toHaveValue("");
  await expect(attachedContext(page)).toHaveCount(0);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(input).toHaveValue("Scoped retry");
  await expect(attachedContext(page)).toContainText("src/App.tsx");
});

test("Files opens local Markdown and tool references at their source line without converting code blocks", async ({
  page,
}) => {
  await desktop(page);
  await page.evaluate(() => {
    (window as any).fixture.setProjectFile(
      "src/App.tsx",
      "// one\n// two\n// three\n",
    );
    (window as any).fixture.setCodexHistory(
      [
        {
          id: "refs",
          status: "completed",
          items: [
            {
              id: "refs-answer",
              type: "agentMessage",
              text: "See `src/App.tsx:2`, [the stylesheet](src/styles.css#L1) and [the last line](file:///work/caelum/src/App.tsx:3).\n\n```text\nsrc/App.tsx:3\n```",
            },
            {
              id: "refs-tool",
              type: "fileChange",
              status: "completed",
              changes: [
                {
                  path: "/work/caelum/src/App.tsx",
                  kind: { type: "update" },
                  diff: "@@ -1 +1 @@\n-old\n+new",
                },
              ],
            },
          ],
        },
      ],
      false,
    );
    window.dispatchEvent(new Event("focus"));
  });
  const patch = page.locator('[data-entry-id="codex-refs-tool"]');
  await expect(patch.locator(".tool-summary")).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  await expect(
    patch.locator(".tool-patch-preview .diff-line.removed"),
  ).toHaveText("-old");
  await expect(
    patch.locator(".tool-patch-preview .diff-line.added"),
  ).toHaveText("+new");
  await page
    .locator(".markdown")
    .getByRole("button", { name: "src/App.tsx:2", exact: true })
    .click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
  await expect(page.locator(".cm-activeLineGutter")).toHaveText("2");
  await expect(page.locator(".code-block .file-reference")).toHaveCount(0);
  await page
    .locator(".markdown")
    .getByRole("button", { name: "the last line", exact: true })
    .click();
  await expect(page.locator(".cm-activeLineGutter")).toHaveText("3");
  await page
    .locator(".markdown")
    .getByRole("button", { name: "src/App.tsx:2", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Copy relative path");
  await expect.poll(() => copiedText(page)).toBe("src/App.tsx");
  await page
    .locator(".markdown")
    .getByRole("button", { name: "the stylesheet", exact: true })
    .click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/styles.css");
  await page
    .getByRole("button", { name: "Edit /work/caelum/src/App.tsx", exact: true })
    .click();
  await page
    .locator(".tool-file-links")
    .getByRole("button", { name: "src/App.tsx", exact: true })
    .click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
});

test("Files highlights Rust, stays read-only, searches locally and preserves exact Unicode selections", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  await desktop(page);
  const source =
    '// Rust preview\r\npub fn main() {\r\n    let count: u32 = 42;\r\n    let label = r#"Caelum 🚀"#;\r\n    println!("{label}: {count}");\r\n}\r\n';
  await page.evaluate(
    (source) => (window as any).fixture.setProjectFile("src/main.rs", source),
    source,
  );
  await findProjectFile(page, "src/main.rs");
  const viewer = page.locator(".code-viewer");
  const preview = viewer.getByRole("textbox", {
    name: "Read-only preview of src/main.rs",
    exact: true,
  });
  await expect(preview).toHaveAttribute("aria-readonly", "true");
  await expect(preview).toHaveAttribute("contenteditable", "false");
  await expect(viewer.locator(".syntax-keyword")).toHaveText([
    "pub",
    "fn",
    "let",
    "let",
  ]);
  await expect(viewer.locator(".syntax-type")).toContainText("u32");
  await expect(viewer.locator(".syntax-number")).toHaveText("42");
  await expect(viewer.locator(".syntax-comment")).toHaveText("// Rust preview");
  const rendered = () =>
    viewer
      .locator(".cm-line")
      .allTextContents()
      .then((lines) => lines.join("\n"));
  await expect.poll(rendered).toBe(source.replaceAll("\r\n", "\n"));
  await preview.press("ControlOrMeta+a");
  await preview.press("x");
  await preview.press("Backspace");
  await expect.poll(rendered).toBe(source.replaceAll("\r\n", "\n"));
  await preview.press("ArrowLeft");
  await preview.press("ControlOrMeta+f");
  const search = viewer.getByRole("textbox", { name: "Find", exact: true });
  await expect(search).toBeFocused();
  await search.pressSequentially("Caelum");
  await search.press("Enter");
  await expect(viewer.locator(".cm-activeLineGutter")).toHaveText("4");
  await expect(viewer.locator(".cm-searchMatch-selected")).toHaveText("Caelum");
  await search.press("Escape");
  await expect(preview).toBeFocused();
  await preview.press("ControlOrMeta+Home");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  const selected = source.split("\r\n").slice(0, 4).join("\r\n") + "\r\n";
  await expect.poll(() => copiedText(page)).toBe(selected);
  await preview.press("Shift+F10");
  await pickMenu(page, "Add selection to prompt");
  const chip = attachedContext(page).getByRole("button", {
    name: "src/main.rs:1-4",
    exact: true,
  });
  await chip.click({ button: "right" });
  await pickMenu(page, "Copy attached snapshot");
  await expect.poll(() => copiedText(page)).toBe(selected);
  await page.screenshot({ path: "test-results/rust-preview-dark.png" });
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(viewer.locator(".syntax-keyword").first()).toHaveCSS(
    "color",
    "rgb(123, 60, 171)",
  );
  await page.screenshot({ path: "test-results/rust-preview-light.png" });
  expect(errors).toEqual([]);
});

test("Files migrates legacy reading positions while delayed Rust grammars preserve navigation and composer focus", async ({
  page,
}) => {
  let release = () => {};
  let requests = 0;
  const loading = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/node_modules/.vite/deps/dist-*.js*", async (route) => {
    const response = await route.fetch();
    if ((await response.text()).includes("function rust(")) {
      requests++;
      await loading;
    }
    await route.fulfill({ response });
  });
  try {
    await desktop(page);
    await page.evaluate(() => {
      (window as any).fixture.setProjectFile(
        "src/main.rs",
        Array.from(
          { length: 200 },
          (_, index) => `pub fn line${index + 1}() -> u32 { ${index + 1} }`,
        ).join("\n"),
      );
      localStorage.setItem(
        "caelum.files.v1:%2Fwork%2Fcaelum",
        JSON.stringify({
          tabs: [{ path: "src/main.rs", mode: "file" }],
          active: "file:src/main.rs",
          views: {
            "file:src/main.rs": {
              viewState: {
                firstPosition: { lineNumber: 40 },
                firstPositionDeltaTop: -4,
                scrollLeft: 0,
              },
              cursorState: [
                {
                  selectionStart: { lineNumber: 41, column: 1 },
                  position: { lineNumber: 41, column: 1 },
                },
              ],
            },
          },
        }),
      );
    });
    await page.getByRole("button", { name: "Files", exact: true }).click();
    const viewer = page.locator(".code-viewer");
    await expect(viewer.getByRole("textbox")).toBeVisible();
    await expect.poll(() => requests).toBeGreaterThan(0);
    await expect(viewer.locator(".syntax-keyword")).toHaveCount(0);
    const firstLine = () =>
      viewer.locator(".cm-line").evaluateAll((lines) => {
        const top =
          lines[0]?.closest(".cm-scroller")!.getBoundingClientRect().top ?? 0;
        return lines.find((line) => line.getBoundingClientRect().bottom > top)
          ?.textContent;
      });
    await expect.poll(firstLine).toBe("pub fn line40() -> u32 { 40 }");
    await expect(viewer.locator(".cm-activeLineGutter")).toHaveText("41");
    await findProjectFile(page, "README.md");
    await page.getByRole("tab", { name: "main.rs", exact: true }).click();
    await expect.poll(firstLine).toBe("pub fn line40() -> u32 { 40 }");
    const scroller = viewer.locator(".cm-scroller");
    const top = await scroller.evaluate((node) => node.scrollTop);
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep this draft and focus");
    release();
    await expect(viewer.locator(".syntax-keyword").first()).toHaveText("pub");
    await expect.poll(firstLine).toBe("pub fn line40() -> u32 { 40 }");
    await expect
      .poll(async () =>
        Math.abs((await scroller.evaluate((node) => node.scrollTop)) - top),
      )
      .toBeLessThanOrEqual(1); // WebKit rounds fractional line heights after highlighting.
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Keep this draft and focus");
  } finally {
    release();
  }
});

test("Files diff panes preserve shared scrolling, separate selections and HEAD snapshots across refreshes", async ({
  page,
}) => {
  await desktop(page);
  const original =
    'export const label = "before 🚀";\r\n' +
    Array.from(
      { length: 200 },
      (_, index) => `export const line${index} = ${index};\r\n`,
    ).join("");
  const modified = original.replace("before", "after");
  await page.evaluate(
    ({ original, modified }) => {
      (window as any).fixture.setGitVersions("src/App.tsx", original, modified);
    },
    { original, modified },
  );
  await findProjectFile(page, "src/App.tsx");
  await page
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  const merge = page.locator(".cm-mergeView");
  const before = merge.getByRole("textbox", {
    name: "Read-only HEAD version of src/App.tsx",
    exact: true,
  });
  const after = merge.getByRole("textbox", {
    name: "Read-only working tree version of src/App.tsx",
    exact: true,
  });
  await expect(merge).toBeVisible();
  await expect(merge.locator(".cm-merge-a .cm-changedLine").first()).toHaveCSS(
    "background-color",
    "rgb(44, 24, 29)",
  );
  await expect(merge.locator(".cm-merge-b .cm-changedLine").first()).toHaveCSS(
    "background-color",
    "rgb(20, 35, 25)",
  );
  await before.press("ControlOrMeta+Home");
  await before.press("Shift+ArrowDown");
  await before.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe('export const label = "before 🚀";\r\n');
  await after.press("ControlOrMeta+Home");
  await after.press("Shift+ArrowDown");
  await after.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe('export const label = "after 🚀";\r\n');
  await merge.evaluate((node) => {
    node.scrollTop = 1200;
  });
  const firstLines = () =>
    merge.locator(".cm-editor").evaluateAll((panes) =>
      panes.map((pane) => {
        const top = pane.closest(".cm-mergeView")!.getBoundingClientRect().top;
        return Array.from(pane.querySelectorAll(".cm-line")).find(
          (line) => line.getBoundingClientRect().bottom > top,
        )?.textContent;
      }),
    );
  await expect
    .poll(firstLines)
    .toEqual([
      expect.stringMatching(/export const line\d/),
      expect.stringMatching(/export const line\d/),
    ]);
  const lines = await firstLines();
  expect(lines[0]).toBe(lines[1]);
  await findProjectFile(page, "README.md");
  await page.getByRole("tab", { name: "App.tsx · Diff", exact: true }).click();
  await expect.poll(firstLines).toEqual(lines);
  await expect.poll(() => merge.evaluate((node) => node.scrollTop)).toBe(1200);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep reviewing");
  await page.evaluate(
    ({ original, modified }) => {
      (window as any).fixture.setGitVersions(
        "src/App.tsx",
        original,
        modified + "// refreshed\r\n",
      );
      window.dispatchEvent(new Event("focus"));
    },
    { original, modified },
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === "git_file_versions",
          ).length,
      ),
    )
    .toBeGreaterThan(2);
  await expect.poll(firstLines).toEqual(lines);
  await expect(input).toBeFocused();
  await before.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe('export const label = "before 🚀";\r\n');
  await after.press("Shift+F10");
  await pickMenu(page, "Copy file preview");
  await expect.poll(() => copiedText(page)).toBe(modified + "// refreshed\r\n");
});

test("Files recovers damaged saved tabs and reports storage failures without losing the open preview", async ({
  page,
}) => {
  await desktop(page);
  await page.evaluate(() =>
    localStorage.setItem(
      "caelum.files.v1:%2Fwork%2Fcaelum",
      JSON.stringify({
        tabs: [
          null,
          { path: "../outside.txt", mode: "file" },
          { path: "README.md", mode: "file" },
          { path: "README.md", mode: "file" },
        ],
        active: "file:missing.ts",
        expanded: [null, "../outside", "src"],
        views: { "file:README.md": { invalid: true } },
        treeScroll: -100,
      }),
    ),
  );
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(
    page.getByRole("tablist", { name: "Open files" }).getByRole("tab"),
  ).toHaveCount(1);
  await expect(page.locator(".file-editor-path")).toHaveText("README.md");
  await expect(page.locator(".code-viewer .cm-editor").first()).toContainText(
    "Project documentation.",
  );
  await page.evaluate(() => {
    const setItem = Storage.prototype.setItem;
    (window as any).restoreFileStorage = () => {
      Storage.prototype.setItem = setItem;
    };
    Storage.prototype.setItem = function (key, value) {
      if (key.startsWith("caelum.files.v1:"))
        throw new DOMException("Storage unavailable", "QuotaExceededError");
      setItem.call(this, key, value);
    };
  });
  await findProjectFile(page, "src/App.tsx");
  const error = page
    .getByRole("alert")
    .filter({ hasText: "Could not save file tabs" });
  await expect(error).toBeVisible();
  await page.evaluate(() => (window as any).restoreFileStorage());
  await error.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(error).toHaveCount(0);
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
});

test("settings switches and persists complete themes, follows the system and preserves live drafts", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  await desktop(page);
  const root = page.locator("html");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft through settings");
  await expect(root).toHaveAttribute("data-theme", "dark");
  const dialog = await openSettings(page);
  await page.screenshot({ path: "test-results/settings-dark.png" });
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await expect(root).toHaveAttribute("data-theme", "light");
  await expect(root).toHaveCSS("background-color", "rgb(250, 250, 252)");
  await expect(page.locator(".composer")).toHaveCSS(
    "background-color",
    "rgb(255, 255, 255)",
  );
  await expect(page.locator(".changes-pane")).toHaveCSS(
    "background-color",
    "rgb(245, 245, 249)",
  );
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "settings-live",
    delta: "Work continues while settings are open",
  });
  await page.keyboard.press("Meta+n");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(input).toHaveValue("Keep this draft through settings");
  await expect(
    page.getByText("Work continues while settings are open", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Settings", exact: true }),
  ).toBeFocused();
  await page.screenshot({ path: "test-results/workspace-light.png" });
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter(
              (call: any) =>
                call.command === "plugin:window|set_background_color",
            )
            .at(-1)?.args.value,
      ),
    )
    .toBe("#fafafc");

  await page.reload();
  await expect(root).toHaveAttribute("data-theme", "light");
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await openSettings(page);
  await expect(
    dialog.getByRole("radio", { name: "Light", exact: true }),
  ).toBeChecked();
  await dialog.getByRole("radio", { name: "System", exact: true }).check();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(root).toHaveAttribute("data-theme", "dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(root).toHaveAttribute("data-theme", "light");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:window|set_theme")
            .at(-1)?.args.value,
      ),
    )
    .toBe(null);
  await dialog.getByRole("radio", { name: "Dark", exact: true }).check();
  await expect(root).toHaveAttribute("data-theme", "dark");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await page.keyboard.press("Meta+,");
  await expect(dialog).toBeVisible();
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(dialog).toBeInViewport();
  await expect(
    dialog.getByRole("button", { name: "Done", exact: true }),
  ).toBeInViewport();
  await page.mouse.click(5, 5);
  await expect(dialog).toHaveCount(0);
});

test("accent colours support keyboard selection, persist and adapt to explicit and system themes", async ({
  page,
}) => {
  await page.addInitScript(() => {
    if (!localStorage.getItem("caelum.settings.v1"))
      localStorage.setItem(
        "caelum.settings.v1",
        JSON.stringify({
          theme: "light",
          providers: { codex: { model: "provider/fast", effort: "low" } },
        }),
      );
  });
  await desktop(page);
  const root = page.locator("html");
  const brand = page.locator(".brand-row");
  await expect(root).toHaveAttribute("data-accent", "violet");
  await expect(brand).toHaveCSS("color", "rgb(108, 86, 176)");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my accent choice");
  const dialog = await openSettings(page);
  const colours = dialog.getByRole("group", {
    name: "Accent colour",
    exact: true,
  });
  await colours
    .getByRole("radio", { name: "Violet", exact: true })
    .press("ArrowRight");
  await expect(
    colours.getByRole("radio", { name: "Blue", exact: true }),
  ).toBeChecked();
  await page.keyboard.press("ArrowRight");
  await expect(
    colours.getByRole("radio", { name: "Teal", exact: true }),
  ).toBeChecked();
  await expect(
    colours.getByRole("radio", { name: "Teal", exact: true }),
  ).toBeFocused();
  await expect(brand).toHaveCSS("color", "rgb(33, 111, 104)");
  await expect(page.locator(".send-button")).toHaveCSS(
    "background-color",
    "rgb(35, 119, 111)",
  );
  await expect(page.locator(".latest-user")).toHaveCSS(
    "border-left-color",
    "rgb(33, 111, 104)",
  );
  await page.screenshot({ path: "test-results/settings-accent-light.png" });
  await dialog.getByRole("radio", { name: "Dark", exact: true }).check();
  await expect(brand).toHaveCSS("color", "rgb(114, 199, 191)");
  await expect(page.locator(".send-button")).toHaveCSS(
    "background-color",
    "rgb(35, 119, 111)",
  );
  await page.screenshot({ path: "test-results/settings-accent-dark.png" });
  await dialog.getByRole("radio", { name: "System", exact: true }).check();
  await page.emulateMedia({ colorScheme: "light" });
  await expect(brand).toHaveCSS("color", "rgb(33, 111, 104)");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(brand).toHaveCSS("color", "rgb(114, 199, 191)");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await input.press("Enter");
  await expect(page.locator(".composer")).toHaveAttribute(
    "data-working",
    "true",
  );
  await openSettings(page);
  await colours.getByRole("radio", { name: "Rose", exact: true }).check();
  await expect
    .poll(() =>
      page
        .locator(".composer")
        .evaluate(
          (element) => getComputedStyle(element, "::before").borderColor,
        ),
    )
    .toBe("rgb(231, 154, 176)");
  await colours.getByRole("radio", { name: "Teal", exact: true }).check();
  await page.reload();
  await expect(root).toHaveAttribute("data-accent", "teal");
  await expect(brand).toHaveCSS("color", "rgb(114, 199, 191)");
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("caelum.settings.v1")!),
    ),
  ).toMatchObject({
    theme: "system",
    accentColor: "teal",
    providers: { codex: { model: "provider/fast", effort: "low" } },
  });
  await page.setViewportSize({ width: 800, height: 620 });
  await openSettings(page);
  await expect(
    colours.getByRole("radio", { name: "Teal", exact: true }),
  ).toBeChecked();
  await colours.getByRole("radio", { name: "Rose", exact: true }).check();
  await colours
    .getByRole("radio", { name: "Rose", exact: true })
    .press("ArrowLeft");
  await expect(
    colours.getByRole("radio", { name: "Amber", exact: true }),
  ).toBeChecked();
  await colours.getByRole("radio", { name: "Violet", exact: true }).check();
  await expect(
    colours.getByRole("radio", { name: "Violet", exact: true }),
  ).toBeChecked();
  await expect(brand).toHaveCSS("color", "rgb(163, 154, 246)");
  await expect(
    dialog.getByRole("button", { name: "Done", exact: true }),
  ).toBeInViewport();
});

test("settings themes and accents update mounted code previews and diffs", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await desktop(page);
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Blue", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click();
  await page
    .getByRole("treeitem", { name: "App.tsx, file", exact: true })
    .click();
  const editor = page.locator(".code-viewer .cm-editor").first();
  await expect(editor).toBeVisible();
  await expect(editor).toHaveCSS("background-color", "rgb(11, 12, 14)");
  await expect(editor.locator(".cm-activeLineGutter")).toHaveCSS(
    "color",
    "rgb(130, 180, 244)",
  );
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Teal", exact: true }).check();
  await expect(editor.locator(".cm-activeLineGutter")).toHaveCSS(
    "color",
    "rgb(114, 199, 191)",
  );
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(editor).toHaveCSS("background-color", "rgb(250, 250, 252)");
  await expect(editor.locator(".cm-activeLineGutter")).toHaveCSS(
    "color",
    "rgb(33, 111, 104)",
  );
  await editor.getByRole("textbox").press("ControlOrMeta+a");
  await expect(editor.locator(".cm-selectionBackground").first()).toHaveCSS(
    "background-color",
    "color(srgb 0.129412 0.435294 0.407843 / 0.2)",
  );
  await page
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  await expect(page.locator(".cm-mergeView")).toBeVisible();
  await expect(editor).toHaveCSS("background-color", "rgb(250, 250, 252)");
  const diffPanes = page.locator(".cm-mergeView .cm-editor");
  await expect(diffPanes).toHaveCount(2);
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("ControlOrMeta+a");
    await expect(pane.locator(".cm-selectionBackground").first()).toHaveCSS(
      "background-color",
      "color(srgb 0.129412 0.435294 0.407843 / 0.2)",
    );
  }
  await page.screenshot({ path: "test-results/files-light.png" });
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Rose", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("ControlOrMeta+a");
    await expect(pane.locator(".cm-selectionBackground").first()).toHaveCSS(
      "background-color",
      "color(srgb 0.647059 0.254902 0.388235 / 0.2)",
    );
  }
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Dark", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(editor).toHaveCSS("background-color", "rgb(11, 12, 14)");
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("ControlOrMeta+a");
    await expect(pane.locator(".cm-selectionBackground").first()).toHaveCSS(
      "background-color",
      "color(srgb 0.905882 0.603922 0.690196 / 0.28)",
    );
  }
});

for (const provider of ["codex", "opencode"]) {
  test(`${provider} settings persist new-session defaults and keep existing sessions and draft overrides independent`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
    }
    const name = provider === "codex" ? "Codex" : "OpenCode";
    const dialog = await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await expect(
      dialog.getByRole("status", { name: `${name} connection status` }),
    ).toHaveText("Available");
    await choose(page, "Default model", "Fast model provider/fast");
    await choose(page, "Default effort", "Low");
    if (provider === "opencode") await choose(page, "Default agent", "Build");
    await page.screenshot({ path: `test-results/${provider}-settings.png` });
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Test model");
    await page.reload();
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await expect(
      dialog.getByRole("button", { name: "Default model", exact: true }),
    ).toContainText("Fast model");
    await expect(
      dialog.getByRole("button", { name: "Default effort", exact: true }),
    ).toHaveText("Low");
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await page.keyboard.press("Meta+n");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Fast model");
    await choose(page, "Model", "Fast model provider/fast");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("Low");
    if (provider === "opencode")
      await expect(
        page.getByRole("button", { name: "Build", exact: true }),
      ).toHaveAttribute("aria-pressed", "true");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Use my saved defaults");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    expect(
      calls.find((call: any) => call.command === `${provider}_start_session`)
        .args,
    ).toMatchObject({
      model: "provider/fast",
      agent: provider === "opencode" ? "build" : null,
    });
    expect(
      calls.find((call: any) => call.command === `${provider}_send_prompt`).args
        .effort,
    ).toBe("low");
    await page.getByRole("button", { name: "Stop generation" }).click();
    await page.keyboard.press("Meta+n");
    await choose(page, "Model", "Test model provider/model");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("Default effort");
    await input.fill("Keep this draft override");
    await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await expect(
      dialog.getByRole("button", { name: "Default model", exact: true }),
    ).toContainText("Fast model");
    await choose(page, "Default model", "Simple model provider/plain");
    await expect(
      dialog.getByRole("button", { name: "Default effort", exact: true }),
    ).toHaveText("Model default");
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await expect(input).toHaveValue("Keep this draft override");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Test model");
  });
}

test("provider settings recover connection errors and isolate delayed capability checks", async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 620 });
  await desktop(page);
  await page.evaluate(() =>
    (window as any).fixture.setChoicesError(
      "codex",
      "Sign in with codex login",
    ),
  );
  const dialog = await openSettings(page, "Providers");
  await expect(
    dialog.getByRole("status", { name: "Codex connection status" }),
  ).toHaveText("Unavailable");
  await expect(dialog.getByRole("alert")).toHaveText(
    "Error: Sign in with codex login",
  );
  await page.evaluate(() => {
    (window as any).fixture.setChoicesError("codex", null);
    (window as any).fixture.hold("provider_choices:codex");
  });
  await dialog.getByRole("button", { name: "Refresh Codex status" }).click();
  await expect(
    dialog.getByRole("status", { name: "Codex connection status" }),
  ).toHaveText("Checking…");
  await dialog.getByRole("button", { name: "OpenCode", exact: true }).click();
  await expect(
    dialog.getByRole("status", { name: "OpenCode connection status" }),
  ).toHaveText("Available");
  await page.evaluate(() =>
    (window as any).fixture.release("provider_choices:codex"),
  );
  await expect(
    dialog.getByRole("status", { name: "OpenCode connection status" }),
  ).toHaveText("Available");
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  await dialog.getByRole("button", { name: "Codex", exact: true }).click();
  await expect(
    dialog.getByRole("status", { name: "Codex connection status" }),
  ).toHaveText("Available");
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .press("ArrowDown");
  await expect(
    page.getByRole("dialog", { name: "Choose default model", exact: true }),
  ).toBeInViewport();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toBeFocused();
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .press("ArrowDown");
  await page
    .getByRole("combobox", { name: "Search default models" })
    .fill("fast");
  await page.keyboard.press("Enter");
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toContainText("Fast model");
  await dialog.getByRole("button", { name: "Reset Codex defaults" }).click();
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toHaveText("Provider default");
  await choose(page, "Default provider", "OpenCode");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page.getByRole("button", { name: "Close caelum", exact: true }).click();
  await openSettings(page, "Providers");
  await expect(dialog.getByRole("status")).toHaveText("No project selected");
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toBeDisabled();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page
    .getByRole("button", { name: "Open project", exact: true })
    .last()
    .click();
  await expect(
    page.getByRole("button", { name: "Provider", exact: true }),
  ).toHaveText("OpenCode");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter(
              (call: any) =>
                call.command === "set_project_provider" &&
                call.args.projectPath === "/work/other",
            )
            .at(-1)?.args.provider,
      ),
    )
    .toBe("opencode");
});

test("settings recover damaged preferences and report storage failures with a working retry", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("caelum.settings.v1", "{broken");
    const setItem = Storage.prototype.setItem;
    (window as any).restoreStorage = () => {
      Storage.prototype.setItem = setItem;
    };
    Storage.prototype.setItem = function (key, value) {
      if (key === "caelum.settings.v1") throw new Error("Storage unavailable");
      setItem.call(this, key, value);
    };
  });
  await desktop(page);
  const dialog = await openSettings(page);
  await expect(dialog.getByRole("alert")).toContainText(
    "Could not read saved settings",
  );
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(dialog.getByRole("alert")).toContainText(
    "Could not save settings",
  );
  await page.evaluate(() => (window as any).restoreStorage());
  await dialog.getByRole("button", { name: "Retry saving" }).click();
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  expect(
    await page.evaluate(
      () => JSON.parse(localStorage.getItem("caelum.settings.v1")!).theme,
    ),
  ).toBe("light");
});

for (const provider of ["codex", "opencode"]) {
  const title =
    provider === "codex" ? "Refine the session workspace" : "OpenCode session";
  const id = provider === "codex" ? "thread-a" : "ses-a";
  test(`${provider} archives old sessions, restores drafts and settings with Undo, and stays removed after restart`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const row = sessionRow(page, title);
    await expect(row.locator(".session-branch")).toHaveAttribute(
      "title",
      provider === "codex"
        ? "Session branch: feat/session-ui-redesign"
        : "Current working-directory branch: main",
    );
    await expect(row.locator(".session-lines")).toHaveAttribute(
      "title",
      "Lines changed in the latest turn",
    );
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep my unsent draft");
    await choose(page, "Model", "Fast model provider/fast");
    await choose(page, "Effort", "Low");
    await row.hover();
    await row
      .getByRole("button", { name: "Archive session", exact: true })
      .click();
    await expect(row).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: "New session", exact: true }),
    ).toBeVisible();
    const notice = page.getByRole("status", { name: "Session archived" });
    await expect(notice).toContainText(title);
    await page.evaluate(() => (window as any).fixture.failArchive());
    await notice.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText(
      "Could not restore session",
    );
    await expect(row).toHaveCount(0);
    await page.getByRole("button", { name: "Dismiss error" }).click();
    await notice.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(input).toBeEnabled();
    await expect(input).toHaveValue("Keep my unsent draft");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Fast model");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("Low");
    await expect(row).toHaveCount(1);
    await expect(notice).toHaveCount(0);
    await row.hover();
    await row
      .getByRole("button", { name: "Archive session", exact: true })
      .click();
    await page.getByRole("button", { name: "Refresh sessions" }).click();
    await expect(row).toHaveCount(0);
    const forgets = await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "forget_project_session",
      ),
    );
    expect(forgets.map((call: any) => call.args)).toEqual(
      Array(2).fill({ projectPath: "/work/caelum", provider, sessionId: id }),
    );
    await page.reload();
    await expect(input).toBeEnabled();
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await expect(row).toHaveCount(0);
  });

  test(`${provider} protects active sessions and keeps archive failures and Undo from disturbing navigation`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const row = sessionRow(page, title);
    const archive = row.getByRole("button", {
      name: "Archive session",
      exact: true,
    });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Work on this session");
    await input.press("Enter");
    await expect(archive).toBeDisabled();
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.succeeded",
      provider === "codex"
        ? { threadId: id, turn: { id: "turn-new", status: "completed" } }
        : { sessionID: id },
      provider,
    );
    await expect(archive).toBeEnabled();
    await page.evaluate(
      ({ provider, id }) =>
        (window as any).fixture.emit("approval-requests", [
          {
            provider,
            sessionId: id,
            projectPath: "/work/caelum",
            requestId: "archive-approval",
            summary: "Approve command",
            description: "Run a command",
          },
        ]),
      { provider, id },
    );
    await expect(archive).toBeDisabled();
    await page.evaluate(() =>
      (window as any).fixture.emit("approval-requests", []),
    );
    await input.fill("Draft kept if archiving fails");
    await page.evaluate(() => (window as any).fixture.failArchive());
    await row.hover();
    await archive.click();
    await expect(page.getByRole("alert")).toContainText(
      "Could not archive session",
    );
    await expect(input).toHaveValue("Draft kept if archiving fails");
    await expect(archive).toBeEnabled();
    await page.getByRole("button", { name: "Dismiss error" }).click();
    await page.evaluate(() =>
      (window as any).fixture.hold("set_session_archived"),
    );
    await archive.click();
    await expect(archive).toBeDisabled();
    await expect(input).toBeDisabled();
    await page
      .getByRole("button", {
        name: "Review the provider adapters",
        exact: true,
      })
      .click();
    await input.fill("Other session draft");
    await page.evaluate(() =>
      (window as any).fixture.release("set_session_archived"),
    );
    await expect(row).toHaveCount(0);
    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(row).toHaveCount(1);
    await expect(input).toHaveValue("Other session draft");
    await expect(page.locator(".breadcrumbs")).toContainText(
      "Review the provider adapters",
    );
    await page.getByRole("button", { name: title, exact: true }).click();
    await expect(input).toHaveValue("Draft kept if archiving fails");
  });
}

test("late listings, history and metadata cannot overwrite an archived and restored session", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const row = sessionRow(page, "OpenCode session");
  await expect(row.locator(".session-branch")).toHaveText("main");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.holdHistory();
    fixture.setOpenWorking(true);
    fixture.hold("session_details");
    fixture.hold("opencode_list_sessions");
    window.dispatchEvent(new Event("focus"));
  });
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await row.hover();
  await row
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(row).toHaveCount(0);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setOpenWorking(false);
    fixture.setSessionDetails("opencode", "ses-a", {
      branch: "fresh-branch",
      branchIsCurrent: true,
      changes: { additions: 19, deletions: 4 },
    });
  });
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(row).toHaveCount(1);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.releaseHistory();
    fixture.release("session_details");
    fixture.release("opencode_list_sessions");
  });
  await expect(row.locator(".session-branch")).toHaveText("fresh-branch");
  await expect(row.locator(".lines-added")).toHaveText("+19");
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveAttribute("data-working", "false");
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  await row.hover();
  await row
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(row).toHaveCount(0);
  await emit(
    page,
    "session.title.updated",
    { sessionID: "ses-a", title: "Late title" },
    "opencode",
  );
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await expect(
    page.locator(".session-item").filter({ hasText: "Late title" }),
  ).toHaveCount(0);
});

test("sidebar metadata and archiving stay scoped when projects reuse a session ID", async ({
  page,
}) => {
  await desktop(page);
  const row = sessionRow(page, "Refine the session workspace");
  await expect(row.locator(".session-branch")).toHaveText(
    "feat/session-ui-redesign",
  );
  await page.evaluate(() => (window as any).fixture.hold("session_details"));
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "turn-1", status: "completed" },
  });
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await page.evaluate(() => (window as any).fixture.release("session_details"));
  await expect(row.locator(".session-branch")).toHaveText("other-project");
  await row.hover();
  await row
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(row).toHaveCount(0);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(row.locator(".session-branch")).toHaveText(
    "feat/session-ui-redesign",
  );
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(
    page.getByRole("status", { name: "Session archived" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "other", exact: true }).click();
  await expect(row.locator(".session-branch")).toHaveText("other-project");
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "set_session_archived",
    ),
  );
  expect(calls.map((call: any) => call.args.projectPath)).toEqual([
    "/work/other",
    "/work/other",
  ]);
});

test("sidebar resizes, retains its preferred width and keeps session details usable at the minimum window size", async ({
  page,
}) => {
  await desktop(page);
  const sidebar = page.getByRole("complementary", {
    name: "Projects and sessions",
  });
  const divider = page.getByRole("separator", { name: "Resize sidebar" });
  const width = async () => (await sidebar.boundingBox())!.width;
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my sidebar draft");
  await page
    .locator(".transcript")
    .evaluate((node) => node.setAttribute("data-mounted", "yes"));
  await divider.focus();
  await divider.press("Home");
  await expect.poll(width).toBe(180);
  await divider.press("ArrowRight");
  await expect.poll(width).toBe(212);
  await divider.press("ArrowLeft");
  await expect.poll(width).toBe(180);
  await divider.press("End");
  await expect.poll(width).toBe(480);
  const handle = (await divider.boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + 100);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2 - 100, handle.y + 100);
  await page.mouse.up();
  await expect.poll(width).toBe(380);
  const preferredWidth = await width();
  await expect(divider).toHaveAttribute(
    "aria-valuenow",
    String(preferredWidth),
  );
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await expect(divider).toHaveCount(0);
  await page.keyboard.press("Meta+\\");
  await expect.poll(width).toBe(preferredWidth);
  await expect(input).toHaveValue("Keep my sidebar draft");
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  const row = sessionRow(page, "Refine the session workspace");
  await expect(row.locator(".lines-added")).toHaveText("+42");
  await page
    .getByRole("textbox", { name: "Search sessions" })
    .fill("fix/provider-adapters");
  await expect(page.locator(".session-item")).toHaveCount(1);
  await page.getByRole("textbox", { name: "Search sessions" }).fill("");
  await page.evaluate(() =>
    (window as any).fixture.setSessionDetails("codex", "thread-a", {
      branch: "feature/a-very-long-branch-name-for-the-sidebar",
      branchIsCurrent: false,
      changes: { additions: 1234, deletions: 56 },
    }),
  );
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "turn-new", status: "completed" },
  });
  await expect(row.locator(".lines-added")).toHaveText("+1,234");
  await page.screenshot({ path: "test-results/session-sidebar-resized.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect.poll(width).toBe(220);
  await expect
    .poll(
      async () =>
        (await page.locator(".session-workspace").boundingBox())!.width,
    )
    .toBeGreaterThanOrEqual(300);
  await page
    .getByRole("button", { name: "Close changes", exact: true })
    .click();
  await expect.poll(width).toBe(preferredWidth);
  await page.getByRole("button", { name: "Show changes", exact: true }).click();
  await expect.poll(width).toBe(220);
  await page.getByRole("button", { name: "Expand project inspector" }).click();
  await expect.poll(width).toBe(preferredWidth);
  await page
    .getByRole("button", { name: "Restore conversation layout" })
    .click();
  await expect.poll(width).toBe(220);
  await expect(input).toHaveValue("Keep my sidebar draft");
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  await row.hover();
  await expect(row.locator(".session-lines")).toBeInViewport({ ratio: 1 });
  await expect(
    row.getByRole("button", { name: "Archive session", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/session-sidebar.png" });
  await page.setViewportSize({ width: 1280, height: 800 });
  await expect.poll(width).toBe(preferredWidth);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setSessionDetails("codex", "thread-a", {
      branch: null,
      branchIsCurrent: false,
      changes: { additions: 0, deletions: 0 },
    });
    fixture.setSessionDetails("codex", "thread-b", {
      branch: null,
      branchIsCurrent: false,
      changes: null,
    });
  });
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await expect(row.locator(".session-branch")).toHaveCount(0);
  await expect(row.locator(".lines-added")).toHaveText("+0");
  await expect(
    sessionRow(page, "Review the provider adapters").locator(".session-lines"),
  ).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setSessionDetails("codex", "thread-b", null),
  );
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await page
    .getByRole("button", {
      name: "Review the provider adapters",
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect.poll(width).toBe(preferredWidth);
  await page.reload();
  await expect.poll(width).toBe(preferredWidth);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect.poll(width).toBe(preferredWidth);
});

test("restores Markdown and tool details in a full-height dark workspace", async ({
  page,
}) => {
  await desktop(page);
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
  await expect(page.getByRole("table")).toBeVisible();
  await page.getByRole("link", { name: "documentation" }).click();
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === "plugin:opener|open_url",
        ).args.url,
    ),
  ).toBe("https://example.com/docs");
  await page.getByRole("button", { name: /Run pnpm test/ }).click();
  await expect(
    page.getByText("14 tests passed", { exact: true }),
  ).toBeVisible();
  await expect(page.locator("body")).toHaveCSS(
    "background-color",
    "rgba(0, 0, 0, 0)",
  );
  await expect(page.locator(":root")).toHaveCSS(
    "background-color",
    "rgb(11, 12, 14)",
  );
  await expect(page.getByText("LOCAL-FIRST AI WORKSPACE")).toHaveCount(0);
  await page.screenshot({ path: "test-results/session-workspace.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeInViewport();
  await expect(
    page.getByRole("button", { name: "Send message", exact: true }),
  ).toBeInViewport();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
});

test("typing and resizing a draft keep the latest response pinned without moving an older reading position", async ({
  page,
}) => {
  await desktop(page);
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const transcript = page.locator(".transcript");
  const gap = () =>
    transcript.evaluate(
      (node) => node.scrollHeight - node.scrollTop - node.clientHeight,
    );
  const height = () =>
    input.evaluate((node) => node.getBoundingClientRect().height);
  await input.fill("First\nSecond\nThird\nFourth");
  await transcript.evaluate((node) => {
    node.scrollTop = node.scrollHeight;
  });
  await expect.poll(gap).toBeLessThanOrEqual(1);
  const bottom = await transcript.evaluate((node) => node.scrollTop);
  await input.pressSequentially(" abc");
  await expect.poll(gap).toBeLessThanOrEqual(1);
  expect(await transcript.evaluate((node) => node.scrollTop)).toBeCloseTo(
    bottom,
    0,
  );

  await input.fill(
    Array.from({ length: 12 }, (_, index) => `Line ${index}`).join("\n"),
  );
  await expect.poll(height).toBe(180);
  await expect.poll(gap).toBeLessThanOrEqual(1);
  await input.pressSequentially(" more");
  await expect.poll(gap).toBeLessThanOrEqual(1);
  await input.fill("Short");
  await expect.poll(height).toBe(56);
  await expect.poll(gap).toBeLessThanOrEqual(1);

  await input.fill(
    "Keep the discussion visible as this draft wraps onto a few more lines while I review the response.",
  );
  await expect.poll(height).toBeGreaterThan(56);
  await page.getByRole("button", { name: "Hide changes", exact: true }).click();
  await expect.poll(height).toBe(56);
  await expect.poll(gap).toBeLessThanOrEqual(1);
  await page.getByRole("button", { name: "Show changes", exact: true }).click();
  await expect.poll(height).toBeGreaterThan(56);
  await expect.poll(gap).toBeLessThanOrEqual(1);

  await page.getByRole("button", { name: "Load earlier messages" }).click();
  await expect(page.locator(".conversation-entry")).toHaveCount(100);
  await transcript.evaluate((node) => {
    node.scrollTop = 250;
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  await input.fill("First\nSecond\nThird\nFourth");
  await input.pressSequentially(" abc");
  expect(await transcript.evaluate((node) => node.scrollTop)).toBe(250);
  await input.fill("Short");
  expect(await transcript.evaluate((node) => node.scrollTop)).toBe(250);
});

for (const provider of ["codex", "opencode"]) {
  test(`${provider} recalls the last sent message with Up Arrow without replacing drafts or crossing sessions`, async ({
    page,
  }) => {
    const oldPrompt =
      provider === "codex"
        ? "Make the workspace darker and easier to use."
        : "Review the OpenCode changes.\nKeep the reply short.";
    const title =
      provider === "codex"
        ? "Refine the session workspace"
        : "OpenCode session";
    const id = provider === "codex" ? "thread-a" : "ses-a";
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await page.evaluate(
        (text) =>
          (window as any).fixture.setOpenHistory([
            { id: "old-user", type: "user", text },
            {
              id: "old-answer",
              type: "assistant",
              content: [{ type: "text", text: "Ready to review." }],
            },
          ]),
        oldPrompt,
      );
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(page.locator(".user-text").last()).toHaveText(oldPrompt);
    await input.press("Shift+ArrowUp");
    await expect(input).toHaveValue("");
    await input.dispatchEvent("keydown", {
      key: "ArrowUp",
      isComposing: true,
    });
    await expect(input).toHaveValue("");
    await input.press("ArrowUp");
    await expect(input).toHaveValue(oldPrompt);
    expect(
      await input.evaluate((node: HTMLTextAreaElement) => node.selectionStart),
    ).toBe(oldPrompt.length);
    await input.fill("An unsent draft\nSecond line");
    await input.press("ArrowUp");
    await expect(input).toHaveValue("An unsent draft\nSecond line");

    const prompt = "Please refine this change.\nKeep the compact layout.";
    await input.fill(prompt);
    await input.press("Enter");
    await expect(input).toHaveValue("");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await expect
      .poll(() =>
        page.evaluate(
          (provider) =>
            (window as any).fixture.calls.filter(
              (call: any) => call.command === `${provider}_send_prompt`,
            ).length,
          provider,
        ),
      )
      .toBe(1);
    await input.press("ArrowUp");
    await expect(input).toHaveValue(prompt);
    await expect(input).toBeFocused();
    await page.screenshot({
      path: `test-results/${provider}-message-contrast.png`,
    });
    await page.setViewportSize({ width: 800, height: 620 });
    await page.screenshot({
      path: `test-results/${provider}-message-contrast-minimum.png`,
    });
    await page.getByRole("button", { name: /New session/ }).click();
    await input.press("ArrowUp");
    await expect(input).toHaveValue("");
    await page.getByRole("button", { name: title, exact: true }).click();
    await expect(input).toHaveValue(prompt);
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.idle",
      provider === "codex"
        ? { threadId: id, turn: { id: "turn-new", status: "completed" } }
        : { sessionID: id, outcome: "succeeded" },
      provider,
    );
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toHaveCount(0);
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.fill("This send will fail");
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await input.fill("");
    await input.press("ArrowUp");
    await expect(input).toHaveValue(prompt);
  });
}

test("first send creates one session; failure retains a retryable draft; generation permits drafting", async ({
  page,
}) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await choose(page, "Model", "Test model provider/model");
  await input.fill("Implement the redesign");
  await page.evaluate(() => (window as any).fixture.failSend());
  await input.press("Enter");
  await expect(page.getByRole("alert")).toContainText("Connection refused");
  await expect(input).toHaveValue("Implement the redesign");
  await input.press("Enter");
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toBeVisible();
  await input.fill("Next task");
  await input.press("Shift+Enter");
  await input.press("Enter");
  await expect(input).toHaveValue("Next task\n");
  const calls = await page.evaluate(() => (window as any).fixture.calls);
  expect(
    calls.filter((call: any) => call.command === "codex_start_session"),
  ).toHaveLength(1);
  expect(
    calls.find((call: any) => call.command === "codex_start_session").args
      .model,
  ).toBe("provider/model");
  expect(
    calls.filter((call: any) => call.command === "codex_send_prompt"),
  ).toHaveLength(2);
  await page.getByRole("button", { name: "Stop generation" }).click();
  await expect(
    page.getByRole("button", { name: "Send message", exact: true }),
  ).toBeVisible();
  await expect(input).toHaveValue("Next task\n");
});

test("session drafts, expanded tools, search, and navigation survive switching", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Draft A");
  await page.getByRole("button", { name: /Run pnpm test/ }).click();
  await page
    .getByRole("button", { name: /Review the provider adapters/ })
    .click();
  await expect(input).toHaveValue("");
  await input.fill("Draft B");
  await page.getByRole("button", { name: "Changes", exact: true }).click();
  await page
    .getByRole("button", { name: /Refine the session workspace/ })
    .click();
  await expect(input).toHaveValue("Draft A");
  await expect(
    page.getByText("14 tests passed", { exact: true }),
  ).toBeVisible();
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("textbox", { name: "Search sessions" }),
  ).toBeFocused();
  await page.getByRole("textbox", { name: "Search sessions" }).fill("adapters");
  await expect(page.locator(".session-item")).toHaveCount(1);
  await page.keyboard.press("Meta+n");
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("");
  await expect(page.locator(".session-item")).toHaveCount(2);
});

test("disconnects reconnect cached sessions and background approvals open their session", async ({
  page,
}) => {
  await desktop(page);
  await emit(page, "codex/processExited", {});
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  const resumes = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "codex_resume_session",
    ),
  );
  expect(resumes).toHaveLength(2);
  await page.evaluate(() =>
    (window as any).fixture.emit("approval-requests", [
      {
        provider: "codex",
        projectPath: "/work/caelum",
        sessionId: "thread-b",
        requestId: "approval-1",
        summary: "Run git status?",
        description: "git status",
      },
    ]),
  );
  await page.getByRole("button", { name: "1 pending approvals" }).click();
  await expect(
    page.getByRole("region", { name: "Approval required" }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Allow once" }).click();
  await expect(
    page.getByRole("region", { name: "Approval required" }),
  ).toHaveCount(0);
});

test("paginated history keeps a bounded transcript and does not follow while reading", async ({
  page,
}) => {
  await desktop(page);
  await page.getByRole("button", { name: "Load earlier messages" }).click();
  await expect(page.locator(".conversation-entry")).toHaveCount(100);
  await page.getByRole("button", { name: "Load earlier messages" }).click();
  await expect(
    page.getByText("Earlier message 0", { exact: true }),
  ).toBeVisible();
  await page.locator(".transcript").evaluate((node) => {
    node.scrollTop = 250;
  });
  const before = await page
    .locator(".transcript")
    .evaluate((node) => node.scrollTop);
  await page
    .getByRole("button", { name: /Review the provider adapters/ })
    .click();
  await page
    .getByRole("button", { name: /Refine the session workspace/ })
    .click();
  expect(
    await page.locator(".transcript").evaluate((node) => node.scrollTop),
  ).toBe(before);
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "live",
    delta: "New answer",
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest" }),
  ).toBeVisible();
  expect(
    await page.locator(".transcript").evaluate((node) => node.scrollTop),
  ).toBe(before);
  await page.getByRole("button", { name: "Jump to latest" }).click();
  await expect(page.getByText("New answer", { exact: true })).toBeInViewport();
});

test("history arriving during streaming preserves new live content", async ({
  page,
}) => {
  await desktop(page);
  await page.evaluate(() => (window as any).fixture.holdHistory());
  await page
    .getByRole("button", { name: /Review the provider adapters/ })
    .click();
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-b",
    itemId: "live-b",
    delta: "Arrived during history",
  });
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await expect(
    page.getByText("Arrived during history", { exact: true }),
  ).toHaveCount(1);
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
});

async function contextMenu(page: Page) {
  const menu = page.getByRole("menu", { name: "Context menu", exact: true });
  await expect(menu).toBeVisible();
  expect(
    await page.evaluate(() => (window as any).fixture.contextPrevented()),
  ).toBe(true);
  const items = await menu
    .locator('[role="menuitem"], [role="separator"]')
    .evaluateAll((nodes) =>
      nodes.map((node) => ({
        text:
          node.getAttribute("role") === "separator"
            ? "Separator"
            : node.textContent!,
        enabled: !(node as HTMLButtonElement).disabled,
      })),
    );
  expect(
    items.some((item) => /inspect|reload|back|forward/i.test(item.text)),
  ).toBe(false);
  return items;
}

async function pickMenu(page: Page, text: string) {
  await page.getByRole("menuitem", { name: text, exact: true }).click();
  await expect(page.getByRole("menu", { name: "Context menu" })).toHaveCount(0);
}

async function copiedText(page: Page) {
  return page.evaluate(() => (window as any).fixture.clipboard());
}

test("context menus support keyboard navigation, viewport edges, themes and dismissal", async ({
  page,
}, testInfo) => {
  await desktop(page);
  const project = page.getByRole("button", { name: "caelum", exact: true });
  const menu = page.getByRole("menu", { name: "Context menu" });
  await project.focus();
  await project.press("Shift+F10");
  await expect(
    page.getByRole("menuitem", { name: "Open project", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("End");
  await expect(
    page.getByRole("menuitem", { name: "Close project", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByRole("menuitem", { name: "Open project", exact: true }),
  ).toBeFocused();
  await page.keyboard.type("cop");
  await expect(
    page.getByRole("menuitem", { name: "Copy project path", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect.poll(() => copiedText(page)).toBe("/work/caelum");
  await expect(menu).toHaveCount(0);
  await expect(project).toBeFocused();
  await project.press("ContextMenu");
  await page.screenshot({ path: testInfo.outputPath("context-menu-dark.png") });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = "light";
  });
  await expect(menu).toHaveCSS("background-color", "rgb(255, 255, 255)");
  await page.screenshot({
    path: testInfo.outputPath("context-menu-light.png"),
  });
  await page.keyboard.press("Escape");
  await expect(project).toBeFocused();
  await project.press("Shift+F10");
  await page.keyboard.press("Tab");
  await expect(menu).toHaveCount(0);
  await expect(project).toBeFocused();

  // Exercise pointer placement separately from keyboard invocation, and wait
  // for the edge position before clicking the project outside the menu.
  await page.mouse.click(1279, 799, { button: "right" });
  await expect(menu).toBeVisible();
  await expect
    .poll(async () => {
      const bounds = await menu.boundingBox();
      return (
        bounds &&
        Math.abs(bounds.x + bounds.width - 1272) < 1 &&
        Math.abs(bounds.y + bounds.height - 792) < 1
      );
    })
    .toBe(true);
  const bounds = (await menu.boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(8);
  expect(bounds.y).toBeGreaterThanOrEqual(8);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(1272);
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(792);
  await project.click();
  await expect(menu).toHaveCount(0);
  await project.press("Shift+F10");
  await page.locator(".transcript").dispatchEvent("scroll");
  await expect(menu).toBeVisible();
  await page.locator(".project-list").dispatchEvent("wheel");
  await expect(menu).toHaveCount(0);
  await project.press("Shift+F10");
  await page.setViewportSize({ width: 1000, height: 700 });
  await expect(menu).toHaveCount(0);
  await project.press("Shift+F10");
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(menu).toHaveCount(0);
});

test("context menus edit the original selection with undo and reject delayed paste into newer drafts", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const edit = async (action: string) => {
    await input.press("Shift+F10");
    await pickMenu(page, action);
  };
  // Seed an existing draft without an initial typing undo group (WebKit coalesces it).
  await input.evaluate((node: HTMLTextAreaElement) => {
    Object.getOwnPropertyDescriptor(
      HTMLTextAreaElement.prototype,
      "value",
    )!.set!.call(node, "Keep this draft");
    node.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await input.evaluate((node: HTMLTextAreaElement) =>
    node.setSelectionRange(5, 9),
  );
  await edit("Cut");
  await expect.poll(() => copiedText(page)).toBe("this");
  await expect(input).toHaveValue("Keep  draft");
  await edit("Undo");
  await expect(input).toHaveValue("Keep this draft");
  await edit("Redo");
  await expect(input).toHaveValue("Keep  draft");
  await page.evaluate(() =>
    (window as any).fixture.setClipboard("another\nlonger"),
  );
  await edit("Paste");
  await expect(input).toHaveValue("Keep another\nlonger draft");
  await edit("Undo");
  await expect(input).toHaveValue("Keep  draft");
  await edit("Select all");
  expect(
    await input.evaluate((node: HTMLTextAreaElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([0, 11]);
  await page.evaluate(() => (window as any).fixture.failCopy());
  await edit("Cut");
  await expect(
    page.getByRole("alert").filter({ hasText: "Clipboard unavailable" }),
  ).toBeVisible();
  await expect(input).toHaveValue("Keep  draft");

  await page.evaluate(() =>
    (window as any).fixture.hold("plugin:clipboard-manager|read_text"),
  );
  await edit("Paste");
  await input.fill("A newer edit");
  await page.evaluate(() =>
    (window as any).fixture.release("plugin:clipboard-manager|read_text"),
  );
  await expect(input).toHaveValue("A newer edit");
  await page.evaluate(() =>
    (window as any).fixture.hold("plugin:clipboard-manager|read_text"),
  );
  await edit("Paste");
  await page.keyboard.press("Meta+n");
  await input.fill("New session draft");
  await page.evaluate(() =>
    (window as any).fixture.release("plugin:clipboard-manager|read_text"),
  );
  await expect(input).toHaveValue("New session draft");
});

test("context menus edit modal fields and respect password, readonly and disabled inputs", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const project = page.getByRole("button", { name: "caelum", exact: true });
  await project.click({ button: "right" });
  await pickMenu(page, "Create worktree");
  const dialog = page.getByRole("dialog", {
    name: "Create worktree",
    exact: true,
  });
  const branch = dialog.getByRole("textbox", { name: "New branch" });
  await branch.fill("feature/search");
  await branch.evaluate((node: HTMLInputElement) =>
    node.setSelectionRange(8, 14),
  );
  await branch.press("Shift+F10");
  await expect(
    dialog.getByRole("menu", { name: "Context menu" }),
  ).toBeVisible();
  await pickMenu(page, "Copy");
  await expect.poll(() => copiedText(page)).toBe("search");
  await expect(branch).toBeFocused();
  expect(
    await branch.evaluate((node: HTMLInputElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([8, 14]);
  await branch.press("Shift+F10");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  expect(
    await branch.evaluate((node: HTMLInputElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([8, 14]);
  await branch.press("Shift+F10");
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByRole("menuitem", { name: "Cut", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(project).toBeFocused();

  await page.getByRole("button", { name: "Model", exact: true }).click();
  const picker = page.getByRole("dialog", {
    name: "Choose model",
    exact: true,
  });
  const modelSearch = picker.getByRole("combobox", { name: "Search models" });
  await modelSearch.fill("fast");
  await modelSearch.press("ControlOrMeta+a");
  await modelSearch.press("Shift+F10");
  await expect(
    picker.getByRole("menu", { name: "Context menu" }),
  ).toBeVisible();
  await pickMenu(page, "Copy");
  await expect.poll(() => copiedText(page)).toBe("fast");
  await expect(modelSearch).toBeFocused();
  await modelSearch.press("Escape");
  await expect(picker).toHaveCount(0);

  const search = page.locator('input[aria-label="Search sessions"]');
  await search.fill("secret");
  await search.evaluate((node: HTMLInputElement) => {
    node.type = "password";
    node.select();
  });
  await search.press("Shift+F10");
  await expect(
    page.getByRole("menuitem", { name: "Cut", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("menuitem", { name: "Copy", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("menuitem", { name: "Paste", exact: true }),
  ).toBeEnabled();
  await page.keyboard.press("Escape");
  await search.evaluate((node: HTMLInputElement) => {
    node.type = "text";
    node.readOnly = true;
    node.select();
  });
  await search.press("Shift+F10");
  await expect(page.getByRole("menuitem")).toHaveText(["Copy", "Select all"]);
  await page.keyboard.press("Escape");
  await search.evaluate((node: HTMLInputElement) => {
    node.disabled = true;
  });
  await search.dispatchEvent("contextmenu");
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("context menus offer editing and precise message, code, link and tool actions", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft");
  await input.evaluate((node: HTMLTextAreaElement) =>
    node.setSelectionRange(5, 9),
  );
  await input.press("Shift+F10");
  await expect
    .poll(async () => (await contextMenu(page)).map((item) => item.text))
    .toEqual([
      "Undo",
      "Redo",
      "Separator",
      "Cut",
      "Copy",
      "Paste",
      "Separator",
      "Select all",
    ]);
  await pickMenu(page, "Copy");
  await expect.poll(() => copiedText(page)).toBe("this");
  await expect(input).toBeFocused();
  expect(
    await input.evaluate((node: HTMLTextAreaElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([5, 9]);

  const message = page.locator(".user-text").first();
  const prompt = await message.innerText();
  await message.click({ button: "right" });
  await pickMenu(page, "Copy message");
  await expect.poll(() => copiedText(page)).toBe(prompt);
  await expect(input).toHaveValue("Keep this draft");
  await message.click({ button: "right" });
  await pickMenu(page, "Add to draft");
  await expect(input).toHaveValue(`Keep this draft\n\n${prompt}`);
  await expect(input).toBeFocused();

  await page.locator(".code-block pre").click({ button: "right" });
  await pickMenu(page, "Copy code");
  await expect.poll(() => copiedText(page)).toBe("const theme = 'near-black';");
  const link = page.getByRole("link", { name: "documentation", exact: true });
  await link.click({ button: "right" });
  await pickMenu(page, "Copy link address");
  await expect.poll(() => copiedText(page)).toBe("https://example.com/docs");
  await link.click({ button: "right" });
  await pickMenu(page, "Open link");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe("https://example.com/docs");

  const selected = page.locator(".markdown strong").first();
  await selected.evaluate((node) => {
    const range = document.createRange();
    range.selectNodeContents(node);
    window.getSelection()!.removeAllRanges();
    window.getSelection()!.addRange(range);
  });
  await selected.dispatchEvent("contextmenu", { bubbles: true });
  await pickMenu(page, "Copy selection");
  await expect.poll(() => copiedText(page)).toBe("more room");
  await page.evaluate(() => window.getSelection()!.removeAllRanges());
  await page
    .getByRole("button", { name: /Run pnpm test/ })
    .click({ button: "right" });
  await pickMenu(page, "Expand tool");
  await page
    .getByText("14 tests passed", { exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Copy output");
  await expect.poll(() => copiedText(page)).toBe("14 tests passed");
});

test("context menus act on the clicked project or session and reject stale actions after navigation", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Session A draft");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const project = page
    .getByRole("navigation", { name: "Open projects" })
    .getByRole("button", { name: "caelum", exact: true });
  await project.click({ button: "right" });
  await pickMenu(page, "Copy project path");
  await expect.poll(() => copiedText(page)).toBe("/work/caelum");
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await project.click({ button: "right" });
  await pickMenu(page, "Open project");
  await expect(input).toHaveValue("Session A draft");
  const other = page.getByRole("button", {
    name: "Review the provider adapters",
    exact: true,
  });
  await other.click({ button: "right" });
  await pickMenu(page, "Copy session ID");
  await expect.poll(() => copiedText(page)).toBe("thread-b");
  await expect(input).toHaveValue("Session A draft");
  await other.click({ button: "right" });
  await pickMenu(page, "Archive session");
  await expect(other).toHaveCount(0);
  await expect(input).toHaveValue("Session A draft");
  await input.press("Enter");
  const current = page.getByRole("button", {
    name: "Refine the session workspace",
    exact: true,
  });
  await current.click({ button: "right" });
  await expect
    .poll(
      async () =>
        (await contextMenu(page)).find(
          (item) => item.text === "Archive session",
        )?.enabled,
    )
    .toBe(false);
  await page.getByRole("button", { name: "Stop generation" }).click();
  await page.locator(".user-text").first().click({ button: "right" });
  const stale = await page
    .getByRole("menuitem", { name: "Add to draft", exact: true })
    .elementHandle();
  await page.keyboard.press("Meta+n");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await input.fill("New draft");
  await stale!.evaluate((node: HTMLElement) => node.click());
  await expect(input).toHaveValue("New draft");

  await project.click({ button: "right" });
  await current.click();
  await expect(page.getByRole("menu")).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.filter((call: any) =>
          call.command.startsWith("plugin:menu|"),
        ).length,
    ),
  ).toBe(0);
});

test("file and diff context menus copy exact content, reveal paths, and use preview actions", async ({
  page,
}) => {
  await desktop(page);
  const css = page.getByRole("button", {
    name: "src/styles.css, Staged modified",
    exact: true,
  });
  await css.click({ button: "right" });
  await pickMenu(page, "Copy relative path");
  await expect.poll(() => copiedText(page)).toBe("src/styles.css");
  await expect(
    page.getByRole("button", {
      name: "src/App.tsx, Unstaged modified",
      exact: true,
    }),
  ).toHaveAttribute("aria-pressed", "true");
  await css.click({ button: "right" });
  await pickMenu(page, "Review changes");
  const patch = page.getByRole("region", { name: "Staged diff", exact: true });
  const text = await patch.locator("pre").textContent();
  await patch.locator("pre").click({ button: "right" });
  await pickMenu(page, "Copy staged diff");
  await expect.poll(() => copiedText(page)).toBe(text);

  await page.getByRole("button", { name: "Files", exact: true }).click();
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Expand folder");
  const file = page.getByRole("treeitem", {
    name: "App.tsx, file",
    exact: true,
  });
  await file.click({ button: "right" });
  await pickMenu(page, "Copy full path");
  await expect.poll(() => copiedText(page)).toBe("/work/caelum/src/App.tsx");
  await file.click({ button: "right" });
  const reveal = (await contextMenu(page)).find((item) =>
    item.text.startsWith("Reveal in"),
  )!.text;
  await pickMenu(page, reveal);
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter(
              (call: any) =>
                call.command === "plugin:opener|reveal_item_in_dir",
            )
            .at(-1)?.args.paths,
      ),
    )
    .toEqual(["/work/caelum/src/App.tsx"]);
  await file.click({ button: "right" });
  await pickMenu(page, "Open file");
  const toolbar = page.locator(".file-editor-toolbar");
  await toolbar.click({ button: "right" });
  await pickMenu(page, "Copy file contents");
  await expect
    .poll(() => copiedText(page))
    .toBe("export const ready = true;\n");
  const preview = page.getByRole("textbox", {
    name: "Read-only preview of src/App.tsx",
    exact: true,
  });
  await preview.click({ button: "right" });
  await pickMenu(page, "Copy file preview");
  await expect
    .poll(() => copiedText(page))
    .toBe("export const ready = true;\n");
  await preview.press("ControlOrMeta+a");
  await preview.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe("export const ready = true;\n");
  await preview.press("Shift+F10");
  await pickMenu(page, "Add selection to prompt");
  await expect(attachedContext(page)).toContainText("src/App.tsx:1");
  await page.evaluate(() => (window as any).fixture.failCopy());
  await toolbar.click({ button: "right" });
  await pickMenu(page, "Copy file contents");
  await expect(
    page.getByRole("alert").filter({ hasText: "Clipboard unavailable" }),
  ).toBeVisible();
});

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} slash new opens its project draft without sending or interrupting the source session`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("A draft already waiting");
    await setHistory(page, provider, [], true);
    const title =
      provider === "codex"
        ? "Refine the session workspace"
        : "OpenCode session";
    const row = sessionRow(page, title);
    await row.getByRole("button", { name: title, exact: true }).click();
    await findProjectFile(page, "src/App.tsx");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    await choose(page, "Model", "Fast model provider/fast");
    await input.fill("/new unexpected");
    await input.press("Enter");
    await expect(
      page
        .getByRole("alert")
        .filter({ hasText: "/new does not take arguments." }),
    ).toBeVisible();
    await expect(input).toHaveValue("/new unexpected");
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await input.fill("/new");
    await expect(page.getByRole("option", { name: /^\/new / })).toHaveCount(1);
    await expect(
      page.getByRole("button", { name: "Stop generation", exact: true }),
    ).toBeVisible();
    await input.press("Enter");
    await expect(
      page.getByRole("heading", { name: "New session", exact: true }),
    ).toBeVisible();
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("A draft already waiting");
    await expect(attachedContext(page)).toHaveCount(0);
    await expect(row.locator(".running-dot")).toBeVisible();
    await expect(
      page.getByRole("button", { name: "Provider", exact: true }),
    ).toContainText(provider === "codex" ? "Codex" : "OpenCode");
    await row.getByRole("button", { name: title, exact: true }).click();
    await expect(input).toBeEmpty();
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Fast model");
    await expect(
      page.getByRole("button", { name: "Stop generation", exact: true }),
    ).toBeVisible();
    // Button submission uses the same local route, even if discovery fails.
    await page.evaluate(() =>
      (window as any).fixture.setCommandError(
        "opencode_commands",
        "Discovery unavailable",
      ),
    );
    await input.fill("/new ");
    await page
      .getByRole("button", { name: "Run command", exact: true })
      .click();
    await expect(input).toHaveValue("A draft already waiting");
    await input.fill("/ne");
    await input.press("Tab");
    await expect(input).toHaveValue("/new ");
    await input.press("Enter");
    await expect(input).toBeEmpty();
    await expect(input).toBeFocused();
    expect(
      await page.evaluate(() =>
        (window as any).fixture.calls.filter((call: any) =>
          /_(start_session|send_prompt|run_command|cancel_turn)$/.test(
            call.command,
          ),
        ),
      ),
    ).toHaveLength(0);
    await input.fill("Start the new task");
    await input.press("Enter");
    await expect
      .poll(() =>
        page.evaluate(
          (provider) =>
            (window as any).fixture.calls.filter(
              (call: any) => call.command === `${provider}_send_prompt`,
            ).length,
          provider,
        ),
      )
      .toBe(1);
    expect(
      await page.evaluate(
        (provider) =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === `${provider}_start_session`,
          ),
        provider,
      ),
    ).toHaveLength(1);
  });
}

test("slash picker discovers native commands, preserves agent shortcuts and submits one new goal", async ({
  page,
}) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/");
  await expect(
    page.getByRole("listbox", { name: "Slash commands" }).getByRole("option"),
  ).toHaveCount(1);
  await expect(page.getByRole("option", { name: /^\/new / })).toBeVisible();
  await input.press("Escape");
  await choose(page, "Provider", "OpenCode");
  await input.fill("/");
  const list = page.getByRole("listbox", { name: "Slash commands" });
  await expect(list.getByRole("option", { name: /^\/goal/ })).toBeVisible();
  await expect(
    list.getByRole("option", { name: /^\/team\/review/ }),
  ).toBeVisible();
  await input.press("ArrowDown");
  await input.press("ArrowDown");
  await expect(list.getByRole("option", { name: /^\/init/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await input.press("Shift+Tab");
  await expect(input).toBeFocused();
  await expect(
    page.getByRole("button", { name: "Build", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await input.press("Escape");
  await expect(list).toHaveCount(0);
  await input.fill("/go");
  await expect(list.getByRole("option")).toHaveCount(1);
  await input.press("Tab");
  await expect(input).toHaveValue("/goal ");
  await expect(input).toBeFocused();
  await expect(list).toHaveCount(0);
  await choose(page, "Model", "Fast model provider/fast");
  await choose(page, "Effort", "Low");
  await page.evaluate(() =>
    (window as any).fixture.setCommandResult({
      text: "Native goal acknowledgement",
      working: false,
    }),
  );
  await input.fill("/goal Finish the task\n  Keep Unicode 🚀\n");
  await input.press("Enter");
  await expect(
    page.getByText("Native goal acknowledgement", { exact: true }),
  ).toBeVisible();
  await expect(input).toBeEmpty();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter((call: any) =>
      [
        "opencode_start_session",
        "opencode_run_command",
        "opencode_send_prompt",
      ].includes(call.command),
    ),
  );
  expect(calls.map((call: any) => call.command)).toEqual([
    "opencode_start_session",
    "opencode_run_command",
  ]);
  expect(calls[0].args).toMatchObject({
    projectPath: "/work/caelum",
    model: "provider/fast",
    agent: "build",
  });
  expect(calls[1].args).toEqual({
    projectPath: "/work/caelum",
    sessionId: "ses-new",
    model: null,
    effort: "low",
    command: {
      name: "goal",
      text: "Finish the task\n  Keep Unicode 🚀\n",
      files: [],
    },
  });
  await expect(page.locator(".conversation-entry.user")).toHaveCount(1);
  await page.getByRole("button", { name: /New session/ }).click();
  await expect(input).toBeEmpty();
});

test("slash lookup failures keep drafts intact and never create a session for unknown commands", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/missing argument");
  await input.press("Enter");
  await expect(
    page
      .getByRole("alert")
      .filter({ hasText: "Unknown OpenCode command: /missing" }),
  ).toBeVisible();
  await expect(input).toHaveValue("/missing argument");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_start_session",
      ),
    ),
  ).toHaveLength(0);
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "opencode_commands",
      "Command discovery unavailable",
    ),
  );
  await input.fill("/goal pause");
  for (let attempt = 0; attempt < 2; attempt++) {
    await input.press("Enter");
    await expect(
      page
        .getByRole("alert")
        .filter({ hasText: "Command discovery unavailable" }),
    ).toBeVisible();
    await expect(input).toHaveValue("/goal pause");
    await expect(
      page.getByRole("button", { name: "Run command", exact: true }),
    ).toBeEnabled();
  }
  await input.fill("/");
  await expect(
    page.getByRole("button", { name: "Retry commands", exact: true }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("opencode_commands", null),
  );
  await page
    .getByRole("button", { name: "Retry commands", exact: true })
    .click();
  await expect(page.getByRole("option", { name: /^\/goal/ })).toBeVisible();
  await input.fill("/goal clear");
  await input.press("Enter");
  await expect(input).toBeEmpty();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === "opencode_run_command",
          ).length,
      ),
    )
    .toBe(1);
});

test("slash goal controls run during execution and failed commands preserve the active Stop control", async ({
  page,
}) => {
  await desktop(page, null);
  await setHistory(page, "opencode", [], true);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const stop = page.getByRole("button", {
    name: "Stop generation",
    exact: true,
  });
  await expect(stop).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("opencode_run_command", "Command failed");
    fixture.hold("opencode_run_command");
  });
  await input.fill("/goal pause");
  await input.press("Enter");
  await expect(input).toBeEmpty();
  await input.press("Enter");
  await expect(stop).toBeVisible();
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_run_command",
      ),
    ),
  ).toHaveLength(1);
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_run_command"),
  );
  await expect(
    page.getByRole("alert").filter({ hasText: "Command failed" }),
  ).toBeVisible();
  await expect(stop).toBeVisible();
  await expect(input).toHaveValue("/goal pause");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("opencode_run_command", null),
  );
  for (const [action, working] of [
    ["pause", false],
    ["resume", true],
    ["clear", false],
  ] as const) {
    await page.evaluate(
      ({ action, working }) =>
        (window as any).fixture.setCommandResult({
          text: `Goal ${action}`,
          working,
        }),
      { action, working },
    );
    await input.fill(`/goal ${action}`);
    await input.press("Enter");
    await expect(
      page.getByText(`Goal ${action}`, { exact: true }),
    ).toBeVisible();
    if (working) await expect(stop).toBeVisible();
    else await expect(stop).toHaveCount(0);
  }
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_send_prompt",
      ),
    ),
  ).toHaveLength(0);
});

test("slash custom commands retain file snapshots on failure and reconcile native agent and model overrides", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.setProjectFile(
      "src/App.tsx",
      "// café 🚀\r\nexport const ready = true;\r\n",
    ),
  );
  await findProjectFile(page, "src/App.tsx");
  await page
    .locator(".file-editor-toolbar")
    .getByRole("button", { name: "Add to prompt", exact: true })
    .click();
  await choose(page, "Model", "Fast model provider/fast");
  await choose(page, "Effort", "Low");
  await page.getByRole("button", { name: "Plan", exact: true }).click();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "opencode_run_command",
      "Native command unavailable",
    ),
  );
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/team");
  await expect(
    page.getByRole("option", { name: /^\/team\/review/ }),
  ).toBeVisible();
  await input.press("Enter");
  await expect(
    page.getByRole("alert").filter({ hasText: "Native command unavailable" }),
  ).toBeVisible();
  await expect(input).toHaveValue("/team/review");
  await expect(attachedContext(page)).toContainText("src/App.tsx");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("opencode_run_command", null);
    fixture.setCommandResult({
      text: "Review the captured file",
      working: false,
      agent: "reviewer",
      model: "provider/plain",
    });
  });
  await input.press("Enter");
  await expect(
    page.getByText("Review the captured file", { exact: true }),
  ).toBeVisible();
  await expect(input).toBeEmpty();
  await expect(attachedContext(page)).toHaveCount(0);
  await expect(
    page.getByRole("status", { name: "Agent selection", exact: true }),
  ).toHaveText("Current: Reviewer");
  await expect(
    page.getByRole("button", { name: "Model", exact: true }),
  ).toContainText("Simple model");
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter((call: any) =>
      ["opencode_select_agent", "opencode_run_command"].includes(call.command),
    ),
  );
  expect(calls.map((call: any) => call.command)).toEqual([
    "opencode_select_agent",
    "opencode_run_command",
    "opencode_run_command",
  ]);
  expect(calls[1].args).toEqual(calls[2].args);
  expect(calls[2].args).toMatchObject({
    model: "provider/fast",
    effort: "low",
    command: { name: "team/review", text: "" },
  });
  const file = calls[2].args.command.files[0];
  expect(file.name).toBe("src/App.tsx");
  expect(Buffer.from(file.uri.split(",")[1], "base64").toString("utf8")).toBe(
    "// café 🚀\r\nexport const ready = true;\r\n",
  );
});

test("slash lookup locks submission, preserves newer drafts and stays scoped through project navigation", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.hold("opencode_commands");
    fixture.setCommandResult({ text: "Background goal", working: false });
    fixture.setCommands("/work/other", [
      { name: "other-only", description: "Another project's command" },
    ]);
  });
  await input.fill("/goal Background objective");
  await input.press("Enter");
  await expect(
    page.getByRole("button", { name: "Run command", exact: true }),
  ).toBeDisabled();
  await input.press("Enter");
  await input.fill("A newer draft");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await choose(page, "Provider", "OpenCode");
  await input.fill("/");
  await expect(page.getByRole("option", { name: /^\/goal/ })).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_commands"),
  );
  await expect(
    page.getByRole("option", { name: /^\/other-only/ }),
  ).toBeVisible();
  await expect(page.getByRole("option", { name: /^\/goal/ })).toHaveCount(0);
  await input.fill("Other project draft");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === "opencode_run_command",
          ).length,
      ),
    )
    .toBe(1);
  await expect(input).toHaveValue("Other project draft");
  await page
    .getByRole("navigation", { name: "Open projects" })
    .getByRole("button", { name: "caelum", exact: true })
    .click();
  await expect(input).toHaveValue("A newer draft");
  await expect(
    page.getByText("Background goal", { exact: true }),
  ).toBeVisible();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "opencode_run_command",
    ),
  );
  expect(calls[0].args).toMatchObject({
    projectPath: "/work/caelum",
    sessionId: "ses-a",
    command: { name: "goal", text: "Background objective" },
  });
});

test("slash usage identifies OpenCode Go, keeps attachments and reported limits through refresh failures", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await findProjectFile(page, "src/App.tsx");
  await page
    .locator(".file-editor-toolbar")
    .getByRole("button", { name: "Add to prompt", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/usage unexpected");
  await input.press("Enter");
  await expect(
    page
      .getByRole("alert")
      .filter({ hasText: "/usage does not take arguments." }),
  ).toBeVisible();
  await expect(input).toHaveValue("/usage unexpected");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("opencode_usage_limits", "Usage unavailable");
    fixture.hold("opencode_usage_limits");
  });
  await input.fill("/usage");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", {
    name: "OpenCode Go usage",
    exact: true,
  });
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByText("Loading OpenCode Go usage…", { exact: true }),
  ).toBeVisible();
  await expect(dialog).toHaveAccessibleDescription(
    "Subscription allowances. Codex usage isn’t included.",
  );
  await expect(dialog).toContainText("No context usage reported");
  await page.keyboard.press("Meta+n");
  await page.keyboard.press("Meta+p");
  await expect(dialog).toBeVisible();
  await expect(
    page.getByRole("combobox", { name: "Find project file" }),
  ).toHaveAttribute("aria-expanded", "false");
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_usage_limits"),
  );
  await expect(dialog.getByRole("alert")).toContainText(
    "Error: Usage unavailable",
  );
  const resetsAt = Date.now() + 2 * 60 * 60 * 1000;
  const limits = {
    workspace: {
      id: "workspace",
      name: "My workspace",
      usageURL: "https://opencode.ai/usage",
    },
    go: {
      rolling: { percent: 42, limited: false, resetsAt },
      weekly: { percent: 20, limited: false, resetsAt: resetsAt + 86400000 },
      monthly: {
        percent: 100,
        limited: true,
        resetsAt: resetsAt + 86400000 * 20,
      },
    },
    budgets: [
      {
        id: "budget",
        limit: 25,
        spent: 4.5,
        scope: "combined",
        per: "set",
        coverage: "all",
        models: [],
        vendors: [],
        shared: true,
        picked: true,
        resetsAt: resetsAt + 86400000 * 20,
      },
    ],
  };
  await page.evaluate((limits) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("opencode_usage_limits", null);
    fixture.setUsageLimits(limits);
  }, limits);
  await dialog
    .getByRole("button", { name: "Retry usage", exact: true })
    .click();
  await expect(dialog).toContainText("My workspace");
  await expect(
    dialog.getByRole("progressbar", {
      name: "OpenCode Go rolling allowance used",
      exact: true,
    }),
  ).toHaveAttribute("value", "42");
  await expect(
    dialog
      .getByRole("region", { name: "OpenCode Go allowances", exact: true })
      .getByRole("progressbar"),
  ).toHaveCount(3);
  await expect(dialog).toContainText("58% remaining");
  await expect(dialog).toContainText("Limit reached");
  await expect(
    dialog.locator(".usage-allowance").first().locator("time"),
  ).toHaveText("Resets in 2h");
  await expect(
    dialog.locator(".usage-allowance").first().locator("time"),
  ).toHaveAttribute("datetime", new Date(resetsAt).toISOString());
  await expect(
    dialog.getByRole("region", {
      name: "OpenCode Console budgets",
      exact: true,
    }),
  ).toContainText("$4.50 / $25.00");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.hold("opencode_usage_limits");
    fixture.setCommandError("opencode_usage_limits", "Usage refresh failed");
  });
  await dialog
    .getByRole("button", { name: "Refresh usage", exact: true })
    .click();
  await expect(
    dialog.getByRole("button", { name: "Refreshing…", exact: true }),
  ).toBeDisabled();
  await expect(
    dialog.getByRole("progressbar", {
      name: "OpenCode Go rolling allowance used",
      exact: true,
    }),
  ).toHaveAttribute("value", "42");
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_usage_limits"),
  );
  await expect(dialog.getByRole("alert")).toContainText(
    "Showing the last reported usage.",
  );
  await expect(
    dialog.getByRole("progressbar", {
      name: "OpenCode Go rolling allowance used",
      exact: true,
    }),
  ).toHaveAttribute("value", "42");
  await page.evaluate((limits) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("opencode_usage_limits", null);
    fixture.setUsageLimits({
      ...limits,
      go: { ...limits.go, rolling: { ...limits.go.rolling, percent: 57 } },
    });
  }, limits);
  await dialog
    .getByRole("button", { name: "Retry usage", exact: true })
    .click();
  await expect(
    dialog.getByRole("progressbar", {
      name: "OpenCode Go rolling allowance used",
      exact: true,
    }),
  ).toHaveAttribute("value", "57");
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(dialog).toBeInViewport({ ratio: 1 });
  await expect(
    dialog.getByRole("region", { name: "OpenCode Go allowances", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  await expect(
    dialog.getByRole("button", { name: "Done", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(
    await dialog.evaluate((node) => node.scrollWidth <= node.clientWidth),
  ).toBe(true);
  await page.screenshot({ path: "test-results/slash-usage.png" });
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(input).toBeFocused();
  await expect(input).toBeEmpty();
  await expect(attachedContext(page)).toContainText("src/App.tsx");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        [
          "opencode_run_command",
          "opencode_send_prompt",
          "opencode_start_session",
        ].includes(call.command),
      ),
    ),
  ).toHaveLength(0);
  const settings = await openSettings(page);
  await settings.getByRole("radio", { name: "Light", exact: true }).check();
  await settings.getByRole("button", { name: "Done", exact: true }).click();
  await input.fill("/usage");
  await input.press("Enter");
  await expect(
    dialog.getByRole("progressbar", {
      name: "OpenCode Go rolling allowance used",
      exact: true,
    }),
  ).toHaveAttribute("value", "57");
  await page.screenshot({ path: "test-results/slash-usage-light.png" });
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
});

test("slash usage reports the measured session model and handles missing provider allowances", async ({
  page,
}) => {
  await desktop(page, null);
  await setHistory(
    page,
    "opencode",
    [
      {
        id: "usage",
        type: "assistant",
        time: { created: 100, completed: 200 },
        model: { providerID: "provider", id: "model" },
        tokens: {
          input: 80000,
          output: 10000,
          reasoning: 2000,
          cache: { read: 9000, write: 1000 },
        },
        content: [{ type: "text", text: "Measured response" }],
      },
    ],
    false,
  );
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await choose(page, "Model", "Fast model provider/fast");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/us");
  await expect(page.getByRole("option", { name: /^\/usage/ })).toBeVisible();
  await input.press("Enter");
  const dialog = page.getByRole("dialog", {
    name: "OpenCode Go usage",
    exact: true,
  });
  await expect(dialog).toContainText("100,000 / 200,000 tokens");
  await expect(dialog).toContainText("Test model");
  await expect(
    dialog.getByRole("region", {
      name: "OpenCode session context",
      exact: true,
    }),
  ).toContainText("50% of context");
  await expect(dialog).toContainText("No OpenCode Go allowances reported");
  await expect(
    dialog.getByRole("region", {
      name: "OpenCode Console budgets",
      exact: true,
    }),
  ).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setUsageLimits({
      workspace: { id: "console-only", usageURL: "https://opencode.ai/usage" },
      budgets: [
        {
          id: "budget",
          limit: 10,
          spent: 3.5,
          scope: "each",
          per: "model",
          coverage: "models",
          models: [
            {
              providerID: "provider",
              modelID: "model",
              name: "Test model",
              spent: 3.5,
            },
          ],
          vendors: [],
          shared: false,
          picked: true,
          resetsAt: Date.now() + 86400000,
        },
      ],
    }),
  );
  await dialog
    .getByRole("button", { name: "Refresh usage", exact: true })
    .click();
  await expect(dialog).toContainText("No OpenCode Go allowances reported");
  await expect(
    dialog.getByRole("region", {
      name: "OpenCode Console budgets",
      exact: true,
    }),
  ).toContainText("$3.50 / $10.00");
  await expect(
    dialog
      .getByRole("region", { name: "OpenCode Go allowances", exact: true })
      .getByRole("progressbar"),
  ).toHaveCount(0);
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Model", exact: true }),
  ).toContainText("Fast model");
});

test("Build and Plan are one-click session controls that distinguish the running agent from the next message", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const build = page.getByRole("button", {
    name: "Build",
    exact: true,
  });
  const plan = page.getByRole("button", { name: "Plan", exact: true });
  const status = page.getByRole("status", {
    name: "Agent selection",
    exact: true,
  });
  const input = page.getByRole("textbox", {
    name: "Message",
    exact: true,
  });
  await expect(build).toHaveAttribute("aria-pressed", "true");
  await expect(status).toHaveText("Current: Build");
  const toolbar = page.locator(".composer-agents");
  const promptBounds = await input.boundingBox();
  expect((await toolbar.boundingBox())!.y).toBeGreaterThanOrEqual(
    promptBounds!.y + promptBounds!.height,
  );
  await emit(
    page,
    "session.execution.started",
    { sessionID: "ses-a" },
    "opencode",
  );
  await input.press("Shift+Tab");
  await expect(input).toBeFocused();
  await expect(plan).toHaveAttribute("aria-pressed", "true");
  await expect(status).toHaveText("Running: Build · Next: Plan");
  const runningIcon = build.locator(".agent-icon");
  await expect(runningIcon).toHaveAttribute("data-running", "true");
  await expect(plan.locator(".agent-icon")).toHaveAttribute(
    "data-running",
    "false",
  );
  expect(
    await runningIcon.evaluate(
      (node) => getComputedStyle(node, "::after").animationName,
    ),
  ).toBe("spin");
  await page.emulateMedia({ reducedMotion: "reduce" });
  expect(
    await runningIcon.evaluate(
      (node) => getComputedStyle(node, "::after").animationName,
    ),
  ).toBe("none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await input.fill("Plan the next change");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_select_agent",
      ),
    ),
  ).toHaveLength(0);
  await page.screenshot({
    path: "test-results/agent-switch-running.png",
  });
  await page
    .getByRole("button", { name: "Stop generation", exact: true })
    .click();
  await expect(status).toHaveText("Current: Build · Next: Plan");
  await expect(toolbar.locator('[data-running="true"]')).toHaveCount(0);
  await page.evaluate(() => {
    (window as any).fixture.hold("opencode_select_agent");
    (window as any).fixture.setOpenWorking(true);
  });
  await input.press("Enter");
  await expect(plan).toBeDisabled();
  await expect(build).toBeDisabled();
  await input.press("Shift+Tab");
  await expect(plan).toHaveAttribute("aria-pressed", "true");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_send_prompt",
      ),
    ),
  ).toHaveLength(0);
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_select_agent"),
  );
  await expect(status).toHaveText("Running: Plan");
  await expect(plan).toBeEnabled();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter((call: any) =>
      ["opencode_select_agent", "opencode_send_prompt"].includes(call.command),
    ),
  );
  expect(calls.map((call: any) => call.command)).toEqual([
    "opencode_select_agent",
    "opencode_send_prompt",
  ]);
  expect(calls[0].args).toEqual({
    projectPath: "/work/caelum",
    sessionId: "ses-a",
    agent: "plan",
  });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(plan).toBeInViewport({ ratio: 1 });
  await expect(toolbar).toBeInViewport({ ratio: 1 });
  await expect(input).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/agent-switch-compact-dark.png" });
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page.screenshot({
    path: "test-results/agent-switch-compact-light.png",
  });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.screenshot({ path: "test-results/agent-switch-light.png" });
  await page
    .getByRole("button", { name: "Stop generation", exact: true })
    .click();
  await page.reload();
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(status).toHaveText("Current: Plan");
  await expect(plan).toHaveAttribute("aria-pressed", "true");
});

test("Shift+Tab cycles available agents without changing the draft or other keyboard navigation", async ({
  page,
}) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Codex draft");
  await input.press("Shift+Tab");
  await expect(input).not.toBeFocused();
  await expect(input).toHaveValue("Codex draft");

  await page.evaluate(() =>
    (window as any).fixture.setChoicesError("opencode", "Provider unavailable"),
  );
  await choose(page, "Provider", "OpenCode");
  const retry = page.getByRole("button", { name: "Load models", exact: true });
  await expect(retry).toBeVisible();
  await input.press("Shift+Tab");
  await expect(input).not.toBeFocused();
  await page.evaluate(() =>
    (window as any).fixture.setChoicesError("opencode", null),
  );
  await retry.click();
  const build = page.getByRole("button", { name: "Build", exact: true });
  await expect(build).toBeVisible();

  const status = page.getByRole("status", {
    name: "Agent selection",
    exact: true,
  });
  await expect(status).toHaveText("Uses provider settings");
  await input.fill("Keep this draft and selection");
  await input.evaluate((node: HTMLTextAreaElement) =>
    node.setSelectionRange(5, 9),
  );
  for (const agent of ["Build", "Plan", "Reviewer", "Build"]) {
    await input.press("Shift+Tab");
    await expect(status).toHaveText(`Next: ${agent}`);
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Keep this draft and selection");
    expect(
      await input.evaluate((node: HTMLTextAreaElement) => [
        node.selectionStart,
        node.selectionEnd,
      ]),
    ).toEqual([5, 9]);
  }
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        [
          "opencode_select_agent",
          "opencode_start_session",
          "opencode_send_prompt",
        ].includes(call.command),
      ),
    ),
  ).toHaveLength(0);

  for (const modifier of [
    "altKey",
    "ctrlKey",
    "metaKey",
    "isComposing",
    "repeat",
  ]) {
    await input.dispatchEvent("keydown", {
      key: "Tab",
      shiftKey: true,
      [modifier]: true,
    });
    await expect(status).toHaveText("Next: Build");
  }
  await input.press("Tab");
  await expect(input).not.toBeFocused();
  await expect(status).toHaveText("Next: Build");
  const search = page.getByRole("textbox", {
    name: "Search sessions",
    exact: true,
  });
  await search.press("Shift+Tab");
  await expect(search).not.toBeFocused();
  await expect(status).toHaveText("Next: Build");

  const dialog = await openSettings(page);
  const done = dialog.getByRole("button", { name: "Done", exact: true });
  await done.press("Shift+Tab");
  await expect(done).not.toBeFocused();
  await done.click();
  await expect(status).toHaveText("Next: Build");
  await expect(input).toHaveValue("Keep this draft and selection");
  await choose(page, "Agent", "Provider default");
  await expect(status).toHaveText("Uses provider settings");
});

test("agent choices survive stale history, navigation, reconnect and failed switches without sending with the wrong agent", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const status = page.getByRole("status", {
    name: "Agent selection",
    exact: true,
  });
  const plan = page.getByRole("button", { name: "Plan", exact: true });
  const input = page.getByRole("textbox", {
    name: "Message",
    exact: true,
  });
  await expect(status).toHaveText("Current: Build");
  await plan.click();
  await input.fill("Keep this prompt");
  await page.evaluate(() => {
    (window as any).fixture.holdHistory();
    window.dispatchEvent(new Event("focus"));
  });
  await page.evaluate(() =>
    (window as any).fixture.setAgent("/work/caelum", "ses-a", "reviewer"),
  );
  await emit(
    page,
    "session.agent.selected",
    { sessionID: "ses-a", agent: "reviewer" },
    "opencode",
  );
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await expect(status).toHaveText("Current: Reviewer · Next: Plan");
  await page.evaluate(() => (window as any).fixture.failAgent());
  await input.press("Enter");
  await expect(input).toHaveValue("Keep this prompt");
  await expect(
    page.getByRole("alert").filter({ hasText: "Agent switch failed" }),
  ).toBeVisible();
  await expect(status).toHaveText("Current: Reviewer · Next: Plan");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_send_prompt",
      ),
    ),
  ).toHaveLength(0);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(status).toHaveText("Current: Build");
  await expect(input).toBeEmpty();
  await page
    .getByRole("navigation", { name: "Open projects" })
    .getByRole("button", { name: "caelum", exact: true })
    .click();
  await expect(status).toHaveText("Current: Reviewer · Next: Plan");
  await expect(input).toHaveValue("Keep this prompt");
  await emit(page, "server.disconnected", {}, "opencode");
  await expect(plan).toBeDisabled();
  await page.evaluate(() =>
    (window as any).fixture.hold("opencode_resume_session"),
  );
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await page.evaluate(() =>
    (window as any).fixture.setAgent("/work/caelum", "ses-a", "build"),
  );
  await emit(
    page,
    "session.agent.selected",
    { sessionID: "ses-a", agent: "build" },
    "opencode",
  );
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_resume_session"),
  );
  await expect(status).toHaveText("Current: Build · Next: Plan");
  await expect(plan).toBeEnabled();
  await page.evaluate(() => (window as any).fixture.failSend());
  await input.press("Enter");
  await expect(
    page.getByRole("alert").filter({ hasText: "Connection refused" }),
  ).toBeVisible();
  await expect(input).toHaveValue("Keep this prompt");
  // The switch succeeded even though the prompt failed: report the native agent.
  await expect(status).toHaveText("Current: Plan");
  await input.press("Enter");
  await expect(status).toHaveText("Running: Plan");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_select_agent",
      ),
    ),
  ).toHaveLength(2);
});

test("new sessions support quick Plan selection, custom agents and an explicit provider default", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  const status = page.getByRole("status", {
    name: "Agent selection",
    exact: true,
  });
  await expect(status).toHaveText("Uses provider settings");
  await page.getByRole("button", { name: "Plan", exact: true }).click();
  await expect(status).toHaveText("Next: Plan");
  await choose(page, "Agent", "Reviewer");
  await expect(status).toHaveText("Next: Reviewer");
  await choose(page, "Agent", "Provider default");
  await expect(status).toHaveText("Uses provider settings");
  await page.getByRole("button", { name: "Plan", exact: true }).click();
  const input = page.getByRole("textbox", {
    name: "Message",
    exact: true,
  });
  await input.fill("Make a plan");
  await input.press("Enter");
  await expect(status).toHaveText("Running: Plan");
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === "opencode_start_session",
        ).args.agent,
    ),
  ).toBe("plan");
  await page
    .getByRole("button", { name: "Stop generation", exact: true })
    .click();
  await choose(page, "Agent", "Reviewer");
  await expect(status).toHaveText("Current: Plan · Next: Reviewer");
  await input.fill("Review it");
  await input.press("Enter");
  await expect(status).toHaveText("Running: Reviewer");
});

test("OpenCode uses provider choices, streams structured tools and reconciles accepted messages", async ({
  page,
}) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Codex draft");
  await choose(page, "Provider", "OpenCode");
  await expect(input).toHaveValue("");
  await choose(page, "Agent", "Build");
  await input.fill("Inspect this project");
  await input.press("Meta+Enter");
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toBeVisible();
  await emit(
    page,
    "session.text.delta",
    {
      sessionID: "ses-new",
      assistantMessageID: "answer",
      delta: "Inspected the files",
    },
    "opencode",
  );
  await emit(
    page,
    "session.tool.called",
    {
      sessionID: "ses-new",
      id: "read-1",
      name: "read",
      input: { filePath: "src/App.tsx" },
    },
    "opencode",
  );
  await emit(
    page,
    "session.tool.success",
    {
      sessionID: "ses-new",
      id: "read-1",
      content: [{ type: "text", text: "File contents" }],
    },
    "opencode",
  );
  await page.getByRole("button", { name: /read src\/App.tsx/ }).click();
  await expect(page.getByText("File contents", { exact: true })).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    const sent = fixture.calls.find(
      (call: any) => call.command === "opencode_send_prompt",
    );
    fixture.setOpenHistory([
      { id: sent.args.messageId, type: "user", text: sent.args.prompt },
      {
        id: "answer",
        type: "assistant",
        content: [{ type: "text", text: "Inspected the files" }],
      },
    ]);
  });
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-new" },
    "opencode",
  );
  await expect(
    page.locator(".user-text").filter({ hasText: "Inspect this project" }),
  ).toHaveCount(1);
  await expect(
    page.getByText("Inspected the files", { exact: true }),
  ).toHaveCount(1);
  await page.keyboard.press("Meta+n");
  await choose(page, "Provider", "Codex");
  await expect(input).toHaveValue("Codex draft");
});

test("navigating while first-send creation is pending does not steal the selected session", async ({
  page,
}) => {
  await desktop(page, null);
  await page.evaluate(() => (window as any).fixture.holdStart());
  await page
    .getByRole("textbox", { name: "Message", exact: true })
    .fill("Background task");
  await page
    .getByRole("textbox", { name: "Message", exact: true })
    .press("Enter");
  await page
    .getByRole("button", { name: /Review the provider adapters/ })
    .click();
  await page.evaluate(() => (window as any).fixture.releaseStart());
  await expect(page.locator(".breadcrumbs strong")).toHaveText(
    "Review the provider adapters",
  );
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
  await expect(
    page.locator(".session-item").filter({ hasText: "Background task" }),
  ).toContainText("Background task");
});

const previewPullRequest = {
  number: 44,
  title: "Improve project navigation",
  url: "https://github.com/example/caelum/pull/44",
  isDraft: false,
  state: "OPEN",
  body: "A **clearer workspace** with independent checkouts.\n\n<!-- template note -->\n## Details\nKeep every task easy to find.",
  author: { login: "oscar" },
  baseRefName: "main",
  headRefName: "feature/navigation",
  headRefOid: "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
  mergeable: "MERGEABLE",
  mergeStateStatus: "CLEAN",
  additions: 128,
  deletions: 42,
  changedFiles: 6,
  reviewDecision: "APPROVED",
  statusCheckRollup: [
    {
      __typename: "CheckRun",
      name: "Tests",
      status: "COMPLETED",
      conclusion: "SUCCESS",
      detailsUrl: "https://github.com/example/caelum/actions/runs/123",
    },
    {
      __typename: "StatusContext",
      context: "Preview deploy",
      state: "PENDING",
      targetUrl: "https://deploy.example.com/preview/44",
    },
  ],
};

async function preparePullRequestPreview(page: Page) {
  await page.clock.install();
  await desktop(page, null, { worktrees: true });
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", pr);
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.setPullRequestDetails(pr.url, pr);
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  return page
    .locator(".project-row")
    .getByRole("button", { name: /Open pull request #44/ });
}

test("PR previews load on demand, stay open across the hover gap, share cached details and open the PR screen", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page);
  const detailCalls = () =>
    page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_pull_request_details",
      ),
    );
  expect(await detailCalls()).toHaveLength(0);
  await trigger.hover();
  await page.getByRole("textbox", { name: "Message", exact: true }).hover();
  await page.clock.runFor(300);
  expect(await detailCalls()).toHaveLength(0);
  await trigger.hover();
  await page.clock.runFor(300);
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(
    card.getByRole("heading", { name: previewPullRequest.title, exact: true }),
  ).toBeVisible();
  await expect(card).toContainText("clearer workspace");
  await expect(card).not.toContainText("template note");
  await expect(card).toContainText("+128");
  await expect(card).toContainText("−42");
  await expect(card).toContainText("6 files changed");
  await expect(card).toContainText("1 passed · 1 pending");
  await expect(card).toContainText("Approved");
  await card.hover();
  await page.clock.runFor(500);
  await expect(card).toBeVisible();
  await page.screenshot({ path: "test-results/pr-preview.png" });
  await expect(page.locator(".breadcrumbs")).toContainText("New session");
  await card.getByRole("button", { name: "View pull request" }).click();
  const expanded = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(
    expanded.getByRole("heading", { name: "Details", exact: true }),
  ).toBeVisible();
  await expect(
    expanded.getByRole("region", { name: "Pull request checks" }),
  ).toContainText("Preview deploy");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeHidden();
  await page.screenshot({ path: "test-results/pr-overview.png" });
  await expanded.getByRole("link", { name: "GitHub", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe(previewPullRequest.url);
  await expanded.getByRole("button", { name: "Back to session" }).click();
  await expect(expanded).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeFocused();
  await expect(card).toHaveCount(0);
  const sessionTrigger = sessionRow(
    page,
    "Refine the session workspace",
  ).getByRole("button", { name: /pull request #44/ });
  await sessionTrigger.hover();
  await page.clock.runFor(300);
  await expect(card).toContainText("+128");
  expect(await detailCalls()).toHaveLength(1);
  await sessionTrigger.click();
  await expect(page.locator(".breadcrumbs")).toContainText("New session");
  await page.locator(".breadcrumbs").click();
  await expect(card).toHaveCount(0);
});

test("PR previews support keyboard access, retry failures and constrain long content in both themes", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page);
  await page.setViewportSize({ width: 800, height: 620 });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      "GitHub authentication required",
    ),
  );
  await trigger.focus();
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(card.getByRole("alert")).toContainText(
    "authentication required",
  );
  await trigger.press("Tab");
  await expect(
    card.getByRole("button", { name: "Retry", exact: true }),
  ).toBeFocused();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_pull_request_details", null);
    fixture.setPullRequestDetails(pr.url, {
      ...pr,
      state: "MERGED",
      title:
        "A pull request with a very long title that must stay inside the preview without pushing the layout sideways",
      headRefName: "feature/" + "long-branch-".repeat(15),
      body:
        "<!-- hidden -->\n" + "This is a readable description. ".repeat(150),
      reviewDecision: "CHANGES_REQUESTED",
      statusCheckRollup: [
        {
          __typename: "CheckRun",
          name: "Tests",
          status: "COMPLETED",
          conclusion: "FAILURE",
        },
      ],
    });
  }, previewPullRequest);
  await page.keyboard.press("Enter");
  await expect(card).toContainText("Merged");
  await expect(card).toContainText("1 failed");
  await expect(card).toContainText("Changes requested");
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await expect(card).toBeInViewport({ ratio: 1 });
    expect(
      await card.evaluate((node) => node.scrollWidth <= node.clientWidth),
    ).toBe(true);
    await page.screenshot({ path: `test-results/pr-preview-${theme}.png` });
  }
  await page.keyboard.press("Escape");
  await expect(card).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await page.clock.runFor(500);
  await expect(card).toHaveCount(0);
  await trigger.press("Enter");
  await trigger.press("Tab");
  await expect(
    card.getByRole("button", { name: "View pull request" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(screen.getByRole("heading", { level: 1 })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(screen).toBeVisible();
  await screen.getByRole("button", { name: "Back to session" }).click();
  await trigger.focus();
  await trigger.press("Enter");
  await trigger.press("Tab");
  await page.keyboard.press("Shift+Tab");
  await expect(trigger).toBeFocused();
  await expect(card).toHaveCount(0);
  await trigger.press("Enter");
  await trigger.press("Tab");
  await page.keyboard.press("Tab");
  await expect(
    card.getByRole("link", { name: "GitHub", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("button", { name: "Close caelum", exact: true }),
  ).toBeFocused();
  await expect(card).toHaveCount(0);
});

test("PR previews discard delayed displays after navigation and isolate identical numbers across repositories", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page);
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_details"),
  );
  await trigger.hover();
  await page.clock.runFor(300);
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(card).toContainText("Loading pull request details");
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest(
      "main",
      {
        ...pr,
        url: "https://github.com/example/other/pull/44",
        title: "Other repository PR",
      },
      "/work/other",
    );
    fixture.setPullRequestDetails("https://github.com/example/other/pull/44", {
      ...pr,
      url: "https://github.com/example/other/pull/44",
      title: "Other repository PR",
      body: "Different description",
      additions: 9,
    });
  }, previewPullRequest);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(card).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  const other = page
    .locator(".project-row")
    .getByRole("button", { name: /pull request #44: Other repository PR/ });
  await other.hover();
  await page.clock.runFor(300);
  await expect(card).toContainText("Different description");
  await expect(card).toContainText("+9");
  await expect(card).not.toContainText("clearer workspace");
  await page.evaluate(() => {
    (window as any).fixture.setGitMetadata(
      { repositoryRoot: "/work/other", branch: null },
      "/work/other",
    );
    window.dispatchEvent(new Event("focus"));
  });
  await expect(other).toHaveCount(0);
  await expect(card).toHaveCount(0);
});

test("PR preview details are shared with linked worktrees and cannot select their checkout", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page);
  await trigger.hover();
  await page.clock.runFor(300);
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(card).toContainText("+128");
  await page.keyboard.press("Escape");
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.addWorktree("/external/navigation", "feature/navigation");
    fixture.setPullRequest("feature/navigation", pr, "/external/navigation");
  }, previewPullRequest);
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Refresh worktrees");
  const checkout = page
    .locator(".worktree-row")
    .getByRole("button", { name: /pull request #44/ });
  await checkout.hover();
  await page.clock.runFor(300);
  await expect(card).toContainText("+128");
  await checkout.click();
  await expect(
    page.getByRole("button", { name: "caelum", exact: true }),
  ).toHaveAttribute("aria-current", "page");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_pull_request_details",
      ),
    ),
  ).toHaveLength(1);
  await page
    .getByRole("button", {
      name: "Switch to worktree feature/navigation",
      exact: true,
    })
    .click();
  await expect(card).toHaveCount(0);
});

test("session PR section groups open sessions and opens an overview without selecting a session", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", pr);
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.setPullRequestDetails(pr.url, pr);
    fixture.setPullRequest("fix/provider-adapters", {
      ...pr,
      number: 45,
      title: "Improve provider adapters",
      url: "https://github.com/example/caelum/pull/45",
      isDraft: true,
    });
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  const section = page.getByRole("region", {
    name: "Pull requests",
    exact: true,
  });
  await expect(section.getByRole("button")).toHaveCount(2);
  const shared = section.getByRole("button", { name: /View pull request #44/ });
  await expect(shared).toHaveAccessibleDescription("#44 · 2 sessions");
  await expect(
    section.getByRole("button", { name: /View draft pull request #45/ }),
  ).toHaveAccessibleDescription("#45 · Review the provider adapters");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft while reviewing PRs");
  await shared.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(screen).toContainText("+128");
  await expect(shared).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await screen.getByRole("button", { name: "Back to session" }).click();
  await expect(input).toHaveValue("Keep this draft while reviewing PRs");
  await expect(page.locator(".breadcrumbs")).toContainText("New session");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        ["codex_resume_session", "opencode_resume_session"].includes(
          call.command,
        ),
      ),
    ),
  ).toHaveLength(0);
  await shared.focus();
  await page.clock.runFor(32);
  await expect(shared).toBeFocused();
  await shared.press("Enter");
  await expect(screen).toBeVisible();
  await page.getByRole("button", { name: "Sessions", exact: true }).click();
  await page.getByRole("textbox", { name: "Search sessions" }).fill("no match");
  await expect(page.locator(".session-list")).toContainText(
    "No matching sessions",
  );
  await expect(section.getByRole("button")).toHaveCount(2);
  await page.getByRole("textbox", { name: "Search sessions" }).fill("");
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await page.screenshot({
      path: `test-results/session-pr-list-${theme}.png`,
    });
  }
  const first = sessionRow(page, "Refine the session workspace");
  await first
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(shared).toHaveAccessibleDescription("#44 · OpenCode session");
  await sessionRow(page, "OpenCode session")
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(shared).toHaveCount(0);
  await expect(page.locator(".project-row .branch-pr-trigger")).toBeVisible();
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(shared).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", null);
    fixture.setPullRequest("fix/provider-adapters", null);
  });
  await page.clock.runFor(60_100);
  await expect(section.getByRole("button")).toHaveCount(0);
  await expect(section).toContainText("No pull requests for open sessions");
});

test("PR screen keeps the sidebar usable and preserves the conversation, draft and reading position", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  const session = page.getByRole("button", {
    name: "Refine the session workspace",
    exact: true,
  });
  await session.click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my session draft");
  await page
    .getByRole("button", { name: "Load earlier messages", exact: true })
    .click();
  const transcript = page.locator(".transcript");
  await transcript.evaluate((node) => {
    node.setAttribute("data-mounted", "yes");
    node.scrollTop = 250;
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  const entry = page
    .getByRole("region", { name: "Pull requests", exact: true })
    .getByRole("button", { name: /View pull request #44/ });
  await entry.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(screen).toContainText("+128");
  await expect(screen.getByRole("heading", { level: 1 })).toBeFocused();
  await expect(entry).toHaveAttribute("aria-current", "page");
  await expect(session).not.toHaveAttribute("aria-current", "page");
  await expect(input).toBeHidden();
  await expect(transcript).toHaveAttribute("data-mounted", "yes");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "pr-background",
    delta: "Progress while reviewing the pull request",
  });
  await expect(screen.getByRole("heading", { level: 1 })).toBeFocused();
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await page.screenshot({ path: `test-results/pr-screen-${theme}.png` });
  }
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(screen).toBeInViewport({ ratio: 1 });
  expect(
    await screen.evaluate((node) => node.scrollWidth <= node.clientWidth),
  ).toBe(true);
  await page.screenshot({ path: "test-results/pr-screen-compact.png" });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await expect(screen).toBeVisible();
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(entry).toHaveAttribute("aria-current", "page");
  await page.getByRole("button", { name: "Sessions", exact: true }).click();
  await expect(screen).toHaveCount(0);
  await expect(input).toHaveValue("Keep my session draft");
  await expect(transcript).toHaveAttribute("data-mounted", "yes");
  await expect
    .poll(() => transcript.evaluate((node) => node.scrollTop))
    .toBe(250);
  await page
    .getByRole("button", { name: "Jump to latest", exact: true })
    .click();
  await expect(
    page.getByText("Progress while reviewing the pull request", {
      exact: true,
    }),
  ).toBeVisible();
  await entry.click();
  await session.click();
  await expect(screen).toHaveCount(0);
  await expect(input).toHaveValue("Keep my session draft");
  await entry.click();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(screen).toHaveCount(0);
  await expect(
    page.getByRole("complementary", { name: "Project files", exact: true }),
  ).toBeVisible();
  await entry.click();
  await page.getByRole("button", { name: "Changes", exact: true }).click();
  await expect(
    page.getByRole("complementary", { name: "Project changes", exact: true }),
  ).toBeVisible();
  await expect(screen).toHaveCount(0);
  await entry.click();
  await page.keyboard.press("Meta+n");
  await expect(screen).toHaveCount(0);
  await expect(input).toHaveValue("");
  await session.click();
  await expect(input).toHaveValue("Keep my session draft");
});

test("PR screen switches between requests, retries refresh failures and rejects stale results after navigation", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    const other = {
      ...pr,
      number: 45,
      title: "Improve provider adapters",
      url: "https://github.com/example/caelum/pull/45",
      body: "Independent provider work",
      additions: 9,
    };
    fixture.setPullRequest("fix/provider-adapters", other);
    fixture.setPullRequestDetails(other.url, other);
    fixture.hold(`github_pull_request_details:${pr.url}`);
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  const list = page.getByRole("region", { name: "Pull requests", exact: true });
  const first = list.getByRole("button", { name: /View pull request #44/ });
  const second = list.getByRole("button", { name: /View pull request #45/ });
  await first.click();
  await expect(
    page.getByRole("region", { name: "Pull request #44", exact: true }),
  ).toContainText("Loading pull request details");
  await second.click();
  const screen = page.getByRole("region", {
    name: "Pull request #45",
    exact: true,
  });
  await expect(screen).toContainText("Independent provider work");
  await expect(second).toHaveAttribute("aria-current", "page");
  await expect(first).not.toHaveAttribute("aria-current", "page");
  await page.evaluate(
    (pr) =>
      (window as any).fixture.release(`github_pull_request_details:${pr.url}`),
    previewPullRequest,
  );
  await expect(screen).not.toContainText("clearer workspace");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      "GitHub unavailable",
    ),
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(screen.getByRole("alert")).toContainText(
    "Showing the last loaded details",
  );
  await expect(screen).toContainText("+9");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      null,
    ),
  );
  await screen.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(screen.getByRole("alert")).toHaveCount(0);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(screen).toHaveCount(0);
  await second.click();
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_details"),
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(screen).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await expect(page.locator(".pr-screen")).toHaveCount(0);
});

test("PR checks show live pending animations, passed ticks and linked results in both themes", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await page.evaluate((pr) => {
    (window as any).fixture.setPullRequestDetails(pr.url, {
      ...pr,
      body: "Check progress and merge this pull request.",
      mergeStateStatus: "BLOCKED",
      statusCheckRollup: [
        pr.statusCheckRollup[0],
        {
          __typename: "CheckRun",
          name: "macOS build",
          status: "IN_PROGRESS",
          conclusion: null,
        },
        {
          __typename: "CheckRun",
          name: "Linux build",
          status: "QUEUED",
          conclusion: null,
        },
        {
          __typename: "CheckRun",
          name: "Lint",
          status: "COMPLETED",
          conclusion: "FAILURE",
          detailsUrl: "javascript:alert(1)",
        },
        {
          __typename: "CheckRun",
          name: "Optional deploy",
          status: "COMPLETED",
          conclusion: "SKIPPED",
        },
        pr.statusCheckRollup[1],
      ],
    });
  }, previewPullRequest);
  await page
    .getByRole("region", { name: "Pull requests", exact: true })
    .getByRole("button", { name: /View pull request #44/ })
    .click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const checks = screen.getByRole("region", { name: "Pull request checks" });
  const rows = checks.getByRole("listitem");
  await expect(rows).toHaveCount(6);
  await expect(rows.filter({ hasText: "Tests" })).toContainText("Passed");
  await expect(
    rows.filter({ hasText: "Tests" }).locator(".lucide-check"),
  ).toBeVisible();
  await expect(rows.filter({ hasText: "macOS build" })).toContainText(
    "In progress",
  );
  await expect(rows.filter({ hasText: "Linux build" })).toContainText("Queued");
  await expect(
    rows.filter({ hasText: "Lint" }).locator(".lucide-x"),
  ).toBeVisible();
  await expect(rows.filter({ hasText: "Lint" }).getByRole("link")).toHaveCount(
    0,
  );
  await expect(rows.filter({ hasText: "Optional deploy" })).toContainText(
    "Skipped",
  );
  const spinner = rows.filter({ hasText: "macOS build" }).locator(".spinning");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await expect(spinner).toHaveCSS("animation-name", "spin");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(spinner).toHaveCSS("animation-name", "none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await checks.getByRole("link", { name: "Tests", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe("https://github.com/example/caelum/actions/runs/123");
  await checks
    .getByRole("link", { name: "Preview deploy", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe("https://deploy.example.com/preview/44");
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await page.screenshot({ path: `test-results/pr-checks-${theme}.png` });
    await screen
      .getByRole("region", { name: "Merge pull request", exact: true })
      .scrollIntoViewIfNeeded();
    await page.screenshot({ path: `test-results/pr-merge-${theme}.png` });
    await screen
      .getByRole("button", { name: "Merge method", exact: true })
      .click();
    const menu = page.getByRole("dialog", { name: "Choose merge method" });
    await expect(menu).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: `test-results/pr-merge-menu-${theme}.png` });
    await page.keyboard.press("Escape");
    await screen.evaluate((node) => {
      node.scrollTop = 0;
    });
  }
  await page.setViewportSize({ width: 800, height: 620 });
  await checks.scrollIntoViewIfNeeded();
  expect(
    await screen.evaluate((node) => node.scrollWidth <= node.clientWidth),
  ).toBe(true);
  await page.screenshot({ path: "test-results/pr-checks-compact.png" });
  await expect(
    screen.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeDisabled();
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        statusCheckRollup: [pr.statusCheckRollup[0]],
      }),
    previewPullRequest,
  );
  await page.clock.runFor(30_100);
  await expect(checks).toContainText("1 passed");
  await expect(checks.locator(".spinning")).toHaveCount(0);
  await expect(
    screen.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeEnabled();
  await screen
    .getByRole("region", { name: "Merge pull request", exact: true })
    .scrollIntoViewIfNeeded();
  await page.screenshot({ path: "test-results/pr-merge-compact.png" });
  await screen
    .getByRole("button", { name: "Merge method", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Choose merge method" }),
  ).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: "test-results/pr-merge-menu-compact.png" });
  await page.getByRole("option", { name: /^Rebase and merge/ }).click();
  await expect(
    screen.getByRole("button", { name: "Rebase and merge", exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_merge_pull_request",
      ),
    ),
  ).toHaveLength(0);
});

test("PR merge is shared across navigation, pins the head and removes merged rows despite delayed discovery", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  const list = page.getByRole("region", { name: "Pull requests", exact: true });
  const first = list.getByRole("button", { name: /View pull request #44/ });
  await first.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(
    screen.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeEnabled();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    const other = {
      ...pr,
      number: 45,
      title: "Another pull request",
      url: "https://github.com/example/caelum/pull/45",
    };
    fixture.setPullRequest("fix/provider-adapters", other);
    fixture.setPullRequestDetails(other.url, other);
    fixture.hold("github_open_pull_request:/work/caelum:main");
    fixture.hold("github_merge_pull_request");
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  const second = list.getByRole("button", { name: /View pull request #45/ });
  await expect(second).toBeVisible();
  const method = screen.getByRole("button", {
    name: "Merge method",
    exact: true,
  });
  await method.press("ArrowDown");
  await expect(
    page.getByRole("option", { name: /^Create a merge commit/ }),
  ).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(method).toBeFocused();
  await screen
    .getByRole("button", { name: "Squash and merge", exact: true })
    .click();
  await expect(
    screen.getByRole("button", { name: "Merging…", exact: true }),
  ).toBeDisabled();
  await expect(
    screen
      .getByRole("button", { name: "Merging…", exact: true })
      .locator(".spinning"),
  ).toBeVisible();
  await expect(
    screen.getByRole("button", { name: "Refresh", exact: true }),
  ).toBeDisabled();
  await screen.getByRole("button", { name: "Back to session" }).click();
  await first.click();
  await expect(
    screen.getByRole("button", { name: "Merge method", exact: true }),
  ).toHaveAttribute("title", "Merge method: Squash and merge");
  await expect(
    screen.getByRole("button", { name: "Merge method", exact: true }),
  ).toBeDisabled();
  await expect(
    screen.getByRole("button", { name: "Merging…", exact: true }),
  ).toBeDisabled();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "github_merge_pull_request",
    ),
  );
  expect(calls).toHaveLength(1);
  expect(calls[0].args).toEqual({
    projectPath: "/work/caelum",
    url: previewPullRequest.url,
    method: "squash",
    headOid: previewPullRequest.headRefOid,
  });
  await second.click();
  const otherScreen = page.getByRole("region", {
    name: "Pull request #45",
    exact: true,
  });
  await page.evaluate(() =>
    (window as any).fixture.release("github_merge_pull_request"),
  );
  await expect(first).toHaveCount(0);
  await expect(otherScreen).toBeVisible();
  await expect(
    otherScreen.getByRole("button", {
      name: "Merge pull request",
      exact: true,
    }),
  ).toBeEnabled();
  await page.evaluate(() =>
    (window as any).fixture.release(
      "github_open_pull_request:/work/caelum:main",
    ),
  );
  await expect(page.locator(".project-row .branch-pr-trigger")).toHaveCount(0);
  await expect(first).toHaveCount(0);
  await expect(second).toHaveAttribute("aria-current", "page");
});

test("PR merge reports failures, reloads changed heads and confirms a successful retry", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  const entry = page
    .getByRole("region", { name: "Pull requests", exact: true })
    .getByRole("button", { name: /View pull request #44/ });
  await entry.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const merge = screen.getByRole("region", {
    name: "Merge pull request",
    exact: true,
  });
  const button = merge.getByRole("button", {
    name: "Merge pull request",
    exact: true,
  });
  await expect(button).toBeEnabled();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_merge_pull_request",
      "Merge method is not allowed (HTTP 405)",
    ),
  );
  await button.click();
  await expect(merge.getByRole("alert")).toContainText(
    "Merge method is not allowed",
  );
  await expect(entry).toBeVisible();
  await expect(button).toBeEnabled();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_merge_pull_request", null);
    fixture.setPullRequestDetails(pr.url, {
      ...pr,
      headRefOid: "b".repeat(40),
    });
  }, previewPullRequest);
  await button.click();
  await expect(merge.getByRole("alert")).toContainText(
    "Head branch was modified",
  );
  await expect(button).toBeEnabled();
  await merge
    .getByRole("button", { name: "Merge method", exact: true })
    .click();
  await page.getByRole("option", { name: /^Rebase and merge/ }).click();
  await merge
    .getByRole("button", { name: "Rebase and merge", exact: true })
    .click();
  await expect(merge.getByRole("status")).toHaveText("Merged into main");
  await expect(merge.getByRole("alert")).toHaveCount(0);
  await expect(merge.getByRole("button")).toHaveCount(0);
  await expect(screen.locator(".pr-state")).toHaveText("Merged");
  await expect(entry).toHaveCount(0);
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "github_merge_pull_request",
    ),
  );
  expect(calls).toHaveLength(3);
  expect(calls[2].args).toMatchObject({
    method: "rebase",
    headOid: "b".repeat(40),
  });
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(merge.getByRole("status")).toHaveText("Merged into main");
});

test("PR merge explains draft, blocked, conflicting and unknown states without submitting", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await page
    .getByRole("region", { name: "Pull requests", exact: true })
    .getByRole("button", { name: /View pull request #44/ })
    .click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const merge = screen.getByRole("region", {
    name: "Merge pull request",
    exact: true,
  });
  await expect(merge).toBeVisible();
  for (const [change, message] of [
    [{ isDraft: true }, "Mark this draft ready"],
    [{ mergeStateStatus: "BLOCKED" }, "Waiting for required checks"],
    [{ mergeable: "CONFLICTING" }, "Resolve merge conflicts"],
    [{ mergeStateStatus: "BEHIND" }, "Update this branch"],
    [{ mergeable: "UNKNOWN" }, "Waiting for GitHub"],
  ] as const) {
    await page.evaluate(
      ({ pr, change }) =>
        (window as any).fixture.setPullRequestDetails(pr.url, {
          ...pr,
          ...change,
        }),
      { pr: previewPullRequest, change },
    );
    await screen.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(merge.getByRole("status")).toContainText(message);
    await expect(
      merge.getByRole("button", { name: "Merge pull request", exact: true }),
    ).toBeDisabled();
  }
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        state: "CLOSED",
        statusCheckRollup: null,
      }),
    previewPullRequest,
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(merge.getByRole("status")).toContainText(
    "This pull request is closed",
  );
  await expect(merge.getByRole("button")).toHaveCount(0);
  await expect(
    screen.getByRole("region", { name: "Pull request checks" }),
  ).toContainText("No checks reported");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_merge_pull_request",
      ),
    ),
  ).toHaveLength(0);
});

test("session PR section discovers off-screen sessions with shared bounded lookups", async ({
  page,
}) => {
  await desktop(page, null);
  await page.setViewportSize({ width: 800, height: 620 });
  const before = await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.hold("github_open_pull_request");
    for (let index = 0; index < 18; index++) {
      fixture.addSession(`background-${index}`, `Background task ${index}`);
      fixture.setSessionDetails("codex", `background-${index}`, {
        branch: `feature/background-${index}`,
        branchIsCurrent: false,
        changes: null,
      });
      if (index === 17)
        fixture.setPullRequest(`feature/background-${index}`, pr);
    }
    return fixture.calls.length;
  }, previewPullRequest);
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await expect
    .poll(() =>
      page.evaluate(
        (before) =>
          (window as any).fixture.calls
            .slice(before)
            .filter(
              (call: any) =>
                call.command === "session_details" &&
                call.args.sessionId === "background-17",
            ).length,
        before,
      ),
    )
    .toBe(1);
  const lookups = await page.evaluate(
    (before) =>
      (window as any).fixture.calls
        .slice(before)
        .filter((call: any) => call.command === "github_open_pull_request"),
    before,
  );
  expect(lookups.length).toBeGreaterThan(0);
  expect(lookups.length).toBeLessThanOrEqual(4);
  await page.evaluate(() =>
    (window as any).fixture.release("github_open_pull_request"),
  );
  const section = page.getByRole("region", {
    name: "Pull requests",
    exact: true,
  });
  const row = section.getByRole("button", { name: /View pull request #44/ });
  await expect(row).toBeVisible();
  await expect(row).toHaveAccessibleDescription("#44 · Background task 17");
  await expect(sessionRow(page, "Background task 17")).not.toBeInViewport();
  await expect(row).toBeInViewport({ ratio: 1 });
  const calls = await page.evaluate(
    (before) => (window as any).fixture.calls.slice(before),
    before,
  );
  expect(
    calls.filter(
      (call: any) =>
        call.command === "github_open_pull_request" &&
        call.args.branch === "feature/background-17",
    ),
  ).toHaveLength(1);
  expect(
    calls.filter((call: any) => call.command === "codex_resume_session"),
  ).toHaveLength(0);
  expect(
    await section.evaluate((node) => node.scrollWidth <= node.clientWidth),
  ).toBe(true);
  await page.screenshot({
    path: "test-results/session-pr-list-many-sessions.png",
  });
});

test("session PR section ignores delayed results from a different project", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.hold(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    );
    fixture.setPullRequest(
      "other-project",
      {
        ...pr,
        url: "https://github.com/example/other/pull/44",
        title: "Other project changes",
      },
      "/work/other",
    );
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const section = page.getByRole("region", {
    name: "Pull requests",
    exact: true,
  });
  await expect(
    section.getByRole("button", { name: /Other project changes/ }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.release(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    ),
  );
  await expect(section.getByRole("button")).toHaveCount(1);
  await expect(section).not.toContainText(previewPullRequest.title);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(
    section.getByRole("button", { name: /Improve project navigation/ }),
  ).toBeVisible();
  await expect(section).not.toContainText("Other project changes");
});

test("branch PR indicators share lookups, distinguish drafts and clear closed or unavailable PRs", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  await page.getByRole("button", { name: /New session/ }).click();
  await choose(page, "Provider", "OpenCode");
  const project = page
    .getByRole("navigation", { name: "Open projects" })
    .getByRole("button", { name: "caelum", exact: true })
    .locator("..");
  const session = sessionRow(page, "Refine the session workspace");
  const openCode = sessionRow(page, "OpenCode session");
  await expect(session.locator(".session-branch")).toHaveText(
    "feat/session-ui-redesign",
  );
  await expect(openCode.locator(".session-branch")).toHaveText("main");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", {
      number: 11,
      title: "Main changes",
      url: "https://github.com/example/repo/pull/11",
      isDraft: false,
    });
    fixture.setPullRequest("feat/session-ui-redesign", {
      number: 40,
      title: "Session UI",
      url: "https://github.com/example/repo/pull/40",
      isDraft: true,
    });
  });
  await page.clock.runFor(60_100);
  await expect(
    project.getByRole("button", {
      name: "Open pull request #11: Main changes",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    openCode.getByRole("button", {
      name: "Open pull request #11: Main changes",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    session.getByRole("button", {
      name: "Draft pull request #40: Session UI",
      exact: true,
    }),
  ).toHaveAttribute("aria-haspopup", "dialog");
  await expect(
    sessionRow(page, "Review the provider adapters").locator(".branch-pr"),
  ).toBeEmpty();
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.command === "github_open_pull_request" &&
          call.args.projectPath === "/work/caelum" &&
          call.args.branch === "main",
      ),
    ),
  ).toHaveLength(2);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", null);
    fixture.setPullRequest("feat/session-ui-redesign", {
      number: 40,
      title: "Ready for review",
      url: "https://github.com/example/repo/pull/40",
      isDraft: false,
    });
  });
  await page.clock.runFor(60_100);
  await expect(project.locator(".branch-pr")).toBeEmpty();
  await expect(openCode.locator(".branch-pr")).toBeEmpty();
  await expect(
    session.getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    }),
  ).toBeVisible();
  await page.setViewportSize({ width: 800, height: 620 });
  await session
    .getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    })
    .scrollIntoViewIfNeeded();
  await expect(
    session.getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    }),
  ).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/branch-pr-indicators.png" });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_open_pull_request",
      "GitHub CLI unavailable",
    ),
  );
  await page.clock.runFor(60_100);
  await expect(session.locator(".branch-pr")).toBeEmpty();
  await expect(project.locator(".project-changes")).toHaveText("2 changed");
  await expect(
    page.getByRole("alert").filter({ hasText: "GitHub CLI unavailable" }),
  ).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("github_open_pull_request", null),
  );
  await page.clock.runFor(60_100);
  await expect(
    session.getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    }),
  ).toBeVisible();
});

test("branch PR indicators stay scoped through branch and project changes and delayed responses", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const projects = page.getByRole("navigation", { name: "Open projects" });
  const current = projects
    .getByRole("button", { name: "caelum", exact: true })
    .locator("..");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setGitMetadata({
      repositoryRoot: "/work/caelum",
      branch: "feat/shared",
    });
    fixture.setPullRequest("feat/shared", {
      number: 10,
      title: "Old branch",
      url: "https://github.com/example/caelum/pull/10",
      isDraft: false,
    });
    fixture.hold("github_open_pull_request:/work/caelum:feat/shared");
    window.dispatchEvent(new Event("focus"));
  });
  await expect(current.locator(".project-branch")).toHaveText("feat/shared");
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).fixture.calls.some(
          (call: any) =>
            call.command === "github_open_pull_request" &&
            call.args.branch === "feat/shared",
        ),
      ),
    )
    .toBe(true);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setGitMetadata({
      repositoryRoot: "/work/caelum",
      branch: "feat/current",
    });
    fixture.setPullRequest("feat/current", {
      number: 20,
      title: "Current branch",
      url: "https://github.com/example/caelum/pull/20",
      isDraft: false,
    });
    window.dispatchEvent(new Event("focus"));
  });
  await expect(
    current.getByRole("button", {
      name: "Open pull request #20: Current branch",
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.release("github_open_pull_request:/work/caelum:feat/shared");
    fixture.setGitMetadata(
      { repositoryRoot: "/work/other", branch: "feat/current" },
      "/work/other",
    );
    fixture.setPullRequest(
      "feat/current",
      {
        number: 30,
        title: "Other repository",
        url: "https://github.com/example/other/pull/30",
        isDraft: false,
      },
      "/work/other",
    );
  });
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const other = projects
    .getByRole("button", { name: "other", exact: true })
    .locator("..");
  await expect(
    other.getByRole("button", {
      name: "Open pull request #30: Other repository",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    current.getByRole("button", {
      name: "Open pull request #20: Current branch",
      exact: true,
    }),
  ).toBeVisible();
  await expect(current.getByRole("button", { name: /Old branch/ })).toHaveCount(
    0,
  );
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setGitMetadata({ repositoryRoot: "/work/caelum", branch: null });
    fixture.setGitMetadata(
      { repositoryRoot: null, branch: null },
      "/work/other",
    );
    window.dispatchEvent(new Event("focus"));
  });
  await expect(current.locator(".project-branch")).toHaveText("Detached HEAD");
  await expect(current.locator(".branch-pr-trigger")).toHaveCount(0);
  await expect(other).toContainText("Not a Git repository");
  await expect(other.locator(".branch-pr-trigger")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.command === "github_open_pull_request" && !call.args.branch,
      ),
    ),
  ).toHaveLength(0);
});

test("project rows show scoped live Git details and background activity, including unavailable repositories", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const projects = page.getByRole("navigation", { name: "Open projects" });
  const current = projects.getByRole("button", {
    name: "caelum",
    exact: true,
  });
  const currentDetails = current.locator("..");
  await expect(currentDetails.locator(".project-path")).toHaveText(
    "/work/caelum",
  );
  await expect(currentDetails.locator(".project-branch")).toHaveText("main");
  await expect(currentDetails.locator(".project-changes")).toHaveText(
    "2 changed",
  );
  await expect(current).toHaveAccessibleDescription(
    /\/work\/caelum.*main.*2 changed/,
  );
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "background" },
  });
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.hold("git_repository_status:/work/other");
    fixture.setGitError("Folder unavailable", "/work/other");
  });
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const other = projects.getByRole("button", { name: "other", exact: true });
  await expect(other).toHaveAttribute("aria-current", "page");
  const otherDetails = other.locator("..");
  await expect(otherDetails).toContainText("Checking Git…");
  await expect(current.locator(".running-dot")).toBeVisible();
  await current.click();
  await page.evaluate(() =>
    (window as any).fixture.release("git_repository_status:/work/other"),
  );
  await expect(otherDetails).toContainText("Git unavailable");
  await expect(currentDetails.locator(".project-changes")).toHaveText(
    "2 changed",
  );
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setGitError(null, "/work/other");
    fixture.setGitMetadata(
      { repositoryRoot: null, branch: null },
      "/work/other",
    );
    fixture.setGitMetadata({
      repositoryRoot: "/work/caelum",
      branch: "feat/a-long-branch-that-stays-within-the-project-row",
    });
    fixture.setGitFiles([]);
    window.dispatchEvent(new Event("focus"));
  });
  await expect(otherDetails).toContainText("Not a Git repository");
  await expect(currentDetails.locator(".project-changes")).toHaveText("Clean");
  await expect(currentDetails.locator(".project-branch")).toHaveAttribute(
    "title",
    "Current branch: feat/a-long-branch-that-stays-within-the-project-row",
  );
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setGitMetadata(
      { repositoryRoot: "/work/other", branch: null },
      "/work/other",
    );
    fixture.emit("approval-requests", [
      {
        provider: "codex",
        projectPath: "/work/caelum",
        sessionId: "thread-a",
        requestId: "project-approval",
        summary: "Run a command?",
        description: "git status",
      },
    ]);
  });
  await page.clock.runFor(30100);
  await expect(otherDetails.locator(".project-branch")).toHaveText(
    "Detached HEAD",
  );
  await expect(otherDetails.locator(".project-changes")).toHaveText("Clean");
  await expect(current).toHaveAccessibleDescription(
    /Running session.*Approval required/,
  );
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/project-details-compact.png" });
  await other.focus();
  await page.keyboard.press("Enter");
  await expect(other).toHaveAttribute("aria-current", "page");
  await projects
    .getByRole("button", { name: "Close other", exact: true })
    .click();
  await expect(other).toHaveCount(0);
  await expect(current).toHaveAttribute("aria-current", "page");
});

test("Changes syntax colours preserve patch text, independent old/new grammars and theme-aware additions and removals", async ({
  page,
}) => {
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const patch = [
    "diff --git a/src/App.tsx b/src/App.tsx",
    "--- a/src/App.tsx",
    "+++ b/src/App.tsx",
    "@@ -1,4 +1,4 @@",
    " export const count = 42;",
    "-/* removed comment starts",
    '+const label = "<script>alert(1)</script> — café";',
    " const ready = true;",
    "-*/",
    "+// a new comment",
    "@@ -30,2 +30,2 @@",
    "---counter;",
    "+++counter;",
    " \tconsole.log(count);",
    "\\ No newline at end of file",
  ].join("\n");
  await page.evaluate((patch) => {
    (window as any).fixture.setGitDiff("src/App.tsx", {
      unstaged: patch,
      staged: "@@ -1 +1 @@\n-const previous = 1;\n+const previous = 2;",
    });
  }, patch);
  await pane.getByRole("button", { name: "Refresh Git status" }).click();
  const unstaged = pane.getByRole("region", {
    name: "Unstaged diff",
    exact: true,
  });
  const added = unstaged.locator(".added").filter({ hasText: "+const label" });
  await expect(added.locator(".syntax-keyword")).toHaveText("const");
  expect(await unstaged.locator("pre").textContent()).toBe(patch);
  await expect(unstaged.locator("script")).toHaveCount(0);
  await expect(
    unstaged.locator(".meta").filter({ hasText: "+++ b/src/App.tsx" }),
  ).toHaveCount(1);
  await expect(
    unstaged.locator(".added").filter({ hasText: "+++counter;" }),
  ).toHaveCount(1);
  await expect(
    unstaged.locator(".removed").filter({ hasText: "---counter;" }),
  ).toHaveCount(1);
  await expect(
    unstaged.locator(".removed .syntax-comment").first(),
  ).toContainText("removed comment");
  await expect(
    unstaged
      .locator(".context")
      .filter({ hasText: "const ready" })
      .locator(".syntax-keyword"),
  ).toHaveText(["const", "true"]);
  await expect(added.locator(".diff-marker")).toHaveText("+");
  await expect(added.locator(".syntax-string")).toHaveCSS("display", "inline");
  await expect(added).toHaveCSS("box-shadow", "none");
  await expect(added).toHaveCSS("background-image", "none");
  await expect(added).toHaveCSS("background-color", "rgb(20, 35, 25)");
  await expect(unstaged.locator(".removed").first()).toHaveCSS(
    "background-color",
    "rgb(44, 24, 29)",
  );
  const staged = pane.getByRole("region", {
    name: "Staged diff",
    exact: true,
  });
  await expect(staged.locator(".added .syntax-number")).toHaveText("2");
  await page.screenshot({ path: "test-results/syntax-diff-dark.png" });
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(added).toHaveCSS("background-color", "rgb(232, 243, 233)");
  await expect(added.locator(".syntax-keyword")).toHaveCSS(
    "color",
    "rgb(123, 60, 171)",
  );
  await page.screenshot({ path: "test-results/syntax-diff-light.png" });

  for (const [path, source, tokenClass, text] of [
    ["src/main.rs", 'let name = "Caelum";', ".syntax-keyword", "let"],
    ["config.json", '{ "enabled": true, "count": 3 }', ".syntax-number", "3"],
    ["script.py", 'return "hello"', ".syntax-keyword", "return"],
    ["notes.unknown", '<img src=x onerror="alert(1)">', "", ""],
    ["image.png", "Binary files /dev/null and b/image.png differ", "", ""],
  ]) {
    const content =
      path === "image.png"
        ? source
        : `--- /dev/null\n+++ b/${path}\n@@ -0,0 +1 @@\n+${source}\n`;
    await page.evaluate(
      ({ path, content }) => {
        const fixture = (window as any).fixture;
        fixture.setGitFiles([{ path, indexStatus: "?", worktreeStatus: "?" }]);
        fixture.setGitDiff(path, {
          unstaged: "",
          staged: "",
          untracked: content,
        });
      },
      { path, content },
    );
    await pane.getByRole("button", { name: "Refresh Git status" }).click();
    const section = pane.getByRole("region", {
      name: "Untracked diff",
      exact: true,
    });
    if (tokenClass) await expect(section.locator(tokenClass)).toHaveText(text);
    await expect(section.locator("pre")).toHaveText(content);
    expect(await section.locator("pre").textContent()).toBe(content);
    await expect(section.locator("img")).toHaveCount(0);
  }
});

test("delayed syntax loading preserves the current file, exact text and manual reading position", async ({
  page,
}) => {
  let release = () => {};
  const loading = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/src/codeSyntax.ts", async (route) => {
    await loading;
    await route.continue();
  });
  try {
    await desktop(page);
    const pane = page.getByRole("complementary", { name: "Project changes" });
    await expect(pane.getByText(/const layout = 'split'/)).toBeVisible();
    await expect(pane.locator(".syntax-keyword")).toHaveCount(0);
    const patch = [
      "@@ -1,62 +1,62 @@",
      " {",
      ...Array.from({ length: 60 }, (_, i) => ` \t"option${i}": ${i},`),
      '-"enabled": false',
      '+"enabled": true',
      " }",
    ].join("\n");
    await page.evaluate((patch) => {
      const fixture = (window as any).fixture;
      fixture.setGitFiles([
        {
          path: "config.json",
          indexStatus: " ",
          worktreeStatus: "M",
          modifiedAt: "999",
          revision: "json-1",
        },
      ]);
      fixture.setGitDiff("config.json", { unstaged: patch });
    }, patch);
    await pane.getByRole("button", { name: "Refresh Git status" }).click();
    await expect(pane.locator(".git-diff-heading")).toHaveText("config.json");
    const panel = pane.locator(".git-diff-panel");
    await panel.evaluate((node) => {
      node.scrollTop = 80;
    });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep this draft");
    release();
    await expect(pane.locator(".added .syntax-keyword")).toHaveText("true");
    expect(await pane.locator("pre").textContent()).toBe(patch);
    expect(await panel.evaluate((node) => node.scrollTop)).toBe(80);
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Keep this draft");
    await expect(pane.locator(".git-diff-heading")).toHaveText("config.json");
  } finally {
    release();
  }
});

test("changes follow the most recently edited file during work and preserve manual review between edits", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("const layout = 'split'");
  await expect(pane.locator(".git-file-item.latest")).toContainText(
    "src/App.tsx",
  );
  await input.fill("Keep this draft");
  await pane
    .getByRole("button", { name: "src/styles.css, Staged modified" })
    .click();
  await expect(pane.getByRole("region", { name: "Staged diff" })).toContainText(
    "revision 1",
  );
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "turn-live" },
  });
  await expect(
    pane.getByRole("button", { name: "Refresh Git status" }),
  ).toBeEnabled();
  await page.evaluate(() => {
    (window as any).fixture.setDiffVersion(2);
    (window as any).fixture.setGitFiles([
      {
        path: "src/App.tsx",
        indexStatus: " ",
        worktreeStatus: "M",
        modifiedAt: "300",
        revision: "app-2",
      },
      {
        path: "src/styles.css",
        indexStatus: "M",
        worktreeStatus: " ",
        modifiedAt: "100",
        revision: "css-1",
      },
    ]);
  });
  await page.clock.runFor(2600);
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("revision 2");
  await expect(
    pane.getByRole("button", { name: "src/App.tsx, Unstaged modified" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(pane.locator(".latest-change.added")).toContainText(
    "revision 2",
  );
  await pane
    .getByRole("button", { name: "src/styles.css, Staged modified" })
    .click();
  await page.clock.runFor(2600);
  await expect(
    pane.getByRole("button", { name: "src/styles.css, Staged modified" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(pane.locator(".git-file-item.latest")).toContainText(
    "src/App.tsx",
  );
  await expect(input).toHaveValue("Keep this draft");
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/session-changes.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(input).toBeInViewport({ ratio: 1 });
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toBeInViewport({ ratio: 1 });
  await expect(pane).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/session-changes-minimum.png" });
  await page.evaluate(() =>
    (window as any).fixture.setGitFiles([
      { path: "src/App.tsx", indexStatus: " ", worktreeStatus: "M" },
    ]),
  );
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "turn-live", status: "completed" },
  });
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("revision 2");
  await expect(
    pane.getByRole("region", { name: "Staged diff", exact: true }),
  ).toHaveCount(0);
  await page.evaluate(() => (window as any).fixture.setGitFiles([]));
  await pane.getByRole("button", { name: "Refresh Git status" }).click();
  await expect(pane.getByText("Working tree clean")).toBeVisible();
  await pane.getByRole("button", { name: "Close changes" }).click();
  await expect(pane).toHaveCount(0);
  await expect(input).toHaveValue("Keep this draft");
  await page.getByRole("button", { name: "Show changes" }).click();
  await expect(pane.getByText("Working tree clean")).toBeVisible();
});

test("repeated edits reveal the updated hunk without shifting the transcript or unchanged review", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const panel = pane.locator(".git-diff-panel");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const refresh = pane.getByRole("button", { name: "Refresh Git status" });
  const update = async (version: number) => {
    await page.evaluate((version) => {
      const fixture = (window as any).fixture;
      const content = [
        "diff --git a/src/App.tsx b/src/App.tsx",
        "@@ -1,40 +1,40 @@",
        "-old header",
        "+existing header",
        ...Array.from({ length: 35 }, (_, index) => ` context ${index}`),
        "@@ -100,3 +100,3 @@",
        "-old footer",
        `+latest footer ${version}`,
        " unchanged footer",
      ].join("\n");
      fixture.setGitDiff("src/App.tsx", { unstaged: content });
      fixture.setGitFiles([
        {
          path: "src/App.tsx",
          indexStatus: " ",
          worktreeStatus: "M",
          modifiedAt: String(300 + version),
          revision: `app-${version}`,
        },
        {
          path: "src/styles.css",
          indexStatus: "M",
          worktreeStatus: " ",
          modifiedAt: "100",
          revision: "css-1",
        },
      ]);
    }, version);
  };
  await update(1);
  await refresh.click();
  await expect(
    pane.getByText("+latest footer 1", { exact: true }),
  ).toBeAttached();
  await page.getByRole("button", { name: "Load earlier messages" }).click();
  await expect(page.locator(".conversation-entry")).toHaveCount(100);
  const transcript = page.locator(".transcript");
  await transcript.evaluate((node) => {
    node.scrollTop = 250;
  });
  await input.fill("Keep this draft and caret");
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "edit" },
  });
  await expect(refresh).toBeEnabled();
  await panel.evaluate((node) => {
    node.scrollTop = 0;
  });
  await update(2);
  await input.focus();
  await page.clock.runFor(2600);
  const latest = pane.locator(".latest-change.added");
  await expect(latest).toHaveText("+latest footer 2\n");
  await expect(latest).toBeInViewport({ ratio: 1 });
  await expect(
    pane.getByText("+existing header", { exact: true }),
  ).not.toHaveClass(/latest-change/);
  expect(await panel.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
  expect(await transcript.evaluate((node) => node.scrollTop)).toBe(250);
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("Keep this draft and caret");
  await page.screenshot({ path: "test-results/latest-change-hunk.png" });
  await panel.evaluate((node) => {
    node.scrollTop = 80;
  });
  await page.clock.runFor(2600);
  expect(await panel.evaluate((node) => node.scrollTop)).toBe(80);
});

test("late Git responses cannot replace a newer selection or another project's diff", async ({
  page,
}) => {
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const refresh = pane.getByRole("button", { name: "Refresh Git status" });
  await expect(pane.getByText(/const layout = 'split'/)).toBeVisible();
  await page.evaluate(() => (window as any).fixture.hold("git_file_diff"));
  await refresh.click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === "git_file_diff",
          ).length,
      ),
    )
    .toBe(2);
  await pane
    .getByRole("button", { name: "src/styles.css, Staged modified" })
    .click();
  await page.evaluate(() => (window as any).fixture.release("git_file_diff"));
  await expect(pane.getByRole("region", { name: "Staged diff" })).toContainText(
    "background",
  );
  await expect(
    pane.getByRole("region", { name: "Unstaged diff", exact: true }),
  ).toHaveCount(0);

  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.hold("git_repository_status");
    fixture.setGitFiles(
      [
        {
          path: "src/App.tsx",
          indexStatus: " ",
          worktreeStatus: "M",
          modifiedAt: "999",
          revision: "other",
        },
      ],
      "/work/other",
    );
    fixture.setGitDiff(
      "src/App.tsx",
      { unstaged: "@@ -1 +1 @@\n-old\n+Other project" },
      "/work/other",
    );
  });
  await refresh.click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".project-row.selected")).toContainText("other");
  await page.evaluate(() =>
    (window as any).fixture.release("git_repository_status"),
  );
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("Other project");
  await expect(pane.locator(".git-file-item.latest")).toContainText(
    "src/App.tsx",
  );
  await expect(pane.getByText(/background: #0b0c0e/)).toHaveCount(0);
});

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} model favourites persist, stay independent and work across composer and settings`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    const name = provider === "codex" ? "Codex" : "OpenCode";
    const model = page.getByRole("button", { name: "Model", exact: true });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep my draft while favouriting");
    await choose(page, "Model", "Test model provider/model");
    await choose(page, "Effort", "High");
    await model.click();
    const popup = page.getByRole("dialog", {
      name: "Choose model",
      exact: true,
    });
    const search = popup.getByRole("combobox", { name: "Search models" });
    await popup
      .getByRole("button", {
        name: "Add Fast model (provider/fast) to favourites",
        exact: true,
      })
      .click();
    await expect(popup).toBeVisible();
    await expect(popup.getByText("Favourites", { exact: true })).toBeVisible();
    expect(
      await popup
        .locator(".choice-option")
        .evaluateAll((buttons) =>
          buttons.map((button) => button.getAttribute("aria-label")),
        ),
    ).toEqual([
      "Default model Use provider settings",
      "Fast model provider/fast",
      "Test model provider/model",
      "Simple model provider/plain",
    ]);
    await search.fill("plain");
    await search.press("Tab");
    const addSimple = popup.getByRole("button", {
      name: "Add Simple model (provider/plain) to favourites",
      exact: true,
    });
    await expect(addSimple).toBeFocused();
    await addSimple.press("Enter");
    await expect(popup).toBeVisible();
    await expect(search).toBeFocused();
    await search.press("Tab");
    await popup
      .getByRole("button", {
        name: "Remove Simple model (provider/plain) from favourites",
        exact: true,
      })
      .press("Space");
    await expect(addSimple).toBeVisible();
    await search.press("Escape");
    await expect(model).toContainText("Test model");
    await expect(model).toBeFocused();
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("High");
    await expect(input).toHaveValue("Keep my draft while favouriting");
    const dialog = await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await dialog
      .getByRole("button", { name: "Reset " + name + " defaults" })
      .click();
    await dialog
      .getByRole("button", { name: "Default model", exact: true })
      .click();
    const defaults = page.getByRole("dialog", {
      name: "Choose default model",
      exact: true,
    });
    await expect(
      defaults.getByRole("button", {
        name: "Remove Fast model (provider/fast) from favourites",
        exact: true,
      }),
    ).toBeVisible();
    await defaults.getByRole("combobox").press("Escape");
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await page.reload();
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await model.click();
    await expect(
      popup.getByRole("button", {
        name: "Remove Fast model (provider/fast) from favourites",
        exact: true,
      }),
    ).toBeVisible();
    await search.press("ArrowDown");
    await search.press("Enter");
    await expect(model).toContainText("Fast model");
    await choose(page, "Provider", provider === "codex" ? "OpenCode" : "Codex");
    await model.click();
    await expect(
      popup.getByRole("button", {
        name: "Add Fast model (provider/fast) to favourites",
        exact: true,
      }),
    ).toBeVisible();
    await search.press("Escape");
    await choose(page, "Provider", name);
    await model.click();
    await popup
      .getByRole("button", {
        name: "Remove Fast model (provider/fast) from favourites",
        exact: true,
      })
      .click();
    await expect(popup.getByText("Favourites", { exact: true })).toHaveCount(0);
    await search.press("Enter");
    await expect(model).toContainText("Fast model");
    expect(
      await page.evaluate(
        (provider) =>
          JSON.parse(localStorage.getItem("caelum.settings.v1")!)
            .favoriteModels[provider],
        provider,
      ),
    ).toEqual([]);
  });
}

test("OpenCode model sources distinguish routed models, support provider search and retain unavailable favourites", async ({
  page,
}) => {
  await desktop(page, null);
  const models = [
    {
      id: "anthropic/shared",
      name: "Shared model",
      provider: { id: "anthropic", name: "Anthropic" },
      efforts: [],
    },
    {
      id: "openrouter/anthropic/shared",
      name: "Shared model",
      provider: { id: "openrouter", name: "OpenRouter" },
      efforts: [],
    },
    {
      id: "work-gateway/local",
      name: "Local model",
      provider: { id: "work-gateway", name: "Company gateway" },
      efforts: [],
    },
    { id: "fallback/local", name: "Fallback model", efforts: [] },
    {
      id: "anthropic/fast",
      name: "Fast direct model",
      provider: { id: "anthropic", name: "Anthropic" },
      efforts: [],
    },
  ];
  await page.evaluate(
    (models) => (window as any).fixture.setModels("opencode", models),
    models,
  );
  await choose(page, "Provider", "OpenCode");
  const model = page.getByRole("button", { name: "Model", exact: true });
  await model.click();
  const popup = page.getByRole("dialog", {
    name: "Choose model",
    exact: true,
  });
  const search = popup.getByRole("combobox", { name: "Search models" });
  await expect(popup.locator(".choice-section")).toHaveText([
    "Anthropic",
    "Company gateway",
    "fallback",
    "OpenRouter",
  ]);
  expect(
    await popup
      .locator(".choice-option")
      .evaluateAll((buttons) =>
        buttons.map((button) => button.getAttribute("aria-label")),
      ),
  ).toEqual([
    "Default model Use provider settings",
    "Shared model anthropic/shared",
    "Fast direct model anthropic/fast",
    "Local model work-gateway/local",
    "Fallback model fallback/local",
    "Shared model openrouter/anthropic/shared",
  ]);
  await search.fill("Company gateway");
  await expect(popup.locator(".choice-section")).toHaveText([
    "Company gateway",
  ]);
  await expect(
    popup.getByRole("button", {
      name: "Local model work-gateway/local",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    popup.getByRole("button", {
      name: "Shared model anthropic/shared",
      exact: true,
    }),
  ).toHaveCount(0);
  await search.fill("fallback");
  await expect(popup.locator(".choice-source")).toHaveText("fallback");
  await search.fill("shared");
  await popup
    .getByRole("button", {
      name: "Add Shared model (openrouter/anthropic/shared) to favourites",
      exact: true,
    })
    .click();
  await expect(popup.locator(".choice-section")).toHaveText([
    "Favourites",
    "Anthropic",
  ]);
  await expect(
    popup.getByRole("button", {
      name: "Add Shared model (anthropic/shared) to favourites",
      exact: true,
    }),
  ).toBeVisible();
  await popup
    .getByRole("button", {
      name: "Shared model openrouter/anthropic/shared",
      exact: true,
    })
    .click();
  await expect(model.locator(".choice-trigger-source")).toHaveText(
    "OpenRouter",
  );
  const dialog = await openSettings(page, "Providers");
  await choose(page, "Default model", "Shared model anthropic/shared");
  await expect(
    dialog
      .getByRole("button", { name: "Default model", exact: true })
      .locator(".choice-trigger-source"),
  ).toHaveText("Anthropic");
  await page.evaluate(() => (window as any).fixture.setModels("opencode", []));
  await dialog.getByRole("button", { name: "Refresh OpenCode status" }).click();
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .click();
  const defaults = page.getByRole("dialog", {
    name: "Choose default model",
    exact: true,
  });
  await expect(
    defaults.getByRole("button", {
      name: "Remove Shared model (openrouter/anthropic/shared) from favourites",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    defaults.getByText("Saved selection · currently unavailable", {
      exact: true,
    }),
  ).toBeVisible();
  await defaults.getByRole("combobox").press("Escape");
  await page.evaluate(
    (models) => (window as any).fixture.setModels("opencode", models),
    models,
  );
  await dialog.getByRole("button", { name: "Refresh OpenCode status" }).click();
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .click();
  await expect(
    defaults.getByRole("button", {
      name: "Remove Shared model (openrouter/anthropic/shared) from favourites",
      exact: true,
    }),
  ).toBeVisible();
  await expect(defaults.locator(".choice-section")).toHaveText([
    "Favourites",
    "Anthropic",
    "Company gateway",
    "fallback",
  ]);
  await page.screenshot({
    path: "test-results/model-providers-favourites.png",
  });
  await defaults.getByRole("combobox").press("Escape");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Use the routed model");
  await input.press("Enter");
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === "opencode_start_session",
        ).args.model,
    ),
  ).toBe("openrouter/anthropic/shared");
});

test("Codex context usage restores native counts, stays session scoped and fits the compact composer", async ({
  page,
}) => {
  await desktop(page);
  await expect(
    page.getByRole("group", { name: "Context usage", exact: true }),
  ).toContainText("Context —");
  await page.evaluate(() =>
    (window as any).fixture.setCodexUsage("thread-a", {
      total: { totalTokens: 900000 },
      last: {
        totalTokens: 50000,
        inputTokens: 45000,
        outputTokens: 5000,
        cachedInputTokens: 30000,
        reasoningOutputTokens: 1000,
      },
      modelContextWindow: 200000,
    }),
  );
  await page.reload();
  const meter = page.getByRole("meter", {
    name: "Context usage",
    exact: true,
  });
  await expect(meter).toContainText("Context 25%");
  await meter.focus();
  await expect(page.getByRole("tooltip")).toContainText(
    "50,000 / 200,000 tokens",
  );
  await choose(page, "Model", "Fast model provider/fast");
  await expect(meter).toHaveAttribute("aria-valuenow", "50000");
  await expect(meter).toHaveAttribute("aria-valuemax", "200000");
  await page
    .getByRole("button", {
      name: "Review the provider adapters",
      exact: true,
    })
    .click();
  await expect(meter).toHaveCount(0);
  await emit(page, "thread/tokenUsage/updated", {
    threadId: "thread-b",
    tokenUsage: { last: { totalTokens: 10000 }, modelContextWindow: 100000 },
  });
  await expect(meter).toContainText("Context 10%");
  await page
    .getByRole("button", {
      name: "Refine the session workspace",
      exact: true,
    })
    .click();
  await expect(meter).toContainText("Context 25%");
  await page.getByRole("button", { name: "Load earlier messages" }).click();
  await expect(meter).toContainText("Context 25%");
  await page.setViewportSize({ width: 800, height: 620 });
  await meter.focus();
  await expect(meter).toBeInViewport({ ratio: 1 });
  await expect(page.getByRole("tooltip")).toBeInViewport({ ratio: 1 });
  await expect(
    page.getByRole("button", { name: "Send message", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/context-usage-compact.png" });
  await emit(page, "thread/tokenUsage/updated", {
    threadId: "thread-a",
    tokenUsage: { last: { totalTokens: 10000 }, modelContextWindow: null },
  });
  await expect(meter).toHaveCount(0);
  const unknownLimit = page.getByRole("group", {
    name: "Context usage",
    exact: true,
  });
  await unknownLimit.focus();
  await expect(unknownLimit).toContainText("Context 10K");
  await expect(page.getByRole("tooltip")).toContainText(
    "Context window size unavailable.",
  );
  await emit(page, "thread/tokenUsage/updated", {
    threadId: "thread-a",
    tokenUsage: { last: { totalTokens: -1 }, modelContextWindow: 0 },
  });
  await expect(unknownLimit).toContainText("Context 10K");
  await page.getByRole("button", { name: "New session" }).click();
  await expect(unknownLimit).toHaveCount(0);
});

test("OpenCode context usage follows live steps, rejects stale history and resets after compaction", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  const tokens = {
    input: 80000,
    output: 10000,
    reasoning: 2000,
    cache: { read: 9000, write: 1000 },
  };
  const history = [
    {
      id: "msg-usage",
      type: "assistant",
      time: { created: 100, completed: 200 },
      model: { providerID: "provider", id: "model" },
      tokens,
      content: [{ type: "text", text: "Last measured response" }],
    },
  ];
  await setHistory(page, "opencode", history, false);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const meter = page.getByRole("meter", {
    name: "Context usage",
    exact: true,
  });
  await expect(meter).toContainText("Context 50%");
  await meter.focus();
  await expect(page.getByRole("tooltip")).toContainText(
    "100,000 / 200,000 tokens",
  );
  await choose(page, "Model", "Fast model provider/fast");
  await expect(meter).toHaveAttribute("aria-valuemax", "200000");
  await page.evaluate(() => {
    (window as any).fixture.holdHistory();
    window.dispatchEvent(new Event("focus"));
  });
  await emit(
    page,
    "session.step.started",
    {
      sessionID: "ses-a",
      assistantMessageID: "live",
      started: 300,
      model: { providerID: "provider", id: "fast" },
    },
    "opencode",
    { created: 300 },
  );
  await emit(
    page,
    "session.step.ended",
    {
      sessionID: "ses-a",
      assistantMessageID: "live",
      tokens: {
        input: 12000,
        output: 2000,
        reasoning: 1000,
        cache: { read: 2000, write: 0 },
      },
    },
    "opencode",
    { created: 400 },
  );
  await expect(meter).toContainText("Context 25%");
  await expect(meter).toHaveAttribute("aria-valuemax", "64000");
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await page.clock.runFor(100);
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await page.clock.runFor(100);
  await expect(meter).toContainText("Context 25%");
  await emit(
    page,
    "session.usage.updated",
    { sessionID: "ses-a", tokens: { ...tokens, input: 900000 } },
    "opencode",
  );
  await expect(meter).toContainText("Context 25%");
  await emit(
    page,
    "session.compaction.started",
    { sessionID: "ses-a", reason: "auto" },
    "opencode",
    { id: "evt_compact", created: 500 },
  );
  await emit(
    page,
    "session.compaction.ended",
    { sessionID: "ses-a", text: "Compact summary", tokens },
    "opencode",
    { created: 600 },
  );
  const unknown = page.getByRole("group", {
    name: "Context usage",
    exact: true,
  });
  await expect(unknown).toContainText("Context —");
  await page.clock.runFor(100);
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await page.clock.runFor(100);
  await expect(unknown).toContainText("Context —");
  await setHistory(
    page,
    "opencode",
    [
      ...history,
      {
        id: "msg_compact",
        type: "compaction",
        status: "completed",
        time: { created: 500 },
        reason: "auto",
        summary: "Compact summary",
        tokens,
      },
      {
        id: "after",
        type: "assistant",
        time: { created: 700, completed: 800 },
        model: { providerID: "provider", id: "fast" },
        tokens: { ...tokens, input: 12000 },
        content: [{ type: "text", text: "After compaction" }],
      },
    ],
    false,
  );
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await expect(meter).toContainText("Context 50%");
  await emit(page, "server.disconnected", {}, "opencode");
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(meter).toContainText("Context 50%");
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await meter.focus();
  await expect(page.getByRole("tooltip")).toContainText(
    "32,000 / 64,000 tokens",
  );
  await page.screenshot({ path: "test-results/context-usage-light.png" });
});

test("model picker searches capabilities, supports keyboard selection and returns focus", async ({
  page,
}) => {
  await desktop(page, null);
  const model = page.getByRole("button", { name: "Model", exact: true });
  await model.focus();
  await model.press("ArrowDown");
  const search = page.getByRole("combobox", { name: "Search models" });
  await expect(search).toBeFocused();
  await search.fill("provider/fast");
  await expect(
    page
      .getByRole("grid", { name: "Model", exact: true })
      .locator(".choice-option"),
  ).toHaveCount(1);
  await search.press("Enter");
  await expect(model).toContainText("Fast model");
  await expect(model).toBeFocused();
  await model.click();
  await search.fill("missing");
  await expect(page.getByText("No matching models")).toBeVisible();
  await search.press("Escape");
  await expect(model).toBeFocused();
  await model.click();
  await search.press("ArrowUp");
  await search.press("Enter");
  await expect(model).toContainText("Test model");
  await model.click();
  await page.screenshot({ path: "test-results/model-picker.png" });
  await page.getByRole("heading", { name: "New session", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Choose model" })).toHaveCount(
    0,
  );
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        call.command.endsWith("_start_session"),
      ),
    ),
  ).toHaveLength(0);
  await page.setViewportSize({ width: 800, height: 620 });
  await model.click();
  await expect(
    page.getByRole("dialog", { name: "Choose model" }),
  ).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: "test-results/model-picker-minimum.png" });
});

test("restored shell notifications and nested tool results display identity and output", async ({
  page,
}) => {
  await desktop(page, null);
  const id = "sh_1894f7b4e001iOMXKwJMCx7VJX";
  await page.evaluate(
    ({ id }) =>
      (window as any).fixture.setOpenHistory([
        {
          id: "shell-message",
          type: "user",
          text: `<shell id="${id}" state="completed" command="nix develop --command cargo test --workspace">\nCompiling caelum-core\ntest result: ok. 53 passed; 0 failed;\n</shell>`,
        },
        {
          id: "tool-message",
          type: "assistant",
          content: [
            {
              id: "nested-shell",
              type: "tool",
              name: "shell",
              state: {
                status: "completed",
                input: { command: "grep schema" },
                content: [
                  {
                    type: "text",
                    text: '<shell id="sh_nested" state="completed" command="grep schema">schema version 3</shell>',
                  },
                ],
              },
            },
          ],
        },
      ]),
    { id },
  );
  await choose(page, "Provider", "OpenCode");
  await page.getByRole("button", { name: /OpenCode session/ }).click();
  const notification = page.getByRole("button", {
    name: /Run nix develop --command cargo test --workspace/,
  });
  await notification.click();
  await expect(page.getByText(id, { exact: true })).toBeVisible();
  await expect(page.getByText(/test result: ok. 53 passed/)).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Copy shell ID" }),
  ).toBeVisible();
  await expect(page.locator(".conversation-entry.user")).toHaveCount(0);
  await expect(page.locator(".tool-activity").first()).toHaveCSS(
    "border-top-width",
    "0px",
  );
  await page.mouse.move(0, 0);
  await expect(notification).toHaveCSS("background-color", "rgba(0, 0, 0, 0)");
  await page.getByRole("button", { name: /shell grep schema/ }).click();
  await expect(page.getByText("sh_nested", { exact: true })).toBeVisible();
  await expect(
    page.getByText("schema version 3", { exact: true }),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/shell-results.png" });
});

test("sending from expanded tools or older history follows live output while the composer stays active", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const activity = page.getByRole("form", { name: "Message composer" });
  await page.getByRole("button", { name: /Run pnpm test/ }).click();
  await input.fill("Continue after expanding a tool");
  await input.press("Enter");
  await expect(activity).toHaveAttribute("data-working", "true");
  await emit(page, "item/reasoning/summaryTextDelta", {
    threadId: "thread-a",
    itemId: "thinking",
    summaryIndex: 0,
    delta: "Inspect the existing implementation",
  });
  await expect(activity).toHaveAttribute("data-working", "true");
  await expect(page.locator(".reasoning-preview")).toContainText(
    "Inspect the existing implementation",
  );
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "live-one",
    delta: "Live after tool expansion",
  });
  await expect(
    page.getByText("Live after tool expansion", { exact: true }),
  ).toBeInViewport();
  await page.getByRole("button", { name: "Load earlier messages" }).click();
  await page.getByRole("button", { name: "Load earlier messages" }).click();
  const before = await page
    .locator(".transcript")
    .evaluate((node) => node.scrollTop);
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "live-one",
    delta: " continues",
  });
  expect(
    await page.locator(".transcript").evaluate((node) => node.scrollTop),
  ).toBe(before);
  await expect(activity).toBeInViewport({ ratio: 1 });
  await expect(activity).toHaveAttribute("data-working", "true");
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(activity).toBeInViewport({ ratio: 1 });
  await expect(input).toBeInViewport({ ratio: 1 });
  await page.getByRole("button", { name: "Stop generation" }).click();
  await expect(activity).toHaveAttribute("data-working", "false");
  await input.fill("Continue from older history");
  await input.press("Enter");
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "live-two",
    delta: "Live after reading older messages",
  });
  await expect(
    page.getByText("Live after reading older messages", { exact: true }),
  ).toBeInViewport();
  await expect(
    page.getByRole("button", { name: "Jump to latest" }),
  ).toHaveCount(0);
});

test("OpenCode keeps the composer active through thinking, retries and tools, with an approval cue", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.getByRole("button", { name: /OpenCode session/ }).click();
  await expect(page.getByText("Restored OpenCode conversation")).toBeVisible();
  const activity = page.getByRole("form", { name: "Message composer" });
  const send = (method: string, data = {}) =>
    emit(
      page,
      method,
      {
        sessionID: "ses-a",
        assistantMessageID: "thinking-answer",
        ordinal: 0,
        ...data,
      },
      "opencode",
    );
  await send("session.reasoning.started");
  await expect(activity).toHaveAttribute("data-working", "true");
  await send("session.reasoning.delta", {
    delta: "**Inspecting the event stream**\n\nCheck ",
  });
  const trace = page.locator("details.reasoning");
  await expect(trace.locator("summary")).toContainText(
    "Inspecting the event stream",
  );
  await trace.locator("summary").click();
  await send("session.reasoning.delta", {
    delta: "the subscription and snapshot.",
  });
  await expect(trace.locator(".reasoning-content")).toContainText(
    "Check the subscription and snapshot.",
  );
  await send("session.retry.scheduled", {
    attempt: 2,
    error: { message: "Provider temporarily unavailable" },
  });
  await expect(activity).toHaveAttribute("data-working", "true");
  await expect(page.getByRole("status", { name: "Agent activity" })).toHaveText(
    "Retrying · attempt 2",
  );
  await page.evaluate(() =>
    (window as any).fixture.emit("approval-requests", [
      {
        provider: "opencode",
        projectPath: "/work/caelum",
        sessionId: "ses-a",
        requestId: "approval-live",
        summary: "Read source?",
        description: "src/App.tsx",
      },
    ]),
  );
  await expect(page.locator(".composer-approval")).toHaveText(
    "Approval required",
  );
  await page.getByRole("button", { name: "Allow once" }).click();
  await send("session.reasoning.ended", {
    text: "**Inspecting the event stream**\n\nCheck the subscription and snapshot.",
  });
  await expect(trace.locator("summary")).toContainText("Thought");
  await send("session.tool.called", {
    id: "read-live",
    name: "read",
    input: { path: "src/App.tsx" },
  });
  await expect(activity).toHaveAttribute("data-working", "true");
  await page.screenshot({ path: "test-results/live-thinking.png" });
  await emit(page, "server.disconnected", {}, "opencode");
  await expect(activity).toHaveAttribute("data-working", "false");
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeDisabled();
  await page.evaluate(() => {
    (window as any).fixture.setOpenWorking(false);
    (window as any).fixture.setOpenHistory([
      {
        id: "thinking-answer",
        type: "assistant",
        time: { completed: 42 },
        content: [
          {
            type: "reasoning",
            text: "Recovered the complete thinking trace",
          },
          { type: "text", text: "Recovered final response" },
        ],
      },
    ]);
  });
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(trace).toHaveCount(1);
  await expect(trace.locator("summary")).toContainText(
    "Recovered the complete thinking trace",
  );
  await page.getByRole("button", { name: "Jump to latest" }).click();
  await expect(page.getByText("Recovered final response")).toBeVisible();
  await expect(activity).toHaveAttribute("data-working", "false");
});

for (const provider of ["codex", "opencode"]) {
  const title =
    provider === "codex" ? "Refine the session workspace" : "OpenCode session";
  const id = provider === "codex" ? "thread-a" : "ses-a";

  test(`${provider} clears the cancellation notice on a new prompt, history refresh and reconnect`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const activity = page.getByRole("form", { name: "Message composer" });
    const cancelled = page.getByText("Turn cancelled", { exact: true });
    for (const resume of ["prompt", "snapshot", "reconnect"]) {
      const history =
        provider === "codex"
          ? [{ id: `cancelled-${resume}`, status: "interrupted", items: [] }]
          : [
              {
                id: `cancelled-${resume}`,
                type: "idle",
                outcome: "interrupted",
              },
            ];
      await setHistory(page, provider, history, true);
      await emit(
        page,
        provider === "codex" ? "turn/started" : "session.execution.started",
        provider === "codex"
          ? { threadId: id, turn: { id: `turn-${resume}` } }
          : { sessionID: id },
        provider,
      );
      await page.getByRole("button", { name: "Stop generation" }).click();
      await setHistory(page, provider, history, false);
      await expect(cancelled).toHaveCount(1);
      await expect(activity).toHaveAttribute("data-working", "false");
      await page.evaluate(() => window.dispatchEvent(new Event("focus")));
      await expect(cancelled).toHaveCount(1);

      const response =
        provider === "codex"
          ? {
              id: `resumed-${resume}`,
              status: "inProgress",
              items: [
                {
                  id: `answer-${resume}`,
                  type: "agentMessage",
                  text: "Resumed work",
                },
              ],
            }
          : {
              id: `resumed-${resume}`,
              type: "assistant",
              content: [{ type: "text", text: "Resumed work" }],
            };
      await setHistory(page, provider, [...history, response], true);
      if (resume === "prompt") {
        const input = page.getByRole("textbox", {
          name: "Message",
          exact: true,
        });
        await input.fill("Continue the cancelled task");
        await input.press("Enter");
      } else if (resume === "reconnect") {
        await emit(
          page,
          provider === "codex" ? "codex/processExited" : "server.disconnected",
          {},
          provider,
        );
        await page
          .getByRole("button", { name: "Reconnect", exact: true })
          .click();
      } else
        await page.evaluate(() => window.dispatchEvent(new Event("focus")));
      await expect(activity).toHaveAttribute("data-working", "true");
      await expect(cancelled).toHaveCount(0);
      const finished =
        provider === "codex"
          ? [...history, { ...response, status: "completed" }]
          : [
              ...history,
              { ...response, time: { completed: 200 } },
              { id: `done-${resume}`, type: "idle", outcome: "succeeded" },
            ];
      await setHistory(page, provider, finished, false);
      await page.evaluate(() => window.dispatchEvent(new Event("focus")));
      await expect(activity).toHaveAttribute("data-working", "false");
      await expect(cancelled).toHaveCount(0);
      await page
        .getByRole("button", {
          name: "Review the provider adapters",
          exact: true,
        })
        .click();
      await page.getByRole("button", { name: title, exact: true }).click();
      await expect(cancelled).toHaveCount(0);
    }
  });

  test(`${provider} restores compaction checkpoints and preserves drafts and reading position while work continues`, async ({
    page,
  }) => {
    await desktop(page, null);
    const summary =
      "## Retained context\n\nKeep the **release plan** and next steps.";
    const earlier =
      provider === "codex"
        ? [
            {
              id: "prior-turn",
              status: "completed",
              items: [
                {
                  id: "prior-user",
                  type: "userMessage",
                  content: [{ type: "text", text: "Keep the release plan" }],
                },
                ...Array.from({ length: 30 }, (_, index) => ({
                  id: `prior-${index}`,
                  type: "agentMessage",
                  text: `Earlier response ${index}`,
                })),
                {
                  id: "restored",
                  type: "contextCompaction",
                  startedAtMs: 100,
                  completedAtMs: 200,
                },
              ],
            },
          ]
        : [
            {
              id: "prior-user",
              type: "user",
              text: "Keep the release plan",
            },
            ...Array.from({ length: 30 }, (_, index) => ({
              id: `prior-${index}`,
              type: "assistant",
              time: { completed: 100 },
              content: [{ type: "text", text: `Earlier response ${index}` }],
            })),
            {
              id: "msg_restored",
              type: "compaction",
              status: "completed",
              reason: "auto",
              summary,
              time: { created: 100 },
            },
          ];
    await setHistory(page, provider, earlier, false);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await page.getByRole("button", { name: title, exact: true }).click();
    const notices = page.getByRole("region", {
      name: "Context compaction",
    });
    const activity = page.getByRole("form", { name: "Message composer" });
    const input = page.getByRole("textbox", {
      name: "Message",
      exact: true,
    });
    const transcript = page.locator(".transcript");
    await expect(notices).toHaveCount(1);
    await expect(notices).toHaveAttribute("data-status", "completed");
    await input.press("ArrowUp");
    await expect(input).toHaveValue("Keep the release plan");
    await input.fill("Draft for after compaction");

    const running =
      provider === "codex"
        ? {
            id: "live-turn",
            status: "inProgress",
            items: [
              { id: "live", type: "contextCompaction", startedAtMs: 300 },
            ],
          }
        : {
            id: "msg_live",
            type: "compaction",
            status: "running",
            reason: "auto",
            summary: "",
            time: { created: 300 },
          };
    await setHistory(page, provider, [...earlier, running], true);
    await emit(
      page,
      provider === "codex" ? "item/started" : "session.compaction.started",
      provider === "codex"
        ? {
            threadId: id,
            item: {
              id: "live",
              type: "contextCompaction",
              startedAtMs: 300,
            },
          }
        : { sessionID: id, reason: "auto" },
      provider,
      { id: "evt_live", created: 300 },
    );
    await expect(notices).toHaveCount(2);
    await expect(notices.last()).toHaveAttribute("data-status", "running");
    await expect(
      page.getByRole("status", { name: "Agent activity" }),
    ).toHaveText("Compacting context…");
    await expect(activity).toHaveAttribute("data-working", "true");
    await transcript.evaluate((node) => {
      node.scrollTop = 200;
    });
    await expect(
      page.getByRole("button", { name: "Jump to latest" }),
    ).toBeVisible();
    await page
      .getByRole("button", {
        name: "Review the provider adapters",
        exact: true,
      })
      .click();
    await expect(input).toHaveValue("");

    const completed =
      provider === "codex"
        ? {
            id: "live-turn",
            status: "inProgress",
            items: [
              {
                id: "live",
                type: "contextCompaction",
                startedAtMs: 300,
                completedAtMs: 400,
              },
            ],
          }
        : { ...running, status: "completed", summary };
    await setHistory(page, provider, [...earlier, completed], true);
    await emit(
      page,
      provider === "codex" ? "item/completed" : "session.compaction.ended",
      provider === "codex"
        ? {
            threadId: id,
            item: {
              id: "live",
              type: "contextCompaction",
              completedAtMs: 400,
            },
          }
        : { sessionID: id, reason: "auto", text: summary },
      provider,
      { id: "evt_end", created: 400 },
    );
    await expect(
      page.getByRole("region", { name: "Context compaction" }),
    ).toHaveCount(0);
    await expect(activity).toHaveAttribute("data-working", "false");
    await page.getByRole("button", { name: title, exact: true }).click();
    await expect(notices).toHaveCount(2);
    await expect(notices.last()).toHaveAttribute("data-status", "completed");
    await expect(activity).toHaveAttribute("data-working", "true");
    await expect(input).toHaveValue("Draft for after compaction");
    await expect
      .poll(() => transcript.evaluate((node) => node.scrollTop))
      .toBe(200);
    await emit(
      page,
      provider === "codex" ? "item/agentMessage/delta" : "session.text.delta",
      provider === "codex"
        ? {
            threadId: id,
            itemId: "after",
            delta: "Continuing after compaction",
          }
        : {
            sessionID: id,
            assistantMessageID: "after",
            ordinal: 0,
            delta: "Continuing after compaction",
          },
      provider,
    );
    await expect
      .poll(() => transcript.evaluate((node) => node.scrollTop))
      .toBe(200);
    await page.getByRole("button", { name: "Jump to latest" }).click();
    await expect(
      page.getByText("Continuing after compaction", { exact: true }),
    ).toBeInViewport();
    if (provider === "opencode") {
      await notices
        .last()
        .getByRole("button", { name: "Context compacted" })
        .click();
      await expect(
        notices.last().getByRole("heading", { name: "Retained context" }),
      ).toBeVisible();
      await expect(notices.last().locator("strong")).toHaveText("release plan");
    } else await expect(notices.last().getByRole("button")).toHaveCount(0);
    await page.screenshot({
      path: `test-results/${provider}-compaction.png`,
    });
    await setHistory(
      page,
      provider,
      [
        ...earlier,
        {
          ...completed,
          ...(provider === "codex" ? { status: "completed" } : {}),
        },
      ],
      false,
    );
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.succeeded",
      provider === "codex"
        ? { threadId: id, turn: { id: "live-turn", status: "completed" } }
        : { sessionID: id },
      provider,
    );
    await expect(activity).toHaveAttribute("data-working", "false");
    await expect(notices).toHaveCount(2);
    await expect(
      page.getByText("Earlier response 0", { exact: true }),
    ).toHaveCount(1);
    await expect(input).toHaveValue("Draft for after compaction");
  });

  test(`${provider} shows failed and interrupted compaction and recovers a disconnected checkpoint`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const notices = page.getByRole("region", {
      name: "Context compaction",
    });
    const activity = page.getByRole("form", { name: "Message composer" });
    const input = page.getByRole("textbox", {
      name: "Message",
      exact: true,
    });
    await input.fill("Keep this through reconnect");
    const history: unknown[] = [];
    for (const outcome of ["failed", "interrupted", "disconnected"]) {
      const item = {
        id: outcome,
        type: "contextCompaction",
        startedAtMs: 100 + history.length,
      };
      const running =
        provider === "codex"
          ? { id: `turn-${outcome}`, status: "inProgress", items: [item] }
          : {
              id: `msg_${outcome}`,
              type: "compaction",
              status: "running",
              reason: "auto",
              summary: "",
              time: { created: item.startedAtMs },
            };
      await setHistory(page, provider, [...history, running], true);
      await emit(
        page,
        provider === "codex" ? "item/started" : "session.compaction.started",
        provider === "codex"
          ? { threadId: id, item }
          : { sessionID: id, reason: "auto" },
        provider,
        { id: `evt_${outcome}`, created: item.startedAtMs },
      );
      await expect(notices.last()).toHaveAttribute("data-status", "running");
      if (outcome === "failed") {
        const error = { message: "Summary request failed" };
        history.push({ ...running, status: "failed", error });
        await setHistory(page, provider, history, false);
        if (provider === "opencode")
          await emit(
            page,
            "session.compaction.failed",
            { sessionID: id, reason: "auto", error },
            provider,
            { id: "evt_failure", created: 101 },
          );
        await emit(
          page,
          provider === "codex" ? "turn/completed" : "session.execution.failed",
          provider === "codex"
            ? {
                threadId: id,
                turn: { id: `turn-${outcome}`, status: "failed", error },
              }
            : { sessionID: id, error },
          provider,
        );
        await expect(notices.last()).toHaveAttribute("data-status", "failed");
        await expect(notices.last().getByRole("alert")).toHaveText(
          "Summary request failed",
        );
      } else if (outcome === "interrupted") {
        history.push(
          provider === "codex"
            ? { ...running, status: "interrupted" }
            : running,
        );
        if (provider === "opencode")
          history.push({
            id: "msg_idle",
            type: "idle",
            outcome: "interrupted",
          });
        await setHistory(page, provider, history, false);
        await page.getByRole("button", { name: "Stop generation" }).click();
        await expect(notices.last()).toHaveAttribute(
          "data-status",
          "interrupted",
        );
      } else {
        await emit(
          page,
          provider === "codex" ? "codex/processExited" : "server.disconnected",
          {},
          provider,
        );
        await expect(notices.last()).toHaveAttribute(
          "data-status",
          "interrupted",
        );
        await expect(input).toBeDisabled();
        history.push(
          provider === "codex"
            ? {
                ...running,
                status: "completed",
                items: [{ ...item, completedAtMs: 200 }],
              }
            : {
                ...running,
                status: "completed",
                native: { encrypted_content: "opaque-provider-checkpoint" },
              },
        );
        await setHistory(page, provider, history, false);
        await page
          .getByRole("button", { name: "Reconnect", exact: true })
          .click();
        await expect(input).toBeEnabled();
        await expect(notices).toHaveCount(3);
        await expect(notices.last()).toHaveAttribute(
          "data-status",
          "completed",
        );
        await expect(notices.last().getByRole("button")).toHaveCount(0);
        await expect(page.getByText("opaque-provider-checkpoint")).toHaveCount(
          0,
        );
        await expect(notices.first()).toHaveAttribute("data-status", "failed");
        await expect(notices.nth(1)).toHaveAttribute(
          "data-status",
          "interrupted",
        );
      }
      await expect(activity).toHaveAttribute("data-working", "false");
      await expect(input).toHaveValue("Keep this through reconnect");
    }
  });
}

test("OpenCode reconciles manual compaction across stale snapshots, streamed summaries and missed starts", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(page.getByText("Restored OpenCode conversation")).toBeVisible();
  const snapshot = {
    id: "msg_manual",
    type: "compaction",
    status: "running",
    reason: "manual",
    summary: "Keep the decisions",
    time: { created: 100 },
  };
  await setHistory(page, "opencode", [snapshot], true);
  await page.evaluate(() => (window as any).fixture.holdHistory());
  await emit(
    page,
    "session.compaction.started",
    { sessionID: "ses-a", inputID: "msg_manual", reason: "manual" },
    "opencode",
    { id: "evt_start", created: 100 },
  );
  const notice = page.getByRole("region", { name: "Context compaction" });
  await expect(notice).toHaveCount(1);
  await expect(notice).toHaveAttribute("data-status", "running");
  await emit(
    page,
    "session.compaction.delta",
    { sessionID: "ses-a", text: "Keep the" },
    "opencode",
    { created: 110 },
  );
  await notice.getByRole("button", { name: "Compacting context…" }).click();
  await emit(
    page,
    "session.compaction.delta",
    { sessionID: "ses-a", text: " decisions" },
    "opencode",
    { created: 120 },
  );
  await expect(notice.locator(".compaction-details")).toHaveText(
    "Keep the decisions",
  );
  const summary = "## Retained context\n\nKeep the decisions and next steps.";
  await emit(
    page,
    "session.compaction.ended",
    { sessionID: "ses-a", reason: "manual", text: summary },
    "opencode",
    { id: "evt_end", created: 130 },
  );
  await expect(notice).toHaveAttribute("data-status", "completed");
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveAttribute("data-working", "true");
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await setHistory(
    page,
    "opencode",
    [{ ...snapshot, status: "completed", summary }],
    false,
  );
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await expect(notice).toHaveCount(1);
  await expect(notice).toHaveAttribute("data-status", "completed");
  await expect(
    notice.getByRole("heading", { name: "Retained context" }),
  ).toBeVisible();
  await expect(notice.locator(".compaction-details")).toContainText(
    "Keep the decisions and next steps.",
  );
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveAttribute("data-working", "false");
  await setHistory(
    page,
    "opencode",
    [
      { ...snapshot, status: "completed", summary },
      {
        id: "msg_missed",
        type: "compaction",
        reason: "auto",
        status: "completed",
        summary: "Recovered after missing the start",
        time: { created: 200 },
      },
    ],
    false,
  );
  await emit(
    page,
    "session.compaction.ended",
    {
      sessionID: "ses-a",
      reason: "auto",
      text: "Recovered after missing the start",
    },
    "opencode",
    { id: "evt_missed_end", created: 220 },
  );
  await expect(notice).toHaveCount(2);
  await expect(notice.last()).toHaveAttribute("data-status", "completed");
  await notice
    .last()
    .getByRole("button", { name: "Context compacted" })
    .click();
  await expect(notice.last()).toContainText(
    "Recovered after missing the start",
  );
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toHaveCount(0);
});

test("patch and execute rows describe restored and live work and retain full tool input", async ({
  page,
}) => {
  await desktop(page, null);
  const tools = [
    {
      name: "patch",
      input: {
        patchText: [
          "*** Begin Patch",
          "*** Update File: src/App.tsx",
          "@@",
          '-export const theme = "dark";',
          "-export const ready = false;",
          "-export const limit = 10;",
          "-export const obsolete = true;",
          '+export const theme = "light";',
          "+export const ready = true;",
          "+export const limit = 20;",
          "+export const lastLine = true;",
          "*** Add File: src/new file.rs",
          "+pub fn answer() -> u32 {",
          "+    42",
          "+}",
          "*** Delete File: src/unused.ts",
          "*** End Patch",
        ].join("\n"),
      },
      preview: "src/App.tsx, src/new file.rs, src/unused.ts",
    },
    {
      name: "execute",
      input: {
        code: '\nconst result = await tools.opencode.models({\n  provider: "openai"\n});\nreturn result;',
      },
      preview:
        'const result = await tools.opencode.models({ provider: "openai" }); return result;',
    },
  ];
  await setHistory(
    page,
    "opencode",
    [
      {
        id: "msg_tools",
        type: "assistant",
        time: { completed: 100 },
        content: tools.map((tool) => ({
          id: `restored-${tool.name}`,
          type: "tool",
          name: tool.name,
          state: {
            status: "completed",
            input: tool.input,
            content: [{ type: "text", text: "Done" }],
          },
        })),
      },
    ],
    false,
  );
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await page.evaluate(() => (window as any).fixture.setOpenWorking(true));
  const composer = page.getByRole("textbox", { name: "Message", exact: true });
  await composer.fill("Keep my patch review draft");
  for (const tool of tools) {
    const restored = page.locator(
      `[data-entry-id="opencode-tool-restored-${tool.name}"]`,
    );
    await expect(restored.locator(".tool-preview")).toHaveText(tool.preview);
    if (tool.name === "patch") {
      await expect(restored.locator(".tool-summary")).toHaveAttribute(
        "aria-expanded",
        "false",
      );
      const preview = restored.getByRole("group", { name: "Patch preview" });
      await expect(preview.locator(".tool-patch-file")).toHaveCount(2);
      await expect(preview.locator(".diff-line.removed")).toHaveCount(3);
      await expect(preview.locator(".diff-line.added")).toHaveCount(6);
      await expect(preview).toContainText("2 more changed lines");
      await expect(preview).toContainText("1 more file");
      await expect(preview).not.toContainText("lastLine");
      await expect(
        preview.locator(".syntax-keyword").filter({ hasText: /^pub$/ }),
      ).toBeVisible();
    } else await expect(restored.locator(".tool-patch-preview")).toHaveCount(0);
    await restored
      .getByRole("button", { name: new RegExp(`^${tool.name} `) })
      .click();
    await expect(restored.locator(".tool-value pre").first()).toHaveText(
      JSON.stringify(tool.input, null, 2),
    );
    if (tool.name === "patch") {
      await restored
        .getByRole("button", { name: "Hide full patch input" })
        .press("Enter");
      await expect(restored.locator(".tool-details")).toHaveCount(0);
      await restored
        .getByRole("button", { name: "Show full patch input" })
        .press("Enter");
      await expect(restored.locator(".tool-value pre").first()).toContainText(
        "lastLine",
      );
    }
    await restored.locator(".tool-summary").click();
    await composer.focus();
    const serialized = JSON.stringify(tool.input);
    await emit(
      page,
      "session.tool.input.delta",
      {
        sessionID: "ses-a",
        id: `live-${tool.name}`,
        name: tool.name,
        delta: serialized.slice(0, 30),
      },
      "opencode",
    );
    const live = page.locator(
      `[data-entry-id="opencode-tool-live-${tool.name}"]`,
    );
    await expect(live.locator(".tool-patch-preview")).toHaveCount(0);
    await emit(
      page,
      "session.tool.input.delta",
      {
        sessionID: "ses-a",
        id: `live-${tool.name}`,
        name: tool.name,
        delta: serialized.slice(30),
      },
      "opencode",
    );
    await expect(live.locator(".tool-preview")).toHaveText(tool.preview);
    if (tool.name === "patch") {
      await expect(
        live
          .locator(".tool-patch-preview .syntax-keyword")
          .filter({ hasText: /^pub$/ }),
      ).toBeVisible();
      await expect(composer).toBeFocused();
    }
    await emit(
      page,
      "session.tool.called",
      {
        sessionID: "ses-a",
        id: `live-${tool.name}`,
        name: tool.name,
        input: tool.input,
      },
      "opencode",
    );
    await expect(live.locator(".tool-preview")).toHaveAttribute(
      "title",
      tool.preview,
    );
    await emit(
      page,
      "session.tool.success",
      {
        sessionID: "ses-a",
        id: `live-${tool.name}`,
        content: [{ type: "text", text: "Done" }],
      },
      "opencode",
    );
    await expect(live.locator(".tool-preview")).toHaveText(tool.preview);
    if (tool.name === "patch") {
      await live.locator(".diff-line.added").first().click({ button: "right" });
      await pickMenu(page, "Copy input");
      await expect
        .poll(() => copiedText(page))
        .toBe(JSON.stringify(tool.input, null, 2));
      await expect(live.locator(".tool-patch-preview")).toBeVisible();
    }
  }
  await page.screenshot({ path: "test-results/tool-previews.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  const livePatch = page.locator('[data-entry-id="opencode-tool-live-patch"]');
  await livePatch.scrollIntoViewIfNeeded();
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/tool-patch-compact.png" });
  const added = livePatch.locator(".diff-line.added").first();
  await expect(added).toHaveCSS("background-color", "rgb(20, 35, 25)");
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(added).toHaveCSS("background-color", "rgb(232, 243, 233)");
  await expect(composer).toHaveValue("Keep my patch review draft");
  await livePatch.scrollIntoViewIfNeeded();
  await page.screenshot({ path: "test-results/tool-patch-light.png" });
});

test("OpenCode stays active across quiet tool gaps and repeated history polls, then settles on completion", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(page.getByText("Restored OpenCode conversation")).toBeVisible();
  await page.evaluate(() => (window as any).fixture.setOpenWorking(true));
  await emit(
    page,
    "session.execution.started",
    { sessionID: "ses-a" },
    "opencode",
  );
  const activity = page.getByRole("form", { name: "Message composer" });
  await expect(activity).toHaveAttribute("data-working", "true");
  await activity.evaluate((node) => {
    (window as any).fixture.activity = [];
    new MutationObserver((records) => {
      for (const record of records)
        (window as any).fixture.activity.push(record.oldValue);
    }).observe(node, {
      attributes: true,
      attributeFilter: ["data-working"],
      attributeOldValue: true,
    });
  });
  for (let index = 0; index < 3; index++) {
    await emit(
      page,
      "session.tool.called",
      {
        sessionID: "ses-a",
        id: `tool-${index}`,
        name: "read",
        input: { path: "src/App.tsx" },
      },
      "opencode",
    );
    await emit(
      page,
      "session.tool.success",
      { sessionID: "ses-a", id: `tool-${index}`, content: [] },
      "opencode",
    );
    await page.clock.runFor(10_100);
    await expect(activity).toHaveAttribute("data-working", "true");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
  }
  expect(await page.evaluate(() => (window as any).fixture.activity)).toEqual(
    [],
  );
  await page.evaluate(() => (window as any).fixture.setOpenWorking(false));
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await page.clock.runFor(15_100);
  await expect(activity).toHaveAttribute("data-working", "false");
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toHaveCount(0);
  expect(await page.evaluate(() => (window as any).fixture.activity)).toEqual([
    "true",
  ]);
});

test("snapshots recover missed live events on polling, focus and cached-session selection", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.getByRole("button", { name: /OpenCode session/ }).click();
  await expect(page.getByText("Restored OpenCode conversation")).toBeVisible();
  await page.evaluate(() => {
    (window as any).fixture.setOpenWorking(true);
    (window as any).fixture.setOpenHistory([
      {
        id: "missed",
        type: "assistant",
        content: [{ type: "reasoning", text: "Thinking from another client" }],
      },
    ]);
  });
  await page.clock.runFor(15_100);
  const activity = page.getByRole("form", { name: "Message composer" });
  await expect(activity).toHaveAttribute("data-working", "true");
  await expect(page.locator(".reasoning-preview")).toContainText(
    "Thinking from another client",
  );
  await page.evaluate(() => {
    (window as any).fixture.setOpenWorking(false);
    (window as any).fixture.setOpenHistory([
      {
        id: "missed",
        type: "assistant",
        time: { completed: 42 },
        content: [
          {
            type: "reasoning",
            text: "Complete thinking from another client",
          },
          { type: "text", text: "Finished while away" },
        ],
      },
    ]);
    window.dispatchEvent(new Event("focus"));
  });
  await expect(
    page.getByText("Finished while away", { exact: true }),
  ).toBeVisible();
  await expect(activity).toHaveAttribute("data-working", "false");
  await page
    .getByRole("button", { name: /Review the provider adapters/ })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.setOpenHistory([
      {
        id: "missed-next",
        type: "assistant",
        time: { completed: 43 },
        content: [{ type: "text", text: "Arrived while on another session" }],
      },
    ]),
  );
  await page.getByRole("button", { name: /OpenCode session/ }).click();
  await expect(
    page.getByText("Arrived while on another session", { exact: true }),
  ).toBeVisible();
});

test("a completion during history loading queues a resync without restoring stale busy state", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate(() => {
    (window as any).fixture.setOpenWorking(true);
    (window as any).fixture.holdHistory();
  });
  await page.getByRole("button", { name: /OpenCode session/ }).click();
  await expect(page.getByText("Loading conversation…")).toBeVisible();
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await page.evaluate(() => {
    (window as any).fixture.setOpenWorking(false);
    (window as any).fixture.setOpenHistory([
      {
        id: "final",
        type: "assistant",
        time: { completed: 42 },
        content: [
          { type: "text", text: "Recovered after the in-flight snapshot" },
        ],
      },
    ]);
    (window as any).fixture.releaseHistory();
  });
  await expect(
    page.getByText("Recovered after the in-flight snapshot", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("status", { name: "Agent activity" }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toHaveCount(0);
});

for (const provider of ["codex", "opencode"]) {
  test(`${provider} keeps a quiet composer animation and elapsed tool time through a long pause`, async ({
    page,
  }) => {
    await page.clock.install();
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: /OpenCode session/ }).click();
    }
    const composer = page.getByRole("form", { name: "Message composer" });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Run a long command");
    await input.press("Enter");
    await emit(
      page,
      provider === "codex" ? "item/started" : "session.tool.called",
      provider === "codex"
        ? {
            threadId: "thread-a",
            item: {
              id: "slow-command",
              type: "commandExecution",
              command: "cargo test",
              status: "inProgress",
            },
          }
        : {
            sessionID: "ses-a",
            id: "slow-command",
            name: "shell",
            input: { command: "cargo test" },
          },
      provider,
    );
    if (provider === "opencode")
      await emit(page, "session.step.ended", { sessionID: "ses-a" }, provider);
    await page.clock.fastForward(125_000);
    await expect(composer).toHaveAttribute("data-working", "true");
    await expect(
      page.getByRole("button", { name: /cargo test/ }),
    ).toContainText("Running · 2m");
    await expect(composer.locator(".spinning")).toHaveCount(0);
    expect(
      await composer.evaluate(
        (node) => getComputedStyle(node, "::before").animationName,
      ),
    ).toBe("working-pulse");
    await input.fill("Draft while the command runs");
    await page.emulateMedia({ reducedMotion: "reduce" });
    expect(
      await composer.evaluate(
        (node) => getComputedStyle(node, "::before").animationName,
      ),
    ).toBe("none");
    await page.setViewportSize({ width: 800, height: 620 });
    await expect(composer).toBeInViewport({ ratio: 1 });
    await page.screenshot({
      path: `test-results/${provider}-working-composer.png`,
    });
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.succeeded",
      provider === "codex"
        ? {
            threadId: "thread-a",
            turn: { id: "turn-new", status: "completed" },
          }
        : { sessionID: "ses-a" },
      provider,
    );
    await expect(composer).toHaveAttribute("data-working", "false");
    await expect(
      page.getByRole("button", { name: /cargo test/ }),
    ).not.toContainText("Running");
    await expect(input).toHaveValue("Draft while the command runs");
  });

  test(`${provider} keeps effort session-scoped through navigation, retries and reconnects and resets it for another model`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: /OpenCode session/ }).click();
    }
    const sessionName =
      provider === "codex"
        ? /Refine the session workspace/
        : /OpenCode session/;
    const effort = page.getByRole("button", { name: "Effort", exact: true });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(effort).toHaveText("Medium");
    await choose(page, "Effort", "High");
    await page
      .getByRole("button", { name: /Review the provider adapters/ })
      .click();
    await expect(effort).toHaveText("Medium");
    await page.keyboard.press("Meta+n");
    await expect(effort).toHaveText("Default effort");
    await page.getByRole("button", { name: sessionName }).click();
    await expect(effort).toHaveText("High");
    await input.fill("Think carefully");
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await expect(effort).toHaveText("High");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await choose(page, "Effort", "Low");
    await emit(
      page,
      provider === "codex" ? "codex/processExited" : "server.disconnected",
      {},
      provider,
    );
    await page.getByRole("button", { name: "Reconnect", exact: true }).click();
    await expect(input).toBeEnabled();
    await expect(effort).toHaveText("Low");
    await input.fill("Use less reasoning");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await choose(
      page,
      "Effort",
      provider === "codex"
        ? "Default effort Use medium effort"
        : "Default effort Use model settings",
    );
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.succeeded",
      provider === "codex"
        ? {
            threadId: "thread-a",
            turn: { id: "turn-new", status: "completed" },
          }
        : { sessionID: "ses-a" },
      provider,
    );
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toHaveCount(0);
    await input.fill("Restore default effort");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    const sends = calls.filter(
      (call: any) => call.command === `${provider}_send_prompt`,
    );
    expect(
      sends.map((call: any) => [call.args.model, call.args.effort]),
    ).toEqual([
      [null, "high"],
      [null, "high"],
      [null, "low"],
      [null, provider === "codex" ? "medium" : ""],
    ]);
    await choose(page, "Effort", "High");
    await choose(page, "Model", "Fast model provider/fast");
    await expect(effort).toHaveText("Default effort");
    await effort.click();
    await expect(
      page.getByRole("option", { name: "High", exact: true }),
    ).toHaveCount(0);
    await page.getByRole("option", { name: "Low", exact: true }).click();
    await page.setViewportSize({ width: 800, height: 620 });
    await effort.click();
    await expect(
      page.getByRole("dialog", { name: "Choose effort" }),
    ).toBeInViewport({ ratio: 1 });
    await page.screenshot({
      path: `test-results/${provider}-effort-picker.png`,
    });
    await page.keyboard.press("Escape");
    await choose(page, "Model", "Simple model provider/plain");
    await expect(effort).toHaveCount(0);
  });

  test(`${provider} applies effort on the first turn and preserves it if the first send fails`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await choose(page, "Effort", "High");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Start with high effort");
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("High");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    expect(
      calls.filter((call: any) => call.command.endsWith("_start_session")),
    ).toHaveLength(1);
    expect(
      calls
        .filter((call: any) => call.command === `${provider}_send_prompt`)
        .map((call: any) => call.args.effort),
    ).toEqual(["high", "high"]);
  });

  test(`${provider} changes the model in an existing session and keeps pending choices through navigation and retries`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: /OpenCode session/ }).click();
    }
    const sessionName =
      provider === "codex"
        ? /Refine the session workspace/
        : /OpenCode session/;
    const id = provider === "codex" ? "thread-a" : "ses-a";
    const model = page.getByRole("button", { name: "Model", exact: true });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(model).toContainText("Test model");
    await choose(page, "Model", "Fast model provider/fast");
    await input.fill("Use the selected model in this conversation");
    await page.keyboard.press("Meta+n");
    await expect(model).toHaveText("Default model");
    await page
      .getByRole("button", { name: /Review the provider adapters/ })
      .click();
    await expect(model).toContainText("Test model");
    await page.getByRole("button", { name: sessionName }).click();
    await expect(model).toContainText("Fast model");
    await expect(input).toHaveValue(
      "Use the selected model in this conversation",
    );
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await expect(model).toContainText("Fast model");
    await expect(input).toHaveValue(
      "Use the selected model in this conversation",
    );
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await expect(model).toBeEnabled();
    await choose(page, "Model", "Test model provider/model");
    await expect(page.getByText("Model applies to next message")).toBeVisible();
    await choose(
      page,
      "Model",
      "Session model Keep this session’s configured model",
    );
    await expect(model).toContainText("Fast model");
    await choose(page, "Model", "Test model provider/model");
    await emit(
      page,
      provider === "codex" ? "codex/processExited" : "server.disconnected",
      {},
      provider,
    );
    await page.getByRole("button", { name: "Reconnect", exact: true }).click();
    await expect(input).toBeEnabled();
    await expect(model).toContainText("Test model");
    await input.fill("Use the next model");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    const sends = calls.filter(
      (call: any) => call.command === `${provider}_send_prompt`,
    );
    expect(
      sends.map((call: any) => [call.args.sessionId, call.args.model]),
    ).toEqual([
      [id, "provider/fast"],
      [id, "provider/fast"],
      [id, "provider/model"],
    ]);
    expect(
      calls.filter((call: any) => call.command.endsWith("_start_session")),
    ).toHaveLength(0);
    await expect(
      provider === "codex"
        ? page.getByRole("heading", { name: "A focused workspace" })
        : page.getByText("Restored OpenCode conversation"),
    ).toBeVisible();
  });
}
