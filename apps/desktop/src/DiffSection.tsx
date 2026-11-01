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
  numbered = false,
  hideHeading = false,
}: {
  title: string;
  path: string;
  content: string;
  highlights?: number[];
  numbered?: boolean;
  hideHeading?: boolean;
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
      {!hideHeading && <h2>{title}</h2>}
      <DiffCode
        path={path}
        lines={lines}
        highlights={highlights}
        numbered={numbered}
      />
    </section>
  );
}

export function DiffCode({
  path,
  lines,
  highlights,
  numbered = false,
}: {
  path: string;
  lines: DiffLine[];
  highlights?: number[];
  numbered?: boolean;
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
    <pre className={numbered ? "diff-numbered" : undefined}>
      {lines.map(({ text, kind, oldLine, newLine }, index) => {
        const lineTokens = tokens?.get(index);
        return (
          <span
            className={`diff-line ${kind}${marked.has(index) ? " latest-change" : ""}`}
            data-latest-change={marked.has(index) || undefined}
            key={index}
          >
            {numbered && (
              <>
                <span
                  className="diff-line-number"
                  data-side="old"
                  aria-hidden="true"
                >
                  {oldLine}
                </span>
                <span
                  className="diff-line-number"
                  data-side="new"
                  aria-hidden="true"
                >
                  {newLine}
                </span>
              </>
            )}
            <span className="diff-source">
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
          </span>
        );
      })}
    </pre>
  );
}
