import { useEffect } from "react";
import EditorComponent, { DiffEditor as DiffEditorComponent, loader, type DiffEditorProps, type EditorProps } from "@monaco-editor/react";
import * as monaco from "monaco-editor/esm/vs/editor/editor.api.js";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";

globalThis.MonacoEnvironment = {
  getWorker(_workerId, label) {
    if (label === "json") return new jsonWorker();
    return new editorWorker();
  },
};

loader.config({ monaco });
monaco.editor.defineTheme("caelum-dark", {
  base: "vs-dark", inherit: true, rules: [],
  colors: {
    "editor.background": "#0B0C0E", "editor.foreground": "#E7E8EC",
    "editorLineNumber.foreground": "#737580", "editorLineNumber.activeForeground": "#B7B2D4",
    "editor.selectionBackground": "#7669C44D", "editor.lineHighlightBackground": "#17181C",
    "editorGutter.background": "#0B0C0E", "editorWidget.background": "#17181C",
    "editorWidget.border": "#25262D", "diffEditor.insertedTextBackground": "#5DA77526",
    "diffEditor.removedTextBackground": "#C96A7926",
  },
});
monaco.editor.defineTheme("caelum-light", {
  base: "vs", inherit: true, rules: [],
  colors: {
    "editor.background": "#FAFAFC", "editor.foreground": "#252630",
    "editorLineNumber.foreground": "#767887", "editorLineNumber.activeForeground": "#6653A3",
    "editor.selectionBackground": "#7669C433", "editor.lineHighlightBackground": "#F0F0F5",
    "editorGutter.background": "#FAFAFC", "editorWidget.background": "#FFFFFF",
    "editorWidget.border": "#DCDDE5", "diffEditor.insertedTextBackground": "#5DA77526",
    "diffEditor.removedTextBackground": "#C96A7926",
  },
});

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
