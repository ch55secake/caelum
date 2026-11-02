import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, emit, attachedContext } from "./fixtures/helpers.js";

for (const following of [true, false]) {
  test(`side question resizing preserves ${following ? "the latest message" : "an older reading position"}`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 800, height: 620 });
    await desktop(page, null);
    await page.evaluate(() => {
      const fixture = (window as any).fixture;
      fixture.setOpenHistory(
        Array.from({ length: 120 }, (_, index) => ({
          id: `message-${index}`,
          type: "assistant",
          content: [{ type: "text", text: `Message ${index}\n\nMore context.` }],
        })),
      );
      fixture.hold("opencode_generate");
      fixture.setGenerateReply("A long side answer.\n\n".repeat(40));
    });
    await choose(page, "Provider", "OpenCode");
    await page
      .getByRole("button", { name: "OpenCode session", exact: true })
      .click();
    const transcript = page.locator(".transcript");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(page.locator(".conversation-entry")).toHaveCount(100);
    if (!following) {
      await transcript.evaluate((node) => {
        node.scrollTop = 250;
      });
      await expect(
        page.getByRole("button", { name: "Jump to latest" }),
      ).toBeVisible();
    }
    const checkPosition = async () => {
      await expect
        .poll(() => transcript.evaluate(
          (node, following) => following
            ? node.scrollHeight - node.scrollTop - node.clientHeight
            : Math.abs(node.scrollTop - 250),
          following,
        ))
        .toBeLessThanOrEqual(1);
    };
    const draft = "Main draft\n".repeat(6).trim();
    await input.fill(draft);
    await checkPosition();
    await input.fill("/btw");
    await input.press("Enter");
    const panel = page.getByRole("region", {
      name: "Side question",
      exact: true,
    });
    const question = panel.getByRole("textbox", {
      name: "Question",
      exact: true,
    });
    await expect(question).toBeFocused();
    await checkPosition();
    await question.fill("First\nSecond\nThird\nFourth");
    await checkPosition();
    await question.pressSequentially(" more text");
    await checkPosition();
    await page.getByRole("button", { name: "Hide changes", exact: true }).click();
    await checkPosition();
    await page.getByRole("button", { name: "Show changes", exact: true }).click();
    await checkPosition();
    await question.fill("A long question\n".repeat(12));
    await checkPosition();
    await question.press("Enter");
    await expect(panel.getByRole("status")).toContainText("Answering");
    await checkPosition();
    await page.evaluate(() =>
      (window as any).fixture.release("opencode_generate"),
    );
    await expect(
      panel.getByRole("region", { name: "Side answer" }),
    ).toBeVisible();
    await checkPosition();
    await expect(panel).toBeInViewport({ ratio: 1 });
    await panel.press("Escape");
    await expect(input).toHaveValue(draft);
    await checkPosition();
    await input.fill("Short main draft");
    await input.fill("/btw");
    await input.press("Enter");
    await expect(question).toBeFocused();
    await checkPosition();
    await panel.press("Escape");
    await expect(input).toHaveValue("Short main draft");
    await checkPosition();
  });
}

test("side questions keep the main draft, attachments and running turn intact", async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 620 });
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("@README");
  await page.getByRole("option", { name: "README.md", exact: true }).click();
  await page.evaluate(() => {
    (window as any).fixture.setOpenWorking(true);
    (window as any).fixture.setGenerateReply(
      "**Short answer**\n\n```ts\nconst independent = true;\n```",
    );
  });
  await emit(
    page,
    "session.execution.started",
    { sessionID: "ses-a" },
    "opencode",
  );
  await emit(
    page,
    "session.text.delta",
    {
      sessionID: "ses-a",
      assistantMessageID: "main-answer",
      delta: "Continuing the main task.",
    },
    "opencode",
  );
  await expect(
    page.getByText("Continuing the main task.", { exact: true }),
  ).toBeVisible();
  const transcript = await page.locator(".transcript-content").innerText();
  await input.fill("Keep my main draft α");
  await expect(
    page.getByRole("button", { name: "Ask a side question", exact: true }),
  ).toHaveCount(0);
  await input.fill("/btw");
  await input.press("Enter");
  const dialog = page.getByRole("region", {
    name: "Side question",
    exact: true,
  });
  const question = dialog.getByRole("textbox", {
    name: "Question",
    exact: true,
  });
  await expect(question).toBeFocused();
  await expect(dialog.getByText("Temporary", { exact: true })).toHaveCount(0);
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await question.fill("Explain this independently α\nSecond line");
  await page.evaluate(() => (window as any).fixture.hold("opencode_generate"));
  await question.press("Enter");
  await expect(input).toHaveCount(0);
  await expect(dialog).toBeFocused();
  await expect(dialog).toBeInViewport({ ratio: 1 });
  await emit(
    page,
    "session.text.delta",
    {
      sessionID: "ses-a",
      assistantMessageID: "main-answer",
      delta: " Still progressing.",
    },
    "opencode",
  );
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_generate"),
  );
  await expect(
    dialog.getByRole("region", { name: "Side answer" }),
  ).toContainText("Short answer");
  await expect(dialog.locator("strong")).toHaveText("Short answer");
  await expect(dialog).toBeFocused();
  await expect(
    page.getByText("Continuing the main task. Still progressing.", {
      exact: true,
    }),
  ).toBeInViewport();
  expect(
    await page.locator(".transcript").evaluate((node) => node.clientHeight),
  ).toBeGreaterThanOrEqual(100);
  await page.screenshot({ path: "test-results/side-question-compact.png" });
  await dialog
    .getByRole("button", { name: "Copy side answer", exact: true })
    .click();
  expect(
    await page.evaluate(() => (window as any).fixture.clipboard()),
  ).toContain("const independent = true;");
  await dialog.getByRole("button", { name: "Edit side question" }).click();
  await expect(question).toBeFocused();
  await expect(question).toHaveValue(
    "Explain this independently α\nSecond line",
  );
  await expect(input).toHaveCount(0);
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(input).toHaveValue("Keep my main draft α");
  await expect(input).toBeFocused();
  await expect(attachedContext(page)).toContainText("README.md");
  await expect(
    page.getByRole("button", { name: "Stop generation", exact: true }),
  ).toBeVisible();
  expect(await page.locator(".transcript-content").innerText()).toBe(
    transcript.replace(
      "Continuing the main task.",
      "Continuing the main task. Still progressing.",
    ),
  );
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter((call: any) =>
      [
        "opencode_generate",
        "opencode_send_prompt",
        "opencode_enqueue_prompt",
        "opencode_run_command",
        "opencode_interrupt",
        "opencode_cancel",
        "opencode_select_agent",
      ].includes(call.command),
    ),
  );
  expect(calls).toEqual([
    {
      command: "opencode_generate",
      args: {
        projectPath: "/work/caelum",
        sessionId: "ses-a",
        prompt: "Explain this independently α\nSecond line",
      },
    },
  ]);
  await input.fill("/btw");
  await input.press("Enter");
  await expect(question).toHaveValue("");
  await expect(dialog.getByRole("region", { name: "Side answer" })).toHaveCount(
    0,
  );
});

test("slash btw asks once, retries without losing the question and ignores late replies after navigation", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "opencode_generate",
      "Temporarily unavailable",
    ),
  );
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/btw What does this mean?");
  await input.press("Enter");
  const dialog = page.getByRole("region", {
    name: "Side question",
    exact: true,
  });
  const question = dialog.getByRole("textbox", {
    name: "Question",
    exact: true,
  });
  await expect(dialog.getByRole("alert")).toContainText(
    "Temporarily unavailable",
  );
  await expect(dialog.locator(".side-question-prompt")).toHaveText(
    "What does this mean?",
  );
  const calls = () =>
    page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "opencode_generate",
      ),
    );
  expect(await calls()).toHaveLength(1);
  await page.evaluate(() => {
    (window as any).fixture.setCommandError("opencode_generate", null);
    (window as any).fixture.hold("opencode_generate");
    (window as any).fixture.setGenerateReply("Old delayed answer");
  });
  await dialog
    .getByRole("button", { name: "Retry side question" })
    .click({ clickCount: 2 });
  await expect(dialog.getByRole("status")).toContainText(
    "Answering independently",
  );
  expect(await calls()).toHaveLength(2);
  await expect(
    dialog.getByRole("button", { name: "Edit side question" }),
  ).toBeDisabled();
  await dialog.focus();
  await page.keyboard.press("Escape");
  await input.fill("Main draft after dismissal");
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await input.fill("Other session draft");
  await page.evaluate(() =>
    (window as any).fixture.release("opencode_generate"),
  );
  await expect(input).toHaveValue("Other session draft");
  await expect(
    page.getByText("Old delayed answer", { exact: true }),
  ).toHaveCount(0);
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(input).toHaveValue("Main draft after dismissal");
  await page.evaluate(() =>
    (window as any).fixture.setGenerateReply("Fresh answer"),
  );
  await input.fill("/btw");
  await input.press("Enter");
  await expect(question).toHaveValue("");
  await question.fill("Fresh question");
  await question.press("Enter");
  await expect(
    dialog.getByRole("region", { name: "Side answer" }),
  ).toContainText("Fresh answer");
  await expect(dialog).not.toContainText("Old delayed answer");
});

test("btw requires an existing OpenCode session and never falls through to a main prompt", async ({
  page,
}) => {
  await desktop(page, null);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  for (const provider of ["Codex", "OpenCode"]) {
    if (provider === "OpenCode") await choose(page, "Provider", provider);
    await input.fill("/btw Temporary question");
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText(
      "Open an OpenCode session",
    );
    await expect(input).toHaveValue("/btw Temporary question");
  }
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.command.endsWith("_send_prompt") ||
          call.command.endsWith("_start_session") ||
          call.command === "opencode_generate",
      ),
    ),
  ).toEqual([]);
});
