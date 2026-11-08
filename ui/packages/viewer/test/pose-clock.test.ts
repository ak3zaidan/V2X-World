/**
 * The interpolator's clock keeps the stream's time across a frame the viewer does not honour.
 *
 * The viewer holds a frame's step to `maxFrameSeconds` (0.5 s), and its fixed-step pose clock with
 * it. The pose stream arrives on the wall clock and does not stop for a long frame. Run on the pose
 * clock, the interpolator saw the arrivals a long frame held up dated early by the time the frame
 * dropped; its rate estimate, fitted over 4 s of arrivals across that step, read the stream fast;
 * its clock map ran ahead of the stream; and for the next few seconds the render clock pressed
 * against the newest snapshot and advanced with each arrival, 0.1 s of sim time on one frame and
 * 0.2 s on the next. The frames here are the ones traffic3d's chase hunt sees under SwiftShader
 * (0.2–0.25 s), the stream the 0.88x an engine reaches there as traffic builds.
 */

import { describe, expect, it } from "vitest";
import { Viewer } from "../src/scene.js";
import type { FrameScheduler, ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { placedPoses } from "./support/one-actor.js";

const CANVAS = { width: 800, height: 600, clientWidth: 800, clientHeight: 600 } as unknown as ViewerCanvas;

/** A scheduler whose clock the test sets: `at(ms)` moves it, `frame(ms)` runs the pending frame then. */
function manual(): FrameScheduler & { at(ms: number): void; frame(ms: number): void } {
  let now = 0;
  let pending: ((t: number) => void) | null = null;
  return {
    request: (cb) => {
      pending = cb;
      return 1;
    },
    cancel: () => {
      pending = null;
    },
    now: () => now,
    at: (ms) => {
      now = ms;
    },
    frame: (ms) => {
      now = ms;
      const cb = pending;
      pending = null;
      cb?.(now);
    },
  };
}

interface Trace {
  /** Sim seconds the drawn clock advanced per frame, over the stream's rate times the frame's length. */
  readonly pace: number[];
  /** Drawn sim time minus the newest snapshot's, per frame. */
  readonly ahead: number[];
}

/**
 * A 0.88x stream (a 0.1 s step every 0.114 s, a few ms of arrival jitter), drawn at 4–5 fps with
 * one frame of `hitchS` at 10 s; traced from 2 s after the hitch to 7 s after it.
 */
function run(hitchS: number): Trace {
  const clock = manual();
  const v = new Viewer({
    canvas: CANVAS,
    theme: "dark",
    autoStart: false,
    scheduler: clock,
    createRenderer: (c) => new NullRenderer(c),
  });
  const rate = 0.88;
  const arrivals: { ms: number; sim: number }[] = [];
  for (let k = 1; k * 0.1 / rate < 25; k++) arrivals.push({ ms: ((k * 0.1) / rate) * 1000 + ((k * 7) % 11), sim: k * 0.1 });
  const frames = [200, 217, 183, 233, 200, 250];
  v.capture(placedPoses([{ actorId: 1, x: 0, y: 0, speedMps: 10 }], 0n));
  v.start();
  const pace: number[] = [];
  const ahead: number[] = [];
  let wall = 0;
  let ai = 0;
  let newest = 0;
  let prev = Number.NaN;
  let hitched = -1;
  for (let k = 0; wall < 18_000; k++) {
    const long = hitched < 0 && wall >= 10_000;
    const d = long ? hitchS * 1000 : frames[k % frames.length];
    if (long) hitched = wall + d;
    while (ai < arrivals.length && arrivals[ai].ms <= wall + d) {
      const a = arrivals[ai++];
      clock.at(a.ms);
      v.capture(placedPoses([{ actorId: 1, x: a.sim * 10, y: 0, speedMps: 10 }], BigInt(Math.round(a.sim * 1e9))));
      newest = a.sim;
    }
    wall += d;
    clock.frame(wall);
    const sim = v.interpolator.renderSimSeconds;
    if (hitched >= 0 && wall >= hitched + 2000 && wall <= hitched + 7000) {
      pace.push((sim - prev) / ((d / 1000) * rate));
      ahead.push(sim - newest);
    }
    prev = sim;
  }
  v.dispose();
  return { pace, ahead };
}

describe("Viewer's pose clock", () => {
  it("keeps the stream's time across a frame longer than maxFrameSeconds", () => {
    for (const hitch of [0.9, 1.5]) {
      const { pace, ahead } = run(hitch);
      const lo = Math.min(...pace);
      const hi = Math.max(...pace);
      const most = Math.max(...ahead);
      // eslint-disable-next-line no-console
      console.log(`${hitch} s frame: drawn pace ${lo.toFixed(2)}–${hi.toFixed(2)} of the stream's, at most ${(most * 1000).toFixed(0)} ms past the newest snapshot`);
      expect(pace.length).toBeGreaterThan(20);
      // Measured before the fix, 0.9 s frame: drawn 115 ms past the newest snapshot (the 120 ms
      // extrapolation limit). After: 26 ms behind it at most, pace 1.00–1.10 (0.99–1.01 at 1.5 s).
      expect(lo, `${hitch} s frame: slowest frame's pace`).toBeGreaterThan(0.85);
      expect(hi, `${hitch} s frame: fastest frame's pace`).toBeLessThan(1.15);
      expect(most, `${hitch} s frame: drawn past the newest snapshot`).toBeLessThan(0.02);
    }
  });
});
