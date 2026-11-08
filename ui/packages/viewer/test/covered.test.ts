/**
 * A viewer whose canvas is covered draws nothing, and keeps time.
 *
 * The Studio's full-screen panels (Metrics, the certificate system, Settings) cover the whole
 * viewport, and the loop went on drawing under them every frame. Under a software rasteriser that
 * drawing took the time the panel over it needed to lay out and paint: the end-to-end suite timed
 * out on panels that never became visible or stable. Covered, the loop advances the clock, the
 * poses and the camera and submits nothing; uncovered, it draws on the next frame. A frame asked
 * for by hand (`renderFrame`, `step`) always draws.
 */

import { describe, expect, it } from "vitest";
import { Viewer } from "../src/scene.js";
import type { FrameScheduler, ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";

const CANVAS = { width: 800, height: 600, clientWidth: 800, clientHeight: 600 } as unknown as ViewerCanvas;

/** A scheduler driven by hand: `frame()` runs the pending callback 16 ms after the last. */
function manual(): FrameScheduler & { frame(): void } {
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
    frame: () => {
      now += 16;
      const cb = pending;
      pending = null;
      cb?.(now);
    },
  };
}

function viewer(): { viewer: Viewer; renderer: () => NullRenderer; clock: ReturnType<typeof manual> } {
  const clock = manual();
  let r: NullRenderer | null = null;
  const v = new Viewer({
    canvas: CANVAS,
    theme: "dark",
    autoStart: false,
    scheduler: clock,
    createRenderer: (c) => (r = new NullRenderer(c)),
  });
  return { viewer: v, renderer: () => r!, clock };
}

describe("Viewer.covered", () => {
  it("the loop submits nothing while covered and keeps the clock moving", () => {
    const { viewer: v, renderer, clock } = viewer();
    v.start();
    for (let i = 0; i < 5; i++) clock.frame();
    const drawn = renderer().renders;
    expect(drawn).toBe(5);
    v.covered = true;
    const before = v.lastFrame.clockSeconds;
    for (let i = 0; i < 30; i++) clock.frame();
    expect(renderer().renders, "frames drawn under the cover").toBe(drawn);
    expect(v.lastFrame.clockSeconds, "the clock advanced under the cover").toBeGreaterThan(before + 0.4);
    v.covered = false;
    clock.frame();
    expect(renderer().renders, "drawn on the first frame after uncovering").toBe(drawn + 1);
    v.dispose();
  });

  it("a drawn frame of the loop goes through renderFrame, which a harness may wrap", () => {
    const { viewer: v, clock } = viewer();
    let seen = 0;
    const draw = v.renderFrame.bind(v);
    v.renderFrame = (t?: number) => {
      seen++;
      return draw(t);
    };
    v.start();
    for (let i = 0; i < 3; i++) clock.frame();
    v.covered = true;
    for (let i = 0; i < 3; i++) clock.frame();
    expect(seen).toBe(3);
    v.dispose();
  });

  it("a frame asked for by hand draws even while covered", () => {
    const { viewer: v, renderer } = viewer();
    v.covered = true;
    const before = renderer().renders;
    v.step(0);
    v.renderFrame();
    expect(renderer().renders).toBe(before + 2);
    v.dispose();
  });
});
