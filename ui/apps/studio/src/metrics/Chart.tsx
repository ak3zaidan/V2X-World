/**
 * The expanded chart and its time brush.
 *
 * The chart is uPlot: a crosshair that snaps to the nearest window, one readout listing every
 * series at that instant (its legend), drag across it to zoom to a range, double-click to go back
 * to the whole run. What it draws is decimated to the chart's width (`model.ts` `decimate`, M4), so
 * a ten-hour run draws as fast as a ten-second one and keeps its spikes; the statistics beside it
 * are computed from the full series.
 *
 * The brush under it shows the whole run and the range in view; drag on it to choose a range, drag
 * the range to move it, drag its edges to resize it. Arrow keys move a focused brush, Shift+arrow
 * resizes it.
 */

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import uPlot from "uplot";

import { useStudio } from "../state/store.js";
import { axisUnit, decimate, formatNumber, formatWithUnit, type Series } from "./model.js";

/** One line on the chart. `slot` is the categorical colour (1–8), fixed per series. */
export interface ChartLine {
  readonly name: string;
  readonly label: string;
  readonly slot: number;
  readonly series: Series;
}

export function slotColour(slot: number): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(`--viz-${slot}`).trim();
  return v === "" ? "#3987e5" : v;
}

function cssVar(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v === "" ? fallback : v;
}

/**
 * Align several decimated series onto one x column for uPlot. A point one series has and another
 * does not is `undefined` in the other (uPlot draws across it); a window a series was not observed
 * in stays `null` (a gap).
 */
function align(parts: readonly [number[], (number | null)[]][]): uPlot.AlignedData {
  const xs = [...new Set(parts.flatMap(([x]) => x))].sort((a, b) => a - b);
  const index = new Map(xs.map((x, i) => [x, i]));
  const cols = parts.map(([x, y]) => {
    const col: (number | null | undefined)[] = new Array<number | null | undefined>(xs.length).fill(undefined);
    for (let i = 0; i < x.length; i++) col[index.get(x[i]) as number] = y[i];
    return col;
  });
  return [xs, ...cols] as unknown as uPlot.AlignedData;
}

export function MainChart({
  lines,
  from,
  to,
  unit,
  height = 340,
  onRange,
  onReset,
  onPlot,
}: {
  lines: readonly ChartLine[];
  from: number;
  to: number;
  unit: string;
  height?: number;
  onRange: (from: number, to: number) => void;
  onReset: () => void;
  onPlot?: (plot: uPlot | null) => void;
}): React.JSX.Element {
  const host = useRef<HTMLDivElement | null>(null);
  const plot = useRef<uPlot | null>(null);
  const theme = useStudio((s) => s.theme);
  const [width, setWidth] = useState(800);
  const range = useRef({ from, to });
  range.current = { from, to };
  const handlers = useRef({ onRange, onReset });
  handlers.current = { onRange, onReset };

  // Width follows the container.
  useLayoutEffect(() => {
    const el = host.current;
    if (!el) return;
    const measure = (): void => setWidth(Math.max(240, Math.floor(el.clientWidth)));
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const shape = lines.map((l) => `${l.name}:${l.slot}:${l.label}`).join("|");

  // The plot is rebuilt when the set of lines, the unit or the theme changes; data and range only
  // update it.
  useLayoutEffect(() => {
    const el = host.current;
    if (!el) return;
    const text = cssVar("--text-dim", "#8fa3b8");
    const grid = cssVar("--grid", cssVar("--border", "#223041"));
    const font = `11px ${cssVar("--sans", "system-ui")}`;
    const opts: uPlot.Options = {
      width,
      height,
      legend: { show: true, live: true },
      cursor: { drag: { x: true, y: false, setScale: false }, points: { size: 8 } },
      select: { show: true, left: 0, top: 0, width: 0, height: 0 },
      scales: {
        x: { time: false, range: () => [range.current.from, range.current.to] },
      },
      axes: [
        {
          stroke: text,
          grid: { stroke: grid, width: 1 },
          ticks: { stroke: grid, width: 1 },
          font,
          labelFont: font,
          label: "simulated time (s)",
          labelSize: 18,
          values: (_u, splits) => splits.map((s) => formatNumber(s)),
        },
        {
          stroke: text,
          grid: { stroke: grid, width: 1 },
          ticks: { stroke: grid, width: 1 },
          font,
          labelFont: font,
          label: axisUnit(unit),
          labelSize: 18,
          size: 56,
          values: (_u, splits) => splits.map((s) => formatNumber(s)),
        },
      ],
      series: [
        { label: "t (s)", value: (_u, v) => (v === null || v === undefined ? "—" : `${formatNumber(v)} s`) },
        ...lines.map((l) => ({
          label: l.label,
          stroke: slotColour(l.slot),
          width: 2,
          spanGaps: false,
          points: { show: false },
          value: (_u: uPlot, v: number | null) => formatWithUnit(v, unit),
        })),
      ],
      hooks: {
        setSelect: [
          (u) => {
            if (u.select.width < 4) return;
            const a = u.posToVal(u.select.left, "x");
            const b = u.posToVal(u.select.left + u.select.width, "x");
            u.setSelect({ left: 0, top: 0, width: 0, height: 0 }, false);
            if (Number.isFinite(a) && Number.isFinite(b) && b > a) handlers.current.onRange(a, b);
          },
        ],
      },
    };
    const p = new uPlot(opts, [[], ...lines.map(() => [])] as unknown as uPlot.AlignedData, el);
    const reset = (): void => handlers.current.onReset();
    p.over.addEventListener("dblclick", reset);
    plot.current = p;
    onPlot?.(p);
    return () => {
      p.over.removeEventListener("dblclick", reset);
      p.destroy();
      plot.current = null;
      onPlot?.(null);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shape, unit, theme, height]);

  useEffect(() => {
    plot.current?.setSize({ width, height });
  }, [width, height]);

  // Data: decimated to about the plot's width in pixels, over the range in view.
  const data = useMemo(() => {
    const buckets = Math.max(50, Math.floor(width));
    return align(lines.map((l) => decimate(l.series, from, to, buckets)));
  }, [lines, from, to, width]);

  useEffect(() => {
    const p = plot.current;
    if (!p) return;
    p.setData(data, false);
    p.setScale("x", { min: from, max: to });
  }, [data, from, to, shape, theme]);

  return <div ref={host} className="mx-chart" data-testid="metric-chart" />;
}

const BW = 1000;
const BH = 48;

/**
 * The whole run with the range in view marked. Pointer: drag on empty track to draw a range, drag
 * the range to move it, drag an edge to resize. Keyboard: arrows move, Shift+arrows resize, Home
 * or Escape shows the whole run.
 */
export function Brush({
  series,
  extent,
  from,
  to,
  onRange,
  onReset,
}: {
  series: Series | null;
  extent: number;
  from: number;
  to: number;
  onRange: (from: number, to: number) => void;
  onReset: () => void;
}): React.JSX.Element {
  const svg = useRef<SVGSVGElement | null>(null);
  const drag = useRef<{ mode: "new" | "move" | "left" | "right"; t0: number; from: number; to: number } | null>(null);
  const [draft, setDraft] = useState<{ from: number; to: number } | null>(null);
  const span = Math.max(1e-9, extent);
  const sx = (t: number): number => (t / span) * BW;
  const path = useMemo(() => {
    if (series === null || series.t.length === 0) return "";
    const [xs, ys] = decimate(series, 0, span, 300);
    let lo = Number.POSITIVE_INFINITY;
    let hi = Number.NEGATIVE_INFINITY;
    for (const y of ys) {
      if (y === null) continue;
      lo = Math.min(lo, y);
      hi = Math.max(hi, y);
    }
    if (!Number.isFinite(lo)) return "";
    if (hi - lo < 1e-12) hi = lo + 1;
    let d = "";
    let pen = false;
    for (let i = 0; i < xs.length; i++) {
      const y = ys[i];
      if (y === null) {
        pen = false;
        continue;
      }
      d += `${pen ? "L" : "M"}${((xs[i] / span) * BW).toFixed(1)},${(4 + (1 - (y - lo) / (hi - lo)) * (BH - 8)).toFixed(1)}`;
      pen = true;
    }
    return d;
  }, [series, span]);

  const shown = draft ?? { from, to };
  const tAt = (clientX: number): number => {
    const box = svg.current?.getBoundingClientRect();
    if (!box) return 0;
    return Math.min(span, Math.max(0, ((clientX - box.left) / Math.max(1, box.width)) * span));
  };
  const onDown = (ev: React.PointerEvent<SVGSVGElement>): void => {
    const t = tAt(ev.clientX);
    const box = svg.current?.getBoundingClientRect();
    const px = box ? (span / Math.max(1, box.width)) * 6 : 0;
    const mode = Math.abs(t - from) <= px ? "left" : Math.abs(t - to) <= px ? "right" : t > from && t < to && to - from < span ? "move" : "new";
    drag.current = { mode, t0: t, from, to };
    ev.currentTarget.setPointerCapture(ev.pointerId);
  };
  const onMove = (ev: React.PointerEvent<SVGSVGElement>): void => {
    const d = drag.current;
    if (!d) return;
    const t = tAt(ev.clientX);
    if (d.mode === "new") setDraft({ from: Math.min(d.t0, t), to: Math.max(d.t0, t) });
    else if (d.mode === "move") {
      const w = d.to - d.from;
      const a = Math.min(span - w, Math.max(0, d.from + (t - d.t0)));
      setDraft({ from: a, to: a + w });
    } else if (d.mode === "left") setDraft({ from: Math.min(t, d.to - 1e-6), to: d.to });
    else setDraft({ from: d.from, to: Math.max(t, d.from + 1e-6) });
  };
  const onUp = (): void => {
    const d = draft;
    drag.current = null;
    setDraft(null);
    // A click without a drag on the empty track is not a range.
    if (d && d.to - d.from > span / 500) onRange(d.from, d.to);
  };
  const onKey = (ev: React.KeyboardEvent<SVGSVGElement>): void => {
    const w = to - from;
    const step = Math.max(span / 100, w / 10);
    if (ev.key === "ArrowLeft" || ev.key === "ArrowRight") {
      const dir = ev.key === "ArrowLeft" ? -1 : 1;
      if (ev.shiftKey) onRange(from, Math.min(span, Math.max(from + step, to + dir * step)));
      else {
        const a = Math.min(span - w, Math.max(0, from + dir * step));
        onRange(a, a + w);
      }
      ev.preventDefault();
    } else if (ev.key === "Home") {
      onReset();
      ev.preventDefault();
    }
  };
  const whole = shown.from <= 0 && shown.to >= span;
  return (
    <svg
      ref={svg}
      className="mx-brush"
      viewBox={`0 0 ${BW} ${BH}`}
      preserveAspectRatio="none"
      role="slider"
      tabIndex={0}
      aria-label="Time range in view"
      aria-valuemin={0}
      aria-valuemax={Math.round(span)}
      aria-valuenow={Math.round(shown.from)}
      aria-valuetext={`${formatNumber(shown.from)} to ${formatNumber(shown.to)} s of ${formatNumber(span)} s`}
      data-testid="metric-brush"
      onPointerDown={onDown}
      onPointerMove={onMove}
      onPointerUp={onUp}
      onPointerCancel={onUp}
      onKeyDown={onKey}
    >
      <rect x={0} y={0} width={BW} height={BH} className="brush-track" />
      {path !== "" ? <path d={path} className="brush-line" vectorEffect="non-scaling-stroke" /> : null}
      {!whole ? (
        <>
          <rect x={0} y={0} width={Math.max(0, sx(shown.from))} height={BH} className="brush-shade" />
          <rect x={sx(shown.to)} y={0} width={Math.max(0, BW - sx(shown.to))} height={BH} className="brush-shade" />
        </>
      ) : null}
      <rect x={sx(shown.from)} y={0.5} width={Math.max(1, sx(shown.to) - sx(shown.from))} height={BH - 1} className="brush-window" vectorEffect="non-scaling-stroke" />
      <rect x={sx(shown.from) - 2} y={0} width={4} height={BH} className="brush-handle" />
      <rect x={sx(shown.to) - 2} y={0} width={4} height={BH} className="brush-handle" />
    </svg>
  );
}
