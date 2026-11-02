import { expect, test } from "@playwright/test";
import {
  emit,
  showPullRequests,
  sessionRow,
  pickMenu,
  previewPullRequest,
  preparePullRequestPreview,
} from "./fixtures/helpers.js";

test("PR previews load on demand, stay open across the hover gap, share cached details and open the PR screen", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page);
  await expect(page.locator(".project-row .branch-pr-trigger")).toHaveCount(0);
  await expect(page.locator(".worktree-row .branch-pr-trigger")).toHaveCount(0);
  const detailCalls = () =>
    page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_pull_request_details",
      ),
    );
  expect(await detailCalls()).toHaveLength(0);
  await trigger.hover();
  await page.getByRole("textbox", { name: "Message", exact: true }).hover();
  await page.clock.runFor(300);
  expect(await detailCalls()).toHaveLength(0);
  await trigger.hover();
  await page.clock.runFor(300);
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(
    card.getByRole("heading", { name: previewPullRequest.title, exact: true }),
  ).toBeVisible();
  await expect(card).toContainText("clearer workspace");
  await expect(card).not.toContainText("template note");
  await expect(card).toContainText("+128");
  await expect(card).toContainText("−42");
  await expect(card).toContainText("6 files changed");
  await expect(card).toContainText("1 passed · 1 pending");
  await expect(card).toContainText("Approved");
  await expect(card.locator(".pr-ready-badge")).toHaveText("Ready to merge");
  await card.hover();
  await page.clock.runFor(500);
  await expect(card).toBeVisible();
  await page.screenshot({ path: "test-results/pr-preview.png" });
  await expect(page.locator(".breadcrumbs")).toContainText("New session");
  await card.getByRole("button", { name: "View pull request" }).click();
  const expanded = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(
    expanded.getByRole("heading", { name: "Details", exact: true }),
  ).toBeVisible();
  await expect(
    expanded.getByRole("region", { name: "Pull request checks" }),
  ).toContainText("Preview deploy");
  await expect(expanded.locator(".pr-ready-badge")).toBeVisible();
  await expect(expanded.locator(".pr-merge")).toHaveAttribute(
    "data-ready",
    "true",
  );
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeHidden();
  await page.screenshot({ path: "test-results/pr-overview.png" });
  await expanded.getByRole("link", { name: "GitHub", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe(previewPullRequest.url);
  await expanded.getByRole("button", { name: "Back to session" }).click();
  await expect(expanded).toHaveCount(0);
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeFocused();
  await expect(card).toHaveCount(0);
  const sessionTrigger = sessionRow(
    page,
    "Refine the session workspace",
  ).getByRole("button", { name: /pull request #44/ });
  await sessionTrigger.hover();
  await page.clock.runFor(300);
  await expect(card).toContainText("+128");
  expect(await detailCalls()).toHaveLength(1);
  await sessionTrigger.click();
  await expect(page.locator(".breadcrumbs")).toContainText("New session");
  await page.locator(".breadcrumbs").click();
  await expect(card).toHaveCount(0);
});

test("PR previews support keyboard access, retry failures and constrain long content in both themes", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page);
  await page.setViewportSize({ width: 800, height: 620 });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      "GitHub authentication required",
    ),
  );
  await trigger.focus();
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(card.getByRole("alert")).toContainText(
    "authentication required",
  );
  await trigger.press("Tab");
  await expect(
    card.getByRole("button", { name: "Retry", exact: true }),
  ).toBeFocused();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_pull_request_details", null);
    fixture.setPullRequestDetails(pr.url, {
      ...pr,
      state: "MERGED",
      title:
        "A pull request with a very long title that must stay inside the preview without pushing the layout sideways",
      headRefName: "feature/" + "long-branch-".repeat(15),
      body:
        "<!-- hidden -->\n" + "This is a readable description. ".repeat(150),
      reviewDecision: "CHANGES_REQUESTED",
      statusCheckRollup: [
        {
          __typename: "CheckRun",
          name: "Tests",
          status: "COMPLETED",
          conclusion: "FAILURE",
        },
      ],
    });
  }, previewPullRequest);
  await page.keyboard.press("Enter");
  await expect(card).toContainText("Merged");
  await expect(card.locator(".pr-ready-badge")).toHaveCount(0);
  await expect(card).toContainText("1 failed");
  await expect(card).toContainText("Changes requested");
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await expect(card).toBeInViewport({ ratio: 1 });
    expect(
      await card.evaluate((node) => node.scrollWidth <= node.clientWidth),
    ).toBe(true);
    await page.screenshot({ path: `test-results/pr-preview-${theme}.png` });
  }
  await page.keyboard.press("Escape");
  await expect(card).toHaveCount(0);
  await expect(trigger).toBeFocused();
  await page.clock.runFor(500);
  await expect(card).toHaveCount(0);
  await trigger.press("Enter");
  await trigger.press("Tab");
  await expect(
    card.getByRole("button", { name: "View pull request" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(screen.getByRole("heading", { level: 1 })).toBeFocused();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await expect(screen).toBeVisible();
  await screen.getByRole("button", { name: "Back to session" }).click();
  await trigger.focus();
  await trigger.press("Enter");
  await trigger.press("Tab");
  await page.keyboard.press("Shift+Tab");
  await expect(trigger).toBeFocused();
  await expect(card).toHaveCount(0);
  await trigger.press("Enter");
  await trigger.press("Tab");
  await page.keyboard.press("Tab");
  await expect(
    card.getByRole("link", { name: "GitHub", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Tab");
  await expect(
    sessionRow(page, "Refine the session workspace").getByRole("button", {
      name: "Archive session",
      exact: true,
    }),
  ).toBeFocused();
  await expect(card).toHaveCount(0);
});

test("PR previews discard delayed displays after navigation and isolate identical numbers across repositories", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page, false);
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_details"),
  );
  await trigger.hover();
  await page.clock.runFor(300);
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(card).toContainText("Loading pull request details");
  await expect(card.locator(".pr-ready-badge")).toHaveCount(0);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest(
      "other-project",
      {
        ...pr,
        url: "https://github.com/example/other/pull/44",
        title: "Other repository PR",
      },
      "/work/other",
    );
    fixture.setPullRequestDetails("https://github.com/example/other/pull/44", {
      ...pr,
      url: "https://github.com/example/other/pull/44",
      title: "Other repository PR",
      body: "Different description",
      additions: 9,
    });
  }, previewPullRequest);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(card).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  const other = sessionRow(page, "Refine the session workspace").getByRole(
    "button",
    { name: /pull request #44: Other repository PR/ },
  );
  await other.hover();
  await page.clock.runFor(300);
  await expect(card).toContainText("Different description");
  await expect(card).toContainText("+9");
  await expect(card).not.toContainText("clearer workspace");
  await page.evaluate(() => {
    (window as any).fixture.setPullRequest(
      "other-project",
      null,
      "/work/other",
    );
  });
  await page.clock.runFor(60_100);
  await expect(other).toHaveCount(0);
  await expect(card).toHaveCount(0);
});

test("session PR previews share details across linked worktrees without selecting a session", async ({
  page,
}) => {
  const trigger = await preparePullRequestPreview(page);
  await trigger.hover();
  await page.clock.runFor(300);
  const card = page.getByRole("dialog", {
    name: "Pull request preview",
    exact: true,
  });
  await expect(card).toContainText("+128");
  await page.keyboard.press("Escape");
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.addWorktree("/external/navigation", "feature/navigation");
    fixture.setPullRequest("feature/navigation", pr, "/external/navigation");
  }, previewPullRequest);
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Refresh worktrees");
  await expect(page.locator(".worktree-row .branch-pr-trigger")).toHaveCount(0);
  const checkout = page.getByRole("button", {
    name: "Switch to worktree feature/navigation",
    exact: true,
  });
  await checkout.click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Review navigation");
  await input.press("Enter");
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    const id = fixture.calls.find(
      (call: any) => call.command === "codex_send_prompt",
    ).args.threadId;
    fixture.setSessionDetails("codex", id, {
      branch: "feature/navigation",
      branchIsCurrent: false,
      changes: null,
    });
  });
  await page
    .getByRole("button", { name: "Refresh sessions", exact: true })
    .click();
  await page.getByRole("button", { name: /^New session/ }).click();
  const session = page
    .locator('.session-row[data-project-path="/external/navigation"]')
    .getByRole("button", { name: /pull request #44/ });
  await session.hover();
  await page.clock.runFor(300);
  await expect(card).toContainText("+128");
  await session.click();
  await expect(checkout).toHaveAttribute("aria-current", "page");
  await expect(page.locator(".breadcrumbs")).toContainText("New session");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_pull_request_details",
      ),
    ),
  ).toHaveLength(1);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(card).toHaveCount(0);
});

test("PR screen keeps the sidebar usable and preserves the conversation, draft and reading position", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  const session = page.getByRole("button", {
    name: "Refine the session workspace",
    exact: true,
  });
  await session.click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my session draft");
  await page
    .getByRole("button", { name: "Load earlier messages", exact: true })
    .click();
  const transcript = page.locator(".transcript");
  await transcript.evaluate((node) => {
    node.setAttribute("data-mounted", "yes");
    node.scrollTop = 250;
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  const entry = (await showPullRequests(page)).getByRole("button", {
    name: /View pull request #44/,
  });
  await entry.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(screen).toContainText("+128");
  await expect(screen.getByRole("heading", { level: 1 })).toBeFocused();
  await expect(entry).toHaveAttribute("aria-current", "page");
  await expect(page.locator(".session-row.selected")).toHaveCount(0);
  await expect(input).toBeHidden();
  await expect(transcript).toHaveAttribute("data-mounted", "yes");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "pr-background",
    delta: "Progress while reviewing the pull request",
  });
  await expect(screen.getByRole("heading", { level: 1 })).toBeFocused();
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await page.screenshot({ path: `test-results/pr-screen-${theme}.png` });
  }
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(screen).toBeInViewport({ ratio: 1 });
  expect(
    await screen.evaluate((node) => node.scrollWidth <= node.clientWidth),
  ).toBe(true);
  await page.screenshot({ path: "test-results/pr-screen-compact.png" });
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await expect(screen).toBeVisible();
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(entry).toHaveAttribute("aria-current", "page");
  await screen.getByRole("button", { name: "Back to session" }).click();
  await expect(screen).toHaveCount(0);
  await expect(input).toHaveValue("Keep my session draft");
  await expect(transcript).toHaveAttribute("data-mounted", "yes");
  await expect
    .poll(() => transcript.evaluate((node) => node.scrollTop))
    .toBe(250);
  await page
    .getByRole("button", { name: "Jump to latest", exact: true })
    .click();
  await expect(
    page.getByText("Progress while reviewing the pull request", {
      exact: true,
    }),
  ).toBeVisible();
  await entry.click();
  await session.click();
  await expect(screen).toHaveCount(0);
  await expect(input).toHaveValue("Keep my session draft");
  await showPullRequests(page);
  await entry.click();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(screen).toHaveCount(0);
  await expect(
    page.getByRole("complementary", { name: "Project files", exact: true }),
  ).toBeVisible();
  await entry.click();
  await page.getByRole("button", { name: "Changes", exact: true }).click();
  await expect(
    page.getByRole("complementary", { name: "Project changes", exact: true }),
  ).toBeVisible();
  await expect(screen).toHaveCount(0);
  await entry.click();
  await page.keyboard.press("Meta+n");
  await expect(screen).toHaveCount(0);
  await expect(input).toHaveValue("");
  await session.click();
  await expect(input).toHaveValue("Keep my session draft");
});

test("PR screen switches between requests, retries refresh failures and rejects stale results after navigation", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    const other = {
      ...pr,
      number: 45,
      title: "Improve provider adapters",
      url: "https://github.com/example/caelum/pull/45",
      body: "Independent provider work",
      additions: 9,
    };
    fixture.setPullRequest("fix/provider-adapters", other);
    fixture.setPullRequestDetails(other.url, other);
    fixture.hold(`github_pull_request_details:${pr.url}`);
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  const list = await showPullRequests(page);
  const first = list.getByRole("button", { name: /View pull request #44/ });
  const second = list.getByRole("button", { name: /View pull request #45/ });
  await first.click();
  await expect(
    page.getByRole("region", { name: "Pull request #44", exact: true }),
  ).toContainText("Loading pull request details");
  await second.click();
  const screen = page.getByRole("region", {
    name: "Pull request #45",
    exact: true,
  });
  await expect(screen).toContainText("Independent provider work");
  await expect(second).toHaveAttribute("aria-current", "page");
  await expect(first).not.toHaveAttribute("aria-current", "page");
  await page.evaluate(
    (pr) =>
      (window as any).fixture.release(`github_pull_request_details:${pr.url}`),
    previewPullRequest,
  );
  await expect(screen).not.toContainText("clearer workspace");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      "GitHub unavailable",
    ),
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(screen.getByRole("alert")).toContainText(
    "Showing the last loaded details",
  );
  await expect(screen).toContainText("+9");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      null,
    ),
  );
  await screen.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(screen.getByRole("alert")).toHaveCount(0);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(screen).toHaveCount(0);
  await second.click();
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_details"),
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(screen).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await expect(page.locator(".pr-screen")).toHaveCount(0);
});
