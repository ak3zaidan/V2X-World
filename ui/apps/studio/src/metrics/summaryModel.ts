/**
 * The run summary's pure half: the scenario in one line, which numbers lead, how a run's windows
 * are reduced to one number, the dominant share of a breakdown, and which backend entities are
 * saturated. No DOM, no engine, so `test/run-summary.test.ts` pins it in Node.
 */

import type { BackendSnapshot } from "../lib/backend.js";
import type { GroupRow, Series } from "./model.js";

// ------------------------------------------------------------------------------------------------
// The scenario in one line
// ------------------------------------------------------------------------------------------------

const RAT_NAMES: Readonly<Record<string, string>> = {
  "dsrc-80211p": "DSRC (IEEE 802.11p)",
  "its-g5": "ITS-G5 (802.11p)",
  "lte-v2x": "LTE-V2X sidelink",
  "lte-v2x-pc5": "LTE-V2X sidelink",
  "nr-v2x": "5G NR-V2X sidelink",
  "nr-v2x-pc5": "5G NR-V2X sidelink",
};

function get(doc: unknown, path: readonly string[]): unknown {
  let cur: unknown = doc;
  for (const k of path) {
    if (typeof cur !== "object" || cur === null) return undefined;
    cur = (cur as Record<string, unknown>)[k];
  }
  return cur;
}

function str(v: unknown): string | null {
  return typeof v === "string" && v.trim() !== "" ? v.trim() : null;
}

/** The credential system a scenario runs, in words. */
export function credentialSystem(scenario: unknown): string | null {
  const id = str(get(scenario, ["security", "protocol", "id"])) ?? str(get(scenario, ["actors", "backend", "protocol"]));
  if (id === null) return null;
  if (id.includes("scms")) return "US SCMS credentials";
  if (id.includes("ccms")) return "EU CCMS credentials";
  return id.replace(/^protocol\//, "");
}

/** The attacker share a scenario declares, or 0. */
export function attackerShare(scenario: unknown): number {
  const list = get(scenario, ["threats", "attackers"]);
  if (!Array.isArray(list)) return 0;
  let total = 0;
  for (const a of list) {
    const f = (a as { fraction?: unknown }).fraction;
    if (typeof f === "number" && Number.isFinite(f)) total += f;
  }
  return Math.min(1, total);
}

/**
 * "manhattan-5min: 300 s, 113 vehicles, DSRC (IEEE 802.11p), US SCMS credentials, 5 % attackers."
 * Each part only when the scenario or the run says it.
 */
export function scenarioLine(
  scenario: unknown,
  run: { readonly tNs: number; readonly tEndNs: number; readonly actors: number; readonly nodes: number },
  fallbackName: string | null,
): string {
  const name = str(get(scenario, ["meta", "name"])) ?? fallbackName ?? "This run";
  const parts: string[] = [];
  const shownS = run.tNs / 1e9;
  const endS = run.tEndNs / 1e9;
  if (shownS > 0) parts.push(endS > 0 && Math.abs(endS - shownS) > 0.5 ? `${round(shownS)} of ${round(endS)} s simulated` : `${round(shownS)} s simulated`);
  if (run.actors > 0) parts.push(`${run.actors.toLocaleString("en-US")} road users at the end`);
  const rat = str(get(scenario, ["radio", "rat"]));
  if (rat !== null) parts.push(RAT_NAMES[rat] ?? rat);
  const cred = credentialSystem(scenario);
  if (cred !== null) parts.push(cred);
  const att = attackerShare(scenario);
  if (att > 0) parts.push(`${Math.round(att * 1000) / 10} % attackers`);
  return parts.length > 0 ? `${name}: ${parts.join(", ")}.` : `${name}.`;
}

/** The scenario's own description, first sentence, if it has one. */
export function scenarioPurpose(scenario: unknown): string | null {
  const d = str(get(scenario, ["meta", "description"]));
  if (d === null) return null;
  const flat = d.replace(/\s+/g, " ");
  const m = /^(.+?[.!?])(\s|$)/.exec(flat);
  return m ? m[1] : flat;
}

function round(x: number): string {
  return x >= 100 ? String(Math.round(x)) : String(Math.round(x * 10) / 10);
}

// ------------------------------------------------------------------------------------------------
// The headline numbers
// ------------------------------------------------------------------------------------------------

/** How a run's windows reduce to one number. */
export type Reduce = "mean" | "max" | "min" | "last" | "sum";

export interface HeadlineItem {
  readonly series: string;
  readonly label: string;
  readonly reduce: Reduce;
}

export interface HeadlineGroup {
  readonly id: string;
  readonly title: string;
  readonly items: readonly HeadlineItem[];
}

/**
 * What a protocol evaluation leads with, in order. A series the run does not measure is left out
 * of its tile, and a tile with nothing measured says so.
 */
export const HEADLINES: readonly HeadlineGroup[] = [
  {
    id: "delivery",
    title: "Delivery",
    items: [
      { series: "pdr[100m]", label: "Delivered within 100 m", reduce: "mean" },
      { series: "pdr[300m]", label: "Delivered within 300 m", reduce: "mean" },
      { series: "nar[100m]", label: "Neighbours known within 100 m", reduce: "mean" },
      { series: "aoi", label: "Age of information", reduce: "mean" },
    ],
  },
  {
    id: "latency",
    title: "Latency",
    items: [
      { series: "e2e_latency", label: "Mean end-to-end latency", reduce: "mean" },
      { series: "e2e_latency.p99", label: "Worst window's p99 latency", reduce: "max" },
      { series: "mac_access_delay.p95", label: "Worst window's p95 channel access", reduce: "max" },
    ],
  },
  {
    id: "channel",
    title: "Channel load",
    items: [
      { series: "cbr", label: "Mean channel busy ratio", reduce: "mean" },
      { series: "cbr.p95", label: "Busiest window's p95 busy ratio", reduce: "max" },
      { series: "airtime_per_node", label: "Air time per node", reduce: "mean" },
    ],
  },
  {
    id: "security",
    title: "Security overhead",
    items: [
      { series: "envelope_overhead", label: "Security bytes per payload byte", reduce: "mean" },
      { series: "verify_wait.p95", label: "Worst window's p95 verification wait", reduce: "max" },
      { series: "unverified_ratio", label: "Delivered without verification", reduce: "mean" },
    ],
  },
  {
    id: "credentials",
    title: "Credentials and revocation",
    items: [
      { series: "cert_pool_valid", label: "Lowest window's mean valid certificates", reduce: "min" },
      { series: "crl_entries", label: "Revoked (CRL entries) at the end", reduce: "last" },
      { series: "det_recall", label: "Attackers detected", reduce: "last" },
      { series: "false_accusations", label: "False accusations", reduce: "sum" },
      { series: "revocation_latency_stage.p95", label: "Slowest revocation stage (p95)", reduce: "max" },
    ],
  },
  {
    id: "traffic",
    title: "Traffic flow",
    items: [
      { series: "mean_speed", label: "Mean speed", reduce: "mean" },
      { series: "flow", label: "Flow", reduce: "mean" },
      { series: "density", label: "Density", reduce: "mean" },
      { series: "ttc_conflicts", label: "Near-collision conflicts (TTC < 1.5 s)", reduce: "sum" },
    ],
  },
];

/** A series reduced over its observed windows; `null` when none was observed. */
export function reduceSeries(s: Series, how: Reduce): number | null {
  let n = 0;
  let acc = how === "max" ? Number.NEGATIVE_INFINITY : how === "min" ? Number.POSITIVE_INFINITY : 0;
  let last: number | null = null;
  for (let i = 0; i < s.v.length; i++) {
    const v = s.v[i];
    if (!Number.isFinite(v)) continue;
    n++;
    last = v;
    if (how === "max") acc = Math.max(acc, v);
    else if (how === "min") acc = Math.min(acc, v);
    else acc += v;
  }
  if (n === 0) return null;
  if (how === "mean") return acc / n;
  if (how === "last") return last;
  return acc;
}

/** The base metric of a series name: `e2e_latency.p99` → `e2e_latency`, `pdr[100m]` → `pdr`. */
export function baseOf(series: string): string {
  return series.replace(/\[[^\]]*\]$/, "").replace(/\.p\d+$/, "");
}

// ------------------------------------------------------------------------------------------------
// Bottlenecks
// ------------------------------------------------------------------------------------------------

/** The largest group of a breakdown and its share of the total, or `null` when there is nothing. */
export function dominant(rows: readonly GroupRow[]): { key: string; value: number; share: number } | null {
  let total = 0;
  let best: GroupRow | null = null;
  for (const r of rows) {
    if (r.value === null || !Number.isFinite(r.value) || r.value <= 0) continue;
    total += r.value;
    if (best === null || (best.value as number) < r.value || ((best.value as number) === r.value && r.key < best.key)) best = r;
  }
  if (best === null || total <= 0) return null;
  return { key: best.key, value: best.value as number, share: (best.value as number) / total };
}

/**
 * The utilisation above which a backend entity counts as saturated, 0.8: queueing delay grows
 * without bound as utilisation approaches 1, and 0.8 is the usual planning ceiling. The run
 * analyst's `entity_utilisation`; an engineering choice, stated as one.
 */
export const ENTITY_UTILISATION_LIMIT = 0.8;

export interface EntityLoad {
  readonly id: string;
  readonly name: string;
  readonly utilisation: number;
  readonly depth: number;
  readonly meanWaitMs: number | null;
}

/**
 * Each backend entity's server utilisation over the run so far — busy time over servers × elapsed
 * time — with its queue depth and mean wait, busiest first. Entities with no queue are left out.
 */
export function entityLoads(s: BackendSnapshot, tNs: number): EntityLoad[] {
  if (!(tNs > 0)) return [];
  const out: EntityLoad[] = [];
  for (const e of s.entities) {
    const q = e.queue;
    if (q === null || q.servers <= 0) continue;
    const u = q.busy_ns / (q.servers * tNs);
    out.push({
      id: e.id,
      name: e.name,
      utilisation: Math.max(0, Math.min(1, u)),
      depth: q.depth,
      meanWaitMs: q.served > 0 ? q.waited_ns / q.served / 1e6 : null,
    });
  }
  return out.sort((a, b) => b.utilisation - a.utilisation || a.id.localeCompare(b.id));
}

/** Human names for latency stages and loss causes, where the engine's key is terse. */
const STAGE_NAMES: Readonly<Record<string, string>> = {
  sign_queue: "waiting to be signed",
  sign: "signing",
  handoff: "the hand-off to the radio",
  mac_aifs: "channel access (AIFS)",
  mac_backoff: "channel access (backoff)",
  mac_defer: "channel access (deferring to others)",
  airtime: "time on air",
  propagation: "propagation",
  reception: "reception",
  verify_queue: "waiting to be verified",
  verify: "verification",
};

export function stageName(key: string): string {
  return STAGE_NAMES[key] ?? key.replace(/_/g, " ");
}

const CAUSE_NAMES: Readonly<Record<string, string>> = {
  "out-of-range": "out of range",
  "below-sensitivity": "weak signal (below sensitivity)",
  collision: "collisions (two senders at once)",
  "preamble-missed": "missed preambles",
  "half-duplex": "half duplex (the receiver was transmitting)",
  "hidden-terminal": "hidden terminals",
  jammed: "jamming",
  fading: "fading",
  "in-band-emission": "in-band emission from nearby senders",
  "resource-collision": "sidelink resource collisions",
  "adjacent-channel": "adjacent-channel interference",
};

export function causeName(key: string): string {
  return CAUSE_NAMES[key] ?? key.replace(/[_-]/g, " ");
}
