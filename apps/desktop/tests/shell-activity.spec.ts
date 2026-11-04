import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, emit } from "./fixtures/helpers.js";

const commands = (page: Page) => page.getByRole("region", { name: "Background commands" });
const backgroundCommand = "opencode_background_shells";
const backgroundScope = `${backgroundCommand}:/work/caelum:ses-a`;

async function openOpenCode(page: Page) {
  await choose(page, "Provider", "OpenCode");
  await page.getByRole("button", { name: "OpenCode session", exact: true }).click();
  await expect(page.getByRole("textbox", { name: "Message", exact: true })).toBeEnabled();
}

test("background jobs stay visible after tool and prompt completion with independent timers", async ({ page }, info) => {
  await page.clock.install();
  await desktop(page, null);
  await page.clock.pauseAt(await page.evaluate(() => Date.now() + 100));
  await openOpenCode(page);
  const startedAt = await page.evaluate(() => Date.now());
  const first = { id: "sh_first", command: "cargo test --workspace", startedAt };
  const second = { id: "sh_second", command: "pnpm test:ui", startedAt: startedAt + 10000 };
  await page.evaluate(() => (window as any).fixture.setOpenWorking(true));
  await emit(page, "session.execution.started", { sessionID: "ses-a" }, "opencode");
  await emit(page, "session.tool.called", {
    sessionID: "ses-a", id: "tool-bg", name: "shell",
    input: { command: first.command, background: true },
  }, "opencode");
  await page.evaluate((shell) => (window as any).fixture.setBackgroundShells([shell]), first);
  await emit(page, "session.tool.success", {
    sessionID: "ses-a", id: "tool-bg",
    content: [{ type: "text", text: "Running in background" }],
  }, "opencode");
  await expect(commands(page)).toContainText("1 background command running");
  await expect(commands(page)).toContainText(first.command);
  await page.clock.fastForward(5000);
  await page.evaluate(() => (window as any).fixture.setOpenWorking(false));
  await emit(page, "session.execution.succeeded", { sessionID: "ses-a" }, "opencode");
  await expect(commands(page).getByRole("status")).toHaveText("Waiting for background command");
  await expect(page.getByRole("form", { name: "Message composer" })).toHaveAttribute("data-working", "false");
  await expect(page.getByLabel("Prompt timing", { exact: true })).toHaveText("Worked for 5s");
  await page.clock.fastForward(5000);
  await expect(commands(page).getByLabel("Command elapsed time")).toHaveText("10s");
  await expect(page.getByLabel("Prompt timing", { exact: true })).toHaveText("Worked for 5s");
  await page.getByRole("textbox", { name: "Message", exact: true }).fill("My next prompt");
  await page.evaluate((shells) => (window as any).fixture.setBackgroundShells(shells), [first, second]);
  await emit(page, "shell.created", { info: { id: second.id } }, "opencode");
  await expect(commands(page).getByRole("status")).toHaveText("Waiting for 2 background commands");
  await expect(commands(page).getByLabel("Command elapsed time")).toHaveText(["10s", "0s"]);
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(commands(page)).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: info.outputPath("background-waiting.png") });
  await page.evaluate((shell) => (window as any).fixture.setBackgroundShells([shell]), second);
  await emit(page, "shell.exited", { id: first.id, status: "exited", exit: 1 }, "opencode");
  await expect(commands(page).getByRole("listitem")).toHaveCount(1);
  await expect(commands(page)).toContainText(second.command);
  // A missed terminal event is reconciled even while the model is idle.
  await page.evaluate(() => (window as any).fixture.setBackgroundShells([]));
  await page.clock.fastForward(5000);
  await expect(commands(page)).toHaveCount(0);
  await expect(page.getByRole("textbox", { name: "Message", exact: true })).toHaveValue("My next prompt");
});

test("background snapshots cannot resurrect a shell after a terminal event", async ({ page }) => {
  await desktop(page, null);
  await page.evaluate(() => (window as any).fixture.setBackgroundShells([
    { id: "sh_done", command: "already completed command", startedAt: Date.now() },
  ]));
  await openOpenCode(page);
  await expect(commands(page)).toContainText("already completed command");
  const before = await page.evaluate((command) => {
    const fixture = (window as any).fixture;
    fixture.hold(command);
    const count = fixture.calls.filter((call: any) => call.command === "opencode_background_shells").length;
    window.dispatchEvent(new Event("focus"));
    return count;
  }, backgroundScope);
  await expect.poll(() => page.evaluate(() => (window as any).fixture.calls.filter(
    (call: any) => call.command === "opencode_background_shells",
  ).length)).toBeGreaterThan(before);
  await page.evaluate(() => (window as any).fixture.setBackgroundShells([]));
  await emit(page, "session.shell.ended", {
    sessionID: "ses-a", shell: { id: "sh_done", command: "already completed command", status: "killed" },
  }, "opencode");
  await expect(commands(page)).toHaveCount(0);
  await page.evaluate((command) => {
    const fixture = (window as any).fixture;
    fixture.release(command);
    // Hold the replacement read too: an accepted stale response must not be
    // hidden by an immediately resolving fresh snapshot.
    fixture.hold(command);
  }, backgroundScope);
  await expect.poll(() => page.evaluate(() => (window as any).fixture.calls.filter(
    (call: any) => call.command === "opencode_background_shells",
  ).length)).toBeGreaterThan(before + 1);
  await expect(commands(page)).toHaveCount(0);
  await page.evaluate((command) => (window as any).fixture.release(command), backgroundScope);
});

test("background jobs restore on reload and stay scoped across projects and stale reads", async ({ page }) => {
  await desktop(page, null, { worktrees: true });
  await page.evaluate((scope) => {
    const fixture = (window as any).fixture;
    for (const path of ["/work/caelum", "/work/other"])
      fixture.setCheckoutSessions(path, "opencode", [
        { id: "ses-a", title: "OpenCode session", updatedAt: 123 },
      ]);
    fixture.setBackgroundShells([{ id: "sh_old", command: "stale original command", startedAt: Date.now() }]);
    fixture.setBackgroundShells([{ id: "sh_other", command: "other project command", startedAt: Date.now() }], "/work/other");
    fixture.hold(scope);
  }, backgroundScope);
  await page.getByRole("button", { name: "Refresh sessions", exact: true }).click();
  await openOpenCode(page);
  await expect.poll(() => page.evaluate(() => (window as any).fixture.calls.some(
    (call: any) => call.command === "opencode_background_shells" && call.args.projectPath === "/work/caelum",
  ))).toBe(true);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await openOpenCode(page);
  await expect(commands(page)).toContainText("other project command");
  await page.evaluate((scope) => (window as any).fixture.release(scope), backgroundScope);
  await expect(commands(page)).not.toContainText("stale original command");
  await page.reload();
  await expect(commands(page)).toContainText("other project command");
  await page.getByRole("button", { name: "New session", exact: true }).click();
  await expect(commands(page)).toHaveCount(0);
  await page.getByRole("button", { name: "OpenCode session", exact: true }).click();
  await expect(commands(page)).toContainText("other project command");
});

test("disconnects invalidate pending background reads and reconnect restores live jobs", async ({ page }) => {
  await desktop(page, null);
  await page.evaluate(() => (window as any).fixture.setBackgroundShells([
    { id: "sh_old", command: "before disconnect", startedAt: Date.now() },
  ]));
  await openOpenCode(page);
  await expect(commands(page)).toContainText("before disconnect");
  await page.evaluate((scope) => {
    (window as any).fixture.hold(scope);
    window.dispatchEvent(new Event("focus"));
  }, backgroundScope);
  await emit(page, "server.disconnected", {}, "opencode");
  await expect(page.getByRole("button", { name: "Reconnect", exact: true })).toBeVisible();
  await page.evaluate((scope) => (window as any).fixture.release(scope), backgroundScope);
  await expect(commands(page)).toHaveCount(0);
  await page.evaluate(() => (window as any).fixture.setBackgroundShells([
    { id: "sh_new", command: "after reconnect", startedAt: Date.now() },
  ]));
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(commands(page)).toContainText("after reconnect");
  await expect(commands(page)).not.toContainText("before disconnect");
});

test("unavailable background status is explicit and recovers on refresh", async ({ page }) => {
  await page.clock.install();
  await desktop(page, null);
  await page.evaluate((command) => (window as any).fixture.setCommandError(command, "Jobs unavailable"), backgroundCommand);
  await openOpenCode(page);
  await expect(commands(page).getByRole("status")).toHaveText("Background command status unavailable");
  await page.evaluate((command) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError(command, null);
    fixture.setBackgroundShells([{ id: "sh_live", command: "restored command", startedAt: Date.now() }]);
  }, backgroundCommand);
  await page.clock.fastForward(5000);
  await expect(commands(page)).toContainText("restored command");
  await expect(commands(page).getByRole("status")).toHaveText("Waiting for background command");
});

for (const following of [true, false]) {
  test(`background status preserves ${following ? "following" : "the reading position"} with a bounded transcript`, async ({ page }) => {
    await page.setViewportSize({ width: 800, height: 620 });
    await desktop(page, null);
    await page.evaluate(() => (window as any).fixture.setOpenHistory(
      Array.from({ length: 120 }, (_, index) => ({
        id: `message-${index}`, type: "assistant",
        content: [{ type: "text", text: `Message ${index}\n\nMore context.` }],
      })),
    ));
    await openOpenCode(page);
    await expect(page.locator(".conversation-entry")).toHaveCount(100);
    const transcript = page.locator(".transcript");
    if (!following) {
      await transcript.evaluate((node) => { node.scrollTop = 250; });
      await expect(page.getByRole("button", { name: "Jump to latest" })).toBeVisible();
    }
    const checkPosition = async () => {
      await expect.poll(() => transcript.evaluate((node, following) => following
        ? node.scrollHeight - node.scrollTop - node.clientHeight
        : Math.abs(node.scrollTop - 250), following)).toBeLessThanOrEqual(1);
    };
    await page.evaluate(() => {
      (window as any).fixture.setBackgroundShells(Array.from({ length: 12 }, (_, index) => ({
        id: `sh_${index}`, command: `command ${index}`, startedAt: Date.now(),
      })));
      window.dispatchEvent(new Event("focus"));
    });
    await expect(commands(page)).toContainText("Waiting for 12 background commands");
    await expect(commands(page)).toBeInViewport({ ratio: 1 });
    await checkPosition();
    await page.evaluate(() => {
      (window as any).fixture.setBackgroundShells([]);
      window.dispatchEvent(new Event("focus"));
    });
    await expect(commands(page)).toHaveCount(0);
    await checkPosition();
    await expect(page.locator(".conversation-entry")).toHaveCount(100);
  });
}

test("native shell activity outlives prompts, reports exit status and reconciles history once", async ({
  page,
}, info) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const started = await page.evaluate(() => Date.now());
  const shell = {
    id: "sh_live",
    command: "cargo test --workspace",
    cwd: "/work/caelum",
    status: "running",
    metadata: { background: true },
    time: { started },
  };
  await emit(
    page,
    "session.shell.started",
    { sessionID: "ses-a", shell },
    "opencode",
  );
  const row = page.locator(".tool-activity").filter({ hasText: shell.command });
  await expect(row).toContainText("/work/caelum");
  await expect(row).toContainText("Background");
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await page.clock.fastForward(5000);
  await expect(
    row.getByRole("button", { name: /Run cargo test/ }),
  ).toContainText("Running · 5s");
  const completed = {
    ...shell,
    status: "exited",
    exit: 7,
    time: { started, completed: started + 5000 },
  };
  await emit(
    page,
    "session.shell.ended",
    {
      sessionID: "ses-a",
      shell: completed,
      output: { output: "Failure details", truncated: true },
    },
    "opencode",
  );
  await expect(row).toContainText("Failed · 5.0s");
  await expect(row).toContainText("Exit 7");
  await expect(row.getByRole("button", { name: /Run cargo test/ })).toHaveAttribute("aria-expanded", "true");
  await expect(row).toContainText("Failure details");
  await expect(row).toContainText("The provider returned partial output");
  await expect(
    row.getByRole("button", { name: "Copy shell ID" }),
  ).toBeVisible();
  await page.evaluate(
    ({ shell, started }) => {
      (window as any).fixture.setOpenHistory([
        {
          id: "msg_live",
          type: "shell",
          shellID: shell.id,
          command: shell.command,
          status: "exited",
          exit: 7,
          time: { created: started, completed: started + 5000 },
          output: { output: "Failure details", truncated: true },
        },
      ]);
      window.dispatchEvent(new Event("focus"));
    },
    { shell, started },
  );
  await expect(row).toHaveCount(1);
  await page.setViewportSize({ width: 800, height: 620 });
  await page.screenshot({ path: info.outputPath("shell-details.png") });
});

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} shows bounded recent output for running shell tools`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
    }
    const output = "old output\n".repeat(5000) + "recent output\nlast line";
    await emit(
      page,
      provider === "codex" ? "item/started" : "session.tool.called",
      provider === "codex"
        ? {
            threadId: "thread-a",
            item: {
              id: "tail",
              type: "commandExecution",
              command: "pnpm test",
              cwd: "/work/caelum",
            },
          }
        : {
            sessionID: "ses-a",
            id: "tail",
            name: "shell",
            input: { command: "pnpm test", workdir: "/work/caelum" },
          },
      provider,
    );
    await emit(
      page,
      provider === "codex"
        ? "item/commandExecution/outputDelta"
        : "session.tool.progress",
      provider === "codex"
        ? { threadId: "thread-a", itemId: "tail", delta: output }
        : { sessionID: "ses-a", id: "tail", metadata: { output } },
      provider,
    );
    const recent = page.getByLabel("Recent shell output", { exact: true });
    await expect(recent).toContainText("recent output\nlast line");
    expect((await recent.textContent())!.length).toBeLessThan(4001);
    const row = page.locator('.tool-activity[data-running="true"]').filter({ hasText: "pnpm test" });
    await expect(row).toContainText("/work/caelum");
    await row.getByRole("button", { name: /pnpm test/ }).click();
    await expect(recent).toHaveCount(0);
    await expect(
      row.getByRole("button", { name: /Show full output/ }),
    ).toBeVisible();
  });
}
