/**
 * The 3D traffic scene in the real page against the real engine: the glitch hunter's geometric
 * classes over aerial, chase and dashboard views, plus the one class only pixels can show — a
 * black or empty frame — and pictures of each view for a human.
 *
 *   VWP_T3D_SCENARIO   the scenario (default: `scenarios/vru-grid.yaml` at 6,000 veh/h)
 *   VWP_T3D_SECONDS    seconds per view (default 12)
 *   VWP_T3D_SHOTS      a directory for jpeg screenshots (default: none)
 *   VWP_T3D_REPORT     a JSON file for the report (default: none)
 *
 * The hunt runs inside the page (`viewer.huntGlitches()`, `@vwp/viewer`'s `glitch.ts`); every
 * 250 ms the canvas is copied into a 2D canvas and its luminance measured, so a frame that is one
 * flat colour — the black frame the owner once reported, a wall filling the view — is counted.
 */

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { expect, test, type Page } from "@playwright/test";

import { EngineProcess, REPO, open, status } from "./support.js";

let engine: EngineProcess;
const SECONDS = Number(process.env.VWP_T3D_SECONDS ?? "12");
const SHOTS = process.env.VWP_T3D_SHOTS;
/** Pictures only, no hunt: for a Studio whose viewer predates `huntGlitches` (the BEFORE pictures). */
const SHOTS_ONLY = process.env.VWP_T3D_SHOTS_ONLY === "1";

test.beforeAll(async () => {
  let path = process.env.VWP_T3D_SCENARIO;
  if (!path) {
    const source = readFileSync(join(REPO, "scenarios/vru-grid.yaml"), "utf8");
    const dir = join(tmpdir(), `vwp-engine-e2e-t3d-${process.pid}`);
    mkdirSync(dir, { recursive: true });
    path = join(dir, "e2e-t3d.yaml");
    writeFileSync(path, source.replace("name: vru-grid", "name: e2e-t3d").replace("rate_veh_per_h: 1500.0", "rate_veh_per_h: 6000.0"));
  }
  engine = new EngineProcess(path);
  await engine.start();
});

test.afterAll(async () => {
  await engine?.stop();
});

interface PixelStats {
  frames: number;
  empty: number;
  minStd: number;
}

/**
 * Start the in-page hunt and the pixel sampler.
 *
 * The sampler reads the canvas right after the viewer draws a frame, from inside the viewer's own
 * `renderFrame`. A WebGL canvas without `preserveDrawingBuffer` is cleared once the frame is
 * composited, so a copy taken from a timer instead reads a blank buffer: the first version of this
 * sampler did that and counted 36 of 40 plainly drawn aerial frames as empty.
 */
async function startHunt(page: Page): Promise<void> {
  if (SHOTS_ONLY) return;
  await page.evaluate(() => {
    type V = { huntGlitches: () => unknown; canvas: HTMLCanvasElement; renderFrame: (t?: number) => unknown };
    const e = window.__vwpStudio?.engine as unknown as { viewer: V };
    const v = e.viewer;
    v.huntGlitches();
    const w = window as unknown as { __t3d?: { stats: PixelStats } };
    const probe = document.createElement("canvas");
    probe.width = 160;
    probe.height = 100;
    const ctx = probe.getContext("2d", { willReadFrequently: true });
    const stats: PixelStats = { frames: 0, empty: 0, minStd: Infinity };
    const sample = (): void => {
      const gl = v.canvas;
      if (!ctx || !gl) return;
      ctx.drawImage(gl, 0, 0, 160, 100);
      const d = ctx.getImageData(0, 0, 160, 100).data;
      let sum = 0;
      let sum2 = 0;
      const n = d.length / 4;
      for (let i = 0; i < d.length; i += 4) {
        const y = 0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2];
        sum += y;
        sum2 += y * y;
      }
      const mean = sum / n;
      const std = Math.sqrt(Math.max(0, sum2 / n - mean * mean));
      stats.frames++;
      if (std < 3 || mean < 4) stats.empty++;
      stats.minStd = Math.min(stats.minStd, std);
    };
    // Wrap this instance's renderFrame (the loop calls `this.renderFrame`); `stopHunt` deletes the
    // own property, which puts the class's method back.
    const draw = Object.getPrototypeOf(v).renderFrame as (this: V, t?: number) => unknown;
    let last = 0;
    v.renderFrame = function (this: V, t?: number) {
      const r = draw.call(this, t);
      const now = performance.now();
      if (now - last >= 250) {
        last = now;
        sample();
      }
      return r;
    };
    w.__t3d = { stats };
  });
}

async function stopHunt(page: Page): Promise<{ report: unknown; pixels: PixelStats }> {
  if (SHOTS_ONLY) return { report: null, pixels: { frames: 0, empty: 0, minStd: 0 } };
  return page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as { viewer: { stopGlitchHunt: () => unknown } };
    delete (e.viewer as unknown as { renderFrame?: unknown }).renderFrame;
    const w = window as unknown as { __t3d?: { stats: PixelStats } };
    return { report: e.viewer.stopGlitchHunt(), pixels: w.__t3d?.stats ?? { frames: 0, empty: 0, minStd: 0 } };
  });
}

async function shot(page: Page, name: string): Promise<void> {
  if (!SHOTS) return;
  mkdirSync(SHOTS, { recursive: true });
  await page.locator("canvas").first().screenshot({ path: join(SHOTS, `${name}.jpg`), type: "jpeg", quality: 70 });
}

/** A live vehicle to follow, preferring one that is moving. */
async function aVehicle(page: Page, want: (name: string, category: number) => boolean): Promise<number> {
  return page.evaluate((src) => {
    const pred = new Function("name", "category", `return (${src})(name, category);`) as (n: string, c: number) => boolean;
    const e = window.__vwpStudio?.engine as unknown as {
      viewer: {
        interpolator: { count: number; outOccupied: Uint8Array; outActorId: Uint32Array; outClassIdx: Uint8Array; outSpeed: Float32Array };
        actors: { classes: readonly { name: string; category: number }[] };
      };
    };
    const it = e.viewer.interpolator;
    let best = -1;
    let bestSpeed = -1;
    for (let s = 0; s < it.count; s++) {
      if (it.outOccupied[s] !== 1) continue;
      const def = e.viewer.actors.classes[it.outClassIdx[s]];
      if (!def || !pred(def.name, def.category)) continue;
      if (it.outSpeed[s] > bestSpeed) {
        bestSpeed = it.outSpeed[s];
        best = it.outActorId[s];
      }
    }
    return best;
  }, want.toString());
}

test("the traffic scene has no geometric glitch and no empty frame in aerial, chase and dashboard", async ({ page }) => {
  test.setTimeout(10 * 60_000);
  await open(page);
  await page.getByTestId("speed").selectOption("1");
  await page.getByTestId("play").click();
  await expect.poll(async () => (await status(page)).state, { timeout: 60_000 }).toBe("running");
  await expect
    .poll(() => page.evaluate(() => window.__vwpStudio?.actorCount() ?? 0), { timeout: 120_000 })
    .toBeGreaterThan(20);
  await page.waitForTimeout(4000);

  const results: Record<string, unknown> = {};
  if (!SHOTS_ONLY) {
    // The empty-frame check must be able to fail: with the scene hidden the canvas is one flat
    // colour, and the sampler has to say so. (Its first version read a cleared buffer and would
    // have called every frame empty, or, fixed the wrong way, none.)
    await startHunt(page);
    await page.evaluate(() => {
      (window.__vwpStudio?.engine as unknown as { viewer: { scene: { visible: boolean } } }).viewer.scene.visible = false;
    });
    await page.waitForTimeout(1500);
    await page.evaluate(() => {
      (window.__vwpStudio?.engine as unknown as { viewer: { scene: { visible: boolean } } }).viewer.scene.visible = true;
    });
    const blank = await stopHunt(page);
    expect(blank.pixels.frames, "sampler ran while the scene was hidden").toBeGreaterThan(2);
    expect(blank.pixels.empty, "a hidden scene reads as empty frames").toBeGreaterThan(0);
  }
  // Aerial.
  await startHunt(page);
  await page.waitForTimeout(SECONDS * 1000);
  await shot(page, "aerial");
  results.aerial = await stopHunt(page);

  // Chase: a car, then a person.
  const car = await aVehicle(page, (n, c) => c === 0 && (n === "passenger" || n === "car"));
  expect(car).toBeGreaterThanOrEqual(0);
  await page.evaluate((id) => (window.__vwpStudio?.engine as unknown as { selectActor(i: number, m: string): Promise<void> }).selectActor(id, "chase"), car);
  await page.waitForTimeout(3000);
  await startHunt(page);
  await page.waitForTimeout(SECONDS * 1000);
  await shot(page, "chase-car");
  results.chase = await stopHunt(page);

  const person = await aVehicle(page, (n) => n === "pedestrian");
  if (person >= 0) {
    await page.evaluate((id) => (window.__vwpStudio?.engine as unknown as { selectActor(i: number, m: string): Promise<void> }).selectActor(id, "chase"), person);
    await page.waitForTimeout(3000);
    await startHunt(page);
    await page.waitForTimeout(SECONDS * 1000);
    await shot(page, "chase-pedestrian");
    results.chasePedestrian = await stopHunt(page);
  }

  // A two-wheeler, when the scenario has one: the lean into turns and the rider are only seen here.
  const rider = await aVehicle(page, (n) => ["bicycle", "motorcycle", "moped", "scooter", "moto"].includes(n));
  if (rider >= 0) {
    await page.evaluate((id) => (window.__vwpStudio?.engine as unknown as { selectActor(i: number, m: string): Promise<void> }).selectActor(id, "chase"), rider);
    await page.waitForTimeout(3000);
    await startHunt(page);
    await page.waitForTimeout(SECONDS * 1000);
    await shot(page, "chase-two-wheeler");
    results.chaseTwoWheeler = await stopHunt(page);
  }

  // Dashboard.
  const driver = await aVehicle(page, (n, c) => c === 0 && n !== "bicycle");
  await page.evaluate((id) => (window.__vwpStudio?.engine as unknown as { selectActor(i: number, m: string): Promise<void> }).selectActor(id, "dashboard"), driver);
  await page.waitForTimeout(3000);
  await startHunt(page);
  await page.waitForTimeout(SECONDS * 1000);
  await shot(page, "dashboard");
  results.dashboard = await stopHunt(page);

  if (process.env.VWP_T3D_REPORT) writeFileSync(process.env.VWP_T3D_REPORT, JSON.stringify(results, null, 2));
  if (SHOTS_ONLY) return;
  for (const [view, r] of Object.entries(results)) {
    const { report, pixels } = r as { report: { counts: Record<string, number>; engineCaused: Record<string, number>; frames: number }; pixels: PixelStats };
    expect(report.frames, `${view}: frames hunted`).toBeGreaterThan(30);
    expect(pixels.frames, `${view}: frames sampled`).toBeGreaterThan(10);
    expect(pixels.empty, `${view}: empty frames (min luminance σ ${pixels.minStd.toFixed(1)})`).toBe(0);
    for (const [cls, n] of Object.entries(report.counts)) {
      const viewerCaused = n - (report.engineCaused[cls] ?? 0);
      expect.soft(viewerCaused, `${view}: ${cls}`).toBe(0);
    }
  }
});
