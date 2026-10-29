/**
 * One metric, expanded, laid out for a researcher scanning fast:
 *
 *   1. the headline: name, the newest value large with its unit and a health chip, and one short
 *      line of verdict ("Above 600 ms limit · unhealthy 83 % of run · worst 834 ms at 17 s");
 *   2. the chart, full width, with the health band shaded and labelled on its edge, a hover
 *      readout, drag to zoom and a time brush under it;
 *   3. one compact strip of statistics over the range in view;
 *   4. related metrics as a row of chips, then the breakdowns the metric has (none: no section);
 *   5. "About this metric", collapsed: a one-sentence definition, and inside it the thresholds and
 *      their source, unit, visibility, the model and what the metric does not account for.
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
import { classify, judge, judgedLabel, judgedSeries, verdictParts, type HealthLevel } from "./health.js";
import { BandTable, HealthChip } from "./HealthBadge.js";
import { slotColour } from "./tokens.js";
import { sideBAvailable } from "./data.js";
import { chartPng, download, downloadCsv, fileName } from "./exporting.js";
import { useFullSeries, useGroups } from "./hooks.js";
import { usePins } from "./pins.js";
import {
  axisUnit,
  dimLabel,
  clockText,
  dataView,
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
  // A metric with a health band opens on the series the band judges, so its zones are drawn.
  if (f.health) {
    const judged = judgedSeries(f.base, f.health);
    if (all.includes(judged)) {
      if (f.headline !== null && judged !== f.headline) return [f.headline, judged];
      if (f.headline === null) return [judged];
    }
  }
  if (f.headline !== null) {
    const p95 = f.percentiles.find((p) => p.endsWith(".p95"));
    return p95 ? [f.headline, p95] : [f.headline];
  }
  return f.values.slice(0, 4).map((v) => v.series);
}

/** The first sentence of a definition, for the collapsed "About" line. */
function firstSentence(text: string): string {
  const para = text.split(/\n\s*\n/)[0]?.trim() ?? "";
  // A full stop that ends a sentence: followed by a space and a capital, or the paragraph's end.
  const m = /^([\s\S]*?[.!?])(?:\s+(?=[A-Z(`])|$)/.exec(para);
  return (m ? m[1] : para).replace(/\s+/g, " ");
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
  related = [],
}: {
  f: MetricFamily;
  route: MetricsRoute;
  setRoute: (r: Partial<MetricsRoute>) => void;
  active: boolean;
  onBack: () => void;
  /** The run's radio technology (`radio.rat`), when the scenario says. */
  rat: string | null;
  /** The metrics that explain or qualify this one and that the run measures (`model.ts` `relatedOf`). */
  related?: readonly MetricFamily[];
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

  // Another metric (a related one, a link): its own series, and no note about the last one's export.
  useEffect(() => {
    setSelected(initialSelection(f, route.metric));
    setNote(null);
  }, [f.base]); // eslint-disable-line react-hooks/exhaustive-deps

  // An export's note is a confirmation, not a fixture: it goes after a few seconds.
  useEffect(() => {
    if (note === null) return;
    const t = setTimeout(() => setNote(null), 6000);
    return () => clearTimeout(t);
  }, [note]);

  const bReady = compareView !== null && compareView.state === "ready" && sideBAvailable();
  const comparing = overlay && bReady;
  // Comparing overlays one series of each run: two runs × several series is a legend nobody reads.
  const lines = useMemo(() => (comparing ? selected.slice(0, 1) : selected), [comparing, selected]);
  const a = useFullSeries(lines, active);
  const b = useFullSeries(comparing ? lines : [], active && comparing, "b");

  // The series the headline value is read from: the one the health band judges, else the first drawn.
  const band = f.health;
  const judgedName = band ? judgedSeries(f.base, band) : null;
  const judgedReported = judgedName !== null && seriesOf(f).includes(judgedName);
  const heroName = judgedReported ? (judgedName as string) : (lines[0] ?? f.headline ?? seriesOf(f)[0] ?? f.base);
  const heroNames = useMemo(() => [heroName], [heroName]);
  const hero = useFullSeries(heroNames, active);
  const heroSeries = hero.series.get(heroName) ?? a.series.get(heroName) ?? EMPTY_SERIES;

  // The time the run has reached, and the range in view.
  const lastA = Math.max(0, ...lines.map((n) => lastT(a.series.get(n))));
  const extent = Math.max(run.tNs / 1e9, lastA, comparing ? Math.max(0, ...lines.map((n) => lastT(b.series.get(n)))) : 0, 1e-3);
  // A fast-forwarded run reports only while its radio runs: over the whole run its line is a sliver
  // at the left. With no range asked for, the chart then opens on the windows that reported, and
  // "Whole run" is one click away (`model.ts` `dataView`).
  const heroView = useMemo(() => dataView(a.series.get(lines[0] ?? "") ?? heroSeries, extent), [a.series, lines, heroSeries, extent]);
  const dataFirst = route.from === null && route.to === null && heroView.zoomed;
  const from = route.from ?? (dataFirst ? heroView.from : 0);
  const to = route.to ?? (dataFirst ? heroView.to : extent);
  const following = route.to === null && !dataFirst;
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

  const stats = useMemo(() => chartLines.map((l) => ({ line: l, stats: rangeStats(l.series, from, to) })), [chartLines, from, to]);
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
    setNote(`CSV saved: ${chartLines.length} series, full resolution, ${formatNumber(from)}–${formatNumber(to)} s.`);
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
  const windows = [60, 600, 3600].filter((w) => extent > w * 1.5);
  const wholeRun = (route.from === null && route.to === null && !dataFirst) || (route.from === 0 && route.to !== null && route.to >= extent - 1e-6);

  return (
    <div className="mx" data-testid="metric-expanded" data-metric={f.base}>
      <div className="mx-topbar">
        <button type="button" className="mx-back" onClick={onBack} data-testid="metric-back" title="Back to all metrics (Esc)">
          ← All metrics
        </button>
        {group ? <span className="mx-crumb">/ {group.label}</span> : null}
        <span className="grow" />
        {note !== null ? (
          <span className="mx-note" role="status" data-testid="metric-note">
            {note}
          </span>
        ) : null}
        <div className="mx-actions" role="toolbar" aria-label="Metric actions">
          <button
            type="button"
            className={`mx-act pin${pinned ? " on" : ""}`}
            aria-pressed={pinned}
            onClick={() => togglePin(f.base)}
            data-testid="metric-pin-expanded"
            title={pinned ? "Unpin from the dashboard" : "Pin to the top of the dashboard"}
          >
            {pinned ? "★ Pinned" : "☆ Pin"}
          </button>
          <span className="mx-act-group">
            <button type="button" className={`mx-act${table ? " on" : ""}`} aria-pressed={table} onClick={() => setTable((t) => !t)} data-testid="metric-table-toggle" disabled={!hasData} title="Show every window's value as a table">
              Table
            </button>
            <button type="button" className="mx-act" onClick={exportCsv} data-testid="metric-export-csv" disabled={!hasData} title="Download the series in view at the engine's full resolution">
              CSV
            </button>
            <button type="button" className="mx-act" onClick={() => void exportPng()} data-testid="metric-export-png" disabled={!hasData} title="Download the chart as a titled image">
              PNG
            </button>
            <button type="button" className="mx-act" onClick={copyLink} data-testid="metric-copy-link" title="Copy a link to this chart over this range">
              Link
            </button>
          </span>
        </div>
      </div>

      <Headline f={f} seriesName={heroName} series={heroSeries} judged={judgedReported} extent={extent} />

      <section className="mx-plot" aria-label="Chart">
      <div className="mx-toolbar">
        {all.length > 1 ? (
          <div className="mx-chips" role="group" aria-label="Series to draw">
            {all.map((n) => {
              const on = lines.includes(n);
              return (
                <button
                  key={n}
                  type="button"
                  className={`mx-chip${on ? " on" : ""}`}
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
        <div className="mx-range" role="group" aria-label="Time range">
          <span className="mx-seg">
            {heroView.zoomed ? (
              <button
                type="button"
                className={dataFirst ? "on" : ""}
                aria-pressed={dataFirst}
                onClick={resetRange}
                data-testid="metric-range-data"
                title={`This metric reported only between ${clockText(heroView.from)} and ${clockText(heroView.to)} of the ${clockText(extent)} run`}
              >
                Windows with data
              </button>
            ) : null}
            <button
              type="button"
              className={wholeRun ? "on" : ""}
              aria-pressed={wholeRun}
              onClick={heroView.zoomed ? () => setRoute({ from: 0, to: extent }) : resetRange}
              data-testid="metric-range-all"
            >
              Whole run
            </button>
            {windows.map((w) => (
              <button key={w} type="button" onClick={() => setRoute({ from: Math.max(0, extent - w), to: null })}>
                Last {w < 3600 ? `${w / 60 >= 1 ? `${w / 60} min` : `${w} s`}` : "hour"}
              </button>
            ))}
          </span>
          <span className="mx-range-text" data-testid="metric-range-text" title="Drag across the chart or the strip under it to zoom; double-click the chart for the whole run.">
            {formatNumber(from)}–{formatNumber(to)} s of {formatNumber(extent)} s{following && run.state === "running" ? ", following the run" : ""}
            {dataFirst ? " (the windows that reported)" : ""}
          </span>
        </div>
      </div>

      {hasData ? (
        <>
          <MainChart
            band={band && chartLines.some((l) => l.name === judgedName) ? band : null}
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
        <p className="mx-empty" data-testid="metric-expanded-empty">
          {a.status === "loading"
            ? "Asking the engine for this metric's full series…"
            : lines.some((n) => a.refused.has(n))
              ? "The engine refused this measurement for this session. A node-profile session sees no ground truth."
              : pooledSamples > 0
                ? `No single window had enough samples to report a value, but the engine measured ${pooledSamples.toLocaleString("en-US")} samples ${dimLabel(f.breakdowns[0] ?? "").toLowerCase()} over this range. The breakdowns below pool them; a denser or longer run gives the chart windows of its own.`
                : run.state === "finished"
                  ? "The run finished without producing this measurement: nothing in the scenario exercised it."
                  : "No window has had enough samples of this measurement yet. The chart appears once one has."}
        </p>
      )}
      </section>

      {hasData ? <StatsStrip rows={stats} unit={f.unit} comparing={comparing} from={from} to={to} /> : null}

      {table && hasData ? <ValueTable lines={chartLines} unit={f.unit} from={from} to={to} /> : null}

      {related.length > 0 ? (
        <nav className="mx-related" aria-label="Related metrics" data-testid="metric-related">
          <span className="mx-label">Related</span>
          {related.map((r) => (
            <button
              key={r.base}
              type="button"
              className="mx-chip"
              onClick={() => setRoute({ metric: r.base, breakdown: null })}
              title={`${r.definition === "" ? r.label : firstSentence(r.definition)} (over the same range)`}
              data-testid={`metric-related-${r.base}`}
            >
              {r.label}
            </button>
          ))}
        </nav>
      ) : null}

      {f.breakdowns.length > 0 ? (
        <section className="mx-breakdowns" aria-label="Breakdowns">
          <h4 className="mx-h">Breakdowns</h4>
          {f.breakdowns.map((d) => (
            <BreakdownSection key={d} f={f} dim={d} range={range} active={active} highlighted={route.breakdown === d} />
          ))}
        </section>
      ) : null}

      <About f={f} rat={rat} latest={latestOf(heroSeries)} judgedName={judgedName} judgedReported={judgedReported} />
    </div>
  );
}

function lastT(s: Series | undefined): number {
  return s && s.t.length > 0 ? s.t[s.t.length - 1] : 0;
}

/**
 * The headline: the metric's name, its newest value large with its unit, a health chip and one
 * short verdict line. Without a health band, the value and when it was measured.
 */
function Headline({
  f,
  seriesName,
  series,
  judged,
  extent,
}: {
  f: MetricFamily;
  seriesName: string;
  series: Series;
  judged: boolean;
  extent: number;
}): React.JSX.Element {
  const band = f.health;
  const latest = latestOf(series);
  const level: HealthLevel | null = band && judged ? classify(band, latest) : null;
  const j = useMemo(() => (band && judged ? judge(f.base, f.unit, band, series, f.label) : null), [band, judged, f.base, f.unit, f.label, series]);
  const stat = band && judged ? judgedLabel(band) : seriesName === f.base ? "" : seriesLabel(f, seriesName);
  const whole = useMemo(() => (level === null ? rangeStats(series, 0, Math.max(extent, lastT(series))) : null), [level, series, extent]);
  let parts: string[] = [];
  if (band && level !== null) parts = verdictParts(band, level, f.unit, j);
  else if (latest !== null) {
    parts = [`newest window at ${formatNumber(lastT(series))} s`];
    if (whole) parts.push(`run mean ${formatWithUnit(whole.mean, f.unit)}`, `max ${formatWithUnit(whole.max, f.unit)}`);
  }
  const unit = unitSuffix(f.unit);
  return (
    <header className="mx-headline">
      <div className="mx-name">
        <h3>{f.label}</h3>
        <span className="mono mx-base">{f.base}</span>
        {f.groundTruth ? (
          <span className="gt-tag" title="Ground truth: the simulator knows it; no deployed device could measure it">
            GT
          </span>
        ) : null}
      </div>
      {latest !== null ? (
        <>
          <div className="mx-reading">
            <span className="mx-value" data-testid="metric-hero-value" title={`The newest window of ${seriesName}`}>
              {formatNumber(latest)}
            </span>
            {unit !== "" ? <span className="mx-unit">{unit}</span> : null}
            {stat !== "" ? <span className="mx-stat">{stat}</span> : null}
            {band && level !== null ? <HealthChip band={band} level={level} unit={f.unit} testId="metric-health-now" large /> : null}
          </div>
          <p className="mx-verdict" data-testid="metric-health-run">
            {parts.map((p, i) => (
              <span key={i} className={i === 0 && level !== null ? `lead ${level}` : undefined}>
                {p}
              </span>
            ))}
          </p>
        </>
      ) : null}
    </header>
  );
}

const STAT_KEYS = ["mean", "min", "p50", "p95", "p99", "max"] as const;

/** One compact strip of statistics per drawn series over the range in view; the caveat is in the ⓘ. */
function StatsStrip({
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
  const explain = `Over ${formatNumber(from)}–${formatNumber(to)} s, in ${unitSuffix(unit) === "" ? axisUnit(unit) : unit}. These are statistics of the values the engine reported per window (one per metric period): for a delay, the spread of the windows' means, not of individual messages; the p95 series is the per-message percentile. Computed from the full series, not from what is drawn.`;
  const multi = rows.length > 1;
  return (
    <div className="mx-stats">
      <table className="mx-stats-table" data-testid="metric-stats">
        <thead>
          <tr>
            <th className="lbl" data-testid="metric-stats-unit">
              {multi ? "series" : "range"} · {axisUnit(unit)}
              <span className="mx-info" tabIndex={0} title={explain} aria-label={explain} data-testid="metric-stats-info">
                i
              </span>
            </th>
            {STAT_KEYS.map((k) => (
              <th key={k}>{k}</th>
            ))}
            <th title="Windows in the range that carry a value">windows</th>
          </tr>
        </thead>
        <tbody>
          {rows.map(({ line, stats }) => (
            <tr key={line.name} data-testid={`metric-stats-${line.name}`}>
              <td className="lbl">
                <i className="key" style={{ background: slotColour(line.slot) }} />
                {line.label}
              </td>
              {STAT_KEYS.map((k) => (
                <td key={k} className="mono" data-stat={k}>
                  {stats ? formatNumber(stats[k]) : "—"}
                </td>
              ))}
              <td className="mono dim" data-stat="n">
                {stats ? stats.n.toLocaleString("en-US") : "0"}
              </td>
            </tr>
          ))}
          {delta ? (
            <tr className="delta">
              <td className="lbl">run B − this run</td>
              {STAT_KEYS.map((k) => {
                const d = delta.b[k] - delta.a[k];
                return (
                  <td key={k} className="mono">
                    {d > 0 ? "+" : ""}
                    {formatNumber(d)}
                  </td>
                );
              })}
              <td />
            </tr>
          ) : null}
        </tbody>
      </table>
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
function MarkdownLite({ text, inline = false }: { text: string; inline?: boolean }): React.JSX.Element {
  const spans = (p: string): React.JSX.Element[] =>
    p.split(/(`[^`]+`)/).map((part, j) =>
      part.startsWith("`") && part.endsWith("`") && part.length > 1 ? <code key={j}>{part.slice(1, -1)}</code> : <span key={j}>{part}</span>,
    );
  if (inline) return <>{spans(text)}</>;
  return (
    <>
      {text
        .split(/\n\s*\n/)
        .filter((p) => p.trim() !== "")
        .map((p, i) => (
          <p key={i}>{spans(p)}</p>
        ))}
    </>
  );
}

/**
 * What the metric is, collapsed to one sentence: open, the whole definition, the health
 * thresholds with their source, the facts a methods section needs, and what it leaves out.
 */
function About({
  f,
  rat,
  latest,
  judgedName,
  judgedReported,
}: {
  f: MetricFamily;
  rat: string | null;
  latest: number | null;
  judgedName: string | null;
  judgedReported: boolean;
}): React.JSX.Element {
  const provenance = useStudio((s) => s.metricProvenance);
  const name = f.headline ?? seriesOf(f)[0] ?? f.base;
  // The inspector's Why tab: the model, its version and the parameter set that produced this
  // metric. The inspector sits under this panel, so the panel closes to show it.
  const why = (): void => {
    useStudio.getState().setWhy(metricSubject(name, latest, f.unit, provenance[name]));
    closePanel();
  };
  const definition = f.definition === "" ? "The engine published no definition for this metric." : f.definition;
  const lead = firstSentence(definition);
  return (
    <details className="mx-about" data-testid="metric-about">
      <summary data-testid="metric-about-toggle">
        <span className="mx-about-title">About this metric</span>
        <span className="mx-about-lead">
          <MarkdownLite text={lead} inline />
        </span>
      </summary>
      <div className="mx-about-body">
        <div className="mx-about-def">
          <MarkdownLite text={definition} />
        </div>
        {f.health ? (
          <div className="mx-about-band">
            {!judgedReported && judgedName !== null ? (
              <p className="dim">
                The healthy range is judged on <code>{judgedName}</code>, which this engine does not report.
              </p>
            ) : null}
            <BandTable band={f.health} unit={f.unit} />
          </div>
        ) : null}
        <dl>
          <dt>Unit</dt>
          <dd>{axisUnit(f.unit)}</dd>
          <dt>Reduced by</dt>
          <dd className="mono">{f.agg || "—"}</dd>
          <dt>Visibility</dt>
          <dd>{f.groundTruth ? "Ground truth: the simulator knows it; no deployed device could measure it." : "Node: what a deployed device could measure itself."}</dd>
          <dt>Source</dt>
          <dd data-testid="metric-source">{f.source ?? "The engine cites no source for this metric."}</dd>
          <dt>Broken down</dt>
          <dd>{f.breakdowns.length > 0 ? f.breakdowns.map((d) => dimLabel(d).replace(/^By /, "by ")).join(", ") : "Whole run only"}</dd>
          {rat !== null && f.group !== "traffic" && f.group !== "simulator" ? (
            <>
              <dt>Radio</dt>
              <dd data-testid="metric-technology" title="The engine runs one technology per run (radio.rat). To compare technologies, run the scenario with each and overlay the runs (Compare, then Overlay run B).">
                <code>{rat}</code> for the whole run; compare technologies by overlaying runs
              </dd>
            </>
          ) : null}
          <dt>Series</dt>
          <dd className="mono">{seriesOf(f).join(", ")}</dd>
          <dt>Model</dt>
          <dd>
            <button type="button" className="linklike" onClick={why} data-testid="metric-why" title="Close the metrics and show, in the inspector, the model, version and parameters behind this measurement">
              Which model produced it
            </button>
          </dd>
        </dl>
        {f.notAccounted.length > 0 ? (
          <div className="mx-caveats">
            <h5>Does not account for</h5>
            <ul>
              {f.notAccounted.map((n) => (
                <li key={n}>{n}</li>
              ))}
            </ul>
          </div>
        ) : null}
      </div>
    </details>
  );
}
