import { expect, test } from "@playwright/test";
import {
  previewPullRequest,
  preparePullRequestReview,
} from "./fixtures/helpers.js";

test("PR files load on demand as continuous diffs with renames, automatic pagination and retained errors", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        ["github_pull_request_files", "github_pull_request_feedback"].includes(
          call.command,
        ),
      ),
    ),
  ).toHaveLength(0);
  await screen.getByRole("tab", { name: "Overview", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(
    screen.getByRole("tab", { name: "Files changed (3)" }),
  ).toBeFocused();
  await expect(
    screen.getByRole("tab", { name: "Files changed (3)" }),
  ).toHaveAttribute("aria-selected", "true");
  const source = screen.getByRole("article", {
    name: "Diff for src/new name.ts",
    exact: true,
  });
  const readme = screen.getByRole("article", {
    name: "Diff for README.md",
    exact: true,
  });
  await expect(source).toContainText("Renamed from src/old name.ts");
  await expect(source.locator(".diff-line.added")).toContainText("newValue");
  await expect(source.locator(".diff-line.removed")).toContainText("oldValue");
  await screen
    .getByRole("navigation", { name: "Changed files" })
    .getByRole("button", { name: /assets\/logo.png/ })
    .click();
  await expect(screen).toContainText("GitHub did not provide a text patch");
  await expect(screen.locator(".pr-file-diff")).toHaveCount(3);
  await screen
    .getByRole("navigation", { name: "Changed files" })
    .getByRole("button", { name: /README.md/ })
    .click();
  await expect(readme.locator(".diff-line.added")).toContainText(
    "More documentation",
  );
  await expect(
    screen.getByRole("button", { name: "Load more files", exact: true }),
  ).toHaveCount(0);
  for (const theme of ["dark", "light"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    expect(
      await screen.evaluate(
        (element) => element.scrollWidth <= element.clientWidth + 1,
      ),
    ).toBe(true);
  }
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_files",
      "Network unavailable",
    ),
  );
  await screen
    .getByRole("button", { name: "Refresh files", exact: true })
    .click();
  await expect(screen.getByRole("alert")).toContainText(
    "Showing the last loaded results",
  );
  await expect(readme.locator(".diff-line.added")).toContainText(
    "More documentation",
  );
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("github_pull_request_files", null),
  );
  await screen
    .getByRole("button", { name: "Retry files", exact: true })
    .click();
  await expect(screen.getByRole("alert")).toHaveCount(0);
  await expect(screen.locator(".pr-file-diff")).toHaveCount(3);
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "github_pull_request_files",
    ),
  );
  expect(calls.map((call: any) => call.args.page)).toEqual([1, 2, 1, 1, 2]);
  expect(
    calls.every((call: any) => call.args.url === previewPullRequest.url),
  ).toBe(true);
});

test("PR file diffs scroll through full patches with sticky navigation, line numbers and collapsible files", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  await page.evaluate((url) => {
    const fixture = (window as any).fixture;
    const patch = (count: number) =>
      `@@ -0,0 +1,${count} @@\n${Array.from({ length: count }, (_, i) => `+const value${i} = ${i};`).join("\n")}`;
    const file = (path: string, text: string) => ({
      path,
      previousPath: null,
      status: "modified",
      additions: 100,
      deletions: 1,
      patch: text,
    });
    fixture.setPullRequestPage(url, "files", 1, {
      baseOid: "base",
      files: [
        file(
          "src/review.ts",
          `@@ -10,3 +20,4 @@\n keep\n-old\n+new\n+extra\n end\n\\ No newline at end of file\n${patch(140)}`,
        ),
        file("docs/guide.md", patch(100)),
      ],
      nextPage: 2,
    });
    fixture.setPullRequestPage(url, "files", 2, {
      baseOid: "base",
      files: [file("src/last.ts", patch(80))],
      nextPage: null,
    });
  }, previewPullRequest.url);
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  const source = screen.getByRole("article", {
    name: "Diff for src/review.ts",
    exact: true,
  });
  const nav = screen.getByRole("navigation", {
    name: "Changed files",
    exact: true,
  });
  await expect(screen.locator(".pr-file-diff")).toHaveCount(2);
  await expect(
    source.locator('.diff-line.context [data-side="old"]'),
  ).toHaveText(["10", "12"]);
  await expect(
    source.locator('.diff-line.context [data-side="new"]'),
  ).toHaveText(["20", "23"]);
  await expect(
    source.locator('.diff-line.removed [data-side="old"]'),
  ).toHaveText("11");
  await expect(
    source.locator('.diff-line.removed [data-side="new"]'),
  ).toHaveText("");
  await expect(
    source.locator('.diff-line.added [data-side="new"]').nth(2),
  ).toHaveText("1");
  expect(
    await source.locator("pre").evaluate((element) => element.clientHeight),
  ).toBeGreaterThan(2000);
  expect(
    await source
      .locator("pre")
      .evaluate((element) => element.scrollHeight <= element.clientHeight + 1),
  ).toBe(true);
  const bounds = (await screen.boundingBox())!;
  await page.mouse.move(
    bounds.x + bounds.width - 120,
    bounds.y + bounds.height / 2,
  );
  await page.mouse.wheel(0, 850);
  await expect
    .poll(() => screen.evaluate((element) => element.scrollTop))
    .toBeGreaterThan(600);
  await expect
    .poll(() =>
      source.locator(".pr-file-heading").evaluate((element) => {
        const screen = element.closest(".pr-screen")!;
        const bar = screen.querySelector(".pr-files-toolbar")!;
        return Math.abs(
          element.getBoundingClientRect().top -
            bar.getBoundingClientRect().bottom,
        );
      }),
    )
    .toBeLessThan(2);
  await expect(
    nav.getByRole("button", { name: /src\/review.ts/ }),
  ).toHaveAttribute("aria-current", "true");
  await expect(screen.locator(".pr-file-diff")).toHaveCount(2);
  await page.screenshot({ path: "test-results/pr-continuous-diff-dark.png" });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_pull_request_files",
      "Next page unavailable",
    ),
  );
  await nav.getByRole("button", { name: /docs\/guide.md/ }).click();
  await page.mouse.move(
    bounds.x + bounds.width - 120,
    bounds.y + bounds.height / 2,
  );
  await page.mouse.wheel(0, 1400);
  await expect(screen.getByRole("alert")).toContainText(
    "Next page unavailable",
  );
  await expect(screen.locator(".pr-file-diff")).toHaveCount(2);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("github_pull_request_files", null),
  );
  await screen
    .getByRole("button", { name: "Retry files", exact: true })
    .click();
  await expect(screen.locator(".pr-file-diff")).toHaveCount(3);
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls
      .filter((call: any) => call.command === "github_pull_request_files")
      .map((call: any) => call.args.page),
  );
  expect(calls).toEqual([1, 2, 2]);
  await nav.getByRole("button", { name: /src\/last.ts/ }).click();
  await expect(
    nav.getByRole("button", { name: /src\/last.ts/ }),
  ).toHaveAttribute("aria-current", "true");
  await screen
    .getByRole("button", { name: "Collapse all", exact: true })
    .click();
  await expect(screen.locator(".pr-file-body:visible")).toHaveCount(0);
  await nav.getByRole("button", { name: /src\/last.ts/ }).click();
  await expect(
    screen.getByRole("button", {
      name: "Collapse diff for src/last.ts",
      exact: true,
    }),
  ).toHaveAttribute("aria-expanded", "true");
  await expect(screen.locator(".pr-file-body:visible")).toHaveCount(1);
  await screen.getByRole("button", { name: "Expand all", exact: true }).click();
  await expect(screen.locator(".pr-file-body:visible")).toHaveCount(3);
  const width = await source.evaluate((element) => element.clientWidth);
  await screen
    .getByRole("button", { name: "Hide file list", exact: true })
    .click();
  await expect(nav).toBeHidden();
  expect(
    await source.evaluate((element) => element.clientWidth),
  ).toBeGreaterThan(width);
  await screen
    .getByRole("button", { name: "Show file list", exact: true })
    .click();
  await nav.getByRole("button", { name: /src\/review.ts/ }).click();
  await page.evaluate(() => (document.documentElement.dataset.theme = "light"));
  await page.screenshot({ path: "test-results/pr-continuous-diff-light.png" });
  await page.setViewportSize({ width: 860, height: 760 });
  expect(
    await screen.evaluate(
      (element) => element.scrollWidth <= element.clientWidth + 1,
    ),
  ).toBe(true);
  await expect(source.locator("pre")).toBeVisible();
});

test("PR review places threads on exact diff sides and keeps outdated, omitted and file-level context beside the file", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  const patch = `@@ -10,3 +10,4 @@\n keep\n-oldValue\n+newValue\n+extraValue\n end\n@@ -40 +50 @@\n-last\n+${"longValue".repeat(80)}`;
  await page.evaluate(
    ({ url, patch }) => {
      const fixture = (window as any).fixture;
      fixture.setPullRequestPage(url, "files", 1, {
        baseOid: "base",
        files: [
          {
            path: "src/new name.ts",
            previousPath: "src/old name.ts",
            status: "renamed",
            additions: 4,
            deletions: 3,
            patch,
          },
          {
            path: "assets/logo.png",
            previousPath: null,
            status: "added",
            additions: 0,
            deletions: 0,
            patch: null,
          },
        ],
        nextPage: 2,
      });
      const thread = (
        id: string,
        side: string,
        line: number | null,
        extra = {},
      ) => ({
        id,
        path: "src/new name.ts",
        line,
        originalLine: line,
        startLine: null,
        diffSide: side,
        isResolved: false,
        isOutdated: false,
        nextCursor: null,
        comments: [
          {
            id: `${id}-comment`,
            body: `Feedback ${id}\n\n\`\`\`suggestion\nreturn value;\n\`\`\``,
            url: `${url}#discussion_${id}`,
            author: { login: "reviewer" },
            createdAt: "2026-11-01T12:00:00Z",
            state: null,
            diffHunk: "@@ -10 +10 @@\n-old\n+new",
          },
        ],
        ...extra,
      });
      fixture.setPullRequestPage(url, "threads", "", {
        threads: [
          thread("left", "LEFT", 11, { path: "src/old name.ts" }),
          thread("right", "RIGHT", 11),
          thread("same-line", "RIGHT", 11, { isResolved: true }),
          thread("range", "RIGHT", 12, { startLine: 10 }),
          thread("outdated", "RIGHT", 11, { isOutdated: true }),
          thread("omitted", "LEFT", 99),
          thread("file-level", "RIGHT", null),
          thread("binary", "RIGHT", null, { path: "assets/logo.png" }),
          thread("missing", "RIGHT", 1, { path: "missing.ts" }),
        ],
        nextCursor: "more-threads",
      });
      fixture.setPullRequestPage(url, "threads", "more-threads", {
        threads: [thread("later", "RIGHT", 2, { path: "README.md" })],
        nextCursor: null,
      });
    },
    { url: previewPullRequest.url, patch },
  );
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  await expect(
    screen.getByRole("tab", { name: "Comments", exact: true }),
  ).toHaveCount(0);
  const source = screen.getByRole("article", {
    name: "Diff for src/new name.ts",
    exact: true,
  });
  const annotation = (id: string) =>
    source.locator(`.diff-annotation:has([data-thread-id="${id}"])`);
  for (const [id, side, line, text] of [
    ["left", "old", "11", "-oldValue"],
    ["right", "new", "11", "+newValue"],
    ["range", "new", "12", "+extraValue"],
  ]) {
    await expect(annotation(id)).toBeVisible();
    expect(
      await annotation(id).evaluate((element, side) => {
        const line = element.previousElementSibling!.querySelector(
          ".diff-line:last-child",
        )!;
        return {
          number: line.getAttribute(`data-${side}-line`),
          text: line.querySelector(".diff-source")!.textContent!.trim(),
        };
      }, side),
    ).toEqual({ number: line, text });
  }
  await expect(annotation("right").locator(".pr-feedback")).toHaveCount(2);
  const resolved = source.locator('[data-thread-id="same-line"]');
  await expect(resolved.locator(".pr-comment")).toBeHidden();
  await resolved.locator("summary").click();
  await expect(resolved.locator(".pr-comment")).toBeVisible();
  await expect(resolved.getByRole("checkbox")).not.toBeChecked();
  await resolved.locator("summary").click();
  await expect(annotation("range")).toContainText("New lines 10–12");
  for (const id of ["outdated", "omitted", "file-level"])
    await expect(
      source.locator(`.pr-file-comments [data-thread-id="${id}"]`),
    ).toHaveCount(1);
  await expect(
    source.locator('[data-thread-id="outdated"] input'),
  ).not.toBeChecked();
  await source
    .locator('[data-thread-id="outdated"]')
    .getByText("Diff context", { exact: true })
    .click();
  await expect(
    source.locator('[data-thread-id="outdated"] .diff-source').last(),
  ).toContainText("+new");
  await expect(
    screen
      .getByRole("article", { name: "Diff for assets/logo.png", exact: true })
      .locator('[data-thread-id="binary"]'),
  ).toHaveCount(1);
  await expect(
    screen
      .getByRole("region", { name: "Comments on other files", exact: true })
      .locator('[data-thread-id="missing"]'),
  ).toHaveCount(1);
  await screen
    .getByRole("button", { name: "Load more review threads", exact: true })
    .click();
  await screen.locator(".pr-files-pagination").scrollIntoViewIfNeeded();
  await expect(
    screen.getByRole("article", { name: "Diff for README.md", exact: true }),
  ).toHaveCount(1);
  await screen
    .getByRole("navigation", { name: "Changed files", exact: true })
    .getByRole("group", { name: "Comments on README.md", exact: true })
    .getByRole("button")
    .click();
  const later = screen
    .getByRole("article", { name: "Diff for README.md", exact: true })
    .locator('.diff-annotation [data-thread-id="later"]');
  await expect(later).toHaveCount(1);
  await expect(screen.locator("[data-thread-id]")).toHaveCount(10);
  expect(
    (
      await source
        .locator(".pr-file-body > .git-diff-section > pre .diff-source")
        .allTextContents()
    ).join(""),
  ).toBe(patch);
  await screen
    .getByRole("navigation", { name: "Changed files", exact: true })
    .getByRole("button", { name: /src\/new name.ts/ })
    .click();
  for (const theme of ["dark", "light"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    await page.screenshot({
      path: `test-results/pr-inline-comments-${theme}.png`,
    });
    expect(
      await screen.evaluate(
        (element) => element.scrollWidth <= element.clientWidth + 1,
      ),
    ).toBe(true);
  }
  await page.setViewportSize({ width: 860, height: 760 });
  expect(
    await screen.evaluate(
      (element) => element.scrollWidth <= element.clientWidth + 1,
    ),
  ).toBe(true);
  expect(
    await screen.evaluate((element) =>
      [...element.querySelectorAll<HTMLElement>("*")]
        .filter(
          (node) =>
            node.getClientRects().length &&
            !node.classList.contains("pr-file-list") &&
            /auto|scroll/.test(getComputedStyle(node).overflowY) &&
            node.scrollHeight > node.clientHeight + 1,
        )
        .map((node) => node.className),
    ),
  ).toEqual([]);
});

test("PR review scrolls its file tree independently while code and expanded comment context scroll the page", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    const patch = `@@ -0,0 +1,100 @@\n${Array.from({ length: 100 }, (_, index) => `+const line${index} = ${index};`).join("\n")}`;
    fixture.setPullRequestDetails(pr.url, { ...pr, changedFiles: 40 });
    fixture.setPullRequestPage(pr.url, "files", 1, {
      baseOid: "base",
      files: Array.from({ length: 40 }, (_, index) => ({
        path: `src/file${index}.ts`,
        previousPath: null,
        status: "added",
        additions: 100,
        deletions: 0,
        patch,
      })),
      nextPage: null,
    });
    fixture.setPullRequestPage(pr.url, "threads", "", {
      threads: [
        {
          id: "old",
          path: "src/file0.ts",
          line: null,
          originalLine: 50,
          startLine: null,
          diffSide: "RIGHT",
          isResolved: false,
          isOutdated: true,
          nextCursor: null,
          comments: [
            {
              id: "old-comment",
              body: "Earlier version",
              url: `${pr.url}#discussion_old`,
              author: null,
              createdAt: "2026-11-01T12:00:00Z",
              state: null,
              diffHunk: patch,
            },
          ],
        },
      ],
      nextCursor: null,
    });
  }, previewPullRequest);
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await screen
    .getByRole("tab", { name: "Files changed (40)", exact: true })
    .click();
  const nav = screen.getByRole("navigation", {
    name: "Changed files",
    exact: true,
  });
  await expect(nav.getByRole("button", { name: /^src\/file/ })).toHaveCount(40);
  expect(
    await nav.evaluate(
      (element) => element.scrollHeight > element.clientHeight + 1,
    ),
  ).toBe(true);
  await nav.getByRole("button", { name: /src\/file0.ts/ }).click();
  const navBounds = (await nav.boundingBox())!;
  const screenBounds = (await screen.boundingBox())!;
  const top = await screen.evaluate((element) => element.scrollTop);
  await page.mouse.move(navBounds.x + 30, navBounds.y + navBounds.height / 2);
  await page.mouse.wheel(0, 300);
  await expect
    .poll(() => nav.evaluate((element) => element.scrollTop))
    .toBeGreaterThan(200);
  expect(await screen.evaluate((element) => element.scrollTop)).toBe(top);
  await nav.focus();
  await page.keyboard.press("End");
  await expect
    .poll(() =>
      nav.evaluate(
        (element) =>
          element.scrollHeight - element.clientHeight - element.scrollTop,
      ),
    )
    .toBeLessThan(2);
  const lastFile = nav.getByRole("button", { name: /^src\/file39.ts/ });
  await expect(lastFile).toBeInViewport();
  await page.mouse.wheel(0, 300);
  expect(await screen.evaluate((element) => element.scrollTop)).toBe(top);
  const treeTop = await nav.evaluate((element) => element.scrollTop);
  await lastFile.click();
  await expect(lastFile).toHaveAttribute("aria-current", "true");
  await expect(
    screen
      .getByRole("article", { name: "Diff for src/file39.ts", exact: true })
      .locator(".pr-file-heading"),
  ).toBeInViewport();
  await screen.getByRole("tab", { name: "Overview", exact: true }).click();
  await screen
    .getByRole("tab", { name: "Files changed (40)", exact: true })
    .click();
  expect(await nav.evaluate((element) => element.scrollTop)).toBe(treeTop);
  await nav.focus();
  await page.keyboard.press("Home");
  await expect.poll(() => nav.evaluate((element) => element.scrollTop)).toBe(0);
  await nav.getByRole("button", { name: /^src\/file0.ts/ }).click();
  const source = screen.getByRole("article", {
    name: "Diff for src/file0.ts",
    exact: true,
  });
  await source.getByText("Diff context", { exact: true }).click();
  const context = source.locator(".pr-comment-context pre");
  expect(
    await context.evaluate((element) => element.clientHeight),
  ).toBeGreaterThan(2000);
  await context.evaluate((element) =>
    element.scrollIntoView({ block: "start" }),
  );
  const before = await screen.evaluate((element) => element.scrollTop);
  const bounds = (await context.boundingBox())!;
  await page.mouse.move(
    bounds.x + 120,
    screenBounds.y + screenBounds.height - 100,
  );
  await page.mouse.wheel(0, 500);
  await expect
    .poll(() => screen.evaluate((element) => element.scrollTop))
    .toBeGreaterThan(before + 400);
  expect(
    await screen.evaluate((element) =>
      [...element.querySelectorAll<HTMLElement>("*")]
        .filter(
          (node) =>
            node.getClientRects().length &&
            /auto|scroll/.test(getComputedStyle(node).overflowY) &&
            node.scrollHeight > node.clientHeight + 1,
        )
        .map((node) => node.className),
    ),
  ).toEqual(["pr-file-list"]);
  await page.setViewportSize({ width: 1280, height: 620 });
  await nav.getByRole("button", { name: /^src\/file0.ts/ }).click();
  await expect
    .poll(() =>
      nav.evaluate((element) => {
        const screen = element.closest(".pr-screen")!;
        return (
          element.getBoundingClientRect().bottom -
          screen.getBoundingClientRect().bottom
        );
      }),
    )
    .toBeLessThanOrEqual(-11);
  await page.setViewportSize({ width: 860, height: 620 });
  await expect
    .poll(() => nav.evaluate((element) => element.clientHeight))
    .toBeLessThanOrEqual(320);
  await nav.evaluate((element) => element.scrollIntoView({ block: "end" }));
  await expect
    .poll(() =>
      nav.evaluate((element) => {
        const toolbar = element
          .closest(".pr-files")!
          .querySelector(".pr-files-toolbar")!;
        return (
          element.getBoundingClientRect().top -
          toolbar.getBoundingClientRect().bottom
        );
      }),
    )
    .toBeGreaterThanOrEqual(0);
  const narrowTop = await screen.evaluate((element) => element.scrollTop);
  const narrowBounds = (await nav.boundingBox())!;
  await page.mouse.move(
    narrowBounds.x + 30,
    narrowBounds.y + narrowBounds.height / 2,
  );
  await page.mouse.wheel(0, 300);
  await expect
    .poll(() => nav.evaluate((element) => element.scrollTop))
    .toBeGreaterThan(200);
  expect(await screen.evaluate((element) => element.scrollTop)).toBe(narrowTop);
});

test("PR large reviews keep tab switches light and retain loaded patches", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    const patch = `@@ -0,0 +1,240 @@\n${Array.from({ length: 240 }, (_, index) => `+export const value${index} = { name: "value", count: ${index} };`).join("\n")}`;
    fixture.setPullRequestDetails(pr.url, { ...pr, changedFiles: 30 });
    fixture.setPullRequestPage(pr.url, "files", 1, {
      baseOid: "base",
      files: Array.from({ length: 30 }, (_, index) => ({
        path: `src/file${index}.ts`,
        previousPath: null,
        status: "added",
        additions: 240,
        deletions: 0,
        patch,
      })),
      nextPage: null,
    });
    fixture.setPullRequestPage(pr.url, "threads", "", {
      threads: [
        {
          id: "far-comment",
          path: "src/file29.ts",
          line: 180,
          originalLine: 180,
          startLine: null,
          diffSide: "RIGHT",
          isResolved: false,
          isOutdated: false,
          nextCursor: null,
          comments: [
            {
              id: "far-comment-body",
              body: "Review the distant file.",
              url: `${pr.url}#discussion_far`,
              author: { login: "reviewer" },
              createdAt: "2026-11-01T12:00:00Z",
              state: null,
              diffHunk: null,
            },
          ],
        },
      ],
      nextCursor: null,
    });
  }, previewPullRequest);
  await screen.getByRole("button", { name: "Refresh", exact: true }).click();
  await screen
    .getByRole("tab", { name: "Files changed (30)", exact: true })
    .click();
  await expect(screen.locator(".pr-file-diff")).toHaveCount(30);
  await expect(screen.locator(".syntax-keyword").first()).toBeVisible();
  expect(await screen.locator(".diff-line").count()).toBeLessThan(500);
  await screen
    .locator(".diff-source")
    .first()
    .evaluate((node) => ((window as any).keptDiffLine = node));
  for (let index = 0; index < 3; index++) {
    await screen.getByRole("tab", { name: "Overview", exact: true }).click();
    await screen
      .getByRole("tab", { name: "Files changed (30)", exact: true })
      .click();
  }
  expect(await screen.locator(".diff-line").count()).toBeLessThan(500);
  expect(
    await screen
      .locator(".diff-source")
      .first()
      .evaluate((node) => node === (window as any).keptDiffLine),
  ).toBe(true);
  const nav = screen.getByRole("navigation", {
    name: "Changed files",
    exact: true,
  });
  await expect(
    nav.getByRole("button", { name: /^src\/file29.ts/ }),
  ).toContainText("1");
  await screen
    .getByRole("button", { name: "Collapse all", exact: true })
    .click();
  await nav
    .getByRole("group", { name: "Comments on src/file29.ts", exact: true })
    .getByRole("button")
    .click();
  const thread = screen.locator('[data-thread-id="far-comment"]');
  await expect(thread).toBeFocused();
  await expect(thread).toBeInViewport();
  await expect(
    screen.getByRole("button", {
      name: "Collapse diff for src/file29.ts",
      exact: true,
    }),
  ).toHaveAttribute("aria-expanded", "true");
  const last = screen.getByRole("article", {
    name: "Diff for src/file29.ts",
    exact: true,
  });
  await expect(last.locator(".syntax-keyword").first()).toHaveText("export");
  expect(await screen.locator(".diff-line").count()).toBeLessThan(1500);
  expect(
    await thread.evaluate(
      (node) =>
        node.getBoundingClientRect().top -
        node
          .closest(".pr-screen")!
          .querySelector(".pr-files-toolbar")!
          .getBoundingClientRect().bottom,
    ),
  ).toBeGreaterThan(35);
  await thread.getByRole("checkbox").uncheck();
  await screen.getByRole("tab", { name: "Overview", exact: true }).click();
  await screen
    .getByRole("tab", { name: "Files changed (30)", exact: true })
    .click();
  await expect(thread.getByRole("checkbox")).not.toBeChecked();
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.filter(
          (call: any) => call.command === "github_pull_request_files",
        ).length,
    ),
  ).toBe(1);
});

test("PR file tree exposes comment locations, states and discussion links with keyboard jumps", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  const nav = screen.getByRole("navigation", {
    name: "Changed files",
    exact: true,
  });
  await expect(nav).toContainText("3 loaded review threads");
  await expect(
    nav.getByRole("button", { name: /^src\/new name.ts/ }),
  ).toContainText("1");
  const comments = nav.getByRole("group", {
    name: "Comments on src/new name.ts",
    exact: true,
  });
  await expect(comments).toContainText("New lines 2–3");
  await expect(comments).toContainText("Preserve the return value");
  const resolvedLink = nav.getByRole("group", {
    name: "Comments on resolved.ts",
    exact: true,
  });
  await expect(resolvedLink).toContainText("Resolved");
  await expect(resolvedLink).not.toContainText("Already addressed");
  await expect(
    nav.getByRole("group", { name: "Comments on outdated.ts", exact: true }),
  ).toContainText("Outdated");
  await screen
    .getByRole("button", {
      name: "Collapse diff for src/new name.ts",
      exact: true,
    })
    .click();
  await comments.getByRole("button").focus();
  await page.keyboard.press("Enter");
  const thread = screen.locator('[data-thread-id="inline"]');
  await expect(thread).toBeFocused();
  await expect(thread).toBeInViewport();
  await expect(thread.getByRole("checkbox")).toBeDisabled();
  await thread
    .getByRole("button", { name: "Load remaining replies", exact: true })
    .click();
  await expect(thread).toContainText("Unicode: 空白");
  await expect(thread.getByRole("checkbox")).toBeChecked();
  await resolvedLink.getByRole("button").click();
  const resolved = screen.locator('[data-thread-id="resolved"]');
  await expect(resolved).toBeFocused();
  await expect(resolved).toBeInViewport();
  await expect(resolved.locator(".pr-comment")).toBeHidden();
  await resolved.locator("summary").focus();
  await page.keyboard.press("Enter");
  await expect(resolved.locator(".pr-comment")).toBeVisible();
  await resolved.getByRole("checkbox").check();
  await resolved.locator("summary").click();
  await expect(resolved.locator("summary")).toContainText("Selected");
  await screen.getByRole("tab", { name: "Overview", exact: true }).click();
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  await expect(resolved.locator(".pr-comment")).toBeHidden();
  await expect(resolved.locator("input")).toBeChecked();
  await nav
    .getByRole("group", { name: "Comments on outdated.ts", exact: true })
    .getByRole("button")
    .click();
  await expect(screen.locator('[data-thread-id="outdated"]')).toBeFocused();
  await expect(screen.locator('[data-thread-id="outdated"]')).toBeInViewport();
  await nav.getByRole("button", { name: /^Discussion/ }).click();
  await expect(
    screen.getByRole("region", { name: "Discussion", exact: true }),
  ).toBeFocused();
  await nav.getByRole("button", { name: /^Reviews/ }).click();
  await expect(
    screen.getByRole("region", { name: "Reviews", exact: true }),
  ).toBeFocused();
  await nav.getByRole("button", { name: /^src\/new name.ts/ }).click();
  for (const theme of ["dark", "light"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    await page.screenshot({
      path: `test-results/pr-comment-tree-${theme}.png`,
    });
  }
});

test("PR inline feedback keeps complete replies and selections when delayed files place the thread", async ({
  page,
}) => {
  const screen = await preparePullRequestReview(page);
  await page.evaluate(() =>
    (window as any).fixture.hold("github_pull_request_files"),
  );
  await screen
    .getByRole("tab", { name: "Files changed (3)", exact: true })
    .click();
  const other = screen.getByRole("region", {
    name: "Comments on other files",
    exact: true,
  });
  await expect(other.locator('[data-thread-id="inline"]')).toHaveCount(1);
  await other
    .getByRole("button", { name: "Load remaining replies", exact: true })
    .click();
  const choice = screen.getByRole("checkbox", {
    name: "Include src/new name.ts:3",
    exact: true,
  });
  await expect(choice).toBeChecked();
  await choice.uncheck();
  await page.evaluate(() =>
    (window as any).fixture.release("github_pull_request_files"),
  );
  const inline = screen.locator('.diff-annotation [data-thread-id="inline"]');
  await expect(inline).toContainText("Unicode: 空白");
  await expect(choice).not.toBeChecked();
  await expect(other.locator('[data-thread-id="inline"]')).toHaveCount(0);
  await expect(screen.locator('[data-thread-id="inline"]')).toHaveCount(1);
  await screen
    .getByRole("button", {
      name: "Collapse diff for src/new name.ts",
      exact: true,
    })
    .click();
  await screen
    .getByRole("button", {
      name: "Expand diff for src/new name.ts",
      exact: true,
    })
    .click();
  await expect(choice).not.toBeChecked();
  await choice.check();
  await screen
    .getByRole("button", { name: "Address selected comments", exact: true })
    .click();
  const prompt = await page.evaluate(
    () =>
      (window as any).fixture.calls.findLast(
        (call: any) => call.command === "codex_send_prompt",
      ).args.prompt,
  );
  expect(prompt).toContain("Unicode: 空白");
  expect(prompt).toContain("Preserve the return value");
});
