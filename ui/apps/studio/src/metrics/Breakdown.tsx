/**
 * A metric's breakdowns: every dimension its catalogue row declares, pooled over the range in view
 * by the engine (a grouped `metrics.query`, §6.12).
 *
 * The pooling is exact, not an average of window values: a proportion pools its successes and
 * trials and carries the Wilson interval of the pooled count, a ratio of sums pools its sums, a
 * distribution pools its sample totals (`crates/v2xw-server/src/live.rs`, `Pool`). So a thin
 * distance bin shows a wide interval rather than a confident point, and a bin nobody reached shows
 * nothing.
 *
 * What each dimension looks like:
 *
 *  * **distance** — the metric against distance with its 95 % band, the way TR 36.885 plots PRR;
 *  * **node** — how the nodes spread (a histogram), then the nodes worst first, each one a link to
 *    follow it in the viewport; thousands of nodes are paged, not drawn;
 *  * **anything else** (message type, stage, cause, channel, radius…) — one bar per value, with its
 *    interval where it has one. Stages are in the order a message lives them.
 *
 * A metric that reports a dimension only together with another (a latency stage per message type)
 * offers the other under "Within".
 */

import { useMemo, useState } from "react";

import { closePanel } from "../shell/route.js";
import { engine } from "../state/engine.js";
import { downloadCsv, fileName } from "./exporting.js";
import { useGroups } from "./hooks.js";
import {
  axisUnit,
  binEdges,
  compareKeys,
  dimLabel,
  formatNumber,
  formatWithUnit,
  histogram,
  metricsHash,
  niceTicks,
  toCsv,
  worstFirst,
  worstFirstLabel,
  type GroupRow,
  type MetricFamily,
} from "./model.js";

/** The latency decomposition's stages, in the order a message lives them (`V2V_STAGES`). */
const STAGE_ORDER = [
  "sign_queue",
  "sign",
  "mac_aifs",
  "mac_backoff",
  "mac_defer",
  "airtime",
  "propagation",
  "reception",
  "verify_queue",
  "verify",
];

/** Dimensions whose values have an order of their own, drawn in it rather than ranked. */
const ORDINAL = new Set(["stage", "radius", "channel", "density_bin", "dist_bin", "level"]);

function ordered(dim: string, rows: readonly GroupRow[], f: MetricFamily): GroupRow[] {
  if (dim === "stage") {
    const at = (k: string): number => {
      const i = STAGE_ORDER.indexOf(k);
      return i < 0 ? STAGE_ORDER.length : i;
    };
    return [...rows].sort((a, b) => at(a.key) - at(b.key) || a.key.localeCompare(b.key));
  }
  if (ORDINAL.has(dim)) return [...rows].sort((a, b) => compareKeys(a.key, b.key));
  return worstFirst(rows, f.polarity);
}

export interface Range {
  readonly fromS: number;
  /** `null`: up to where the run is now, following it. */
  readonly toS: number | null;
}

function rangeNs(r: Range): { fromNs: number; toNs?: number } {
  return { fromNs: Math.round(r.fromS * 1e9), ...(r.toS === null ? {} : { toNs: Math.round(r.toS * 1e9) }) };
}

/** The values of a metric's other dimensions, for the "Within" choice. */
function useWithin(f: MetricFamily, dim: string, range: Range, firstKey: string | null, active: boolean): { dim: string; key: string }[] {
  const others = f.breakdowns.filter((d) => d !== dim && d !== "node" && d !== "dist_bin").slice(0, 2);
  const where = firstKey === null ? {} : { [dim]: firstKey };
  const a = useGroups(others[0] !== undefined && firstKey !== null ? f.base : null, others[0] ?? "", rangeNs(range), where, active);
  const b = useGroups(others[1] !== undefined && firstKey !== null ? f.base : null, others[1] ?? "", rangeNs(range), where, active);
  return [
    ...(others[0] !== undefined ? a.rows.map((r) => ({ dim: others[0], key: r.key })) : []),
    ...(others[1] !== undefined ? b.rows.map((r) => ({ dim: others[1], key: r.key })) : []),
  ];
}

export function BreakdownSection({
  f,
  dim,
  range,
  active,
  highlighted,
}: {
  f: MetricFamily;
  dim: string;
  range: Range;
  active: boolean;
  highlighted: boolean;
}): React.JSX.Element {
  const [within, setWithin] = useState<{ dim: string; key: string } | null>(null);
  const where = within === null ? {} : { [within.dim]: within.key };
  const { rows, status, error, stale } = useGroups(f.base, dim, rangeNs(range), where, active);
  const shown = useMemo(() => ordered(dim, rows, f).filter((r) => r.value !== null || r.n > 0), [dim, rows, f]);
  const firstKey = rows.length > 0 ? rows[0].key : null;
  const withinOptions = useWithin(f, dim, range, firstKey, active);
  const [copied, setCopied] = useState(false);

  const rangeText = `${formatNumber(range.fromS)}–${range.toS === null ? "now" : `${formatNumber(range.toS)}`} s`;
  const exportCsv = (): void => {
    const csv = toCsv(
      [dim, f.unit === "" ? f.base : `${f.base} (${f.unit})`, "lo_95", "hi_95", "n"],
      ordered(dim, rows, f).map((r) => [r.key, r.value, r.lo, r.hi, r.n]),
    );
    downloadCsv(fileName([f.base, "by", dim, within ? `${within.dim}-${within.key}` : "", `${formatNumber(range.fromS)}-${range.toS === null ? "end" : formatNumber(range.toS)}s`], "csv"), csv);
  };
  const link = (): void => {
    const hash = metricsHash({ metric: f.base, breakdown: dim, from: range.fromS, to: range.toS });
    const url = `${location.origin}${location.pathname}${location.search}${hash}`;
    void navigator.clipboard?.writeText(url).then(
      () => {
        setCopied(true);
        setTimeout(() => setCopied(false), 1500);
      },
      () => undefined,
    );
  };

  let body: React.ReactNode;
  if (status === "error") {
    body = <p className="dim bd-empty">The engine did not answer this breakdown{error ? `: ${error}` : ""}.</p>;
  } else if (shown.length === 0) {
    body = (
      <p className="dim bd-empty" data-testid={`breakdown-empty-${dim}`}>
        {status === "loading"
          ? "Asking the engine…"
          : `Nothing in ${rangeText} was measured ${dimLabel(dim).toLowerCase()}${within ? ` within ${within.dim} = ${within.key}` : ""}${
              withinOptions.length > 0 && within === null ? ". This metric reports it together with another dimension; choose one under Within." : "."
            }`}
      </p>
    );
  } else if (dim === "dist_bin") {
    body = <DistanceChart rows={shown} f={f} />;
  } else if (dim === "node") {
    body = <NodeBreakdown rows={shown} f={f} />;
  } else {
    body = <Bars rows={shown} f={f} dim={dim} />;
  }

  return (
    <section
      id={`metric-breakdown-${dim}`}
      className={`bd${highlighted ? " highlighted" : ""}${stale ? " stale" : ""}`}
      data-testid={`breakdown-${dim}`}
      aria-busy={stale}
    >
      <header className="bd-head">
        <h4>{dimLabel(dim)}</h4>
        <span className="dim">
          pooled over {rangeText}
          {dim === "node" ? ` · ${worstFirstLabel(f.polarity)}` : ""}
        </span>
        <span className="grow" />
        {withinOptions.length > 0 || within !== null ? (
          <label className="bd-within">
            <span className="dim">Within</span>
            <select
              value={within === null ? "" : `${within.dim}=${within.key}`}
              onChange={(e) => {
                const v = e.target.value;
                if (v === "") setWithin(null);
                else {
                  const i = v.indexOf("=");
                  setWithin({ dim: v.slice(0, i), key: v.slice(i + 1) });
                }
              }}
              data-testid={`breakdown-within-${dim}`}
            >
              <option value="">all</option>
              {within !== null && !withinOptions.some((o) => o.dim === within.dim && o.key === within.key) ? (
                <option value={`${within.dim}=${within.key}`}>{`${within.dim} = ${within.key}`}</option>
              ) : null}
              {withinOptions.map((o) => (
                <option key={`${o.dim}=${o.key}`} value={`${o.dim}=${o.key}`}>{`${o.dim} = ${o.key}`}</option>
              ))}
            </select>
          </label>
        ) : null}
        <button type="button" className="small" onClick={exportCsv} disabled={rows.length === 0} data-testid={`breakdown-csv-${dim}`} title="The pooled rows, with their intervals and sample counts">
          CSV
        </button>
        <button type="button" className="small" onClick={link} title="Copy a link to this breakdown over this range">
          {copied ? "Copied" : "Link"}
        </button>
      </header>
      {body}
    </section>
  );
}

/** Values in `[0, 1]` with an interval, as TR 36.885 draws PRR: against distance, with a band. */
function DistanceChart({ rows, f }: { rows: readonly GroupRow[]; f: MetricFamily }): React.JSX.Element {
  const W = 640;
  const H = 220;
  const P = { l: 52, r: 12, t: 10, b: 34 };
  const points = rows
    .map((r) => ({ r, e: binEdges(r.key) }))
    .filter((p): p is { r: GroupRow; e: [number, number] } => p.e !== null && p.r.value !== null)
    .map(({ r, e }) => ({ x: e[1] > e[0] ? (e[0] + e[1]) / 2 : e[0], y: r.value as number, lo: r.lo, hi: r.hi, n: r.n, key: r.key }));
  const unbinned = rows.filter((r) => binEdges(r.key) === null);
  const xmax = Math.max(50, ...points.map((p) => p.x)) * 1.05;
  const ys = points.flatMap((p) => [p.y, p.lo ?? p.y, p.hi ?? p.y]);
  const bounded = f.unit === "ratio";
  const ymin = bounded ? 0 : Math.min(0, ...ys);
  const ymax = bounded ? 1 : Math.max(1e-9, ...ys) * 1.05;
  const sx = (x: number): number => P.l + (x / xmax) * (W - P.l - P.r);
  const sy = (y: number): number => P.t + (1 - (y - ymin) / Math.max(1e-12, ymax - ymin)) * (H - P.t - P.b);
  const line = points.map((p, i) => `${i === 0 ? "M" : "L"}${sx(p.x)},${sy(p.y)}`).join("");
  const banded = points.length > 1 && points.every((p) => p.lo !== null && p.hi !== null);
  const band = banded
    ? `${points.map((p, i) => `${i === 0 ? "M" : "L"}${sx(p.x)},${sy(p.hi as number)}`).join("")}${[...points]
        .reverse()
        .map((p) => `L${sx(p.x)},${sy(p.lo as number)}`)
        .join("")}Z`
    : null;
  const xt = niceTicks(0, xmax, 6);
  const yt = niceTicks(ymin, ymax, 5);
  return (
    <div className="bd-chart">
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={`${f.label} against distance`} data-testid="breakdown-distance-chart">
        {yt.map((y) => (
          <g key={`y${y}`}>
            <line x1={P.l} x2={W - P.r} y1={sy(y)} y2={sy(y)} className="grid" />
            <text x={P.l - 6} y={sy(y) + 4} textAnchor="end" className="axis">
              {formatNumber(y)}
            </text>
          </g>
        ))}
        {xt.map((x) => (
          <text key={`x${x}`} x={sx(x)} y={H - P.b + 16} textAnchor="middle" className="axis">
            {formatNumber(x)}
          </text>
        ))}
        <line x1={P.l} x2={W - P.r} y1={H - P.b} y2={H - P.b} className="baseline" />
        <text x={(P.l + W - P.r) / 2} y={H - 4} textAnchor="middle" className="axis">
          distance between sender and receiver (m)
        </text>
        <text x={12} y={(P.t + H - P.b) / 2} textAnchor="middle" className="axis" transform={`rotate(-90 12 ${(P.t + H - P.b) / 2})`}>
          {axisUnit(f.unit)}
        </text>
        {band ? <path d={band} className="band" /> : null}
        <path d={line} className="line" />
        {points.map((p) => (
          <g key={p.key} className="pt">
            <circle cx={sx(p.x)} cy={sy(p.y)} r={4} className="dot" />
            <circle cx={sx(p.x)} cy={sy(p.y)} r={12} className="hit">
              <title>
                {`${p.key} m: ${formatWithUnit(p.y, f.unit)}${p.lo !== null && p.hi !== null ? ` (95 % interval ${formatNumber(p.lo)}–${formatNumber(p.hi)})` : ""}, over ${p.n.toLocaleString("en-US")} samples`}
              </title>
            </circle>
          </g>
        ))}
      </svg>
      <p className="dim bd-note">
        {banded ? "The band is the 95 % Wilson interval of the pooled count. " : ""}
        Each point is a distance bin, drawn at its centre. Hover a point for its interval and sample count; the CSV has every bin.
        {unbinned.length > 0 ? ` Not drawn: ${unbinned.map((r) => `${r.key} (${formatWithUnit(r.value, f.unit)})`).join(", ")}.` : ""}
      </p>
    </div>
  );
}

/** One bar per value, with its interval where the pooled figure has one. */
function Bars({ rows, f, dim }: { rows: readonly GroupRow[]; f: MetricFamily; dim: string }): React.JSX.Element {
  const max = Math.max(1e-12, ...rows.map((r) => Math.max(Math.abs(r.value ?? 0), Math.abs(r.hi ?? 0))));
  const shown = rows.slice(0, 40);
  return (
    <div className="bd-bars" role="table" aria-label={`${f.label} ${dimLabel(dim).toLowerCase()}`}>
      {shown.map((r) => (
        <div key={r.key} className="bd-bar-row" role="row" data-testid={`breakdown-row-${dim}-${r.key}`}>
          <span className="bd-key mono" role="rowheader" title={r.key}>
            {r.key}
          </span>
          <span className="bd-track" role="cell">
            {r.value !== null ? <span className="bd-bar" style={{ width: `${(Math.abs(r.value) / max) * 100}%` }} /> : null}
            {r.lo !== null && r.hi !== null ? (
              <span className="bd-whisker" style={{ left: `${(Math.abs(r.lo) / max) * 100}%`, width: `${(Math.abs(r.hi - r.lo) / max) * 100}%` }} title="95 % interval" />
            ) : null}
          </span>
          <span className="bd-val mono" role="cell" title={`${r.n.toLocaleString("en-US")} samples${r.lo !== null && r.hi !== null ? `; 95 % interval ${formatNumber(r.lo)}–${formatNumber(r.hi)}` : ""}`}>
            {formatWithUnit(r.value, f.unit)}
          </span>
          <span className="bd-n dim mono" role="cell">
            n {r.n.toLocaleString("en-US")}
          </span>
        </div>
      ))}
      {rows.length > shown.length ? <p className="dim bd-note">{rows.length - shown.length} more in the CSV.</p> : null}
    </div>
  );
}

const NODE_PAGE = 25;

/** Follow a node in the viewport: its vehicle if it has one, else the radio itself. */
function followNode(node: number): void {
  for (const [actor, n] of engine.nodeByActor) {
    if (n === node) {
      void engine.selectActor(actor, "chase");
      closePanel();
      return;
    }
  }
  void engine.selectNode(node);
  closePanel();
}

/** How the nodes spread, and the worst of them. */
function NodeBreakdown({ rows, f }: { rows: readonly GroupRow[]; f: MetricFamily }): React.JSX.Element {
  const [limit, setLimit] = useState(NODE_PAGE);
  const [find, setFind] = useState("");
  const ranked = useMemo(() => worstFirst(rows, f.polarity), [rows, f.polarity]);
  const hist = useMemo(() => histogram(rows.map((r) => r.value).filter((v): v is number => v !== null), 24), [rows]);
  const list = find.trim() === "" ? ranked.slice(0, limit) : ranked.filter((r) => r.key === find.trim());
  const rank = new Map(ranked.map((r, i) => [r.key, i + 1]));
  const hmax = Math.max(1, ...hist.counts);
  return (
    <div className="bd-nodes">
      {hist.counts.length > 1 ? (
        <figure className="bd-hist" aria-label={`How ${rows.length} nodes spread`}>
          <div className="bd-hist-bars">
            {hist.counts.map((c, i) => (
              <span
                key={i}
                className="bd-hist-bar"
                style={{ height: `${(c / hmax) * 100}%` }}
                title={`${c} node${c === 1 ? "" : "s"} between ${formatNumber(hist.edges[i])} and ${formatNumber(hist.edges[i + 1])}${f.unit === "" ? "" : ` ${f.unit}`}`}
              />
            ))}
          </div>
          <figcaption className="dim">
            <span>{formatWithUnit(hist.edges[0], f.unit)}</span>
            <span className="grow" />
            <span>
              {rows.length.toLocaleString("en-US")} nodes · {axisUnit(f.unit)}
            </span>
            <span className="grow" />
            <span>{formatWithUnit(hist.edges[hist.edges.length - 1], f.unit)}</span>
          </figcaption>
        </figure>
      ) : null}
      <div className="bd-node-tools">
        <input
          type="search"
          placeholder="Find a node id"
          value={find}
          onChange={(e) => setFind(e.target.value)}
          aria-label="Find a node by id"
          data-testid="breakdown-node-find"
        />
      </div>
      <table className="bd-table">
        <thead>
          <tr>
            <th>rank</th>
            <th>node</th>
            <th>{f.unit === "" ? "value" : f.unit}</th>
            <th>95 % interval</th>
            <th>samples</th>
            <th />
          </tr>
        </thead>
        <tbody>
          {list.map((r) => (
            <tr key={r.key} data-testid={`breakdown-node-${r.key}`}>
              <td className="mono dim">{rank.get(r.key)}</td>
              <td className="mono">{r.key}</td>
              <td className="mono">{formatNumber(r.value)}</td>
              <td className="mono dim">{r.lo !== null && r.hi !== null ? `${formatNumber(r.lo)}–${formatNumber(r.hi)}` : "—"}</td>
              <td className="mono dim">{r.n.toLocaleString("en-US")}</td>
              <td>
                <button type="button" className="small" onClick={() => followNode(Number(r.key))} title="Close the metrics and follow this node in the viewport">
                  Follow
                </button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      {find.trim() === "" && ranked.length > limit ? (
        <button type="button" className="small" onClick={() => setLimit((l) => l + 100)}>
          Show 100 more of {ranked.length.toLocaleString("en-US")}
        </button>
      ) : null}
      {find.trim() !== "" && list.length === 0 ? <p className="dim bd-note">No node {find.trim()} in this range.</p> : null}
    </div>
  );
}
