/**
 * Where the ground is, and the two geometric guarantees the camera owes it.
 *
 * **The ground is not the bottom of the world's box.** Every part of the viewer used to put the
 * ground plane, the landuse, the junction discs and the crosswalks at `world.bbox.minZM`, and
 * clamped the camera to it. That is the street level only while nothing in the world is below the
 * street. The OSM importer puts a tunnel 6 m below ground per `layer` level
 * (`crates/v2xw-world/src/osm.rs`, `TUNNEL_LEVEL_DEPTH_M`): on Manhattan the FDR Drive, the Park
 * Avenue and Queens-Midtown tunnels put `minZM` at −6 m or −12 m. The ground plane then lay 6 m
 * under every street, every junction disc 6 m under its junction — a hole at every intersection —
 * and between the road ribbons the ground "went down" (the owner's report, 2026-10-06). The
 * camera's floor was the same −6 m, so a camera could sink under the streets and the buildings.
 *
 * The ground level is the level the buildings stand on: the median of their base heights. A world
 * without buildings falls back to its junctions, which are at grade except where a junction is a
 * tunnel's or a bridge's, and a world with neither to zero inside the box.
 */

import type { Matrix4, PerspectiveCamera } from "three";
import type { VwpWorld } from "@vwp/protocol";

/**
 * Highest a drawn road surface rises above the ground level, metres: a crossing (0.22) or a lane
 * marking (0.26) on a sidewalk lifted 0.08 above the carriageway, plus a kerb's 0.15 in the lane
 * geometry (a US kerb is 6 in., 0.15 m: AASHTO Green Book 2018 §4.7). Rounded up.
 */
export const SURFACE_TOP_M = 0.5;

/** Ground-level cache: a world is immutable once decoded. */
const cache = new WeakMap<VwpWorld, number>();

function median(values: Float64Array): number {
  values.sort();
  const n = values.length;
  return n % 2 === 1 ? values[(n - 1) >> 1] : (values[n / 2 - 1] + values[n / 2]) / 2;
}

/** The world's ground level, metres: see the module note. */
export function groundLevelOf(world: VwpWorld | null | undefined): number {
  if (!world) return 0;
  const hit = cache.get(world);
  if (hit !== undefined) return hit;
  let z: number;
  const b = world.buildings;
  const finiteOf = (n: number, at: (i: number) => number): Float64Array => {
    const out: number[] = [];
    for (let i = 0; i < n; i++) {
      const v = at(i);
      if (Number.isFinite(v)) out.push(v);
    }
    return Float64Array.from(out);
  };
  const bases = finiteOf(b.count, (i) => b.baseZM[i]);
  if (bases.length > 0) {
    z = median(bases);
  } else {
    const j = finiteOf(world.junctions.count, (i) => world.junctions.at(i).zM);
    z = j.length > 0 ? median(j) : 0;
  }
  const lo = world.bbox.minZM;
  const hi = world.bbox.maxZM;
  if (Number.isFinite(lo) && Number.isFinite(hi) && lo <= hi) z = Math.min(hi, Math.max(lo, z));
  if (!Number.isFinite(z)) z = 0;
  cache.set(world, z);
  return z;
}

/**
 * The lowest world height reached by the camera's near-plane rectangle, per metre of near
 * distance, below the camera (a negative number), or 0 when no corner of it points down.
 *
 * Exact for any perspective projection, including the lens shift the Studio's view insets apply:
 * the four corners are unprojected from the projection's own inverse, so their directions are what
 * the GPU clips against. Needs `camera.matrixWorld` and `camera.projectionMatrixInverse` current.
 */
export function nearPlaneDropPerMetre(camera: PerspectiveCamera): number {
  const inv: Matrix4 = camera.projectionMatrixInverse;
  const p = inv.elements;
  const m = camera.matrixWorld.elements;
  let lowest = 0;
  for (let k = 0; k < 4; k++) {
    const nx = k & 1 ? 1 : -1;
    const ny = k & 2 ? 1 : -1;
    // Unproject (nx, ny, −1, 1): view-space point on the near plane.
    const x = p[0] * nx + p[4] * ny - p[8] + p[12];
    const y = p[1] * nx + p[5] * ny - p[9] + p[13];
    const z = p[2] * nx + p[6] * ny - p[10] + p[14];
    const w = p[3] * nx + p[7] * ny - p[11] + p[15];
    if (!(Math.abs(w) > 1e-12)) continue;
    const vz = z / w;
    if (!(vz < 0)) continue;
    // Per metre of near distance: scale so the view-space depth is −1.
    const s = -1 / vz;
    const dx = (x / w) * s;
    const dy = (y / w) * s;
    // World z of the direction (dx, dy, −1) through the camera's rotation.
    const wz = m[2] * dx + m[6] * dy - m[10];
    if (wz < lowest) lowest = wz;
  }
  return lowest;
}

/**
 * The largest near distance whose near-plane rectangle stays above `surfaceZ`, metres, or
 * `Infinity` when the rectangle never points down.
 */
export function nearPlaneLimit(camera: PerspectiveCamera, surfaceZ: number): number {
  const drop = nearPlaneDropPerMetre(camera);
  if (!(drop < 0)) return Number.POSITIVE_INFINITY;
  return Math.max(0, camera.matrixWorld.elements[14] - surfaceZ) / -drop;
}
