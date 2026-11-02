import {
  HighlightStyle,
  LanguageDescription,
  type LanguageSupport,
} from "@codemirror/language";
import { languages } from "@codemirror/language-data";
import { highlightTree, tags } from "@lezer/highlight";
import type { SyntaxToken } from "./diffHighlighting.js";

// Shared by the file viewer and patch snippets; colours come from Caelum's theme.
export const codeHighlightStyle = HighlightStyle.define([
  { tag: tags.comment, class: "syntax-comment" },
  {
    tag: [tags.keyword, tags.bool, tags.null, tags.annotation],
    class: "syntax-keyword",
  },
  { tag: [tags.string, tags.regexp, tags.character], class: "syntax-string" },
  {
    tag: [tags.number, tags.constant(tags.variableName)],
    class: "syntax-number",
  },
  { tag: [tags.propertyName, tags.attributeName], class: "syntax-property" },
  { tag: [tags.typeName, tags.className, tags.tagName], class: "syntax-type" },
]);

export async function fileLanguage(
  path: string,
): Promise<LanguageSupport | null> {
  const language = LanguageDescription.matchFilename(
    languages,
    path.split(/[\\/]/).at(-1) ?? path,
  );
  return language ? language.load() : null;
}

export async function syntaxTokenizer(path: string) {
  const support = await fileLanguage(path);
  return (text: string): SyntaxToken[][] => {
    const lines = text.split("\n");
    const tokens: SyntaxToken[][] = lines.map(() => [{ offset: 0, type: "" }]);
    if (!support) return tokens;
    const starts = [0];
    for (let index = 0; index < lines.length - 1; index++)
      starts.push(starts[index] + lines[index].length + 1);
    let line = 0;
    highlightTree(
      support.language.parser.parse(text),
      codeHighlightStyle,
      (from, to, classes) => {
        while (line + 1 < starts.length && starts[line + 1] <= from) line++;
        for (
          let index = line;
          index < lines.length && starts[index] < to;
          index++
        ) {
          const start = Math.max(0, from - starts[index]);
          const end = Math.min(lines[index].length, to - starts[index]);
          if (end <= start) continue;
          const row = tokens[index];
          if (row.at(-1)?.offset === start) row.pop();
          row.push({ offset: start, type: classes });
          if (end < lines[index].length) row.push({ offset: end, type: "" });
        }
      },
    );
    return tokens;
  };
}
