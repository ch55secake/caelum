import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  attachedContext,
  contextMenu,
  pickMenu,
  copiedText,
} from "./fixtures/helpers.js";

test("context menus support keyboard navigation, viewport edges, themes and dismissal", async ({
  page,
}, testInfo) => {
  await desktop(page);
  const project = page.getByRole("button", { name: "caelum", exact: true });
  const menu = page.getByRole("menu", { name: "Context menu" });
  await project.focus();
  await project.press("Shift+F10");
  await expect(
    page.getByRole("menuitem", { name: "Open project", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("End");
  await expect(
    page.getByRole("menuitem", { name: "Close project", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByRole("menuitem", { name: "Open project", exact: true }),
  ).toBeFocused();
  await page.keyboard.type("cop");
  await expect(
    page.getByRole("menuitem", { name: "Copy project path", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect.poll(() => copiedText(page)).toBe("/work/caelum");
  await expect(menu).toHaveCount(0);
  await expect(project).toBeFocused();
  await project.press("ContextMenu");
  await page.screenshot({ path: testInfo.outputPath("context-menu-dark.png") });
  await page.evaluate(() => {
    document.documentElement.dataset.theme = "light";
  });
  await expect(menu).toHaveCSS("background-color", "rgb(255, 255, 255)");
  await page.screenshot({
    path: testInfo.outputPath("context-menu-light.png"),
  });
  await page.keyboard.press("Escape");
  await expect(project).toBeFocused();
  await project.press("Shift+F10");
  await page.keyboard.press("Tab");
  await expect(menu).toHaveCount(0);
  await expect(project).toBeFocused();

  // Exercise pointer placement separately from keyboard invocation, and wait
  // for the edge position before clicking the project outside the menu.
  await page.mouse.click(1279, 799, { button: "right" });
  await expect(menu).toBeVisible();
  await expect
    .poll(async () => {
      const bounds = await menu.boundingBox();
      return (
        bounds &&
        Math.abs(bounds.x + bounds.width - 1272) < 1 &&
        Math.abs(bounds.y + bounds.height - 792) < 1
      );
    })
    .toBe(true);
  const bounds = (await menu.boundingBox())!;
  expect(bounds.x).toBeGreaterThanOrEqual(8);
  expect(bounds.y).toBeGreaterThanOrEqual(8);
  expect(bounds.x + bounds.width).toBeLessThanOrEqual(1272);
  expect(bounds.y + bounds.height).toBeLessThanOrEqual(792);
  await project.click();
  await expect(menu).toHaveCount(0);
  await project.press("Shift+F10");
  await page.locator(".transcript").dispatchEvent("scroll");
  await expect(menu).toBeVisible();
  await page.locator(".project-list").dispatchEvent("wheel");
  await expect(menu).toHaveCount(0);
  await project.press("Shift+F10");
  await page.setViewportSize({ width: 1000, height: 700 });
  await expect(menu).toHaveCount(0);
  await project.press("Shift+F10");
  await page.evaluate(() => window.dispatchEvent(new Event("blur")));
  await expect(menu).toHaveCount(0);
});

test("context menus edit the original selection with undo and reject delayed paste into newer drafts", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const edit = async (action: string) => {
    await input.press("Shift+F10");
    await pickMenu(page, action);
  };
  // Seed an existing draft without an initial typing undo group (WebKit coalesces it).
  await input.evaluate((node: HTMLTextAreaElement) => {
    Object.getOwnPropertyDescriptor(
      HTMLTextAreaElement.prototype,
      "value",
    )!.set!.call(node, "Keep this draft");
    node.dispatchEvent(new Event("input", { bubbles: true }));
  });
  await input.evaluate((node: HTMLTextAreaElement) =>
    node.setSelectionRange(5, 9),
  );
  await edit("Cut");
  await expect.poll(() => copiedText(page)).toBe("this");
  await expect(input).toHaveValue("Keep  draft");
  await edit("Undo");
  await expect(input).toHaveValue("Keep this draft");
  await edit("Redo");
  await expect(input).toHaveValue("Keep  draft");
  await page.evaluate(() =>
    (window as any).fixture.setClipboard("another\nlonger"),
  );
  await edit("Paste");
  await expect(input).toHaveValue("Keep another\nlonger draft");
  await edit("Undo");
  await expect(input).toHaveValue("Keep  draft");
  await edit("Select all");
  expect(
    await input.evaluate((node: HTMLTextAreaElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([0, 11]);
  await page.evaluate(() => (window as any).fixture.failCopy());
  await edit("Cut");
  await expect(
    page.getByRole("alert").filter({ hasText: "Clipboard unavailable" }),
  ).toBeVisible();
  await expect(input).toHaveValue("Keep  draft");

  await page.evaluate(() =>
    (window as any).fixture.hold("plugin:clipboard-manager|read_text"),
  );
  await edit("Paste");
  await input.fill("A newer edit");
  await page.evaluate(() =>
    (window as any).fixture.release("plugin:clipboard-manager|read_text"),
  );
  await expect(input).toHaveValue("A newer edit");
  await page.evaluate(() =>
    (window as any).fixture.hold("plugin:clipboard-manager|read_text"),
  );
  await edit("Paste");
  await page.keyboard.press("Meta+n");
  await input.fill("New session draft");
  await page.evaluate(() =>
    (window as any).fixture.release("plugin:clipboard-manager|read_text"),
  );
  await expect(input).toHaveValue("New session draft");
});

test("context menus edit modal fields and respect password, readonly and disabled inputs", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const project = page.getByRole("button", { name: "caelum", exact: true });
  await project.click({ button: "right" });
  await pickMenu(page, "Create worktree");
  const dialog = page.getByRole("dialog", {
    name: "Create worktree",
    exact: true,
  });
  const branch = dialog.getByRole("textbox", { name: "New branch" });
  await branch.fill("feature/search");
  await branch.evaluate((node: HTMLInputElement) =>
    node.setSelectionRange(8, 14),
  );
  await branch.press("Shift+F10");
  await expect(
    dialog.getByRole("menu", { name: "Context menu" }),
  ).toBeVisible();
  await pickMenu(page, "Copy");
  await expect.poll(() => copiedText(page)).toBe("search");
  await expect(branch).toBeFocused();
  expect(
    await branch.evaluate((node: HTMLInputElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([8, 14]);
  await branch.press("Shift+F10");
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  expect(
    await branch.evaluate((node: HTMLInputElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([8, 14]);
  await branch.press("Shift+F10");
  await page.keyboard.press("Home");
  await page.keyboard.press("ArrowDown");
  await expect(
    page.getByRole("menuitem", { name: "Cut", exact: true }),
  ).toBeFocused();
  await page.keyboard.press("Escape");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await expect(project).toBeFocused();

  await page.getByRole("button", { name: "Model", exact: true }).click();
  const picker = page.getByRole("dialog", {
    name: "Choose model",
    exact: true,
  });
  const modelSearch = picker.getByRole("combobox", { name: "Search models" });
  await modelSearch.fill("fast");
  await modelSearch.press("ControlOrMeta+a");
  await modelSearch.press("Shift+F10");
  await expect(
    picker.getByRole("menu", { name: "Context menu" }),
  ).toBeVisible();
  await pickMenu(page, "Copy");
  await expect.poll(() => copiedText(page)).toBe("fast");
  await expect(modelSearch).toBeFocused();
  await modelSearch.press("Escape");
  await expect(picker).toHaveCount(0);

  await page.keyboard.press("Meta+k");
  const search = page.locator('input[aria-label="Search sessions"]');
  await search.fill("secret");
  await search.evaluate((node: HTMLInputElement) => {
    node.type = "password";
    node.select();
  });
  await search.press("Shift+F10");
  await expect(
    page.getByRole("menuitem", { name: "Cut", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("menuitem", { name: "Copy", exact: true }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("menuitem", { name: "Paste", exact: true }),
  ).toBeEnabled();
  await page.keyboard.press("Escape");
  await search.evaluate((node: HTMLInputElement) => {
    node.type = "text";
    node.readOnly = true;
    node.select();
  });
  await search.press("Shift+F10");
  await expect(page.getByRole("menuitem")).toHaveText(["Copy", "Select all"]);
  await page.keyboard.press("Escape");
  await search.evaluate((node: HTMLInputElement) => {
    node.disabled = true;
  });
  await search.dispatchEvent("contextmenu");
  await expect(page.getByRole("menu")).toHaveCount(0);
});

test("context menus offer editing and precise message, code, link and tool actions", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft");
  await input.evaluate((node: HTMLTextAreaElement) =>
    node.setSelectionRange(5, 9),
  );
  await input.press("Shift+F10");
  await expect
    .poll(async () => (await contextMenu(page)).map((item) => item.text))
    .toEqual([
      "Undo",
      "Redo",
      "Separator",
      "Cut",
      "Copy",
      "Paste",
      "Separator",
      "Select all",
    ]);
  await pickMenu(page, "Copy");
  await expect.poll(() => copiedText(page)).toBe("this");
  await expect(input).toBeFocused();
  expect(
    await input.evaluate((node: HTMLTextAreaElement) => [
      node.selectionStart,
      node.selectionEnd,
    ]),
  ).toEqual([5, 9]);

  const message = page.locator(".user-text").first();
  const prompt = await message.innerText();
  await message.click({ button: "right" });
  await pickMenu(page, "Copy message");
  await expect.poll(() => copiedText(page)).toBe(prompt);
  await expect(input).toHaveValue("Keep this draft");
  await message.click({ button: "right" });
  await pickMenu(page, "Add to draft");
  await expect(input).toHaveValue(`Keep this draft\n\n${prompt}`);
  await expect(input).toBeFocused();

  await page.locator(".code-block pre").click({ button: "right" });
  await pickMenu(page, "Copy code");
  await expect.poll(() => copiedText(page)).toBe("const theme = 'near-black';");
  const link = page.getByRole("link", { name: "documentation", exact: true });
  await link.click({ button: "right" });
  await pickMenu(page, "Copy link address");
  await expect.poll(() => copiedText(page)).toBe("https://example.com/docs");
  await link.click({ button: "right" });
  await pickMenu(page, "Open link");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1)?.args.url,
      ),
    )
    .toBe("https://example.com/docs");

  const selected = page.locator(".markdown strong").first();
  await selected.evaluate((node) => {
    const range = document.createRange();
    range.selectNodeContents(node);
    window.getSelection()!.removeAllRanges();
    window.getSelection()!.addRange(range);
  });
  await selected.dispatchEvent("contextmenu", { bubbles: true });
  await pickMenu(page, "Copy selection");
  await expect.poll(() => copiedText(page)).toBe("more room");
  await page.evaluate(() => window.getSelection()!.removeAllRanges());
  await page
    .getByRole("button", { name: /Run pnpm test/ })
    .click({ button: "right" });
  await pickMenu(page, "Expand tool");
  await page
    .getByText("14 tests passed", { exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Copy output");
  await expect.poll(() => copiedText(page)).toBe("14 tests passed");
});

test("context menus act on the clicked project or session and reject stale actions after navigation", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Session A draft");
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  const project = page
    .getByRole("navigation", { name: "Open projects" })
    .getByRole("button", { name: "caelum", exact: true });
  await project.click({ button: "right" });
  await pickMenu(page, "Copy project path");
  await expect.poll(() => copiedText(page)).toBe("/work/caelum");
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await project.click({ button: "right" });
  await pickMenu(page, "Open project");
  await expect(input).toHaveValue("Session A draft");
  const other = page.getByRole("button", {
    name: "Review the provider adapters",
    exact: true,
  });
  await other.click({ button: "right" });
  await pickMenu(page, "Copy session ID");
  await expect.poll(() => copiedText(page)).toBe("thread-b");
  await expect(input).toHaveValue("Session A draft");
  await other.click({ button: "right" });
  await pickMenu(page, "Archive session");
  await expect(other).toHaveCount(0);
  await expect(input).toHaveValue("Session A draft");
  await input.press("Enter");
  const current = page.getByRole("button", {
    name: "Refine the session workspace",
    exact: true,
  });
  await current.click({ button: "right" });
  await expect
    .poll(
      async () =>
        (await contextMenu(page)).find(
          (item) => item.text === "Archive session",
        )?.enabled,
    )
    .toBe(false);
  await page.getByRole("button", { name: "Stop generation" }).click();
  await page.locator(".user-text").first().click({ button: "right" });
  const stale = await page
    .getByRole("menuitem", { name: "Add to draft", exact: true })
    .elementHandle();
  await page.keyboard.press("Meta+n");
  await expect(page.getByRole("menu")).toHaveCount(0);
  await input.fill("New draft");
  await stale!.evaluate((node: HTMLElement) => node.click());
  await expect(input).toHaveValue("New draft");

  await project.click({ button: "right" });
  await current.click();
  await expect(page.getByRole("menu")).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.filter((call: any) =>
          call.command.startsWith("plugin:menu|"),
        ).length,
    ),
  ).toBe(0);
});

test("file and diff context menus copy exact content, reveal paths, and use preview actions", async ({
  page,
}) => {
  await desktop(page);
  const css = page.getByRole("button", {
    name: "src/styles.css, Staged modified",
    exact: true,
  });
  await css.click({ button: "right" });
  await pickMenu(page, "Copy relative path");
  await expect.poll(() => copiedText(page)).toBe("src/styles.css");
  await expect(
    page.getByRole("button", {
      name: "src/App.tsx, Unstaged modified",
      exact: true,
    }),
  ).toHaveAttribute("aria-pressed", "true");
  await css.click({ button: "right" });
  await pickMenu(page, "Review changes");
  const patch = page.getByRole("region", { name: "Staged diff", exact: true });
  const text = await patch.locator("pre").textContent();
  await patch.locator("pre").click({ button: "right" });
  await pickMenu(page, "Copy staged diff");
  await expect.poll(() => copiedText(page)).toBe(text);

  await page.getByRole("button", { name: "Files", exact: true }).click();
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Expand folder");
  const file = page.getByRole("treeitem", {
    name: "App.tsx, file",
    exact: true,
  });
  await file.click({ button: "right" });
  await pickMenu(page, "Copy full path");
  await expect.poll(() => copiedText(page)).toBe("/work/caelum/src/App.tsx");
  await file.click({ button: "right" });
  const reveal = (await contextMenu(page)).find((item) =>
    item.text.startsWith("Reveal in"),
  )!.text;
  await pickMenu(page, reveal);
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter(
              (call: any) =>
                call.command === "plugin:opener|reveal_item_in_dir",
            )
            .at(-1)?.args.paths,
      ),
    )
    .toEqual(["/work/caelum/src/App.tsx"]);
  await file.click({ button: "right" });
  await pickMenu(page, "Open file");
  const toolbar = page.locator(".file-editor-toolbar");
  await toolbar.click({ button: "right" });
  await pickMenu(page, "Copy file contents");
  await expect
    .poll(() => copiedText(page))
    .toBe("export const ready = true;\n");
  const preview = page.getByRole("textbox", {
    name: "Read-only preview of src/App.tsx",
    exact: true,
  });
  await preview.click({ button: "right" });
  await pickMenu(page, "Copy file preview");
  await expect
    .poll(() => copiedText(page))
    .toBe("export const ready = true;\n");
  await preview.press("ControlOrMeta+a");
  await preview.press("Shift+F10");
  await pickMenu(page, "Copy selection");
  await expect
    .poll(() => copiedText(page))
    .toBe("export const ready = true;\n");
  await preview.press("Shift+F10");
  await pickMenu(page, "Add selection to prompt");
  await expect(attachedContext(page)).toContainText("src/App.tsx:1");
  await page.evaluate(() => (window as any).fixture.failCopy());
  await toolbar.click({ button: "right" });
  await pickMenu(page, "Copy file contents");
  await expect(
    page.getByRole("alert").filter({ hasText: "Clipboard unavailable" }),
  ).toBeVisible();
});
