import { test } from "node:test";
import assert from "node:assert/strict";
import { inboxMessages, reconcileInbox, queueEvent } from "./messageQueueState.js";
import { applyConversationEvent } from "../conversation/conversationState.js";
import { createSessionWorkspace } from "../sessions/workspaceState.js";

const item = (id, text = "Same prompt") => ({ id, type: "user", delivery: "queue", payload: { text } });
test("inbox events reconcile identical prompts by ID and ignore late enqueues after delivery", () => {
  let session = createSessionWorkspace({ thread: { id: "s" } });
  for (const id of ["msg_1", "msg_2"])
    session = queueEvent(session, "session.inbox.enqueued", { inboxID: id, item: item(id) });
  assert.equal(session.queuedMessages.length, 2);
  session = applyConversationEvent(session, "opencode", "session.inbox.delivered", { inboxID: "msg_1" });
  assert.equal(session.conversation[0].id, "opencode-msg_1");
  assert.equal(session.isWorking, true);
  session = queueEvent(session, "session.inbox.enqueued", { inboxID: "msg_1", item: item("msg_1") });
  assert.deepEqual(session.queuedMessages.map((entry) => entry.id), ["msg_2"]);
  session = queueEvent(session, "session.inbox.cancelled", { inboxID: "msg_2" });
  assert.equal(session.queuedMessages.length, 0);
});

test("snapshots preserve unacknowledged and local sends but remove consumed remote inputs", () => {
  const [pending, consumed, local] = inboxMessages([item("msg_1"), item("msg_2"), item("msg_3")]);
  pending.pending = true;
  local.remote = false;
  assert.deepEqual(reconcileInbox([pending, consumed, local], []).map((entry) => entry.id), ["msg_1", "msg_3"]);
  assert.equal(inboxMessages([{ type: "synthetic", id: "msg_s", payload: { text: "internal" } }]).length, 0);
});

test("tool and model-step completion do not drain the queue; explicit stop pauses it", () => {
  let session = { ...createSessionWorkspace({ thread: { id: "s" } }), isWorking: true, queuedMessages: inboxMessages([item("msg_1")]) };
  session = applyConversationEvent(session, "opencode", "session.step.ended", { assistantMessageID: "msg_a" });
  assert.equal(session.isWorking, true);
  assert.equal(session.queuedMessages.length, 1);
  session = applyConversationEvent(session, "opencode", "session.execution.interrupted", {});
  assert.equal(session.queuePaused, true);
  assert.equal(session.queuedMessages.length, 1);
});
