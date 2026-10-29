const decodeAttribute = (text) =>
  text.replace(/&(?:quot|apos|amp|lt|gt|#\d+|#x[\da-f]+);/gi, (entity) => {
    const named = {
      "&quot;": '"',
      "&apos;": "'",
      "&amp;": "&",
      "&lt;": "<",
      "&gt;": ">",
    };
    if (named[entity.toLowerCase()]) return named[entity.toLowerCase()];
    const value = entity.toLowerCase().startsWith("&#x")
      ? parseInt(entity.slice(3, -1), 16)
      : parseInt(entity.slice(2, -1), 10);
    return Number.isFinite(value) && value > 0 && value <= 0x10ffff
      ? String.fromCodePoint(value)
      : entity;
  });

// These are provider notification envelopes, not arbitrary HTML. Leave quoted
// examples, malformed envelopes, and unrelated markup as ordinary message text.
export function splitShellMessages(text) {
  const parts = [];
  const pattern =
    /(?:^|\n) {0,3}<shell\b((?:"[^"]*"|'[^']*'|[^'">])*)>([\s\S]*?)<\/shell>/g;
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    let fence = null;
    for (const line of text.slice(0, match.index).split("\n")) {
      const marker = line.match(/^ {0,3}(`{3,}|~{3,})/);
      if (!marker) continue;
      if (!fence) fence = marker[1];
      else if (marker[1][0] === fence[0] && marker[1].length >= fence.length)
        fence = null;
    }
    if (fence) continue;
    const attributes = {};
    const attributePattern = /\s+([\w-]+)\s*=\s*(?:"([^"]*)"|'([^']*)')/y;
    let end = 0,
      attribute,
      malformed = false;
    while ((attribute = attributePattern.exec(match[1]))) {
      const [, key, double, single] = attribute;
      if (Object.hasOwn(attributes, key)) malformed = true;
      attributes[key] = decodeAttribute(double ?? single);
      end = attributePattern.lastIndex;
    }
    if (malformed || match[1].slice(end).trim()) continue;
    if (
      !attributes.id ||
      !attributes.command ||
      !["running", "completed", "failed", "cancelled", "interrupted"].includes(
        attributes.state,
      )
    )
      continue;
    if (match.index > last)
      parts.push({ type: "text", text: text.slice(last, match.index) });
    parts.push({
      type: "shell",
      id: attributes.id,
      command: attributes.command,
      state: attributes.state,
      output: match[2].replace(/^\s*\n/, "").replace(/\n\s*$/, ""),
    });
    last = match.index + match[0].length;
  }
  if (last < text.length) parts.push({ type: "text", text: text.slice(last) });
  return parts;
}

export function conversationPresentation(entries) {
  return entries.flatMap((entry) => {
    if (
      !["user", "assistant", "status"].includes(entry.kind) ||
      !entry.text.includes("<shell")
    )
      return [entry];
    const parts = splitShellMessages(entry.text);
    if (!parts.some((part) => part.type === "shell")) return [entry];
    return parts.flatMap((part, index) =>
      part.type === "shell"
        ? [
            {
              id: `${entry.id}:shell:${part.id}`,
              kind: "tool",
              text: "",
              name: "Run",
              executionId: part.id,
              input: part.command,
              output: part.output,
              status: part.state === "cancelled" ? "interrupted" : part.state,
            },
          ]
        : part.text.trim()
          ? [{ ...entry, id: `${entry.id}:text:${index}`, text: part.text }]
          : [],
    );
  });
}
