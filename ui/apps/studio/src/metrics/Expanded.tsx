/**
 * One metric, expanded: a large chart with a time brush, the statistics of the range in view,
 * every breakdown the metric supports, what the metric is and where it comes from, and export.
 *
 * With a second run open for comparison (the Compare panel, an engine on another port) the chart
 * overlays the two runs, on the same simulated time, and the statistics say how they differ.
 */

import { useEffect, useMemo, useRef, useState } from "react";
import type uPlot from "uplot";

import { metricSubject } from "../lib/provenance.js";
import { closePanel } from "../shell/route.js";
import { useStudio } from "../state/store.js";
import { BreakdownSection, type Range } from "./Breakdown.js";
import { Brush, MainChart, type ChartLine } from "./Chart.js";
import { slotColour } from "./tokens.js";
import { sideBAvailable } from "./data.js";
import { chartPng, download, downloadCsv, fileName } from "./exporting.js";
import { useFullSeries, useGroups } from "./hooks.js";
import { usePins } from "./pins.js";
import {
  axisUnit,
  dimLabel,
  formatNumber,
  formatWithUnit,
  metricsHash,
  latestOf,
  observedCount,
  rangeStats,
  seriesCsv,
  seriesOf,
  unitSuffix,
  EMPTY_SERIES,
  GROUPS,
  type MetricFamily,
  type MetricsRoute,
  type RangeStats,
  type Series,
} from "./model.js";

/** How a series reads in a chip and a legend. */
function seriesLabel(f: MetricFamily, name: string): string {
  if (name === f.base) return f.agg === "mean" && f.percentiles.length > 0 ? "mean" : f.base;
  if (name.startsWith(`${f.base}.`)) return name.slice(f.base.length + 1);
  if (name.startsWith(`${f.base}[`)) return `${f.valueDim ?? "value"} = ${name.slice(f.base.length + 1, -1)}`;
  return name;
}

/** The colour slot a series always has: its place in the metric's series list, 1–8. */
function slotOf(f: MetricFamily, name: string): number {
  const i = seriesOf(f).indexOf(name);
  return (Math.max(0, i) % 8) + 1;
}

/** The series shown when a metric opens: the one the address names, else the headline and a p95. */
function initialSelection(f: MetricFamily, requested: string | null): string[] {
  const all = seriesOf(f);
  if (requested !== null && all.includes(requested) && requested !== f.base) return [requested];
  if (f.headline !== null) {
    const p95 = f.percentiles.find((p) => p.endsWith(".p95"));
    return p95 ? [f.headline, p95] : [f.headline];
  }
  return f.values.slice(0, 4).map((v) => v.series);
}

const MAX_LINES = 8;

/** One identity for "no filter", so a hook keyed on it does not see a new object every render. */
const NO_FILTER: Readonly<Record<string, string>> = Object.freeze({});

export function Expanded({
  f,
  route,
  setRoute,
  active,
  onBack,
  rat,
}: {
  f: MetricFamily;
  route: MetricsRoute;
  setRoute: (r: Partial<MetricsRoute>) => void;
  active: boolean;
  onBack: () => void;
  /** The run's radio technology (`radio.rat`), when the scenario says. */
  rat: string | null;
}): React.JSX.Element {
  const run = useStudio((s) => s.run);
  const hello = useStudio((s) => s.hello);
  const compareView = useStudio((s) => s.compare);
  const pinned = usePins((s) => s.pins.includes(f.base));
  const togglePin = usePins((s) => s.toggle);
  const [selected, setSelected] = useState<string[]>(() => initialSelection(f, route.metric));
  const [overlay, setOverlay] = useState(false);
  const [table, setTable] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const plotRef = useRef<uPlot | null>(null);

  useEffect(() => setSelected(initialSelection(f, route.metric)), [f.base]); // eslint-disable-line react-hooks/exhaustive-deps

  const bReady = compareView !== null && compareView.state === "ready" && sideBAvailable();
  const comparing = overlay && bReady;
  // Comparing overlays one series of each run: two runs × several series is a legend nobody reads.
  const lines = useMemo(() => (comparing ? selected.slice(0, 1) : selected), [comparing, selected]);
  const a = useFullSeries(lines, active);
  const b = useFullSeries(comparing ? lines : [], active && comparing, "b");

  // The time the run has reached, and the range in view.
  const lastA = Math.max(0, ...lines.map((n) => lastT(a.series.get(n))));
  const extent = Math.max(run.tNs / 1e9, lastA, comparing ? Math.max(0, ...lines.map((n) => lastT(b.series.get(n)))) : 0, 1e-3);
  const from = route.from ?? 0;
  const to = route.to ?? extent;
  const following = route.to === null;
  const range: Range = { fromS: from, toS: following ? null : to };

  const chartLines: ChartLine[] = useMemo(() => {
    const out: ChartLine[] = lines.map((n) => ({
      name: n,
      label: comparing ? `this run · ${seriesLabel(f, n)}` : seriesLabel(f, n),
      slot: comparing ? 1 : slotOf(f, n),
      series: a.series.get(n) ?? EMPTY_SERIES,
    }));
    if (comparing) {
      for (const n of lines) {
        out.push({ name: `B:${n}`, label: `run B · ${seriesLabel(f, n)}`, slot: 2, series: b.series.get(n) ?? EMPTY_SERIES });
      }
    }
    return out;
  }, [lines, comparing, a.series, b.series, f]);

  const stats = useMemo(
    () =>
      chartLines.map((l) => ({ line: l, stats: rangeStats(l.series, from, to) })),
    [chartLines, from, to],
  );
  const hasData = chartLines.some((l) => observedCount(l.series) > 0);
  // With no window to draw, whether anything was measured at all: a metric too thin to report in
  // any one window (fewer samples than its minimum) can still pool to a value over the run, which
  // is a different finding from "nothing exercised it". Asked only when there is nothing to draw.
  const thin = useGroups(
    !hasData && a.status === "ready" && f.breakdowns.length > 0 ? f.base : null,
    f.breakdowns[0] ?? "",
    { fromNs: Math.round(from * 1e9), ...(following ? {} : { toNs: Math.round(to * 1e9) }) },
    NO_FILTER,
    active,
  );
  const pooledSamples = thin.status === "ready" ? thin.rows.reduce((s, r) => s + r.n, 0) : 0;

  const setRange = (a0: number, b0: number): void => {
    const lo = Math.max(0, Math.min(a0, b0));
    const hi = Math.min(extent, Math.max(a0, b0));
    if (hi - lo <= 0) return;
    setRoute({ from: lo, to: hi >= extent - 1e-9 && run.state === "running" ? null : hi });
  };
  const resetRange = (): void => setRoute({ from: null, to: null });

  const toggleSeries = (name: string): void => {
    setSelected((s) => {
      if (comparing) return [name];
      if (s.includes(name)) return s.length === 1 ? s : s.filter((x) => x !== name);
      if (s.length >= MAX_LINES) return s;
      // Two series never share a colour: one that would is swapped for the other.
      const slot = slotOf(f, name);
      return [...s.filter((x) => slotOf(f, x) !== slot), name];
    });
  };

  const exportCsv = (): void => {
    const names = chartLines.map((l) => l.name);
    const units = chartLines.map(() => f.unit);
    const csv = seriesCsv(names, units, chartLines.map((l) => l.series), from, to);
    downloadCsv(fileName([f.base, `${formatNumber(from)}-${formatNumber(to)}s`], "csv"), csv);
    setNote(`CSV saved: ${chartLines.length} series at the engine's full resolution, ${formatNumber(from)}–${formatNumber(to)} s.`);
  };
  const exportPng = async (): Promise<void> => {
    const p = plotRef.current;
    if (!p) return;
    const blob = await chartPng(
      p.ctx.canvas,
      `${f.label} (${f.base})`,
      `${hello?.scenarioName ?? "run"} · ${formatNumber(from)}–${formatNumber(to)} s · ${axisUnit(f.unit)}`,
      chartLines.map((l) => ({ label: l.label, colour: slotColour(l.slot) })),
    );
    if (blob) {
      download(fileName([f.base, `${formatNumber(from)}-${formatNumber(to)}s`], "png"), blob);
      setNote("PNG saved.");
    } else setNote("This browser could not encode the chart as a PNG.");
  };
  const copyLink = (): void => {
    const url = `${location.origin}${location.pathname}${location.search}${metricsHash({ ...route, metric: route.metric ?? f.base })}`;
    void navigator.clipboard?.writeText(url).then(
      () => setNote("Link copied: it opens this chart over this range."),
      () => setNote(url),
    );
  };

  // A breakdown the address names is scrolled into view once it exists.
  useEffect(() => {
    if (route.breakdown === null) return;
    const el = document.getElementById(`metric-breakdown-${route.breakdown}`);
    el?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, [route.breakdown, f.base]);

  const group = GROUPS.find((g) => g.id === f.group);
  const all = seriesOf(f);

  return (
    <div className="mx" data-testid="metric-expanded" data-metric={f.base}>
      <div className="mx-head">
        <button type="button" className="small" onClick={onBack} data-testid="metric-back" title="Back to all metrics (Esc)">
          ← All metrics
        </button>
        <div className="mx-title">
          <h3>{f.label}</h3>
          <span className="mono dim">{f.base}</span>
          <span className="dim">· {group?.label}</span>
          {f.groundTruth ? <span className="gt-tag">GT</span> : null}
        </div>
        <span className="grow" />
        <button type="button" className={`small${pinned ? " active" : ""}`} aria-pressed={pinned} onClick={() => togglePin(f.base)} data-testid="metric-pin-expanded">
          {pinned ? "★ Pinned" : "☆ Pin"}
        </button>
        <button type="button" className={`small${table ? " active" : ""}`} aria-pressed={table} onClick={() => setTable((t) => !t)} data-testid="metric-table-toggle" disabled={!hasData}>
          Table
        </button>
        <button type="button" className="small" onClick={exportCsv} data-testid="metric-export-csv" disabled={!hasData} title="The series in view at the engine's full resolution">
          CSV
        </button>
        <button type="button" className="small" onClick={() => void exportPng()} data-testid="metric-export-png" disabled={!hasData} title="The chart as an image, titled">
          PNG
        </button>
        <button type="button" className="small" onClick={copyLink} data-testid="metric-copy-link" title="Copy a link to this chart over this range">
          Link
        </button>
      </div>
      {note !== null ? (
        <p className="mx-note dim" role="status" data-testid="metric-note">
          {note}
        </p>
      ) : null}

      <div className="mx-controls">
        {all.length > 1 ? (
          <div className="mx-chips" role="group" aria-label="Series to draw">
            {all.map((n) => {
              const on = lines.includes(n);
              return (
                <button
                  key={n}
                  type="button"
                  className={`chip${on ? " on" : ""}`}
                  aria-pressed={on}
                  onClick={() => toggleSeries(n)}
                  data-testid={`metric-series-${n}`}
                  title={n}
                >
                  <i style={{ background: on ? slotColour(comparing ? 1 : slotOf(f, n)) : "transparent" }} />
                  {seriesLabel(f, n)}
                </button>
              );
            })}
          </div>
        ) : null}
        <span className="grow" />
        {compareView !== null ? (
          <label className="check mx-compare" title={bReady ? "Overlay the same measurement from run B (the Compare panel's second engine)" : compareView.source === "replay" ? "Run B is a recording, which holds poses and signals but no metric samples" : "Run B is not ready"}>
            <input type="checkbox" checked={comparing} disabled={!bReady} onChange={(e) => setOverlay(e.target.checked)} data-testid="metric-compare" />
            Overlay run B
          </label>
        ) : null}
      </div>

      <div className="mx-range" role="group" aria-label="Time range">
        <button type="button" className={`small${route.from === null && route.to === null ? " active" : ""}`} onClick={resetRange} data-testid="metric-range-all">
          Whole run
        </button>
        {[60, 600, 3600].filter((w) => extent > w * 1.5).map((w) => (
          <button key={w} type="button" className="small" onClick={() => setRoute({ from: Math.max(0, extent - w), to: null })}>
            Last {w < 3600 ? `${w / 60 >= 1 ? `${w / 60} min` : `${w} s`}` : "hour"}
          </button>
        ))}
        <span className="dim" data-testid="metric-range-text">
          {formatNumber(from)}–{formatNumber(to)} s of {formatNumber(extent)} s{following && run.state === "running" ? ", following the run" : ""}
        </span>
        <span className="grow" />
        <span className="dim">Drag across the chart or the strip below to zoom; double-click for the whole run.</span>
      </div>

      {hasData ? (
        <>
          <MainChart
            lines={chartLines}
            from={from}
            to={to}
            unit={f.unit}
            onRange={setRange}
            onReset={resetRange}
            onPlot={(p) => {
              plotRef.current = p;
            }}
          />
          <Brush series={chartLines[0]?.series ?? null} extent={extent} from={from} to={to} onRange={setRange} onReset={resetRange} />
        </>
      ) : (
        <p className="mx-empty dim" data-testid="metric-expanded-empty">
          {a.status === "loading"
            ? "Asking the engine for this metric's full series…"
            : lines.some((n) => a.refused.has(n))
              ? "The engine refused this measurement for this session. A node-profile session sees no ground truth."
              : pooledSamples > 0
                ? `No single window had enough samples to report a value — each needs the metric's minimum sample count — but the engine measured ${pooledSamples.toLocaleString("en-US")} samples ${dimLabel(f.breakdowns[0] ?? "").toLowerCase()} over this range. The breakdowns below pool them. A denser or longer run gives the chart windows of its own.`
                : run.state === "finished"
                  ? "The run finished without producing this measurement: nothing in the scenario exercised it."
                  : "No window has had enough samples of this measurement yet. The chart appears once one has."}
        </p>
      )}

      {hasData ? <StatsTable rows={stats} unit={f.unit} comparing={comparing} from={from} to={to} /> : null}

      {table && hasData ? <ValueTable lines={chartLines} unit={f.unit} from={from} to={to} /> : null}

      <div className="mx-sections">
        {f.breakdowns.length > 0 ? (
          <div className="mx-breakdowns">
            <h4 className="mx-h">Breakdowns</h4>
            {f.breakdowns.map((d) => (
              <BreakdownSection key={d} f={f} dim={d} range={range} active={active} highlighted={route.breakdown === d} />
            ))}
          </div>
        ) : (
          <p className="dim">This metric has no breakdown: the engine measures it for the whole run only.</p>
        )}
        {rat !== null && f.group !== "traffic" && f.group !== "simulator" ? (
          <p className="dim bd-note" data-testid="metric-technology">
            By radio technology: this run is all <code>{rat}</code>. The engine runs one technology per run (<code>radio.rat</code>), as a
            deployment does on one channel. To compare technologies, run the scenario with each and overlay the two runs here (Compare,
            then Overlay run B).
          </p>
        ) : null}
        <About f={f} latest={lines.length > 0 ? latestOf(a.series.get(lines[0]) ?? EMPTY_SERIES) : null} />
      </div>
    </div>
  );
}

function lastT(s: Series | undefined): number {
  return s && s.t.length > 0 ? s.t[s.t.length - 1] : 0;
}

const STAT_KEYS = ["min", "mean", "p50", "p95", "p99", "max"] as const;

function StatsTable({
  rows,
  unit,
  comparing,
  from,
  to,
}: {
  rows: readonly { line: ChartLine; stats: RangeStats | null }[];
  unit: string;
  comparing: boolean;
  from: number;
  to: number;
}): React.JSX.Element {
  const delta = comparing && rows.length === 2 && rows[0].stats && rows[1].stats ? { a: rows[0].stats, b: rows[1].stats } : null;
  return (
    <div className="mx-stats">
      <table className="bd-table" data-testid="metric-stats">
        <thead>
          <tr>
            <th>series</th>
            <th title="Windows in the range that carry a value">windows</th>
            {STAT_KEYS.map((k) => (
              <th key={k}>{k}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map(({ line, stats }) => (
            <tr key={line.name} data-testid={`metric-stats-${line.name}`}>
              <td>
                <i className="key" style={{ background: slotColour(line.slot) }} />
                {line.label}
              </td>
              <td className="mono">{stats ? stats.n.toLocaleString("en-US") : "0"}</td>
              {STAT_KEYS.map((k) => (
                <td key={k} className="mono" data-stat={k}>
                  {stats ? formatNumber(stats[k]) : "—"}
                </td>
              ))}
            </tr>
          ))}
          {delta ? (
            <tr className="delta">
              <td>run B − this run</td>
              <td />
              {STAT_KEYS.map((k) => {
                const d = delta.b[k] - delta.a[k];
                return (
                  <td key={k} className="mono">
                    {d > 0 ? "+" : ""}
                    {formatNumber(d)}
                  </td>
                );
              })}
            </tr>
          ) : null}
        </tbody>
      </table>
      <p className="dim bd-note">
        Over {formatNumber(from)}–{formatNumber(to)} s, in {unitSuffix(unit) === "" ? axisUnit(unit) : unit}. These are statistics of the values the engine reported per window (one
        per metric period): for a delay, the spread of the windows&apos; means, not of individual messages — the p95 series is the per-message percentile. Computed from the full series,
        not from what is drawn.
      </p>
    </div>
  );
}

const TABLE_ROWS = 500;

function ValueTable({ lines, unit, from, to }: { lines: readonly ChartLine[]; unit: string; from: number; to: number }): React.JSX.Element {
  const rows = useMemo(() => {
    const times = new Set<number>();
    for (const l of lines) for (let i = 0; i < l.series.t.length; i++) if (l.series.t[i] >= from && l.series.t[i] <= to) times.add(l.series.t[i]);
    const sorted = [...times].sort((x, y) => x - y);
    const maps = lines.map((l) => {
      const m = new Map<number, number>();
      for (let i = 0; i < l.series.t.length; i++) m.set(l.series.t[i], l.series.v[i]);
      return m;
    });
    return { total: sorted.length, list: sorted.slice(0, TABLE_ROWS).map((t) => ({ t, vs: maps.map((m) => m.get(t)) })) };
  }, [lines, from, to]);
  return (
    <div className="mx-table" data-testid="metric-value-table">
      <table className="bd-table">
        <thead>
          <tr>
            <th>t (s)</th>
            {lines.map((l) => (
              <th key={l.name}>{l.label}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.list.map((r) => (
            <tr key={r.t}>
              <td className="mono">{formatNumber(r.t)}</td>
              {r.vs.map((v, i) => (
                <td key={lines[i].name} className="mono">
                  {v === undefined || Number.isNaN(v) ? "—" : formatWithUnit(v, unit)}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
      {rows.total > TABLE_ROWS ? <p className="dim bd-note">The first {TABLE_ROWS} of {rows.total.toLocaleString("en-US")} windows; the CSV has all of them.</p> : null}
    </div>
  );
}

/** Inline Markdown the catalogue's definitions use: `code` spans, and paragraphs. */
function MarkdownLite({ text }: { text: string }): React.JSX.Element {
  return (
    <>
      {text
        .split(/\n\s*\n/)
        .filter((p) => p.trim() !== "")
        .map((p, i) => (
          <p key={i}>
            {p.split(/(`[^`]+`)/).map((part, j) =>
              part.startsWith("`") && part.endsWith("`") && part.length > 1 ? <code key={j}>{part.slice(1, -1)}</code> : <span key={j}>{part}</span>,
            )}
          </p>
        ))}
    </>
  );
}

function About({ f, latest }: { f: MetricFamily; latest: number | null }): React.JSX.Element {
  const provenance = useStudio((s) => s.metricProvenance);
  const name = f.headline ?? seriesOf(f)[0] ?? f.base;
  // The inspector's Why tab: the model, its version and the parameter set that produced this
  // metric. The inspector sits under this panel, so the panel closes to show it.
  const why = (): void => {
    useStudio.getState().setWhy(metricSubject(name, latest, f.unit, provenance[name]));
    closePanel();
  };
  return (
    <section className="mx-about" data-testid="metric-about" aria-label="What this metric is">
      <h4 className="mx-h">What it is</h4>
      <MarkdownLite text={f.definition === "" ? "The engine published no definition for this metric." : f.definition} />
      <button type="button" className="small" onClick={why} data-testid="metric-why" title="Close the metrics and show, in the inspector, the model, version and parameters behind this measurement">
        Which model produced it
      </button>
      <dl>
        <dt>Unit</dt>
        <dd>{axisUnit(f.unit)}</dd>
        <dt>Reduced by</dt>
        <dd className="mono">{f.agg || "—"}</dd>
        <dt>Visibility</dt>
        <dd>
          {f.groundTruth
            ? "Ground truth: the simulator knows it; no deployed device could measure it."
            : "Node: what a deployed device could measure itself."}
        </dd>
        <dt>Source</dt>
        <dd data-testid="metric-source">{f.source ?? "The engine cites no source for this metric."}</dd>
        {f.breakdowns.length > 0 ? (
          <>
            <dt>Broken down</dt>
            <dd>{f.breakdowns.map((d) => dimLabel(d).replace(/^By /, "by ")).join(", ")}</dd>
          </>
        ) : null}
        <dt>Series</dt>
        <dd className="mono">{seriesOf(f).join(", ")}</dd>
      </dl>
      {f.notAccounted.length > 0 ? (
        <>
          <h5>What it does not account for</h5>
          <ul>
            {f.notAccounted.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </>
      ) : null}
    </section>
  );
}
