import assert from "node:assert/strict";
import test from "node:test";
import { createFileContext } from "./fileContext.js";
import { commandFiles, parseSlashCommand } from "./slashCommands.js";

test("slash commands keep native names, control arguments and multiline objectives", () => {
  assert.deepEqual(parseSlashCommand("/goal"), { name: "goal", arguments: "" });
  for (const action of ["pause", "resume", "clear"])
    assert.deepEqual(parseSlashCommand(`/goal ${action}`), {
      name: "goal",
      arguments: action,
    });
  assert.deepEqual(
    parseSlashCommand(
      '/team/review src/日本語.ts\n  Keep "quotes" and $ARGUMENTS.\n',
    ),
    {
      name: "team/review",
      arguments: 'src/日本語.ts\n  Keep "quotes" and $ARGUMENTS.\n',
    },
  );
  assert.deepEqual(parseSlashCommand("/usage \n"), {
    name: "usage",
    arguments: "",
  });
  for (const text of [
    "",
    "/",
    "/ ",
    "// comment",
    "Describe /goal",
    " /goal",
    "A prompt\n/goal",
  ])
    assert.equal(parseSlashCommand(text), null);
});

test("native command attachments transport exact UTF-8 snapshots separately from arguments", () => {
  const files = [
    createFileContext(
      "src/日本語.ts",
      '\uFEFFconst 🚀 = "é";\r\n\treturn true;\n',
      { startLine: 4, endLine: 6 },
    ),
    createFileContext("large.txt", "🚀".repeat(20_000)),
    createFileContext("empty.txt", ""),
  ];
  const encoded = commandFiles(files);
  for (const [index, file] of encoded.entries()) {
    assert.equal(file.name, files[index].path);
    assert.ok(file.uri.startsWith("data:text/plain;base64,"));
    assert.equal(
      Buffer.from(file.uri.split(",")[1], "base64").toString("utf8"),
      files[index].content,
    );
  }
  assert.equal(encoded[0].description, "Lines 4–6 of src/日本語.ts");
  assert.equal(encoded[1].description, "Snapshot of large.txt (truncated)");
});
