const OPEN = "\n\n<caelum-file-context>\n";
const CLOSE = "\n</caelum-file-context>";
export const MAX_FILE_CONTEXT = 16;
const MAX_SNAPSHOT_BYTES = 64 * 1024;
const encoder = new TextEncoder();

export function fileContextLabel(file) {
  return (
    file.path +
    (file.startLine
      ? `:${file.startLine}${file.endLine !== file.startLine ? `-${file.endLine}` : ""}`
      : "")
  );
}

export function createFileContext(path, content, range, truncated = false) {
  const bytes = encoder.encode(content);
  let end = Math.min(bytes.length, MAX_SNAPSHOT_BYTES);
  while (end < bytes.length && (bytes[end] & 0xc0) === 0x80) end--;
  const excerpt = new TextDecoder("utf-8", { ignoreBOM: true }).decode(
    bytes.subarray(0, end),
  );
  return {
    id: crypto.randomUUID(),
    path,
    content: excerpt,
    ...(range ? { startLine: range.startLine, endLine: range.endLine } : {}),
    truncated: truncated || end < bytes.length,
  };
}

export function promptWithFileContext(text, files) {
  return files.length
    ? `${text}${OPEN}${JSON.stringify({ version: 1, files })}${CLOSE}`
    : text;
}

export function splitFileContext(prompt) {
  const start = prompt.lastIndexOf(OPEN);
  if (start >= 0 && prompt.endsWith(CLOSE)) {
    try {
      const value = JSON.parse(
        prompt.slice(start + OPEN.length, -CLOSE.length),
      );
      if (
        value?.version === 1 &&
        Array.isArray(value.files) &&
        value.files.length > 0 &&
        value.files.length <= MAX_FILE_CONTEXT &&
        new Set(value.files.map((file) => file?.id)).size ===
          value.files.length &&
        value.files.every(
          (file) =>
            file &&
            typeof file.id === "string" &&
            file.id &&
            typeof file.path === "string" &&
            file.path &&
            typeof file.content === "string" &&
            encoder.encode(file.content).length <= MAX_SNAPSHOT_BYTES &&
            typeof file.truncated === "boolean" &&
            ((file.startLine === undefined && file.endLine === undefined) ||
              (Number.isSafeInteger(file.startLine) &&
                file.startLine > 0 &&
                Number.isSafeInteger(file.endLine) &&
                file.endLine >= file.startLine)),
        )
      )
        return { text: prompt.slice(0, start), files: value.files };
    } catch {
      /* Ordinary messages and incomplete streamed envelopes remain text. */
    }
  }
  return { text: prompt, files: [] };
}

export function fileReference(value, project) {
  if (typeof value !== "string" || !value || /[\n\r\0]/.test(value))
    return null;
  const match = value
    .trim()
    .match(/^(.*?)(?::(\d+)(?::(\d+))?(?:-\d+)?|#L(\d+)(?:-L?\d+)?)?$/);
  if (!match) return null;
  let path = match[1];
  if (path.startsWith("file://")) {
    try {
      const url = new URL(path);
      if (url.hostname && url.hostname !== "localhost") return null;
      path = decodeURIComponent(url.pathname);
    } catch {
      return null;
    }
  }
  const root = project.replace(/\/$/, "");
  if (path.startsWith(`${root}/`)) path = path.slice(root.length + 1);
  path = path.replace(/^\.\//, "");
  if (
    /^(?:[a-z]+:|\/)/i.test(path) ||
    path.split("/").some((part) => !part || part === "." || part === "..")
  )
    return null;
  if (
    !/\.[\w-]+$/.test(path) &&
    !/^(?:Dockerfile|Makefile|LICENSE|AGENTS\.md)$/.test(path.split("/").at(-1))
  )
    return null;
  const line = match[2] ?? match[4];
  const column = match[3];
  if (
    [line, column].some(
      (value) =>
        value !== undefined &&
        (!Number.isSafeInteger(Number(value)) || Number(value) < 1),
    )
  )
    return null;
  return {
    path,
    line: line ? Number(line) : undefined,
    column: column ? Number(column) : undefined,
  };
}
