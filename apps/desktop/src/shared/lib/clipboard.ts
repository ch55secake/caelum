import { isTauri } from "@tauri-apps/api/core";
import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";

export function copyText(text: string) {
  return isTauri() ? writeText(text) : navigator.clipboard.writeText(text);
}

export function pasteText() {
  return isTauri() ? readText() : navigator.clipboard.readText();
}
