import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "@playwright/test";
import { desktop } from "../tests/fixtures/desktop.js";

test.use({ video: { mode: "on", size: { width: 1280, height: 1000 } } });

test("workspace pane motion", async ({ page }) => {
  const output = process.env.CAELUM_PREVIEW_DIR ?? "preview-results";
  mkdirSync(output, { recursive: true });
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep the draft and conversation in place as the panes move.");
  // Exercise the macOS titlebar geometry in the WebKit replay as well.
  await page
    .locator(".app-shell")
    .evaluate((node) => node.classList.add("macos-titlebar-overlay"));
  for (const theme of ["dark", "light"]) {
    await page.evaluate((theme) => {
      document.documentElement.dataset.theme = theme;
    }, theme);
    for (const label of [
      "Collapse sidebar",
      "Expand sidebar",
      "Hide changes",
      "Show changes",
      "Expand project inspector",
      "Restore conversation layout",
    ]) {
      await page.getByRole("button", { name: label, exact: true }).click();
      await expect(page.locator(".pane-slot[data-motion]")).toHaveCount(0);
      // Deliberate viewing pauses for the recording, not test synchronization.
      await page.waitForTimeout(400);
    }
  }
  await expect(input).toHaveValue(
    "Keep the draft and conversation in place as the panes move.",
  );
  await page.close();
  await page.video()!.saveAs(join(output, "react-pane-motion.webm"));
});
