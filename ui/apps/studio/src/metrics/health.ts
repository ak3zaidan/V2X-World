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

import { formatNumber, formatWithUnit, labelOf, type Series } from "./model.js";

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
  const j = band.judged;
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

/** The thresholds in words, for a tooltip: "healthy ≤ 0.5 · caution ≤ 0.68 · unhealthy above". */
export function thresholdsText(band: HealthBand, unit: string): string {
  const u = unit === "ratio" || unit === "1" || unit === "-" ? "" : ` ${unit}`;
  if (band.better === "lower") {
    return `healthy ≤ ${formatNumber(band.caution)}${u} · caution ≤ ${formatNumber(band.unhealthy)}${u} · unhealthy above`;
  }
  return `healthy ≥ ${formatNumber(band.caution)}${u} · caution ≥ ${formatNumber(band.unhealthy)}${u} · unhealthy below`;
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
  /** The mean of the windows that reported a value. */
  readonly mean: number;
  /** The level of that mean. */
  readonly level: HealthLevel;
  /** The worst window's value and its start, in simulated seconds. */
  readonly worst: number;
  readonly worstAtS: number;
  /** The worst window's level, which can be worse than the mean's: a spike. */
  readonly worstLevel: HealthLevel;
  /** Windows reported, and the shares of them in caution and unhealthy. */
  readonly windows: number;
  readonly cautionShare: number;
  readonly unhealthyShare: number;
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
export function judge(base: string, unit: string, band: HealthBand, series: Series, label = labelOf(base)): Judgement | null {
  let n = 0;
  let sum = 0;
  let caution = 0;
  let unhealthy = 0;
  let worst = Number.NaN;
  let worstAt = 0;
  let outFrom: number | null = null;
  let outTo: number | null = null;
  const worse = (a: number, b: number): boolean => (band.better === "lower" ? a > b : a < b);
  for (let i = 0; i < series.v.length; i++) {
    const v = series.v[i];
    if (!Number.isFinite(v)) continue;
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
  const mean = sum / n;
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
    worst,
    worstAtS: worstAt,
    worstLevel,
    windows: n,
    cautionShare,
    unhealthyShare,
    outFromS: outFrom,
    outToS: outTo,
    severity,
  };
}

/** The judgements outside the healthy band (by mean or by a window), worst first; ties by name. */
export function outOfBand(all: readonly Judgement[]): Judgement[] {
  return all
    .filter((j) => j.level !== "healthy" || j.worstLevel !== "healthy")
    .sort((a, b) => b.severity - a.severity || a.base.localeCompare(b.base));
}

/** "unhealthy 34 % of the run, worst 0.81 at 42 s" — when and how bad, in one line. */
export function whenText(j: Judgement): string {
  const parts: string[] = [];
  if (j.unhealthyShare > 0) parts.push(`unhealthy ${pct(j.unhealthyShare)} of the run`);
  if (j.cautionShare > 0) parts.push(`caution ${pct(j.cautionShare)}`);
  parts.push(`worst ${formatWithUnit(j.worst, j.unit)} at ${formatNumber(j.worstAtS)} s`);
  if (j.outFromS !== null && j.outToS !== null && j.outToS > j.outFromS) {
    parts.push(`out of band from ${formatNumber(j.outFromS)} s to ${formatNumber(j.outToS)} s`);
  }
  return parts.join(", ");
}

// ------------------------------------------------------------------------------------------------
// The expanded view's one-line verdict
// ------------------------------------------------------------------------------------------------

/**
 * Where a value sits against its band, in a few words: "Above 600 ms limit", "Below 0.95 target",
 * "Within 500 ms target". The threshold named is the one that decides the level.
 */
export function limitText(band: HealthBand, level: HealthLevel, unit: string): string {
  const u = unit === "ratio" || unit === "1" || unit === "-" || unit === "" ? "" : ` ${unit}`;
  const lower = band.better === "lower";
  if (level === "healthy") return `Within ${formatNumber(band.caution)}${u} target`;
  if (level === "caution") return `${lower ? "Above" : "Below"} ${formatNumber(band.caution)}${u} target`;
  return `${lower ? "Above" : "Below"} ${formatNumber(band.unhealthy)}${u} limit`;
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
  parts.push(`worst ${formatWithUnit(j.worst, unit)} at ${formatNumber(j.worstAtS)} s`);
  return parts;
}

function pct(x: number): string {
  const p = x * 100;
  return p > 0 && p < 1 ? "<1 %" : `${Math.round(p)} %`;
}
