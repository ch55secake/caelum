import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose } from "./fixtures/helpers.js";

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

test("first send creates one session; failure retains a retryable draft; generation permits queuing", async ({
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
  await expect(input).toHaveValue("");
  await expect(
    page.getByRole("region", { name: "Queued messages" }),
  ).toContainText("Next task");
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
  await expect(input).toHaveValue("");
  await expect(
    page.getByRole("region", { name: "Queued messages" }),
  ).toContainText("Paused");
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
  await expect(page.locator(".session-item")).toHaveCount(3);
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
  await expect(
    page.getByRole("button", { name: "Jump to latest" }),
  ).toHaveCount(0);
  await expect(page.getByText("Recovered final response")).toBeInViewport();
  await expect(activity).toHaveAttribute("data-working", "false");
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
}
