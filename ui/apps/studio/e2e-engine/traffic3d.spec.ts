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

/** Start the in-page hunt and the pixel sampler. */
async function startHunt(page: Page): Promise<void> {
  if (SHOTS_ONLY) return;
  await page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as { viewer: { huntGlitches: () => unknown; canvas: HTMLCanvasElement } };
    e.viewer.huntGlitches();
    const w = window as unknown as { __t3d?: { stats: PixelStats; timer: number } };
    const probe = document.createElement("canvas");
    probe.width = 160;
    probe.height = 100;
    const ctx = probe.getContext("2d", { willReadFrequently: true });
    const stats: PixelStats = { frames: 0, empty: 0, minStd: Infinity };
    const timer = window.setInterval(() => {
      const gl = e.viewer.canvas;
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
    }, 250);
    w.__t3d = { stats, timer };
  });
}

async function stopHunt(page: Page): Promise<{ report: unknown; pixels: PixelStats }> {
  if (SHOTS_ONLY) return { report: null, pixels: { frames: 0, empty: 0, minStd: 0 } };
  return page.evaluate(() => {
    const e = window.__vwpStudio?.engine as unknown as { viewer: { stopGlitchHunt: () => unknown } };
    const w = window as unknown as { __t3d?: { stats: PixelStats; timer: number } };
    if (w.__t3d) window.clearInterval(w.__t3d.timer);
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
