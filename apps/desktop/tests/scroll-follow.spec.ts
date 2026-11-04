import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit } from "./fixtures/helpers.js";

const gap = (page: Page) =>
  page
    .locator(".transcript")
    .evaluate((node) => node.scrollHeight - node.scrollTop - node.clientHeight);
const stream = (page: Page, delta: string) =>
  emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "scrolling",
    delta,
  });

test("opening tool details keeps following new messages until the user scrolls up", async ({
  page,
}) => {
  await desktop(page);
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);
  await page.getByRole("button", { name: /Run pnpm test/ }).click();
  await stream(page, "A new response\n\n".repeat(20) + "Latest response");
  await expect(page.locator(".transcript")).toContainText("Latest response");
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toHaveCount(0);
});

test("even a one-pixel upward scroll pauses following and returning to the bottom resumes it", async ({
  page,
}) => {
  await desktop(page);
  await stream(page, "Streaming response\n\n".repeat(25));
  const transcript = page.locator(".transcript");
  await expect(transcript).toContainText("Streaming response");
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);
  const position = () => transcript.evaluate((node) => node.scrollTop);
  const bottom = await position();
  await transcript.hover();
  await page.mouse.wheel(0, -1);
  await expect.poll(position).toBeCloseTo(bottom - 1, 0);
  const jump = page.getByRole("button", {
    name: "Jump to latest",
    exact: true,
  });
  await expect(jump).toBeVisible();
  await page.mouse.wheel(0, -23);
  await expect.poll(position).toBeCloseTo(bottom - 24, 0);
  const reading = await position();
  await stream(page, "More output\n\n".repeat(10));
  await expect(transcript).toContainText("More output");
  await expect.poll(position).toBeCloseTo(reading, 0);
  await page.setViewportSize({ width: 1000, height: 700 });
  await expect.poll(position).toBeCloseTo(reading, 0);

  await transcript.hover();
  await page.mouse.wheel(0, 10000);
  await expect(jump).toHaveCount(0);
  await stream(page, "Following again\n\n".repeat(10));
  await expect(transcript).toContainText("Following again");
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);
});

test("nested scrolling keeps following and history refreshes respect keyboard reading positions", async ({
  page,
}) => {
  await desktop(page);
  await emit(page, "item/reasoning/summaryTextDelta", {
    threadId: "thread-a",
    itemId: "thinking",
    summaryIndex: 0,
    delta: "Considering the implementation\n\n".repeat(40),
  });
  await page.getByLabel("Thinking trace", { exact: true }).click();
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);
  const trace = page.locator(".reasoning-content");
  await trace.evaluate((node) => {
    node.scrollTop = 120;
  });
  await trace.hover();
  await page.mouse.wheel(0, -24);
  await expect.poll(() => trace.evaluate((node) => node.scrollTop)).toBe(96);
  const jump = page.getByRole("button", { name: "Jump to latest", exact: true });
  await expect(jump).toHaveCount(0);
  await stream(page, "Following after nested scrolling\n\n".repeat(10));
  const transcript = page.locator(".transcript");
  await expect(transcript).toContainText("Following after nested scrolling");
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);

  // A refresh can start after Home pauses following but before the browser's
  // native keyboard scroll has moved away from the bottom.
  await page.evaluate(() => {
    (window as any).fixture.holdHistory();
    document.addEventListener(
      "keydown",
      () => window.dispatchEvent(new Event("focus")),
      { once: true },
    );
  });
  await transcript.focus();
  await expect(transcript).toBeFocused();
  await page.keyboard.press("Home");
  await expect.poll(() => transcript.evaluate((node) => node.scrollTop)).toBe(0);
  await expect(jump).toBeVisible();
  await expect(
    page.getByRole("button", { name: "Loading…", exact: true }),
  ).toBeDisabled();
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  const history = page.getByRole("button", {
    name: "Load earlier messages",
    exact: true,
  });
  await expect(history).toBeEnabled();
  await expect.poll(() => transcript.evaluate((node) => node.scrollTop)).toBe(0);
  await stream(page, "Output while reading earlier messages");
  await expect(transcript).toContainText("Output while reading earlier messages");
  await expect.poll(() => transcript.evaluate((node) => node.scrollTop)).toBe(0);
  await page.evaluate(() => {
    (window as any).fixture.holdHistory();
    window.dispatchEvent(new Event("focus"));
  });
  await expect(
    page.getByRole("button", { name: "Loading…", exact: true }),
  ).toBeDisabled();
  await page.keyboard.press("End");
  await expect(jump).toHaveCount(0);
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await expect(history).toBeEnabled();
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);
  await stream(page, "\n\nFollowing after keyboard scrolling\n\n".repeat(10));
  await expect(transcript).toContainText("Following after keyboard scrolling");
  await expect.poll(() => gap(page)).toBeLessThanOrEqual(1);
});
