/**
 * "Compare with previous run": two runs side by side in one dialog.
 *
 * The engine keeps the previous runs' digests (`runs.list`) and computes the comparison
 * (`runs.compare`, `crates/v2xw-server/src/compare.rs`): what changed in the settings, every
 * headline metric pooled over each run with its change, which way is better for it, its health
 * level on each side, and whether the change is meaningful (crosses a health band, or ≥ 10 % and
 * past the unit's resolution — and, with different seeds, past twice the combined batch-means
 * error). This dialog shows that, overlays the selected metric's two series, and exports or hands
 * it to the agent. Any two kept runs can be compared from the Runs tab (`openRunCompare(a, b)`).
 */

import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import uPlot from "uplot";
import { create } from "zustand";
import type { RunDigest, RunMetricRow, RunsCompareResult, RunsListResult } from "@vwp/protocol";

import { CloseIcon } from "../shell/Icons.js";
import { openPanel } from "../shell/route.js";
import { useAgent } from "../state/agent.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { HealthDot } from "./HealthBadge.js";
import { download } from "./exporting.js";
import { formatWithUnit } from "./model.js";
import { agentPrompt, deltaText, exportName, pctText, rowLabel, sideText, tally, verdictClass, visibleRows } from "./runCompareModel.js";
import "./metrics.css";

interface CompareState {
  open: boolean;
  a: string;
  b: string;
  setOpen: (open: boolean) => void;
}

export const useRunCompare = create<CompareState>((set) => ({
  open: false,
  a: "previous",
  b: "current",
  setOpen: (open) => set({ open }),
}));

/** Opens the comparison of two kept runs (refs from `runs.list`); previous vs current by default. */
export function openRunCompare(a = "previous", b = "current"): void {
  useRunCompare.setState({ open: true, a, b });
}

/** Whether the engine has a previous run to compare with, refreshed when the run changes state. */
export function useCanComparePrevious(): boolean {
  const state = useStudio((s) => s.run.state);
  const generation = useStudio((s) => s.run.generation);
  const connected = useStudio((s) => s.connection) === "open";
  const [can, setCan] = useState(false);
  useEffect(() => {
    if (!connected) {
      setCan(false);
      return;
    }
    let live = true;
    engine
      .request("runs.list", {})
      .then((r: RunsListResult) => live && setCan(r.can_compare_previous))
      .catch(() => live && setCan(false));
    return () => {
      live = false;
    };
  }, [state, generation, connected]);
  return can;
}

/** The header's "Compare with previous run" button; hidden until there is a previous run. */
export function ComparePreviousButton(): React.JSX.Element | null {
  const can = useCanComparePrevious();
  const open = useRunCompare((s) => s.open);
  if (!can) return null;
  return (
    <button
      type="button"
      className={`icon-button labelled${open ? " on" : ""}`}
      aria-pressed={open}
      onClick={() => (open ? useRunCompare.getState().setOpen(false) : openRunCompare())}
      data-testid="compare-previous-button"
      title="Compare this run with the previous one: what changed in the settings and in every metric"
    >
      <span>Compare with previous</span>
    </button>
  );
}

export function RunCompare(): React.JSX.Element | null {
  const open = useRunCompare((s) => s.open);
  if (!open) return null;
  return <CompareDialog />;
}

type Load = { status: "loading" } | { status: "ready"; data: RunsCompareResult } | { status: "error"; error: string };

function close(): void {
  useRunCompare.getState().setOpen(false);
}

function CompareDialog(): React.JSX.Element {
  const a = useRunCompare((s) => s.a);
  const b = useRunCompare((s) => s.b);
  const runState = useStudio((s) => s.run.state);
  const [load, setLoad] = useState<Load>({ status: "loading" });
  const [all, setAll] = useState(false);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const host = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let live = true;
    setLoad({ status: "loading" });
    engine
      .request("runs.compare", { a, b })
      .then((data) => {
        if (!live) return;
        setLoad({ status: "ready", data });
        setSelected((s) => s ?? data.rows.find((r) => r.meaningful)?.name ?? data.rows[0]?.name ?? null);
      })
      .catch((e: unknown) => live && setLoad({ status: "error", error: e instanceof Error ? e.message : String(e) }));
    return () => {
      live = false;
    };
    // A finished run has its whole digest: re-read when the current one finishes.
  }, [a, b, tick, runState === "finished"]);

  useEffect(() => {
    host.current?.focus();
    const onKey = (ev: KeyboardEvent): void => {
      if (ev.key === "Escape" && !ev.defaultPrevented) {
        ev.preventDefault();
        ev.stopPropagation();
        close();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const data = load.status === "ready" ? load.data : null;
  const rows = useMemo(() => (data ? visibleRows(data.rows, all, query) : []), [data, all, query]);
  const counts = data ? tally(data.rows) : null;

  const exportAs = async (format: "csv" | "markdown"): Promise<void> => {
    if (!data) return;
    const r = await engine.request("runs.compare", { a, b, format });
    download(exportName(data, format === "csv" ? "csv" : "md"), new Blob([r.text ?? ""], { type: format === "csv" ? "text/csv;charset=utf-8" : "text/markdown;charset=utf-8" }));
  };
  const askAgent = (): void => {
    if (!data) return;
    close();
    openPanel("agent");
    void useAgent.getState().ask(agentPrompt(data));
  };

  return (
    <div className="rsum-backdrop" onMouseDown={(e) => e.target === e.currentTarget && close()} data-testid="run-compare-backdrop">
      <div ref={host} className="rsum rcmp" role="dialog" aria-modal="true" aria-labelledby="rcmp-title" tabIndex={-1} data-testid="run-compare">
        <header className="rsum-head">
          <h2 id="rcmp-title">Compare runs</h2>
          <span className="grow" />
          <button type="button" className="small" onClick={() => setTick((t) => t + 1)} disabled={load.status === "loading"} data-testid="run-compare-refresh">
            {load.status === "loading" ? "Reading…" : "Refresh"}
          </button>
          <button type="button" className="small" onClick={() => void exportAs("csv")} disabled={!data} data-testid="run-compare-csv">
            CSV
          </button>
          <button type="button" className="small" onClick={() => void exportAs("markdown")} disabled={!data} data-testid="run-compare-md">
            Markdown
          </button>
          <button type="button" className="small" onClick={askAgent} disabled={!data} data-testid="run-compare-agent" title="Hand both runs and this comparison to the agent">
            Ask the agent
          </button>
          <button type="button" className="icon-button" onClick={close} aria-label="Close the comparison" title="Close (Esc)" data-testid="run-compare-close">
            <CloseIcon />
          </button>
        </header>
        <div className="rsum-body">
          {load.status === "error" ? (
            <p className="metrics-message" data-testid="run-compare-error">
              The engine could not compare these runs: {load.error}
            </p>
          ) : data === null ? (
            <p className="dim">Comparing…</p>
          ) : (
            <>
              <div className="rcmp-sides" data-testid="run-compare-sides">
                <div className="rcmp-side a">
                  <span className="rcmp-tag">A · {a === "previous" ? "previous run" : a === "current" ? "current run" : "baseline"}</span>
                  <span>{sideText(data.a)}</span>
                </div>
                <div className="rcmp-side b">
                  <span className="rcmp-tag">B · {b === "current" ? "current run" : b === "previous" ? "previous run" : "compared"}</span>
                  <span>{sideText(data.b)}</span>
                </div>
              </div>
              <p className={`rcmp-note ${data.same_seed ? "same" : "noise"}`} data-testid="run-compare-noise" data-same-seed={data.same_seed}>
                {data.noise_note}
              </p>

              <section className="rsum-section" aria-label="Main differences">
                <h3>Main differences</h3>
                <ul className="rcmp-summary" data-testid="run-compare-summary">
                  {data.summary.map((line, i) => (
                    <li key={i}>{line}</li>
                  ))}
                </ul>
              </section>

              <section className="rsum-section" aria-label="Settings changed">
                <h3>What changed in the settings</h3>
                {data.settings.length === 0 ? (
                  <p className="dim">No setting differs.</p>
                ) : (
                  <table className="rcmp-table" data-testid="run-compare-settings">
                    <thead>
                      <tr>
                        <th>Setting</th>
                        <th>Section</th>
                        <th>A</th>
                        <th>B</th>
                      </tr>
                    </thead>
                    <tbody>
                      {data.settings.map((s) => (
                        <tr key={s.path} title={s.path}>
                          <td>{s.label}</td>
                          <td className="dim">{s.group}</td>
                          <td className="num">{s.text.split(" → ")[0]}</td>
                          <td className="num">{s.text.split(" → ")[1]}</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                )}
              </section>

              {selected !== null ? <Overlay a={a} b={b} name={selected} row={data.rows.find((r) => r.name === selected) ?? null} tick={tick} /> : null}

              <section className="rsum-section" aria-label="Metrics">
                <div className="rcmp-tools">
                  <h3>
                    Metrics{" "}
                    {counts ? (
                      <span className="dim">
                        · {counts.meaningful} of {counts.total} changed meaningfully ({counts.better} better, {counts.worse} worse)
                      </span>
                    ) : null}
                  </h3>
                  <span className="grow" />
                  <input type="search" placeholder="Filter metrics" value={query} onChange={(e) => setQuery(e.target.value)} aria-label="Filter metrics" />
                  <label className="rcmp-all">
                    <input type="checkbox" checked={all} onChange={(e) => setAll(e.target.checked)} data-testid="run-compare-all" /> Show all
                  </label>
                </div>
                {rows.length === 0 ? (
                  <p className="dim">{all || query ? "No metric matches." : "No metric changed meaningfully. Tick “Show all” to see every metric."}</p>
                ) : (
                  <table className="rcmp-table" data-testid="run-compare-metrics">
                    <thead>
                      <tr>
                        <th>Metric</th>
                        <th className="num">A (previous)</th>
                        <th className="num">B (current)</th>
                        <th className="num">Change</th>
                        <th className="num">% change</th>
                        <th>Health A → B</th>
                      </tr>
                    </thead>
                    <tbody>
                      {rows.map((r) => (
                        <MetricTr key={r.name} r={r} selected={r.name === selected} onSelect={() => setSelected(r.name)} />
                      ))}
                    </tbody>
                  </table>
                )}
              </section>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function MetricTr({ r, selected, onSelect }: { r: RunMetricRow; selected: boolean; onSelect: () => void }): React.JSX.Element {
  const cls = verdictClass(r);
  const why =
    r.better === "neutral"
      ? "Neither direction is better for this metric."
      : `${r.better === "higher" ? "Higher" : "Lower"} is better.${r.beyond_noise === false ? " Within run-to-run noise." : ""}`;
  return (
    <tr className={`${selected ? "on" : ""}${r.meaningful ? " meaningful" : ""}`} onClick={onSelect} data-testid={`run-compare-row-${r.name}`} data-verdict={r.verdict} title={why}>
      <td>{rowLabel(r.name)}</td>
      <td className="num">{formatWithUnit(r.a, r.unit)}</td>
      <td className="num">{formatWithUnit(r.b, r.unit)}</td>
      <td className={`num rcmp-${cls}`}>{deltaText(r)}</td>
      <td className={`num rcmp-${cls}`}>
        {pctText(r)}
        {r.verdict === "better" ? " ▲ better" : r.verdict === "worse" ? " ▼ worse" : ""}
      </td>
      <td>
        {r.level_a !== null && r.level_b !== null ? (
          <span className="rcmp-health">
            <HealthDot level={r.level_a} /> {r.level_a}
            {r.level_a !== r.level_b ? (
              <>
                {" → "}
                <HealthDot level={r.level_b} /> {r.level_b}
              </>
            ) : null}
          </span>
        ) : (
          <span className="dim">—</span>
        )}
      </td>
    </tr>
  );
}

/** The selected metric's two series on one time axis. */
function Overlay({ a, b, name, row, tick }: { a: string; b: string; name: string; row: RunMetricRow | null; tick: number }): React.JSX.Element {
  const hostRef = useRef<HTMLDivElement | null>(null);
  const theme = useStudio((s) => s.theme);
  const [series, setSeries] = useState<{ a: [number, number][]; b: [number, number][] } | null>(null);

  useEffect(() => {
    let live = true;
    const one = (run: string): Promise<[number, number][]> =>
      engine.request("runs.digest", { run, metrics: [name] }).then((d: RunDigest) => d.metrics.find((m) => m.name === name)?.series ?? []);
    Promise.all([one(a), one(b)])
      .then(([sa, sb]) => live && setSeries({ a: sa, b: sb }))
      .catch(() => live && setSeries({ a: [], b: [] }));
    return () => {
      live = false;
    };
  }, [a, b, name, tick]);

  useLayoutEffect(() => {
    const host = hostRef.current;
    if (!host || series === null) return;
    const css = getComputedStyle(document.documentElement);
    const ca = css.getPropertyValue("--viz-1").trim() || "#3987e5";
    const cb = css.getPropertyValue("--viz-2").trim() || "#e69f00";
    const grid = css.getPropertyValue("--border").trim() || "#223041";
    const text = css.getPropertyValue("--text-dim").trim() || "#8fa3b8";
    const xs = [...new Set([...series.a.map((p) => p[0]), ...series.b.map((p) => p[0])])].sort((x, y) => x - y);
    const at = (s: [number, number][]): (number | null)[] => {
      const m = new Map(s.map((p) => [p[0], p[1]]));
      return xs.map((x) => m.get(x) ?? null);
    };
    const width = Math.max(320, host.clientWidth || 640);
    const plot = new uPlot(
      {
        width,
        height: 200,
        legend: { show: true },
        cursor: { show: true, drag: { x: false, y: false } },
        scales: { x: { time: false } },
        axes: [
          { stroke: text, grid: { stroke: grid, width: 1 }, size: 26, font: "10px system-ui", label: "simulated time (s)", labelSize: 14, labelFont: "10px system-ui" },
          { stroke: text, grid: { stroke: grid, width: 1 }, size: 52, font: "10px system-ui" },
        ],
        series: [
          { label: "t (s)" },
          { label: "A (previous)", stroke: ca, width: 1.6, points: { show: false }, spanGaps: true },
          { label: "B (current)", stroke: cb, width: 1.6, points: { show: false }, spanGaps: true },
        ],
      },
      [xs, at(series.a), at(series.b)] as unknown as uPlot.AlignedData,
      host,
    );
    return () => plot.destroy();
  }, [series, theme]);

  return (
    <section className="rsum-section" aria-label="Overlaid chart" data-testid="run-compare-chart">
      <h3>
        {rowLabel(name)} over time{row ? <span className="dim"> · {row.unit === "" ? "" : `${row.unit} · `}click a metric below to chart it</span> : null}
      </h3>
      {series !== null && series.a.length === 0 && series.b.length === 0 ? <p className="dim">No series kept for this metric.</p> : null}
      <div ref={hostRef} className="rcmp-chart" />
    </section>
  );
}
