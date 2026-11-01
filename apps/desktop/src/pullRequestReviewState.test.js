import test from "node:test";
import assert from "node:assert/strict";
import {
  feedbackSelectedByDefault,
  reviewPrompt,
} from "./pullRequestReviewState.js";

const pr = {
  url: "https://github.com/upstream/repo/pull/44",
  title: "Navigation",
  headRefName: "feature/navigation",
  headRefOid: "a".repeat(40),
  baseRefName: "main",
};
const comment = {
  author: { login: "reviewer" },
  body: 'Please keep `空白` and "quotes".\n```suggestion\nreturn value;\n```',
  url: `${pr.url}#discussion_r1`,
  state: null,
  diffHunk: "@@ -2 +2 @@\n-old\n+new",
};
const thread = {
  id: "thread",
  kind: "thread",
  comments: [comment],
  thread: {
    path: "src/a.ts",
    line: 2,
    startLine: null,
    originalLine: 2,
    diffSide: "LEFT",
    isResolved: false,
    isOutdated: false,
    nextCursor: null,
  },
};

test("review selection distinguishes current unresolved feedback from resolved, outdated, approved and incomplete threads", () => {
  assert.equal(feedbackSelectedByDefault(thread), true);
  for (const patch of [
    { isResolved: true },
    { isOutdated: true },
    { nextCursor: "more" },
  ])
    assert.equal(
      feedbackSelectedByDefault({
        ...thread,
        thread: { ...thread.thread, ...patch },
      }),
      false,
    );
  for (const state of ["APPROVED", "DISMISSED", "PENDING"])
    assert.equal(
      feedbackSelectedByDefault({
        kind: "review",
        comments: [{ ...comment, state }],
      }),
      false,
    );
  assert.equal(
    feedbackSelectedByDefault({
      kind: "review",
      comments: [{ ...comment, state: "CHANGES_REQUESTED" }],
    }),
    true,
  );
});

test("review prompts preserve exact selected text, repository, commit and line context without silently truncating feedback", () => {
  const prompt = reviewPrompt(pr, [thread]);
  assert.ok(
    prompt.includes(pr.url) &&
      prompt.includes(pr.headRefOid) &&
      prompt.includes(pr.headRefName),
  );
  const context = JSON.parse(prompt.split("selected review feedback:\n")[1]);
  assert.equal(context[0].comments[0].body, comment.body);
  assert.equal(context[0].path, "src/a.ts");
  assert.equal(context[0].side, "LEFT");
  assert.equal(context[0].comments[0].diffHunk, comment.diffHunk);
  assert.throws(() => reviewPrompt(pr, []), /Select feedback/);
  assert.throws(
    () =>
      reviewPrompt(pr, [
        { ...thread, thread: { ...thread.thread, nextCursor: "more" } },
      ]),
    /remaining replies/,
  );
  assert.throws(
    () =>
      reviewPrompt(pr, [
        { ...thread, comments: [{ ...comment, body: "x".repeat(100_000) }] },
      ]),
    /Select fewer comments/,
  );
});
