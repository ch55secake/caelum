import { test } from "node:test";
import assert from "node:assert/strict";
import {
  codexItem,
  openCodeMessage,
  applyConversationEvent,
  mergeHistory,
} from "./conversationState.js";
import {
  conversationPresentation,
  lastUserMessage,
  visibleMessageText,
} from "./shellMessages.js";
import { createSessionWorkspace } from "./workspaceState.js";

test("provider attachments survive history and live tool reconciliation", () => {
  const image = {
    type: "file",
    mime: "image/png",
    uri: "data:image/png;base64,aGVsbG8=",
    name: "Screenshot",
  };
  const history = openCodeMessage({
    id: "a",
    type: "assistant",
    content: [
      {
        type: "tool",
        id: "t",
        name: "screenshot",
        state: {
          status: "completed",
          content: [{ type: "text", text: "Captured" }, image],
        },
      },
    ],
  });
  let session = createSessionWorkspace({ thread: { id: "s" } });
  session = applyConversationEvent(
    session,
    "opencode",
    "session.tool.success",
    { id: "t", content: [{ type: "text", text: "Captured" }, image] },
  );
  const merged = mergeHistory(session.conversation, history);
  assert.equal(merged.length, 1);
  assert.equal(merged[0].output, "Captured");
  assert.deepEqual(merged[0].images, [{ src: image.uri, alt: "Screenshot" }]);
  assert.equal(
    openCodeMessage({
      id: "u",
      type: "user",
      files: [{ mime: "image/png", data: "aGVsbG8=" }],
    })[0].images.length,
    1,
  );
  assert.equal(
    codexItem({
      id: "u",
      type: "userMessage",
      content: [{ type: "image", url: image.uri }],
    }).images[0].src,
    image.uri,
  );
});

test("reminders disappear from presentation and recall while quoted examples and attachments survive", () => {
  const text =
    "<system-reminder>internal</system-reminder>\n\nDo the work\n<system-reminder>hidden\nmore\n</system-reminder>";
  const entries = [
    ...openCodeMessage({
      id: "s",
      type: "system",
      text: "internal instructions",
    }),
    ...openCodeMessage({ id: "u", type: "user", text }),
    ...openCodeMessage({
      id: "r",
      type: "synthetic",
      text: "<system-reminder>internal</system-reminder>",
    }),
  ];
  assert.equal(conversationPresentation(entries).length, 1);
  assert.equal(conversationPresentation(entries)[0].text, "Do the work");
  assert.equal(lastUserMessage(entries), "Do the work");
  assert.equal(entries[1].text, text);
  const example = "```xml\n<system-reminder>example</system-reminder>\n```";
  assert.equal(visibleMessageText(example), example);
  assert.equal(
    visibleMessageText("Useful\n<system-reminder>unfinished"),
    "Useful",
  );
  assert.equal(
    conversationPresentation([
      {
        id: "image",
        kind: "user",
        text: "",
        images: [{ src: "image.png", alt: "Image" }],
      },
    ]).length,
    1,
  );
});
