import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  emit,
  choose,
  openSettings,
  findProjectFile,
  createCheckout,
  pickMenu,
} from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} worktrees group checkouts, isolate drafts and files, support multiple sessions and restore selection`, async ({
    page,
  }) => {
    await desktop(page, null, { worktrees: true });
    await expect(page.locator(".worktree-list")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Create worktree for caelum" }),
    ).toHaveCount(0);
    await expect(page.locator(".worktree-row")).toHaveCount(0);
    await expect(
      page.getByRole("button", { name: "Session worktree", exact: true }),
    ).toHaveCount(0);
    const message = page.getByRole("textbox", { name: "Message", exact: true });
    await message.fill("Original checkout draft");
    const settings = await openSettings(page, "Worktrees");
    await settings
      .getByRole("textbox", { name: "Worktree folder" })
      .fill("/custom/checkouts");
    await settings.getByRole("button", { name: "Done", exact: true }).click();
    const path = await createCheckout(page);
    expect(path).toBe("/custom/checkouts/caelum-a1b2/feature-search");
    await expect(page.locator(".worktree-row")).toHaveCount(2);
    await page
      .getByRole("button", { name: "Session worktree", exact: true })
      .click();
    await expect(
      page.getByRole("option", { name: /^Original checkout/ }),
    ).toBeVisible();
    await expect(
      page.getByRole("option", { name: /^feature\/search/ }),
    ).toBeVisible();
    await page.keyboard.press("Escape");
    await expect(page.locator(".project-group")).toHaveCount(1);
    await expect(page.locator(".breadcrumbs")).toContainText("feature/search");
    await findProjectFile(page, "src/App.tsx");
    await expect(page.locator(".code-viewer")).toContainText(
      'checkout = "feature/search"',
    );
    await page.evaluate((path) => {
      const fixture = (window as any).fixture;
      fixture.setGitFiles(
        [{ path: "src/App.tsx", indexStatus: " ", worktreeStatus: "M" }],
        path,
      );
      fixture.setGitDiff(
        "src/App.tsx",
        { unstaged: "@@ -1 +1 @@\n-original checkout\n+worktree change" },
        path,
      );
    }, path);
    await page.getByRole("button", { name: "Changes", exact: true }).click();
    await expect(
      page.getByRole("region", { name: "Unstaged diff", exact: true }),
    ).toContainText("worktree change");
    await page
      .getByRole("button", {
        name: "caelum",
        exact: true,
      })
      .click();
    await expect(message).toHaveValue("Original checkout draft");
    await findProjectFile(page, "src/App.tsx");
    await expect(page.locator(".code-viewer")).toContainText("ready = true");
    await page
      .getByRole("button", { name: "Session worktree", exact: true })
      .click();
    await page.getByRole("option", { name: /^feature\/search/ }).click();
    await expect(message).toHaveValue("");
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await message.fill("First worktree task");
    await message.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", {
        name: "Remove worktree feature/search",
        exact: true,
      }),
    ).toBeDisabled();
    await message.fill("First session draft");
    await page.getByRole("button", { name: "New session" }).click();
    await message.fill("Second worktree task");
    await message.press("Enter");
    await expect
      .poll(() =>
        page.evaluate(
          (provider) =>
            (window as any).fixture.calls.filter(
              (call: any) => call.command === `${provider}_start_session`,
            ).length,
          provider,
        ),
      )
      .toBe(2);
    const calls = await page.evaluate(
      (provider) =>
        (window as any).fixture.calls.filter(
          (call: any) => call.command === `${provider}_send_prompt`,
        ),
      provider,
    );
    expect(calls).toHaveLength(2);
    expect(calls.map((call: any) => call.args.projectPath)).toEqual([
      path,
      path,
    ]);
    expect(calls[0].args.threadId ?? calls[0].args.sessionId).not.toBe(
      calls[1].args.threadId ?? calls[1].args.sessionId,
    );
    await page
      .getByRole("button", {
        name: "caelum",
        exact: true,
      })
      .click();
    await expect(message).toHaveValue("Original checkout draft");
    await page
      .getByRole("button", {
        name: "Switch to worktree feature/search",
        exact: true,
      })
      .click();
    await expect(page.locator(".provider-label")).toHaveText(
      provider === "codex" ? "Codex" : "OpenCode",
    );
    await page.reload();
    await expect(message).toBeEnabled();
    await expect(page.locator(".project-group")).toHaveCount(1);
    await expect(page.locator(".breadcrumbs > span").first()).toHaveText(
      "caelum",
    );
    await expect(
      page.getByRole("button", {
        name: "Switch to worktree feature/search",
        exact: true,
      }),
    ).toHaveAttribute("aria-current", "page");
    await expect(page.locator(".provider-label")).toHaveText(
      provider === "codex" ? "Codex" : "OpenCode",
    );
    const restored = await openSettings(page, "Worktrees");
    await expect(
      restored.getByRole("textbox", { name: "Worktree folder" }),
    ).toHaveValue("/custom/checkouts");
  });
}

test("composer worktree picker searches checkouts, skips unavailable options and returns to the original checkout", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Original checkout draft");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.addWorktree("/external/missing", "missing", { available: false });
    fixture.addWorktree("/external/linked", "feature/linked");
  });
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  const picker = page
    .locator(".composer-controls")
    .getByRole("button", { name: "Session worktree", exact: true });
  await expect(picker).toContainText("main");
  await picker.press("ArrowDown");
  const search = page.getByRole("combobox", {
    name: "Search session worktrees",
    exact: true,
  });
  await expect(search).toBeFocused();
  await expect(page.getByRole("option", { name: /^missing / })).toBeDisabled();
  await search.press("ArrowDown");
  await search.press("Enter");
  await expect(picker).toContainText("feature/linked");
  await expect(input).toHaveValue("");
  await input.fill("Linked checkout draft");
  await picker.click();
  await search.fill("/work/caelum");
  await expect(page.getByRole("option")).toHaveCount(1);
  await search.press("Enter");
  await expect(picker).toContainText("main");
  await expect(input).toHaveValue("Original checkout draft");
  await picker.click();
  await search.press("Escape");
  await expect(picker).toBeFocused();
  await page.setViewportSize({ width: 800, height: 620 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(
    800,
  );
  await picker.click();
  await expect(
    page.getByRole("dialog", {
      name: "Choose session worktree",
      exact: true,
    }),
  ).toBeInViewport({ ratio: 1 });
  await page.screenshot({
    path: "test-results/composer-worktree-picker.png",
  });
});

test("worktrees discover external checkouts, reject removal failures and ignore stale inventory after removal", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const path = await createCheckout(page, "fix/layout");
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Keep this draft until removal succeeds");
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.addWorktree("/external/locked", "locked", {
      locked: "another task",
    });
    fixture.addWorktree("/external/missing", null, {
      available: false,
      prunable: "directory missing",
    });
  });
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await expect(
    page.getByRole("button", { name: "Remove worktree locked", exact: true }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree missing",
      exact: true,
    }),
  ).toBeDisabled();
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "remove_worktree",
      "Git: contains modified or untracked files",
    ),
  );
  await page
    .getByRole("button", { name: "Remove worktree fix/layout", exact: true })
    .click();
  const dialog = page.getByRole("dialog", {
    name: "Remove worktree",
    exact: true,
  });
  await expect(dialog).toContainText(path);
  await dialog
    .getByRole("button", { name: "Remove worktree", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText(
    "modified or untracked",
  );
  await expect(message).toHaveValue("Keep this draft until removal succeeds");
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await page.evaluate(() => {
    const fixture = (window as any).fixture;
    fixture.setCommandError("remove_worktree", null);
    fixture.hold("git_worktrees");
  });
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await page
    .getByRole("button", { name: "Remove worktree fix/layout", exact: true })
    .click();
  await dialog
    .getByRole("button", { name: "Remove worktree", exact: true })
    .click();
  await page.evaluate(() => (window as any).fixture.release("git_worktrees"));
  await expect(dialog).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree fix/layout",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await expect(message).toBeEnabled();
  await page.evaluate((path) => {
    const fixture = (window as any).fixture;
    fixture.emit("codex-event", {
      projectPath: path,
      method: "thread/name/updated",
      params: { threadId: "late", threadName: "Late result" },
    });
    fixture.addWorktree(path, "fix/layout");
  }, path);
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await page
    .getByRole("button", { name: "Switch to worktree fix/layout", exact: true })
    .click();
  await expect(message).toHaveValue("");
  await page
    .getByRole("button", { name: "Remove worktree fix/layout", exact: true })
    .click();
  await dialog
    .getByRole("button", { name: "Remove worktree", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  await page.reload();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree fix/layout",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
});

test("worktrees recover creation failures and delayed selection cannot steal another project", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  await page.evaluate(() =>
    (window as any).fixture.setCommandError(
      "create_worktree",
      "Git: branch already exists",
    ),
  );
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Create worktree");
  const dialog = page.getByRole("dialog", {
    name: "Create worktree",
    exact: true,
  });
  const branch = await dialog
    .getByRole("textbox", { name: "New branch" })
    .inputValue();
  expect(branch).not.toBe("");
  await expect(
    dialog.getByRole("combobox", { name: "Start from" }),
  ).toHaveValue("main");
  await dialog
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(dialog.getByRole("alert")).toContainText(
    "branch already exists",
  );
  await expect(dialog.getByRole("textbox", { name: "New branch" })).toHaveValue(
    branch,
  );
  await page.evaluate(() =>
    (window as any).fixture.setCommandError("create_worktree", null),
  );
  await dialog
    .getByRole("button", { name: "Create worktree", exact: true })
    .click();
  await expect(dialog).toHaveCount(0);
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls
          .filter((call: any) => call.command === "create_worktree")
          .at(-1).args,
    ),
  ).toMatchObject({ branch, base: "main" });
  await page
    .getByRole("button", { name: "Create worktree for caelum" })
    .click();
  await expect(
    dialog.getByRole("textbox", { name: "New branch" }),
  ).not.toHaveValue(branch);
  await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
  await page
    .getByRole("button", {
      name: "caelum",
      exact: true,
    })
    .click();
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await page.evaluate(() => (window as any).fixture.hold("open_worktree"));
  await page
    .getByRole("button", {
      name: `Switch to worktree ${branch}`,
      exact: true,
    })
    .click();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await page.evaluate(() => (window as any).fixture.release("open_worktree"));
  await expect(page.locator(".breadcrumbs")).toContainText("other");
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(
    page.getByRole("button", {
      name: "caelum",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await page.evaluate(() => (window as any).fixture.hold("open_worktree"));
  await page
    .getByRole("button", {
      name: `Switch to worktree ${branch}`,
      exact: true,
    })
    .click();
  await page.getByRole("button", { name: "Close caelum", exact: true }).click();
  await expect(page.locator(".project-group")).toHaveCount(1);
  await page.evaluate(() => (window as any).fixture.release("open_worktree"));
  await expect
    .poll(() =>
      page.evaluate(() =>
        JSON.parse(sessionStorage.getItem("worktree-workspace")!).projects.map(
          (entry: any) => entry.project.path,
        ),
      ),
    )
    .toEqual(["/work/other"]);
  await page.reload();
  await expect(
    page.getByRole("button", {
      name: `Switch to worktree ${branch}`,
      exact: true,
    }),
  ).toHaveCount(0);
});

test("worktrees keep drafts when the active checkout is removed externally", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const path = await createCheckout(page);
  const message = page.getByRole("textbox", { name: "Message", exact: true });
  await message.fill("Retain the draft");
  await page.evaluate(
    (path) => (window as any).fixture.forgetWorktree(path),
    path,
  );
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await expect(message).toBeDisabled();
  await expect(
    page.getByText(
      "This worktree is unavailable. Choose another checkout from the sidebar.",
    ),
  ).toBeVisible();
  await page
    .getByRole("button", {
      name: "caelum",
      exact: true,
    })
    .click();
  await expect(message).toBeEnabled();
  await page.evaluate(
    (path) => (window as any).fixture.addWorktree(path, "feature/search"),
    path,
  );
  await expect(page.locator(".worktree-list")).toHaveCount(0);
  await page
    .getByRole("button", { name: "caelum", exact: true })
    .click({ button: "right" });
  await pickMenu(page, "Refresh worktrees");
  await page
    .getByRole("button", {
      name: "Switch to worktree feature/search",
      exact: true,
    })
    .click();
  await expect(message).toHaveValue("Retain the draft");
});

test("worktrees restore their project group when the original project directory is missing", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  await createCheckout(page);
  await page.evaluate(() =>
    (window as any).fixture.addWorktree("/work/caelum", "main", {
      main: true,
      available: false,
    }),
  );
  await page.reload();
  await expect(page.locator(".project-group")).toHaveCount(1);
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree Original checkout",
      exact: true,
    }),
  ).toBeDisabled();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree feature/search",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await expect(page.locator(".breadcrumbs > span").first()).toHaveText(
    "caelum",
  );
  await createCheckout(page, "feature/second");
  await page.evaluate(() =>
    (window as any).fixture.addWorktree("/external/checkout", "external"),
  );
  await page
    .getByRole("button", { name: "Refresh worktrees for caelum" })
    .click();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree external",
      exact: true,
    }),
  ).toBeEnabled();
  for (const branch of ["feature/second", "feature/search"]) {
    await page
      .getByRole("button", { name: `Remove worktree ${branch}`, exact: true })
      .click();
    const dialog = page.getByRole("dialog", {
      name: "Remove worktree",
      exact: true,
    });
    await dialog
      .getByRole("button", { name: "Remove worktree", exact: true })
      .click();
    await expect(dialog).toHaveCount(0);
  }
  await page
    .getByRole("button", { name: "Switch to worktree external", exact: true })
    .click();
  await expect(
    page.getByRole("button", {
      name: "Switch to worktree external",
      exact: true,
    }),
  ).toHaveAttribute("aria-current", "page");
  await expect(
    page.getByRole("textbox", { name: "Message", exact: true }),
  ).toBeEnabled();
});
