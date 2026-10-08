/**
 * "Some intersections have poles in the middle of the road."
 *
 * A mast-arm pole was put one footway past its own edge's outermost lane. A street drawn as two
 * side-by-side edges going the same way then got the left edge's pole between them, in a lane;
 * and a stop line on a cross street's edge put the pole on the cross street. On the Midtown and
 * Portland extracts that was 17 poles each. The pole now goes to the nearest spot clear of every
 * road surface.
 */

import { describe, expect, it } from "vitest";
import type { VwpWorld } from "@vwp/protocol";
import { clearOfRoad, onRoad, roadSurface } from "../src/signals.js";

/** Two 3 m drive lanes heading +x, on separate edges side by side: y = 0 (left) and y = −3 (right). */
function twoEdges(): VwpWorld {
  const x = Float32Array.from([-50, 0, -50, 0]);
  const y = Float32Array.from([0, 0, -3, -3]);
  return {
    lanes: {
      count: 2,
      laneId: Uint32Array.from([1, 2]),
      pointOff: Uint32Array.from([0, 2]),
      pointCount: Uint32Array.from([2, 2]),
      edgeId: Uint32Array.from([10, 11]),
      widthM: Float32Array.from([3, 3]),
      laneType: Uint8Array.from([0, 0]),
      indexInEdge: Uint8Array.from([0, 0]),
    },
    lanePoints: { count: 4, x, y, z: new Float32Array(4) },
  } as unknown as VwpWorld;
}

describe("signal poles", () => {
  it("never stand in a lane of a neighbouring edge", () => {
    const road = roadSurface(twoEdges());
    // The left edge's head at its stop line (0, 0), heading +x: its own kerb is 0.5 × 3 + 1.2 m
    // to the right, which is inside the right edge's lane.
    expect(onRoad(road, 0, -2.7)).toBe(true);
    const [px, py] = clearOfRoad(road, 0, 0, 1, 0, 2.7);
    expect(onRoad(road, px, py)).toBe(false);
    expect(py).toBeLessThan(-5);
    expect(Math.abs(px)).toBeLessThan(1e-9);
  });

  it("stay at the usual kerb spot when that is clear", () => {
    const road = roadSurface(twoEdges());
    // The right edge's head at (0, −3): its kerb spot is clear, so nothing moves.
    const [px, py] = clearOfRoad(road, 0, -3, 1, 0, 2.7);
    expect(px).toBeCloseTo(0, 9);
    expect(py).toBeCloseTo(-5.7, 5);
  });
});
