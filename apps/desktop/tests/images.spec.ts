import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose, setHistory } from "./fixtures/helpers.js";

test("chat renders inline images, opens previews and hides internal reminders", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  const png =
    "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAusB9Wl2nQAAAABJRU5ErkJggg==";
  await setHistory(
    page,
    "opencode",
    [
      { id: "internal", type: "system", text: "Internal instruction secret" },
      {
        id: "user-image",
        type: "user",
        text: "<system-reminder>Private reminder</system-reminder>\nExplain the screenshot",
        files: [
          {
            mime: "image/png",
            data: png.split(",")[1],
            name: "Attached screenshot",
          },
        ],
      },
      {
        id: "markdown-image",
        type: "assistant",
        content: [{ type: "text", text: `![Rendered screenshot](${png})` }],
      },
    ],
    false,
  );
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(
    page.getByText("Explain the screenshot", { exact: true }),
  ).toBeVisible();
  await expect(
    page.getByText(/Internal instruction secret|Private reminder/),
  ).toHaveCount(0);
  await expect(
    page.getByRole("img", { name: "Attached screenshot", exact: true }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Expand Rendered screenshot" })
    .click();
  await expect(
    page.getByRole("dialog", { name: "Image preview" }),
  ).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(page.getByRole("dialog", { name: "Image preview" })).toHaveCount(
    0,
  );
  await emit(
    page,
    "session.tool.success",
    {
      sessionID: "ses-a",
      id: "screenshot",
      name: "screenshot",
      content: [
        { type: "file", mime: "image/png", uri: png, name: "Live screenshot" },
      ],
    },
    "opencode",
  );
  await expect(
    page.getByRole("img", { name: "Live screenshot" }),
  ).toBeVisible();
  await page
    .getByRole("textbox", { name: "Message", exact: true })
    .press("ArrowUp");
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toHaveValue("Explain the screenshot");
  await page.setViewportSize({ width: 800, height: 620 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
});

test("image preview centres realistic images, preserves aspect ratios and supports zoom and panning", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  const images = await page.evaluate(() =>
    [
      { name: "Landscape", width: 2400, height: 1500 },
      { name: "Portrait", width: 900, height: 2400 },
      { name: "Small", width: 64, height: 48 },
    ].map((image) => {
      const canvas = document.createElement("canvas");
      canvas.width = image.width;
      canvas.height = image.height;
      const context = canvas.getContext("2d")!;
      context.fillStyle = "#212235";
      context.fillRect(0, 0, image.width, image.height);
      context.fillStyle = "#a5b4fc";
      context.fillRect(20, 20, image.width / 2, image.height / 2);
      context.font = "48px sans-serif";
      context.fillText(image.name, 40, image.height - 60);
      return { ...image, url: canvas.toDataURL() };
    }),
  );
  await setHistory(
    page,
    "opencode",
    images.map((image) => ({
      id: image.name,
      type: "assistant",
      content: [{ type: "text", text: `![${image.name}](${image.url})` }],
    })),
    false,
  );
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  for (const size of [
    { width: 1280, height: 800 },
    { width: 800, height: 620 },
  ]) {
    await page.setViewportSize(size);
    for (const image of images) {
      const thumbnail = page.getByRole("button", {
        name: `Expand ${image.name}`,
        exact: true,
      });
      await thumbnail.click();
      const dialog = page.getByRole("dialog", { name: "Image preview" });
      const preview = dialog.getByRole("img", {
        name: image.name,
        exact: true,
      });
      await expect(preview).toBeVisible();
      await expect(preview).toBeInViewport({ ratio: 1 });
      const box = (await dialog.boundingBox())!;
      expect(Math.abs(box.x + box.width / 2 - size.width / 2)).toBeLessThan(1);
      expect(Math.abs(box.y + box.height / 2 - size.height / 2)).toBeLessThan(
        1,
      );
      const fitted = (await preview.boundingBox())!;
      expect(
        Math.abs(fitted.width / fitted.height - image.width / image.height),
      ).toBeLessThan(0.01);
      expect(fitted.width).toBeLessThanOrEqual(image.width);
      await dialog
        .getByRole("button", { name: "Actual size", exact: true })
        .click();
      await expect
        .poll(async () => (await preview.boundingBox())!.width)
        .toBe(image.width);
      const canvas = dialog.getByRole("region", { name: "Image canvas" });
      if (image.width > size.width) {
        const before = await canvas.evaluate((node) => node.scrollLeft);
        const viewport = (await canvas.boundingBox())!;
        await page.mouse.move(viewport.x + 200, viewport.y + 100);
        await page.mouse.down();
        await page.mouse.move(viewport.x + 100, viewport.y + 60);
        await page.mouse.up();
        expect(
          await canvas.evaluate((node) => node.scrollLeft),
        ).toBeGreaterThan(before + 50);
      }
      await dialog.getByRole("button", { name: "Fit", exact: true }).click();
      await expect(preview).toBeInViewport({ ratio: 1 });
      if (image.name === "Landscape")
        await dialog.screenshot({
          path: `test-results/image-preview-${size.width}.png`,
        });
      await page.keyboard.press("Escape");
      await expect(dialog).toHaveCount(0);
      await expect(thumbnail).toBeFocused();
    }
  }
});
