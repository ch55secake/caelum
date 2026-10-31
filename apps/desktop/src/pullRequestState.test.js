import assert from "node:assert/strict";
import test from "node:test";
import {
  checkSummary,
  pullRequestState,
  reviewStatus,
} from "./pullRequestState.js";

test("PR checks combine Actions and legacy statuses without calling pending or neutral checks passed", () => {
  const run = (conclusion, status = "COMPLETED") => ({
    __typename: "CheckRun",
    name: "CI",
    status,
    conclusion,
  });
  const result = checkSummary([
    run("SUCCESS"),
    run("NEUTRAL"),
    run("SKIPPED"),
    run(null, "IN_PROGRESS"),
    run("CANCELLED"),
    { __typename: "StatusContext", context: "Deploy", state: "ERROR" },
    { __typename: "StatusContext", context: "Build", state: "SUCCESS" },
    run("FUTURE_STATE"),
  ]);
  assert.deepEqual(result, {
    tone: "failure",
    text: "2 passed · 2 failed · 1 pending · 2 neutral / skipped · 1 unknown",
  });
  assert.deepEqual(checkSummary([run(null, "QUEUED")]), {
    tone: "pending",
    text: "1 pending",
  });
  assert.deepEqual(checkSummary([run(null)]), {
    tone: "unknown",
    text: "1 unknown",
  });
  assert.deepEqual(checkSummary(null), {
    tone: "neutral",
    text: "No checks reported",
  });
});

test("terminal PR state takes precedence over draft and absent review decisions stay explicit", () => {
  assert.equal(pullRequestState({ state: "MERGED", isDraft: true }), "Merged");
  assert.equal(pullRequestState({ state: "CLOSED", isDraft: true }), "Closed");
  assert.equal(pullRequestState({ state: "OPEN", isDraft: true }), "Draft");
  assert.equal(reviewStatus("CHANGES_REQUESTED"), "Changes requested");
  assert.equal(reviewStatus(null), "No review decision");
  assert.equal(reviewStatus(""), "No review decision");
});
