import { expect, test } from "@playwright/test";
import {
  showPullRequests,
  previewPullRequest,
  preparePullRequestPreview,
} from "./fixtures/helpers.js";

test("PR checks show live pending animations, passed ticks and linked results in both themes", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await page.evaluate((pr) => {
    (window as any).fixture.setPullRequestDetails(pr.url, {
      ...pr,
      body: "Check progress and merge this pull request.",
      mergeStateStatus: "BLOCKED",
      statusCheckRollup: [
        pr.statusCheckRollup[0],
        {
          __typename: "CheckRun",
          name: "macOS build",
          status: "IN_PROGRESS",
          conclusion: null,
          startedAt: new Date(Date.now() - 65_000).toISOString(),
          completedAt: null,
        },
        {
          __typename: "CheckRun",
          name: "Linux build",
          status: "QUEUED",
          conclusion: null,
        },
        {
          __typename: "CheckRun",
          name: "Lint",
          status: "COMPLETED",
          conclusion: "FAILURE",
          startedAt: "2026-11-01T12:00:00Z",
          completedAt: "2026-11-01T12:00:12Z",
          detailsUrl: "javascript:alert(1)",
        },
        {
          __typename: "CheckRun",
          name: "Optional deploy",
          status: "COMPLETED",
          conclusion: "SKIPPED",
        },
        pr.statusCheckRollup[1],
      ],
    });
  }, previewPullRequest);
  await (await showPullRequests(page))
    .getByRole("button", { name: /View pull request #44/ })
    .click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const checks = screen.getByRole("region", { name: "Pull request checks" });
  const rows = checks.getByRole("listitem");
  await expect(rows).toHaveCount(6);
  await expect(rows.filter({ hasText: "Tests" })).toContainText("Passed");
  await expect(
    rows.filter({ hasText: "Tests" }).locator(".lucide-check"),
  ).toBeVisible();
  await expect(rows.filter({ hasText: "macOS build" })).toContainText(
    "In progress",
  );
  await expect(rows.filter({ hasText: "Linux build" })).toContainText("Queued");
  await expect(
    rows.filter({ hasText: "Lint" }).locator(".lucide-x"),
  ).toBeVisible();
  await expect(rows.filter({ hasText: "Lint" }).getByRole("link")).toHaveCount(
    0,
  );
  await expect(rows.filter({ hasText: "Optional deploy" })).toContainText(
    "Skipped",
  );
  const spinner = rows.filter({ hasText: "macOS build" }).locator(".spinning");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await expect(spinner).toHaveCSS("animation-name", "spin");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(spinner).toHaveCSS("animation-name", "none");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await checks.getByRole("link", { name: "Tests", exact: true }).click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe("https://github.com/example/caelum/actions/runs/123");
  await checks
    .getByRole("link", { name: "Preview deploy", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe("https://deploy.example.com/preview/44");
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await page.screenshot({ path: `test-results/pr-checks-${theme}.png` });
    await screen
      .getByRole("region", { name: "Merge pull request", exact: true })
      .scrollIntoViewIfNeeded();
    await page.screenshot({ path: `test-results/pr-merge-${theme}.png` });
    await screen
      .getByRole("button", { name: "Merge method", exact: true })
      .click();
    const menu = page.getByRole("dialog", { name: "Choose merge method" });
    await expect(menu).toBeInViewport({ ratio: 1 });
    await page.screenshot({ path: `test-results/pr-merge-menu-${theme}.png` });
    await page.keyboard.press("Escape");
    await screen.evaluate((node) => {
      node.scrollTop = 0;
    });
  }
  await page.setViewportSize({ width: 800, height: 620 });
  await checks.scrollIntoViewIfNeeded();
  expect(
    await screen.evaluate((node) => node.scrollWidth <= node.clientWidth),
  ).toBe(true);
  await page.screenshot({ path: "test-results/pr-checks-compact.png" });
  await expect(
    screen.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeDisabled();
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        statusCheckRollup: [pr.statusCheckRollup[0]],
      }),
    previewPullRequest,
  );
  await page.clock.runFor(30_100);
  await expect(checks).toContainText("1 passed");
  await expect(checks.locator(".spinning")).toHaveCount(0);
  await expect(
    screen.getByRole("button", { name: "Merge pull request", exact: true }),
  ).toBeEnabled();
  await screen
    .getByRole("region", { name: "Merge pull request", exact: true })
    .scrollIntoViewIfNeeded();
  await page.screenshot({ path: "test-results/pr-merge-compact.png" });
  await screen
    .getByRole("button", { name: "Merge method", exact: true })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Choose merge method" }),
  ).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: "test-results/pr-merge-menu-compact.png" });
  await page.getByRole("option", { name: /^Rebase and merge/ }).click();
  await expect(
    screen.getByRole("button", { name: "Rebase and merge", exact: true }),
  ).toBeEnabled();
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "github_merge_pull_request",
      ),
    ),
  ).toHaveLength(0);
});

test("PR check timers tick locally, resume from server timestamps and stop on completion", async ({
  page,
}) => {
  await preparePullRequestPreview(page);
  await page.clock.pauseAt(new Date("2030-01-01T12:00:00Z"));
  await page.evaluate((pr) => {
    (window as any).fixture.setPullRequestDetails(pr.url, {
      ...pr,
      statusCheckRollup: [
        pr.statusCheckRollup[0],
        {
          __typename: "CheckRun",
          name: "Build",
          status: "IN_PROGRESS",
          conclusion: null,
          startedAt: "2030-01-01T11:58:55Z",
          completedAt: "0001-01-01T00:00:00Z",
        },
        {
          __typename: "CheckRun",
          name: "Queued",
          status: "QUEUED",
          conclusion: null,
          startedAt: "0001-01-01T00:00:00Z",
          completedAt: null,
        },
        pr.statusCheckRollup[1],
      ],
    });
  }, previewPullRequest);
  const entry = (await showPullRequests(page)).getByRole("button", {
    name: /View pull request #44/,
  });
  await entry.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  const rows = screen
    .getByRole("region", { name: "Pull request checks" })
    .getByRole("listitem");
  const completed = rows.filter({ hasText: "Tests" }).locator("time");
  const running = rows.filter({ hasText: "Build" }).locator("time");
  await expect(completed).toHaveText("2m 14s");
  await expect(completed).toHaveAttribute("datetime", "PT134S");
  await expect(running).toHaveText("1m 5s");
  await expect(running).toHaveAttribute("aria-label", "Elapsed time: 1m 5s");
  await expect(rows.filter({ hasText: "Queued" }).locator("time")).toHaveCount(
    0,
  );
  await expect(
    rows.filter({ hasText: "Preview deploy" }).locator("time"),
  ).toHaveCount(0);
  const detailCalls = () =>
    page.evaluate(
      () =>
        (window as any).fixture.calls.filter(
          (call: any) => call.command === "github_pull_request_details",
        ).length,
    );
  const requests = await detailCalls();
  await page.clock.runFor(3000);
  await expect(running).toHaveText("1m 8s");
  await expect(completed).toHaveText("2m 14s");
  expect(await detailCalls()).toBe(requests);
  await screen.getByRole("button", { name: "Back to session" }).click();
  await page.clock.runFor(10_000);
  await entry.click();
  await expect(running).toHaveText("1m 18s");
  await page.evaluate(
    (pr) =>
      (window as any).fixture.setPullRequestDetails(pr.url, {
        ...pr,
        statusCheckRollup: [
          pr.statusCheckRollup[0],
          {
            __typename: "CheckRun",
            name: "Build",
            status: "COMPLETED",
            conclusion: "SUCCESS",
            startedAt: "2030-01-01T11:58:55Z",
            completedAt: "2030-01-01T12:00:05Z",
          },
        ],
      }),
    previewPullRequest,
  );
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await expect(running).toHaveText("1m 10s");
  await expect(running).toHaveAttribute("aria-label", "Duration: 1m 10s");
  await page.clock.runFor(5000);
  await expect(running).toHaveText("1m 10s");
});
