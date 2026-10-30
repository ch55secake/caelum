import { useEffect } from "react";
import EditorComponent, { DiffEditor as DiffEditorComponent, loader, type DiffEditorProps, type EditorProps } from "@monaco-editor/react";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api.js";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import { accentColors } from "./accentColors.js";

globalThis.MonacoEnvironment = {
  getWorker(_workerId, label) {
    if (label === "json") return new jsonWorker();
    return new editorWorker();
  },
};

loader.config({ monaco });
for (const [accentColor, palette] of Object.entries(accentColors)) {
  for (const theme of ["dark", "light"] as const) {
    const dark = theme === "dark";
    const accent = palette[theme];
    monaco.editor.defineTheme(`caelum-${theme}-${accentColor}`, {
      base: dark ? "vs-dark" : "vs", inherit: true, rules: [],
      colors: {
        "editor.background": dark ? "#0B0C0E" : "#FAFAFC", "editor.foreground": dark ? "#E7E8EC" : "#252630",
        "editorLineNumber.foreground": dark ? "#737580" : "#767887", "editorLineNumber.activeForeground": accent,
        "editor.selectionBackground": `${accent}${dark ? "47" : "33"}`, "editor.inactiveSelectionBackground": `${accent}26`,
        "editor.selectionHighlightBackground": `${accent}1A`, "editor.lineHighlightBackground": dark ? "#17181C" : "#F0F0F5",
        "editorGutter.background": dark ? "#0B0C0E" : "#FAFAFC", "editorWidget.background": dark ? "#17181C" : "#FFFFFF",
        "editorWidget.border": dark ? "#25262D" : "#DCDDE5", "diffEditor.insertedTextBackground": "#5DA77526",
        "diffEditor.removedTextBackground": "#C96A7926", "focusBorder": accent, "editorCursor.foreground": accent,
        "textLink.foreground": accent, "textLink.activeForeground": accent, "editorLink.activeForeground": accent,
      },
    });
  }
}

const languageLoaders: Record<string, () => Promise<unknown>> = {
  cpp: () => import("monaco-editor/esm/vs/languages/definitions/cpp/register.js"),
  csharp: () => import("monaco-editor/esm/vs/languages/definitions/csharp/register.js"),
  css: () => import("monaco-editor/esm/vs/languages/definitions/css/register.js"),
  go: () => import("monaco-editor/esm/vs/languages/definitions/go/register.js"),
  html: () => import("monaco-editor/esm/vs/languages/definitions/html/register.js"),
  java: () => import("monaco-editor/esm/vs/languages/definitions/java/register.js"),
  javascript: () => import("monaco-editor/esm/vs/languages/definitions/javascript/register.js"),
  json: () => import("monaco-editor/esm/vs/language/json/monaco.contribution.js"),
  markdown: () => import("monaco-editor/esm/vs/languages/definitions/markdown/register.js"),
  python: () => import("monaco-editor/esm/vs/languages/definitions/python/register.js"),
  rust: () => import("monaco-editor/esm/vs/languages/definitions/rust/register.js"),
  shell: () => import("monaco-editor/esm/vs/languages/definitions/shell/register.js"),
  sql: () => import("monaco-editor/esm/vs/languages/definitions/sql/register.js"),
  typescript: () => import("monaco-editor/esm/vs/languages/definitions/typescript/register.js"),
  xml: () => import("monaco-editor/esm/vs/languages/definitions/xml/register.js"),
  yaml: () => import("monaco-editor/esm/vs/languages/definitions/yaml/register.js"),
};

function useLanguage(language: string | undefined) {
  useEffect(() => {
    const load = language ? languageLoaders[language] : undefined;
    if (load) void load().catch(() => undefined);
  }, [language]);
}

export function Editor(props: EditorProps) {
  useLanguage(props.language);
  return <EditorComponent {...props} />;
}

export function DiffEditor(props: DiffEditorProps) {
  useLanguage(props.language);
  return <DiffEditorComponent {...props} />;
}
