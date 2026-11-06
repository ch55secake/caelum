import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { expect, test, type Page } from "@playwright/test";
import workspace from "../../../docs/design/workspace.json" with { type: "json" };
import { desktop } from "../tests/fixtures/desktop.js";
import {
  defaultSettings,
  SETTINGS_KEY,
} from "../src/domain/settings/settingsState.js";

const output = process.env.CAELUM_PREVIEW_DIR ?? "preview-results";

async function capture(page: Page, name: string) {
  await expect(page.locator(".pane-slot[data-motion]")).toHaveCount(0);
  await page.locator(".transcript").evaluate((node) => { node.scrollTop = 0; });
  await page.mouse.move(0, 0);
  await page.screenshot({ path: join(output, name), animations: "disabled" });
}

for (const theme of ["dark", "light"] as const) {
  test(`workspace ${theme}`, async ({ page }) => {
    mkdirSync(output, { recursive: true });
    await page.addInitScript(
      ({ key, settings }) => localStorage.setItem(key, JSON.stringify(settings)),
      { key: SETTINGS_KEY, settings: { ...defaultSettings(), theme } },
    );
    await desktop(page, null);
    await page.getByRole("button", { name: "Hide changes", exact: true }).click();
    await expect(page.locator(".pane-slot[data-motion]")).toHaveCount(0);
    await page.evaluate((sample) => {
      const fixture = (window as any).fixture;
      fixture.setCodexHistory(
        [{
          id: "preview-turn",
          status: "completed",
          items: [
            {
              id: "preview-user", type: "userMessage",
              content: [{ type: "text", text: sample.prompt }],
            },
            {
              id: "preview-tool", type: "commandExecution",
              command: sample.command, aggregatedOutput: sample.output,
              status: "completed",
            },
            { id: "preview-answer", type: "agentMessage", text: sample.response },
          ],
        }],
        false,
      );
      fixture.setGitFiles([{
        path: "src/workspace.css", indexStatus: " ",
        worktreeStatus: "M", modifiedAt: "300",
      }]);
      fixture.setGitDiff("src/workspace.css", {
        staged: "", unstaged: sample.patch, untracked: "",
      });
    }, workspace);
    await page.getByRole("button", { name: workspace.title, exact: true }).click();
    await expect(page.locator(".transcript")).toContainText("A little more room to think");
    await page.getByRole("textbox", { name: "Message", exact: true }).fill(workspace.draft);
    await page.getByRole("textbox", { name: "Message", exact: true }).blur();
    // Refresh the shared Git snapshot before taking either layout.
    await page.getByRole("button", { name: "Show changes", exact: true }).click();
    await expect(page.locator(".git-diff-panel")).toContainText("--radius: 12px");
    await page.getByRole("button", { name: "Hide changes", exact: true }).click();
    await capture(page, `react-${theme}.png`);
    await page.getByRole("button", { name: "Show changes", exact: true }).click();
    await expect(page.locator(".git-diff-panel")).toContainText("--radius: 12px");
    await capture(page, `react-${theme}-inspector.png`);
    await page.setViewportSize({ width: 800, height: 620 });
    await expect(page.getByRole("textbox", { name: "Message", exact: true })).toBeInViewport();
    await capture(page, `react-${theme}-compact.png`);
  });
}
