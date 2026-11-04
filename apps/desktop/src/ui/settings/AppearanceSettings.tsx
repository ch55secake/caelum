import { Check, Monitor, Moon, Sun } from "lucide-react";
import type { CSSProperties } from "react";
import type { Preferences } from "../../application/useSettings.js";
import {
  accentColors,
  type AccentColor,
} from "../../domain/settings/accentColors.js";
import {
  themes,
  type ThemeFamily,
  type ThemePalette,
} from "../../domain/settings/themes.js";

function PalettePreview({ palette }: { palette: ThemePalette }) {
  return (
    <span
      className="palette-preview"
      aria-hidden="true"
      style={
        {
          "--preview-bg": palette.tokens.bg,
          "--preview-sidebar": palette.tokens.sidebar,
          "--preview-panel": palette.tokens.panel,
          "--preview-border": palette.tokens.border,
          "--preview-text": palette.tokens.text,
          "--preview-accent": palette.accent,
          "--preview-keyword": palette.tokens["syntax-keyword"],
          "--preview-string": palette.tokens["syntax-string"],
        } as CSSProperties
      }
    >
      <span className="palette-preview-sidebar">
        <i />
        <i />
        <i />
      </span>
      <span className="palette-preview-content">
        <i />
        <i />
        <i />
        <b />
      </span>
    </span>
  );
}

export function AppearanceSettings({
  preferences,
}: {
  preferences: Preferences;
}) {
  const { settings, resolvedTheme, update } = preferences;
  const family = themes[settings.themeFamily];
  const accents = [
    { id: "theme", name: "Theme", color: family[resolvedTheme].accent },
    ...Object.entries(accentColors).map(([id, palette]) => ({
      id,
      name: palette.name,
      color: palette[resolvedTheme],
    })),
  ];
  return (
    <>
      <h2>Appearance</h2>
      <p className="settings-description">
        Choose how Caelum looks on this device.
      </p>
      <fieldset className="theme-options appearance-options">
        <legend>Appearance mode</legend>
        {(
          [
            { id: "dark", name: "Dark", Icon: Moon },
            { id: "light", name: "Light", Icon: Sun },
            { id: "system", name: "System", Icon: Monitor },
          ] as const
        ).map(({ id, name, Icon }) => (
          <label className="theme-option" key={id}>
            <input
              type="radio"
              name="theme"
              value={id}
              checked={settings.theme === id}
              onChange={() => update((current) => ({ ...current, theme: id }))}
            />
            <span className="theme-option-name">
              <Icon size={14} />
              {name}
              {settings.theme === id && <Check size={14} />}
            </span>
          </label>
        ))}
      </fieldset>
      <p className="settings-note">
        System follows your device’s appearance automatically.
      </p>
      <fieldset className="theme-options palette-options">
        <legend>Theme</legend>
        {Object.entries(themes).map(([id, theme]) => (
          <label className="theme-option" key={id}>
            <input
              type="radio"
              name="theme-family"
              value={id}
              checked={settings.themeFamily === id}
              onChange={() =>
                update((current) => ({
                  ...current,
                  themeFamily: id as ThemeFamily,
                  accentColor: "theme",
                }))
              }
            />
            <PalettePreview palette={theme[resolvedTheme]} />
            <span className="theme-option-name">
              {theme.name}
              {settings.themeFamily === id && <Check size={14} />}
            </span>
          </label>
        ))}
      </fieldset>
      <p className="settings-note">
        Palettes include the workspace, code, diffs, and native window.
      </p>
      <fieldset className="accent-options">
        <legend>Accent colour</legend>
        {accents.map(({ id, name, color }) => (
          <label className="accent-option" key={id}>
            <input
              type="radio"
              name="accent-color"
              value={id}
              checked={settings.accentColor === id}
              onChange={() =>
                update((current) => ({
                  ...current,
                  accentColor: id as AccentColor,
                }))
              }
            />
            <span
              className="accent-swatch"
              style={{ "--swatch-color": color } as CSSProperties}
              aria-hidden="true"
            >
              {settings.accentColor === id && <Check size={15} />}
            </span>
            <span>{name}</span>
          </label>
        ))}
      </fieldset>
      <p className="settings-note">
        Use the theme’s accent or choose your own highlight colour.
      </p>
      {preferences.themeError && (
        <p className="settings-provider-error" role="alert">
          {preferences.themeError}
        </p>
      )}
    </>
  );
}
