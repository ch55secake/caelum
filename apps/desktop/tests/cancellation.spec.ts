import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { attachedContext, choose } from "./fixtures/helpers.js";

const cancellations = (page: Page) => page.evaluate(() =>
  (window as any).fixture.calls.filter((call: any) => call.command.endsWith("_cancel_turn")),
);

for (const provider of ["codex", "opencode"]) {
  test(`${provider} double Escape stops the active turn, pauses its queue and preserves the draft`, async ({ page }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: "OpenCode session", exact: true }).click();
    }
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Run the task");
    await input.press("Enter");
    await expect(page.getByRole("button", { name: "Stop generation" })).toBeVisible();
    await input.fill("Follow-up task");
    await input.press("Enter");
    await expect(page.getByRole("region", { name: "Queued messages" })).toContainText("Follow-up task");
    await input.fill("@README");
    await page.getByRole("option", { name: "README.md", exact: true }).click();
    await input.fill("Keep my unsent draft");
    await input.press("Escape");
    expect(await cancellations(page)).toHaveLength(0);
    await input.press("Escape");
    await expect(page.getByRole("button", { name: "Stop generation" })).toHaveCount(0);
    expect(await cancellations(page)).toEqual([{
      command: `${provider}_cancel_turn`,
      args: { projectPath: "/work/caelum", threadId: provider === "codex" ? "thread-a" : "ses-a", sessionId: provider === "codex" ? "thread-a" : "ses-a" },
    }]);
    await expect(page.getByRole("region", { name: "Queued messages" })).toContainText("Paused");
    await expect(input).toHaveValue("Keep my unsent draft");
    await expect(attachedContext(page)).toContainText("README.md");
    await input.press("Escape");
    await input.press("Escape");
    expect(await cancellations(page)).toHaveLength(1);
  });
}

test("Escape cancellation requires two fresh presses in the same session", async ({ page }) => {
  await page.clock.install();
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Run task A");
  await input.press("Enter");
  await expect(page.getByRole("button", { name: "Stop generation" })).toBeVisible();
  await input.press("Escape");
  await page.clock.fastForward(751);
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);
  await input.dispatchEvent("keydown", { key: "Escape", repeat: true });
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);
  await input.dispatchEvent("keydown", { key: "Escape", isComposing: true });
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);
  await input.press("Shift+Escape");
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);
  await input.press("x");
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);

  await page.getByRole("button", { name: "Review the provider adapters", exact: true }).click();
  await input.fill("Run task B");
  await input.press("Enter");
  await expect(page.getByRole("button", { name: "Stop generation" })).toBeVisible();
  await input.press("Escape");
  // Programmatic navigation avoids resetting the shortcut via pointerdown.
  await page.getByRole("button", { name: "Refine the session workspace", exact: true }).evaluate((node: HTMLButtonElement) => node.click());
  await expect(input).toHaveValue("x");
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);
  await input.press("Escape");
  await expect.poll(async () => (await cancellations(page)).length).toBe(1);
  expect((await cancellations(page))[0].args.threadId).toBe("thread-a");
  await page.getByRole("button", { name: "Review the provider adapters", exact: true }).click();
  await expect(page.getByRole("button", { name: "Stop generation" })).toBeVisible();
});

test("menus, dialogs and side-question dismissal take priority over Escape cancellation", async ({ page }) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.getByRole("button", { name: "OpenCode session", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Run the main task");
  await input.press("Enter");
  await expect(page.getByRole("button", { name: "Stop generation" })).toBeVisible();
  await input.fill("/");
  await expect(page.getByRole("listbox", { name: "Slash commands" })).toBeVisible();
  await input.press("Escape");
  await expect(page.getByRole("listbox", { name: "Slash commands" })).toHaveCount(0);
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);

  await page.getByRole("button", { name: "Model", exact: true }).click();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Choose model" })).toHaveCount(0);
  await page.keyboard.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);

  await input.fill("/usage");
  await input.press("Enter");
  const usage = page.getByRole("dialog", { name: "OpenCode Go usage", exact: true });
  await expect(usage).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(usage).toHaveCount(0);
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);

  await input.fill("/btw");
  await input.press("Enter");
  const question = page.getByRole("textbox", { name: "Question", exact: true });
  await expect(question).toBeFocused();
  await question.press("Escape");
  await expect(question).toHaveCount(0);
  await input.press("Escape");
  expect(await cancellations(page)).toHaveLength(0);
  await input.press("Escape");
  await expect.poll(async () => (await cancellations(page)).length).toBe(1);
});
