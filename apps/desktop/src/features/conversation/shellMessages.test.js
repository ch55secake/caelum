import assert from "node:assert/strict";
import test from "node:test";
import {
  conversationPresentation,
  splitShellMessages,
  lastUserMessage,
} from "./shellMessages.js";

const id = "sh_1894f7b4e001iOMXKwJMCx7VJX";
const command = "nix develop --command cargo test --workspace";
const output =
  "   Compiling caelum-core v0.1.4\n\nrunning 53 tests\ntest result: ok. 53 passed; 0 failed;\n";
const notification = `<shell id="${id}" state="completed" command="${command}">\n${output}</shell>`;

test("recalling a prompt skips provider shell notifications and preserves multiline user text", () => {
  const text = "Review this change.\nKeep the draft intact.";
  assert.equal(
    lastUserMessage([
      { id: "old", kind: "user", text: "Older message" },
      { id: "prompt", kind: "user", text },
      { id: "shell", kind: "user", text: notification },
      { id: "answer", kind: "assistant", text: "Done" },
    ]),
    text,
  );
  assert.equal(
    lastUserMessage([{ id: "shell", kind: "user", text: notification }]),
    null,
  );
  assert.equal(lastUserMessage([]), null);
});

test("background shell notifications preserve command, execution identity and multiline output", () => {
  const original = {
    id: "msg-notification",
    kind: "user",
    text: notification,
  };
  const [tool] = conversationPresentation([original]);
  assert.equal(tool.kind, "tool");
  assert.equal(tool.executionId, id);
  assert.equal(tool.status, "completed");
  assert.equal(tool.input, command);
  assert.equal(tool.output, output.trimEnd());
  assert.equal(original.text, notification);
  assert.equal(
    conversationPresentation([
      { ...original, text: notification.replace("53 passed", "54 passed") },
    ])[0].id,
    tool.id,
  );
});

test("mixed messages and multiple shell results keep their order and status", () => {
  const text = `Finished checking.\n${notification}\nOne more command:\n<shell command='git diff' state='cancelled' id='second'>partial</shell>\nPlease review.`;
  const entries = conversationPresentation([
    { id: "message", kind: "assistant", text },
  ]);
  assert.deepEqual(
    entries.map((entry) => entry.kind),
    ["assistant", "tool", "assistant", "tool", "assistant"],
  );
  assert.equal(entries[0].text, "Finished checking.");
  assert.equal(entries[3].status, "interrupted");
  assert.equal(entries[4].text, "\nPlease review.");
  assert.equal(
    new Set(entries.map((entry) => entry.id)).size,
    entries.length,
  );
});

test("shell attributes decode entities without changing raw command output", () => {
  const [part] = splitShellMessages(
    '<shell id="one" command="echo &quot;&lt;ok&gt;&quot; &amp;&amp; echo &#39;hi&#39; &#x1f680;" state="failed">&lt;raw&gt;</shell>',
  );
  assert.equal(part.command, "echo \"<ok>\" && echo 'hi' 🚀");
  assert.equal(part.output, "&lt;raw&gt;");
  assert.equal(part.state, "failed");
});

test("fenced examples, indented code and malformed envelopes remain message text", () => {
  const examples = [
    `\`\`\`xml\n${notification}\n\`\`\``,
    `~~~~\n${notification}\n~~~~`,
    notification
      .split("\n")
      .map((line) => `    ${line}`)
      .join("\n"),
    notification.replace("</shell>", ""),
    notification.replace('state="completed"', 'state="unknown"'),
    notification.replace(`id="${id}"`, ""),
    notification.replace('state="completed"', 'state="completed" invalid'),
    notification.replace(
      'state="completed"',
      'state="completed" state="failed"',
    ),
    `Example: \`${notification}\``,
  ];
  for (const text of examples) {
    const entry = { id: "example", kind: "user", text };
    assert.deepEqual(conversationPresentation([entry]), [entry]);
  }
  const mixed = `\`\`\`xml\n${notification}\n\`\`\`\n${notification}`;
  assert.deepEqual(
    splitShellMessages(mixed).map((part) => part.type),
    ["text", "shell"],
  );
});

test("tool outputs can be split for display without replacing the parent invocation", () => {
  const tool = {
    id: "tool",
    kind: "tool",
    text: "",
    name: "shell",
    output: notification,
  };
  assert.deepEqual(conversationPresentation([tool]), [tool]);
  assert.equal(splitShellMessages(tool.output)[0].id, id);
});
