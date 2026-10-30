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

test("cancellation is a current-turn outcome rather than a permanent transcript message", () => {
  for (const provider of ["codex", "opencode"]) {
    const started =
      provider === "codex" ? "turn/started" : "session.execution.started";
    const cancelled =
      provider === "codex"
        ? "turn/completed"
        : "session.execution.interrupted";
    let live = applyConversationEvent(session(), provider, started, {});
    live = applyConversationEvent(live, provider, cancelled, {
      turn: { id: "cancelled", status: "interrupted" },
    });
    assert.equal(live.lastOutcome, "interrupted");
    assert.equal(live.isWorking, false);
    assert.deepEqual(live.conversation, []);
    live = applyConversationEvent(live, provider, started, {});
    assert.equal(live.lastOutcome, null);
    assert.equal(live.isWorking, true);
  }
  const restored = historyEntries("opencode", [
    { id: "msg_cancelled", type: "idle", outcome: "interrupted" },
    {
      id: "msg_resumed",
      type: "assistant",
      content: [{ type: "text", text: "Continuing the work" }],
    },
    {
      id: "msg_error",
      type: "idle",
      outcome: "failed",
      error: { message: "Provider unavailable" },
    },
  ]);
  assert.deepEqual(
    restored.map((entry) => entry.text),
    ["Continuing the work", "Provider unavailable"],
  );
});

test("Codex compaction is a stable checkpoint that keeps execution active until the turn ends", () => {
  let live = session();
  const item = { id: "compact-1", type: "contextCompaction" };
  live = applyConversationEvent(live, "codex", "item/started", { item });
  assert.equal(live.isWorking, true);
  assert.equal(live.activity.label, "Compacting context…");
  assert.equal(live.conversation[0].kind, "compaction");
  assert.equal(live.conversation[0].status, "running");
  live = applyConversationEvent(live, "codex", "item/completed", { item });
  assert.equal(live.isWorking, true);
  assert.equal(live.activity.label, "Working…");
  live.conversation = mergeHistory(
    live.conversation,
    historyEntries("codex", [{ status: "inProgress", items: [item] }]),
  );
  assert.equal(live.conversation.length, 1);
  assert.equal(live.conversation[0].status, "completed");
  live = applyConversationEvent(live, "codex", "turn/completed", {
    turn: { id: "turn", status: "completed" },
  });
  live = applyConversationEvent(live, "codex", "item/started", { item });
  assert.equal(
    live.isWorking,
    false,
    "late starts must not reopen completed compaction",
  );
  const restored = historyEntries("codex", [
    {
      status: "inProgress",
      items: [
        item,
        { id: "after", type: "agentMessage", text: "Continuing" },
      ],
    },
  ]);
  assert.equal(
    restored[0].status,
    "completed",
    "compaction before a continuing response is complete",
  );
});

test("Codex cancelled and failed compactions settle without losing their checkpoint or error", () => {
  for (const outcome of ["failed", "interrupted"]) {
    const item = { id: "compact", type: "contextCompaction" };
    let live = applyConversationEvent(session(), "codex", "item/started", {
      item,
    });
    live = applyConversationEvent(live, "codex", "turn/completed", {
      turn: {
        id: "turn",
        status: outcome,
        error: { message: "Context limit reached" },
      },
    });
    assert.equal(live.isWorking, false);
    assert.equal(live.conversation[0].status, outcome);
    assert.equal(live.conversation[0].complete, true);
    if (outcome === "failed")
      assert.equal(live.conversation[0].error, "Context limit reached");
    const restored = historyEntries("codex", [
      { status: outcome, items: [item] },
    ]);
    assert.equal(restored[0].status, outcome);
    const completed = historyEntries("codex", [
      { status: outcome, items: [{ ...item, completedAtMs: 42 }] },
    ]);
    assert.equal(completed[0].status, "completed");
  }
});

for (const reason of ["auto", "manual"]) {
  test(`OpenCode ${reason} compaction streams and reconciles native history without duplicate summaries`, () => {
    let live = session();
    const inputID = reason === "manual" ? "msg_manual" : undefined;
    const messageId = inputID ?? "msg_compact";
    const send = (phase, data = {}, created = 100) => {
      live = applyConversationEvent(
        live,
        "opencode",
        `session.compaction.${phase}`,
        { reason, ...data },
        { id: "evt_compact", created },
      );
    };
    const snapshot = (status, summary) =>
      historyEntries("opencode", [
        {
          id: messageId,
          type: "compaction",
          reason,
          status,
          summary,
          time: { created: 100 },
        },
      ]);
    send("started", { inputID });
    assert.equal(live.conversation[0].id, `opencode-${messageId}`);
    assert.equal(live.activity.label, "Compacting context…");
    send("delta", { text: "Keep the" }, 110);
    live.conversation = mergeHistory(
      live.conversation,
      snapshot("running", "Keep the decisions"),
    );
    send("started", { inputID });
    send("delta", { text: " decisions" }, 120);
    assert.equal(live.conversation[0].text, "Keep the decisions");
    assert.equal(live.conversation.length, 1);
    send(
      "ended",
      { text: "## Summary\nKeep the decisions and next steps." },
      130,
    );
    assert.equal(
      live.isWorking,
      true,
      "compaction completion is not execution completion",
    );
    assert.equal(live.conversation[0].status, "completed");
    live.conversation = mergeHistory(
      live.conversation,
      snapshot("running", "stale summary"),
    );
    assert.equal(
      live.conversation[0].text,
      "## Summary\nKeep the decisions and next steps.",
    );
    assert.equal(live.conversation[0].status, "completed");
    live = applyConversationEvent(
      live,
      "opencode",
      "session.execution.succeeded",
      {},
    );
    send("started", { inputID });
    send("delta", { text: " late" }, 140);
    assert.equal(live.isWorking, false);
    assert.equal(live.conversation.length, 1);
    live = applyConversationEvent(
      live,
      "opencode",
      "session.compaction.started",
      { reason },
      { id: "evt_second", created: 200 },
    );
    send("ended", { text: "stale previous result" }, 150);
    assert.equal(live.conversation[1].status, "running");
    assert.equal(live.conversation[1].text, "");
  });
}

test("OpenCode missed starts and terminal snapshots recover without inventing checkpoint IDs", () => {
  let live = applyConversationEvent(
    session(),
    "opencode",
    "session.compaction.ended",
    { text: "Recovered summary" },
    { id: "evt_end", created: 120 },
  );
  assert.equal(live.conversation.length, 0);
  live.conversation = mergeHistory(
    live.conversation,
    historyEntries("opencode", [
      {
        id: "msg_start",
        type: "compaction",
        status: "completed",
        reason: "auto",
        summary: "Recovered summary",
        time: { created: 100 },
      },
    ]),
  );
  live = applyConversationEvent(
    live,
    "opencode",
    "session.compaction.started",
    { reason: "auto" },
    { id: "evt_start", created: 100 },
  );
  assert.equal(live.isWorking, false);
  assert.equal(live.conversation.length, 1);
  assert.equal(live.conversation[0].text, "Recovered summary");
});

test("OpenCode idle history settles unfinished compaction before later work", () => {
  for (const outcome of ["succeeded", "failed", "interrupted"]) {
    const restored = historyEntries("opencode", [
      {
        id: "msg_compact",
        type: "compaction",
        status: "running",
        summary: "Partial summary",
        reason: "auto",
      },
      {
        id: "msg_idle",
        type: "idle",
        outcome,
        error: { message: "Provider unavailable" },
      },
      {
        id: "msg_next",
        type: "compaction",
        status: "running",
        reason: "auto",
      },
    ]);
    assert.equal(
      restored[0].status,
      outcome === "succeeded" ? "completed" : outcome,
    );
    assert.equal(restored[0].complete, true);
    assert.equal(restored[0].text, "Partial summary");
    if (outcome === "failed")
      assert.equal(restored[0].error, "Provider unavailable");
    assert.equal(restored.at(-1).status, "running");
    assert.equal(restored.at(-1).complete, false);
  }
});

test("OpenCode compaction failures, shutdown resumption and native checkpoints are explicit", () => {
  let live = applyConversationEvent(
    session(),
    "opencode",
    "session.compaction.started",
    { reason: "auto" },
    { id: "evt_compact", created: 100 },
  );
  live = applyConversationEvent(
    live,
    "opencode",
    "session.execution.interrupted",
    { reason: "shutdown" },
  );
  assert.equal(live.conversation[0].status, "interrupted");
  assert.equal(live.conversation[0].complete, false);
  assert.equal(live.lastOutcome, "paused");
  live = applyConversationEvent(
    live,
    "opencode",
    "session.compaction.delta",
    { text: "Resumed" },
    { created: 150 },
  );
  assert.equal(live.conversation[0].status, "running");
  assert.equal(live.isWorking, true);
  live = applyConversationEvent(
    live,
    "opencode",
    "session.compaction.failed",
    { error: { message: "Summary request failed" } },
    { created: 200 },
  );
  assert.equal(live.conversation[0].status, "failed");
  assert.equal(live.conversation[0].error, "Summary request failed");
  live = applyConversationEvent(
    live,
    "opencode",
    "session.execution.failed",
    {
      error: { message: "Summary request failed" },
    },
  );
  assert.equal(live.isWorking, false);
  const restored = historyEntries("opencode", [
    {
      id: "msg_native",
      type: "compaction",
      status: "completed",
      reason: "auto",
      summary: "",
      native: { encrypted_content: "opaque" },
      time: { created: 300 },
    },
  ]);
  assert.equal(restored[0].kind, "compaction");
  assert.equal(restored[0].status, "completed");
  assert.equal(restored[0].text, "");
  assert.equal(restored[0].input, undefined);
});
