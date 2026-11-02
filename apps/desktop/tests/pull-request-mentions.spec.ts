import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  attachedContext,
  choose,
  emit,
  previewPullRequest,
} from "./fixtures/helpers.js";
import { splitPromptContext } from "../src/domain/composer/promptContext.js";

const pr = {
  ...previewPullRequest,
  number: 61,
  title: "Side questions",
  url: "https://github.com/example/caelum/pull/61",
  headRefName: "feat/session-ui-redesign",
  baseRefName: "feat/timeline",
  statusCheckRollup: [
    { ...previewPullRequest.statusCheckRollup[0], conclusion: "FAILURE" },
  ],
};
const ancestor = {
  ...pr,
  number: 60,
  title: "Timeline navigation",
  url: "https://github.com/example/caelum/pull/60",
  headRefName: "feat/timeline",
  baseRefName: "trunk",
};
const attached = (page: Page) =>
  page.locator('.composer [aria-label="Attached pull requests"]');
const inputFor = (page: Page) =>
  page.getByRole("textbox", { name: "Message", exact: true });

async function prepare(page: Page, provider = "codex") {
  await page.clock.install();
  await desktop(page, null);
  if (provider === "opencode") await choose(page, "Provider", "OpenCode");
  await page.evaluate(
    ({ pr, ancestor }) => {
      const fixture = (window as any).fixture;
      fixture.setPullRequest("feat/session-ui-redesign", pr);
      fixture.setPullRequest("main", pr);
      fixture.setPullRequest("feat/timeline", ancestor);
      fixture.setPullRequest("fix/provider-adapters", {
        ...pr,
        number: 77,
        title: "Unrelated session",
        url: "https://github.com/example/caelum/pull/77",
        baseRefName: "trunk",
      });
      fixture.setPullRequestDetails(pr.url, pr);
      fixture.setPullRequestDetails(ancestor.url, ancestor);
    },
    { pr, ancestor },
  );
  await page.clock.runFor(60_100);
  await page
    .getByRole("button", {
      name:
        provider === "codex"
          ? "Refine the session workspace"
          : "OpenCode session",
      exact: true,
    })
    .click();
  await expect(inputFor(page)).toBeEnabled();
}

for (const provider of ["codex", "opencode"]) {
  test(`${provider} mentions attach session stack snapshots, survive failed sends and render clean history`, async ({
    page,
  }) => {
    await prepare(page, provider);
    const input = inputFor(page);
    await input.fill("@README");
    await page.getByRole("option", { name: "README.md", exact: true }).click();
    await input.fill("Fix the failing tests in #Side carefully");
    for (let i = 0; i < 10; i++) await input.press("ArrowLeft");
    const list = page.getByRole("listbox", { name: "Pull request mentions" });
    await expect(list.getByRole("option")).toHaveText("#61Side questions");
    await input.press("Tab");
    await expect(input).toHaveValue("Fix the failing tests in #61 carefully");
    expect(
      await input.evaluate((node: HTMLTextAreaElement) => node.selectionStart),
    ).toBe("Fix the failing tests in #61 ".length);
    await expect(attached(page)).toContainText("#61 · Side questions");
    await expect(list).toHaveCount(0);
    // Reattaching deduplicates by repository URL and refreshes the snapshot.
    await input.fill("Fix #61");
    await input.press("Enter");
    await expect(input).toHaveValue("Fix #61 ");
    await expect(attached(page).locator(".pr-context-chip")).toHaveCount(1);
    await input.fill("Fix the failing tests in #61");
    await input.press("Escape");
    await page.evaluate(
      ({ provider, pr }) => {
        const fixture = (window as any).fixture;
        fixture.setPullRequestDetails(pr.url, {
          ...pr,
          headRefOid: "changed-after-selection",
        });
        fixture.hold(`${provider}_send_prompt`);
        fixture.failSend();
      },
      { provider, pr },
    );
    await input.press("Enter");
    await expect(attached(page)).toHaveCount(0);
    await input.fill("Newer draft");
    await page.evaluate(
      (provider) => (window as any).fixture.release(`${provider}_send_prompt`),
      provider,
    );
    await expect(input).toHaveValue(
      "Fix the failing tests in #61\n\nNewer draft",
    );
    await expect(attached(page)).toContainText("#61");
    await expect(attachedContext(page)).toContainText("README.md");
    await input.press("Enter");
    await expect(input).toHaveValue("");
    const sent = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === `${provider}_send_prompt`)
          .map((call: any) => call.args.prompt),
      provider,
    );
    expect(sent).toHaveLength(2);
    const context = splitPromptContext(sent[1]);
    expect(context.text).toBe("Fix the failing tests in #61\n\nNewer draft");
    expect(context.files[0].path).toBe("README.md");
    expect(context.pullRequests).toHaveLength(1);
    expect(context.pullRequests[0]).toMatchObject({
      number: 61,
      url: pr.url,
      head: pr.headRefName,
      base: pr.baseRefName,
      commit: pr.headRefOid,
      checks: [
        {
          name: "Tests",
          status: "Failed",
          url: previewPullRequest.statusCheckRollup[0].detailsUrl,
        },
      ],
    });
    const transcript = page.locator(".transcript-content");
    await expect(
      transcript.locator('[aria-label="Sent pull request context"]'),
    ).toContainText("#61");
    await expect(transcript).not.toContainText("<caelum-pull-request-context>");
  });

  test(`${provider} queued PR context stays attached through delivery`, async ({
    page,
  }) => {
    await prepare(page, provider);
    const input = inputFor(page);
    if (provider === "opencode") {
      await page.evaluate(() => (window as any).fixture.setOpenWorking(true));
      await emit(
        page,
        "session.execution.started",
        { sessionID: "ses-a" },
        "opencode",
      );
    } else await emit(page, "turn/started", { threadId: "thread-a" });
    await input.fill("Fix #61");
    await input.press("Enter");
    await expect(input).toHaveValue("Fix #61 ");
    await input.press("Enter");
    const queue = page.getByRole("region", { name: "Queued messages" });
    await expect(
      queue.locator('[aria-label="Sent pull request context"]'),
    ).toContainText("#61");
    await expect(queue).not.toContainText("caelum-pull-request-context");
    await expect(attached(page)).toHaveCount(0);
    if (provider === "opencode") {
      await page.reload();
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
      await expect(
        queue.locator('[aria-label="Sent pull request context"]'),
      ).toContainText("#61");
      await page.evaluate(() => (window as any).fixture.deliverQueued());
    } else
      await emit(page, "turn/completed", {
        threadId: "thread-a",
        turn: { id: "done", status: "completed" },
      });
    await expect(queue).toHaveCount(0);
    await expect(
      page.locator(
        '.transcript-content [aria-label="Sent pull request context"]',
      ),
    ).toContainText("#61");
    await expect(
      page.locator(".user-text").filter({ hasText: "Fix #61" }),
    ).toHaveCount(1);
  });
}

test("PR completion isolates sessions and repositories and ignores dismissed or stale detail reads", async ({
  page,
}) => {
  await prepare(page);
  const input = inputFor(page);
  const list = page.getByRole("listbox", { name: "Pull request mentions" });
  await input.fill("Inspect #");
  await expect(list.getByRole("option")).toHaveCount(2);
  await expect(list).not.toContainText("Unrelated");
  await input.press("ArrowDown");
  await input.press("Tab");
  await expect(input).toHaveValue("Inspect #60 ");
  await attached(page)
    .getByRole("button", { name: "Remove #60 context" })
    .click();
  await expect(attached(page)).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      "GitHub offline",
    ),
  );
  await input.fill("Inspect #61");
  await input.press("Enter");
  await expect(page.locator(".slash-note[role=alert]")).toContainText(
    "GitHub offline",
  );
  await expect(input).toHaveValue("Inspect #61");
  await page.evaluate(() => {
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      null,
    );
    (window as any).fixture.hold("github_pull_request_details");
  });
  await page.getByRole("button", { name: "Retry pull requests" }).click();
  await expect(
    page.getByRole("button", { name: "Send message", exact: true }),
  ).toBeDisabled();
  await input.fill("New draft");
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  await expect(input).toHaveValue("New draft");
  await expect(attached(page)).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_details"),
  );
  await input.fill("Inspect #61");
  await input.press("Enter");
  await expect(
    page.getByRole("status").filter({ hasText: "Attaching PR" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  await input.fill("#");
  await expect(list.getByRole("option")).toHaveCount(1);
  await expect(list).toContainText("#77");
  await expect(attached(page)).toHaveCount(0);
  await page.evaluate((pr) => {
    const other = {
      ...pr,
      url: "https://github.com/example/other/pull/61",
      title: "Other repository",
      baseRefName: "trunk",
    };
    (window as any).fixture.setPullRequest(
      "other-project",
      other,
      "/work/other",
    );
    (window as any).fixture.setPullRequestDetails(other.url, other);
  }, pr);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await input.fill("Inspect #61");
  await expect(list.getByRole("option")).toHaveText("#61Other repository");
  await input.press("Enter");
  await expect(attached(page).getByRole("link")).toHaveAttribute(
    "href",
    "https://github.com/example/other/pull/61",
  );
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(attached(page)).toHaveCount(0);
  await expect(input).toHaveValue("Inspect #61");
});

test("btw PR mentions use independent context, retry the captured snapshot and restore main attachments", async ({
  page,
}) => {
  await prepare(page, "opencode");
  const input = inputFor(page);
  await input.fill("@README");
  await page.getByRole("option", { name: "README.md", exact: true }).click();
  await input.fill("Main task #61");
  await input.press("Enter");
  await expect(input).toHaveValue("Main task #61 ");
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
  await expect(attached(page)).toHaveCount(0);
  await expect(attachedContext(page)).toHaveCount(0);
  await question.fill("Why is #Timeline");
  await question.press("Enter");
  await expect(question).toHaveValue("Why is #60 ");
  await question.press("End");
  await question.press("Shift+Enter");
  await question.press("End");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("opencode_generate", "Retry later"),
  );
  await question.press("Enter");
  await expect(panel.getByRole("alert")).toContainText("Retry later");
  await page.evaluate((pr) => {
    (window as any).fixture.setCommandError("opencode_generate", null);
    (window as any).fixture.setPullRequestDetails(pr.url, {
      ...pr,
      headRefOid: "new-head",
    });
  }, ancestor);
  await panel.getByRole("button", { name: "Retry side question" }).click();
  await expect(
    panel.getByRole("region", { name: "Side answer" }),
  ).toContainText("A temporary answer.");
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter((call: any) =>
      [
        "opencode_generate",
        "opencode_send_prompt",
        "opencode_enqueue_prompt",
        "opencode_select_agent",
      ].includes(call.command),
    ),
  );
  expect(calls.map((call: any) => call.command)).toEqual([
    "opencode_generate",
    "opencode_generate",
  ]);
  expect(calls[0].args.prompt).toBe(calls[1].args.prompt);
  const context = splitPromptContext(calls[0].args.prompt);
  expect(context.files).toEqual([]);
  expect(context.pullRequests).toHaveLength(1);
  expect(context.pullRequests[0]).toMatchObject({
    number: 60,
    commit: ancestor.headRefOid,
  });
  await panel.getByRole("button", { name: "Back to message" }).click();
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("Main task #61 ");
  await expect(attached(page)).toContainText("#61");
  await expect(attached(page)).not.toContainText("#60");
  await expect(attachedContext(page)).toContainText("README.md");
  await input.fill("/btw");
  await input.press("Enter");
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_details"),
  );
  await question.fill("Inspect #60");
  await question.press("Enter");
  await expect(panel.getByRole("status")).toContainText("Attaching PR");
  await panel.getByRole("button", { name: "Back to message" }).click();
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  await expect(input).toHaveValue("Main task #61 ");
  await expect(attached(page)).not.toContainText("#60");
  // Mentions in slash arguments are also side-only, before takeover starts.
  await input.fill("/btw Explain #Timeline");
  await input.press("Enter");
  await expect(input).toHaveValue("/btw Explain #60 ");
  await expect(attached(page)).toContainText("#60");
  await input.press("Enter");
  await expect(
    panel.getByRole("region", { name: "Side answer" }),
  ).toBeVisible();
  const inline = await page.evaluate(
    () =>
      (window as any).fixture.calls
        .filter((call: any) => call.command === "opencode_generate")
        .at(-1).args.prompt,
  );
  expect(splitPromptContext(inline).pullRequests[0]).toMatchObject({
    number: 60,
    commit: "new-head",
  });
  await panel.getByRole("button", { name: "Back to message" }).click();
  await expect(input).toHaveValue("Main task #61 ");
  await expect(attached(page)).toContainText("#61");
  await expect(attached(page)).not.toContainText("#60");
});
