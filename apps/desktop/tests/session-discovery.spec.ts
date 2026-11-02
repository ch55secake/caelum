import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { choose } from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"] as const) {
  test(`session discovery tolerates an uninstalled ${provider} and finds it after installation`, async ({
    page,
  }) => {
    await desktop(page, null, { missingProviders: [provider] });
    const refresh = page.getByRole("button", {
      name: "Refresh sessions",
      exact: true,
    });
    const availableTitle =
      provider === "codex"
        ? "OpenCode session"
        : "Refine the session workspace";
    const missingTitle =
      provider === "codex"
        ? "Refine the session workspace"
        : "OpenCode session";
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(
      page.getByRole("button", { name: availableTitle, exact: true }),
    ).toBeVisible();
    await expect(refresh).toBeEnabled();
    await expect(page.locator(".session-discovery-error")).toHaveCount(0);
    await expect(page.locator(".session-workspace .error-banner")).toHaveCount(
      0,
    );
    await expect(
      page.getByRole("button", { name: missingTitle, exact: true }),
    ).toHaveCount(0);
    await input.fill("Keep this draft");
    await page.evaluate(() => window.dispatchEvent(new Event("focus")));
    expect(
      await page.evaluate(
        (provider) =>
          (window as any).fixture.calls.filter(
            (call: any) => call.command === `${provider}_list_sessions`,
          ).length,
        provider,
      ),
    ).toBe(1);
    await page.evaluate(
      (provider) =>
        (window as any).fixture.setProviderInstalled(provider, true),
      provider,
    );
    await refresh.click();
    await expect(
      page.getByRole("button", { name: missingTitle, exact: true }),
    ).toBeVisible();
    await expect(refresh).toBeEnabled();
    await expect(page.locator(".session-discovery-error")).toHaveCount(0);
    await expect(input).toHaveValue("Keep this draft");
  });
}

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} creation errors stay separate from session discovery and survive refresh`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    const refresh = page.getByRole("button", {
      name: "Refresh sessions",
      exact: true,
    });
    const discoveryError = page.locator(".session-discovery-error");
    const creationError = page.locator(".session-workspace .error-banner");
    await expect(refresh).toBeEnabled();
    await page.evaluate(
      (provider) =>
        (window as any).fixture.setCommandError(
          `${provider}_start_session`,
          "Session creation failed",
        ),
      provider,
    );
    await input.fill("Keep my failed prompt");
    await input.press("Enter");
    await expect(creationError).toContainText("Session creation failed");
    await expect(input).toHaveValue("Keep my failed prompt");
    await expect(discoveryError).toHaveCount(0);

    await page
      .getByRole("button", {
        name:
          provider === "codex"
            ? "Refine the session workspace"
            : "OpenCode session",
        exact: true,
      })
      .click();
    await expect(page.locator(".transcript")).toBeVisible();
    await expect(creationError).toHaveCount(0);
    await page
      .getByRole("button", { name: "New session", exact: true })
      .click();
    await expect(input).toHaveValue("Keep my failed prompt");
    await input.press("Enter");
    await expect(creationError).toContainText("Session creation failed");

    await page.evaluate(
      (provider) =>
        (window as any).fixture.setCommandError(
          `${provider}_list_sessions`,
          "Session listing failed",
        ),
      provider,
    );
    await refresh.click();
    await expect(discoveryError).toBeVisible();
    await expect(discoveryError).toHaveAttribute(
      "title",
      /Session listing failed/,
    );
    await expect(creationError).toContainText("Session creation failed");
    await expect(
      page.getByRole("button", {
        name: "Refine the session workspace",
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      page.getByRole("button", { name: "OpenCode session", exact: true }),
    ).toBeVisible();

    await page.evaluate(
      (provider) =>
        (window as any).fixture.setCommandError(
          `${provider}_list_sessions`,
          null,
        ),
      provider,
    );
    await discoveryError
      .getByRole("button", { name: "Retry", exact: true })
      .click();
    await expect(refresh).toBeEnabled();
    await expect(discoveryError).toHaveCount(0);
    await expect(creationError).toContainText("Session creation failed");
    await expect(input).toHaveValue("Keep my failed prompt");
    await page.evaluate(
      (provider) =>
        (window as any).fixture.setCommandError(
          `${provider}_start_session`,
          null,
        ),
      provider,
    );
    await creationError
      .getByRole("button", { name: "Retry", exact: true })
      .click();
    await expect(creationError).toHaveCount(0);
    await expect(
      page.getByRole("article").filter({ hasText: "Keep my failed prompt" }),
    ).toBeVisible();
    await expect(discoveryError).toHaveCount(0);
  });
}

test("project session discovery reports partial failures and late refreshes do not change the active project", async ({
  page,
}) => {
  await desktop(page, null, { worktrees: true });
  const path = "/external/slow";
  await page.evaluate((path) => {
    const fixture = (window as any).fixture;
    fixture.addWorktree(path, "feature/slow");
    fixture.addWorktree("/external/missing", "missing", { available: false });
    fixture.setCheckoutSessions(path, "codex", [
      {
        id: "slow",
        name: "Recovered linked session",
        preview: "",
        updatedAt: 200,
      },
    ]);
    fixture.setCheckoutSessions(path, "opencode", [
      { id: "ses-linked", title: "Available linked session", updatedAt: 201 },
    ]);
    fixture.setCommandError(
      `codex_list_sessions:${path}`,
      "Provider unavailable in linked checkout",
    );
  }, path);
  await page.evaluate(() => window.dispatchEvent(new Event("focus")));
  await expect(
    page.getByRole("button", {
      name: "Available linked session",
      exact: true,
    }),
  ).toBeVisible();
  await expect(page.locator(".session-discovery-error")).toContainText(
    "Could not load all sessions",
  );
  await expect(page.locator(".session-discovery-error")).toHaveAttribute(
    "title",
    "feature/slow · Codex (/external/slow): Provider unavailable in linked checkout",
  );
  await expect(
    page.getByRole("button", {
      name: "Refine the session workspace",
      exact: true,
    }),
  ).toBeVisible();
  await page.evaluate((path) => {
    const fixture = (window as any).fixture;
    fixture.setCommandError(`codex_list_sessions:${path}`, null);
    fixture.hold(`codex_list_sessions:${path}`);
  }, path);
  await page
    .locator(".session-discovery-error")
    .getByRole("button", { name: "Retry", exact: true })
    .click();
  await expect(
    page.getByRole("button", { name: "Refresh sessions", exact: true }),
  ).toBeDisabled();
  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.locator(".breadcrumbs > span").first()).toHaveText("other");
  await page.evaluate(
    (path) => (window as any).fixture.release(`codex_list_sessions:${path}`),
    path,
  );
  await expect(
    page.getByRole("button", { name: "Refresh sessions", exact: true }),
  ).toBeEnabled();
  await expect(
    page.getByRole("button", {
      name: "Recovered linked session",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(page.locator(".breadcrumbs > span").first()).toHaveText("other");
  await page.getByRole("button", { name: "caelum", exact: true }).click();
  await expect(
    page.getByRole("button", {
      name: "Recovered linked session",
      exact: true,
    }),
  ).toBeVisible();
  await expect(page.locator(".session-discovery-error")).toHaveCount(0);
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter(
        (call: any) =>
          call.command.endsWith("_list_sessions") &&
          call.args.projectPath === "/external/missing",
      ),
    ),
  ).toEqual([]);
});
