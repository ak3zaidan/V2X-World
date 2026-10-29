/**
 * Health bands, as the page uses them: is this value healthy for a real deployment, and why.
 *
 * The bands themselves are the engine's (`crates/v2xw-metrics/src/health.rs`), published with each
 * metric in the catalogue, so a card, a chart, the run summary and the run analyst judge against
 * the same numbers and cite the same sources. Nothing here invents a threshold; a metric with no
 * band is shown without a judgement.
 *
 * Pure: no DOM, no engine, no store, so `test/metrics-health.test.ts` pins it in Node.
 */

import type { MetricHealthBand } from "@vwp/protocol";

import { clockText, formatWithUnit, labelOf, readable, type Series } from "./model.js";

export type HealthBand = MetricHealthBand;
export type HealthLevel = "healthy" | "caution" | "unhealthy";

/** Reads a band out of a catalogue row, or `null` when it is absent or malformed. */
export function bandOf(raw: unknown): HealthBand | null {
  if (typeof raw !== "object" || raw === null) return null;
  const b = raw as Partial<HealthBand>;
  if (b.better !== "lower" && b.better !== "higher") return null;
  if (typeof b.caution !== "number" || typeof b.unhealthy !== "number") return null;
  if (!Number.isFinite(b.caution) || !Number.isFinite(b.unhealthy)) return null;
  return {
    better: b.better,
    judged: typeof b.judged === "string" ? b.judged : "",
    caution: b.caution,
    unhealthy: b.unhealthy,
    healthy_note: b.healthy_note ?? "",
    caution_note: b.caution_note ?? "",
    unhealthy_note: b.unhealthy_note ?? "",
    source: b.source ?? "",
    basis: b.basis === "standard" || b.basis === "literature" ? b.basis : "engineering",
  };
}

/** The level of a value; `null` for a missing or non-finite one, which is not judged. Matches `HealthBand::classify`. */
export function classify(band: HealthBand, v: number | null | undefined): HealthLevel | null {
  if (v === null || v === undefined || !Number.isFinite(v)) return null;
  if (band.better === "lower") {
    if (v > band.unhealthy) return "unhealthy";
    if (v > band.caution) return "caution";
    return "healthy";
  }
  if (v < band.unhealthy) return "unhealthy";
  if (v < band.caution) return "caution";
  return "healthy";
}

export function noteOf(band: HealthBand, level: HealthLevel): string {
  return level === "healthy" ? band.healthy_note : level === "caution" ? band.caution_note : band.unhealthy_note;
}

/** The series a band judges: the metric's name plus the band's suffix. */
export function judgedSeries(base: string, band: HealthBand): string {
  return `${base}${band.judged}`;
}

/** How the judged statistic is named to a reader: "p99", "within 100 m", or "" for the headline. */
export function judgedLabel(band: HealthBand): string {
  return statLabel(band.judged);
}

/** The same, from a band's `judged` suffix alone (".p99", "[100m]", ""). */
export function statLabel(j: string): string {
  if (j === "") return "";
  if (j.startsWith(".")) return j.slice(1);
  const m = /^\[(\d+)m\]$/.exec(j);
  if (m) return `within ${m[1]} m`;
  return j.replace(/^\[|\]$/g, "");
}

export const LEVEL_LABEL: Readonly<Record<HealthLevel, string>> = {
  healthy: "Healthy",
  caution: "Caution",
  unhealthy: "Unhealthy",
};

export const BASIS_LABEL: Readonly<Record<HealthBand["basis"], string>> = {
  standard: "Set by a standard",
  literature: "A convention in the literature",
  engineering: "An engineering choice (no standard sets it)",
};

/** "0.71 — above the 0.68 ETSI congestion target; vehicles will throttle their messages". */
export function verdictLine(band: HealthBand, v: number, unit: string): string {
  const level = classify(band, v);
  if (level === null) return "";
  return `${formatWithUnit(v, unit)} — ${noteOf(band, level)}`;
}

/** A threshold or a judged value as a reader compares it: a share as a percentage ("68 %"), "600 ms". */
export function bandValue(v: number, unit: string): string {
  const s = readable(v, unit === "-" ? "" : unit);
  return s.unit === "" ? s.value : `${s.value} ${s.unit}`;
}

/** The thresholds in words, for a tooltip: "healthy ≤ 50 % · caution ≤ 68 % · unhealthy above". */
export function thresholdsText(band: HealthBand, unit: string): string {
  const t = (v: number): string => bandValue(v, unit);
  if (band.better === "lower") {
    return `healthy ≤ ${t(band.caution)} · caution ≤ ${t(band.unhealthy)} · unhealthy above`;
  }
  return `healthy ≥ ${t(band.caution)} · caution ≥ ${t(band.unhealthy)} · unhealthy below`;
}

// ------------------------------------------------------------------------------------------------
// The run's own expectation
// ------------------------------------------------------------------------------------------------

/**
 * The share of vehicles the scenario gives a cellular modem (`net.uu.params.penetration`), when it
 * is below 1; otherwise `null`. A run that leaves vehicles without a modem on purpose (to exercise
 * the roadside relay) expects that many to be out of the backend's reach at times.
 */
export function cellularPenetration(scenario: unknown): number | null {
  const p = (scenario as { net?: { uu?: { params?: { penetration?: unknown } } } } | null)?.net?.uu?.params?.penetration;
  return typeof p === "number" && Number.isFinite(p) && p >= 0 && p < 1 ? p : null;
}

/**
 * The pseudonym calendar a scenario sets (`security.protocol.params`): certificates valid at once
 * per period (`certs_per_period`), the period's length (`i_period_s`) and how many periods a vehicle
 * holds (`pool_periods`). Each part `null` when the scenario leaves it to the protocol's default.
 */
export interface PseudonymCalendar {
  readonly perPeriod: number | null;
  readonly periodS: number | null;
  readonly periodsHeld: number | null;
}

/** The deployment's issue per period (CAMP SCMS: 20 a week, Whyte et al., IEEE VNC 2013), which the engine's pool band assumes. */
export const DEPLOYMENT_CERTS_PER_PERIOD = 20;

function posNum(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) && v > 0 ? v : null;
}

export function pseudonymCalendar(scenario: unknown): PseudonymCalendar {
  const p = (scenario as { security?: { protocol?: { params?: Record<string, unknown> } } } | null)?.security?.protocol?.params ?? {};
  return { perPeriod: posNum(p["certs_per_period"]), periodS: posNum(p["i_period_s"]), periodsHeld: posNum(p["pool_periods"]) };
}

/** The scenario's certificates per period, when it issues fewer than the deployment's 20 (else `null`: the engine's band stands). */
export function scaledPool(scenario: unknown): number | null {
  const n = pseudonymCalendar(scenario).perPeriod;
  return n !== null && n < DEPLOYMENT_CERTS_PER_PERIOD ? n : null;
}

/**
 * The pool band scaled to the scenario's issue. The engine's band (caution under 5, unhealthy
 * under 1) assumes the deployment's 20 certificates a period; a scenario that issues 5 a period
 * holds 5 when its pool is full, so the caution line moves with the issue (a quarter of it, as 5 is
 * of 20) and the unhealthy line (cannot sign) stays at 1.
 */
function poolBand(band: HealthBand, scenario: unknown): HealthBand {
  const n = scaledPool(scenario);
  if (n === null || band.better !== "higher") return band;
  const caution = Math.max(band.unhealthy, (band.caution * n) / DEPLOYMENT_CERTS_PER_PERIOD);
  const per = `${n} a period`;
  return {
    ...band,
    caution,
    unhealthy: Math.min(band.unhealthy, caution),
    healthy_note: `vehicles hold most of the ${per} this scenario issues`,
    caution_note: `vehicles are down to under a quarter of the ${per} this scenario issues`,
    unhealthy_note: band.unhealthy_note,
    source: `${band.source}; scaled to this scenario's ${per} (security.protocol.params certs_per_period ${n}, against the deployment's ${DEPLOYMENT_CERTS_PER_PERIOD}), so a full pool here reads about ${n}`,
  };
}

/**
 * A band as this run should be judged against it. The engine's bands assume every vehicle is
 * equipped; where the scenario sets something below that on purpose, the band is scaled to the
 * scenario's own expectation and says so, so a deliberate choice is not reported as a fault.
 *
 * Two bands: `backend_link_up` (the share of vehicles that can reach the credential backend)
 * scaled by the scenario's cellular penetration, and `cert_pool_valid` scaled by the scenario's
 * certificates per period (`poolBand`). A vehicle without a modem can still reach the
 * backend through a relaying roadside unit, so the run should do at least as well as its penetration.
 */
export function expectedBand(base: string, band: HealthBand, scenario: unknown): HealthBand {
  if (base === "cert_pool_valid") return poolBand(band, scenario);
  if (base !== "backend_link_up") return band;
  const p = cellularPenetration(scenario);
  if (p === null || band.better !== "higher") return band;
  const pct = `${Math.round(p * 100)} %`;
  return {
    ...band,
    caution: band.caution * p,
    unhealthy: band.unhealthy * p,
    healthy_note: `at least the ${pct} of vehicles the scenario gives a cellular modem can reach the backend`,
    caution_note: `slightly fewer vehicles reach the backend than the ${pct} with a cellular modem`,
    unhealthy_note: `well under the ${pct} of vehicles with a cellular modem can reach the backend`,
    source: `${band.source}; scaled to this scenario's cellular penetration of ${pct} (net.uu penetration ${p}), since the run leaves the rest without a modem on purpose`,
  };
}

/** Every family's band as this run should be judged against it (`expectedBand`). */
export function withExpectations<T extends { readonly base: string; readonly health: HealthBand | null }>(families: readonly T[], scenario: unknown): T[] {
  if (cellularPenetration(scenario) === null && scaledPool(scenario) === null) return [...families];
  return families.map((f) => (f.health === null ? f : { ...f, health: expectedBand(f.base, f.health, scenario) }));
}

// ------------------------------------------------------------------------------------------------
// A run's verdict on one metric
// ------------------------------------------------------------------------------------------------

/** One banded metric judged over a run. */
export interface Judgement {
  readonly base: string;
  readonly label: string;
  readonly unit: string;
  readonly band: HealthBand;
  /** The judged series' name. */
  readonly series: string;
  /** The mean of the windows that reported a value; for a cumulative metric, its last window (the whole run's figure). */
  readonly mean: number;
  /** The level of that mean. */
  readonly level: HealthLevel;
  /** The metric accumulates over the run (`linkability`): its last window is the run's value, and a mean of its windows would mean nothing. */
  readonly cumulative: boolean;
  /** The worst window's value and its start, in simulated seconds. */
  readonly worst: number;
  readonly worstAtS: number;
  /** The worst window's level, which can be worse than the mean's: a spike. */
  readonly worstLevel: HealthLevel;
  /** Windows reported, and the shares of them in caution and unhealthy. */
  readonly windows: number;
  readonly cautionShare: number;
  readonly unhealthyShare: number;
  /** The first and last window that reported, in simulated seconds. */
  readonly firstS: number;
  readonly lastS: number;
  /** The first and last window outside the healthy band, in simulated seconds, or null. */
  readonly outFromS: number | null;
  readonly outToS: number | null;
  /** 0 healthy; larger is worse. Orders the run summary's list. */
  readonly severity: number;
}

const RANK: Readonly<Record<HealthLevel, number>> = { healthy: 0, caution: 1, unhealthy: 2 };

/** The worse of two levels. */
export function worseLevel(a: HealthLevel, b: HealthLevel): HealthLevel {
  return RANK[a] >= RANK[b] ? a : b;
}

/**
 * Judges one metric's series over a run: its mean level, its worst window, how much of the run it
 * spent outside the healthy band, and a severity that ranks it.
 *
 * Severity: the mean's level counts most (2 for unhealthy, 1 for caution), then the share of the
 * run spent unhealthy, then caution; a run whose mean is healthy but had unhealthy windows still
 * ranks above a fully healthy one. `null` when the series has no observed window.
 */
export function judge(base: string, unit: string, band: HealthBand, series: Series, label = labelOf(base), cumulative = false): Judgement | null {
  let last = Number.NaN;
  let n = 0;
  let sum = 0;
  let caution = 0;
  let unhealthy = 0;
  let worst = Number.NaN;
  let worstAt = 0;
  let outFrom: number | null = null;
  let outTo: number | null = null;
  let firstS = 0;
  let lastS = 0;
  const worse = (a: number, b: number): boolean => (band.better === "lower" ? a > b : a < b);
  for (let i = 0; i < series.v.length; i++) {
    const v = series.v[i];
    if (!Number.isFinite(v)) continue;
    if (n === 0) firstS = series.t[i];
    lastS = series.t[i];
    last = v;
    n++;
    sum += v;
    const lv = classify(band, v);
    if (lv === "caution") caution++;
    if (lv === "unhealthy") unhealthy++;
    if (lv !== "healthy") {
      if (outFrom === null) outFrom = series.t[i];
      outTo = series.t[i];
    }
    if (Number.isNaN(worst) || worse(v, worst)) {
      worst = v;
      worstAt = series.t[i];
    }
  }
  if (n === 0) return null;
  const mean = cumulative ? last : sum / n;
  const level = classify(band, mean) ?? "healthy";
  const worstLevel = classify(band, worst) ?? "healthy";
  const cautionShare = caution / n;
  const unhealthyShare = unhealthy / n;
  const severity = RANK[level] * 10 + unhealthyShare * 4 + cautionShare + (RANK[worstLevel] > 0 ? 0.5 * RANK[worstLevel] : 0);
  return {
    base,
    label,
    unit,
    band,
    series: judgedSeries(base, band),
    mean,
    level,
    cumulative,
    worst,
    worstAtS: worstAt,
    worstLevel,
    firstS,
    lastS,
    windows: n,
    cautionShare,
    unhealthyShare,
    outFromS: outFrom,
    outToS: outTo,
    severity,
  };
}

/** Whether a metric's definition says it accumulates over the run, so its newest window is the run's value. */
export function isCumulative(definition: string): boolean {
  return /cumulative over the run|over the run so far|since the run (began|started)/i.test(definition);
}

/** The judgements outside the healthy band (by mean or by a window), worst first; ties by name. */
export function outOfBand(all: readonly Judgement[]): Judgement[] {
  return all
    .filter((j) => j.level !== "healthy" || j.worstLevel !== "healthy")
    .sort((a, b) => b.severity - a.severity || a.base.localeCompare(b.base));
}

/**
 * "unhealthy 34 % of the windows, worst 81 % at 42 s" — when and how bad, in one line. With the
 * run's length, a metric that reported in only part of the run says so: a fast-forwarded run has
 * radio windows only while the radio ran, and "from 296 s to 592 s" on a two-day run needs that.
 */
export function whenText(j: Judgement, runS?: number): string {
  const parts: string[] = [];
  if (j.unhealthyShare > 0) parts.push(`unhealthy ${pct(j.unhealthyShare)} of the windows`);
  if (j.cautionShare > 0) parts.push(`caution ${pct(j.cautionShare)}`);
  parts.push(`worst ${bandValue(j.worst, j.unit)} at ${clockText(j.worstAtS)}`);
  if (j.outFromS !== null && j.outToS !== null && j.outToS > j.outFromS) {
    parts.push(`out of band from ${clockText(j.outFromS)} to ${clockText(j.outToS)}`);
  }
  const span = j.lastS - j.firstS;
  if (runS !== undefined && runS > 0 && span < runS * 0.5) {
    parts.push(
      `(its ${j.windows.toLocaleString("en-US")} ${j.windows === 1 ? "window" : "windows"} all fall between ${clockText(j.firstS)} and ${clockText(j.lastS)} of a ${clockText(runS)} run: the rest reported no value)`,
    );
  }
  return parts.join(", ");
}

// ------------------------------------------------------------------------------------------------
// The expanded view's one-line verdict
// ------------------------------------------------------------------------------------------------

/**
 * Where a value sits against its band, in a few words: "Above 600 ms limit", "Below 0.95 target",
 * "Below 500 ms target" (healthy). The threshold named is the one that decides the level; a value
 * exactly on it counts as the better level, as `classify` does.
 */
export function limitText(band: HealthBand, level: HealthLevel, unit: string): string {
  const t = (v: number): string => bandValue(v, unit);
  const lower = band.better === "lower";
  if (level === "healthy") return `${lower ? "Below" : "Above"} ${t(band.caution)} target`;
  if (level === "caution") return `${lower ? "Above" : "Below"} ${t(band.caution)} target`;
  return `${lower ? "Above" : "Below"} ${t(band.unhealthy)} limit`;
}

/**
 * The verdict under the expanded view's value, as short parts a reader scans:
 * ["Above 600 ms limit", "unhealthy 83 % of run", "worst 834 ms at 17 s"]. The first part is the
 * newest value's level (`level`); the rest are the run's, from its judgement.
 */
export function verdictParts(band: HealthBand, level: HealthLevel, unit: string, j: Judgement | null): string[] {
  const parts = [limitText(band, level, unit)];
  if (j === null) return parts;
  if (j.unhealthyShare > 0) parts.push(`unhealthy ${pct(j.unhealthyShare)} of run`);
  if (j.cautionShare > 0) parts.push(`caution ${pct(j.cautionShare)}`);
  if (j.unhealthyShare === 0 && j.cautionShare === 0) parts.push(`healthy in all ${j.windows.toLocaleString("en-US")} windows`);
  parts.push(`worst ${bandValue(j.worst, unit)} at ${clockText(j.worstAtS)}`);
  return parts;
}

function pct(x: number): string {
  const p = x * 100;
  return p > 0 && p < 1 ? "<1 %" : `${Math.round(p)} %`;
}
