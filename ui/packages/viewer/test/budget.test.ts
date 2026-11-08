/**
 * The performance-budget measurement (60 fps with 5,000 rendered vehicles).
 *
 * Run it with a forced garbage collector for a trustworthy allocation number:
 *
 * ```sh
 * pnpm --filter @vwp/viewer bench      # NODE_OPTIONS=--expose-gc vitest run test/budget.test.ts
 * ```
 *
 * Without `--expose-gc` the heap delta is still printed but is noise, and the strict assertion is
 * skipped rather than asserted on a number nobody can trust.
 *
 * The same holds for time. The counts (vehicles drawn, instance capacity) are asserted on every run.
 * The CPU budget is a time, and time depends on the machine's load: two consecutive runs of this
 * test on a shared 8-core machine at load average 7.6 measured 7.8 ms and 3.6 ms of wall-clock per
 * frame (p99 26.6 ms and 9.8 ms) for the same work, and the old wall-clock assertion failed under
 * load with nothing wrong in the code. So it is asserted only where a time means something: on a quiet
 * machine (load average under half the cores, normal priority), or always with
 * `VWP_PERF_STRICT=1`, which `pnpm --filter @vwp/viewer bench` sets. It is measured as this
 * process's CPU time (`process.cpuUsage`), not wall-clock, so time spent descheduled never counts.
 * When it is not asserted the test says why, beside the number.
 */

import { availableParallelism, getPriority, loadavg } from "node:os";
import { describe, expect, it } from "vitest";
import { Viewer } from "../src/scene.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { SyntheticStream, makeGridWorld } from "./support/fixture.js";

const CANVAS = { width: 1920, height: 1080, clientWidth: 1920, clientHeight: 1080 } as unknown as ViewerCanvas;
const gc = (globalThis as { gc?: () => void }).gc;

/** Why the CPU budget cannot be asserted on this run, or null when it can. */
function timingUntrusted(): string | null {
  if (process.env.VWP_PERF_STRICT === "1") return null;
  const load = loadavg()[0];
  const cores = availableParallelism();
  if (load > cores / 2) return `load average ${load.toFixed(1)} on ${cores} cores`;
  let priority = 0;
  try {
    priority = getPriority();
  } catch {
    /* unknown: treat as normal */
  }
  if (priority > 0) return `running at nice ${priority}`;
  return null;
}

function percentile(sorted: Float64Array, q: number): number {
  const i = Math.min(sorted.length - 1, Math.max(0, Math.round(q * (sorted.length - 1))));
  return sorted[i];
}

describe("performance budget", () => {
  it("draws 5,000 vehicles for 600 frames inside the CPU budget and without churning the heap", () => {
    const viewer = new Viewer({
      canvas: CANVAS,
      theme: "dark",
      autoStart: false,
      createRenderer: (c) => new NullRenderer(c),
    });
    const grid = makeGridWorld({ blocks: 16, blockM: 120, buildingsPerBlock: 3 });
    viewer.setWorld(grid.world);
    const stream = new SyntheticStream(5000, grid);
    stream.keyframe();
    viewer.capture(stream.poses, 0);

    // Frame the whole crowd: every one of the 5,000 is inside the frustum and gets an instance.
    viewer.cameras.focusOn(0, 0, 0);
    viewer.cameras.altitudeM = 2400;
    viewer.cameras.snap();

    const FRAMES = 600;
    const FRAME_MS = 1000 / 60;
    let t = 0;
    const step = (): void => {
      t += FRAME_MS;
      if (Math.round(t / FRAME_MS) % 6 === 0) {
        stream.advance(0.1);
        stream.delta();
        viewer.capture(stream.poses, t / 1000);
      }
      viewer.renderFrame(t);
    };

    for (let i = 0; i < 60; i++) step(); // warm-up: bucket growth and JIT
    viewer.stats.reset();

    gc?.();
    const heapBefore = process.memoryUsage().heapUsed;
    const capacityBefore = viewer.actors.allocatedCapacity;
    const samples = new Float64Array(FRAMES);
    let drawn = 0;
    const cpuBefore = process.cpuUsage();
    for (let i = 0; i < FRAMES; i++) {
      const a = performance.now();
      step();
      samples[i] = performance.now() - a;
      drawn += viewer.lastFrame.actorsDrawn;
    }
    const cpu = process.cpuUsage(cpuBefore);
    const cpuMs = (cpu.user + cpu.system) / 1000 / FRAMES;
    const untrusted = timingUntrusted();
    const capacityAfter = viewer.actors.allocatedCapacity;
    gc?.();
    const heapAfter = process.memoryUsage().heapUsed;

    const sorted = Float64Array.from(samples).sort();
    let sum = 0;
    for (const v of samples) sum += v;
    const snap = viewer.stats.snapshot();
    const heapMb = (heapAfter - heapBefore) / 1024 / 1024;

    // eslint-disable-next-line no-console
    console.log(
      [
        "",
        `frames:        ${FRAMES} at a simulated 60 fps, 10 Hz deltas`,
        `actors drawn:  ${(drawn / FRAMES).toFixed(0)} per frame of ${stream.count} live`,
        `wall/frame:    mean ${(sum / FRAMES).toFixed(3)} ms, p50 ${percentile(sorted, 0.5).toFixed(3)}, ` +
        `p95 ${percentile(sorted, 0.95).toFixed(3)}, p99 ${percentile(sorted, 0.99).toFixed(3)}, ` +
        `max ${percentile(sorted, 1).toFixed(3)}`,
        `viewer CPU:    mean ${snap.cpuMeanMs.toFixed(3)} ms (interpolate + cull + LOD + matrix write)`,
        `process CPU:   ${cpuMs.toFixed(3)} ms/frame (budget 8; ${untrusted === null ? "asserted" : `not asserted: ${untrusted}`})`,
        `draw calls:    ${snap.drawCalls}`,
        `instance cap:  ${capacityBefore} → ${capacityAfter}`,
        `heap delta:    ${heapMb.toFixed(2)} MB${gc ? " (after forced GC)" : " (no --expose-gc: noise)"}`,
        "",
      ].join("\n"),
    );

    expect(drawn / FRAMES).toBeGreaterThan(4900);
    expect(capacityAfter).toBe(capacityBefore);
    // The CPU half of the frame must leave the GPU its share of a 16.7 ms budget.
    if (untrusted === null) expect(cpuMs).toBeLessThan(8);
    if (gc) expect(Math.abs(heapMb)).toBeLessThan(4);
    viewer.dispose();
  });
});
