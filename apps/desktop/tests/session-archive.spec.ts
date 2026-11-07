import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose, sessionRow } from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"]) {
  const title =
    provider === "codex" ? "Refine the session workspace" : "OpenCode session";
  const id = provider === "codex" ? "thread-a" : "ses-a";
  test(`${provider} protects active sessions and keeps archive failures and Undo from disturbing navigation`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const row = sessionRow(page, title);
    const archive = row.getByRole("button", {
      name: "Archive session",
      exact: true,
    });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Work on this session");
    await input.press("Enter");
    await expect(archive).toBeDisabled();
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.succeeded",
      provider === "codex"
        ? { threadId: id, turn: { id: "turn-new", status: "completed" } }
        : { sessionID: id },
      provider,
    );
    await expect(archive).toBeEnabled();
    await page.evaluate(
      ({ provider, id }) =>
        (window as any).fixture.emit("approval-requests", [
          {
            provider,
            sessionId: id,
            projectPath: "/work/caelum",
            requestId: "archive-approval",
            summary: "Approve command",
            description: "Run a command",
          },
        ]),
      { provider, id },
    );
    await expect(archive).toBeDisabled();
    await page.evaluate(() =>
      (window as any).fixture.emit("approval-requests", []),
    );
    await input.fill("Draft kept if archiving fails");
    await page.evaluate(() => (window as any).fixture.failArchive());
    await row.hover();
    await archive.click();
    await expect(page.getByRole("alert")).toContainText(
      "Could not archive session",
    );
    await expect(input).toHaveValue("Draft kept if archiving fails");
    await expect(archive).toBeEnabled();
    await page.getByRole("button", { name: "Dismiss error" }).click();
    await page.evaluate(() =>
      (window as any).fixture.hold("set_session_archived"),
    );
    await archive.click();
    await expect(archive).toBeDisabled();
    await expect(input).toBeDisabled();
    await page
      .getByRole("button", {
        name: "Review the provider adapters",
        exact: true,
      })
      .click();
    await input.fill("Other session draft");
    await page.evaluate(() =>
      (window as any).fixture.release("set_session_archived"),
    );
    await expect(row).toHaveCount(0);
    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(row).toHaveCount(1);
    await expect(input).toHaveValue("Other session draft");
    await expect(page.locator(".breadcrumbs")).toContainText(
      "Review the provider adapters",
    );
    await page.getByRole("button", { name: title, exact: true }).click();
    await expect(input).toHaveValue("Draft kept if archiving fails");
  });
}

test("OpenCode sessions no longer owned by a project can be forgotten locally", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const row = sessionRow(page, "OpenCode session");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCheckoutSessions("/work/caelum", "opencode", []);
    fixture.setCommandError(
      "set_session_archived",
      "OpenCode session does not belong to this project",
    );
  });
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await expect(row).toHaveCount(1);
  await row.hover();
  await row.getByRole("button", { name: "Archive session" }).click();
  await expect(row).toHaveCount(0);
  await expect(page.getByRole("alert")).toContainText(
    "It was removed from Caelum, but was not archived in OpenCode",
  );
});

test("late listings, history and metadata cannot overwrite an archived and restored session", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const row = sessionRow(page, "OpenCode session");
  await expect(row.locator(".session-branch")).toHaveText("main");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.holdHistory();
    fixture.setOpenWorking(true);
    fixture.hold("session_details");
    fixture.hold("opencode_list_sessions");
    window.dispatchEvent(new Event("focus"));
  });
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await row.hover();
  await row
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(row).toHaveCount(0);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setOpenWorking(false);
    fixture.setSessionDetails("opencode", "ses-a", {
      branch: "fresh-branch",
      branchIsCurrent: false,
      changes: { additions: 19, deletions: 4 },
    });
  });
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(row).toHaveCount(1);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.releaseHistory();
    fixture.release("session_details");
    fixture.release("opencode_list_sessions");
  });
  await expect(row.locator(".session-branch")).toHaveText("fresh-branch");
  await expect(row.locator(".lines-added")).toHaveText("+19");
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveAttribute("data-working", "false");
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  await row.hover();
  await row
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(row).toHaveCount(0);
  await emit(
    page,
    "session.title.updated",
    { sessionID: "ses-a", title: "Late title" },
    "opencode",
  );
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await expect(
    page.locator(".session-item").filter({ hasText: "Late title" }),
  ).toHaveCount(0);
});
