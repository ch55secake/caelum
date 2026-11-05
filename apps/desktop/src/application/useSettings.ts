import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { accentColors } from "../domain/settings/accentColors.js";
import { themes } from "../domain/settings/themes.js";
import type { ProviderKind } from "../domain/sessions/workspaceState.js";
import {
  defaultSettings,
  normalizeSettings,
  SETTINGS_KEY,
  type AppSettings,
  type Theme,
} from "../domain/settings/settingsState.js";

function readSettings() {
  try {
    return {
      settings: normalizeSettings(
        JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "null"),
      ),
      error: null,
    };
  } catch {
    return {
      settings: defaultSettings(),
      error: "Could not read saved settings. Changes will retry saving them.",
    };
  }
}

const resolveTheme = (theme: Theme) =>
  theme === "system"
    ? window.matchMedia("(prefers-color-scheme: dark)").matches
      ? "dark"
      : "light"
    : theme;

function applyAppearance(settings: AppSettings) {
  const root = document.documentElement;
  const theme = resolveTheme(settings.theme);
  const family = themes[settings.themeFamily];
  const palette = family[theme];
  const accent =
    settings.accentColor === "theme"
      ? {
          dark: family.dark.accent,
          light: family.light.accent,
          solid: palette.solid,
          hover: palette.hover,
        }
      : accentColors[settings.accentColor];
  root.dataset.theme = theme;
  root.dataset.themeFamily = settings.themeFamily;
  root.dataset.accent = settings.accentColor;
  for (const [name, color] of Object.entries(palette.tokens))
    root.style.setProperty(`--${name}`, color);
  root.style.setProperty("--accent-dark", accent.dark);
  root.style.setProperty("--accent-light", accent.light);
  root.style.setProperty("--accent-solid", accent.solid);
  root.style.setProperty("--accent-solid-hover", accent.hover);
  return theme;
}

// Apply the saved appearance before React's first paint.
export function initializeAppearance() {
  applyAppearance(readSettings().settings);
}

export function useSettings() {
  const [initial] = useState(readSettings);
  const [settings, setSettings] = useState(initial.settings);
  const current = useRef(settings);
  const [storageError, setStorageError] = useState<string | null>(
    initial.error,
  );
  const [themeError, setThemeError] = useState<string | null>(null);
  const [resolvedTheme, setResolvedTheme] = useState<"dark" | "light">(() =>
    resolveTheme(settings.theme),
  );
  const nativeUpdates = useRef(Promise.resolve());

  function save(value: AppSettings) {
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(value));
      setStorageError(null);
    } catch {
      setStorageError(
        "Could not save settings on this device. Your changes apply for this window.",
      );
    }
  }
  function update(change: (value: AppSettings) => AppSettings) {
    const next = normalizeSettings(change(current.current));
    current.current = next;
    setSettings(next);
    save(next);
  }
  function toggleModelFavourite(provider: ProviderKind, id: string) {
    if (!id) return;
    update((value) => {
      const favorites = value.favoriteModels[provider];
      return {
        ...value,
        favoriteModels: {
          ...value.favoriteModels,
          [provider]: favorites.includes(id)
            ? favorites.filter((model) => model !== id)
            : [...favorites, id],
        },
      };
    });
  }

  useLayoutEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      setResolvedTheme(applyAppearance(settings));
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings.theme, settings.themeFamily, settings.accentColor]);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    nativeUpdates.current = nativeUpdates.current.then(async () => {
      if (disposed) return;
      try {
        const window = getCurrentWindow();
        await window.setTheme(
          settings.theme === "system" ? null : settings.theme,
        );
        if (disposed) return;
        await window.setBackgroundColor(
          themes[settings.themeFamily][resolvedTheme].tokens.bg,
        );
        if (!disposed) setThemeError(null);
      } catch (reason) {
        if (!disposed)
          setThemeError(
            `Could not update the native window appearance: ${String(reason)}`,
          );
      }
    });
    return () => {
      disposed = true;
    };
  }, [settings.theme, settings.themeFamily, resolvedTheme]);

  return {
    settings,
    resolvedTheme,
    update,
    toggleModelFavourite,
    storageError,
    themeError,
    retrySave: () => save(current.current),
  };
}

export type Preferences = ReturnType<typeof useSettings>;
