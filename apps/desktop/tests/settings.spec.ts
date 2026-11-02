import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose, sessionRow, openSettings } from "./fixtures/helpers.js";

test("settings switches and persists complete themes, follows the system and preserves live drafts", async ({
  page,
}) => {
  await page.emulateMedia({ colorScheme: "light" });
  await desktop(page);
  const root = page.locator("html");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft through settings");
  await expect(root).toHaveAttribute("data-theme", "dark");
  const dialog = await openSettings(page);
  await page.screenshot({ path: "test-results/settings-dark.png" });
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await expect(root).toHaveAttribute("data-theme", "light");
  await expect(root).toHaveCSS("background-color", "rgb(250, 250, 252)");
  await expect(page.locator(".composer")).toHaveCSS(
    "background-color",
    "rgb(255, 255, 255)",
  );
  await expect(page.locator(".changes-pane")).toHaveCSS(
    "background-color",
    "rgb(245, 245, 249)",
  );
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "settings-live",
    delta: "Work continues while settings are open",
  });
  await page.keyboard.press("Meta+n");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(input).toHaveValue("Keep this draft through settings");
  await expect(
    page.getByText("Work continues while settings are open", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Settings", exact: true }),
  ).toBeFocused();
  await page.screenshot({ path: "test-results/workspace-light.png" });
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter(
              (call: any) =>
                call.command === "plugin:window|set_background_color",
            )
            .at(-1)?.args.value,
      ),
    )
    .toBe("#fafafc");

  await page.reload();
  await expect(root).toHaveAttribute("data-theme", "light");
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await openSettings(page);
  await expect(
    dialog.getByRole("radio", { name: "Light", exact: true }),
  ).toBeChecked();
  await dialog.getByRole("radio", { name: "System", exact: true }).check();
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(root).toHaveAttribute("data-theme", "dark");
  await page.emulateMedia({ colorScheme: "light" });
  await expect(root).toHaveAttribute("data-theme", "light");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:window|set_theme")
            .at(-1)?.args.value,
      ),
    )
    .toBe(null);
  await dialog.getByRole("radio", { name: "Dark", exact: true }).check();
  await expect(root).toHaveAttribute("data-theme", "dark");
  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
  await page.keyboard.press("Meta+,");
  await expect(dialog).toBeVisible();
  await page.setViewportSize({ width: 800, height: 620 });
  await expect(dialog).toBeInViewport();
  await expect(
    dialog.getByRole("button", { name: "Done", exact: true }),
  ).toBeInViewport();
  await page.mouse.click(5, 5);
  await expect(dialog).toHaveCount(0);
});

test("accent colours support keyboard selection, persist and adapt to explicit and system themes", async ({
  page,
}) => {
  await page.addInitScript(() => {
    if (!localStorage.getItem("caelum.settings.v1"))
      localStorage.setItem(
        "caelum.settings.v1",
        JSON.stringify({
          theme: "light",
          providers: { codex: { model: "provider/fast", effort: "low" } },
        }),
      );
  });
  await desktop(page);
  const root = page.locator("html");
  const brand = page.locator(".brand-row");
  await expect(root).toHaveAttribute("data-accent", "violet");
  await expect(brand).toHaveCSS("color", "rgb(108, 86, 176)");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my accent choice");
  const dialog = await openSettings(page);
  const colours = dialog.getByRole("group", {
    name: "Accent colour",
    exact: true,
  });
  await colours
    .getByRole("radio", { name: "Violet", exact: true })
    .press("ArrowRight");
  await expect(
    colours.getByRole("radio", { name: "Blue", exact: true }),
  ).toBeChecked();
  await page.keyboard.press("ArrowRight");
  await expect(
    colours.getByRole("radio", { name: "Teal", exact: true }),
  ).toBeChecked();
  await expect(
    colours.getByRole("radio", { name: "Teal", exact: true }),
  ).toBeFocused();
  await expect(brand).toHaveCSS("color", "rgb(33, 111, 104)");
  await expect(page.locator(".send-button")).toHaveCSS(
    "background-color",
    "rgb(35, 119, 111)",
  );
  await expect(page.locator(".latest-user")).toHaveCSS(
    "border-left-color",
    "rgb(33, 111, 104)",
  );
  await page.screenshot({ path: "test-results/settings-accent-light.png" });
  await dialog.getByRole("radio", { name: "Dark", exact: true }).check();
  await expect(brand).toHaveCSS("color", "rgb(114, 199, 191)");
  await expect(page.locator(".send-button")).toHaveCSS(
    "background-color",
    "rgb(35, 119, 111)",
  );
  await page.screenshot({ path: "test-results/settings-accent-dark.png" });
  await dialog.getByRole("radio", { name: "System", exact: true }).check();
  await page.emulateMedia({ colorScheme: "light" });
  await expect(brand).toHaveCSS("color", "rgb(33, 111, 104)");
  await page.emulateMedia({ colorScheme: "dark" });
  await expect(brand).toHaveCSS("color", "rgb(114, 199, 191)");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await input.press("Enter");
  await expect(page.locator(".composer")).toHaveAttribute(
    "data-working",
    "true",
  );
  await openSettings(page);
  await colours.getByRole("radio", { name: "Rose", exact: true }).check();
  await expect
    .poll(() =>
      page
        .locator(".composer")
        .evaluate(
          (element) => getComputedStyle(element, "::before").borderColor,
        ),
    )
    .toBe("rgb(231, 154, 176)");
  await colours.getByRole("radio", { name: "Teal", exact: true }).check();
  await page.reload();
  await expect(root).toHaveAttribute("data-accent", "teal");
  await expect(brand).toHaveCSS("color", "rgb(114, 199, 191)");
  expect(
    await page.evaluate(() =>
      JSON.parse(localStorage.getItem("caelum.settings.v1")!),
    ),
  ).toMatchObject({
    theme: "system",
    accentColor: "teal",
    providers: { codex: { model: "provider/fast", effort: "low" } },
  });
  await page.setViewportSize({ width: 800, height: 620 });
  await openSettings(page);
  await expect(
    colours.getByRole("radio", { name: "Teal", exact: true }),
  ).toBeChecked();
  await colours.getByRole("radio", { name: "Rose", exact: true }).check();
  await colours
    .getByRole("radio", { name: "Rose", exact: true })
    .press("ArrowLeft");
  await expect(
    colours.getByRole("radio", { name: "Amber", exact: true }),
  ).toBeChecked();
  await colours.getByRole("radio", { name: "Violet", exact: true }).check();
  await expect(
    colours.getByRole("radio", { name: "Violet", exact: true }),
  ).toBeChecked();
  await expect(brand).toHaveCSS("color", "rgb(163, 154, 246)");
  await expect(
    dialog.getByRole("button", { name: "Done", exact: true }),
  ).toBeInViewport();
});

test("settings themes and accents update mounted code previews and diffs", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await desktop(page);
  const dialog = await openSettings(page);
  await dialog.getByRole("radio", { name: "Blue", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page.getByRole("button", { name: "Files", exact: true }).click();
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click();
  await page
    .getByRole("treeitem", { name: "App.tsx, file", exact: true })
    .click();
  const editor = page.locator(".code-viewer .cm-editor").first();
  await expect(editor).toBeVisible();
  await expect(editor).toHaveCSS("background-color", "rgb(11, 12, 14)");
  await expect(editor.locator(".cm-activeLineGutter")).toHaveCSS(
    "color",
    "rgb(130, 180, 244)",
  );
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Teal", exact: true }).check();
  await expect(editor.locator(".cm-activeLineGutter")).toHaveCSS(
    "color",
    "rgb(114, 199, 191)",
  );
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(editor).toHaveCSS("background-color", "rgb(250, 250, 252)");
  await expect(editor.locator(".cm-activeLineGutter")).toHaveCSS(
    "color",
    "rgb(33, 111, 104)",
  );
  await editor.getByRole("textbox").press("ControlOrMeta+a");
  await expect(editor.locator(".cm-selectionBackground").first()).toHaveCSS(
    "background-color",
    "color(srgb 0.129412 0.435294 0.407843 / 0.2)",
  );
  await page
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  await expect(page.locator(".cm-mergeView")).toBeVisible();
  await expect(editor).toHaveCSS("background-color", "rgb(250, 250, 252)");
  const diffPanes = page.locator(".cm-mergeView .cm-editor");
  await expect(diffPanes).toHaveCount(2);
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("ControlOrMeta+a");
    await expect(pane.locator(".cm-selectionBackground").first()).toHaveCSS(
      "background-color",
      "color(srgb 0.129412 0.435294 0.407843 / 0.2)",
    );
  }
  await page.screenshot({ path: "test-results/files-light.png" });
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Rose", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("ControlOrMeta+a");
    await expect(pane.locator(".cm-selectionBackground").first()).toHaveCSS(
      "background-color",
      "color(srgb 0.647059 0.254902 0.388235 / 0.2)",
    );
  }
  await openSettings(page);
  await dialog.getByRole("radio", { name: "Dark", exact: true }).check();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(editor).toHaveCSS("background-color", "rgb(11, 12, 14)");
  for (const pane of await diffPanes.all()) {
    await pane.getByRole("textbox").press("ControlOrMeta+a");
    await expect(pane.locator(".cm-selectionBackground").first()).toHaveCSS(
      "background-color",
      "color(srgb 0.905882 0.603922 0.690196 / 0.28)",
    );
  }
});

for (const provider of ["codex", "opencode"]) {
  test(`${provider} settings persist new-session defaults and keep existing sessions and draft overrides independent`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
    }
    const name = provider === "codex" ? "Codex" : "OpenCode";
    const dialog = await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await expect(
      dialog.getByRole("status", { name: `${name} connection status` }),
    ).toHaveText("Available");
    await choose(page, "Default model", "Fast model provider/fast");
    await choose(page, "Default effort", "Low");
    if (provider === "opencode") await choose(page, "Default agent", "Build");
    await page.screenshot({ path: `test-results/${provider}-settings.png` });
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Test model");
    await page.reload();
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await expect(
      dialog.getByRole("button", { name: "Default model", exact: true }),
    ).toContainText("Fast model");
    await expect(
      dialog.getByRole("button", { name: "Default effort", exact: true }),
    ).toHaveText("Low");
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await page.keyboard.press("Meta+n");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Fast model");
    await choose(page, "Model", "Fast model provider/fast");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("Low");
    if (provider === "opencode")
      await expect(
        page.getByRole("button", { name: "Build", exact: true }),
      ).toHaveAttribute("aria-pressed", "true");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Use my saved defaults");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    expect(
      calls.find((call: any) => call.command === `${provider}_start_session`)
        .args,
    ).toMatchObject({
      model: "provider/fast",
      agent: provider === "opencode" ? "build" : null,
    });
    expect(
      calls.find((call: any) => call.command === `${provider}_send_prompt`).args
        .effort,
    ).toBe("low");
    await page.getByRole("button", { name: "Stop generation" }).click();
    await page.keyboard.press("Meta+n");
    await choose(page, "Model", "Test model provider/model");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("Default effort");
    await input.fill("Keep this draft override");
    await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await expect(
      dialog.getByRole("button", { name: "Default model", exact: true }),
    ).toContainText("Fast model");
    await choose(page, "Default model", "Simple model provider/plain");
    await expect(
      dialog.getByRole("button", { name: "Default effort", exact: true }),
    ).toHaveText("Model default");
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await expect(input).toHaveValue("Keep this draft override");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Test model");
  });
}

test("provider settings recover connection errors and isolate delayed capability checks", async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 620 });
  await desktop(page);
  await page.evaluate(() =>
    (window as any).fixture.setChoicesError(
      "codex",
      "Sign in with codex login",
    ),
  );
  const dialog = await openSettings(page, "Providers");
  await expect(
    dialog.getByRole("status", { name: "Codex connection status" }),
  ).toHaveText("Unavailable");
  await expect(dialog.getByRole("alert")).toHaveText(
    "Error: Sign in with codex login",
  );
  await page.evaluate(() => {
    (window as any).fixture.setChoicesError("codex", null);
    (window as any).fixture.hold("provider_choices:codex");
  });
  await dialog.getByRole("button", { name: "Refresh Codex status" }).click();
  await expect(
    dialog.getByRole("status", { name: "Codex connection status" }),
  ).toHaveText("Checking…");
  await dialog.getByRole("button", { name: "OpenCode", exact: true }).click();
  await expect(
    dialog.getByRole("status", { name: "OpenCode connection status" }),
  ).toHaveText("Available");
  await page.evaluate(() =>
    (window as any).fixture.release("provider_choices:codex"),
  );
  await expect(
    dialog.getByRole("status", { name: "OpenCode connection status" }),
  ).toHaveText("Available");
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  await dialog.getByRole("button", { name: "Codex", exact: true }).click();
  await expect(
    dialog.getByRole("status", { name: "Codex connection status" }),
  ).toHaveText("Available");
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .press("ArrowDown");
  await expect(
    page.getByRole("dialog", { name: "Choose default model", exact: true }),
  ).toBeInViewport();
  await page.keyboard.press("Escape");
  await expect(dialog).toBeVisible();
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toBeFocused();
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .press("ArrowDown");
  await page
    .getByRole("combobox", { name: "Search default models" })
    .fill("fast");
  await page.keyboard.press("Enter");
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toContainText("Fast model");
  await dialog.getByRole("button", { name: "Reset Codex defaults" }).click();
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toHaveText("Provider default");
  await choose(page, "Default provider", "OpenCode");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page.getByRole("button", { name: "Close caelum", exact: true }).click();
  await openSettings(page, "Providers");
  await expect(dialog.getByRole("status")).toHaveText("No project selected");
  await expect(
    dialog.getByRole("button", { name: "Default model", exact: true }),
  ).toBeDisabled();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await page
    .getByRole("button", { name: "Open project", exact: true })
    .last()
    .click();
  await expect(
    page.getByRole("button", { name: "Provider", exact: true }),
  ).toHaveText("OpenCode");
  await expect
    .poll(() =>
      page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter(
              (call: any) =>
                call.command === "set_project_provider" &&
                call.args.projectPath === "/work/other",
            )
            .at(-1)?.args.provider,
      ),
    )
    .toBe("opencode");
});

test("settings recover damaged preferences and report storage failures with a working retry", async ({
  page,
}) => {
  await page.addInitScript(() => {
    localStorage.setItem("caelum.settings.v1", "{broken");
    const setItem = Storage.prototype.setItem;
    (window as any).restoreStorage = () => {
      Storage.prototype.setItem = setItem;
    };
    Storage.prototype.setItem = function (key, value) {
      if (key === "caelum.settings.v1") throw new Error("Storage unavailable");
      setItem.call(this, key, value);
    };
  });
  await desktop(page);
  const dialog = await openSettings(page);
  await expect(dialog.getByRole("alert")).toContainText(
    "Could not read saved settings",
  );
  await dialog.getByRole("radio", { name: "Light", exact: true }).check();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(dialog.getByRole("alert")).toContainText(
    "Could not save settings",
  );
  await page.evaluate(() => (window as any).restoreStorage());
  await dialog.getByRole("button", { name: "Retry saving" }).click();
  await expect(dialog.getByRole("alert")).toHaveCount(0);
  expect(
    await page.evaluate(
      () => JSON.parse(localStorage.getItem("caelum.settings.v1")!).theme,
    ),
  ).toBe("light");
});

for (const provider of ["codex", "opencode"]) {
  const title =
    provider === "codex" ? "Refine the session workspace" : "OpenCode session";
  const id = provider === "codex" ? "thread-a" : "ses-a";
  test(`${provider} archives old sessions, restores drafts and settings with Undo, and stays removed after restart`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const row = sessionRow(page, title);
    await expect(row.locator(".session-branch")).toHaveAttribute(
      "title",
      provider === "codex"
        ? "Session branch: feat/session-ui-redesign"
        : "Session branch: main",
    );
    await expect(row.locator(".session-lines")).toHaveAttribute(
      "title",
      "Lines changed in the latest turn",
    );
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep my unsent draft");
    await choose(page, "Model", "Fast model provider/fast");
    await choose(page, "Effort", "Low");
    await row.hover();
    await row
      .getByRole("button", { name: "Archive session", exact: true })
      .click();
    await expect(row).toHaveCount(0);
    await expect(
      page.getByRole("heading", { name: "New session", exact: true }),
    ).toBeVisible();
    const notice = page.getByRole("status", { name: "Session archived" });
    await expect(notice).toContainText(title);
    await page.evaluate(() => (window as any).fixture.failArchive());
    await notice.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(page.getByRole("alert")).toContainText(
      "Could not restore session",
    );
    await expect(row).toHaveCount(0);
    await page.getByRole("button", { name: "Dismiss error" }).click();
    await notice.getByRole("button", { name: "Undo", exact: true }).click();
    await expect(input).toBeEnabled();
    await expect(input).toHaveValue("Keep my unsent draft");
    await expect(
      page.getByRole("button", { name: "Model", exact: true }),
    ).toContainText("Fast model");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("Low");
    await expect(row).toHaveCount(1);
    await expect(notice).toHaveCount(0);
    await row.hover();
    await row
      .getByRole("button", { name: "Archive session", exact: true })
      .click();
    await page.getByRole("button", { name: "Refresh sessions" }).click();
    await expect(row).toHaveCount(0);
    const forgets = await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "forget_project_session",
      ),
    );
    expect(forgets.map((call: any) => call.args)).toEqual(
      Array(2).fill({ projectPath: "/work/caelum", provider, sessionId: id }),
    );
    await page.reload();
    await expect(input).toBeEnabled();
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await expect(row).toHaveCount(0);
  });
}
