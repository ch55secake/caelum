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
      let failNextSend = false;
      let sequence = 0;
      let historyGate: Promise<void> | null = null;
      let startGate: Promise<void> | null = null;
      let releaseStart: () => void = () => {};
      let openHistory: unknown[] = [
        {
          id: "msg-history",
          type: "assistant",
          content: [{ type: "text", text: "Restored OpenCode conversation" }],
        },
      ];
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
                models: [{ id: "provider/model", name: "Test model" }],
                agents:
                  args.provider === "opencode"
                    ? [{ id: "build", name: "Build" }]
                    : [],
              };
            if (command === "codex_list_sessions") return { project, threads };
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
                thread: threads.find((item) => item.id === args.threadId) ?? {
                  id: args.threadId,
                  name: "Fresh session",
                },
              };
            if (command === "opencode_resume_session")
              return {
                project,
                session: { id: args.sessionId, title: "OpenCode session" },
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
            if (command === "opencode_session_history")
              return { nextCursor: null, data: openHistory };
            if (command.endsWith("_start_session")) {
              if (startGate) await startGate;
              return command.startsWith("codex")
                ? {
                    project,
                    thread: { id: "thread-new", name: null, preview: "" },
                  }
                : { project, session: { id: "ses-new", title: null } };
            }
            if (command.endsWith("_send_prompt")) {
              if (failNextSend) {
                failNextSend = false;
                throw new Error("Connection refused");
              }
              if (command.startsWith("codex"))
                emit("codex-event", {
                  projectPath: project.path,
                  method: "turn/started",
                  params: { threadId: args.threadId, turn: { id: "turn-new" } },
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
            if (command === "project_git_status")
              return {
                projectPath: project.path,
                repositoryRoot: project.path,
                branch: "main",
                changes: [],
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

test("first send creates one session; failure retains a retryable draft; generation permits drafting", async ({
  page,
}) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await page
    .getByRole("combobox", { name: "Model", exact: true })
    .selectOption("provider/model");
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

test("OpenCode uses provider choices, streams structured tools and reconciles accepted messages", async ({
  page,
}) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Codex draft");
  await page
    .getByRole("combobox", { name: "Provider", exact: true })
    .selectOption("opencode");
  await expect(input).toHaveValue("");
  await page
    .getByRole("combobox", { name: "Agent", exact: true })
    .selectOption("build");
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
  await page
    .getByRole("combobox", { name: "Provider", exact: true })
    .selectOption("codex");
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
