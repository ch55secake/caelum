import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose, question, setQuestions } from "./fixtures/helpers.js";

test("Codex questions preserve drafts across sessions and retry the original request", async ({
  page,
  browserName,
}) => {
  await desktop(page);
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Next message draft");
  await setQuestions(page, [question()]);
  const card = page.getByRole("form", { name: "Implementation choices" });
  await expect(card).toBeVisible();
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveCount(0);
  await expect(card).toBeFocused();
  await expect(card.locator("input:checked")).toHaveCount(0);
  // macOS WebKit uses Option+Tab to include radio buttons in tab navigation.
  await page.keyboard.press(
    browserName === "webkit" && process.platform === "darwin"
      ? "Alt+Tab"
      : "Tab",
  );
  await expect(
    card.getByRole("radio", { name: "Small Focused change" }),
  ).toBeFocused();
  await expect(page.locator(".connection-status")).toHaveText(
    "Waiting for your answer",
  );
  await card.getByRole("button", { name: "Submit answers" }).click();
  await expect(
    card.getByRole("radio", { name: "Small Focused change" }),
  ).toBeFocused();
  await page.keyboard.press("Space");
  await page.keyboard.press("ArrowDown");
  await expect(
    card.getByRole("radio", { name: "Large Full rewrite" }),
  ).toBeChecked();
  const customAnswer = card.getByRole("textbox", { name: "Approach" });
  await customAnswer.click();
  await expect(
    card.getByRole("radio", { name: "Type your own answer" }),
  ).toBeChecked();
  await customAnswer.pressSequentially("Incremental refactor");
  const singleLineHeight = (await customAnswer.boundingBox())!.height;
  await customAnswer.press("Shift+Enter");
  await customAnswer.pressSequentially("Preserve the existing API.");
  await expect(customAnswer).toHaveValue(
    "Incremental refactor\nPreserve the existing API.",
  );
  expect((await customAnswer.boundingBox())!.height).toBeGreaterThan(
    singleLineHeight,
  );
  await customAnswer.fill("Incremental refactor");
  expect((await customAnswer.boundingBox())!.height).toBe(singleLineHeight);
  await card.getByRole("radio", { name: "Small Focused change" }).check();
  await expect(customAnswer).toHaveValue("Incremental refactor");
  await card.getByRole("radio", { name: "Type your own answer" }).check();
  await expect(customAnswer).toBeFocused();
  await card
    .getByRole("textbox", { name: "Details" })
    .fill("Keep compatibility");
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await expect(card).toHaveCount(0);
  await expect(message).toBeVisible();
  await page.getByRole("button", { name: "1 pending questions" }).click();
  await expect(message).toHaveCount(0);
  await expect(card.getByRole("textbox", { name: "Details" })).toHaveValue(
    "Keep compatibility",
  );
  await expect(card.getByRole("textbox", { name: "Approach" })).toHaveValue(
    "Incremental refactor",
  );
  await customAnswer.focus();
  await customAnswer.dispatchEvent("keydown", {
    key: "Enter",
    isComposing: true,
  });
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "resolve_question",
      ),
    ),
  ).toHaveLength(0);
  await page.evaluate(() => {
    (window as any).fixture.setCommandError(
      "resolve_question",
      "Connection unavailable",
    );
    (window as any).fixture.hold("resolve_question");
  });
  await customAnswer.press("Enter");
  await expect(card.getByRole("button", { name: "Sending…" })).toBeDisabled();
  await expect(card.getByRole("textbox", { name: "Details" })).toBeDisabled();
  await page.evaluate(() =>
    (window as any).fixture.release("resolve_question"),
  );
  await expect(card.getByRole("alert")).toContainText("Connection unavailable");
  await expect(message).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("resolve_question", null),
  );
  await card.getByRole("textbox", { name: "Details" }).press("Enter");
  await expect(card).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toHaveValue("Next message draft");
  await expect(message).toBeFocused();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "resolve_question",
    ),
  );
  expect(calls).toHaveLength(2);
  expect(calls[1].args).toEqual({
    provider: "codex",
    projectPath: "/work/caelum",
    sessionId: "thread-a",
    requestId: "41",
    answer: { approach: "Incremental refactor", detail: "Keep compatibility" },
  });
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "codex_send_prompt",
      ),
    ),
  ).toHaveLength(0);
  await setQuestions(page, [question()]);
  await card.getByRole("button", { name: "Cancel questions" }).press("Enter");
  await expect(message).toHaveValue("Next message draft");
  await expect(message).toBeFocused();
});

test("OpenCode questions submit multiple and custom answers with typed conditional fields", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await setQuestions(page, [
    question({
      provider: "opencode",
      sessionId: "ses-a",
      requestId: "frm_1",
      fields: [
        {
          key: "q0",
          type: "multiselect",
          title: "Targets",
          required: true,
          custom: true,
          options: [
            { value: "Mac", label: "macOS" },
            { value: "Linux", label: "Linux" },
          ],
        },
        {
          key: "enabled",
          type: "boolean",
          title: "Enable rollout",
          required: true,
          default: false,
        },
        {
          key: "count",
          type: "integer",
          title: "Batch size",
          required: true,
          minimum: 1,
          default: 2,
          when: [{ key: "enabled", op: "eq", value: true }],
        },
        { key: "metadata", type: "string", hidden: true, default: "preserved" },
      ],
    }),
  ]);
  const card = page.getByRole("form", { name: "Implementation choices" });
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveCount(0);
  await card.getByRole("checkbox", { name: "macOS" }).check();
  await card.getByRole("checkbox", { name: "Linux" }).focus();
  await page.keyboard.press("Space");
  const customAnswers = card.getByRole("textbox", {
    name: "Targets: custom answers",
  });
  await customAnswers.fill("Windows");
  await customAnswers.press("Shift+Enter");
  await customAnswers.pressSequentially("FreeBSD");
  await expect(
    card.getByRole("spinbutton", { name: "Batch size" }),
  ).toHaveCount(0);
  await card.getByRole("radio", { name: "Yes", exact: true }).check();
  await card.getByRole("spinbutton", { name: "Batch size" }).fill("3");
  await customAnswers.press("Enter");
  await expect(card).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === "resolve_question")
          .at(-1).args,
    ),
  ).toEqual({
    provider: "opencode",
    projectPath: "/work/caelum",
    sessionId: "ses-a",
    requestId: "frm_1",
    answer: {
      q0: ["Mac", "Linux", "Windows", "FreeBSD"],
      enabled: true,
      count: 3,
      metadata: "preserved",
    },
  });
});

test("pending questions recover on reconnect and stay scoped across projects and providers", async ({
  page,
}) => {
  const first = question();
  const second = question({ provider: "opencode", sessionId: "ses-a" });
  const third = question({ projectPath: "/work/other", sessionId: "thread-a" });
  await desktop(page, "thread-a", { initialQuestions: [first, second, third] });
  const card = page.getByRole("form", { name: "Implementation choices" });
  await expect(card).toHaveCount(1);
  await card.getByRole("button", { name: "Cancel questions" }).click();
  await expect(card).toHaveCount(0);
  await page.getByRole("button", { name: "2 pending questions" }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("OpenCode session");
  await expect(card).toHaveCount(1);
  await setQuestions(page, [third]); // answered by another client
  await expect(card).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  await emit(page, "server.disconnected", {}, "opencode");
  await setQuestions(page, [second, third], false); // form predates the new connection
  await page.getByRole("button", { name: "Reconnect", exact: true }).click();
  await expect(card).toHaveCount(1);
  await page.getByRole("button", { name: "2 pending questions" }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await card.getByRole("button", { name: "Cancel questions" }).click();
  const replies = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "resolve_question",
    ),
  );
  expect(replies.map((call: any) => call.args)).toEqual([
    {
      provider: "codex",
      projectPath: "/work/caelum",
      sessionId: "thread-a",
      requestId: "41",
      answer: null,
    },
    {
      provider: "codex",
      projectPath: "/work/other",
      sessionId: "thread-a",
      requestId: "41",
      answer: null,
    },
  ]);
});

test("a stale pending-question snapshot cannot resurrect an externally cancelled question", async ({
  page,
}) => {
  await desktop(page, "thread-a", {
    initialQuestions: [question()],
    holdQuestions: true,
  });
  await setQuestions(page, []);
  await page.evaluate(() =>
    (window as any).fixture.release("pending_questions"),
  );
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(page.locator(".question-card")).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "1 pending questions" }),
  ).toHaveCount(0);
});
