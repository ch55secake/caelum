import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, emit, question, setQuestions } from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} publishes pending output with input requests and terminal events while timers are paused`, async ({
    page,
  }) => {
    await page.clock.install();
    await desktop(page, provider === "codex" ? "thread-a" : null);
    const id = provider === "codex" ? "thread-a" : "ses-a";
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
    }
    await emit(
      page,
      provider === "codex" ? "turn/started" : "session.execution.started",
      provider === "codex"
        ? { threadId: id, turn: { id: "published" } }
        : { sessionID: id },
      provider,
    );
    await expect(
      page.getByLabel("Prompt timing", { exact: true }),
    ).toContainText("Working");
    await page.clock.pauseAt(await page.evaluate(() => Date.now() + 100));
    const delta = (text: string) =>
      emit(
        page,
        provider === "codex" ? "item/agentMessage/delta" : "session.text.delta",
        provider === "codex"
          ? { threadId: id, itemId: "publication", delta: text }
          : { sessionID: id, assistantMessageID: "publication", delta: text },
        provider,
      );
    await delta("Output before question");
    await setQuestions(page, [question({ provider, sessionId: id })]);
    await expect(page.locator(".transcript")).toContainText(
      "Output before question",
    );
    await delta(" and before interruption");
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.interrupted",
      provider === "codex"
        ? { threadId: id, turn: { id: "published", status: "interrupted" } }
        : { sessionID: id, reason: "user" },
      provider,
    );
    await expect(page.locator(".transcript")).toContainText(
      "Output before question and before interruption",
    );
    await expect(
      page.getByLabel("Prompt timing", { exact: true }),
    ).toContainText("Interrupted");
    await page.clock.fastForward(100);
    await expect(
      page.getByLabel("Prompt timing", { exact: true }),
    ).toContainText("Interrupted");
  });
}

test("pending background output stays ordered across navigation and stale history", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await expect(
    page.getByText("A focused workspace", { exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(
    page.getByText("A focused workspace", { exact: true }),
  ).toBeVisible();
  await page.evaluate(() => (window as any).fixture.holdHistory());
  await page.clock.pauseAt(await page.evaluate(() => Date.now() + 100));
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    window.dispatchEvent(new Event("focus"));
    for (let index = 0; index < 50; index++) {
      for (const threadId of ["thread-a", "thread-b"])
        fixture.emit("codex-event", {
          projectPath: "/work/caelum",
          method: "item/agentMessage/delta",
          params: {
            threadId,
            itemId: "ordered",
            delta: `${threadId}:${index} `,
          },
        });
    }
  });
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .evaluate((button: HTMLButtonElement) => button.click());
  const output = page.locator('[data-entry-id="codex-ordered"]');
  await expect(output).toContainText(
    Array.from({ length: 50 }, (_, index) => `thread-b:${index}`).join(" "),
  );
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .evaluate((button: HTMLButtonElement) => button.click());
  await expect(output).toContainText(
    Array.from({ length: 50 }, (_, index) => `thread-a:${index}`).join(" "),
  );
});
