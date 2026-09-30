/**
 * Instanced actor rendering.
 *
 * 09-ui §4: "one `InstancedMesh` per (class × LOD level); capacity preallocated, `count` set to the
 * visible instances after CPU frustum culling; instances moved between LOD meshes by distance",
 * citing the measurement that count-based culling plus per-LOD meshes almost doubled the frame rate
 * on an integrated GPU. The loop below is that, with one refinement: a class can be drawn with more
 * than one **model** (a passenger car is a sedan, a crossover or, in New York, a yellow cab;
 * `vehicle-models.ts`), so a bucket is one (model × LOD) and an actor's model is a stable function
 * of its id.
 *
 * 1. one pass over the live slots computes distance and the LOD band (with hysteresis, so an actor
 *    sitting on a band edge does not change detail every frame), then tests a bounding sphere
 *    against the six frustum planes with plain arithmetic (no `Sphere`, no `Vector3`, no allocation);
 * 2. survivors are appended to their bucket — the 16 floats of the instance matrix are written
 *    straight into `instanceMatrix.array`, because the matrix is only a yaw about +z and a
 *    translation — and the four floats of the animation attribute next to them: the wheels' roll and
 *    steer, the gait phase, and the packed lamps (`actor-material.ts`);
 * 3. each bucket's `count` is set to the number written and only that prefix of each buffer is
 *    uploaded.
 *
 * Nothing in {@link ActorRenderer.update} allocates once the buckets have reached their steady-state
 * capacity: no `Object3D`, no `Matrix4`, no array, no closure — and no `{start, count}` either,
 * which is what `BufferAttribute.addUpdateRange` would push on every call, so each attribute owns
 * one range object that is mutated and re-pushed instead.
 *
 * ## Colour
 *
 * By default a benign vehicle is drawn in its own paint — a colour drawn once per vehicle from the
 * fleet's colour distribution, or its livery (the yellow cab, the transit bus) — and a person in
 * their own clothes, because a street of identical grey boxes is not traffic. An actor in any
 * §3.3.4 state the palette names (attacker, reported, revoked) or the selected one is painted in
 * that state's colour, as before, so the network's story still reads at street level; the aerial
 * mark (`overlays.ts`) keeps the state palette for every actor. `paint: "state"` restores the
 * all-state colouring. Instance colours are written, and uploaded, only when an instance's colour
 * actually changes.
 */

import {
  Color,
  DynamicDrawUsage,
  Group,
  InstancedBufferAttribute,
  InstancedMesh,
  Matrix4,
  type BufferGeometry,
  type Camera,
  type Material,
} from "three";
import { ActorState } from "@vwp/protocol";
import { makeActorMaterial, makeActorUniforms, packLamps, type ActorUniforms } from "./actor-material.js";
import {
  CLOTHING_PALETTE, PAINT_PALETTE, buildActorModel, hashId, liveryOf, modelVariants,
  type ActorModelInfo, type ActorModelKind,
} from "./vehicle-models.js";
import type { ActorClassDef, LodLevel } from "./types.js";
import type { ActorStateColorKey, ViewerTheme } from "./theme.js";

/** Tuning for {@link ActorRenderer}. */
export interface ActorRendererOptions {
  /** Class table, normally derived from `Hello.classes` by {@link classesFromHello}. */
  readonly classes: readonly ActorClassDef[];
  readonly theme: ViewerTheme;
  /** Hard ceiling on drawn instances across all buckets. Default 20,000. */
  readonly maxActors?: number;
  /** Starting capacity of each bucket. Default 128. */
  readonly initialCapacity?: number;
  /** `[LOD0→LOD1, LOD1→LOD2]` switch distances in metres. Default `[90, 400]`. */
  readonly lodDistancesM?: readonly [number, number];
  /** Extra metres added to every bounding radius before the frustum test. Default 1. */
  readonly cullMarginM?: number;
  /** Let LOD-0 actors cast shadows. Default false — 5,000 shadow casters is not a 60 fps budget. */
  readonly castShadows?: boolean;
  /**
   * Height added to every actor's z, metres. Default 0.1 — the height the world renderer draws the
   * road surface at above the lane centreline (`Z_ROAD` in `world-render.ts`), so a vehicle's tyres
   * sit on the asphalt instead of 10 cm into it.
   */
  readonly groundOffsetM?: number;
  /** Draw ground-truth-only state (the `ATTACKER` bit) in the actor colour. Default true. */
  readonly showGroundTruth?: boolean;
  /**
   * Paint actors with no state bit set in their *class* colour instead of their own paint.
   * Default false. Kept for the class-coloured legend (09-ui §10, finding Q13).
   */
  readonly colorBenignByClass?: boolean;
  /**
   * `"realistic"` (default): a benign vehicle in its own paint or livery, a person in their own
   * clothes, and any other state in its state colour. `"state"`: every actor in its state colour,
   * benign in `theme.actorState.benign`.
   */
  readonly paint?: "realistic" | "state";
}

/**
 * The colour buckets an actor can be painted in, in the order `theme.actorState` declares them.
 * The index into this array *is* the index into the renderer's packed state-colour table.
 */
export const ACTOR_STATE_COLOR_KEYS: readonly ActorStateColorKey[] = [
  "benign", "attacker", "reported", "revoked", "selected",
];

/** Colour key for "this actor's class colour", one past the state keys. */
const CLASS_COLOR_KEY = ACTOR_STATE_COLOR_KEYS.length;
/** Colour key for "this actor's own paint". */
const PAINT_COLOR_KEY = CLASS_COLOR_KEY + 1;

/**
 * Which colour bucket an actor falls into, as an index into {@link ACTOR_STATE_COLOR_KEYS}:
 * §3.3.4 bits 0–2, selection first, and `benign` when no bit is set.
 *
 * This is the single decision every place that paints an actor shares — {@link ActorRenderer.update}
 * writing instance colours, {@link ActorRenderer.legend} describing them, and the aerial vehicle
 * mark in `overlays.ts`.
 */
export function actorStateColorIndex(
  state: number,
  selected: boolean,
  showGroundTruth = true,
): number {
  if (selected) return 4;
  if (state & ActorState.REVOKED) return 3;
  if (showGroundTruth && (state & ActorState.ATTACKER)) return 1;
  if (state & ActorState.REPORTED) return 2;
  return 0;
}

/** {@link actorStateColorIndex}, named. */
export function actorColorKey(
  state: number,
  selected: boolean,
  showGroundTruth = true,
): ActorStateColorKey {
  return ACTOR_STATE_COLOR_KEYS[actorStateColorIndex(state, selected, showGroundTruth)];
}

/** One row of {@link ActorRenderer.legend}: a colour the scene really draws, and what it means. */
export interface ActorLegendEntry {
  /**
   * A state bucket, a per-class row, or `"paint"`: benign actors drawn in their own paint, whose
   * `color` is the commonest paint and stands for the whole palette.
   */
  readonly kind: "state" | "class" | "paint";
  /** The `theme.actorState` key, or the class name. */
  readonly key: string;
  readonly label: string;
  /** Packed `0xRRGGBB`, exactly the value written into `instanceColor` (for `"paint"`, one of them). */
  readonly color: number;
  /** Set on `kind: "class"` rows. */
  readonly classIndex?: number;
}

const STATE_LABELS: Readonly<Record<ActorStateColorKey, string>> = {
  benign: "Benign",
  attacker: "Attacker (GT)",
  reported: "Reported",
  revoked: "Revoked",
  selected: "Selected",
};

/** The per-frame inputs {@link ActorRenderer.update} reads. All arrays are indexed by slot. */
export interface ActorUpdateContext {
  readonly position: Float32Array;
  readonly heading: Float32Array;
  readonly classIdx: Uint8Array;
  readonly state: Uint8Array;
  readonly occupied: Uint8Array;
  readonly actorId: Uint32Array;
  /** Speed along the heading, m/s. Without it the walking amplitude comes from displacement. */
  readonly speed?: Float32Array;
  /** The §3.3.5 lamps byte per slot. Without it every lamp is dark. */
  readonly lamps?: Uint8Array;
  /** How much of each actor is there, `[0, 1]` (the interpolator's fade-in and fade-out). Default 1. */
  readonly fade?: Float32Array;
  /** Seconds since the previous update, for the steering filter. Default 1/60. */
  readonly dtSeconds?: number;
  /** A monotonic clock, seconds, for flashing lamps. */
  readonly timeSeconds?: number;
  /** Slot high-water mark. */
  readonly count: number;
  /** Camera whose `matrixWorld` and `projectionMatrix` are already up to date. */
  readonly camera: Camera;
  /** Set false to draw every live actor and measure the difference. Default true. */
  readonly cull?: boolean;
}

/** What one {@link ActorRenderer.update} did. */
export interface ActorUpdateStats {
  /** Live slots considered. */
  readonly live: number;
  /** Instances written into instance buffers. */
  readonly drawn: number;
  /** Live slots rejected by the frustum test. */
  readonly culled: number;
  /** Live slots rejected because `maxActors` was reached. */
  readonly dropped: number;
  /** Buckets with at least one instance — the actor share of the draw-call budget. */
  readonly buckets: number;
  readonly lod0: number;
  readonly lod1: number;
  readonly lod2: number;
}

/** One drawable model: a class drawn one way. */
interface Model {
  readonly classIndex: number;
  readonly kind: ActorModelKind;
  readonly info: ActorModelInfo;
  /** Upper edge of this model's share of its class, in `[0, 1]`. */
  readonly cumulative: number;
  /** The fixed livery, linear RGB, or null for a palette draw. */
  readonly livery: [number, number, number] | null;
}

interface Bucket {
  mesh: InstancedMesh<BufferGeometry, Material>;
  capacity: number;
  cursor: number;
  matrix: Float32Array;
  anim: Float32Array;
  animAttr: InstancedBufferAttribute;
  /** Slot that produced each written instance, for picking and hover read-back. */
  slots: Int32Array;
  /** Colour key last written into each instance slot; `0xff` means "never written". */
  colorKey: Uint8Array;
  /** Actor whose paint was last written into each instance slot (paint colours are per actor). */
  colorActor: Uint32Array;
  /** Set when an instance colour actually changed, cleared when the colours are uploaded. */
  colorDirty: boolean;
  readonly matrixRange: { start: number; count: number };
  readonly colorRange: { start: number; count: number };
  readonly animRange: { start: number; count: number };
  readonly modelIndex: number;
  readonly classIndex: number;
  readonly lod: LodLevel;
}

/** Build the viewer's class table from a `Hello` class table (§3.1.4). */
export function classesFromHello(
  hello: {
    readonly classes: {
      readonly count: number;
      readonly strName: Uint32Array;
      readonly lengthM: Float32Array;
      readonly widthM: Float32Array;
      readonly heightM: Float32Array;
      readonly colorRgba: Uint32Array;
      readonly category: Uint8Array;
    };
    readonly strings: readonly string[];
  },
): ActorClassDef[] {
  const t = hello.classes;
  const out: ActorClassDef[] = [];
  for (let i = 0; i < t.count; i++) {
    const rgba = t.colorRgba[i] >>> 0;
    out.push({
      index: i,
      name: hello.strings[t.strName[i]] ?? `class_${i}`,
      lengthM: t.lengthM[i],
      widthM: t.widthM[i],
      heightM: t.heightM[i],
      // `color_rgba` is 0xRRGGBBAA; the viewer wants 0xRRGGBB.
      color: (rgba >>> 8) & 0xffffff,
      category: t.category[i],
    });
  }
  return out;
}

/** A sensible class table for worlds that arrive without a `Hello` (tests, static world viewing). */
export const DEFAULT_ACTOR_CLASSES: readonly ActorClassDef[] = [
  { index: 0, name: "car", lengthM: 4.5, widthM: 1.8, heightM: 1.5, color: 0x8fa6bd, category: 0 },
  { index: 1, name: "truck", lengthM: 10.0, widthM: 2.5, heightM: 3.4, color: 0xb08a58, category: 0 },
  { index: 2, name: "bus", lengthM: 12.0, widthM: 2.55, heightM: 3.2, color: 0xd08f3a, category: 0 },
  { index: 3, name: "moto", lengthM: 2.1, widthM: 0.8, heightM: 1.4, color: 0xa0d0c0, category: 0 },
  { index: 4, name: "bicycle", lengthM: 1.7, widthM: 0.6, heightM: 1.6, color: 0x7fc98a, category: 1 },
  { index: 5, name: "pedestrian", lengthM: 0.5, widthM: 0.5, heightM: 1.75, color: 0xe6d2a8, category: 1 },
  { index: 6, name: "emergency", lengthM: 5.4, widthM: 2.0, heightM: 2.2, color: 0xe05050, category: 0 },
  { index: 7, name: "rail", lengthM: 24.0, widthM: 3.0, heightM: 3.8, color: 0x9090a8, category: 0 },
];

const TAU = Math.PI * 2;

function wrap(a: number): number {
  return a - Math.floor(a / TAU + 0.5) * TAU;
}

/**
 * Draws every actor the stream reports, as instanced meshes grouped by model and LOD.
 *
 * The renderer owns one `Group`; add it to the scene once and never touch the children.
 */
export class ActorRenderer {
  /** The scene node holding every actor bucket. */
  readonly group = new Group();
  /** Shared by every actor material; the viewer sets `uNight`, `update` sets `uTime`. */
  readonly uniforms: ActorUniforms = makeActorUniforms();

  #classes: ActorClassDef[];
  #theme: ViewerTheme;
  #models: Model[] = [];
  /** Per class: the first model index and how many models it has. */
  #classModels: Int32Array = new Int32Array(0);
  #buckets: Bucket[] = [];
  #geometries: BufferGeometry[] = [];
  #materials: Material[] = [];
  #radius = new Float32Array(0);
  #halfHeight = new Float32Array(0);
  #classColor = new Float32Array(0);
  #stateColor = new Float32Array(5 * 3);
  #maxActors: number;
  #initialCapacity: number;
  #lod0 = 90;
  #lod1 = 400;
  #cullMargin: number;
  #groundOffset: number;
  #castShadows: boolean;
  #showGroundTruth: boolean;
  #benignByClass: boolean;
  #paint: "realistic" | "state";
  #nyc = false;
  #taxiShare = 0.2;
  #deliveryMopedShare = 0.5;
  #selectedActorId = -1;
  #hiddenActorId = -1;
  #color = new Color();
  #stats: ActorUpdateStats = {
    live: 0, drawn: 0, culled: 0, dropped: 0, buckets: 0, lod0: 0, lod1: 0, lod2: 0,
  };
  /** Plane coefficients `(nx, ny, nz, d)` × 6, refreshed each update. */
  #planes = new Float32Array(24);
  /** Scratch view-projection matrix; reused so {@link update} allocates nothing. */
  #viewProjection = new Matrix4();
  /** Slots drawn this frame, in bucket order; `overlays.ts` reuses them instead of re-culling. */
  readonly visibleSlots: Int32Array;
  /**
   * The LOD band each slot was drawn at this frame, −1 when it was not drawn (culled, hidden,
   * empty). Indexed by slot up to the last update's `count`; read by the glitch hunter.
   */
  slotLod = new Int8Array(0);
  #visibleCount = 0;

  // Per-slot memory, keyed by the actor id it was computed for.
  #slotCap = 0;
  #slotId = new Uint32Array(0);
  #slotModel = new Int16Array(0);
  #slotPaint = new Float32Array(0);
  #slotPhase = new Float32Array(0);
  #slotBand = new Int8Array(0);
  #odo = new Float64Array(0);
  #gait = new Float64Array(0);
  #amp = new Float32Array(0);
  #steer = new Float32Array(0);
  #px = new Float64Array(0);
  #py = new Float64Array(0);
  #ph = new Float32Array(0);

  constructor(options: ActorRendererOptions) {
    this.#theme = options.theme;
    this.#maxActors = Math.max(1, options.maxActors ?? 20_000);
    this.#initialCapacity = Math.max(8, options.initialCapacity ?? 128);
    this.#cullMargin = options.cullMarginM ?? 1;
    this.#groundOffset = options.groundOffsetM ?? 0.1;
    this.#castShadows = options.castShadows ?? false;
    this.#showGroundTruth = options.showGroundTruth ?? true;
    this.#benignByClass = options.colorBenignByClass ?? false;
    this.#paint = options.paint ?? "realistic";
    if (options.lodDistancesM) {
      this.#lod0 = options.lodDistancesM[0];
      this.#lod1 = options.lodDistancesM[1];
    }
    this.group.name = "actors";
    this.visibleSlots = new Int32Array(this.#maxActors);
    this.#classes = options.classes.length > 0 ? [...options.classes] : [...DEFAULT_ACTOR_CLASSES];
    this.#refreshStateColors();
    this.#rebuild();
  }

  /** The class table currently in use. */
  get classes(): readonly ActorClassDef[] {
    return this.#classes;
  }

  /** Stats from the last {@link update}. */
  get stats(): ActorUpdateStats {
    return this.#stats;
  }

  /** How many slots {@link visibleSlots} holds. */
  get visibleCount(): number {
    return this.#visibleCount;
  }

  /** `[LOD0→LOD1, LOD1→LOD2]` in metres. */
  get lodDistancesM(): readonly [number, number] {
    return [this.#lod0, this.#lod1];
  }

  set lodDistancesM(v: readonly [number, number]) {
    this.#lod0 = v[0];
    this.#lod1 = v[1];
  }

  /** Set the two LOD switch distances without allocating (the frame loop calls this every frame). */
  setLodDistances(lod0: number, lod1: number): void {
    this.#lod0 = lod0;
    this.#lod1 = lod1;
  }

  /** Whether the ground-truth `ATTACKER` bit colours actors (09-ui §6: GT overlays can be locked off). */
  get showGroundTruth(): boolean {
    return this.#showGroundTruth;
  }

  set showGroundTruth(v: boolean) {
    if (v === this.#showGroundTruth) return;
    this.#showGroundTruth = v;
    this.#invalidateColors();
  }

  /** Whether benign actors are painted in their class colour instead of their own paint. */
  get colorBenignByClass(): boolean {
    return this.#benignByClass;
  }

  set colorBenignByClass(v: boolean) {
    if (v === this.#benignByClass) return;
    this.#benignByClass = v;
    this.#invalidateColors();
  }

  /** `"realistic"` or `"state"`; see {@link ActorRendererOptions.paint}. */
  get paint(): "realistic" | "state" {
    return this.#paint;
  }

  set paint(v: "realistic" | "state") {
    if (v === this.#paint) return;
    this.#paint = v;
    this.#invalidateColors();
  }

  /**
   * Whether the world is in New York City, where a share of passenger cars are drawn as yellow
   * cabs. The viewer decides it from `Hello`'s geodetic origin.
   */
  get newYork(): boolean {
    return this.#nyc;
  }

  /**
   * Set the region-dependent liveries: New York's yellow cabs and their share of passenger cars,
   * and the share of mopeds with a delivery box. Rebuilds the models if anything changed.
   *
   * The taxi share is a *drawing* parameter: the engine's classes have no taxi, so a yellow cab is
   * a passenger car in a medallion livery. Its default, 0.2, is a round figure — medallion cabs
   * were a quarter to a third of Midtown's vehicles before app-based for-hire vehicles, and fewer
   * since — stated as a choice, not a count. The delivery-box share of mopeds (0.5) is the same
   * kind of choice.
   */
  setRegion(nyc: boolean, taxiShare = this.#taxiShare, deliveryMopedShare = this.#deliveryMopedShare): void {
    if (nyc === this.#nyc && taxiShare === this.#taxiShare && deliveryMopedShare === this.#deliveryMopedShare) return;
    this.#nyc = nyc;
    this.#taxiShare = taxiShare;
    this.#deliveryMopedShare = deliveryMopedShare;
    this.#rebuild();
  }

  /** Actor id drawn in the selection colour, or −1. */
  get selectedActorId(): number {
    return this.#selectedActorId;
  }

  /**
   * One actor whose instance is not written this frame, or −1: the camera is inside its body (the
   * dashboard view), so drawing it would fill the frame with the inside of its own paint.
   */
  get hiddenActorId(): number {
    return this.#hiddenActorId;
  }

  set hiddenActorId(id: number) {
    this.#hiddenActorId = id;
  }

  set selectedActorId(id: number) {
    this.#selectedActorId = id;
  }

  /**
   * Exactly the colours this renderer draws, as a legend: the state colours it can write, the
   * class colours when those are on, and — in realistic paint — one `"paint"` row saying that
   * benign actors wear their own colours.
   */
  legend(): ActorLegendEntry[] {
    const out: ActorLegendEntry[] = [];
    const s = this.#theme.actorState;
    const realistic = this.#paint === "realistic" && !this.#benignByClass;
    for (const key of ACTOR_STATE_COLOR_KEYS) {
      if (key === "attacker" && !this.#showGroundTruth) continue;
      if (key === "benign" && (this.#benignByClass || realistic)) continue;
      out.push({ kind: "state", key, label: STATE_LABELS[key], color: s[key] });
    }
    if (realistic) {
      out.push({ kind: "paint", key: "benign", label: "Benign (own paint)", color: PAINT_PALETTE[0].hex });
    }
    if (this.#benignByClass) {
      for (let i = 0; i < this.#classes.length; i++) {
        const def = this.#classes[i];
        out.push({
          kind: "class",
          key: def.name,
          label: `${def.name} (benign)`,
          color: def.color !== 0 ? def.color : this.#theme.actorCategory[Math.min(def.category, 3)],
          classIndex: i,
        });
      }
    }
    return out;
  }

  /** Force a colour rewrite and upload on the next {@link update}. */
  #invalidateColors(): void {
    for (const b of this.#buckets) {
      b.colorKey.fill(0xff);
      b.colorDirty = true;
    }
  }

  /** Replace the class table (a new `Hello`). Rebuilds geometries and buckets. */
  setClasses(classes: readonly ActorClassDef[]): void {
    this.#classes = classes.length > 0 ? [...classes] : [...DEFAULT_ACTOR_CLASSES];
    this.#rebuild();
  }

  /** Swap the palette without rebuilding geometry. */
  setTheme(theme: ViewerTheme): void {
    this.#theme = theme;
    this.#refreshStateColors();
    this.#refreshClassColors();
    this.#invalidateColors();
  }

  #refreshStateColors(): void {
    const s = this.#theme.actorState;
    const keys = ACTOR_STATE_COLOR_KEYS.map((k) => s[k]);
    for (let i = 0; i < keys.length; i++) {
      this.#color.setHex(keys[i]);
      this.#stateColor[i * 3] = this.#color.r;
      this.#stateColor[i * 3 + 1] = this.#color.g;
      this.#stateColor[i * 3 + 2] = this.#color.b;
    }
  }

  #refreshClassColors(): void {
    const n = this.#classes.length;
    if (this.#classColor.length < n * 3) this.#classColor = new Float32Array(n * 3);
    for (let i = 0; i < n; i++) {
      const def = this.#classes[i];
      const hex = def.color !== 0 ? def.color : this.#theme.actorCategory[Math.min(def.category, 3)];
      this.#color.setHex(hex);
      this.#classColor[i * 3] = this.#color.r;
      this.#classColor[i * 3 + 1] = this.#color.g;
      this.#classColor[i * 3 + 2] = this.#color.b;
    }
  }

  #rebuild(): void {
    this.#disposeBuckets();
    const n = this.#classes.length;
    this.#radius = new Float32Array(n);
    this.#halfHeight = new Float32Array(n);
    this.#classColor = new Float32Array(n * 3);
    this.#refreshClassColors();
    this.#materials = [0, 1, 2].map((lod) => makeActorMaterial(lod as LodLevel, this.uniforms));
    this.#models = [];
    this.#classModels = new Int32Array(n * 2);
    this.#slotId.fill(0xffffffff);
    for (let c = 0; c < n; c++) {
      const def = this.#classes[c];
      this.#radius[c] = Math.hypot(def.lengthM, def.widthM, def.heightM) * 0.5 + this.#cullMargin;
      this.#halfHeight[c] = def.heightM * 0.5;
      const variants = modelVariants(def, this.#nyc, this.#taxiShare, this.#deliveryMopedShare);
      this.#classModels[c * 2] = this.#models.length;
      this.#classModels[c * 2 + 1] = variants.length;
      let total = 0;
      for (const v of variants) total += v.weight;
      let acc = 0;
      for (const v of variants) {
        acc += total > 0 ? v.weight / total : 1 / variants.length;
        const modelIndex = this.#models.length;
        const livery = liveryOf(v.kind);
        let lin: [number, number, number] | null = null;
        if (livery !== null) {
          this.#color.setHex(livery);
          lin = [this.#color.r, this.#color.g, this.#color.b];
        }
        const built = [0, 1, 2].map((lod) => buildActorModel(v.kind, def, lod as LodLevel));
        this.#models.push({ classIndex: c, kind: v.kind, info: built[0].info, cumulative: acc, livery: lin });
        for (let lod = 0; lod <= 2; lod++) {
          this.#geometries.push(built[lod].geometry);
          this.#buckets.push(this.#makeBucket(modelIndex, c, lod as LodLevel, this.#initialCapacity, built[lod].geometry));
        }
      }
    }
  }

  #makeBucket(
    modelIndex: number, classIndex: number, lod: LodLevel, capacity: number, geometry: BufferGeometry,
  ): Bucket {
    const def = this.#classes[classIndex];
    const material = this.#materials[lod];
    const mesh = new InstancedMesh<BufferGeometry, Material>(geometry, material, capacity);
    mesh.name = `actors/${def.name}/${this.#models[modelIndex]?.kind ?? "model"}/lod${lod}`;
    // We do our own culling; three's bounding sphere would be stale the moment an instance moves.
    mesh.frustumCulled = false;
    mesh.matrixAutoUpdate = false;
    mesh.matrix.identity();
    mesh.count = 0;
    mesh.visible = false;
    mesh.castShadow = this.#castShadows && lod === 0;
    mesh.receiveShadow = false;
    // Touch the colour attribute once so it exists and `setColorAt` never allocates in the hot loop.
    this.#color.setRGB(1, 1, 1);
    mesh.setColorAt(0, this.#color);
    mesh.instanceMatrix.setUsage(DynamicDrawUsage);
    if (mesh.instanceColor) mesh.instanceColor.setUsage(DynamicDrawUsage);
    const anim = new Float32Array(capacity * 4);
    const animAttr = new InstancedBufferAttribute(anim, 4);
    animAttr.setUsage(DynamicDrawUsage);
    geometry.setAttribute("iAnim", animAttr);
    this.group.add(mesh);
    return {
      mesh,
      capacity,
      cursor: 0,
      matrix: mesh.instanceMatrix.array as Float32Array,
      anim,
      animAttr,
      slots: new Int32Array(capacity),
      colorKey: new Uint8Array(capacity).fill(0xff),
      colorActor: new Uint32Array(capacity).fill(0xffffffff),
      colorDirty: true,
      matrixRange: { start: 0, count: 0 },
      colorRange: { start: 0, count: 0 },
      animRange: { start: 0, count: 0 },
      modelIndex,
      classIndex,
      lod,
    };
  }

  /**
   * Double a bucket's capacity. `InstancedMesh` cannot be resized, so a fresh one replaces it — and
   * the instances **already written this frame** are copied across, because growth happens in the
   * middle of {@link update}'s write loop and leaving them behind would flash that bucket's earlier
   * instances at the origin for one frame.
   */
  #grow(b: Bucket): void {
    const next = Math.min(this.#maxActors, b.capacity * 2);
    if (next <= b.capacity) return;
    const oldMesh = b.mesh;
    const oldMatrix = b.matrix;
    const oldAnim = b.anim;
    const oldColor = (oldMesh.instanceColor?.array ?? null) as Float32Array | null;
    const oldSlots = b.slots;
    const oldKeys = b.colorKey;
    const oldActors = b.colorActor;
    const written = Math.min(b.cursor, b.capacity);

    this.group.remove(oldMesh);
    const fresh = this.#makeBucket(b.modelIndex, b.classIndex, b.lod, next, oldMesh.geometry);
    if (written > 0) {
      fresh.matrix.set(oldMatrix.subarray(0, written * 16), 0);
      fresh.anim.set(oldAnim.subarray(0, written * 4), 0);
      const freshColor = (fresh.mesh.instanceColor?.array ?? null) as Float32Array | null;
      if (freshColor && oldColor) freshColor.set(oldColor.subarray(0, written * 3), 0);
      fresh.slots.set(oldSlots.subarray(0, written), 0);
      fresh.colorKey.set(oldKeys.subarray(0, written), 0);
      fresh.colorActor.set(oldActors.subarray(0, written), 0);
    }
    // The geometry is shared with the fresh mesh; only the old mesh's instance buffers go.
    oldMesh.dispose();

    b.mesh = fresh.mesh;
    b.capacity = next;
    b.matrix = fresh.matrix;
    b.anim = fresh.anim;
    b.animAttr = fresh.animAttr;
    b.slots = fresh.slots;
    b.colorKey = fresh.colorKey;
    b.colorActor = fresh.colorActor;
    b.colorDirty = true;
  }

  #ensureSlots(n: number): void {
    if (n <= this.#slotCap) return;
    let c = Math.max(64, this.#slotCap);
    while (c < n) c *= 2;
    const g64 = (a: Float64Array): Float64Array => {
      const o = new Float64Array(c);
      o.set(a);
      return o;
    };
    const g32 = (a: Float32Array, k = 1): Float32Array => {
      const o = new Float32Array(c * k);
      o.set(a);
      return o;
    };
    const id = new Uint32Array(c).fill(0xffffffff);
    id.set(this.#slotId);
    this.#slotId = id;
    const m = new Int16Array(c);
    m.set(this.#slotModel);
    this.#slotModel = m;
    const band = new Int8Array(c).fill(-1);
    band.set(this.#slotBand);
    this.#slotBand = band;
    this.#slotPaint = g32(this.#slotPaint, 3);
    this.#slotPhase = g32(this.#slotPhase);
    this.#odo = g64(this.#odo);
    this.#gait = g64(this.#gait);
    this.#amp = g32(this.#amp);
    this.#steer = g32(this.#steer);
    this.#px = g64(this.#px);
    this.#py = g64(this.#py);
    this.#ph = g32(this.#ph);
    this.#slotCap = c;
  }

  /** Adopt a slot for a (new) actor: its model, its paint, its flash phase, a fresh odometer. */
  #adopt(s: number, id: number, c: number, x: number, y: number, h: number): void {
    this.#slotId[s] = id;
    const first = this.#classModels[c * 2];
    const count = this.#classModels[c * 2 + 1];
    const u = hashId(id, 1);
    let mi = first;
    for (let k = 0; k < count; k++) {
      mi = first + k;
      if (u < this.#models[mi].cumulative) break;
    }
    this.#slotModel[s] = mi;
    const model = this.#models[mi];
    const p = s * 3;
    if (model.livery) {
      this.#slotPaint[p] = model.livery[0];
      this.#slotPaint[p + 1] = model.livery[1];
      this.#slotPaint[p + 2] = model.livery[2];
    } else {
      const def = this.#classes[c];
      let hex: number;
      if (def.category === 1 || model.kind === "pedestrian") {
        hex = CLOTHING_PALETTE[Math.floor(hashId(id, 2) * CLOTHING_PALETTE.length)];
      } else {
        const v = hashId(id, 2);
        let acc = 0;
        hex = PAINT_PALETTE[0].hex;
        for (const e of PAINT_PALETTE) {
          acc += e.weight;
          if (v < acc) {
            hex = e.hex;
            break;
          }
        }
      }
      this.#color.setHex(hex);
      this.#slotPaint[p] = this.#color.r;
      this.#slotPaint[p + 1] = this.#color.g;
      this.#slotPaint[p + 2] = this.#color.b;
    }
    this.#slotPhase[s] = hashId(id, 3);
    this.#slotBand[s] = -1;
    this.#odo[s] = hashId(id, 4) * 10;
    this.#gait[s] = hashId(id, 5) * TAU;
    this.#amp[s] = 0;
    this.#steer[s] = 0;
    this.#px[s] = x;
    this.#py[s] = y;
    this.#ph[s] = h;
  }

  /**
   * Write one frame's worth of instances. Returns the same object every call — copy what you need.
   */
  update(ctx: ActorUpdateContext): ActorUpdateStats {
    const cam = ctx.camera;
    const cull = ctx.cull !== false;
    const buckets = this.#buckets;
    for (let i = 0; i < buckets.length; i++) buckets[i].cursor = 0;
    this.#visibleCount = 0;
    this.#ensureSlots(ctx.count);
    const dt = ctx.dtSeconds !== undefined && ctx.dtSeconds > 0 ? Math.min(0.25, ctx.dtSeconds) : 1 / 60;
    if (ctx.timeSeconds !== undefined) this.uniforms.uTime.value = ctx.timeSeconds % 3600;
    // The steering filter: a first-order lag of 0.15 s, about the time a driver takes to wind the
    // wheel into a turn, so the drawn angle follows the path's curvature without its noise.
    const steerK = 1 - Math.exp(-dt / 0.15);
    const ampK = 1 - Math.exp(-dt / 0.25);

    if (cull) this.#extractPlanes(cam);

    const camX = cam.matrixWorld.elements[12];
    const camY = cam.matrixWorld.elements[13];
    const camZ = cam.matrixWorld.elements[14];
    // Hysteresis on the LOD bands: an actor changes detail 10 % past a boundary on the way out and
    // 10 % inside it on the way back, so one parked on a boundary under a jittering camera does
    // not change model every frame (the glitch hunter's `lod_pop`).
    const lod0In = (this.#lod0 * 0.9) ** 2;
    const lod0Out = (this.#lod0 * 1.1) ** 2;
    const lod1In = (this.#lod1 * 0.9) ** 2;
    const lod1Out = (this.#lod1 * 1.1) ** 2;
    const lod0Sq = this.#lod0 * this.#lod0;
    const lod1Sq = this.#lod1 * this.#lod1;

    const pos = ctx.position;
    const head = ctx.heading;
    const cls = ctx.classIdx;
    const st = ctx.state;
    const occ = ctx.occupied;
    const ids = ctx.actorId;
    const spd = ctx.speed;
    const lampsIn = ctx.lamps;
    const fadeIn = ctx.fade;
    const nClasses = this.#classes.length;
    const planes = this.#planes;
    const lift = this.#groundOffset;
    const stateColor = this.#stateColor;
    const classColor = this.#classColor;
    const selected = this.#selectedActorId;
    const selectedU = selected >>> 0;
    const hidden = this.#hiddenActorId;
    const hiddenU = hidden >>> 0;
    const gt = this.#showGroundTruth;
    const benignByClass = this.#benignByClass;
    const realistic = this.#paint === "realistic";
    const col = this.#color;
    const models = this.#models;

    if (this.slotLod.length < ctx.count) {
      let n = Math.max(64, this.slotLod.length);
      while (n < ctx.count) n *= 2;
      this.slotLod = new Int8Array(n);
    }
    const slotLod = this.slotLod;
    slotLod.fill(-1, 0, ctx.count);

    let live = 0;
    let drawn = 0;
    let culled = 0;
    let dropped = 0;
    let lod0n = 0;
    let lod1n = 0;
    let lod2n = 0;

    for (let s = 0; s < ctx.count; s++) {
      if (occ[s] === 0) continue;
      live++;
      let c = cls[s];
      if (c >= nClasses) c = 0;
      const p = s * 3;
      const x = pos[p];
      const y = pos[p + 1];
      const h = head[s];
      const id = ids[s];
      if (this.#slotId[s] !== id || models[this.#slotModel[s]]?.classIndex !== c) this.#adopt(s, id, c, x, y, h);
      const model = models[this.#slotModel[s]];

      // Animation state, for every live actor, so one entering the view is already rolling.
      const mx = x - this.#px[s];
      const my = y - this.#py[s];
      const moved = Math.hypot(mx, my);
      if (moved < 8) {
        // Signed along the heading, so a vehicle that reverses rolls its wheels backwards.
        const along = mx * Math.cos(h) + my * Math.sin(h);
        this.#odo[s] += along;
        const v = spd ? Math.abs(spd[s]) : moved / dt;
        if (model.info.strideM > 0) this.#gait[s] += (Math.abs(along) / model.info.strideM) * TAU;
        // Swing amplitude: full at a normal walk (1.3 m/s), none standing.
        const target = Math.min(1, v / 1.3);
        this.#amp[s] += (target - this.#amp[s]) * ampK;
        if (model.info.wheelbaseM > 0 && v > 0.8) {
          const yaw = wrap(h - this.#ph[s]) / dt;
          // Kinematic bicycle model: tan δ = wheelbase · yaw rate / speed.
          let delta = Math.atan((model.info.wheelbaseM * yaw) / v);
          if (delta > 0.6) delta = 0.6;
          else if (delta < -0.6) delta = -0.6;
          this.#steer[s] += (delta - this.#steer[s]) * steerK;
        }
      }
      this.#px[s] = x;
      this.#py[s] = y;
      this.#ph[s] = h;

      // Still live and still pickable; just not drawn. See `hiddenActorId`.
      if (hidden >= 0 && id === hiddenU) continue;
      const z = pos[p + 2] + lift;
      const dx = x - camX;
      const dy = y - camY;
      const dz = z - camZ;
      const dist2 = dx * dx + dy * dy + dz * dz;

      const r = this.#radius[c];
      if (cull) {
        const cz = z + this.#halfHeight[c];
        let outside = false;
        for (let k = 0; k < 6; k++) {
          const o = k * 4;
          if (planes[o] * x + planes[o + 1] * y + planes[o + 2] * cz + planes[o + 3] < -r) {
            outside = true;
            break;
          }
        }
        if (outside) {
          culled++;
          this.#slotBand[s] = -1;
          continue;
        }
      }

      if (drawn >= this.#maxActors) {
        dropped++;
        continue;
      }

      const prevBand = this.#slotBand[s];
      let lod: LodLevel;
      if (prevBand === 0) lod = dist2 < lod0Out ? 0 : dist2 < lod1Out ? 1 : 2;
      else if (prevBand === 1) lod = dist2 < lod0In ? 0 : dist2 < lod1Out ? 1 : 2;
      else if (prevBand === 2) lod = dist2 < lod0In ? 0 : dist2 < lod1In ? 1 : 2;
      else lod = dist2 < lod0Sq ? 0 : dist2 < lod1Sq ? 1 : 2;
      this.#slotBand[s] = lod;
      if (lod === 0) lod0n++;
      else if (lod === 1) lod1n++;
      else lod2n++;

      const b = buckets[this.#slotModel[s] * 3 + lod];
      if (b.cursor >= b.capacity) {
        if (b.capacity >= this.#maxActors) {
          dropped++;
          continue;
        }
        this.#grow(b);
      }

      const i = b.cursor++;
      const m = b.matrix;
      const o = i * 16;
      const cosH = Math.cos(h);
      const sinH = Math.sin(h);
      // Column-major, identical to what `Matrix4.toArray` would write for
      // makeRotationZ(h) then setPosition(x, y, z).
      m[o] = cosH; m[o + 1] = sinH; m[o + 2] = 0; m[o + 3] = 0;
      m[o + 4] = -sinH; m[o + 5] = cosH; m[o + 6] = 0; m[o + 7] = 0;
      m[o + 8] = 0; m[o + 9] = 0; m[o + 10] = 1; m[o + 11] = 0;
      m[o + 12] = x; m[o + 13] = y; m[o + 14] = z; m[o + 15] = 1;

      const a = b.anim;
      const ao = i * 4;
      const wr = model.info.wheelRadiusM;
      a[ao] = wr > 0 ? (this.#odo[s] / wr) % TAU : 0;
      a[ao + 1] = this.#steer[s];
      // A cyclist's legs follow the crank: one turn per 6.5 m, a mid gear's development.
      a[ao + 2] = model.info.strideM > 0 ? this.#gait[s] % TAU : ((this.#odo[s] / 6.5) * TAU) % TAU;
      a[ao + 3] = packLamps(
        lampsIn ? lampsIn[s] : 0, this.#slotPhase[s], model.info.strideM > 0 ? this.#amp[s] : 0.8,
        fadeIn ? fadeIn[s] : 1,
      );

      // Colour: selection and §3.3.4 state first (the order `actorColorKey` gives, which `legend()`
      // reports from); a benign actor in its own paint, or its class colour, or the benign colour.
      // Written, and uploaded, only on a change.
      const state = st[s];
      let ci = actorStateColorIndex(state, selected >= 0 && id === selectedU, gt);
      if (ci === 0 && benignByClass) ci = CLASS_COLOR_KEY;
      else if (ci === 0 && realistic) ci = PAINT_COLOR_KEY;
      const paintOwner = ci === PAINT_COLOR_KEY ? id : 0xffffffff;
      if (b.colorKey[i] !== ci || b.colorActor[i] !== paintOwner) {
        b.colorKey[i] = ci;
        b.colorActor[i] = paintOwner;
        b.colorDirty = true;
        if (ci === PAINT_COLOR_KEY) {
          col.setRGB(this.#slotPaint[p], this.#slotPaint[p + 1], this.#slotPaint[p + 2]);
        } else if (ci === CLASS_COLOR_KEY) {
          col.setRGB(classColor[c * 3], classColor[c * 3 + 1], classColor[c * 3 + 2]);
        } else {
          col.setRGB(stateColor[ci * 3], stateColor[ci * 3 + 1], stateColor[ci * 3 + 2]);
        }
        b.mesh.setColorAt(i, col);
      }

      b.slots[i] = s;
      slotLod[s] = lod;
      this.visibleSlots[this.#visibleCount++] = s;
      drawn++;
    }

    let usedBuckets = 0;
    for (let k = 0; k < buckets.length; k++) {
      const b = buckets[k];
      const n = b.cursor;
      b.mesh.count = n;
      b.mesh.visible = n > 0;
      if (n > 0) {
        usedBuckets++;
        const im = b.mesh.instanceMatrix;
        im.clearUpdateRanges();
        b.matrixRange.start = 0;
        b.matrixRange.count = n * 16;
        // Not `addUpdateRange`, which allocates a fresh `{start, count}` on every call
        // (three 0.186.0, BufferAttribute.js:181). three clears the array after each upload, so
        // the same object is re-pushed rather than kept.
        im.updateRanges.push(b.matrixRange);
        im.needsUpdate = true;
        const aa = b.animAttr;
        aa.clearUpdateRanges();
        b.animRange.start = 0;
        b.animRange.count = n * 4;
        aa.updateRanges.push(b.animRange);
        aa.needsUpdate = true;
        const ic = b.mesh.instanceColor;
        if (ic && b.colorDirty) {
          ic.clearUpdateRanges();
          b.colorRange.start = 0;
          b.colorRange.count = n * 3;
          ic.updateRanges.push(b.colorRange);
          ic.needsUpdate = true;
          b.colorDirty = false;
        }
      }
    }

    this.#stats = {
      live, drawn, culled, dropped, buckets: usedBuckets, lod0: lod0n, lod1: lod1n, lod2: lod2n,
    };
    return this.#stats;
  }

  /** The first bucket a `(class, lod)` pair draws into — the class's first model. */
  bucketAt(classIndex: number, lod: LodLevel): {
    readonly mesh: InstancedMesh<BufferGeometry, Material>;
    readonly count: number;
    readonly capacity: number;
  } | null {
    return this.bucketsAt(classIndex, lod)[0] ?? null;
  }

  /** Every bucket a `(class, lod)` pair draws into, one per model of the class. */
  bucketsAt(classIndex: number, lod: LodLevel): {
    readonly mesh: InstancedMesh<BufferGeometry, Material>;
    readonly count: number;
    readonly capacity: number;
    readonly model: ActorModelKind;
  }[] {
    const out: { mesh: InstancedMesh<BufferGeometry, Material>; count: number; capacity: number; model: ActorModelKind }[] = [];
    if (classIndex < 0 || classIndex * 2 + 1 >= this.#classModels.length) return out;
    const first = this.#classModels[classIndex * 2];
    const count = this.#classModels[classIndex * 2 + 1];
    for (let k = 0; k < count; k++) {
      const b = this.#buckets[(first + k) * 3 + lod];
      if (b) out.push({ mesh: b.mesh, count: b.cursor, capacity: b.capacity, model: this.#models[first + k].kind });
    }
    return out;
  }

  /** The model an actor in `slot` was last drawn as, or null. */
  modelOfSlot(slot: number): ActorModelKind | null {
    if (slot < 0 || slot >= this.#slotCap || this.#slotId[slot] === 0xffffffff) return null;
    return this.#models[this.#slotModel[slot]]?.kind ?? null;
  }

  /** The steering angle drawn for `slot`, radians (left positive). */
  steerOfSlot(slot: number): number {
    return slot >= 0 && slot < this.#slotCap ? this.#steer[slot] : 0;
  }

  /** Total instance capacity currently allocated across all buckets. */
  get allocatedCapacity(): number {
    let n = 0;
    for (const b of this.#buckets) n += b.capacity;
    return n;
  }

  #extractPlanes(cam: Camera): void {
    const m = this.#viewProjection.multiplyMatrices(cam.projectionMatrix, cam.matrixWorldInverse).elements;
    const p = this.#planes;
    const set = (k: number, a: number, b: number, c: number, d: number): void => {
      const inv = 1 / (Math.hypot(a, b, c) || 1);
      p[k * 4] = a * inv;
      p[k * 4 + 1] = b * inv;
      p[k * 4 + 2] = c * inv;
      p[k * 4 + 3] = d * inv;
    };
    set(0, m[3] - m[0], m[7] - m[4], m[11] - m[8], m[15] - m[12]);
    set(1, m[3] + m[0], m[7] + m[4], m[11] + m[8], m[15] + m[12]);
    set(2, m[3] + m[1], m[7] + m[5], m[11] + m[9], m[15] + m[13]);
    set(3, m[3] - m[1], m[7] - m[5], m[11] - m[9], m[15] - m[13]);
    set(4, m[3] - m[2], m[7] - m[6], m[11] - m[10], m[15] - m[14]);
    set(5, m[3] + m[2], m[7] + m[6], m[11] + m[10], m[15] + m[14]);
  }

  #disposeBuckets(): void {
    for (const b of this.#buckets) {
      this.group.remove(b.mesh);
      b.mesh.dispose();
    }
    this.#buckets = [];
    for (const g of this.#geometries) g.dispose();
    this.#geometries = [];
    for (const m of this.#materials) m.dispose();
    this.#materials = [];
  }

  /** Release every GPU resource this renderer owns. */
  dispose(): void {
    this.#disposeBuckets();
    this.group.removeFromParent();
  }
}
