import { Fragment, useEffect, useMemo, useState, type ReactNode } from "react";
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
  annotations,
}: {
  title: string;
  path: string;
  content: string;
  highlights?: number[];
  numbered?: boolean;
  hideHeading?: boolean;
  annotations?: ReadonlyMap<number, ReactNode>;
}) {
  const contextMenu = useContextMenu();
  const lines = useMemo(() => diffLines(content), [content]);
  if (!content) return null;
  return (
    <section
      className="git-diff-section"
      aria-label={`${title} diff`}
      onContextMenu={(event) => {
        if ((event.target as Element).closest(".diff-annotation")) return;
        contextMenu(event, [
          copyAction(`Copy ${title.toLowerCase()} diff`, content),
          copyAction("Copy relative path", path),
        ]);
      }}
    >
      {!hideHeading && <h2>{title}</h2>}
      <DiffCode
        path={path}
        lines={lines}
        highlights={highlights}
        numbered={numbered}
        annotations={annotations}
      />
    </section>
  );
}

export function DiffCode({
  path,
  lines,
  highlights,
  numbered = false,
  annotations,
}: {
  path: string;
  lines: DiffLine[];
  highlights?: number[];
  numbered?: boolean;
  annotations?: ReadonlyMap<number, ReactNode>;
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
  const rows = lines.map(({ text, kind, oldLine, newLine }, index) => {
    const lineTokens = tokens?.get(index);
    return (
      <span
        className={`diff-line ${kind}${marked.has(index) ? " latest-change" : ""}`}
        data-latest-change={marked.has(index) || undefined}
        data-old-line={oldLine}
        data-new-line={newLine}
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
      </span>
    );
  });
  // Keep highlighting across the whole hunk, but place rich comments outside
  // the code's horizontal scroller so they wrap to the available review width.
  const ends = [
    ...new Set([
      ...[...(annotations?.keys() ?? [])].filter(
        (index) => index >= 0 && index < lines.length,
      ),
      lines.length - 1,
    ]),
  ].sort((a, b) => a - b);
  return ends.map((end, index) => (
    <Fragment key={index ? ends[index - 1] + 1 : 0}>
      <pre className={numbered ? "diff-numbered" : undefined}>
        {rows.slice(index ? ends[index - 1] + 1 : 0, end + 1)}
      </pre>
      {annotations?.has(end) && (
        <div className="diff-annotation">{annotations.get(end)}</div>
      )}
    </Fragment>
  ));
}
