/**
 * The owner's own path through the Studio, against the real engine on a ready-made scenario:
 * Run, pause, play, speed, restart while running, a setting changed then "Restart now", a run to
 * its end with its summary, then another scenario and another city from the picker. Every step
 * checks the engine's own `run.status`, and the page must log no error the whole way.
 *
 *   VWP_ENGINE_BIN=<v2xw-server> pnpm exec playwright test -c playwright.engine.config.ts owner-flow
 */
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, REPO, closeSettings, open, openSettings, setField, status, type Status } from "./support.js";

const engine = new EngineProcess(join(REPO, "scenarios/midtown-normal.yaml"), undefined, "0");
const shots = process.env.VWP_SHOTS ?? "";

test.describe.configure({ mode: "serial" });
test.beforeAll(async () => engine.start());
test.afterAll(async () => engine.stop());

async function until(page: Page, what: string, ok: (s: Status) => boolean, ms = 60_000): Promise<Status> {
  const end = Date.now() + ms;
  let s = await status(page);
  while (Date.now() < end) {
    s = await status(page);
    if (ok(s)) return s;
    await page.waitForTimeout(200);
  }
  throw new Error(`${what}: still ${JSON.stringify({ state: s.state, t: s.t_ns / 1e9, gen: s.generation, speed: s.speed })}`);
}

async function shot(page: Page, name: string): Promise<void> {
  if (shots) await page.screenshot({ path: join(shots, `${name}.png`) });
}

test("the owner's run lifecycle has no stuck state and no console error", async ({ page }) => {
  const errors: string[] = [];
  page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
  page.on("console", (m) => {
    if (m.type() === "error") errors.push(`console: ${m.text()}`);
  });
  await open(page);
  await shot(page, "01-open");

  // Run from the header's primary action.
  const g0 = (await status(page)).generation;
  await page.getByTestId("primary-action").click();
  let s = await until(page, "Run starts", (x) => x.generation > g0 && x.state === "running");
  // Slow it down so the controls are pressed on a live run.
  await page.getByTestId("speed").selectOption("1");
  s = await until(page, "speed 1x", (x) => x.speed === 1);
  await page.waitForTimeout(1500);
  await shot(page, "02-running");

  await page.getByTestId("pause").click();
  s = await until(page, "pause", (x) => x.state === "paused");
  const tPaused = s.t_ns;
  await page.waitForTimeout(800);
  expect((await status(page)).t_ns, "time stands still while paused").toBe(tPaused);
  await page.getByTestId("play").click();
  await until(page, "play", (x) => x.state === "running" && x.t_ns > tPaused);

  await page.getByTestId("speed").selectOption("10");
  await until(page, "speed 10x", (x) => x.speed === 10);

  // Restart while running: a new generation, from the start, still running.
  const g1 = (await status(page)).generation;
  await page.getByTestId("restart-run").click();
  s = await until(page, "restart while running", (x) => x.generation > g1 && (x.state === "running" || x.state === "paused"));
  await shot(page, "03-restarted");

  // Change a setting while running, then "Restart now".
  await setField(page, "/time/duration_s", "20");
  await page.getByTestId("apply").click();
  await expect(page.getByTestId("scenario-message")).toContainText(/restart|Applied|take effect/i);
  await closeSettings(page);
  const g2 = (await status(page)).generation;
  const now = page.getByTestId("restart-now");
  await expect(now).toBeVisible();
  await now.click();
  s = await until(page, "Restart now", (x) => x.generation > g2);
  expect(s.t_end_ns, "the applied run length reached the run").toBe(20e9);
  await page.getByTestId("speed").selectOption("0");
  s = await until(page, "run to the end", (x) => x.state === "finished", 120_000);
  await page.waitForTimeout(1000);
  await shot(page, "04-finished");
  await expect(page.getByTestId("status-headline")).not.toHaveText("");

  // Another scenario, then another city, from the picker.
  for (const pick of [{ preset: "Rush hour" }, { city: "portland" }, { city: "chicago" }]) {
    await page.getByTestId("scenario-switcher").click();
    await expect(page.getByTestId("scenario-picker")).toBeVisible();
    if (pick.preset) {
      const row = page.getByTestId("picker-scenario").filter({ hasText: new RegExp(pick.preset, "i") }).first();
      await row.click();
    }
    if (pick.city) await page.locator(`[data-testid="picker-city"][data-city="${pick.city}"]`).click();
    const g = (await status(page)).generation;
    await page.getByTestId("picker-run").click();
    s = await until(page, `run ${JSON.stringify(pick)}`, (x) => x.generation > g && x.state !== "idle", 120_000);
    s = await until(page, `traffic on ${JSON.stringify(pick)}`, (x) => x.engine.stats.actors_seen > 0 || x.state === "finished", 120_000);
    expect(s.engine.stats.actors_seen, `vehicles on ${JSON.stringify(pick)}`).toBeGreaterThan(0);
    await shot(page, `05-${pick.preset ?? pick.city}`);
  }
  expect(errors, errors.join("\n")).toEqual([]);
  void openSettings;
});
