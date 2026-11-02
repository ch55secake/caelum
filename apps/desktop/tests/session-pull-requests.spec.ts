import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  emit,
  choose,
  showPullRequests,
  sessionRow,
  findProjectFile,
  previewPullRequest,
} from "./fixtures/helpers.js";

test("project sessions discover unopened checkouts and keep navigation, activity, PRs and archives checkout-scoped", async ({
  page,
}) => {
  await desktop(page, "thread-a", { worktrees: true });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Original session draft");
  const path = "/external/linked";
  await page.evaluate(
    ({ path, pr }) => {
      const fixture = (window as any).fixture;
      fixture.addWorktree(path, "feature/linked");
      fixture.setCheckoutSessions(path, "codex", [
        {
          id: "thread-a",
          name: "Linked Codex session",
          preview: "",
          updatedAt: 125,
          isWorking: true,
        },
      ]);
      fixture.setCheckoutSessions(path, "opencode", [
        { id: "ses-a", title: "Linked OpenCode session", updatedAt: 126 },
      ]);
      for (const [provider, id] of [
        ["codex", "thread-a"],
        ["opencode", "ses-a"],
      ])
        fixture.setSessionDetails(
          provider,
          id,
          { branch: "feature/linked", branchIsCurrent: false, changes: null },
          path,
        );
      fixture.setPullRequest("feature/linked", pr, path);
      fixture.setPullRequestDetails(pr.url, pr);
    },
    { path, pr: previewPullRequest },
  );
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  const linked = page.locator(
    `.session-row[data-project-path="${path}"][data-provider="codex"]`,
  );
  const original = page.locator(
    '.session-row[data-project-path="/work/caelum"][data-session-id="thread-a"]',
  );
  await expect(linked).toContainText("Linked Codex session");
  await expect(
    page.getByRole("button", {
      name: "Linked OpenCode session",
      exact: true,
    }),
  ).toBeVisible();
  await expect(original).toHaveClass(/selected/);
  await expect(input).toHaveValue("Original session draft");
  await expect(linked.getByLabel("Running", { exact: true })).toBeVisible();
  await expect(
    linked.getByRole("button", { name: "Archive session", exact: true }),
  ).toBeDisabled();
  expect(
    await page.evaluate((path) => {
      const fixture = (window as any).fixture;
      return fixture.calls.filter(
        (call: any) =>
          [
            "open_worktree",
            "remember_project",
            "sync_provider_sessions",
            "codex_resume_session",
            "opencode_resume_session",
          ].includes(call.command) &&
          (call.args.projectPath === path || call.args.worktreePath === path),
      );
    }, path),
  ).toEqual([]);
  expect(
    await page.evaluate(
      (path) =>
        JSON.parse(sessionStorage.getItem("worktree-workspace")!).projects.some(
          (entry: any) => entry.project.path === path,
        ),
      path,
    ),
  ).toBe(false);
  await page.evaluate(
    (path) =>
      (window as any).fixture.emit("codex-event", {
        projectPath: path,
        method: "turn/completed",
        params: {
          threadId: "thread-a",
          turn: { id: "background", status: "completed" },
        },
      }),
    path,
  );
  await expect(linked.getByLabel("Running", { exact: true })).toHaveCount(0);
  await page.evaluate(
    (path) =>
      (window as any).fixture.emit("approval-requests", [
        {
          provider: "codex",
          projectPath: path,
          sessionId: "thread-a",
          requestId: "approve-linked",
          summary: "Run linked task",
          description: "Run tests",
        },
      ]),
    path,
  );
  await expect(
    linked.getByLabel("Approval required", { exact: true }),
  ).toBeVisible();
  await expect(
    original.getByLabel("Approval required", { exact: true }),
  ).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.emit("approval-requests", []),
  );
  const prs = await showPullRequests(page);
  const pr = prs.getByRole("button", { name: /View pull request #44/ });
  await expect(pr).toHaveAccessibleDescription("#44 · 2 sessions");
  await pr.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(screen).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === "github_pull_request_details",
        ).args.projectPath,
    ),
  ).toBe(path);
  await screen.getByRole("button", { name: "Back to session" }).click();
  await page.keyboard.press("Meta+k");
  const search = page.getByRole("textbox", {
    name: "Search sessions",
    exact: true,
  });
  await search.fill("codex");
  await page.evaluate(() => (window as any).fixture.hold("open_worktree"));
  await linked
    .getByRole("button", { name: "Linked Codex session", exact: true })
    .click();
  await expect
    .poll(() =>
      page.evaluate(
        (path) =>
          (window as any).fixture.calls.some(
            (call: any) =>
              call.command === "open_worktree" &&
              call.args.worktreePath === path,
          ),
        path,
      ),
    )
    .toBe(true);
  await original.locator(".sidebar-row-select").click();
  await page.evaluate(() => (window as any).fixture.release("open_worktree"));
  await expect
    .poll(() =>
      page.evaluate(
        (path) =>
          JSON.parse(
            sessionStorage.getItem("worktree-workspace")!,
          ).projects.some((entry: any) => entry.project.path === path),
        path,
      ),
    )
    .toBe(true);
  await expect(original).toHaveClass(/selected/);
  await expect(input).toHaveValue("Original session draft");
  await linked
    .getByRole("button", { name: "Linked Codex session", exact: true })
    .click();
  await expect(linked).toHaveClass(/selected/);
  await expect(original).not.toHaveClass(/selected/);
  await expect(search).toHaveValue("codex");
  await expect(page.locator(".breadcrumbs")).toContainText("feature/linked");
  await expect(input).toHaveValue("");
  await input.fill("Linked session draft");
  await findProjectFile(page, "src/App.tsx");
  await expect(page.locator(".code-viewer")).toContainText(
    'checkout = "feature/linked"',
  );
  await original.locator(".sidebar-row-select").click();
  await expect(input).toHaveValue("Original session draft");
  await expect(page.locator(".session-row.selected")).toHaveCount(1);
  await linked
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(linked).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === "set_session_archived",
        ).args,
    ),
  ).toMatchObject({
    projectPath: path,
    provider: "codex",
    sessionId: "thread-a",
    archived: true,
  });
  await expect(original).toHaveClass(/selected/);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(linked).toBeVisible();
  await expect(original).toHaveClass(/selected/);
  await linked.locator(".sidebar-row-select").click();
  await expect(input).toHaveValue("Linked session draft");
  await search.fill("/external/linked");
  await expect(page.locator(".session-row")).toHaveCount(2);
  await page.reload();
  await expect(page.locator(".breadcrumbs")).toContainText("feature/linked");
  await expect(linked).toHaveClass(/selected/);
  await expect(original).toBeVisible();
});

test("session PR section groups open sessions and opens an overview without selecting a session", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", pr);
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.setPullRequestDetails(pr.url, pr);
    fixture.setPullRequest("fix/provider-adapters", {
      ...pr,
      number: 45,
      title: "Improve provider adapters",
      url: "https://github.com/example/caelum/pull/45",
      isDraft: true,
    });
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  const section = await showPullRequests(page);
  await expect(section.getByRole("button")).toHaveCount(2);
  const shared = section.getByRole("button", { name: /View pull request #44/ });
  await expect(shared).toHaveAccessibleDescription("#44 · 2 sessions");
  await expect(
    section.getByRole("button", { name: /View draft pull request #45/ }),
  ).toHaveAccessibleDescription("#45 · Review the provider adapters");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft while reviewing PRs");
  await shared.click();
  const screen = page.getByRole("region", {
    name: "Pull request #44",
    exact: true,
  });
  await expect(screen).toContainText("+128");
  await expect(shared).toHaveAttribute("aria-current", "page");
  await expect(page.getByRole("dialog")).toHaveCount(0);
  await screen.getByRole("button", { name: "Back to session" }).click();
  await expect(input).toHaveValue("Keep this draft while reviewing PRs");
  await expect(page.locator(".breadcrumbs")).toContainText("New session");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        ["codex_resume_session", "opencode_resume_session"].includes(
          call.command,
        ),
      ),
    ),
  ).toHaveLength(0);
  await shared.focus();
  await page.clock.runFor(32);
  await expect(shared).toBeFocused();
  await shared.press("Enter");
  await expect(screen).toBeVisible();
  await screen.getByRole("button", { name: "Back to session" }).click();
  await page.keyboard.press("Meta+k");
  await page.getByRole("textbox", { name: "Search sessions" }).fill("no match");
  await expect(page.locator(".session-list")).toContainText(
    "No matching sessions",
  );
  await showPullRequests(page);
  await expect(section.getByRole("button")).toHaveCount(2);
  await page.getByRole("textbox", { name: "Search sessions" }).fill("");
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    await page.screenshot({
      path: `test-results/session-pr-list-${theme}.png`,
    });
  }
  const first = sessionRow(page, "Refine the session workspace");
  await first
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await showPullRequests(page);
  await expect(shared).toHaveAccessibleDescription("#44 · OpenCode session");
  await sessionRow(page, "OpenCode session")
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await showPullRequests(page);
  await expect(shared).toHaveCount(0);
  await expect(
    page
      .locator(".session-row")
      .getByRole("button", { name: /pull request #44/ }),
  ).toHaveCount(0);
  await expect(page.locator(".project-row .branch-pr-trigger")).toHaveCount(0);
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(shared).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", null);
    fixture.setPullRequest("fix/provider-adapters", null);
  });
  await page.clock.runFor(60_100);
  await expect(section.getByRole("button")).toHaveCount(0);
  await expect(section).toContainText("No pull requests for open sessions");
});

test("session PR section discovers off-screen sessions with shared bounded lookups", async ({
  page,
}) => {
  await desktop(page, null);
  await page.setViewportSize({ width: 800, height: 620 });
  const before = await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.hold("github_open_pull_request");
    for (let index = 0; index < 18; index++) {
      fixture.addSession(`background-${index}`, `Background task ${index}`);
      fixture.setSessionDetails("codex", `background-${index}`, {
        branch: `feature/background-${index}`,
        branchIsCurrent: false,
        changes: null,
      });
      if (index === 17)
        fixture.setPullRequest(`feature/background-${index}`, pr);
    }
    return fixture.calls.length;
  }, previewPullRequest);
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await expect
    .poll(() =>
      page.evaluate(
        (before) =>
          (window as any).fixture.calls
            .slice(before)
            .filter(
              (call: any) =>
                call.command === "session_details" &&
                call.args.sessionId === "background-17",
            ).length,
        before,
      ),
    )
    .toBe(1);
  const lookups = await page.evaluate(
    (before) =>
      (window as any).fixture.calls
        .slice(before)
        .filter((call: any) => call.command === "github_open_pull_request"),
    before,
  );
  expect(lookups.length).toBeGreaterThan(0);
  expect(lookups.length).toBeLessThanOrEqual(4);
  await page.evaluate(() =>
    (window as any).fixture.release("github_open_pull_request"),
  );
  const section = await showPullRequests(page);
  const row = section.getByRole("button", { name: /View pull request #44/ });
  await expect(row).toBeVisible();
  await expect(row).toHaveAccessibleDescription("#44 · Background task 17");
  await expect(sessionRow(page, "Background task 17")).not.toBeInViewport();
  await expect(row).toBeInViewport({ ratio: 1 });
  const calls = await page.evaluate(
    (before) => (window as any).fixture.calls.slice(before),
    before,
  );
  expect(
    calls.filter(
      (call: any) =>
        call.command === "github_open_pull_request" &&
        call.args.branch === "feature/background-17",
    ),
  ).toHaveLength(1);
  expect(
    calls.filter((call: any) => call.command === "codex_resume_session"),
  ).toHaveLength(0);
  expect(
    await section.evaluate((node) => node.scrollWidth <= node.clientWidth),
  ).toBe(true);
  await page.screenshot({
    path: "test-results/session-pr-list-many-sessions.png",
  });
});

test("session PR section ignores delayed results from a different project", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.hold(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    );
    fixture.setPullRequest(
      "other-project",
      {
        ...pr,
        url: "https://github.com/example/other/pull/44",
        title: "Other project changes",
      },
      "/work/other",
    );
  }, previewPullRequest);
  await page.clock.runFor(60_100);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const section = await showPullRequests(page);
  await expect(
    section.getByRole("button", { name: /Other project changes/ }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.release(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    ),
  );
  await expect(section.getByRole("button")).toHaveCount(1);
  await expect(section).not.toContainText(previewPullRequest.title);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await showPullRequests(page);
  await expect(
    section.getByRole("button", { name: /Improve project navigation/ }),
  ).toBeVisible();
  await expect(section).not.toContainText("Other project changes");
});

test("session PR indicators share lookups, distinguish drafts and preserve matches through unavailable lookups", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  await page.getByRole("button", { name: /New session/ }).click();
  await choose(page, "Provider", "OpenCode");
  const project = page
    .getByRole("navigation", { name: "Open projects" })
    .getByRole("button", { name: "caelum", exact: true })
    .locator("..");
  const session = sessionRow(page, "Refine the session workspace");
  const openCode = sessionRow(page, "OpenCode session");
  await expect(session.locator(".session-branch")).toHaveText(
    "feat/session-ui-redesign",
  );
  await expect(openCode.locator(".session-branch")).toHaveText("main");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", {
      number: 11,
      title: "Main changes",
      url: "https://github.com/example/repo/pull/11",
      isDraft: false,
    });
    fixture.setPullRequest("feat/session-ui-redesign", {
      number: 40,
      title: "Session UI",
      url: "https://github.com/example/repo/pull/40",
      isDraft: true,
    });
  });
  await page.clock.runFor(60_100);
  await expect(page.locator(".project-row .branch-pr-trigger")).toHaveCount(0);
  await expect(
    openCode.getByRole("button", {
      name: "Open pull request #11: Main changes",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    session.getByRole("button", {
      name: "Draft pull request #40: Session UI",
      exact: true,
    }),
  ).toHaveAttribute("aria-haspopup", "dialog");
  await expect(
    sessionRow(page, "Review the provider adapters").locator(".branch-pr"),
  ).toBeEmpty();
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.command === "github_open_pull_request" &&
          call.args.projectPath === "/work/caelum" &&
          call.args.branch === "main",
      ),
    ),
  ).toHaveLength(2);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("main", null);
    fixture.setPullRequest("feat/session-ui-redesign", {
      number: 40,
      title: "Ready for review",
      url: "https://github.com/example/repo/pull/40",
      isDraft: false,
    });
  });
  await page.clock.runFor(60_100);
  await expect(openCode.locator(".branch-pr")).toBeEmpty();
  await expect(
    session.getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    }),
  ).toBeVisible();
  await page.setViewportSize({ width: 800, height: 620 });
  await session
    .getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    })
    .scrollIntoViewIfNeeded();
  await expect(
    session.getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    }),
  ).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/session-pr-indicators.png" });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "github_open_pull_request",
      "GitHub CLI unavailable",
    ),
  );
  await page.clock.runFor(60_100);
  await expect(
    session.getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    }),
  ).toBeVisible();
  const section = await showPullRequests(page);
  await expect(section.getByRole("status")).toContainText(
    "Showing last known results",
  );
  await expect(project.locator(".project-changes")).toHaveText("2 changed");
  await expect(
    page.getByRole("alert").filter({ hasText: "GitHub CLI unavailable" }),
  ).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("github_open_pull_request", null),
  );
  await section.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(section.getByRole("status")).toHaveCount(0);
  await expect(
    session.getByRole("button", {
      name: "Open pull request #40: Ready for review",
      exact: true,
    }),
  ).toBeVisible();
});

test("session PR refresh bypasses cached misses for both providers and preserves results during outages", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.setPullRequest("main", {
      ...pr,
      number: 45,
      url: pr.url.replace("44", "45"),
      title: "OpenCode changes",
    });
  }, previewPullRequest);
  const refresh = page.getByRole("button", {
    name: "Refresh sessions",
    exact: true,
  });
  await refresh.click();
  const section = await showPullRequests(page);
  await expect(section.locator(".session-pr-item")).toHaveCount(2);
  const codex = sessionRow(page, "Refine the session workspace").getByRole(
    "button",
    { name: /pull request #44/ },
  );
  const opencode = sessionRow(page, "OpenCode session").getByRole("button", {
    name: /pull request #45/,
  });
  await expect(codex).toBeVisible();
  await expect(opencode).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_open_pull_request", "Network unavailable");
    fixture.hold("github_open_pull_request");
  });
  await refresh.click();
  await expect(codex).toBeVisible();
  await expect(opencode).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.release("github_open_pull_request"),
  );
  await showPullRequests(page);
  await expect(section.getByRole("status")).toContainText(
    "Showing last known results",
  );
  await expect(section).not.toContainText("No pull requests for open sessions");
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await page.clock.runFor(60_100);
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(codex).toBeVisible();
  await expect(opencode).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("github_open_pull_request", null);
    fixture.setPullRequest("feat/session-ui-redesign", null);
  });
  await page.clock.runFor(15_100);
  await showPullRequests(page);
  await expect(section.getByRole("status")).toHaveCount(0);
  await expect(codex).toHaveCount(0);
  await expect(opencode).toBeVisible();
  await showPullRequests(page);
  await expect(section.locator(".session-pr-item")).toHaveCount(1);
});

test("session PR explicit refresh discards older in-flight discovery and picks up changed branch metadata", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page, null);
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.hold(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    );
  }, previewPullRequest);
  await page.clock.runFor(60_100);
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
    .toBe(2);
  await page.evaluate((pr) => {
    (window as any).fixture.setPullRequest("feat/session-ui-redesign", {
      ...pr,
      number: 45,
      url: pr.url.replace("44", "45"),
      title: "Fresh pull request",
    });
    (window as any).fixture.hold("session_details");
  }, previewPullRequest);
  const refresh = page.getByRole("button", {
    name: "Refresh sessions",
    exact: true,
  });
  await refresh.click();
  await expect(refresh).toBeDisabled();
  await page.evaluate(() => {
    const command =
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign";
    (window as any).fixture.release(command);
    (window as any).fixture.hold(command);
  });
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
  const section = await showPullRequests(page);
  await expect(
    section.getByRole("button", { name: /View pull request #44/ }),
  ).toHaveCount(0);
  await page.evaluate(() => {
    (window as any).fixture.release(
      "github_open_pull_request:/work/caelum:feat/session-ui-redesign",
    );
    (window as any).fixture.release("session_details");
  });
  await expect(
    section.getByRole("button", { name: /View pull request #45/ }),
  ).toBeVisible();
  await expect(refresh).toBeEnabled();
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setSessionDetails("codex", "thread-a", {
      branch: "feature/new",
      branchIsCurrent: false,
      changes: null,
    });
    fixture.setPullRequest("feature/new", {
      ...pr,
      number: 46,
      url: pr.url.replace("44", "46"),
      title: "New branch",
    });
  }, previewPullRequest);
  await refresh.click();
  await expect(
    section.getByRole("button", { name: /View pull request #46/ }),
  ).toBeVisible();
  await expect(
    section.getByRole("button", { name: /View pull request #45/ }),
  ).toHaveCount(0);
});

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} session keeps every PR in a branch stack through live updates, archive and merge`, async ({
    page,
  }) => {
    await page.clock.install();
    await desktop(page, null);
    const id = provider === "codex" ? "thread-a" : "ses-a";
    const title =
      provider === "codex"
        ? "Refine the session workspace"
        : "OpenCode session";
    const branches = [
      "stack/first",
      "stack/alias",
      "stack/second",
      "stack/third",
    ];
    await page.evaluate(
      ({ provider, id, branches, pr }) => {
        const fixture = (window as any).fixture;
        // A restored session has already moved past its first PR branch.
        fixture.setSessionDetails(provider, id, {
          branch: "stack/second",
          branches,
          branchIsCurrent: false,
          changes: null,
        });
        fixture.setPullRequest("stack/first", pr);
        fixture.setPullRequest("stack/alias", pr);
        const second = {
          ...pr,
          number: 45,
          title: "Second stacked PR",
          url: pr.url.replace("44", "45"),
        };
        fixture.setPullRequest("stack/second", second);
        fixture.setPullRequestDetails(second.url, second);
      },
      { provider, id, branches, pr: previewPullRequest },
    );
    const refresh = page.getByRole("button", {
      name: "Refresh sessions",
      exact: true,
    });
    await refresh.click();
    const section = await showPullRequests(page);
    const first = section.getByRole("button", {
      name: /View pull request #44/,
    });
    const second = section.getByRole("button", {
      name: /View pull request #45/,
    });
    const third = section.getByRole("button", {
      name: /View draft pull request #46/,
    });
    await expect(section.locator(".session-pr-item")).toHaveCount(2);
    await expect(first).toHaveAccessibleDescription(`#44 · ${title}`);
    await expect(second).toHaveAccessibleDescription(`#45 · ${title}`);
    await expect
      .poll(() =>
        page.evaluate(() =>
          (window as any).fixture.calls.some(
            (call: any) =>
              call.command === "github_open_pull_request" &&
              call.args.branch === "stack/third",
          ),
        ),
      )
      .toBe(true);

    await page.evaluate(
      ({ provider, id, branches, pr }) => {
        const fixture = (window as any).fixture;
        // Create a PR on an earlier branch, then move on before the turn ends.
        fixture.setPullRequest("stack/third", {
          ...pr,
          number: 46,
          title: "Third stacked PR",
          url: pr.url.replace("44", "46"),
          isDraft: true,
        });
        const details = {
          branch: "stack/next",
          branches: [...branches, "stack/next"],
          branchIsCurrent: false,
          changes: null,
        };
        fixture.setSessionDetails(provider, id, details);
        fixture.emit("session-branch-updated", {
          projectPath: "/work/caelum",
          provider,
          sessionId: id,
          branch: details.branch,
          branches: details.branches,
        });
      },
      { provider, id, branches, pr: previewPullRequest },
    );
    await expect(sessionRow(page, title).locator(".session-branch")).toHaveText(
      "stack/next",
    );
    await expect(third).toHaveAccessibleDescription(`#46 · ${title}`);
    await expect(section.locator(".session-pr-item")).toHaveCount(3);

    await page.evaluate(
      ({ provider, id, branches, pr }) => {
        const fixture = (window as any).fixture;
        fixture.setPullRequest("fix/provider-adapters", pr);
        fixture.setSessionDetails(provider, id, {
          branch: null,
          branches,
          branchIsCurrent: false,
          changes: null,
        });
        fixture.emit("session-branch-updated", {
          projectPath: "/work/caelum",
          provider,
          sessionId: id,
          branch: null,
          branches,
        });
      },
      {
        provider,
        id,
        branches: [...branches, "stack/next"],
        pr: previewPullRequest,
      },
    );
    await refresh.click();
    await expect(
      sessionRow(page, title).locator(".session-branch"),
    ).toHaveCount(0);
    await expect(first).toHaveAccessibleDescription("#44 · 2 sessions");
    await expect(section.locator(".session-pr-item")).toHaveCount(3);
    await sessionRow(page, title)
      .getByRole("button", { name: "Archive session", exact: true })
      .click();
    await showPullRequests(page);
    await expect(first).toHaveAccessibleDescription(
      "#44 · Review the provider adapters",
    );
    await expect(section.locator(".session-pr-item")).toHaveCount(1);
    await page.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(section.locator(".session-pr-item")).toHaveCount(3);
    await expect(first).toHaveAccessibleDescription("#44 · 2 sessions");

    await second.click();
    const screen = page.getByRole("region", {
      name: "Pull request #45",
      exact: true,
    });
    await screen
      .getByRole("region", { name: "Merge pull request", exact: true })
      .getByRole("button", { name: "Merge pull request", exact: true })
      .click();
    await expect(second).toHaveCount(0);
    await expect(first).toBeVisible();
    await expect(third).toBeVisible();
    await page.evaluate(() => {
      for (const branch of [
        "stack/first",
        "stack/alias",
        "fix/provider-adapters",
      ])
        (window as any).fixture.setPullRequest(branch, null);
    });
    await page.clock.runFor(60_100);
    await expect(first).toHaveCount(0);
    await expect(section.locator(".session-pr-item")).toHaveCount(1);
    await expect(third).toBeVisible();
  });
}

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} live Git branch updates bypass slow metadata and refresh PR discovery during execution`, async ({
    page,
  }) => {
    await page.clock.install();
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    const id = provider === "codex" ? "thread-a" : "ses-a";
    const row = sessionRow(
      page,
      provider === "codex"
        ? "Refine the session workspace"
        : "OpenCode session",
    );
    await expect(row.locator(".session-branch")).toHaveText(
      provider === "codex" ? "feat/session-ui-redesign" : "main",
    );
    const reads = () =>
      page.evaluate(
        ({ provider, id }) =>
          (window as any).fixture.calls.filter(
            (call: any) =>
              call.command === "session_details" &&
              call.args.provider === provider &&
              call.args.sessionId === id,
          ).length,
        { provider, id },
      );
    const before = await reads();
    await page.evaluate(() => (window as any).fixture.hold("session_details"));
    if (provider === "codex")
      await emit(page, "turn/started", {
        threadId: id,
        turn: { id: "live-turn" },
      });
    else
      await emit(
        page,
        "session.execution.started",
        { sessionID: id },
        provider,
      );
    await expect.poll(reads).toBeGreaterThan(before);
    await page.evaluate(
      ({ provider, id, pr }) => {
        const fixture = (window as any).fixture;
        fixture.setSessionDetails(provider, id, {
          branch: "feature/live",
          branchIsCurrent: false,
          changes: { additions: 9, deletions: 2 },
        });
        fixture.setPullRequest("feature/live", pr);
        fixture.emit("session-branch-updated", {
          projectPath: "/work/caelum",
          provider,
          sessionId: id,
          branch: "feature/live",
        });
      },
      { provider, id, pr: previewPullRequest },
    );
    await expect(row.locator(".session-branch")).toHaveText("feature/live");
    await expect(
      row.getByRole("button", { name: /pull request #44/ }),
    ).toBeVisible();
    await expect(
      sessionRow(page, "Review the provider adapters").locator(
        ".session-branch",
      ),
    ).toHaveText("fix/provider-adapters");
    await page.evaluate(() =>
      (window as any).fixture.release("session_details"),
    );
    await expect.poll(reads).toBeGreaterThan(before + 1);
    await expect(row.locator(".lines-added")).toHaveText("+9");
    await expect(row.locator(".session-branch")).toHaveText("feature/live");
    await page.evaluate(
      ({ provider, id }) =>
        (window as any).fixture.emit("session-branch-updated", {
          projectPath: "/work/caelum",
          provider,
          sessionId: id,
          branch: null,
        }),
      { provider, id },
    );
    await expect(row.locator(".session-branch")).toHaveCount(0);
    await expect(
      row.getByRole("button", { name: /pull request #44/ }),
    ).toHaveCount(0);
    await expect(row.locator(".lines-added")).toHaveText("+9");
  });
}

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} session PR discovery refreshes immediately when execution finishes`, async ({
    page,
  }) => {
    await page.clock.install();
    await desktop(page);
    if (provider === "opencode") {
      await page.getByRole("button", { name: /^New session/ }).click();
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
    }
    const branch = provider === "codex" ? "feat/session-ui-redesign" : "main";
    await expect
      .poll(() =>
        page.evaluate(
          (branch) =>
            (window as any).fixture.calls.some(
              (call: any) =>
                call.command === "github_open_pull_request" &&
                call.args.branch === branch,
            ),
          branch,
        ),
      )
      .toBe(true);
    await page.evaluate(
      ({ pr, branch }) => (window as any).fixture.setPullRequest(branch, pr),
      { pr: previewPullRequest, branch },
    );
    if (provider === "codex")
      await emit(page, "turn/completed", {
        threadId: "thread-a",
        turn: { id: "turn-1", status: "completed" },
      });
    else
      await emit(
        page,
        "session.execution.succeeded",
        { sessionID: "ses-a" },
        "opencode",
      );
    await expect(
      (await showPullRequests(page)).getByRole("button", {
        name: /View pull request #44/,
      }),
    ).toBeVisible();
  });
}

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} session PR discovery waits for fresh metadata when execution overlaps a read`, async ({
    page,
  }) => {
    await page.clock.install();
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    const id = provider === "codex" ? "thread-a" : "ses-a";
    const row = sessionRow(
      page,
      provider === "codex"
        ? "Refine the session workspace"
        : "OpenCode session",
    );
    await expect(row.locator(".session-branch")).toHaveText(
      provider === "codex" ? "feat/session-ui-redesign" : "main",
    );
    const branch = "fix/provider-adapters";
    await expect
      .poll(() =>
        page.evaluate(
          (branch) =>
            (window as any).fixture.calls.some(
              (call: any) =>
                call.command === "github_open_pull_request" &&
                call.args.branch === branch,
            ),
          branch,
        ),
      )
      .toBe(true);
    const reads = await page.evaluate(
      ({ provider, id }) =>
        (window as any).fixture.calls.filter(
          (call: any) =>
            call.command === "session_details" &&
            call.args.provider === provider &&
            call.args.sessionId === id,
        ).length,
      { provider, id },
    );
    await page.evaluate(() => (window as any).fixture.hold("session_details"));
    if (provider === "codex")
      await emit(page, "turn/started", {
        threadId: id,
        turn: { id: "turn-1" },
      });
    else
      await emit(
        page,
        "session.execution.started",
        { sessionID: id },
        "opencode",
      );
    await expect
      .poll(() =>
        page.evaluate(
          ({ provider, id }) =>
            (window as any).fixture.calls.filter(
              (call: any) =>
                call.command === "session_details" &&
                call.args.provider === provider &&
                call.args.sessionId === id,
            ).length,
          { provider, id },
        ),
      )
      .toBe(reads + 1);
    await page.evaluate(
      ({ provider, id, branch, pr }) => {
        (window as any).fixture.setSessionDetails(provider, id, {
          branch,
          branchIsCurrent: false,
          changes: null,
        });
        (window as any).fixture.setPullRequest(branch, pr);
      },
      { provider, id, branch, pr: previewPullRequest },
    );
    if (provider === "codex")
      await emit(page, "turn/completed", {
        threadId: id,
        turn: { id: "turn-1", status: "completed" },
      });
    else
      await emit(
        page,
        "session.execution.succeeded",
        { sessionID: id },
        "opencode",
      );
    await page.evaluate(() =>
      (window as any).fixture.release("session_details"),
    );
    await expect(row.locator(".session-branch")).toHaveText(branch);
    await expect(
      row.getByRole("button", { name: /pull request #44/ }),
    ).toBeVisible();
    await expect(
      (await showPullRequests(page)).getByRole("button", {
        name: /View pull request #44/,
      }),
    ).toContainText("2 sessions");
  });
}

test("session PR indicators stay scoped through branch and project changes and delayed responses", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const projects = page.getByRole("navigation", { name: "Open projects" });
  const current = sessionRow(page, "Refine the session workspace");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setSessionDetails("codex", "thread-a", {
      branch: "feat/shared",
      branchIsCurrent: false,
      changes: null,
    });
    fixture.setPullRequest("feat/shared", {
      number: 10,
      title: "Old branch",
      url: "https://github.com/example/caelum/pull/10",
      isDraft: false,
    });
    fixture.hold("github_open_pull_request:/work/caelum:feat/shared");
  });
  await page
    .getByRole("button", { name: "Refresh sessions", exact: true })
    .click();
  await expect(current.locator(".session-branch")).toHaveText("feat/shared");
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).fixture.calls.some(
          (call: any) =>
            call.command === "github_open_pull_request" &&
            call.args.branch === "feat/shared",
        ),
      ),
    )
    .toBe(true);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setSessionDetails("codex", "thread-a", {
      branch: "feat/current",
      branchIsCurrent: false,
      changes: null,
    });
    fixture.setPullRequest("feat/current", {
      number: 20,
      title: "Current branch",
      url: "https://github.com/example/caelum/pull/20",
      isDraft: false,
    });
  });
  await page
    .getByRole("button", { name: "Refresh sessions", exact: true })
    .click();
  await expect(
    current.getByRole("button", {
      name: "Open pull request #20: Current branch",
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.release("github_open_pull_request:/work/caelum:feat/shared");
    fixture.setPullRequest(
      "feat/current",
      {
        number: 30,
        title: "Other repository",
        url: "https://github.com/example/other/pull/30",
        isDraft: false,
      },
      "/work/other",
    );
  });
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(
    current.getByRole("button", {
      name: "Open pull request #30: Other repository",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    current.getByRole("button", { name: /#20: Current branch/ }),
  ).toHaveCount(0);
  await projects.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(
    current.getByRole("button", {
      name: "Open pull request #20: Current branch",
      exact: true,
    }),
  ).toBeVisible();
  await expect(current.getByRole("button", { name: /Old branch/ })).toHaveCount(
    0,
  );
  await expect(
    current.getByRole("button", { name: /#30: Other repository/ }),
  ).toHaveCount(0);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setSessionDetails("codex", "thread-a", {
      branch: null,
      branchIsCurrent: false,
      changes: null,
    });
  });
  await page
    .getByRole("button", { name: "Refresh sessions", exact: true })
    .click();
  await expect(current.locator(".session-branch")).toHaveCount(0);
  await expect(current.locator(".branch-pr-trigger")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.command === "github_open_pull_request" && !call.args.branch,
      ),
    ),
  ).toHaveLength(0);
});
