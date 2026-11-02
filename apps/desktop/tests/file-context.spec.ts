import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  choose,
  findProjectFile,
  attachedContext,
  pickMenu,
} from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"]) {
  test(`${provider} Files context sends exact file and selection snapshots, removes chips and restores failed sends`, async ({
    page,
  }) => {
    await page.setViewportSize({ width: 1500, height: 900 });
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page
        .getByRole("button", { name: "OpenCode session", exact: true })
        .click();
    }
    await page.evaluate(() =>
      (window as any).fixture.setProjectFile(
        "src/App.tsx",
        "export const selected = true;\nexport const untouched = false;\n",
      ),
    );
    await findProjectFile(page, "src/App.tsx");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await attachedContext(page)
      .getByRole("button", { name: "Remove src/App.tsx context", exact: true })
      .click();
    await expect(attachedContext(page)).toHaveCount(0);
    const editorInput = page.getByRole("textbox", {
      name: "Read-only preview of src/App.tsx",
      exact: true,
    });
    await editorInput.focus();
    await editorInput.press("ControlOrMeta+Home");
    await editorInput.press("Shift+ArrowDown");
    await page
      .getByRole("button", { name: "Ask about selection", exact: true })
      .click();
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(input).toBeFocused();
    await expect(input).toHaveValue("Explain the selected code.");
    await expect(attachedContext(page)).toContainText("src/App.tsx:1");
    await findProjectFile(page, "README.md");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(
      page.getByRole("alert").filter({ hasText: "Connection refused" }),
    ).toBeVisible();
    await expect(
      attachedContext(page).locator(".file-context-chip"),
    ).toHaveCount(2);
    await expect(input).toHaveValue("Explain the selected code.");
    await page.getByRole("button", { name: /New session/ }).click();
    await expect(attachedContext(page)).toHaveCount(0);
    await page
      .getByRole("button", {
        name:
          provider === "codex"
            ? "Refine the session workspace"
            : "OpenCode session",
        exact: true,
      })
      .click();
    await expect(
      attachedContext(page).locator(".file-context-chip"),
    ).toHaveCount(2);
    await input.press("Enter");
    await expect(attachedContext(page)).toHaveCount(0);
    await expect(
      page.locator('[aria-label="Sent file context"] .file-context-chip'),
    ).toHaveCount(2);
    await expect(page.locator(".user-text").last()).toHaveText(
      "Explain the selected code.",
    );
    const prompts = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === `${provider}_send_prompt`)
          .map((call: any) => call.args.prompt),
      provider,
    );
    expect(prompts).toHaveLength(2);
    expect(prompts[1]).toBe(prompts[0]);
    const payload = JSON.parse(
      prompts[1]
        .split("<caelum-file-context>\n")[1]
        .split("\n</caelum-file-context>")[0],
    );
    expect(payload.files).toEqual([
      expect.objectContaining({
        path: "src/App.tsx",
        content: "export const selected = true;\n",
        startLine: 1,
        endLine: 1,
        truncated: false,
      }),
      expect.objectContaining({
        path: "README.md",
        content: "# Caelum\nProject documentation.\n",
        truncated: false,
      }),
    ]);
    await input.press("ArrowUp");
    await expect(input).toHaveValue("Explain the selected code.");
    await page.screenshot({
      path: `test-results/${provider}-file-context.png`,
    });
  });
}

for (const provider of ["codex", "opencode"]) {
  test(`${provider} Files context transfers on first send and isolates new-session provider drafts`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await findProjectFile(page, "src/App.tsx");
    await page
      .locator(".file-editor-toolbar")
      .getByRole("button", { name: "Add to prompt", exact: true })
      .click();
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Use this snapshot");
    await choose(page, "Provider", provider === "codex" ? "OpenCode" : "Codex");
    await expect(attachedContext(page)).toHaveCount(0);
    await choose(page, "Provider", provider === "codex" ? "Codex" : "OpenCode");
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await expect(input).toHaveValue("Use this snapshot");
    await page.evaluate(() => {
      (window as any).fixture.holdStart();
      (window as any).fixture.failSend();
    });
    await input.press("Enter");
    await expect(input).toBeDisabled();
    await page
      .getByRole("button", {
        name: "Refine the session workspace",
        exact: true,
      })
      .click();
    await expect(attachedContext(page)).toHaveCount(0);
    await page.evaluate(() => (window as any).fixture.releaseStart());
    await expect(
      page.getByRole("button", { name: "Use this snapshot", exact: true }),
    ).toBeVisible();
    await expect(page.locator(".breadcrumbs strong")).toHaveText(
      "Refine the session workspace",
    );
    await page
      .getByRole("button", { name: "Use this snapshot", exact: true })
      .click();
    await expect(
      page.getByRole("alert").filter({ hasText: "Connection refused" }),
    ).toBeVisible();
    await expect(attachedContext(page)).toContainText("src/App.tsx");
    await input.press("Enter");
    await expect(
      page.locator('[aria-label="Sent file context"]'),
    ).toContainText("src/App.tsx");
    await expect(attachedContext(page)).toHaveCount(0);
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    expect(
      calls.filter((call: any) => call.command === `${provider}_start_session`),
    ).toHaveLength(1);
    expect(
      calls.filter((call: any) => call.command === `${provider}_send_prompt`),
    ).toHaveLength(2);
    await page.getByRole("button", { name: /New session/ }).click();
    await expect(attachedContext(page)).toHaveCount(0);
    await expect(input).toHaveValue("");
  });
}

test("Files attachment callbacks and failed sends stay scoped across session and project navigation", async ({
  page,
}) => {
  await desktop(page);
  await findProjectFile(page, "src/App.tsx");
  await page
    .getByRole("treeitem", { name: "src, folder, collapsed", exact: true })
    .click();
  await page.evaluate(() =>
    (window as any).fixture.hold("read_project_file:src/App.tsx"),
  );
  await page
    .getByRole("treeitem", { name: "App.tsx, file", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Add to prompt");
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
  await expect(attachedContext(page)).toHaveCount(0);
  await page
    .locator(".file-editor-toolbar")
    .getByRole("button", { name: "Add to prompt", exact: true })
    .click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Scoped retry");
  await page.evaluate(() => {
    (window as any).fixture.hold("codex_send_prompt");
    (window as any).fixture.failSend();
  });
  await input.press("Enter");
  await expect(attachedContext(page)).toHaveCount(0);
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(attachedContext(page)).toHaveCount(0);
  await page.evaluate(() =>
    (window as any).fixture.release("codex_send_prompt"),
  );
  await expect(input).toHaveValue("");
  await expect(attachedContext(page)).toHaveCount(0);
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(input).toHaveValue("Scoped retry");
  await expect(attachedContext(page)).toContainText("src/App.tsx");
});
