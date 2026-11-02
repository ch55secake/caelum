import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose, openSettings } from "./fixtures/helpers.js";

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
  await page.keyboard.press("Meta+k");
  const search = page.getByRole("textbox", {
    name: "Search sessions",
    exact: true,
  });
  await expect(search).toBeFocused();
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
