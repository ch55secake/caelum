import assert from "node:assert/strict";
import test from "node:test";
import { restoreFileView, selectedFileText } from "./codeViewState.js";

test("legacy reading positions migrate without losing scroll or selection", () => {
  const view = restoreFileView({
    viewState: {
      firstPosition: { lineNumber: 42, column: 1 },
      firstPositionDeltaTop: -7,
      scrollLeft: 80,
    },
    cursorState: [
      {
        selectionStart: { lineNumber: 45, column: 8 },
        position: { lineNumber: 43, column: 2 },
      },
    ],
  });
  assert.deepEqual(view, {
    main: {
      line: 42,
      offset: 7,
      left: 80,
      anchor: { line: 45, column: 8 },
      head: { line: 43, column: 2 },
    },
  });
  assert.deepEqual(restoreFileView(view), view);
  const diff = { ...view, original: { ...view.main, line: 40 } };
  assert.deepEqual(restoreFileView(diff), diff);
  assert.equal(
    restoreFileView({
      viewState: {
        firstPosition: { lineNumber: 1 },
        firstPositionDeltaTop: 10,
      },
    }).main.offset,
    -10,
  );
});

test("corrupt saved positions fall back to readable coordinates", () => {
  for (const value of [null, false, "invalid", {}, []])
    assert.equal(restoreFileView(value), null);
  assert.deepEqual(
    restoreFileView({
      main: {
        line: -1,
        offset: Infinity,
        left: -10,
        anchor: { line: 1.5, column: NaN },
        head: { line: "4", column: -2 },
      },
    }),
    {
      main: {
        line: 1,
        offset: 0,
        left: 0,
        anchor: { line: 1, column: 1 },
        head: { line: 1, column: 1 },
      },
    },
  );
});

test("UTF-16 selections preserve Unicode and original mixed line endings", () => {
  const content = 'let café = "🚀";\r\n\tsecond\rthird\nlast';
  assert.equal(
    selectedFileText(content, { line: 1, column: 13 }, { line: 1, column: 15 }),
    "🚀",
  );
  assert.equal(
    selectedFileText(content, { line: 1, column: 5 }, { line: 3, column: 4 }),
    'café = "🚀";\r\n\tsecond\rthi',
  );
  assert.equal(
    selectedFileText(content, { line: 1, column: 1 }, { line: 2, column: 1 }),
    'let café = "🚀";\r\n',
  );
  assert.equal(
    selectedFileText(content, { line: 3, column: 1 }, { line: 4, column: 99 }),
    "third\nlast",
  );
  assert.equal(
    selectedFileText("\r\n\n", { line: 1, column: 1 }, { line: 3, column: 1 }),
    "\r\n\n",
  );
  assert.equal(
    selectedFileText("", { line: 1, column: 1 }, { line: 1, column: 1 }),
    "",
  );
});
