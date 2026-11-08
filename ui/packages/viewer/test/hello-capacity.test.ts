/**
 * `Hello.actor_capacity` is a bound and "a preallocation hint" (§3.1.1), not an allocation.
 *
 * The engine announces 2^20 slots unless a scenario states its fleet. Taken literally, the
 * interpolator's columns alone came to about 250 MB, and with the pose and slot buffers the
 * Manhattan page held 410 MB of array buffers while drawing a few hundred actors (heap snapshot,
 * 2026-09-29). The viewer now preallocates a few thousand slots and grows with the traffic.
 */

import { describe, expect, it } from "vitest";
import type { HelloMessage } from "@vwp/protocol";
import { Viewer } from "../src/scene.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { SyntheticStream, makeGridWorld } from "./support/fixture.js";

const CANVAS = { width: 640, height: 360, clientWidth: 640, clientHeight: 360 } as unknown as ViewerCanvas;

/** The fields `applyHello` reads, as the engine's default `Hello` sets them. */
function hello(actorCapacity: number): HelloMessage {
  return {
    actorCapacity,
    mobilityStepNs: 100_000_000n,
    helloFlags: 0x01,
    runId: new Uint8Array(16),
    strings: [""],
    classes: {
      count: 0,
      strName: new Uint32Array(0),
      lengthM: new Float32Array(0),
      widthM: new Float32Array(0),
      heightM: new Float32Array(0),
      colorRgba: new Uint32Array(0),
      category: new Uint8Array(0),
    },
  } as unknown as HelloMessage;
}

describe("§3.1.1 — actor_capacity is a bound and a hint, not an allocation", () => {
  it("a Hello announcing 2^20 slots preallocates a few thousand, and the traffic grows it", () => {
    const viewer = new Viewer({
      canvas: CANVAS,
      theme: "dark",
      autoStart: false,
      createRenderer: (c) => new NullRenderer(c),
    });
    viewer.applyHello(hello(1 << 20));
    expect(viewer.interpolator.capacity).toBeLessThanOrEqual(4096);

    // A run denser than the preallocation: every actor is still drawn.
    const grid = makeGridWorld({ blocks: 16, blockM: 120, buildingsPerBlock: 1 });
    viewer.setWorld(grid.world);
    const stream = new SyntheticStream(6000, grid);
    stream.keyframe();
    viewer.capture(stream.poses, 0);
    viewer.interpolator.sample(0);
    expect(viewer.interpolator.capacity).toBeGreaterThanOrEqual(6000);
    expect(viewer.interpolator.count).toBe(6000);
  });

  it("a small announced capacity is still preallocated in full", () => {
    const viewer = new Viewer({
      canvas: CANVAS,
      theme: "dark",
      autoStart: false,
      createRenderer: (c) => new NullRenderer(c),
    });
    viewer.applyHello(hello(3000));
    expect(viewer.interpolator.capacity).toBeGreaterThanOrEqual(3000);
  });
});
