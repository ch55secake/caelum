export type ThemeFamily = "caelum" | "catppuccin" | "nord" | "tokyo-night";
export type ThemePalette = {
  accent: string;
  solid: string;
  hover: string;
  tokens: Record<string, string>;
};
export const themes: Record<
  ThemeFamily,
  {
    name: string;
    dark: ThemePalette;
    light: ThemePalette;
  }
>;
