import type { AccentColor } from "./accentColors.js";
import type { ThemeFamily } from "./themes.js";
import type {
  ProviderKind,
  ProviderWorkspaceState,
} from "../sessions/workspaceState.js";

export type Theme = "dark" | "light" | "system";
export type ProviderDefaults = {
  model: string;
  effort: string;
  agent: string;
};
export type AppSettings = {
  notifications: {
    mode: "off" | "background" | "always";
    volume: number;
    backgroundShells: boolean;
    inputNeeded: boolean;
  };
  theme: Theme;
  themeFamily: ThemeFamily;
  accentColor: AccentColor;
  defaultProvider: ProviderKind;
  worktreeDirectory: string;
  favoriteModels: Record<ProviderKind, string[]>;
  providers: Record<ProviderKind, ProviderDefaults>;
};
export const SETTINGS_KEY: string;
export function defaultSettings(): AppSettings;
export function normalizeSettings(value: unknown): AppSettings;
export function newSessionOptions(
  workspace: ProviderWorkspaceState | undefined,
  defaults: ProviderDefaults,
): ProviderDefaults;
