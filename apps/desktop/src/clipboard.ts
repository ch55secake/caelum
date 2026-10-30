import { isTauri } from "@tauri-apps/api/core";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";

export function copyText(text: string) {
  return isTauri() ? writeText(text) : navigator.clipboard.writeText(text);
}
