import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  choose,
  setHistory,
  sessionRow,
  findProjectFile,
  attachedContext,
} from "./fixtures/helpers.js";

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
  ).toHaveCount(2);
  await expect(page.getByRole("option", { name: /^\/new / })).toBeVisible();
  await input.press("Escape");
  await choose(page, "Provider", "OpenCode");
  await input.fill("/");
  const list = page.getByRole("listbox", { name: "Slash commands" });
  await expect(list.getByRole("option", { name: /^\/goal/ })).toBeVisible();
  await expect(
    list.getByRole("option", { name: /^\/team\/review/ }),
  ).toBeVisible();
  await input.fill("/re");
  await input.press("ArrowDown");
  await expect(
    list.getByRole("option", { name: /^\/team\/review/ }),
  ).toHaveAttribute("aria-selected", "true");
  await input.press("ArrowUp");
  await expect(list.getByRole("option", { name: /^\/review/ })).toHaveAttribute(
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
