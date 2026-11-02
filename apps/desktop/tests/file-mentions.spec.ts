import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { attachedContext, choose } from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"]) {
  test(`${provider} file mentions complete at the caret and send captured snapshots`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await page.evaluate(() =>
      (window as any).fixture.setProjectFile(
        "docs/hello world.md",
        "Snapshot α\n",
      ),
    );
    await input.fill("Review @hello carefully");
    for (let i = 0; i < 10; i++) await input.press("ArrowLeft");
    const list = page.getByRole("listbox", { name: "File mentions" });
    await expect(
      list.getByRole("option", { name: "docs/hello world.md" }),
    ).toBeVisible();
    await expect(input).toHaveAttribute("aria-activedescendant", /.+/);
    await input.press("Tab");
    await expect(input).toHaveValue('Review @"docs/hello world.md" carefully');
    expect(
      await input.evaluate((node: HTMLTextAreaElement) => node.selectionStart),
    ).toBe('Review @"docs/hello world.md" '.length);
    await expect(attachedContext(page)).toContainText("docs/hello world.md");
    await expect(list).toHaveCount(0);
    await expect(input).toBeFocused();
    await page.evaluate(() =>
      (window as any).fixture.setProjectFile(
        "docs/hello world.md",
        "Changed later",
      ),
    );
    await input.press("ControlOrMeta+End");
    await input.press("Enter");
    await expect(input).toHaveValue("");
    const prompt = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === `${provider}_send_prompt`,
        ).args.prompt,
      provider,
    );
    expect(prompt).toContain("Snapshot α");
    expect(prompt).not.toContain("Changed later");
    expect(prompt).toContain('Review @"docs/hello world.md" carefully');
  });
}

test("file completion rejects stale searches and reads, retains failures and isolates navigation", async ({
  page,
}) => {
  await desktop(page);
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  const list = page.getByRole("listbox", { name: "File mentions" });
  await page.evaluate(() =>
    (window as any).fixture.hold("search_project_files:App"),
  );
  await input.fill("Inspect @App");
  await expect
    .poll(() =>
      page.evaluate(() =>
        (window as any).fixture.calls.some(
          (call: any) =>
            call.command === "search_project_files" &&
            call.args.query === "App",
        ),
      ),
    )
    .toBe(true);
  await input.fill("Inspect @README");
  await expect(
    list.getByRole("option", { name: "README.md", exact: true }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.release("search_project_files:App"),
  );
  await expect(list.getByRole("option")).toHaveCount(1);
  await page.evaluate(() =>
    (window as any).fixture.setProjectFile("README.md", null),
  );
  await input.press("Enter");
  await expect(
    page.getByRole("alert").filter({ hasText: "File not found" }),
  ).toBeVisible();
  await expect(input).toHaveValue("Inspect @README");
  await expect(attachedContext(page)).toHaveCount(0);
  await input.fill("Inspect @App");
  await expect(
    list.getByRole("option", { name: "src/App.tsx", exact: true }),
  ).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:src/App.tsx"),
  );
  await input.press("Enter");
  await expect(
    page.getByRole("status").filter({ hasText: "Attaching snapshot" }),
  ).toBeVisible();
  await input.fill("Newer draft");
  await page.evaluate(() =>
    (window as any).fixture.release("read_project_file:src/App.tsx"),
  );
  await expect(input).toHaveValue("Newer draft");
  await expect(attachedContext(page)).toHaveCount(0);
  await input.fill("Inspect @App");
  await expect(list.getByRole("option")).toBeVisible();
  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:src/App.tsx"),
  );
  await input.press("Tab");
  await expect(
    page.getByRole("status").filter({ hasText: "Attaching snapshot" }),
  ).toBeVisible();
  await page
    .getByRole("button", { name: "Review the provider adapters", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.release("read_project_file:src/App.tsx"),
  );
  await expect(attachedContext(page)).toHaveCount(0);
  await page
    .getByRole("button", { name: "Refine the session workspace", exact: true })
    .click();
  await expect(input).toHaveValue("Inspect @App");
  await expect(attachedContext(page)).toHaveCount(0);
  await input.fill("mail@example.com");
  await expect(list).toHaveCount(0);
  await input.fill("@missing");
  await expect(
    page.getByText("No matching files.", { exact: true }),
  ).toBeVisible();
  await input.press("Enter");
  await expect(input).toHaveValue("@missing");
  await input.press("Escape");
  await expect(list).toHaveCount(0);
  await input.fill("/new");
  await expect(
    page.getByRole("listbox", { name: "Slash commands" }),
  ).toBeVisible();
});

test("file mentions deduplicate snapshots, enforce attachment limits and mark partial text", async ({
  page,
}) => {
  await desktop(page, null);
  await page.evaluate(() => {
    for (let i = 0; i < 17; i++)
      (window as any).fixture.setProjectFile(
        `sample-${i}.txt`,
        i === 0 ? "α".repeat(40000) : `file ${i}`,
      );
  });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  for (const i of [
    0,
    0,
    ...Array.from({ length: 16 }, (_, index) => index + 1),
  ]) {
    await input.fill(`Read @sample-${i}.txt`);
    await page
      .getByRole("option", { name: `sample-${i}.txt`, exact: true })
      .click();
    if (i < 16) await expect(input).toHaveValue(`Read @sample-${i}.txt `);
  }
  await expect(attachedContext(page).locator(".file-context-chip")).toHaveCount(
    16,
  );
  await expect(input).toHaveValue("Read @sample-16.txt");
  await expect(page.locator(".slash-note[role=alert]")).toContainText(
    "up to 16",
  );
  await input.press("Escape");
  await input.fill("Review these files");
  await input.press("Enter");
  const prompt = await page.evaluate(
    () =>
      (window as any).fixture.calls.find(
        (call: any) => call.command === "codex_send_prompt",
      ).args.prompt,
  );
  const payload = JSON.parse(
    prompt
      .split("<caelum-file-context>\n")[1]
      .split("\n</caelum-file-context>")[0],
  );
  expect(payload.files).toHaveLength(16);
  expect(payload.files[0]).toMatchObject({
    path: "sample-0.txt",
    truncated: true,
  });
  expect(new TextEncoder().encode(payload.files[0].content)).toHaveLength(
    65536,
  );
});
