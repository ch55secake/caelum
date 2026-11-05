import { expect, test, type Page } from "@playwright/test";
import { desktop } from "./fixtures/desktop.js";
import {
  emit,
  openSettings,
  question,
  setQuestions,
} from "./fixtures/helpers.js";

async function soundFixture(page: Page) {
  await page.addInitScript(() => {
    (window as any).sounds = [];
    class Audio {
      state = "running";
      currentTime = 0;
      destination = {};
      resume() {
        this.state = "running";
        return Promise.resolve();
      }
      suspend() {
        this.state = "suspended";
        return Promise.resolve();
      }
      createGain() {
        return {
          connect() {},
          disconnect() {},
          gain: {
            setValueAtTime() {},
            linearRampToValueAtTime(value: number) {
              (window as any).soundVolume = value;
            },
            exponentialRampToValueAtTime() {},
          },
        };
      }
      createOscillator() {
        return {
          frequency: { setValueAtTime() {} },
          connect() {},
          disconnect() {},
          start() {
            (window as any).sounds.push((window as any).soundVolume);
          },
          stop() {},
        };
      }
    }
    (window as any).AudioContext = Audio;
  });
  await page.clock.install();
}
const sounds = (page: Page) =>
  page.evaluate(() => (window as any).sounds as number[]);
async function complete(page: Page, id: string, thread = "thread-a") {
  await emit(page, "turn/started", { threadId: thread, turn: { id } });
  await emit(page, "turn/completed", {
    threadId: thread,
    turn: { id, status: "completed" },
  });
}

test("completion sounds default to background only, deduplicate terminals, and honour Always and Off", async ({
  page,
}) => {
  await soundFixture(page);
  await desktop(page);
  await complete(page, "foreground");
  expect(await sounds(page)).toHaveLength(0);
  await complete(page, "background", "thread-b");
  await expect.poll(() => sounds(page)).toHaveLength(1);
  await page.clock.fastForward(1000);
  await emit(page, "turn/completed", {
    threadId: "thread-b",
    turn: { id: "background", status: "completed" },
  });
  await emit(page, "item/completed", {
    threadId: "thread-b",
    item: {
      id: "tool",
      type: "commandExecution",
      command: "true",
      status: "completed",
    },
  });
  expect(await sounds(page)).toHaveLength(1);
  const settings = await openSettings(page, "Notifications");
  await expect(
    settings.getByRole("radio", { name: "Background only", exact: true }),
  ).toBeChecked();
  await settings.getByRole("radio", { name: "Always", exact: true }).check();
  await settings.getByRole("slider", { name: "Sound volume" }).press("Home");
  for (let i = 0; i < 25; i++) await page.keyboard.press("ArrowRight");
  await settings
    .getByRole("button", { name: "Test sound", exact: true })
    .click();
  await expect.poll(() => sounds(page)).toHaveLength(2);
  expect((await sounds(page)).at(-1)).toBeCloseTo(0.01);
  await settings.getByRole("button", { name: "Done", exact: true }).click();
  await complete(page, "always");
  await expect.poll(() => sounds(page)).toHaveLength(3);
  await page.clock.fastForward(1000);
  await openSettings(page, "Notifications");
  await settings.getByRole("radio", { name: "Off", exact: true }).check();
  await complete(page, "off", "thread-b");
  expect(await sounds(page)).toHaveLength(3);
  await page.reload();
  await openSettings(page, "Notifications");
  await expect(
    settings.getByRole("radio", { name: "Off", exact: true }),
  ).toBeChecked();
  await expect(
    settings.getByRole("slider", { name: "Sound volume" }),
  ).toHaveValue("25");
  expect(await sounds(page)).toHaveLength(0);
});

test("shell and input-needed sounds are independent, quiet during restoration, and emitted once", async ({
  page,
}) => {
  await soundFixture(page);
  await desktop(page, "thread-a", { initialQuestions: [question()] });
  expect(await sounds(page)).toHaveLength(0);
  const settings = await openSettings(page, "Notifications");
  await settings.getByRole("radio", { name: "Always", exact: true }).check();
  await setQuestions(page, [question()]);
  expect(await sounds(page)).toHaveLength(0);
  await setQuestions(page, [
    question(),
    question({ requestId: "new", sessionId: "thread-b" }),
  ]);
  await expect.poll(() => sounds(page)).toHaveLength(1);
  await page.clock.fastForward(1000);
  await setQuestions(page, [
    question(),
    question({ requestId: "new", sessionId: "thread-b" }),
  ]);
  expect(await sounds(page)).toHaveLength(1);
  await settings
    .getByRole("checkbox", { name: "Approvals and questions needing input" })
    .uncheck();
  await setQuestions(page, [
    question({ requestId: "disabled", sessionId: "thread-b" }),
  ]);
  expect(await sounds(page)).toHaveLength(1);
  const shell = (id: string) => ({
    sessionID: "ses-a",
    shell: {
      id,
      command: "cargo test",
      status: "exited",
      exit: 0,
      metadata: { background: true },
      time: { started: 1, completed: 2 },
    },
    output: { output: "done", truncated: false },
  });
  await emit(page, "session.shell.ended", shell("disabled"), "opencode");
  expect(await sounds(page)).toHaveLength(1);
  await settings
    .getByRole("checkbox", { name: "Background shell completions" })
    .check();
  await emit(page, "session.shell.ended", shell("enabled"), "opencode");
  await expect.poll(() => sounds(page)).toHaveLength(2);
  await page.clock.fastForward(1000);
  await emit(
    page,
    "session.synthetic",
    {
      sessionID: "ses-a",
      text: '<shell id="enabled" state="completed" command="cargo test">done</shell>',
    },
    "opencode",
    { id: "evt_done", created: await page.evaluate(() => Date.now()) },
  );
  expect(await sounds(page)).toHaveLength(2);
  await emit(page, "session.shell.ended", shell("historical"), "opencode", {
    id: "evt_old",
    created: 1,
  });
  await emit(
    page,
    "session.execution.started",
    { sessionID: "ses-a" },
    "opencode",
    { id: "evt_start", created: 1 },
  );
  await emit(
    page,
    "session.execution.succeeded",
    { sessionID: "ses-a" },
    "opencode",
    { id: "evt_end", created: 2 },
  );
  expect(await sounds(page)).toHaveLength(2);
});

test("Test sound schedules a real short tone and releases its audio nodes", async ({ page }, info) => {
  await page.addInitScript(() => {
    const create = AudioContext.prototype.createOscillator;
    (window as any).tonesEnded = 0;
    AudioContext.prototype.createOscillator = function () {
      const oscillator = create.call(this);
      oscillator.addEventListener("ended", () => (window as any).tonesEnded++);
      return oscillator;
    };
  });
  await desktop(page);
  const settings = await openSettings(page, "Notifications");
  await settings.getByRole("button", { name: "Test sound", exact: true }).click();
  await expect.poll(() => page.evaluate(() => (window as any).tonesEnded)).toBe(1);
  await expect(settings.getByRole("alert")).toHaveCount(0);
  await page.setViewportSize({ width: 800, height: 620 });
  await page.screenshot({ path: info.outputPath("notifications-minimum.png") });
});
