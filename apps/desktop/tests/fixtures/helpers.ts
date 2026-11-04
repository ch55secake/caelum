import { expect, type Page } from "@playwright/test";
import type { QuestionPrompt } from "../../src/domain/sessions/questionState.js";
import { desktop } from "./desktop.js";

export async function installPausedClock(page: Page) {
  const time = new Date("2030-01-01T12:00:00Z");
  // Pause before loading the app, with enough headroom for slow CI calls.
  await page.clock.install({ time: time.getTime() - 60_000 });
  await page.clock.pauseAt(time);
}

export async function emit(
  page: Page,
  method: string,
  params: Record<string, unknown>,
  provider = "codex",
  metadata: { id?: string; created?: number } = {},
) {
  await page.evaluate(
    ({ method, params, provider, metadata }) =>
      (window as any).fixture.emit(
        `${provider}-event`,
        provider === "codex"
          ? { projectPath: "/work/caelum", method, params }
          : {
              projectPath: "/work/caelum",
              eventType: method,
              data: params,
              ...metadata,
            },
      ),
    { method, params, provider, metadata },
  );
}

export async function choose(page: Page, label: string, option: string) {
  await page.getByRole("button", { name: label, exact: true }).click();
  await page
    .getByRole(
      label === "Model" || label === "Default model" ? "button" : "option",
      { name: option, exact: true },
    )
    .click();
}

export async function showPullRequests(page: Page) {
  const section = page.getByRole("region", {
    name: "Pull requests",
    exact: true,
  });
  await expect(section).toBeVisible();
  return section;
}

export async function setHistory(
  page: Page,
  provider: string,
  data: unknown[],
  isWorking: boolean,
) {
  await page.evaluate(
    ({ provider, data, isWorking }) => {
      const fixture = (window as any).fixture;
      if (provider === "codex") fixture.setCodexHistory(data, isWorking);
      else {
        fixture.setOpenHistory(data);
        fixture.setOpenWorking(isWorking);
      }
    },
    { provider, data, isWorking },
  );
}

export function sessionRow(page: Page, title: string) {
  return page
    .locator(".session-row")
    .filter({ has: page.getByRole("button", { name: title, exact: true }) });
}

export async function openSettings(page: Page, section = "Appearance") {
  await page.getByRole("button", { name: "Settings", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Settings", exact: true });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: section, exact: true }).click();
  return dialog;
}

export async function findProjectFile(page: Page, path: string) {
  await page.keyboard.press("Meta+p");
  const search = page.getByRole("combobox", { name: "Find project file" });
  await expect(search).toBeFocused();
  await search.fill(path);
  await expect(
    page.getByRole("option", { name: path, exact: true }),
  ).toBeVisible();
  await search.press("Enter");
  await expect(page.locator(".file-editor-path")).toHaveText(path);
  await expect(page.locator(".code-viewer .cm-editor").first()).toBeVisible();
}

export const attachedContext = (page: Page) =>
  page.locator('.composer [aria-label="Attached context"]');

export async function createCheckout(page: Page, branch = "feature/search") {
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Create worktree");
  const dialog = page.getByRole("dialog", {
    name: "Create worktree",
    exact: true,
  });
  await dialog
    .getByRole("textbox", { name: "New branch", exact: true })
    .fill(branch);
  await dialog
    .getByRole("combobox", { name: "Start from", exact: true })
    .selectOption("release");
  await expect(dialog.locator(".worktree-destination")).toContainText(
    branch.replaceAll("/", "-"),
  );
  const destination = await dialog
    .locator(".worktree-destination")
    .textContent();
  await dialog
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("button", { name: "Session worktree", exact: true }),
  ).toContainText(branch);
  return destination!;
}

export const question = (
  overrides: Partial<QuestionPrompt> = {},
): QuestionPrompt => ({
  provider: "codex",
  projectPath: "/work/caelum",
  sessionId: "thread-a",
  requestId: "41",
  title: "Implementation choices",
  fields: [
    {
      key: "approach",
      type: "string",
      title: "Approach",
      description: "How should we proceed?",
      required: true,
      custom: true,
      options: [
        { value: "Small", label: "Small", description: "Focused change" },
        { value: "Large", label: "Large", description: "Full rewrite" },
      ],
    },
    { key: "detail", type: "string", title: "Details", required: true },
  ],
  ...overrides,
});

export async function setQuestions(
  page: Page,
  requests: QuestionPrompt[],
  notify = true,
) {
  await page.evaluate(
    ({ requests, notify }) =>
      (window as any).fixture.setQuestions(requests, notify),
    { requests, notify },
  );
}

export async function contextMenu(page: Page) {
  const menu = page.getByRole("menu", { name: "Context menu", exact: true });
  await expect(menu).toBeVisible();
  expect(
    await page.evaluate(() => (window as any).fixture.contextPrevented()),
  ).toBe(true);
  const items = await menu
    .locator('[role="menuitem"], [role="separator"]')
    .evaluateAll((nodes) =>
      nodes.map((node) => ({
        text:
          node.getAttribute("role") === "separator"
            ? "Separator"
            : node.textContent!,
        enabled: !(node as HTMLButtonElement).disabled,
      })),
    );
  expect(
    items.some((item) => /inspect|reload|back|forward/i.test(item.text)),
  ).toBe(false);
  return items;
}

export async function pickMenu(page: Page, text: string) {
  await page.getByRole("menuitem", { name: text, exact: true }).click();
  await expect(page.getByRole("menu", { name: "Context menu" })).toHaveCount(0);
}

export async function copiedText(page: Page) {
  return page.evaluate(() => (window as any).fixture.clipboard());
}

export const previewPullRequest = {
  number: 44,
  title: "Improve project navigation",
  url: "https://github.com/example/caelum/pull/44",
  isDraft: false,
  state: "OPEN",
  body: "A **clearer workspace** with independent checkouts.\n\n<!-- template note -->\n## Details\nKeep every task easy to find.",
  author: { login: "oscar" },
  baseRefName: "main",
  headRefName: "feature/navigation",
  headRefOid: "abcdefabcdefabcdefabcdefabcdefabcdefabcd",
  mergeable: "MERGEABLE",
  mergeStateStatus: "CLEAN",
  additions: 128,
  deletions: 42,
  changedFiles: 6,
  reviewDecision: "APPROVED",
  statusCheckRollup: [
    {
      __typename: "CheckRun",
      name: "Tests",
      status: "COMPLETED",
      conclusion: "SUCCESS",
      startedAt: "2026-11-01T12:00:00Z",
      completedAt: "2026-11-01T12:02:14Z",
      detailsUrl: "https://github.com/example/caelum/actions/runs/123",
    },
    {
      __typename: "StatusContext",
      context: "Preview deploy",
      state: "PENDING",
      targetUrl: "https://deploy.example.com/preview/44",
    },
  ],
};

export async function preparePullRequestPreview(page: Page, worktrees = true) {
  await page.clock.install();
  await desktop(page, null, { worktrees });
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", pr);
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.setPullRequestDetails(pr.url, pr);
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  return sessionRow(page, "Refine the session workspace").getByRole("button", {
    name: /Open pull request #44/,
  });
}

export async function preparePullRequestReview(
  page: Page,
  provider: "codex" | "opencode" = "codex",
  selected = true,
) {
  await page.clock.install();
  await desktop(page, selected && provider === "codex" ? "thread-a" : null);
  if (provider === "opencode") {
    await choose(page, "Provider", "OpenCode");
    if (selected)
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
  }
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.setPullRequest("main", pr);
    fixture.setPullRequestDetails(pr.url, { ...pr, changedFiles: 3 });
    const comment = (id: string, body: string) => ({
      id,
      body,
      url: `${pr.url}#discussion_${id}`,
      author: { login: "reviewer" },
      createdAt: "2026-11-01T12:00:00Z",
      state: null,
      diffHunk: null,
    });
    fixture.setPullRequestPage(pr.url, "files", 1, {
      baseOid: "base",
      files: [
        {
          path: "src/new name.ts",
          previousPath: "src/old name.ts",
          status: "renamed",
          additions: 1,
          deletions: 1,
          patch:
            "@@ -1,3 +1,3 @@\n-const oldValue = false;\n+const newValue = true;\n const value = newValue;\n return value;",
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
    fixture.setPullRequestPage(pr.url, "files", 2, {
      baseOid: "base",
      files: [
        {
          path: "README.md",
          previousPath: null,
          status: "modified",
          additions: 1,
          deletions: 0,
          patch: "@@ -1 +1,2 @@\n # Project\n+More documentation",
        },
      ],
      nextPage: null,
    });
    fixture.setPullRequestPage(pr.url, "discussion", "", {
      comments: [comment("discussion", "Please update the documentation.")],
      threads: [],
      nextCursor: "discussion-next",
    });
    fixture.setPullRequestPage(pr.url, "discussion", "discussion-next", {
      comments: [comment("later", "Also include an example.")],
      threads: [],
      nextCursor: null,
    });
    fixture.setPullRequestPage(pr.url, "reviews", "", {
      comments: [
        {
          ...comment("review", "Please handle **empty input**."),
          state: "CHANGES_REQUESTED",
        },
      ],
      threads: [],
      nextCursor: null,
    });
    const thread = {
      id: "inline",
      path: "src/new name.ts",
      line: 3,
      startLine: 2,
      originalLine: 3,
      diffSide: "RIGHT",
      isResolved: false,
      isOutdated: false,
      nextCursor: "replies-next",
      comments: [
        {
          ...comment(
            "inline-first",
            "Preserve the return value.\n```suggestion\nreturn value;\n```",
          ),
          diffHunk: "@@ -2 +2 @@\n-old\n+value",
        },
      ],
    };
    fixture.setPullRequestPage(pr.url, "threads", "", {
      comments: [],
      threads: [
        thread,
        {
          ...thread,
          id: "resolved",
          path: "resolved.ts",
          isResolved: true,
          nextCursor: null,
          comments: [comment("resolved-comment", "Already addressed.")],
        },
        {
          ...thread,
          id: "outdated",
          path: "outdated.ts",
          isOutdated: true,
          nextCursor: null,
          comments: [comment("outdated-comment", "Old implementation.")],
        },
      ],
      nextCursor: null,
    });
    fixture.setPullRequestPage(pr.url, "replies", "replies-next", {
      comments: [
        comment("reply", "This also needs to preserve Unicode: 空白."),
      ],
      threads: [],
      nextCursor: null,
    });
  }, previewPullRequest);
  await page
    .getByRole("button", { name: "Refresh sessions", exact: true })
    .click();
  await (await showPullRequests(page))
    .getByRole("button", { name: /View pull request #44/ })
    .click();
  return page.getByRole("region", { name: "Pull request #44", exact: true });
}
