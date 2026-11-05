import assert from "node:assert/strict";
import test from "node:test";
import {
  applyConversationEvent,
  historyEntries,
  mergeHistory,
} from "./conversationState.js";
import { shellEntry, recentShellOutput } from "./shellActivity.js";
import { conversationPresentation } from "./shellMessages.js";
import { createSessionWorkspace } from "../sessions/workspaceState.js";

test("native shells retain their identity across events, history and background notification envelopes", () => {
  let session = createSessionWorkspace({ thread: { id: "session" } });
  const shell = {
    id: "sh_1",
    command: "pnpm test",
    cwd: "/work/repo",
    status: "running",
    time: { started: 1000 },
  };
  session = applyConversationEvent(
    session,
    "opencode",
    "session.shell.started",
    { shell },
  );
  assert.equal(session.isWorking, false);
  session = applyConversationEvent(
    session,
    "opencode",
    "session.execution.succeeded",
    {},
  );
  assert.equal(session.conversation[0].status, "running");
  session = applyConversationEvent(session, "opencode", "session.shell.ended", {
    shell: {
      ...shell,
      status: "exited",
      exit: 2,
      time: { ...shell.time, completed: 4000 },
    },
    output: { output: "Failed assertion", truncated: true },
  });
  const entries = historyEntries("opencode", [
    {
      type: "shell",
      id: "msg_1",
      shellID: "sh_1",
      command: shell.command,
      status: "exited",
      exit: 2,
      time: { created: 1000, completed: 4000 },
      output: { output: "Failed assertion", truncated: true },
    },
  ]);
  const merged = mergeHistory(session.conversation, entries);
  assert.equal(merged.length, 1);
  assert.equal(merged[0].shell.cwd, "/work/repo");
  assert.equal(merged[0].shell.exit, 2);
  assert.equal(merged[0].status, "failed");
  assert.equal(merged[0].durationMs, 3000);
  assert.equal(merged[0].shell.outputTruncated, true);
  const displayed = conversationPresentation([
    ...merged,
    {
      id: "synthetic",
      kind: "status",
      text: '<shell id="sh_1" command="pnpm test" state="failed">Failed assertion</shell>',
    },
  ]);
  assert.equal(displayed.length, 1);
  assert.equal(displayed[0].shell.exit, 2);
  assert.equal(displayed[0].shell.background, true);
  const late = applyConversationEvent(
    session,
    "opencode",
    "session.shell.started",
    { shell },
  );
  assert.equal(late.conversation[0].status, "failed");
});

test("shell previews are bounded tails and status never invents exit codes", () => {
  const output = "old line\n".repeat(10000) + "latest line\nexit detail";
  const recent = recentShellOutput(output);
  assert.ok(recent.endsWith("latest line\nexit detail"));
  assert.ok(recent.length <= 4000);
  assert.ok(recent.split("\n").length <= 6);
  for (const status of ["timeout", "killed", "exited"]) {
    const entry = shellEntry({ id: "shell", status, command: "cmd" });
    assert.equal(entry.shell.exit, undefined);
    assert.equal(
      entry.status,
      status === "timeout"
        ? "failed"
        : status === "killed"
          ? "interrupted"
          : "completed",
    );
  }
});
