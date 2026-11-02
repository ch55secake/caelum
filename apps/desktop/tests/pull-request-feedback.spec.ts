import { expect, test } from "@playwright/test";
import {
  emit,
  showPullRequests,
  findProjectFile,
  attachedContext,
  previewPullRequest,
  preparePullRequestReview,
} from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"] as const) {
  test(`PR comments send selected complete feedback to the current ${provider} session and preserve its draft`, async ({
    page,
  }) => {
    const screen = await preparePullRequestReview(page, provider);
    await screen
      .getByRole("button", { name: "Back to session", exact: true })
      .click();
    await findProjectFile(page, "src/App.tsx");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    await page
      .getByRole("textbox", { name: "Message", exact: true })
      .fill("Keep my next task draft.");
    await (await showPullRequests(page))
      .getByRole("button", { name: /View pull request #44/ })
      .click();
    await screen
      .getByRole("tab", { name: "Files changed (3)", exact: true })
      .click();
    await expect(
      screen.getByRole("region", { name: "Reviews", exact: true }),
    ).toContainText("changes requested");
    await expect(
      screen.getByRole("checkbox", {
        name: "Include resolved.ts:3",
        exact: true,
        includeHidden: true,
      }),
    ).not.toBeChecked();
    await expect(
      screen.getByRole("checkbox", {
        name: "Include outdated.ts:3",
        exact: true,
      }),
    ).not.toBeChecked();
    const inline = screen.getByRole("checkbox", {
      name: "Include src/new name.ts:3",
      exact: true,
    });
    await expect(inline).toBeDisabled();
    await screen
      .getByRole("button", { name: "Load remaining replies", exact: true })
      .click();
    await expect(inline).toBeChecked();
    await expect(screen).toContainText("Unicode: 空白");
    await screen
      .getByRole("button", {
        name: "Load more discussion comments",
        exact: true,
      })
      .click();
    await expect(screen).toContainText("Also include an example");
    await screen
      .getByRole("region", { name: "Discussion", exact: true })
      .getByRole("checkbox")
      .first()
      .uncheck();
    await screen.getByRole("tab", { name: "Overview", exact: true }).click();
    await screen
      .getByRole("tab", { name: "Files changed (3)", exact: true })
      .click();
    await expect(
      screen
        .getByRole("region", { name: "Discussion", exact: true })
        .getByRole("checkbox")
        .first(),
    ).not.toBeChecked();
    await expect(inline).toBeChecked();
    const thread = screen.locator('.diff-annotation [data-thread-id="inline"]');
    await expect(thread).toContainText("Preserve the return value");
    await expect(thread.getByText("Diff context", { exact: true })).toHaveCount(
      0,
    );
    await screen
      .getByRole("button", { name: "Address selected comments", exact: true })
      .click();
    await expect(screen).toHaveCount(0);
    await expect(
      page.getByRole("textbox", { name: "Message", exact: true }),
    ).toHaveValue("Keep my next task draft.");
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    const calls = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls.filter(
          (call: any) => call.command === `${provider}_send_prompt`,
        ),
      provider,
    );
    expect(calls).toHaveLength(1);
    expect(calls[0].args.sessionId).toBe(
      provider === "codex" ? "thread-a" : "ses-a",
    );
    expect(calls[0].args.projectPath).toBe("/work/caelum");
    const prompt = calls[0].args.prompt;
    expect(prompt).toContain(previewPullRequest.url);
    expect(prompt).toContain(previewPullRequest.headRefOid);
    expect(prompt).toContain("Unicode: 空白");
    expect(prompt).toContain("src/new name.ts");
    expect(prompt).toContain("Also include an example");
    for (const absent of [
      "Already addressed",
      "Old implementation",
      "Please update the documentation",
      "Keep my next task draft",
      "caelum-file-context",
    ])
      expect(prompt).not.toContain(absent);
  });
}

test("PR comments require an idle existing session, retain failed sends and respect newer navigation", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page, "codex", false);
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  await expect(screen).toContainText("Select a session to address feedback");
  await expect(
    screen.getByRole("button", {
      name: "Address selected comments",
      exact: true,
    }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await page
    .getByRole("textbox", { name: "Message", exact: true })
    .fill("Untouched draft");
  await (await showPullRequests(page))
    .getByRole("button", { name: /View pull request #44/ })
    .click();
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "busy" },
  });
  await expect(
    screen.getByRole("button", {
      name: "Address selected comments",
      exact: true,
    }),
  ).toBeDisabled();
  await expect(screen).toContainText("Wait for the current turn");
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "busy", status: "completed" },
  });
  await page.evaluate(() => (window as any).fixture.failSend());
  await screen
    .getByRole("button", { name: "Address selected comments", exact: true })
    .click();
  await expect(screen.getByRole("alert")).toContainText(
    "Could not send review feedback",
  );
  await page.evaluate(() => (window as any).fixture.hold("codex_send_prompt"));
  await screen
    .getByRole("button", { name: "Address selected comments", exact: true })
    .click();
  await expect(
    screen.getByRole("button", { name: "Sending…", exact: true }),
  ).toBeDisabled();
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.release("codex_send_prompt"),
  );
  await expect(page.locator(".breadcrumbs strong")).toHaveText(
    "Review the provider adapters",
  );
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toHaveValue("Untouched draft");
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.filter(
          (call: any) => call.command === "codex_send_prompt",
        ).length,
    ),
  ).toBe(2);
});

test("PR comments revalidate cached replies and require every reply page after refresh failures", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  const inline = screen.getByRole("checkbox", {
    name: "Include src/new name.ts:3",
    exact: true,
  });
  await screen
    .getByRole("button", { name: "Load remaining replies", exact: true })
    .click();
  await expect(inline).toBeChecked();
  await screen
    .getByRole("button", { name: "Refresh comments", exact: true })
    .click();
  await expect(
    screen.getByRole("button", { name: "Refresh comments", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() => {
    (window as any).fixture.hold("github_pull_request_feedback");
    (window as any).fixture.setCommandError(
      "github_pull_request_feedback",
      "Replies unavailable",
    );
  });
  await screen
    .getByRole("button", { name: "Load remaining replies", exact: true })
    .click();
  await expect(
    screen.getByText("Loading replies…", { exact: true }),
  ).toBeVisible();
  await expect(inline).toBeDisabled();
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_feedback"),
  );
  await expect(screen.getByRole("alert")).toContainText("Replies unavailable");
  await expect(inline).toBeDisabled();
  await page.evaluate((url) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_pull_request_feedback", null);
    const comment = (id: string, body: string) => ({
      id,
      body,
      url: `${url}#discussion_${id}`,
      author: null,
      createdAt: "2026-11-01T12:00:00Z",
      state: null,
      diffHunk: null,
    });
    fixture.setPullRequestPage(url, "replies", "replies-next", {
      comments: [comment("reply", "Updated reply")],
      threads: [],
      nextCursor: "replies-last",
    });
    fixture.setPullRequestPage(url, "replies", "replies-last", {
      comments: [comment("last-reply", "Last reply on the thread")],
      threads: [],
      nextCursor: null,
    });
  }, previewPullRequest.url);
  await screen
    .getByRole("button", { name: "Retry replies", exact: true })
    .click();
  await expect(screen).toContainText("Updated reply");
  await expect(inline).toBeDisabled();
  await screen
    .getByRole("button", { name: "Load more replies", exact: true })
    .click();
  await expect(inline).toBeChecked();
  await expect(screen).toContainText("Last reply on the thread");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_feedback",
      "Refresh unavailable",
    ),
  );
  await screen
    .getByRole("button", { name: "Refresh comments", exact: true })
    .click();
  await expect(screen.getByRole("alert")).toHaveCount(3);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_feedback",
      null,
    ),
  );
  await screen
    .getByRole("button", { name: "Load remaining replies", exact: true })
    .click();
  await screen
    .getByRole("button", { name: "Load more replies", exact: true })
    .click();
  await expect(inline).toBeChecked();
  await screen
    .getByRole("button", { name: "Retry review threads", exact: true })
    .click();
  await expect(
    screen.getByRole("button", { name: "Load remaining replies", exact: true }),
  ).toBeVisible();
  await expect(inline).toBeDisabled();
});

test("PR comments retain refresh failures, isolate delayed responses and reload for a new head", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  await expect(screen).toContainText("Please update the documentation");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_feedback",
      "Rate limited",
    ),
  );
  await screen
    .getByRole("button", { name: "Refresh comments", exact: true })
    .click();
  await expect(screen.getByRole("alert")).toHaveCount(3);
  await expect(screen).toContainText("Please update the documentation");
  await expect(
    screen.getByRole("button", {
      name: "Address selected comments",
      exact: true,
    }),
  ).toBeDisabled();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_feedback",
      null,
    ),
  );
  await screen
    .getByRole("button", { name: "Refresh comments", exact: true })
    .click();
  await expect(screen.getByRole("alert")).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_feedback"),
  );
  await screen
    .getByRole("button", { name: "Refresh comments", exact: true })
    .click();
  await page.evaluate((pr) => {
    (window as any).fixture.setPullRequestDetails(pr.url, {
      ...pr,
      headRefOid: "f".repeat(40),
    });
    (window as any).fixture.setPullRequestPage(pr.url, "discussion", "", {
      comments: [
        {
          id: "new-head-comment",
          body: "Feedback on the new head",
          url: `${pr.url}#issuecomment-1`,
          author: null,
          state: null,
          createdAt: "2026-11-01T12:00:00Z",
          diffHunk: null,
        },
      ],
      threads: [],
      nextCursor: null,
    });
  }, previewPullRequest);
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    screen.getByText("Please update the documentation.", { exact: true }),
  ).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_feedback"),
  );
  await expect(screen).toContainText("Feedback on the new head");
  await expect(screen).not.toContainText("Please update the documentation");
  await screen
    .getByRole("button", { name: "Address selected comments", exact: true })
    .click();
  const prompt = await page.evaluate(
    () =>
      (window as any).fixture.calls.findLast(
        (call: any) => call.command === "codex_send_prompt",
      ).args.prompt,
  );
  expect(prompt).toContain("f".repeat(40));
  expect(prompt).toContain("Feedback on the new head");
  expect(prompt).not.toContain("Please update the documentation");
});
