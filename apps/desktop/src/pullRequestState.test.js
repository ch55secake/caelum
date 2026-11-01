import assert from "node:assert/strict";
import test from "node:test";
import {
  checkSummary,
  checkStatus,
  canUpdateBranch,
  mergeStatus,
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

test("check rows distinguish running, queued and terminal states and only link web destinations", () => {
  const run = {
    __typename: "CheckRun",
    name: "CI",
    status: "IN_PROGRESS",
    conclusion: null,
    detailsUrl: "https://github.com/example/repo/actions/runs/1",
  };
  assert.equal(checkStatus(run).text, "In progress");
  assert.equal(checkStatus({ ...run, status: "QUEUED" }).text, "Queued");
  assert.equal(
    checkStatus({ ...run, status: "COMPLETED", conclusion: "TIMED_OUT" }).text,
    "Timed out",
  );
  assert.equal(checkStatus(run).url, run.detailsUrl);
  assert.equal(
    checkStatus({ ...run, detailsUrl: "javascript:alert(1)" }).url,
    null,
  );
  assert.equal(
    checkStatus({
      __typename: "StatusContext",
      context: "Deploy",
      state: "SUCCESS",
      targetUrl: "https://deploy.example.com/1",
    }).url,
    "https://deploy.example.com/1",
  );
});

test("merge availability follows GitHub rules and the displayed head, not optional check results", () => {
  const pr = {
    state: "OPEN",
    isDraft: false,
    mergeable: "MERGEABLE",
    mergeStateStatus: "CLEAN",
    headRefOid: "a".repeat(40),
    baseRefName: "main",
  };
  assert.equal(mergeStatus(pr).enabled, true);
  assert.equal(
    mergeStatus({ ...pr, mergeStateStatus: "UNSTABLE" }).enabled,
    true,
  );
  for (const change of [
    { state: "MERGED" },
    { state: "CLOSED" },
    { isDraft: true },
    { mergeable: "CONFLICTING" },
    { mergeable: "UNKNOWN" },
    { mergeStateStatus: "BLOCKED" },
    { mergeStateStatus: "BEHIND" },
    { mergeStateStatus: "UNKNOWN" },
    { mergeStateStatus: "FUTURE_STATE" },
    { reviewDecision: "CHANGES_REQUESTED" },
    { headRefOid: "" },
  ])
    assert.equal(
      mergeStatus({ ...pr, ...change }).enabled,
      false,
      JSON.stringify(change),
    );
  assert.equal(
    mergeStatus({ ...pr, state: "MERGED", isDraft: true }).text,
    "Merged into main",
  );
});

test("branch updates allow drafts and blocked reviews but require an open, conflict-free head", () => {
  const pr = {
    state: "OPEN",
    mergeable: "MERGEABLE",
    mergeStateStatus: "BEHIND",
    headRefOid: "a".repeat(40),
  };
  for (const change of [
    {},
    { isDraft: true },
    { mergeStateStatus: "BLOCKED", reviewDecision: "CHANGES_REQUESTED" },
    { mergeStateStatus: "CLEAN" },
  ])
    assert.equal(canUpdateBranch({ ...pr, ...change }), true);
  for (const change of [
    { state: "MERGED" },
    { state: "CLOSED" },
    { mergeable: "CONFLICTING" },
    { mergeable: "UNKNOWN" },
    { mergeStateStatus: "DIRTY" },
    { headRefOid: "" },
  ])
    assert.equal(
      canUpdateBranch({ ...pr, ...change }),
      false,
      JSON.stringify(change),
    );
});
