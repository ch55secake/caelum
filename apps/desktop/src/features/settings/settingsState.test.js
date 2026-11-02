import assert from "node:assert/strict";
import test from "node:test";
import {
  defaultSettings,
  normalizeSettings,
  newSessionOptions,
} from "./settingsState.js";

test("saved preferences recover missing and invalid fields without mixing provider defaults", () => {
  const settings = normalizeSettings({
    theme: "light",
    defaultProvider: "opencode",
    providers: {
      codex: { model: "model-a", effort: 7 },
      opencode: { model: "provider/model-b", effort: "high", agent: "build" },
    },
  });
  assert.equal(settings.theme, "light");
  assert.equal(settings.accentColor, "violet");
  assert.equal(settings.defaultProvider, "opencode");
  assert.deepEqual(settings.favoriteModels, { codex: [], opencode: [] });
  assert.deepEqual(
    normalizeSettings({
      ...settings,
      favoriteModels: {
        codex: "bad",
        opencode: ["one/model", "two/model", "one/model", null, 8, "", " "],
      },
    }),
    {
      ...settings,
      favoriteModels: { codex: [], opencode: ["one/model", "two/model"] },
    },
  );
  assert.deepEqual(settings.providers.codex, {
    model: "model-a",
    effort: "",
    agent: "",
  });
  assert.deepEqual(settings.providers.opencode, {
    model: "provider/model-b",
    effort: "high",
    agent: "build",
  });
  assert.deepEqual(normalizeSettings({ ...settings, accentColor: "teal" }), {
    ...settings,
    accentColor: "teal",
  });
  for (const accentColor of [null, "unknown", "__proto__", {}, 7])
    assert.deepEqual(
      normalizeSettings({ ...settings, accentColor }),
      settings,
    );
  for (const value of [
    null,
    [],
    "bad",
    { theme: "unknown", defaultProvider: "unknown", providers: {} },
  ])
    assert.deepEqual(normalizeSettings(value), defaultSettings());
});

test("draft overrides distinguish inheritance from native defaults and reset inherited effort for another model", () => {
  const defaults = {
    model: "provider/model",
    effort: "high",
    agent: "build",
  };
  assert.deepEqual(newSessionOptions(undefined, defaults), defaults);
  assert.deepEqual(
    newSessionOptions({ model: null, effort: null, agent: null }, defaults),
    defaults,
  );
  assert.deepEqual(
    newSessionOptions({ model: "provider/fast", effort: null }, defaults),
    { model: "provider/fast", effort: "", agent: "build" },
  );
  assert.deepEqual(
    newSessionOptions({ model: "", effort: "", agent: "" }, defaults),
    { model: "", effort: "", agent: "" },
  );
  assert.deepEqual(newSessionOptions({ effort: "low" }, defaults), {
    ...defaults,
    effort: "low",
  });
  assert.deepEqual(defaults, {
    model: "provider/model",
    effort: "high",
    agent: "build",
  });
});
