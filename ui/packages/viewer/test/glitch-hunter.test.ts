/**
 * The glitch hunter's rules, each shown able to go red.
 *
 * A check that cannot fail is worth nothing (the project's standing rule), so every class is
 * provoked here by injecting exactly the defect it names into an otherwise clean frame — after the
 * viewer drew it and before the hunter looked — and the test asserts the hunter saw it. The clean
 * frames before each injection are asserted to produce no viewer-caused event, which is the other
 * half: a rule that fires on a clean scene is noise.
 */

import { describe, expect, it } from "vitest";
import { Viewer } from "../src/scene.js";
import { GLITCH_CLASSES, GlitchHunter, obbPenetration, type GlitchClass } from "../src/glitch.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { SyntheticStream, makeGridWorld } from "./support/fixture.js";

const CANVAS = { width: 1280, height: 720, clientWidth: 1280, clientHeight: 720 } as unknown as ViewerCanvas;

interface Rig {
  viewer: Viewer;
  hunter: GlitchHunter;
  stream: SyntheticStream;
  t: number;
  frame: number;
}

function rig(actors = 40): Rig {
  const viewer = new Viewer({
    canvas: CANVAS, theme: "dark", autoStart: false, createRenderer: (c) => new NullRenderer(c),
  });
  const grid = makeGridWorld({ blocks: 6, blockM: 120, buildingsPerBlock: 1 });
  viewer.setWorld(grid.world);
  const stream = new SyntheticStream(actors, grid);
  stream.keyframe();
  viewer.capture(stream.poses, 0);
  const hunter = new GlitchHunter(viewer);
  return { viewer, hunter, stream, t: 0, frame: 0 };
}

/** Draw one 60 fps frame (a delta every sixth), without handing it to the hunter. */
function draw(r: Rig): void {
  r.t += 1000 / 60;
  if (r.frame % 6 === 0) {
    r.stream.advance(0.1);
    r.stream.delta();
    r.viewer.capture(r.stream.poses);
    r.hunter.observeSnapshot(r.stream.poses);
  }
  r.frame++;
  r.viewer.renderFrame(r.t);
}

function clean(r: Rig, frames: number): void {
  for (let i = 0; i < frames; i++) {
    draw(r);
    r.hunter.afterFrame();
  }
}

function viewerCaused(h: GlitchHunter, c: GlitchClass): number {
  const r = h.report();
  return r.counts[c] - (r.engineCaused[c] ?? 0);
}

/** A slot drawn this frame, in the view's interior, and at least `minPx` tall. */
function drawnSlot(r: Rig, category = 0): number {
  const v = r.viewer;
  const it = v.interpolator;
  for (let s = 0; s < it.count; s++) {
    if (it.outOccupied[s] !== 1 || v.actors.slotLod[s] < 0) continue;
    const def = v.actors.classes[it.outClassIdx[s]];
    if (def.category !== category) continue;
    if (Math.abs(it.outSpeed[s]) < 1) continue;
    return s;
  }
  throw new Error("no drawn slot");
}

/** Put the camera close over the traffic, so actors are big enough to be judged. */
function closeUp(r: Rig): void {
  r.viewer.setCameraMode("map", true);
  r.viewer.cameras.fitExtent(160);
  r.viewer.frameActors(20, 160);
  r.viewer.cameras.snap();
}

describe("GlitchHunter — every class can go red", () => {
  it("separating-axis penetration is zero apart, the overlap together", () => {
    expect(obbPenetration(0, 0, 2.5, 0.9, 1, 0, 6, 0, 2.5, 0.9, 1, 0)).toBe(0);
    expect(obbPenetration(0, 0, 2.5, 0.9, 1, 0, 4, 0, 2.5, 0.9, 1, 0)).toBeCloseTo(1, 6);
    // Rotated 90°: the other box's half-width is its extent along x.
    expect(obbPenetration(0, 0, 2.5, 0.9, 1, 0, 3, 0, 2.5, 0.9, 0, 1)).toBeCloseTo(0.4, 6);
  });

  it("a clean synthetic run has no viewer-caused event", () => {
    const r = rig();
    closeUp(r);
    clean(r, 180);
    const rep = r.hunter.report();
    for (const c of GLITCH_CLASSES) {
      expect(viewerCaused(r.hunter, c), `${c}: ${JSON.stringify(rep.examples[c]?.slice(0, 2))}`).toBe(0);
    }
    expect(rep.metrics.actorFrames).toBeGreaterThan(100);
  });

  const cases: [GlitchClass, (r: Rig) => void][] = [
    ["teleport", (r) => {
      const s = drawnSlot(r);
      r.viewer.interpolator.outPosition[s * 3] += 4;
    }],
    ["heading_snap", (r) => {
      const s = drawnSlot(r);
      r.viewer.interpolator.outHeading[s] += 0.6;
    }],
    ["stutter", (r) => {
      // One frame frozen in the middle of steady motion.
      const s = drawnSlot(r);
      const it = r.viewer.interpolator;
      const v = Math.abs(it.outSpeed[s]);
      const h = it.outHeading[s];
      it.outPosition[s * 3] -= Math.cos(h) * v / 60;
      it.outPosition[s * 3 + 1] -= Math.sin(h) * v / 60;
    }],
    ["pop", (r) => {
      const s = drawnSlot(r);
      r.viewer.actors.slotLod[s] = -1;
    }],
    ["lod_pop", (r) => {
      const s = drawnSlot(r);
      const lod = r.viewer.actors.slotLod;
      lod[s] = lod[s] === 2 ? 1 : 2;
    }],
    ["overlap_vehicle", (r) => {
      const it = r.viewer.interpolator;
      const a = drawnSlot(r);
      let b = -1;
      for (let s = 0; s < it.count; s++) {
        if (s !== a && it.outOccupied[s] === 1 && r.viewer.actors.slotLod[s] >= 0
          && r.viewer.actors.classes[it.outClassIdx[s]].category === 0) { b = s; break; }
      }
      it.outPosition[b * 3] = it.outPosition[a * 3] + 1;
      it.outPosition[b * 3 + 1] = it.outPosition[a * 3 + 1];
    }],
    ["overlap_pedestrian", (r) => {
      const it = r.viewer.interpolator;
      const a = drawnSlot(r, 0);
      const p = drawnSlot(r, 1);
      it.outPosition[p * 3] = it.outPosition[a * 3];
      it.outPosition[p * 3 + 1] = it.outPosition[a * 3 + 1];
    }],
    ["overlap_building", (r) => {
      const w = r.viewer.worldRenderer.world!;
      const b = w.buildings;
      const ring = w.ringPoints;
      let cx = 0;
      let cy = 0;
      for (let k = 0; k < b.ringCount[0]; k++) {
        cx += ring.x[b.ringOff[0] + k];
        cy += ring.y[b.ringOff[0] + k];
      }
      const s = drawnSlot(r);
      r.viewer.interpolator.outPosition[s * 3] = cx / b.ringCount[0];
      r.viewer.interpolator.outPosition[s * 3 + 1] = cy / b.ringCount[0];
      r.viewer.interpolator.outPosition[s * 3 + 2] = 0;
      // A slot the overlap check reaches: move the camera over it.
      r.viewer.camera.position.set(cx / b.ringCount[0], cy / b.ringCount[0] - 30, 200);
      r.viewer.camera.updateMatrixWorld();
    }],
    ["z_fighting", (r) => {
      const cam = r.viewer.camera;
      cam.near = 0.01;
      cam.far = 12000;
      cam.updateProjectionMatrix();
    }],
    ["camera_clip", (r) => {
      const w = r.viewer.worldRenderer.world!;
      const b = w.buildings;
      const ring = w.ringPoints;
      let cx = 0;
      let cy = 0;
      for (let k = 0; k < b.ringCount[0]; k++) {
        cx += ring.x[b.ringOff[0] + k];
        cy += ring.y[b.ringOff[0] + k];
      }
      r.viewer.camera.position.set(cx / b.ringCount[0], cy / b.ringCount[0], 1.5);
      r.viewer.camera.updateMatrixWorld();
    }],
    ["empty_frame", (r) => {
      r.viewer.camera.position.set(Number.NaN, 0, 10);
    }],
  ];

  for (const [cls, inject] of cases) {
    it(`${cls}: an injected defect is counted`, () => {
      const r = rig();
      closeUp(r);
      clean(r, 90);
      const before = viewerCaused(r.hunter, cls);
      expect(before, `${cls} before injection: ${JSON.stringify(r.hunter.report().examples[cls])}`).toBe(0);
      draw(r);
      inject(r);
      r.hunter.afterFrame();
      if (cls === "pop") {
        // Drawn, not drawn … and drawn again next frame is a flicker as well.
        draw(r);
        r.hunter.afterFrame();
        expect(viewerCaused(r.hunter, "flicker")).toBeGreaterThan(0);
      }
      expect(viewerCaused(r.hunter, cls) + (r.hunter.report().engineCaused[cls] ?? 0)).toBeGreaterThan(before);
    });
  }

  it("subject_lost and chase_framing: the chase camera turned away and raised", () => {
    const r = rig();
    closeUp(r);
    clean(r, 30);
    const s = drawnSlot(r);
    r.viewer.flyTo(r.viewer.interpolator.outActorId[s], "chase", true);
    clean(r, 120);
    expect(viewerCaused(r.hunter, "subject_lost")).toBe(0);
    expect(viewerCaused(r.hunter, "chase_framing"), JSON.stringify(r.hunter.report().examples.chase_framing)).toBe(0);
    draw(r);
    // Look straight up.
    const cam = r.viewer.camera;
    cam.lookAt(cam.position.x, cam.position.y, cam.position.z + 100);
    cam.updateMatrixWorld();
    r.hunter.afterFrame();
    expect(viewerCaused(r.hunter, "subject_lost")).toBe(1);
    draw(r);
    // Straight down from 60 m.
    const it = r.viewer.interpolator;
    const slot = r.viewer.followSlot;
    cam.position.set(it.outPosition[slot * 3] + 0.5, it.outPosition[slot * 3 + 1], 60);
    r.viewer.cameras.look.set(it.outPosition[slot * 3], it.outPosition[slot * 3 + 1], 1);
    cam.lookAt(r.viewer.cameras.look);
    cam.updateMatrixWorld();
    r.hunter.afterFrame();
    expect(viewerCaused(r.hunter, "chase_framing")).toBeGreaterThan(0);
  });

  it("a signal lamp that changes and changes back within 0.3 s is a flicker", () => {
    const r = rig();
    closeUp(r);
    const sig = r.viewer.worldRenderer.signals;
    expect(sig.count).toBeGreaterThan(0);
    const id = r.viewer.worldRenderer.world!.signals.at(0).signalId;
    const block = (phase: number) => ({
      count: 1, signalId: Uint32Array.of(id), timeToChangeDs: Uint16Array.of(50), phase: Uint8Array.of(phase), reserved: Uint8Array.of(0),
    });
    sig.applyKeyframe(block(6));
    clean(r, 30);
    sig.applyDelta(block(3));
    clean(r, 2);
    expect(viewerCaused(r.hunter, "flicker")).toBe(0);
    sig.applyDelta(block(6));
    clean(r, 2);
    expect(viewerCaused(r.hunter, "flicker")).toBe(1);
  });
});
