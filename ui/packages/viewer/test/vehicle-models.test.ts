/**
 * The road-user models: each fits its class's box, stands on the ground, faces +x, and carries the
 * parts the actor shader animates — wheels about their hubs, lamps where FMVSS 108 puts them,
 * a person's limbs about hips and shoulders.
 */

import { Box3, ShaderLib, Vector3 } from "three";
import { describe, expect, it } from "vitest";
import { ACTOR_SHADER_PATCH_POINTS, packLamps } from "../src/actor-material.js";
import {
  ACTOR_PART, PAINT_PALETTE, buildActorModel, hashId, modelVariants, type ActorModelKind,
} from "../src/vehicle-models.js";
import type { ActorClassDef } from "../src/types.js";

/** The engine's classes (SUMO vType defaults, `v2xw_mobility::classes`). */
const ENGINE: ActorClassDef[] = [
  ["passenger", 5.0, 1.8, 1.5, 0], ["emergency", 6.5, 2.16, 2.86, 0], ["delivery", 6.5, 2.16, 2.86, 0],
  ["truck", 7.1, 2.4, 2.4, 0], ["trailer", 16.5, 2.55, 4.0, 0], ["bus", 12.0, 2.5, 3.4, 0],
  ["coach", 14.0, 2.6, 4.0, 0], ["motorcycle", 2.2, 0.9, 1.5, 0], ["moped", 2.1, 0.8, 1.7, 0],
  ["bicycle", 1.6, 0.65, 1.7, 1], ["pedestrian", 0.215, 0.478, 1.719, 1], ["scooter", 1.2, 0.5, 1.7, 1],
].map(([name, l, w, h, cat], index) => ({
  index, name: name as string, lengthM: l as number, widthM: w as number, heightM: h as number, color: 0, category: cat as number,
}));

function parts(kind: ActorModelKind, def: ActorClassDef, lod: 0 | 1 | 2): Set<number> {
  const { geometry } = buildActorModel(kind, def, lod);
  const a = geometry.getAttribute("aPart").array as Float32Array;
  const out = new Set<number>();
  for (const v of a) out.add(v);
  geometry.dispose();
  return out;
}

describe("road-user models", () => {
  for (const def of ENGINE) {
    for (const v of modelVariants(def, true, 0.2, 0.5)) {
      it(`${def.name} as ${v.kind}: inside its class box, on the ground, all three LODs`, () => {
        for (const lod of [0, 1, 2] as const) {
          const { geometry, info } = buildActorModel(v.kind, def, lod);
          const box = new Box3().setFromBufferAttribute(geometry.getAttribute("position") as never);
          const size = box.getSize(new Vector3());
          expect(size.x, "length").toBeLessThanOrEqual(def.lengthM + 0.15);
          expect(size.y, "width (mirrors allowed 0.3 m)").toBeLessThanOrEqual(def.widthM + 0.35);
          expect(box.min.z, "on the ground").toBeGreaterThanOrEqual(-0.02);
          expect(box.max.z, "height").toBeLessThanOrEqual(def.heightM + 0.25);
          expect(geometry.getAttribute("aPart").count).toBe(geometry.getAttribute("position").count);
          expect(geometry.getAttribute("aPivot").count).toBe(geometry.getAttribute("position").count);
          if (v.kind !== "pedestrian") expect(info.wheelRadiusM).toBeGreaterThan(0.08);
          geometry.dispose();
        }
      });
    }
  }

  it("every face is wound to face out: its winding agrees with its normal", () => {
    // A face wound the wrong way is culled from outside and drawn from inside: a tyre with no
    // tread, a body you see through. Checked on every model at every LOD.
    const bad: string[] = [];
    for (const def of ENGINE) {
      for (const v of modelVariants(def, true, 0.2, 0.5)) {
        for (const lod of [0, 1, 2] as const) {
          const { geometry } = buildActorModel(v.kind, def, lod);
          const pos = geometry.getAttribute("position").array as Float32Array;
          const nrm = geometry.getAttribute("normal").array as Float32Array;
          const idx = geometry.getIndex()!.array;
          let wrong = 0;
          for (let t = 0; t < idx.length; t += 3) {
            const [a, b, c] = [idx[t], idx[t + 1], idx[t + 2]];
            const ux = pos[b * 3] - pos[a * 3];
            const uy = pos[b * 3 + 1] - pos[a * 3 + 1];
            const uz = pos[b * 3 + 2] - pos[a * 3 + 2];
            const vx = pos[c * 3] - pos[a * 3];
            const vy = pos[c * 3 + 1] - pos[a * 3 + 1];
            const vz = pos[c * 3 + 2] - pos[a * 3 + 2];
            const gx = uy * vz - uz * vy;
            const gy = uz * vx - ux * vz;
            const gz = ux * vy - uy * vx;
            if (Math.hypot(gx, gy, gz) < 1e-9) continue;
            const d = gx * nrm[a * 3] + gy * nrm[a * 3 + 1] + gz * nrm[a * 3 + 2];
            if (d < 0) wrong++;
          }
          if (wrong > 0) bad.push(`${v.kind} lod${lod}: ${wrong} of ${idx.length / 3}`);
          geometry.dispose();
        }
      }
    }
    expect(bad).toEqual([]);
  });

  it("a car has spinning and steering wheels, head, tail, stop and both indicators", () => {
    const car = ENGINE[0];
    const p = parts("sedan", car, 0);
    for (const want of [
      ACTOR_PART.PAINT, ACTOR_PART.GLASS, ACTOR_PART.WHEEL_FRONT, ACTOR_PART.WHEEL_REAR, ACTOR_PART.HEAD,
      ACTOR_PART.TAIL, ACTOR_PART.IND_LEFT, ACTOR_PART.IND_RIGHT, ACTOR_PART.BRAKE,
    ]) expect(p.has(want), `part ${want}`).toBe(true);
  });

  it("puts the headlamps at the front and the tail lamps at the back, the left indicator on the left", () => {
    const { geometry } = buildActorModel("sedan", ENGINE[0], 0);
    const pos = geometry.getAttribute("position").array as Float32Array;
    const part = geometry.getAttribute("aPart").array as Float32Array;
    const mean = (code: number): [number, number, number] => {
      let x = 0;
      let y = 0;
      let z = 0;
      let n = 0;
      for (let i = 0; i < part.length; i++) {
        if (part[i] !== code) continue;
        x += pos[i * 3];
        y += pos[i * 3 + 1];
        z += pos[i * 3 + 2];
        n++;
      }
      return [x / n, y / n, z / n];
    };
    const head = mean(ACTOR_PART.HEAD);
    const tail = mean(ACTOR_PART.TAIL);
    expect(head[0]).toBeGreaterThan(2.2);
    expect(tail[0]).toBeLessThan(-2.2);
    // FMVSS 108 Table I: headlamps 22–54 in (0.56–1.37 m) above the road, tail lamps 15–72 in.
    expect(head[2]).toBeGreaterThan(0.56 - 0.1);
    expect(head[2]).toBeLessThan(1.37);
    expect(tail[2]).toBeGreaterThan(0.38);
    expect(mean(ACTOR_PART.IND_LEFT)[1]).toBeGreaterThan(0.5);
    expect(mean(ACTOR_PART.IND_RIGHT)[1]).toBeLessThan(-0.5);
    geometry.dispose();
  });

  it("a wheel's pivot is its hub, one radius above the road", () => {
    const { geometry, info } = buildActorModel("bus", ENGINE[5], 0);
    const part = geometry.getAttribute("aPart").array as Float32Array;
    const piv = geometry.getAttribute("aPivot").array as Float32Array;
    let seen = 0;
    for (let i = 0; i < part.length; i++) {
      if (part[i] !== ACTOR_PART.WHEEL_FRONT && part[i] !== ACTOR_PART.WHEEL_REAR) continue;
      expect(piv[i * 3 + 2]).toBeCloseTo(info.wheelRadiusM, 5);
      seen++;
    }
    expect(seen).toBeGreaterThan(0);
    geometry.dispose();
  });

  it("a person has legs pivoting at the hips and arms at the shoulders, and a stride", () => {
    const def = ENGINE[10];
    const { geometry, info } = buildActorModel("pedestrian", def, 0);
    const part = geometry.getAttribute("aPart").array as Float32Array;
    const piv = geometry.getAttribute("aPivot").array as Float32Array;
    let hip = -1;
    let shoulder = -1;
    for (let i = 0; i < part.length; i++) {
      if (part[i] === ACTOR_PART.LEG_LEFT) hip = piv[i * 3 + 2];
      if (part[i] === ACTOR_PART.ARM_LEFT) shoulder = piv[i * 3 + 2];
    }
    expect(hip).toBeGreaterThan(0.7);
    expect(hip).toBeLessThan(0.95);
    expect(shoulder).toBeGreaterThan(hip + 0.3);
    expect(info.strideM).toBeCloseTo(1.4, 5);
    geometry.dispose();
  });

  it("an ambulance carries red and blue beacons; a bus has no beacon", () => {
    const amb = parts("ambulance", ENGINE[1], 0);
    expect(amb.has(ACTOR_PART.BEACON_RED)).toBe(true);
    expect(amb.has(ACTOR_PART.BEACON_BLUE)).toBe(true);
    expect(parts("bus", ENGINE[5], 0).has(ACTOR_PART.BEACON_RED)).toBe(false);
  });

  it("New York gets yellow cabs among its passenger cars, elsewhere none", () => {
    const nyc = modelVariants(ENGINE[0], true, 0.2, 0.5);
    const other = modelVariants(ENGINE[0], false, 0.2, 0.5);
    expect(nyc.find((v) => v.kind === "taxi")?.weight).toBeCloseTo(0.2, 6);
    expect(other.some((v) => v.kind === "taxi")).toBe(false);
    const sum = (vs: readonly { weight: number }[]): number => vs.reduce((a, v) => a + v.weight, 0);
    expect(sum(nyc)).toBeCloseTo(1, 6);
    expect(sum(other)).toBeCloseTo(1, 6);
  });

  it("the paint palette is a distribution and the id hash is uniform enough to follow it", () => {
    expect(PAINT_PALETTE.reduce((a, e) => a + e.weight, 0)).toBeCloseTo(1, 6);
    const bins = new Array(10).fill(0);
    for (let id = 0; id < 20_000; id++) bins[Math.floor(hashId(id, 2) * 10)]++;
    for (const b of bins) expect(Math.abs(b - 2000)).toBeLessThan(200);
  });
});

describe("the actor shader patch", () => {
  it("finds every chunk it replaces in the installed three.js", () => {
    for (const lib of [ShaderLib.phong, ShaderLib.lambert]) {
      const src = lib.vertexShader + lib.fragmentShader;
      for (const point of ACTOR_SHADER_PATCH_POINTS) expect(src.includes(point), point).toBe(true);
    }
  });

  it("packs the lamps, flash phase, swing amplitude and fade exactly into one float", () => {
    const v = packLamps(0x55, 0.5, 1, 1);
    expect(v % 256).toBe(0x55);
    expect(Math.floor(v / 256) % 64).toBe(32);
    expect(Math.floor(v / 16384) % 64).toBe(63);
    expect(Math.floor(v / 1048576)).toBe(15);
    // Every field at its maximum still fits a float's 24-bit mantissa exactly.
    const max = packLamps(255, 0.999, 1, 1);
    expect(max).toBe(2 ** 24 - 1);
    expect(Math.fround(max)).toBe(max);
    expect(Math.floor(packLamps(0, 0, 0, 0.5) / 1048576)).toBe(8);
  });
});
