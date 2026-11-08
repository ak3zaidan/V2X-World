/**
 * One vehicle's certificate lifecycle as a timeline (the workspace's vehicle drill-down): its
 * enrolment, every pseudonym download (the first batch, then each top-up), each pseudonym change,
 * the misbehaviour reports it sent and its revocation, on the run's clock.
 *
 * Built from what the engine published about the vehicle and nothing else: the flow instances it
 * took part in (`inspect.flows {device}`), its pseudonym history and grid (`inspect.pki
 * {topic: "vehicle"}`). The factory and dealer steps before the run (`preload`) are not on the
 * run's clock, so they are listed apart. Pure (`test/pki-vehicle.test.ts`).
 */

import type { FlowRow } from "./pki.js";
import type { VehicleDetail } from "./pkiRecords.js";

/** One event on the timeline. */
export interface LifeEvent {
  readonly lane: "enrolment" | "download" | "pseudonym" | "report" | "revocation";
  readonly t: number;
  readonly end: number;
  readonly label: string;
  readonly instance: string | null;
}

/** The lanes, top to bottom, in plain words. */
export const LANES: readonly { readonly id: LifeEvent["lane"]; readonly label: string; readonly term: string }[] = [
  { id: "enrolment", label: "Enrolment", term: "Enrolment certificate from the ECA (via the DCM)" },
  { id: "download", label: "Downloads", term: "Pseudonym certificate batches from the RA: the first, then each top-up" },
  { id: "pseudonym", label: "Signing as", term: "The pseudonym certificate it signs with; each change is a new pseudonym" },
  { id: "report", label: "Reports sent", term: "Misbehaviour reports it sent to the MA" },
  { id: "revocation", label: "Revocation", term: "A revocation list it installed names it" },
];

/** The timeline: events on the run's clock, the factory steps apart, and the span to draw. */
export interface Lifecycle {
  readonly events: readonly LifeEvent[];
  readonly beforeRun: readonly FlowRow[];
  readonly t0: number;
  readonly t1: number;
  /** Downloads per i-period in the grid it holds now: week → certificates. */
  readonly batches: readonly { readonly i: number; readonly certs: number }[];
}

/** The vehicle's lifecycle from its detail and the flow instances it took part in. */
export function lifecycleOf(
  d: Pick<VehicleDetail, "pseudonyms" | "grid" | "revoked" | "t" | "first_t"> & { readonly active?: string | null },
  flows: readonly FlowRow[],
  nowNs: number,
): Lifecycle {
  const events: LifeEvent[] = [];
  const beforeRun: FlowRow[] = [];
  const downloads = flows.filter((f) => f.flow === "provisioning").sort((a, b) => a.first_t - b.first_t);
  for (const f of flows) {
    if (f.flow === "preload") {
      beforeRun.push(f);
      continue;
    }
    if (f.flow === "enrolment") events.push({ lane: "enrolment", t: f.first_t, end: f.last_t, label: "enrolled", instance: f.instance });
    else if (f.flow === "report") events.push({ lane: "report", t: f.first_t, end: f.last_t, label: "report", instance: f.instance });
  }
  downloads.forEach((f, k) =>
    events.push({ lane: "download", t: f.first_t, end: f.last_t, label: k === 0 && beforeRun.length === 0 ? "first batch" : "top-up", instance: f.instance }),
  );
  const ps = [...d.pseudonyms].sort((a, b) => a.since - b.since);
  // The newest pseudonym runs to "now" while it is still the one in use; a vehicle between two
  // pseudonyms (no active certificate in its record) stopped signing with it by the record's time.
  const lastEnd = (p: { readonly since: number; readonly id8: string }): number =>
    d.active === undefined || d.active === p.id8 ? Math.max(p.since, nowNs) : Math.max(p.since, Math.min(d.t, nowNs));
  ps.forEach((p, k) => events.push({ lane: "pseudonym", t: p.since, end: k + 1 < ps.length ? ps[k + 1].since : lastEnd(p), label: p.id8, instance: null }));
  if (d.revoked) events.push({ lane: "revocation", t: d.t, end: d.t, label: "revoked (as of this record)", instance: null });
  const times = events.flatMap((e) => [e.t, e.end]);
  const t0 = Math.min(d.first_t, ...times);
  const t1 = Math.max(nowNs, d.t, ...times);
  const per = new Map<number, number>();
  for (const c of d.grid.held) if (c.state !== "not-yet-available") per.set(c.i, (per.get(c.i) ?? 0) + 1);
  const batches = [...per.entries()].sort((a, b) => a[0] - b[0]).map(([i, certs]) => ({ i, certs }));
  return { events: events.sort((a, b) => a.t - b.t), beforeRun, t0, t1: t1 > t0 ? t1 : t0 + 1, batches };
}

/** The one unit an axis reaching `maxNs` is labelled in, so its ticks never mix seconds and minutes. */
export function axisUnit(maxNs: number): { readonly unit: string; readonly ns: number } {
  const s = maxNs / 1e9;
  if (s <= 300) return { unit: "s", ns: 1e9 };
  if (s <= 3 * 3600) return { unit: "min", ns: 60e9 };
  if (s <= 3 * 86_400) return { unit: "h", ns: 3600e9 };
  return { unit: "d", ns: 86_400e9 };
}

/** A tick's label in the axis's unit: at most one decimal, no trailing zero ("90 s", "1.5 min"). */
export function tickLabel(ns: number, u: { readonly unit: string; readonly ns: number }): string {
  const v = Math.round((ns / u.ns) * 10) / 10;
  return `${v} ${u.unit}`;
}

/** A round tick step for a span of `ns`, so the axis shows four to eight labels. */
export function tickStep(ns: number): number {
  const s = ns / 1e9;
  const steps = [1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 21_600, 43_200, 86_400, 172_800, 604_800];
  return (steps.find((x) => s / x <= 8) ?? 604_800) * 1e9;
}
