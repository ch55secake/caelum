import assert from "node:assert/strict";
import test from "node:test";
import {
  applyConversationEvent,
  historyEntries,
  mergeHistory,
} from "./conversationState.js";
const session = () => ({ conversation: [], isWorking: false });

test("Codex live items and history share IDs, preserve complete messages and structured tool results", () => {
  let live = session();
  const item = {
    id: "call",
    type: "commandExecution",
    command: "pnpm test",
    status: "inProgress",
  };
  live = applyConversationEvent(live, "codex", "item/started", { item });
  live = applyConversationEvent(
    live,
    "codex",
    "item/commandExecution/outputDelta",
    { itemId: "call", delta: "pass\n" },
  );
  live = applyConversationEvent(live, "codex", "item/completed", {
    item: {
      ...item,
      status: "completed",
      aggregatedOutput: "pass\n",
      durationMs: 420,
    },
  });
  live = applyConversationEvent(live, "codex", "item/agentMessage/delta", {
    itemId: "answer",
    delta: "All ",
  });
  live = applyConversationEvent(live, "codex", "item/completed", {
    item: { id: "answer", type: "agentMessage", text: "All passed" },
  });
  const restored = historyEntries("codex", [
    {
      status: "completed",
      items: [
        {
          ...item,
          status: "completed",
          aggregatedOutput: "pass\n",
          durationMs: 420,
        },
        { id: "answer", type: "agentMessage", text: "All passed" },
      ],
    },
  ]);
  const merged = mergeHistory(live.conversation, restored);
  assert.equal(merged.length, 2);
  assert.equal(merged[0].input, "pnpm test");
  assert.equal(merged[0].output, "pass\n");
  assert.equal(merged[0].status, "completed");
  assert.equal(merged[1].text, "All passed");
  assert.equal(merged[1].kind, "assistant");
});

test("OpenCode tool input, failure, output and history retain their identities", () => {
  let live = session();
  live = applyConversationEvent(
    live,
    "opencode",
    "session.tool.input.started",
    { id: "tool", name: "bash" },
  );
  live = applyConversationEvent(live, "opencode", "session.tool.input.delta", {
    id: "tool",
    delta: '{"command":"pnpm test"}',
  });
  live = applyConversationEvent(live, "opencode", "session.tool.called", {
    id: "tool",
    input: { command: "pnpm test" },
  });
  live = applyConversationEvent(live, "opencode", "session.tool.failed", {
    id: "tool",
    error: { message: "Exit code 1" },
    content: [{ type: "text", text: "failed assertion" }],
  });
  const restored = historyEntries("opencode", [
    {
      id: "msg",
      type: "assistant",
      content: [
        {
          type: "tool",
          id: "tool",
          name: "bash",
          state: {
            status: "error",
            input: { command: "pnpm test" },
            error: { message: "Exit code 1" },
            content: [{ type: "text", text: "failed assertion" }],
          },
          time: { ran: 100, completed: 400 },
        },
      ],
    },
  ]);
  const merged = mergeHistory(live.conversation, restored);
  assert.equal(merged.length, 1);
  assert.equal(merged[0].status, "failed");
  assert.deepEqual(merged[0].input, { command: "pnpm test" });
  assert.equal(merged[0].output, "failed assertion");
  assert.equal(merged[0].error, "Exit code 1");
});

test("history reconciliation keeps live suffixes, older pages, and optimistic identity without duplication", () => {
  const entry = (id, text) => ({ id, text, kind: "assistant" });
  let live = [
    entry("old", "Old"),
    entry("a", "hello world"),
    entry("b", "still streaming"),
  ];
  live = mergeHistory(
    live,
    [entry("a", "hello"), entry("b", "still")],
    "refresh",
  );
  assert.deepEqual(
    live.map((item) => item.id),
    ["old", "a", "b"],
  );
  assert.equal(live[1].text, "hello world");
  live = mergeHistory(live, [entry("older", "Older"), entry("old", "Old")]);
  assert.deepEqual(
    live.map((item) => item.id),
    ["older", "old", "a", "b"],
  );
  assert.equal(
    mergeHistory([entry("x", "world!")], [entry("x", "hello world")])[0].text,
    "hello world!",
  );
  const repeated = [
    { id: "previous", kind: "user", text: "Again" },
    { id: "local-new", kind: "user", text: "Again", optimistic: true },
  ];
  assert.deepEqual(
    mergeHistory(repeated, [
      { id: "previous", kind: "user", text: "Again" },
      { id: "new", kind: "user", text: "Again" },
    ]).map((item) => item.id),
    ["previous", "new"],
  );
});

test("long sessions retain all entries while live events continue", () => {
  let live = session();
  for (let index = 0; index < 450; index++)
    live = applyConversationEvent(live, "codex", "item/completed", {
      item: {
        id: String(index),
        type: "agentMessage",
        text: `Message ${index}`,
      },
    });
  assert.equal(live.conversation.length, 450);
  assert.equal(live.conversation[0].text, "Message 0");
});

test("reconnect backfill pages fill a gap without moving older cached turns", () => {
  const entry = (id) => ({ id, kind: "assistant", text: id });
  let live = [entry("old"), entry("latest")];
  live = mergeHistory(
    live,
    [entry("gap-2"), entry("gap-3")],
    "prepend",
    "latest",
  );
  assert.deepEqual(
    live.map((item) => item.id),
    ["old", "gap-2", "gap-3", "latest"],
  );
  live = mergeHistory(live, [entry("old"), entry("gap-1")], "prepend", "gap-2");
  assert.deepEqual(
    live.map((item) => item.id),
    ["old", "gap-1", "gap-2", "gap-3", "latest"],
  );
});

test("a history snapshot ahead of the event stream does not duplicate late deltas", () => {
  let live = applyConversationEvent(
    session(),
    "codex",
    "item/agentMessage/delta",
    { itemId: "a", delta: "Hello" },
  );
  live.conversation = mergeHistory(live.conversation, [
    { id: "codex-a", kind: "assistant", text: "Hello world" },
  ]);
  live = applyConversationEvent(live, "codex", "item/agentMessage/delta", {
    itemId: "a",
    delta: " world",
  });
  live = applyConversationEvent(live, "codex", "item/agentMessage/delta", {
    itemId: "a",
    delta: "!",
  });
  assert.equal(live.conversation[0].text, "Hello world!");
  live.conversation = mergeHistory(live.conversation, [
    {
      id: "codex-a",
      kind: "assistant",
      text: "Hello world! Done.",
      complete: true,
    },
  ]);
  live = applyConversationEvent(live, "codex", "item/agentMessage/delta", {
    itemId: "a",
    delta: " Done.",
  });
  assert.equal(live.conversation[0].text, "Hello world! Done.");
});

test("command output reconciles an in-flight snapshot without repeating streamed chunks", () => {
  let live = applyConversationEvent(session(), "codex", "item/started", {
    item: { id: "cmd", type: "commandExecution", command: "pnpm test" },
  });
  live = applyConversationEvent(
    live,
    "codex",
    "item/commandExecution/outputDelta",
    { itemId: "cmd", delta: "Running" },
  );
  live.conversation = mergeHistory(live.conversation, [
    {
      id: "codex-cmd",
      kind: "tool",
      text: "",
      status: "running",
      output: "Running tests",
    },
  ]);
  live = applyConversationEvent(
    live,
    "codex",
    "item/commandExecution/outputDelta",
    { itemId: "cmd", delta: " tests" },
  );
  assert.equal(live.conversation[0].output, "Running tests");
});
