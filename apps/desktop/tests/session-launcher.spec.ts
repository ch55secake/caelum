import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, createCheckout, emit } from "./fixtures/helpers.js";

const setup = async (page: Page) => {
  await page
    .getByRole("button", { name: "Session setup", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "New session setup",
    exact: true,
  });
  await expect(dialog).toBeVisible();
  return dialog;
};
const starts = (page: Page) =>
  page.evaluate(() =>
    (window as any).fixture.calls.filter((call: any) =>
      call.command.endsWith("_start_session"),
    ),
  );

test("primary click and keyboard open the current checkout/provider draft without creating a session", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Original checkout draft");
  await createCheckout(page, "feature/launcher");
  await choose(page, "Provider", "OpenCode");
  await message.fill("Worktree draft");
  await page.getByRole("button", { name: "New session", exact: true }).click();
  await expect(message).toBeFocused();
  await expect(message).toHaveValue("Worktree draft");
  await expect(page.locator(".new-session-context")).toContainText(
    "feature/launcher",
  );
  await expect(page.locator(".new-session-context")).toContainText("OpenCode");
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await page.keyboard.press("Meta+n");
  await expect(message).toBeFocused();
  await expect(message).toHaveValue("Worktree draft");
  const dialog = await setup(page);
  await expect(
    dialog.getByRole("button", { name: "Session checkout", exact: true }),
  ).toContainText("feature/launcher");
  await expect(
    dialog.getByRole("button", { name: "Session provider", exact: true }),
  ).toContainText("OpenCode");
  await page.keyboard.press("Escape");
  await expect(
    page.getByRole("button", { name: "Session setup", exact: true }),
  ).toBeFocused();
  expect(await starts(page)).toHaveLength(0);
});

test("setup searches projects, opens folders and scopes the provider on first send while background work continues", async ({
  page,
}) => {
  await desktop(page, "thread-a", { worktrees: true });
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Keep the active session draft");
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "background" },
  });
  const dialog = await setup(page);
  await dialog.getByRole("button", { name: "Open folder…" }).click();
  await expect(
    dialog.getByRole("button", { name: "Session project", exact: true }),
  ).toContainText("other");
  await expect(page.locator(".breadcrumbs")).toContainText("caelum");
  await dialog
    .getByRole("button", { name: "Session project", exact: true })
    .click();
  const search = dialog.getByRole("combobox", {
    name: "Search session projects",
  });
  await search.fill("/work/caelum");
  await expect(dialog.getByRole("option", { name: /caelum/ })).toHaveCount(1);
  await search.press("Escape");
  await expect(dialog).toBeVisible();
  await choose(page, "Session provider", "OpenCode");
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    turnId: "background",
    itemId: "live",
    delta: "Still working in the original session",
  });
  await dialog
    .getByRole("button", { name: "Start writing", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  await expect(message).toBeFocused();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await expect(
    page.getByRole("button", { name: "Provider", exact: true }),
  ).toContainText("OpenCode");
  expect(await starts(page)).toHaveLength(0);
  await message.fill("A separate project task");
  await message.press("Enter");
  await expect.poll(async () => (await starts(page)).length).toBe(1);
  expect((await starts(page))[0]).toMatchObject({
    command: "opencode_start_session",
    args: { projectPath: "/work/other" },
  });
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(message).toHaveValue("Keep the active session draft");
  await expect(page.locator(".provider-label")).toContainText("Codex");
  await expect(page.locator(".transcript")).toContainText(
    "Still working in the original session",
  );
});

test("worktree setup retains selections through creation and opening errors, and prevents duplicate starts", async ({
  page,
}, info) => {
  await desktop(page, null, { worktrees: true });
  const dialog = await setup(page);
  await choose(page, "Session provider", "OpenCode");
  await dialog
    .getByRole("button", { name: "Create worktree…", exact: true })
    .click();
  const create = page.getByRole("dialog", {
    name: "Create worktree",
    exact: true,
  });
  await page.keyboard.press("Escape");
  await expect(create).toHaveCount(0);
  await expect(dialog).toBeVisible();
  await dialog
    .getByRole("button", { name: "Create worktree…", exact: true })
    .click();
  await create
    .getByRole("textbox", { name: "New branch", exact: true })
    .fill("feature/setup");
  await create
    .getByRole("combobox", { name: "Start from", exact: true })
    .selectOption("release");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "create_worktree",
      "Git: branch already exists",
    ),
  );
  await create
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(create.getByRole("alert")).toContainText(
    "branch already exists",
  );
  await expect(
    create.getByRole("textbox", { name: "New branch", exact: true }),
  ).toHaveValue("feature/setup");
  await expect(
    create.getByRole("combobox", { name: "Start from", exact: true }),
  ).toHaveValue("release");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("create_worktree", null),
  );
  await create
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(create).toHaveCount(0);
  await expect(
    dialog.getByRole("button", { name: "Session checkout", exact: true }),
  ).toContainText("feature/setup");
  await expect(
    dialog.getByRole("button", { name: "Session provider", exact: true }),
  ).toContainText("OpenCode");
  await expect(page.locator(".breadcrumbs")).not.toContainText("feature/setup");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "open_worktree",
      "Checkout temporarily unavailable",
    ),
  );
  await dialog
    .getByRole("button", { name: "Start writing", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText(
    "temporarily unavailable",
  );
  await expect(
    dialog.getByRole("button", { name: "Session checkout", exact: true }),
  ).toContainText("feature/setup");
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(
    dialog.getByRole("button", { name: "Start writing", exact: true }),
  ).toBeInViewport();
  await page.screenshot({ path: info.outputPath("session-setup-minimum.png") });
  await page.evaluate(() => {
    (window as any).fixture.setCommandError("open_worktree", null);
    (window as any).fixture.hold("open_worktree");
  });
  await dialog
    .getByRole("button", { name: "Start writing", exact: true })
    .dblclick();
  await expect(
    dialog.getByRole("button", { name: "Opening…", exact: true }),
  ).toBeDisabled();
  await page.evaluate(() => (window as any).fixture.release("open_worktree"));
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".breadcrumbs")).toContainText("feature/setup");
  await expect(
    page.getByRole("button", { name: "Provider", exact: true }),
  ).toContainText("OpenCode");
  expect(await starts(page)).toHaveLength(0);
  await page.screenshot({
    path: info.outputPath("session-welcome-minimum.png"),
  });
});

test("empty workspaces can launch setup with the keyboard and choose a folder", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  await page.getByRole("button", { name: "Close caelum", exact: true }).click();
  await page.keyboard.press("Meta+n");
  const dialog = page.getByRole("dialog", {
    name: "New session setup",
    exact: true,
  });
  await expect(
    dialog.getByRole("button", { name: "Start writing", exact: true }),
  ).toBeDisabled();
  await dialog.getByRole("button", { name: "Open folder…" }).click();
  await dialog
    .getByRole("button", { name: "Start writing", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  expect(await starts(page)).toHaveLength(0);
});
