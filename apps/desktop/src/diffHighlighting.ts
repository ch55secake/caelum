export type DiffLine = {
  text: string;
  kind: "meta" | "hunk" | "context" | "added" | "removed";
};
export type SyntaxToken = { offset: number; type: string };

export function diffLines(content: string): DiffLine[] {
  let inHunk = false;
  return content.split("\n").map((text) => {
    if (/^@@ -\d+(?:,\d+)? \+\d+(?:,\d+)? @@/.test(text)) {
      inHunk = true;
      return { text, kind: "hunk" };
    }
    if (text.startsWith("diff ")) inHunk = false;
    const kind = inHunk
      ? ({ "+": "added", "-": "removed", " ": "context" } as const)[
          text[0] as "+" | "-" | " "
        ]
      : undefined;
    return { text, kind: kind ?? "meta" };
  });
}

export function highlightDiff(
  lines: DiffLine[],
  tokenize: (text: string) => SyntaxToken[][],
): Map<number, SyntaxToken[]> {
  const result = new Map<number, SyntaxToken[]>();
  let hunk: number[] = [];
  const flush = () => {
    // Tokenize both versions separately so removed strings/comments cannot
    // bleed into additions. Reset at gaps, whose source context is unknown.
    for (const side of ["removed", "added"]) {
      const indices = hunk.filter(
        (index) =>
          lines[index].kind === side || lines[index].kind === "context",
      );
      const text = indices
        .map((index) => lines[index].text.slice(1))
        .join("\n");
      if (text.length > 200_000) continue;
      tokenize(text).forEach((tokens, index) => {
        if (indices[index] !== undefined) result.set(indices[index], tokens);
      });
    }
    hunk = [];
  };
  lines.forEach((line, index) => {
    if (line.kind === "hunk" || line.text.startsWith("diff ")) flush();
    else if (["context", "added", "removed"].includes(line.kind))
      hunk.push(index);
  });
  flush();
  return result;
}

export function syntaxClass(type: string): string | undefined {
  if (/^comment/.test(type)) return "syntax-comment";
  if (/^(string\.key|attribute|key(?:\.|$))/.test(type))
    return "syntax-property";
  if (/^(string|regexp)/.test(type)) return "syntax-string";
  if (/^(keyword|annotation)/.test(type)) return "syntax-keyword";
  if (/^(number|constant)/.test(type)) return "syntax-number";
  if (/^(type|tag|delimiter\.tag)/.test(type)) return "syntax-type";
  return undefined;
}
