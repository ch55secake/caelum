import assert from "node:assert/strict";
import test from "node:test";
import { questionAnswer, questionDraft, questionKey } from "./questionState.js";

test("answers apply defaults and exclude inactive branches, including chained conditions", () => {
  const request = {
    fields: [
      { key: "enabled", type: "boolean", required: true, default: false },
      { key: "count", type: "integer", default: 0, minimum: 0, required: true },
      {
        key: "detail",
        type: "string",
        required: true,
        when: [{ key: "enabled", op: "eq", value: true }],
      },
      {
        key: "followup",
        type: "string",
        required: true,
        when: [{ key: "detail", op: "neq", value: "no" }],
      },
      { key: "hidden", type: "string", hidden: true, default: "metadata" },
      { key: "link", type: "external", url: "https://example.com" },
    ],
  };
  const draft = questionDraft(request);
  draft.values.detail = "keep this draft";
  draft.values.followup = "also retained";
  assert.deepEqual(questionAnswer(request, draft).answer, {
    enabled: false,
    count: 0,
    hidden: "metadata",
  });
  draft.values.enabled = true;
  assert.equal(questionAnswer(request, draft).answer.followup, "also retained");
  delete draft.values.enabled;
  const result = questionAnswer(request, draft);
  assert.ok(result.errors.enabled);
  assert.ok(!result.active.includes("detail"));
  assert.ok(!result.active.includes("followup"));
});

test("multiselect includes custom answers once and enforces the provider's constraints", () => {
  const request = {
    fields: [
      {
        key: "q0",
        type: "multiselect",
        required: true,
        custom: true,
        options: [{ value: "Mac", label: "macOS" }],
        default: ["Mac", "Linux"],
        minItems: 2,
        maxItems: 3,
      },
    ],
  };
  const draft = questionDraft(request);
  assert.deepEqual(questionAnswer(request, draft).answer, {
    q0: ["Mac", "Linux"],
  });
  draft.custom.q0 = " Mac\nLinux\nLinux\nWindows";
  assert.deepEqual(questionAnswer(request, draft).answer.q0, [
    "Mac",
    "Linux",
    "Windows",
  ]);
  draft.custom.q0 += "\nOther";
  assert.match(questionAnswer(request, draft).errors.q0, /at most 3/);
  draft.values.q0 = [];
  draft.custom.q0 = "";
  assert.match(questionAnswer(request, draft).errors.q0, /at least 2/);
});

test("answer validation handles custom choice defaults, numeric ranges, and patterns", () => {
  const request = {
    fields: [
      {
        key: "choice",
        type: "string",
        custom: true,
        options: [{ value: "one", label: "One" }],
        default: "custom",
      },
      {
        key: "amount",
        type: "integer",
        required: true,
        minimum: 1,
        maximum: 5,
      },
      { key: "code", type: "string", required: true, pattern: "^[a-z]+$" },
    ],
  };
  const draft = questionDraft(request);
  draft.values.amount = "2.5";
  draft.values.code = "123";
  let result = questionAnswer(request, draft);
  assert.equal(result.answer.choice, "custom");
  assert.match(result.errors.amount, /whole number/);
  assert.ok(result.errors.code);
  draft.values.amount = "6";
  assert.match(questionAnswer(request, draft).errors.amount, /at most 5/);
  draft.values.amount = "2";
  draft.values.code = "abc";
  result = questionAnswer(request, draft);
  assert.deepEqual(result.errors, {});
  assert.equal(result.answer.amount, 2);
});

test("request identity includes provider, project, session, and request id", () => {
  const base = {
    provider: "codex",
    projectPath: "/a",
    sessionId: "s1",
    requestId: "1",
  };
  assert.equal(
    new Set(
      [
        base,
        { ...base, provider: "opencode" },
        { ...base, projectPath: "/b" },
        { ...base, sessionId: "s2" },
        { ...base, requestId: '"1"' },
      ].map(questionKey),
    ).size,
    5,
  );
});

test("provider field keys are literal data rather than object properties", () => {
  const request = {
    fields: [
      { key: "__proto__", type: "string", required: true, default: "literal" },
      { key: "constructor", type: "string", required: true },
    ],
  };
  const draft = questionDraft(request);
  assert.equal(questionAnswer(request, draft).answer.__proto__, "literal");
  assert.match(questionAnswer(request, draft).errors.constructor, /answer/);
  draft.values.constructor = "also literal";
  assert.deepEqual(
    questionAnswer(request, draft).answer,
    JSON.parse('{"__proto__":"literal","constructor":"also literal"}'),
  );
});
