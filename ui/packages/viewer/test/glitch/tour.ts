/**
 * The camera tour the glitch hunter watches: aerial, then chase over several subjects of different
 * sizes (a car, the largest vehicle on the road, a person, a cyclist), then dashboard.
 */

import type { GlitchReport } from "../../src/glitch.js";
import type { StreamReplay } from "./replay.js";

export interface TourOptions {
  readonly warmupS?: number;
  readonly aerialS?: number;
  /** Seconds per chase subject. */
  readonly chaseS?: number;
  readonly dashboardS?: number;
  /** Aerial extent, metres. Default 420. */
  readonly aerialExtentM?: number;
  /** Log progress to stderr. */
  readonly verbose?: boolean;
}

export interface TourResult {
  readonly report: GlitchReport;
  /** Which subjects the chase followed, by class name. */
  readonly subjects: string[];
}

/** The live slot, by class category and name preference, nearest the densest 100 m cell. */
function pickSubject(replay: StreamReplay, want: (name: string, category: number) => boolean, exclude: Set<number>): number | null {
  const v = replay.viewer;
  const it = v.interpolator;
  const classes = v.actors.classes;
  const n = it.count;
  const cells = new Map<string, number>();
  for (let s = 0; s < n; s++) {
    if (it.outOccupied[s] !== 1) continue;
    const k = `${Math.floor(it.outPosition[s * 3] / 100)},${Math.floor(it.outPosition[s * 3 + 1] / 100)}`;
    cells.set(k, (cells.get(k) ?? 0) + 1);
  }
  let best = "";
  let bestN = -1;
  for (const [k, c] of cells) if (c > bestN) { best = k; bestN = c; }
  const [cx, cy] = best.split(",").map((q) => (Number(q) + 0.5) * 100);
  let pick = -1;
  let pickD = Infinity;
  for (let s = 0; s < n; s++) {
    if (it.outOccupied[s] !== 1) continue;
    const id = it.outActorId[s];
    if (exclude.has(id)) continue;
    const c = it.outClassIdx[s] < classes.length ? classes[it.outClassIdx[s]] : classes[0];
    if (!want(c.name, c.category)) continue;
    if (Math.abs(it.outSpeed[s]) < 0.8) continue; // something that is going somewhere
    const d = Math.hypot(it.outPosition[s * 3] - cx, it.outPosition[s * 3 + 1] - cy);
    if (d < pickD) {
      pickD = d;
      pick = s;
    }
  }
  return pick >= 0 ? it.outActorId[pick] : null;
}

/** Run the tour; the hunter accumulates across it (see `report.countsByMode`). */
export function runTour(replay: StreamReplay, options: TourOptions = {}): TourResult {
  const v = replay.viewer;
  const log = (m: string): void => {
    if (options.verbose) process.stderr.write(`[tour ${(replay.nowMs / 1000).toFixed(1)} s] ${m}\n`);
  };
  replay.run(options.warmupS ?? 4, false);
  // Aerial: the Studio's plan view, zoomed to street-reading scale over the traffic.
  v.setCameraMode("map", true);
  v.cameras.fitExtent(options.aerialExtentM ?? 420);
  log("aerial");
  replay.run(options.aerialS ?? 20);

  const subjects: string[] = [];
  const used = new Set<number>();
  const kinds: [string, (n: string, c: number) => boolean][] = [
    ["car", (n) => n === "passenger" || n === "car"],
    ["large", (n) => ["bus", "coach", "truck", "trailer", "delivery", "emergency"].includes(n)],
    ["pedestrian", (n) => n === "pedestrian"],
    ["two-wheeler", (n) => ["bicycle", "motorcycle", "moped", "scooter", "moto"].includes(n)],
  ];
  for (const [label, want] of kinds) {
    const id = pickSubject(replay, want, used);
    if (id === null) {
      log(`no ${label} to follow`);
      continue;
    }
    used.add(id);
    const slot = findSlot(replay, id);
    const cls = slot >= 0 ? v.actors.classes[v.interpolator.outClassIdx[slot]]?.name ?? "?" : "?";
    subjects.push(cls);
    log(`chase ${label}: ${cls} ${id}`);
    v.flyTo(id, "chase");
    replay.run(options.chaseS ?? 15);
  }

  const dash = pickSubject(replay, (n, c) => c === 0 && n !== "bicycle", used);
  if (dash !== null) {
    log(`dashboard ${dash}`);
    v.flyTo(dash, "dashboard");
    replay.run(options.dashboardS ?? 20);
  }
  return { report: replay.hunter.report(), subjects };
}

function findSlot(replay: StreamReplay, id: number): number {
  const it = replay.viewer.interpolator;
  for (let s = 0; s < it.count; s++) if (it.outOccupied[s] === 1 && it.outActorId[s] === id) return s;
  return -1;
}
