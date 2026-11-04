import assert from "node:assert/strict";
import test from "node:test";
import {
  activityEvent,
  activityKey,
  applyActivity,
  emptyActivity,
  projectActivity,
  readMarkers,
  savedMarkers,
  MAX_READ_MARKERS,
} from "./activityState.js";
import {
  createProjectWorkspace,
  createSessionWorkspace,
} from "./workspaceState.js";

test("completion follows whole executions and ignores steps, duplicates and stale terminals", () => {
  for (const provider of ["codex", "opencode"]) {
    const event = (method, data, envelope) =>
      activityEvent(provider, method, data, envelope);
    const start =
      provider === "codex"
        ? event("turn/started", { threadId: "a", turn: { id: "first" } })
        : event(
            "session.execution.started",
            { sessionID: "a" },
            { id: "first", created: 100 },
          );
    let record = applyActivity(undefined, start, true, 100).record;
    assert.equal(record.run.startedAt, 100);
    assert.equal(record.unread, false);
    assert.equal(
      event("session.step.ended", { sessionID: "a", tokens: {} }),
      null,
    );
    const tool = event(
      provider === "codex" ? "item/completed" : "session.tool.success",
      { threadId: "a", sessionID: "a" },
    );
    const toolResult = applyActivity(record, tool, false, 200);
    assert.equal(toolResult.completion, undefined);
    assert.equal(toolResult.record.run.finishedAt, null);
    const terminal =
      provider === "codex"
        ? event("turn/completed", {
            threadId: "a",
            turn: { id: "first", status: "completed" },
          })
        : event(
            "session.execution.succeeded",
            { sessionID: "a" },
            { id: "done-first", created: 300 },
          );
    const result = applyActivity(toolResult.record, terminal, false, 300);
    assert.equal(
      result.completion.finishedAt - result.completion.startedAt,
      200,
    );
    assert.equal(result.record.unread, true);
    assert.equal(
      applyActivity(result.record, terminal, false, 400).completion,
      undefined,
    );
    const nextStart = {
      ...start,
      executionId: provider === "codex" ? "second" : undefined,
      eventId: "second",
      at: 500,
    };
    record = applyActivity(result.record, nextStart, false, 500).record;
    assert.equal(applyActivity(record, terminal, false, 600).record, record);
    assert.equal(record.run.id, "second");
  }
});

test("unknown completion and historical replays are silent and do not mark viewed content unread", () => {
  const terminal = {
    sessionId: "a",
    type: "completed",
    outcome: "completed",
    eventId: "done",
    at: 200,
  };
  const record = { ...emptyActivity(), readAt: 300 };
  const result = applyActivity(record, terminal, false, 400);
  assert.equal(result.completion, undefined);
  assert.equal(result.record.unread, false);
  const started = applyActivity(
    undefined,
    { sessionId: "a", type: "started", eventId: "start" },
    true,
    100,
  ).record;
  const shutdown = applyActivity(
    started,
    { ...terminal, outcome: "interrupted", quiet: true },
    false,
    200,
  );
  assert.equal(shutdown.completion, undefined);
  assert.equal(shutdown.record.run.outcome, "interrupted");
});

test("persisted read markers are bounded, validated and contain no run or conversation payloads", () => {
  const records = new Map(
    Array.from({ length: MAX_READ_MARKERS + 10 }, (_, index) => [
      activityKey("/work/one", "codex", String(index)),
      {
        ...emptyActivity(),
        updatedAt: index,
        readAt: index,
        unread: index % 2 === 0,
        run: { id: "private", startedAt: 0, finishedAt: null, outcome: null },
      },
    ]),
  );
  const saved = savedMarkers(records);
  assert.equal(saved.length, MAX_READ_MARKERS);
  assert.equal(saved[0][0], activityKey("/work/one", "codex", "10"));
  assert.deepEqual(Object.keys(saved[0][1]), ["unread", "updatedAt", "readAt"]);
  const restored = readMarkers(saved);
  assert.equal(restored.get(saved[0][0]).run, null);
  assert.equal(restored.get(saved[0][0]).unread, true);
  assert.equal(readMarkers({ broken: true }).size, 0);
  assert.equal(
    readMarkers([
      ["bad", {}],
      [
        activityKey("/work", "codex", "x"),
        { unread: true, readAt: -1, updatedAt: 10 },
      ],
    ]).size,
    0,
  );
});

test("project activity deduplicates session scopes while preserving simultaneous working, unread and attention", () => {
  const workspace = createProjectWorkspace();
  const thread = {
    id: "same",
    name: "Task",
    preview: "",
    updatedAt: 1,
    isWorking: true,
  };
  workspace.providers.codex.sessions = [thread];
  workspace.providers.codex.sessionsById.same = createSessionWorkspace({
    project: { path: "/one", name: "one" },
    provider: "codex",
    thread,
  });
  workspace.providers.opencode.sessions = [thread];
  const records = new Map([
    [
      activityKey("/one", "codex", "same"),
      { ...emptyActivity(), unread: true },
    ],
  ]);
  const request = { projectPath: "/one", provider: "codex", sessionId: "same" };
  const result = projectActivity(
    { "/one": workspace, "/one-worktree": workspace, "/other": workspace },
    (path) => path.startsWith("/one"),
    (key) => records.get(key),
    [request, request, { ...request, provider: "opencode" }],
  );
  assert.equal(result.working.length, 4);
  assert.equal(result.unread.length, 1);
  assert.equal(result.attention.length, 2);
});
