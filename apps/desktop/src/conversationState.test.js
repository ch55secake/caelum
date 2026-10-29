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
  live = applyConversationEvent(
    live,
    "opencode",
    "session.tool.input.delta",
    {
      id: "tool",
      delta: '{"command":"pnpm test"}',
    },
  );
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
  live = mergeHistory(
    live,
    [entry("old"), entry("gap-1")],
    "prepend",
    "gap-2",
  );
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

test("OpenCode interleaved reasoning, text and tools reconcile by type-scoped ordinals", () => {
  let live = session();
  const send = (method, data = {}) => {
    live = applyConversationEvent(live, "opencode", method, {
      assistantMessageID: "answer",
      ...data,
    });
  };
  send("session.reasoning.started", { ordinal: 0 });
  send("session.reasoning.delta", { ordinal: 0, delta: "Inspect the" });
  assert.equal(live.activity.label, "Thinking…");
  const snapshot = (complete = false) =>
    historyEntries("opencode", [
      {
        id: "answer",
        type: "assistant",
        time: complete ? { completed: 42 } : {},
        content: [
          { type: "reasoning", text: "Inspect the project" },
          { type: "text", text: "I’ll inspect it." },
          {
            type: "tool",
            id: "read",
            name: "read",
            state: { status: "completed" },
          },
          { type: "reasoning", text: "Check the result" },
          { type: "text", text: "Finished." },
        ],
      },
    ]);
  // An in-flight snapshot can be ahead of the SSE reader, including starts.
  live.conversation = mergeHistory(live.conversation, snapshot());
  send("session.reasoning.started", { ordinal: 0 });
  send("session.reasoning.delta", { ordinal: 0, delta: " project" });
  send("session.reasoning.ended", {
    ordinal: 0,
    text: "Inspect the project",
  });
  send("session.text.started", { ordinal: 0 });
  send("session.text.delta", { ordinal: 0, delta: "I’ll inspect it." });
  send("session.text.ended", { ordinal: 0, text: "I’ll inspect it." });
  send("session.reasoning.started", { ordinal: 1 });
  send("session.reasoning.delta", { ordinal: 1, delta: "Check the result" });
  send("session.text.started", { ordinal: 1 });
  send("session.text.delta", { ordinal: 1, delta: "Finished." });
  send("session.text.ended", { ordinal: 1, text: "Finished." });
  send("session.step.ended");
  assert.equal(
    live.isWorking,
    true,
    "a step boundary is not execution completion",
  );
  send("session.execution.succeeded");
  assert.equal(live.isWorking, false);
  assert.equal(live.activity, null);
  live.conversation = mergeHistory(live.conversation, snapshot(true));
  assert.deepEqual(
    live.conversation.map(({ kind, text }) => [kind, text]),
    [
      ["reasoning", "Inspect the project"],
      ["assistant", "I’ll inspect it."],
      ["tool", ""],
      ["reasoning", "Check the result"],
      ["assistant", "Finished."],
    ],
  );
  send("session.reasoning.delta", { ordinal: 1, delta: "Check the result" });
  assert.equal(
    live.isWorking,
    false,
    "late deltas cannot reopen a completed trace",
  );
  assert.equal(live.conversation[3].text, "Check the result");
});

test("Codex streams indexed summaries, prefers them to raw content, and adopts authoritative completion", () => {
  let live = session();
  const send = (method, data) => {
    live = applyConversationEvent(live, "codex", method, {
      itemId: "thinking",
      ...data,
    });
  };
  send("item/reasoning/textDelta", {
    contentIndex: 0,
    delta: "Detailed reasoning",
  });
  assert.equal(live.conversation[0].text, "Detailed reasoning");
  send("item/reasoning/summaryTextDelta", {
    summaryIndex: 0,
    delta: "Inspect",
  });
  const restored = historyEntries("codex", [
    {
      status: "inProgress",
      items: [
        {
          id: "thinking",
          type: "reasoning",
          summary: ["Inspect files"],
          content: ["Detailed reasoning"],
        },
      ],
    },
  ]);
  live.conversation = mergeHistory(live.conversation, restored);
  send("item/started", {
    item: { id: "thinking", type: "reasoning", summary: [], content: [] },
  });
  send("item/reasoning/summaryTextDelta", {
    summaryIndex: 0,
    delta: " files",
  });
  send("item/reasoning/summaryPartAdded", { summaryIndex: 1 });
  send("item/reasoning/summaryTextDelta", {
    summaryIndex: 1,
    delta: "Choose a fix",
  });
  assert.equal(live.conversation[0].text, "Inspect files\n\nChoose a fix");
  assert.equal(live.isWorking, true);
  assert.equal(live.activity.label, "Thinking…");
  send("item/completed", {
    item: {
      id: "thinking",
      type: "reasoning",
      summary: ["Inspected files", "Chose a fix"],
      content: ["Detailed reasoning"],
    },
  });
  send("item/reasoning/summaryTextDelta", {
    summaryIndex: 1,
    delta: " late",
  });
  assert.equal(live.conversation[0].text, "Inspected files\n\nChose a fix");
  assert.equal(live.conversation[0].complete, true);
});

test("retry, interruption, and missed final deltas settle activity and keep restorable thinking", () => {
  let live = applyConversationEvent(session(), "opencode", "session.status", {
    status: { type: "busy" },
  });
  live = applyConversationEvent(live, "opencode", "session.reasoning.delta", {
    assistantMessageID: "a",
    ordinal: 0,
    delta: "Partial",
  });
  live = applyConversationEvent(live, "opencode", "session.retry.scheduled", {
    attempt: 2,
    error: { message: "Rate limit" },
  });
  assert.equal(live.activity.label, "Retrying · attempt 2");
  assert.equal(live.activity.detail, "Rate limit");
  live = applyConversationEvent(
    live,
    "opencode",
    "session.execution.interrupted",
    {},
  );
  assert.equal(live.isWorking, false);
  assert.equal(live.activity, null);
  assert.equal(live.conversation[0].status, "interrupted");
  live.conversation = mergeHistory(
    live.conversation,
    historyEntries("opencode", [
      {
        id: "a",
        type: "assistant",
        time: { completed: 42 },
        content: [{ type: "reasoning", text: "Partial trace recovered" }],
      },
    ]),
  );
  assert.equal(live.conversation[0].text, "Partial trace recovered");
});
