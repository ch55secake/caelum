export type DiffLine = {
  text: string;
  kind: "meta" | "hunk" | "context" | "added" | "removed";
  oldLine?: number;
  newLine?: number;
};
export type SyntaxToken = { offset: number; type: string };

export function diffLines(content: string): DiffLine[] {
  let inHunk = false;
  let oldLine = 0;
  let newLine = 0;
  return content.split("\n").map((text) => {
    const hunk = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(text);
    if (hunk) {
      inHunk = true;
      oldLine = Number(hunk[1]);
      newLine = Number(hunk[2]);
      return { text, kind: "hunk" };
    }
    if (text.startsWith("diff ")) inHunk = false;
    const kind = inHunk
      ? ({ "+": "added", "-": "removed", " ": "context" } as const)[
          text[0] as "+" | "-" | " "
        ]
      : undefined;
    return {
      text,
      kind: kind ?? "meta",
      ...(kind === "removed" || kind === "context"
        ? { oldLine: oldLine++ }
        : {}),
      ...(kind === "added" || kind === "context" ? { newLine: newLine++ } : {}),
    };
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
  return /^syntax-(?:comment|keyword|string|number|property|type)$/.test(type)
    ? type
    : undefined;
}
