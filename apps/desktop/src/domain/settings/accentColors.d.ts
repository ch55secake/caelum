export type AccentColor =
  | "theme"
  | "violet"
  | "blue"
  | "teal"
  | "green"
  | "amber"
  | "rose";
export const accentColors: Record<
  Exclude<AccentColor, "theme">,
  { name: string; dark: string; light: string; solid: string; hover: string }
>;
