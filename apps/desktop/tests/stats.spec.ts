import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose, attachedContext } from "./fixtures/helpers.js";

const tokens = {
  input: 1000,
  output: 2000,
  reasoning: 3000,
  cache: { read: 4000, write: 5000 },
};
const stats = {
  range: { from: Date.UTC(2026, 9, 1), to: Date.UTC(2026, 10, 1) },
  sessions: 12,
  subagents: 2,
  prompts: 40,
  steps: 120,
  tokens,
  cost: 1.25,
  activeDays: 2,
  streak: 1,
  activity: [
    { date: "2026-10-01", steps: 100 },
    { date: "2026-10-20", steps: 20 },
  ],
  models: [
    {
      model: { providerID: "provider", id: "model", variant: "high" },
      steps: 120,
      tokens,
      cost: 1.25,
    },
  ],
};

test("slash stats searches projects by name and path across all recorded history and preserves context", async ({
  page,
}) => {
  await page.setViewportSize({ width: 800, height: 620 });
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate(
    (value) => (window as any).fixture.setStats(value),
    stats,
  );
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("@README");
  await page.getByRole("option", { name: "README.md", exact: true }).click();
  await input.fill("/stats");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", { name: "OpenCode statistics" });
  await expect(
    dialog.getByRole("definition").filter({ hasText: "15,000" }),
  ).toBeVisible();
  await expect(
    dialog.getByRole("region", { name: "Usage by model" }),
  ).toContainText("provider · high");
  await expect(
    dialog.getByRole("img", {
      name: "2026-10-01: 100 model steps",
      exact: true,
    }),
  ).toBeVisible();
  await expect(dialog.getByRole("columnheader")).toHaveCount(4);
  await dialog.screenshot({ path: "test-results/stats-compact.png" });
  await dialog
    .getByRole("button", { name: "Token details for model · provider · high" })
    .click();
  await expect(dialog.locator(".stats-model-tokens")).toContainText("5,000");
  const calls = () =>
    page.evaluate(() =>
      (window as any).fixture.calls
        .filter((call: any) => call.command === "opencode_stats")
        .map((call: any) => call.args),
    );
  expect((await calls())[0].filter).toMatchObject({
    project: null,
    from: null,
    to: null,
  });
  await expect(dialog.getByRole("group", { name: "Date range" })).toHaveCount(0);
  await expect(dialog.locator('input[type="date"]')).toHaveCount(0);
  const project = dialog.getByRole("button", { name: "Project", exact: true });
  await project.click();
  const search = dialog.getByRole("combobox", { name: "Search projects" });
  await expect(search).toBeFocused();
  await search.fill("/work/other");
  await expect(dialog.getByRole("option")).toHaveCount(1);
  await expect(dialog.getByRole("option", { name: "Other /work/other" })).toBeInViewport({ ratio: 1 });
  await dialog.screenshot({ path: "test-results/stats-project-picker.png" });
  await search.press("Enter");
  await expect
    .poll(async () => (await calls()).at(-1).filter)
    .toMatchObject({ project: "project-two", from: null, to: null });
  await expect(project).toBeFocused();
  await expect(project).toContainText("Other");
  await project.press("ArrowDown");
  await search.fill("Caelum");
  await expect(dialog.getByRole("option", { name: "Caelum /work/caelum" })).toBeVisible();
  await search.press("Escape");
  await expect(dialog).toBeVisible();
  await expect(project).toBeFocused();
  await expect(project).toContainText("Other");
  await choose(page, "Project", "All projects");
  await expect.poll(async () => (await calls()).at(-1).filter)
    .toMatchObject({ project: null, from: null, to: null });
  await expect(
    dialog.getByRole("definition").filter({ hasText: "15,000" }),
  ).toBeVisible();
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  await expect(input).toBeFocused();
  await expect(input).toHaveValue("");
  await expect(attachedContext(page)).toContainText("README.md");
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        [
          "opencode_start_session",
          "opencode_send_prompt",
          "opencode_run_command",
        ].includes(call.command),
      ),
    ),
  ).toEqual([]);
});

test("stats retain same-filter refresh results and reject delayed results after filter changes or dismissal", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate(
    (value) => (window as any).fixture.setStats(value),
    stats,
  );
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/stats");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", { name: "OpenCode statistics" });
  await expect(
    dialog.getByRole("definition").filter({ hasText: "15,000" }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("opencode_stats", "Offline"),
  );
  await dialog.getByRole("button", { name: "Refresh statistics" }).click();
  await expect(dialog.getByRole("alert")).toContainText(
    "last reported statistics",
  );
  await expect(
    dialog.getByRole("definition").filter({ hasText: "15,000" }),
  ).toBeVisible();
  await page.evaluate(() => {
    (window as any).fixture.setCommandError("opencode_stats", null);
    (window as any).fixture.hold("opencode_stats");
  });
  await dialog.getByRole("button", { name: "Retry statistics" }).click();
  await expect(dialog.getByRole("status")).toContainText("Loading statistics");
  await page.evaluate((value) => {
    (window as any).fixture.setStats({
      ...value,
      tokens: {
        input: 0,
        output: 0,
        reasoning: 0,
        cache: { read: 0, write: 0 },
      },
      steps: 0,
      models: [],
      activity: [],
    });
    (window as any).fixture.release("opencode_stats");
  }, stats);
  await choose(page, "Project", "Other /work/other");
  await expect(
    dialog.getByText("No recorded model usage.", { exact: true }),
  ).toBeVisible();
  await expect(
    dialog.getByRole("definition").filter({ hasText: "15,000" }),
  ).toHaveCount(0);
  await page.evaluate(() => (window as any).fixture.hold("opencode_stats"));
  await dialog.getByRole("button", { name: "Refresh statistics" }).click();
  await page.keyboard.press("Escape");
  await input.fill("Keep this draft");
  await page.evaluate(() => (window as any).fixture.release("opencode_stats"));
  await expect(dialog).toHaveCount(0);
  await expect(input).toHaveValue("Keep this draft");
});

test("stats abbreviate millions and billions with exact totals and token details available", async ({ page }) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page.evaluate((value) => {
    const tokens = { input: 1_234_567, output: 999_999, reasoning: 42, cache: { read: 2_400_000_000, write: 1_000_000 } };
    (window as any).fixture.setStats({ ...value, tokens, models: [{ ...value.models[0], tokens }] });
  }, stats);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("/stats");
  await input.press("Enter");
  const dialog = page.getByRole("dialog", { name: "OpenCode statistics" });
  await expect(dialog.locator(".stats-summary dd").first()).toHaveText("2.4B");
  await expect(dialog.locator(".stats-summary dd").first().locator("span")).toHaveAttribute("title", "2,403,234,608");
  await dialog.getByText("Token breakdown", { exact: true }).click();
  const breakdown = dialog.locator(".stats-token-details");
  await expect(breakdown.getByTitle("1,234,567", { exact: true })).toHaveText("1.2M");
  await expect(breakdown.getByTitle("999,999", { exact: true })).toHaveText("999,999");
  await expect(breakdown.getByTitle("1,000,000", { exact: true })).toHaveText("1M");
  await expect(breakdown.getByTitle("2,400,000,000", { exact: true })).toHaveText("2.4B");
  await dialog.getByRole("button", { name: "Token details for model · provider · high" }).click();
  await expect(dialog.locator(".stats-model-tokens").getByTitle("1,234,567", { exact: true })).toHaveText("1.2M");
});
