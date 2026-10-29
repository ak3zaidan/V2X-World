/**
 * One dashboard card: a metric's name, its newest value with its unit, and its whole run so far
 * as a line. The card is the button that expands it.
 *
 * A card with nothing to draw says why, in words, instead of drawing an empty box (the owner's
 * rule, and 08-measurement-and-data.md's: a missing number is not a zero).
 */

import { useMemo, useState } from "react";
import type { MetricHealthBand } from "@vwp/protocol";

import { classify, judgedLabel, judgedSeries, limitText, noteOf } from "./health.js";
import { HealthChip } from "./HealthBadge.js";
import { usePins } from "./pins.js";
import {
  axisUnit,
  cardSeries,
  clockText,
  dataView,
  display,
  displayText,
  meaningOf,
  observedCount,
  type MetricFamily,
  type Series,
} from "./model.js";
import type { Overview } from "./hooks.js";

const W = 300;
const H = 64;

/** Why a card has no line, in a sentence. */
function emptyReason(f: MetricFamily, o: Overview, runState: string, profileNode: boolean): string {
  const names = cardSeries(f);
  if (f.groundTruth && profileNode) {
    return "Ground truth: a node-profile session sees only what a deployed device could measure.";
  }
  if (names.some((n) => o.refused.has(n))) return "The engine refused this measurement for this session.";
  if (o.status === "loading") return "Asking the engine…";
  if (o.status === "error") return "The engine did not answer. It may be restarting; this retries on its own.";
  if (runState === "idle" || runState === "loading") return "No run yet. Press Run to start one.";
  // Either nothing exercised it, or no one window reached the metric's minimum sample count;
  // the expanded view tells the two apart (it asks the engine what the whole run pooled).
  if (runState === "finished") return "No window of this run reported a value. Open it to see whether anything was measured at all.";
  return "No window has had enough samples yet. It appears once one has.";
}

function firstT(s: Series): number {
  for (let i = 0; i < s.t.length; i++) if (!Number.isNaN(s.v[i])) return s.t[i];
  return 0;
}

function lastT(s: Series): number {
  for (let i = s.t.length - 1; i >= 0; i--) if (!Number.isNaN(s.v[i])) return s.t[i];
  return 0;
}

/** "0.5–0.9 %", "12–48 ms": a line's range, its unit said once when both ends share it. */
function rangeText(base: string, lo: number, hi: number, unit: string): string {
  const a = display(base, lo, unit);
  const b = display(base, hi, unit);
  if (a.unit === b.unit) return `${a.value}–${b.value}${b.unit === "" ? "" : ` ${b.unit}`}`;
  return `${displayText(base, lo, unit)} – ${displayText(base, hi, unit)}`;
}

/** A sparkline of one series with a hover readout. `from`/`to` are the x extent in seconds. */
export function Sparkline({
  series,
  from,
  to,
  unit,
  label,
  band = null,
  base = "",
  byWindow = false,
}: {
  series: Series;
  from: number;
  to: number;
  unit: string;
  label: string;
  /** The metric, so a value reads as the cards and the summary read it (`model.ts` `display`). */
  base?: string;
  /** Space the windows evenly, not by time: a fast-forwarded run's windows are minutes, then days, apart. */
  byWindow?: boolean;
  /** The health band, drawn as threshold lines where they fall inside the line's range. */
  band?: MetricHealthBand | null;
}): React.JSX.Element {
  const [hover, setHover] = useState<number | null>(null);
  const geom = useMemo(() => {
    let lo = Number.POSITIVE_INFINITY;
    let hi = Number.NEGATIVE_INFINITY;
    for (let i = 0; i < series.v.length; i++) {
      const v = series.v[i];
      if (Number.isNaN(v)) continue;
      if (v < lo) lo = v;
      if (v > hi) hi = v;
    }
    if (!Number.isFinite(lo)) return null;
    // The range the line actually spans, for the axis text; the padding below is only for drawing.
    const flat = hi - lo < 1e-12;
    const dataLo = lo;
    const dataHi = hi;
    if (flat) {
      const pad = Math.abs(hi) > 0 ? Math.abs(hi) * 0.05 : 1;
      lo -= pad;
      hi += pad;
    }
    const span = Math.max(1e-9, to - from);
    // By window (a sparse, fast-forwarded run): each reported window one step apart, as the
    // overview's trend lines draw it; otherwise by simulated time.
    const n = observedCount(series);
    const rank = new Int32Array(series.t.length);
    for (let i = 0, k = 0; i < series.t.length; i++) if (!Number.isNaN(series.v[i])) rank[i] = k++;
    const sx = (i: number): number => (byWindow ? (n <= 1 ? W / 2 : (rank[i]! / (n - 1)) * W) : ((series.t[i] - from) / span) * W);
    const sy = (v: number): number => 4 + (1 - (v - lo) / (hi - lo)) * (H - 8);
    let d = "";
    let pen = false;
    for (let i = 0; i < series.t.length; i++) {
      const v = series.v[i];
      if (Number.isNaN(v)) {
        if (!byWindow) pen = false;
        continue;
      }
      d += `${pen ? "L" : "M"}${sx(i).toFixed(1)},${sy(v).toFixed(1)}`;
      pen = true;
    }
    // A lone point draws nothing as a path; give it a dot.
    const dots = observedCount(series) === 1 ? [...series.v].findIndex((v) => !Number.isNaN(v)) : -1;
    return { d, lo, hi, sx, sy, dots, flat, dataLo, dataHi };
  }, [series, from, to, byWindow]);

  if (geom === null) return <div className="mcard-plot" />;
  const onMove = (ev: React.PointerEvent<SVGSVGElement>): void => {
    const box = ev.currentTarget.getBoundingClientRect();
    const x = ((ev.clientX - box.left) / Math.max(1, box.width)) * W;
    // Nearest observed window.
    let best = -1;
    let bestD = Number.POSITIVE_INFINITY;
    for (let i = 0; i < series.t.length; i++) {
      if (Number.isNaN(series.v[i])) continue;
      const dd = Math.abs(geom.sx(i) - x);
      if (dd < bestD) {
        bestD = dd;
        best = i;
      }
    }
    setHover(best >= 0 ? best : null);
  };
  return (
    <div className="mcard-plot">
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        role="img"
        aria-label={`${label} over simulated time, ${displayText(base, geom.dataLo, unit)} to ${displayText(base, geom.dataHi, unit)}`}
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
      >
        {band
          ? (
              [
                ["caution", band.caution],
                ["unhealthy", band.unhealthy],
              ] as const
            )
              .filter(([, v]) => v >= geom.lo && v <= geom.hi)
              .map(([lv, v]) => (
                <line key={lv} x1={0} x2={W} y1={geom.sy(v)} y2={geom.sy(v)} className={`mthreshold ${lv}`} vectorEffect="non-scaling-stroke" />
              ))
          : null}
        <path d={geom.d} className="mline" vectorEffect="non-scaling-stroke" />
        {geom.dots >= 0 ? <circle cx={geom.sx(geom.dots)} cy={geom.sy(series.v[geom.dots])} r={3} className="mdot" /> : null}
        {hover !== null ? (
          <line x1={geom.sx(hover)} x2={geom.sx(hover)} y1={0} y2={H} className="mcross" vectorEffect="non-scaling-stroke" />
        ) : null}
      </svg>
      <div className="mcard-axis">
        {hover !== null ? (
          <span className="mcard-readout">
            <strong>{displayText(base, series.v[hover], unit)}</strong> at {clockText(series.t[hover])}
          </span>
        ) : (
          <>
            <span title={byWindow ? "The first window that reported a value" : undefined}>{clockText(byWindow ? firstT(series) : from)}</span>
            <span className="grow" />
            <span title="The range of the line">{geom.flat ? `constant ${displayText(base, geom.dataLo, unit)}` : rangeText(base, geom.dataLo, geom.dataHi, unit)}</span>
            <span className="grow" />
            <span title={byWindow ? "The windows that reported, evenly spaced: this run reported in short stretches far apart, which on a time axis would be a sliver" : undefined}>
              {byWindow ? (observedCount(series) === 1 ? "one window" : `${clockText(lastT(series))} · by window`) : clockText(to)}
            </span>
          </>
        )}
      </div>
    </div>
  );
}

/** A breakdown-only metric's card: each value's newest window, as bars. */
function MiniBars({ f, latest }: { f: MetricFamily; latest: ReadonlyMap<string, number | null> }): React.JSX.Element | null {
  const rows = f.values
    .map((v) => ({ key: v.key, value: latest.get(v.series) ?? null }))
    .filter((r): r is { key: string; value: number } => r.value !== null);
  if (rows.length === 0) return null;
  const max = Math.max(1e-12, ...rows.map((r) => Math.abs(r.value)));
  const shown = rows.slice(0, 6);
  return (
    <div className="mcard-bars" role="list" aria-label={`${f.label}, newest window, by ${f.valueDim ?? "value"}`}>
      {shown.map((r) => (
        <div key={r.key} className="mbar-row" role="listitem">
          <span className="mbar-key mono">{r.key}</span>
          <span className="mbar-track">
            <span className="mbar" style={{ width: `${Math.max(1, (Math.abs(r.value) / max) * 100)}%` }} />
          </span>
          <span className="mbar-val mono">{displayText(f.base, r.value, f.unit)}</span>
        </div>
      ))}
      {rows.length > shown.length ? <div className="dim mbar-more">and {rows.length - shown.length} more</div> : null}
    </div>
  );
}

export function MetricCard({
  f,
  overview,
  runState,
  profileNode,
  onOpen,
}: {
  f: MetricFamily;
  overview: Overview;
  runState: string;
  profileNode: boolean;
  onOpen: (f: MetricFamily) => void;
}): React.JSX.Element {
  const pinned = usePins((s) => s.pins.includes(f.base));
  const toggle = usePins((s) => s.toggle);
  const headline = f.headline;
  const series = headline === null ? null : overview.series.get(headline);
  const latest = headline === null ? null : (overview.latest.get(headline) ?? null);
  const hasLine = series !== undefined && series !== null && observedCount(series) > 0;
  const hasBars = headline === null && f.values.some((v) => (overview.latest.get(v.series) ?? null) !== null);
  const empty = !hasLine && !hasBars;
  // The health band judges one series: the headline, a percentile or a breakdown value. A card
  // judges only when the engine streams that series; it never judges a different statistic.
  const band = f.health;
  const judged = band ? judgedSeries(f.base, band) : null;
  const judgedValue = judged !== null && overview.latest.has(judged) ? (overview.latest.get(judged) ?? null) : null;
  const level = band && !empty ? classify(band, judgedValue) : null;
  const shown = display(f.base, latest, f.unit);
  const end = Math.max(overview.toS, series && series.t.length > 0 ? series.t[series.t.length - 1] : 0);
  const view = hasLine && series ? { end, sparse: dataView(series, end).sparse } : null;
  return (
    <article
      className={`mcard${empty ? " empty" : ""}${level ? ` h-${level}` : ""}`}
      data-testid={`metric-card-${f.base}`}
      data-has-data={empty ? "false" : "true"}
      data-health={level ?? "none"}
      aria-label={f.label}
    >
      <header className="mcard-head">
        <button type="button" className="mcard-title linklike" onClick={() => onOpen(f)} data-testid={`metric-open-${f.base}`} title="Expand: a large chart, statistics, breakdowns, definition and export">
          {f.label}
        </button>
        {f.groundTruth ? (
          <span className="gt-tag" title="Ground truth: a quantity the simulator knows and no deployed device could measure">
            GT
          </span>
        ) : null}
        <button
          type="button"
          className={`icon-button pin${pinned ? " on" : ""}`}
          aria-pressed={pinned}
          aria-label={pinned ? `Unpin ${f.label}` : `Pin ${f.label}`}
          title={pinned ? "Unpin" : "Pin: show it first here, and in the header"}
          onClick={() => toggle(f.base)}
          data-testid={`metric-pin-${f.base}`}
        >
          {pinned ? "★" : "☆"}
        </button>
      </header>
      <div className="mcard-name mono dim">{f.base}</div>
      <p className="mcard-meaning" title={f.definition} data-testid={`metric-meaning-${f.base}`}>
        {meaningOf(f)}
      </p>
      {empty ? (
        <p className="mcard-empty dim" data-testid={`metric-empty-${f.base}`}>
          {emptyReason(f, overview, runState, profileNode)}
        </p>
      ) : (
        // The title is the keyboard's way in; the body is a larger target for the pointer.
        <div className="mcard-body" onClick={() => onOpen(f)} data-testid={`metric-body-${f.base}`}>
          {headline !== null ? (
            <div className="mcard-value" title="The newest window's value">
              <span className="mcard-num" data-testid={`metric-value-${f.base}`}>
                {shown.value}
              </span>
              <span className="mcard-unit">{shown.unit === "" ? axisUnit(f.unit) : shown.unit}</span>
              {band && level ? <HealthChip band={band} level={level} unit={f.unit} testId={`metric-health-${f.base}`} /> : null}
            </div>
          ) : headline === null && band && level ? (
            <HealthChip band={band} level={level} unit={f.unit} testId={`metric-health-${f.base}`} />
          ) : null}
          {band && level && (level !== "healthy" || judgedLabel(band) !== "") ? (
            <div className={`mcard-verdict ${level}`} title={noteOf(band, level)}>
              {judgedLabel(band) !== "" ? (
                <span>
                  {judgedLabel(band)} <strong className="mono">{displayText(f.base, judgedValue, f.unit)}</strong> ·{" "}
                </span>
              ) : null}
              {limitText(band, level, f.unit)}
            </div>
          ) : null}
          {hasLine && series ? (
            <Sparkline
              series={series}
              from={0}
              to={view?.end ?? 0}
              byWindow={view?.sparse ?? false}
              unit={f.unit}
              base={f.base}
              label={f.label}
              band={band && band.judged === "" ? band : null}
            />
          ) : (
            <MiniBars f={f} latest={overview.latest} />
          )}
        </div>
      )}
    </article>
  );
}
