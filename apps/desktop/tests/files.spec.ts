import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  openSettings,
  findProjectFile,
  attachedContext,
  pickMenu,
  copiedText,
} from "./fixtures/helpers.js";

test("Files searches filenames and contents beside the conversation, handles ignored files and rejects stale results", async ({
  page,
}) => {
  await desktop(page);
  const navigation = page.getByRole("navigation", {
    name: "Workspace",
    exact: true,
  });
  await expect(
    page.getByRole("region", { name: "Sessions", exact: true }),
  ).toBeVisible();
  await expect(
    navigation.getByRole("button", { name: "Changes", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.evaluate(() =>
    (window as any).fixture.setProjectFile(
      "src/App.tsx",
      "// header\nexport const ready = true;\n// footer\n",
    ),
  );
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft visible");
  await findProjectFile(page, "src/App.tsx");
  await expect(
    page.getByRole("region", { name: "Sessions", exact: true }),
  ).toBeVisible();
  await expect(
    navigation.getByRole("button", { name: "Files", exact: true }),
  ).toHaveClass(/active/);
  await expect(
    navigation.getByRole("button", { name: "Files", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(
    navigation.getByRole("button", { name: "Changes", exact: true }),
  ).toHaveAttribute("aria-pressed", "false");
  await expect(input).toBeVisible();
  await expect(input).toHaveValue("Keep this draft visible");
  await expect(
    page.getByRole("heading", { name: "A focused workspace" }),
  ).toBeVisible();
  await page.keyboard.press("Meta+Shift+f");
  const contents = page.getByRole("combobox", {
    name: "Search project contents",
  });
  await expect(contents).toBeFocused();
  await contents.fill("ready");
  await expect(
    page.getByRole("option", { name: /src\/App.tsx:2/ }),
  ).toBeVisible();
  await contents.press("Enter");
  await expect(page.locator(".cm-activeLineGutter")).toHaveText("2");
  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:src/styles.css"),
  );
  await page.keyboard.press("Meta+p");
  const filenames = page.getByRole("combobox", { name: "Find project file" });
  await filenames.fill("src/styles.css");
  await expect(
    page.getByRole("option", { name: "src/styles.css", exact: true }),
  ).toBeVisible();
  await filenames.press("Enter");
  await expect(
    page.getByText("Loading file preview…", { exact: true }),
  ).toBeVisible();
  const tabs = page.getByRole("tablist", { name: "Open files" });
  await expect(tabs.getByRole("tab")).toHaveCount(2);
  await tabs.getByRole("tab", { name: "styles.css", exact: true }).focus();
  await page.evaluate(() =>
    (window as any).fixture.release("read_project_file:src/styles.css"),
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          JSON.parse(
            localStorage.getItem("caelum.files.v1:%2Fwork%2Fcaelum") ?? "{}",
          ).views?.["file:src/styles.css"]?.main,
      ),
    )
    .toBeTruthy();
  await expect(
    tabs.getByRole("tab", { name: "styles.css", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("ArrowLeft");
  await expect(
    tabs.getByRole("tab", { name: "App.tsx", exact: true }),
  ).toBeFocused();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");

  await page.keyboard.press("Meta+p");
  const search = page.getByRole("combobox", { name: "Find project file" });
  await search.fill("generated");
  await expect(
    page.getByText("No matching files.", { exact: true }),
  ).toBeVisible();
  await page.getByRole("checkbox", { name: "Include ignored" }).check();
  await expect(
    page.getByRole("option", { name: "vendor/generated.ts" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.hold("search_project_files:README"),
  );
  await search.fill("README");
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).fixture.calls.some(
          (call: any) =>
            call.command === "search_project_files" &&
            call.args.query === "README",
        ),
      ),
    )
    .toBe(true);
  await search.fill("styles");
  await expect(
    page.getByRole("option", { name: "src/styles.css" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.release("search_project_files:README"),
  );
  await expect(page.getByRole("option", { name: "README.md" })).toHaveCount(0);
  await search.press("Escape");
  await page.getByRole("button", { name: "Recent", exact: true }).click();
  await expect(
    page
      .locator(".changed-files-scroll")
      .getByRole("button", { name: "src/App.tsx", exact: true }),
  ).toBeVisible();
  await tabs
    .getByRole("button", { name: "Close src/styles.css", exact: true })
    .click();
  await expect(tabs.getByRole("tab")).toHaveCount(1);
});

test("Files restores tabs, expanded folders and reading positions, refreshes in place and keeps project workspaces separate", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await desktop(page);
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setProjectFile(
      "src/App.tsx",
      Array.from(
        { length: 220 },
        (_, index) => `export const line${index + 1} = ${index + 1};`,
      ).join("\n"),
    );
    for (let index = 0; index < 260; index++)
      fixture.setProjectFile(`src/extra-${index}.ts`, "export {};\n");
  });
  await findProjectFile(page, "src/App.tsx");
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click();
  await page
    .getByRole("treeitem", { name: "Load more files in src", exact: true })
    .click();
  await expect(
    page.getByRole("treeitem", { name: /extra-.*file/ }),
  ).toHaveCount(260);
  const tree = page.locator(".file-tree-scroll");
  await tree.evaluate((node) => {
    node.scrollTop = 700;
  });
  const code = page.locator(".code-viewer .cm-editor").first();
  const editorInput = page.getByRole("textbox", {
    name: "Read-only preview of src/App.tsx",
    exact: true,
  });
  await editorInput.focus();
  await editorInput.press("PageDown");
  await editorInput.press("PageDown");
  const stored = () =>
    page.evaluate(() =>
      JSON.parse(
        localStorage.getItem("caelum.files.v1:%2Fwork%2Fcaelum") ?? "{}",
      ),
    );
  await expect
    .poll(
      async () => (await stored()).views?.["file:src/App.tsx"]?.main?.line ?? 0,
    )
    .toBeGreaterThan(20);
  const view = (await stored()).views["file:src/App.tsx"].main;
  const firstVisibleLine = () =>
    code.locator(".cm-line").evaluateAll((nodes) => {
      const bounds = nodes[0]?.closest(".cm-scroller")?.getBoundingClientRect();
      return nodes.find(
        (node) => node.getBoundingClientRect().bottom > (bounds?.top ?? 0),
      )?.textContent;
    });
  const line = await firstVisibleLine();
  await findProjectFile(page, "README.md");
  await page.getByRole("tab", { name: "App.tsx", exact: true }).click();
  await expect.poll(firstVisibleLine).toBe(line);
  const updated =
    Array.from(
      { length: 220 },
      (_, index) => `export const line${index + 1} = ${index + 1};`,
    ).join("\n") + `\n// refreshed ${"x".repeat(512)}\n`;
  await page.evaluate((updated) => {
    (window as any).fixture.setProjectFile("src/App.tsx", updated);
    window.dispatchEvent(new Event("focus"));
  }, updated);
  await expect(page.locator(".file-editor-size")).toHaveText(
    `${(updated.length / 1024).toFixed(1)} KiB`,
  );
  await expect(
    page.getByRole("treeitem", { name: /extra-.*file/ }),
  ).toHaveCount(260);
  await expect
    .poll(async () => (await stored()).views?.["file:src/App.tsx"]?.main)
    .toEqual(view);
  await expect.poll(firstVisibleLine).toBe(line);
  await page.getByRole("button", { name: "Close files", exact: true }).click();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect.poll(firstVisibleLine).toBe(line);
  await expect.poll(() => tree.evaluate((node) => node.scrollTop)).toBe(700);
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(
    page.getByRole("tablist", { name: "Open files" }).getByRole("tab"),
  ).toHaveCount(2);
  await expect(
    page.getByRole("treeitem", { name: /extra-.*file/ }),
  ).toHaveCount(260);
  await expect.poll(firstVisibleLine).toBe(line);
  await expect.poll(() => tree.evaluate((node) => node.scrollTop)).toBe(700);

  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".file-tabs")).toHaveCount(0);
  await findProjectFile(page, "src/App.tsx");
  await expect(code).toContainText("other");
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(
    page.getByRole("tablist", { name: "Open files" }).getByRole("tab"),
  ).toHaveCount(2);
  await expect.poll(firstVisibleLine).toBe(line);
});

test("Files inspector resizes and expands while keeping the conversation mounted and handles missing files and late reads", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1500, height: 900 });
  await desktop(page);
  await findProjectFile(page, "src/App.tsx");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my place");
  await page
    .locator(".transcript")
    .evaluate((node) => node.setAttribute("data-mounted", "yes"));
  const pane = page.getByRole("complementary", {
    name: "Project files",
    exact: true,
  });
  const width = (await pane.boundingBox())!.width;
  const resize = page.getByRole("separator", {
    name: "Resize project inspector",
  });
  await resize.focus();
  await resize.press("ArrowLeft");
  await expect
    .poll(async () => (await pane.boundingBox())!.width)
    .toBeGreaterThan(width);
  const divider = (await resize.boundingBox())!;
  await page.mouse.move(divider.x + divider.width / 2, divider.y + 100);
  await page.mouse.down();
  await page.mouse.move(divider.x - 80, divider.y + 100);
  await page.mouse.up();
  await expect
    .poll(async () => (await pane.boundingBox())!.width)
    .toBeGreaterThan(width + 60);
  const tree = pane.getByRole("complementary", {
    name: "Project files and changes",
  });
  const treeResize = page.getByRole("separator", { name: "Resize file tree" });
  const treeWidth = async () => (await tree.boundingBox())!.width;
  const initialTreeWidth = await treeWidth();
  const inspectorWidth = (await pane.boundingBox())!.width;
  const preview = page.locator(".code-viewer");
  await preview.evaluate((node) => node.setAttribute("data-mounted", "yes"));
  await treeResize.focus();
  await treeResize.press("ArrowRight");
  await expect.poll(treeWidth).toBe(initialTreeWidth + 32);
  const treeDivider = (await treeResize.boundingBox())!;
  await page.mouse.move(
    treeDivider.x + treeDivider.width / 2,
    treeDivider.y + 100,
  );
  await page.mouse.down();
  await page.mouse.move(
    treeDivider.x + treeDivider.width / 2 + 60,
    treeDivider.y + 100,
  );
  await page.mouse.up();
  await expect.poll(treeWidth).toBe(initialTreeWidth + 92);
  const preferredTreeWidth = await treeWidth();
  await expect
    .poll(async () => (await pane.boundingBox())!.width)
    .toBe(inspectorWidth);
  await expect(preview).toHaveAttribute("data-mounted", "yes");
  await page.getByRole("button", { name: "Expand project inspector" }).click();
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
  await expect(input).toBeHidden();
  await expect(
    page.getByRole("region", { name: "Sessions", exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Files", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  await page
    .getByRole("button", { name: "Restore conversation layout" })
    .click();
  await expect(input).toHaveValue("Keep my place");
  await expect(
    page.getByRole("region", { name: "Sessions", exact: true }),
  ).toBeVisible();
  await expect(page.locator(".transcript")).toHaveAttribute(
    "data-mounted",
    "yes",
  );
  await page.screenshot({ path: "test-results/files-inspector-dark.png" });
  await page
    .getByRole("button", { name: "Load earlier messages", exact: true })
    .click();
  const transcript = page.locator(".transcript");
  await transcript.evaluate((node) => {
    node.scrollTop = 250;
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Expand project inspector" }).click();
  await page
    .getByRole("button", { name: "Restore conversation layout" })
    .click();
  await expect
    .poll(() => transcript.evaluate((node) => node.scrollTop))
    .toBe(250);

  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:README.md"),
  );
  await page.keyboard.press("Meta+p");
  const search = page.getByRole("combobox", { name: "Find project file" });
  await search.fill("README.md");
  await page.getByRole("option", { name: "README.md", exact: true }).click();
  await expect(
    page.getByText("Loading file preview…", { exact: true }),
  ).toBeVisible();
  await page.getByRole("tab", { name: "App.tsx", exact: true }).click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
  await page.evaluate(() =>
    (window as any).fixture.release("read_project_file:README.md"),
  );
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
  await page.evaluate(() => {
    (window as any).fixture.setProjectFile("src/App.tsx", null);
    window.dispatchEvent(new Event("focus"));
  });
  await expect(
    page.getByRole("alert").filter({ hasText: "File not found: src/App.tsx" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.setProjectFile("src/App.tsx", "// restored\n"),
  );
  await page.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(page.locator(".code-viewer .cm-editor").first()).toContainText(
    "restored",
  );
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page.screenshot({ path: "test-results/files-inspector-light.png" });
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(input).toBeVisible();
  await expect.poll(treeWidth).toBeLessThan(preferredTreeWidth);
  await expect
    .poll(
      async () =>
        (await page.locator(".file-editor-panel").boundingBox())!.width,
    )
    .toBeGreaterThanOrEqual(160);
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await page.screenshot({ path: "test-results/files-inspector-compact.png" });
  await page.setViewportSize({ width: 1500, height: 900 });
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect.poll(treeWidth).toBe(initialTreeWidth);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect.poll(treeWidth).toBe(preferredTreeWidth);
});

test("Files opens local Markdown and tool references at their source line without converting code blocks", async ({
  page,
}) => {
  await desktop(page);
  await page.evaluate(() => {
    (window as any).fixture.setProjectFile(
      "src/App.tsx",
      "// one\n// two\n// three\n",
    );
    (window as any).fixture.setCodexHistory(
      [
        {
          id: "refs",
          status: "completed",
          items: [
            {
              id: "refs-answer",
              type: "agentMessage",
              text: "See `src/App.tsx:2`, [the stylesheet](src/styles.css#L1) and [the last line](file:///work/caelum/src/App.tsx:3).\n\n```text\nsrc/App.tsx:3\n```",
            },
            {
              id: "refs-tool",
              type: "fileChange",
              status: "completed",
              changes: [
                {
                  path: "/work/caelum/src/App.tsx",
                  kind: { type: "update" },
                  diff: "@@ -1 +1 @@\n-old\n+new",
                },
              ],
            },
          ],
        },
      ],
      false,
    );
    window.dispatchEvent(new Event("focus"));
  });
  const patch = page.locator('[data-entry-id="codex-refs-tool"]');
  await expect(patch.locator(".tool-summary")).toHaveAttribute(
    "aria-expanded",
    "false",
  );
  await expect(
    patch.locator(".tool-patch-preview .diff-line.removed"),
  ).toHaveText("-old");
  await expect(
    patch.locator(".tool-patch-preview .diff-line.added"),
  ).toHaveText("+new");
  await page
    .locator(".markdown")
    .getByRole("button", { name: "src/App.tsx:2", exact: true })
    .click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
  await expect(page.locator(".cm-activeLineGutter")).toHaveText("2");
  await expect(page.locator(".code-block .file-reference")).toHaveCount(0);
  await page
    .locator(".markdown")
    .getByRole("button", { name: "the last line", exact: true })
    .click();
  await expect(page.locator(".cm-activeLineGutter")).toHaveText("3");
  await page
    .locator(".markdown")
    .getByRole("button", { name: "src/App.tsx:2", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Copy relative path");
  await expect.poll(() => copiedText(page)).toBe("src/App.tsx");
  await page
    .locator(".markdown")
    .getByRole("button", { name: "the stylesheet", exact: true })
    .click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/styles.css");
  await page
    .getByRole("button", { name: "Edit /work/caelum/src/App.tsx", exact: true })
    .click();
  await page
    .locator(".tool-file-links")
    .getByRole("button", { name: "src/App.tsx", exact: true })
    .click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
});

test("Files highlights Rust, stays read-only, searches locally and preserves exact Unicode selections", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  page.on("console", (message) => {
    if (message.type() === "error") errors.push(message.text());
  });
  await desktop(page);
  const source =
    '// Rust preview\r\npub fn main() {\r\n    let count: u32 = 42;\r\n    let label = r#"Caelum 🚀"#;\r\n    println!("{label}: {count}");\r\n}\r\n';
  await page.evaluate(
    (source) => (window as any).fixture.setProjectFile("src/main.rs", source),
    source,
  );
  await findProjectFile(page, "src/main.rs");
  const viewer = page.locator(".code-viewer");
  const preview = viewer.getByRole("textbox", {
    name: "Read-only preview of src/main.rs",
    exact: true,
  });
  await expect(preview).toHaveAttribute("aria-readonly", "true");
  await expect(preview).toHaveAttribute("contenteditable", "false");
  await expect(viewer.locator(".syntax-keyword")).toHaveText([
    "pub",
    "fn",
    "let",
    "let",
  ]);
  await expect(viewer.locator(".syntax-type")).toContainText("u32");
  await expect(viewer.locator(".syntax-number")).toHaveText("42");
  await expect(viewer.locator(".syntax-comment")).toHaveText("// Rust preview");
  const rendered = () =>
    viewer
      .locator(".cm-line")
      .allTextContents()
      .then((lines) => lines.join("\n"));
  await expect.poll(rendered).toBe(source.replaceAll("\r\n", "\n"));
  await preview.press("ControlOrMeta+a");
  await preview.press("x");
  await preview.press("Backspace");
  await expect.poll(rendered).toBe(source.replaceAll("\r\n", "\n"));
  await preview.press("ArrowLeft");
  await preview.press("ControlOrMeta+f");
  const search = viewer.getByRole("textbox", { name: "Find", exact: true });
  await expect(search).toBeFocused();
  await search.pressSequentially("Caelum");
  await search.press("Enter");
  await expect(viewer.locator(".cm-activeLineGutter")).toHaveText("4");
  await expect(viewer.locator(".cm-searchMatch-selected")).toHaveText("Caelum");
  await search.press("Escape");
  await expect(preview).toBeFocused();
  await preview.press("ControlOrMeta+Home");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+ArrowDown");
  await preview.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  const selected = source.split("\r\n").slice(0, 4).join("\r\n") + "\r\n";
  await expect.poll(() => copiedText(page)).toBe(selected);
  await preview.press("Shift+F10");
  await pickMenu(page, "Add selection to prompt");
  const chip = attachedContext(page).getByRole("button", {
    name: "src/main.rs:1-4",
    exact: true,
  });
  await chip.click({ button: "right" });
  await pickMenu(page, "Copy attached snapshot");
  await expect.poll(() => copiedText(page)).toBe(selected);
  await page.screenshot({ path: "test-results/rust-preview-dark.png" });
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(viewer.locator(".syntax-keyword").first()).toHaveCSS(
    "color",
    "rgb(123, 60, 171)",
  );
  await page.screenshot({ path: "test-results/rust-preview-light.png" });
  expect(errors).toEqual([]);
});

test("Files migrates legacy reading positions while delayed Rust grammars preserve navigation and composer focus", async ({
  page,
}) => {
  let release = () => {};
  let requests = 0;
  const loading = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/node_modules/.vite/deps/dist-*.js*", async (route) => {
    const response = await route.fetch();
    if ((await response.text()).includes("function rust(")) {
      requests++;
      await loading;
    }
    await route.fulfill({ response });
  });
  try {
    await desktop(page);
    await page.evaluate(() => {
      (window as any).fixture.setProjectFile(
        "src/main.rs",
        Array.from(
          { length: 200 },
          (_, index) => `pub fn line${index + 1}() -> u32 { ${index + 1} }`,
        ).join("\n"),
      );
      localStorage.setItem(
        "caelum.files.v1:%2Fwork%2Fcaelum",
        JSON.stringify({
          tabs: [{ path: "src/main.rs", mode: "file" }],
          active: "file:src/main.rs",
          views: {
            "file:src/main.rs": {
              viewState: {
                firstPosition: { lineNumber: 40 },
                firstPositionDeltaTop: -4,
                scrollLeft: 0,
              },
              cursorState: [
                {
                  selectionStart: { lineNumber: 41, column: 1 },
                  position: { lineNumber: 41, column: 1 },
                },
              ],
            },
          },
        }),
      );
    });
    await page.getByRole("button", { name: "Files", exact: true }).click();
    const viewer = page.locator(".code-viewer");
    await expect(viewer.getByRole("textbox")).toBeVisible();
    await expect.poll(() => requests).toBeGreaterThan(0);
    await expect(viewer.locator(".syntax-keyword")).toHaveCount(0);
    const firstLine = () =>
      viewer.locator(".cm-line").evaluateAll((lines) => {
        const top =
          lines[0]?.closest(".cm-scroller")!.getBoundingClientRect().top ?? 0;
        return lines.find((line) => line.getBoundingClientRect().bottom > top)
          ?.textContent;
      });
    await expect.poll(firstLine).toBe("pub fn line40() -> u32 { 40 }");
    await expect(viewer.locator(".cm-activeLineGutter")).toHaveText("41");
    await findProjectFile(page, "README.md");
    await page.getByRole("tab", { name: "main.rs", exact: true }).click();
    await expect.poll(firstLine).toBe("pub fn line40() -> u32 { 40 }");
    const scroller = viewer.locator(".cm-scroller");
    const top = await scroller.evaluate((node) => node.scrollTop);
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep this draft and focus");
    release();
    await expect(viewer.locator(".syntax-keyword").first()).toHaveText("pub");
    await expect.poll(firstLine).toBe("pub fn line40() -> u32 { 40 }");
    await expect
      .poll(async () =>
        Math.abs((await scroller.evaluate((node) => node.scrollTop)) - top),
      )
      .toBeLessThanOrEqual(2); // WebKit rounds fractional line heights after highlighting.
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Keep this draft and focus");
  } finally {
    release();
  }
});

test("Files diff panes preserve shared scrolling, separate selections and HEAD snapshots across refreshes", async ({
  page,
}) => {
  await desktop(page);
  const original =
    'export const label = "before 🚀";\r\n' +
    Array.from(
      { length: 200 },
      (_, index) => `export const line${index} = ${index};\r\n`,
    ).join("");
  const modified = original.replace("before", "after");
  await page.evaluate(
    ({ original, modified }) => {
      (window as any).fixture.setGitVersions("src/App.tsx", original, modified);
    },
    { original, modified },
  );
  await findProjectFile(page, "src/App.tsx");
  await page
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  const merge = page.locator(".cm-mergeView");
  const before = merge.getByRole("textbox", {
    name: "Read-only HEAD version of src/App.tsx",
    exact: true,
  });
  const after = merge.getByRole("textbox", {
    name: "Read-only working tree version of src/App.tsx",
    exact: true,
  });
  await expect(merge).toBeVisible();
  await expect(merge.locator(".cm-merge-a .cm-changedLine").first()).toHaveCSS(
    "background-color",
    "rgb(44, 24, 29)",
  );
  await expect(merge.locator(".cm-merge-b .cm-changedLine").first()).toHaveCSS(
    "background-color",
    "rgb(20, 35, 25)",
  );
  await before.press("ControlOrMeta+Home");
  await before.press("Shift+ArrowDown");
  await before.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe('export const label = "before 🚀";\r\n');
  await after.press("ControlOrMeta+Home");
  await after.press("Shift+ArrowDown");
  await after.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe('export const label = "after 🚀";\r\n');
  await merge.evaluate((node) => {
    node.scrollTop = 1200;
  });
  const firstLines = () =>
    merge.locator(".cm-editor").evaluateAll((panes) =>
      panes.map((pane) => {
        const top = pane.closest(".cm-mergeView")!.getBoundingClientRect().top;
        return Array.from(pane.querySelectorAll(".cm-line")).find(
          (line) => line.getBoundingClientRect().bottom > top,
        )?.textContent;
      }),
    );
  await expect
    .poll(firstLines)
    .toEqual([
      expect.stringMatching(/export const line\d/),
      expect.stringMatching(/export const line\d/),
    ]);
  const lines = await firstLines();
  expect(lines[0]).toBe(lines[1]);
  await findProjectFile(page, "README.md");
  await page.getByRole("tab", { name: "App.tsx · Diff", exact: true }).click();
  await expect.poll(firstLines).toEqual(lines);
  await expect.poll(() => merge.evaluate((node) => node.scrollTop)).toBe(1200);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep reviewing");
  await page.evaluate(
    ({ original, modified }) => {
      (window as any).fixture.setGitVersions(
        "src/App.tsx",
        original,
        modified + "// refreshed\r\n",
      );
      window.dispatchEvent(new Event("focus"));
    },
    { original, modified },
  );
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === "git_file_versions",
          ).length,
      ),
    )
    .toBeGreaterThan(2);
  await expect.poll(firstLines).toEqual(lines);
  await expect(input).toBeFocused();
  await before.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe('export const label = "before 🚀";\r\n');
  await after.press("Shift+F10");
  await pickMenu(page, "Copy file preview");
  await expect.poll(() => copiedText(page)).toBe(modified + "// refreshed\r\n");
});

test("Files recovers damaged saved tabs and reports storage failures without losing the open preview", async ({
  page,
}) => {
  await desktop(page);
  await page.evaluate(() =>
    localStorage.setItem(
      "caelum.files.v1:%2Fwork%2Fcaelum",
      JSON.stringify({
        tabs: [
          null,
          { path: "../outside.txt", mode: "file" },
          { path: "README.md", mode: "file" },
          { path: "README.md", mode: "file" },
        ],
        active: "file:missing.ts",
        expanded: [null, "../outside", "src"],
        views: { "file:README.md": { invalid: true } },
        treeScroll: -100,
      }),
    ),
  );
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(
    page.getByRole("tablist", { name: "Open files" }).getByRole("tab"),
  ).toHaveCount(1);
  await expect(page.locator(".file-editor-path")).toHaveText("README.md");
  await expect(page.locator(".code-viewer .cm-editor").first()).toContainText(
    "Project documentation.",
  );
  await page.evaluate(() => {
    const setItem = Storage.prototype.setItem;
    (window as any).restoreFileStorage = () => {
      Storage.prototype.setItem = setItem;
    };
    Storage.prototype.setItem = function (key, value) {
      if (key.startsWith("caelum.files.v1:"))
        throw new DOMException("Storage unavailable", "QuotaExceededError");
      setItem.call(this, key, value);
    };
  });
  await findProjectFile(page, "src/App.tsx");
  const error = page
    .getByRole("alert")
    .filter({ hasText: "Could not save file tabs" });
  await expect(error).toBeVisible();
  await page.evaluate(() => (window as any).restoreFileStorage());
  await error.getByRole("button", { name: "Retry", exact: true }).click();
  await expect(error).toHaveCount(0);
  await page.reload();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await expect(page.locator(".file-editor-path")).toHaveText("src/App.tsx");
});
