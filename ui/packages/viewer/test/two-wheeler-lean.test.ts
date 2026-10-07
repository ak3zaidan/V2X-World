/**
 * A two-wheeler leans into a turn by the angle a steady turn needs, tan φ = v²/(g·R), and a car
 * does not lean at all. Driven through the real `ActorRenderer.update` round a circle, and read
 * back both from the renderer's own lean and from the instance matrix it wrote.
 */

import { PerspectiveCamera, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import { ActorRenderer } from "../src/actors.js";
import { DARK_THEME } from "../src/theme.js";
import type { ActorClassDef } from "../src/types.js";

const CLASSES: ActorClassDef[] = [
  { index: 0, name: "passenger", lengthM: 5.0, widthM: 1.8, heightM: 1.5, color: 0, category: 0 },
  { index: 1, name: "motorcycle", lengthM: 2.2, widthM: 0.9, heightM: 1.5, color: 0, category: 0 },
  { index: 2, name: "bicycle", lengthM: 1.6, widthM: 0.65, heightM: 1.7, color: 0, category: 1 },
];

const camera = new PerspectiveCamera(60, 1.6, 0.1, 5000);
camera.up.set(0, 0, 1);
camera.position.set(0, -80, 80);
camera.lookAt(0, 0, 0);
camera.updateMatrixWorld();
camera.updateProjectionMatrix();

/** Drive one actor of `cls` round a circle of `radius` at `speed` (left turn if `left`). */
function driveCircle(cls: number, radius: number, speed: number, left: boolean, seconds: number): ActorRenderer {
  const r = new ActorRenderer({ classes: CLASSES, theme: DARK_THEME });
  const position = new Float32Array(3);
  const heading = new Float32Array(1);
  const spd = new Float32Array([speed]);
  const ctx = {
    position, heading, speed: spd, classIdx: new Uint8Array([cls]), state: new Uint8Array(1),
    occupied: new Uint8Array([1]), actorId: new Uint32Array([7]), count: 1, camera, cull: false, dtSeconds: 1 / 60,
  };
  const omega = (left ? 1 : -1) * (speed / radius);
  for (let f = 0; f <= seconds * 60; f++) {
    const t = f / 60;
    const a = omega * t;
    // Centre at (0, ±R): the actor starts at the origin heading +x.
    const cy = left ? radius : -radius;
    position[0] = radius * Math.sin(Math.abs(a));
    position[1] = cy - (left ? 1 : -1) * radius * Math.cos(a);
    heading[0] = a;
    r.update(ctx);
  }
  return r;
}

/** The drawn "up" of the only instance of `cls`: the third column of its instance matrix. */
function drawnUp(r: ActorRenderer, cls: number): Vector3 {
  for (const lod of [0, 1, 2] as const) {
    for (const b of r.bucketsAt(cls, lod)) {
      if (b.count === 0) continue;
      const m = b.mesh.instanceMatrix.array as Float32Array;
      return new Vector3(m[8], m[9], m[10]).normalize();
    }
  }
  throw new Error("nothing drawn");
}

describe("two-wheelers lean into turns", () => {
  const radius = 20;
  const speed = 8;
  const want = Math.atan((speed * speed) / (9.80665 * radius));

  it("a motorcycle in a steady left turn leans left by atan(v²/gR)", () => {
    const r = driveCircle(1, radius, speed, true, 3);
    expect(r.leanOfSlot(0)).toBeCloseTo(-want, 2);
    // The instance matrix carries it: the drawn up leans towards the centre of the turn (+y side
    // of a vehicle heading along the circle's tangent), by the same angle.
    const up = drawnUp(r, 1);
    expect(Math.acos(up.z)).toBeCloseTo(want, 2);
    const centre = new Vector3(0, radius, 0);
    const p = new Vector3(radius * Math.sin((speed / radius) * 3), radius - radius * Math.cos((speed / radius) * 3), 0);
    const inward = centre.sub(p).normalize();
    expect(up.x * inward.x + up.y * inward.y, "leans into the turn").toBeGreaterThan(0);
  });

  it("a bicycle in a right turn leans right", () => {
    const r = driveCircle(2, radius, 6, false, 3);
    expect(r.leanOfSlot(0)).toBeCloseTo(Math.atan(36 / (9.80665 * radius)), 2);
  });

  it("a car never leans, and a two-wheeler on a straight stays upright", () => {
    expect(driveCircle(0, radius, speed, true, 3).leanOfSlot(0)).toBe(0);
    const straight = driveCircle(1, 1e9, speed, true, 2);
    expect(Math.abs(straight.leanOfSlot(0))).toBeLessThan(1e-6);
    expect(drawnUp(straight, 1).z).toBeCloseTo(1, 6);
  });
});
