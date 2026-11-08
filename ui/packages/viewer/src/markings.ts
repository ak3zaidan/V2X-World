/**
 * Road markings that follow the lanes: what a driver on a Manhattan street actually sees painted
 * on it.
 *
 * The previous renderer drew one solid line down the right edge of every lane and a yellow line on
 * the left edge of every rightmost lane — so a four-lane avenue had four solid white lines and a
 * yellow one between its first and second lanes, which is no road anywhere. The rules here are the
 * MUTCD's (2009, Part 3), which New York City's DOT Street Design Manual follows:
 *
 * - **Lane lines** between two lanes going the same way are broken white: 10-ft segments with
 *   30-ft gaps (§3A.06, 3.05 m and 9.14 m); a bus lane is separated by a solid white line (§3D.02).
 * - **Edge lines**: the right edge of the rightmost travel lane is solid white (§3B.06). The left
 *   edge of a one-way roadway is solid yellow (§3B.06).
 * - **Centre line**: where the lanes of the other direction are alongside, a double solid yellow
 *   (§3B.01) — each direction draws its half.
 * - **Stop lines** (§3B.16): solid white, 12–24 in deep (0.45 m drawn), across every approach lane
 *   of a signalised or stop-controlled junction, at the lane's end.
 * - **Lane-use arrows** (§3B.20): on an approach whose lanes do not all make the same movements,
 *   each lane carries the arrow of the movements its connectors make (a left-only lane a left
 *   arrow, a shared lane a combined arrow), 2.4 m long, 6 m before the stop line.
 * - **Crosswalks** (§3B.18): high-visibility "ladder" markings — the two transverse lines and
 *   24-in longitudinal bars at 24-in spacing — which is NYC DOT's standard at signalised crossings.
 * - **Bus and bike lanes** take the colours NYC paints them: terra-cotta red for bus lanes (FHWA
 *   Interim Approval IA-22, red-coloured transit-only lanes) and green for bike lanes (IA-14).
 *
 * Line width is 6 in (0.15 m), NYC's standard; the MUTCD allows 4–6 in for normal lines.
 */

import type { VwpWorld } from "@vwp/protocol";
import type { MeshBuilder } from "./geometry.js";

/** Lane types, vwp-v1 §4.3. */
export const LANE_TYPE = {
  DRIVE: 0,
  BIKE: 1,
  SIDEWALK: 2,
  BUS: 3,
  PARKING: 4,
  INTERNAL: 5,
  CROSSING: 6,
} as const;

const LINE_W = 0.15;
const DASH_M = 3.05;
const GAP_M = 9.14;
const STOP_BAR_DEPTH = 0.45;
const ARROW_LEN = 2.4;
const ARROW_BACK = 6;

/** Movements a lane's connectors make. */
export const MOVE = { LEFT: 1, STRAIGHT: 2, RIGHT: 4, UTURN: 8 } as const;

/** What a marking pass needs from the world renderer. */
export interface MarkingSink {
  /** The builder for the tile containing `(x, y)`. */
  readonly at: (x: number, y: number) => MeshBuilder;
  /** Height of paint above the lane surface, metres. */
  readonly z: number;
  readonly white: readonly [number, number, number];
  readonly yellow: readonly [number, number, number];
}

/** What {@link buildLaneMarkings} drew, for the build report and tests. */
export interface MarkingReport {
  readonly laneLines: number;
  readonly edgeLines: number;
  readonly centreLines: number;
  readonly stopBars: number;
  readonly arrows: number;
  /** Approach lanes whose movements were inferred from their connectors. */
  readonly approaches: number;
}

interface LaneGeom {
  readonly i: number;
  readonly off: number;
  readonly n: number;
}

function heading(xs: Float32Array, ys: Float32Array, a: number, b: number): number {
  return Math.atan2(ys[b] - ys[a], xs[b] - xs[a]);
}

function wrap(a: number): number {
  return a - Math.floor(a / (2 * Math.PI) + 0.5) * 2 * Math.PI;
}

/** What {@link laneMovements} finds: per lane index, its movements and the junction it enters. */
export interface LaneMovements {
  /** {@link MOVE} bits of the connectors that leave lane `i`'s end; 0 for a lane that is not an approach. */
  readonly moves: Int32Array;
  /** The junction id lane `i` enters, or −1. */
  readonly approachJunction: Int32Array;
}

/**
 * The movements each travel lane makes at its end, read from the internal connector lanes that
 * start where it ends and leave in its direction: left, straight, right or U-turn by the angle
 * the connector turns through. Shared by the lane-use arrows here and the signal heads' lens
 * shapes (`signals.ts`), which must agree on what a lane is for.
 */
export function laneMovements(world: VwpWorld): LaneMovements {
  const lanes = world.lanes;
  const xs = world.lanePoints.x;
  const ys = world.lanePoints.y;
  const L = lanes.count;
  const carries = (t: number): boolean => t === LANE_TYPE.DRIVE || t === LANE_TYPE.BUS;
  const conStart = new Map<string, number[]>();
  const key1 = (x: number, y: number): string => `${Math.round(x)},${Math.round(y)}`;
  for (let i = 0; i < L; i++) {
    if (lanes.laneType[i] !== LANE_TYPE.INTERNAL || lanes.pointCount[i] < 2) continue;
    const o = lanes.pointOff[i];
    const k = key1(xs[o], ys[o]);
    let list = conStart.get(k);
    if (!list) {
      list = [];
      conStart.set(k, list);
    }
    list.push(i);
  }
  const moves = new Int32Array(L);
  const approachJunction = new Int32Array(L).fill(-1);
  for (let i = 0; i < L; i++) {
    if (!carries(lanes.laneType[i]) || lanes.pointCount[i] < 2) continue;
    const o = lanes.pointOff[i];
    const last = o + lanes.pointCount[i] - 1;
    const hIn = heading(xs, ys, last - 1, last);
    const rx = Math.round(xs[last]);
    const ry = Math.round(ys[last]);
    let m = 0;
    for (let ox = -1; ox <= 1; ox++) {
      for (let oy = -1; oy <= 1; oy++) {
        for (const c of conStart.get(`${rx + ox},${ry + oy}`) ?? []) {
          const co = lanes.pointOff[c];
          const cn = lanes.pointCount[c];
          if (Math.hypot(xs[co] - xs[last], ys[co] - ys[last]) > 1.2) continue;
          const h0 = heading(xs, ys, co, co + 1);
          if (Math.cos(h0 - hIn) < 0.5) continue;
          const h1 = heading(xs, ys, co + cn - 2, co + cn - 1);
          const d = wrap(h1 - hIn);
          if (Math.abs(d) > 2.6) m |= MOVE.UTURN;
          else if (d > 0.5) m |= MOVE.LEFT;
          else if (d < -0.5) m |= MOVE.RIGHT;
          else m |= MOVE.STRAIGHT;
          const jid = lanes.junctionId[c];
          if (jid !== 0xffffffff) approachJunction[i] = jid;
        }
      }
    }
    moves[i] = m;
  }
  return { moves, approachJunction };
}

/**
 * A line `offset` metres left of lane `l`'s centreline, solid or dashed, `LINE_W` wide.
 * `phase` shifts the dash pattern so adjacent lines do not all start at the same point.
 */
function laneLine(
  sink: MarkingSink, world: VwpWorld, l: LaneGeom, offset: number, dashed: boolean,
  c: readonly [number, number, number],
): void {
  const xs = world.lanePoints.x;
  const ys = world.lanePoints.y;
  const zs = world.lanePoints.z;
  const hw = LINE_W / 2;
  // Walk the polyline by arc length, emitting quads for the "on" stretches.
  let s = 0;
  const period = DASH_M + GAP_M;
  for (let k = 0; k + 1 < l.n; k++) {
    const a = l.off + k;
    const b = a + 1;
    const dx = xs[b] - xs[a];
    const dy = ys[b] - ys[a];
    const len = Math.hypot(dx, dy);
    if (len < 1e-4) continue;
    const ux = dx / len;
    const uy = dy / len;
    const nx = -uy;
    const ny = ux;
    let t = 0;
    while (t < len - 1e-4) {
      let on: boolean;
      let next: number;
      if (dashed) {
        const phase = (s + t) % period;
        on = phase < DASH_M;
        // At least a millimetre of progress: at a pattern boundary the remainder can be smaller
        // than the spacing of floats near `t`, and `t + remainder === t` would never advance.
        next = Math.min(len, Math.max(t + 1e-3, t + (on ? DASH_M - phase : period - phase)));
      } else {
        on = true;
        next = len;
      }
      if (on && next - t > 0.05) {
        const x0 = xs[a] + ux * t + nx * offset;
        const y0 = ys[a] + uy * t + ny * offset;
        const x1 = xs[a] + ux * next + nx * offset;
        const y1 = ys[a] + uy * next + ny * offset;
        const z0 = (zs ? zs[a] + (zs[b] - zs[a]) * (t / len) : 0) + sink.z;
        const z1 = (zs ? zs[a] + (zs[b] - zs[a]) * (next / len) : 0) + sink.z;
        const bld = sink.at(x0, y0);
        const v0 = bld.addVertex(x0 + nx * hw, y0 + ny * hw, z0, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
        const v1 = bld.addVertex(x0 - nx * hw, y0 - ny * hw, z0, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
        const v2 = bld.addVertex(x1 - nx * hw, y1 - ny * hw, z1, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
        const v3 = bld.addVertex(x1 + nx * hw, y1 + ny * hw, z1, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
        bld.addQuad(v0, v1, v2, v3);
      }
      t = next;
    }
    s += len;
  }
}

/** A flat quad centred at `(cx, cy)`, `along` metres along `(ux, uy)` and `across` across it. */
function quad(
  b: MeshBuilder, cx: number, cy: number, z: number, ux: number, uy: number, along: number, across: number,
  c: readonly [number, number, number],
): void {
  const px = -uy;
  const py = ux;
  const ha = along / 2;
  const hc = across / 2;
  const v0 = b.addVertex(cx - ux * ha - px * hc, cy - uy * ha - py * hc, z, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
  const v1 = b.addVertex(cx + ux * ha - px * hc, cy + uy * ha - py * hc, z, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
  const v2 = b.addVertex(cx + ux * ha + px * hc, cy + uy * ha + py * hc, z, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
  const v3 = b.addVertex(cx - ux * ha + px * hc, cy - uy * ha + py * hc, z, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
  b.addQuad(v0, v1, v2, v3);
}

/** A triangle in the ground plane. */
function tri(
  b: MeshBuilder, ax: number, ay: number, bx: number, by: number, cx: number, cy: number, z: number,
  c: readonly [number, number, number],
): void {
  const v0 = b.addVertex(ax, ay, z, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
  const v1 = b.addVertex(bx, by, z, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
  const v2 = b.addVertex(cx, cy, z, 0, 0, 1, 0, 0, c[0], c[1], c[2]);
  // Counter-clockwise from above.
  if ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax) >= 0) b.addTriangle(v0, v1, v2);
  else b.addTriangle(v0, v2, v1);
}

/**
 * A lane-use arrow at `(x, y)` pointing along `h`: a shaft, and a head for each movement in
 * `moves` — straight ahead, bent left or right, or hooked back for a U-turn.
 */
function arrow(b: MeshBuilder, x: number, y: number, z: number, h: number, moves: number, c: readonly [number, number, number]): void {
  const ux = Math.cos(h);
  const uy = Math.sin(h);
  const px = -uy;
  const py = ux;
  const shaftW = 0.15;
  const head = 0.8;
  const straightLen = ARROW_LEN - head;
  // Shaft from the tail to the head's base.
  const tailX = x - ux * ARROW_LEN / 2;
  const tailY = y - uy * ARROW_LEN / 2;
  const shaftEnd = moves & MOVE.STRAIGHT ? straightLen : straightLen * 0.55;
  quad(b, tailX + ux * shaftEnd / 2, tailY + uy * shaftEnd / 2, z, ux, uy, shaftEnd, shaftW, c);
  if (moves & MOVE.STRAIGHT) {
    const bx = tailX + ux * straightLen;
    const by = tailY + uy * straightLen;
    tri(b, bx + px * 0.4, by + py * 0.4, bx - px * 0.4, by - py * 0.4, bx + ux * head, by + uy * head, z, c);
  }
  for (const side of [1, -1]) {
    const bit = side > 0 ? MOVE.LEFT : MOVE.RIGHT;
    if (!(moves & bit)) continue;
    // A bent arm from the shaft's middle, turning 60° to the side, ending in a head.
    const sx = tailX + ux * shaftEnd;
    const sy = tailY + uy * shaftEnd;
    const ang = h + side * (Math.PI / 3);
    const ax = Math.cos(ang);
    const ay = Math.sin(ang);
    const arm = 0.7;
    quad(b, sx + ax * arm / 2, sy + ay * arm / 2, z, ax, ay, arm, shaftW, c);
    const hx = sx + ax * arm;
    const hy = sy + ay * arm;
    const qx = -ay;
    const qy = ax;
    tri(b, hx + qx * 0.35, hy + qy * 0.35, hx - qx * 0.35, hy - qy * 0.35, hx + ax * 0.6, hy + ay * 0.6, z, c);
  }
  if (moves & MOVE.UTURN) {
    // A hook to the left, back towards the tail.
    const sx = tailX + ux * shaftEnd;
    const sy = tailY + uy * shaftEnd;
    quad(b, sx + px * 0.35, sy + py * 0.35, z, px, py, 0.7, shaftW, c);
    const hx = sx + px * 0.7;
    const hy = sy + py * 0.7;
    tri(b, hx + ux * 0.0 + px * 0.35, hy + py * 0.35, hx - px * 0.35, hy - py * 0.35, hx - ux * 0.6, hy - uy * 0.6, z, c);
  }
}

/**
 * Every lane line, edge line, centre line, stop line and lane-use arrow of `world`, into the
 * builders `sink.at` hands out.
 *
 * `controlOf(junctionId)` is the junction's §4.5 control (2 signal, 3 stop); a stop line is drawn
 * only where one of those controls the approach.
 */
export function buildLaneMarkings(world: VwpWorld, sink: MarkingSink): MarkingReport {
  const lanes = world.lanes;
  const xs = world.lanePoints.x;
  const ys = world.lanePoints.y;
  const zs = world.lanePoints.z;
  const L = lanes.count;

  const carries = (t: number): boolean => t === LANE_TYPE.DRIVE || t === LANE_TYPE.BUS;

  // Lanes by edge, ordered right to left.
  const byEdge = new Map<number, number[]>();
  for (let i = 0; i < L; i++) {
    if (lanes.pointCount[i] < 2) continue;
    const t = lanes.laneType[i];
    if (!carries(t) && t !== LANE_TYPE.BIKE && t !== LANE_TYPE.PARKING) continue;
    let list = byEdge.get(lanes.edgeId[i]);
    if (!list) {
      list = [];
      byEdge.set(lanes.edgeId[i], list);
    }
    list.push(i);
  }
  for (const list of byEdge.values()) list.sort((a, b) => lanes.indexInEdge[a] - lanes.indexInEdge[b]);

  const geom = (i: number): LaneGeom => ({ i, off: lanes.pointOff[i], n: lanes.pointCount[i] });

  // The left edge of each edge's leftmost travel lane, sampled at its middle, on a 10 m grid: an
  // edge is two-way where another edge's left edge runs the other way within a metre and a half.
  const CELL = 10;
  const leftEdges = new Map<string, { edge: number; x: number; y: number; h: number }[]>();
  const leftmost = new Map<number, number>();
  for (const [edge, list] of byEdge) {
    let lm = -1;
    for (const i of list) if (carries(lanes.laneType[i])) lm = i;
    if (lm < 0) continue;
    leftmost.set(edge, lm);
    const g = geom(lm);
    const mid = g.off + Math.floor((g.n - 1) / 2);
    const h = heading(xs, ys, mid, Math.min(mid + 1, g.off + g.n - 1));
    const hw = lanes.widthM[lm] / 2;
    const x = xs[mid] - Math.sin(h) * hw;
    const y = ys[mid] + Math.cos(h) * hw;
    const key = `${Math.floor(x / CELL)},${Math.floor(y / CELL)}`;
    let cell = leftEdges.get(key);
    if (!cell) {
      cell = [];
      leftEdges.set(key, cell);
    }
    cell.push({ edge, x, y, h });
  }
  const twoWay = (edge: number): boolean => {
    const lm = leftmost.get(edge);
    if (lm === undefined) return false;
    const g = geom(lm);
    const mid = g.off + Math.floor((g.n - 1) / 2);
    const h = heading(xs, ys, mid, Math.min(mid + 1, g.off + g.n - 1));
    const hw = lanes.widthM[lm] / 2;
    const x = xs[mid] - Math.sin(h) * hw;
    const y = ys[mid] + Math.cos(h) * hw;
    const cx = Math.floor(x / CELL);
    const cy = Math.floor(y / CELL);
    for (let ox = -1; ox <= 1; ox++) {
      for (let oy = -1; oy <= 1; oy++) {
        for (const e of leftEdges.get(`${cx + ox},${cy + oy}`) ?? []) {
          if (e.edge === edge) continue;
          if (Math.hypot(e.x - x, e.y - y) > 3.5) continue;
          if (Math.cos(e.h - h) < -0.8) return true;
        }
      }
    }
    return false;
  };

  let laneLines = 0;
  let edgeLines = 0;
  let centreLines = 0;
  for (const [edge, list] of byEdge) {
    const travel = list.filter((i) => carries(lanes.laneType[i]));
    if (travel.length === 0) continue;
    // Right edge of the rightmost travel lane.
    const right = travel[0];
    laneLine(sink, world, geom(right), -(lanes.widthM[right] / 2 - LINE_W), false, sink.white);
    edgeLines++;
    // Between neighbouring travel lanes.
    for (let k = 0; k + 1 < travel.length; k++) {
      const a = travel[k];
      const b = travel[k + 1];
      const solid = lanes.laneType[a] === LANE_TYPE.BUS || lanes.laneType[b] === LANE_TYPE.BUS;
      laneLine(sink, world, geom(a), lanes.widthM[a] / 2, !solid, sink.white);
      laneLines++;
    }
    // Left edge: half a double yellow where the other direction is alongside, a yellow edge line
    // on a one-way street.
    const left = travel[travel.length - 1];
    const hw = lanes.widthM[left] / 2;
    if (twoWay(edge)) {
      laneLine(sink, world, geom(left), hw - LINE_W * 0.5 - 0.06, false, sink.yellow);
      centreLines++;
    } else {
      laneLine(sink, world, geom(left), hw - LINE_W, false, sink.yellow);
      edgeLines++;
    }
  }

  // Approach lanes and their movements, from the connectors that start where they end.
  const junctionControl = new Map<number, number>();
  for (let j = 0; j < world.junctions.count; j++) {
    const jn = world.junctions.at(j);
    junctionControl.set(jn.junctionId, jn.control);
  }
  const { moves, approachJunction } = laneMovements(world);
  let approaches = 0;
  for (let i = 0; i < L; i++) if (moves[i] !== 0) approaches++;

  // Stop lines and arrows.
  let stopBars = 0;
  let arrows = 0;
  for (const list of byEdge.values()) {
    const travel = list.filter((i) => carries(lanes.laneType[i]) && moves[i] !== 0);
    if (travel.length === 0) continue;
    const mixed = travel.length >= 2 && travel.some((i) => moves[i] !== moves[travel[0]]);
    for (const i of travel) {
      const o = lanes.pointOff[i];
      const n = lanes.pointCount[i];
      const last = o + n - 1;
      const h = heading(xs, ys, last - 1, last);
      const ux = Math.cos(h);
      const uy = Math.sin(h);
      const z = (zs ? zs[last] : 0) + sink.z;
      const control = junctionControl.get(approachJunction[i]) ?? 0;
      const laneLen = (() => {
        let s = 0;
        for (let k = o; k < last; k++) s += Math.hypot(xs[k + 1] - xs[k], ys[k + 1] - ys[k]);
        return s;
      })();
      if (control === 2 || control === 3) {
        const b = sink.at(xs[last], ys[last]);
        quad(b, xs[last] - ux * (STOP_BAR_DEPTH / 2 + 0.3), ys[last] - uy * (STOP_BAR_DEPTH / 2 + 0.3), z,
          ux, uy, STOP_BAR_DEPTH, lanes.widthM[i] - 0.1, sink.white);
        stopBars++;
      }
      const turnOnly = !(moves[i] & MOVE.STRAIGHT);
      if ((mixed || turnOnly) && laneLen > ARROW_BACK + ARROW_LEN + 2) {
        const b = sink.at(xs[last], ys[last]);
        const back = ARROW_BACK + ARROW_LEN / 2;
        arrow(b, xs[last] - ux * back, ys[last] - uy * back, z, h, moves[i], sink.white);
        arrows++;
      }
    }
  }
  return { laneLines, edgeLines, centreLines, stopBars, arrows, approaches };
}

/**
 * High-visibility crosswalk markings between `(x1, y1)` and `(x2, y2)`, `width` wide: two
 * transverse lines along the crosswalk's edges and 0.6 m bars at 0.6 m gaps across it, parallel to
 * the traffic (MUTCD 2009 §3B.18; NYC DOT's high-visibility standard).
 */
export function addCrosswalk(
  b: MeshBuilder, x1: number, y1: number, x2: number, y2: number, width: number, z: number,
  c: readonly [number, number, number],
): number {
  const dx = x2 - x1;
  const dy = y2 - y1;
  const len = Math.hypot(dx, dy);
  if (len < 0.5) return 0;
  const ux = dx / len;
  const uy = dy / len;
  const px = -uy;
  const py = ux;
  const hw = Math.max(1.2, width) / 2;
  // Transverse lines.
  for (const s of [1, -1]) {
    quad(b, (x1 + x2) / 2 + px * s * (hw - 0.1), (y1 + y2) / 2 + py * s * (hw - 0.1), z, ux, uy, len, 0.2, c);
  }
  // Bars: 0.6 m wide at 1.2 m pitch, centred.
  const pitch = 1.2;
  const bars = Math.max(1, Math.floor((len - 0.3) / pitch));
  const start = (len - (bars - 1) * pitch) / 2;
  for (let k = 0; k < bars; k++) {
    const t = start + k * pitch;
    quad(b, x1 + ux * t, y1 + uy * t, z, ux, uy, 0.6, 2 * hw - 0.5, c);
  }
  return bars;
}
