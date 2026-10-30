const point = (value) => ({
  line: Number.isSafeInteger(value?.line) && value.line > 0 ? value.line : 1,
  column:
    Number.isSafeInteger(value?.column) && value.column > 0 ? value.column : 1,
});
const nonnegative = (value) =>
  Number.isFinite(value) ? Math.max(0, value) : 0;
const readingPosition = (value) => ({
  line: point(value).line,
  offset: Number.isFinite(value?.offset) ? value.offset : 0,
  left: nonnegative(value?.left),
  anchor: point(value?.anchor),
  head: point(value?.head),
});

export function restoreFileView(value) {
  if (!value || typeof value !== "object") return null;
  if (value.main)
    return {
      main: readingPosition(value.main),
      ...(value.original ? { original: readingPosition(value.original) } : {}),
    };
  // Keep reading positions from the previous Monaco-backed viewer.
  if (value.viewState) {
    const cursor = value.cursorState?.[0];
    return {
      main: readingPosition({
        line: value.viewState.firstPosition?.lineNumber,
        offset: -value.viewState.firstPositionDeltaTop,
        left: value.viewState.scrollLeft,
        anchor: {
          line: cursor?.selectionStart?.lineNumber,
          column: cursor?.selectionStart?.column,
        },
        head: {
          line: cursor?.position?.lineNumber,
          column: cursor?.position?.column,
        },
      }),
    };
  }
  return null;
}

export function selectedFileText(content, start, end) {
  // Editor coordinates are UTF-16, but its document normalizes line endings.
  // Read the original snapshot to preserve CRLF and mixed endings exactly.
  const starts = [0];
  for (const match of content.matchAll(/\r\n|\r|\n/g))
    starts.push(match.index + match[0].length);
  const offset = ({ line, column }) => {
    const index = Math.min(starts.length, Math.max(1, line)) - 1;
    const from = starts[index];
    const to =
      index + 1 < starts.length
        ? content.slice(from, starts[index + 1]).replace(/[\r\n]+$/, "")
            .length + from
        : content.length;
    return Math.min(to, from + Math.max(0, column - 1));
  };
  return content.slice(offset(start), offset(end));
}
