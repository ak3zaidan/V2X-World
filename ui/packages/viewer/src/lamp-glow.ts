/**
 * Light that vehicles throw at night: the pool a car's dipped headlamps lay on the road ahead of
 * it, and — from far away, where a vehicle is a box a few pixels long — the points of its head and
 * tail lamps, which are what a city at night actually looks like from above.
 *
 * Two draws for the whole fleet, both additive and depth-tested but not depth-writing, so they
 * brighten what is under them and never hide it:
 *
 * - **Pools**: one flat quad per lit vehicle within {@link LampGlowOptions.poolRangeM}, on the road
 *   from the front bumper to about 25 m ahead, brightest near the car and fading with distance
 *   and at the edges. A dipped beam is aimed down so that its cut-off meets the road some 30–40 m
 *   ahead (UN ECE R112 / R48 aim, −1 % inclination at 10 m); the pool stops short of that.
 * - **Points**: a white point at each front corner and a red one at each rear corner of every lit
 *   vehicle, sized in pixels, fading in with the scene's darkness.
 *
 * Nothing is drawn in daylight: at `darkness` 0 both are hidden and the loop does not run.
 */

import {
  AdditiveBlending,
  BufferAttribute,
  BufferGeometry,
  DynamicDrawUsage,
  Group,
  InstancedBufferAttribute,
  InstancedMesh,
  Points,
  ShaderMaterial,
  type Camera,
} from "three";
import type { ActorClassDef } from "./types.js";
import { ABOVE_SURFACE_OFFSET_UNITS } from "./world-render.js";

/** `lamps` bits this layer reads (vwp-v1 §3.3.5). */
const LOW_BEAM = 0x10;
const BRAKE = 0x01;

export interface LampGlowOptions {
  /** Most pools drawn. Default 1,500. */
  readonly maxPools?: number;
  /** Most lamp points drawn (four per vehicle). Default 24,000. */
  readonly maxPoints?: number;
  /** Pools are drawn for lit vehicles nearer than this, metres. Default 220. */
  readonly poolRangeM?: number;
}

export interface LampGlowInputs {
  readonly position: Float32Array;
  readonly heading: Float32Array;
  readonly classIdx: Uint8Array;
  readonly occupied: Uint8Array;
  readonly lamps: Uint8Array;
  readonly fade?: Float32Array;
  readonly count: number;
  readonly classes: readonly ActorClassDef[];
  readonly camera: Camera;
  /** 0 daylight … 1 night. */
  readonly darkness: number;
  /** Height of the road surface above the pose's z, metres. */
  readonly groundOffsetM: number;
  /** A slot not to draw (the camera is inside it). */
  readonly hiddenSlot?: number;
}

const POOL_VERTEX = /* glsl */ `
attribute vec3 iPool;
varying vec2 vUv;
varying float vStrength;
void main() {
  vUv = uv;
  vStrength = iPool.x;
  gl_Position = projectionMatrix * modelViewMatrix * instanceMatrix * vec4(position, 1.0);
}
`;

const POOL_FRAGMENT = /* glsl */ `
uniform vec3 uColor;
varying vec2 vUv;
varying float vStrength;
void main() {
  // u across the beam (-1..1), v along it (0 at the bumper .. 1 at the far end).
  float across = 1.0 - smoothstep(0.35, 1.0, abs(vUv.x * 2.0 - 1.0));
  float along = smoothstep(0.0, 0.08, vUv.y) * (1.0 - smoothstep(0.35, 1.0, vUv.y));
  float a = across * along * vStrength;
  gl_FragColor = vec4(uColor * a, 1.0);
}
`;

const POINT_VERTEX = /* glsl */ `
attribute vec3 color;
uniform float uSize;
varying vec3 vColor;
void main() {
  vColor = color;
  vec4 mv = modelViewMatrix * vec4(position, 1.0);
  gl_Position = projectionMatrix * mv;
  gl_PointSize = uSize;
}
`;

const POINT_FRAGMENT = /* glsl */ `
varying vec3 vColor;
void main() {
  vec2 d = gl_PointCoord - 0.5;
  float r = length(d) * 2.0;
  float a = 1.0 - smoothstep(0.2, 1.0, r);
  gl_FragColor = vec4(vColor * a, 1.0);
}
`;

/** Headlight pools and lamp points for every lit vehicle. */
export class LampGlow {
  readonly group = new Group();
  #pools: InstancedMesh<BufferGeometry, ShaderMaterial>;
  #poolAttr: InstancedBufferAttribute;
  #poolData: Float32Array;
  #points: Points<BufferGeometry, ShaderMaterial>;
  #pointPos: Float32Array;
  #pointCol: Float32Array;
  #maxPools: number;
  #maxPoints: number;
  #poolRange: number;
  /** Pools and points drawn by the last update. */
  poolCount = 0;
  pointCount = 0;

  constructor(options: LampGlowOptions = {}) {
    this.#maxPools = options.maxPools ?? 1500;
    this.#maxPoints = options.maxPoints ?? 24_000;
    this.#poolRange = options.poolRangeM ?? 220;
    this.group.name = "lamp-glow";

    // A unit quad: x across (−0.5..0.5), y along (0..1), in the ground plane.
    const g = new BufferGeometry();
    g.setAttribute("position", new BufferAttribute(new Float32Array([
      0, -0.5, 0, 1, -0.5, 0, 1, 0.5, 0, 0, 0.5, 0,
    ]), 3));
    g.setAttribute("uv", new BufferAttribute(new Float32Array([0, 0, 0, 1, 1, 1, 1, 0]), 2));
    g.setIndex([0, 1, 2, 0, 2, 3]);
    this.#poolData = new Float32Array(this.#maxPools * 3);
    this.#poolAttr = new InstancedBufferAttribute(this.#poolData, 3);
    this.#poolAttr.setUsage(DynamicDrawUsage);
    g.setAttribute("iPool", this.#poolAttr);
    const poolMat = new ShaderMaterial({
      vertexShader: POOL_VERTEX,
      fragmentShader: POOL_FRAGMENT,
      uniforms: { uColor: { value: [1.0, 0.9, 0.72] } },
      transparent: true,
      depthWrite: false,
      blending: AdditiveBlending,
      polygonOffset: true,
      polygonOffsetFactor: -4,
      polygonOffsetUnits: ABOVE_SURFACE_OFFSET_UNITS,
      name: "lamp-pools",
    });
    // three wants a Color-like for vec3 uniforms; an array works with ShaderMaterial.
    this.#pools = new InstancedMesh(g, poolMat, this.#maxPools);
    this.#pools.frustumCulled = false;
    this.#pools.count = 0;
    this.#pools.instanceMatrix.setUsage(DynamicDrawUsage);
    this.#pools.renderOrder = 2;

    const pg = new BufferGeometry();
    this.#pointPos = new Float32Array(this.#maxPoints * 3);
    this.#pointCol = new Float32Array(this.#maxPoints * 3);
    const pa = new BufferAttribute(this.#pointPos, 3);
    pa.setUsage(DynamicDrawUsage);
    const ca = new BufferAttribute(this.#pointCol, 3);
    ca.setUsage(DynamicDrawUsage);
    pg.setAttribute("position", pa);
    pg.setAttribute("color", ca);
    pg.setDrawRange(0, 0);
    const pointMat = new ShaderMaterial({
      vertexShader: POINT_VERTEX,
      fragmentShader: POINT_FRAGMENT,
      uniforms: { uSize: { value: 5 } },
      transparent: true,
      depthWrite: false,
      blending: AdditiveBlending,
      name: "lamp-points",
    });
    this.#points = new Points(pg, pointMat);
    this.#points.frustumCulled = false;
    this.#points.renderOrder = 3;
    this.group.add(this.#pools, this.#points);
    this.group.visible = false;
  }

  /** Lay this frame's pools and points. Cheap and a no-op in daylight. */
  update(inp: LampGlowInputs): void {
    const dark = Math.max(0, Math.min(1, inp.darkness));
    if (dark < 0.05) {
      this.group.visible = false;
      this.poolCount = 0;
      this.pointCount = 0;
      return;
    }
    this.group.visible = true;
    const cam = inp.camera.matrixWorld.elements;
    const cx = cam[12];
    const cy = cam[13];
    const range2 = this.#poolRange * this.#poolRange;
    const m = this.#pools.instanceMatrix.array as Float32Array;
    const pd = this.#poolData;
    const pp = this.#pointPos;
    const pc = this.#pointCol;
    let pools = 0;
    let points = 0;
    const nClasses = inp.classes.length;
    for (let s = 0; s < inp.count; s++) {
      if (inp.occupied[s] === 0 || s === inp.hiddenSlot) continue;
      const lamps = inp.lamps[s];
      if ((lamps & LOW_BEAM) === 0) continue;
      const def = inp.classes[inp.classIdx[s] < nClasses ? inp.classIdx[s] : 0];
      if (!def || def.category === 1) continue;
      const fade = inp.fade ? inp.fade[s] : 1;
      if (fade <= 0.02) continue;
      const p = s * 3;
      const x = inp.position[p];
      const y = inp.position[p + 1];
      const z = inp.position[p + 2] + inp.groundOffsetM;
      const h = inp.heading[s];
      const c = Math.cos(h);
      const sn = Math.sin(h);
      const hl = def.lengthM / 2;
      const hw = Math.min(def.widthM / 2 - 0.25, 0.75);
      const twoWheeler = def.lengthM < 2.8;
      // Lamp points: front white, rear red (brighter when braking).
      const lampZ = z + Math.min(0.75, def.heightM * 0.45);
      const brake = (lamps & BRAKE) !== 0 ? 1.8 : 1;
      const sides = twoWheeler ? [0] : [hw, -hw];
      for (const off of sides) {
        if (points + 2 > this.#maxPoints) break;
        const lx = -sn * off;
        const ly = c * off;
        let q = points * 3;
        pp[q] = x + c * hl + lx; pp[q + 1] = y + sn * hl + ly; pp[q + 2] = lampZ;
        pc[q] = 1.0 * dark * fade; pc[q + 1] = 0.95 * dark * fade; pc[q + 2] = 0.82 * dark * fade;
        points++;
        q = points * 3;
        pp[q] = x - c * hl + lx; pp[q + 1] = y - sn * hl + ly; pp[q + 2] = lampZ;
        pc[q] = 0.9 * brake * dark * fade; pc[q + 1] = 0.05 * dark * fade; pc[q + 2] = 0.03 * dark * fade;
        points++;
      }
      // A pool ahead of it, if it is near enough to matter.
      const dx = x - cx;
      const dy = y - cy;
      if (dx * dx + dy * dy > range2 || pools >= this.#maxPools) continue;
      const len = twoWheeler ? 14 : 24;
      const width = twoWheeler ? 4 : 7;
      const o = pools * 16;
      // Local x along the beam (scaled by its length), local y across (by its width); the quad's
      // own x runs 0..1 from the bumper forward.
      m[o] = c * len; m[o + 1] = sn * len; m[o + 2] = 0; m[o + 3] = 0;
      m[o + 4] = -sn * width; m[o + 5] = c * width; m[o + 6] = 0; m[o + 7] = 0;
      m[o + 8] = 0; m[o + 9] = 0; m[o + 10] = 1; m[o + 11] = 0;
      m[o + 12] = x + c * hl; m[o + 13] = y + sn * hl; m[o + 14] = z + 0.02; m[o + 15] = 1;
      pd[pools * 3] = 0.55 * dark * fade;
      pools++;
    }
    this.#pools.count = pools;
    this.#pools.instanceMatrix.needsUpdate = pools > 0;
    this.#poolAttr.needsUpdate = pools > 0;
    const pg = this.#points.geometry;
    pg.setDrawRange(0, points);
    (pg.getAttribute("position") as BufferAttribute).needsUpdate = points > 0;
    (pg.getAttribute("color") as BufferAttribute).needsUpdate = points > 0;
    this.poolCount = pools;
    this.pointCount = points;
  }

  dispose(): void {
    this.#pools.geometry.dispose();
    this.#pools.material.dispose();
    this.#pools.dispose();
    this.#points.geometry.dispose();
    this.#points.material.dispose();
    this.group.removeFromParent();
  }
}
