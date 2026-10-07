/**
 * The owner's 2026-10-06 report, the halves that need no GPU:
 *
 * - "close to the ground the ground seems to go down and there are holes in it";
 * - "the camera ended up below the map — under the buildings and the whole scene".
 *
 * Both came from one assumption: that the bottom of the world's box is the ground. A tunnel 6 m
 * down (the OSM importer's `TUNNEL_LEVEL_DEPTH_M`) put the ground plane, the landuse, every
 * junction disc and every crosswalk 6 m under the streets, and the camera's floor with them.
 *
 * This file drives the real viewer (a null renderer: every matrix, no pixels) through random poses
 * and random user input in every camera mode, over a grid world with a tunnel, and checks every
 * drawn frame geometrically:
 *
 * 1. the camera is above every road surface and outside every (non-ghosted) building;
 * 2. the near plane's rectangle does not reach down into a road surface (that cut is a hole);
 * 3. every pixel ray below the horizon that the far plane lets through lands on the ground quad;
 * 4. nothing drawn as a street surface lies under the street, apart from the tunnel itself.
 *
 * The checks are written against three.js's own camera and the scene graph only — nothing from
 * the code under test computes them — so they ran unchanged against the viewer before the fix,
 * where every one of them failed (see BUILD-STATUS, scene-and-cameras track).
 */

import { describe, expect, it } from "vitest";
import { Mesh, Vector3, type BufferGeometry, type PlaneGeometry } from "three";
import { Viewer } from "../src/scene.js";
import type { ViewerCanvas } from "../src/types.js";
import type { CameraMode } from "../src/cameras.js";
import { NullRenderer } from "./support/null-renderer.js";
import { makeGridWorld } from "./support/fixture.js";
import { placedPoses, type PlacedActor } from "./support/one-actor.js";

const CANVAS = { width: 1280, height: 800, clientWidth: 1280, clientHeight: 800 } as unknown as ViewerCanvas;
const TUNNEL_M = 6;
const grid = makeGridWorld({ blocks: 6, blockM: 120, buildingsPerBlock: 2, tunnelDepthM: TUNNEL_M });
/** The fixture's buildings stand at z = 0: that is its ground. */
const GRADE = 0;
/** The highest a road surface is drawn above the ground: a marking on a sidewalk, plus a kerb. */
const SURFACE_TOP = 0.5;

/** Mulberry32: a small seeded generator, so a failure names a reproducible pose. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function viewer(): Viewer {
  const v = new Viewer({ canvas: CANVAS, autoStart: false, createRenderer: (c) => new NullRenderer(c) });
  v.setWorld(grid.world);
  return v;
}

/** Whether `(x, y)` is on the fixture's tunnel (under the street at `axes[1]`). */
function onTunnel(x: number, y: number): boolean {
  const a = grid.axes;
  return Math.abs(y - a[1]) < 4.5 && x > a[1] - 3 && x < a[a.length - 2] + 3;
}

/**
 * The lowest world z of the near-plane rectangle per metre of near distance (negative when it
 * points down), from the projection's own inverse: exact with a lens shift too.
 */
function nearDropPerMetre(v: Viewer): number {
  const cam = v.camera;
  let lowest = 0;
  const c = new Vector3();
  const p = cam.position;
  const f = new Vector3();
  cam.getWorldDirection(f);
  for (const [nx, ny] of [[-1, -1], [1, -1], [-1, 1], [1, 1]] as const) {
    c.set(nx, ny, -1).unproject(cam);
    // A point on the near plane: its depth along the view axis is `near`.
    const depth = c.clone().sub(p).dot(f);
    const dz = (c.z - p.z) / depth;
    if (dz < lowest) lowest = dz;
  }
  return lowest;
}

interface Check {
  failures: string[];
  /** Frames failing each check, so one check failing everywhere cannot hide the others. */
  counts: Record<string, number>;
  frames: number;
}

function checkFrame(v: Viewer, label: string, out: Check): void {
  out.frames++;
  const fail = (kind: string, msg: string): void => {
    const n = (out.counts[kind] = (out.counts[kind] ?? 0) + 1);
    if (n <= 3) out.failures.push(`${label}: ${msg}`);
  };
  const cam = v.camera;
  const p = cam.position;
  if (!Number.isFinite(p.x) || !Number.isFinite(p.y) || !Number.isFinite(p.z)) {
    fail("not_finite", "camera not finite");
    return;
  }
  // 1. Above every road surface, outside every building that is not ghosted.
  if (p.z < GRADE + SURFACE_TOP + 0.05) fail("under_ground", `camera at z ${p.z.toFixed(2)}, under the road surface`);
  const w = v.worldRenderer;
  const b = w.buildingIndexAt(p.x, p.y);
  if (b >= 0 && p.z < w.buildingTopOf(b) && b !== w.ghostBuilding && b !== w.ghostBuilding2) {
    fail("in_building", `camera inside building ${b} (z ${p.z.toFixed(1)} under its roof ${w.buildingTopOf(b).toFixed(1)})`);
  }
  // 2. The near plane does not cut the road.
  cam.updateMatrixWorld();
  const lowest = p.z + nearDropPerMetre(v) * cam.near;
  if (lowest < GRADE + SURFACE_TOP - 1e-4) {
    fail("near_cuts_road", `near plane (${cam.near.toFixed(2)} m) reaches z ${lowest.toFixed(2)}, into the road`);
  }
  // 3. The ground: at the ground level, and under every ray below the horizon within the far plane.
  const g = w.ground;
  const gz = g.position.z;
  if (Math.abs(gz - GRADE) > 0.1) fail("ground_not_at_grade", `ground plane at z ${gz.toFixed(2)}, not at the ground level ${GRADE}`);
  const half = (g.geometry as PlaneGeometry).parameters.width / 2;
  const f = new Vector3();
  cam.getWorldDirection(f);
  const d = new Vector3();
  let misses = 0;
  let worst = "";
  for (let iy = 0; iy <= 24; iy++) {
    for (let ix = 0; ix <= 24; ix++) {
      d.set(-1 + ix / 12, -1 + iy / 12, 0.5).unproject(cam).sub(p).normalize();
      if (d.z > -1e-4) continue;
      const t = (gz - p.z) / d.z;
      if (t <= 0) continue;
      if (t * d.dot(f) > cam.far) continue; // past the far plane: not drawn by anything
      const hx = p.x + d.x * t - g.position.x;
      const hy = p.y + d.y * t - g.position.y;
      if (Math.abs(hx) > half || Math.abs(hy) > half) {
        misses++;
        worst = `ray (${ix - 12}, ${iy - 12})/12 meets the ground plane ${Math.hypot(hx, hy).toFixed(0)} m from its centre, past its ${half.toFixed(0)} m half-edge`;
      }
    }
  }
  if (misses > 0) fail("ray_misses_ground", `${misses} of 625 rays below the horizon miss the ground: ${worst}`);
}

/** 4. Every street-surface vertex at or above the ground, except the tunnel's. */
function surfacesUnderGround(v: Viewer): string[] {
  const out: string[] = [];
  v.worldRenderer.tiles.traverse((o) => {
    if (!(o instanceof Mesh)) return;
    const pos = (o.geometry as BufferGeometry).getAttribute("position");
    if (!pos) return;
    for (let i = 0; i < pos.count; i++) {
      const z = pos.getZ(i);
      if (z < GRADE - 0.01 && !onTunnel(pos.getX(i), pos.getY(i))) {
        if (out.length < 5) out.push(`${o.name} vertex at (${pos.getX(i).toFixed(1)}, ${pos.getY(i).toFixed(1)}, ${z.toFixed(2)})`);
      }
    }
  });
  v.worldRenderer.markings.traverse((o) => {
    if (!(o instanceof Mesh)) return;
    const pos = (o.geometry as BufferGeometry).getAttribute("position");
    if (!pos) return;
    for (let i = 0; i < pos.count; i++) {
      if (pos.getZ(i) < GRADE - 0.01 && !onTunnel(pos.getX(i), pos.getY(i)) && out.length < 10) {
        out.push(`${o.name} marking vertex at z ${pos.getZ(i).toFixed(2)}`);
      }
    }
  });
  return out;
}

const ev = (type: string, init: Record<string, unknown>): Event => Object.assign(new Event(type, { cancelable: true }), init);

/** Three actors: on a street, in the tunnel, and on the street over the tunnel. */
function actors(r: () => number, t: number): PlacedActor[] {
  const a = grid.axes;
  const along = a[1] + (a[a.length - 2] - a[1]) * ((t * 0.05 + r() * 0.0) % 1);
  return [
    { actorId: 1, x: a[2] + 1.65, y: a[0] + 40 + (t * 8) % 400, headingRad: Math.PI / 2, speedMps: 8 },
    { actorId: 2, x: along, y: a[1] + 1.65, z: -TUNNEL_M, headingRad: 0, speedMps: 12 },
    { actorId: 3, x: a[3] - 1.65, y: a[2] + 20, headingRad: -Math.PI / 2, speedMps: 3, classIdx: 0 },
  ];
}

const MODES: readonly CameraMode[] = ["map", "chase", "dashboard", "free", "rsu"];

describe("ground and camera geometry, every mode, random poses", () => {
  it("draws no street surface under the street (the holes at every junction)", () => {
    const v = viewer();
    expect(surfacesUnderGround(v)).toEqual([]);
    v.dispose();
  });

  for (const mode of MODES) {
    it(`keeps the camera above the ground, the near plane out of the road and the ground under the horizon: ${mode}`, () => {
      const r = rng(0x5eed + mode.length * 977 + mode.charCodeAt(0));
      const out: Check = { failures: [], counts: {}, frames: 0 };
      const v = viewer();
      const input = new EventTarget();
      v.cameras.attachInput(input);
      let clockNs = 0n;
      let clock = 0;
      const feed = (live: boolean): void => {
        clockNs += 100_000_000n;
        clock += 0.1;
        v.capture(placedPoses(live ? actors(r, clock) : [], clockNs), clock);
      };
      feed(true);
      v.step(1 / 60);
      const a = grid.axes;
      for (let pose = 0; pose < 24; pose++) {
        const label = `${mode} pose ${pose}`;
        // Where the pose starts.
        switch (mode) {
          case "map":
            v.setCameraMode("map", r() < 0.5);
            v.cameras.focusOn(a[0] + r() * (a[a.length - 1] - a[0]), a[0] + r() * (a[a.length - 1] - a[0]), r() < 0.3 ? -TUNNEL_M : 0);
            break;
          case "chase":
          case "dashboard":
            v.flyTo(1 + Math.floor(r() * 3), mode, r() < 0.5);
            break;
          case "free": {
            // Seeded from anywhere the camera could have been: under the street, in a tower, high up.
            v.setCameraMode("map", true);
            v.camera.position.set(a[0] + r() * 600, a[0] + r() * 600, -TUNNEL_M * 2 + r() * 120);
            v.cameras.look.set(v.camera.position.x + 30, v.camera.position.y + 10, v.camera.position.z - 20 + r() * 30);
            v.setCameraMode("free", true);
            break;
          }
          case "rsu":
            v.flyTo(1 + Math.floor(r() * 3), "chase", true);
            v.cameras.viewFromSite(Math.floor(r() * Math.max(1, v.worldRenderer.siteCount)), r() < 0.5);
            break;
        }
        // What the user does: drag the view anywhere, zoom anywhere, hold keys, seek, lose the subject.
        const frames = 40;
        let held: string[] = [];
        for (let f = 0; f < frames; f++) {
          const roll = r();
          if (roll < 0.08) {
            const x0 = 400 + r() * 400;
            const y0 = 300 + r() * 200;
            input.dispatchEvent(ev("pointerdown", { clientX: x0, clientY: y0, button: 0, shiftKey: r() < 0.2, pointerId: 1 }));
            input.dispatchEvent(ev("pointermove", { clientX: x0 + (r() - 0.5) * 3000, clientY: y0 + (r() - 0.5) * 3000 }));
            input.dispatchEvent(ev("pointerup", {}));
          } else if (roll < 0.14) {
            input.dispatchEvent(ev("wheel", { deltaY: (r() - 0.5) * 6000 }));
          } else if (roll < 0.22) {
            for (const k of held) input.dispatchEvent(ev("keyup", { code: k, key: k }));
            const keys = ["KeyW", "KeyA", "KeyS", "KeyD", "KeyQ", "KeyE", "KeyC", "Space", "ShiftLeft", "ArrowUp"];
            held = keys.filter(() => r() < 0.3);
            // Down is the dangerous direction: hold it often.
            if (r() < 0.5) held.push("KeyQ");
            for (const k of held) input.dispatchEvent(ev("keydown", { code: k, key: k }));
          } else if (roll < 0.25) {
            // A seek backwards: the stream restarts from an earlier time.
            clockNs = clockNs > 2_000_000_000n ? clockNs - 2_000_000_000n : 0n;
          } else if (roll < 0.27) {
            // The followed actor leaves the stream.
            feed(false);
          } else if (roll < 0.28) {
            v.cameras.setFollowPose(Number.NaN, 0, Number.NEGATIVE_INFINITY, Number.NaN, Number.NaN);
          }
          if (f % 6 === 0) feed(r() > 0.05);
          v.step(r() < 0.1 ? 0.25 : 1 / 60);
          checkFrame(v, `${label} frame ${f}`, out);
        }
        for (const k of held) input.dispatchEvent(ev("keyup", { code: k, key: k }));
      }
      v.dispose();
      expect(out.frames).toBeGreaterThan(900);
      expect(out.counts, `frames failing each check, of ${out.frames}:\n${out.failures.join("\n")}`).toEqual({});
    });
  }

  it("keeps the chase camera on the street over a subject in the tunnel", () => {
    const v = viewer();
    const a = grid.axes;
    v.capture(placedPoses([{ actorId: 9, x: a[2], y: a[1] + 1.65, z: -TUNNEL_M, headingRad: 0, speedMps: 0 }], 0n), 0);
    v.step(1 / 60);
    v.flyTo(9, "chase", true);
    for (let i = 0; i < 120; i++) v.step(1 / 60);
    expect(v.camera.position.z).toBeGreaterThan(GRADE + SURFACE_TOP);
    // Still over the subject, not thrown somewhere else.
    expect(Math.hypot(v.camera.position.x - a[2], v.camera.position.y - (a[1] + 1.65))).toBeLessThan(25);
    v.dispose();
  });
});
