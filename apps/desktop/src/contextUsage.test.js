import assert from "node:assert/strict";
import test from "node:test";
import {
  codexContextUsage,
  openCodeContextUsage,
  openCodeHistoryUsage,
} from "./contextUsage.js";
import {
  applyCodexEvent,
  applyOpenCodeEvent,
  createSessionWorkspace,
  updateProviderWorkspace,
} from "./workspaceState.js";

const tokens = {
  input: 12000,
  output: 2000,
  reasoning: 500,
  cache: { read: 5000, write: 1000 },
};
const model = { providerID: "gateway", id: "vendor/model" };
const assistant = (id, created, usage = tokens) => ({
  id,
  type: "assistant",
  time: { created },
  model,
  tokens: usage,
});

test("context usage uses the latest request, includes cache once and never invents missing limits", () => {
  assert.deepEqual(
    codexContextUsage({
      total: { totalTokens: 900000 },
      last: {
        totalTokens: 20000,
        inputTokens: 18000,
        cachedInputTokens: 5000,
        outputTokens: 2000,
        reasoningOutputTokens: 500,
      },
      modelContextWindow: 200000,
    }),
    {
      usedTokens: 20000,
      contextWindow: 200000,
      model: null,
      timestamp: null,
    },
  );
  assert.deepEqual(openCodeContextUsage(tokens, "gateway/vendor/model"), {
    usedTokens: 20000,
    contextWindow: null,
    model: "gateway/vendor/model",
    timestamp: null,
  });
  for (const invalid of [undefined, null, -1, NaN, Infinity, "100", 0.5]) {
    assert.equal(codexContextUsage({ last: { totalTokens: invalid } }), null);
    assert.equal(
      openCodeContextUsage({ ...tokens, input: invalid }, null),
      null,
    );
  }
  for (const limit of [undefined, null, 0, -1, NaN, Infinity, "200000"]) {
    assert.equal(
      codexContextUsage({
        last: { totalTokens: 0 },
        modelContextWindow: limit,
      }).contextWindow,
      null,
    );
  }
  assert.equal(
    openCodeContextUsage(
      { input: 0, output: 0, cache: { read: 0, write: 0 } },
      null,
    ).usedTokens,
    0,
  );
});

test("history recovers the latest measured context and rejects pre-compaction and stale snapshots", () => {
  const old = assistant("old", 100, { ...tokens, input: 80000 });
  const latest = assistant("latest", 200);
  const compact = {
    id: "compact",
    type: "compaction",
    status: "completed",
    time: { created: 300 },
    tokens: { ...tokens, input: 100000 },
  };
  assert.equal(
    openCodeHistoryUsage([old, latest, { type: "idle" }]).usedTokens,
    20000,
  );
  assert.equal(openCodeHistoryUsage([old, compact]), null);
  assert.equal(
    openCodeHistoryUsage([old, { ...compact, status: "failed" }]).usedTokens,
    88000,
  );
  assert.equal(
    openCodeHistoryUsage([old, compact, assistant("after", 400)]).timestamp,
    400,
  );
  assert.equal(openCodeHistoryUsage([old, latest], 300), undefined);
  assert.equal(openCodeHistoryUsage([compact], 400), undefined);
  assert.equal(openCodeHistoryUsage([]), undefined);
});

test("usage stays project/session scoped and follows actual model steps rather than pending selections or lifetime totals", () => {
  let state = {};
  for (const path of ["/work/one", "/work/two"])
    for (const provider of ["codex", "opencode"])
      state = updateProviderWorkspace(state, path, provider, (value) => ({
        ...value,
        sessionsById: Object.fromEntries(
          ["a", "b"].map((id) => [
            id,
            {
              ...createSessionWorkspace({
                project: { path, name: path },
                provider,
                thread: { id },
              }),
              model: "different/pending",
            },
          ]),
        ),
      }));
  const get = (provider, path = "/work/one", id = "a") =>
    state[path].providers[provider].sessionsById[id];
  state = applyCodexEvent(state, {
    projectPath: "/work/one",
    method: "thread/tokenUsage/updated",
    params: {
      threadId: "a",
      tokenUsage: {
        total: { totalTokens: 500000 },
        last: { totalTokens: 20000 },
        modelContextWindow: 200000,
      },
    },
  });
  assert.equal(get("codex").contextUsage.usedTokens, 20000);
  assert.equal(get("codex", "/work/two").contextUsage, null);
  assert.equal(get("codex", "/work/one", "b").contextUsage, null);
  assert.equal(get("opencode").contextUsage, null);
  const emit = (eventType, data, created = 200) => {
    state = applyOpenCodeEvent(state, {
      projectPath: "/work/one",
      eventType,
      data: { sessionID: "a", ...data },
      id: `evt_${created}`,
      created,
    });
  };
  emit("session.step.started", {
    assistantMessageID: "step",
    model,
    started: 100,
  });
  emit("session.step.ended", { assistantMessageID: "step", tokens });
  assert.equal(get("opencode").contextUsage.model, "gateway/vendor/model");
  assert.equal(get("opencode").contextUsage.usedTokens, 20000);
  emit("session.usage.updated", { tokens: { ...tokens, input: 900000 } });
  assert.equal(get("opencode").contextUsage.usedTokens, 20000);
  assert.equal(get("opencode", "/work/two").contextUsage, null);
  assert.equal(get("opencode", "/work/one", "b").contextUsage, null);
  emit("session.compaction.started", { reason: "auto" }, 300);
  emit("session.compaction.ended", { text: "summary", tokens }, 400);
  assert.equal(get("opencode").contextUsage, null);
  emit("session.step.ended", { assistantMessageID: "step", tokens }, 200);
  assert.equal(get("opencode").contextUsage, null);
  emit(
    "session.step.started",
    { assistantMessageID: "after", model, started: 500 },
    500,
  );
  emit(
    "session.step.ended",
    { assistantMessageID: "after", tokens: { ...tokens, input: 1000 } },
    600,
  );
  assert.equal(get("opencode").contextUsage.usedTokens, 9000);
  // An end event can arrive after reconnect without its compaction start.
  emit("session.compaction.ended", { text: "Another summary", tokens }, 900);
  emit("session.step.ended", { assistantMessageID: "after", tokens }, 600);
  assert.equal(get("opencode").contextUsage, null);
});
