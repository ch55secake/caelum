import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose } from "./fixtures/helpers.js";

test("Codex queued messages drain in order in their background session and pause on failures", async ({
  page,
}) => {
  await desktop(page);
  await emit(page, "turn/started", { threadId: "thread-a" });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  for (const text of ["Next task", "Then review"]) {
    await input.fill(text);
    await input.press("Enter");
    await expect(input).toHaveValue("");
  }
  await expect(
    page.getByRole("region", { name: "Queued messages" }).locator("li"),
  ).toHaveCount(2);
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await input.fill("Independent draft");
  const sent = () =>
    page.evaluate(() =>
      (window as any).fixture.calls
        .filter((call: any) => call.command === "codex_send_prompt")
        .map((call: any) => call.args),
    );
  expect(await sent()).toHaveLength(0);
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "one", status: "completed" },
  });
  await expect.poll(sent).toHaveLength(1);
  expect((await sent())[0]).toMatchObject({
    projectPath: "/work/caelum",
    threadId: "thread-a",
    prompt: "Next task",
  });
  await expect(input).toHaveValue("Independent draft");
  await page.evaluate(() => (window as any).fixture.failSend());
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "two", status: "completed" },
  });
  await expect.poll(sent).toHaveLength(2);
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  const queue = page.getByRole("region", { name: "Queued messages" });
  await expect(queue).toContainText("Connection refused");
  await expect(queue).toContainText("Then review");
  await queue.getByRole("button", { name: "Resume queue" }).click();
  await expect.poll(sent).toHaveLength(3);
  await expect(queue).toHaveCount(0);
});

test("OpenCode queued messages restore from the native inbox, cancel and reconcile delivery", async ({
  page,
}) => {
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
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  for (const text of ["First queued", "Remove this"]) {
    await input.fill(text);
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Queue message" }),
    ).toBeDisabled();
    await expect(input).toHaveValue("");
  }
  const queue = page.getByRole("region", { name: "Queued messages" });
  await expect(queue.locator("li")).toHaveCount(2);
  await queue
    .getByRole("button", { name: "Remove queued message: Remove this" })
    .click();
  await expect(queue.locator("li")).toHaveCount(1);
  await page.reload();
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(queue).toContainText("First queued");
  await page.evaluate(() => (window as any).fixture.deliverQueued());
  await expect(queue).toHaveCount(0);
  await expect(
    page.locator(".user-text").filter({ hasText: "First queued" }),
  ).toHaveCount(1);
  const calls = await page.evaluate(() => (window as any).fixture.calls);
  expect(
    calls.filter((call: any) => call.command === "opencode_send_prompt"),
  ).toHaveLength(0);
});

test("queued OpenCode settings apply after the turn and stale inbox snapshots preserve pending work", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate(() => (window as any).fixture.hold("opencode_inbox"));
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
  await choose(page, "Effort", "High");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Use the queued settings");
  await input.press("Enter");
  const queue = page.getByRole("region", { name: "Queued messages" });
  await expect(queue).toContainText("Use the queued settings");
  await page.evaluate(() => (window as any).fixture.release("opencode_inbox"));
  const sent = () =>
    page.evaluate(() =>
      (window as any).fixture.calls
        .filter((call: any) => call.command === "opencode_send_prompt")
        .map((call: any) => call.args),
    );
  expect(await sent()).toHaveLength(0);
  await page.evaluate(() => (window as any).fixture.setOpenWorking(false));
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await expect.poll(sent).toHaveLength(1);
  expect((await sent())[0]).toMatchObject({
    sessionId: "ses-a",
    prompt: "Use the queued settings",
    effort: "high",
  });
  await expect(queue).toHaveCount(0);
});
