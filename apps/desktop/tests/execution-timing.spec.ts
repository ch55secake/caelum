import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, emit } from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} times the whole prompt across tools, navigation and completion`, async ({
    page,
  }) => {
    await page.clock.install();
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
      await page.evaluate(() => (window as any).fixture.setOpenWorking(true));
    }
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await page.evaluate(
      (provider) => (window as any).fixture.hold(`${provider}_send_prompt`),
      provider,
    );
    await input.fill("Measure all the work");
    await input.press("Enter");
    const timing = page.getByLabel("Prompt timing", { exact: true });
    await expect(timing).toContainText("Working · 0s");
    await page.clock.fastForward(5000);
    await page.evaluate(
      (provider) => (window as any).fixture.release(`${provider}_send_prompt`),
      provider,
    );
    if (provider === "opencode")
      await emit(
        page,
        "session.execution.started",
        { sessionID: "ses-a" },
        provider,
      );
    await emit(
      page,
      provider === "codex" ? "item/started" : "session.tool.called",
      provider === "codex"
        ? {
            threadId: "thread-a",
            item: {
              id: "slow",
              type: "commandExecution",
              command: "cargo test",
            },
          }
        : {
            sessionID: "ses-a",
            id: "slow",
            name: "shell",
            input: { command: "cargo test" },
          },
      provider,
    );
    await page.clock.fastForward(20000);
    await emit(
      page,
      provider === "codex" ? "item/completed" : "session.tool.success",
      provider === "codex"
        ? {
            threadId: "thread-a",
            item: {
              id: "slow",
              type: "commandExecution",
              command: "cargo test",
              status: "completed",
              exitCode: 0,
            },
          }
        : {
            sessionID: "ses-a",
            id: "slow",
            content: [{ type: "text", text: "passed" }],
          },
      provider,
    );
    await expect(timing).toContainText("Working · 25s");
    await page
      .getByRole("button", { name: "New session", exact: true })
      .click();
    await page.clock.fastForward(10000);
    await page
      .getByRole("button", {
        name:
          provider === "codex"
            ? "Refine the session workspace"
            : "OpenCode session",
        exact: true,
      })
      .click();
    await expect(timing).toContainText("Working · 35s");
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
    await expect(timing).toContainText("Completed · 35s");
    await page.clock.fastForward(10000);
    await expect(timing).toContainText("Completed · 35s");
  });
}

test("queued waiting stays separate from execution and failed sends stop the prompt clock", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "first" },
  });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Run this next");
  await input.press("Enter");
  await page.clock.fastForward(10000);
  await expect(
    page.getByRole("region", { name: "Queued messages" }),
  ).toContainText("Waiting 10s");
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "first", status: "completed" },
  });
  const timing = page.getByLabel("Prompt timing", { exact: true });
  await expect(timing).toContainText("Working · 0s");
  await expect(
    page.getByLabel("Prompt queue time", { exact: true }),
  ).toContainText("Queued for 10s");
  await page.clock.fastForward(5000);
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "turn-new", status: "interrupted" },
  });
  await expect(timing).toContainText("Interrupted · 5s");
  await page.evaluate(() => (window as any).fixture.failSend());
  await input.fill("A failed send");
  await input.press("Enter");
  await expect(timing).toContainText("Failed · 0s");
  await page.clock.fastForward(5000);
  await expect(timing).toContainText("Failed · 0s");
  await expect(input).toHaveValue("A failed send");
});
