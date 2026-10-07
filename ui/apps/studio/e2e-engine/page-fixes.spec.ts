/**
 * The four page faults the owner hit, each checked once against the real engine:
 *
 *  1. the map view is lit as day whatever the scenario's clock says (`phase1-grid` starts at
 *     08:00 UTC, which is 03:00 local solar time at its Manhattan origin: night under the clock);
 *  2. the playback-speed menu paces the run (1× advances about one simulated second a second);
 *  3. the Restart button in the time bar rewinds a run that is playing;
 *  4. an applied edit says "Changes take effect when you restart" and offers "Restart now".
 *
 * Screenshots land in `test-results/page-fixes/` for a person to look at.
 */

import { expect, test } from "@playwright/test";
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import { EngineProcess, REPO, open, status } from "./support.js";

const scenario = join(tmpdir(), `vwp-page-fixes-${process.pid}.yaml`);
writeFileSync(
  scenario,
  readFileSync(join(REPO, "scenarios/phase1-grid.yaml"), "utf8")
    .replace("name: phase1-grid", "name: page-fixes")
    .replace("duration_s: 60.0", "duration_s: 120.0")
    .replace("rate_veh_per_h: 30.0", "rate_veh_per_h: 3000.0"),
);
const engine = new EngineProcess(scenario);
const shots = "test-results/page-fixes";

test.beforeAll(async () => {
  await engine.start();
});
test.afterAll(async () => {
  await engine.stop();
});

test("lighting, speed, restart and the restart note", async ({ page }) => {
  await open(page);
  const lighting = (): Promise<{ mode: string | null; darkness: number }> =>
    page.evaluate(() => {
      const v = (window.__vwpStudio?.engine as unknown as { viewer: { lighting: string | null; worldRenderer: { darkness: number } } }).viewer;
      return { mode: v.lighting, darkness: v.worldRenderer.darkness };
    });

  // 2. speed: start at "as fast as possible" (the engine was started with --speed 0), pick 1×.
  await page.getByTestId("play").click();
  await expect.poll(async () => (await status(page)).state).toBe("running");
  await page.getByTestId("speed").selectOption("1");
  await expect.poll(async () => (await status(page)).speed).toBe(1);
  await expect(page.getByTestId("speed")).toHaveValue("1");
  const a = await status(page);
  await page.waitForTimeout(3000);
  const b = await status(page);
  const rate = (b.t_ns - a.t_ns) / 1e9 / 3;
  console.log(`speed 1×: ${rate.toFixed(2)} simulated s per wall s`);
  expect(rate).toBeGreaterThan(0.5);
  expect(rate).toBeLessThan(1.6);

  // 1. lighting: day by default, and the map view stays day even under "follow the clock".
  expect((await lighting()).mode).toBe("day");
  await page.screenshot({ path: `${shots}/1-map-day.png` });
  await page.getByTestId("overlays-button").click();
  await page.getByTestId("lighting").selectOption("clock");
  await page.waitForTimeout(500);
  const mapUnderClock = await lighting();
  expect(mapUnderClock.mode).toBe("clock");
  expect(mapUnderClock.darkness).toBe(0);
  await page.getByTestId("lighting").selectOption("night");
  await page.keyboard.press("Escape");
  await page.evaluate(() => (window.__vwpStudio?.engine as unknown as { viewer: { setCameraMode(m: string): void } }).viewer.setCameraMode("chase"));
  await page.waitForTimeout(1500);
  expect((await lighting()).darkness).toBeGreaterThan(0.5);
  await page.screenshot({ path: `${shots}/1-chase-night.png` });
  await page.evaluate(() => (window.__vwpStudio?.engine as unknown as { viewer: { setCameraMode(m: string): void } }).viewer.setCameraMode("map"));

  // 3. restart while running.
  const before = (await status(page)).t_ns;
  expect(before).toBeGreaterThan(2e9);
  await page.getByTestId("restart-run").click();
  await expect.poll(async () => { const s = await status(page); return s.state === "running" && s.t_ns < before; }).toBe(true);
  await expect.poll(async () => (await status(page)).speed).toBe(1);
  await page.screenshot({ path: `${shots}/3-restarted.png` });

  // 4. an edit says it waits for a restart, and offers one.
  await page.getByTestId("run-duration-select").selectOption("300");
  await expect(page.getByTestId("run-duration-note")).toContainText("Changes take effect when you restart");
  await expect(page.getByTestId("restart-now")).toBeVisible();
  await page.screenshot({ path: `${shots}/4-restart-note.png` });
  const gen = (await status(page)).generation;
  await page.getByTestId("restart-now").click();
  await expect.poll(async () => (await status(page)).t_end_ns).toBe(300e9);
  expect((await status(page)).generation).toBeGreaterThan(gen);
  await page.waitForTimeout(4000);
  await page.screenshot({ path: `${shots}/4-after-restart-now.png` });
});
