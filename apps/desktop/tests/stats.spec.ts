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

test("slash stats aggregates OpenCode history, filters by project and dates and preserves context", async ({
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
  await dialog.getByLabel("Statistics project").selectOption("project-two");
  await dialog.getByRole("button", { name: "7 days", exact: true }).click();
  const week = await page.evaluate(() => {
    const to = new Date();
    to.setHours(0, 0, 0, 0);
    to.setDate(to.getDate() + 1);
    const from = new Date(to);
    from.setDate(from.getDate() - 7);
    return { from: from.getTime(), to: to.getTime() };
  });
  await expect
    .poll(async () => (await calls()).at(-1).filter)
    .toMatchObject({ project: "project-two", ...week });
  await dialog.getByRole("button", { name: "Custom", exact: true }).click();
  await dialog.getByLabel("Statistics from").fill("2026-10-01");
  await dialog.getByLabel("Statistics through").fill("2026-10-20");
  const bounds = await page.evaluate(() => ({
    from: new Date("2026-10-01T00:00:00").getTime(),
    to: new Date("2026-10-21T00:00:00").getTime(),
  }));
  await expect
    .poll(async () => (await calls()).at(-1).filter)
    .toMatchObject({ project: "project-two", ...bounds });
  await dialog.getByLabel("Statistics through").fill("2026-09-01");
  await expect(dialog.getByRole("alert")).toContainText("valid date range");
  await expect(dialog.getByRole("definition")).toHaveCount(0);
  await dialog.getByRole("button", { name: "All time", exact: true }).click();
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
  await dialog.getByLabel("Statistics project").selectOption("project-two");
  await expect(
    dialog.getByText("No recorded model usage in this range.", { exact: true }),
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
