import assert from "node:assert/strict";
import test from "node:test";
import { createPublication, deferredOutput } from "./publication.js";

test("continuous output publishes the latest state without extending the deadline", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let current = "";
  const published = [];
  const publication = createPublication(() => published.push(current));
  for (const delta of ["first", " second", " third"]) {
    current += delta;
    publication.defer();
    t.mock.timers.tick(10);
  }
  assert.deepEqual(published, []);
  t.mock.timers.tick(2);
  assert.deepEqual(published, ["first second third"]);
});

test("urgent publication includes preceding output and cancels its stale timer", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let current = "streaming";
  const published = [];
  const publication = createPublication(() => published.push(current));
  publication.defer();
  current = "completed";
  publication.flush();
  current = "new session";
  t.mock.timers.tick(1000);
  assert.deepEqual(published, ["completed"]);
  publication.defer();
  publication.cancel();
  t.mock.timers.tick(1000);
  assert.deepEqual(published, ["completed"]);
});

test("a stalled timer cannot defer a burst indefinitely", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let current = 0;
  const published = [];
  const publication = createPublication(() => published.push(current));
  for (current = 1; current <= 1024; current++) publication.defer();
  assert.deepEqual(published, [128, 256, 384, 512, 640, 768, 896, 1024]);
  t.mock.timers.tick(1000);
  assert.equal(published.length, 8);
});

test("only incremental output is deferred, including when methods are unfamiliar", () => {
  for (const [provider, methods] of [
    [
      "codex",
      [
        "turn/completed",
        "item/started",
        "item/completed",
        "item/tool/requestUserInput",
        "codex/processExited",
        "error",
      ],
    ],
    [
      "opencode",
      [
        "session.execution.succeeded",
        "session.execution.failed",
        "session.execution.interrupted",
        "session.tool.failed",
        "permission.asked",
        "form.created",
        "server.disconnected",
      ],
    ],
  ]) {
    for (const method of [...methods, "unknown/delta"])
      assert.equal(deferredOutput(provider, method), false, method);
  }
  assert.equal(deferredOutput("codex", "item/agentMessage/delta"), true);
  assert.equal(deferredOutput("opencode", "session.text.delta"), true);
});
