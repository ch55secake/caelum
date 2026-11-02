import { expect, type Page } from "@playwright/test";
import type { QuestionPrompt } from "../../src/features/sessions/questionState.js";
import type { GitWorktree } from "../../src/features/projects/worktreeState.js";

export async function desktop(
  page: Page,
  selected: string | null = "thread-a",
  questionOptions: {
    initialQuestions?: QuestionPrompt[];
    holdQuestions?: boolean;
    worktrees?: boolean;
    missingProviders?: ("codex" | "opencode")[];
  } = {},
) {
  await page.addInitScript(
    ({
      selected,
      initialQuestions,
      holdQuestions,
      worktrees,
      missingProviders,
    }) => {
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
      const checkoutSessions = new Map<string, any[]>(
        JSON.parse(sessionStorage.getItem("checkout-sessions") ?? "[]"),
      );
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
      const uninstalled = new Set(missingProviders);
      const slashCommands = new Map<
        string,
        { name: string; description?: string }[]
      >();
      let usageLimits: unknown = null;
      let stats: unknown = null;
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
      const pullRequestPages = new Map<string, unknown>();
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
      const inbox = new Map<string, any[]>(
        JSON.parse(sessionStorage.getItem("inbox") ?? "[]"),
      );
      const saveInbox = () =>
        sessionStorage.setItem("inbox", JSON.stringify([...inbox]));
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
          deliverQueued: (path = project.path, id = "ses-a") => {
            const key = sessionKey(path, "opencode", id);
            const [item, ...rest] = inbox.get(key) ?? [];
            if (!item) return;
            inbox.set(key, rest);
            saveInbox();
            openHistory.push({
              id: item.id,
              type: "user",
              text: item.payload.text,
            });
            openWorking = true;
            emit("opencode-event", {
              projectPath: path,
              eventType: "session.inbox.delivered",
              data: { sessionID: id, inboxID: item.id },
            });
            emit("opencode-event", {
              projectPath: path,
              eventType: "session.execution.started",
              data: { sessionID: id },
            });
          },
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
          setSessionDetails: (
            provider: string,
            id: string,
            value: unknown,
            path?: string,
          ) => {
            sessionDetails.set(
              `${path ? `${path}\0` : ""}${provider}:${id}`,
              value,
            );
          },
          setCheckoutSessions: (
            path: string,
            provider: string,
            sessions: any[],
          ) => {
            checkoutSessions.set(`${path}\0${provider}`, sessions);
            sessionStorage.setItem(
              "checkout-sessions",
              JSON.stringify([...checkoutSessions]),
            );
          },
          addSession: (id: string, name: string) => {
            threads.push({ id, name, preview: "", updatedAt: 1 });
            if (worktrees)
              savedProjects.get(project.path)?.codexSessionIds.push(id);
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
          setPullRequestPage: (
            url: string,
            kind: string,
            cursor: string | number,
            value: unknown,
          ) => {
            pullRequestPages.set(`${url}:${kind}:${cursor}`, value);
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
          setProviderInstalled: (
            provider: "codex" | "opencode",
            installed: boolean,
          ) => {
            if (installed) uninstalled.delete(provider);
            else uninstalled.add(provider);
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
          setStats: (value: unknown) => {
            stats = value;
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
              const scoped = `${args.projectPath}\0${args.provider}:${args.sessionId}`;
              const key = sessionDetails.has(scoped)
                ? scoped
                : `${args.provider}:${args.sessionId}`;
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
                    branchIsCurrent: false,
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
              command === "opencode_stats" ||
              command === "opencode_stats_projects"
            ) {
              const snapshot = stats;
              const error = commandErrors.get(command);
              await gates.get(command)?.promise;
              if (error) throw new Error(error);
              return command === "opencode_stats"
                ? snapshot
                : [
                    {
                      id: "project-one",
                      name: "Caelum",
                      canonical: "/work/caelum",
                    },
                    {
                      id: "project-two",
                      name: "Other",
                      canonical: "/work/other",
                    },
                  ];
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
                threads: (
                  checkoutSessions.get(`${args.projectPath}\0codex`) ??
                  (worktrees
                    ? (
                        savedProjects.get(args.projectPath)?.codexSessionIds ??
                        []
                      ).map(
                        (id: string) =>
                          threads.find((entry) => entry.id === id) ?? {
                            id,
                            name: "Worktree session",
                            preview: "",
                            updatedAt: 124,
                          },
                      )
                    : threads)
                ).filter(
                  (item) =>
                    !archived.has(
                      sessionKey(args.projectPath, "codex", item.id),
                    ),
                ),
              };
              const scope = `${command}:${args.projectPath}`;
              const error =
                commandErrors.get(scope) ?? commandErrors.get(command);
              const missing = uninstalled.has("codex");
              await (gates.get(scope) ?? gates.get(command))?.promise;
              if (missing)
                throw {
                  kind: "notInstalled",
                  message: "Codex CLI was not found.",
                };
              if (error) throw { kind: "failed", message: error };
              return snapshot;
            }
            if (command === "opencode_list_sessions") {
              const snapshot = {
                project,
                sessions: (
                  checkoutSessions.get(`${args.projectPath}\0opencode`) ??
                  (worktrees
                    ? (
                        savedProjects.get(args.projectPath)
                          ?.opencodeSessionIds ?? []
                      ).map((id: string) => ({
                        id,
                        title: "OpenCode session",
                        updatedAt: 124,
                      }))
                    : [
                        {
                          id: "ses-a",
                          title: "OpenCode session",
                          updatedAt: 123,
                        },
                      ])
                ).filter(
                  (item) =>
                    !archived.has(
                      sessionKey(args.projectPath, "opencode", item.id),
                    ),
                ),
              };
              const scope = `${command}:${args.projectPath}`;
              const error =
                commandErrors.get(scope) ?? commandErrors.get(command);
              const missing = uninstalled.has("opencode");
              await (gates.get(scope) ?? gates.get(command))?.promise;
              if (missing)
                throw {
                  kind: "notInstalled",
                  message: "OpenCode CLI was not found.",
                };
              if (error) throw { kind: "failed", message: error };
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
                project: checkoutProject(args.projectPath),
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
                project: checkoutProject(args.projectPath),
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
            if (
              [
                "opencode_inbox",
                "opencode_enqueue_prompt",
                "opencode_cancel_queued_prompt",
              ].includes(command)
            ) {
              const key = sessionKey(
                args.projectPath,
                "opencode",
                args.sessionId,
              );
              const snapshot = structuredClone(inbox.get(key) ?? []);
              await gates.get(command)?.promise;
              if (commandErrors.has(command))
                throw new Error(commandErrors.get(command));
              if (command === "opencode_inbox") return snapshot;
              if (command === "opencode_enqueue_prompt") {
                const item = {
                  id: args.messageId,
                  type: "user",
                  delivery: "queue",
                  payload: { text: args.prompt },
                };
                if (
                  !(inbox.get(key) ?? []).some((entry) => entry.id === item.id)
                )
                  inbox.set(key, [...(inbox.get(key) ?? []), item]);
                emit("opencode-event", {
                  projectPath: args.projectPath,
                  eventType: "session.inbox.enqueued",
                  data: { sessionID: args.sessionId, inboxID: item.id, item },
                });
              } else {
                inbox.set(
                  key,
                  (inbox.get(key) ?? []).filter(
                    (entry) => entry.id !== args.messageId,
                  ),
                );
                emit("opencode-event", {
                  projectPath: args.projectPath,
                  eventType: "session.inbox.cancelled",
                  data: { sessionID: args.sessionId, inboxID: args.messageId },
                });
              }
              saveInbox();
              return;
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
              const error = commandErrors.get(command);
              if (error) throw new Error(error);
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
            if (
              [
                "github_pull_request_files",
                "github_pull_request_feedback",
              ].includes(command)
            ) {
              const kind =
                command === "github_pull_request_files" ? "files" : args.kind;
              const result =
                pullRequestPages.get(
                  `${args.url}:${kind}:${args.page ?? args.cursor ?? ""}`,
                ) ??
                (kind === "files"
                  ? { files: [], baseOid: "base", nextPage: null }
                  : { comments: [], threads: [], nextCursor: null });
              const error = commandErrors.get(command);
              await (gates.get(`${command}:${args.url}`) ?? gates.get(command))
                ?.promise;
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
            if (command === "github_update_pull_request_branch") {
              const error = commandErrors.get(command);
              await (gates.get(`${command}:${args.url}`) ?? gates.get(command))
                ?.promise;
              if (error) throw new Error(error);
              const pr = pullRequests.get(args.url) as any;
              if (!pr || pr.headRefOid !== args.headOid)
                throw new Error("Head branch was modified (HTTP 422)");
              // Like GitHub's 202 response, acceptance does not change the head
              // until a later detail response reports the completed update.
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
