/**
 * Where the dashboard's numbers come from: the engine's own store, asked with `metrics.query`
 * (§6.12), never the page's stream buffer.
 *
 * # Why the query and not the stream
 *
 * The stream (`MetricSample` frames, §3.7) reaches a page only while its socket is open, and only
 * from the moment it opened. The page keeps the last 900 samples of each series in a ring. So a
 * page opened half-way through a run, a page reloaded after one (the last QA found it showed
 * nothing), and any run longer than fifteen minutes all had an incomplete picture — and a
 * dashboard that shows statistics of a range must have the whole range. The engine keeps every
 * sample of every series for the run (`LiveEngine::history`), answers `metrics.query` over the
 * socket and over `POST /rpc` alike, and bins on request. So:
 *
 *  * the **overview** asks for every visible card's series binned to about 240 points across the
 *    run so far — a display downsample made by the engine, one call per refresh;
 *  * an **expanded** chart asks for its series at the engine's full resolution (one value per
 *    metric period), incrementally, and keeps them: the statistics and the CSV are computed from
 *    those, and only the drawing is decimated (`model.ts` `decimate`);
 *  * a **breakdown** is a grouped query over the brushed range, which the engine pools exactly
 *    (successes and trials, sums, sample totals — `live.rs` `Pool`).
 *
 * Every call is `quiet`: a refresh that fails is the dashboard's to explain in words, not an
 * error in the user's log.
 */

import type { MetricsQueryParams, MetricsQueryResult } from "@vwp/protocol";

import { compare } from "../state/compare.js";
import { engine } from "../state/engine.js";
import { groupRows, seriesFromRows, EMPTY_SERIES, type GroupRow, type Series, type SeriesDef } from "./model.js";

/** Which run a query goes to: this page's engine, or the comparison's side B. */
export type Side = "a" | "b";

/** The engine's metric period, from the `Hello` (§3.1.1); 1 s when no stream has said. */
export function metricPeriodNs(side: Side = "a"): number {
  const hello = side === "a" ? engine.client?.hello : compare.hello;
  const p = hello ? Number(hello.metricPeriodNs) : 0;
  return p > 0 ? p : 1_000_000_000;
}

/** Whether side B is an engine that can answer metric queries. */
export function sideBAvailable(): boolean {
  return compare.source === "engine" && (compare.client !== null || compare.baseUrl !== "");
}

async function query(side: Side, params: MetricsQueryParams): Promise<MetricsQueryResult> {
  if (side === "a") return engine.request("metrics.query", params, { quiet: true });
  const client = compare.client;
  if (client !== null && client.state === "streaming") return client.request("metrics.query", params);
  if (compare.baseUrl === "") throw new Error("the comparison has no engine to ask");
  const res = await fetch(`${compare.baseUrl}/rpc`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: Date.now(), method: "metrics.query", params }),
  });
  if (!res.ok) throw new Error(`side B answered ${res.status}`);
  const body = (await res.json()) as { result?: MetricsQueryResult; error?: { message?: string } };
  if (body.error) throw new Error(body.error.message ?? "side B refused the query");
  return body.result as MetricsQueryResult;
}

/** The catalogue: every series the run can report, with unit, dimensions, definition and source. */
export async function fetchCatalogue(side: Side = "a"): Promise<SeriesDef[]> {
  const res = await query(side, {});
  return (res.catalogue ?? []).map((c) => {
    const source = c.source && typeof c.source["ref"] === "string" ? (c.source["ref"] as string) : undefined;
    return {
      name: c.name,
      base: c.base ?? c.name,
      unit: c.unit ?? "",
      dims: c.dims ?? [],
      agg: c.agg ?? "",
      visibility: c.visibility ?? "",
      definition: c.definition_md ?? "",
      ...(source ? { source } : {}),
      notAccounted: c.not_accounted ?? [],
    };
  });
}

export interface SeriesRequest {
  readonly fromNs: number;
  /** Omitted: up to where the run is shown. */
  readonly toNs?: number;
  readonly binNs: number;
  readonly limit?: number;
}

/**
 * Several series over one range, one per name, in the order asked.
 *
 * One call for all of them; if the engine refuses it (a name it does not know, a ground-truth
 * metric in a node-profile session), each name is asked on its own so one refusal does not blank
 * every chart. A name the engine refuses comes back empty, and `refused` says which.
 */
export async function fetchSeries(
  names: readonly string[],
  req: SeriesRequest,
  side: Side = "a",
): Promise<{ series: Map<string, Series>; refused: Set<string> }> {
  const series = new Map<string, Series>();
  const refused = new Set<string>();
  if (names.length === 0) return { series, refused };
  const params = (list: readonly string[]): MetricsQueryParams => ({
    metrics: [...list],
    t_from_ns: Math.max(0, Math.floor(req.fromNs)),
    ...(req.toNs !== undefined ? { t_to_ns: Math.max(0, Math.ceil(req.toNs)) } : {}),
    bin_ns: Math.max(1, Math.round(req.binNs)),
    limit: req.limit ?? 10_000,
  });
  try {
    const res = await query(side, params(names));
    const cols = seriesFromRows(res.rows, names.length);
    names.forEach((n, i) => series.set(n, cols[i] ?? EMPTY_SERIES));
    return { series, refused };
  } catch {
    // Fall through to one name at a time.
  }
  await Promise.all(
    names.map(async (n) => {
      try {
        const res = await query(side, params([n]));
        series.set(n, seriesFromRows(res.rows, 1)[0] ?? EMPTY_SERIES);
      } catch {
        refused.add(n);
        series.set(n, EMPTY_SERIES);
      }
    }),
  );
  return { series, refused };
}

/**
 * The simulated span a grouped answer's rows actually pool, in seconds, when the engine says. On a
 * long, dense run the engine keeps its oldest breakdown samples merged into time blocks, so what it
 * pooled can be coarser or narrower than what was asked. `blockS` is 0 when no block was used.
 */
export interface PooledSpan {
  readonly fromS: number;
  readonly toS: number;
  readonly blockS: number;
}

/** A grouped answer's pooled span, or `null` when the engine reports none (or pooled nothing). */
export function pooledSpan(res: Pick<MetricsQueryResult, "pooled_from_ns" | "pooled_to_ns" | "pooled_block_ns">): PooledSpan | null {
  const from = res.pooled_from_ns;
  const to = res.pooled_to_ns;
  if (typeof from !== "number" || typeof to !== "number" || !Number.isFinite(from) || !Number.isFinite(to)) return null;
  const block = typeof res.pooled_block_ns === "number" && res.pooled_block_ns > 0 ? res.pooled_block_ns : 0;
  return { fromS: from / 1e9, toS: to / 1e9, blockS: block / 1e9 };
}

/**
 * One metric grouped by one dimension over `[fromNs, toNs]`, pooled by the engine. `where` pins the
 * metric's other dimensions (a message type for a stage breakdown). An engine that has no
 * breakdowns, or a metric that has none of this dimension, answers no rows.
 */
export async function fetchGroups(
  metric: string,
  dim: string,
  range: { fromNs: number; toNs?: number },
  where: Readonly<Record<string, string>> = {},
  side: Side = "a",
): Promise<{ rows: GroupRow[]; pooled: PooledSpan | null }> {
  const res = await query(side, {
    metrics: [metric],
    group_by: [dim as "t"],
    ...(Object.keys(where).length > 0 ? { where: { ...where } } : {}),
    t_from_ns: Math.max(0, Math.floor(range.fromNs)),
    ...(range.toNs !== undefined ? { t_to_ns: Math.max(0, Math.ceil(range.toNs)) } : {}),
    limit: 100_000,
  });
  return { rows: groupRows(res.rows), pooled: pooledSpan(res) };
}
