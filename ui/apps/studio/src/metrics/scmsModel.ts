/**
 * The credential system at a glance: the SCMS (or CCMS) part of the run, answered as six
 * questions a researcher asks, each with a one-sentence answer and a few numbers.
 *
 *  1. Are devices getting certificates?
 *  2. Is revocation working, and how fast?
 *  3. Is misbehaviour being caught, and are honest devices affected?
 *  4. Can a vehicle be tracked across pseudonym changes?
 *  5. What does security cost the radio and the CPU?
 *  6. Can vehicles reach the backend, and is it keeping up?
 *
 * Two sources, both the engine's: the metric catalogue (`metrics.query`, a series over time, with
 * the published health band where one exists) and the Backend view's snapshot (`inspect.entity
 * {entity: "backend"}`, each authority's own running counts). Nothing is computed here that the
 * engine did not count; this module only adds counts across providers, divides where the meaning is
 * a share, and words it. A good or bad cue appears only where a published band or the run's own
 * expectation (an honest device revoked, a vehicle with no certificate to sign with) gives one, and
 * the cue's reason is always stated beside it.
 *
 * Pure: no DOM, no engine, no store, so `test/metrics-scms.test.ts` pins it in Node.
 */

import type { BackendEntity, BackendSnapshot } from "../lib/backend.js";
import { classify, noteOf, type HealthBand, type HealthLevel } from "./health.js";
import { display, formatNumber, glanceNumber, readable, type GroupRow, type MetricFamily, type Series } from "./model.js";

import { entityLoads, ENTITY_UTILISATION_LIMIT } from "./summaryModel.js";

export { readable as shown, readableBytes as bytes, readableDuration as duration } from "./model.js";

// ------------------------------------------------------------------------------------------------
// Inputs
// ------------------------------------------------------------------------------------------------

/** What the overview knows about the run's metrics. */
export interface MetricReader {
  /** The family of a base name, when the run measures it. */
  family(base: string): MetricFamily | undefined;
  /** A series' value in its newest window, or `null`. */
  latest(series: string): number | null;
  /** A series across the run so far, for a sparkline. */
  series(series: string): Series | undefined;
  /** A metric pooled by the engine over the whole run, grouped by one dimension (`det_recall` by `level`). */
  pooled(metric: string, dim: string): readonly GroupRow[] | undefined;
}

/** The run-wide groupings the overview asks the engine for, when the run measures the metric. */
export const POOLED: readonly { readonly metric: string; readonly dim: string }[] = [
  { metric: "det_recall", dim: "level" },
  { metric: "det_precision", dim: "level" },
  { metric: "det_fpr", dim: "level" },
  { metric: "false_accusations", dim: "level" },
  { metric: "revocation_latency_stage", dim: "stage" },
  { metric: "loss_rate", dim: "cause" },
];

/** The series the overview's sparklines and numbers read, beyond the cards' own headlines. */
export const OVERVIEW_SERIES: readonly string[] = [
  "cert_pool_valid",
  "backend_link_up",
  "crl_entries",
  "crl_bytes",
  "time_to_decision",
  "time_to_detect",
  "revocation_latency_stage.p95",
  "security_overhead",
  "envelope_overhead",
  "full_cert_share",
  "verify_rate",
  "verify_cost",
  "verify_wait.p95",
  "verify_queue_depth",
  "unverified_ratio",
  "bytes_backend",
  "digest_misses",
  "pseudonym_change_rate",
  "linkability",
];

// ------------------------------------------------------------------------------------------------
// Outputs
// ------------------------------------------------------------------------------------------------

/** A cue's level, or a fact with no verdict. */
export type Tone = HealthLevel | "info";

export interface ScmsTile {
  /** A stable key: the test id is `scms-tile-<id>`. */
  readonly id: string;
  /** Plain English, with the standard term beside it: "Valid pseudonym certificates (PCs) per vehicle". */
  readonly title: string;
  /** The number as shown ("40", "21 of 23", "97.4"). */
  readonly value: string;
  /** Its unit as shown after it ("%", "s", "devices"), or "". */
  readonly unit: string;
  /** What the number means, in one line. */
  readonly meaning: string;
  /** A cue only where a band or the run's expectation gives one. */
  readonly level: HealthLevel | null;
  /** Why the cue: the threshold and its source. Present whenever `level` is. */
  readonly cue?: string;
  /** The metric this tile opens, when it is a catalogue metric. */
  readonly metric?: string;
  /** The series its sparkline draws. */
  readonly spark?: string;
  /** Where the number comes from, for the tooltip: a metric name or the authority and its count. */
  readonly source: string;
  /** The same count per provider, when more than one provider runs. */
  readonly split?: string;
  /**
   * Set instead of a cue when the number cannot be judged yet because the pipeline behind it is
   * still working by design (reports held in the RA's shuffle): what is pending, and when it clears.
   */
  readonly pending?: string;
}

/** One step of a funnel: how many reports reached it. */
export interface FunnelStep {
  readonly id: string;
  readonly label: string;
  readonly value: number;
  readonly hint: string;
  /** What happened to the reports lost between the step before and this one ("132 not uploaded yet"). */
  readonly drop?: string;
}

/** A labelled bar: a revocation stage's time, a security check's rejections, an authority's load. */
export interface ScmsBar {
  readonly key: string;
  readonly label: string;
  readonly value: number;
  readonly display: string;
  readonly level: HealthLevel | null;
  readonly hint?: string;
}

export interface ScmsBars {
  readonly title: string;
  readonly note: string;
  /** The value a full bar stands for (1 for a share, the largest value otherwise). */
  readonly max: number;
  /** A threshold line on the bars, as a share of `max`, with its reason. */
  readonly limit?: { readonly at: number; readonly label: string };
  readonly rows: readonly ScmsBar[];
  /** The metric the bars break down, for the "open the chart" link. */
  readonly metric?: string;
  /** Said when there is nothing to draw. */
  readonly empty: string;
  /** Said instead of the bars when every bar would be too small to read (every authority idle). */
  readonly idle?: string;
}

export type SectionId = "certs" | "revocation" | "misbehaviour" | "privacy" | "cost" | "backend";

export interface ScmsSection {
  readonly id: SectionId;
  /** The question, as the heading. */
  readonly question: string;
  /** The answer, in one sentence. */
  readonly answer: string;
  /** The worst cue among its tiles and bars, or `null` when none is judged. */
  readonly level: HealthLevel | null;
  /** True when a tile could not be judged yet because its pipeline is still working (see `ScmsTile.pending`). */
  readonly pending?: boolean;
  readonly tiles: readonly ScmsTile[];
  readonly funnel?: readonly FunnelStep[];
  readonly bars?: ScmsBars;
}

export interface ScmsOverview {
  /** "US SCMS (CAMP design)", "EU CCMS". */
  readonly system: string;
  /** "2 providers · ECDSA P-256 signatures". */
  readonly facts: string;
  readonly providers: readonly string[];
  readonly sections: readonly ScmsSection[];
  /** Every authority's counts, for the raw table. */
  readonly raw: readonly { readonly id: string; readonly name: string; readonly counts: readonly (readonly [string, string])[] }[];
}

// ------------------------------------------------------------------------------------------------
// Reading the snapshot
// ------------------------------------------------------------------------------------------------

/** The role an entity plays, without its provider prefix: `provider-b/ra` → `ra`. */
export function roleOf(id: string): string {
  const i = id.lastIndexOf("/");
  return i < 0 ? id : id.slice(i + 1);
}

/** The provider an entity belongs to: its id's prefix, its own `provider` count, or the first provider. */
export function providerOf(e: Pick<BackendEntity, "id" | "state">, first: string): string {
  const i = e.id.lastIndexOf("/");
  if (i > 0) return e.id.slice(0, i);
  const p = e.state["provider"];
  return typeof p === "string" && p !== "" ? p : first;
}

/** The scenario's first provider's name, or "provider 1". */
export function firstProvider(scenario: unknown, s: BackendSnapshot | null = null): string {
  const list = (scenario as { security?: { scms?: { deployment?: { providers?: unknown } } } } | null)?.security?.scms?.deployment?.providers;
  if (Array.isArray(list) && list.length > 0) {
    const n = (list[0] as { name?: unknown }).name;
    if (typeof n === "string" && n !== "") return n;
  }
  // The engine names the other providers by prefix ("provider-b/ra"); the home one has none.
  // Beside a "provider-b", the home provider is the deployment's "provider-a".
  for (const e of s?.entities ?? []) {
    const m = /^(.*)-b\//.exec(e.id);
    if (m) return `${m[1]}-a`;
  }
  return "home provider";
}

function num(v: unknown): number | null {
  return typeof v === "number" && Number.isFinite(v) ? v : null;
}

/** A count summed over every provider's entity of these roles, by the first key each one has. */
export interface Count {
  readonly total: number;
  readonly byProvider: readonly (readonly [string, number])[];
}

export function countOf(s: BackendSnapshot | null, roles: readonly string[], keys: readonly string[], first: string): Count | null {
  if (s === null) return null;
  let total = 0;
  let any = false;
  const by = new Map<string, number>();
  for (const e of s.entities) {
    if (!roles.includes(roleOf(e.id))) continue;
    let v: number | null = null;
    for (const k of keys) {
      v = num(e.state[k]);
      if (v !== null) break;
    }
    if (v === null) continue;
    any = true;
    total += v;
    const p = providerOf(e, first);
    by.set(p, (by.get(p) ?? 0) + v);
  }
  return any ? { total, byProvider: [...by] } : null;
}

/** The first numeric value of a key on an entity of a role (a cadence, a latency the devices report). */
function firstOf(s: BackendSnapshot | null, roles: readonly string[], key: string): number | null {
  if (s === null) return null;
  for (const e of s.entities) if (roles.includes(roleOf(e.id))) {
    const v = num(e.state[key]);
    if (v !== null) return v;
  }
  return null;
}

/** "provider-a 16 · provider-b 7", only when more than one provider counted. */
function splitText(c: Count | null): string | undefined {
  if (c === null || c.byProvider.length < 2) return undefined;
  return c.byProvider.map(([p, v]) => `${p} ${formatNumber(v)}`).join(" · ");
}

// ------------------------------------------------------------------------------------------------
// Words for numbers
// ------------------------------------------------------------------------------------------------

function cap(t: string): string {
  return t.length === 0 ? t : t[0]!.toUpperCase() + t.slice(1);
}

function words(v: number | null, unit: string): string {
  const s = readable(v, unit);
  return s.unit === "" ? s.value : s.unit === "%" ? `${s.value} %` : `${s.value} ${s.unit}`;
}

/**
 * A small share as one comparable figure: one decimal of a percentage ("1.1 %", "0.5 %"), "< 0.1 %"
 * below that, so a list of rejection rates never mixes "1.1 %" with "0.498 %".
 */
export function sharePct(v: number): string {
  const p = v * 100;
  if (p > 0 && p < 0.1) return "< 0.1 %";
  return `${(Math.round(p * 10) / 10).toLocaleString("en-US", { minimumFractionDigits: p < 10 ? 1 : 0, maximumFractionDigits: 1 })} %`;
}

/** "All completed.", "7 completed, 2 in flight.", "8 completed, 1 refused." */
export function topupOutcome(started: number, completed: number, inFlight: number, refused: number): string {
  if (started === 0) return "None needed yet.";
  if (completed >= started) return started === 1 ? "It completed." : "All completed.";
  const parts = [`${formatNumber(completed)} completed`];
  if (inFlight > 0) parts.push(`${formatNumber(inFlight)} in flight`);
  if (refused > 0) parts.push(`${formatNumber(refused)} refused`);
  return `${parts.join(", ")}.`;
}

/** "healthy from 5, unhealthy below 1 (an engineering choice …)", for a band's tooltip. */
export function cueText(band: HealthBand, level: HealthLevel, unit: string): string {
  const t = (v: number): string => words(v, unit);
  const range =
    band.better === "lower"
      ? `healthy up to ${t(band.caution)}, unhealthy above ${t(band.unhealthy)}`
      : `healthy from ${t(band.caution)}, unhealthy below ${t(band.unhealthy)}`;
  const basis = band.basis === "standard" ? "set by a standard" : band.basis === "literature" ? "a convention in the literature" : "an engineering choice, no standard sets it";
  return `${noteOf(band, level)}. Thresholds: ${range} (${basis}). Source: ${band.source || "the engine's health table"}.`;
}

/** The worse of two levels; `null` is no verdict. */
export function worst(levels: readonly (HealthLevel | null)[]): HealthLevel | null {
  let w: HealthLevel | null = null;
  for (const l of levels) {
    if (l === null) continue;
    if (w === null || rank(l) > rank(w)) w = l;
  }
  return w;
}

function rank(l: HealthLevel): number {
  return l === "unhealthy" ? 2 : l === "caution" ? 1 : 0;
}

// ------------------------------------------------------------------------------------------------
// Tiles
// ------------------------------------------------------------------------------------------------

/** A tile for a catalogue metric: its newest window (or the judged statistic), its band's cue. */
function metricTile(
  m: MetricReader,
  base: string,
  spec: { id?: string; title: string; meaning: string; series?: string; spark?: string | null; mean?: boolean; unit?: string },
): ScmsTile | null {
  const f = m.family(base);
  if (f === undefined) return null;
  const series = spec.series ?? f.headline ?? base;
  // A bursty rate (backend bytes, verifications) is read as its mean over the windows that
  // reported: its newest window is often an idle one between bursts.
  const v = spec.mean ? meanOf(m.series(series)) ?? m.latest(series) : m.latest(series);
  if (v === null) return null;
  const unit = f.unit;
  // The same words as the cards and the run summary: a share in %, a wait under 1 ms in µs, a
  // ratio that can pass 1 as a multiple ("1.26 × payload").
  const r = display(base, v, unit);
  const s = { value: r.value, unit: spec.unit ?? r.unit };
  const band = f.health;
  const judges = band !== null && `${base}${band.judged}` === series;
  const level = judges && band ? classify(band, v) : null;
  return {
    id: spec.id ?? base,
    title: spec.title,
    value: s.value,
    unit: s.unit,
    meaning: spec.meaning,
    level,
    ...(level !== null && band ? { cue: cueText(band, level, unit) } : {}),
    metric: base,
    ...(spec.spark === null ? {} : { spark: spec.spark ?? series }),
    source: `Metric ${series}, ${spec.mean ? "mean of the windows that reported" : "newest window"}. ${f.definition}`.trim(),
  };
}

/** The same tile with its cue set aside: the number cannot be judged yet, and `text` says why. */
function asPending(t: ScmsTile, text: string): ScmsTile {
  const out: { -readonly [K in keyof ScmsTile]: ScmsTile[K] } = { ...t, level: null, pending: text };
  delete out.cue;
  return out;
}

/** A tile for an authority's own count. */
function countTile(
  c: Count | null,
  spec: { id: string; title: string; unit: string; meaning: string; source: string; level?: HealthLevel | null; cue?: string; value?: string; metric?: string },
): ScmsTile | null {
  if (c === null) return null;
  return {
    id: spec.id,
    title: spec.title,
    value: spec.value ?? formatNumber(c.total),
    unit: spec.unit,
    meaning: spec.meaning,
    level: spec.level ?? null,
    ...(spec.level != null && spec.cue ? { cue: spec.cue } : {}),
    source: spec.source,
    ...(spec.metric !== undefined ? { metric: spec.metric } : {}),
    ...(splitText(c) ? { split: splitText(c) } : {}),
  };
}

/** The mean of a series' observed windows, or `null`. */
export function meanOf(s: Series | undefined): number | null {
  if (s === undefined) return null;
  let sum = 0;
  let n = 0;
  for (const v of s.v) {
    if (!Number.isFinite(v)) continue;
    sum += v;
    n++;
  }
  return n === 0 ? null : sum / n;
}

function present<T>(xs: readonly (T | null | undefined)[]): T[] {
  return xs.filter((x): x is T => x !== null && x !== undefined);
}

/** The pooled value of one group, or `null`. */
function pooledValue(m: MetricReader, metric: string, dim: string, key: string): number | null {
  const rows = m.pooled(metric, dim);
  const r = rows?.find((x) => x.key === key);
  return r && r.value !== null && Number.isFinite(r.value) ? r.value : null;
}

// ------------------------------------------------------------------------------------------------
// The five questions
// ------------------------------------------------------------------------------------------------

/** Losses that are security's doing, in the engine's cause keys, with what each means. */
export const SECURITY_CAUSES: Readonly<Record<string, string>> = {
  "signature-invalid": "Invalid signature",
  revoked: "Sender revoked (on the CRL)",
  "unknown-signer": "Signer's certificate unknown",
  "verify-overflow": "Verifier queue full",
  "verify-policy-drop": "Skipped by the verification policy",
  "crl-overdue": "CRL overdue, messages not trusted",
  "generation-time-window": "Too old, or from the future",
  "psid-not-registered": "Overheard reports meant for a roadside unit (PSID)",
  "receiver-rule": "Refused by a receiver rule",
};

/** What each security loss means, for its bar's tooltip. */
const CAUSE_HINTS: Readonly<Record<string, string>> = {
  "signature-invalid": "The signature did not verify: a forged or corrupted message.",
  revoked: "The sender's certificate is on the receiver's revocation list: revocation working.",
  "unknown-signer": "The message carried only a digest of a certificate the receiver has not seen yet, so it could not be checked.",
  "verify-overflow": "The verifier's queue was full when the message arrived.",
  "verify-policy-drop": "The receiver's verification policy skipped checking this message.",
  "crl-overdue": "The receiver's revocation list is past its next-update time, so it trusts no one until it gets a new one.",
  "generation-time-window": "The message's timestamp was too old or in the future.",
  "psid-not-registered":
    "Vehicles overhearing misbehaviour reports relayed to a roadside unit (PSID 0x26). No vehicle application takes them, so IEEE 1609.3 discards them before any check. Expected wherever reports travel the roadside relay; not a security failure.",
  "receiver-rule": "A receiver rule refused the message.",
};

/** Losses that happen by design rather than because a check failed: kept apart from the headline share. */
const BY_DESIGN: ReadonlySet<string> = new Set(["psid-not-registered"]);

/** Plain names for revocation stages (`decision->issued`); unknown stages keep the engine's words. */
const STAGE_WORDS: Readonly<Record<string, string>> = {
  detect: "detected",
  report_sent: "report sent",
  report_received: "report at the MA",
  decision: "MA decides",
  resolved: "device identified (Pseudonym CA and Linkage Authorities)",
  issued: "on a signed CRL",
  published: "CRL published",
  first_rsu_broadcast: "first roadside broadcast",
  downloaded: "CRL downloaded",
  processed: "CRL processed",
  enforced: "enforced by receivers",
  residual_harm: "last harm",
  blocklisted: "blocklisted at the RA",
  last_valid_credential_expiry: "last certificate expires",
  installed: "installed",
};

/** The order of a revocation's stages (`v2xw_proto::stage::StageId`), for the bars. */
const STAGE_ORDER: readonly string[] = [
  "detect",
  "report_sent",
  "report_received",
  "decision",
  "resolved",
  "blocklisted",
  "issued",
  "published",
  "first_rsu_broadcast",
  "downloaded",
  "processed",
  "installed",
  "enforced",
  "residual_harm",
  "last_valid_credential_expiry",
];

/** Where a `from->to` transition sits in a revocation; unknown stages go last. */
export function stageRank(key: string): number {
  const [from = "", to = ""] = key.split("->");
  const f = STAGE_ORDER.indexOf(from.trim());
  const t = STAGE_ORDER.indexOf(to.trim());
  return (f < 0 ? 99 : f) * 100 + (t < 0 ? 99 : t);
}

export function stageLabel(key: string): string {
  const parts = key.split("->");
  if (parts.length !== 2) return key.replace(/[_-]/g, " ");
  const w = (p: string): string => STAGE_WORDS[p.trim()] ?? p.trim().replace(/[_-]/g, " ");
  return `${w(parts[0]!)} → ${w(parts[1]!)}`;
}

/** The tile with the worst cue, when any tile is worse than good. */
export function worstTile(tiles: readonly ScmsTile[]): ScmsTile | null {
  let w: ScmsTile | null = null;
  for (const t of tiles) {
    if (t.level === null || t.level === "healthy") continue;
    if (w === null || rank(t.level) > rank(w.level!)) w = t;
  }
  return w;
}

/** A flagged tile as the clause an answer leads with: "28 % of vehicles cannot reach the backend". */
export function flaggedText(t: ScmsTile): string {
  const n = Number(t.value.replace(/,/g, ""));
  switch (t.id) {
    case "backend_link_up":
      return Number.isFinite(n) && t.unit === "%" ? `${glanceNumber(100 - n)} % of vehicles cannot reach the backend right now` : "some vehicles cannot reach the backend";
    case "starved":
      return `${t.value} ${t.unit} ran out of valid certificates`;
    case "cert_pool_valid":
      return `vehicles are down to ${t.value} valid certificates each on average`;
    case "enrolled":
      return `only ${t.value} devices are enrolled`;
    case "topups":
      return "the RA refused some top-ups";
    default:
      return `${t.title.replace(/\s*\([^)]*\)\s*$/, "").replace(/^./, (c) => c.toLowerCase())}: ${t.value}${t.unit === "" ? "" : t.unit === "%" ? " %" : ` ${t.unit}`}`;
  }
}

/** What the RA's files_served tile means, with why a provider's count can be zero. */
function filesMeaning(files: Count | null): string {
  const base = "Newer Local Policy Files (LPF) and certificate chain files (LCCF) sent to devices whose copy was older.";
  const zero = files?.byProvider.filter(([, v]) => v === 0).map(([p]) => p) ?? [];
  return zero.length > 0 && zero.length < (files?.byProvider.length ?? 0)
    ? `${base} ${zero.join(" and ")}'s devices already held the current versions, so none was sent there.`
    : base;
}

function certsSection(m: MetricReader, s: BackendSnapshot | null, first: string): ScmsSection {
  const ccms = s?.system === "ccms";
  const devices = countOf(s, ["ee"], ["devices", "stations"], first);
  const enrolled = countOf(s, ["ee"], ["enrolled"], first);
  const issued = countOf(s, ccms ? ["aa"] : ["pca"], ccms ? ["tickets_issued"] : ["certs_issued"], first);
  const started = countOf(s, ["ee"], ["topups_started"], first);
  const completed = countOf(s, ["ee"], ["topups_completed"], first);
  const refused = countOf(s, ["ee"], ["topups_refused"], first);
  const inFlight = countOf(s, ["ee"], ["topups_in_flight"], first);
  const starved = countOf(s, ["ee"], ["vehicles_starved"], first);
  const unavailable = countOf(s, ["ee"], ["cert_unavailable_s"], first);
  const files = countOf(s, ["ra"], ["files_served"], first);
  const topupMs = firstOf(s, ["ee"], "topup_latency_ms");
  const certName = ccms ? "authorisation tickets (ATs)" : "pseudonym certificates (PCs)";

  const allEnrolled = enrolled !== null && devices !== null && enrolled.total >= devices.total;
  const tiles = present([
    enrolled !== null && devices !== null
      ? countTile(enrolled, {
          id: "enrolled",
          title: ccms ? "Stations enrolled (enrolment credential)" : "Devices enrolled (enrolment certificate, EC)",
          value: `${formatNumber(enrolled.total)} of ${formatNumber(devices.total)}`,
          unit: "devices",
          meaning: "Hold the long-term credential used to ask for pseudonym certificates.",
          level: allEnrolled ? "healthy" : "caution",
          cue: allEnrolled
            ? "Every device is enrolled, as the run expects: each is enrolled before it is deployed."
            : "Some devices are not enrolled: they cannot ask for pseudonym certificates. The run expects every device enrolled before it is deployed; check the ECA's refusals and blocklist.",
          source: ccms ? "Devices (ee): enrolled of stations." : "Devices (ee): enrolled of devices, from the Backend snapshot.",
        })
      : null,
    metricTile(m, "cert_pool_valid", {
      title: `Valid ${certName} per vehicle`,
      meaning: "Mean over vehicles, newest window: certificates a vehicle could sign with right now. It switches between them to stay unlinkable.",
    }),
    starved !== null
      ? countTile(starved, {
          id: "starved",
          title: "Vehicles left without a certificate",
          unit: starved.total === 1 ? "vehicle" : "vehicles",
          meaning:
            unavailable !== null && unavailable.total > 0
              ? `Vehicles that ran out of valid certificates; ${words(unavailable.total, "s")} without one in all.`
              : "Vehicles that ran out of valid certificates and could not sign their messages.",
          level: starved.total > 0 ? "unhealthy" : "healthy",
          cue:
            starved.total > 0
              ? "A vehicle with no valid certificate cannot sign, so receivers drop its safety messages. The run expects none."
              : "None ran out: every vehicle could sign throughout. The run expects none.",
          source: "Devices (ee): vehicles_starved and cert_unavailable_s, from the Backend snapshot.",
        })
      : null,
    countTile(issued, {
      id: "issued",
      title: ccms ? "Authorisation tickets issued (by the AA)" : "Pseudonym certificates issued (by the Pseudonym CA, ACA)",
      unit: "certificates",
      meaning: ccms ? "Tickets the Authorisation Authority has signed so far." : "Signed so far by the Pseudonym CA (Authorization CA, ACA), in batches the RA assembles.",
      source: ccms ? "AA: tickets_issued." : "Pseudonym CA (the engine's pca): certs_issued, from the Backend snapshot.",
    }),
    started !== null && completed !== null
      ? countTile(completed, {
          id: "topups",
          title: "Certificate top-ups (pool refills)",
          value: formatNumber(started.total),
          unit: "requested",
          meaning: `${topupOutcome(started.total, completed.total, inFlight?.total ?? 0, refused?.total ?? 0)} A vehicle asks the RA for its next batch when its pool runs low${topupMs !== null ? `; one takes ${words(topupMs, "ms")} on average` : ""}.`,
          level: refused !== null && refused.total > 0 ? "caution" : null,
          cue: "The RA refused some top-ups (blocklist, enrolment or policy): those devices did not get their next batch. The run expects none refused unless it revokes devices.",
          source: "Devices (ee): topups_started, topups_completed, topups_in_flight, topups_refused, topup_latency_ms.",
        })
      : null,
    countTile(files, {
      id: "downloads",
      title: "Policy and chain files served (by the RA)",
      unit: "files",
      meaning: filesMeaning(files),
      source:
        "RA: files_served, from the Backend snapshot. It counts Local Policy Files (LPF) and Local Certificate Chain Files (LCCF) sent because a device's copy was older; certificate batches are not counted here (the top-ups tile counts them).",
    }),
  ]);

  const pool = m.latest("cert_pool_valid");
  const parts: string[] = [];
  const flagged = worstTile(tiles);
  if (flagged !== null) parts.push(flaggedText(flagged));
  if (enrolled !== null && devices !== null && flagged?.id !== "enrolled") parts.push(allEnrolled ? `all ${formatNumber(devices.total)} devices are enrolled` : `${formatNumber(enrolled.total)} of ${formatNumber(devices.total)} devices are enrolled`);
  if (pool !== null && flagged?.id !== "cert_pool_valid") parts.push(`vehicles hold ${glanceNumber(pool)} valid certificates each on average`);
  if (starved !== null && flagged?.id !== "starved") parts.push(starved.total === 0 ? "none has run out" : `${formatNumber(starved.total)} ran out`);
  const level = worst(tiles.map((t) => t.level));
  const lead = level === "unhealthy" ? "Not all" : level === "caution" ? "Mostly" : parts.length > 0 ? "Yes" : "";
  const answer =
    parts.length === 0
      ? "The run has not reported certificate issuance yet."
      : flagged !== null
        ? `${lead}: ${parts[0]}.${parts.length > 1 ? ` Otherwise ${parts.slice(1).join(", ")}.` : ""}`
        : `${lead}: ${parts.join(", ")}.`;
  return { id: "certs", question: "Are devices getting certificates?", answer, level, tiles };
}

/**
 * Why no vehicle has installed a revocation entry yet. The engine's crl_installs counts entries (one
 * revoked device on one vehicle's list), not list files, so an empty list installs nothing.
 */
function noInstallText(listed: number, downloads: Count | null, frames: Count | null, skipped: Count | null, rsu: Count | null): string {
  const d = downloads?.total ?? 0;
  const f = frames?.total ?? 0;
  const why =
    listed === 0
      ? "the lists published so far are empty (nothing is revoked), so there is nothing to install"
      : skipped !== null && skipped.total > 0
        ? `${formatNumber(skipped.total)} received ${skipped.total === 1 ? "list was" : "lists were"} skipped because the vehicle could not verify its signer yet`
        : d === 0 && f === 0
          ? "no vehicle has had a chance to fetch a list: nothing downloaded from the CRL store and no roadside broadcast yet"
          : `${formatNumber(d)} ${d === 1 ? "download" : "downloads"} and ${formatNumber(f)} roadside broadcast ${f === 1 ? "frame" : "frames"} so far, none installed yet`;
  return `No vehicle holds a revoked device's entry yet: ${why}.${rsu !== null && rsu.total > 0 ? ` Roadside units have installed ${formatNumber(rsu.total)} ${rsu.total === 1 ? "list" : "lists"}.` : ""}`;
}

function revocationSection(m: MetricReader, s: BackendSnapshot | null, first: string): ScmsSection {
  const ccms = s?.system === "ccms";
  const revoked = countOf(s, ["ma"], ["revocations"], first);
  const versions = countOf(s, ccms ? ["ca"] : ["crlg"], ccms ? ["ca_crl_sequence"] : ["versions_signed"], first);
  const cadence = firstOf(s, ["crlg"], "cadence_s");
  const installs = countOf(s, ["ee"], ["crl_installs"], first);
  const rsuInstalls = countOf(s, ["rsu"], ["crls_installed"], first);
  const rejected = countOf(s, ["ee"], ["crls_rejected"], first);
  const downloads = countOf(s, ccms ? ["ca"] : ["crl-store"], ccms ? ["ca_crl_served"] : ["downloads_served"], first);
  const frames = countOf(s, ["crl-broadcast"], ["rsu_frames"], first);
  const enforced = countOf(s, ["ee"], ["revoked_receptions"], first);
  const listed = countOf(s, ccms ? ["ca"] : ["crlg"], ccms ? ["ca_crl_entries"] : ["entries"], first);
  const skipped = countOf(s, ["ee"], ["crls_skipped_unverifiable"], first);
  const fetchMs = firstOf(s, ["ee"], "crl_fetch_latency_ms");
  const multi = listed !== null && listed.byProvider.length > 1;

  const tiles = present([
    countTile(revoked, {
      id: "revoked",
      title: "Devices revoked (by the MA)",
      unit: revoked?.total === 1 ? "device" : "devices",
      meaning: "Decided by the Misbehaviour Authority; each becomes an entry on the CRL.",
      source: "MA: revocations, from the Backend snapshot.",
    }),
    // With several providers each signs its own list: the CRL Generators' counts add up, while the
    // crl_entries metric follows whichever list was published last, so it opens only for one provider.
    multi
      ? countTile(listed, {
          id: "crl_entries",
          title: "Entries on the revocation lists (CRLs)",
          unit: listed?.total === 1 ? "entry" : "entries",
          meaning: "Revoked devices on the newest CRL of each provider, added up; receivers refuse their messages.",
          source: "CRL Generators: entries, from the Backend snapshot. The crl_entries metric follows only the list published last.",
          metric: "crl_entries",
        })
      : (metricTile(m, "crl_entries", {
          title: "Entries on the revocation list (CRL)",
          meaning: "Revoked devices on the newest published certificate revocation list; receivers refuse their messages.",
        }) ??
        countTile(listed, {
          id: "crl_entries",
          title: "Entries on the revocation list (CRL)",
          unit: listed?.total === 1 ? "entry" : "entries",
          meaning: "Revoked devices on the newest published certificate revocation list; receivers refuse their messages.",
          source: "CRL Generator: entries, from the Backend snapshot.",
        })),
    metricTile(m, "crl_bytes", {
      title: multi ? "Newest CRL's size" : "CRL size",
      meaning: multi ? "Bytes of the list published last, by any provider; it grows with every revocation." : "What each device downloads to stay current; it grows with every revocation.",
    }),
    versions !== null
      ? countTile(versions, {
          id: "crl-versions",
          title: "Revocation lists signed (CRL versions)",
          unit: versions.total === 1 ? "version" : "versions",
          meaning: cadence !== null ? `The CRL Generator signs a new list every ${words(cadence, "s")}, revocations or not.` : "Signed versions of the revocation list so far.",
          source: ccms ? "CA: ca_crl_sequence." : "CRL Generator: versions_signed and cadence_s, from the Backend snapshot.",
        })
      : null,
    installs !== null
      ? countTile(installs, {
          id: "crl-installs",
          title: "Revoked-device entries installed by vehicles",
          unit: installs.total === 1 ? "entry" : "entries",
          meaning: installs.total === 0 ? noInstallText(listed?.total ?? m.latest("crl_entries") ?? 0, downloads, frames, skipped, rsuInstalls) : `Each counts one revoked device added to one vehicle's list. ${downloads !== null ? `From ${formatNumber(downloads.total)} downloads from the CRL store` : "From downloads"}${frames !== null ? ` and ${formatNumber(frames.total)} roadside broadcast frames` : ""}${fetchMs !== null ? `; a download takes ${glanceNumber(fetchMs)} ms on average` : ""}${skipped !== null && skipped.total > 0 ? `; ${formatNumber(skipped.total)} skipped because the vehicle could not verify them` : ""}${rsuInstalls !== null ? `; roadside units installed ${formatNumber(rsuInstalls.total)} ${rsuInstalls.total === 1 ? "list" : "lists"}` : ""}.`,
          level: rejected !== null && rejected.total > 0 ? "caution" : null,
          cue: `${rejected !== null ? formatNumber(rejected.total) : "Some"} CRLs were rejected by the vehicles that received them (a bad signature, an untrusted signer or an older version). The run expects every genuine CRL to install.`,
          source: "Devices (ee): crl_installs (revocation entries newly installed, summed over vehicles), crls_rejected, crls_skipped_unverifiable, crl_fetch_latency_ms; CRL store: downloads_served; CRL broadcast: rsu_frames; RSUs: crls_installed.",
        })
      : null,
    enforced !== null
      ? countTile(enforced, {
          id: "enforced",
          title: "Messages from revoked devices dropped",
          unit: "messages",
          meaning: "Refused because the sender is on the receiver's CRL: revocation taking effect.",
          source: "Devices (ee): revoked_receptions, from the Backend snapshot.",
        })
      : null,
    metricTile(m, "time_to_decision", {
      title: "Attack to revocation decision",
      meaning: "Mean time from an attack starting to the MA deciding to revoke the attacker.",
    }),
    metricTile(m, "revocation_latency_stage", {
      id: "slowest-stage",
      title: "Slowest revocation step (p95)",
      series: "revocation_latency_stage.p95",
      meaning: "95 % of revocation steps finished within this; the steps are broken down below.",
    }),
  ]);

  const stageRows = (m.pooled("revocation_latency_stage", "stage") ?? [])
    .filter((r) => r.value !== null && Number.isFinite(r.value))
    .sort((a, b) => stageRank(a.key) - stageRank(b.key) || a.key.localeCompare(b.key));
  const band = m.family("revocation_latency_stage")?.health ?? null;
  const maxStage = Math.max(0, ...stageRows.map((r) => r.value as number));
  const bars: ScmsBars | undefined =
    m.family("revocation_latency_stage") !== undefined
      ? {
          title: "Where the revocation time goes",
          note: "Mean time of each step, in the order a revocation goes through them, over the run so far.",
          max: maxStage,
          metric: "revocation_latency_stage",
          empty: "No revocation has gone through yet: the steps appear once the MA decides one.",
          rows: stageRows.map((r) => {
            const v = r.value as number;
            return { key: r.key, label: stageLabel(r.key), value: v, display: words(v, "s"), level: null };
          }),
          ...(band && maxStage > 0 && band.caution <= maxStage
            ? { limit: { at: band.caution / maxStage, label: `${words(band.caution, "s")}: ${band.healthy_note}` } }
            : {}),
        }
      : undefined;

  const entries = listed?.total ?? m.latest("crl_entries");
  const decide = m.latest("time_to_decision");
  let answer: string;
  if (revoked === null && entries === null) answer = "The run has not reported any revocation activity yet.";
  else if ((revoked?.total ?? 0) === 0 && (entries ?? 0) === 0)
    answer = `Nothing has been revoked yet${versions !== null ? `; the CRL is still published on schedule (${formatNumber(versions.total)} so far)` : ""}.`;
  else {
    const n = revoked?.total ?? entries ?? 0;
    answer = `${formatNumber(n)} ${n === 1 ? "device has" : "devices have"} been revoked${decide !== null ? `, ${words(decide, "s")} after the attack began on average` : ""}${enforced !== null && enforced.total > 0 ? `, and receivers have dropped ${formatNumber(enforced.total)} of their messages` : ""}.`;
  }
  return { id: "revocation", question: "Is revocation working, and how fast?", answer, level: worst(tiles.map((t) => t.level)), tiles, ...(bars ? { bars } : {}) };
}

function misbehaviourSection(m: MetricReader, s: BackendSnapshot | null, first: string): ScmsSection {
  const filed = countOf(s, ["ee"], ["reports_filed"], first);
  const cell = countOf(s, ["ee"], ["reports_over_cellular"], first);
  const viaRsu = countOf(s, ["ee"], ["reports_via_rsu"], first);
  const waiting = countOf(s, ["ee"], ["reports_waiting_for_coverage"], first);
  const shuffle = countOf(s, ["ra"], ["reports_in_shuffle"], first);
  const atMa = countOf(s, ["ma"], ["reports_received"], first);
  const cases = countOf(s, ["ma"], ["cases_opened", "cases_total"], first);
  const decisions = countOf(s, ["ma"], ["revocation_decisions"], first);
  const revoked = countOf(s, ["ma"], ["revocations"], first);
  const uuMs = firstOf(s, ["ee"], "report_uu_latency_ms");
  const relayMs = firstOf(s, ["ee"], "report_relay_latency_ms");

  const shuffleS = firstOf(s, ["ra"], "shuffle_window_s");
  const sentTotal = cell || viaRsu ? (cell?.total ?? 0) + (viaRsu?.total ?? 0) : null;
  const notSent = filed !== null && sentTotal !== null ? Math.max(0, filed.total - sentTotal) : 0;
  // Downstream of the upload, the RA and the MA hold more than the vehicles count as sent (the MA
  // also receives reports another provider's MA shares): then the filed-to-sent gap is not reports
  // left behind, and saying "N still on the vehicles" would be wrong.
  const downstream = (shuffle?.total ?? 0) + (atMa?.total ?? 0);
  const gapIsReal = sentTotal !== null && downstream <= sentTotal;
  const funnel: FunnelStep[] = present([
    filed && { id: "filed", label: "Filed by vehicles", value: filed.total, hint: "Misbehaviour reports (MBRs) the vehicles' detectors wrote." },
    sentTotal !== null
      ? {
          id: "sent",
          label: "Uploaded to the RA",
          value: sentTotal,
          hint: `${formatNumber(cell?.total ?? 0)} over cellular, ${formatNumber(viaRsu?.total ?? 0)} relayed by roadside units${waiting ? `; ${formatNumber(waiting.total)} waiting for coverage` : ""}.${
            notSent > 0 && !gapIsReal
              ? ` The RA and the MA hold ${formatNumber(downstream)} reports, more than this counts as sent (the MA also receives reports other providers share), so the ${formatNumber(notSent)} between filed and sent are not reports left behind; the engine does not break the difference down.`
              : ""
          }`,
          ...(notSent > 0 && gapIsReal
            ? {
                drop: `${formatNumber(notSent)} not uploaded${waiting && waiting.total > 0 ? ` (${formatNumber(waiting.total)} wait for coverage)` : ""}`,
              }
            : {}),
        }
      : null,
    shuffle && {
      id: "shuffle",
      label: "Held in the RA's shuffle",
      value: shuffle.total,
      hint: `Held and mixed by the RA so the MA cannot tell who reported${shuffleS !== null ? `; released when the ${words(shuffleS, "s")} shuffle window closes` : "; released when the shuffle window closes"}. A count of reports there now, not a step every report has left.`,
    },
    atMa && {
      id: "ma",
      label: "Received by the MA",
      value: atMa.total,
      hint: "Reports the Misbehaviour Authority has received.",
      ...(shuffle && shuffle.total > 0 ? { drop: `${formatNumber(shuffle.total)} wait in the shuffle` } : {}),
    },
    cases && { id: "cases", label: "Suspects with a case", value: cases.total, hint: "Suspects the MA is collecting evidence on (counted per suspect, not per report)." },
    (decisions ?? revoked) && { id: "revoked", label: "Revoked", value: (revoked ?? decisions)!.total, hint: "Suspects the MA decided to revoke." },
  ]);
  // Reports wait in the RA's shuffle by design; until any reach the MA and it revokes someone, a low
  // recall is the pipeline still working, not detection failing.
  const pendingText =
    shuffle !== null && shuffle.total > 0 && (revoked?.total ?? decisions?.total ?? 0) === 0
      ? `${formatNumber(shuffle.total)} ${shuffle.total === 1 ? "report waits" : "reports wait"} in the RA's shuffle${shuffleS !== null ? `, released every ${words(shuffleS, "s")}` : ""}; detection is judged once they reach the MA.`
      : null;

  const recallVehicle = pooledValue(m, "det_recall", "level", "vehicle");
  const recallReport = pooledValue(m, "det_recall", "level", "report");
  const precVehicle = pooledValue(m, "det_precision", "level", "vehicle");
  const fpr = pooledValue(m, "det_fpr", "level", "report") ?? pooledValue(m, "det_fpr", "level", "vehicle");
  const honestRevoked = pooledValue(m, "false_accusations", "level", "vehicle");
  const honestReported = pooledValue(m, "false_accusations", "level", "report");
  const recallBand = m.family("det_recall")?.health ?? null;
  const fprBand = m.family("det_fpr")?.health ?? null;

  const ratioTile = (id: string, title: string, v: number | null, meaning: string, band: HealthBand | null, metric: string, source: string): ScmsTile | null => {
    if (v === null || m.family(metric) === undefined) return null;
    const level = band ? classify(band, v) : null;
    const sh = readable(v, "ratio");
    return { id, title, value: sh.value, unit: sh.unit, meaning, level, ...(level && band ? { cue: cueText(band, level, "ratio") } : {}), metric, source };
  };

  const recallTile = ratioTile(
      "recall",
      recallVehicle !== null ? "Attackers revoked (detection recall)" : "Attackers reported (detection recall, per report)",
      recallVehicle ?? recallReport,
      recallVehicle !== null ? "Share of attacking vehicles the system revoked, over the run so far." : "Share of attackers reported at least once, over the run so far.",
      recallBand,
      "det_recall",
      `Metric det_recall, level = ${recallVehicle !== null ? "vehicle" : "report"}, pooled over the run by the engine.`,
  );
  const tiles = present([
    // Only a would-be bad verdict (or a bare zero) is set aside: a good recall stands as judged.
    recallTile !== null && pendingText !== null && (recallTile.level === "caution" || recallTile.level === "unhealthy" || (recallTile.level === null && (recallVehicle ?? recallReport) === 0))
      ? asPending(recallTile, pendingText)
      : recallTile !== null && shuffle !== null && shuffle.total > 0
        ? // Judged, but not final: say what is still on its way to the MA.
          { ...recallTile, meaning: `${recallTile.meaning} ${formatNumber(shuffle.total)} ${shuffle.total === 1 ? "report still waits" : "reports still wait"} in the RA's shuffle; more attackers may be revoked once they reach the MA.` }
        : recallTile,
    honestRevoked !== null && m.family("false_accusations") !== undefined
      ? {
          id: "honest-revoked",
          title: "Honest vehicles revoked (false accusations)",
          value: formatNumber(honestRevoked),
          unit: honestRevoked === 1 ? "vehicle" : "vehicles",
          meaning:
            honestReported === null
              ? "Honest vehicles the system revoked by mistake."
              : honestReported === 0
                ? "Honest vehicles revoked by mistake; no honest vehicle was even reported."
                : `Honest vehicles revoked by mistake; ${formatNumber(honestReported)} were reported at least once.`,
          level: (honestRevoked > 0 ? "unhealthy" : "healthy") as HealthLevel,
          cue:
            honestRevoked > 0
              ? "An honest vehicle was revoked: it can no longer be trusted by anyone. The run expects none."
              : "No honest vehicle was revoked. The run expects none.",
          metric: "false_accusations",
          source: "Metric false_accusations, level = vehicle, pooled over the run by the engine.",
        }
      : null,
    ratioTile("fpr", "Honest vehicles wrongly flagged (false-positive rate)", fpr, "Share of honest vehicles a detector reported, over the run so far.", fprBand, "det_fpr", "Metric det_fpr, pooled over the run by the engine."),
    ratioTile("precision", "Revocations that were right (precision)", precVehicle, "Share of revoked vehicles that really were attackers.", null, "det_precision", "Metric det_precision, level = vehicle, pooled over the run by the engine."),
    metricTile(m, "time_to_detect", {
      title: "Attack to first report at the MA",
      meaning: "Mean time from an attack starting to the first correct report reaching the MA.",
    }),
    uuMs !== null || relayMs !== null
      ? {
          id: "report-latency",
          title: "Report upload time",
          value: glanceNumber((uuMs ?? relayMs)!),
          unit: "ms",
          meaning: `Mean time for a report to reach the RA${uuMs !== null ? " over cellular" : ""}${uuMs !== null && relayMs !== null ? `; ${glanceNumber(relayMs)} ms through a roadside unit` : relayMs !== null ? " through a roadside unit" : ""}.`,
          level: null,
          source: "Devices (ee): report_uu_latency_ms and report_relay_latency_ms, from the Backend snapshot.",
        }
      : null,
  ]);

  const level = worst(tiles.map((t) => t.level));
  let answer: string;
  if (filed === null && tiles.length === 0) answer = "The run has not reported misbehaviour detection yet.";
  else {
    const parts: string[] = [];
    if (filed !== null) parts.push(`vehicles have filed ${formatNumber(filed.total)} misbehaviour ${filed.total === 1 ? "report" : "reports"}`);
    const r = recallVehicle ?? recallReport;
    const pending = tiles.some((t) => t.pending !== undefined);
    if (pending && pendingText !== null) parts.push(pendingText.replace(/\.$/, "").replace("; detection is judged", ", so detection is judged"));
    else if (r !== null) parts.push(`${words(r, "ratio")} of attackers ${recallVehicle !== null ? "revoked" : "reported"}`);
    if (honestRevoked !== null) parts.push(honestRevoked === 0 ? "no honest vehicle revoked" : `${formatNumber(honestRevoked)} honest ${honestRevoked === 1 ? "vehicle" : "vehicles"} revoked`);
    answer = parts.length > 0 ? `${parts[0]![0]!.toUpperCase()}${parts.join("; ").slice(1)}.` : "No report has been filed yet.";
  }
  return { id: "misbehaviour", question: "Is misbehaviour being caught, and are honest devices affected?", answer, level, ...(tiles.some((t) => t.pending !== undefined) ? { pending: true } : {}), tiles, ...(funnel.length > 0 ? { funnel } : {}) };
}

/**
 * Privacy: the reason pseudonym certificates exist. Linkability is the share of pseudonym changes
 * a worst-case eavesdropper links back to the same vehicle (cumulative over the run); the change
 * rate is how often vehicles switch. `null` when the run measures neither.
 */
function privacySection(m: MetricReader, s: BackendSnapshot | null, first: string): ScmsSection | null {
  const changes = countOf(s, ["ee"], ["pseudonym_changes"], first);
  const link = metricTile(m, "linkability", {
    title: "Pseudonym changes an eavesdropper links (linkability)",
    meaning: "Share of pseudonym changes a worst-case roadside observer links back to the same vehicle, over the run so far. Lower is better.",
  });
  const rate = metricTile(m, "pseudonym_change_rate", {
    title: "Pseudonym changes per vehicle-hour",
    meaning: "How often a vehicle switches to a new certificate, mean over the run. Changing often makes tracking harder; each change uses a certificate.",
    mean: true,
  });
  if (link === null && rate === null) return null;
  const tiles = present([
    link,
    rate,
    changes !== null && changes.total > 0
      ? countTile(changes, {
          id: "pseudonym-changes",
          title: "Pseudonym changes so far",
          unit: changes.total === 1 ? "change" : "changes",
          meaning: "Times a vehicle switched to another pseudonym certificate, added up over the fleet.",
          source: "Devices (ee): pseudonym_changes, from the Backend snapshot.",
        })
      : null,
  ]);
  const lk = m.latest("linkability");
  const level = worst(tiles.map((t) => t.level));
  const share = lk !== null ? words(lk, "ratio") : null;
  const lead =
    share === null
      ? null
      : level === "unhealthy"
        ? `Yes, mostly: an eavesdropper links ${share} of pseudonym changes back to the same vehicle, so pseudonyms are not protecting vehicles here`
        : level === "caution"
          ? `Sometimes: an eavesdropper links ${share} of pseudonym changes back to the same vehicle`
          : level === "healthy"
            ? `Rarely: an eavesdropper links only ${share} of pseudonym changes back to the same vehicle`
            : `An eavesdropper links ${share} of pseudonym changes back to the same vehicle`;
  const rateV = rate !== null ? `vehicles change pseudonym ${rate.value} times per vehicle-hour` : null;
  const parts = present([lead, rateV]);
  return {
    id: "privacy",
    question: "Can a vehicle be tracked across pseudonym changes?",
    answer: `${cap(parts.join("; "))}.`,
    level,
    tiles,
  };
}

function costSection(m: MetricReader): ScmsSection {
  const tiles = present([
    metricTile(m, "security_overhead", {
      title: "Security's share of the bytes on air",
      meaning: "Header, certificate or digest, and signature, as a share of each frame.",
    }) ??
      metricTile(m, "envelope_overhead", {
        title: "Security bytes per payload byte",
        meaning: "Security envelope bytes as a multiple of the application's own bytes: 1.3 × means 30 % more security than payload.",
      }),
    metricTile(m, "full_cert_share", {
      title: "Messages carrying a full certificate",
      meaning: "The rest carry an 8-byte digest of the certificate instead, which is much smaller.",
    }),
    metricTile(m, "verify_wait", {
      id: "verify-wait",
      title: "Wait before verification (p95)",
      series: "verify_wait.p95",
      meaning: "95 % of messages waited less than this for the verifier (safety budget: 100 ms end to end).",
    }),
    metricTile(m, "verify_queue_depth", {
      title: "Messages queued for verification",
      unit: "messages, on average",
      meaning: "Queue length a new message finds; above one, the verifier is rarely idle.",
    }),
    metricTile(m, "unverified_ratio", {
      title: "Delivered without being verified",
      meaning: "Messages passed to applications before their signature was checked.",
    }),
    metricTile(m, "verify_rate", {
      title: "Verifications per second (all devices)",
      meaning: "Signature checks completed across the fleet each second, mean over the run.",
      mean: true,
    }),
    metricTile(m, "verify_cost", {
      title: "CPU time per verification",
      meaning: "Mean processing time of one signature check on the device's hardware.",
    }),
    metricTile(m, "bytes_backend", {
      title: "Credential traffic to and from the backend",
      meaning: "Certificate requests, downloads, reports and CRLs on the backend links, mean over the run.",
      mean: true,
    }),
  ]);

  const rows = (m.pooled("loss_rate", "cause") ?? [])
    .filter((r) => r.key in SECURITY_CAUSES && r.value !== null && Number.isFinite(r.value) && (r.value as number) > 0)
    .sort((a, b) => (b.value as number) - (a.value as number));
  const lossMax = Math.max(0, ...rows.map((r) => r.value as number));
  const bars: ScmsBars | undefined =
    m.family("loss_rate") !== undefined
      ? {
          title: "Messages rejected by security checks, by reason",
          note: "Share of all reception attempts lost to each check, over the run so far. No standard sets an acceptable rate, so the bars carry no verdict; hover a reason for what it means.",
          max: lossMax,
          metric: "loss_rate",
          empty: "No message was rejected by a security check.",
          rows: rows.map((r) => ({
            key: r.key,
            label: `${SECURITY_CAUSES[r.key]!}${BY_DESIGN.has(r.key) ? " · expected" : ""}`,
            value: r.value as number,
            display: sharePct(r.value as number),
            level: null,
            hint: CAUSE_HINTS[r.key] ?? r.key,
          })),
        }
      : undefined;

  const level = worst(tiles.map((t) => t.level));
  const over = m.latest("security_overhead");
  const wait = m.latest("verify_wait.p95");
  const parts: string[] = [];
  if (over !== null) parts.push(`security takes ${words(over, "ratio")} of the bytes on air`);
  if (wait !== null) parts.push(wait <= 0 ? "messages do not queue to be verified (95th-percentile wait 0 ms)" : `95 % of messages wait under ${words(wait, "ms")} to be verified`);
  const failed = rows.filter((r) => !BY_DESIGN.has(r.key)).reduce((a, r) => a + (r.value as number), 0);
  const designed = rows.filter((r) => BY_DESIGN.has(r.key)).reduce((a, r) => a + (r.value as number), 0);
  if (rows.length > 0)
    parts.push(
      `${sharePct(failed)} of receptions are rejected by a security check, a rate no standard judges${designed > 0 ? ` (another ${sharePct(designed)} are overheard reports meant for a roadside unit, dropped by design)` : ""}`,
    );
  const answer = parts.length > 0 ? `${parts[0]![0]!.toUpperCase()}${parts.join("; ").slice(1)}.` : "The run has not measured security's cost yet.";
  return { id: "cost", question: "What does security cost the radio and the CPU?", answer, level, tiles, ...(bars ? { bars } : {}) };
}

/** A utilisation as a percentage: "12.5", "< 0.1" for a server that did a little work. */
export function busyPct(u: number): string {
  if (u > 0 && u < 0.001) return "< 0.1";
  return glanceNumber(u * 100);
}

function backendSection(m: MetricReader, s: BackendSnapshot | null, tNs: number, first: string): ScmsSection | null {
  // Reaching the backend is coverage, not issuance: it answers here, beside the backend's load.
  const reach = metricTile(m, "backend_link_up", {
    title: "Vehicles that can reach the backend",
    meaning: "Share with cellular or roadside-unit access to the credential system, newest window.",
  });
  if (s === null) {
    if (reach === null) return null;
    const lv = reach.level;
    return {
      id: "backend",
      question: "Can vehicles reach the backend, and is it keeping up?",
      answer: lv !== null && lv !== "healthy" ? `${cap(flaggedText(reach))}.` : `${reach.value} % of vehicles can reach the backend.`,
      level: lv,
      tiles: [reach],
    };
  }
  const multi = new Set(s.entities.map((e) => providerOf(e, first))).size > 1;
  const named = (id: string, name: string): string => {
    if (!multi) return name;
    const p = providerOf(s.entities.find((e) => e.id === id) ?? { id, state: {} }, first);
    return name.includes(p) ? name : `${name.replace(/\s*\([^)]*\)\s*$/, "")} (${p})`;
  };
  const loads = entityLoads(s, tNs)
    .filter((l) => l.utilisation > 0 || l.depth > 0)
    .map((l) => ({ ...l, name: named(l.id, l.name) }));
  const served = new Map(s.entities.map((e) => [e.id, e.queue?.served ?? 0] as const));
  const levelOf = (u: number): HealthLevel => (u > ENTITY_UTILISATION_LIMIT ? "unhealthy" : u >= ENTITY_UTILISATION_LIMIT * 0.75 ? "caution" : "healthy");
  const shownLoads = loads.slice(0, 8);
  const busiestU = loads[0]?.utilisation ?? 0;
  const bars: ScmsBars = {
    title: "Busiest authorities",
    note: "Share of the run each authority's servers were busy, with the mean wait of a request.",
    max: 1,
    limit: { at: ENTITY_UTILISATION_LIMIT, label: "80 %: the usual planning ceiling, past which queues grow without bound (an engineering choice)" },
    empty: "No authority has done queued work yet.",
    // Bars all under 1 % against an 80 % line say nothing; one sentence says it better.
    ...(loads.length > 0 && busiestU < 0.01
      ? { idle: `Every authority is idle more than ${busiestU < 0.001 ? "99.9" : "99"} % of the time: the busiest, ${loads[0]!.name}, is busy ${busyPct(busiestU)} %. The bars appear when one does real work (1 % or more).` }
      : {}),
    rows: shownLoads.map((l) => ({
      key: l.id,
      label: l.name,
      value: l.utilisation,
      display: `${busyPct(l.utilisation)} %`,
      level: levelOf(l.utilisation),
      hint: `${formatNumber(served.get(l.id) ?? 0)} requests served${l.meanWaitMs !== null ? `, ${words(l.meanWaitMs, "ms")} mean wait` : ""}${l.depth > 0 ? `, ${formatNumber(l.depth)} waiting now` : ""}.`,
    })),
  };
  let messages = 0;
  let bytesTotal = 0;
  for (const e of s.edges) {
    messages += e.messages;
    bytesTotal += e.bytes;
  }
  const queued = s.entities.reduce((a, e) => a + (e.queue?.depth ?? 0), 0);
  const top = loads[0];
  const level = top ? levelOf(top.utilisation) : null;
  const tiles: ScmsTile[] = [
    ...(reach ? [reach] : []),
    {
      id: "backend-messages",
      title: "Messages between authorities and devices",
      value: formatNumber(messages),
      unit: "messages",
      meaning: `${words(bytesTotal, "B")} in all, on ${formatNumber(s.edges.length)} links.`,
      level: null,
      source: "Backend snapshot: every link's message and byte count since the run began.",
    },
    {
      id: "backend-queued",
      title: "Requests waiting now",
      value: formatNumber(queued),
      unit: queued === 1 ? "request" : "requests",
      meaning: "Requests sitting in every authority's queue at this moment.",
      level: null,
      source: "Backend snapshot: the sum of every queue's depth.",
    },
  ];
  if (top) {
    tiles.splice(reach ? 1 : 0, 0, {
      id: "backend-busiest",
      title: "Busiest authority",
      value: busyPct(top.utilisation),
      unit: "% busy",
      meaning: `${top.name}${top.meanWaitMs !== null ? `; a request waits ${words(top.meanWaitMs, "ms")} on average` : ""}.`,
      level,
      cue:
        level === "healthy"
          ? "Below 60 % busy: well clear of the 80 % planning ceiling past which queues grow without bound (an engineering choice, the run analyst's entity_utilisation)."
          : level === "caution"
            ? "Between 60 and 80 % busy: approaching the 80 % planning ceiling past which queues grow without bound (an engineering choice)."
            : "Past the 80 % planning ceiling: its queue grows without bound (an engineering choice, the run analyst's entity_utilisation).",
      source: "Backend snapshot: busy time over servers × elapsed time.",
    });
  }
  const load =
    top === undefined
      ? "no authority has done queued work yet"
      : level === "healthy"
        ? `the busiest authority, ${top.name}, is busy ${busyPct(top.utilisation)} % of the time`
        : `${top.name} is busy ${busyPct(top.utilisation)} % of the time${level === "unhealthy" ? ", past the 80 % planning ceiling" : ", close to the 80 % planning ceiling"}`;
  const reachBad = reach !== null && reach.level !== null && reach.level !== "healthy";
  const sectionLevel = worst([level, reach?.level ?? null]);
  const answer = reachBad
    ? `${cap(flaggedText(reach))}; ${load}.`
    : `${sectionLevel === "healthy" ? "Yes: " : ""}${sectionLevel === "healthy" ? load : cap(load)}${reach !== null ? `; ${reach.value} % of vehicles can reach it` : ""}.`;
  return { id: "backend", question: "Can vehicles reach the backend, and is it keeping up?", answer, level: sectionLevel, tiles, bars };
}

// ------------------------------------------------------------------------------------------------
// The whole overview
// ------------------------------------------------------------------------------------------------

const SIGNATURES: Readonly<Record<string, string>> = {
  "ecdsa-p256": "ECDSA P-256 signatures",
};

/**
 * The catalogue metrics the overview shows a value for (a tile or a bar chart with rows): a card
 * below with no per-window value of its own points here instead of saying "no data".
 */
export function coveredMetrics(o: ScmsOverview): string[] {
  const out = new Set<string>();
  for (const s of o.sections) {
    for (const t of s.tiles) if (t.metric !== undefined) out.add(t.metric);
    if (s.bars?.metric !== undefined && s.bars.rows.length > 0) out.add(s.bars.metric);
  }
  return [...out];
}

/** Whether the run has a credential system to show: a backend snapshot or any of its metrics. */
export function hasCredentials(s: BackendSnapshot | null, m: MetricReader): boolean {
  if (s !== null && s.entities.length > 0) return true;
  return CREDENTIAL_METRICS.some((b) => m.family(b) !== undefined);
}

/** The metrics only a run with a credential system measures. */
export const CREDENTIAL_METRICS: readonly string[] = ["cert_pool_valid", "crl_entries", "time_to_decision", "det_recall"];

export function scmsOverview(s: BackendSnapshot | null, m: MetricReader, opts: { readonly tNs: number; readonly scenario: unknown }): ScmsOverview {
  const first = firstProvider(opts.scenario, s);
  const providers = s === null ? [] : [...new Set(s.entities.filter((e) => roleOf(e.id) === "ee" || roleOf(e.id) === "ma").map((e) => providerOf(e, first)))];
  const system =
    s?.system === "ccms"
      ? "EU CCMS: the Cooperative ITS Credential Management System (ETSI TS 102 941)"
      : s?.system === "scms" || s === null
        ? "US SCMS: the Security Credential Management System (IEEE 1609.2.1)"
        : s.system.toUpperCase();
  const facts = [
    providers.length > 1 ? `${providers.length} providers: ${providers.join(", ")}` : providers.length === 1 ? `1 provider: ${providers[0]}` : null,
    s ? (SIGNATURES[s.signature] ?? `${s.signature} signatures`) : null,
  ]
    .filter((x): x is string => x !== null)
    .join(" · ");
  const sections = present([
    certsSection(m, s, first),
    revocationSection(m, s, first),
    misbehaviourSection(m, s, first),
    privacySection(m, s, first),
    costSection(m),
    backendSection(m, s, opts.tNs, first),
  ]);
  const raw =
    s === null
      ? []
      : s.entities.map((e) => ({
          id: e.id,
          name: e.name,
          counts: Object.entries(e.state)
            .filter(([, v]) => typeof v === "number" || typeof v === "boolean" || typeof v === "string")
            .map(([k, v]) => [k, typeof v === "number" ? formatNumber(v) : String(v)] as const),
        }));
  return { system, facts, providers, sections, raw };
}
