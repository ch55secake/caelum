const MAX_FILES = 2;
const MAX_LINES = 6;
const MAX_LINE_LENGTH = 300;

function patchLine(text) {
  const kind = text.startsWith("@@")
    ? "hunk"
    : { "+": "added", "-": "removed", " ": "context" }[text[0]];
  return kind ? { text, kind } : null;
}

function envelope(text) {
  const files = [];
  let file;
  for (const textLine of text.split(/\r?\n/)) {
    const header = textLine.match(/^\*\*\* (Add|Update|Delete) File: (.+)$/);
    if (header) {
      file = {
        path: header[2].trim(),
        kind: header[1].toLowerCase(),
        lines: [],
      };
      files.push(file);
    } else if (textLine === "*** End Patch") break;
    else if (file) {
      const move = textLine.match(/^\*\*\* Move to: (.+)$/);
      if (move) file.moveTo = move[1].trim();
      else {
        const line = patchLine(textLine);
        if (line) file.lines.push(line);
      }
    }
  }
  return files;
}

function nativeChange(change) {
  if (typeof change?.path !== "string" || typeof change.diff !== "string")
    return null;
  const kind = change.kind?.type ?? change.kind ?? "update";
  const text = change.diff.split(/\r?\n/);
  if (text.at(-1) === "") text.pop();
  const unified = text.some((line) => /^@@(?: |$)/.test(line));
  const lines = [];
  let inHunk = false;
  for (const source of text) {
    if (!unified && ["add", "delete"].includes(kind)) {
      lines.push({
        kind: kind === "add" ? "added" : "removed",
        text: `${kind === "add" ? "+" : "-"}${source}`,
      });
    } else if (/^@@(?: |$)/.test(source)) {
      inHunk = true;
      lines.push({ kind: "hunk", text: source });
    } else if (source.startsWith("diff ")) inHunk = false;
    else if (inHunk) {
      const line = patchLine(source);
      if (line) lines.push(line);
    }
  }
  const moveTo = change.kind?.movePath ?? change.kind?.move_path;
  return {
    path: change.path,
    kind: ["add", "delete"].includes(kind) ? kind : "update",
    moveTo: typeof moveTo === "string" ? moveTo : undefined,
    lines,
  };
}

function excerpt(file) {
  const added = [];
  const removed = [];
  file.lines.forEach((line, index) => {
    if (line.kind === "added") added.push(index);
    if (line.kind === "removed") removed.push(index);
  });
  // Keep both sides of a replacement, even when a large deletion comes first.
  const selected = new Set([
    ...removed.slice(0, MAX_LINES / 2),
    ...added.slice(0, MAX_LINES / 2),
  ]);
  for (const index of [...removed, ...added]) {
    if (selected.size >= MAX_LINES) break;
    selected.add(index);
  }
  const lines = [];
  let previous = -1;
  let shortened = false;
  for (const index of [...selected].sort((a, b) => a - b)) {
    if (previous >= 0 && index > previous + 1)
      lines.push({ kind: "hunk", text: "…" });
    const line = file.lines[index];
    shortened ||= line.text.length > MAX_LINE_LENGTH;
    lines.push({
      ...line,
      text:
        line.text.length > MAX_LINE_LENGTH
          ? `${line.text.slice(0, MAX_LINE_LENGTH)}…`
          : line.text,
    });
    previous = index;
  }
  return {
    ...file,
    lines,
    additions: added.length,
    deletions: removed.length,
    omittedLines: added.length + removed.length - selected.size,
    shortened,
  };
}

/** Read only the supplied patch; never inspect files or evaluate tool arguments. */
export function toolPatchPreview(value) {
  let input = value;
  if (typeof input === "string") {
    try {
      input = JSON.parse(input);
    } catch {
      // Freeform and partially streamed patch envelopes can already have lines.
    }
  }
  const text =
    typeof input === "string" ? input : (input?.patchText ?? input?.patch);
  const changes = Array.isArray(input) ? input : input?.changes;
  const files =
    typeof text === "string" &&
    /^\*\*\* (?:Begin Patch|(?:Add|Update|Delete) File:)/.test(text.trimStart())
      ? envelope(text)
      : Array.isArray(changes)
        ? changes.map(nativeChange).filter(Boolean)
        : [];
  return {
    files: files.slice(0, MAX_FILES).map(excerpt),
    omittedFiles: Math.max(0, files.length - MAX_FILES),
  };
}
