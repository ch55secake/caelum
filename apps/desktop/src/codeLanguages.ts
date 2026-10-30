export function languageForPath(path: string): string {
  const name = path.split(/[\\/]/).at(-1)?.toLowerCase() ?? "";
  const extension = name.split(".").at(-1) ?? "";
  const languages: Record<string, string> = {
    c: "c",
    cc: "cpp",
    cpp: "cpp",
    cs: "csharp",
    css: "css",
    go: "go",
    h: "cpp",
    hpp: "cpp",
    html: "html",
    java: "java",
    js: "javascript",
    json: "json",
    jsonc: "json",
    jsx: "javascript",
    md: "markdown",
    cjs: "javascript",
    mjs: "javascript",
    cts: "typescript",
    mts: "typescript",
    php: "php",
    py: "python",
    rb: "ruby",
    rs: "rust",
    sh: "shell",
    bash: "shell",
    zsh: "shell",
    sql: "sql",
    ini: "ini",
    toml: "ini",
    ts: "typescript",
    tsx: "typescript",
    xml: "xml",
    yaml: "yaml",
    yml: "yaml",
  };
  if (name === "dockerfile" || name.startsWith("dockerfile."))
    return "dockerfile";
  return languages[extension] ?? "plaintext";
}
