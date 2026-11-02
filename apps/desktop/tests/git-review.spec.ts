import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, openSettings } from "./fixtures/helpers.js";

test("Changes syntax colours preserve patch text, independent old/new grammars and theme-aware additions and removals", async ({
  page,
}) => {
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const patch = [
    "diff --git a/src/App.tsx b/src/App.tsx",
    "--- a/src/App.tsx",
    "+++ b/src/App.tsx",
    "@@ -1,4 +1,4 @@",
    " export const count = 42;",
    "-/* removed comment starts",
    '+const label = "<script>alert(1)</script> — café";',
    " const ready = true;",
    "-*/",
    "+// a new comment",
    "@@ -30,2 +30,2 @@",
    "---counter;",
    "+++counter;",
    " \tconsole.log(count);",
    "\\ No newline at end of file",
  ].join("\n");
  await page.evaluate((patch) => {
    (window as any).fixture.setGitDiff("src/App.tsx", {
      unstaged: patch,
      staged: "@@ -1 +1 @@\n-const previous = 1;\n+const previous = 2;",
    });
  }, patch);
  await pane.getByRole("button", { name: "Refresh Git status" }).click();
  const unstaged = pane.getByRole("region", {
    name: "Unstaged diff",
    exact: true,
  });
  const added = unstaged.locator(".added").filter({ hasText: "+const label" });
  await expect(added.locator(".syntax-keyword")).toHaveText("const");
  expect(await unstaged.locator("pre").textContent()).toBe(patch);
  await expect(unstaged.locator("script")).toHaveCount(0);
  await expect(
    unstaged.locator(".meta").filter({ hasText: "+++ b/src/App.tsx" }),
  ).toHaveCount(1);
  await expect(
    unstaged.locator(".added").filter({ hasText: "+++counter;" }),
  ).toHaveCount(1);
  await expect(
    unstaged.locator(".removed").filter({ hasText: "---counter;" }),
  ).toHaveCount(1);
  await expect(
    unstaged.locator(".removed .syntax-comment").first(),
  ).toContainText("removed comment");
  await expect(
    unstaged
      .locator(".context")
      .filter({ hasText: "const ready" })
      .locator(".syntax-keyword"),
  ).toHaveText(["const", "true"]);
  await expect(added.locator(".diff-marker")).toHaveText("+");
  await expect(added.locator(".syntax-string")).toHaveCSS("display", "inline");
  await expect(added).toHaveCSS("box-shadow", "none");
  await expect(added).toHaveCSS("background-image", "none");
  await expect(added).toHaveCSS("background-color", "rgb(20, 35, 25)");
  await expect(unstaged.locator(".removed").first()).toHaveCSS(
    "background-color",
    "rgb(44, 24, 29)",
  );
  const staged = pane.getByRole("region", {
    name: "Staged diff",
    exact: true,
  });
  await expect(staged.locator(".added .syntax-number")).toHaveText("2");
  await page.screenshot({ path: "test-results/syntax-diff-dark.png" });
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(added).toHaveCSS("background-color", "rgb(232, 243, 233)");
  await expect(added.locator(".syntax-keyword")).toHaveCSS(
    "color",
    "rgb(123, 60, 171)",
  );
  await page.screenshot({ path: "test-results/syntax-diff-light.png" });

  for (const [path, source, tokenClass, text] of [
    ["src/main.rs", 'let name = "Caelum";', ".syntax-keyword", "let"],
    ["config.json", '{ "enabled": true, "count": 3 }', ".syntax-number", "3"],
    ["script.py", 'return "hello"', ".syntax-keyword", "return"],
    ["notes.unknown", '<img src=x onerror="alert(1)">', "", ""],
    ["image.png", "Binary files /dev/null and b/image.png differ", "", ""],
  ]) {
    const content =
      path === "image.png"
        ? source
        : `--- /dev/null\n+++ b/${path}\n@@ -0,0 +1 @@\n+${source}\n`;
    await page.evaluate(
      ({ path, content }) => {
        const fixture = (window as any).fixture;
        fixture.setGitFiles([{ path, indexStatus: "?", worktreeStatus: "?" }]);
        fixture.setGitDiff(path, {
          unstaged: "",
          staged: "",
          untracked: content,
        });
      },
      { path, content },
    );
    await pane.getByRole("button", { name: "Refresh Git status" }).click();
    const section = pane.getByRole("region", {
      name: "Untracked diff",
      exact: true,
    });
    if (tokenClass) await expect(section.locator(tokenClass)).toHaveText(text);
    await expect(section.locator("pre")).toHaveText(content);
    expect(await section.locator("pre").textContent()).toBe(content);
    await expect(section.locator("img")).toHaveCount(0);
  }
});

test("delayed syntax loading preserves the current file, exact text and manual reading position", async ({
  page,
}) => {
  let release = () => {};
  const loading = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/src/shared/code/codeSyntax.ts", async (route) => {
    await loading;
    await route.continue();
  });
  try {
    await desktop(page);
    const pane = page.getByRole("complementary", { name: "Project changes" });
    await expect(pane.getByText(/const layout = 'split'/)).toBeVisible();
    await expect(pane.locator(".syntax-keyword")).toHaveCount(0);
    const patch = [
      "@@ -1,62 +1,62 @@",
      " {",
      ...Array.from({ length: 60 }, (_, i) => ` \t"option${i}": ${i},`),
      '-"enabled": false',
      '+"enabled": true',
      " }",
    ].join("\n");
    await page.evaluate((patch) => {
      const fixture = (window as any).fixture;
      fixture.setGitFiles([
        {
          path: "config.json",
          indexStatus: " ",
          worktreeStatus: "M",
          modifiedAt: "999",
          revision: "json-1",
        },
      ]);
      fixture.setGitDiff("config.json", { unstaged: patch });
    }, patch);
    await pane.getByRole("button", { name: "Refresh Git status" }).click();
    await expect(pane.locator(".git-diff-heading")).toHaveText("config.json");
    const panel = pane.locator(".git-diff-panel");
    await panel.evaluate((node) => {
      node.scrollTop = 80;
    });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep this draft");
    release();
    await expect(pane.locator(".added .syntax-keyword")).toHaveText("true");
    expect(await pane.locator("pre").textContent()).toBe(patch);
    expect(await panel.evaluate((node) => node.scrollTop)).toBe(80);
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Keep this draft");
    await expect(pane.locator(".git-diff-heading")).toHaveText("config.json");
  } finally {
    release();
  }
});

test("changes follow the most recently edited file during work and preserve manual review between edits", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("const layout = 'split'");
  await expect(pane.locator(".git-file-item.latest")).toContainText(
    "src/App.tsx",
  );
  await input.fill("Keep this draft");
  await pane
    .getByRole("button", { name: "src/styles.css, Staged modified" })
    .click();
  await expect(pane.getByRole("region", { name: "Staged diff" })).toContainText(
    "revision 1",
  );
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "turn-live" },
  });
  await expect(
    pane.getByRole("button", { name: "Refresh Git status" }),
  ).toBeEnabled();
  await page.evaluate(() => {
    (window as any).fixture.setDiffVersion(2);
    (window as any).fixture.setGitFiles([
      {
        path: "src/App.tsx",
        indexStatus: " ",
        worktreeStatus: "M",
        modifiedAt: "300",
        revision: "app-2",
      },
      {
        path: "src/styles.css",
        indexStatus: "M",
        worktreeStatus: " ",
        modifiedAt: "100",
        revision: "css-1",
      },
    ]);
  });
  await page.clock.runFor(2600);
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("revision 2");
  await expect(
    pane.getByRole("button", { name: "src/App.tsx, Unstaged modified" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(pane.locator(".latest-change.added")).toContainText(
    "revision 2",
  );
  await pane
    .getByRole("button", { name: "src/styles.css, Staged modified" })
    .click();
  await page.clock.runFor(2600);
  await expect(
    pane.getByRole("button", { name: "src/styles.css, Staged modified" }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(pane.locator(".git-file-item.latest")).toContainText(
    "src/App.tsx",
  );
  await expect(input).toHaveValue("Keep this draft");
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
  await page.screenshot({ path: "test-results/session-changes.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(input).toBeInViewport({ ratio: 1 });
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toBeInViewport({ ratio: 1 });
  await expect(pane).toBeInViewport({ ratio: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/session-changes-minimum.png" });
  await page.evaluate(() =>
    (window as any).fixture.setGitFiles([
      { path: "src/App.tsx", indexStatus: " ", worktreeStatus: "M" },
    ]),
  );
  await emit(page, "turn/completed", {
    threadId: "thread-a",
    turn: { id: "turn-live", status: "completed" },
  });
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("revision 2");
  await expect(
    pane.getByRole("region", { name: "Staged diff", exact: true }),
  ).toHaveCount(0);
  await page.evaluate(() => (window as any).fixture.setGitFiles([]));
  await pane.getByRole("button", { name: "Refresh Git status" }).click();
  await expect(pane.getByText("Working tree clean")).toBeVisible();
  await pane.getByRole("button", { name: "Close changes" }).click();
  await expect(pane).toHaveCount(0);
  await expect(input).toHaveValue("Keep this draft");
  await page.getByRole("button", { name: "Show changes" }).click();
  await expect(pane.getByText("Working tree clean")).toBeVisible();
});

test("repeated edits reveal the updated hunk without shifting the transcript or unchanged review", async ({
  page,
}) => {
  await page.clock.install();
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const panel = pane.locator(".git-diff-panel");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const refresh = pane.getByRole("button", { name: "Refresh Git status" });
  const update = async (version: number) => {
    await page.evaluate((version) => {
      const fixture = (window as any).fixture;
      const content = [
        "diff --git a/src/App.tsx b/src/App.tsx",
        "@@ -1,40 +1,40 @@",
        "-old header",
        "+existing header",
        ...Array.from({ length: 35 }, (_, index) => ` context ${index}`),
        "@@ -100,3 +100,3 @@",
        "-old footer",
        `+latest footer ${version}`,
        " unchanged footer",
      ].join("\n");
      fixture.setGitDiff("src/App.tsx", { unstaged: content });
      fixture.setGitFiles([
        {
          path: "src/App.tsx",
          indexStatus: " ",
          worktreeStatus: "M",
          modifiedAt: String(300 + version),
          revision: `app-${version}`,
        },
        {
          path: "src/styles.css",
          indexStatus: "M",
          worktreeStatus: " ",
          modifiedAt: "100",
          revision: "css-1",
        },
      ]);
    }, version);
  };
  await update(1);
  await refresh.click();
  await expect(
    pane.getByText("+latest footer 1", { exact: true }),
  ).toBeAttached();
  await page.getByRole("button", { name: "Load earlier messages" }).click();
  await expect(page.locator(".conversation-entry")).toHaveCount(100);
  const transcript = page.locator(".transcript");
  await transcript.evaluate((node) => {
    node.scrollTop = 250;
  });
  await input.fill("Keep this draft and caret");
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  await emit(page, "turn/started", {
    threadId: "thread-a",
    turn: { id: "edit" },
  });
  await expect(refresh).toBeEnabled();
  await panel.evaluate((node) => {
    node.scrollTop = 0;
  });
  await update(2);
  await input.focus();
  await page.clock.runFor(2600);
  const latest = pane.locator(".latest-change.added");
  await expect(latest).toHaveText("+latest footer 2\n");
  await expect(latest).toBeInViewport({ ratio: 1 });
  await expect(
    pane.getByText("+existing header", { exact: true }),
  ).not.toHaveClass(/latest-change/);
  expect(await panel.evaluate((node) => node.scrollTop)).toBeGreaterThan(0);
  expect(await transcript.evaluate((node) => node.scrollTop)).toBe(250);
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("Keep this draft and caret");
  await page.screenshot({ path: "test-results/latest-change-hunk.png" });
  await panel.evaluate((node) => {
    node.scrollTop = 80;
  });
  await page.clock.runFor(2600);
  expect(await panel.evaluate((node) => node.scrollTop)).toBe(80);
});

test("late Git responses cannot replace a newer selection or another project's diff", async ({
  page,
}) => {
  await desktop(page);
  const pane = page.getByRole("complementary", { name: "Project changes" });
  const refresh = pane.getByRole("button", { name: "Refresh Git status" });
  await expect(pane.getByText(/const layout = 'split'/)).toBeVisible();
  await page.evaluate(() => (window as any).fixture.hold("git_file_diff"));
  await refresh.click();
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === "git_file_diff",
          ).length,
      ),
    )
    .toBe(2);
  await pane
    .getByRole("button", { name: "src/styles.css, Staged modified" })
    .click();
  await page.evaluate(() => (window as any).fixture.release("git_file_diff"));
  await expect(pane.getByRole("region", { name: "Staged diff" })).toContainText(
    "background",
  );
  await expect(
    pane.getByRole("region", { name: "Unstaged diff", exact: true }),
  ).toHaveCount(0);

  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.hold("git_repository_status");
    fixture.setGitFiles(
      [
        {
          path: "src/App.tsx",
          indexStatus: " ",
          worktreeStatus: "M",
          modifiedAt: "999",
          revision: "other",
        },
      ],
      "/work/other",
    );
    fixture.setGitDiff(
      "src/App.tsx",
      { unstaged: "@@ -1 +1 @@\n-old\n+Other project" },
      "/work/other",
    );
  });
  await refresh.click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".project-row.selected")).toContainText("other");
  await page.evaluate(() =>
    (window as any).fixture.release("git_repository_status"),
  );
  await expect(
    pane.getByRole("region", { name: "Unstaged diff" }),
  ).toContainText("Other project");
  await expect(pane.locator(".git-file-item.latest")).toContainText(
    "src/App.tsx",
  );
  await expect(pane.getByText(/background: #0b0c0e/)).toHaveCount(0);
});
