/**
 * The credential system at a glance, at the top of the metrics dashboard: five questions about the
 * SCMS (or CCMS) part of the run, each answered in one sentence, with the numbers behind the answer
 * as tiles, a report funnel, and bars where a breakdown says more than a number.
 *
 * Every tile says where its number comes from (hover or focus it), every cue says why, and a tile
 * that is a catalogue metric opens that metric's full chart. The authorities' own counts, unlabelled
 * and complete, are one click away under "Raw counts". The words and numbers are
 * `scmsModel.ts`'s, which the unit tests pin; this file only fetches and draws.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import { snapshotOf, type BackendSnapshot } from "../lib/backend.js";
import { openPanel } from "../shell/route.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { fetchGroups } from "./data.js";
import { LevelMark } from "./HealthBadge.js";
import { LIVE_REFRESH_MS, useRunKey, type Overview } from "./hooks.js";
import { formatNumber, observedCount, type GroupRow, type MetricFamily, type Series } from "./model.js";
import { hasCredentials, scmsOverview, POOLED, type FunnelStep, type MetricReader, type ScmsBars, type ScmsSection, type ScmsTile } from "./scmsModel.js";

/** How often a playing run's SCMS numbers refresh: the snapshot is once a simulated second. */
const SCMS_REFRESH_MS = Math.max(LIVE_REFRESH_MS, 3000);

const OPEN_KEY = "vwp.metrics.scms.open";
const RAW_KEY = "vwp.metrics.scms.raw";

function readFlag(key: string, fallback: boolean): boolean {
  try {
    const v = localStorage.getItem(key);
    return v === null ? fallback : v === "1";
  } catch {
    return fallback;
  }
}

function writeFlag(key: string, v: boolean): void {
  try {
    localStorage.setItem(key, v ? "1" : "0");
  } catch {
    /* a private window: the choice lasts until the page closes */
  }
}

interface Sources {
  readonly snapshot: BackendSnapshot | null;
  readonly pooled: ReadonlyMap<string, readonly GroupRow[]>;
  readonly status: "loading" | "ready" | "error";
}

/**
 * The Backend snapshot and the run-wide groupings, refreshed while the run plays and once when it
 * stops or seeks. A source the engine does not have (no credential system, no breakdown) is simply
 * absent; the overview shows what remains.
 */
function useScmsSources(families: readonly MetricFamily[], active: boolean): Sources {
  const runKey = useRunKey();
  const tNs = useStudio((s) => s.run.tNs);
  const playing = useStudio((s) => s.run.state === "running" || s.run.state === "seeking");
  const [state, setState] = useState<Sources>({ snapshot: null, pooled: new Map(), status: "loading" });
  const last = useRef({ key: "", wall: 0, tNs: -1 });
  const inFlight = useRef(false);
  const asks = useMemo(() => POOLED.filter((p) => families.some((f) => f.base === p.metric && f.breakdowns.includes(p.dim))), [families]);
  const key = `${runKey}|${asks.map((a) => a.metric).join(",")}`;

  useEffect(() => setState({ snapshot: null, pooled: new Map(), status: "loading" }), [runKey]);

  useEffect(() => {
    if (!active || inFlight.current) return;
    const now = Date.now();
    const l = last.current;
    const due = l.key !== key || (l.tNs !== tNs && (!playing || now - l.wall >= SCMS_REFRESH_MS));
    if (!due) return;
    last.current = { key, wall: now, tNs };
    inFlight.current = true;
    const requested = key;
    void (async () => {
      const [backend, ...groups] = await Promise.all([
        engine.request("inspect.entity", { entity: "backend", limit: 4 }, { quiet: true, timeoutMs: 4000 }).catch(() => null),
        ...asks.map((a) =>
          fetchGroups(a.metric, a.dim, { fromNs: 0, toNs: tNs })
            .then((r) => [`${a.metric}/${a.dim}`, r.rows] as const)
            .catch(() => null),
        ),
      ]);
      inFlight.current = false;
      if (last.current.key !== requested) return;
      const pooled = new Map<string, readonly GroupRow[]>();
      for (const g of groups) if (g !== null) pooled.set(g[0], g[1]);
      setState({ snapshot: backend === null ? null : snapshotOf(backend), pooled, status: backend === null && pooled.size === 0 ? "error" : "ready" });
    })();
  }, [active, key, tNs, playing, asks]);
  return state;
}

/** A tile's trend: a line of the run so far, no axes; the card it opens has those. */
function MiniSpark({ series, label }: { series: Series; label: string }): React.JSX.Element | null {
  const d = useMemo(() => {
    if (observedCount(series) < 2) return null;
    let lo = Number.POSITIVE_INFINITY;
    let hi = Number.NEGATIVE_INFINITY;
    for (const v of series.v) {
      if (Number.isNaN(v)) continue;
      lo = Math.min(lo, v);
      hi = Math.max(hi, v);
    }
    // Spaced by reported window, not by time: a fast-forwarded run reports in short radio windows
    // days apart, and on a time axis its whole trend is a sliver at the end. The full chart keeps time.
    const n = observedCount(series);
    const flat = hi - lo < 1e-12;
    let path = "";
    let k = 0;
    for (let i = 0; i < series.t.length; i++) {
      const v = series.v[i]!;
      if (Number.isNaN(v)) continue;
      const x = (k / Math.max(1, n - 1)) * 120;
      const y = flat ? 12 : 2 + (1 - (v - lo) / (hi - lo)) * 20;
      path += `${k === 0 ? "M" : "L"}${x.toFixed(1)},${y.toFixed(1)}`;
      k++;
    }
    return { path, lo, hi };
  }, [series]);
  if (d === null) return null;
  return (
    <svg className="scms-spark" viewBox="0 0 120 24" preserveAspectRatio="none" role="img" aria-label={`${label}: trend over the windows that reported, ${formatNumber(d.lo)} to ${formatNumber(d.hi)}`}>
      <path d={d.path} vectorEffect="non-scaling-stroke" />
    </svg>
  );
}

function Tile({ t, overview, onOpen }: { t: ScmsTile; overview: Overview; onOpen: (base: string) => void }): React.JSX.Element {
  const spark = t.spark ? overview.series.get(t.spark) : undefined;
  const tip = [t.cue, t.source, t.metric ? "Click to open its full chart." : null].filter(Boolean).join("\n\n");
  const body = (
    <>
      <span className="scms-tile-title">{t.title}</span>
      <span className="scms-tile-value">
        <span className="scms-num" data-testid={`scms-value-${t.id}`}>
          {t.value}
        </span>
        {t.unit !== "" ? <span className="scms-unit">{t.unit}</span> : null}
        {t.level ? <LevelMark level={t.level} /> : null}
      </span>
      <span className="scms-tile-meaning">{t.meaning}</span>
      {t.level && t.level !== "healthy" && t.cue ? <span className={`scms-tile-cue ${t.level}`}>{t.cue.split(". Thresholds:")[0]}</span> : null}
      {t.split ? <span className="scms-tile-split mono">{t.split}</span> : null}
      {spark ? <MiniSpark series={spark} label={t.title} /> : null}
    </>
  );
  const common = { className: `scms-tile${t.level ? ` ${t.level}` : ""}${t.metric ? " opens" : ""}`, title: tip, "data-testid": `scms-tile-${t.id}`, "data-level": t.level ?? "none" };
  return t.metric ? (
    <button type="button" {...common} onClick={() => onOpen(t.metric!)}>
      {body}
    </button>
  ) : (
    <div {...common} tabIndex={0} role="group" aria-label={`${t.title}: ${t.value} ${t.unit}`}>
      {body}
    </div>
  );
}

function Funnel({ steps }: { steps: readonly FunnelStep[] }): React.JSX.Element {
  const max = Math.max(1, ...steps.map((s) => s.value));
  return (
    <div className="scms-block">
      <h5>Where the reports are</h5>
      <ol className="scms-funnel" data-testid="scms-funnel" aria-label="Misbehaviour reports, from the vehicle to the Misbehaviour Authority">
        {steps.map((s, i) => (
          <li key={s.id} className="scms-step" title={s.hint} tabIndex={0} data-testid={`scms-step-${s.id}`}>
            <span className="scms-step-num mono">{formatNumber(s.value)}</span>
            <span className="scms-step-label">{s.label}</span>
            <span className="scms-step-bar" aria-hidden="true">
              <span style={{ width: `${Math.max(s.value > 0 ? 3 : 0, (s.value / max) * 100)}%` }} />
            </span>
            {i < steps.length - 1 ? <span className="scms-step-arrow" aria-hidden="true">→</span> : null}
          </li>
        ))}
      </ol>
      <p className="dim scms-note">Hover or focus a step for what it counts. Reports wait in the RA's shuffle by design, so the MA cannot tell who sent them.</p>
    </div>
  );
}

function Bars({ b, onOpen, testId }: { b: ScmsBars; onOpen: (base: string) => void; testId: string }): React.JSX.Element {
  return (
    <div className="scms-block" data-testid={testId}>
      <h5>
        {b.title}
        {b.metric ? (
          <button type="button" className="linklike scms-chart-link" onClick={() => onOpen(b.metric!)}>
            Full chart
          </button>
        ) : null}
      </h5>
      {b.rows.length === 0 ? (
        <p className="dim scms-note">{b.empty}</p>
      ) : (
        <>
          <ul className="scms-bars">
            {b.rows.map((r) => (
              <li key={r.key} className="scms-bar-row" title={r.hint ?? r.key} data-testid={`scms-bar-${r.key}`}>
                <span className="scms-bar-label">{r.label}</span>
                <span className="scms-bar-track">
                  <span className={`scms-bar${r.level ? ` ${r.level}` : ""}`} style={{ width: `${Math.max(1, Math.min(100, (r.value / Math.max(1e-12, b.max)) * 100))}%` }} />
                  {b.limit ? <span className="scms-bar-limit" style={{ left: `${b.limit.at * 100}%` }} title={b.limit.label} /> : null}
                </span>
                <span className="scms-bar-val mono">{r.display}</span>
              </li>
            ))}
          </ul>
          <p className="dim scms-note">
            {b.note}
            {b.limit ? ` The dashed line: ${b.limit.label}.` : ""}
          </p>
        </>
      )}
    </div>
  );
}

function Panel({ s, overview, onOpen }: { s: ScmsSection; overview: Overview; onOpen: (base: string) => void }): React.JSX.Element {
  return (
    <article className={`scms-panel${s.level ? ` ${s.level}` : ""}`} id={`scms-${s.id}`} data-testid={`scms-section-${s.id}`} data-level={s.level ?? "none"} aria-labelledby={`scms-q-${s.id}`}>
      <header className="scms-panel-head">
        <h4 id={`scms-q-${s.id}`}>{s.question}</h4>
        <p className="scms-answer" data-testid={`scms-answer-${s.id}`}>
          {s.level ? <LevelMark level={s.level} /> : null}
          <span>{s.answer}</span>
        </p>
      </header>
      {s.tiles.length > 0 ? (
        <div className="scms-tiles">
          {s.tiles.map((t) => (
            <Tile key={t.id} t={t} overview={overview} onOpen={onOpen} />
          ))}
        </div>
      ) : null}
      {s.funnel ? <Funnel steps={s.funnel} /> : null}
      {s.bars ? <Bars b={s.bars} onOpen={onOpen} testId={`scms-bars-${s.id}`} /> : null}
    </article>
  );
}

export function ScmsOverview({
  families,
  overview,
  active,
  onOpen,
}: {
  families: readonly MetricFamily[];
  overview: Overview;
  active: boolean;
  onOpen: (f: MetricFamily) => void;
}): React.JSX.Element | null {
  const [open, setOpen] = useState(() => readFlag(OPEN_KEY, true));
  const [raw, setRaw] = useState(() => readFlag(RAW_KEY, false));
  const src = useScmsSources(families, active);
  const tNs = useStudio((s) => s.run.tNs);
  const scenario = useStudio((s) => s.scenario);
  const reader = useMemo<MetricReader>(
    () => ({
      family: (b) => families.find((f) => f.base === b),
      latest: (n) => overview.latest.get(n) ?? null,
      series: (n) => overview.series.get(n),
      pooled: (m, d) => src.pooled.get(`${m}/${d}`),
    }),
    [families, overview, src.pooled],
  );
  if (!hasCredentials(src.snapshot, reader)) return null;
  const o = scmsOverview(src.snapshot, reader, { tNs: src.snapshot?.t ?? tNs, scenario });
  const openBase = (base: string): void => {
    const f = families.find((x) => x.base === base);
    if (f) onOpen(f);
  };
  return (
    <section id="metrics-group-scms" className="scms" data-testid="metrics-scms" aria-labelledby="scms-title">
      <header className="scms-head">
        <button
          type="button"
          className="scms-toggle"
          aria-expanded={open}
          aria-controls="scms-body"
          onClick={() => {
            setOpen(!open);
            writeFlag(OPEN_KEY, !open);
          }}
          data-testid="scms-toggle"
        >
          <span className={`scms-caret${open ? " open" : ""}`} aria-hidden="true">
            ▸
          </span>
          <h3 id="scms-title">Credentials and security</h3>
        </button>
        <span className="scms-facts dim" data-testid="scms-facts">
          {o.system}
          {o.facts !== "" ? ` · ${o.facts}` : ""}
        </span>
        <span className="grow" />
        {!open ? (
          <span className="scms-mini" aria-label="Each question's verdict">
            {o.sections.map((s) => (s.level ? <LevelMark key={s.id} level={s.level} compact title={`${s.question} ${s.answer}`} /> : null))}
          </span>
        ) : null}
        {src.snapshot !== null ? (
          <>
            <button
              type="button"
              className="small"
              aria-pressed={raw}
              onClick={() => {
                setRaw(!raw);
                writeFlag(RAW_KEY, !raw);
                if (!open) setOpen(true);
              }}
              data-testid="scms-raw-toggle"
              title="Every authority's own counts, exactly as the engine reports them"
            >
              {raw ? "Hide raw counts" : "Raw counts"}
            </button>
            <button type="button" className="small" onClick={() => openPanel("backend")} title="The authorities as a diagram, with every message between them">
              Backend view
            </button>
          </>
        ) : null}
      </header>
      {open ? (
        <div id="scms-body">
          <p className="dim scms-intro">
            Five questions about the credential system, each answered from the engine's own counts and metrics. Hover or focus a number for where it comes from; click one with a
            chart to open it.
            {src.status === "loading" ? " Reading the authorities' counts…" : src.snapshot === null ? " The engine published no Backend snapshot, so only the catalogue's metrics appear." : ""}
          </p>
          <div className="scms-grid">
            {o.sections.map((s) => (
              <Panel key={s.id} s={s} overview={overview} onOpen={openBase} />
            ))}
          </div>
          {raw && o.raw.length > 0 ? (
            <div className="scms-raw" data-testid="scms-raw">
              <h4>Raw counts, as each authority reports them</h4>
              <div className="scms-raw-grid">
                {o.raw.map((e) => (
                  <dl key={e.id} className="scms-raw-entity">
                    <dt className="scms-raw-name">
                      {e.name} <span className="mono dim">{e.id}</span>
                    </dt>
                    {e.counts.length === 0 ? <dd className="dim">No counts.</dd> : null}
                    {e.counts.map(([k, v]) => (
                      <dd key={k}>
                        <span className="mono dim">{k}</span>
                        <span className="mono">{v}</span>
                      </dd>
                    ))}
                  </dl>
                ))}
              </div>
            </div>
          ) : null}
        </div>
      ) : null}
    </section>
  );
}
