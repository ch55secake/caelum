import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  emit,
  openSettings,
  question,
  setQuestions,
  sessionRow,
} from "./fixtures/helpers.js";

const projectRow = (page: Page) =>
  page
    .locator(".project-row")
    .filter({ has: page.getByRole("button", { name: "caelum", exact: true }) });
const unread = (page: Page) =>
  projectRow(page).getByRole("button", { name: /unread.*Open next unread/ });

test("working and unread coexist for unopened sessions, and unread clears only after their latest history is shown", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft");
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "viewed",
    delta: "Visible update",
  });
  await expect(unread(page)).toHaveCount(0);
  await emit(page, "turn/started", {
    threadId: "thread-b",
    turn: { id: "background" },
  });
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-b",
    itemId: "new",
    delta: "An unread background response",
  });
  await expect(unread(page)).toHaveAccessibleName(
    "1 unread session. Open next unread",
  );
  await expect(projectRow(page).getByLabel("2 working sessions")).toBeVisible();
  const background = sessionRow(page, "Review the provider adapters");
  await expect(background.locator(".running-dot")).toBeVisible();
  await expect(background.locator(".unread-dot")).toBeVisible();
  await page.evaluate(() => (window as any).fixture.holdHistory());
  await unread(page).click();
  await expect(page.locator(".breadcrumbs")).toContainText(
    "Review the provider adapters",
  );
  await expect(unread(page)).toBeVisible();
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await expect(unread(page)).toHaveCount(0);
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(input).toHaveValue("Keep this draft");
});

test("foreground, reading position and modal visibility govern read acknowledgements", async ({
  page,
}) => {
  await desktop(page);
  await page
    .getByRole("button", { name: "Load earlier messages", exact: true })
    .click();
  await expect(page.locator(".history-button:disabled")).toHaveCount(0);
  await page.locator(".transcript").evaluate((node) => {
    node.scrollTop = 100;
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "while-reading",
    delta: "Read this later",
  });
  await expect(unread(page)).toBeVisible();
  await unread(page).click();
  await expect(unread(page)).toBeVisible();
  await page
    .getByRole("button", { name: "Jump to latest", exact: true })
    .click();
  await expect(unread(page)).toHaveCount(0);
  const settings = await openSettings(page);
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "under-settings",
    delta: "Hidden by settings",
  });
  await expect(projectRow(page).locator(".unread")).toHaveCount(1);
  await settings.getByRole("button", { name: "Done", exact: true }).click();
  await expect(unread(page)).toHaveCount(0);
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "out-of-focus",
    delta: "Read on return",
  });
  await expect(unread(page)).toBeVisible();
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await expect(unread(page)).toHaveCount(0);
});

test("read markers survive restart without replaying historical activity or losing background unread state", async ({
  page,
}) => {
  await desktop(page);
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-b",
    itemId: "persisted",
    delta: "Waiting for you",
  });
  await expect(unread(page)).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(() => localStorage.getItem("caelum.session-read.v1")),
    )
    .toContain('"unread":true');
  await page.reload();
  await expect(unread(page)).toHaveAccessibleName(
    "1 unread session. Open next unread",
  );
  await unread(page).click();
  await expect(unread(page)).toHaveCount(0);
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await page.reload();
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeVisible();
  await expect(unread(page)).toHaveCount(0);
});

test("project activity counts providers and worktrees, and approvals and questions share attention by session", async ({
  page,
}, info) => {
  await desktop(page, "thread-a", { worktrees: true });
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.addWorktree("/work/linked", "feature/linked");
    fixture.setCheckoutSessions("/work/linked", "opencode", [
      { id: "thread-a", title: "Linked task", updatedAt: 1, isWorking: true },
    ]);
    window.dispatchEvent(new Event("focus"));
  });
  await expect(
    page.getByRole("button", { name: "Linked task", exact: true }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.emit("opencode-event", {
      projectPath: "/work/linked",
      eventType: "session.text.delta",
      data: {
        sessionID: "thread-a",
        assistantMessageID: "linked",
        delta: "Background worktree output",
      },
    }),
  );
  await setQuestions(page, [
    question(),
    question({
      requestId: "42",
      projectPath: "/work/linked",
      provider: "opencode",
    }),
  ]);
  await page.evaluate(() =>
    (window as any).fixture.emit("approval-requests", [
      {
        provider: "codex",
        projectPath: "/work/caelum",
        sessionId: "thread-a",
        requestId: "approve",
        summary: "Run a command?",
        description: "git status",
      },
    ]),
  );
  await expect(
    projectRow(page).getByRole("button", {
      name: "2 sessions need attention",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    projectRow(page).getByLabel("1 working session", { exact: true }),
  ).toBeVisible();
  await expect(unread(page)).toHaveAccessibleName(
    "1 unread session. Open next unread",
  );
  await page.setViewportSize({ width: 800, height: 620 });
  await page.screenshot({
    path: info.outputPath("project-activity-minimum.png"),
  });
  await unread(page).click();
  await expect(page.locator(".breadcrumbs")).toContainText("Linked task");
  await expect(unread(page)).toHaveCount(0);
  await projectRow(page)
    .getByRole("button", { name: "2 sessions need attention", exact: true })
    .click();
  await expect(page.locator(".breadcrumbs")).toContainText(
    "Refine the session workspace",
  );
});
