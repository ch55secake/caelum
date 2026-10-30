import { expect, test, type Page } from "@playwright/test";

async function desktop(page: Page, selected: string | null = "thread-a") {
  await page.addInitScript(
    ({ selected }) => {
      const callbacks = new Map<number, (event: unknown) => void>();
      const listeners = new Map<string, number[]>();
      const project = { name: "caelum", path: "/work/caelum" };
      const otherProject = { name: "other", path: "/work/other" };
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
      const models = new Map(
        ["thread-a", "thread-b", "ses-a"].map((id) => [id, "provider/model"]),
      );
      const efforts = new Map(
        ["thread-a", "thread-b", "ses-a"].map((id) => [id, "medium"]),
      );
      let failNextSend = false;
      const choicesErrors = new Map<string, string>();
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
          emit,
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
          ) => {
            sessionDetails.set(`${provider}:${id}`, value);
          },
          setGitFiles: (files: GitChange[], path = project.path) => {
            gitProjects.set(path, files);
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
            if (command === "restore_workspace")
              return {
                projects: [
                  {
                    project,
                    provider: "codex",
                    codexSessionId:
                      selected &&
                      !archived.has(
                        sessionKey(project.path, "codex", selected),
                      )
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
              sessionStorage.setItem(
                "archived",
                JSON.stringify([...archived]),
              );
              return;
            }
            if (command === "pending_approvals") return [];
            if (command === "plugin:opener|open_url") return;
            if (command === "provider_choices") {
              const error = choicesErrors.get(args.provider);
              await gates.get(`${command}:${args.provider}`)?.promise;
              if (error) throw new Error(error);
              return {
                models: [
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
                  { id: "provider/plain", name: "Simple model", efforts: [] },
                ],
                defaultModel: "provider/model",
                agents:
                  args.provider === "opencode"
                    ? [{ id: "build", name: "Build" }]
                    : [],
              };
            }
            if (command === "codex_list_sessions") {
              const snapshot = {
                project,
                threads: threads.filter(
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
                sessions: [
                  { id: "ses-a", title: "OpenCode session", updatedAt: 123 },
                ].filter(
                  (item) =>
                    !archived.has(
                      sessionKey(args.projectPath, "opencode", item.id),
                    ),
                ),
              };
              await gates.get(command)?.promise;
              return snapshot;
            }
            if (command === "codex_resume_session")
              return {
                project,
                model: models.get(args.threadId),
                effort: efforts.get(args.threadId),
                thread: threads.find((item) => item.id === args.threadId) ?? {
                  id: args.threadId,
                  name: "Fresh session",
                },
              };
            if (command === "opencode_resume_session")
              return {
                project,
                session: {
                  id: args.sessionId,
                  title: "OpenCode session",
                  isWorking: openWorking,
                  model: models.get(args.sessionId),
                  effort: efforts.get(args.sessionId),
                },
              };
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
              };
              if (historyGate) await historyGate;
              return snapshot;
            }
            if (command.endsWith("_start_session")) {
              if (startGate) await startGate;
              const id = command.startsWith("codex")
                ? "thread-new"
                : "ses-new";
              const model = args.model || "provider/model";
              models.set(id, model);
              return command.startsWith("codex")
                ? {
                    project,
                    model,
                    thread: { id: "thread-new", name: null, preview: "" },
                  }
                : { project, session: { id: "ses-new", title: null, model } };
            }
            if (command.endsWith("_send_prompt")) {
              if (failNextSend) {
                failNextSend = false;
                throw new Error("Connection refused");
              }
              const id = args.threadId ?? args.sessionId;
              if (args.model) models.set(id, args.model);
              if (args.effort != null) efforts.set(id, args.effort);
              if (command.startsWith("codex"))
                emit("codex-event", {
                  projectPath: project.path,
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
                  projectPath: project.path,
                  eventType: "session.execution.interrupted",
                  data: { sessionID: args.sessionId, reason: "abort" },
                });
              } else
                emit("codex-event", {
                  projectPath: project.path,
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
            if (command === "git_repository_status") {
              const snapshot = {
                projectPath: args.projectPath,
                repositoryRoot: args.projectPath,
                branch: "main",
                changes: gitProjects.get(args.projectPath) ?? [],
              };
              await gates.get(command)?.promise;
              return snapshot;
            }
            if (command === "list_project_directory")
              return {
                path: args.path,
                total: 1,
                nextOffset: null,
                entries:
                  args.path === ""
                    ? [
                        {
                          name: "src",
                          path: "src",
                          kind: "directory",
                          sizeBytes: null,
                        },
                      ]
                    : [
                        {
                          name: "App.tsx",
                          path: "src/App.tsx",
                          kind: "file",
                          sizeBytes: 28,
                        },
                      ],
              };
            if (command === "read_project_file")
              return {
                path: args.path,
                content: "export const ready = true;\n",
                sizeBytes: 27,
                truncated: false,
              };
            if (command === "git_file_versions")
              return {
                path: args.filePath,
                original: "export const ready = false;\n",
                modified: "export const ready = true;\n",
                truncated: false,
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
            )
              return;
            throw new Error(`Unhandled fixture command: ${command}`);
          },
        },
      });
    },
    { selected },
  );
  await page.goto("/");
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
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
  await page.getByRole("option", { name: option, exact: true }).click();
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

test("settings themes and accents update mounted Monaco file previews and diffs", async ({
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
  const editor = page.locator('.monaco-editor[role="code"]').first();
  await expect(editor).toBeVisible();
  await expect(editor).toHaveCSS("background-color", "rgb(11, 12, 14)");
  await expect(editor.locator(".active-line-number")).toHaveCSS(
    "color",
    "rgb(130, 180, 244)",
  );
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Teal", exact: true }).check();
  await expect(editor.locator(".active-line-number")).toHaveCSS(
    "color",
    "rgb(114, 199, 191)",
  );
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(editor).toHaveCSS("background-color", "rgb(250, 250, 252)");
  await expect(editor.locator(".active-line-number")).toHaveCSS(
    "color",
    "rgb(33, 111, 104)",
  );
  await editor.getByRole("textbox").press("Meta+a");
  await expect(editor.locator(".selected-text").first()).toHaveCSS(
    "background-color",
    "rgba(33, 111, 104, 0.2)",
  );
  await page
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  await expect(page.locator(".monaco-diff-editor")).toBeVisible();
  await expect(editor).toHaveCSS("background-color", "rgb(250, 250, 252)");
  const diffPanes = page.locator(".monaco-diff-editor").getByRole("code");
  await expect(diffPanes).toHaveCount(2);
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("Meta+a");
    await expect(pane.locator(".selected-text").first()).toHaveCSS(
      "background-color",
      "rgba(33, 111, 104, 0.2)",
    );
  }
  await page.screenshot({ path: "test-results/files-light.png" });
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Rose", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("Meta+a");
    await expect(pane.locator(".selected-text").first()).toHaveCSS(
      "background-color",
      "rgba(165, 65, 99, 0.2)",
    );
  }
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Dark", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(editor).toHaveCSS("background-color", "rgb(11, 12, 14)");
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("Meta+a");
    await expect(pane.locator(".selected-text").first()).toHaveCSS(
      "background-color",
      /^rgba\(231, 154, 176, 0\./,
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
    ).toHaveText("Test model");
    await page.reload();
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await expect(
      dialog.getByRole("button", { name: "Default model", exact: true }),
    ).toHaveText("Fast model");
    await expect(
      dialog.getByRole("button", { name: "Default effort", exact: true }),
    ).toHaveText("Low");
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await page.keyboard.press("Meta+n");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toHaveText("Fast model");
    await choose(page, "Model", "Fast model provider/fast");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("Low");
    if (provider === "opencode")
      await expect(
        page.getByRole("button", { name: "Agent", exact: true }),
      ).toHaveText("Build");
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
      calls.find((call: any) => call.command === `${provider}_send_prompt`)
        .args.effort,
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
    ).toHaveText("Fast model");
    await choose(page, "Default model", "Simple model provider/plain");
    await expect(
      dialog.getByRole("button", { name: "Default effort", exact: true }),
    ).toHaveText("Model default");
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await expect(input).toHaveValue("Keep this draft override");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toHaveText("Test model");
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
  ).toHaveText("Fast model");
  await dialog.getByRole("button", { name: "Reset Codex defaults" }).click();
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toHaveText("Provider default");
  await choose(page, "Default provider", "OpenCode");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page
    .getByRole("button", { name: "Close caelum", exact: true })
    .click();
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
      if (key === "caelum.settings.v1")
        throw new Error("Storage unavailable");
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
    provider === "codex"
      ? "Refine the session workspace"
      : "OpenCode session";
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
    ).toHaveText("Fast model");
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
  await page
    .getByRole("button", { name: "Open project", exact: true })
    .click();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await page.evaluate(() =>
    (window as any).fixture.release("session_details"),
  );
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

test("sidebar details refresh, allow branch search and fit at the minimum window size", async ({
  page,
}) => {
  await desktop(page);
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
  await page.setViewportSize({ width: 800, height: 620 });
  await row.hover();
  await expect(row.locator(".session-lines")).toBeInViewport({ ratio: 1 });
  await expect(
    row.getByRole("button", { name: "Archive session", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBe(800);
  await page.screenshot({ path: "test-results/session-sidebar.png" });
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
    sessionRow(page, "Review the provider adapters").locator(
      ".session-lines",
    ),
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
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBe(800);
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
  await page
    .getByRole("button", { name: "Hide changes", exact: true })
    .click();
  await expect.poll(height).toBe(56);
  await expect.poll(gap).toBeLessThanOrEqual(1);
  await page
    .getByRole("button", { name: "Show changes", exact: true })
    .click();
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
      await input.evaluate(
        (node: HTMLTextAreaElement) => node.selectionStart,
      ),
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
  await page
    .getByRole("textbox", { name: "Search sessions" })
    .fill("adapters");
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
  await expect(
    page.getByText("New answer", { exact: true }),
  ).toBeInViewport();
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
  await expect(
    page.getByText("File contents", { exact: true }),
  ).toBeVisible();
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
  await expect(
    pane.getByRole("region", { name: "Staged diff" }),
  ).toContainText("revision 1");
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
  expect(
    await page.evaluate(() => document.documentElement.scrollWidth),
  ).toBe(800);
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
  await expect(
    pane.getByRole("region", { name: "Staged diff" }),
  ).toContainText("background");
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
  await page
    .getByRole("button", { name: "Open project", exact: true })
    .click();
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
  await expect(page.getByRole("option")).toHaveCount(1);
  await search.press("Enter");
  await expect(model).toHaveText("Fast model");
  await expect(model).toBeFocused();
  await model.click();
  await search.fill("missing");
  await expect(page.getByText("No matching models")).toBeVisible();
  await search.press("Escape");
  await expect(model).toBeFocused();
  await model.click();
  await search.press("ArrowUp");
  await search.press("Enter");
  await expect(model).toHaveText("Test model");
  await model.click();
  await page.screenshot({ path: "test-results/model-picker.png" });
  await page
    .getByRole("heading", { name: "New session", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Choose model" }),
  ).toHaveCount(0);
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
  await expect(notification).toHaveCSS(
    "background-color",
    "rgba(0, 0, 0, 0)",
  );
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
  await expect(
    page.getByText("Restored OpenCode conversation"),
  ).toBeVisible();
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
  await expect(
    page.getByRole("status", { name: "Agent activity" }),
  ).toHaveText("Retrying · attempt 2");
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
    provider === "codex"
      ? "Refine the session workspace"
      : "OpenCode session";
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
          provider === "codex"
            ? "codex/processExited"
            : "server.disconnected",
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
      await expect(notices.last().locator("strong")).toHaveText(
        "release plan",
      );
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
          provider === "codex"
            ? "turn/completed"
            : "session.execution.failed",
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
          provider === "codex"
            ? "codex/processExited"
            : "server.disconnected",
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
        await expect(
          page.getByText("opaque-provider-checkpoint"),
        ).toHaveCount(0);
        await expect(notices.first()).toHaveAttribute(
          "data-status",
          "failed",
        );
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
  await expect(
    page.getByText("Restored OpenCode conversation"),
  ).toBeVisible();
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
        patchText:
          "*** Begin Patch\n*** Update File: src/App.tsx\n@@\n-old\n+new\n*** Add File: src/new file.ts\n+export {};\n*** End Patch",
      },
      preview: "src/App.tsx, src/new file.ts",
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
  for (const tool of tools) {
    const restored = page.locator(
      `[data-entry-id="opencode-tool-restored-${tool.name}"]`,
    );
    await expect(restored.locator(".tool-preview")).toHaveText(tool.preview);
    await restored
      .getByRole("button", { name: new RegExp(`^${tool.name} `) })
      .click();
    await expect(restored.locator(".tool-value pre").first()).toHaveText(
      JSON.stringify(tool.input, null, 2),
    );
    await emit(
      page,
      "session.tool.input.delta",
      {
        sessionID: "ses-a",
        id: `live-${tool.name}`,
        name: tool.name,
        delta: JSON.stringify(tool.input),
      },
      "opencode",
    );
    const live = page.locator(
      `[data-entry-id="opencode-tool-live-${tool.name}"]`,
    );
    await expect(live.locator(".tool-preview")).toHaveText(tool.preview);
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
  }
  await page.screenshot({ path: "test-results/tool-previews.png" });
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
  await expect(
    page.getByText("Restored OpenCode conversation"),
  ).toBeVisible();
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
  expect(await page.evaluate(() => (window as any).fixture.activity)).toEqual(
    ["true"],
  );
});

test("snapshots recover missed live events on polling, focus and cached-session selection", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.getByRole("button", { name: /OpenCode session/ }).click();
  await expect(
    page.getByText("Restored OpenCode conversation"),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as any).fixture.setOpenWorking(true);
    (window as any).fixture.setOpenHistory([
      {
        id: "missed",
        type: "assistant",
        content: [
          { type: "reasoning", text: "Thinking from another client" },
        ],
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
      await emit(
        page,
        "session.step.ended",
        { sessionID: "ses-a" },
        provider,
      );
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
    await page
      .getByRole("button", { name: "Reconnect", exact: true })
      .click();
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
    await expect(model).toHaveText("Test model");
    await choose(page, "Model", "Fast model provider/fast");
    await input.fill("Use the selected model in this conversation");
    await page.keyboard.press("Meta+n");
    await expect(model).toHaveText("Default model");
    await page
      .getByRole("button", { name: /Review the provider adapters/ })
      .click();
    await expect(model).toHaveText("Test model");
    await page.getByRole("button", { name: sessionName }).click();
    await expect(model).toHaveText("Fast model");
    await expect(input).toHaveValue(
      "Use the selected model in this conversation",
    );
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await expect(model).toHaveText("Fast model");
    await expect(input).toHaveValue(
      "Use the selected model in this conversation",
    );
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await expect(model).toBeEnabled();
    await choose(page, "Model", "Test model provider/model");
    await expect(
      page.getByText("Model applies to next message"),
    ).toBeVisible();
    await choose(
      page,
      "Model",
      "Session model Keep this session’s configured model",
    );
    await expect(model).toHaveText("Fast model");
    await choose(page, "Model", "Test model provider/model");
    await emit(
      page,
      provider === "codex" ? "codex/processExited" : "server.disconnected",
      {},
      provider,
    );
    await page
      .getByRole("button", { name: "Reconnect", exact: true })
      .click();
    await expect(input).toBeEnabled();
    await expect(model).toHaveText("Test model");
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
