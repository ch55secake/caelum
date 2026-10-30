const compact = (value) =>
  (typeof value === "string"
    ? value
    : value == null
      ? ""
      : JSON.stringify(value)
  )
    .replace(/\s+/g, " ")
    .trim();

function patchFiles(text) {
  if (typeof text !== "string") return [];
  const files = [];
  for (const line of text.split(/\r?\n/)) {
    const file = line.match(/^\*\*\* (?:Add|Update|Delete) File: (.+)$/);
    const move = line.match(/^\*\*\* Move to: (.+)$/);
    if (file) files.push(file[1].trim());
    else if (move && files.length)
      files[files.length - 1] += ` → ${move[1].trim()}`;
  }
  return [...new Set(files)];
}

/** Summarize native tool arguments without evaluating executable input. */
export function toolSummary(value) {
  let input = value;
  if (typeof input === "string") {
    try {
      input = JSON.parse(input);
    } catch {
      // Freeform patches, commands, and incomplete streamed JSON stay text.
    }
  }
  const fields = input && typeof input === "object" ? input : {};
  const files = patchFiles(
    fields.patchText ??
      fields.patch ??
      (typeof input === "string" ? input : null),
  );
  if (files.length) return files.join(", ");
  // Codex fileChange items provide a list of changed paths instead of a patch envelope.
  if (
    Array.isArray(input) &&
    input.every((change) => typeof change?.path === "string")
  )
    return [...new Set(input.map((change) => change.path))].join(", ");
  if (typeof fields.code === "string") {
    const code = fields.code
      .split(/\r?\n/)
      .filter((line) => !line.trimStart().startsWith("//"))
      .join(" ");
    return compact(code || fields.code).slice(0, 500);
  }
  const summary =
    fields.command ??
    fields.filePath ??
    fields.path ??
    fields.pattern ??
    fields.query ??
    fields.description ??
    input;
  return compact(Array.isArray(summary) ? summary.join(" ") : summary).slice(
    0,
    500,
  );
}
