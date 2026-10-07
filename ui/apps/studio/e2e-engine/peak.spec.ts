/**
 * The page at peak load: Midtown's evening rush (`scenarios/midtown-rush-hour.yaml`) against the
 * real engine, measured as the owner would run it, with budgets that fail when a change makes the
 * page, the scene, the inspector or the metrics fall over under it.
 *
 *   VWP_PEAK_SCENARIO  the scenario (default: scenarios/midtown-rush-hour.yaml)
 *   VWP_PEAK_SECONDS   wall seconds of streaming to measure over (default 60)
 *   VWP_PEAK_REPORT    a JSON file for the measurements (default: none)
 *   VWP_PEAK_SHOTS     a directory for screenshots (default: none)
 *
 * What is measured, every 5 s of wall clock while the run streams:
 *  - the viewer's own frame statistics (`FrameStats`, 09-ui §4's budget instrument): CPU work per
 *    frame (interpolate, cull, LOD, matrix and animation writes) and whole-frame time;
 *  - main-thread long tasks (the Long Tasks API: any task over 50 ms, the W3C definition), which is
 *    what makes a page stop answering clicks;
 *  - the tab's JS heap after a forced collection, and the engine's physical footprint;
 *  - the run's simulated clock, so the engine's throughput is known.
 * Then the inspector is opened on a vehicle and the metrics panel is opened, each timed from the
 * click to its content.
 *
 * Headless Chromium rasterises through SwiftShader, so whole-frame time here is a software-
 * rasteriser figure and only the CPU half is budgeted (see the budgets below); a GPU run is
 * `VWP_HEADED=1`.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, REPO, heapBytes, open, openSettings, status } from "./support.js";

const SECONDS = Number(process.env.VWP_PEAK_SECONDS ?? "60");
const SHOTS = process.env.VWP_PEAK_SHOTS;
const REPORT = process.env.VWP_PEAK_REPORT;

/**
 * The budgets. Each is a ceiling a regression crosses, set from the measurement recorded in
 * docs/design/BUILD-STATUS.md (stabilise track, 2026-10-06) with headroom for a loaded machine,
 * and each says what it protects.
 */
const BUDGET = {
  /** Viewer CPU per frame, mean, ms. 60 fps leaves 16.7 ms a frame (ADR 0011's target); the
   * viewer's share must leave the rest for React and the stream. `test/budget.test.ts` holds the
   * viewer alone to 8 ms at 5,000 actors; in the page, with the stream decoding beside it, 12. */
  viewerCpuMeanMs: 12,
  /** The longest main-thread task, ms. A click waits for the task in front of it; past about
   * 100 ms a response no longer feels immediate (RAIL's response budget, web.dev). Four times
   * that is a page that has visibly hung, which is the failure this catches. */
  longestTaskMs: 400,
  /** Share of wall time spent in long tasks. Above a quarter the page is stuttering throughout. */
  longTaskShare: 0.25,
  /** Tab heap after GC, MB: the soak measured 43–62 MB on the default scenario; a peak run's
   * pose buffers, feeds and metric series may take several times that, not without bound. */
  heapMb: 600,
  /** Engine physical footprint, MB. The seek history is capped at `--retain-mb` (1,024 MB by
   * default) and a peak world and run take about 1.4 GB beside it (measured); past 3 GB the
   * page and the engine no longer fit an 8 GB laptop together. */
  engineMb: 3_000,
  /** From the click to the inspector's content, and to the metrics panel's, ms. */
  openMs: 5_000,
};

let engine: EngineProcess;

test.beforeAll(async () => {
  engine = new EngineProcess(process.env.VWP_PEAK_SCENARIO ?? join(REPO, "scenarios/midtown-rush-hour.yaml"));
  await engine.start();
});

test.afterAll(async () => {
  await engine?.stop();
});

interface Sample {
  wall_s: number;
  sim_s: number;
  actors: number;
  viewer_cpu_ms: number;
  frame_ms: number;
  p95_ms: number;
  fps: number;
  heap_mb: number;
  engine_mb: number;
  long_tasks: number;
  long_task_ms: number;
  longest_task_ms: number;
}

/** Install a long-task observer that accumulates into `window.__peak`. */
async function watchLongTasks(page: Page): Promise<void> {
  await page.evaluate(() => {
    const w = window as unknown as { __peak?: { n: number; ms: number; max: number } };
    w.__peak = { n: 0, ms: 0, max: 0 };
    new PerformanceObserver((list) => {
      for (const e of list.getEntries()) {
        const p = w.__peak!;
        p.n++;
        p.ms += e.duration;
        p.max = Math.max(p.max, e.duration);
      }
    }).observe({ type: "longtask", buffered: false });
  });
}

async function sample(page: Page, t0: number): Promise<Sample> {
  const heap = await heapBytes(page);
  const st = await status(page);
  const v = await page.evaluate(() => {
    type Snap = { fps: number; frameMs: number; p95Ms: number; cpuMs: number };
    const api = window.__vwpStudio as unknown as {
      engine: { viewer: { stats: { snapshot(): Snap; reset(): void } } | null; client: { poses: { count: number } } | null };
    };
    const snap = api.engine.viewer?.stats.snapshot();
    api.engine.viewer?.stats.reset();
    const p = (window as unknown as { __peak: { n: number; ms: number; max: number } }).__peak;
    const out = { snap, actors: api.engine.client?.poses.count ?? 0, lt: { ...p } };
    p.n = 0;
    p.ms = 0;
    p.max = 0;
    return out;
  });
  return {
    wall_s: (Date.now() - t0) / 1000,
    sim_s: st.t_ns / 1e9,
    actors: v.actors,
    viewer_cpu_ms: v.snap?.cpuMs ?? 0,
    frame_ms: v.snap?.frameMs ?? 0,
    p95_ms: v.snap?.p95Ms ?? 0,
    fps: v.snap?.fps ?? 0,
    heap_mb: heap / 2 ** 20,
    engine_mb: engine.footprintKb() / 1024,
    long_tasks: v.lt.n,
    long_task_ms: v.lt.ms,
    longest_task_ms: v.lt.max,
  };
}

async function shot(page: Page, name: string): Promise<void> {
  if (!SHOTS) return;
  mkdirSync(SHOTS, { recursive: true });
  await page.screenshot({ path: join(SHOTS, `${name}.jpg`), type: "jpeg", quality: 60 });
}

test("the page holds up at Midtown's evening peak", async ({ page }) => {
  test.setTimeout((SECONDS + 300) * 1000);
  await open(page);
  await watchLongTasks(page);

  // Run, as the owner does: from the settings window's Run.
  await openSettings(page);
  await page.getByTestId("run-start").click();
  await expect.poll(async () => (await status(page)).state, { timeout: 120_000 }).toBe("running");
  await expect(page.getByTestId("settings-window")).toHaveCount(0, { timeout: 30_000 });

  const t0 = Date.now();
  const samples: Sample[] = [];
  while ((Date.now() - t0) / 1000 < SECONDS) {
    await page.waitForTimeout(5_000);
    samples.push(await sample(page, t0));
  }
  await shot(page, "peak-overview");

  // The inspector on a vehicle that carries a radio.
  const actor = await page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as { nodeByActor: Map<number, number> };
    return [...e.nodeByActor.keys()].at(-1) ?? -1;
  });
  expect(actor, "no equipped vehicle to inspect").toBeGreaterThanOrEqual(0);
  let t = Date.now();
  await page.evaluate((a) => window.__vwpStudio?.engine.selectActor(a, "chase"), actor);
  await expect(page.getByTestId("obu-hud")).toBeVisible({ timeout: 30_000 });
  const inspectorMs = Date.now() - t;
  await page.waitForTimeout(3_000);
  samples.push(await sample(page, t0));
  await shot(page, "peak-chase");

  // The metrics panel.
  t = Date.now();
  await page.getByTestId("metrics-button").click();
  await expect(page.locator('[data-testid^="metric-card"]').first()).toBeVisible({ timeout: 30_000 });
  const metricsMs = Date.now() - t;
  await page.waitForTimeout(3_000);
  samples.push(await sample(page, t0));
  await shot(page, "peak-metrics");
  await page.keyboard.press("Escape");

  const st = await status(page);
  const streamed = samples.slice(0, -2);
  const mean = (xs: number[]): number => xs.reduce((a, b) => a + b, 0) / Math.max(1, xs.length);
  const wall = samples.at(-1)!.wall_s;
  const report = {
    scenario: engine.scenario,
    wall_s: wall,
    sim_s: st.t_ns / 1e9,
    sim_per_wall: samples.at(-1)!.sim_s / wall,
    actors_peak: Math.max(...samples.map((s) => s.actors)),
    viewer_cpu_mean_ms: mean(streamed.map((s) => s.viewer_cpu_ms)),
    frame_mean_ms: mean(streamed.map((s) => s.frame_ms)),
    longest_task_ms: Math.max(...samples.map((s) => s.longest_task_ms)),
    long_task_share: samples.reduce((a, s) => a + s.long_task_ms, 0) / (wall * 1000),
    heap_peak_mb: Math.max(...samples.map((s) => s.heap_mb)),
    engine_peak_mb: Math.max(...samples.map((s) => s.engine_mb)),
    inspector_ms: inspectorMs,
    metrics_ms: metricsMs,
    engine_stats: st.engine.stats,
    retained_bytes: st.engine.retained_bytes,
    samples,
  };
  // eslint-disable-next-line no-console
  console.log(`PEAK ${JSON.stringify({ ...report, samples: undefined })}`);
  if (REPORT) writeFileSync(REPORT, JSON.stringify(report, null, 2));

  expect.soft(report.viewer_cpu_mean_ms, "viewer CPU per frame, mean ms").toBeLessThan(BUDGET.viewerCpuMeanMs);
  expect.soft(report.longest_task_ms, "longest main-thread task, ms").toBeLessThan(BUDGET.longestTaskMs);
  expect.soft(report.long_task_share, "share of wall time in long tasks").toBeLessThan(BUDGET.longTaskShare);
  expect.soft(report.heap_peak_mb, "tab heap after GC, MB").toBeLessThan(BUDGET.heapMb);
  expect.soft(report.engine_peak_mb, "engine physical footprint, MB").toBeLessThan(BUDGET.engineMb);
  expect.soft(report.inspector_ms, "click to inspector, ms").toBeLessThan(BUDGET.openMs);
  expect.soft(report.metrics_ms, "click to metrics, ms").toBeLessThan(BUDGET.openMs);
  // The run streamed, and the page saw the crowd.
  expect(report.sim_s).toBeGreaterThan(0);
  expect(report.actors_peak).toBeGreaterThan(1_000);
});
