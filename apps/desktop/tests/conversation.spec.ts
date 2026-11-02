import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  emit,
  choose,
  setHistory,
  openSettings,
  pickMenu,
  copiedText,
} from "./fixtures/helpers.js";

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
