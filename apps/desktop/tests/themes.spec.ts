import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { openSettings, findProjectFile } from "./fixtures/helpers.js";
import { themes } from "../src/domain/settings/themes.js";

const rgb = (hex: string) =>
  `rgb(${hex
    .slice(1)
    .match(/../g)!
    .map((part) => parseInt(part, 16))
    .join(", ")})`;

for (const [id, family] of Object.entries(themes)) {
  test(`${family.name} applies complete light, dark and system palettes and restores the saved family`, async ({
    page,
  }, info) => {
    await desktop(page);
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Preserve this session draft");
    const dialog = await openSettings(page);
    // Switch away first, including when the requested family is the default.
    await dialog
      .getByRole("radio", {
        name: id === "nord" ? "Caelum" : "Nord",
        exact: true,
      })
      .check();
    await dialog.getByRole("radio", { name: family.name, exact: true }).check();
    const root = page.locator("html");
    await expect(root).toHaveAttribute("data-theme-family", id);
    await expect(root).toHaveAttribute("data-accent", "theme");
    for (const mode of ["light", "dark"] as const) {
      await dialog
        .getByRole("radio", {
          name: mode === "light" ? "Light" : "Dark",
          exact: true,
        })
        .check();
      const palette = family[mode];
      await expect(root).toHaveCSS("background-color", rgb(palette.tokens.bg));
      await expect(root).toHaveCSS("color", rgb(palette.tokens.text));
      await expect(page.locator(".sidebar")).toHaveCSS(
        "background-color",
        rgb(palette.tokens.sidebar),
      );
      await expect(page.locator(".composer")).toHaveCSS(
        "background-color",
        rgb(palette.tokens["composer-bg"]),
      );
      await expect(page.locator(".brand-row")).toHaveCSS(
        "color",
        rgb(palette.accent),
      );
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
        .toBe(palette.tokens.bg);
    }
    await dialog.getByRole("radio", { name: "System", exact: true }).check();
    await page.emulateMedia({ colorScheme: "light" });
    await expect(root).toHaveCSS(
      "background-color",
      rgb(family.light.tokens.bg),
    );
    await page.emulateMedia({ colorScheme: "dark" });
    await expect(root).toHaveCSS(
      "background-color",
      rgb(family.dark.tokens.bg),
    );
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
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await expect(input).toHaveValue("Preserve this session draft");
    await page.reload();
    await expect(root).toHaveAttribute("data-theme-family", id);
    await expect(root).toHaveCSS(
      "background-color",
      rgb(family.dark.tokens.bg),
    );
    await openSettings(page);
    await expect(
      dialog.getByRole("radio", { name: family.name, exact: true }),
    ).toBeChecked();
    await page.screenshot({ path: info.outputPath(`${id}-settings.png`) });
    await page.setViewportSize({ width: 800, height: 620 });
    await dialog.getByRole("radio", { name: "Rose", exact: true }).check();
    await expect(
      dialog.getByRole("button", { name: "Done", exact: true }),
    ).toBeInViewport();
    expect(
      await dialog
        .locator(".settings-content")
        .evaluate((element) => element.scrollWidth <= element.clientWidth),
    ).toBe(true);
    await page.screenshot({ path: info.outputPath(`${id}-minimum.png`) });
  });
}

test("palette changes repaint mounted syntax and diff surfaces without replacing editors", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1600, height: 900 });
  await desktop(page);
  await findProjectFile(page, "src/App.tsx");
  await page
    .getByRole("button", { name: "Review changes", exact: true })
    .click();
  const editor = page.locator(".cm-editor.cm-merge-b");
  const token = editor.locator(".syntax-keyword").first();
  await expect(token).toBeVisible();
  const original = await token.elementHandle();
  const text = await editor.locator(".cm-content").innerText();
  const dialog = await openSettings(page);
  for (const family of [
    themes.catppuccin,
    themes.nord,
    themes["tokyo-night"],
    themes.caelum,
  ]) {
    await dialog.getByRole("radio", { name: family.name, exact: true }).check();
    for (const mode of ["dark", "light"] as const) {
      await dialog
        .getByRole("radio", {
          name: mode === "light" ? "Light" : "Dark",
          exact: true,
        })
        .check();
      await expect(token).toHaveCSS(
        "color",
        rgb(family[mode].tokens["syntax-keyword"]),
      );
      await expect(editor).toHaveCSS(
        "background-color",
        rgb(family[mode].tokens.bg),
      );
      expect(
        await editor
          .locator(".cm-changedLine")
          .first()
          .evaluate((element) => {
            const expected = document.createElement("span");
            expected.style.backgroundColor = "var(--added-bg)";
            element.append(expected);
            const matches =
              getComputedStyle(element).backgroundColor ===
              getComputedStyle(expected).backgroundColor;
            expected.remove();
            return matches;
          }),
      ).toBe(true);
    }
  }
  expect(await original!.evaluate((element) => element.isConnected)).toBe(true);
  await expect(editor.locator(".cm-content")).toHaveText(text);
});

test("Settings returns pointer focus quietly and keeps a visible keyboard focus ring", async ({
  page,
}) => {
  await desktop(page);
  const trigger = page.getByRole("button", { name: "Settings", exact: true });
  for (const collapsed of [false, true]) {
    if (collapsed)
      await page.getByRole("button", { name: "Collapse sidebar" }).click();
    await page
      .getByRole("textbox", { name: "Message", exact: true })
      .fill("Draft");
    const dialog = await openSettings(page, "Worktrees");
    await dialog
      .getByRole("textbox", { name: "Worktree folder" })
      .fill("~/checkouts");
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await page.mouse.move(5, 5);
    await expect(trigger).toBeFocused();
    await expect(trigger).toHaveCSS("outline-style", "none");
    await trigger.press("Enter");
    await expect(dialog).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(trigger).toBeFocused();
    await expect(trigger).toHaveCSS("outline-style", "solid");
    await trigger.press("Tab");
    await expect(trigger).not.toBeFocused();
  }
});
