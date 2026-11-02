import { expect, test } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import { emit, choose, setHistory } from "./fixtures/helpers.js";

for (const provider of ["codex", "opencode"]) {
  const title =
    provider === "codex" ? "Refine the session workspace" : "OpenCode session";
  const id = provider === "codex" ? "thread-a" : "ses-a";
  test(`${provider} clears the cancellation notice on a new prompt, history refresh and reconnect`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const activity = page.getByRole("form", { name: "Message composer" });
    const cancelled = page.getByText("Turn cancelled", { exact: true });
    for (const resume of ["prompt", "snapshot", "reconnect"]) {
      const history =
        provider === "codex"
          ? [{ id: `cancelled-${resume}`, status: "interrupted", items: [] }]
          : [
              {
                id: `cancelled-${resume}`,
                type: "idle",
                outcome: "interrupted",
              },
            ];
      await setHistory(page, provider, history, true);
      await emit(
        page,
        provider === "codex" ? "turn/started" : "session.execution.started",
        provider === "codex"
          ? { threadId: id, turn: { id: `turn-${resume}` } }
          : { sessionID: id },
        provider,
      );
      await page.getByRole("button", { name: "Stop generation" }).click();
      await setHistory(page, provider, history, false);
      await expect(cancelled).toHaveCount(1);
      await expect(activity).toHaveAttribute("data-working", "false");
      await page.evaluate(() => window.dispatchEvent(new Event("focus")));
      await expect(cancelled).toHaveCount(1);

      const response =
        provider === "codex"
          ? {
              id: `resumed-${resume}`,
              status: "inProgress",
              items: [
                {
                  id: `answer-${resume}`,
                  type: "agentMessage",
                  text: "Resumed work",
                },
              ],
            }
          : {
              id: `resumed-${resume}`,
              type: "assistant",
              content: [{ type: "text", text: "Resumed work" }],
            };
      await setHistory(page, provider, [...history, response], true);
      if (resume === "prompt") {
        const input = page.getByRole("textbox", {
          name: "Message",
          exact: true,
        });
        await input.fill("Continue the cancelled task");
        await input.press("Enter");
      } else if (resume === "reconnect") {
        await emit(
          page,
          provider === "codex" ? "codex/processExited" : "server.disconnected",
          {},
          provider,
        );
        await page
          .getByRole("button", { name: "Reconnect", exact: true })
          .click();
      } else
        await page.evaluate(() => window.dispatchEvent(new Event("focus")));
      await expect(activity).toHaveAttribute("data-working", "true");
      await expect(cancelled).toHaveCount(0);
      const finished =
        provider === "codex"
          ? [...history, { ...response, status: "completed" }]
          : [
              ...history,
              { ...response, time: { completed: 200 } },
              { id: `done-${resume}`, type: "idle", outcome: "succeeded" },
            ];
      await setHistory(page, provider, finished, false);
      await page.evaluate(() => window.dispatchEvent(new Event("focus")));
      await expect(activity).toHaveAttribute("data-working", "false");
      await expect(cancelled).toHaveCount(0);
      await page
        .getByRole("button", {
          name: "Review the provider adapters",
          exact: true,
        })
        .click();
      await page.getByRole("button", { name: title, exact: true }).click();
      await expect(cancelled).toHaveCount(0);
    }
  });
}

for (const provider of ["codex", "opencode"]) {
  const title =
    provider === "codex" ? "Refine the session workspace" : "OpenCode session";
  const id = provider === "codex" ? "thread-a" : "ses-a";
  test(`${provider} restores compaction checkpoints and preserves drafts and reading position while work continues`, async ({
    page,
  }) => {
    await desktop(page, null);
    const summary =
      "## Retained context\n\nKeep the **release plan** and next steps.";
    const earlier =
      provider === "codex"
        ? [
            {
              id: "prior-turn",
              status: "completed",
              items: [
                {
                  id: "prior-user",
                  type: "userMessage",
                  content: [{ type: "text", text: "Keep the release plan" }],
                },
                ...Array.from({ length: 30 }, (_, index) => ({
                  id: `prior-${index}`,
                  type: "agentMessage",
                  text: `Earlier response ${index}`,
                })),
                {
                  id: "restored",
                  type: "contextCompaction",
                  startedAtMs: 100,
                  completedAtMs: 200,
                },
              ],
            },
          ]
        : [
            {
              id: "prior-user",
              type: "user",
              text: "Keep the release plan",
            },
            ...Array.from({ length: 30 }, (_, index) => ({
              id: `prior-${index}`,
              type: "assistant",
              time: { completed: 100 },
              content: [{ type: "text", text: `Earlier response ${index}` }],
            })),
            {
              id: "msg_restored",
              type: "compaction",
              status: "completed",
              reason: "auto",
              summary,
              time: { created: 100 },
            },
          ];
    await setHistory(page, provider, earlier, false);
    if (provider === "opencode") await choose(page, "Provider", "OpenCode");
    await page.getByRole("button", { name: title, exact: true }).click();
    const notices = page.getByRole("region", {
      name: "Context compaction",
    });
    const activity = page.getByRole("form", { name: "Message composer" });
    const input = page.getByRole("textbox", {
      name: "Message",
      exact: true,
    });
    const transcript = page.locator(".transcript");
    await expect(notices).toHaveCount(1);
    await expect(notices).toHaveAttribute("data-status", "completed");
    await input.press("ArrowUp");
    await expect(input).toHaveValue("Keep the release plan");
    await input.fill("Draft for after compaction");

    const running =
      provider === "codex"
        ? {
            id: "live-turn",
            status: "inProgress",
            items: [
              { id: "live", type: "contextCompaction", startedAtMs: 300 },
            ],
          }
        : {
            id: "msg_live",
            type: "compaction",
            status: "running",
            reason: "auto",
            summary: "",
            time: { created: 300 },
          };
    await setHistory(page, provider, [...earlier, running], true);
    await emit(
      page,
      provider === "codex" ? "item/started" : "session.compaction.started",
      provider === "codex"
        ? {
            threadId: id,
            item: {
              id: "live",
              type: "contextCompaction",
              startedAtMs: 300,
            },
          }
        : { sessionID: id, reason: "auto" },
      provider,
      { id: "evt_live", created: 300 },
    );
    await expect(notices).toHaveCount(2);
    await expect(notices.last()).toHaveAttribute("data-status", "running");
    await expect(
      page.getByRole("status", { name: "Agent activity" }),
    ).toHaveText("Compacting context…");
    await expect(activity).toHaveAttribute("data-working", "true");
    await transcript.evaluate((node) => {
      node.scrollTop = 200;
    });
    await expect(
      page.getByRole("button", { name: "Jump to latest" }),
    ).toBeVisible();
    await page
      .getByRole("button", {
        name: "Review the provider adapters",
        exact: true,
      })
      .click();
    await expect(input).toHaveValue("");

    const completed =
      provider === "codex"
        ? {
            id: "live-turn",
            status: "inProgress",
            items: [
              {
                id: "live",
                type: "contextCompaction",
                startedAtMs: 300,
                completedAtMs: 400,
              },
            ],
          }
        : { ...running, status: "completed", summary };
    await setHistory(page, provider, [...earlier, completed], true);
    await emit(
      page,
      provider === "codex" ? "item/completed" : "session.compaction.ended",
      provider === "codex"
        ? {
            threadId: id,
            item: {
              id: "live",
              type: "contextCompaction",
              completedAtMs: 400,
            },
          }
        : { sessionID: id, reason: "auto", text: summary },
      provider,
      { id: "evt_end", created: 400 },
    );
    await expect(
      page.getByRole("region", { name: "Context compaction" }),
    ).toHaveCount(0);
    await expect(activity).toHaveAttribute("data-working", "false");
    await page.getByRole("button", { name: title, exact: true }).click();
    await expect(notices).toHaveCount(2);
    await expect(notices.last()).toHaveAttribute("data-status", "completed");
    await expect(activity).toHaveAttribute("data-working", "true");
    await expect(input).toHaveValue("Draft for after compaction");
    await expect
      .poll(() => transcript.evaluate((node) => node.scrollTop))
      .toBe(200);
    await emit(
      page,
      provider === "codex" ? "item/agentMessage/delta" : "session.text.delta",
      provider === "codex"
        ? {
            threadId: id,
            itemId: "after",
            delta: "Continuing after compaction",
          }
        : {
            sessionID: id,
            assistantMessageID: "after",
            ordinal: 0,
            delta: "Continuing after compaction",
          },
      provider,
    );
    await expect
      .poll(() => transcript.evaluate((node) => node.scrollTop))
      .toBe(200);
    await page.getByRole("button", { name: "Jump to latest" }).click();
    await expect(
      page.getByText("Continuing after compaction", { exact: true }),
    ).toBeInViewport();
    if (provider === "opencode") {
      await notices
        .last()
        .getByRole("button", { name: "Context compacted" })
        .click();
      await expect(
        notices.last().getByRole("heading", { name: "Retained context" }),
      ).toBeVisible();
      await expect(notices.last().locator("strong")).toHaveText("release plan");
    } else await expect(notices.last().getByRole("button")).toHaveCount(0);
    await page.screenshot({
      path: `test-results/${provider}-compaction.png`,
    });
    await setHistory(
      page,
      provider,
      [
        ...earlier,
        {
          ...completed,
          ...(provider === "codex" ? { status: "completed" } : {}),
        },
      ],
      false,
    );
    await emit(
      page,
      provider === "codex" ? "turn/completed" : "session.execution.succeeded",
      provider === "codex"
        ? { threadId: id, turn: { id: "live-turn", status: "completed" } }
        : { sessionID: id },
      provider,
    );
    await expect(activity).toHaveAttribute("data-working", "false");
    await expect(notices).toHaveCount(2);
    await expect(
      page.getByText("Earlier response 0", { exact: true }),
    ).toHaveCount(1);
    await expect(input).toHaveValue("Draft for after compaction");
  });
}

for (const provider of ["codex", "opencode"]) {
  const title =
    provider === "codex" ? "Refine the session workspace" : "OpenCode session";
  const id = provider === "codex" ? "thread-a" : "ses-a";
  test(`${provider} shows failed and interrupted compaction and recovers a disconnected checkpoint`, async ({
    page,
  }) => {
    await desktop(page, provider === "codex" ? id : null);
    if (provider === "opencode") {
      await choose(page, "Provider", "OpenCode");
      await page.getByRole("button", { name: title, exact: true }).click();
    }
    const notices = page.getByRole("region", {
      name: "Context compaction",
    });
    const activity = page.getByRole("form", { name: "Message composer" });
    const input = page.getByRole("textbox", {
      name: "Message",
      exact: true,
    });
    await input.fill("Keep this through reconnect");
    const history: unknown[] = [];
    for (const outcome of ["failed", "interrupted", "disconnected"]) {
      const item = {
        id: outcome,
        type: "contextCompaction",
        startedAtMs: 100 + history.length,
      };
      const running =
        provider === "codex"
          ? { id: `turn-${outcome}`, status: "inProgress", items: [item] }
          : {
              id: `msg_${outcome}`,
              type: "compaction",
              status: "running",
              reason: "auto",
              summary: "",
              time: { created: item.startedAtMs },
            };
      await setHistory(page, provider, [...history, running], true);
      await emit(
        page,
        provider === "codex" ? "item/started" : "session.compaction.started",
        provider === "codex"
          ? { threadId: id, item }
          : { sessionID: id, reason: "auto" },
        provider,
        { id: `evt_${outcome}`, created: item.startedAtMs },
      );
      await expect(notices.last()).toHaveAttribute("data-status", "running");
      if (outcome === "failed") {
        const error = { message: "Summary request failed" };
        history.push({ ...running, status: "failed", error });
        await setHistory(page, provider, history, false);
        if (provider === "opencode")
          await emit(
            page,
            "session.compaction.failed",
            { sessionID: id, reason: "auto", error },
            provider,
            { id: "evt_failure", created: 101 },
          );
        await emit(
          page,
          provider === "codex" ? "turn/completed" : "session.execution.failed",
          provider === "codex"
            ? {
                threadId: id,
                turn: { id: `turn-${outcome}`, status: "failed", error },
              }
            : { sessionID: id, error },
          provider,
        );
        await expect(notices.last()).toHaveAttribute("data-status", "failed");
        await expect(notices.last().getByRole("alert")).toHaveText(
          "Summary request failed",
        );
      } else if (outcome === "interrupted") {
        history.push(
          provider === "codex"
            ? { ...running, status: "interrupted" }
            : running,
        );
        if (provider === "opencode")
          history.push({
            id: "msg_idle",
            type: "idle",
            outcome: "interrupted",
          });
        await setHistory(page, provider, history, false);
        await page.getByRole("button", { name: "Stop generation" }).click();
        await expect(notices.last()).toHaveAttribute(
          "data-status",
          "interrupted",
        );
      } else {
        await emit(
          page,
          provider === "codex" ? "codex/processExited" : "server.disconnected",
          {},
          provider,
        );
        await expect(notices.last()).toHaveAttribute(
          "data-status",
          "interrupted",
        );
        await expect(input).toBeDisabled();
        history.push(
          provider === "codex"
            ? {
                ...running,
                status: "completed",
                items: [{ ...item, completedAtMs: 200 }],
              }
            : {
                ...running,
                status: "completed",
                native: { encrypted_content: "opaque-provider-checkpoint" },
              },
        );
        await setHistory(page, provider, history, false);
        await page
          .getByRole("button", { name: "Reconnect", exact: true })
          .click();
        await expect(input).toBeEnabled();
        await expect(notices).toHaveCount(3);
        await expect(notices.last()).toHaveAttribute(
          "data-status",
          "completed",
        );
        await expect(notices.last().getByRole("button")).toHaveCount(0);
        await expect(page.getByText("opaque-provider-checkpoint")).toHaveCount(
          0,
        );
        await expect(notices.first()).toHaveAttribute("data-status", "failed");
        await expect(notices.nth(1)).toHaveAttribute(
          "data-status",
          "interrupted",
        );
      }
      await expect(activity).toHaveAttribute("data-working", "false");
      await expect(input).toHaveValue("Keep this through reconnect");
    }
  });
}

test("OpenCode reconciles manual compaction across stale snapshots, streamed summaries and missed starts", async ({
  page,
}) => {
  await desktop(page, null);
  await choose(page, "Provider", "OpenCode");
  await page
    .getByRole("button", { name: "OpenCode session", exact: true })
    .click();
  await expect(page.getByText("Restored OpenCode conversation")).toBeVisible();
  const snapshot = {
    id: "msg_manual",
    type: "compaction",
    status: "running",
    reason: "manual",
    summary: "Keep the decisions",
    time: { created: 100 },
  };
  await setHistory(page, "opencode", [snapshot], true);
  await page.evaluate(() => (window as any).fixture.holdHistory());
  await emit(
    page,
    "session.compaction.started",
    { sessionID: "ses-a", inputID: "msg_manual", reason: "manual" },
    "opencode",
    { id: "evt_start", created: 100 },
  );
  const notice = page.getByRole("region", { name: "Context compaction" });
  await expect(notice).toHaveCount(1);
  await expect(notice).toHaveAttribute("data-status", "running");
  await emit(
    page,
    "session.compaction.delta",
    { sessionID: "ses-a", text: "Keep the" },
    "opencode",
    { created: 110 },
  );
  await notice.getByRole("button", { name: "Compacting context…" }).click();
  await emit(
    page,
    "session.compaction.delta",
    { sessionID: "ses-a", text: " decisions" },
    "opencode",
    { created: 120 },
  );
  await expect(notice.locator(".compaction-details")).toHaveText(
    "Keep the decisions",
  );
  const summary = "## Retained context\n\nKeep the decisions and next steps.";
  await emit(
    page,
    "session.compaction.ended",
    { sessionID: "ses-a", reason: "manual", text: summary },
    "opencode",
    { id: "evt_end", created: 130 },
  );
  await expect(notice).toHaveAttribute("data-status", "completed");
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveAttribute("data-working", "true");
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
  );
  await setHistory(
    page,
    "opencode",
    [{ ...snapshot, status: "completed", summary }],
    false,
  );
  await page.evaluate(() => (window as any).fixture.releaseHistory());
  await expect(notice).toHaveCount(1);
  await expect(notice).toHaveAttribute("data-status", "completed");
  await expect(
    notice.getByRole("heading", { name: "Retained context" }),
  ).toBeVisible();
  await expect(notice.locator(".compaction-details")).toContainText(
    "Keep the decisions and next steps.",
  );
  await expect(
    page.getByRole("form", { name: "Message composer" }),
  ).toHaveAttribute("data-working", "false");
  await setHistory(
    page,
    "opencode",
    [
      { ...snapshot, status: "completed", summary },
      {
        id: "msg_missed",
        type: "compaction",
        reason: "auto",
        status: "completed",
        summary: "Recovered after missing the start",
        time: { created: 200 },
      },
    ],
    false,
  );
  await emit(
    page,
    "session.compaction.ended",
    {
      sessionID: "ses-a",
      reason: "auto",
      text: "Recovered after missing the start",
    },
    "opencode",
    { id: "evt_missed_end", created: 220 },
  );
  await expect(notice).toHaveCount(2);
  await expect(notice.last()).toHaveAttribute("data-status", "completed");
  await notice
    .last()
    .getByRole("button", { name: "Context compacted" })
    .click();
  await expect(notice.last()).toContainText(
    "Recovered after missing the start",
  );
  await expect(
    page.getByRole("button", { name: "Stop generation" }),
  ).toHaveCount(0);
});
