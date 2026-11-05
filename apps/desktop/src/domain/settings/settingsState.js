import { accentColors } from "./accentColors.js";
import { themes } from "./themes.js";

export const SETTINGS_KEY = "caelum.settings.v1";

export function defaultSettings() {
  return {
    theme: "dark",
    themeFamily: "caelum",
    accentColor: "violet",
    defaultProvider: "codex",
    worktreeDirectory: "",
    notifications: {
      mode: "background",
      volume: 50,
      backgroundShells: false,
      inputNeeded: true,
    },
    favoriteModels: { codex: [], opencode: [] },
    providers: {
      codex: { model: "", effort: "", agent: "" },
      opencode: { model: "", effort: "", agent: "" },
    },
  };
}

export function normalizeSettings(value) {
  const settings = defaultSettings();
  const notifications = value?.notifications;
  if (["off", "background", "always"].includes(notifications?.mode))
    settings.notifications.mode = notifications.mode;
  if (Number.isFinite(notifications?.volume))
    settings.notifications.volume = Math.max(
      0,
      Math.min(100, notifications.volume),
    );
  for (const key of ["backgroundShells", "inputNeeded"])
    if (typeof notifications?.[key] === "boolean")
      settings.notifications[key] = notifications[key];
  if (["dark", "light", "system"].includes(value?.theme))
    settings.theme = value.theme;
  if (
    typeof value?.themeFamily === "string" &&
    Object.hasOwn(themes, value.themeFamily)
  )
    settings.themeFamily = value.themeFamily;
  if (
    typeof value?.accentColor === "string" &&
    (value.accentColor === "theme" ||
      Object.hasOwn(accentColors, value.accentColor))
  )
    settings.accentColor = value.accentColor;
  if (["codex", "opencode"].includes(value?.defaultProvider))
    settings.defaultProvider = value.defaultProvider;
  if (typeof value?.worktreeDirectory === "string")
    settings.worktreeDirectory = value.worktreeDirectory;
  for (const provider of ["codex", "opencode"]) {
    const favorites = value?.favoriteModels?.[provider];
    if (Array.isArray(favorites))
      settings.favoriteModels[provider] = [
        ...new Set(
          favorites.filter((id) => typeof id === "string" && id.trim()),
        ),
      ];
    for (const key of ["model", "effort", "agent"])
      if (typeof value?.providers?.[provider]?.[key] === "string")
        settings.providers[provider][key] = value.providers[provider][key];
  }
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
