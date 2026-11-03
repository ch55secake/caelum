import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  emit,
  showPullRequests,
  sessionRow,
  previewPullRequest,
} from "./fixtures/helpers.js";

test("sidebar metadata and archiving stay scoped when projects reuse a session ID", async ({
  page,
}) => {
  await desktop(page);
  const row = sessionRow(page, "Refine the session workspace");
  await expect(row.locator(".session-branch")).toHaveText(
    "feat/session-ui-redesign",
  );
  await page.evaluate(() => (window as any).fixture.hold("session_details"));
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "turn-1", status: "completed" },
  });
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await page.evaluate(() => (window as any).fixture.release("session_details"));
  await expect(row.locator(".session-branch")).toHaveText("other-project");
  await row.hover();
  await row
    .getByRole("button", { name: "Archive session", exact: true })
    .click();
  await expect(row).toHaveCount(0);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(row.locator(".session-branch")).toHaveText(
    "feat/session-ui-redesign",
  );
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(
    page.getByRole("status", { name: "Session archived" }),
  ).toHaveCount(0);
  await page.getByRole("button", { name: "other", exact: true }).click();
  await expect(row.locator(".session-branch")).toHaveText("other-project");
  const calls = await page.evaluate(() =>
    (window as any).fixture.calls.filter(
      (call: any) => call.command === "set_session_archived",
    ),
  );
  expect(calls.map((call: any) => call.args.projectPath)).toEqual([
    "/work/other",
    "/work/other",
  ]);
});

test("sidebar shows project details and stacked activity lists with independent scrolling and project search", async ({
  page,
}) => {
  await desktop(page, "thread-a", { worktrees: true });
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Project A draft");
  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.addWorktree("/work/sidebar", "feature/sidebar");
    for (let index = 0; index < 24; index++) {
      fixture.addSession(`sidebar-${index}`, `Background task ${index}`);
      fixture.setSessionDetails("codex", `sidebar-${index}`, {
        branch: `feature/task-${index}`,
        branchIsCurrent: false,
        changes: null,
      });
      fixture.setPullRequest(`feature/task-${index}`, {
        ...pr,
        number: 100 + index,
        title: `Task ${index}`,
        url: `https://github.com/example/caelum/pull/${100 + index}`,
      });
    }
  }, previewPullRequest);
  await page
    .getByRole("button", { name: "Refresh sessions", exact: true })
    .click();
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  const disclosure = page.getByRole("button", {
    name: "Worktrees for caelum",
    exact: true,
  });
  await expect(disclosure).toHaveAttribute("aria-expanded", "true");
  await disclosure.focus();
  await disclosure.press("Enter");
  await expect(disclosure).toHaveAttribute("aria-expanded", "false");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await message.fill("Project B draft");
  const projects = page.getByRole("navigation", { name: "Open projects" });
  const first = projects.getByRole("button", { name: "caelum", exact: true });
  const second = projects.getByRole("button", { name: "other", exact: true });
  await expect(first).toBeInViewport({ ratio: 1 });
  await expect(second).toBeInViewport({ ratio: 1 });
  await expect(
    projects.locator(".project-row").first().locator(".project-branch"),
  ).toBeVisible();
  await expect(
    projects.locator(".project-row").first().locator(".project-changes"),
  ).toBeVisible();
  await first.focus();
  await expect(page.locator(".sidebar-details-tooltip")).toContainText(
    "/work/caelum",
  );
  await first.press("Escape");
  await expect(page.locator(".sidebar-details-tooltip")).toHaveCount(0);
  await first.press("Enter");
  await expect(message).toHaveValue("Project A draft");
  await expect(disclosure).toHaveAttribute("aria-expanded", "false");
  const sessionSection = page.getByRole("region", {
    name: "Sessions",
    exact: true,
  });
  const requestSection = await showPullRequests(page);
  await expect(requestSection.locator(".sidebar-section-count")).toHaveText(
    "24",
  );
  const sessions = sessionSection.locator(".session-list");
  const requests = requestSection.locator(".sidebar-request-panel");
  expect((await requestSection.boundingBox())!.y).toBeGreaterThan(
    (await sessions.boundingBox())!.y,
  );
  await expect(
    sessionSection.getByRole("heading", { name: "Sessions", exact: true }),
  ).toBeInViewport();
  await expect(
    requestSection.getByRole("heading", { name: "Pull requests", exact: true }),
  ).toBeInViewport();
  await sessions.evaluate((node) => (node.scrollTop = 500));
  await expect
    .poll(() => sessions.evaluate((node) => node.scrollTop))
    .toBe(500);
  await requests.evaluate((node) => (node.scrollTop = 300));
  await expect
    .poll(() => requests.evaluate((node) => node.scrollTop))
    .toBe(300);
  await expect
    .poll(() => sessions.evaluate((node) => node.scrollTop))
    .toBe(500);
  await disclosure.click();
  await page
    .getByRole("button", {
      name: "Switch to worktree feature/sidebar",
      exact: true,
    })
    .click();
  await expect(page.locator(".breadcrumbs")).toContainText("feature/sidebar");
  await expect(sessions.locator(".session-row")).toHaveCount(26);
  await expect
    .poll(() => sessions.evaluate((node) => node.scrollTop))
    .toBe(500);
  await expect
    .poll(() => requests.evaluate((node) => node.scrollTop))
    .toBe(300);
  await first.click();
  await expect(message).toHaveValue("Project A draft");
  await disclosure.click();
  await page.keyboard.press("Meta+k");
  const search = page.getByRole("textbox", {
    name: "Search sessions",
    exact: true,
  });
  await expect(search).toBeFocused();
  await search.fill("Background task 2");
  await expect(sessions.locator(".session-row")).toHaveCount(5);
  await expect
    .poll(() => requests.evaluate((node) => node.scrollTop))
    .toBe(300);
  await second.click();
  await expect(sessionSection).toBeVisible();
  await expect(requestSection).toBeVisible();
  await expect(message).toHaveValue("Project B draft");
  await page.keyboard.press("Meta+k");
  await search.fill("adapters");
  await first.click();
  await expect
    .poll(() => requests.evaluate((node) => node.scrollTop))
    .toBe(300);
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await page.getByRole("button", { name: "Expand sidebar" }).click();
  await expect(disclosure).toHaveAttribute("aria-expanded", "false");
  await expect
    .poll(() => requests.evaluate((node) => node.scrollTop))
    .toBe(300);
  await page.keyboard.press("Meta+k");
  await expect(search).toHaveValue("Background task 2");
  await search.press("Escape");
  await expect(search).toBeHidden();
  await expect(
    page
      .locator(".sidebar")
      .getByRole("button", { name: "Search sessions", exact: true }),
  ).toBeFocused();
  await expect(sessions.locator(".session-row")).toHaveCount(26);
  await disclosure.click();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree feature/sidebar",
      exact: true,
    }),
  ).toBeVisible();
  await disclosure.click();
  await requests.evaluate((node) => {
    node.scrollTop = 0;
  });
  for (const theme of ["dark", "light"]) {
    await page.evaluate(
      (theme) => (document.documentElement.dataset.theme = theme),
      theme,
    );
    await page.screenshot({
      path: `test-results/stacked-sidebar-${theme}.png`,
    });
  }
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(first).toBeInViewport({ ratio: 1 });
  await expect(second).toBeInViewport({ ratio: 1 });
  await expect(
    page.getByRole("button", { name: "Settings", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(await sessions.evaluate((node) => node.clientHeight)).toBeGreaterThan(
    80,
  );
  expect(await requests.evaluate((node) => node.clientHeight)).toBeGreaterThan(
    60,
  );
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/stacked-sidebar-small.png" });
});

test("project subtitles restore selected sessions across providers without resuming background conversations", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  await page.evaluate(() => {
    sessionStorage.setItem(
      "worktree-workspace",
      JSON.stringify({
        activeProjectPath: "/work/other",
        projects: [
          {
            project: { name: "caelum", path: "/work/caelum" },
            provider: "codex",
            codexSessionId: "thread-b",
            opencodeSessionId: null,
            codexSessionIds: ["thread-a", "thread-b"],
            opencodeSessionIds: [],
          },
          {
            project: { name: "other", path: "/work/other" },
            provider: "opencode",
            codexSessionId: null,
            opencodeSessionId: "ses-a",
            codexSessionIds: [],
            opencodeSessionIds: ["ses-a"],
          },
        ],
      }),
    );
  });
  await page.reload();
  const projects = page.getByRole("navigation", { name: "Open projects" });
  const first = projects.getByRole("button", { name: "caelum", exact: true });
  const second = projects.getByRole("button", { name: "other", exact: true });
  const firstDetails = first.locator("../..");
  const secondDetails = second.locator("../..");
  await expect(firstDetails.locator(".project-session")).toHaveText(
    "Review the provider adapters",
  );
  await expect(secondDetails.locator(".project-session")).toHaveText(
    "OpenCode session",
  );
  await expect(second).toHaveAttribute("aria-current", "page");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.args.projectPath === "/work/caelum" &&
          ["codex_resume_session", "codex_session_history"].includes(
            call.command,
          ),
      ),
    ),
  ).toHaveLength(0);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.emit("codex-event", {
      projectPath: "/work/caelum",
      method: "thread/name/updated",
      params: {
        threadId: "thread-b",
        threadName: "Review deployment adapters",
      },
    });
    fixture.emit("opencode-event", {
      projectPath: "/work/other",
      eventType: "session.renamed",
      data: { sessionID: "ses-a", title: "Improve the release workflow" },
    });
  });
  await expect(firstDetails.locator(".project-session")).toHaveText(
    "Review deployment adapters",
  );
  await expect(secondDetails.locator(".project-session")).toHaveText(
    "Improve the release workflow",
  );
  await page
    .locator(".sidebar")
    .screenshot({ path: "test-results/project-subtitles.png" });
  await first.click();
  await expect(sessionRow(page, "Review deployment adapters")).toHaveClass(
    /selected/,
  );
  await sessionRow(page, "Refine the session workspace")
    .getByRole("button", {
      name: "Refine the session workspace",
      exact: true,
    })
    .click();
  await emit(page, "thread/name/updated", {
    threadId: "thread-b",
    threadName: "Background session renamed again",
  });
  await second.click();
  await expect(firstDetails.locator(".project-session")).toHaveText(
    "Refine the session workspace",
  );
  await expect(secondDetails.locator(".project-session")).toHaveText(
    "Improve the release workflow",
  );
  await page.mouse.move(700, 50);
  await first.focus();
  await expect(page.locator(".sidebar-details-tooltip")).toContainText(
    "Refine the session workspace",
  );
  await expect(first).toHaveAccessibleDescription(
    /Refine the session workspace/,
  );
  const divider = page.getByRole("separator", { name: "Resize sidebar" });
  await divider.focus();
  await divider.press("Home");
  await page
    .locator(".sidebar")
    .screenshot({ path: "test-results/project-subtitles-narrow.png" });
});

test("sidebar resizes, retains its preferred width and keeps session details usable at the minimum window size", async ({
  page,
}) => {
  await desktop(page);
  const sidebar = page.getByRole("complementary", {
    name: "Projects and sessions",
  });
  const divider = page.getByRole("separator", { name: "Resize sidebar" });
  const width = async () => (await sidebar.boundingBox())!.width;
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my sidebar draft");
  await page
    .locator(".transcript")
    .evaluate((node) => node.setAttribute("data-mounted", "yes"));
  await divider.focus();
  await divider.press("Home");
  await expect.poll(width).toBe(180);
  await divider.press("ArrowRight");
  await expect.poll(width).toBe(212);
  await divider.press("ArrowLeft");
  await expect.poll(width).toBe(180);
  await divider.press("End");
  await expect.poll(width).toBe(480);
  const handle = (await divider.boundingBox())!;
  await page.mouse.move(handle.x + handle.width / 2, handle.y + 100);
  await page.mouse.down();
  await page.mouse.move(handle.x + handle.width / 2 - 100, handle.y + 100);
  await page.mouse.up();
  await expect.poll(width).toBe(380);
  const preferredWidth = await width();
  await expect(divider).toHaveAttribute(
    "aria-valuenow",
    String(preferredWidth),
  );
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await expect(divider).toHaveCount(0);
  await page.keyboard.press("Meta+\\");
  await expect.poll(width).toBe(preferredWidth);
  await expect(input).toHaveValue("Keep my sidebar draft");
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  const row = sessionRow(page, "Refine the session workspace");
  await expect(row.locator(".lines-added")).toHaveText("+42");
  await page.keyboard.press("Meta+k");
  await page
    .getByRole("textbox", { name: "Search sessions" })
    .fill("fix/provider-adapters");
  await expect(page.locator(".session-item")).toHaveCount(1);
  await page.getByRole("textbox", { name: "Search sessions" }).fill("");
  await page.evaluate(() =>
    (window as any).fixture.setSessionDetails("codex", "thread-a", {
      branch: "feature/a-very-long-branch-name-for-the-sidebar",
      branchIsCurrent: false,
      changes: { additions: 1234, deletions: 56 },
    }),
  );
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "turn-new", status: "completed" },
  });
  await expect(row.locator(".lines-added")).toHaveText("+1,234");
  await page.screenshot({ path: "test-results/session-sidebar-resized.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect.poll(width).toBe(220);
  await expect
    .poll(
      async () =>
        (await page.locator(".session-workspace").boundingBox())!.width,
    )
    .toBeGreaterThanOrEqual(300);
  await page
    .getByRole("button", { name: "Close changes", exact: true })
    .click();
  await expect.poll(width).toBe(preferredWidth);
  await page.getByRole("button", { name: "Show changes", exact: true }).click();
  await expect.poll(width).toBe(220);
  await page.getByRole("button", { name: "Expand project inspector" }).click();
  await expect.poll(width).toBe(preferredWidth);
  await page
    .getByRole("button", { name: "Restore conversation layout" })
    .click();
  await expect.poll(width).toBe(220);
  await expect(input).toHaveValue("Keep my sidebar draft");
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  await row.hover();
  await expect(row.locator(".session-lines")).toBeInViewport({ ratio: 1 });
  await expect(
    row.getByRole("button", { name: "Archive session", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/session-sidebar.png" });
  await page.setViewportSize({ width: 1280, height: 800 });
  await expect.poll(width).toBe(preferredWidth);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setSessionDetails("codex", "thread-a", {
      branch: null,
      branchIsCurrent: false,
      changes: { additions: 0, deletions: 0 },
    });
    fixture.setSessionDetails("codex", "thread-b", {
      branch: null,
      branchIsCurrent: false,
      changes: null,
    });
  });
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await expect(row.locator(".session-branch")).toHaveCount(0);
  await expect(row.locator(".lines-added")).toHaveText("+0");
  await expect(
    sessionRow(page, "Review the provider adapters").locator(".session-lines"),
  ).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setSessionDetails("codex", "thread-b", null),
  );
  await page.getByRole("button", { name: "Refresh sessions" }).click();
  await page
    .getByRole("button", {
      name: "Review the provider adapters",
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
  await expect(page.getByRole("alert")).toHaveCount(0);
  await expect.poll(width).toBe(preferredWidth);
  await page.reload();
  await expect.poll(width).toBe(preferredWidth);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect.poll(width).toBe(preferredWidth);
});

test("project rows show scoped live Git details and background activity, including unavailable repositories", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const projects = page.getByRole("navigation", { name: "Open projects" });
  const current = projects.getByRole("button", {
    name: "caelum",
    exact: true,
  });
  const currentDetails = current.locator("../..");
  await expect(currentDetails.locator(".project-path")).toHaveText(
    "/work/caelum",
  );
  await expect(currentDetails.locator(".project-branch")).toHaveText("main");
  await expect(currentDetails.locator(".project-changes")).toHaveText(
    "2 changed",
  );
  await expect(current).toHaveAccessibleDescription(
    /\/work\/caelum.*main.*2 changed/,
  );
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "background" },
  });
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.hold("git_repository_status:/work/other");
    fixture.setGitError("Folder unavailable", "/work/other");
  });
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const other = projects.getByRole("button", { name: "other", exact: true });
  await expect(other).toHaveAttribute("aria-current", "page");
  const otherDetails = other.locator("../..");
  await expect(otherDetails).toContainText("Checking Git…");
  await expect(
    currentDetails.locator(".project-heading .running-dot"),
  ).toBeVisible();
  await current.click();
  await page.evaluate(() =>
    (window as any).fixture.release("git_repository_status:/work/other"),
  );
  await expect(otherDetails).toContainText("Git unavailable");
  await expect(currentDetails.locator(".project-changes")).toHaveText(
    "2 changed",
  );
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setGitError(null, "/work/other");
    fixture.setGitMetadata(
      { repositoryRoot: null, branch: null },
      "/work/other",
    );
    fixture.setGitMetadata({
      repositoryRoot: "/work/caelum",
      branch: "feat/a-long-branch-that-stays-within-the-project-row",
    });
    fixture.setGitFiles([]);
    window.dispatchEvent(new Event("focus"));
  });
  await expect(otherDetails).toContainText("Not a Git repository");
  await expect(currentDetails.locator(".project-changes")).toHaveText("Clean");
  await expect(currentDetails.locator(".project-branch")).toHaveAttribute(
    "title",
    "Current branch: feat/a-long-branch-that-stays-within-the-project-row",
  );
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setGitMetadata(
      { repositoryRoot: "/work/other", branch: null },
      "/work/other",
    );
    fixture.emit("approval-requests", [
      {
        provider: "codex",
        projectPath: "/work/caelum",
        sessionId: "thread-a",
        requestId: "project-approval",
        summary: "Run a command?",
        description: "git status",
      },
    ]);
  });
  await page.clock.runFor(30100);
  await expect(otherDetails.locator(".project-branch")).toHaveText(
    "Detached HEAD",
  );
  await expect(otherDetails.locator(".project-changes")).toHaveText("Clean");
  await expect(current).toHaveAccessibleDescription(
    /Running session.*Approval required/,
  );
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/project-details-compact.png" });
  await other.focus();
  await page.keyboard.press("Enter");
  await expect(other).toHaveAttribute("aria-current", "page");
  await projects
    .getByRole("button", { name: "Close other", exact: true })
    .click();
  await expect(other).toHaveCount(0);
  await expect(current).toHaveAttribute("aria-current", "page");
});
