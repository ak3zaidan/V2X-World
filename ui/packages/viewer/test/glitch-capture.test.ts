/**
 * The glitch hunter over a real engine capture.
 *
 *   VWP_GLITCH_CAPTURE=<dir with stream.vwpcap and world.vwb> \
 *   VWP_GLITCH_REPORT=<file.json> pnpm --filter @vwp/viewer exec vitest run test/glitch-capture.test.ts
 *
 * Skipped without a capture (the capture is tens of megabytes and comes from a running engine:
 * `scripts/capture-stream.mjs`). With one, it tours aerial, chase (a car, the largest vehicle, a
 * person, a two-wheeler) and dashboard, writes the per-class report, and fails on any glitch class
 * that the viewer — not the engine's own data — caused, outside the allowances written below.
 *
 * `VWP_GLITCH_EXPECT=report` only reports (the BEFORE measurement is taken that way).
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { GLITCH_CLASSES, type GlitchClass } from "../src/glitch.js";
import { StreamReplay, readCapture } from "./glitch/replay.js";
import { runTour } from "./glitch/tour.js";

const dir = process.env.VWP_GLITCH_CAPTURE;
const have = dir !== undefined && existsSync(join(dir, "stream.vwpcap"));

describe.skipIf(!have)("glitch hunter — real engine capture", () => {
  it("tours aerial, chase and dashboard and reports every glitch class", () => {
    const frames = readCapture(readFileSync(join(dir!, "stream.vwpcap")));
    const world = readFileSync(join(dir!, "world.vwb"));
    const replay = new StreamReplay(world, frames, {
      seed: Number(process.env.VWP_GLITCH_SEED ?? "24593"),
      logarithmicDepthBuffer: process.env.VWP_GLITCH_LOGDEPTH === "1" ? true : undefined,
    });
    const t0 = performance.now();
    const secs = Number(process.env.VWP_GLITCH_SECONDS ?? "20");
    const result = runTour(replay, {
      aerialS: secs, chaseS: secs * 0.75, dashboardS: secs, verbose: process.env.VWP_GLITCH_VERBOSE === "1",
    });
    const wallS = (performance.now() - t0) / 1000;
    const r = result.report;
    const out = {
      capture: dir,
      frames: frames.length,
      simSeconds: replay.lastSimS - replay.firstSimS,
      subjects: result.subjects,
      wallSeconds: wallS,
      ...r,
    };
    if (process.env.VWP_GLITCH_REPORT) writeFileSync(process.env.VWP_GLITCH_REPORT, JSON.stringify(out, null, 2));
    const line = GLITCH_CLASSES.map((c) => `${c}=${r.counts[c]}${r.engineCaused[c] ? `(${r.engineCaused[c]} engine)` : ""}`).join(" ");
    process.stderr.write(`glitch report: ${r.frames} frames ${JSON.stringify(r.framesByMode)}\n${line}\n`);
    expect(r.frames).toBeGreaterThan(100);
    if (process.env.VWP_GLITCH_EXPECT === "report") return;
    // Viewer-caused events must be zero in every class.
    const viewerCaused = (c: GlitchClass): number => r.counts[c] - (r.engineCaused[c] ?? 0);
    for (const c of GLITCH_CLASSES) {
      expect.soft(viewerCaused(c), `${c}: ${JSON.stringify(r.examples[c]?.slice(0, 3))}`).toBe(0);
    }
  });
});
