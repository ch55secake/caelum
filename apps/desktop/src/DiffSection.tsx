import { useEffect, useMemo, useState } from "react";
import { useContextMenu, copyAction } from "./ContextMenu.js";
import {
  diffLines,
  highlightDiff,
  syntaxClass,
  type DiffLine,
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
  const contextMenu = useContextMenu();
  const lines = useMemo(() => diffLines(content), [content]);
  if (!content) return null;
  return (
    <section
      className="git-diff-section"
      aria-label={`${title} diff`}
      onContextMenu={(event) =>
        contextMenu(event, [
          copyAction(`Copy ${title.toLowerCase()} diff`, content),
          copyAction("Copy relative path", path),
        ])
      }
    >
      <h2>{title}</h2>
      <DiffCode path={path} lines={lines} highlights={highlights} />
    </section>
  );
}

export function DiffCode({
  path,
  lines,
  highlights,
}: {
  path: string;
  lines: DiffLine[];
  highlights?: number[];
}) {
  const [highlighted, setHighlighted] = useState<{
    lines: typeof lines;
    path: string;
    tokens: Map<number, SyntaxToken[]>;
  } | null>(null);
  useEffect(() => {
    if (!lines.length) return;
    let disposed = false;
    void import("./codeSyntax.js")
      .then(({ syntaxTokenizer }) => syntaxTokenizer(path))
      .then((tokenize) => {
        if (!disposed)
          setHighlighted({
            lines,
            path,
            tokens: highlightDiff(lines, tokenize),
          });
      })
      .catch(() => {
        /* Keep the readable plain diff if a grammar cannot load. */
      });
    return () => {
      disposed = true;
    };
  }, [lines, path]);
  const marked = new Set(highlights);
  const tokens =
    highlighted?.lines === lines && highlighted.path === path
      ? highlighted.tokens
      : null;
  return (
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
                  <span key={token.offset} className={syntaxClass(token.type)}>
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
  );
}
