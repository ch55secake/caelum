import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, emit } from "./fixtures/helpers.js";

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
