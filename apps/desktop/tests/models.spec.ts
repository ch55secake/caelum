import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose, openSettings } from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"] as const) {
  test(`${provider} model favourites persist, stay independent and work across composer and settings`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    const name = provider === "codex" ? "Codex" : "OpenCode";
    const model = page.getByRole("button", { name: "Model", exact: true });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Keep my draft while favouriting");
    await choose(page, "Model", "Test model provider/model");
    await choose(page, "Effort", "High");
    await model.click();
    const popup = page.getByRole("dialog", {
      name: "Choose model",
      exact: true,
    });
    const search = popup.getByRole("combobox", { name: "Search models" });
    await popup
      .getByRole("button", {
        name: "Add Fast model (provider/fast) to favourites",
        exact: true,
      })
      .click();
    await expect(popup).toBeVisible();
    await expect(popup.getByText("Favourites", { exact: true })).toBeVisible();
    expect(
      await popup
        .locator(".choice-option")
        .evaluateAll((buttons) =>
          buttons.map((button) => button.getAttribute("aria-label")),
        ),
    ).toEqual([
      "Default model Use provider settings",
      "Fast model provider/fast",
      "Test model provider/model",
      "Simple model provider/plain",
    ]);
    await search.fill("plain");
    await search.press("Tab");
    const addSimple = popup.getByRole("button", {
      name: "Add Simple model (provider/plain) to favourites",
      exact: true,
    });
    await expect(addSimple).toBeFocused();
    await addSimple.press("Enter");
    await expect(popup).toBeVisible();
    await expect(search).toBeFocused();
    await search.press("Tab");
    await popup
      .getByRole("button", {
        name: "Remove Simple model (provider/plain) from favourites",
        exact: true,
      })
      .press("Space");
    await expect(addSimple).toBeVisible();
    await search.press("Escape");
    await expect(model).toContainText("Test model");
    await expect(model).toBeFocused();
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("High");
    await expect(input).toHaveValue("Keep my draft while favouriting");
    const dialog = await openSettings(page, "Providers");
    await dialog.getByRole("button", { name, exact: true }).click();
    await dialog
      .getByRole("button", { name: "Reset " + name + " defaults" })
      .click();
    await dialog
      .getByRole("button", { name: "Default model", exact: true })
      .click();
    const defaults = page.getByRole("dialog", {
      name: "Choose default model",
      exact: true,
    });
    await expect(
      defaults.getByRole("button", {
        name: "Remove Fast model (provider/fast) from favourites",
        exact: true,
      }),
    ).toBeVisible();
    await defaults.getByRole("combobox").press("Escape");
    await expect(dialog).toBeVisible();
    await dialog.getByRole("button", { name: "Done", exact: true }).click();
    await page.reload();
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await model.click();
    await expect(
      popup.getByRole("button", {
        name: "Remove Fast model (provider/fast) from favourites",
        exact: true,
      }),
    ).toBeVisible();
    await search.press("ArrowDown");
    await search.press("Enter");
    await expect(model).toContainText("Fast model");
    await choose(page, "Provider", provider === "codex" ? "OpenCode" : "Codex");
    await model.click();
    await expect(
      popup.getByRole("button", {
        name: "Add Fast model (provider/fast) to favourites",
        exact: true,
      }),
    ).toBeVisible();
    await search.press("Escape");
    await choose(page, "Provider", name);
    await model.click();
    await popup
      .getByRole("button", {
        name: "Remove Fast model (provider/fast) from favourites",
        exact: true,
      })
      .click();
    await expect(popup.getByText("Favourites", { exact: true })).toHaveCount(0);
    await search.press("Enter");
    await expect(model).toContainText("Fast model");
    expect(
      await page.evaluate(
        (provider) =>
          JSON.parse(localStorage.getItem("caelum.settings.v1")!)
            .favoriteModels[provider],
        provider,
      ),
    ).toEqual([]);
  });
}

test("OpenCode model sources distinguish routed models, support provider search and retain unavailable favourites", async ({
  page,
}) => {
  await desktop(page, null);
  const models = [
    {
      id: "anthropic/shared",
      name: "Shared model",
      provider: { id: "anthropic", name: "Anthropic" },
      efforts: [],
    },
    {
      id: "openrouter/anthropic/shared",
      name: "Shared model",
      provider: { id: "openrouter", name: "OpenRouter" },
      efforts: [],
    },
    {
      id: "work-gateway/local",
      name: "Local model",
      provider: { id: "work-gateway", name: "Company gateway" },
      efforts: [],
    },
    { id: "fallback/local", name: "Fallback model", efforts: [] },
    {
      id: "anthropic/fast",
      name: "Fast direct model",
      provider: { id: "anthropic", name: "Anthropic" },
      efforts: [],
    },
  ];
  await page.evaluate(
    (models) => (window as any).fixture.setModels("opencode", models),
    models,
  );
  await choose(page, "Provider", "OpenCode");
  const model = page.getByRole("button", { name: "Model", exact: true });
  await model.click();
  const popup = page.getByRole("dialog", {
    name: "Choose model",
    exact: true,
  });
  const search = popup.getByRole("combobox", { name: "Search models" });
  await expect(popup.locator(".choice-section")).toHaveText([
    "Anthropic",
    "Company gateway",
    "fallback",
    "OpenRouter",
  ]);
  expect(
    await popup
      .locator(".choice-option")
      .evaluateAll((buttons) =>
        buttons.map((button) => button.getAttribute("aria-label")),
      ),
  ).toEqual([
    "Default model Use provider settings",
    "Shared model anthropic/shared",
    "Fast direct model anthropic/fast",
    "Local model work-gateway/local",
    "Fallback model fallback/local",
    "Shared model openrouter/anthropic/shared",
  ]);
  await search.fill("Company gateway");
  await expect(popup.locator(".choice-section")).toHaveText([
    "Company gateway",
  ]);
  await expect(
    popup.getByRole("button", {
      name: "Local model work-gateway/local",
      exact: true,
    }),
  ).toBeVisible();
  await expect(
    popup.getByRole("button", {
      name: "Shared model anthropic/shared",
      exact: true,
    }),
  ).toHaveCount(0);
  await search.fill("fallback");
  await expect(popup.locator(".choice-source")).toHaveText("fallback");
  await search.fill("shared");
  await popup
    .getByRole("button", {
      name: "Add Shared model (openrouter/anthropic/shared) to favourites",
      exact: true,
    })
    .click();
  await expect(popup.locator(".choice-section")).toHaveText([
    "Favourites",
    "Anthropic",
  ]);
  await expect(
    popup.getByRole("button", {
      name: "Add Shared model (anthropic/shared) to favourites",
      exact: true,
    }),
  ).toBeVisible();
  await popup
    .getByRole("button", {
      name: "Shared model openrouter/anthropic/shared",
      exact: true,
    })
    .click();
  await expect(model.locator(".choice-trigger-source")).toHaveText(
    "OpenRouter",
  );
  const dialog = await openSettings(page, "Providers");
  await choose(page, "Default model", "Shared model anthropic/shared");
  await expect(
    dialog
      .getByRole("button", { name: "Default model", exact: true })
      .locator(".choice-trigger-source"),
  ).toHaveText("Anthropic");
  await page.evaluate(() => (window as any).fixture.setModels("opencode", []));
  await dialog.getByRole("button", { name: "Refresh OpenCode status" }).click();
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .click();
  const defaults = page.getByRole("dialog", {
    name: "Choose default model",
    exact: true,
  });
  await expect(
    defaults.getByRole("button", {
      name: "Remove Shared model (openrouter/anthropic/shared) from favourites",
      exact: true,
    }),
  ).toHaveCount(0);
  await expect(
    defaults.getByText("Saved selection · currently unavailable", {
      exact: true,
    }),
  ).toBeVisible();
  await defaults.getByRole("combobox").press("Escape");
  await page.evaluate(
    (models) => (window as any).fixture.setModels("opencode", models),
    models,
  );
  await dialog.getByRole("button", { name: "Refresh OpenCode status" }).click();
  await dialog
    .getByRole("button", { name: "Default model", exact: true })
    .click();
  await expect(
    defaults.getByRole("button", {
      name: "Remove Shared model (openrouter/anthropic/shared) from favourites",
      exact: true,
    }),
  ).toBeVisible();
  await expect(defaults.locator(".choice-section")).toHaveText([
    "Favourites",
    "Anthropic",
    "Company gateway",
    "fallback",
  ]);
  await page.screenshot({
    path: "test-results/model-providers-favourites.png",
  });
  await defaults.getByRole("combobox").press("Escape");
  await dialog.getByRole("button", { name: "Done", exact: true }).click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await input.fill("Use the routed model");
  await input.press("Enter");
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toBeVisible();
  expect(
    await page.evaluate(
      () =>
        (window as any).fixture.calls.find(
          (call: any) => call.command === "opencode_start_session",
        ).args.model,
    ),
  ).toBe("openrouter/anthropic/shared");
});

test("model picker searches capabilities, supports keyboard selection and returns focus", async ({
  page,
}) => {
  await desktop(page, null);
  const model = page.getByRole("button", { name: "Model", exact: true });
  await model.focus();
  await model.press("ArrowDown");
  const search = page.getByRole("combobox", { name: "Search models" });
  await expect(search).toBeFocused();
  await search.fill("provider/fast");
  await expect(
    page
      .getByRole("grid", { name: "Model", exact: true })
      .locator(".choice-option"),
  ).toHaveCount(1);
  await search.press("Enter");
  await expect(model).toContainText("Fast model");
  await expect(model).toBeFocused();
  await model.click();
  await search.fill("missing");
  await expect(page.getByText("No matching models")).toBeVisible();
  await search.press("Escape");
  await expect(model).toBeFocused();
  await model.click();
  await search.press("ArrowUp");
  await search.press("Enter");
  await expect(model).toContainText("Test model");
  await model.click();
  await page.screenshot({ path: "test-results/model-picker.png" });
  await page.getByRole("heading", { name: "New session", exact: true }).click();
  await expect(page.getByRole("dialog", { name: "Choose model" })).toHaveCount(
    0,
  );
  expect(
    await page.evaluate(() =>
      (window as any).fixture.calls.filter((call: any) =>
        call.command.endsWith("_start_session"),
      ),
    ),
  ).toHaveLength(0);
  await page.setViewportSize({ width: 800, height: 620 });
  await model.click();
  await expect(
    page.getByRole("dialog", { name: "Choose model" }),
  ).toBeInViewport({ ratio: 1 });
  await page.screenshot({ path: "test-results/model-picker-minimum.png" });
});

for (const provider of ["codex", "opencode"]) {
  test(`${provider} keeps effort session-scoped through navigation, retries and reconnects and resets it for another model`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: /OpenCode session/ }).click();
    }
    const sessionName =
      provider === "codex"
        ? /Refine the session workspace/
        : /OpenCode session/;
    const effort = page.getByRole("button", { name: "Effort", exact: true });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(effort).toHaveText("Medium");
    await choose(page, "Effort", "High");
    await page
      .getByRole("button", { name: /Review the provider adapters/ })
      .click();
    await expect(effort).toHaveText("Medium");
    await page.keyboard.press("Meta+n");
    await expect(effort).toHaveText("Default effort");
    await page.getByRole("button", { name: sessionName }).click();
    await expect(effort).toHaveText("High");
    await input.fill("Think carefully");
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await expect(effort).toHaveText("High");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await choose(page, "Effort", "Low");
    await emit(
      page,
      provider === "codex" ? "codex/processExited" : "server.disconnected",
      {},
      provider,
    );
    await page.getByRole("button", { name: "Reconnect", exact: true }).click();
    await expect(input).toBeEnabled();
    await expect(effort).toHaveText("Low");
    await input.fill("Use less reasoning");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await choose(
      page,
      "Effort",
      provider === "codex"
        ? "Default effort Use medium effort"
        : "Default effort Use model settings",
    );
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.succeeded",
      provider === "codex"
        ? {
            threadId: "thread-a",
            turn: { id: "turn-new", status: "completed" },
          }
        : { sessionID: "ses-a" },
      provider,
    );
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toHaveCount(0);
    await input.fill("Restore default effort");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    const sends = calls.filter(
      (call: any) => call.command === `${provider}_send_prompt`,
    );
    expect(
      sends.map((call: any) => [call.args.model, call.args.effort]),
    ).toEqual([
      [null, "high"],
      [null, "high"],
      [null, "low"],
      [null, provider === "codex" ? "medium" : ""],
    ]);
    await choose(page, "Effort", "High");
    await choose(page, "Model", "Fast model provider/fast");
    await expect(effort).toHaveText("Default effort");
    await effort.click();
    await expect(
      page.getByRole("option", { name: "High", exact: true }),
    ).toHaveCount(0);
    await page.getByRole("option", { name: "Low", exact: true }).click();
    await page.setViewportSize({ width: 800, height: 620 });
    await effort.click();
    await expect(
      page.getByRole("dialog", { name: "Choose effort" }),
    ).toBeInViewport({ ratio: 1 });
    await page.screenshot({
      path: `test-results/${provider}-effort-picker.png`,
    });
    await page.keyboard.press("Escape");
    await choose(page, "Model", "Simple model provider/plain");
    await expect(effort).toHaveCount(0);
  });
}

for (const provider of ["codex", "opencode"]) {
  test(`${provider} applies effort on the first turn and preserves it if the first send fails`, async ({
    page,
  }) => {
    await desktop(page, null);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await choose(page, "Effort", "High");
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await input.fill("Start with high effort");
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await expect(
      page.getByRole("button", { name: "Effort", exact: true }),
    ).toHaveText("High");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    expect(
      calls.filter((call: any) => call.command.endsWith("_start_session")),
    ).toHaveLength(1);
    expect(
      calls
        .filter((call: any) => call.command === `${provider}_send_prompt`)
        .map((call: any) => call.args.effort),
    ).toEqual(["high", "high"]);
  });
}

for (const provider of ["codex", "opencode"]) {
  test(`${provider} changes the model in an existing session and keeps pending choices through navigation and retries`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? "thread-a" : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: /OpenCode session/ }).click();
    }
    const sessionName =
      provider === "codex"
        ? /Refine the session workspace/
        : /OpenCode session/;
    const id = provider === "codex" ? "thread-a" : "ses-a";
    const model = page.getByRole("button", { name: "Model", exact: true });
    const input = page.getByRole("textbox", { name: "Message", exact: true });
    await expect(model).toContainText("Test model");
    await choose(page, "Model", "Fast model provider/fast");
    await input.fill("Use the selected model in this conversation");
    await page.keyboard.press("Meta+n");
    await expect(model).toHaveText("Default model");
    await page
      .getByRole("button", { name: /Review the provider adapters/ })
      .click();
    await expect(model).toContainText("Test model");
    await page.getByRole("button", { name: sessionName }).click();
    await expect(model).toContainText("Fast model");
    await expect(input).toHaveValue(
      "Use the selected model in this conversation",
    );
    await page.evaluate(() => (window as any).fixture.failSend());
    await input.press("Enter");
    await expect(page.getByRole("alert")).toContainText("Connection refused");
    await expect(model).toContainText("Fast model");
    await expect(input).toHaveValue(
      "Use the selected model in this conversation",
    );
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    await expect(model).toBeEnabled();
    await choose(page, "Model", "Test model provider/model");
    await expect(page.getByText("Model applies to next message")).toBeVisible();
    await choose(
      page,
      "Model",
      "Session model Keep this session’s configured model",
    );
    await expect(model).toContainText("Fast model");
    await choose(page, "Model", "Test model provider/model");
    await emit(
      page,
      provider === "codex" ? "codex/processExited" : "server.disconnected",
      {},
      provider,
    );
    await page.getByRole("button", { name: "Reconnect", exact: true }).click();
    await expect(input).toBeEnabled();
    await expect(model).toContainText("Test model");
    await input.fill("Use the next model");
    await input.press("Enter");
    await expect(
      page.getByRole("button", { name: "Stop generation" }),
    ).toBeVisible();
    const calls = await page.evaluate(() => (window as any).fixture.calls);
    const sends = calls.filter(
      (call: any) => call.command === `${provider}_send_prompt`,
    );
    expect(
      sends.map((call: any) => [call.args.sessionId, call.args.model]),
    ).toEqual([
      [id, "provider/fast"],
      [id, "provider/fast"],
      [id, "provider/model"],
    ]);
    expect(
      calls.filter((call: any) => call.command.endsWith("_start_session")),
    ).toHaveLength(0);
    await expect(
      provider === "codex"
        ? page.getByRole("heading", { name: "A focused workspace" })
        : page.getByText("Restored OpenCode conversation"),
    ).toBeVisible();
  });
}
