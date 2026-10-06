/**
 * A bigger vehicle keeps its detail further out. The glitch hunter's dense Midtown capture caught
 * buses changing level of detail while 40 px tall in the chase view: the LOD distances were a
 * car's for everything, and a bus 80 m away is more than twice a car's height on screen. The
 * distances now stretch with the class's size (height against a car's 1.5 m, length against 5 m),
 * never shrinking below a car's.
 */

import { PerspectiveCamera } from "three";
import { describe, expect, it } from "vitest";
import { ActorRenderer } from "../src/actors.js";
import { DARK_THEME } from "../src/theme.js";
import type { ActorClassDef } from "../src/types.js";

const CLASSES: ActorClassDef[] = [
  { index: 0, name: "passenger", lengthM: 5.0, widthM: 1.8, heightM: 1.5, color: 0, category: 0 },
  { index: 1, name: "bus", lengthM: 12.0, widthM: 2.5, heightM: 3.4, color: 0, category: 0 },
  { index: 2, name: "pedestrian", lengthM: 0.4, widthM: 0.5, heightM: 1.7, color: 0, category: 1 },
];

describe("level of detail by size", () => {
  it("draws a bus at full detail where a car of the same distance has dropped to the middle level", () => {
    const camera = new PerspectiveCamera(60, 1.6, 0.1, 5000);
    camera.up.set(0, 0, 1);
    camera.position.set(0, 0, 2);
    camera.lookAt(1, 0, 2);
    camera.updateMatrixWorld();
    camera.updateProjectionMatrix();
    const r = new ActorRenderer({ classes: CLASSES, theme: DARK_THEME });
    // Three actors side by side 150 m ahead (default LOD distances: 90 m and 400 m).
    const position = new Float32Array([150, -6, 0, 150, 0, 0, 150, 6, 0]);
    const ctx = {
      position, heading: new Float32Array(3), speed: new Float32Array(3), classIdx: new Uint8Array([0, 1, 2]),
      state: new Uint8Array(3), occupied: new Uint8Array([1, 1, 1]), actorId: new Uint32Array([1, 2, 3]),
      count: 3, camera, cull: false, dtSeconds: 1 / 60,
    };
    r.update(ctx);
    expect(r.slotLod[0], "car at 150 m").toBe(1);
    expect(r.slotLod[1], "bus at 150 m").toBe(0);
    expect(r.slotLod[2], "person at 150 m, never nearer than a car's distances").toBe(1);
  });
});
