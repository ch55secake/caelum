import assert from "node:assert/strict";
import test from "node:test";
import { toolPatchPreview } from "./toolPatch.js";

test("patch snippets retain changed source, paths with spaces and renames across input formats", () => {
  const patchText = [
    "*** Begin Patch",
    "*** Update File: src/old name.rs",
    "*** Move to: src/new name.rs",
    "@@ pub fn main() {",
    "     // unchanged context",
    '-    let label = "old";',
    '+    let label = "Caelum 🚀";',
    "*** Delete File: src/unused.rs",
    "*** End Patch",
  ].join("\r\n");
  for (const input of [
    patchText,
    { patchText },
    { patch: patchText },
    JSON.stringify({ patchText }),
  ]) {
    const { files, omittedFiles } = toolPatchPreview(input);
    assert.equal(omittedFiles, 0);
    assert.equal(files[0].path, "src/old name.rs");
    assert.equal(files[0].moveTo, "src/new name.rs");
    assert.deepEqual(files[0].lines, [
      { kind: "removed", text: '-    let label = "old";' },
      { kind: "added", text: '+    let label = "Caelum 🚀";' },
    ]);
    assert.equal(files[0].additions, 1);
    assert.equal(files[0].deletions, 1);
    assert.equal(files[1].kind, "delete");
    assert.deepEqual(files[1].lines, []);
  }
});

test("large replacements show both sides with bounded files, lines and line lengths", () => {
  const patchText = [
    "*** Begin Patch",
    "*** Update File: src/large.ts",
    "@@",
    ...Array.from({ length: 50 }, (_, index) => `-const old${index} = true;`),
    ...Array.from({ length: 50 }, (_, index) => `+const new${index} = false;`),
    "*** Add File: src/generated.ts",
    `+${"x".repeat(10_000)}`,
    "+second line",
    "*** Delete File: src/unused.ts",
    "*** End Patch",
  ].join("\n");
  const { files, omittedFiles } = toolPatchPreview({ patchText });
  assert.equal(files.length, 2);
  assert.equal(omittedFiles, 1);
  assert.equal(files[0].additions, 50);
  assert.equal(files[0].deletions, 50);
  assert.equal(files[0].omittedLines, 94);
  assert.deepEqual(
    files[0].lines.map((line) => line.kind),
    ["removed", "removed", "removed", "hunk", "added", "added", "added"],
  );
  assert.equal(files[0].lines[4].text, "+const new0 = false;");
  assert.equal(files[1].lines[0].text.length, 301);
  assert.ok(files[1].lines[0].text.endsWith("…"));
  assert.equal(files[1].shortened, true);
  assert.equal(files[1].additions, 2);
  assert.equal(files[1].omittedLines, 0);
});

test("native file changes distinguish unified headers from changed lines and mark gaps", () => {
  const changes = [
    {
      path: "/work/caelum/src/main.rs",
      kind: { type: "update", movePath: "/work/caelum/src/lib.rs" },
      diff: "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1 +1 @@\n---old\n+++new\n@@ -20 +20 @@\n-old\n+new\n\\ No newline at end of file",
    },
  ];
  for (const input of [changes, { changes }, JSON.stringify(changes)]) {
    const file = toolPatchPreview(input).files[0];
    assert.equal(file.additions, 2);
    assert.equal(file.deletions, 2);
    assert.equal(file.moveTo, "/work/caelum/src/lib.rs");
    assert.deepEqual(
      file.lines.map((line) => line.text),
      ["---old", "+++new", "…", "-old", "+new"],
    );
  }
});

test("native additions and deletions can carry raw file contents including diff-like source", () => {
  const files = toolPatchPreview([
    { path: "new.txt", kind: { type: "add" }, diff: "+++source\r\nsecond\r\n" },
    { path: "old.txt", kind: { type: "delete" }, diff: "---source\n" },
  ]).files;
  assert.deepEqual(files[0].lines, [
    { kind: "added", text: "++++source" },
    { kind: "added", text: "+second" },
  ]);
  assert.deepEqual(files[1].lines, [{ kind: "removed", text: "----source" }]);
  assert.equal(files[0].additions, 2);
  assert.equal(files[1].deletions, 1);
});

test("partial patches remain readable while unrelated or incomplete JSON inputs produce no snippets", () => {
  assert.deepEqual(
    toolPatchPreview("*** Begin Patch\n*** Update File: a.ts\n@@\n-old\n+new")
      .files[0].lines,
    [
      { kind: "removed", text: "-old" },
      { kind: "added", text: "+new" },
    ],
  );
  for (const input of [
    null,
    undefined,
    123,
    [],
    [null, { path: "a.ts", diff: 7 }],
    { code: "throw new Error('never execute')" },
    { command: "echo '+source'" },
    '{"patchText":"*** Begin Patch\\n',
  ])
    assert.deepEqual(toolPatchPreview(input), { files: [], omittedFiles: 0 });
});
