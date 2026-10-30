declare module "monaco-editor/esm/vs/editor/editor.api.js" {
  export * from "monaco-editor";
}

declare module "monaco-editor/esm/vs/languages/definitions/*/register.js";
declare module "monaco-editor/esm/vs/language/json/monaco.contribution.js";
declare module "monaco-editor/esm/vs/languages/features/json/tokenization.js" {
  export function createTokenizationSupport(
    comments: boolean,
  ): import("monaco-editor").languages.TokensProvider;
}
