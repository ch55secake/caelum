import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  defaultSettings,
  normalizeSettings,
  SETTINGS_KEY,
  type AppSettings,
  type Theme,
} from "./settingsState.js";

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

// Apply the saved appearance before React's first paint.
export function initializeTheme() {
  document.documentElement.dataset.theme = resolveTheme(
    readSettings().settings.theme,
  );
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

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const theme = resolveTheme(settings.theme);
      document.documentElement.dataset.theme = theme;
      setResolvedTheme(theme);
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings.theme]);

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
          resolvedTheme === "dark" ? "#0b0c0e" : "#fafafc",
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
  }, [settings.theme, resolvedTheme]);

  return {
    settings,
    resolvedTheme,
    update,
    storageError,
    themeError,
    retrySave: () => save(current.current),
  };
}
