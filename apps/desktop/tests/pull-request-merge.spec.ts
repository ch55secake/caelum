import { expect, test } from "@playwright/test";
import {
  showPullRequests,
  sessionRow,
  previewPullRequest,
  preparePullRequestPreview,
} from "./fixtures/helpers.js";

test("PR merge is shared across navigation, pins the head and removes merged rows despite delayed discovery", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  const list = await showPullRequests(page);
  const first = list.getByRole("button", { name: /View pull request #44/ });
  await first.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(
    screen.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeEnabled();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    const other = {
      ...pr,
      number: 45,
      title: "Another pull request",
      url: "https://github.com/example/caelum/pull/45",
    };
    fixture.setPullRequest("fix/provider-adapters", other);
    fixture.setPullRequestDetails(other.url, other);
    fixture.hold(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    );
    fixture.hold("github_merge_pull_request");
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  const second = list.getByRole("button", { name: /View pull request #45/ });
  await expect(second).toBeVisible();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls.filter(
            (call: any) =>
              call.command === "github_open_pull_request" &&
              call.args.branch === "feat/session-ui-redesign",
          ).length,
      ),
    )
    .toBe(3);
  const method = screen.getByRole("button", {
    name: "Merge method",
    exact: true,
  });
  await method.press("ArrowDown");
  await expect(
    page.getByRole("option", { name: /^Create a merge commit/ }),
  ).toHaveAttribute("aria-selected", "true");
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(method).toBeFocused();
  await screen
    .getByRole("button", { name: "Squash and merge", exact: true })
    .click();
  await expect(
    screen.getByRole("button", { name: "Merging…", exact: true }),
  ).toBeDisabled();
  await expect(screen.locator(".pr-ready-badge")).toHaveCount(0);
  await expect(screen.locator(".pr-merge")).toHaveAttribute(
    "data-ready",
    "false",
  );
  await expect(
    screen
      .getByRole("button", { name: "Merging…", exact: true })
      .locator(".spinning"),
  ).toBeVisible();
  await expect(
    screen.getByRole("button", { name: "Refresh", exact: true }),
  ).toBeDisabled();
  await expect(
    screen.getByRole("button", { name: "Update branch", exact: true }),
  ).toBeDisabled();
  await screen.getByRole("button", { name: "Back to session" }).click();
  await first.click();
  await expect(
    screen.getByRole("button", { name: "Merge method", exact: true }),
  ).toHaveAttribute("title", "Merge method: Squash and merge");
  await expect(
    screen.getByRole("button", { name: "Merge method", exact: true }),
  ).toBeDisabled();
  await expect(
    screen.getByRole("button", { name: "Merging…", exact: true }),
  ).toBeDisabled();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "github_merge_pull_request",
    ),
  );
  expect(calls).toHaveLength(1);
  expect(calls[0].args).toEqual({
    projectPath: "/work/caelum",
    url: previewPullRequest.url,
    method: "squash",
    headOid: previewPullRequest.headRefOid,
  });
  await second.click();
  const otherScreen = page.getByRole("region", {
    name: "Pull request #45",
    exact: true,
  });
  await page.evaluate(() =>
    (window as any).fixture.release("github_merge_pull_request"),
  );
  await expect(first).toHaveCount(0);
  await expect(otherScreen).toBeVisible();
  await expect(
    otherScreen.getByRole("button", {
      name: "Merge pull request",
      exact: true,
    }),
  ).toBeEnabled();
  await page.evaluate(() =>
    (window as any).fixture.release(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    ),
  );
  await expect(
    sessionRow(page, "Refine the session workspace").locator(
      ".branch-pr-trigger",
    ),
  ).toHaveCount(0);
  await expect(first).toHaveCount(0);
  await expect(second).toHaveAttribute("aria-current", "page");
});

test("PR merge reports failures, reloads changed heads and confirms a successful retry", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  const entry = (await showPullRequests(page)).getByRole("button", {
    name: /View pull request #44/,
  });
  await entry.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const merge = screen.getByRole("region", {
    name: "Merge pull request",
    exact: true,
  });
  const button = merge.getByRole("button", {
    name: "Merge pull request",
    exact: true,
  });
  await expect(button).toBeEnabled();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_merge_pull_request",
      "Merge method is not allowed (HTTP 405)",
    ),
  );
  await button.click();
  await expect(merge.getByRole("alert")).toContainText(
    "Merge method is not allowed",
  );
  await expect(entry).toBeVisible();
  await expect(button).toBeEnabled();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_merge_pull_request", null);
    fixture.setPullRequestDetails(pr.url, {
      ...pr,
      headRefOid: "b".repeat(40),
    });
  }, previewPullRequest);
  await button.click();
  await expect(merge.getByRole("alert")).toContainText(
    "Head branch was modified",
  );
  await expect(button).toBeEnabled();
  await merge
    .getByRole("button", { name: "Merge method", exact: true })
    .click();
  await page.getByRole("option", { name: /^Rebase and merge/ }).click();
  await merge
    .getByRole("button", { name: "Rebase and merge", exact: true })
    .click();
  await expect(merge.getByRole("status")).toHaveText("Merged into main");
  await expect(merge.getByRole("alert")).toHaveCount(0);
  await expect(merge.getByRole("button")).toHaveCount(0);
  await expect(screen.locator(".pr-state")).toHaveText("Merged");
  await expect(screen.locator(".pr-ready-badge")).toHaveCount(0);
  await expect(merge).toHaveAttribute("data-ready", "false");
  await expect(entry).toHaveCount(0);
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "github_merge_pull_request",
    ),
  );
  expect(calls).toHaveLength(3);
  expect(calls[2].args).toMatchObject({
    method: "rebase",
    headOid: "b".repeat(40),
  });
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(merge.getByRole("status")).toHaveText("Merged into main");
});

test("PR merge explains draft, blocked, conflicting and unknown states without submitting", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await (await showPullRequests(page))
    .getByRole("button", { name: /View pull request #44/ })
    .click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const merge = screen.getByRole("region", {
    name: "Merge pull request",
    exact: true,
  });
  await expect(merge).toBeVisible();
  await expect(screen.locator(".pr-ready-badge")).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_details"),
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(merge.getByRole("status")).toHaveText(
    "Checking merge readiness…",
  );
  await expect(merge).toHaveAttribute("data-ready", "false");
  await expect(screen.locator(".pr-ready-badge")).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_details"),
  );
  await expect(merge).toHaveAttribute("data-ready", "true");
  await expect(screen.locator(".pr-ready-badge")).toBeVisible();
  for (const [change, message] of [
    [{ isDraft: true }, "Mark this draft ready"],
    [{ mergeStateStatus: "BLOCKED" }, "Waiting for required checks"],
    [{ mergeable: "CONFLICTING" }, "Resolve merge conflicts"],
    [{ mergeStateStatus: "BEHIND" }, "Update this branch"],
    [{ mergeable: "UNKNOWN" }, "Waiting for GitHub"],
  ] as const) {
    await page.evaluate(
      ({ pr, change }) =>
        (window as any).fixture.setPullRequestDetails(pr.url, {
          ...pr,
          ...change,
        }),
      { pr: previewPullRequest, change },
    );
    await screen.getByRole("button", { name: "Refresh", exact: true }).click();
    await expect(merge.getByRole("status")).toContainText(message);
    await expect(merge).toHaveAttribute("data-ready", "false");
    await expect(screen.locator(".pr-ready-badge")).toHaveCount(0);
    await expect(
      merge.getByRole("button", { name: "Merge pull request", exact: true }),
    ).toBeDisabled();
    await expect(
      merge.getByRole("button", { name: "Update branch", exact: true }),
    ).toBeEnabled({
      enabled: !["CONFLICTING", "UNKNOWN"].includes(
        (change as { mergeable?: string }).mergeable ?? "",
      ),
    });
  }
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        state: "CLOSED",
        statusCheckRollup: null,
      }),
    previewPullRequest,
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(merge.getByRole("status")).toContainText(
    "This pull request is closed",
  );
  await expect(merge.getByRole("button")).toHaveCount(0);
  await expect(screen.locator(".pr-ready-badge")).toHaveCount(0);
  await expect(
    screen.getByRole("region", { name: "Pull request checks" }),
  ).toContainText("No checks reported");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_merge_pull_request",
      ),
    ),
  ).toHaveLength(0);
});

test("PR branch updates preserve progress across navigation and wait for the new head before merging", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequestDetails(pr.url, {
      ...pr,
      mergeStateStatus: "BEHIND",
    });
    fixture.hold("github_update_pull_request_branch");
  }, previewPullRequest);
  const entry = (await showPullRequests(page)).getByRole("button", {
    name: /View pull request #44/,
  });
  await entry.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const merge = screen.getByRole("region", {
    name: "Merge pull request",
    exact: true,
  });
  await expect(
    merge.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeDisabled();
  await merge
    .getByRole("button", { name: "Update branch", exact: true })
    .click();
  await expect(merge.getByRole("status")).toHaveText(
    "Updating branch from main…",
  );
  await expect(merge).toHaveAttribute("data-ready", "false");
  await expect(screen.locator(".pr-ready-badge")).toHaveCount(0);
  await expect(
    merge.getByRole("button", { name: "Updating…", exact: true }),
  ).toBeDisabled();
  await expect(
    merge
      .getByRole("button", { name: "Updating…", exact: true })
      .locator(".spinning"),
  ).toBeVisible();
  await expect(
    merge.getByRole("button", { name: "Merge method", exact: true }),
  ).toBeDisabled();
  await expect(
    screen.getByRole("button", { name: "Refresh", exact: true }),
  ).toBeDisabled();
  await screen.getByRole("button", { name: "Back to session" }).click();
  await entry.click();
  await expect(
    merge.getByRole("button", { name: "Updating…", exact: true }),
  ).toBeDisabled();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "github_update_pull_request_branch",
    ),
  );
  expect(calls).toHaveLength(1);
  expect(calls[0].args).toEqual({
    projectPath: "/work/caelum",
    url: previewPullRequest.url,
    headOid: previewPullRequest.headRefOid,
  });

  await page.evaluate(() =>
    (window as any).fixture.release("github_update_pull_request_branch"),
  );
  await page.clock.runFor(10_100);
  // An accepted update with unchanged metadata must not unlock either action.
  await expect(
    merge.getByRole("button", { name: "Updating…", exact: true }),
  ).toBeDisabled();
  await expect(
    merge.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeDisabled();
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        headRefOid: "b".repeat(40),
        mergeStateStatus: "BLOCKED",
        statusCheckRollup: [
          {
            __typename: "CheckRun",
            name: "Tests",
            status: "IN_PROGRESS",
            conclusion: null,
          },
        ],
      }),
    previewPullRequest,
  );
  await page.clock.runFor(10_100);
  await expect(
    merge.getByRole("button", { name: "Update branch", exact: true }),
  ).toBeEnabled();
  await expect(
    merge.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeDisabled();
  await expect(
    screen.getByRole("region", { name: "Pull request checks" }),
  ).toContainText("In progress");
  await expect(entry).toBeVisible();
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        headRefOid: "b".repeat(40),
        statusCheckRollup: [pr.statusCheckRollup[0]],
      }),
    previewPullRequest,
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await merge
    .getByRole("button", { name: "Merge pull request", exact: true })
    .click();
  await expect(merge.getByRole("status")).toHaveText("Merged into main");
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === "github_merge_pull_request",
        ).args.headOid,
    ),
  ).toBe("b".repeat(40));
});

test("PR branch update failures retain the PR and refresh stale heads for retry", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  const entry = (await showPullRequests(page)).getByRole("button", {
    name: /View pull request #44/,
  });
  await entry.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const merge = screen.getByRole("region", {
    name: "Merge pull request",
    exact: true,
  });
  const update = merge.getByRole("button", {
    name: "Update branch",
    exact: true,
  });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_update_pull_request_branch",
      "Write access required (HTTP 403)",
    ),
  );
  await update.click();
  await expect(merge.getByRole("alert")).toContainText("Write access required");
  await expect(update).toBeEnabled();
  await expect(entry).toBeVisible();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_update_pull_request_branch", null);
    fixture.setPullRequestDetails(pr.url, {
      ...pr,
      headRefOid: "b".repeat(40),
    });
  }, previewPullRequest);
  await update.click();
  await expect(merge.getByRole("alert")).toContainText(
    "Head branch was modified",
  );
  await expect(update).toBeEnabled();
  await update.click();
  await expect(merge.getByRole("alert")).toHaveCount(0);
  await expect(merge).toHaveAttribute("data-ready", "false");
  await expect(screen.locator(".pr-ready-badge")).toHaveCount(0);
  await expect(
    merge.getByRole("button", { name: "Updating…", exact: true }),
  ).toBeDisabled();
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        headRefOid: "c".repeat(40),
      }),
    previewPullRequest,
  );
  await page.clock.runFor(10_100);
  await expect(update).toBeEnabled();
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "github_update_pull_request_branch",
    ),
  );
  expect(calls).toHaveLength(3);
  expect(calls[2].args.headOid).toBe("b".repeat(40));
  await expect(entry).toBeVisible();
});

test("PR branch updates recover from unconfirmed updates and refresh failures", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await (await showPullRequests(page))
    .getByRole("button", { name: /View pull request #44/ })
    .click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const merge = screen.getByRole("region", {
    name: "Merge pull request",
    exact: true,
  });
  await merge
    .getByRole("button", { name: "Update branch", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      "Network unavailable",
    ),
  );
  await page.clock.runFor(100_100);
  await expect(merge.getByRole("alert")).toContainText(
    "GitHub has not confirmed the branch update yet",
  );
  await expect(
    screen.getByRole("button", { name: "Refresh", exact: true }),
  ).toBeEnabled();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_details",
      null,
    ),
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(
    merge.getByRole("button", { name: "Update branch", exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_update_pull_request_branch",
      ),
    ),
  ).toHaveLength(1);
});
