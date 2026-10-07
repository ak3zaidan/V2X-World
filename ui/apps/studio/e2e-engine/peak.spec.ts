/**
 * The page at peak load: Midtown's evening rush (`scenarios/midtown-peak-load.yaml`) against the
 * real engine, measured as the owner would run it, with budgets that fail when a change makes the
 * page, the scene, the inspector or the metrics fall over under it.
 *
 *   VWP_PEAK_SCENARIO  the scenario (default: scenarios/midtown-peak-load.yaml)
 *   VWP_PEAK_SECONDS   wall seconds of streaming to measure over (default 60)
 *   VWP_PEAK_REPORT    a JSON file for the measurements (default: none)
 *   VWP_PEAK_SHOTS     a directory for screenshots (default: none)
 *   VWP_PEAK_PROFILE   a directory to save the three CPU profiles in (default: not saved)
 *
 * What is measured:
 *  - every 5 s while the run streams: the viewer's own frame statistics (`FrameStats`, 09-ui §4's
 *    budget instrument), main-thread long tasks (the Long Tasks API's 50 ms definition), the tab's
 *    JS heap after a forced collection, the engine's physical footprint, and the run's clock;
 *  - the V8 CPU profile of three windows — the streaming, a vehicle being selected (chase camera,
 *    HUD, inspector, node feed) and the metrics panel being opened — split into the page's own
 *    script time and the time spent blocked in synchronous WebGL calls;
 *  - the time from each of those two clicks to its content.
 *
 * Why the split. Headless Chromium rasterises through SwiftShader, a software GPU. A synchronous
 * WebGL call (`getExtension`, `getProgramInfoLog`, a canvas resize) waits for the GPU process to
 * finish the frames queued before it, so in this harness a long task is mostly the software
 * rasteriser: the first measurement found a 5,004 ms task on selecting a vehicle of which 4.1 s
 * was `WebGLRenderer.setSize` and `getExtension`, and 25 ms was React. Script time is the page's
 * own cost in any browser, so it is budgeted always; long tasks are budgeted only with a GPU
 * (`VWP_HEADED=1`), where they are the page's.
 */

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

import { expect, test, type CDPSession, type Page } from "@playwright/test";

import { EngineProcess, REPO, heapBytes, open, openSettings, status } from "./support.js";

const SECONDS = Number(process.env.VWP_PEAK_SECONDS ?? "60");
const SHOTS = process.env.VWP_PEAK_SHOTS;
const REPORT = process.env.VWP_PEAK_REPORT;
const PROFILE_DIR = process.env.VWP_PEAK_PROFILE;
const GPU = process.env.VWP_HEADED === "1";

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
  /** The page's own script time as a share of the streaming window. Above half, a click waits
   * behind script most of the time and frames are dropped to it; measured 0.012 (BUILD-STATUS.md). */
  streamingScriptShare: 0.5,
  /** Script time of one interaction (select a vehicle; open the metrics panel), ms, in the
   * development build the owner runs (`pnpm dev`, React's DEV mode). A second of script is a
   * page that visibly stops. */
  interactionScriptMs: 1_000,
  /** With a GPU only: the longest main-thread task, ms. RAIL puts the response budget at 100 ms
   * (web.dev/rail); four times that is a page that has visibly hung. */
  longestTaskMs: 400,
  /** Tab heap after GC, MB: the soak measured 43–62 MB on the default scenario; a peak run's
   * pose buffers, feeds and metric series may take several times that, not without bound. */
  heapMb: 600,
  /** Engine physical footprint, MB. The seek history is capped at `--retain-mb` (1,024 MB by
   * default) and the world and the run take about 1.4 GB beside it at this scenario's peak
   * (measured); past 3 GB the page and the engine no longer fit an 8 GB laptop together. */
  engineMb: 3_000,
  /** From the click to the inspector's content, and to the metrics panel's, ms. */
  openMs: 5_000,
};

let engine: EngineProcess;

test.beforeAll(async () => {
  engine = new EngineProcess(process.env.VWP_PEAK_SCENARIO ?? join(REPO, "scenarios/midtown-peak-load.yaml"));
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
    type Snap = { fps: number; frameMs: number; cpuMs: number };
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
    fps: v.snap?.fps ?? 0,
    heap_mb: heap / 2 ** 20,
    engine_mb: engine.footprintKb() / 1024,
    long_tasks: v.lt.n,
    long_task_ms: v.lt.ms,
    longest_task_ms: v.lt.max,
  };
}

/**
 * Native functions whose self time is the main thread blocked on the GPU process: synchronous
 * WebGL queries, and the canvas resize inside three.js's `setSize`. In SwiftShader they wait out
 * the software rasteriser.
 */
const GPU_SYNC = new Set([
  "getExtension",
  "getProgramInfoLog",
  "getShaderInfoLog",
  "getProgramParameter",
  "getShaderParameter",
  "getParameter",
  "getError",
  "readPixels",
  "finish",
  "getContextAttributes",
  "WebGLRenderer.setSize",
]);

interface ProfileSummary {
  /** Sampled wall time, ms. */
  total_ms: number;
  /** The page's own JS (and the browser work it calls), ms: total less idle, the engine's own
   * "(program)" bookkeeping, garbage collection and GPU waits. */
  script_ms: number;
  gpu_wait_ms: number;
  gc_ms: number;
  /** The heaviest functions by self time. */
  top: string[];
}

async function startProfile(page: Page): Promise<CDPSession> {
  const cdp = await page.context().newCDPSession(page);
  await cdp.send("Profiler.enable");
  await cdp.send("Profiler.setSamplingInterval", { interval: 1000 });
  await cdp.send("Profiler.start");
  return cdp;
}

async function stopProfile(cdp: CDPSession, name: string): Promise<ProfileSummary> {
  const { profile } = (await cdp.send("Profiler.stop")) as unknown as {
    profile: {
      nodes: { id: number; callFrame: { functionName: string; url: string; lineNumber: number } }[];
      samples: number[];
      timeDeltas: number[];
    };
  };
  await cdp.detach();
  if (PROFILE_DIR) {
    mkdirSync(PROFILE_DIR, { recursive: true });
    writeFileSync(join(PROFILE_DIR, `${name}.cpuprofile`), JSON.stringify(profile));
  }
  const byId = new Map(profile.nodes.map((n) => [n.id, n]));
  const self = new Map<string, number>();
  const sum = { total: 0, idle: 0, program: 0, gc: 0, gpu: 0 };
  profile.samples.forEach((id, i) => {
    const ms = (profile.timeDeltas[i] ?? 0) / 1000;
    const f = byId.get(id)?.callFrame;
    if (!f) return;
    sum.total += ms;
    const fn = f.functionName;
    if (fn === "(idle)") sum.idle += ms;
    else if (fn === "(program)") sum.program += ms;
    else if (fn === "(garbage collector)") sum.gc += ms;
    else if (GPU_SYNC.has(fn)) sum.gpu += ms;
    const key = `${fn || "(anonymous)"} ${f.url.split("/").slice(-2).join("/")}:${f.lineNumber + 1}`;
    self.set(key, (self.get(key) ?? 0) + ms);
  });
  const top = [...self.entries()]
    .filter(([k]) => !k.startsWith("(idle)"))
    .sort((a, b) => b[1] - a[1])
    .slice(0, 12)
    .map(([k, ms]) => `${ms.toFixed(0).padStart(6)} ms  ${k}`);
  return {
    total_ms: sum.total,
    script_ms: sum.total - sum.idle - sum.program - sum.gc - sum.gpu,
    gpu_wait_ms: sum.gpu,
    gc_ms: sum.gc,
    top,
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

  // The control: 300 ms of the page's own script must read as script, not as idle or GPU, or the
  // script budgets below could never fail.
  let cdp = await startProfile(page);
  await page.evaluate(() => {
    const end = performance.now() + 300;
    let x = 0;
    while (performance.now() < end) x += Math.sqrt(x + 1);
    return x;
  });
  const control = await stopProfile(cdp, "control");
  expect(control.script_ms, `the profiler saw ${control.script_ms.toFixed(0)} ms of a 300 ms busy loop as script`).toBeGreaterThan(250);

  const t0 = Date.now();
  const samples: Sample[] = [];
  cdp = await startProfile(page);
  while ((Date.now() - t0) / 1000 < SECONDS) {
    await page.waitForTimeout(5_000);
    samples.push(await sample(page, t0));
  }
  const streaming = await stopProfile(cdp, "streaming");
  await shot(page, "peak-overview");

  // A vehicle that carries a radio, selected as a click on it does: chase camera, HUD, inspector,
  // node feed.
  const actor = await page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as { nodeByActor: Map<number, number> };
    return [...e.nodeByActor.keys()].at(-1) ?? -1;
  });
  expect(actor, "no equipped vehicle to inspect").toBeGreaterThanOrEqual(0);
  cdp = await startProfile(page);
  let t = Date.now();
  await page.evaluate((a) => window.__vwpStudio?.engine.selectActor(a, "chase"), actor);
  await expect(page.getByTestId("obu-hud")).toBeVisible({ timeout: 30_000 });
  const inspectorMs = Date.now() - t;
  await page.waitForTimeout(6_000);
  const inspector = await stopProfile(cdp, "inspector");
  samples.push(await sample(page, t0));
  await shot(page, "peak-chase");

  // The metrics panel.
  cdp = await startProfile(page);
  t = Date.now();
  await page.getByTestId("metrics-button").click();
  await expect(page.locator('[data-testid^="metric-card"]').first()).toBeVisible({ timeout: 30_000 });
  const metricsMs = Date.now() - t;
  await page.waitForTimeout(6_000);
  const metrics = await stopProfile(cdp, "metrics");
  samples.push(await sample(page, t0));
  await shot(page, "peak-metrics");
  await page.keyboard.press("Escape");

  const st = await status(page);
  const streamed = samples.slice(0, -2);
  const mean = (xs: number[]): number => xs.reduce((a, b) => a + b, 0) / Math.max(1, xs.length);
  const wall = samples.at(-1)!.wall_s;
  const report = {
    scenario: engine.scenario,
    mode: GPU ? "headed-gpu" : "headless-swiftshader",
    wall_s: wall,
    sim_s: st.t_ns / 1e9,
    sim_per_wall: samples.at(-1)!.sim_s / wall,
    actors_peak: Math.max(...samples.map((s) => s.actors)),
    viewer_cpu_mean_ms: mean(streamed.map((s) => s.viewer_cpu_ms)),
    frame_mean_ms: mean(streamed.map((s) => s.frame_ms)),
    streaming_script_share: streaming.script_ms / Math.max(1, streaming.total_ms),
    streaming_gpu_wait_share: streaming.gpu_wait_ms / Math.max(1, streaming.total_ms),
    inspector_script_ms: inspector.script_ms,
    inspector_gpu_wait_ms: inspector.gpu_wait_ms,
    metrics_script_ms: metrics.script_ms,
    metrics_gpu_wait_ms: metrics.gpu_wait_ms,
    longest_task_ms: Math.max(...samples.map((s) => s.longest_task_ms)),
    long_task_share: samples.reduce((a, s) => a + s.long_task_ms, 0) / (wall * 1000),
    heap_peak_mb: Math.max(...samples.map((s) => s.heap_mb)),
    engine_peak_mb: Math.max(...samples.map((s) => s.engine_mb)),
    inspector_ms: inspectorMs,
    metrics_ms: metricsMs,
    engine_stats: st.engine.stats,
    retained_bytes: st.engine.retained_bytes,
    profiles: { streaming, inspector, metrics },
    samples,
  };
  // eslint-disable-next-line no-console
  console.log(`PEAK ${JSON.stringify({ ...report, samples: undefined, profiles: undefined })}`);
  for (const [name, p] of Object.entries(report.profiles)) {
    // eslint-disable-next-line no-console
    console.log(`PROFILE ${name} script ${p.script_ms.toFixed(0)} ms, gpu wait ${p.gpu_wait_ms.toFixed(0)} ms, of ${p.total_ms.toFixed(0)} ms\n  ${p.top.join("\n  ")}`);
  }
  if (REPORT) writeFileSync(REPORT, JSON.stringify(report, null, 2));

  expect.soft(report.viewer_cpu_mean_ms, "viewer CPU per frame, mean ms").toBeLessThan(BUDGET.viewerCpuMeanMs);
  expect.soft(report.streaming_script_share, "page script share while streaming").toBeLessThan(BUDGET.streamingScriptShare);
  expect.soft(report.inspector_script_ms, "script time selecting a vehicle, ms").toBeLessThan(BUDGET.interactionScriptMs);
  expect.soft(report.metrics_script_ms, "script time opening metrics, ms").toBeLessThan(BUDGET.interactionScriptMs);
  if (GPU) expect.soft(report.longest_task_ms, "longest main-thread task, ms").toBeLessThan(BUDGET.longestTaskMs);
  expect.soft(report.heap_peak_mb, "tab heap after GC, MB").toBeLessThan(BUDGET.heapMb);
  expect.soft(report.engine_peak_mb, "engine physical footprint, MB").toBeLessThan(BUDGET.engineMb);
  expect.soft(report.inspector_ms, "click to inspector, ms").toBeLessThan(BUDGET.openMs);
  expect.soft(report.metrics_ms, "click to metrics, ms").toBeLessThan(BUDGET.openMs);
  // The run streamed, and the page saw the crowd.
  expect(report.sim_s).toBeGreaterThan(0);
  expect(report.actors_peak).toBeGreaterThan(1_000);
});
