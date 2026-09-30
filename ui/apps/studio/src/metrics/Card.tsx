/**
 * One dashboard card: a metric's name, its newest value with its unit, and its whole run so far
 * as a line. The card is the button that expands it.
 *
 * A card with nothing to draw says why, in words, instead of drawing an empty box (the owner's
 * rule, and 08-measurement-and-data.md's: a missing number is not a zero).
 */

import { useMemo, useState } from "react";

import { usePins } from "./pins.js";
import {
  axisUnit,
  cardSeries,
  formatNumber,
  formatWithUnit,
  observedCount,
  unitSuffix,
  type MetricFamily,
  type Series,
} from "./model.js";
import type { Overview } from "./hooks.js";

const W = 300;
const H = 64;

/** Why a card has no line, in a sentence. */
export function emptyReason(f: MetricFamily, o: Overview, runState: string, profileNode: boolean): string {
  const names = cardSeries(f);
  if (names.some((n) => o.refused.has(n)) && f.groundTruth && profileNode) {
    return "Ground truth: a node-profile session sees only what a deployed device could measure.";
  }
  if (names.some((n) => o.refused.has(n))) return "The engine refused this measurement for this session.";
  if (o.status === "loading") return "Asking the engine…";
  if (o.status === "error") return "The engine did not answer. It may be restarting; this retries on its own.";
  if (runState === "idle" || runState === "loading") return "No run yet. Press Run to start one.";
  if (runState === "finished") return "Not measured in this run: nothing in the scenario exercised it.";
  return "No window has had enough samples yet. It appears once one has.";
}

/** A sparkline of one series with a hover readout. `from`/`to` are the x extent in seconds. */
export function Sparkline({
  series,
  from,
  to,
  unit,
  label,
}: {
  series: Series;
  from: number;
  to: number;
  unit: string;
  label: string;
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
    if (hi - lo < 1e-12) {
      const pad = Math.abs(hi) > 0 ? Math.abs(hi) * 0.05 : 1;
      lo -= pad;
      hi += pad;
    }
    const span = Math.max(1e-9, to - from);
    const sx = (t: number): number => ((t - from) / span) * W;
    const sy = (v: number): number => 4 + (1 - (v - lo) / (hi - lo)) * (H - 8);
    let d = "";
    let pen = false;
    for (let i = 0; i < series.t.length; i++) {
      const v = series.v[i];
      if (Number.isNaN(v)) {
        pen = false;
        continue;
      }
      d += `${pen ? "L" : "M"}${sx(series.t[i]).toFixed(1)},${sy(v).toFixed(1)}`;
      pen = true;
    }
    // A lone point draws nothing as a path; give it a dot.
    const dots = observedCount(series) === 1 ? [...series.v].findIndex((v) => !Number.isNaN(v)) : -1;
    return { d, lo, hi, sx, sy, dots };
  }, [series, from, to]);

  if (geom === null) return <div className="mcard-plot" />;
  const onMove = (ev: React.PointerEvent<SVGSVGElement>): void => {
    const box = ev.currentTarget.getBoundingClientRect();
    const t = from + ((ev.clientX - box.left) / Math.max(1, box.width)) * (to - from);
    // Nearest observed window.
    let best = -1;
    let bestD = Number.POSITIVE_INFINITY;
    for (let i = 0; i < series.t.length; i++) {
      if (Number.isNaN(series.v[i])) continue;
      const dd = Math.abs(series.t[i] - t);
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
        aria-label={`${label} over simulated time, ${formatNumber(geom.lo)} to ${formatNumber(geom.hi)} ${axisUnit(unit)}`}
        onPointerMove={onMove}
        onPointerLeave={() => setHover(null)}
      >
        <path d={geom.d} className="mline" vectorEffect="non-scaling-stroke" />
        {geom.dots >= 0 ? <circle cx={geom.sx(series.t[geom.dots])} cy={geom.sy(series.v[geom.dots])} r={3} className="mdot" /> : null}
        {hover !== null ? (
          <line x1={geom.sx(series.t[hover])} x2={geom.sx(series.t[hover])} y1={0} y2={H} className="mcross" vectorEffect="non-scaling-stroke" />
        ) : null}
      </svg>
      <div className="mcard-axis">
        {hover !== null ? (
          <span className="mcard-readout">
            <strong>{formatWithUnit(series.v[hover], unit)}</strong> at {formatNumber(series.t[hover])} s
          </span>
        ) : (
          <>
            <span>{formatNumber(from)} s</span>
            <span className="grow" />
            <span title="The range of the line">
              {formatNumber(geom.lo)}–{formatNumber(geom.hi)}
              {unitSuffix(unit) === "" ? "" : ` ${unitSuffix(unit)}`}
            </span>
            <span className="grow" />
            <span>{formatNumber(to)} s</span>
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
          <span className="mbar-val mono">{formatWithUnit(r.value, f.unit)}</span>
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
  return (
    <article
      className={`mcard${empty ? " empty" : ""}`}
      data-testid={`metric-card-${f.base}`}
      data-has-data={empty ? "false" : "true"}
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
      {empty ? (
        <p className="mcard-empty dim" data-testid={`metric-empty-${f.base}`}>
          {emptyReason(f, overview, runState, profileNode)}
        </p>
      ) : (
        // The title is the keyboard's way in; the body is a larger target for the pointer.
        <div className="mcard-body" onClick={() => onOpen(f)} data-testid={`metric-body-${f.base}`}>
          {headline !== null ? (
            <div className="mcard-value">
              <span className="mcard-num" data-testid={`metric-value-${f.base}`}>
                {formatNumber(latest)}
              </span>
              <span className="mcard-unit">{unitSuffix(f.unit) === "" ? axisUnit(f.unit) : f.unit}</span>
              <span className="dim mcard-when">newest window</span>
            </div>
          ) : null}
          {hasLine && series ? (
            <Sparkline series={series} from={0} to={Math.max(overview.toS, series.t.length > 0 ? series.t[series.t.length - 1] : 0)} unit={f.unit} label={f.label} />
          ) : (
            <MiniBars f={f} latest={overview.latest} />
          )}
        </div>
      )}
    </article>
  );
}
