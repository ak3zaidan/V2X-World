/**
 * The glitch hunter: watches a {@link Viewer} frame by frame and names every visual defect it can
 * measure, by class.
 *
 * The owner's standard is "extremely smooth and perfect, with zero bugs or glitches". That is not a
 * standard anyone can hold a renderer to by looking at it — a glitch that happens once a minute on
 * one car in a crowd is invisible in a screenshot and obvious in a demo — so every class below is a
 * rule evaluated on every drawn actor of every frame, and the report is a count per class. The same
 * object runs headless (the vitest replay of a real engine capture, `test/glitch/`) and in the page
 * (`window.__vwpStudio.viewer`), because it reads only what the viewer itself drew.
 *
 * ## The classes
 *
 * | class | rule |
 * |---|---|
 * | `pop` | an actor appears or vanishes **inside** the view — not at its edge, not by culling — within `nearM` of the camera |
 * | `teleport` | a drawn actor moves further in one frame than its speed allows: `> max(0.75 m, 2.5·v·dt + 0.3 m)` |
 * | `stutter` | frame-to-frame screen-space jerk of a tracked actor, `|Δ²p| > max(1.5 px, 0.5·|Δp|)` while it moves more than 1 px a frame; also the camera itself |
 * | `heading_snap` | a vehicle's drawn heading turns more than 0.12 rad (6.9°) in one frame, a person's more than 0.35 rad, or a stationary vehicle rotates |
 * | `overlap_vehicle` | two drawn vehicle bodies (length × width boxes) interpenetrate by more than 5 cm |
 * | `overlap_building` | a drawn vehicle's footprint is inside a building below its roof, and that building has no road through it |
 * | `overlap_pedestrian` | a drawn vehicle and a drawn person interpenetrate by more than 5 cm |
 * | `z_fighting` | the depth buffer cannot resolve the smallest gap between overlapping road layers somewhere the camera can see |
 * | `flicker` | an actor drawn, not drawn, drawn again in three consecutive frames inside the view; a lamp that changes and changes back within 0.3 s; a ghosted building toggling |
 * | `lod_pop` | an actor changes detail level while it is larger than 30 px on screen, or changes it three times in a second |
 * | `camera_clip` | the camera or its near plane is inside a building or under the ground, inside a drawn vehicle, or a wall stands between a chase camera and its subject |
 * | `empty_frame` | a non-finite camera, a view that sees no part of the world, or a wall filling the view |
 * | `subject_lost` | in chase or dashboard, the followed actor is live but not in the frame |
 * | `chase_framing` | the chase camera looks down on its subject steeper than 35°, or the subject fills less than 6 % or more than 70 % of the frame's height |
 *
 * Each overlap and teleport also says whether the engine's own data had it (`cause: "engine"`: the
 * bodies overlap in the newest snapshot, or the interpolator snapped a real discontinuity) or only
 * the drawing did (`cause: "viewer"`). A glitch the engine sent is still a glitch on screen, but it is
 * fixed in a different place.
 *
 * ## What it cannot see
 *
 * Pixels. Rasterisation artefacts — a shader that outputs black, a mipmap shimmer — need the
 * browser; the Playwright pass reads the canvas for those (`e2e/glitch.spec.ts`). Everything here is
 * geometry, which is where the owner's reports have come from.
 */

import { Vector3 } from "three";
import type { PoseBuffer } from "@vwp/protocol";
import type { Viewer } from "./scene.js";

const SCRATCH_DIR = new Vector3();

/** Every class the hunter counts, in report order. */
export const GLITCH_CLASSES = [
  "pop",
  "teleport",
  "stutter",
  "heading_snap",
  "overlap_vehicle",
  "overlap_building",
  "overlap_pedestrian",
  "z_fighting",
  "flicker",
  "lod_pop",
  "camera_clip",
  "empty_frame",
  "subject_lost",
  "chase_framing",
] as const;

export type GlitchClass = (typeof GLITCH_CLASSES)[number];

/** One detected defect. */
export interface GlitchEvent {
  readonly cls: GlitchClass;
  /** Frame index since the hunter started. */
  readonly frame: number;
  /** The viewer's render clock, seconds. */
  readonly timeS: number;
  /** The camera mode the frame was drawn in. */
  readonly mode: string;
  readonly actorId?: number;
  /** Whether the engine's own data had it, or only the drawing. */
  readonly cause?: "engine" | "viewer";
  readonly detail: string;
}

/** What {@link GlitchHunter.report} returns. */
export interface GlitchReport {
  readonly frames: number;
  readonly framesByMode: Record<string, number>;
  /** Events per class. */
  readonly counts: Record<GlitchClass, number>;
  /** Events per class and camera mode. */
  readonly countsByMode: Record<string, Partial<Record<GlitchClass, number>>>;
  /** Events per class whose cause is the engine's data. */
  readonly engineCaused: Partial<Record<GlitchClass, number>>;
  /** The first few events of each class, for a human. */
  readonly examples: Partial<Record<GlitchClass, GlitchEvent[]>>;
  /** The first few events of each class in each camera mode. */
  readonly examplesByMode: Record<string, Partial<Record<GlitchClass, GlitchEvent[]>>>;
  /** Supporting measurements, not glitches. */
  readonly metrics: {
    /** Drawn actors examined, summed over frames. */
    readonly actorFrames: number;
    /** RMS frame-to-frame screen jerk of tracked actors, px. */
    readonly jerkRmsPx: number;
    /** Largest single screen jerk seen, px. */
    readonly jerkMaxPx: number;
    /** RMS screen jerk of the followed actor in chase/dashboard, px. */
    readonly followJerkRmsPx: number;
    /** Largest drawn per-frame heading change of a vehicle, rad. */
    readonly headingStepMaxRad: number;
    /** Person–person overlaps (people brush past each other; reported, not counted). */
    readonly pedestrianPairsOverlapping: number;
    /** Distinct frames with at least one stutter event, and those within 0.5 s of a camera flight's end. */
    readonly stutterFrames: number;
    readonly stutterFramesAfterFlight: number;
    /** When those frames were (render clock, s), the first 40, with the mode and the worst event. */
    readonly stutterFrameList: readonly string[];
    /** Frame-time statistics of the clock the frames were drawn on, ms. */
    readonly frameMsMean: number;
    readonly frameMsMax: number;
  };
}

/** Tuning for {@link GlitchHunter}. */
export interface GlitchHunterOptions {
  /** Only actors nearer than this are judged for overlaps, metres. Default 220. */
  readonly nearM?: number;
  /**
   * An actor smaller than this on screen (its larger body dimension, px) is not judged for pops,
   * stutter, flicker or LOD: a speck. Default 3.
   */
  readonly minPx?: number;
  /** Width of the border in which an actor may enter or leave the view without being a pop, px. Default 24. */
  readonly edgePx?: number;
  /** Events kept per class in the report. Default 6. */
  readonly examplesPerClass?: number;
  /** The depth-buffer bits the renderer has. Default 24 (WebGL's usual depth buffer). */
  readonly depthBits?: number;
  /**
   * The smallest vertical gap between two road layers that overlap, metres. Default the one the
   * world renderer draws with (`WorldRenderer.minLayerGapM`).
   */
  readonly minLayerGapM?: number;
}

const NO_ID = 0xffffffff;
const TAU = Math.PI * 2;

function wrap(a: number): number {
  return a - Math.floor(a / TAU + 0.5) * TAU;
}

/**
 * Penetration depth of two boxes on the ground plane (separating-axis test over the four edge
 * normals), metres; 0 when they are apart.
 */
export function obbPenetration(
  ax: number, ay: number, ahl: number, ahw: number, ac: number, as: number,
  bx: number, by: number, bhl: number, bhw: number, bc: number, bs: number,
): number {
  const dx = bx - ax;
  const dy = by - ay;
  let best = Infinity;
  // Axes: a's forward and left, b's forward and left.
  const axes = [ac, as, -as, ac, bc, bs, -bs, bc];
  for (let k = 0; k < 8; k += 2) {
    const ux = axes[k];
    const uy = axes[k + 1];
    const d = Math.abs(dx * ux + dy * uy);
    const ra = ahl * Math.abs(ac * ux + as * uy) + ahw * Math.abs(-as * ux + ac * uy);
    const rb = bhl * Math.abs(bc * ux + bs * uy) + bhw * Math.abs(-bs * ux + bc * uy);
    const o = ra + rb - d;
    if (o <= 0) return 0;
    if (o < best) best = o;
  }
  return best;
}

/**
 * Watches a viewer; call {@link afterFrame} after every `viewer.renderFrame`, and
 * {@link observeSnapshot} with the pose buffer whenever the viewer captures one (for the engine/
 * viewer attribution of overlaps).
 */
export class GlitchHunter {
  readonly viewer: Viewer;
  readonly nearM: number;
  readonly minPx: number;
  readonly edgePx: number;
  readonly examplesPerClass: number;
  readonly depthBits: number;
  readonly minLayerGapM: number;

  #frame = 0;
  #framesByMode: Record<string, number> = {};
  #counts = Object.fromEntries(GLITCH_CLASSES.map((c) => [c, 0])) as Record<GlitchClass, number>;
  #byMode: Record<string, Partial<Record<GlitchClass, number>>> = {};
  #engine: Partial<Record<GlitchClass, number>> = {};
  #examples: Partial<Record<GlitchClass, GlitchEvent[]>> = {};
  #examplesByMode: Record<string, Partial<Record<GlitchClass, GlitchEvent[]>>> = {};
  #listeners: ((e: GlitchEvent) => void)[] = [];

  // Per-slot tracking, indexed by slot.
  #cap = 0;
  #id = new Uint32Array(0);
  #tracked = new Uint8Array(0); // consecutive frames with a screen position, saturating
  #drawn = new Uint8Array(0); // bit0 this frame, bit1 last, bit2 the one before
  #interior = new Uint8Array(0); // same bit history, "inside the view's interior"
  #sx = new Float64Array(0);
  #sy = new Float64Array(0);
  #psx = new Float64Array(0);
  #psy = new Float64Array(0);
  #wx = new Float64Array(0);
  #wy = new Float64Array(0);
  #wh = new Float64Array(0);
  #wv = new Float64Array(0);
  /**
   * This frame the street camera cut with its subject's data jump: every other actor moves on
   * screen with it, and that stutter is the data's, not the drawing's.
   */
  #frameDataCut = false;
  #prevVp = new Float64Array(16);
  #prevVpValid = false;
  #transitPrev = false;
  #lastStutterFrame = -1;
  #stutterFrames = 0;
  #stutterFramesAfterFlight = 0;
  #stutterFrameList: string[] = [];
  #flightEndedAt = -Infinity;
  #wasInTransit = false;
  /** Whether the followed slot was snapped last frame. */
  #followSnapPrev = false;
  /** Whether the interpolator snapped each slot last frame. */
  #snapPrev = new Uint8Array(0);
  /** The fade each slot was last drawn at. */
  #fade = new Float32Array(0);
  #lod = new Int8Array(0);
  #lodT0 = new Float64Array(0);
  #lodT1 = new Float64Array(0);
  #overlapUntil = new Float64Array(0);
  #buildingOverlap = new Uint8Array(0);

  // Pair episodes: "a,b" → last frame seen overlapping.
  #pairs = new Map<string, number>();
  // Newest raw snapshot, for attribution.
  #rawPos = new Float32Array(0);
  #rawHead = new Float32Array(0);
  #rawId = new Uint32Array(0);
  #rawCount = 0;

  // Signals: head → last phase, the phase before it, and when it changed.
  #sigPhase = new Uint8Array(0);
  #sigPrev = new Uint8Array(0);
  #sigChangedAt = new Float64Array(0);
  #ghostHist: number[] = [];
  #ghostChangedAt = -Infinity;

  /** The previous frame's dt, seconds. */
  #prevDt = 1 / 60;
  // Camera history for camera stutter.
  #cam = [NaN, NaN, NaN, NaN, NaN, NaN];
  #camValid = 0;

  // Metrics.
  #actorFrames = 0;
  #jerkSum2 = 0;
  #jerkN = 0;
  #jerkMax = 0;
  #followJerkSum2 = 0;
  #followJerkN = 0;
  #headingStepMax = 0;
  #pedPairs = 0;
  #frameMsSum = 0;
  #frameMsMax = 0;
  #clipEpisode = false;
  #lostEpisode = false;
  #framingEpisode = false;
  #zEpisode = false;
  #emptyEpisode = false;

  constructor(viewer: Viewer, options: GlitchHunterOptions = {}) {
    this.viewer = viewer;
    this.nearM = options.nearM ?? 220;
    this.minPx = options.minPx ?? 3;
    this.edgePx = options.edgePx ?? 24;
    this.examplesPerClass = options.examplesPerClass ?? 6;
    this.depthBits = options.depthBits ?? 24;
    this.minLayerGapM = options.minLayerGapM ?? viewer.worldRenderer.minLayerGapM;
  }

  /** Be told of every event as it is found. */
  onEvent(listener: (e: GlitchEvent) => void): () => void {
    this.#listeners.push(listener);
    return () => {
      this.#listeners = this.#listeners.filter((l) => l !== listener);
    };
  }

  /** Forget every count and all tracking. */
  reset(): void {
    this.#frame = 0;
    this.#framesByMode = {};
    for (const c of GLITCH_CLASSES) this.#counts[c] = 0;
    this.#byMode = {};
    this.#engine = {};
    this.#examples = {};
    this.#examplesByMode = {};
    this.#drawn.fill(0);
    this.#interior.fill(0);
    this.#tracked.fill(0);
    this.#id.fill(NO_ID);
    this.#pairs.clear();
    this.#camValid = 0;
    this.#actorFrames = 0;
    this.#jerkSum2 = 0;
    this.#jerkN = 0;
    this.#jerkMax = 0;
    this.#followJerkSum2 = 0;
    this.#followJerkN = 0;
    this.#headingStepMax = 0;
    this.#pedPairs = 0;
    this.#frameMsSum = 0;
    this.#frameMsMax = 0;
  }

  /** Copy the pose buffer the viewer just captured: the engine's own data, for attribution. */
  observeSnapshot(poses: PoseBuffer): void {
    const n = poses.count;
    if (this.#rawPos.length < n * 3) {
      this.#rawPos = new Float32Array(n * 3 * 2);
      this.#rawHead = new Float32Array(n * 2);
      this.#rawId = new Uint32Array(n * 2);
    }
    this.#rawPos.set(poses.positions.subarray(0, n * 3));
    this.#rawHead.set(poses.headings.subarray(0, n));
    for (let i = 0; i < n; i++) this.#rawId[i] = poses.occupied[i] === 1 ? poses.actorId[i] : NO_ID;
    this.#rawCount = n;
  }

  #ensure(n: number): void {
    if (n <= this.#cap) return;
    let c = Math.max(64, this.#cap);
    while (c < n) c *= 2;
    const g8 = (a: Uint8Array): Uint8Array<ArrayBuffer> => {
      const o = new Uint8Array(c);
      o.set(a);
      return o;
    };
    const g64 = (a: Float64Array): Float64Array<ArrayBuffer> => {
      const o = new Float64Array(c);
      o.set(a);
      return o;
    };
    const id = new Uint32Array(c).fill(NO_ID);
    id.set(this.#id);
    this.#id = id;
    this.#tracked = g8(this.#tracked);
    this.#drawn = g8(this.#drawn);
    this.#interior = g8(this.#interior);
    this.#buildingOverlap = g8(this.#buildingOverlap);
    this.#snapPrev = g8(this.#snapPrev);
    this.#sx = g64(this.#sx);
    this.#sy = g64(this.#sy);
    this.#psx = g64(this.#psx);
    this.#psy = g64(this.#psy);
    this.#wx = g64(this.#wx);
    this.#wy = g64(this.#wy);
    this.#wh = g64(this.#wh);
    this.#wv = g64(this.#wv);
    const fd = new Float32Array(c);
    fd.set(this.#fade);
    this.#fade = fd;
    this.#lodT0 = g64(this.#lodT0);
    this.#lodT1 = g64(this.#lodT1);
    this.#overlapUntil = g64(this.#overlapUntil);
    const lod = new Int8Array(c).fill(-1);
    lod.set(this.#lod);
    this.#lod = lod;
    this.#cap = c;
  }

  #emit(e: GlitchEvent): void {
    this.#counts[e.cls]++;
    if (e.cls === "stutter" && e.frame !== this.#lastStutterFrame) {
      this.#lastStutterFrame = e.frame;
      this.#stutterFrames++;
      if (e.timeS - this.#flightEndedAt < 0.5) this.#stutterFramesAfterFlight++;
      if (this.#stutterFrameList.length < 40) {
        this.#stutterFrameList.push(`${e.timeS.toFixed(2)} ${e.mode}${e.timeS - this.#flightEndedAt < 0.5 ? " (landing)" : ""}: ${e.detail}`);
      }
    }
    const m = (this.#byMode[e.mode] ??= {});
    m[e.cls] = (m[e.cls] ?? 0) + 1;
    if (e.cause === "engine") this.#engine[e.cls] = (this.#engine[e.cls] ?? 0) + 1;
    const ex = (this.#examples[e.cls] ??= []);
    if (ex.length < this.examplesPerClass) ex.push(e);
    const exm = ((this.#examplesByMode[e.mode] ??= {})[e.cls] ??= []);
    if (exm.length < 3) exm.push(e);
    for (const l of this.#listeners) l(e);
  }

  /** Examine the frame the viewer just drew. */
  afterFrame(): void {
    const v = this.viewer;
    const frame = this.#frame++;
    const mode = v.cameras.mode;
    this.#framesByMode[mode] = (this.#framesByMode[mode] ?? 0) + 1;
    const report = v.lastFrame;
    const dt = report.dtSeconds;
    const t = report.clockSeconds;
    const ms = dt * 1000;
    const transitNow = v.cameras.inTransit;
    if (this.#wasInTransit && !transitNow) this.#flightEndedAt = t;
    this.#wasInTransit = transitNow;
    this.#frameMsSum += ms;
    if (ms > this.#frameMsMax) this.#frameMsMax = ms;

    const cam = v.camera;
    const { width: W, height: H } = v.size;
    const e = cam.matrixWorldInverse.elements;
    const pr = cam.projectionMatrix.elements;
    // view-projection = P · V, column-major.
    const vp = new Float64Array(16);
    for (let c = 0; c < 4; c++) {
      for (let r = 0; r < 4; r++) {
        let s = 0;
        for (let k = 0; k < 4; k++) s += pr[k * 4 + r] * e[c * 4 + k];
        vp[c * 4 + r] = s;
      }
    }
    const camX = cam.position.x;
    const camY = cam.position.y;
    const camZ = cam.position.z;
    const focalPx = H / 2 / Math.tan((cam.fov * Math.PI) / 360);

    this.#checkCamera(frame, t, mode);

    const it = v.interpolator;
    const n = it.count;
    this.#ensure(n);
    const pos = it.outPosition;
    const head = it.outHeading;
    const spd = it.outSpeed;
    const ids = it.outActorId;
    const occ = it.outOccupied;
    const cls = it.outClassIdx;
    const snappedArr = it.outSnapped;
    // How much of each actor is drawn (a fade-in or fade-out); a viewer without fades draws all.
    const fadeArr = (it as { outFade?: Float32Array }).outFade;
    const classes = v.actors.classes;
    const slotLod = v.actors.slotLod;
    const hiddenId = v.actors.hiddenActorId >>> 0;
    const followId = v.cameras.followActorId;
    const edge = this.edgePx;
    const inTransit = v.cameras.inTransit;

    // Slots that went away since the last frame: a vanish inside the view is a pop.
    for (let s = n; s < this.#cap; s++) {
      if (this.#id[s] === NO_ID) continue;
      this.#vanish(s, frame, t, mode);
    }

    for (let s = 0; s < n; s++) {
      const live = occ[s] === 1;
      const id = live ? ids[s] : NO_ID;
      const isNew = this.#id[s] !== id;
      if (isNew) {
        if (this.#id[s] !== NO_ID) this.#vanish(s, frame, t, mode);
        this.#id[s] = id;
        this.#tracked[s] = 0;
        this.#drawn[s] = 0;
        this.#interior[s] = 0;
        this.#lod[s] = -1;
        this.#lodT0[s] = -Infinity;
        this.#lodT1[s] = -Infinity;
        this.#buildingOverlap[s] = 0;
        this.#overlapUntil[s] = -Infinity;
        if (!live) continue;
      } else if (!live) {
        continue;
      }
      if (id === hiddenId) {
        this.#drawn[s] = 0;
        this.#tracked[s] = 0;
        continue;
      }
      let c = cls[s];
      if (c >= classes.length) c = 0;
      const def = classes[c];
      const vru = def.category === 1;
      const p = s * 3;
      const x = pos[p];
      const y = pos[p + 1];
      const z = pos[p + 2];
      const h = head[s];
      const speed = Math.abs(spd[s]);
      const lod = s < slotLod.length ? slotLod[s] : -1;
      const drawn = lod >= 0;
      const dx = x - camX;
      const dy = y - camY;
      const dz = z - camZ;
      const dist2 = dx * dx + dy * dy + dz * dz;
      const dist = Math.sqrt(dist2);

      // Project the body centre.
      const cz = z + def.heightM * 0.5;
      const cw = vp[3] * x + vp[7] * y + vp[11] * cz + vp[15];
      let sx = NaN;
      let sy = NaN;
      let interior = false;
      if (cw > 1e-6) {
        sx = ((vp[0] * x + vp[4] * y + vp[8] * cz + vp[12]) / cw * 0.5 + 0.5) * W;
        sy = (1 - ((vp[1] * x + vp[5] * y + vp[9] * cz + vp[13]) / cw * 0.5 + 0.5)) * H;
        const sizePx = (Math.max(def.lengthM, def.heightM) * focalPx) / Math.max(1e-3, dist);
        interior = sx > edge && sx < W - edge && sy > edge && sy < H - edge && sizePx >= this.minPx;
      }
      const drawnHist = ((this.#drawn[s] << 1) | (drawn ? 1 : 0)) & 7;
      const intHist = ((this.#interior[s] << 1) | (interior ? 1 : 0)) & 7;
      const wasDrawn = (drawnHist & 2) !== 0;
      const hadHistory = this.#tracked[s] > 0;

      if (drawn) this.#actorFrames++;

      // Pop: appeared inside the view. A brand-new actor (no history) that is drawn in the
      // interior on its first frame is a spawn in view; one that was tracked but not drawn is a
      // pop only if it was not culled — i.e. it was already in the interior last frame.
      const fade = fadeArr && s < fadeArr.length ? fadeArr[s] : 1;
      // An actor that dissolves in or out over several frames has not popped: only an appearance
      // or a disappearance at more than half strength is one.
      // Pop: appeared inside the view, at more than half strength. A new actor is a spawn in
      // view (the engine's); a known one that was not drawn last frame is a pop only if it was
      // inside last frame's view too — not if the camera turned or flew to it.
      if (drawn && !wasDrawn && interior && fade >= 0.5 && !inTransit && !this.#transitPrev) {
        const wasInView = !isNew && this.#inPrevView(x, y, cz, W, H);
        if (isNew || wasInView) {
          // A flicker is a separate, stronger claim; see below.
          if ((drawnHist & 4) === 0) {
            this.#emit({
              cls: "pop", frame, timeS: t, mode, actorId: id,
              cause: isNew ? "engine" : "viewer",
              detail: isNew
                ? `${def.name} appeared ${dist.toFixed(0)} m from the camera, inside the view (a spawn in view)`
                : `${def.name} reappeared inside the view at ${dist.toFixed(0)} m`,
            });
          }
        }
      }
      if (!drawn && wasDrawn && interior && (intHist & 2) !== 0 && id !== hiddenId && this.#fade[s] >= 0.5 && !inTransit) {
        // Drawn last frame, in the interior both frames, not drawn now: vanished in view.
        this.#emit({
          cls: "pop", frame, timeS: t, mode, actorId: id, cause: "viewer",
          detail: `${def.name} vanished inside the view at ${dist.toFixed(0)} m while still live`,
        });
      }
      // Flicker: drawn, not drawn, drawn — all in the interior.
      if (drawnHist === 5 && intHist === 7) {
        this.#emit({
          cls: "flicker", frame, timeS: t, mode, actorId: id, cause: "viewer",
          detail: `${def.name} blinked out for one frame at ${dist.toFixed(0)} m`,
        });
      }

      if (drawn && wasDrawn && hadHistory) {
        // Teleport.
        const mx = x - this.#wx[s];
        const my = y - this.#wy[s];
        const moved = Math.hypot(mx, my);
        const vmax = Math.max(speed, this.#wv[s]);
        const allowed = Math.max(0.75, 2.5 * vmax * Math.max(dt, 1 / 240) + 0.3);
        if (moved > allowed) {
          this.#emit({
            cls: "teleport", frame, timeS: t, mode, actorId: id,
            cause: snappedArr[s] === 1 ? "engine" : "viewer",
            detail: `${def.name} moved ${moved.toFixed(2)} m in one ${(dt * 1000).toFixed(1)} ms frame at ${vmax.toFixed(1)} m/s`,
          });
        }
        // Heading snap.
        const dh = Math.abs(wrap(h - this.#wh[s]));
        if (!vru && dh > this.#headingStepMax) this.#headingStepMax = dh;
        const limit = vru ? 0.35 : 0.12;
        const stationary = !vru && speed < 0.3 && this.#wv[s] < 0.3 && moved < 0.01;
        if (dh > limit || (stationary && dh > 0.03)) {
          this.#emit({
            cls: "heading_snap", frame, timeS: t, mode, actorId: id,
            cause: snappedArr[s] === 1 ? "engine" : "viewer",
            detail: stationary
              ? `${def.name} rotated ${(dh * 180 / Math.PI).toFixed(1)}° in one frame while standing still`
              : `${def.name} turned ${(dh * 180 / Math.PI).toFixed(1)}° in one frame at ${speed.toFixed(1)} m/s`,
          });
        }
        // LOD pop and thrash.
        const prevLod = this.#lod[s];
        if (prevLod >= 0 && lod !== prevLod) {
          const radius = Math.hypot(def.lengthM, def.widthM, def.heightM) * 0.5;
          const sizePx = (2 * radius * focalPx) / Math.max(1e-3, dist);
          const thrash = t - this.#lodT0[s] < 1.0;
          if ((sizePx > 30 && interior) || thrash) {
            this.#emit({
              cls: "lod_pop", frame, timeS: t, mode, actorId: id, cause: "viewer",
              detail: thrash
                ? `${def.name} changed detail level three times within a second (now ${lod})`
                : `${def.name} changed detail ${prevLod}→${lod} while ${sizePx.toFixed(0)} px tall`,
            });
          }
          this.#lodT0[s] = this.#lodT1[s];
          this.#lodT1[s] = t;
        }
      }

      // Stutter: second difference of the screen position.
      // A frame the interpolator snapped (a discontinuity in the data, counted as a teleport) and
      // the one after it are not judged for stutter: the jump is the event, already counted.
      const snapNear = snappedArr[s] === 1 || this.#snapPrev[s] === 1;
      this.#snapPrev[s] = snappedArr[s];
      if (drawn && interior && !snapNear && this.#tracked[s] >= 2 && Number.isFinite(this.#psx[s])) {
        // Screen velocities per second, expressed in pixels per 60 Hz frame, so a frame the
        // display dropped (twice the time, twice the motion) is not mistaken for a jerk.
        const k1 = 1 / (60 * Math.max(dt, 1e-4));
        const k0 = 1 / (60 * Math.max(this.#prevDt, 1e-4));
        const vx = (sx - this.#sx[s]) * k1;
        const vy = (sy - this.#sy[s]) * k1;
        const pvx = (this.#sx[s] - this.#psx[s]) * k0;
        const pvy = (this.#sy[s] - this.#psy[s]) * k0;
        const sp = Math.max(Math.hypot(vx, vy), Math.hypot(pvx, pvy));
        const jerk = Math.hypot(vx - pvx, vy - pvy);
        if (sp > 1 || jerk > 1) {
          this.#jerkSum2 += jerk * jerk;
          this.#jerkN++;
          if (jerk > this.#jerkMax) this.#jerkMax = jerk;
          const isFollow = followId !== null && id === (followId >>> 0);
          if (isFollow) {
            this.#followJerkSum2 += jerk * jerk;
            this.#followJerkN++;
          }
          if (jerk > Math.max(1.5, 0.5 * sp) && !inTransit) {
            this.#emit({
              cls: "stutter", frame, timeS: t, mode, actorId: id, cause: this.#frameDataCut ? "engine" : "viewer",
              detail: `${def.name}${isFollow ? " (followed)" : ""} jerked ${jerk.toFixed(1)} px on screen moving ${sp.toFixed(1)} px/frame`,
            });
          }
        }
      }

      // Shift history.
      this.#drawn[s] = drawnHist;
      this.#interior[s] = intHist;
      if (drawn && Number.isFinite(sx)) {
        this.#psx[s] = this.#tracked[s] >= 1 ? this.#sx[s] : NaN;
        this.#psy[s] = this.#tracked[s] >= 1 ? this.#sy[s] : NaN;
        this.#sx[s] = sx;
        this.#sy[s] = sy;
        if (this.#tracked[s] < 250) this.#tracked[s]++;
      } else {
        this.#tracked[s] = 0;
      }
      this.#fade[s] = drawn ? fade : 0;
      this.#wx[s] = x;
      this.#wy[s] = y;
      this.#wh[s] = h;
      this.#wv[s] = speed;
      this.#lod[s] = lod;
    }

    this.#checkOverlaps(frame, t, mode);
    this.#checkSignalsAndGhosts(frame, t, mode);
    this.#checkSubject(frame, t, mode, vp, W, H);
    this.#prevVp.set(vp);
    this.#prevVpValid = true;
    this.#transitPrev = inTransit;
    this.#prevDt = dt > 0 ? dt : this.#prevDt;
  }

  /** Whether a world point was inside the interior of last frame's view. */
  #inPrevView(x: number, y: number, z: number, W: number, H: number): boolean {
    if (!this.#prevVpValid) return false;
    const m = this.#prevVp;
    const w = m[3] * x + m[7] * y + m[11] * z + m[15];
    if (w <= 1e-6) return false;
    const sx = ((m[0] * x + m[4] * y + m[8] * z + m[12]) / w * 0.5 + 0.5) * W;
    const sy = (1 - ((m[1] * x + m[5] * y + m[9] * z + m[13]) / w * 0.5 + 0.5)) * H;
    const e = this.edgePx;
    return sx > e && sx < W - e && sy > e && sy < H - e;
  }

  /** A tracked slot whose actor went away: a pop if it was inside the view. */
  #vanish(s: number, frame: number, t: number, mode: string): void {
    const inView = (this.#interior[s] & 1) !== 0 && (this.#drawn[s] & 1) !== 0 && this.#fade[s] >= 0.5;
    if (inView && this.#id[s] !== (this.viewer.actors.hiddenActorId >>> 0)) {
      this.#emit({
        cls: "pop", frame, timeS: t, mode, actorId: this.#id[s], cause: "engine",
        detail: "an actor vanished inside the view (a despawn in view)",
      });
    }
    this.#id[s] = NO_ID;
    this.#drawn[s] = 0;
    this.#interior[s] = 0;
    this.#tracked[s] = 0;
  }

  #checkOverlaps(frame: number, t: number, mode: string): void {
    const v = this.viewer;
    const it = v.interpolator;
    const pos = it.outPosition;
    const head = it.outHeading;
    const cls = it.outClassIdx;
    const ids = it.outActorId;
    const classes = v.actors.classes;
    const slotLod = v.actors.slotLod;
    const n = it.count;
    const w = v.worldRenderer;
    const cam = v.camera.position;
    const near2 = this.nearM * this.nearM;
    // Broad phase on a 12 m grid over drawn actors near the camera.
    const CELL = 12;
    const grid = new Map<number, number[]>();
    const key = (gx: number, gy: number): number => (gx + 50000) * 100000 + (gy + 50000);
    for (let s = 0; s < n; s++) {
      if (s >= slotLod.length || slotLod[s] < 0) continue;
      const p = s * 3;
      const dx = pos[p] - cam.x;
      const dy = pos[p + 1] - cam.y;
      if (dx * dx + dy * dy > near2) continue;
      const k = key(Math.floor(pos[p] / CELL), Math.floor(pos[p + 1] / CELL));
      let list = grid.get(k);
      if (!list) {
        list = [];
        grid.set(k, list);
      }
      list.push(s);

      // Vehicle inside a building.
      let c = cls[s];
      if (c >= classes.length) c = 0;
      const def = classes[c];
      if (def.category === 0) {
        const cs = Math.cos(head[s]);
        const sn = Math.sin(head[s]);
        const hl = def.lengthM / 2 - 0.1;
        const hw = def.widthM / 2 - 0.1;
        let inside = -1;
        for (const [fx, fy] of [[hl, hw], [hl, -hw], [-hl, hw], [-hl, -hw], [0, 0]]) {
          const x = pos[p] + fx * cs - fy * sn;
          const y = pos[p + 1] + fx * sn + fy * cs;
          const b = w.buildingIndexAt(x, y);
          if (b >= 0 && pos[p + 2] < w.buildingTopOf(b) - 0.5 && !w.hasPassage(b)) {
            inside = b;
            break;
          }
        }
        const was = this.#buildingOverlap[s] === 1;
        this.#buildingOverlap[s] = inside >= 0 ? 1 : 0;
        if (inside >= 0 && !was) {
          this.#emit({
            cls: "overlap_building", frame, timeS: t, mode, actorId: ids[s], cause: "engine",
            detail: `${def.name} drawn inside building ${w.buildingIdOf(inside)}, which has no road through it`,
          });
        }
      }
    }
    const seen = new Set<string>();
    for (const [k, list] of grid) {
      const gx = Math.floor(k / 100000) - 50000;
      const gy = (k % 100000) - 50000;
      for (let ox = -1; ox <= 1; ox++) {
        for (let oy = -1; oy <= 1; oy++) {
          const other = grid.get(key(gx + ox, gy + oy));
          if (!other) continue;
          for (const a of list) {
            for (const b of other) {
              if (b <= a) continue;
              this.#pair(a, b, frame, t, mode, pos, head, cls, ids, classes, seen);
            }
          }
        }
      }
    }
    // Close the episodes that did not recur this frame.
    for (const [k, last] of this.#pairs) if (last < frame - 1) this.#pairs.delete(k);
  }

  #pair(
    a: number, b: number, frame: number, t: number, mode: string,
    pos: Float32Array, head: Float32Array, cls: Uint8Array, ids: Uint32Array,
    classes: Viewer["actors"]["classes"], seen: Set<string>,
  ): void {
    let ca = cls[a];
    if (ca >= classes.length) ca = 0;
    let cb = cls[b];
    if (cb >= classes.length) cb = 0;
    const da = classes[ca];
    const db = classes[cb];
    const va = da.category === 0;
    const vb = db.category === 0;
    const pa = a * 3;
    const pb = b * 3;
    if (Math.abs(pos[pa + 2] - pos[pb + 2]) > 3) return; // different levels (a ramp over a road)
    const depth = obbPenetration(
      pos[pa], pos[pa + 1], da.lengthM / 2, da.widthM / 2, Math.cos(head[a]), Math.sin(head[a]),
      pos[pb], pos[pb + 1], db.lengthM / 2, db.widthM / 2, Math.cos(head[b]), Math.sin(head[b]),
    );
    const idA = ids[a];
    const idB = ids[b];
    const pk = idA < idB ? `${idA},${idB}` : `${idB},${idA}`;
    if (seen.has(pk)) return;
    seen.add(pk);
    if (!va && !vb) {
      if (depth > 0.1) this.#pedPairs++;
      return;
    }
    if (depth <= 0.05) return;
    const ongoing = this.#pairs.has(pk);
    this.#pairs.set(pk, frame);
    if (ongoing) return;
    // Did the engine's own newest snapshot have them overlapping?
    let cause: "engine" | "viewer" = "viewer";
    const ra = this.#rawSlot(a, idA);
    const rb = this.#rawSlot(b, idB);
    if (ra >= 0 && rb >= 0) {
      const rp = this.#rawPos;
      const rh = this.#rawHead;
      const rd = obbPenetration(
        rp[ra * 3], rp[ra * 3 + 1], da.lengthM / 2, da.widthM / 2, Math.cos(rh[ra]), Math.sin(rh[ra]),
        rp[rb * 3], rp[rb * 3 + 1], db.lengthM / 2, db.widthM / 2, Math.cos(rh[rb]), Math.sin(rh[rb]),
      );
      if (rd > 0.02) cause = "engine";
    }
    const cls2: GlitchClass = va && vb ? "overlap_vehicle" : "overlap_pedestrian";
    this.#emit({
      cls: cls2, frame, timeS: t, mode, actorId: idA, cause,
      detail: `${da.name} ${idA} and ${db.name} ${idB} interpenetrate by ${depth.toFixed(2)} m`,
    });
  }

  #rawSlot(slot: number, id: number): number {
    return slot < this.#rawCount && this.#rawId[slot] === id ? slot : -1;
  }

  #checkCamera(frame: number, t: number, mode: string): void {
    const v = this.viewer;
    const cam = v.camera;
    const p = cam.position;
    const w = v.worldRenderer;
    const world = w.world;
    const finite = Number.isFinite(p.x) && Number.isFinite(p.y) && Number.isFinite(p.z)
      && Number.isFinite(cam.fov) && cam.projectionMatrix.elements.every(Number.isFinite);
    let empty: string | null = null;
    if (!finite) empty = "the camera is not finite";
    let clip: string | null = null;
    if (finite && world) {
      const b = w.buildingIndexAt(p.x, p.y);
      if (b >= 0 && p.z < w.buildingTopOf(b) && b !== w.ghostBuilding && b !== w.ghostBuilding2) {
        clip = `the camera is inside building ${w.buildingIdOf(b)}`;
      } else if (p.z < world.bbox.minZM + 0.15) {
        clip = "the camera is under the ground";
      } else {
        // Near-plane corners.
        const dir = cam.getWorldDirection(SCRATCH_DIR);
        const near = cam.near;
        const halfH = Math.tan((cam.fov * Math.PI) / 360) * near;
        const halfW = halfH * cam.aspect;
        // The camera's own right and up axes (the first two columns of its world matrix). Taking
        // "up" as world z, as this did, put a corner of a plan view's near plane — horizontal, 364 m
        // below a camera at 507 m — 150 m lower than it is, into every roof it passed over.
        const m = cam.matrixWorld.elements;
        const rl = Math.hypot(m[0], m[1], m[2]) || 1;
        const ul = Math.hypot(m[4], m[5], m[6]) || 1;
        for (const [a, bb] of [[1, 1], [1, -1], [-1, 1], [-1, -1]]) {
          const x = p.x + dir.x * near + (m[0] / rl) * halfW * a + (m[4] / ul) * halfH * bb;
          const y = p.y + dir.y * near + (m[1] / rl) * halfW * a + (m[5] / ul) * halfH * bb;
          const z = p.z + dir.z * near + (m[2] / rl) * halfW * a + (m[6] / ul) * halfH * bb;
          const k = w.buildingIndexAt(x, y);
          if (k >= 0 && z < w.buildingTopOf(k) && k !== w.ghostBuilding && k !== w.ghostBuilding2) {
            clip = `the near plane cuts into building ${w.buildingIdOf(k)}`;
            break;
          }
        }
        // A wall filling the view.
        if (!clip && mode !== "map") {
          for (const d of [0.5, 1, 1.5, 2, 2.5]) {
            const k = w.buildingIndexAt(p.x + dir.x * d, p.y + dir.y * d);
            if (k >= 0 && p.z + dir.z * d < w.buildingTopOf(k) && k !== w.ghostBuilding && k !== w.ghostBuilding2) {
              empty = `a wall ${d.toFixed(1)} m in front of the camera fills the view`;
              break;
            }
          }
        }
        // Looking away from the whole world.
        if (!empty && mode === "map") {
          const bb = world.bbox;
          const lx = v.cameras.look.x;
          const ly = v.cameras.look.y;
          if (lx < bb.minXM - 50 || lx > bb.maxXM + 50 || ly < bb.minYM - 50 || ly > bb.maxYM + 50) {
            empty = "the plan view looks at a point outside the world";
          }
        }
      }
    }
    if (clip && !this.#clipEpisode) {
      this.#emit({ cls: "camera_clip", frame, timeS: t, mode, cause: "viewer", detail: clip });
    }
    this.#clipEpisode = clip !== null;
    if (empty && !this.#emptyEpisode) {
      this.#emit({ cls: "empty_frame", frame, timeS: t, mode, cause: "viewer", detail: empty });
    }
    this.#emptyEpisode = empty !== null;

    // Camera stutter: second difference of the camera position against its motion.
    const c = this.#cam;
    if (finite) {
      const fs = v.followSlot;
      const subjectJumped = fs >= 0 && (v.interpolator.outSnapped[fs] === 1 || this.#followSnapPrev);
      this.#followSnapPrev = fs >= 0 && v.interpolator.outSnapped[fs] === 1;
      this.#frameDataCut = subjectJumped && (mode === "chase" || mode === "dashboard");
      if (this.#camValid >= 2 && !v.cameras.inTransit && !subjectJumped) {
        const dtNow = Math.max(v.lastFrame.dtSeconds, 1e-4);
        const k1 = 1 / (60 * dtNow);
        const k0 = 1 / (60 * Math.max(this.#prevDt, 1e-4));
        const vx = (p.x - c[0]) * k1;
        const vy = (p.y - c[1]) * k1;
        const vz = (p.z - c[2]) * k1;
        const pvx = (c[0] - c[3]) * k0;
        const pvy = (c[1] - c[4]) * k0;
        const pvz = (c[2] - c[5]) * k0;
        const sp = Math.max(Math.hypot(vx, vy, vz), Math.hypot(pvx, pvy, pvz));
        const jerk = Math.hypot(vx - pvx, vy - pvy, vz - pvz);
        const dist = p.distanceTo(v.cameras.look);
        // In pixels at the look distance.
        const H = v.size.height;
        const pxPerM = H / 2 / Math.tan((v.camera.fov * Math.PI) / 360) / Math.max(1, dist);
        if (jerk * pxPerM > Math.max(2, 0.5 * sp * pxPerM) && mode !== "map") {
          this.#emit({
            cls: "stutter", frame, timeS: t, mode, cause: "viewer",
            detail: `the camera jerked ${(jerk * 100).toFixed(1)} cm (${(jerk * pxPerM).toFixed(1)} px) moving ${(sp * 100).toFixed(1)} cm/frame`,
          });
        }
      }
      c[3] = c[0]; c[4] = c[1]; c[5] = c[2];
      c[0] = p.x; c[1] = p.y; c[2] = p.z;
      if (this.#camValid < 3) this.#camValid++;
    } else {
      this.#camValid = 0;
    }

    // Depth precision against the thinnest layer gap, where the ground is visible.
    if (finite && world) {
      const near = cam.near;
      const far = cam.far;
      const logDepth = v.logarithmicDepth;
      let visibleM = far;
      const fog = v.scene.fog as { far?: number } | null;
      if (fog && typeof fog.far === "number") visibleM = Math.min(visibleM, fog.far);
      const b = world.bbox;
      const span = Math.hypot(b.maxXM - b.minXM, b.maxYM - b.minYM);
      visibleM = Math.min(visibleM, span + p.z);
      // Where two layers overlap — a junction disc over a sidewalk corner, a crossing over a lane —
      // the overlap is a couple of metres across. Beyond the distance at which 2 m spans two
      // pixels, a shimmer there is below a pixel and cannot be seen; that is as far as it is judged.
      const focal = v.size.height / 2 / Math.tan((cam.fov * Math.PI) / 360);
      visibleM = Math.min(visibleM, focal);
      // Resolvable depth step at distance d: d² / (near · 2^bits) for a standard buffer; a
      // logarithmic buffer resolves a constant fraction, d · ln(far/near) / 2^bits.
      const steps = 2 ** this.depthBits;
      const res = logDepth
        ? (visibleM * Math.log(far / near)) / steps
        : (visibleM * visibleM) / (near * steps);
      // Two overlapping layers are told apart when they are two depth steps apart: the gap, in
      // steps at that distance, plus the steps the surface shader's rank bias adds (a bias on
      // gl_Position.z, so none under a logarithmic buffer, which writes its own fragment depth).
      const rankBias = v.worldRenderer.layerBiasSteps;
      const biasSteps = logDepth || !(rankBias > 0) ? 0 : rankBias * 2 ** (this.depthBits - 24);
      const separation = this.minLayerGapM / res + biasSteps;
      const bad = separation < 2;
      if (bad && !this.#zEpisode) {
        this.#emit({
          cls: "z_fighting", frame, timeS: t, mode, cause: "viewer",
          detail: `depth step ${(res * 100).toFixed(1)} cm at ${visibleM.toFixed(0)} m (near ${near.toFixed(2)} m): the ${(this.minLayerGapM * 100).toFixed(0)} cm road-layer gap and a ${biasSteps.toFixed(1)}-step rank bias separate layers by ${separation.toFixed(2)} steps, under 2`,
        });
      }
      this.#zEpisode = bad;
    }
  }

  #checkSignalsAndGhosts(frame: number, t: number, mode: string): void {
    const sig = this.viewer.worldRenderer.signals;
    const n = sig.count;
    if (this.#sigPhase.length !== n) {
      this.#sigPhase = new Uint8Array(n).fill(0xff);
      this.#sigPrev = new Uint8Array(n).fill(0xff);
      this.#sigChangedAt = new Float64Array(n).fill(-Infinity);
    }
    for (let i = 0; i < n; i++) {
      const ph = sig.phaseAt(i);
      if (ph === this.#sigPhase[i]) continue;
      // Changed. Back to what it was within 0.3 s is a flicker (a real signal holds a state for
      // seconds: the shortest J2735 interval a plan here writes is a 3 s amber).
      if (ph === this.#sigPrev[i] && t - this.#sigChangedAt[i] < 0.3 && this.#sigPhase[i] !== 0xff) {
        this.#emit({
          cls: "flicker", frame, timeS: t, mode, cause: "viewer",
          detail: `signal head ${i} went ${this.#sigPrev[i]}→${this.#sigPhase[i]}→${ph} within ${((t - this.#sigChangedAt[i]) * 1000).toFixed(0)} ms`,
        });
      }
      this.#sigPrev[i] = this.#sigPhase[i];
      this.#sigPhase[i] = ph;
      this.#sigChangedAt[i] = t;
    }
    const w = this.viewer.worldRenderer;
    const g = w.ghostBuilding * 100000 + w.ghostBuilding2;
    const hist = this.#ghostHist;
    if (hist.length === 0 || hist[hist.length - 1] !== g) {
      if (hist.length >= 2 && hist[hist.length - 2] === g && t - this.#ghostChangedAt < 0.5) {
        this.#emit({
          cls: "flicker", frame, timeS: t, mode, cause: "viewer",
          detail: "a ghosted building was hidden, shown and hidden again within half a second",
        });
      }
      hist.push(g);
      if (hist.length > 4) hist.shift();
      this.#ghostChangedAt = t;
    }
  }

  #checkSubject(frame: number, t: number, mode: string, vp: Float64Array, W: number, H: number): void {
    const v = this.viewer;
    const followId = v.cameras.followActorId;
    const street = mode === "chase" || mode === "dashboard";
    const slot = v.followSlot;
    if (!street || followId === null || slot < 0 || v.cameras.inTransit) {
      this.#lostEpisode = false;
      this.#framingEpisode = false;
      return;
    }
    const it = v.interpolator;
    const p = slot * 3;
    const x = it.outPosition[p];
    const y = it.outPosition[p + 1];
    const z = it.outPosition[p + 2];
    let c = it.outClassIdx[slot];
    if (c >= v.actors.classes.length) c = 0;
    const def = v.actors.classes[c];
    const project = (px: number, py: number, pz: number): [number, number] | null => {
      const w = vp[3] * px + vp[7] * py + vp[11] * pz + vp[15];
      if (w <= 1e-6) return null;
      return [
        ((vp[0] * px + vp[4] * py + vp[8] * pz + vp[12]) / w * 0.5 + 0.5) * W,
        (1 - ((vp[1] * px + vp[5] * py + vp[9] * pz + vp[13]) / w * 0.5 + 0.5)) * H,
      ];
    };
    if (mode === "chase") {
      const mid = project(x, y, z + def.heightM * 0.5);
      const lost = mid === null || mid[0] < 0 || mid[0] > W || mid[1] < 0 || mid[1] > H;
      if (lost && !this.#lostEpisode) {
        this.#emit({
          cls: "subject_lost", frame, timeS: t, mode, actorId: followId, cause: "viewer",
          detail: `the followed ${def.name} is outside the chase frame`,
        });
      }
      this.#lostEpisode = lost;
      // A wall between the camera and the subject.
      const cam = v.camera.position;
      const w = v.worldRenderer;
      let occluded = false;
      const tz = z + def.heightM * 0.6;
      for (let k = 1; k < 16; k++) {
        const u = k / 16;
        const qx = cam.x + (x - cam.x) * u;
        const qy = cam.y + (y - cam.y) * u;
        const qz = cam.z + (tz - cam.z) * u;
        const b = w.buildingIndexAt(qx, qy);
        if (b >= 0 && qz < w.buildingTopOf(b) && b !== w.ghostBuilding && b !== w.ghostBuilding2) {
          occluded = true;
          break;
        }
      }
      // Framing: how steeply the camera looks down, and how big the subject is.
      const look = v.cameras.look;
      const horiz = Math.hypot(cam.x - look.x, cam.y - look.y);
      const pitchDeg = (Math.atan2(cam.z - look.z, horiz) * 180) / Math.PI;
      const top = project(x, y, z + def.heightM);
      const bottom = project(x, y, z);
      const frac = top && bottom ? Math.abs(bottom[1] - top[1]) / H : 0;
      const badFrame = pitchDeg > 35 || frac < 0.06 || frac > 0.7;
      if (badFrame && !this.#framingEpisode) {
        this.#emit({
          cls: "chase_framing", frame, timeS: t, mode, actorId: followId, cause: "viewer",
          detail: `chase camera ${pitchDeg.toFixed(0)}° above the horizon; the ${def.name} is ${(frac * 100).toFixed(0)} % of the frame's height`,
        });
      }
      this.#framingEpisode = badFrame;
      if (occluded && !this.#clipEpisode) {
        this.#emit({
          cls: "camera_clip", frame, timeS: t, mode, actorId: followId, cause: "viewer",
          detail: `a building stands between the chase camera and the ${def.name}`,
        });
        this.#clipEpisode = true;
      }
    }
  }

  /** The counts so far. */
  report(): GlitchReport {
    const frames = this.#frame;
    return {
      frames,
      framesByMode: { ...this.#framesByMode },
      counts: { ...this.#counts },
      countsByMode: JSON.parse(JSON.stringify(this.#byMode)) as GlitchReport["countsByMode"],
      engineCaused: { ...this.#engine },
      examples: JSON.parse(JSON.stringify(this.#examples)) as GlitchReport["examples"],
      examplesByMode: JSON.parse(JSON.stringify(this.#examplesByMode)) as GlitchReport["examplesByMode"],
      metrics: {
        actorFrames: this.#actorFrames,
        jerkRmsPx: this.#jerkN > 0 ? Math.sqrt(this.#jerkSum2 / this.#jerkN) : 0,
        jerkMaxPx: this.#jerkMax,
        followJerkRmsPx: this.#followJerkN > 0 ? Math.sqrt(this.#followJerkSum2 / this.#followJerkN) : 0,
        headingStepMaxRad: this.#headingStepMax,
        pedestrianPairsOverlapping: this.#pedPairs,
        stutterFrames: this.#stutterFrames,
        stutterFramesAfterFlight: this.#stutterFramesAfterFlight,
        stutterFrameList: [...this.#stutterFrameList],
        frameMsMean: frames > 0 ? this.#frameMsSum / frames : 0,
        frameMsMax: this.#frameMsMax,
      },
    };
  }
}
