import assert from "node:assert/strict";
import test from "node:test";

import {
  applyCodexEvent,
  applyOpenCodeEvent,
  createProjectWorkspace,
  createSessionWorkspace,
  projectHasActiveTurn,
  restoreProjectWorkspace,
  updateProviderWorkspace,
  updateProjectWorkspace,
  updateSessionWorkspace,
} from "./workspaceState.js";

const projectA = { name: "alpha", path: "/work/alpha" };
const projectB = { name: "beta", path: "/work/beta" };

function sessionState(project, provider, id) {
  return createSessionWorkspace({
    project,
    provider,
    thread: { id, name: id, preview: "", updatedAt: null },
  });
}

function withSession(
  workspaces,
  project,
  provider,
  id,
  state = sessionState(project, provider, id),
) {
  return updateProviderWorkspace(
    workspaces,
    project.path,
    provider,
    (current) => ({
      ...current,
      sessionsById: { ...current.sessionsById, [id]: state },
    }),
  );
}

test("project provider, session drafts, and new-session drafts stay independent", () => {
  let workspaces = {};
  workspaces = updateProjectWorkspace(workspaces, projectA.path, (current) => ({
    ...current,
    provider: "opencode",
  }));
  workspaces = updateProviderWorkspace(
    workspaces,
    projectA.path,
    "opencode",
    (current) => ({
      ...current,
      selectedSessionId: "ses-alpha",
      draft: "new draft",
    }),
  );

  assert.equal(workspaces[projectA.path].provider, "opencode");
  workspaces = withSession(workspaces, projectA, "opencode", "ses-alpha");
  workspaces = withSession(workspaces, projectA, "opencode", "ses-beta");
  workspaces = updateSessionWorkspace(
    workspaces,
    projectA.path,
    "opencode",
    "ses-alpha",
    (current) => ({ ...current, draft: "alpha draft" }),
  );
  assert.equal(
    workspaces[projectA.path].providers.opencode.sessionsById["ses-alpha"]
      .draft,
    "alpha draft",
  );
  assert.equal(
    workspaces[projectA.path].providers.opencode.sessionsById["ses-beta"].draft,
    "",
  );
  assert.equal(workspaces[projectA.path].providers.opencode.draft, "new draft");
  assert.equal(
    workspaces[projectA.path].providers.opencode.selectedSessionId,
    "ses-alpha",
  );
  assert.equal(workspaces[projectB.path], undefined);
  assert.equal(createProjectWorkspace().provider, "codex");
});

test("restores provider and selected session IDs without copying conversation history", () => {
  const workspace = restoreProjectWorkspace({
    provider: "opencode",
    codexSessionId: "thread-1",
    opencodeSessionId: "session-1",
    codexSessionIds: ["thread-1", "thread-older"],
    opencodeSessionIds: ["session-1"],
  });

  assert.equal(workspace.provider, "opencode");
  assert.equal(workspace.providers.codex.selectedSessionId, "thread-1");
  assert.equal(workspace.providers.opencode.selectedSessionId, "session-1");
  assert.deepEqual(
    workspace.providers.codex.sessions.map((session) => session.id),
    ["thread-1", "thread-older"],
  );
  assert.deepEqual(workspace.providers.opencode.sessionsById, {});
});

test("Codex events update only their originating project and session, even when unselected", () => {
  let workspaces = withSession({}, projectA, "codex", "thread-a");
  workspaces = withSession(
    workspaces,
    projectA,
    "codex",
    "thread-a-background",
  );
  workspaces = withSession(workspaces, projectB, "codex", "thread-b");
  workspaces = updateProviderWorkspace(
    workspaces,
    projectA.path,
    "codex",
    (current) => ({
      ...current,
      selectedSessionId: "thread-a-background",
    }),
  );

  const updated = applyCodexEvent(workspaces, {
    projectPath: projectA.path,
    method: "item/agentMessage/delta",
    params: { threadId: "thread-a", itemId: "item-1", delta: "hello" },
  });

  assert.deepEqual(
    updated[projectA.path].providers.codex.sessionsById["thread-a"]
      .conversation,
    [
      {
        id: "codex-item-1",
        kind: "assistant",
        text: "hello",
        streamText: "hello",
      },
    ],
  );
  assert.deepEqual(
    updated[projectA.path].providers.codex.sessionsById["thread-a-background"]
      .conversation,
    [],
  );
  assert.deepEqual(
    updated[projectB.path].providers.codex.sessionsById["thread-b"]
      .conversation,
    [],
  );
});

test("OpenCode events update the matching project and session only", () => {
  let workspaces = withSession({}, projectA, "opencode", "ses-a");
  workspaces = withSession(workspaces, projectA, "opencode", "ses-a-other");
  workspaces = withSession(workspaces, projectB, "opencode", "ses-b");

  const updated = applyOpenCodeEvent(workspaces, {
    projectPath: projectB.path,
    eventType: "session.text.delta",
    data: {
      sessionID: "ses-b",
      assistantMessageID: "message-b",
      delta: "response",
    },
  });

  assert.deepEqual(
    updated[projectA.path].providers.opencode.sessionsById["ses-a"]
      .conversation,
    [],
  );
  assert.deepEqual(
    updated[projectA.path].providers.opencode.sessionsById["ses-a-other"]
      .conversation,
    [],
  );
  assert.deepEqual(
    updated[projectB.path].providers.opencode.sessionsById["ses-b"]
      .conversation,
    [
      {
        id: "opencode-message-b",
        kind: "assistant",
        text: "response",
        streamText: "response",
      },
    ],
  );
});

test("concurrent turns remain independent and cancellation status is scoped", () => {
  let workspaces = withSession({}, projectA, "codex", "thread-a", {
    ...sessionState(projectA, "codex", "thread-a"),
    isWorking: true,
  });
  workspaces = withSession(workspaces, projectB, "opencode", "ses-b", {
    ...sessionState(projectB, "opencode", "ses-b"),
    isWorking: true,
  });

  assert.equal(projectHasActiveTurn(workspaces[projectA.path]), true);
  assert.equal(projectHasActiveTurn(workspaces[projectB.path]), true);

  const updated = applyCodexEvent(workspaces, {
    projectPath: projectA.path,
    method: "turn/completed",
    params: {
      threadId: "thread-a",
      turn: { id: "turn-a", status: "interrupted" },
    },
  });

  assert.equal(
    updated[projectA.path].providers.codex.sessionsById["thread-a"].isWorking,
    false,
  );
  assert.equal(
    updated[projectB.path].providers.opencode.sessionsById["ses-b"].isWorking,
    true,
  );
  assert.equal(projectHasActiveTurn(updated[projectA.path]), false);
  assert.equal(projectHasActiveTurn(updated[projectB.path]), true);
  assert.equal(
    updated[projectA.path].providers.codex.sessionsById["thread-a"]
      .conversation[0].text,
    "Turn cancelled",
  );
});
