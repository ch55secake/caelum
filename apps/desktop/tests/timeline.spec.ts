import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, emit, attachedContext } from "./fixtures/helpers.js";

async function timelineHistory(page: Page, provider: string) {
  await page.evaluate((provider) => {
    const id = provider === "codex" ? "thread-a" : "ses-a";
    for (const [cursor, start, nextCursor] of [
      [null, 80, "middle"],
      ["middle", 40, "earliest"],
      ["earliest", 0, null],
    ] as const) {
      const data = Array.from({ length: 40 }, (_, index) => {
        const number = start + index;
        const text = `Prompt ${number}${number === 3 ? " — old anchor" : ""}`;
        return provider === "codex"
          ? {
              id: `turn-${number}`,
              status: "completed",
              items: [
                {
                  id: `u-${number}`,
                  type: "userMessage",
                  content: [{ type: "text", text }],
                },
                {
                  id: `a-${number}`,
                  type: "agentMessage",
                  text: `Answer ${number}`,
                },
              ],
            }
          : [
              { id: `u-${number}`, type: "user", text },
              {
                id: `a-${number}`,
                type: "assistant",
                content: [{ type: "text", text: `Answer ${number}` }],
              },
            ];
      }).flat();
      (window as any).fixture.setHistoryPage(provider, id, cursor, {
        data,
        nextCursor,
        isWorking: false,
      });
    }
  }, provider);
}

for (const provider of ["codex", "opencode"]) {
  test(`${provider} timeline loads older pages and reveals messages outside the bounded transcript`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 800, height: 620 });
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await timelineHistory(page, provider);
    const title =
      provider === "codex"
        ? "Refine the session workspace"
        : "OpenCode session";
    await page.getByRole("button", { name: title, exact: true }).click();
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(page.locator(".user-text").last()).toHaveText("Prompt 119");
    await input.fill("@README");
    await page.getByRole("option", { name: "README.md", exact: true }).click();
    await input.fill("/timeline");
    await input.press("Enter");
    const dialog = page.getByRole("dialog", { name: "Session timeline" });
    await expect(dialog.getByRole("status")).toHaveText("120 messages");
    const search = dialog.getByRole("searchbox", {
      name: "Search user messages",
    });
    await expect(search).toBeFocused();
    await expect(dialog.getByRole("option").first()).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await dialog.screenshot({
      path: `test-results/${provider}-timeline-compact.png`,
    });
    await search.fill("old anchor");
    await expect(dialog.getByRole("option")).toHaveCount(1);
    await expect(dialog.locator("mark")).toHaveText("old anchor");
    await search.press("Enter");
    await expect(dialog).toHaveCount(0);
    const target = page.locator(`[data-entry-id="${provider}-u-3"]`);
    await expect(target).toBeInViewport();
    await expect(target).toHaveAttribute("data-timeline-target", "true");
    await expect(target).toBeFocused();
    expect(
      await page.locator(".conversation-entry").count(),
    ).toBeLessThanOrEqual(100);
    await expect(attachedContext(page)).toContainText("README.md");
    const position = await page
      .locator(".transcript")
      .evaluate((node) => node.scrollTop);
    if (provider === "codex")
      await emit(page, "item/agentMessage/delta", {
        threadId: "thread-a",
        itemId: "new-answer",
        delta: "Live tail",
      });
    else
      await emit(
        page,
        "session.text.delta",
        {
          sessionID: "ses-a",
          assistantMessageID: "new-answer",
          delta: "Live tail",
        },
        "opencode",
      );
    expect(
      await page.locator(".transcript").evaluate((node) => node.scrollTop),
    ).toBe(position);
    await input.fill("/timeline");
    await input.press("Enter");
    await expect(dialog).toBeVisible();
    await search.press("ArrowDown");
    await expect(dialog.getByRole("option").nth(1)).toHaveAttribute(
      "aria-selected",
      "true",
    );
    // Keys work from the list and header too, including WebKit's search Escape.
    await dialog.getByRole("listbox", { name: "User messages" }).focus();
    await page.keyboard.press("ArrowDown");
    await expect(dialog.getByRole("option").nth(2)).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await dialog.getByRole("button", { name: "Close timeline" }).focus();
    await page.keyboard.press("ArrowUp");
    await expect(dialog.getByRole("option").nth(1)).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await search.press("End");
    await expect(dialog.getByRole("option").last()).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await expect(dialog.getByRole("option").last()).toBeInViewport();
    await search.fill("Prompt");
    await page.keyboard.press("Escape");
    await expect(input).toBeFocused();
    expect(
      await page.locator(".transcript").evaluate((node) => node.scrollTop),
    ).toBe(position);
    await page
      .getByRole("button", { name: "Jump to latest", exact: true })
      .click();
    await expect(page.getByText("Live tail", { exact: true })).toBeInViewport();
    await expect(page.locator('[data-timeline-target="true"]')).toHaveCount(0);
    const history = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === `${provider}_session_history`)
          .map((call: any) => call.args.cursor),
      provider,
    );
    expect(history).toContain("middle");
    expect(history).toContain("earliest");
  });
}

test("timeline retries page failures and pending loads stay scoped after dismissal and navigation", async ({
  page,
}) => {
  await desktop(page, null);
  await timelineHistory(page, "codex");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "codex_session_history:thread-a:middle",
      "History offline",
    ),
  );
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/timeline");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Session timeline" });
  await expect(dialog.getByRole("alert")).toContainText("History offline");
  await expect(dialog.getByRole("option")).toHaveCount(40);
  await page.evaluate(() => {
    (window as any).fixture.setCommandError(
      "codex_session_history:thread-a:middle",
      null,
    );
    (window as any).fixture.hold("codex_session_history:thread-a:middle");
  });
  await dialog.getByRole("button", { name: "Retry timeline" }).click();
  await expect(dialog.getByRole("status")).toContainText("Loading earlier");
  await page.keyboard.press("Escape");
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await input.fill("Keep this other draft");
  await page.evaluate(() =>
    (window as any).fixture.release("codex_session_history:thread-a:middle"),
  );
  await expect(input).toHaveValue("Keep this other draft");
  await expect(page.locator(".user-text")).not.toContainText(["Prompt 40"]);
  await expect(dialog).toHaveCount(0);
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await input.fill("/timeline");
  await input.press("Enter");
  await expect(dialog.getByRole("status")).toHaveText("120 messages");
  await page.keyboard.press("Escape");
  await page.getByRole("button", { name: /New session/ }).click();
  await input.fill("/timeline");
  await input.press("Enter");
  await expect(page.getByRole("alert")).toContainText("Open a session");
  await expect(input).toHaveValue("/timeline");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.command.endsWith("_send_prompt") ||
          call.command.endsWith("_start_session"),
      ),
    ),
  ).toEqual([]);
});
