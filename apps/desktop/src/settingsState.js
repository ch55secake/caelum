import { accentColors } from "./accentColors.js";

export const SETTINGS_KEY = "caelum.settings.v1";

export function defaultSettings() {
  return {
    theme: "dark",
    accentColor: "violet",
    defaultProvider: "codex",
    providers: {
      codex: { model: "", effort: "", agent: "" },
      opencode: { model: "", effort: "", agent: "" },
    },
  };
}

export function normalizeSettings(value) {
  const settings = defaultSettings();
  if (["dark", "light", "system"].includes(value?.theme))
    settings.theme = value.theme;
  if (
    typeof value?.accentColor === "string" &&
    Object.hasOwn(accentColors, value.accentColor)
  )
    settings.accentColor = value.accentColor;
  if (["codex", "opencode"].includes(value?.defaultProvider))
    settings.defaultProvider = value.defaultProvider;
  for (const provider of ["codex", "opencode"])
    for (const key of ["model", "effort", "agent"])
      if (typeof value?.providers?.[provider]?.[key] === "string")
        settings.providers[provider][key] = value.providers[provider][key];
  return settings;
}

// Null inherits the saved default; an empty string explicitly uses the
// provider/model default. A draft's model override also resets its effort.
export function newSessionOptions(workspace, defaults) {
  return {
    model: workspace?.model ?? defaults.model,
    effort:
      workspace?.effort ?? (workspace?.model == null ? defaults.effort : ""),
    agent: workspace?.agent ?? defaults.agent,
  };
}
