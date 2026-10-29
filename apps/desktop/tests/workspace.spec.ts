import { expect, test, type Page } from "@playwright/test";

async function desktop(page: Page, selected: string | null = "thread-a") {
  await page.addInitScript(
    ({ selected }) => {
      const callbacks = new Map<number, (event: unknown) => void>();
      const listeners = new Map<string, number[]>();
      const project = { name: "caelum", path: "/work/caelum" };
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
      let sequence = 0;
      let historyGate: Promise<void> | null = null;
      let startGate: Promise<void> | null = null;
      let releaseStart: () => void = () => {};
      let gitFiles = [
        { path: "src/App.tsx", indexStatus: " ", worktreeStatus: "M" },
        { path: "src/styles.css", indexStatus: "M", worktreeStatus: " " },
      ];
      let diffVersion = 1;
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
          setGitFiles: (files: typeof gitFiles) => {
            gitFiles = files;
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
            if (command === "restore_workspace")
              return {
                projects: [
                  {
                    project,
                    provider: "codex",
                    codexSessionId: selected,
                    opencodeSessionId: null,
                    codexSessionIds: threads.map((item) => item.id),
                    opencodeSessionIds: [],
                  },
                ],
                activeProjectPath: project.path,
              };
            if (command === "startup_project") return null;
            if (command === "open_project") return project;
            if (command === "pending_approvals") return [];
            if (command === "plugin:opener|open_url") return;
            if (command === "provider_choices")
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
            if (command === "codex_list_sessions")
              return { project, threads };
            if (command === "opencode_list_sessions")
              return {
                project,
                sessions: [
                  { id: "ses-a", title: "OpenCode session", updatedAt: 123 },
                ],
              };
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
              if (historyGate) await historyGate;
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
            if (command === "git_repository_status")
              return {
                projectPath: project.path,
                repositoryRoot: project.path,
                branch: "main",
                changes: gitFiles,
              };
            if (command === "git_file_diff")
              return {
                path: args.filePath,
                staged: args.filePath.endsWith(".css")
                  ? `diff --git a/${args.filePath} b/${args.filePath}\n@@ -1 +1 @@\n-background: white;\n+background: #0b0c0e; /* revision ${diffVersion} */`
                  : "",
                unstaged: args.filePath.endsWith(".tsx")
                  ? `diff --git a/${args.filePath} b/${args.filePath}\n@@ -1 +1 @@\n-const layout = 'page';\n+const layout = 'split'; // revision ${diffVersion}`
                  : "",
                untracked: "",
              };
            if (
              [
                "remember_project",
                "set_active_project",
                "set_project_provider",
                "select_project_session",
                "sync_provider_sessions",
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
) {
  await page.evaluate(
    ({ method, params, provider }) =>
      (window as any).fixture.emit(
        `${provider}-event`,
        provider === "codex"
          ? { projectPath: "/work/caelum", method, params }
          : { projectPath: "/work/caelum", eventType: method, data: params },
      ),
    { method, params, provider },
  );
}

async function choose(page: Page, label: string, option: string) {
  await page.getByRole("button", { name: label, exact: true }).click();
  await page.getByRole("option", { name: option, exact: true }).click();
}

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

test("changes stay beside the conversation and refresh the selected diff during work", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("const layout = 'split'");
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
  await page.evaluate(() => (window as any).fixture.setDiffVersion(2));
  await page.clock.runFor(2600);
  await expect(
    pane.getByRole("region", { name: "Staged diff" }),
  ).toContainText("revision 2");
  await expect(
    pane.getByRole("button", { name: "src/styles.css, Staged modified" }),
  ).toHaveAttribute("aria-pressed", "true");
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
  ).toHaveText("Agent working");
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
