/**
 * Traffic-signal heads: what each lantern shows, and the stop bar on the road in front of it.
 *
 * ## Which rows light which heads
 *
 * The world carries one record per *physical head* (§4.5), and several heads share a `signal_id`
 * — the controller's id (`crates/v2xw-world/src/serde_vwp.rs`: "a controller with heads on four
 * approaches produces four records that share its `signal_id`"). A stream signal row (§3.3.3) is
 * keyed by that same id, so it applies to **every** head of the controller. The previous renderer
 * kept a `Map<signal_id, row index>` and let each head overwrite the last: one head per controller
 * ever changed colour and the rest stayed dark, which is "sometimes I see the light turn green and
 * sometimes not" exactly.
 *
 * A keyframe (§3.3.3) is the complete signal state, so it is applied as one: every head not in it
 * goes back to "no data". A delta (§3.4.7) carries only the rows that changed and is applied on top.
 * The {@link SignalRenderer.applyKeyframe}/{@link SignalRenderer.applyDelta} split is what makes
 * the lamps come out right after a seek, a rewind or a reconnect — a lamp that depended on having
 * seen every delta since the last keyframe is the other half of the owner's report.
 *
 * The engine now publishes a row per signal *group* (vwp-v1 §3.3.3): its id is
 * `(controller + 1) · 65536 + group`, and it lights only the heads of that group, so the
 * cross-street heads of a junction show their own movement's state. A plain controller id
 * (< 65536, what the mock server and older recordings send) still lights every head of that
 * controller. Each head is therefore indexed under both keys.
 *
 * ## What a lamp looks like
 *
 * A three-aspect vertical lantern, red over amber over green (MUTCD 2009 §4D.11 and Vienna
 * Convention 1968 Art. 23: the arrangement is fixed so position alone carries the meaning), facing
 * the approach it controls. The lit aspect is drawn unlit-shaded at full value, the others at a
 * tenth — so position, colour and brightness all say the same thing and none of them depends on the
 * sun. Flashing states (J2735 `stop-Then-Proceed`, `caution-Conflicting-Traffic`) flash at 1 Hz,
 * inside MUTCD §4D.30's 50–60 flashes a minute. The J2735 `MovementPhaseState` meanings are from SAE
 * J2735 (2016) DE_MovementPhaseState.
 *
 * A stop bar across the controlled lane repeats the state on the road surface, which is the only
 * place it can be read from map altitude where a 1 m lantern is a fraction of a pixel. At street
 * level it fades out ({@link SignalRenderer.setBarVisibility}): there the lamps can be read, and the
 * white stop line painted on the road (`markings.ts`) is what a driver sees.
 *
 * ## How a head is built and mounted
 *
 * Each section is a round 12-inch (300 mm) lens (MUTCD 2009 §4D.07) under a tunnel visor, in a
 * dark housing. The engine places a head over its approach lane's stop line, 5 m up; it is hung
 * from a mast arm that runs out from a pole on the kerb to the right of the approach (the usual
 * US overhead mounting, MUTCD §4D.15 — bottom of the housing at least 15 ft above the road).
 * Heads of one approach share a pole. A pedestrian head stands on its own post at the kerb.
 *
 * ## Lens shapes
 *
 * A head over a lane that only turns (left, or left and U-turn; or right) and whose signal group is
 * not the group of a through lane on the same approach is a separate turn signal face, and its
 * lenses are arrows pointing the way the lane turns: a steady red arrow, a yellow arrow and a green
 * arrow (MUTCD 2009 §4D.06 and §4D.20). When such a face's group is in a permissive state (J2735
 * `permissive-Movement-Allowed`) it shows the flashing yellow arrow, which is how a separate face
 * says "turn, but yield" (§4D.18 and §4D.20); a circular green is never shown in an arrow-only
 * face. Every other head has round lenses. A pedestrian head's sections are its symbols, the
 * UPRAISED HAND and the WALKING PERSON (§4E.04), not discs.
 */

import {
  BufferGeometry,
  Color,
  Group,
  InstancedMesh,
  Matrix4,
  MeshBasicMaterial,
  MeshLambertMaterial,
  type Material,
} from "three";
import type { SignalBlock, VwpWorld } from "@vwp/protocol";
import { MeshBuilder, addBox, addCylinder, withUnitVertexColors } from "./geometry.js";
import type { ViewerTheme } from "./theme.js";
import { MOVE, laneMovements } from "./markings.js";

/**
 * The stream id of one signal group's row: `(controller + 1) · 65536 + group` (vwp-v1 §3.3.3,
 * `v2xw_world::signal_group_wire_id`). Always ≥ 65536, so it never collides with a plain
 * controller id.
 */
export function signalGroupWireId(controller: number, group: number): number {
  return (controller + 1) * 65536 + group;
}

/** Aspect bits. */
const RED = 1;
const AMBER = 2;
const GREEN = 4;

/** Phase code for "the stream has said nothing about this head". Not a J2735 value. */
export const PHASE_NO_DATA = 0xff;

/** What a J2735 `MovementPhaseState` lights: aspect bits, and whether they flash. */
export interface SignalAspect {
  readonly lamps: number;
  readonly flashing: boolean;
  /** Short word for the HUD and tests. */
  readonly name: "no-data" | "unavailable" | "dark" | "red" | "red-flashing" | "red-amber" | "green" | "amber" | "amber-flashing";
}

/** SAE J2735 (2016) DE_MovementPhaseState → the aspect a lantern shows. */
export function aspectOf(phase: number): SignalAspect {
  switch (phase) {
    case 0: return { lamps: 0, flashing: false, name: "unavailable" };
    case 1: return { lamps: 0, flashing: false, name: "dark" };
    case 2: return { lamps: RED, flashing: true, name: "red-flashing" }; // stop-Then-Proceed
    case 3: return { lamps: RED, flashing: false, name: "red" }; // stop-And-Remain
    case 4: return { lamps: RED | AMBER, flashing: false, name: "red-amber" }; // pre-Movement
    case 5: // permissive-Movement-Allowed
    case 6: return { lamps: GREEN, flashing: false, name: "green" }; // protected-Movement-Allowed
    case 7: // permissive-clearance
    case 8: return { lamps: AMBER, flashing: false, name: "amber" }; // protected-clearance
    case 9: return { lamps: AMBER, flashing: true, name: "amber-flashing" }; // caution-Conflicting-Traffic
    default: return { lamps: 0, flashing: false, name: "no-data" };
  }
}

/**
 * The flashing yellow arrow: what a separate turn face shows for a permissive movement (MUTCD 2009
 * §4D.18, §4D.20).
 */
const FLASHING_YELLOW_ARROW: SignalAspect = { lamps: AMBER, flashing: true, name: "amber-flashing" };

/** A lens's shape. */
export const LENS = { BALL: 0, LEFT_ARROW: 1, RIGHT_ARROW: 2, HAND: 3, WALKER: 4 } as const;
const LENS_SHAPES = 5;
/** "No lens here" in the per-lamp shape table (the hidden middle slot of a pedestrian head). */
const NO_LENS = 0xff;

/** One head's state, as {@link SignalRenderer.headState} reports it. */
export interface SignalHeadState {
  readonly signalId: number;
  readonly phase: number;
  readonly aspect: SignalAspect;
  /** Deciseconds to the next change as last reported, `0xFFFF` unknown. */
  readonly timeToChangeDs: number;
}

/** Unlit aspects are drawn at this fraction of their lit value. */
const UNLIT = 0.1;
/** Flashing period, seconds (MUTCD §4D.30: 50–60 flashes per minute). */
const FLASH_PERIOD_S = 1;

/** Lantern dimensions, metres: a 12-inch three-section head is about 0.35 × 1.05. */
const HOUSING_W = 0.36;
const HOUSING_H = 1.05;
const HOUSING_D = 0.26;
const LAMP_SPACING = 0.32;
/** A 12-inch lens (MUTCD 2009 §4D.07), metres of diameter. */
const LENS_D = 0.3;
/** Mounting: pole radius and the arm's section, metres. */
const POLE_R = 0.13;
const ARM_T = 0.16;
/**
 * A pedestrian head (§4.5 `kind` 1), MUTCD 2009 §4E.04: two sections, the UPRAISED HAND above the
 * WALKING PERSON, each about 12 inches square. It shows walk while its crosswalk may be entered
 * (J2735 movement-allowed), the hand flashing through the pedestrian change interval
 * (clearance), and the hand steady for don't-walk; and it has no stop bar, because its lane is
 * the crosswalk, not an approach.
 */
const PED_HOUSING_W = 0.34;
const PED_HOUSING_H = 0.72;
const PED_LAMP_SPACING = 0.34;
/** The walking person is lunar white (MUTCD §4E.04). */
const PED_WALK_COLOR = 0xf2f4f7;

/** Stop bar depth along the lane, metres (MUTCD §3B.16: 12–24 inches). */
const STOP_BAR_DEPTH = 0.5;
/** Height of the stop bar above the lane centreline, metres: just above lane markings (0.26). */
const STOP_BAR_Z = 0.28;
/** The stop bars' polygon offset, units: two steps above the lane markings'. */
export const STOP_BAR_OFFSET_UNITS = -14;

export class SignalRenderer {
  readonly group = new Group();

  #housing: InstancedMesh<BufferGeometry, Material> | null = null;
  /** One instanced mesh per lens shape ({@link LENS}); null where no lamp has that shape. */
  #lampMeshes: (InstancedMesh<BufferGeometry, Material> | null)[] = [];
  /** Per lamp `i * 3 + k`: its shape ({@link LENS}, or {@link NO_LENS}) and its slot in that mesh. */
  #lampShape = new Uint8Array(0);
  #lampSlot = new Uint32Array(0);
  /** 1 for a separate turn face (arrow lenses). */
  #arrowFace = new Uint8Array(0);
  #bars: InstancedMesh<BufferGeometry, Material> | null = null;
  #poles: InstancedMesh<BufferGeometry, Material> | null = null;
  #arms: InstancedMesh<BufferGeometry, Material> | null = null;
  #mountMaterial = new MeshLambertMaterial({ color: 0x3a3f45, name: "signal-mounts" });
  /** How visible the coloured stop bars are, `[0, 1]`; see {@link setBarVisibility}. */
  #barVisibility = 1;
  /** Mast-arm poles built. */
  poleCount = 0;
  #anyBar = false;
  #housingMaterial = new MeshLambertMaterial({ color: 0x1b1e22, name: "signal-housing" });
  #lampMaterial = new MeshBasicMaterial({ vertexColors: true, toneMapped: false, name: "signal-lamps" });
  #barMaterial = new MeshBasicMaterial({
    vertexColors: true, toneMapped: false, name: "signal-stop-bars", transparent: true,
    // Above the road surface's highest rank bias and the markings (`world-render.ts`
    // ABOVE_SURFACE_OFFSET_UNITS, −12; a literal here because world-render imports this module).
    polygonOffset: true, polygonOffsetFactor: -4, polygonOffsetUnits: STOP_BAR_OFFSET_UNITS,
  });
  #geometries: BufferGeometry[] = [];

  #count = 0;
  /** 1 for a pedestrian head (§4.5 `kind` 1), 0 for a vehicle one. */
  #pedestrian = new Uint8Array(0);
  #headSignal = new Uint32Array(0);
  #phase = new Uint8Array(0);
  #ttc = new Uint16Array(0);
  /** Stream signal id (a group id or a plain controller id) → head indices. */
  #heads = new Map<number, number[]>();
  #hasBar = new Uint8Array(0);
  #colors = { red: new Color(), amber: new Color(), green: new Color() };
  #scratch = new Color();
  #flashOn = true;
  #anyFlashing = false;
  #theme: ViewerTheme;
  /** The world last built; a rebuild of the same one keeps the lamp states. */
  #builtFrom: VwpWorld | null = null;

  constructor(theme: ViewerTheme) {
    this.#theme = theme;
    this.group.name = "world/signals";
    this.setTheme(theme);
  }

  /** Number of heads built. */
  get count(): number {
    return this.#count;
  }

  setTheme(theme: ViewerTheme): void {
    this.#theme = theme;
    this.#colors.red.setHex(theme.signalRed);
    this.#colors.amber.setHex(theme.signalAmber);
    this.#colors.green.setHex(theme.signalGreen);
    this.#repaint();
  }

  /**
   * Build one lantern and one stop bar per §4.5 signal record. What each controller showed is kept
   * across a rebuild — a theme swap rebuilds the world, and must not blank every lamp until the
   * next keyframe (which, on a paused run, never comes).
   */
  build(world: VwpWorld): void {
    // Kept per head: the same world builds the same heads in the same order, and two heads of
    // one controller can be in different groups, so the controller id is not a fine enough key.
    const kept = new Map<number, { phase: number; ttc: number }>();
    for (let i = 0; world === this.#builtFrom && i < this.#count; i++) {
      if (this.#phase[i] !== PHASE_NO_DATA) kept.set(i, { phase: this.#phase[i], ttc: this.#ttc[i] });
    }
    this.clear();
    this.#builtFrom = world;
    const n = world.signals.count;
    this.#count = n;
    this.#headSignal = new Uint32Array(n);
    this.#phase = new Uint8Array(n).fill(PHASE_NO_DATA);
    this.#ttc = new Uint16Array(n).fill(0xffff);
    this.#hasBar = new Uint8Array(n);
    this.#pedestrian = new Uint8Array(n);
    this.#arrowFace = new Uint8Array(n);
    this.#lampShape = new Uint8Array(n * 3).fill(NO_LENS);
    this.#lampSlot = new Uint32Array(n * 3);
    this.#heads.clear();
    if (n === 0) return;

    // The approach lane's end: where the stop line is and which way the head must face.
    const laneIndex = new Map<number, number>();
    for (let i = 0; i < world.lanes.count; i++) laneIndex.set(world.lanes.laneId[i], i);

    // Which lens each lamp has: arrows on a separate turn face, symbols on a pedestrian head.
    const shapes = lensShapes(world, laneIndex);
    const housingGeom = housingGeometry();
    // Lenses are flat shapes on the face towards the approach (local −x).
    const lensGeoms = [
      withUnitVertexColors(lensGeometry(LENS_D / 2, 16)),
      withUnitVertexColors(arrowLensGeometry(1)),
      withUnitVertexColors(arrowLensGeometry(-1)),
      withUnitVertexColors(handLensGeometry()),
      withUnitVertexColors(walkerLensGeometry()),
    ];
    const barGeom = withUnitVertexColors(boxGeometry(1, 1, 0.02));
    const poleGeom = cylinderGeometry();
    const armGeom = boxGeometry(1, 1, 1);
    this.#geometries.push(housingGeom, ...lensGeoms, barGeom, poleGeom, armGeom);

    const housing = new InstancedMesh<BufferGeometry, Material>(housingGeom, this.#housingMaterial, n);
    // Lamp (i, k) → its shape's mesh and a slot in it.
    const perShape = new Array<number>(LENS_SHAPES).fill(0);
    for (let i = 0; i < n; i++) {
      for (let k = 0; k < 3; k++) {
        const shape = shapes.lamp[i * 3 + k];
        this.#lampShape[i * 3 + k] = shape;
        if (shape === NO_LENS) continue;
        this.#lampSlot[i * 3 + k] = perShape[shape]++;
      }
      this.#arrowFace[i] = shapes.arrowFace[i];
    }
    const lampMeshes = perShape.map((count, shape) => {
      if (count === 0) return null;
      const mesh = new InstancedMesh<BufferGeometry, Material>(lensGeoms[shape], this.#lampMaterial, count);
      mesh.name = shape === LENS.BALL ? "world/signal-lamps" : `world/signal-lamps-${LENS_NAMES[shape]}`;
      return mesh;
    });
    const bars = new InstancedMesh<BufferGeometry, Material>(barGeom, this.#barMaterial, n);
    housing.name = "world/signal-housings";
    bars.name = "world/signal-stop-bars";
    for (const m of [housing, bars, ...lampMeshes]) {
      if (!m) continue;
      m.castShadow = false;
      m.receiveShadow = false;
    }

    const m = new Matrix4();
    const lx = world.lanePoints.x;
    const ly = world.lanePoints.y;
    const lz = world.lanePoints.z;
    // Mounts: a pole per kerb spot (shared by the heads of one approach) and an arm per head.
    const poleAt = new Map<string, { x: number; y: number; z0: number; top: number }>();
    const arms: { x0: number; y0: number; x1: number; y1: number; z: number }[] = [];
    for (let i = 0; i < n; i++) {
      const s = world.signals.at(i);
      this.#headSignal[i] = s.signalId;
      const ped = s.kind === 1;
      this.#pedestrian[i] = ped ? 1 : 0;
      for (const key of [signalGroupWireId(s.signalId, s.group), s.signalId]) {
        let list = this.#heads.get(key);
        if (!list) {
          list = [];
          this.#heads.set(key, list);
        }
        list.push(i);
      }

      let yaw = 0;
      let bar: { x: number; y: number; z: number; width: number } | null = null;
      // (A pedestrian head gets its yaw from its crosswalk lane below, and no bar.)
      const li = laneIndex.get(s.laneId);
      if (li !== undefined && world.lanes.pointCount[li] >= 2 && !ped) {
        const off = world.lanes.pointOff[li];
        const last = off + world.lanes.pointCount[li] - 1;
        const dx = lx[last] - lx[last - 1];
        const dy = ly[last] - ly[last - 1];
        const len = Math.hypot(dx, dy);
        if (len > 1e-6) {
          yaw = Math.atan2(dy, dx);
          const ux = dx / len;
          const uy = dy / len;
          bar = {
            x: lx[last] - ux * STOP_BAR_DEPTH * 0.5,
            y: ly[last] - uy * STOP_BAR_DEPTH * 0.5,
            z: (lz ? lz[last] : 0) + STOP_BAR_Z,
            width: Math.max(1, world.lanes.widthM[li] - 0.2),
          };
        }
      }
      if (ped && li !== undefined && world.lanes.pointCount[li] >= 2) {
        // A pedestrian head stands at the far kerb of its crosswalk and faces back along it,
        // at the people about to cross.
        const off = world.lanes.pointOff[li];
        const last = off + world.lanes.pointCount[li] - 1;
        yaw = Math.atan2(ly[last] - ly[last - 1], lx[last] - lx[last - 1]);
      }
      // The mount. A vehicle head hangs from an arm out of a pole at the kerb to the right of its
      // approach: the lane's right edge plus the lanes to its right, and a metre of footway. A
      // pedestrian head is on its own post.
      if (li !== undefined && world.lanes.pointCount[li] >= 2) {
        const ground = s.zM - (ped ? 2.6 : 5.0);
        const cs = Math.cos(yaw);
        const sn = Math.sin(yaw);
        if (ped) {
          const key = `${Math.round(s.xM * 2)},${Math.round(s.yM * 2)}`;
          if (!poleAt.has(key)) poleAt.set(key, { x: s.xM, y: s.yM, z0: ground, top: s.zM - PED_HOUSING_H / 2 });
        } else {
          const lanesRight = world.lanes.indexInEdge[li];
          const kerb = (lanesRight + 0.5) * world.lanes.widthM[li] + 1.2;
          // Right of the direction of travel is (sin, −cos).
          const px = s.xM + sn * kerb;
          const py = s.yM - cs * kerb;
          const key = `${Math.round(px * 2)},${Math.round(py * 2)}`;
          const armZ = s.zM + HOUSING_H / 2 + ARM_T / 2;
          const pole = poleAt.get(key);
          if (!pole) poleAt.set(key, { x: px, y: py, z0: ground, top: armZ + 0.4 });
          else pole.top = Math.max(pole.top, armZ + 0.4);
          arms.push({ x0: px, y0: py, x1: s.xM, y1: s.yM, z: armZ });
        }
      }
      const c = Math.cos(yaw);
      const sn = Math.sin(yaw);
      // Housing: local +x along the approach's travel direction, so its −x face looks back at the
      // drivers (or the pedestrians) who have to read it. A pedestrian head is the two-section
      // box, scaled from the vehicle housing's geometry.
      const hy = ped ? PED_HOUSING_W / HOUSING_W : 1;
      const hz = ped ? PED_HOUSING_H / HOUSING_H : 1;
      m.set(
        c, -sn * hy, 0, s.xM,
        sn, c * hy, 0, s.yM,
        0, 0, hz, s.zM,
        0, 0, 0, 1,
      );
      housing.setMatrixAt(i, m);
      for (let k = 0; k < 3; k++) {
        // A pedestrian head has two sections: the hand in the red lamp's slot, the walking
        // person in the green one's; the amber slot has no lens.
        const shape = this.#lampShape[i * 3 + k];
        const mesh = shape === NO_LENS ? null : lampMeshes[shape];
        if (!mesh) continue;
        const dz = ped ? (k === 0 ? 0.5 : -0.5) * PED_LAMP_SPACING : (1 - k) * LAMP_SPACING;
        const fx = -(HOUSING_D / 2 + 0.02);
        m.set(
          c, -sn, 0, s.xM + c * fx,
          sn, c, 0, s.yM + sn * fx,
          0, 0, 1, s.zM + dz,
          0, 0, 0, 1,
        );
        mesh.setMatrixAt(this.#lampSlot[i * 3 + k], m);
      }
      if (bar) {
        // Unit box scaled to the bar: depth along the lane, width across it.
        m.set(
          c * STOP_BAR_DEPTH, -sn * bar.width, 0, bar.x,
          sn * STOP_BAR_DEPTH, c * bar.width, 0, bar.y,
          0, 0, 1, bar.z,
          0, 0, 0, 1,
        );
        this.#hasBar[i] = 1;
      } else {
        m.makeScale(0, 0, 0);
      }
      bars.setMatrixAt(i, m);
    }
    // Poles and arms, instanced: a unit cylinder and a unit box, scaled and turned.
    const poleList = [...poleAt.values()];
    const poles = new InstancedMesh<BufferGeometry, Material>(poleGeom, this.#mountMaterial, Math.max(1, poleList.length));
    const armMesh = new InstancedMesh<BufferGeometry, Material>(armGeom, this.#mountMaterial, Math.max(1, arms.length));
    poles.name = "world/signal-poles";
    armMesh.name = "world/signal-arms";
    poleList.forEach((p, k) => {
      const h = Math.max(0.5, p.top - p.z0);
      m.set(
        POLE_R, 0, 0, p.x,
        0, POLE_R, 0, p.y,
        0, 0, h, p.z0,
        0, 0, 0, 1,
      );
      poles.setMatrixAt(k, m);
    });
    poles.count = poleList.length;
    arms.forEach((a, k) => {
      const dx = a.x1 - a.x0;
      const dy = a.y1 - a.y0;
      const len = Math.max(0.2, Math.hypot(dx, dy) + 0.2);
      const c = dx / Math.max(1e-6, len - 0.2);
      const sn = dy / Math.max(1e-6, len - 0.2);
      m.set(
        c * len, -sn * ARM_T, 0, (a.x0 + a.x1) / 2,
        sn * len, c * ARM_T, 0, (a.y0 + a.y1) / 2,
        0, 0, ARM_T, a.z,
        0, 0, 0, 1,
      );
      armMesh.setMatrixAt(k, m);
    });
    armMesh.count = arms.length;
    this.poleCount = poleList.length;
    for (const mesh of [housing, bars, poles, armMesh, ...lampMeshes]) {
      if (!mesh) continue;
      mesh.instanceMatrix.needsUpdate = true;
      mesh.computeBoundingSphere();
    }
    this.#housing = housing;
    this.#lampMeshes = lampMeshes;
    this.#bars = bars;
    this.#poles = poles;
    this.#arms = armMesh;
    this.group.add(housing, bars, poles, armMesh);
    for (const mesh of lampMeshes) if (mesh) this.group.add(mesh);
    for (let i = 0; i < n; i++) {
      const k = kept.get(i);
      if (k) {
        this.#phase[i] = k.phase;
        this.#ttc[i] = k.ttc;
      }
    }
    this.#repaint();
  }

  /**
   * A keyframe's signal block: the complete state (§3.3.3). Every head the block does not mention
   * goes back to "no data", so nothing survives from before a seek.
   */
  applyKeyframe(block: SignalBlock | null): void {
    this.#phase.fill(PHASE_NO_DATA);
    this.#ttc.fill(0xffff);
    if (block) this.#applyRows(block);
    this.#repaint();
  }

  /** A delta's signal block: only the rows that changed (§3.4.7). */
  applyDelta(block: SignalBlock | null): void {
    if (!block || block.count === 0) return;
    this.#applyRows(block);
    this.#repaint();
  }

  /** Forget every state (a new run). */
  resetStates(): void {
    this.#phase.fill(PHASE_NO_DATA);
    this.#ttc.fill(0xffff);
    this.#repaint();
  }

  #applyRows(block: SignalBlock): void {
    for (let r = 0; r < block.count; r++) {
      const heads = this.#heads.get(block.signalId[r]);
      if (!heads) continue;
      const phase = block.phase[r];
      const ttc = block.timeToChangeDs[r];
      for (const h of heads) {
        this.#phase[h] = phase;
        this.#ttc[h] = ttc;
      }
    }
  }

  /** The J2735 phase head `i` shows, or {@link PHASE_NO_DATA}; allocation-free, for per-frame checks. */
  phaseAt(i: number): number {
    return i >= 0 && i < this.#count ? this.#phase[i] : PHASE_NO_DATA;
  }

  /** True if head `i` is a pedestrian head (§4.5 `kind` 1). */
  isPedestrianHead(i: number): boolean {
    return i >= 0 && i < this.#count && this.#pedestrian[i] === 1;
  }

  /** What head `i` shows. */
  headState(i: number): SignalHeadState | null {
    if (i < 0 || i >= this.#count) return null;
    return {
      signalId: this.#headSignal[i],
      phase: this.#phase[i],
      aspect: this.#aspectAt(i),
      timeToChangeDs: this.#ttc[i],
    };
  }

  /** The colour actually written for head `i`'s lamp `k` (0 red, 1 amber, 2 green), for tests. */
  lampColor(i: number, k: number): [number, number, number] | null {
    if (i < 0 || i >= this.#count) return null;
    const shape = this.#lampShape[i * 3 + k];
    const mesh = shape === NO_LENS ? null : this.#lampMeshes[shape];
    if (!mesh?.instanceColor) return null;
    const a = mesh.instanceColor.array as Float32Array;
    const o = this.#lampSlot[i * 3 + k] * 3;
    return [a[o], a[o + 1], a[o + 2]];
  }

  /** The shape of head `i`'s lamp `k` ({@link LENS}), or null where it has none. */
  lensShape(i: number, k: number): number | null {
    if (i < 0 || i >= this.#count) return null;
    const shape = this.#lampShape[i * 3 + k];
    return shape === NO_LENS ? null : shape;
  }

  /** What head `i` shows: its phase's aspect, or the flashing yellow arrow on a turn face. */
  #aspectAt(i: number): SignalAspect {
    const phase = this.#phase[i];
    // J2735 5 = permissive-Movement-Allowed.
    if (this.#arrowFace[i] && phase === 5) return FLASHING_YELLOW_ARROW;
    return aspectOf(phase);
  }

  #setLamp(i: number, k: number, c: Color): void {
    const shape = this.#lampShape[i * 3 + k];
    const mesh = shape === NO_LENS ? null : this.#lampMeshes[shape];
    if (mesh) mesh.setColorAt(this.#lampSlot[i * 3 + k], c);
  }

  /**
   * How visible the coloured stop bars are: 1 from the air, where a lantern is too small to read
   * and the bar is how a state shows, fading to 0 at street level, where the lamps are read and
   * the painted white stop line is what is on the road. `distanceM` is the camera's distance to
   * what it looks at.
   */
  setBarVisibility(distanceM: number): void {
    const t = Math.min(1, Math.max(0, (distanceM - 60) / (160 - 60)));
    const v = t * t * (3 - 2 * t);
    if (Math.abs(v - this.#barVisibility) < 0.01) return;
    this.#barVisibility = v;
    this.#barMaterial.opacity = v;
    if (this.#bars) this.#bars.visible = v > 0.02 && this.#anyBar;
  }

  /** Advance flashing aspects. Cheap: returns at once unless a head is flashing. */
  update(timeSeconds: number): void {
    if (!this.#anyFlashing) return;
    const on = ((timeSeconds / FLASH_PERIOD_S) % 1 + 1) % 1 < 0.5;
    if (on === this.#flashOn) return;
    this.#flashOn = on;
    this.#repaint();
  }

  #repaint(): void {
    const bars = this.#bars;
    if (!bars) return;
    const c = this.#scratch;
    const off = this.#theme.signalDark;
    let flashing = false;
    for (let i = 0; i < this.#count; i++) {
      const aspect = this.#aspectAt(i);
      if (this.#pedestrian[i]) {
        // Walk (movement allowed) lights the walking person; clearance flashes the hand; stop
        // shows it steady (MUTCD §4E.02). The hand is the amber theme colour, the nearest the
        // palette has to Portland orange.
        const clearing = (aspect.lamps & AMBER) !== 0;
        if (clearing) flashing = true;
        const hand = (aspect.lamps & (RED | AMBER)) !== 0 && !(clearing && !this.#flashOn);
        c.copy(this.#colors.amber);
        if (!hand) c.multiplyScalar(UNLIT);
        this.#setLamp(i, 0, c);
        c.setHex(PED_WALK_COLOR);
        if (!(aspect.lamps & GREEN)) c.multiplyScalar(UNLIT);
        this.#setLamp(i, 2, c);
        c.setHex(off);
        bars.setColorAt(i, c);
        continue;
      }
      if (aspect.flashing) flashing = true;
      const lit = aspect.flashing && !this.#flashOn ? 0 : aspect.lamps;
      const lampColors = [this.#colors.red, this.#colors.amber, this.#colors.green];
      for (let k = 0; k < 3; k++) {
        const bit = k === 0 ? RED : k === 1 ? AMBER : GREEN;
        c.copy(lampColors[k]);
        if (!(lit & bit)) c.multiplyScalar(UNLIT);
        this.#setLamp(i, k, c);
      }
      // The bar says the same thing on the road. "No data" and "dark" draw no bar at all: a grey
      // bar would read as a state.
      if (aspect.lamps === 0 || !this.#hasBar[i]) {
        c.setHex(off);
      } else if (aspect.lamps & RED) {
        c.copy(this.#colors.red);
      } else if (aspect.lamps & AMBER) {
        c.copy(this.#colors.amber);
      } else {
        c.copy(this.#colors.green);
      }
      if (aspect.flashing && !this.#flashOn) c.multiplyScalar(0.35);
      bars.setColorAt(i, c);
    }
    this.#anyFlashing = flashing;
    for (const mesh of this.#lampMeshes) if (mesh?.instanceColor) mesh.instanceColor.needsUpdate = true;
    if (bars.instanceColor) bars.instanceColor.needsUpdate = true;
    // Bars with no state are hidden rather than painted grey.
    let anyBar = false;
    for (let i = 0; i < this.#count; i++) if (this.#hasBar[i] && this.#aspectAt(i).lamps !== 0) anyBar = true;
    this.#anyBar = anyBar;
    bars.visible = anyBar && this.#barVisibility > 0.02;
  }

  clear(): void {
    for (const mesh of [this.#housing, this.#bars, this.#poles, this.#arms, ...this.#lampMeshes]) {
      if (!mesh) continue;
      this.group.remove(mesh);
      mesh.dispose();
    }
    for (const g of this.#geometries) g.dispose();
    this.#geometries = [];
    this.#housing = null;
    this.#lampMeshes = [];
    this.#bars = null;
    this.#poles = null;
    this.#arms = null;
  }

  dispose(): void {
    this.clear();
    this.#count = 0;
    this.#heads.clear();
    this.#builtFrom = null;
    this.#housingMaterial.dispose();
    this.#lampMaterial.dispose();
    this.#barMaterial.dispose();
    this.#mountMaterial.dispose();
  }
}

/**
 * The housing: the box, and above each section a tunnel visor — a U-shaped hood of three thin
 * slabs projecting 0.22 m towards the approach — so a lamp is lit inside a hood, as a real one is.
 */
function housingGeometry(): BufferGeometry {
  const b = new MeshBuilder({ vertexCapacity: 256, indexCapacity: 512 });
  addBox(b, 0, 0, 0, HOUSING_D, HOUSING_W, HOUSING_H, 0);
  const visor = 0.22;
  const r = LENS_D / 2 + 0.03;
  for (let k = 0; k < 3; k++) {
    const z = (1 - k) * LAMP_SPACING;
    const x = -(HOUSING_D / 2 + visor / 2);
    addBox(b, x, 0, z + r, visor, 2 * r, 0.02, 0);
    addBox(b, x, r, z + r * 0.3, visor, 0.02, r * 1.4, 0);
    addBox(b, x, -r, z + r * 0.3, visor, 0.02, r * 1.4, 0);
  }
  const g = b.toGeometry();
  if (!g) throw new Error("signal housing geometry is empty");
  return g;
}

/** A round lens of radius `r` in the y–z plane, facing −x. */
function lensGeometry(r: number, segments: number): BufferGeometry {
  const b = new MeshBuilder({ vertexCapacity: segments + 2, indexCapacity: segments * 3 });
  const c = b.addVertex(0, 0, 0, -1, 0, 0);
  for (let i = 0; i < segments; i++) {
    const a = (i / segments) * Math.PI * 2;
    b.addVertex(0, Math.cos(a) * r, Math.sin(a) * r, -1, 0, 0);
  }
  for (let i = 0; i < segments; i++) {
    // Counter-clockwise seen from −x.
    b.addTriangle(c, 1 + ((i + 1) % segments), 1 + i);
  }
  const g = b.toGeometry();
  if (!g) throw new Error("lens geometry is empty");
  return g;
}

/** A unit cylinder standing on the origin: radius 1, height 1, along +z. */
function cylinderGeometry(): BufferGeometry {
  const b = new MeshBuilder({ vertexCapacity: 64, indexCapacity: 128 });
  addCylinder(b, 0, 0, 0, 1, 1, 8);
  const g = b.toGeometry();
  if (!g) throw new Error("pole geometry is empty");
  return g;
}

function boxGeometry(sx: number, sy: number, sz: number): BufferGeometry {
  const b = new MeshBuilder({ vertexCapacity: 32, indexCapacity: 64 });
  addBox(b, 0, 0, 0, sx, sy, sz, 0);
  const g = b.toGeometry();
  if (!g) throw new Error("signal geometry is empty");
  return g;
}

const LENS_NAMES = ["ball", "left-arrow", "right-arrow", "hand", "walker"] as const;

/**
 * Each lamp's lens shape. A pedestrian head: the hand, no middle lens, the walking person. A vehicle
 * head is a separate turn face (arrows) when its lane only turns one way and its signal group
 * controls no through lane of the same approach (same controller, same edge); otherwise balls.
 */
function lensShapes(world: VwpWorld, laneIndex: Map<number, number>): { lamp: Uint8Array; arrowFace: Uint8Array } {
  const n = world.signals.count;
  const lamp = new Uint8Array(n * 3).fill(LENS.BALL);
  const arrowFace = new Uint8Array(n);
  const { moves } = laneMovements(world);
  const key = (signal: number, li: number): string => `${signal}:${world.lanes.edgeId[li]}`;
  // The groups that control a through movement, per controller and approach edge.
  const through = new Map<string, Set<number>>();
  for (let i = 0; i < n; i++) {
    const s = world.signals.at(i);
    if (s.kind === 1) continue;
    const li = laneIndex.get(s.laneId);
    if (li === undefined || !(moves[li] & MOVE.STRAIGHT)) continue;
    const k = key(s.signalId, li);
    let set = through.get(k);
    if (!set) {
      set = new Set();
      through.set(k, set);
    }
    set.add(s.group);
  }
  for (let i = 0; i < n; i++) {
    const s = world.signals.at(i);
    if (s.kind === 1) {
      lamp[i * 3] = LENS.HAND;
      lamp[i * 3 + 1] = NO_LENS;
      lamp[i * 3 + 2] = LENS.WALKER;
      continue;
    }
    const li = laneIndex.get(s.laneId);
    if (li === undefined) continue;
    const m = moves[li];
    const left = (m & MOVE.LEFT) !== 0 && (m & ~(MOVE.LEFT | MOVE.UTURN)) === 0;
    const right = m === MOVE.RIGHT;
    if (!left && !right) continue;
    const groups = through.get(key(s.signalId, li));
    if (!groups || groups.has(s.group)) continue;
    arrowFace[i] = 1;
    lamp.fill(left ? LENS.LEFT_ARROW : LENS.RIGHT_ARROW, i * 3, i * 3 + 3);
  }
  return { lamp, arrowFace };
}

/**
 * A flat convex polygon on a lens face (the y–z plane, facing −x), as a fan from its first point.
 * A viewer at −x looking along +x with z up has +y on their left, so a polygon counter-clockwise
 * in (y, z) is clockwise to them: such a polygon is reversed before it is added.
 */
function addFacePolygon(b: MeshBuilder, pts: readonly (readonly [number, number])[]): void {
  let area = 0;
  for (let i = 0; i < pts.length; i++) {
    const [y0, z0] = pts[i];
    const [y1, z1] = pts[(i + 1) % pts.length];
    area += y0 * z1 - y1 * z0;
  }
  const ordered = area > 0 ? [...pts].reverse() : pts;
  const base = ordered.map(([y, z]) => b.addVertex(0, y, z, -1, 0, 0));
  for (let i = 1; i + 1 < base.length; i++) b.addTriangle(base[0], base[i], base[i + 1]);
}

/** A bar on the lens face from `(y0, z0)` to `(y1, z1)`, `w` wide. */
function addFaceBar(b: MeshBuilder, y0: number, z0: number, y1: number, z1: number, w: number): void {
  const dy = y1 - y0;
  const dz = z1 - z0;
  const len = Math.hypot(dy, dz) || 1;
  const py = (-dz / len) * (w / 2);
  const pz = (dy / len) * (w / 2);
  addFacePolygon(b, [[y0 + py, z0 + pz], [y1 + py, z1 + pz], [y1 - py, z1 - pz], [y0 - py, z0 - pz]]);
}

function lensFromBuilder(b: MeshBuilder, what: string): BufferGeometry {
  const g = b.toGeometry();
  if (!g) throw new Error(`${what} lens geometry is empty`);
  return g;
}

/**
 * An arrow lens (MUTCD 2009 §4D.06): only the arrow is lit, a shaft and a head filling most of a
 * 12-inch lens. `side` +1 points to the driver's left (local +y: the face looks back along −x at a
 * driver travelling +x, whose left is +y), −1 to their right.
 */
function arrowLensGeometry(side: 1 | -1): BufferGeometry {
  const b = new MeshBuilder({ vertexCapacity: 16, indexCapacity: 24 });
  const r = LENS_D / 2;
  addFaceBar(b, -side * r * 0.75, 0, side * r * 0.1, 0, r * 0.38);
  addFacePolygon(b, [[side * r * 0.05, r * 0.55], [side * r * 0.8, 0], [side * r * 0.05, -r * 0.55]]);
  return lensFromBuilder(b, "arrow");
}

/** The UPRAISED HAND (MUTCD 2009 §4E.04): a palm, four fingers and a thumb, about 0.24 m tall. */
function handLensGeometry(): BufferGeometry {
  const b = new MeshBuilder({ vertexCapacity: 32, indexCapacity: 48 });
  addFacePolygon(b, [[-0.06, -0.12], [0.06, -0.12], [0.065, 0.0], [-0.065, 0.0]]);
  for (let f = 0; f < 4; f++) {
    const y = -0.048 + f * 0.032;
    const top = f === 1 || f === 2 ? 0.11 : 0.085;
    addFaceBar(b, y, -0.01, y, top, 0.026);
  }
  // The thumb, out to the side and up.
  addFaceBar(b, 0.055, -0.07, 0.1, -0.005, 0.028);
  return lensFromBuilder(b, "hand");
}

/** The WALKING PERSON (MUTCD 2009 §4E.04): head, body, arms and legs mid-stride, about 0.26 m tall. */
function walkerLensGeometry(): BufferGeometry {
  const b = new MeshBuilder({ vertexCapacity: 48, indexCapacity: 72 });
  const head: [number, number][] = [];
  for (let i = 0; i < 8; i++) {
    const a = (i / 8) * Math.PI * 2;
    head.push([0.012 + Math.cos(a) * 0.026, 0.1 + Math.sin(a) * 0.026]);
  }
  addFacePolygon(b, head);
  addFaceBar(b, 0.008, 0.065, -0.004, -0.02, 0.04); // body
  addFaceBar(b, 0.004, 0.05, 0.05, 0.0, 0.022); // forward arm
  addFaceBar(b, 0.0, 0.05, -0.045, 0.01, 0.022); // back arm
  addFaceBar(b, -0.004, -0.015, 0.045, -0.12, 0.026); // forward leg
  addFaceBar(b, -0.004, -0.015, -0.05, -0.12, 0.026); // back leg
  return lensFromBuilder(b, "walker");
}
