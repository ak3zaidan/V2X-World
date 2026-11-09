/**
 * The stutter rule judges motion as steady in the camera's frame, not on the screen.
 *
 * Perspective bends a straight, steady path on screen: an oncoming car passing a chase camera a few
 * metres off sweeps across the picture faster every frame, as 1/depth. At the four or five frames a
 * second a software renderer draws, one frame's change of that sweep is more than half the sweep,
 * so a rule on the screen-space second difference counted a car drawn exactly where it was as a
 * stutter: the Studio's traffic-scene e2e, chasing a cyclist, reported one such "stutter" on an
 * oncoming passenger car now and then (the same car replayed from the engine's stream: 10.4 px at
 * 8.5 m from the camera). A real stutter — a frame frozen in the middle of that same pass — still
 * counts.
 */

import { describe, expect, it } from "vitest";
import { Viewer } from "../src/scene.js";
import { GlitchHunter, type GlitchEvent } from "../src/glitch.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { makeGridWorld } from "./support/fixture.js";
import { placedPoses } from "./support/one-actor.js";

const CANVAS = { width: 1500, height: 950, clientWidth: 1500, clientHeight: 950 } as unknown as ViewerCanvas;
const STEP = 0.1;
/** A software renderer's frame: 4 frames a second. */
const FRAME_S = 0.25;
const RIDER = 1;
const ONCOMING = 2;

interface Pass {
  /** Stutter events on the oncoming car. */
  readonly onOncoming: GlitchEvent[];
  /** How close the car came to the camera, metres. */
  readonly closestM: number;
  /** Frames hunted. */
  readonly frames: number;
}

/**
 * A cyclist riding east at 5.5 m/s, followed by the chase camera, and a car coming the other way at
 * 9.4 m/s in the next lane, `lateral` metres across: the car passes the camera. `startX` moves the
 * car's start, which moves the pass against the frames. `freezeAt` holds the car's drawn pose for
 * one frame at that sim time (a stutter the viewer could cause).
 */
function oncomingPass(lateral: number, startX: number, freezeAt = Number.NaN): Pass {
  const viewer = new Viewer({ canvas: CANVAS, theme: "dark", autoStart: false, createRenderer: (c) => new NullRenderer(c) });
  const grid = makeGridWorld({ blocks: 4, blockM: 400, buildingsPerBlock: 0 });
  viewer.setWorld(grid.world);
  const y0 = grid.axes[1];
  const x0 = grid.axes[0] + 60;
  const at = (t: number) => placedPoses(
    [
      { actorId: RIDER, x: x0 + 5.5 * t, y: y0 - 2, headingRad: 0, speedMps: 5.5, classIdx: 4 },
      { actorId: ONCOMING, x: x0 + startX - 9.4 * t, y: y0 - 2 + lateral, headingRad: Math.PI, speedMps: 9.4, classIdx: 0 },
    ],
    BigInt(Math.round(t * 1e9)),
  );
  let next = 0;
  const deliver = (clock: number): void => {
    while (next * STEP <= clock + 1e-9) {
      viewer.capture(at(next * STEP), next * STEP);
      next++;
    }
  };
  deliver(0);
  viewer.renderFrame(0);
  viewer.flyTo(RIDER, "chase", true);
  const hunter = new GlitchHunter(viewer);
  const events: GlitchEvent[] = [];
  hunter.onEvent((e) => {
    if (e.cls === "stutter" && e.actorId === ONCOMING) events.push(e);
  });
  let closest = Infinity;
  let frozen: number[] | null = null;
  for (let k = 1; k * FRAME_S <= 14; k++) {
    const clock = k * FRAME_S;
    deliver(clock);
    viewer.renderFrame(clock * 1000);
    const it = viewer.interpolator;
    const sim = it.renderSimSeconds;
    for (let s = 0; s < it.count; s++) {
      if (it.outOccupied[s] !== 1 || it.outActorId[s] !== ONCOMING) continue;
      if (frozen !== null && Math.abs(sim - freezeAt) < FRAME_S / 2) {
        it.outPosition[s * 3] = frozen[0];
        it.outPosition[s * 3 + 1] = frozen[1];
      }
      frozen = [it.outPosition[s * 3], it.outPosition[s * 3 + 1]];
      const cam = viewer.camera.position;
      closest = Math.min(closest, Math.hypot(it.outPosition[s * 3] - cam.x, it.outPosition[s * 3 + 1] - cam.y));
    }
    if (clock > 2) hunter.afterFrame();
  }
  return { onOncoming: events, closestM: closest, frames: hunter.report().frames };
}

describe("GlitchHunter — perspective is not stutter", () => {
  it("does not count an oncoming car passing a chase camera at four frames a second", () => {
    const seen: string[] = [];
    // The pass at six phases against the frames, in the next lane and one further across.
    for (const lateral of [3.5, 5]) {
      for (const startX of [120, 120.6, 121.2, 121.8, 122.4, 123]) {
        const r = oncomingPass(lateral, startX);
        expect(r.frames).toBeGreaterThan(40);
        // The car comes within a few metres of the camera, where perspective bends its path most.
        expect(r.closestM).toBeLessThan(8);
        for (const e of r.onOncoming) seen.push(`${lateral} m across, start ${startX}: ${e.detail}`);
      }
    }
    // Judged on the screen-space second difference, 7 of these 12 passes were one "stutter" each,
    // 5 to 12 px on a car drawn exactly on its path.
    expect(seen).toEqual([]);
  });

  it("still counts a frame frozen in the middle of the same pass", () => {
    const r = oncomingPass(3.5, 120, 7);
    expect(r.onOncoming.length, "the frozen frame is a stutter").toBeGreaterThan(0);
  });
});
