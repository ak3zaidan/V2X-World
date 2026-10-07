/**
 * A person in a crowd zig-zags: the social-force model's 0.1 s steps swing their velocity side to
 * side and, now and then, back. The drawn path through those positions must still be continuous
 * in velocity at every snapshot. Before, each knot's tangent was limited only against the segment
 * being drawn, so a step that went back against one neighbour's chord got a zero tangent at the
 * end of one segment and a full one at the start of the next: a kink ten times a second, which the
 * glitch hunter counted as people near a chase camera jerking several pixels on the dense Midtown
 * capture.
 */

import { describe, expect, it } from "vitest";
import { PoseInterpolator } from "../src/interp.js";
import { placedPoses } from "./support/one-actor.js";

const STEP = 0.1;

describe("a person's drawn path", () => {
  it("has no velocity jump at a snapshot when the person zig-zags", () => {
    const interp = new PoseInterpolator();
    interp.setVruClasses(Uint8Array.from([1]));
    // 1.3 m/s along +x, ±3 cm sideways at alternate steps, and every fourth step 15 cm back.
    const at = (k: number): { x: number; y: number } => ({
      x: k * STEP * 1.3 - (k % 4 === 3 ? 0.15 : 0),
      y: k % 2 === 0 ? 0.03 : -0.03,
    });
    const N = 40;
    let next = 0;
    let prev: { s: number; x: number; y: number } | null = null;
    let prevV: [number, number] | null = null;
    let worst = 0;
    const dt = 0.001;
    for (let clock = 0; clock < N * STEP; clock += dt) {
      while (next <= N && next * STEP <= clock) {
        const p = at(next);
        interp.capture(placedPoses([{ actorId: 1, x: p.x, y: p.y, headingRad: 0, speedMps: 1.3, classIdx: 0 }], BigInt(Math.round(next * STEP * 1e9))), next * STEP);
        next++;
      }
      const info = interp.sample(clock);
      const s = info.renderSimSeconds;
      const x = interp.outPosition[0];
      const y = interp.outPosition[1];
      if (clock < 6 * STEP) continue;
      if (prev && s - prev.s > 2e-4) {
        const v: [number, number] = [(x - prev.x) / (s - prev.s), (y - prev.y) / (s - prev.s)];
        if (prevV) worst = Math.max(worst, Math.hypot(v[0] - prevV[0], v[1] - prevV[1]));
        prevV = v;
        prev = { s, x, y };
      } else if (!prev) {
        prev = { s, x, y };
      }
    }
    // Continuous velocity changes by accel × ~1 ms between samples: hundredths of a m/s. A kink is
    // a jump of the order of the walking speed.
    expect(worst).toBeLessThan(0.15);
  });
});
