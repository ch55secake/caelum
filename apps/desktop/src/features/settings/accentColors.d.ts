export type AccentColor =
  "violet" | "blue" | "teal" | "green" | "amber" | "rose";
export const accentColors: Record<
  AccentColor,
  { name: string; dark: string; light: string; solid: string; hover: string }
>;
