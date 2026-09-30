/**
 * The metrics dashboard's pure half: what a metric is to the page, how the dashboard groups and
 * names it, which way is worse, the statistics of a range, the display downsampling, the CSV, and
 * the dashboard's addresses.
 *
 * Nothing here touches the DOM, the engine or the store, so `test/metrics-model.test.ts` pins all
 * of it in plain Node.
 *
 * # What a card is
 *
 * The engine's catalogue (`metrics.query` with no metric list, §6.12) has one row per *series* the
 * stream can carry: a metric's headline (`e2e_latency`), a distribution's percentiles
 * (`e2e_latency.p95`) and a declared breakdown's values (`latency_stage[airtime]`). Every row names
 * its `base`, the metric it is a view of. A researcher thinks in metrics, not series, so the
 * dashboard shows one card per base and puts the series behind it in the expanded view.
 */

// ------------------------------------------------------------------------------------------------
// The catalogue
// ------------------------------------------------------------------------------------------------

/** One catalogue row, as the page keeps it. */
export interface SeriesDef {
  readonly name: string;
  readonly base: string;
  readonly unit: string;
  readonly dims: readonly string[];
  readonly agg: string;
  readonly visibility: string;
  readonly definition: string;
  readonly source?: string;
  readonly notAccounted: readonly string[];
}

/** One metric: a dashboard card. */
export interface MetricFamily {
  readonly base: string;
  readonly label: string;
  readonly group: GroupId;
  readonly unit: string;
  readonly visibility: string;
  readonly agg: string;
  /** The metric's definition, without the per-series notes the catalogue prefixes. */
  readonly definition: string;
  readonly source?: string;
  readonly notAccounted: readonly string[];
  /** The dimensions a breakdown can group by: the catalogue's, less time and run. */
  readonly breakdowns: readonly string[];
  /** The headline series, or `null` for a metric that only exists per value (a stage's share). */
  readonly headline: string | null;
  /** A distribution's percentile series, in p50, p95, p99 order. */
  readonly percentiles: readonly string[];
  /** The live breakdown's series (`loss_rate[collision]`), in the catalogue's order. */
  readonly values: readonly { readonly key: string; readonly series: string }[];
  /** The dimension those series are values of, when the catalogue says. */
  readonly valueDim: string | null;
  readonly polarity: Polarity;
  /** Ground truth: a quantity no deployed device could measure (08-measurement-and-data.md §1). */
  readonly groundTruth: boolean;
}

/** Every series a card can plot, headline first. */
export function seriesOf(f: MetricFamily): string[] {
  return [...(f.headline === null ? [] : [f.headline]), ...f.percentiles, ...f.values.map((v) => v.series)];
}

/** The series a card draws: its headline, else its per-value series. */
export function cardSeries(f: MetricFamily): string[] {
  return f.headline !== null ? [f.headline] : f.values.map((v) => v.series);
}

/** Strips the note the engine prefixes to a series' definition (`live.rs` `metric_series`). */
function baseDefinition(text: string): string {
  return text
    .replace(/^Mean over the window\.\s*/, "")
    .replace(/^The p\d+ \(type-7 quantile\) over the window\.\s*/, "")
    .replace(/^For [a-z_]+ = [^\s]+\.\s*/, "");
}

/**
 * Build the cards from the catalogue.
 *
 * Grouped by `base`, in the catalogue's order within a group. A row whose name is neither its base,
 * a percentile nor a `[value]` series is kept as a headline of its own: an engine that does not
 * publish `base` has one card per series, which is still every measurement.
 */
export function familiesOf(rows: readonly SeriesDef[]): MetricFamily[] {
  const byBase = new Map<string, SeriesDef[]>();
  for (const r of rows) {
    const list = byBase.get(r.base) ?? [];
    list.push(r);
    byBase.set(r.base, list);
  }
  const out: MetricFamily[] = [];
  for (const [base, list] of byBase) {
    const headline = list.find((r) => r.name === base) ?? null;
    const percentiles = ["p50", "p95", "p99"]
      .map((q) => list.find((r) => r.name === `${base}.${q}`))
      .filter((r): r is SeriesDef => r !== undefined)
      .map((r) => r.name);
    const values: { key: string; series: string }[] = [];
    let valueDim: string | null = null;
    for (const r of list) {
      if (!r.name.startsWith(`${base}[`) || !r.name.endsWith("]")) continue;
      values.push({ key: r.name.slice(base.length + 1, -1), series: r.name });
      const m = /^For ([a-z_]+) = /.exec(r.definition);
      if (m && valueDim === null) valueDim = m[1];
    }
    const first = headline ?? list[0];
    out.push({
      base,
      label: labelOf(base),
      group: groupOf(base),
      unit: first.unit,
      visibility: first.visibility,
      agg: first.agg,
      definition: baseDefinition(first.definition),
      ...(first.source ? { source: first.source } : {}),
      notAccounted: first.notAccounted,
      breakdowns: first.dims.filter((d) => d !== "t" && d !== "run"),
      headline: headline?.name ?? (values.length === 0 && percentiles.length === 0 ? first.name : null),
      percentiles,
      values,
      valueDim,
      polarity: polarityOf(base),
      groundTruth: first.visibility === "GT" || first.visibility === "NODE+GT",
    });
  }
  return out;
}

// ------------------------------------------------------------------------------------------------
// Groups: what a researcher asks
// ------------------------------------------------------------------------------------------------

export type GroupId =
  | "channel"
  | "delivery"
  | "latency"
  | "security"
  | "misbehaviour"
  | "privacy"
  | "traffic"
  | "safety"
  | "overhead"
  | "simulator"
  | "other";

export interface GroupSpec {
  readonly id: GroupId;
  readonly label: string;
  /** The question the group answers, shown under its heading. */
  readonly question: string;
}

/** In the order a V2X study reads them. */
export const GROUPS: readonly GroupSpec[] = [
  { id: "channel", label: "Channel load", question: "How busy is the channel, and is congestion control holding it?" },
  { id: "delivery", label: "Delivery and reliability", question: "Do messages get through, to whom, and how fresh is what receivers know?" },
  { id: "latency", label: "Latency", question: "How long does a message take, and where does the time go?" },
  { id: "security", label: "Security and PKI", question: "What do signing, verification and the credential system cost?" },
  { id: "misbehaviour", label: "Misbehaviour detection", question: "Are attackers caught, how fast, and at what false-alarm cost?" },
  { id: "privacy", label: "Privacy", question: "Can a vehicle be followed across its pseudonym changes?" },
  { id: "traffic", label: "Traffic", question: "How is the traffic moving?" },
  { id: "safety", label: "Safety applications", question: "How close do road users come to conflict, and do warnings help?" },
  { id: "overhead", label: "Overhead and bytes", question: "What does each useful byte cost on the air and the backhaul?" },
  { id: "simulator", label: "Simulator", question: "How hard is this machine working to run the simulation? Machine-dependent; never part of a result." },
  { id: "other", label: "Other", question: "Measurements this page has no group for yet." },
];

const EXPLICIT: Readonly<Record<string, GroupId>> = {
  cbr: "channel",
  channel_load: "channel",
  channel_occupancy: "channel",
  offered_load: "channel",
  carried_load: "channel",
  airtime_per_node: "channel",
  mac_queue_depth: "channel",
  mac_drops: "channel",
  half_duplex_rate: "channel",
  collision_rate: "channel",
  pdr: "delivery",
  pdr_all_pairs: "delivery",
  pdr_by_cause: "delivery",
  per: "delivery",
  delivery_ratio: "delivery",
  loss_rate: "delivery",
  goodput: "delivery",
  pir: "delivery",
  aoi: "delivery",
  aoi_peak: "delivery",
  nar: "delivery",
  e2e_latency: "latency",
  latency_stage: "latency",
  latency_stage_share: "latency",
  latency_trace_rejected: "latency",
  mac_access_delay: "latency",
  verify_rate: "security",
  verify_cost: "security",
  verify_wait: "security",
  verify_queue_depth: "security",
  unverified_ratio: "security",
  full_cert_share: "security",
  envelope_overhead: "security",
  revocation_latency_stage: "security",
  crl_entries: "security",
  crl_bytes: "security",
  cert_pool_valid: "security",
  backend_link_up: "security",
  false_accusations: "misbehaviour",
  time_to_detect: "misbehaviour",
  time_to_decision: "misbehaviour",
  pseudonym_change_rate: "privacy",
  linkability: "privacy",
  mean_speed: "traffic",
  density: "traffic",
  flow: "traffic",
  speed: "traffic",
  acceleration: "traffic",
  headway_time: "traffic",
  headway_distance: "traffic",
  ttc_min: "safety",
  ttc_conflicts: "safety",
  drac: "safety",
  pet: "safety",
  security_overhead: "overhead",
  net_header_overhead: "overhead",
  link_overhead: "overhead",
  cert_bytes_share: "overhead",
  air_bytes_per_payload_byte: "overhead",
  bytes_per_vehicle_hour: "overhead",
  events_processed: "simulator",
  events_per_second: "simulator",
  wall_clock_per_sim_second: "simulator",
  memory_high_water_mark: "simulator",
};

/** Name patterns for metrics the table above does not know (another track's new ones). */
const PATTERNS: readonly (readonly [RegExp, GroupId])[] = [
  [/^det_|misbehav|^mbr_|detector/, "misbehaviour"],
  [/pseudonym|linkab|privacy|anonym|track(ing|ed)_/, "privacy"],
  [/^frag_/, "delivery"],
  [/^bytes_|overhead/, "overhead"],
  [/latency|delay/, "latency"],
  [/cert|crl|^verify|^sign|scms|ccms|pki|revocation|enrol|^ra_|^pca_|^ma_/, "security"],
  [/warning|^app_|denm|eebl|^fcw|^ima|^lta|^vru_|collision_warning|alert|spat|map_|safety/, "safety"],
  [/ttc|pet|drac|conflict|near_miss/, "safety"],
  [/cbr|channel|airtime|^mac_|dcc|congestion|queue/, "channel"],
  [/pdr|delivery|loss|reception|aoi|awareness|goodput/, "delivery"],
  [/speed|flow|density|headway|travel|stops|pedestrian|jaywalk|vehicle|traffic|motorcycle/, "traffic"],
  [/wall_clock|events_per|memory|runtime/, "simulator"],
];

export function groupOf(base: string): GroupId {
  const explicit = EXPLICIT[base];
  if (explicit !== undefined) return explicit;
  for (const [re, group] of PATTERNS) if (re.test(base)) return group;
  return "other";
}

/** The name a reader knows a metric by; the engine's name stays visible beside it. */
const LABELS: Readonly<Record<string, string>> = {
  cbr: "Channel busy ratio",
  channel_load: "Channel load around each node",
  channel_occupancy: "Channel occupancy",
  offered_load: "Offered load",
  carried_load: "Carried load",
  airtime_per_node: "Air time per node",
  mac_queue_depth: "MAC queue depth",
  mac_drops: "MAC queue drops",
  mac_access_delay: "Channel access delay",
  half_duplex_rate: "Half-duplex losses",
  collision_rate: "Collision losses",
  pdr: "Packet delivery ratio",
  pdr_all_pairs: "Packet delivery ratio, all pairs",
  pdr_by_cause: "Packet delivery by loss cause",
  per: "Packet error ratio",
  delivery_ratio: "Delivered to the application",
  loss_rate: "Loss rate by cause",
  goodput: "Goodput",
  pir: "Packet inter-reception time",
  aoi: "Age of information",
  aoi_peak: "Peak age of information",
  nar: "Neighbour awareness ratio",
  e2e_latency: "End-to-end latency",
  latency_stage: "Latency by stage",
  latency_stage_share: "Share of latency by stage",
  latency_trace_rejected: "Rejected latency traces",
  verify_rate: "Verifications per second",
  verify_cost: "Verification cost",
  verify_wait: "Verification wait",
  verify_queue_depth: "Verification queue depth",
  unverified_ratio: "Messages left unverified",
  full_cert_share: "Messages carrying a full certificate",
  envelope_overhead: "Security envelope overhead",
  revocation_latency_stage: "Revocation latency by stage",
  crl_entries: "CRL entries",
  crl_bytes: "CRL size",
  cert_pool_valid: "Valid pseudonym certificates held",
  backend_link_up: "Backend link availability",
  det_recall: "Detection recall",
  det_precision: "Detection precision",
  det_fpr: "False positive rate",
  det_accuracy: "Detection accuracy",
  det_f1: "Detection F1",
  time_to_detect: "Time to detect",
  time_to_decision: "Time to revocation decision",
  false_accusations: "False accusations",
  pseudonym_change_rate: "Pseudonym changes",
  linkability: "Linkability",
  mean_speed: "Mean speed",
  density: "Density",
  flow: "Flow",
  speed: "Speed",
  acceleration: "Acceleration",
  headway_time: "Time headway",
  headway_distance: "Distance headway",
  ttc_min: "Minimum time to collision",
  ttc_conflicts: "Time-to-collision conflicts",
  drac: "Deceleration to avoid a crash",
  pet: "Post-encroachment time",
  security_overhead: "Security overhead",
  net_header_overhead: "Network header overhead",
  link_overhead: "Link overhead",
  cert_bytes_share: "Certificate bytes share",
  air_bytes_per_payload_byte: "Air bytes per payload byte",
  bytes_per_vehicle_hour: "Bytes per vehicle-hour",
  bytes_total: "Bytes by path",
  events_processed: "Events processed",
  events_per_second: "Events per second",
  wall_clock_per_sim_second: "Wall clock per simulated second",
  memory_high_water_mark: "Memory high-water mark",
};

export function labelOf(base: string): string {
  const known = LABELS[base];
  if (known !== undefined) return known;
  const words = base.replace(/_/g, " ").trim();
  return words.length === 0 ? base : words[0].toUpperCase() + words.slice(1);
}

// ------------------------------------------------------------------------------------------------
// Which way is worse
// ------------------------------------------------------------------------------------------------

/** Whether a larger value is better, worse, or neither (a speed, a flow, a count of events). */
export type Polarity = "higher-better" | "lower-better" | "neutral";

const HIGHER_BETTER = new Set([
  "pdr",
  "pdr_all_pairs",
  "delivery_ratio",
  "goodput",
  "nar",
  "det_recall",
  "det_precision",
  "det_accuracy",
  "det_f1",
  "ttc_min",
  "pet",
  "headway_time",
  "headway_distance",
  "cert_pool_valid",
  "backend_link_up",
  "carried_load",
]);

const NEUTRAL = new Set([
  "mean_speed",
  "speed",
  "acceleration",
  "flow",
  "density",
  "offered_load",
  "verify_rate",
  "crl_entries",
  "pseudonym_change_rate",
  "events_processed",
  "events_per_second",
  "bytes_total",
]);

export function polarityOf(base: string): Polarity {
  if (HIGHER_BETTER.has(base)) return "higher-better";
  if (NEUTRAL.has(base)) return "neutral";
  if (/recall|precision|accuracy|delivery|pdr|availability|_up$/.test(base)) return "higher-better";
  return "lower-better";
}

/** How a ranking by this polarity is headed. */
export function worstFirstLabel(p: Polarity): string {
  return p === "higher-better" ? "lowest first" : p === "lower-better" ? "highest first" : "largest first";
}

/**
 * Sort rows worst first: the lowest delivery, the highest latency; a neutral metric largest first.
 * Rows without a value go last. Ties keep their key order, so the ranking does not shuffle between
 * two refreshes of the same numbers.
 */
export function worstFirst<T extends { readonly key: string; readonly value: number | null }>(rows: readonly T[], p: Polarity): T[] {
  const sign = p === "higher-better" ? 1 : -1;
  return [...rows].sort((a, b) => {
    if (a.value === null && b.value === null) return compareKeys(a.key, b.key);
    if (a.value === null) return 1;
    if (b.value === null) return -1;
    const d = sign * (a.value - b.value);
    return d !== 0 ? d : compareKeys(a.key, b.key);
  });
}

/** Numeric keys (node ids) in number order, the rest as text. */
export function compareKeys(a: string, b: string): number {
  // By the number a key starts with, as the engine orders groups (`introspect.rs` `group_order`):
  // node 9 before node 10, and the distance bin `50-100` before `100-150`.
  const lead = (k: string): number | null => {
    const m = /^\d+(?:\.\d+)?/.exec(k.trim());
    return m ? Number(m[0]) : null;
  };
  const na = lead(a);
  const nb = lead(b);
  if (na !== null && nb !== null && na !== nb) return na - nb;
  if (na !== null && nb === null) return -1;
  if (na === null && nb !== null) return 1;
  return a.localeCompare(b);
}

// ------------------------------------------------------------------------------------------------
// Dimensions
// ------------------------------------------------------------------------------------------------

const DIM_LABELS: Readonly<Record<string, string>> = {
  dist_bin: "By distance",
  node: "By node",
  msg_type: "By message type",
  stage: "By stage",
  rat: "By radio technology",
  cause: "By loss cause",
  channel: "By channel",
  class: "By vehicle class",
  radius: "By neighbourhood radius",
  region: "By region",
  primitive: "By cryptographic primitive",
  bucket: "By path",
  flow: "By flow",
  level: "By level (report or vehicle)",
  cell: "By confusion-matrix cell",
  detector: "By detector",
  protocol: "By credential protocol",
  tier: "By fidelity tier",
  density_bin: "By density",
};

export function dimLabel(dim: string): string {
  return DIM_LABELS[dim] ?? `By ${dim.replace(/_/g, " ")}`;
}

/** A distance-bin label (`20-40`, `1000+`) as its lower and upper edge in metres, else `null`. */
export function binEdges(label: string): [number, number] | null {
  const m = /^(\d+(?:\.\d+)?)(?:-(\d+(?:\.\d+)?)|\+)$/.exec(label);
  if (!m) return null;
  const lo = Number(m[1]);
  const hi = m[2] !== undefined ? Number(m[2]) : lo;
  return [lo, hi];
}

/** A grouped query's row: the dimension's value and the pooled figure with its interval. */
export interface GroupRow {
  readonly key: string;
  readonly value: number | null;
  readonly lo: number | null;
  readonly hi: number | null;
  readonly n: number;
}

/** A grouped `metrics.query` answer (`[key, v, v.lo, v.hi, v.n]` per row) as rows. */
export function groupRows(rows: readonly (readonly unknown[])[] | undefined): GroupRow[] {
  return (rows ?? []).map((row) => ({
    key: String(row[0]),
    value: typeof row[1] === "number" && Number.isFinite(row[1]) ? row[1] : null,
    lo: typeof row[2] === "number" && Number.isFinite(row[2]) ? row[2] : null,
    hi: typeof row[3] === "number" && Number.isFinite(row[3]) ? row[3] : null,
    n: typeof row[4] === "number" ? row[4] : 0,
  }));
}

// ------------------------------------------------------------------------------------------------
// Series: time in seconds, NaN for a window the metric was not observed in
// ------------------------------------------------------------------------------------------------

/** One series at the engine's resolution, time ascending. */
export interface Series {
  readonly t: Float64Array;
  readonly v: Float64Array;
}

export const EMPTY_SERIES: Series = { t: new Float64Array(0), v: new Float64Array(0) };

/**
 * A time-binned `metrics.query` answer (`[t_ns, v1, v2, …]` rows) as one series per column.
 * `null` (not observed) becomes NaN, which the charts draw as a gap and the statistics skip.
 */
export function seriesFromRows(rows: readonly (readonly unknown[])[] | undefined, columns: number): Series[] {
  const list = rows ?? [];
  const out: Series[] = [];
  for (let c = 0; c < columns; c++) {
    const t = new Float64Array(list.length);
    const v = new Float64Array(list.length);
    for (let i = 0; i < list.length; i++) {
      const row = list[i];
      t[i] = Number(row[0]) / 1e9;
      const x = row[c + 1];
      v[i] = typeof x === "number" && Number.isFinite(x) ? x : Number.NaN;
    }
    out.push({ t, v });
  }
  return out;
}

/**
 * Append `more` to `base`, replacing whatever `base` holds from `more`'s first instant on (the last
 * window of the previous fetch was still open, and a seek back rewrites the tail).
 */
export function appendSeries(base: Series, more: Series): Series {
  if (more.t.length === 0) return base;
  const first = more.t[0];
  let keep = base.t.length;
  while (keep > 0 && base.t[keep - 1] >= first) keep--;
  const t = new Float64Array(keep + more.t.length);
  const v = new Float64Array(keep + more.t.length);
  t.set(base.t.subarray(0, keep));
  v.set(base.v.subarray(0, keep));
  t.set(more.t, keep);
  v.set(more.v, keep);
  return { t, v };
}

/** The newest observed value, or `null`. */
export function latestOf(s: Series): number | null {
  for (let i = s.v.length - 1; i >= 0; i--) if (!Number.isNaN(s.v[i])) return s.v[i];
  return null;
}

/** How many windows carry a value. */
export function observedCount(s: Series): number {
  let n = 0;
  for (let i = 0; i < s.v.length; i++) if (!Number.isNaN(s.v[i])) n++;
  return n;
}

/**
 * The bin a query should ask for: a whole number of the engine's metric periods, wide enough that
 * `spanS` fits in `maxPoints`. Never finer than the period — the engine has nothing finer.
 */
export function chooseBinNs(spanS: number, periodNs: number, maxPoints: number): number {
  const period = Math.max(1, Math.round(periodNs));
  const want = (Math.max(0, spanS) * 1e9) / Math.max(1, maxPoints);
  return period * Math.max(1, Math.ceil(want / period));
}

// ------------------------------------------------------------------------------------------------
// Statistics of a range
// ------------------------------------------------------------------------------------------------

export interface RangeStats {
  /** Windows in the range that carry a value. */
  readonly n: number;
  readonly min: number;
  readonly mean: number;
  readonly p50: number;
  readonly p95: number;
  readonly p99: number;
  readonly max: number;
}

/** The type-7 quantile (R's default, and `v2xw-metrics`' own) of an ascending array. */
export function quantile7(sorted: ArrayLike<number>, q: number): number {
  const n = sorted.length;
  if (n === 0) return Number.NaN;
  if (n === 1) return sorted[0];
  const h = (n - 1) * q;
  const lo = Math.floor(h);
  const hi = Math.min(n - 1, lo + 1);
  return sorted[lo] + (h - lo) * (sorted[hi] - sorted[lo]);
}

/**
 * The statistics of the windows in `[from, to]` (seconds, inclusive), or `null` when none carries
 * a value. These are statistics of the per-window values the engine reported — for a delay, the
 * spread of the windows' means, not of individual messages; the page says so beside them.
 */
export function rangeStats(s: Series, from: number, to: number): RangeStats | null {
  const values: number[] = [];
  let sum = 0;
  // Kahan: an hour of a 1 s metric is 3,600 additions, and a mean that drifts in its last digit
  // would disagree with the CSV a reader recomputes it from.
  let c = 0;
  for (let i = 0; i < s.t.length; i++) {
    const t = s.t[i];
    if (t < from || t > to) continue;
    const x = s.v[i];
    if (Number.isNaN(x)) continue;
    values.push(x);
    const y = x - c;
    const next = sum + y;
    c = next - sum - y;
    sum = next;
  }
  if (values.length === 0) return null;
  const sorted = Float64Array.from(values).sort();
  return {
    n: sorted.length,
    min: sorted[0],
    mean: sum / sorted.length,
    p50: quantile7(sorted, 0.5),
    p95: quantile7(sorted, 0.95),
    p99: quantile7(sorted, 0.99),
    max: sorted[sorted.length - 1],
  };
}

// ------------------------------------------------------------------------------------------------
// Downsampling for display — never for the statistics or the export
// ------------------------------------------------------------------------------------------------

/**
 * Reduce `[from, to]` of a series to at most about `4 × buckets` points for drawing, keeping each
 * bucket's first, lowest, highest and last point in time order (M4). A line drawn from these
 * covers exactly the pixels the full series would, so a one-window spike in a ten-hour run is still
 * on screen. A bucket with no observed value becomes one gap (`null`), so an unobserved stretch is
 * not bridged by a line. Series of at most `4 × buckets` points in range are returned as they are.
 */
export function decimate(s: Series, from: number, to: number, buckets: number): [number[], (number | null)[]] {
  let i0 = lowerBound(s.t, from);
  let i1 = upperBound(s.t, to);
  // One point either side, so the line runs to the edge of the view instead of stopping short.
  if (i0 > 0) i0--;
  if (i1 < s.t.length) i1++;
  const n = i1 - i0;
  const xs: number[] = [];
  const ys: (number | null)[] = [];
  if (n <= 4 * Math.max(1, buckets)) {
    for (let i = i0; i < i1; i++) {
      xs.push(s.t[i]);
      ys.push(Number.isNaN(s.v[i]) ? null : s.v[i]);
    }
    return [xs, ys];
  }
  const span = Math.max(1e-12, s.t[i1 - 1] - s.t[i0]);
  let i = i0;
  for (let b = 0; b < buckets && i < i1; b++) {
    const edge = s.t[i0] + ((b + 1) * span) / buckets;
    let first = -1;
    let last = -1;
    let lo = -1;
    let hi = -1;
    const bucketStart = i;
    for (; i < i1 && (s.t[i] <= edge || b === buckets - 1); i++) {
      const x = s.v[i];
      if (Number.isNaN(x)) continue;
      if (first < 0) first = i;
      last = i;
      if (lo < 0 || x < s.v[lo]) lo = i;
      if (hi < 0 || x > s.v[hi]) hi = i;
    }
    if (first < 0) {
      if (i > bucketStart) {
        xs.push(s.t[bucketStart]);
        ys.push(null);
      }
      continue;
    }
    for (const k of [...new Set([first, lo, hi, last])].sort((a, z) => a - z)) {
      xs.push(s.t[k]);
      ys.push(s.v[k]);
    }
  }
  return [xs, ys];
}

/** First index with `t[i] >= x`. */
export function lowerBound(t: Float64Array, x: number): number {
  let lo = 0;
  let hi = t.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (t[mid] < x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

/** First index with `t[i] > x`. */
export function upperBound(t: Float64Array, x: number): number {
  let lo = 0;
  let hi = t.length;
  while (lo < hi) {
    const mid = (lo + hi) >> 1;
    if (t[mid] <= x) lo = mid + 1;
    else hi = mid;
  }
  return lo;
}

// ------------------------------------------------------------------------------------------------
// Numbers with their units
// ------------------------------------------------------------------------------------------------

/** A number as a reader should see it: enough digits to compare, never a trailing `e-7`. */
export function formatNumber(v: number | null | undefined): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "—";
  const a = Math.abs(v);
  if (a === 0) return "0";
  // A whole number reads as one, at any size: an axis tick of 200 m is "200", not "200.0".
  if (Number.isInteger(v) && a < 1e15) return v.toLocaleString("en-US");
  if (a >= 1e6) return v.toLocaleString("en-US", { maximumFractionDigits: 0 });
  if (a >= 1000) return v.toLocaleString("en-US", { maximumFractionDigits: 0 });
  if (a >= 100) return v.toFixed(1);
  if (a >= 10) return v.toFixed(2);
  if (a >= 0.001) return v.toPrecision(3).replace(/(\.\d*?)0+$/, "$1").replace(/\.$/, "");
  return v.toExponential(2);
}

/** A unit as it reads after a number (`ms`, `m/s`); a dimensionless ratio reads as nothing. */
export function unitSuffix(unit: string): string {
  const u = unit.trim();
  if (u === "" || u === "1" || u === "ratio") return "";
  return u;
}

/** `0.942` for a ratio, `12.4 ms` for a delay, `1,204 count` for a count. */
export function formatWithUnit(v: number | null | undefined, unit: string): string {
  const n = formatNumber(v);
  const u = unitSuffix(unit);
  return n === "—" || u === "" ? n : `${n} ${u}`;
}

/** The unit an axis names: a ratio says its scale, because 0.9 and 90 % are different numbers. */
export function axisUnit(unit: string): string {
  const u = unit.trim();
  if (u === "ratio") return "ratio (0–1)";
  if (u === "" || u === "1") return "unitless";
  return u;
}

// ------------------------------------------------------------------------------------------------
// CSV
// ------------------------------------------------------------------------------------------------

function csvCell(x: string | number | null): string {
  if (x === null) return "";
  if (typeof x === "number") return Number.isFinite(x) ? String(x) : "";
  return /[",\n\r]/.test(x) ? `"${x.replace(/"/g, '""')}"` : x;
}

/** RFC 4180: comma-separated, CRLF line ends, quoted where a cell needs it; numbers at full precision. */
export function toCsv(header: readonly string[], rows: readonly (readonly (string | number | null)[])[]): string {
  return [header, ...rows].map((r) => r.map(csvCell).join(",")).join("\r\n") + "\r\n";
}

/**
 * The CSV of a range of several series, at the engine's full resolution: one row per instant any
 * of them has, `t_s` first, an empty cell where a series has no window at that instant.
 */
export function seriesCsv(names: readonly string[], units: readonly string[], series: readonly Series[], from: number, to: number): string {
  const times = new Set<number>();
  for (const s of series) for (let i = 0; i < s.t.length; i++) if (s.t[i] >= from && s.t[i] <= to) times.add(s.t[i]);
  const sorted = [...times].sort((a, b) => a - b);
  const index = series.map((s) => {
    const m = new Map<number, number>();
    for (let i = 0; i < s.t.length; i++) m.set(s.t[i], s.v[i]);
    return m;
  });
  const header = ["t_s", ...names.map((n, i) => ((units[i] ?? "").trim() === "" ? n : `${n} (${units[i]})`))];
  const rows = sorted.map((t) => [t, ...index.map((m) => {
    const v = m.get(t);
    return v === undefined || Number.isNaN(v) ? null : v;
  })]);
  return toCsv(header, rows);
}

// ------------------------------------------------------------------------------------------------
// Addresses: every chart and breakdown can be linked to
// ------------------------------------------------------------------------------------------------

/**
 * Where the dashboard is: a metric expanded or not, one of its breakdowns, a time range, a group,
 * a search.
 *
 * The address is the URL hash, so a link opens the page at exactly that view. The forms, stable
 * from this version on (the agent harness writes them):
 *
 *   #metrics                                   the dashboard
 *   #metrics?group=latency                     scrolled to a group
 *   #metrics?q=verify                          with a search
 *   #metrics/e2e_latency                       one metric, expanded
 *   #metrics/e2e_latency.p95                   the same card, that series first
 *   #metrics/pdr/dist_bin                      a breakdown of it
 *   #metrics/pdr/node?from=30&to=90            over simulated seconds 30 to 90
 *
 * A name is percent-encoded (`latency_stage%5Bairtime%5D`), times are simulated seconds.
 */
export interface MetricsRoute {
  readonly metric: string | null;
  readonly breakdown: string | null;
  readonly from: number | null;
  readonly to: number | null;
  readonly group: string | null;
  readonly q: string | null;
}

export const DASHBOARD: MetricsRoute = { metric: null, breakdown: null, from: null, to: null, group: null, q: null };

/** The route a hash names, or `null` when the hash is not the dashboard's. */
export function parseMetricsHash(hash: string): MetricsRoute | null {
  const h = hash.replace(/^#/, "");
  const [path, query = ""] = splitOnce(h, "?");
  const parts = path.split("/");
  if (parts[0] !== "metrics") return null;
  const params = new URLSearchParams(query);
  const num = (k: string): number | null => {
    const raw = params.get(k);
    if (raw === null || raw.trim() === "") return null;
    const x = Number(raw);
    return Number.isFinite(x) && x >= 0 ? x : null;
  };
  const decode = (s: string | undefined): string | null => {
    if (s === undefined || s === "") return null;
    try {
      return decodeURIComponent(s);
    } catch {
      return s;
    }
  };
  let from = num("from");
  let to = num("to");
  if (from !== null && to !== null && to < from) [from, to] = [to, from];
  return {
    metric: decode(parts[1]),
    breakdown: decode(parts[2]),
    from,
    to,
    group: params.get("group"),
    q: params.get("q"),
  };
}

/** The hash for a route: `metricsHash({...DASHBOARD, metric: "pdr", breakdown: "dist_bin"})`. */
export function metricsHash(route: Partial<MetricsRoute>): string {
  let path = "metrics";
  if (route.metric) {
    path += `/${encodeURIComponent(route.metric)}`;
    if (route.breakdown) path += `/${encodeURIComponent(route.breakdown)}`;
  }
  const params = new URLSearchParams();
  if (route.metric && route.from !== null && route.from !== undefined) params.set("from", trimSeconds(route.from));
  if (route.metric && route.to !== null && route.to !== undefined) params.set("to", trimSeconds(route.to));
  if (!route.metric && route.group) params.set("group", route.group);
  if (!route.metric && route.q) params.set("q", route.q);
  const query = params.toString();
  return `#${path}${query === "" ? "" : `?${query}`}`;
}

function trimSeconds(s: number): string {
  return String(Math.round(s * 1000) / 1000);
}

function splitOnce(s: string, sep: string): [string, string | undefined] {
  const i = s.indexOf(sep);
  return i < 0 ? [s, undefined] : [s.slice(0, i), s.slice(i + 1)];
}

// ------------------------------------------------------------------------------------------------
// Search
// ------------------------------------------------------------------------------------------------

/** Whether a card matches a search: every word in its name, label, group, unit or definition. */
export function matchesSearch(f: MetricFamily, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter((w) => w !== "");
  if (words.length === 0) return true;
  const group = GROUPS.find((g) => g.id === f.group)?.label ?? "";
  const hay = [f.base, f.label, group, f.unit, f.definition, f.source ?? "", ...f.breakdowns, ...seriesOf(f)].join(" ").toLowerCase();
  return words.every((w) => hay.includes(w));
}

// ------------------------------------------------------------------------------------------------
// Chart helpers
// ------------------------------------------------------------------------------------------------

/** About `count` round ticks across `[lo, hi]`. */
export function niceTicks(lo: number, hi: number, count: number): number[] {
  const span = hi - lo;
  if (!(span > 0)) return [lo];
  const raw = span / Math.max(1, count);
  const mag = 10 ** Math.floor(Math.log10(raw));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * mag).find((s) => s >= raw) ?? 10 * mag;
  const out: number[] = [];
  // Integer multiples of the step, trimmed to twelve digits: 3 × 0.2 is 0.6000000000000001.
  for (let k = Math.ceil(lo / step - 1e-9); k * step <= hi + step * 1e-9; k++) out.push(Number((k * step).toPrecision(12)));
  return out;
}

/** Equal-width bins over the values' range. */
export function histogram(values: readonly number[], bins: number): { edges: number[]; counts: number[] } {
  if (values.length === 0) return { edges: [], counts: [] };
  let lo = Number.POSITIVE_INFINITY;
  let hi = Number.NEGATIVE_INFINITY;
  for (const v of values) {
    lo = Math.min(lo, v);
    hi = Math.max(hi, v);
  }
  if (hi - lo < 1e-12) return { edges: [lo, hi], counts: [values.length] };
  const n = Math.max(1, Math.min(bins, Math.ceil(Math.sqrt(values.length)) * 2));
  const w = (hi - lo) / n;
  const counts = new Array<number>(n).fill(0);
  for (const v of values) counts[Math.min(n - 1, Math.floor((v - lo) / w))]++;
  return { edges: Array.from({ length: n + 1 }, (_, i) => lo + i * w), counts };
}

// ------------------------------------------------------------------------------------------------
// What a breakdown actually pooled
// ------------------------------------------------------------------------------------------------

/**
 * What a breakdown's header says it pooled, and the caveat when that is not what was asked.
 *
 * `asked` is the range in view (`toS` null: up to now). `pooled` is the engine's report of the
 * earliest and latest sample instants it pooled, and the block size of any older samples it keeps
 * merged (`live.rs` `BreakdownStore`); `null` when the engine does not report one. A sample's
 * instant is its window's, so the pooled span starts up to a metric period after the asked one:
 * that is not worth a caveat, and `periodS` sets the tolerance.
 */
export function pooledText(
  asked: { readonly fromS: number; readonly toS: number | null },
  pooled: { readonly fromS: number; readonly toS: number; readonly blockS: number } | null,
  periodS: number,
): { span: string; caveat: string | null } {
  const askedText = `${formatNumber(asked.fromS)}–${asked.toS === null ? "now" : formatNumber(asked.toS)} s`;
  if (pooled === null) return { span: askedText, caveat: null };
  const span = `${formatNumber(pooled.fromS)}–${formatNumber(pooled.toS)} s`;
  const askedTo = asked.toS ?? Number.POSITIVE_INFINITY;
  const tolerance = Math.max(2 * periodS, 1e-9);
  if (pooled.blockS > 0) {
    return {
      span,
      caveat: `Asked for ${askedText}. On a run this long the engine keeps its older breakdown samples merged into ${formatNumber(pooled.blockS)} s blocks (a block counts when its middle is in the range), so this breakdown pools ${span}. The chart and statistics above use every window.`,
    };
  }
  if (pooled.fromS > asked.fromS + tolerance || pooled.toS > askedTo + tolerance) {
    return { span, caveat: `Asked for ${askedText}; the samples of this breakdown in that range span ${span}.` };
  }
  return { span: askedText, caveat: null };
}
