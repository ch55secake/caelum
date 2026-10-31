import type { AccentColor } from "./accentColors.js";
import type {
  ProviderKind,
  ProviderWorkspaceState,
} from "./workspaceState.js";

export type Theme = "dark" | "light" | "system";
export type ProviderDefaults = {
  model: string;
  effort: string;
  agent: string;
};
export type AppSettings = {
  theme: Theme;
  accentColor: AccentColor;
  defaultProvider: ProviderKind;
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
