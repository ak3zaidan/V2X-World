/**
 * Camera modes and the controller that animates between them.
 *
 * 09-ui §3 is explicit: "a single `PerspectiveCamera` is used for both views … the transition is an
 * interpolation of (position, target, field of view) with smooth damping, following jevpilot's
 * exponential lerp for camera position and look target (`1 − exp(−4·dt)` and `1 − exp(−6·dt)`)". Those
 * two constants are {@link CameraControllerOptions.positionLambda} and `lookLambda`, and they are why
 * {@link CameraController.flyTo} needs no tweening machinery at all: setting the mode changes the
 * *desired* pose, and the same exponential smoothing that keeps the chase camera behind a car also
 * carries the camera down from 600 m to 9 m behind it, continuously, in about a second. There is no
 * second scene, no reload, and no cut.
 *
 * Top-down is *nearly* straight down rather than exactly: `Object3D.lookAt` is degenerate when the
 * view direction is parallel to `up`, which for a z-up world is precisely the 90° pitch 09-ui asks
 * for. The controller keeps a horizontal offset of `altitude · 1e-4` (10 cm at a 1 km altitude), so
 * the basis is well defined and the picture is indistinguishable from a true plan view.
 */

import { MathUtils, Vector3 } from "three";
import type { PerspectiveCamera } from "three";
import type { CameraMode } from "@vwp/protocol";
import type { WorldRenderer } from "./world-render.js";

export type { CameraMode };

/** The five persistent modes; `jump` is an action, handled as "snap, then chase". */
export const CAMERA_MODES: readonly CameraMode[] = ["map", "chase", "dashboard", "free", "rsu"];

/** Tuning for {@link CameraController}. */
export interface CameraControllerOptions {
  readonly camera: PerspectiveCamera;
  /** Supplies building footprints for {@link CameraController.keepCameraOutsideBuildings}. */
  readonly world?: WorldRenderer | null;
  /** Position smoothing rate; `1 − exp(−λ·dt)`. Default 4 (09-ui §3). */
  readonly positionLambda?: number;
  /** Look-target smoothing rate. Default 6 (09-ui §3). */
  readonly lookLambda?: number;
  /** Field-of-view smoothing rate. Default 5. */
  readonly fovLambda?: number;
  /** Chase distance behind the followed actor, metres. Default 9. */
  readonly chaseDistanceM?: number;
  /** Chase elevation angle, degrees. Default 16. */
  readonly chasePitchDeg?: number;
  /** Height of the chase look target above the actor's origin, metres. Default 1.4. */
  readonly chaseLookHeightM?: number;
  /** Driver's eye height in dashboard mode, metres. Default 1.25. */
  readonly dashboardEyeHeightM?: number;
  /** Starting map altitude, metres. Default 700. */
  readonly mapAltitudeM?: number;
  readonly minAltitudeM?: number;
  readonly maxAltitudeM?: number;
  /** Clearance kept above a roof the camera would otherwise be inside, metres. Default 3. */
  readonly buildingClearanceM?: number;
  /**
   * Lowest the camera may be above the ground it is looking at, metres. Default 0.6.
   *
   * Dragging the chase camera's pitch below the horizon used to put it under the road — the
   * orbit allowed −20° at up to 400 m — and a camera under a double-sided ground plane sees
   * nothing but the plane's underside: the black frame the owner reported.
   */
  readonly groundClearanceM?: number;
  /** Chase-yaw smoothing rate, `1 − exp(−λ·dt)`. Default 4. */
  readonly yawLambda?: number;
  /**
   * Longest camera-to-target distance at which the occlusion march runs, metres. Default 60 — a
   * chase or dashboard working distance. Beyond it only the roof lift applies; see
   * {@link CameraController.keepCameraOutsideBuildings}.
   */
  readonly occlusionRangeM?: number;
  /**
   * Called when the user presses `f` over the element {@link CameraController.attachInput} is bound
   * to: "frame what is live". The controller has no idea where the actors are, so the owner — the
   * {@link WorldRenderer}'s viewer — supplies the action.
   */
  readonly onFrameActors?: () => void;
  /**
   * Longest a mode change's camera flight may take, seconds. Default 2.4.
   *
   * A mode change flies the camera along a duration-bounded eased path rather than leaving it to
   * the exponential tracking law, which travels any distance in the same 0.58 s. See
   * `CameraController#beginFlight`.
   */
  readonly flyMaxSeconds?: number;
  /** Field of view per mode, degrees. */
  readonly fovMapDeg?: number;
  readonly fovChaseDeg?: number;
  readonly fovDashboardDeg?: number;
}

/** A serialisable camera state, shaped for §6.7 `view.camera`'s result. */
export interface CameraState {
  readonly mode: CameraMode;
  readonly position: { x: number; y: number; z: number };
  readonly target: { x: number; y: number; z: number };
  readonly fovDeg: number;
  readonly followActorId: number | null;
  readonly altitudeM: number;
  readonly distanceM: number;
  readonly bearingRad: number;
}

/** What {@link CameraController.attachInput} binds to; `HTMLElement`, `Window` and `Document` all fit. */
export type InputTarget = Pick<EventTarget, "addEventListener" | "removeEventListener">;

const DEG = Math.PI / 180;

/** Default chase elevation, degrees: a camera a little above the roof, looking down the street. */
const CHASE_PITCH_DEG = 13;

/**
 * Natural frequency of the chase yaw's critically damped spring, rad/s. A critically damped
 * second-order follow settles without overshoot in about `4.7 / ω` (to 1 %): 1.3 s here, so the
 * camera swings round a corner with the car, a beat behind, and never oscillates.
 */
const CHASE_YAW_OMEGA = 3.6;

/** The chase distance for a car, metres, that `distanceM` (the zoom) is relative to. */
const CHASE_REFERENCE_M = 9;


/**
 * One exact step of a critically damped spring: error `e` and rate `v` after `dt` towards zero,
 * `x(t) = (e + (v + ωe)t)·e^(−ωt)`. Exact, so it is frame-rate independent.
 */
function dampedStep(e: number, v: number, omega: number, dt: number): [number, number] {
  const k = Math.exp(-omega * dt);
  const a = v + omega * e;
  return [(e + a * dt) * k, (v - omega * a * dt) * k];
}

/** Zero velocity and zero acceleration at both ends of [0, 1]. */
function smootherstep(u: number): number {
  return u * u * u * (u * (u * 6 - 15) + 10);
}

/**
 * Move `x` (with velocity `v`) towards `target` for `dt` seconds as a critically damped spring of
 * natural frequency `omega`, exactly: `e(t) = (e₀ + (v₀ + ωe₀)t)·e^(−ωt)` per axis.
 */
function springTo(x: Vector3, v: Vector3, target: Vector3, omega: number, dt: number): void {
  if (!(dt > 0)) return;
  const k = Math.exp(-omega * dt);
  for (const axis of ["x", "y", "z"] as const) {
    const e = x[axis] - target[axis];
    const a = v[axis] + omega * e;
    x[axis] = target[axis] + (e + a * dt) * k;
    v[axis] = (v[axis] - omega * a * dt) * k;
  }
}

/**
 * {@link springTo} towards a target moving at `(tvx, tvy, 0)`: the spring acts on the error and
 * the relative velocity, so a target moving steadily is followed with no lag. `target` is where
 * the target is at the end of the step; it was `tv·dt` behind at its start.
 */
function springFollow(x: Vector3, v: Vector3, target: Vector3, tvx: number, tvy: number, omega: number, dt: number): void {
  if (!(dt > 0)) return;
  const k = Math.exp(-omega * dt);
  const tv = [tvx, tvy, 0];
  const axes = ["x", "y", "z"] as const;
  for (let n = 0; n < 3; n++) {
    const axis = axes[n];
    const start = target[axis] - tv[n] * dt;
    const e = x[axis] - start;
    const u = v[axis] - tv[n];
    const a = u + omega * e;
    x[axis] = target[axis] + (e + a * dt) * k;
    v[axis] = tv[n] + (u - omega * a * dt) * k;
  }
}

function finite3(v: Vector3): boolean {
  return Number.isFinite(v.x) && Number.isFinite(v.y) && Number.isFinite(v.z);
}

/**
 * Keep `v` such that `[v − half, v + half]` stays inside `[lo, hi]`; centre it when it cannot fit.
 */
function clampSpan(v: number, lo: number, hi: number, half: number): number {
  if (hi - lo <= 2 * half) return (lo + hi) / 2;
  return MathUtils.clamp(v, lo + half, hi - half);
}

/**
 * Drives one `PerspectiveCamera` through every viewer mode.
 *
 * The owner calls {@link setFollowPose} each frame with the interpolated pose of the followed actor
 * (the controller never reaches into the pose buffer itself, so replay, live and test all look the
 * same to it), then {@link update} with the frame's `dt`.
 */
export class CameraController {
  readonly camera: PerspectiveCamera;

  #world: WorldRenderer | null;
  #mode: CameraMode = "map";
  #followActorId: number | null = null;
  #followValid = false;
  /**
   * Whether {@link setFollowPose} has ever supplied a pose for the *current* followed actor.
   *
   * Distinct from `#followValid`, which is false the moment the actor stops streaming. The chase
   * and dashboard modes need a *place to be*, and the last known pose of a stopped vehicle is a
   * perfectly good one; the map focus is not (finding: chase fell back to the map focus, which at
   * start-up is the world's centre, and parked the camera in a random city block a kilometre from
   * the only vehicle).
   */
  #followPosKnown = false;
  #rsuIndex = -1;
  /** A street-level mode that was asked for with no vehicle to sit behind, or null. */
  #rejectedMode: CameraMode | null = null;
  /** Set by the pointer and wheel handlers, so automatic framing never overrides a user gesture. */
  #userMoved = false;

  /** The point the camera orbits or looks at, in ENU metres. */
  readonly target = new Vector3();
  /** The smoothed look target actually handed to `lookAt`. */
  readonly look = new Vector3();

  #desiredPosition = new Vector3();
  #desiredLook = new Vector3();
  #followPos = new Vector3();
  #followHeading = 0;
  #followSpeed = 0;

  #altitude: number;
  #minAltitude: number;
  #maxAltitude: number;
  #bearing = Math.PI / 2; // looking north
  #orbitYaw = 0;
  #orbitPitch: number;
  #chaseDistance: number;
  #chaseLookHeight: number;
  #eyeHeight: number;
  #clearance: number;
  #groundClearance: number;
  #occlusionRange: number;
  /** The heading the chase camera sits behind, smoothed; see `update`. NaN until seeded. */
  #chaseYaw = Number.NaN;
  /** Its rate, rad/s: the yaw follows the heading as a critically damped spring. */
  #chaseYawRate = 0;
  /**
   * The followed subject's body, metres, and whether it is a person or a rider: what the chase and
   * dashboard framing is sized from ({@link setFollowSubject}). A car until told otherwise.
   */
  #subjectL = 4.5;
  #subjectW = 1.8;
  #subjectH = 1.5;
  #subjectVru = false;
  /**
   * How far the chase camera may stand from its look point, after walls: follows the occlusion
   * march's answer quickly inwards and slowly back out, so a lamp post of a façade corner passing
   * behind the car is a glide, not a cut.
   */
  #clearDist = Number.POSITIVE_INFINITY;
  /** Set by {@link setFollowPose} when the subject's pose jumped this frame. */
  #subjectJumped = false;
  readonly yawLambda: number;
  /** Where the visible part of the viewport sits inside the canvas; see {@link setViewInsets}. */
  #insets = { top: 0, right: 0, bottom: 0, left: 0 };
  /** Frames on which the camera had to be recovered from a non-finite state (diagnostic). */
  recoveries = 0;
  #freeVelocity = new Vector3();
  /** The spring's velocities for the camera position and its look point, m/s. */
  #posVel = new Vector3();
  #lookVel = new Vector3();
  #freeYaw = 0;
  #freePitch = -0.3;

  readonly positionLambda: number;
  readonly lookLambda: number;
  readonly fovLambda: number;
  #flyMaxSeconds: number;
  /** Seconds elapsed in the current flight, and its total; equal means "not flying". */
  #flyT = 0;
  #flyDuration = 0;
  #flyStart = new Vector3();
  /** Cruise height of a flight to street level, or NaN for a straight flight; see `#beginFlight`. */
  #flyCruise = Number.NaN;
  /**
   * Set when the followed subject changes in a street-level mode: the next update flies to the new
   * subject instead of sliding there at street level through every building in between.
   */
  #pendingFlight = false;
  #flyStartLook = new Vector3();
  #fovMap: number;
  #fovChase: number;
  #fovDashboard: number;
  #desiredFov: number;

  #onFrameActors: (() => void) | null;
  #viewportW = 1280;
  #viewportH = 720;
  #detach: (() => void) | null = null;
  #scratch = new Vector3();
  #scratchB = new Vector3();

  constructor(options: CameraControllerOptions) {
    this.camera = options.camera;
    this.camera.up.set(0, 0, 1);
    this.#world = options.world ?? null;
    this.positionLambda = options.positionLambda ?? 4;
    this.lookLambda = options.lookLambda ?? 6;
    this.fovLambda = options.fovLambda ?? 5;
    this.#chaseDistance = options.chaseDistanceM ?? 9;
    this.#orbitPitch = (options.chasePitchDeg ?? CHASE_PITCH_DEG) * DEG;
    this.#chaseLookHeight = options.chaseLookHeightM ?? 1.4;
    this.#eyeHeight = options.dashboardEyeHeightM ?? 1.25;
    this.#altitude = options.mapAltitudeM ?? 700;
    this.#minAltitude = options.minAltitudeM ?? 12;
    this.#maxAltitude = options.maxAltitudeM ?? 20_000;
    this.#clearance = options.buildingClearanceM ?? 3;
    this.#groundClearance = Math.max(0.1, options.groundClearanceM ?? 0.6);
    this.yawLambda = options.yawLambda ?? 4;
    this.#occlusionRange = options.occlusionRangeM ?? 60;
    this.#flyMaxSeconds = Math.max(0.05, options.flyMaxSeconds ?? 2.4);
    this.#fovMap = options.fovMapDeg ?? 45;
    this.#fovChase = options.fovChaseDeg ?? 55;
    this.#fovDashboard = options.fovDashboardDeg ?? 68;
    this.#onFrameActors = options.onFrameActors ?? null;
    this.#desiredFov = this.#fovMap;
    this.camera.fov = this.#fovMap;
    this.camera.updateProjectionMatrix();
    this.#computeDesired();
    this.snap();
  }

  /** Current mode. */
  get mode(): CameraMode {
    return this.#mode;
  }

  /** Actor the camera follows, or null. */
  get followActorId(): number | null {
    return this.#followActorId;
  }

  /**
   * Whether the camera has a vehicle to sit behind — a followed actor with at least one known
   * pose, live or last-seen. The street-level modes are meaningless without one.
   */
  get hasFollowSubject(): boolean {
    return this.#followActorId !== null && this.#followPosKnown;
  }

  /** Which modes are only meaningful with a followed vehicle. */
  static needsFollowSubject(mode: CameraMode): boolean {
    return mode === "chase" || mode === "dashboard";
  }

  /**
   * The street-level mode most recently asked for and refused for want of a vehicle, or null.
   *
   * {@link setMode} returns the mode it actually applied; this says which one it would not give.
   * The caller owns the sentence shown to the user — the controller only refuses to draw a frame
   * that means nothing.
   */
  get rejectedMode(): CameraMode | null {
    return this.#rejectedMode;
  }

  /** Whether a mode-change flight is in progress. A snap during one is a visible cut. */
  get isFlying(): boolean {
    return this.#flyT < this.#flyDuration;
  }

  /** Whether a flight is in progress or will start on the next update (a change of subject). */
  get inTransit(): boolean {
    return this.#pendingFlight || this.#flyT < this.#flyDuration;
  }

  /** Whether the user has panned, orbited or zoomed. Automatic framing must not override them. */
  get userHasMoved(): boolean {
    return this.#userMoved;
  }

  /** Map altitude in metres. */
  get altitudeM(): number {
    return this.#altitude;
  }

  set altitudeM(v: number) {
    if (!Number.isFinite(v)) return;
    this.#altitude = MathUtils.clamp(v, this.#minAltitude, this.#altitudeCeiling());
  }

  /**
   * The highest useful map altitude: the one at which the plan view covers the world two and a half
   * times over, or the configured maximum if that is lower. Zooming out to the 20 km maximum on a
   * 2 km world left a 5 % speck in a frame of flat background — the camera fuzz's "flat frame".
   */
  #altitudeCeiling(): number {
    const b = this.#world?.world?.bbox;
    if (!b) return this.#maxAltitude;
    const span = Math.max(b.maxXM - b.minXM, b.maxYM - b.minYM, 100);
    const half = Math.tan((this.camera.fov * DEG) / 2);
    return Math.max(this.#minAltitude + 1, Math.min(this.#maxAltitude, (span * 2.5) / 2 / Math.max(0.05, half)));
  }

  /** Chase/orbit distance in metres. */
  get distanceM(): number {
    return this.#chaseDistance;
  }

  set distanceM(v: number) {
    this.#chaseDistance = MathUtils.clamp(v, 2, 400);
  }

  /** Map bearing in radians; `π/2` is north-up. */
  get bearingRad(): number {
    return this.#bearing;
  }

  set bearingRad(v: number) {
    this.#bearing = v;
  }

  /** Attach a world so the camera can avoid building interiors. */
  setWorld(world: WorldRenderer | null): void {
    this.#world = world;
  }

  /** Tell the controller the drawing-buffer size, for pan scaling and the projection aspect. */
  setViewportSize(width: number, height: number): void {
    this.#viewportW = Math.max(1, width);
    this.#viewportH = Math.max(1, height);
    this.#applyProjectionWindow();
  }

  /**
   * Keep the part of the viewport under an interface panel out of the framing.
   *
   * The point the camera looks at is put in the middle of the *unobstructed* rectangle by shifting
   * the lens, and everything else about the projection — field of view, scale — is left alone, so
   * the map does not zoom when a panel opens. A chase camera puts its vehicle a little below the
   * centre of the frame, which is exactly where the floating OBU HUD sits; with the HUD's height as
   * the bottom inset the vehicle is a little below the centre of what the user can see instead.
   * CSS pixels, clamped so at least a quarter of the viewport stays framed.
   */
  setViewInsets(insets: { top?: number; right?: number; bottom?: number; left?: number }): void {
    const w = this.#viewportW;
    const h = this.#viewportH;
    const clamp = (v: number | undefined, max: number): number =>
      Math.max(0, Math.min(max, v !== undefined && Number.isFinite(v) ? v : 0));
    const top = clamp(insets.top, h * 0.75);
    const bottom = clamp(insets.bottom, h * 0.75 - top);
    const left = clamp(insets.left, w * 0.75);
    const right = clamp(insets.right, w * 0.75 - left);
    const i = this.#insets;
    if (i.top === top && i.bottom === bottom && i.left === left && i.right === right) return;
    this.#insets = { top, right, bottom, left };
    this.#applyProjectionWindow();
  }

  /** The insets currently applied, CSS pixels. */
  get viewInsets(): { readonly top: number; readonly right: number; readonly bottom: number; readonly left: number } {
    return this.#insets;
  }

  #applyProjectionWindow(): void {
    const { top, right, bottom, left } = this.#insets;
    const w = this.#viewportW;
    const h = this.#viewportH;
    this.camera.aspect = w / h;
    if (top === 0 && right === 0 && bottom === 0 && left === 0) {
      this.camera.clearViewOffset();
    } else {
      // A lens shift: the projection keeps its scale — the map's zoom, the marks' pixel sizes — and
      // its principal point moves to the middle of the unobstructed rectangle.
      this.camera.setViewOffset(w, h, (right - left) / 2, (bottom - top) / 2, w, h);
    }
    this.camera.updateProjectionMatrix();
  }

  /**
   * Switch mode. The camera animates there; pass `instant` to cut.
   *
   * `"jump"` is treated as "snap to the target, then chase", matching 09-ui §3's description of it as
   * an action rather than a persistent mode.
   */
  setMode(mode: CameraMode, instant = false): CameraMode {
    if (mode === "jump") {
      if (!this.hasFollowSubject) {
        this.#rejectedMode = "chase";
        return this.#mode;
      }
      this.#mode = "chase";
      this.#computeDesired();
      this.snap();
      return this.#mode;
    }
    // A street-level mode with nothing to sit behind used to fall back to the map focus, which at
    // start-up is the *world's centre*. On phase1-manhattan that put the chase camera 1,050 m from
    // the only vehicle, at 20 m up inside a city block, looking at a point 8 m away: two dark
    // planes, one lane marking, no car. That frame is not a chase view of anything, so it is not
    // drawn. The caller is told which mode was refused and decides what to say.
    if (CameraController.needsFollowSubject(mode) && !this.hasFollowSubject) {
      this.#rejectedMode = mode;
      return this.#mode;
    }
    // `rsu` is the same shape of claim about a mast that may not exist.
    if (mode === "rsu" && (this.#world === null || this.#world.siteCount === 0)) {
      this.#rejectedMode = mode;
      return this.#mode;
    }
    this.#rejectedMode = null;
    const changed = mode !== this.#mode;
    this.#mode = mode;
    this.#desiredFov = mode === "dashboard" ? this.#fovDashboard : mode === "map" ? this.#fovMap : this.#fovChase;
    if (mode === "free") {
      // Seed the free camera from wherever the camera currently is, so "free" never jumps.
      const dir = this.#scratch.copy(this.look).sub(this.camera.position);
      if (dir.lengthSq() > 1e-6) {
        dir.normalize();
        this.#freeYaw = Math.atan2(dir.y, dir.x);
        this.#freePitch = Math.asin(MathUtils.clamp(dir.z, -1, 1));
      }
      this.#desiredPosition.copy(this.camera.position);
      this.#freeVelocity.set(0, 0, 0);
    }
    this.#computeDesired();
    if (instant) this.snap();
    else if (changed) this.#beginFlight();
    return this.#mode;
  }

  /** Follow an actor (or nothing). Does not change the mode. */
  follow(actorId: number | null): void {
    // A different subject invalidates the remembered pose; re-following the same one keeps it, so
    // re-selecting a parked vehicle does not throw away the only place the camera can stand.
    if (actorId !== this.#followActorId) {
      this.#followPosKnown = false;
      this.#chaseYaw = Number.NaN;
      this.#chaseYawRate = 0;
      this.#clearDist = Number.POSITIVE_INFINITY;
      // Switching subject in a street mode is a journey across the city. The tracking law would
      // slide the camera there at street level — measured by the camera fuzz: lifted onto a 138 m
      // roof on the way, the frame one flat grey — so it is flown instead, over the roofs.
      this.#pendingFlight = actorId !== null && CameraController.needsFollowSubject(this.#mode);
    }
    this.#followActorId = actorId;
    this.#followValid = false;
  }

  /**
   * The signature interaction of 09-ui §1.2: from the top-down map, click a vehicle and the camera
   * flies down into a chase view of it. One call, no scene switch — the mode change re-aims the
   * desired pose and the exponential smoothing in {@link update} does the rest.
   */
  flyTo(actorId: number, mode: CameraMode = "chase", instant = false): CameraMode {
    this.follow(actorId);
    // `follow` cleared the remembered pose for a new subject, so `setMode` would refuse a
    // street-level mode on the very frame the user clicked a vehicle. Seeding the pose here is
    // what {@link setFollowPose} would do a few milliseconds later anyway; the owner supplies the
    // real one on the next frame.
    if (CameraController.needsFollowSubject(mode) && !this.#followPosKnown) {
      this.#followPos.copy(this.target);
      this.#followPosKnown = true;
      this.#followValid = false;
    }
    return this.setMode(mode, instant);
  }

  /** Watch from an RSU mast. `siteIndex` indexes {@link WorldRenderer.sitePositions}. */
  viewFromSite(siteIndex: number, instant = false): void {
    this.#rsuIndex = siteIndex;
    this.setMode("rsu", instant);
  }

  /** Move the map focus. */
  focusOn(x: number, y: number, z = 0): void {
    this.target.set(x, y, z);
  }

  /** Set the map altitude so the vertical field of view covers `extentM` metres. */
  fitExtent(extentM: number): void {
    const half = Math.max(1, extentM) / 2;
    this.altitudeM = half / Math.tan((this.camera.fov * DEG) / 2);
  }

  /** The extent in metres the map view currently covers vertically. */
  get extentM(): number {
    return 2 * this.#altitude * Math.tan((this.camera.fov * DEG) / 2);
  }

  /**
   * Tell the controller how big the followed subject is: the chase camera stands off and looks at
   * it in proportion (a bus from further back and higher up than a car, a person from closer and
   * lower), and the dashboard camera sits at its driver's eye. `vru` is a person or a rider.
   */
  setFollowSubject(lengthM: number, widthM: number, heightM: number, vru: boolean): void {
    if (!(lengthM > 0) || !(heightM > 0)) return;
    this.#subjectL = lengthM;
    this.#subjectW = widthM > 0 ? widthM : this.#subjectW;
    this.#subjectH = heightM;
    this.#subjectVru = vru;
  }

  /**
   * The chase framing for the current subject: distance from its centre, look height and camera
   * elevation. The subject fills about a quarter of the frame's height whatever its size: the
   * distance is half its length plus a stand-off that grows with its height and length.
   */
  chaseFraming(): { distanceM: number; lookHeightM: number; pitchRad: number } {
    const L = this.#subjectL;
    const H = this.#subjectH;
    const standOff = Math.max(this.#subjectVru ? 3.6 : 4.5, 1.5 * H + 0.35 * L);
    const zoom = this.#chaseDistance / CHASE_REFERENCE_M;
    return {
      distanceM: (L / 2 + standOff) * zoom,
      lookHeightM: Math.max(0.9, 0.72 * H),
      pitchRad: this.#orbitPitch - (this.#subjectVru ? 3 * DEG : 0),
    };
  }

  /** Feed the followed actor's interpolated pose. Call once per frame before {@link update}. */
  setFollowPose(x: number, y: number, z: number, headingRad: number, speedMps: number, jumped = false): void {
    // A discontinuity in the subject's own data (the interpolator snapped it) is followed with a
    // cut, not a whip: a spring chasing a car that jumped two metres swings the whole picture.
    if (jumped) this.#subjectJumped = true;
    // A non-finite pose must never reach the camera: one NaN in the position and every matrix
    // downstream is NaN, which draws nothing at all — a black frame.
    if (!Number.isFinite(x) || !Number.isFinite(y) || !Number.isFinite(z)) return;
    const heading = Number.isFinite(headingRad) ? headingRad : this.#followHeading;
    const speed = Number.isFinite(speedMps) ? speedMps : 0;
    if (!Number.isFinite(this.#chaseYaw)) this.#chaseYaw = heading;
    this.#followPos.set(x, y, z);
    this.#followHeading = heading;
    this.#followSpeed = speed;
    this.#followValid = true;
    this.#followPosKnown = true;
    if (this.#mode !== "free" && this.#mode !== "rsu") this.target.set(x, y, z);
  }

  /**
   * The followed actor left the stream; the camera holds its last position instead of snapping.
   *
   * `#followPos` is deliberately left alone. This method used only to clear `#followValid`, which
   * sent `#computeDesired` to the *map focus* instead — the opposite of what this comment promised
   * — so a run ending, or a vehicle despawning, teleported the chase camera to wherever the plan
   * view happened to be pointing.
   */
  clearFollowPose(): void {
    this.#followValid = false;
  }

  /** Free-fly input: a body-frame velocity in metres per second. */
  setFreeVelocity(forward: number, right: number, up: number): void {
    this.#freeVelocity.set(forward, right, up);
  }

  /** Cut to the desired pose with no animation. */
  snap(): void {
    this.#posVel.set(0, 0, 0);
    this.#lookVel.set(0, 0, 0);
    this.#flyT = this.#flyDuration; // a cut is not a flight
    if (this.#followPosKnown) {
      this.#chaseYaw = this.#followHeading;
      this.#chaseYawRate = 0;
    }
    this.#clearDist = Number.POSITIVE_INFINITY;
    this.#computeDesired();
    // The building and ground constraints are applied by `update`, which always runs before a
    // frame is drawn; a cut only places the camera.
    this.camera.position.copy(this.#desiredPosition);
    this.look.copy(this.#desiredLook);
    this.camera.fov = this.#desiredFov;
    this.camera.updateProjectionMatrix();
    this.camera.lookAt(this.look);
    this.camera.updateMatrixWorld();
  }

  /**
   * Advance the smoothing by `dt` seconds.
   *
   * `1 − exp(−λ·dt)` is the frame-rate-independent form of a lerp: the same visual rate whether the
   * frame took 4 ms or 40 ms, which a bare `lerp(x, 0.1)` does not give.
   */
  update(dt: number): void {
    const step = Math.max(0, Math.min(0.25, Number.isFinite(dt) ? dt : 0));
    if (this.#mode === "free") this.#integrateFree(step);
    // The chase camera sits behind a *smoothed* heading. Behind the raw one it rode every tenth of
    // a degree of heading noise at nine metres' lever arm, and a U-turn swung it straight through
    // the car; the smoothed yaw orbits round instead.
    //
    // It is a critically damped spring on the heading, not a first-order lag: a first-order lag
    // starts turning at full rate the instant the car does, which reads as the camera being
    // yanked, while the spring eases into the turn and out of it with no overshoot.
    if (Number.isFinite(this.#chaseYaw)) {
      let e = this.#chaseYaw - this.#followHeading;
      e -= Math.floor(e / (2 * Math.PI) + 0.5) * 2 * Math.PI;
      const [ne, nv] = dampedStep(e, this.#chaseYawRate, CHASE_YAW_OMEGA, step);
      this.#chaseYaw = this.#followHeading + ne;
      this.#chaseYawRate = nv;
    } else if (this.#followPosKnown) {
      this.#chaseYaw = this.#followHeading;
      this.#chaseYawRate = 0;
    }
    this.#computeDesired();
    const street = (this.#mode === "chase" || this.#mode === "dashboard") && this.#followPosKnown;
    if (street && this.#mode === "chase") this.#applyClearance(this.#desiredPosition, this.#desiredLook, step);
    this.keepCameraOutsideBuildings(this.#desiredPosition, this.#desiredLook);
    this.#keepAboveGround(this.#desiredPosition);

    if (this.#pendingFlight && this.#followPosKnown && this.#followValid) {
      this.#pendingFlight = false;
      this.#beginFlight();
    }
    const followStep = step;
    let flew = false;
    if (this.#flyT < this.#flyDuration) {
      const px = this.camera.position.x;
      const py = this.camera.position.y;
      const pz = this.camera.position.z;
      const lx = this.look.x;
      const ly = this.look.y;
      const lz = this.look.z;
      this.#advanceFlight(step);
      flew = true;
      if (this.#flyT >= this.#flyDuration && street && step > 0) {
        // Landed. The follow takes over with the velocity the flight had on its last frame, so
        // the camera's velocity is continuous through the landing; the spring absorbs the
        // difference from the subject's own (a few centimetres of overshoot, not a stop).
        this.#posVel.set((this.camera.position.x - px) / step, (this.camera.position.y - py) / step, (this.camera.position.z - pz) / step);
        this.#lookVel.set((this.look.x - lx) / step, (this.look.y - ly) / step, (this.look.z - lz) / step);
      }
    }
    if (flew) {
      // (the flight moved the camera this frame)
    } else if (street && !this.#pendingFlight) {
      // A critically damped follow with the subject's own velocity fed forward. The old law
      // lerped the camera towards its desired point, which at 15 m/s trailed by v/λ = 3.75 m more
      // than it meant to and pumped the car back and forth in the frame at every change of speed.
      // Pinning the camera to the car rigidly instead passed every kink of the car's drawn path
      // straight to the whole picture. With the velocity fed forward there is no lag at a steady
      // speed; the spring (1/ω: 0.1 s behind a car, 0.04 s in the driver's seat) only smooths the
      // departures from it. A jump bigger than the spring should hide is taken at once.
      if (!finite3(this.camera.position) || !finite3(this.look)) {
        this.recoveries++;
        this.camera.position.copy(this.#desiredPosition);
        this.look.copy(this.#desiredLook);
        this.#posVel.set(0, 0, 0);
        this.#lookVel.set(0, 0, 0);
      }
      const dash = this.#mode === "dashboard";
      const omega = dash ? 25 : 10;
      // A subject that has left the stream is standing where it was last seen.
      const speed = this.#followValid ? this.#followSpeed : 0;
      const c = Math.cos(this.#followHeading);
      const sn = Math.sin(this.#followHeading);
      const vx = c * speed;
      const vy = sn * speed;
      const cut = this.#subjectJumped;
      this.#subjectJumped = false;
      if (cut || this.camera.position.distanceToSquared(this.#desiredPosition) > 3600) {
        this.camera.position.copy(this.#desiredPosition);
        this.#posVel.set(vx, vy, 0);
      } else {
        springFollow(this.camera.position, this.#posVel, this.#desiredPosition, vx, vy, omega, followStep);
      }
      if (cut || this.look.distanceToSquared(this.#desiredLook) > 3600) {
        this.look.copy(this.#desiredLook);
        this.#lookVel.set(vx, vy, 0);
      } else {
        springFollow(this.look, this.#lookVel, this.#desiredLook, vx, vy, omega, followStep);
      }
    } else {
      // A critically damped spring per axis rather than the exponential lerp 09-ui §3 named. The
      // lerp starts every move at its full speed, λ·distance, in one frame: each time the plan
      // view's focus moved (following the traffic, or a click) every vehicle on screen jumped by
      // the whole first step, which the glitch hunter counts as a stutter of every one of them.
      // The spring starts from rest and arrives without overshoot; at ω = 2λ it
      // covers 90 % of a move in about the same 0.55 s the lerp did. Exact per step, so it is
      // frame-rate independent.
      springTo(this.camera.position, this.#posVel, this.#desiredPosition, 2 * this.positionLambda, step);
      springTo(this.look, this.#lookVel, this.#desiredLook, 2 * this.lookLambda, step);
    }
    const kf = 1 - Math.exp(-step * this.fovLambda);

    // The smoothed position can still clip a roof, or the ground, on the way; fix it after the lerp.
    this.keepCameraOutsideBuildings(this.camera.position, this.look);
    this.#keepAboveGround(this.camera.position);

    const fov = this.camera.fov + (this.#desiredFov - this.camera.fov) * kf;
    if (Math.abs(fov - this.camera.fov) > 1e-4) {
      this.camera.fov = fov;
      this.camera.updateProjectionMatrix();
    }
    this.#guardFinite();
    // `lookAt` along a zero-length direction is degenerate; nudge the target rather than let the
    // basis collapse.
    if (this.camera.position.distanceToSquared(this.look) < 1e-6) this.look.z -= 0.01;
    this.camera.lookAt(this.look);
    this.camera.updateMatrixWorld();
  }

  /**
   * Pull the chase camera in front of a wall between it and its subject — smoothly. The march of
   * {@link keepCameraOutsideBuildings} answers "how far back can the camera stand"; that answer
   * jumps as the car passes a façade corner, and applying it raw was a cut. The distance allowed
   * follows it in 0.08 s when it shrinks (a wall must never be seen through for more than a frame
   * or two) and in 0.6 s when it grows back.
   */
  #applyClearance(pos: Vector3, lookAt: Vector3, dt: number): void {
    const world = this.#world;
    const dir = this.#scratchB.copy(pos).sub(lookAt);
    const want = dir.length();
    if (want < 1e-3) return;
    dir.multiplyScalar(1 / want);
    let allowed = want;
    if (world) {
      const steps = Math.min(48, Math.max(6, Math.ceil(want * 2)));
      const own = world.buildingTopAt(lookAt.x, lookAt.y) > lookAt.z ? world.buildingIndexAt(lookAt.x, lookAt.y) : -1;
      for (let i = 1; i <= steps; i++) {
        const t = (want * i) / steps;
        const x = lookAt.x + dir.x * t;
        const y = lookAt.y + dir.y * t;
        const z = lookAt.z + dir.z * t;
        if (own >= 0 && world.buildingIndexAt(x, y) === own) continue;
        const top = world.buildingTopAt(x, y);
        if (top > -Infinity && z < top) {
          allowed = Math.max(0.8, t - 0.6);
          break;
        }
      }
    }
    if (!Number.isFinite(this.#clearDist)) this.#clearDist = allowed;
    const tau = allowed < this.#clearDist ? 0.08 : 0.6;
    this.#clearDist += (allowed - this.#clearDist) * (1 - Math.exp(-dt / tau));
    // Never further than the wall allows right now, whatever the smoothing says.
    const d = Math.min(want, this.#clearDist, allowed);
    pos.set(lookAt.x + dir.x * d, lookAt.y + dir.y * d, lookAt.z + dir.z * d);
  }

  /**
   * Keep `pos` at least {@link CameraControllerOptions.groundClearanceM} above the ground under the
   * subject: the followed vehicle's own height in the street modes, the world's floor otherwise.
   */
  #keepAboveGround(pos: Vector3): boolean {
    let ground = -Infinity;
    const world = this.#world?.world;
    if (world) ground = world.bbox.minZM;
    if ((this.#mode === "chase" || this.#mode === "dashboard") && this.#followPosKnown) {
      ground = Math.max(ground, this.#followPos.z);
    } else if (this.#mode === "rsu" || this.#mode === "free") {
      ground = Math.max(ground, Math.min(this.target.z, this.look.z));
    }
    if (!Number.isFinite(ground)) return false;
    const floor = ground + this.#groundClearance;
    if (pos.z < floor) {
      pos.z = floor;
      return true;
    }
    return false;
  }

  /**
   * Last line of defence: a camera with a non-finite position, target or field of view draws a
   * black frame and never recovers by itself. Put it back on the desired pose, or over the world.
   */
  #guardFinite(): void {
    const p = this.camera.position;
    const l = this.look;
    if (finite3(p) && finite3(l) && Number.isFinite(this.camera.fov) && this.camera.fov > 1) return;
    this.recoveries++;
    this.#posVel.set(0, 0, 0);
    this.#lookVel.set(0, 0, 0);
    this.#flyT = this.#flyDuration;
    if (!Number.isFinite(this.camera.fov) || this.camera.fov <= 1) this.camera.fov = this.#desiredFov;
    if (finite3(this.#desiredPosition) && finite3(this.#desiredLook)) {
      p.copy(this.#desiredPosition);
      l.copy(this.#desiredLook);
    } else {
      const b = this.#world?.world?.bbox;
      const cx = b ? (b.minXM + b.maxXM) / 2 : 0;
      const cy = b ? (b.minYM + b.maxYM) / 2 : 0;
      const cz = b ? b.minZM : 0;
      if (!finite3(this.target)) this.target.set(cx, cy, cz);
      if (!Number.isFinite(this.#altitude)) this.#altitude = 700;
      this.#mode = "map";
      this.#computeDesired();
      p.copy(this.#desiredPosition);
      l.copy(this.#desiredLook);
    }
    this.camera.updateProjectionMatrix();
  }

  /**
   * Start a timed flight from wherever the camera is to wherever the new mode wants it.
   *
   * 09-ui §3's exponential lerp is the right *tracking* law and the wrong *travel* law.
   * `1 − exp(−λ·dt)` covers 90 % of the remaining distance in `2.3/λ` seconds whatever the
   * distance is, so at λ = 4 a 10 m correction and a 1.7 km descent both take 0.58 s and both
   * begin at their maximum speed, `λ · distance`. Measured on the real fly-down over
   * `scenarios/phase1-manhattan.yaml`: frame 3, fifty milliseconds after the click, was already
   * 305 m below the start; 0.4 s in, 80 % of a 1,690 m drop was done; the remaining 30 m took
   * another 1.5 s. A snap followed by a crawl — the cut the design says this must not be.
   *
   * So a mode change hands the camera to a duration-bounded path: `lerp(start, desired, e(t/T))`
   * with a smootherstep `e`, which starts and ends at zero velocity and lands exactly on the
   * desired pose, from where the exponential tracking law takes over with no error and therefore
   * no discontinuity. `T` grows with the square root of the distance, so a 1.7 km descent takes
   * 2.4 s and a chase-to-dashboard hop takes half a second.
   *
   * The path is written *absolutely* from the stored start pose rather than accumulated frame by
   * frame, which is what keeps it frame-rate independent in the strict sense the smoothing law is:
   * one 0.2 s step lands exactly where two 0.1 s steps do.
   */
  #beginFlight(): void {
    this.#posVel.set(0, 0, 0);
    this.#lookVel.set(0, 0, 0);
    this.#computeDesired();
    this.#flyStart.copy(this.camera.position);
    this.#flyStartLook.copy(this.look);
    const dist = this.#flyStart.distanceTo(this.#desiredPosition);
    this.#flyDuration = MathUtils.clamp(0.35 + 0.055 * Math.sqrt(dist), 0.35, this.#flyMaxSeconds);
    this.#flyT = 0;
    // A flight that ends at street level is planned, not lerped: to cruise height (clear of every
    // roof on the way), across, and straight down onto the destination, which is in the street.
    // The straight line from the plan view into a street canyon crossed the building beside the
    // street, and the roof rule then parked the camera on that roof looking at it — the camera
    // fuzz measured frames of one flat colour at the end of the fly-down and of every change of
    // subject. A flight to anywhere else keeps the straight eased line.
    // A hop at street level — chase to dashboard, a change of subject a few metres away — is a
    // straight eased move: planning it over the roofs sent the camera 15 m above the tallest
    // building and back down in half a second.
    const hop = Math.hypot(this.#desiredPosition.x - this.#flyStart.x, this.#desiredPosition.y - this.#flyStart.y);
    this.#flyCruise = CameraController.needsFollowSubject(this.#mode) && (hop > 60 || this.#flyStart.z > this.#desiredPosition.z + 40)
      ? this.#cruiseFor(this.#flyStart, this.#desiredPosition)
      : Number.NaN;
  }

  /**
   * Cruise height for a flight to street level: fifteen metres over the tallest roof between the
   * ends, and for a flight that starts low, a gentle arc besides (a quarter of the distance, at
   * most 600 m). A flight that starts above that simply descends to it.
   */
  #cruiseFor(from: Vector3, to: Vector3): number {
    const world = this.#world;
    const hd = Math.hypot(to.x - from.x, to.y - from.y);
    let top = world?.world ? world.world.bbox.minZM : Math.min(from.z, to.z);
    if (world) {
      const samples = Math.min(64, Math.max(8, Math.ceil(hd / 15)));
      for (let i = 0; i <= samples; i++) {
        const t = i / samples;
        const h = world.buildingTopAt(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
        if (h > top) top = h;
      }
    }
    let cruise = Math.max(top + 15, to.z + 10);
    if (from.z < cruise) cruise = Math.max(cruise, Math.min(Math.min(from.z, to.z) + 0.25 * hd, to.z + 600));
    return cruise;
  }

  /** One step of the flight begun by {@link #beginFlight}. */
  #advanceFlight(step: number): void {
    this.#flyT += step;
    const u = MathUtils.clamp(this.#flyT / this.#flyDuration, 0, 1);
    const p = this.camera.position;
    const cruise = this.#flyCruise;
    if (!Number.isFinite(cruise)) {
      // Smootherstep: zero velocity *and* zero acceleration at both ends.
      const e = smootherstep(u);
      p.lerpVectors(this.#flyStart, this.#desiredPosition, e);
      this.look.lerpVectors(this.#flyStartLook, this.#desiredLook, e);
      return;
    }
    const startLow = this.#flyStart.z < cruise - 1;
    // From a low start: climb to cruise, across, then straight down. From the plan view: the eased
    // straight descent, but never below cruise until the camera is over its destination, then
    // straight down.
    const a0 = startLow ? 0.2 : 0;
    // The last 40 % of a flight to the street is the descent onto the subject: over the last
    // fifth, as it was, a 50 m drop ended at 150 m/s in two frames.
    const c2 = 0.6;
    const across = smootherstep(MathUtils.clamp((u - a0) / (c2 - a0), 0, 1));
    let z: number;
    if (startLow) {
      if (u < 0.2) z = this.#flyStart.z + (cruise - this.#flyStart.z) * smootherstep(u / 0.2);
      else if (u < c2) z = cruise;
      else z = cruise + (this.#desiredPosition.z - cruise) * smootherstep((u - c2) / (1 - c2));
    } else {
      const straight = (v: number): number => this.#flyStart.z + (this.#desiredPosition.z - this.#flyStart.z) * smootherstep(v);
      const atC2 = Math.max(straight(c2), cruise);
      z = u < c2 ? Math.max(straight(u), cruise) : atC2 + (this.#desiredPosition.z - atC2) * smootherstep((u - c2) / (1 - c2));
    }
    p.x = this.#flyStart.x + (this.#desiredPosition.x - this.#flyStart.x) * across;
    p.y = this.#flyStart.y + (this.#desiredPosition.y - this.#flyStart.y) * across;
    p.z = z;
    this.look.lerpVectors(this.#flyStartLook, this.#desiredLook, across);
    // While climbing out of a street, look ahead along the flight 30° below the horizon instead of
    // straight down at a subject beside the building the camera is passing: straight down, the
    // roof a few metres below filled the frame.
    if (startLow && u < c2) {
      const w = Math.min(smootherstep(MathUtils.clamp(u / 0.2, 0, 1)), smootherstep(MathUtils.clamp((c2 - u) / 0.15, 0, 1)));
      let hx = this.#desiredPosition.x - this.#flyStart.x;
      let hy = this.#desiredPosition.y - this.#flyStart.y;
      const hl = Math.hypot(hx, hy);
      if (hl > 1e-3 && w > 0) {
        hx /= hl;
        hy /= hl;
        // Never past the destination (at the world's edge that was a view of the void beyond it).
        const remaining = Math.hypot(this.#desiredPosition.x - p.x, this.#desiredPosition.y - p.y);
        const ahead = Math.max(30, Math.min((p.z - this.#desiredPosition.z) * 1.5, remaining));
        let ax = p.x + hx * ahead;
        let ay = p.y + hy * ahead;
        const b = this.#world?.world?.bbox;
        if (b) {
          ax = MathUtils.clamp(ax, b.minXM, b.maxXM);
          ay = MathUtils.clamp(ay, b.minYM, b.maxYM);
        }
        const drop = Math.max(Math.hypot(ax - p.x, ay - p.y), 1) * Math.tan(30 * DEG);
        this.look.x += (ax - this.look.x) * w;
        this.look.y += (ay - this.look.y) * w;
        this.look.z += (p.z - drop - this.look.z) * w;
      }
    }
  }

  /**
   * jevpilot's `keepCameraOutsideBuildings` rule (09-ui §3), in two parts:
   *
   * 1. if the camera is inside a building footprint below its roof, lift it to the roof plus a
   *    clearance. This applies at any distance, which is what keeps a fly-down from ending up in a
   *    stairwell;
   * 2. if the camera is within {@link CameraControllerOptions.occlusionRangeM} of the look target —
   *    chase and dashboard working distances — march from the target towards it and stop at the
   *    first point inside a building, so a wall never comes between the camera and the car.
   *
   * The march is deliberately *not* run at long range. During a map-to-chase fly-down the camera is
   * hundreds of metres up and the ray to the vehicle passes through every rooftop in the block; a
   * naive march would yank the camera to the vehicle's bumper on the first frame and destroy the
   * continuity the whole interaction is built on.
   *
   * Mutates `pos` in place. Returns true when it moved it.
   */
  keepCameraOutsideBuildings(pos: Vector3, lookAt: Vector3): boolean {
    const world = this.#world;
    if (!world || this.#mode === "map") return false;
    let moved = false;

    // 1. Within working distance, march from the target to the camera and stop short of the first
    //    wall. `buildingTopAt` ignores a building the viewer has ghosted — the passage the followed
    //    vehicle is driving through — so the camera stays behind the car instead of being thrown
    //    onto the roof above it, where the roof hides the very car it is following.
    const dir = this.#scratchB.copy(pos).sub(lookAt);
    const dist = dir.length();
    if (dist >= 1e-3 && dist <= this.#occlusionRange) {
      dir.multiplyScalar(1 / dist);
      const steps = Math.min(48, Math.max(4, Math.ceil(dist)));
      let hitT = -1;
      // A target inside a building (the smoothed look point trailing a vehicle out of a passage,
      // or cutting a corner past a façade) is not a reason to pull the camera onto the target and
      // lift it onto that roof — measured: a frame of that roof and nothing else. The march starts
      // where the ray leaves the target's own building.
      const own = world.buildingTopAt(lookAt.x, lookAt.y) > lookAt.z ? world.buildingIndexAt(lookAt.x, lookAt.y) : -1;
      for (let i = 1; i <= steps; i++) {
        const t = (dist * i) / steps;
        const x = lookAt.x + dir.x * t;
        const y = lookAt.y + dir.y * t;
        const z = lookAt.z + dir.z * t;
        if (own >= 0 && world.buildingIndexAt(x, y) === own) continue;
        const top = world.buildingTopAt(x, y);
        if (top > -Infinity && z < top) {
          hitT = t;
          break;
        }
      }
      if (hitT > 0) {
        const pull = Math.max(0.5, hitT - dist / steps);
        pos.set(lookAt.x + dir.x * pull, lookAt.y + dir.y * pull, lookAt.z + dir.z * pull);
        moved = true;
      }
    } else if (dist > this.#occlusionRange && !this.isFlying && CameraController.needsFollowSubject(this.#mode)) {
      // 1b. Zoomed out beyond working distance: pulling in would throw the zoom away, and lifting
      //     onto whichever roof the camera is in leaves it looking across that roof — measured by
      //     the camera fuzz, a frame of one flat grey. Raise the camera instead, just enough that
      //     the line of sight clears every roof between it and the subject.
      if (this.#raiseToClear(pos, lookAt)) moved = true;
    }
    // 2. Anything still inside a solid building — the fly-down at long range, or a target inside a
    //    building the viewer could not ghost — goes above its roof (jevpilot's rule, 09-ui §3).
    if (this.#liftAboveRoof(pos)) moved = true;
    return moved;
  }

  /**
   * Raise `pos` (keeping its x and y) to the lowest height from which the segment to `lookAt` passes
   * `buildingClearanceM` over every roof in between. Samples inside the target's own building are
   * skipped — a subject inside a building cannot be seen over its walls, and the viewer ghosts that
   * building anyway. Bounded at three times the distance above the target.
   */
  #raiseToClear(pos: Vector3, lookAt: Vector3): boolean {
    const world = this.#world;
    if (!world) return false;
    const hx = pos.x - lookAt.x;
    const hy = pos.y - lookAt.y;
    const hd = Math.hypot(hx, hy);
    if (hd < 1e-3) return false;
    const steps = Math.min(64, Math.max(8, Math.ceil(hd / 4)));
    let need = pos.z;
    let ownBuilding = world.buildingTopAt(lookAt.x, lookAt.y) > lookAt.z;
    for (let i = 1; i <= steps; i++) {
      const t = i / steps;
      const top = world.buildingTopAt(lookAt.x + hx * t, lookAt.y + hy * t);
      if (ownBuilding) {
        if (top > lookAt.z) continue;
        ownBuilding = false;
      }
      if (top === -Infinity) continue;
      // The ray's height at t is lookAt.z + (Z − lookAt.z)·t; it must clear top + clearance.
      const z = lookAt.z + (top + this.#clearance - lookAt.z) / t;
      if (z > need) need = z;
    }
    need = Math.min(need, lookAt.z + 3 * Math.hypot(hd, pos.z - lookAt.z));
    if (need > pos.z) {
      pos.z = need;
      return true;
    }
    return false;
  }

  /** Raise `pos` to the roof plus the clearance if it is inside a building. Returns true if moved. */
  #liftAboveRoof(pos: Vector3): boolean {
    const world = this.#world;
    if (!world) return false;
    const top = world.buildingTopAt(pos.x, pos.y);
    if (top > -Infinity && pos.z < top + this.#clearance) {
      pos.z = top + this.#clearance;
      return true;
    }
    return false;
  }

  #integrateFree(dt: number): void {
    const v = this.#freeVelocity;
    if (v.lengthSq() < 1e-8) return;
    const cy = Math.cos(this.#freeYaw);
    const sy = Math.sin(this.#freeYaw);
    const cp = Math.cos(this.#freePitch);
    const sp = Math.sin(this.#freePitch);
    const fx = cy * cp;
    const fy = sy * cp;
    const fz = sp;
    // Right = forward × up, with up = +z.
    const rx = fy;
    const ry = -fx;
    this.#desiredPosition.x += (fx * v.x + rx * v.y) * dt;
    this.#desiredPosition.y += (fy * v.x + ry * v.y) * dt;
    this.#desiredPosition.z += (fz * v.x + v.z) * dt;
  }

  #computeDesired(): void {
    const d = this.#desiredPosition;
    const l = this.#desiredLook;
    switch (this.#mode) {
      case "map": {
        this.#clampMapTarget();
        // Nearly straight down; see the header note on `lookAt` degeneracy.
        const horizontal = Math.max(this.#altitude * 1e-4, 1e-3);
        d.set(
          this.target.x - Math.cos(this.#bearing) * horizontal,
          this.target.y - Math.sin(this.#bearing) * horizontal,
          this.target.z + this.#altitude,
        );
        l.copy(this.target);
        break;
      }
      case "chase": {
        // `#followPos`, not the map focus: see `clearFollowPose`. `setMode` guarantees a subject
        // exists before this mode can be entered, so the pose is always one of this vehicle's.
        const base = this.#followPos;
        const heading = Number.isFinite(this.#chaseYaw) ? this.#chaseYaw : this.#followHeading;
        const yaw = heading + Math.PI + this.#orbitYaw;
        const f = this.chaseFraming();
        // A little extra trail at speed, for the sense of it: 0 at rest, +20 % at 30 m/s.
        const dist = f.distanceM * (1 + Math.min(0.2, this.#followSpeed / 150));
        const cp = Math.cos(f.pitchRad);
        const sp = Math.sin(f.pitchRad);
        d.set(
          base.x + Math.cos(yaw) * dist * cp,
          base.y + Math.sin(yaw) * dist * cp,
          base.z + dist * sp + f.lookHeightM,
        );
        l.set(base.x, base.y, base.z + f.lookHeightM);
        break;
      }
      case "dashboard": {
        const base = this.#followPos;
        const h = this.#followHeading + this.#orbitYaw;
        const fx = Math.cos(h);
        const fy = Math.sin(h);
        // The driver's eye. A car's driver sits about 2 m behind the front bumper and 0.35 m left
        // of the centre line (left-hand drive), a bus or truck driver right at the front and high,
        // a rider over the saddle; a person's eyes are about 93 % of their height (the 1.60 m eye
        // height of the 50th-percentile man, 1.75 m tall, NASA-STD-3000 anthropometry).
        const L = this.#subjectL;
        const H = this.#subjectH;
        let ahead: number;
        let left: number;
        let eye: number;
        if (this.#subjectVru) {
          ahead = 0.12;
          left = 0;
          eye = Math.max(1.1, 0.93 * H);
        } else if (L >= 6.8) {
          ahead = L / 2 - 1.0;
          left = Math.min(0.55, this.#subjectW * 0.25);
          eye = Math.min(H - 0.3, Math.max(1.9, 0.72 * H));
        } else if (L <= 2.6) {
          ahead = -0.05;
          left = 0;
          eye = Math.max(1.2, 0.95 * H);
        } else {
          ahead = L / 2 - Math.min(2.0, 0.42 * L);
          left = Math.min(0.37, this.#subjectW * 0.2);
          eye = this.#eyeHeight * (H > 1.7 ? H / 1.5 : 1);
          eye = Math.min(H - 0.12, eye);
        }
        const lx = -fy;
        const ly = fx;
        d.set(base.x + fx * ahead + lx * left, base.y + fy * ahead + ly * left, base.z + eye);
        const pitch = MathUtils.clamp(this.#orbitPitch - 16 * DEG, -0.5, 0.5);
        l.set(d.x + fx * 30, d.y + fy * 30, d.z + Math.tan(pitch) * 30);
        break;
      }
      case "rsu": {
        const world = this.#world;
        if (world && this.#rsuIndex >= 0 && this.#rsuIndex < world.siteCount) {
          const p = world.sitePositions;
          const i = this.#rsuIndex * 3;
          d.set(p[i], p[i + 1], p[i + 2] + 1.2);
        } else {
          d.set(this.target.x, this.target.y, this.target.z + 25);
        }
        if (this.#followPosKnown) l.copy(this.#followPos);
        else l.set(this.target.x, this.target.y, this.target.z);
        if (l.distanceToSquared(d) < 1) l.set(d.x + 30, d.y, d.z - 8);
        break;
      }
      case "free":
      default: {
        const cy = Math.cos(this.#freeYaw);
        const sy = Math.sin(this.#freeYaw);
        const cp = Math.cos(this.#freePitch);
        l.set(
          d.x + cy * cp * 40,
          d.y + sy * cp * 40,
          d.z + Math.sin(this.#freePitch) * 40,
        );
        break;
      }
    }
  }

  /**
   * Keep the plan view's window inside the world, so the aerial view never shows void.
   *
   * In `map` mode {@link setFollowPose} moves the focus onto the followed vehicle every frame. On
   * phase1-manhattan the vehicle drives to within 20 m of the world's north edge, which put about
   * 40 % of the frame outside the city — a black band with nothing in it. The same happens when a
   * drag reaches the edge.
   *
   * The window is the vertical extent by the horizontal extent, rotated by the bearing; its
   * axis-aligned span is what has to fit. When the world is *narrower* than the window on an axis
   * the focus is centred on that axis instead, which is the only framing that can be right.
   */
  #clampMapTarget(): void {
    const world = this.#world?.world;
    if (!world) return;
    const b = world.bbox;
    const halfV = this.extentM / 2;
    const halfH = halfV * Math.max(0.01, this.camera.aspect);
    const ab = Math.abs(Math.cos(this.#bearing));
    const sb = Math.abs(Math.sin(this.#bearing));
    const spanX = halfH * sb + halfV * ab;
    const spanY = halfH * ab + halfV * sb;
    this.target.x = clampSpan(this.target.x, b.minXM, b.maxXM, spanX);
    this.target.y = clampSpan(this.target.y, b.minYM, b.maxYM, spanY);
  }

  /** A plain snapshot, shaped for §6.7 `view.camera`. */
  state(): CameraState {
    return {
      mode: this.#mode,
      position: { x: this.camera.position.x, y: this.camera.position.y, z: this.camera.position.z },
      target: { x: this.look.x, y: this.look.y, z: this.look.z },
      fovDeg: this.camera.fov,
      followActorId: this.#followActorId,
      altitudeM: this.#altitude,
      distanceM: this.#chaseDistance,
      bearingRad: this.#bearing,
    };
  }

  /**
   * Bind pointer and wheel input: drag to pan (map) or orbit (chase, dashboard, free, rsu), wheel to
   * change altitude or distance, shift-drag to rotate the map bearing. Returns a detach function;
   * calling {@link attachInput} again detaches the previous binding first.
   */
  attachInput(element: InputTarget): () => void {
    this.detachInput();
    let dragging = false;
    let lastX = 0;
    let lastY = 0;
    let shift = false;

    const onPointerDown = (e: Event): void => {
      const ev = e as PointerEvent;
      dragging = true;
      this.#userMoved = true;
      lastX = ev.clientX;
      lastY = ev.clientY;
      shift = ev.shiftKey || ev.button === 2;
      // Capture the pointer so a drag that leaves the canvas keeps delivering `pointermove` here
      // instead of silently ending mid-gesture.
      const target = element as { setPointerCapture?: (id: number) => void };
      if (typeof target.setPointerCapture === "function" && ev.pointerId !== undefined) {
        try {
          target.setPointerCapture(ev.pointerId);
        } catch {
          // A synthetic or already-released pointer; dragging still works without capture.
        }
      }
    };
    const onPointerMove = (e: Event): void => {
      if (!dragging) return;
      const ev = e as PointerEvent;
      const dx = ev.clientX - lastX;
      const dy = ev.clientY - lastY;
      lastX = ev.clientX;
      lastY = ev.clientY;
      if (this.#mode === "map") {
        if (shift) {
          this.#bearing -= dx * 0.005;
        } else {
          // Screen pixels → metres at the focus plane.
          const mPerPx = (2 * this.#altitude * Math.tan((this.camera.fov * DEG) / 2)) / this.#viewportH;
          const cb = Math.cos(this.#bearing);
          const sb = Math.sin(this.#bearing);
          // Screen right is (sin b, −cos b); screen up is (cos b, sin b).
          this.target.x -= (dx * sb + dy * cb) * mPerPx;
          this.target.y -= (-dx * cb + dy * sb) * mPerPx;
        }
      } else if (this.#mode === "free") {
        this.#freeYaw -= dx * 0.004;
        this.#freePitch = MathUtils.clamp(this.#freePitch - dy * 0.004, -1.5, 1.5);
      } else {
        this.#orbitYaw -= dx * 0.006;
        this.#orbitPitch = MathUtils.clamp(this.#orbitPitch + dy * 0.004, -0.35, 1.25);
      }
    };
    const onPointerUp = (_e: Event): void => {
      dragging = false;
    };
    const onWheel = (e: Event): void => {
      const ev = e as WheelEvent;
      // The gesture zooms the camera, so it must not also scroll an ancestor. That is only
      // possible on a listener registered non-passively (finding Q20): the Studio happens to set
      // `body { overflow: hidden }`, but `attachInput` is a library surface and cannot assume it.
      if (typeof ev.preventDefault === "function" && ev.cancelable !== false) ev.preventDefault();
      this.#userMoved = true;
      const k = Math.exp(ev.deltaY * 0.0012);
      if (this.#mode === "map") this.altitudeM = this.#altitude * k;
      else this.distanceM = this.#chaseDistance * k;
    };

    const onKeyDown = (e: Event): void => {
      const ev = e as KeyboardEvent;
      if (ev.ctrlKey || ev.metaKey || ev.altKey) return;
      // `f` for "frame": the one keystroke that answers "where is the vehicle?" when a single
      // actor is loose in a square kilometre of city.
      // Registered passively with the pointer listeners, so no `preventDefault` here — `f` has no
      // default action on a canvas to cancel, and calling it on a passive listener is a warning.
      if (ev.key === "f" || ev.key === "F") this.#onFrameActors?.();
    };

    // `pointerleave` is deliberately *not* bound: with pointer capture a drag survives leaving the
    // element, and treating leave as pointer-up would end it mid-gesture.
    const passive: readonly (readonly [string, (e: Event) => void])[] = [
      ["pointerdown", onPointerDown],
      ["pointermove", onPointerMove],
      ["pointerup", onPointerUp],
      ["pointercancel", onPointerUp],
      ["keydown", onKeyDown],
    ];
    for (const [type, fn] of passive) element.addEventListener(type, fn, { passive: true });
    element.addEventListener("wheel", onWheel, { passive: false });

    this.#detach = (): void => {
      for (const [type, fn] of passive) element.removeEventListener(type, fn);
      element.removeEventListener("wheel", onWheel);
    };
    return this.#detach;
  }

  /** Undo {@link attachInput}. */
  detachInput(): void {
    if (this.#detach) {
      this.#detach();
      this.#detach = null;
    }
  }

  /** Release listeners. The camera itself belongs to the caller. */
  dispose(): void {
    this.detachInput();
    this.#world = null;
    this.#onFrameActors = null;
  }
}
