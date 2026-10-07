/**
 * The free camera's controls (owner, 2026-10-06: "the free camera should be easier to control and
 * move around"). Before this it had no keys at all: drag turned the view and nothing moved it.
 */

import { describe, expect, it } from "vitest";
import { Viewer } from "../src/scene.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { makeGridWorld } from "./support/fixture.js";

const CANVAS = { width: 1280, height: 800, clientWidth: 1280, clientHeight: 800 } as unknown as ViewerCanvas;
const grid = makeGridWorld({ blocks: 6, blockM: 120, buildingsPerBlock: 2, tunnelDepthM: 6 });

const ev = (type: string, init: Record<string, unknown>): Event => Object.assign(new Event(type, { cancelable: true }), init);

/** A free camera standing in the street at `axes[2]`, eye height 2 m, looking north (+y). */
function freeAtStreet(): { v: Viewer; input: EventTarget } {
  const v = new Viewer({ canvas: CANVAS, autoStart: false, createRenderer: (c) => new NullRenderer(c) });
  v.setWorld(grid.world);
  const a = grid.axes;
  v.camera.position.set(a[2], a[1] + 20, 2);
  v.cameras.look.set(a[2], a[1] + 60, 2);
  v.setCameraMode("free", true);
  v.step(1 / 60);
  const input = new EventTarget();
  v.cameras.attachInput(input);
  return { v, input };
}

function hold(v: Viewer, input: EventTarget, code: string, seconds: number): void {
  input.dispatchEvent(ev("keydown", { code, key: code }));
  for (let t = 0; t < seconds; t += 1 / 60) v.step(1 / 60);
  input.dispatchEvent(ev("keyup", { code, key: code }));
}

describe("free camera", () => {
  it("W flies forward along the ground, not into it, and stops when released", () => {
    const { v, input } = freeAtStreet();
    const y0 = v.camera.position.y;
    const z0 = v.camera.position.z;
    hold(v, input, "KeyW", 1);
    const moved = v.camera.position.y - y0;
    expect(moved).toBeGreaterThan(4);
    expect(Math.abs(v.camera.position.z - z0)).toBeLessThan(0.01);
    // Released: it eases to a stop within a quarter of a second, no drift after.
    for (let i = 0; i < 30; i++) v.step(1 / 60);
    const y1 = v.camera.position.y;
    for (let i = 0; i < 30; i++) v.step(1 / 60);
    expect(Math.abs(v.camera.position.y - y1)).toBeLessThan(0.01);
    v.dispose();
  });

  it("Shift is faster, the wheel sets the speed, and Q never takes it under the road", () => {
    const { v, input } = freeAtStreet();
    const base = v.cameras.freeSpeedMps;
    input.dispatchEvent(ev("wheel", { deltaY: -500 }));
    expect(v.cameras.freeSpeedMps).toBeGreaterThan(base * 1.5);
    input.dispatchEvent(ev("wheel", { deltaY: 500 }));
    const y0 = v.camera.position.y;
    hold(v, input, "KeyW", 0.5);
    const slow = v.camera.position.y - y0;
    input.dispatchEvent(ev("keydown", { code: "ShiftLeft", key: "Shift" }));
    const y1 = v.camera.position.y;
    hold(v, input, "KeyW", 0.5);
    input.dispatchEvent(ev("keyup", { code: "ShiftLeft", key: "Shift" }));
    expect(v.camera.position.y - y1).toBeGreaterThan(slow * 2);
    hold(v, input, "KeyQ", 3);
    expect(v.camera.position.z).toBeGreaterThanOrEqual(v.cameras.floorZ - 1e-9);
    expect(v.camera.position.z).toBeGreaterThan(0.5);
    v.dispose();
  });

  it("slides along a building instead of entering it", () => {
    const { v, input } = freeAtStreet();
    // Turn to face east (+x), into the block beside the street.
    v.camera.position.set(grid.axes[2] + 5, grid.axes[2] + 60, 3);
    v.cameras.look.set(grid.axes[2] + 60, grid.axes[2] + 60, 3);
    v.setCameraMode("map", true);
    v.setCameraMode("free", true);
    hold(v, input, "KeyW", 4);
    const p = v.camera.position;
    const b = v.worldRenderer.buildingIndexAt(p.x, p.y);
    expect(b < 0 || p.z >= v.worldRenderer.buildingTopOf(b), `camera inside building ${b}`).toBe(true);
    v.dispose();
  });

  it("R puts the camera back over the city", () => {
    const { v, input } = freeAtStreet();
    hold(v, input, "KeyQ", 1);
    input.dispatchEvent(ev("keydown", { code: "KeyR", key: "r" }));
    input.dispatchEvent(ev("keyup", { code: "KeyR", key: "r" }));
    for (let i = 0; i < 240; i++) v.step(1 / 60);
    expect(v.camera.position.z).toBeGreaterThan(80);
    expect(v.cameras.mode).toBe("free");
    v.dispose();
  });
});
