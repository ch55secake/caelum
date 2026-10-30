import { useEffect, useMemo, useState } from "react";
import { languageForPath } from "./codeLanguages.js";
import {
  diffLines,
  highlightDiff,
  syntaxClass,
  type SyntaxToken,
} from "./diffHighlighting.js";

export function DiffSection({
  title,
  path,
  content,
  highlights,
}: {
  title: string;
  path: string;
  content: string;
  highlights?: number[];
}) {
  const lines = useMemo(() => diffLines(content), [content]);
  const language = languageForPath(path);
  const [highlighted, setHighlighted] = useState<{
    lines: typeof lines;
    language: string;
    tokens: Map<number, SyntaxToken[]>;
  } | null>(null);
  useEffect(() => {
    if (!content || language === "plaintext") return;
    let disposed = false;
    void import("./MonacoEditors.js")
      .then(({ syntaxTokenizer }) => syntaxTokenizer(language))
      .then((tokenize) => {
        if (!disposed)
          setHighlighted({
            lines,
            language,
            tokens: highlightDiff(lines, tokenize),
          });
      })
      .catch(() => {
        /* Keep the readable plain diff if a grammar cannot load. */
      });
    return () => {
      disposed = true;
    };
  }, [content, lines, language]);
  if (!content) return null;
  const marked = new Set(highlights);
  const tokens =
    highlighted?.lines === lines && highlighted.language === language
      ? highlighted.tokens
      : null;
  return (
    <section className="git-diff-section" aria-label={`${title} diff`}>
      <h2>{title}</h2>
      <pre>
        {lines.map(({ text, kind }, index) => {
          const lineTokens = tokens?.get(index);
          return (
            <span
              className={`diff-line ${kind}${marked.has(index) ? " latest-change" : ""}`}
              data-latest-change={marked.has(index) || undefined}
              key={index}
            >
              {lineTokens ? (
                <>
                  <span className="diff-marker">{text[0]}</span>
                  {lineTokens.map((token, tokenIndex) => (
                    <span
                      key={token.offset}
                      className={syntaxClass(token.type)}
                    >
                      {text.slice(
                        token.offset + 1,
                        lineTokens[tokenIndex + 1]
                          ? lineTokens[tokenIndex + 1].offset + 1
                          : undefined,
                      )}
                    </span>
                  ))}
                </>
              ) : (
                text
              )}
              {index < lines.length - 1 ? "\n" : ""}
            </span>
          );
        })}
      </pre>
    </section>
  );
}
