import assert from "node:assert/strict";
import test from "node:test";
import { changedDiffLines, latestChangedFile } from "./gitReviewState.js";

const file = (path, modifiedAt, revision = modifiedAt) => ({
  path,
  modifiedAt,
  revision,
  indexStatus: " ",
  worktreeStatus: "M",
});
const diff = (unstaged = "", staged = "", untracked = "") => ({
  path: "file.ts",
  unstaged,
  staged,
  untracked,
});

test("latest edits use full-precision timestamps and detect edits to already-dirty files", () => {
  const first = file("a.ts", "1793275200000000001");
  const second = file("z.ts", "1793275200000000002");
  assert.equal(latestChangedFile(null, [first, second]), second);
  assert.equal(latestChangedFile([first, second], [second, first]), null);
  const edited = {
    ...first,
    modifiedAt: "1793275200000000003",
    revision: "new",
  };
  assert.equal(latestChangedFile([first, second], [edited, second]), edited);
  const staged = { ...first, revision: "different-index-blob" };
  assert.equal(latestChangedFile([first, second], [staged, second]), staged);
  const deleted = { ...first, modifiedAt: null, worktreeStatus: "D" };
  assert.equal(
    latestChangedFile([first, second], [deleted, second]),
    deleted,
  );
  assert.equal(latestChangedFile(null, [deleted, second]), second);
  assert.equal(latestChangedFile([first, second], []), null);
});

test("only newly edited hunks are highlighted, ignoring shifted offsets and context", () => {
  const before = diff(
    "@@ -1 +1 @@\n-old\n+existing\n@@ -40 +40 @@\n context\n-previous\n+version one",
  );
  const after = diff(
    "@@ -2 +2 @@\n-old\n+existing\n@@ -41 +41 @@\n other context\n-previous\n+version two",
  );
  assert.deepEqual(changedDiffLines(before, after), {
    unstaged: [3, 5, 6],
    staged: [],
    untracked: [],
  });
  assert.deepEqual(changedDiffLines(after, after).unstaged, []);
  assert.deepEqual(
    changedDiffLines(after, diff("@@ -2 +2 @@\n-old\n+existing")).unstaged,
    [],
  );
});

test("initial, staged, untracked, binary and repeated identical hunks remain distinguishable", () => {
  const text = "diff --git a/file b/file\n@@ -1 +1 @@\n-old\n+new";
  assert.deepEqual(changedDiffLines(null, diff(text, text, text)), {
    unstaged: [1, 2, 3],
    staged: [1, 2, 3],
    untracked: [1, 2, 3],
  });
  assert.deepEqual(
    changedDiffLines(diff(text), diff(`${text}\n@@ -20 +20 @@\n-old\n+new`))
      .unstaged,
    [4, 5, 6],
  );
  assert.deepEqual(
    changedDiffLines(null, diff("Binary files differ")).unstaged,
    [],
  );
  assert.deepEqual(changedDiffLines(null, diff()).unstaged, []);
});
