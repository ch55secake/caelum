import assert from "node:assert/strict";
import test from "node:test";
import {
  createFileContext,
  fileContextLabel,
  fileReference,
  promptWithFileContext,
  splitFileContext,
} from "./fileContext.js";
import { conversationPresentation, lastUserMessage } from "./shellMessages.js";
import { toolFilePaths } from "./toolSummary.js";

test("file and selection snapshots round trip with exact text, paths and line ranges", () => {
  const text = "Explain this change.\nKeep the answer short.";
  const files = [
    createFileContext(
      "src/file with spaces.ts",
      'const tag = "</caelum-file-context>";\n🚀\t\r\n',
    ),
    createFileContext("src/App.tsx", "  return <App />;\n", {
      startLine: 20,
      endLine: 21,
    }),
  ];
  const prompt = promptWithFileContext(text, files);
  assert.deepEqual(splitFileContext(prompt), { text, files });
  assert.equal(fileContextLabel(files[1]), "src/App.tsx:20-21");
  assert.equal(
    fileContextLabel({ ...files[1], endLine: 20 }),
    "src/App.tsx:20",
  );
  const entry = { id: "sent", kind: "user", text: prompt };
  assert.deepEqual(conversationPresentation([entry]), [
    { ...entry, text, fileContext: files },
  ]);
  assert.equal(lastUserMessage([entry]), text);
  assert.equal(entry.text, prompt);
  assert.equal(promptWithFileContext(text, []), text);
});

test("snapshots cap UTF-8 bytes without splitting Unicode and mark partial source previews", () => {
  assert.equal(
    createFileContext("bom.txt", "\uFEFFhello\r\n").content,
    "\uFEFFhello\r\n",
  );
  const content = "x".repeat(65535) + "🚀";
  const file = createFileContext("large.txt", content);
  assert.equal(file.content, "x".repeat(65535));
  assert.equal(file.truncated, true);
  const unicode = createFileContext("unicode.txt", "é🚀".repeat(20000));
  assert.ok(new TextEncoder().encode(unicode.content).length <= 65536);
  assert.ok(!unicode.content.includes("�"));
  assert.equal(
    createFileContext("exact.txt", "x".repeat(65536)).truncated,
    false,
  );
  assert.equal(
    createFileContext("preview.txt", "short preview", undefined, true)
      .truncated,
    true,
  );
});

test("incomplete, malformed and unsupported context envelopes stay visible as ordinary text", () => {
  const file = createFileContext("src/App.tsx", "hello");
  const envelope = (value) =>
    `Example\n\n<caelum-file-context>\n${JSON.stringify(value)}\n</caelum-file-context>`;
  const valid = promptWithFileContext("Example", [file]);
  const inputs = [
    valid.slice(0, -1),
    `${valid}\nother text`,
    `\`\`\`xml\n${valid}\n\`\`\``,
    envelope(null),
    envelope({ version: 2, files: [file] }),
    envelope({ version: 1, files: [] }),
    envelope({ version: 1, files: [file, file] }),
    envelope({ version: 1, files: [{ ...file, endLine: 10 }] }),
    envelope({ version: 1, files: [{ ...file, startLine: 10, endLine: 2 }] }),
    envelope({ version: 1, files: [{ ...file, content: "é".repeat(65536) }] }),
    envelope({
      version: 1,
      files: Array.from({ length: 17 }, (_, id) => ({
        ...file,
        id: String(id),
      })),
    }),
  ];
  for (const text of inputs)
    assert.deepEqual(splitFileContext(text), { text, files: [] });
});

test("local file references preserve line coordinates and reject external or invalid paths", () => {
  for (const value of [
    "src/App.tsx:12:5",
    "./src/App.tsx:12:5",
    "/work/caelum/src/App.tsx:12:5",
    "file:///work/caelum/src/App.tsx:12:5",
  ]) {
    assert.deepEqual(fileReference(value, "/work/caelum"), {
      path: "src/App.tsx",
      line: 12,
      column: 5,
    });
  }
  assert.deepEqual(fileReference("src/App.tsx#L12-L15", "/work/caelum"), {
    path: "src/App.tsx",
    line: 12,
    column: undefined,
  });
  assert.equal(
    fileReference("file:///work/caelum/file%20name.ts", "/work/caelum").path,
    "file name.ts",
  );
  assert.equal(fileReference("Dockerfile", "/work/caelum").path, "Dockerfile");
  for (const value of [
    "../outside.ts",
    "src/../../outside.ts",
    "/work/caelum-other/file.ts",
    "https://example.com/file.ts",
    "javascript:run.ts",
    "file://remote/work/caelum/file.ts",
    "src/App.tsx:0",
    "src/App.tsx:99999999999999999",
    "src\n/file.ts",
    "src",
    "",
  ]) {
    assert.equal(fileReference(value, "/work/caelum"), null, value);
  }
});

test("tool file links come from structured paths and patches rather than executable arguments", () => {
  const patchText =
    "*** Begin Patch\n*** Update File: src/old.ts\n*** Move to: src/new file.ts\n*** End Patch";
  assert.deepEqual(toolFilePaths(JSON.stringify({ patchText })), [
    "src/old.ts",
    "src/new file.ts",
  ]);
  assert.deepEqual(
    toolFilePaths({
      files: [{ path: "a.ts" }, { file_path: "b.ts" }, { filePath: "a.ts" }],
    }),
    ["a.ts", "b.ts"],
  );
  assert.deepEqual(
    toolFilePaths({
      command: "cat src/private.ts",
      code: "throw new Error('do not evaluate')",
    }),
    [],
  );
});
