import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  emit,
  choose,
  setHistory,
  openSettings,
  findProjectFile,
  attachedContext,
} from "./fixtures/helpers.js";

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
    "Subscription allowances.",
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

test("Codex rejects usage commands in new, idle and running sessions while keeping the draft and attachments", async ({ page }) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  for (const state of ["new", "idle", "running"]) {
    if (state === "idle")
      await page.getByRole("button", { name: "Refine the session workspace", exact: true }).click();
    if (state === "running") {
      await input.fill("Start the task");
      await input.press("Enter");
      await expect(page.getByRole("button", { name: "Stop generation" })).toBeVisible();
    }
    await input.fill("@README");
    await page.getByRole("option", { name: "README.md", exact: true }).click();
    await input.fill("/usage");
    await expect(page.getByRole("option", { name: /^\/usage/ })).toHaveCount(0);
    await input.press("Enter");
    await expect(page.getByRole("alert").filter({ hasText: "/usage is only available with OpenCode." })).toBeVisible();
    await expect(input).toHaveValue("/usage");
    await expect(attachedContext(page)).toContainText("README.md");
    await input.fill("/usage extra");
    await page.getByRole("button", { name: "Run command", exact: true }).click();
    await expect(input).toHaveValue("/usage extra");
  }
  expect(await page.evaluate(() => (window as any).fixture.calls.filter((call: any) =>
    call.command.endsWith("_send_prompt") || call.command.endsWith("_start_session") ||
    call.command.endsWith("_run_command") || call.command.endsWith("_enqueue_prompt") ||
    call.command === "opencode_usage_limits",
  ).map((call: any) => call.command))).toEqual(["codex_send_prompt"]);
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
