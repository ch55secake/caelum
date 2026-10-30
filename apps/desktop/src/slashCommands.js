export function parseSlashCommand(text) {
  if (text.startsWith("//")) return null;
  const match = text.match(/^\/([^\s]+)(?:\s+([\s\S]*))?$/);
  return match ? { name: match[1], arguments: match[2] ?? "" } : null;
}

export function commandFiles(files) {
  return files.map((file) => ({
    uri: `data:text/plain;base64,${btoa(Array.from(new TextEncoder().encode(file.content), (byte) => String.fromCharCode(byte)).join(""))}`,
    name: file.path,
    description: `${file.startLine ? `Lines ${file.startLine}–${file.endLine} of` : "Snapshot of"} ${file.path}${file.truncated ? " (truncated)" : ""}`,
  }));
}
