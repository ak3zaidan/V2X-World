/**
 * An actor that leaves the stream — a car driving off the edge of the map, a trip's end — fades out
 * over {@link FADE_SECONDS}. While it fades it has to keep moving as it was: held where its last
 * snapshot put it, a car leaving at 13 m/s stopped dead in one frame and dissolved standing still,
 * which a chase camera shows as a jerk of several pixels on a car that is not the one followed (the
 * glitch hunter's `stutter`, seen in the Studio's traffic-scene e2e).
 */

import { describe, expect, it } from "vitest";
import { FADE_SECONDS, PoseInterpolator } from "../src/interp.js";
import { placedPoses, type PlacedActor } from "./support/one-actor.js";

const STEP = 0.1;

/** A car on a straight road along +x at `v` m/s, and one that stays, parked beside it. */
function scene(t: number, leaving: boolean, v = 13): PlacedActor[] {
  const stays: PlacedActor = { actorId: 7, x: 0, y: 20, headingRad: 0, speedMps: 0 };
  const goes: PlacedActor = { actorId: 9, x: v * t, y: 0, headingRad: 0, speedMps: v };
  // The departing car is in slot 1, so the one that stays keeps slot 0 when it is gone.
  return leaving ? [stays] : [stays, goes];
}

interface Fade {
  /** Frames drawn while the departing car was fading, 0 < fade < 1. */
  readonly frames: number;
  /** Smallest and largest distance it moved per sim second over those frames, m/s. */
  readonly minSpeed: number;
  readonly maxSpeed: number;
}

/** Drive the car off at `leaveAt` s and sample at `fps`, measuring its motion while it fades. */
function departure(fps: number, leaveAt = 2, v = 13): Fade {
  const interp = new PoseInterpolator();
  let next = 0;
  let frames = 0;
  let minSpeed = Infinity;
  let maxSpeed = 0;
  let prev: { x: number; sim: number } | null = null;
  const end = leaveAt + 1;
  for (let clock = 0; clock < end; clock += 1 / fps) {
    while (next * STEP <= clock) {
      const t = next * STEP;
      interp.capture(placedPoses(scene(t, t > leaveAt + 1e-9, v), BigInt(Math.round(t * 1e9))), t);
      next++;
    }
    const info = interp.sample(clock);
    if (interp.outOccupied[1] !== 1 || interp.outActorId[1] !== 9) {
      prev = null;
      continue;
    }
    const x = interp.outPosition[3];
    const fade = interp.outFade[1];
    const sim = info.renderSimSeconds;
    if (prev && fade > 0 && fade < 1 && sim - prev.sim > 1e-6) {
      const speed = (x - prev.x) / (sim - prev.sim);
      frames++;
      minSpeed = Math.min(minSpeed, speed);
      maxSpeed = Math.max(maxSpeed, speed);
    }
    prev = { x, sim };
  }
  return { frames, minSpeed, maxSpeed };
}

describe("PoseInterpolator — an actor leaving the stream", () => {
  it("keeps moving at its own speed while it fades out, at 60 fps and at a software renderer's 5", () => {
    for (const fps of [60, 5]) {
      const r = departure(fps);
      // eslint-disable-next-line no-console
      console.log(`${fps} fps: ${r.frames} fading frames, ${r.minSpeed.toFixed(2)}–${r.maxSpeed.toFixed(2)} m/s (drives at 13)`);
      // The fade is FADE_SECONDS of sim time: at 60 fps that is many frames, at 5 fps one or two.
      expect(r.frames, `${fps} fps: frames drawn while fading`).toBeGreaterThanOrEqual(fps === 60 ? Math.floor(FADE_SECONDS * 60) - 2 : 1);
      // Held at its last pose, it moved 0 m/s on every fading frame (the jerk the hunter counted).
      expect(r.minSpeed, `${fps} fps: slowest fading frame`).toBeGreaterThan(13 * 0.9);
      expect(r.maxSpeed, `${fps} fps: fastest fading frame`).toBeLessThan(13 * 1.1);
    }
  });
});
