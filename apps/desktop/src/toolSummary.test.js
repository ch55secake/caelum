import assert from "node:assert/strict";
import test from "node:test";
import { toolSummary } from "./toolSummary.js";

test("patch previews identify every affected file, including renames and paths with spaces", () => {
  const patchText = [
    "*** Begin Patch",
    "*** Add File: src/new file.ts",
    "+export const ready = true;",
    "*** Update File: src/old.ts",
    "*** Move to: src/renamed.ts",
    "@@",
    "-old",
    "+new",
    "*** Delete File: src/unused.ts",
    "*** End Patch",
  ].join("\n");
  const expected =
    "src/new file.ts, src/old.ts → src/renamed.ts, src/unused.ts";
  for (const input of [
    patchText,
    { patchText },
    JSON.stringify({ patchText }),
    { patch: patchText },
  ])
    assert.equal(toolSummary(input), expected);
  assert.equal(
    toolSummary({
      patchText: "*** Begin Patch\n*** Update File: src/partial.ts\n@@",
    }),
    "src/partial.ts",
  );
  assert.equal(
    toolSummary([
      { path: "src/a.ts", diff: "…" },
      { path: "src/b.ts", diff: "…" },
    ]),
    "src/a.ts, src/b.ts",
  );
});

test("execute previews show multiline JavaScript and tool calls without executing them", () => {
  const code =
    '\n// Inspect available models\nconst result = await tools.opencode.models({\n  provider: "openai"\n});\nreturn result;';
  const expected =
    'const result = await tools.opencode.models({ provider: "openai" }); return result;';
  assert.equal(toolSummary({ code }), expected);
  assert.equal(toolSummary(JSON.stringify({ code })), expected);
  const executable = 'throw new Error("Do not execute the preview")';
  assert.equal(toolSummary({ code: executable }), executable);
  assert.equal(toolSummary({ code: "x".repeat(1000) }).length, 500);
});

test("ordinary and partially streamed tool inputs keep useful previews", () => {
  assert.equal(
    toolSummary({ command: ["git", "diff", "--stat"] }),
    "git diff --stat",
  );
  assert.equal(toolSummary({ path: "src/App.tsx" }), "src/App.tsx");
  assert.equal(
    toolSummary({ query: "native event API" }),
    "native event API",
  );
  assert.equal(
    toolSummary({ task: "Inspect the event stream" }),
    '{"task":"Inspect the event stream"}',
  );
  assert.equal(
    toolSummary('{\n  "code": "await fetch('),
    '{ "code": "await fetch(',
  );
  assert.equal(toolSummary(null), "");
  assert.equal(toolSummary(undefined), "");
});
