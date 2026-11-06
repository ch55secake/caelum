import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit } from "./fixtures/helpers.js";

// Sample native animations at a known time instead of racing wall-clock delays.
async function toggleAt(page: Page, label: string, slot: string, time: number) {
  return page.evaluate(
    async ({ label, slot, time }) => {
      document
        .querySelector<HTMLButtonElement>(`[aria-label="${label}"]`)!
        .click();
      await new Promise(requestAnimationFrame);
      const node = document.querySelector<HTMLElement>(slot)!;
      const animation = node.getAnimations().find((animation) =>
        (animation.effect as KeyframeEffect)
          .getKeyframes()
          .some((frame) => "flexBasis" in frame),
      );
      if (!animation) throw new Error(`No layout motion for ${label}`);
      animation.pause();
      animation.currentTime = time;
      return node.getBoundingClientRect().width;
    },
    { label, slot, time },
  );
}

async function settle(page: Page) {
  await page.evaluate(() => {
    for (const node of document.querySelectorAll(".pane-slot"))
      for (const animation of node.getAnimations()) animation.finish();
  });
  await expect(page.locator(".pane-slot[data-motion]")).toHaveCount(0);
}

test("pane toggles move the layout together and reverse without jumping or losing focus", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Keep this draft through motion");
  const sidebar = page.locator(".sidebar-slot");
  const fullSidebar = (await sidebar.boundingBox())!.width;
  const closing = await toggleAt(page, "Collapse sidebar", ".sidebar-slot", 60);
  expect(closing).toBeGreaterThan(0);
  expect(closing).toBeLessThan(fullSidebar);
  expect((await page.locator(".main-panel").boundingBox())!.x).toBeCloseTo(
    closing,
    0,
  );
  expect((await page.locator(".sidebar").boundingBox())!.width).toBe(fullSidebar);
  await expect(sidebar).toHaveAttribute("inert", "");
  await expect(
    page.getByRole("button", { name: "Expand sidebar" }),
  ).toBeFocused();
  const reversed = await toggleAt(page, "Expand sidebar", ".sidebar-slot", 0);
  expect(reversed).toBeCloseTo(closing, 0);
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("textbox", { name: "Search sessions", exact: true }),
  ).toBeFocused();
  await settle(page);
  expect((await sidebar.boundingBox())!.width).toBe(fullSidebar);

  const inspector = page.locator(".inspector-slot");
  const fullInspector = (await inspector.boundingBox())!.width;
  const halfway = await toggleAt(page, "Hide changes", ".inspector-slot", 60);
  expect(halfway).toBeGreaterThan(0);
  expect(halfway).toBeLessThan(fullInspector);
  const conversation = (await page.locator(".session-slot").boundingBox())!;
  expect(conversation.x + conversation.width).toBeCloseTo(
    (await inspector.boundingBox())!.x,
    0,
  );
  const restored = await toggleAt(page, "Show changes", ".inspector-slot", 0);
  expect(restored).toBeCloseTo(halfway, 0);
  // Reverse again before entry finishes: closing contents must remain mounted.
  await toggleAt(page, "Hide changes", ".inspector-slot", 20);
  await expect(page.locator(".project-inspector")).toHaveCount(1);
  await settle(page);
  await expect(inspector).toBeHidden();
  await page.keyboard.press("Meta+p");
  await expect(
    page.getByRole("combobox", { name: "Find project file" }),
  ).toBeFocused();
  await settle(page);
  await expect(input).toHaveValue("Keep this draft through motion");
});

test("inspector expansion preserves the transcript, reading position and streaming follow", async ({
  page,
}) => {
  await desktop(page);
  const transcript = page.locator(".transcript");
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("A persistent draft");
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "motion-stream",
    delta: "A streamed paragraph.\n\n".repeat(40),
  });
  await expect(transcript).toContainText("A streamed paragraph.");
  await transcript.evaluate((node) => {
    node.setAttribute("data-mounted", "yes");
    node.scrollTop = 250;
  });
  await expect(
    page.getByRole("button", { name: "Jump to latest", exact: true }),
  ).toBeVisible();
  const width = (await page.locator(".inspector-slot").boundingBox())!.width;
  const expanding = await toggleAt(
    page,
    "Expand project inspector",
    ".inspector-slot",
    60,
  );
  expect(expanding).toBeGreaterThan(width);
  expect(expanding).toBeLessThan(
    (await page.locator(".session-layout").boundingBox())!.width,
  );
  await expect(page.locator(".session-slot")).toHaveAttribute("inert", "");
  await settle(page);
  await expect(input).toBeHidden();
  await page
    .getByRole("button", { name: "Restore conversation layout" })
    .click();
  await settle(page);
  await expect(transcript).toHaveAttribute("data-mounted", "yes");
  await expect
    .poll(() => transcript.evaluate((node) => node.scrollTop))
    .toBe(250);
  await expect(input).toHaveValue("A persistent draft");
  await page.getByRole("button", { name: "Jump to latest", exact: true }).click();
  await toggleAt(page, "Hide changes", ".inspector-slot", 60);
  await emit(page, "item/agentMessage/delta", {
    threadId: "thread-a",
    itemId: "motion-stream",
    delta: "More output while moving.\n\n".repeat(20),
  });
  await settle(page);
  await expect
    .poll(() =>
      transcript.evaluate(
        (node) => node.scrollHeight - node.scrollTop - node.clientHeight,
      ),
    )
    .toBeLessThanOrEqual(1);
});

test("resizing stays immediate and reduced motion snaps even during an interrupted transition", async ({
  page,
}) => {
  await desktop(page);
  const divider = page.getByRole("separator", {
    name: "Resize project inspector",
  });
  const inspector = page.locator(".inspector-slot");
  const width = (await inspector.boundingBox())!.width;
  await divider.press("ArrowLeft");
  expect((await inspector.boundingBox())!.width).toBe(width + 32);
  expect(await inspector.evaluate((node) => node.getAnimations().length)).toBe(0);
  const handle = (await divider.boundingBox())!;
  await page.mouse.move(handle.x + 4, handle.y + 100);
  await page.mouse.down();
  await page.mouse.move(handle.x - 76, handle.y + 100);
  expect((await inspector.boundingBox())!.width).toBe(width + 112);
  expect(await inspector.evaluate((node) => node.getAnimations().length)).toBe(0);
  await page.mouse.up();

  await toggleAt(page, "Hide changes", ".inspector-slot", 60);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await expect(inspector).toBeHidden();
  await page.getByRole("button", { name: "Show changes", exact: true }).click();
  expect((await inspector.boundingBox())!.width).toBe(width + 112);
  expect(await inspector.evaluate((node) => node.getAnimations().length)).toBe(0);
  await page.getByRole("button", { name: "Expand project inspector" }).click();
  expect((await inspector.boundingBox())!.width).toBe(
    (await page.locator(".session-layout").boundingBox())!.width,
  );
  await page.getByRole("button", { name: "Collapse sidebar" }).click();
  await expect(page.locator(".sidebar-slot")).toBeHidden();
  await page.keyboard.press("Meta+k");
  await expect(
    page.getByRole("textbox", { name: "Search sessions", exact: true }),
  ).toBeFocused();
  await expect(page.locator(".pane-slot[data-motion]")).toHaveCount(0);
});
