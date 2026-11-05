import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { openSettings, copiedText } from "./fixtures/helpers.js";

const repository = "https://github.com/ch55secake/caelum";

test("About displays native metadata, copies details and manually checks for platform updates", async ({
  page,
}, info) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep my draft");
  const dialog = await openSettings(page, "About");
  const about = dialog.getByRole("region", { name: "About Caelum" });
  await expect(about.getByText("0.1.12", { exact: true })).toBeVisible();
  await expect(
    about.getByText("Apple silicon (arm64)", { exact: true }),
  ).toBeVisible();
  await about.getByRole("button", { name: "Copy application details" }).click();
  expect(await copiedText(page)).toBe(
    `Caelum 0.1.12\nPlatform: macos\nArchitecture: aarch64\n${repository}`,
  );
  for (const [name, href] of [
    ["Repository", repository],
    ["Releases", `${repository}/releases`],
    ["License", `${repository}/blob/main/LICENSE`],
  ]) {
    await expect(
      about.getByRole("link", { name, exact: true }),
    ).toHaveAttribute("href", href);
    await about.getByRole("link", { name, exact: true }).click();
    expect(
      await page.evaluate(
        () =>
          (window as any).fixture.calls
            .filter((call: any) => call.command === "plugin:opener|open_url")
            .at(-1).args.url,
      ),
    ).toBe(href);
  }
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "check_for_updates",
      ),
    ),
  ).toHaveLength(0);
  await page.evaluate(() => (window as any).fixture.hold("check_for_updates"));
  await about
    .getByRole("button", { name: "Check for updates", exact: true })
    .dblclick();
  await expect(
    about.getByRole("button", { name: "Checking…", exact: true }),
  ).toBeDisabled();
  await dialog.getByRole("button", { name: "Appearance", exact: true }).click();
  await page.evaluate(() =>
    (window as any).fixture.release("check_for_updates"),
  );
  await dialog.getByRole("button", { name: "About", exact: true }).click();
  await expect(about.getByRole("status")).toHaveText(
    "Caelum 0.2.0 is available.",
  );
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) => call.command === "check_for_updates",
      ),
    ),
  ).toHaveLength(1);
  await expect(
    about.getByRole("link", { name: "Release notes" }),
  ).toHaveAttribute("href", `${repository}/releases/tag/v0.2.0`);
  await expect(
    about.getByRole("link", { name: "Download update" }),
  ).toHaveAttribute(
    "href",
    `${repository}/releases/download/v0.2.0/Caelum-v0.2.0-aarch64.dmg`,
  );
  await page.setViewportSize({ width: 800, height: 620 });
  await about
    .getByRole("link", { name: "Download update" })
    .scrollIntoViewIfNeeded();
  await expect(
    dialog.getByRole("button", { name: "Done", exact: true }),
  ).toBeInViewport();
  await page.screenshot({ path: info.outputPath("about-minimum.png") });
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(input).toHaveValue("Keep my draft");
});

test("About retries failures, distinguishes current and ahead versions, and ignores dismissed checks", async ({
  page,
}) => {
  await desktop(page);
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("app_info", "Metadata unavailable"),
  );
  const dialog = await openSettings(page, "About");
  await expect(dialog.getByRole("alert")).toContainText("Metadata unavailable");
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("app_info", null),
  );
  await dialog.getByRole("button", { name: "Retry", exact: true }).click();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "check_for_updates",
      "GitHub is limiting update checks. Try again later.",
    ),
  );
  await dialog
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText(
    "limiting update checks",
  );
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("check_for_updates", null);
    fixture.setUpdateCheck({
      currentVersion: "0.1.12",
      latestVersion: "0.1.12",
      status: "upToDate",
      releaseUrl: "https://github.com/ch55secake/caelum/releases/tag/v0.1.12",
      downloadUrl: null,
    });
  });
  await dialog.getByRole("button", { name: "Retry update check" }).click();
  await expect(dialog.getByRole("status")).toContainText("You’re up to date");
  await expect(
    dialog.getByRole("link", { name: "Download update" }),
  ).toHaveCount(0);
  await page.evaluate(() => (window as any).fixture.hold("check_for_updates"));
  await dialog
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await page.keyboard.press("Escape");
  await openSettings(page, "About");
  await page.evaluate(() =>
    (window as any).fixture.release("check_for_updates"),
  );
  await expect(dialog.getByRole("status")).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.setUpdateCheck({
      currentVersion: "0.3.0-dev.1",
      latestVersion: "0.2.0",
      status: "ahead",
      releaseUrl: "https://github.com/ch55secake/caelum/releases/tag/v0.2.0",
      downloadUrl: null,
    }),
  );
  await dialog
    .getByRole("button", { name: "Check for updates", exact: true })
    .click();
  await expect(dialog.getByRole("status")).toContainText(
    "ahead of the latest stable release",
  );
  await expect(
    dialog.getByRole("link", { name: "Download update" }),
  ).toHaveCount(0);
});
