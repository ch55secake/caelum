import { test, expect, type Page } from "@playwright/test";
import { writeFile } from "node:fs/promises";
import { execFileSync } from "node:child_process";
import os from "node:os";
import { desktop } from "../tests/fixtures/desktop.js";
import { choose, previewPullRequest, showPullRequests } from "../tests/fixtures/helpers.js";

const pause = (page: Page, ms: number) => page.evaluate(
  (ms) => new Promise<void>((resolve) => setTimeout(resolve, ms)), ms,
);
const reset = (page: Page) => page.evaluate(() => (window as any).__CAELUM_PROFILE__.reset());
const session = (page: Page, name: string) => page.getByRole("button", { name, exact: true });
const first = "Refine the session workspace";
const second = "Review the provider adapters";

// The scheduler sends on a wall-clock cadence, rather than awaiting React after
// each event. Latency ends at DOM mutation (not compositor presentation).
async function stream(page: Page, concurrent: boolean) {
  return page.evaluate(async (concurrent) => {
    const fixture = (window as any).fixture;
    const started = performance.now();
    const itemId = concurrent ? "profile-concurrent-answer" : "profile-answer";
    const pending = new Map<string, number>();
    const latency: number[] = [];
    const observer = new MutationObserver(() => {
      const text = document.querySelector(`[data-entry-id="codex-${itemId}"]`)?.textContent ?? "";
      for (const [token, time] of pending) if (text.includes(token)) {
        latency.push(performance.now() - time);
        pending.delete(token);
      }
    });
    observer.observe(document.getElementById("root")!, { subtree: true, childList: true, characterData: true });
    const codex = (method: string, params: Record<string, unknown>) => fixture.emit("codex-event", {
      projectPath: "/work/caelum", method, params,
    });
    const opencode = (eventType: string, data: Record<string, unknown>) => fixture.emit("opencode-event", {
      projectPath: "/work/caelum", eventType, data,
    });
    codex("turn/started", { threadId: "thread-a", turn: { id: "profile-turn" } });
    if (concurrent) {
      codex("turn/started", { threadId: "thread-b", turn: { id: "profile-background" } });
      opencode("session.execution.started", { sessionID: "ses-a", executionID: "profile-open" });
    }
    for (let index = 0; index < 500; index++) {
      const token = `p${index.toString().padStart(4, "0")}`;
      pending.set(token, performance.now());
      const delta = `${token} **streamed** text\n`;
      codex("item/agentMessage/delta", { threadId: "thread-a", itemId, delta });
      if (concurrent) {
        codex("item/agentMessage/delta", { threadId: "thread-b", itemId: "profile-background-answer", delta });
        opencode("session.text.delta", { sessionID: "ses-a", assistantMessageID: "profile-open-answer", delta });
      }
      await new Promise<void>((resolve) => setTimeout(resolve, Math.max(0, started + (index + 1) * 10 - performance.now())));
    }
    codex("turn/completed", { threadId: "thread-a", turn: { id: "profile-turn", status: "completed" } });
    if (concurrent) {
      codex("turn/completed", { threadId: "thread-b", turn: { id: "profile-background", status: "completed" } });
      opencode("session.execution.succeeded", { sessionID: "ses-a", executionID: "profile-open" });
    }
    await new Promise<void>((resolve) => requestAnimationFrame(() => requestAnimationFrame(() => resolve())));
    observer.disconnect();
    latency.sort((a, b) => a - b);
    return {
      targetEventsPerSecond: concurrent ? 300 : 100,
      deltaEvents: concurrent ? 1500 : 500,
      actualDurationMs: performance.now() - started,
      outputLatencyMs: {
        count: latency.length, missing: pending.size,
        p50: latency[Math.ceil(latency.length * 0.5) - 1],
        p95: latency[Math.ceil(latency.length * 0.95) - 1],
        max: latency.at(-1),
      },
    };
  }, concurrent);
}

test("release frontend workloads", async ({ page, browser, browserName }, info) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  const samples: Record<string, unknown>[] = [];
  const cdp = browserName === "chromium" ? await page.context().newCDPSession(page) : null;
  if (cdp) await cdp.send("Performance.enable");
  const capture = async (name: string, extra = {}) => {
    const frontend = await page.evaluate(() => (window as any).__CAELUM_PROFILE__.snapshot());
    if (name === "single-stream") {
      expect(frontend.renders.count).toBeGreaterThan(0);
      expect(frontend.events["codex-event:item/agentMessage/delta"].count).toBe(500);
    }
    const metrics = cdp ? await cdp.send("Performance.getMetrics") : null;
    samples.push({ name, ...frontend, ...extra, chromium: metrics?.metrics ?? null });
  };
  await desktop(page);
  await expect(page.getByText("A focused workspace", { exact: true })).toBeVisible();
  await capture("fresh-context-restoration");
  await page.reload();
  await expect(page.getByText("A focused workspace", { exact: true })).toBeVisible();
  await capture("warm-restoration");
  await pause(page, 500);
  await reset(page);
  await pause(page, 5000);
  await capture("idle");

  await reset(page);
  const single = await stream(page, false);
  expect(single.outputLatencyMs.missing).toBe(0);
  await capture("single-stream", single);

  // Load all three sessions so background execution exercises loaded state too.
  await session(page, second).click();
  await expect(page.getByRole("textbox", { name: "Message", exact: true })).toBeEnabled();
  await page.getByRole("button", { name: /New session/ }).click();
  await choose(page, "Provider", "OpenCode");
  await session(page, "OpenCode session").click();
  await expect(page.getByText("Restored OpenCode conversation", { exact: true })).toBeVisible();
  await session(page, first).click();
  await pause(page, 100);
  await reset(page);
  const concurrent = await stream(page, true);
  expect(concurrent.outputLatencyMs.missing).toBe(0);
  await expect(page.locator(".session-row").getByLabel("Running", { exact: true })).toHaveCount(0);
  await capture("concurrent-streams", concurrent);

  await page.getByRole("button", { name: /New session/ }).click();
  await page.evaluate(() => {
    (window as any).fixture.addSession("profile-history", "Profile long history");
    for (let page = 0; page < 10; page++) {
      (window as any).fixture.setHistoryPage("codex", "profile-history", page ? `page-${page}` : null, {
        data: Array.from({ length: 100 }, (_, index) => {
          const id = (9 - page) * 100 + index;
          return {
            id: `turn-${id}`, status: "completed", items: [
              { id: `user-${id}`, type: "userMessage", content: [{ type: "text", text: `Prompt ${id}` }] },
              { id: `answer-${id}`, type: "agentMessage", text: `Answer ${id}\n\n` + "A bounded **markdown** history with `code`.\n".repeat(20) },
            ],
          };
        }),
        nextCursor: page < 9 ? `page-${page + 1}` : null, isWorking: false,
      });
    }
  });
  await page.getByRole("button", { name: "Refresh sessions", exact: true }).click();
  await reset(page);
  await session(page, "Profile long history").click();
  const input = page.getByRole("textbox", { name: "Message", exact: true });
  await expect(page.getByText("Prompt 999", { exact: true })).toBeVisible();
  await input.fill("/timeline");
  await input.press("Enter");
  await expect(page.getByRole("dialog", { name: "Session timeline" }).getByRole("status")).toHaveText("1000 messages");
  await page.keyboard.press("Escape");
  expect(await page.locator(".conversation-entry").count()).toBeLessThanOrEqual(100);
  await capture("long-history", { loadedEntries: 2000 });

  await reset(page);
  for (let index = 0; index < 20; index++) {
    await session(page, index % 2 ? "Profile long history" : first).click();
    await expect(input).toBeEnabled();
  }
  await capture("session-navigation", { switches: 20 });

  await page.getByRole("button", { name: "Open project", exact: true }).click();
  await expect(page.getByRole("button", { name: "other", exact: true })).toBeVisible();
  await reset(page);
  for (let index = 0; index < 20; index++) {
    await page.getByRole("button", { name: index % 2 ? "other" : "caelum", exact: true }).click();
    await expect(input).toBeEnabled();
  }
  await capture("project-navigation", { switches: 20 });
  await page.getByRole("button", { name: "caelum", exact: true }).click();

  await page.evaluate((pr) => {
    const fixture = (window as any).fixture;
    fixture.setPullRequest("feat/session-ui-redesign", pr);
    fixture.setPullRequestDetails(pr.url, { ...pr, changedFiles: 100 });
    fixture.setPullRequestPage(pr.url, "files", 1, {
      baseOid: "profile-base", nextPage: null,
      files: Array.from({ length: 100 }, (_, index) => ({
        path: `src/profile-${index}.ts`, previousPath: null, status: "modified", additions: 200, deletions: 200,
        patch: "@@ -1,200 +1,200 @@\n" + Array.from({ length: 200 }, (_, line) =>
          `-export const before${line} = ${line};\n+export const after${line} = ${line + 1};`).join("\n"),
      })),
    });
  }, previewPullRequest);
  await page.getByRole("button", { name: "Refresh sessions", exact: true }).click();
  const prs = await showPullRequests(page);
  await reset(page);
  await prs.getByRole("button", { name: /View pull request #44/ }).click();
  await page.getByRole("tab", { name: "Files changed (100)", exact: true }).click();
  await expect(page.locator(".pr-file-diff")).toHaveCount(100);
  await pause(page, 1000);
  await capture("large-pull-request", { files: 100, diffLines: 40_000 });
  await reset(page);
  await page.getByRole("button", { name: "Back to session", exact: true }).click();
  await page.getByRole("button", { name: /New session/ }).click();
  await pause(page, 5000);
  await capture("views-closed");
  if (cdp) {
    await cdp.send("HeapProfiler.collectGarbage");
    await capture("views-closed-after-forced-gc");
  }
  expect(errors).toEqual([]);
  const report = {
    revision: execFileSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" }).trim(),
    platform: `${os.platform()} ${os.release()} ${os.arch()}`, cpus: os.cpus()[0]?.model,
    browser: browserName, browserVersion: browser.version(), repeat: info.repeatEachIndex,
    instrumented: true, nativeAndProviders: "mocked", samples,
  };
  await writeFile(info.outputPath("metrics.json"), JSON.stringify(report, null, 2));
});
