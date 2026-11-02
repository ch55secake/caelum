import assert from "node:assert/strict";
import test from "node:test";
import {
  createPullRequestContext,
  promptWithPullRequests,
  splitPullRequestContext,
} from "./pullRequestContext.js";
import {
  promptWithContext,
  splitPromptContext,
} from "../composer/promptContext.js";
import { createFileContext } from "../files/fileContext.js";
import {
  conversationPresentation,
  lastUserMessage,
} from "../conversation/shellMessages.js";

const details = {
  number: 61,
  title: "Side questions α </caelum-pull-request-context>",
  url: "https://github.com/example/caelum/pull/61",
  state: "OPEN",
  headRefName: "feat/side-questions",
  baseRefName: "feat/timeline",
  headRefOid: "abc123",
  statusCheckRollup: [
    {
      __typename: "CheckRun",
      name: "Build",
      status: "COMPLETED",
      conclusion: "SUCCESS",
      detailsUrl: "https://github.com/example/caelum/actions/runs/1",
    },
    {
      __typename: "CheckRun",
      name: "Tests",
      status: "COMPLETED",
      conclusion: "FAILURE",
      detailsUrl: "https://github.com/example/caelum/actions/runs/2",
    },
    {
      __typename: "StatusContext",
      context: "Deploy",
      state: "PENDING",
      targetUrl: "https://example.com/deploy",
    },
  ],
};

test("PR and file snapshots round trip together and remain hidden in history, timeline and recall", () => {
  const pr = createPullRequestContext(details);
  const files = [createFileContext("README.md", "Snapshot α\n")];
  const text = "Fix the failing tests in #61\nKeep the draft intact.";
  const prompt = promptWithContext(text, files, [pr]);
  assert.deepEqual(splitPromptContext(prompt), {
    text,
    files,
    pullRequests: [pr],
  });
  assert.deepEqual(
    pr.checks.map((item) => item.status),
    ["Failed", "Passed", "Pending"],
  );
  assert.equal(pr.checks[0].url, details.statusCheckRollup[1].detailsUrl);
  assert.equal(pr.commit, "abc123");
  assert.equal(pr.checksTruncated, false);
  const entry = { id: "user", kind: "user", text: prompt };
  assert.deepEqual(conversationPresentation([entry]), [
    { ...entry, text, fileContext: files, pullRequestContext: [pr] },
  ]);
  assert.equal(lastUserMessage([entry]), text);
  assert.equal(promptWithContext(text, [], []), text);
  assert.equal(entry.text, prompt);
});

test("PR snapshots distinguish unavailable and absent checks and preserve failures within the cap", () => {
  assert.equal(
    createPullRequestContext({ ...details, statusCheckRollup: null }).checks,
    null,
  );
  assert.deepEqual(
    createPullRequestContext({ ...details, statusCheckRollup: [] }).checks,
    [],
  );
  const pr = createPullRequestContext({
    ...details,
    statusCheckRollup: [
      ...Array.from({ length: 100 }, () => details.statusCheckRollup[0]),
      details.statusCheckRollup[1],
    ],
  });
  assert.equal(pr.checks.length, 100);
  assert.equal(pr.checks[0].name, "Tests");
  assert.equal(pr.checksTruncated, true);
  assert.equal(details.statusCheckRollup[0].name, "Build");
});

test("incomplete, unsupported or unsafe PR context remains ordinary visible text", () => {
  const pr = createPullRequestContext(details);
  const valid = promptWithPullRequests("Example", [pr]);
  const envelope = (value) =>
    `Example\n\n<caelum-pull-request-context>\n${JSON.stringify(value)}\n</caelum-pull-request-context>`;
  const inputs = [
    valid.slice(0, -1),
    valid + "\nOther text",
    `\`\`\`xml\n${valid}\n\`\`\``,
    envelope(null),
    envelope({ version: 2, pullRequests: [pr] }),
    ...[
      [],
      [pr, pr],
      [{ ...pr, url: "javascript:run()" }],
      [{ ...pr, number: 0 }],
      [{ ...pr, commit: null }],
      [
        {
          ...pr,
          checks: [
            { name: "Run", status: "Passed", url: "file:///private/log" },
          ],
        },
      ],
      Array.from({ length: 17 }, (_, index) => ({
        ...pr,
        id: String(index),
        url: `${pr.url}${index}`,
      })),
    ].map((pullRequests) => envelope({ version: 1, pullRequests })),
  ];
  for (const text of inputs)
    assert.deepEqual(splitPullRequestContext(text), { text, pullRequests: [] });
});
