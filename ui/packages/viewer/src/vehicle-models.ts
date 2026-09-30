/**
 * Procedural road-user models: a car looks like a car, a bus like a bus, a person walks.
 *
 * Every model is built from numbers in the actor's local frame — **+x forward, +y left, +z up,
 * origin on the ground at the centre of the class's bounding box** — so an instance matrix is a
 * yaw and a translation and nothing else, exactly as before. What is new is that each vertex
 * carries two more attributes the actor shader reads ({@link ACTOR_PART}):
 *
 * - `aPart`, which part of the body the vertex belongs to: the paint (which takes the per-instance
 *   colour), fixed trim and glass (which keep their own), a front or a rear wheel (which spin, and
 *   the front ones steer), a lamp of a given function (which lights from the stream's `lamps`
 *   byte, §3.3.5), or a limb (which swings with the walking phase);
 * - `aPivot`, the point a moving part turns about: a wheel's hub, a hip, a shoulder.
 *
 * So wheels turn and steer, brake lamps and indicators light, beacons flash and people walk,
 * all in the vertex shader of one instanced draw per (model, LOD) — no per-actor object, no
 * skeleton, no texture.
 *
 * ## The models, and what they are modelled on
 *
 * Proportions are the class's `lengthM × widthM × heightM` (SUMO's vType defaults, which the
 * engine's classes transcribe) and never exceed it, so what is drawn is what is picked and what
 * the auditor tests for overlap. Within the box the shapes follow the vehicle types that make up
 * Midtown traffic: a three-box sedan and a crossover (passenger cars), the yellow cab (an NYC
 * livery of the same car), a 40-ft low-floor transit bus with its destination sign, a coach, a
 * single-unit box truck, a tractor-semitrailer, a high-roof delivery van, a Type III ambulance with
 * a light bar, a motorcycle and a moped with their riders (a share of mopeds with a delivery box,
 * as Midtown's are), a bicycle with its rider, a stand-up e-scooter, and a person on foot.
 *
 * Wheel radii are real ones: 0.33 m for a car's 205/55 R16, 0.50 m for a bus's 305/70 R22.5, 0.31 m
 * for a motorcycle's 17-inch rim with its tyre, 0.34 m for a 700c bicycle wheel.
 *
 * Lamps sit where FMVSS 108 Table I puts them: headlamps at the front corners between 22 and 54
 * inches up, stop and tail lamps at the rear corners between 15 and 72 inches, turn signals
 * outboard, a centre high-mounted stop lamp at the top of the rear window.
 */

import { BufferAttribute, Color, type BufferGeometry } from "three";
import { MeshBuilder, addBox, earClip } from "./geometry.js";
import type { ActorClassDef, LodLevel } from "./types.js";

/** Part codes, the `aPart` attribute. The actor shader switches on them (`actor-material.ts`). */
export const ACTOR_PART = {
  /** Takes the per-instance body colour. */
  PAINT: 0,
  /** Keeps its own vertex colour. */
  FIXED: 1,
  /** A front wheel: spins and steers about its hub. */
  WHEEL_FRONT: 2,
  /** A rear wheel: spins about its hub. */
  WHEEL_REAR: 3,
  /** Headlamp: lit with `LOW_BEAM`. */
  HEAD: 4,
  /** Tail and stop lamp: dim with `LOW_BEAM`, bright with `BRAKE`. */
  TAIL: 5,
  /** Left turn signal: flashes with `TURN_LEFT` or `HAZARD`. */
  IND_LEFT: 6,
  /** Right turn signal. */
  IND_RIGHT: 7,
  /** Reversing lamp: `REVERSE`. */
  REVERSE: 8,
  /** Warning beacon, red: flashes with `EMERGENCY`. */
  BEACON_RED: 9,
  /** Warning beacon, blue: flashes with `EMERGENCY`, in counter-phase. */
  BEACON_BLUE: 10,
  /** Left leg: swings about the hip with the walking phase. */
  LEG_LEFT: 11,
  /** Right leg, in counter-phase. */
  LEG_RIGHT: 12,
  /** Left arm: swings about the shoulder against the left leg. */
  ARM_LEFT: 13,
  /** Right arm. */
  ARM_RIGHT: 14,
  /** Centre high-mounted stop lamp: `BRAKE` only. */
  BRAKE: 15,
  /** Glass: keeps its colour, and is the shiny part. */
  GLASS: 16,
} as const;

/** The models this file builds. */
export type ActorModelKind =
  | "sedan" | "crossover" | "taxi"
  | "bus" | "coach" | "box-truck" | "semi" | "van" | "ambulance"
  | "motorcycle" | "moped" | "moped-delivery" | "bicycle" | "scooter" | "pedestrian"
  | "tram" | "generic";

/** Facts about a model the renderer needs every frame. */
export interface ActorModelInfo {
  /** Rolling radius of the wheels, metres (0 for a person). */
  readonly wheelRadiusM: number;
  /** Distance between the axles, metres: what turns a yaw rate into a steering angle. */
  readonly wheelbaseM: number;
  /** A person's stride (two steps), metres; 0 for anything on wheels. */
  readonly strideM: number;
}

const COLOR = new Color();
/** An sRGB hex as the linear triple a vertex colour must be. */
function lin(hex: number): [number, number, number] {
  COLOR.setHex(hex);
  return [COLOR.r, COLOR.g, COLOR.b];
}

const WHITE = lin(0xffffff);
const TYRE = lin(0x1a1b1d);
const HUB = lin(0x9aa0a6);
const GLASS = lin(0x223040);
const TRIM = lin(0x2a2c30);
const HEAD_LENS = lin(0xdfe6ee);
const TAIL_LENS = lin(0x5a0d10);
const AMBER_LENS = lin(0x7a4a0a);
const PLATE = lin(0xe8e8e0);
const SKIN = lin(0xc89b7b);
const CLOTH_DARK = lin(0x2d3442);
const HELMET = lin(0x30343a);

/**
 * A {@link MeshBuilder} that also records each vertex's part and pivot. The part and pivot are
 * state: set them, then add geometry, and every vertex added takes them.
 */
class PartBuilder extends MeshBuilder {
  part: number = ACTOR_PART.PAINT;
  pivot: [number, number, number] = [0, 0, 0];
  parts = new Float32Array(1024);
  pivots = new Float32Array(1024 * 3);

  constructor() {
    super({ color: true, vertexCapacity: 1024, indexCapacity: 2048 });
  }

  override addVertex(
    x: number, y: number, z: number,
    nx: number, ny: number, nz: number,
    u = 0, v = 0,
    r = 1, g = 1, b = 1,
  ): number {
    const i = super.addVertex(x, y, z, nx, ny, nz, u, v, r, g, b);
    if (this.parts.length <= i) {
      const p = new Float32Array(this.parts.length * 2);
      p.set(this.parts);
      this.parts = p;
      const q = new Float32Array(this.pivots.length * 2);
      q.set(this.pivots);
      this.pivots = q;
    }
    this.parts[i] = this.part;
    this.pivots[i * 3] = this.pivot[0];
    this.pivots[i * 3 + 1] = this.pivot[1];
    this.pivots[i * 3 + 2] = this.pivot[2];
    return i;
  }

  /** Set the part (and optionally the pivot) for what is added next. */
  as(part: number, pivot: [number, number, number] = [0, 0, 0]): this {
    this.part = part;
    this.pivot = pivot;
    return this;
  }

  finish(): BufferGeometry {
    const g = this.toGeometry();
    if (!g) throw new Error("empty actor model");
    g.setAttribute("aPart", new BufferAttribute(this.parts.slice(0, this.vertexCount), 1));
    g.setAttribute("aPivot", new BufferAttribute(this.pivots.slice(0, this.vertexCount * 3), 3));
    return g;
  }
}

function box(
  b: PartBuilder, cx: number, cy: number, cz: number, sx: number, sy: number, sz: number,
  c: readonly [number, number, number] = WHITE,
): void {
  addBox(b, cx, cy, cz, sx, sy, sz, 0, c[0], c[1], c[2]);
}

/**
 * Extrude a side profile — a polygon in the (x, z) plane, counter-clockwise with x forward and z
 * up — across `y0..y1`: two caps and a flat-shaded band.
 */
function profile(
  b: PartBuilder, pts: readonly (readonly [number, number])[], y0: number, y1: number,
  c: readonly [number, number, number] = WHITE,
): void {
  const n = pts.length;
  const xs = new Float32Array(n);
  const zs = new Float32Array(n);
  for (let i = 0; i < n; i++) {
    xs[i] = pts[i][0];
    zs[i] = pts[i][1];
  }
  const tri: number[] = [];
  earClip(xs, zs, 0, n, tri);
  // −y cap: counter-clockwise in (x, z) faces −y.
  let base = b.vertexCount;
  for (let i = 0; i < n; i++) b.addVertex(xs[i], y0, zs[i], 0, -1, 0, 0, 0, c[0], c[1], c[2]);
  for (let i = 0; i + 2 < tri.length; i += 3) b.addTriangle(base + tri[i], base + tri[i + 1], base + tri[i + 2]);
  base = b.vertexCount;
  for (let i = 0; i < n; i++) b.addVertex(xs[i], y1, zs[i], 0, 1, 0, 0, 0, c[0], c[1], c[2]);
  for (let i = 0; i + 2 < tri.length; i += 3) b.addTriangle(base + tri[i], base + tri[i + 2], base + tri[i + 1]);
  // Band.
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    const dx = xs[j] - xs[i];
    const dz = zs[j] - zs[i];
    const len = Math.hypot(dx, dz);
    if (len < 1e-6) continue;
    const nx = dz / len;
    const nz = -dx / len;
    const a = b.addVertex(xs[i], y0, zs[i], nx, 0, nz, 0, 0, c[0], c[1], c[2]);
    const bb = b.addVertex(xs[i], y1, zs[i], nx, 0, nz, 0, 0, c[0], c[1], c[2]);
    const cc = b.addVertex(xs[j], y1, zs[j], nx, 0, nz, 0, 0, c[0], c[1], c[2]);
    const d = b.addVertex(xs[j], y0, zs[j], nx, 0, nz, 0, 0, c[0], c[1], c[2]);
    b.addQuad(a, bb, cc, d);
  }
}

/** A wheel: a cylinder along y, the tyre dark, a lighter hub with spokes so its spin shows. */
function wheel(
  b: PartBuilder, front: boolean, x: number, y: number, r: number, w: number, lod: LodLevel,
): void {
  const part = front ? ACTOR_PART.WHEEL_FRONT : ACTOR_PART.WHEEL_REAR;
  b.as(part, [x, y, r]);
  const seg = lod === 0 ? 12 : 8;
  const y0 = y - w / 2;
  const y1 = y + w / 2;
  // Tread.
  for (let i = 0; i < seg; i++) {
    const a0 = (i / seg) * Math.PI * 2;
    const a1 = ((i + 1) / seg) * Math.PI * 2;
    const x0 = x + Math.cos(a0) * r;
    const z0 = r + Math.sin(a0) * r;
    const x1 = x + Math.cos(a1) * r;
    const z1 = r + Math.sin(a1) * r;
    const nx = Math.cos((a0 + a1) / 2);
    const nz = Math.sin((a0 + a1) / 2);
    const v0 = b.addVertex(x0, y0, z0, nx, 0, nz, 0, 0, TYRE[0], TYRE[1], TYRE[2]);
    const v1 = b.addVertex(x0, y1, z0, nx, 0, nz, 0, 0, TYRE[0], TYRE[1], TYRE[2]);
    const v2 = b.addVertex(x1, y1, z1, nx, 0, nz, 0, 0, TYRE[0], TYRE[1], TYRE[2]);
    const v3 = b.addVertex(x1, y0, z1, nx, 0, nz, 0, 0, TYRE[0], TYRE[1], TYRE[2]);
    // (v0, v1, v2) turns counter-clockwise seen from outside the tread.
    b.addQuad(v0, v1, v2, v3);
  }
  // Side walls (both faces), and on the outer side a hub with spokes.
  for (const side of [-1, 1]) {
    const yy = side < 0 ? y0 : y1;
    const c = b.addVertex(x, yy, r, 0, side, 0, 0, 0, TYRE[0], TYRE[1], TYRE[2]);
    const first = b.vertexCount;
    for (let i = 0; i < seg; i++) {
      const a = (i / seg) * Math.PI * 2;
      b.addVertex(x + Math.cos(a) * r, yy, r + Math.sin(a) * r, 0, side, 0, 0, 0, TYRE[0], TYRE[1], TYRE[2]);
    }
    for (let i = 0; i < seg; i++) {
      const p = first + i;
      const q = first + ((i + 1) % seg);
      if (side > 0) b.addTriangle(c, q, p);
      else b.addTriangle(c, p, q);
    }
  }
  if (lod === 0) {
    // Three spokes on each face: a wheel that turns is a wheel whose pattern moves.
    for (const side of [-1, 1]) {
      const yy = side < 0 ? y0 - 0.005 : y1 + 0.005;
      for (let k = 0; k < 3; k++) {
        const a = (k / 3) * Math.PI;
        const sx = Math.abs(Math.cos(a)) * r * 1.1 + 0.03;
        const sz = Math.abs(Math.sin(a)) * r * 1.1 + 0.03;
        box(b, x, yy, r, Math.max(0.05, sx), 0.01, Math.max(0.05, sz), HUB);
      }
    }
  }
}

/** A lamp: a small box of a lamp part, flush with a face. */
function lamp(
  b: PartBuilder, part: number, x: number, y: number, z: number, sx: number, sy: number, sz: number,
  c: readonly [number, number, number],
): void {
  b.as(part);
  box(b, x, y, z, sx, sy, sz, c);
}

/** Head, tail, indicator and reversing lamps on a body `L × W`, front face at `+L/2`. */
function carLamps(
  b: PartBuilder, L: number, W: number, headZ: number, tailZ: number, lod: LodLevel,
  frontX = L / 2, rearX = -L / 2,
): void {
  const d = 0.06;
  const hw = W / 2;
  for (const s of [1, -1]) {
    lamp(b, ACTOR_PART.HEAD, frontX + d / 2 - 0.01, s * (hw - 0.3), headZ, d, 0.34, 0.14, HEAD_LENS);
    lamp(b, s > 0 ? ACTOR_PART.IND_LEFT : ACTOR_PART.IND_RIGHT, frontX + d / 2 - 0.01, s * (hw - 0.08), headZ, d, 0.12, 0.1, AMBER_LENS);
    lamp(b, ACTOR_PART.TAIL, rearX - d / 2 + 0.01, s * (hw - 0.22), tailZ, d, 0.3, 0.16, TAIL_LENS);
    lamp(b, s > 0 ? ACTOR_PART.IND_LEFT : ACTOR_PART.IND_RIGHT, rearX - d / 2 + 0.01, s * (hw - 0.07), tailZ, d, 0.1, 0.12, AMBER_LENS);
    if (lod === 0) {
      lamp(b, ACTOR_PART.REVERSE, rearX - d / 2 + 0.01, s * (hw - 0.44), tailZ, d, 0.1, 0.1, HEAD_LENS);
    }
  }
  if (lod === 0) {
    // Licence plates.
    b.as(ACTOR_PART.FIXED);
    box(b, frontX + 0.015, 0, headZ - 0.22, 0.02, 0.32, 0.16, PLATE);
    box(b, rearX - 0.015, 0, tailZ - 0.2, 0.02, 0.32, 0.16, PLATE);
  }
}

/**
 * The underside of a body from `x0` to `x1` (left to right) at height `z`, notched with a wheel
 * arch — a half circle of radius `R` round each hub at `(ax, r)` — so the wheels show below the
 * body line instead of being buried in it. Returned left to right, for the bottom edge of a
 * counter-clockwise side profile.
 */
function underside(
  x0: number, x1: number, z: number, axleXs: readonly number[], r: number, R: number,
): [number, number][] {
  const pts: [number, number][] = [[x0, z]];
  const seg = 8;
  for (const ax of [...axleXs].sort((a, b) => a - b)) {
    if (r + R <= z) continue; // the wheel is below the body anyway
    if (ax - R <= x0 + 0.02 || ax + R >= x1 - 0.02) continue; // not under this part of the body
    // Where the arch meets the underside: the circle's intersection with the line z.
    const dz = z - r;
    const half = dz >= R ? 0 : Math.sqrt(R * R - dz * dz);
    if (half <= 0) continue;
    // Over the top of the wheel, whether the underside is above the hub or below it.
    const theta = Math.asin(Math.max(-1, Math.min(1, dz / R)));
    const a0 = Math.PI - theta;
    const a1 = theta;
    pts.push([ax - half, z]);
    for (let k = 1; k < seg; k++) {
      const a = a0 + ((a1 - a0) * k) / seg;
      pts.push([ax + Math.cos(a) * R, r + Math.sin(a) * R]);
    }
    pts.push([ax + half, z]);
  }
  pts.push([x1, z]);
  return pts;
}

/** The four wheels of a two-axle vehicle. */
function axles(b: PartBuilder, xFront: number, xRear: number, W: number, r: number, tyreW: number, lod: LodLevel): void {
  const y = W / 2 - tyreW / 2 - 0.03;
  for (const s of [1, -1]) {
    wheel(b, true, xFront, s * y, r, tyreW, lod);
    wheel(b, false, xRear, s * y, r, tyreW, lod);
  }
}

function sedan(b: PartBuilder, L: number, W: number, H: number, lod: LodLevel, roofSign: boolean, crossover: boolean): ActorModelInfo {
  const r = Math.min(0.33, H * 0.23);
  const clear = crossover ? 0.2 : 0.16;
  const hl = L / 2;
  const belt = crossover ? H * 0.6 : H * 0.56;
  const hood = crossover ? H * 0.58 : H * 0.54;
  const roof = H * (roofSign ? 0.93 : 0.99);
  const wb = L * 0.58;
  const xf = wb / 2 + L * 0.02;
  const xr = -wb / 2 + L * 0.02;
  // Lower body.
  b.as(ACTOR_PART.PAINT);
  profile(b, [
    [-hl, clear + 0.1],
    ...underside(-hl + 0.12, hl - 0.12, clear, [xr, xf], r, r + 0.05),
    [hl, clear + 0.12],
    [hl, hood - 0.12], [hl - 0.35, hood], [L * 0.16, belt], [-L * 0.3, belt],
    [-hl + 0.1, crossover ? belt : belt - 0.05], [-hl, belt - 0.2],
  ], -W / 2, W / 2);
  // Greenhouse: glass, with a painted roof panel on top.
  const gw = W * 0.86;
  const wsBase = L * 0.16;
  const roofFront = crossover ? L * 0.02 : -L * 0.04;
  const roofRear = crossover ? -L * 0.4 : -L * 0.27;
  const rearBase = crossover ? -L * 0.46 : -L * 0.36;
  b.as(ACTOR_PART.GLASS);
  profile(b, [
    [rearBase, belt], [wsBase, belt], [roofFront, roof - 0.06], [roofRear, roof - 0.06],
  ], -gw / 2, gw / 2, GLASS);
  b.as(ACTOR_PART.PAINT);
  box(b, (roofFront + roofRear) / 2, 0, roof - 0.03, roofFront - roofRear + 0.08, gw + 0.02, 0.06);
  if (lod === 0) {
    // Pillars, so the greenhouse reads as a roof on posts rather than a block of glass.
    for (const s of [1, -1]) {
      box(b, (wsBase + roofFront) / 2, s * (gw / 2 + 0.005), (belt + roof) / 2 - 0.03, 0.09, 0.012, roof - belt - 0.06);
      box(b, (rearBase + roofRear) / 2, s * (gw / 2 + 0.005), (belt + roof) / 2 - 0.03, 0.14, 0.012, roof - belt - 0.06);
      box(b, (roofFront + roofRear) / 2 - 0.05, s * (gw / 2 + 0.005), (belt + roof) / 2 - 0.03, 0.08, 0.012, roof - belt - 0.06);
      // Mirrors.
      box(b, wsBase - 0.05, s * (W / 2 + 0.07), belt + 0.08, 0.12, 0.14, 0.1);
    }
    // Grille and bumpers.
    b.as(ACTOR_PART.FIXED);
    box(b, hl + 0.005, 0, hood * 0.62, 0.02, W * 0.42, hood * 0.2, TRIM);
    box(b, hl - 0.05, 0, clear + 0.14, 0.12, W * 0.96, 0.1, TRIM);
    box(b, -hl + 0.05, 0, clear + 0.14, 0.12, W * 0.96, 0.1, TRIM);
    // Centre high-mounted stop lamp at the top of the rear window.
    lamp(b, ACTOR_PART.BRAKE, rearBase + 0.06, 0, belt + 0.03, 0.04, 0.24, 0.04, TAIL_LENS);
  }
  if (roofSign) {
    // The NYC medallion taxi's roof light.
    b.as(ACTOR_PART.FIXED);
    box(b, (roofFront + roofRear) / 2, 0, roof + 0.1, 0.28, 0.72, 0.16, lin(0xf2f0e6));
  }
  carLamps(b, L, W, hood - 0.18, belt - 0.14, lod);
  axles(b, xf, xr, W, r, 0.21, lod);
  return { wheelRadiusM: r, wheelbaseM: xf - xr, strideM: 0 };
}

function bus(b: PartBuilder, L: number, W: number, H: number, lod: LodLevel, coach: boolean): ActorModelInfo {
  const r = Math.min(0.5, H * 0.15);
  const hl = L / 2;
  const floor = 0.32;
  const top = H - (coach ? 0.02 : 0.12);
  const busWb = L * (coach ? 0.55 : 0.52);
  const busXf = hl - L * 0.2;
  const busAxles = coach ? [busXf, busXf - busWb, busXf - busWb - 1.3] : [busXf, busXf - busWb];
  b.as(ACTOR_PART.PAINT);
  profile(b, [
    ...underside(-hl, hl, floor, busAxles, r, r + 0.06),
    [hl, top - 0.35], [hl - 0.25, top], [-hl + 0.1, top], [-hl, top - 0.15],
  ], -W / 2, W / 2);
  // Window band down both sides and the windscreen.
  b.as(ACTOR_PART.GLASS);
  const winLo = coach ? H * 0.45 : H * 0.36;
  const winHi = top - (coach ? 0.35 : 0.3);
  for (const s of [1, -1]) box(b, -0.3, s * (W / 2 + 0.005), (winLo + winHi) / 2, L * 0.86, 0.02, winHi - winLo, GLASS);
  box(b, hl + 0.005, 0, (H * 0.3 + winHi) / 2 + 0.05, 0.02, W * 0.9, winHi - H * 0.3, GLASS);
  box(b, -hl - 0.005, 0, (winLo + winHi) / 2, 0.02, W * 0.8, (winHi - winLo) * 0.8, GLASS);
  b.as(ACTOR_PART.FIXED);
  if (!coach) {
    // Destination sign (amber LEDs on black), and the livery stripe below the windows.
    box(b, hl + 0.01, 0, winHi + 0.14, 0.02, W * 0.8, 0.22, lin(0xd99a1e));
    for (const s of [1, -1]) box(b, -0.3, s * (W / 2 + 0.006), winLo - 0.18, L * 0.9, 0.01, 0.16, lin(0x1f4fa0));
    // Doors on the kerb (right) side: front and centre.
    if (lod === 0) {
      for (const dx of [hl - 1.2, -0.4]) box(b, dx, -(W / 2 + 0.008), (floor + winHi) / 2, 1.1, 0.01, winHi - floor, lin(0x2a3440));
    }
  }
  const wb = busWb;
  const xf = busXf;
  carLamps(b, L, W, floor + 0.55, floor + 0.7, lod);
  axles(b, xf, xf - wb, W, r, 0.3, lod);
  if (coach) {
    // Tag axle.
    const y = W / 2 - 0.17;
    for (const s of [1, -1]) wheel(b, false, xf - wb - 1.3, s * y, r, 0.3, lod);
  }
  return { wheelRadiusM: r, wheelbaseM: wb, strideM: 0 };
}

function truck(b: PartBuilder, L: number, W: number, H: number, lod: LodLevel, semi: boolean): ActorModelInfo {
  const r = Math.min(0.5, H * 0.13);
  const hl = L / 2;
  const cabL = semi ? 3.6 : 2.2;
  const cabH = semi ? Math.min(H, 3.2) : Math.min(H * 0.95, 2.6);
  const frame = r + 0.25;
  // Cab.
  b.as(ACTOR_PART.PAINT);
  profile(b, [
    ...underside(hl - cabL, hl, frame, [hl - 1.1], r, r + 0.06),
    [hl, cabH * 0.62], [hl - 0.35, cabH], [hl - cabL, cabH],
  ], -W / 2, W / 2);
  b.as(ACTOR_PART.GLASS);
  box(b, hl - 0.2, 0, cabH * 0.78, 0.3, W * 0.86, cabH * 0.26, GLASS);
  for (const s of [1, -1]) box(b, hl - 0.8, s * (W / 2 + 0.004), cabH * 0.76, 0.8, 0.01, cabH * 0.28, GLASS);
  // Cargo box or trailer: white, the colour most of them are.
  b.as(ACTOR_PART.FIXED);
  const boxFront = hl - cabL - 0.15;
  const boxRear = -hl;
  const boxLo = semi ? 1.2 : frame + 0.05;
  box(b, (boxFront + boxRear) / 2, 0, (boxLo + H) / 2, boxFront - boxRear, W, H - boxLo, lin(0xe9e9e4));
  box(b, (hl - cabL + boxRear) / 2, 0, frame - 0.1, hl - cabL - boxRear, W * 0.5, 0.2, TRIM);
  carLamps(b, L, W, frame + 0.45, boxLo + 0.3, lod);
  const y = W / 2 - 0.2;
  const xf = hl - 1.1;
  for (const s of [1, -1]) {
    wheel(b, true, xf, s * y, r, 0.3, lod);
    if (semi) {
      wheel(b, false, hl - cabL - 0.3, s * y, r, 0.3, lod);
      wheel(b, false, hl - cabL - 1.6, s * y, r, 0.3, lod);
      wheel(b, false, -hl + 1.6, s * y, r, 0.3, lod);
      wheel(b, false, -hl + 2.9, s * y, r, 0.3, lod);
    } else {
      wheel(b, false, -hl + 1.4, s * y, r, 0.3, lod);
    }
  }
  return { wheelRadiusM: r, wheelbaseM: semi ? cabL + 0.5 : xf + hl - 1.4, strideM: 0 };
}

function van(b: PartBuilder, L: number, W: number, H: number, lod: LodLevel, ambulance: boolean): ActorModelInfo {
  const r = Math.min(0.37, H * 0.13);
  const hl = L / 2;
  const floor = r + 0.12;
  const vanWb = L * 0.55;
  const vanXf = hl - L * 0.17;
  const vanAxles = [vanXf, vanXf - vanWb];
  const R = r + 0.05;
  b.as(ambulance ? ACTOR_PART.FIXED : ACTOR_PART.PAINT);
  if (ambulance) {
    // Type III: a van cab and a square module behind it, white.
    const white = lin(0xf1f1ee);
    profile(b, [
      ...underside(hl - 1.9, hl, floor, vanAxles, r, R),
      [hl, H * 0.4], [hl - 0.7, H * 0.66], [hl - 1.9, H * 0.66],
    ], -W / 2 + 0.05, W / 2 - 0.05, white);
    // The module, from behind the cab to the rear, arched over the rear wheels.
    profile(b, [
      ...underside(-hl, hl - 1.9, floor, vanAxles, r, R),
      [hl - 1.9, H - 0.12], [-hl, H - 0.12],
    ], -W / 2, W / 2, white);
    // The red band round the module.
    for (const s of [1, -1]) box(b, -0.5, s * (W / 2 + 0.005), H * 0.42, L - 2.4, 0.01, 0.24, lin(0xc0231e));
    b.as(ACTOR_PART.GLASS);
    box(b, hl - 0.45, 0, H * 0.52, 0.5, W * 0.84, H * 0.16, GLASS);
    // Light bar on the cab roof and beacons at the module's upper corners.
    lamp(b, ACTOR_PART.BEACON_RED, hl - 1.3, 0.35, H * 0.66 + 0.07, 0.3, 0.5, 0.12, lin(0x6a1010));
    lamp(b, ACTOR_PART.BEACON_BLUE, hl - 1.3, -0.35, H * 0.66 + 0.07, 0.3, 0.5, 0.12, lin(0x10206a));
    for (const s of [1, -1]) {
      lamp(b, s > 0 ? ACTOR_PART.BEACON_RED : ACTOR_PART.BEACON_BLUE, hl - 1.95, s * (W / 2 - 0.1), H - 0.2, 0.08, 0.16, 0.14, lin(0x6a1010));
      lamp(b, s > 0 ? ACTOR_PART.BEACON_BLUE : ACTOR_PART.BEACON_RED, -hl - 0.03, s * (W / 2 - 0.1), H - 0.2, 0.08, 0.16, 0.14, lin(0x6a1010));
    }
  } else {
    // A high-roof delivery van.
    profile(b, [
      ...underside(-hl, hl, floor, vanAxles, r, R),
      [hl, H * 0.38], [hl - 0.55, H * 0.55], [hl - 1.2, H * 0.97], [-hl, H * 0.97],
    ], -W / 2, W / 2);
    b.as(ACTOR_PART.GLASS);
    box(b, hl - 0.85, 0, H * 0.72, 0.5, W * 0.84, H * 0.22, GLASS);
    for (const s of [1, -1]) box(b, hl - 1.35, s * (W / 2 + 0.004), H * 0.66, 0.7, 0.01, H * 0.22, GLASS);
  }
  const wb = vanWb;
  const xf = vanXf;
  carLamps(b, L, W, H * 0.3, H * 0.3, lod);
  axles(b, xf, xf - wb, W, r, 0.24, lod);
  return { wheelRadiusM: r, wheelbaseM: wb, strideM: 0 };
}

/** A rider, seated or standing, with arms reaching to the bars. Legs are fixed on a motorcycle. */
function rider(b: PartBuilder, hipX: number, hipZ: number, headTop: number, barX: number, barZ: number, lean: number, helmet: boolean): void {
  b.as(ACTOR_PART.FIXED);
  const torso = Math.max(0.3, headTop - hipZ - 0.28);
  const tx = hipX + lean * 0.5;
  box(b, tx, 0, hipZ + torso / 2, 0.24, 0.36, torso, CLOTH_DARK);
  box(b, hipX + lean + 0.02, 0, hipZ + torso + 0.13, 0.24, 0.24, 0.26, helmet ? HELMET : SKIN);
  // Arms to the bars.
  for (const s of [1, -1]) {
    const sx = tx + 0.05;
    const sz = hipZ + torso * 0.9;
    box(b, (sx + barX) / 2, s * 0.2, (sz + barZ) / 2, Math.abs(barX - sx) + 0.08, 0.08, Math.abs(sz - barZ) + 0.08, CLOTH_DARK);
  }
}

function twoWheeler(b: PartBuilder, L: number, W: number, H: number, lod: LodLevel, kind: "motorcycle" | "moped" | "moped-delivery"): ActorModelInfo {
  const r = kind === "motorcycle" ? 0.31 : 0.25;
  const axle = L / 2 - r - 0.02;
  const tyreW = kind === "motorcycle" ? 0.16 : 0.11;
  wheel(b, true, axle, 0, r, tyreW, lod);
  wheel(b, false, -axle, 0, r, tyreW, lod);
  b.as(ACTOR_PART.PAINT);
  if (kind === "motorcycle") {
    // Tank, seat and tail.
    profile(b, [[-axle + 0.1, r + 0.1], [axle - 0.2, r + 0.15], [axle - 0.1, r + 0.55], [0.15, r + 0.6], [-0.35, r + 0.45], [-axle, r + 0.5]], -0.16, 0.16);
  } else {
    // Step-through body with a floorboard.
    profile(b, [[-axle, r + 0.05], [axle - 0.1, r - 0.05], [axle, r + 0.7], [axle - 0.15, r + 0.75], [0.05, r + 0.12], [-0.3, r + 0.45], [-axle - 0.05, r + 0.5]], -0.15, 0.15);
  }
  b.as(ACTOR_PART.FIXED);
  const barZ = r + (kind === "motorcycle" ? 0.72 : 0.8);
  box(b, axle - 0.12, 0, barZ, 0.05, Math.min(W, 0.72), 0.04, TRIM);
  // Fork.
  box(b, axle - 0.05, 0, (r + barZ) / 2, 0.05, 0.12, barZ - r, TRIM);
  if (kind === "moped-delivery") {
    // The insulated delivery box on the rear rack.
    box(b, -axle + 0.1, 0, r + 0.75, 0.42, 0.42, 0.42, lin(0xd23b2a));
  }
  lamp(b, ACTOR_PART.HEAD, axle - 0.02, 0, barZ - 0.1, 0.06, 0.14, 0.12, HEAD_LENS);
  lamp(b, ACTOR_PART.TAIL, -axle - 0.05, 0, r + 0.45, 0.05, 0.12, 0.07, TAIL_LENS);
  for (const s of [1, -1]) {
    const ind = s > 0 ? ACTOR_PART.IND_LEFT : ACTOR_PART.IND_RIGHT;
    lamp(b, ind, axle - 0.1, s * 0.16, barZ - 0.14, 0.05, 0.05, 0.05, AMBER_LENS);
    lamp(b, ind, -axle - 0.02, s * 0.12, r + 0.42, 0.05, 0.05, 0.05, AMBER_LENS);
  }
  const seatZ = r + (kind === "motorcycle" ? 0.55 : 0.5);
  rider(b, -0.15, seatZ, Math.min(H, seatZ + 0.85), axle - 0.14, barZ, 0.12, true);
  // Legs to the pegs.
  b.as(ACTOR_PART.FIXED);
  for (const s of [1, -1]) box(b, 0.05, s * 0.17, (seatZ + r) / 2 + 0.05, 0.14, 0.12, seatZ - r, CLOTH_DARK);
  return { wheelRadiusM: r, wheelbaseM: 2 * axle, strideM: 0 };
}

function bicycle(b: PartBuilder, L: number, W: number, H: number, lod: LodLevel, scooter: boolean): ActorModelInfo {
  const r = scooter ? 0.11 : Math.min(0.34, L * 0.21);
  const axle = L / 2 - r;
  const tyre = scooter ? 0.05 : 0.035;
  wheel(b, true, axle, 0, r, tyre, lod);
  wheel(b, false, -axle, 0, r, tyre, lod);
  b.as(ACTOR_PART.PAINT);
  const barZ = scooter ? 1.0 : r * 2.25;
  if (scooter) {
    box(b, 0, 0, r + 0.06, axle * 2, 0.16, 0.05);
    box(b, axle - 0.02, 0, (r + barZ) / 2, 0.04, 0.04, barZ - r);
  } else {
    const frameZ = r * 1.25;
    box(b, 0, 0, frameZ + 0.12, axle * 1.7, 0.035, 0.035);
    box(b, -axle * 0.1, 0, (r + frameZ + r * 0.9) / 2, 0.035, 0.035, frameZ + r * 0.9 - r);
    box(b, axle * 0.45, 0, (r + frameZ + 0.12) / 2 + 0.05, axle * 1.0, 0.035, 0.035);
  }
  b.as(ACTOR_PART.FIXED);
  box(b, axle * 0.85, 0, barZ, 0.035, Math.min(W, 0.58), 0.035, TRIM);
  if (!scooter) lamp(b, ACTOR_PART.TAIL, -axle - r * 0.6, 0, r * 1.6, 0.04, 0.06, 0.05, TAIL_LENS);
  if (scooter) {
    // Standing rider.
    const stand = r + 0.1;
    b.as(ACTOR_PART.FIXED);
    for (const s of [1, -1]) box(b, -0.05 + s * 0.08, s * 0.1, stand + 0.42, 0.14, 0.12, 0.84, CLOTH_DARK);
    rider(b, -0.02, stand + 0.84, Math.min(H, 1.72), axle * 0.85, barZ, 0.08, false);
  } else {
    const saddleZ = r * 2.1;
    // Pedalling legs: they swing with the crank (the walking phase, driven by the wheel).
    b.as(ACTOR_PART.LEG_LEFT, [-axle * 0.1, 0.1, saddleZ]);
    box(b, -axle * 0.1 + 0.05, 0.1, (r + saddleZ) / 2 + 0.05, 0.14, 0.12, saddleZ - r, CLOTH_DARK);
    b.as(ACTOR_PART.LEG_RIGHT, [-axle * 0.1, -0.1, saddleZ]);
    box(b, -axle * 0.1 + 0.05, -0.1, (r + saddleZ) / 2 + 0.05, 0.14, 0.12, saddleZ - r, CLOTH_DARK);
    rider(b, -axle * 0.15, saddleZ, Math.min(H, saddleZ + 0.78), axle * 0.85, barZ, 0.3, true);
  }
  return { wheelRadiusM: r, wheelbaseM: 2 * axle, strideM: scooter ? 0 : 2 * Math.PI * r * 1.9 };
}

/**
 * A person on foot, facing +x: legs from the hips, a torso, arms from the shoulders, a head. The
 * legs and arms swing in opposition with the walking phase. `L` is front to back, `W` shoulder
 * to shoulder (the class's 0.215 × 0.478 × 1.719 m).
 */
function pedestrian(b: PartBuilder, L: number, W: number, H: number, lod: LodLevel): ActorModelInfo {
  const legH = H * 0.47;
  const torsoH = H * 0.3;
  const headH = H - legH - torsoH;
  const torsoW = W * 0.78;
  const depth = Math.max(0.16, L);
  const hipY = torsoW * 0.24;
  const legW = torsoW * 0.36;
  b.as(ACTOR_PART.LEG_LEFT, [0, hipY, legH]);
  box(b, 0, hipY, legH / 2, depth * 0.8, legW, legH, CLOTH_DARK);
  b.as(ACTOR_PART.LEG_RIGHT, [0, -hipY, legH]);
  box(b, 0, -hipY, legH / 2, depth * 0.8, legW, legH, CLOTH_DARK);
  // Torso takes the paint (the clothing colour, per person).
  b.as(ACTOR_PART.PAINT);
  box(b, 0, 0, legH + torsoH / 2, depth, torsoW, torsoH);
  const shoulderZ = legH + torsoH * 0.95;
  const armH = torsoH * 1.05;
  b.as(ACTOR_PART.ARM_LEFT, [0, torsoW / 2 + 0.05, shoulderZ]);
  box(b, 0, torsoW / 2 + 0.05, shoulderZ - armH / 2, depth * 0.55, 0.09, armH);
  b.as(ACTOR_PART.ARM_RIGHT, [0, -(torsoW / 2 + 0.05), shoulderZ]);
  box(b, 0, -(torsoW / 2 + 0.05), shoulderZ - armH / 2, depth * 0.55, 0.09, armH);
  b.as(ACTOR_PART.FIXED);
  const headR = Math.min(headH * 0.48, 0.12);
  box(b, 0, 0, legH + torsoH + headH * 0.12, 0.08, 0.08, headH * 0.25, SKIN);
  box(b, 0, 0, H - headR, headR * 1.7, headR * 1.6, headR * 2, SKIN);
  if (lod === 0) box(b, -0.01, 0, H - headR * 0.35, headR * 1.8, headR * 1.7, headR * 0.7, lin(0x2a2018));
  // 0.7 m steps: a 1.4 m stride at the 1.3 m/s a pedestrian walks (about 1.85 steps a second,
  // inside the 1.8–2.0 Hz cadence reported for free walking).
  return { wheelRadiusM: 0, wheelbaseM: 0, strideM: 1.4 };
}

/** Which models a class is drawn with, and how often each (weights sum to 1). */
export function modelVariants(def: ActorClassDef, nyc: boolean, taxiShare: number, deliveryMopedShare: number): readonly { kind: ActorModelKind; weight: number }[] {
  const n = def.name.toLowerCase();
  if (n === "passenger" || n === "car") {
    const taxi = nyc ? Math.max(0, Math.min(1, taxiShare)) : 0;
    return [
      { kind: "sedan", weight: (1 - taxi) * 0.55 },
      { kind: "crossover", weight: (1 - taxi) * 0.45 },
      ...(taxi > 0 ? [{ kind: "taxi" as const, weight: taxi }] : []),
    ];
  }
  if (n === "taxi") return [{ kind: "taxi", weight: 1 }];
  if (n === "bus") return [{ kind: "bus", weight: 1 }];
  if (n === "coach") return [{ kind: "coach", weight: 1 }];
  if (n === "truck") return [{ kind: "box-truck", weight: 1 }];
  if (n === "trailer" || n === "semi") return [{ kind: "semi", weight: 1 }];
  if (n === "delivery" || n === "van") return [{ kind: "van", weight: 1 }];
  if (n === "emergency" || n === "ambulance") return [{ kind: "ambulance", weight: 1 }];
  if (n === "motorcycle" || n === "moto") return [{ kind: "motorcycle", weight: 1 }];
  if (n === "moped" || n === "delivery-moped") {
    const d = Math.max(0, Math.min(1, n === "delivery-moped" ? 1 : deliveryMopedShare));
    const both: { kind: ActorModelKind; weight: number }[] = [
      { kind: "moped-delivery", weight: d },
      { kind: "moped", weight: 1 - d },
    ];
    return both.filter((v) => v.weight > 0);
  }
  if (n === "bicycle" || n === "cyclist") return [{ kind: "bicycle", weight: 1 }];
  if (n === "scooter" || n === "e-scooter") return [{ kind: "scooter", weight: 1 }];
  if (n === "pedestrian" || n === "person") return [{ kind: "pedestrian", weight: 1 }];
  if (n === "rail" || n === "tram") return [{ kind: "tram", weight: 1 }];
  // An unknown class: judge by category and size.
  if (def.category === 1) return [{ kind: def.lengthM > 0.9 ? "bicycle" : "pedestrian", weight: 1 }];
  if (def.lengthM >= 10) return [{ kind: "bus", weight: 1 }];
  if (def.lengthM >= 6.8) return [{ kind: "box-truck", weight: 1 }];
  if (def.lengthM >= 5.8 || def.heightM >= 2.2) return [{ kind: "van", weight: 1 }];
  if (def.lengthM <= 2.6) return [{ kind: "motorcycle", weight: 1 }];
  return [{ kind: "sedan", weight: 1 }];
}

/**
 * Build one model at one level of detail, sized to `def`. LOD 2 is a single box, the far view's
 * silhouette, with every vertex paint.
 */
export function buildActorModel(kind: ActorModelKind, def: ActorClassDef, lod: LodLevel): { geometry: BufferGeometry; info: ActorModelInfo } {
  const b = new PartBuilder();
  const L = Math.max(0.2, def.lengthM);
  const W = Math.max(0.2, def.widthM);
  const H = Math.max(0.3, def.heightM);
  let info: ActorModelInfo;
  if (lod === 2) {
    b.as(ACTOR_PART.PAINT);
    box(b, 0, 0, H / 2, L, W, H);
    info = modelInfoOf(kind, L, W, H);
    return { geometry: b.finish(), info };
  }
  switch (kind) {
    case "sedan": info = sedan(b, L, W, H, lod, false, false); break;
    case "crossover": info = sedan(b, L, W, H, lod, false, true); break;
    case "taxi": info = sedan(b, L, W, H, lod, true, false); break;
    case "bus": info = bus(b, L, W, H, lod, false); break;
    case "coach": info = bus(b, L, W, H, lod, true); break;
    case "tram": info = bus(b, L, W, H, lod, false); break;
    case "box-truck": info = truck(b, L, W, H, lod, false); break;
    case "semi": info = truck(b, L, W, H, lod, true); break;
    case "van": info = van(b, L, W, H, lod, false); break;
    case "ambulance": info = van(b, L, W, H, lod, true); break;
    case "motorcycle": info = twoWheeler(b, L, W, H, lod, "motorcycle"); break;
    case "moped": info = twoWheeler(b, L, W, H, lod, "moped"); break;
    case "moped-delivery": info = twoWheeler(b, L, W, H, lod, "moped-delivery"); break;
    case "bicycle": info = bicycle(b, L, W, H, lod, false); break;
    case "scooter": info = bicycle(b, L, W, H, lod, true); break;
    case "pedestrian": info = pedestrian(b, L, W, H, lod); break;
    default: info = sedan(b, L, W, H, lod, false, false); break;
  }
  return { geometry: b.finish(), info };
}

/** The facts of a model without keeping its geometry. */
export function modelInfoOf(kind: ActorModelKind, L: number, W: number, H: number): ActorModelInfo {
  if (kind === "pedestrian") return { wheelRadiusM: 0, wheelbaseM: 0, strideM: 1.4 };
  const built = buildActorModel(kind, { index: 0, name: kind, lengthM: L, widthM: W, heightM: H, color: 0, category: 0 }, 1);
  built.geometry.dispose();
  return built.info;
}

/**
 * Body colours of the North American fleet by colour family, rounded from Axalta's annual
 * Global Automotive Color Popularity reports (white about a quarter, then black, grey, silver,
 * blue, red). A palette, not a measurement of any street: it makes a queue look like one, and it
 * is a parameter of the viewer, not of the simulation.
 */
export const PAINT_PALETTE: readonly { hex: number; weight: number }[] = [
  { hex: 0xe9eaec, weight: 0.26 }, // white
  { hex: 0x17181a, weight: 0.21 }, // black
  { hex: 0x6d7074, weight: 0.18 }, // grey
  { hex: 0xb5b8bc, weight: 0.10 }, // silver
  { hex: 0x1f3f7a, weight: 0.09 }, // blue
  { hex: 0x9a1b1f, weight: 0.09 }, // red
  { hex: 0x3b4d3a, weight: 0.03 }, // green
  { hex: 0x7a5a3a, weight: 0.02 }, // brown / beige
  { hex: 0xc9a23a, weight: 0.02 }, // gold / yellow
];

/** What a class's body is painted, when it has a fixed livery rather than a palette draw. */
export function liveryOf(kind: ActorModelKind): number | null {
  switch (kind) {
    case "taxi": return 0xf6b511; // NYC TLC taxi yellow (Dupont M6284 "Taxi Yellow")
    case "bus": return 0xeef0f2; // MTA New York City Transit: white body, blue stripe
    case "coach": return 0xdfe3e8;
    case "ambulance": return 0xf1f1ee;
    case "tram": return 0xd9dce0;
    default: return null;
  }
}

/** Clothing colours for pedestrians and riders' jackets. */
export const CLOTHING_PALETTE: readonly number[] = [
  0x2b2d31, 0x3d4a5c, 0x6b4f3a, 0x8c3b3b, 0x2f4f3a, 0x4a6d8c, 0xb8b1a3, 0x5b4b7a, 0x1f2a44, 0x9a8c6c,
];

/** A small integer hash (Wang/Jenkins style), so a variant or a colour is stable per actor. */
export function hashId(id: number, salt: number): number {
  let h = (id ^ 0x9e3779b9 ^ Math.imul(salt, 0x85ebca6b)) >>> 0;
  h = Math.imul(h ^ (h >>> 16), 0x7feb352d) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x846ca68b) >>> 0;
  h = (h ^ (h >>> 16)) >>> 0;
  return h / 4294967296;
}
