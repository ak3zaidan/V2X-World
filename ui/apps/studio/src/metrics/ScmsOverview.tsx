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
import { counterText } from "../lib/pkiTopology.js";
import { openPanel } from "../shell/route.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { fetchGroups, fetchSeries, metricPeriodNs } from "./data.js";
import { RUN_MEAN_SERIES, SUMMARY_POINTS } from "./summaryModel.js";
import { LevelMark } from "./HealthBadge.js";
import { LIVE_REFRESH_MS, useRunKey, type Overview } from "./hooks.js";
import { chooseBinNs, display, displayText, formatNumber, type GroupRow, type MetricFamily, type Series } from "./model.js";
import { coveredMetrics, hasCredentials, meanOf, scmsOverview, sparkShape, POOLED, type FunnelStep, type MetricReader, type ScmsBars, type ScmsSection, type ScmsTile } from "./scmsModel.js";
import type { HealthLevel } from "./health.js";

/** The section's name: the dashboard's navigation uses the same words. */
export const SCMS_TITLE = "Credentials at a glance";


/** What the overview tells the dashboard: each question's verdict, and the metrics it shows a value for. */
export interface ScmsReport {
  readonly verdicts: readonly ScmsVerdict[];
  readonly covered: readonly string[];
}

/** One question's verdict, as the navigation marks it. */
export interface ScmsVerdict {
  readonly id: string;
  readonly question: string;
  readonly answer: string;
  readonly level: HealthLevel | null;
  readonly pending: boolean;
  /** Why no verdict, when the question is not judged on purpose. */
  readonly notJudged?: string;
}

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
  /** Run means at the run summary's resolution (`RUN_MEAN_SERIES`). */
  readonly means: ReadonlyMap<string, number>;
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
  const [state, setState] = useState<Sources>({ snapshot: null, pooled: new Map(), means: new Map(), status: "loading" });
  const last = useRef({ key: "", wall: 0, tNs: -1 });
  const inFlight = useRef(false);
  const asks = useMemo(() => POOLED.filter((p) => families.some((f) => f.base === p.metric && f.breakdowns.includes(p.dim))), [families]);
  const meanNames = useMemo(() => RUN_MEAN_SERIES.filter((n) => families.some((f) => f.base === n)), [families]);
  const key = `${runKey}|${asks.map((a) => a.metric).join(",")}|${meanNames.join(",")}`;

  useEffect(() => setState({ snapshot: null, pooled: new Map(), means: new Map(), status: "loading" }), [runKey]);

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
      const period = metricPeriodNs();
      const [backend, fine, ...groups] = await Promise.all([
        engine.request("inspect.entity", { entity: "backend", limit: 4 }, { quiet: true, timeoutMs: 4000 }).catch(() => null),
        meanNames.length > 0 && tNs > 0
          ? fetchSeries(meanNames, { fromNs: 0, toNs: tNs, binNs: chooseBinNs(tNs / 1e9, period, SUMMARY_POINTS), limit: SUMMARY_POINTS * 2 + 4 }).catch(() => null)
          : Promise.resolve(null),
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
      const means = new Map<string, number>();
      for (const [n, s] of fine?.series ?? []) {
        const v = meanOf(s);
        if (v !== null) means.set(n, v);
      }
      setState({ snapshot: backend === null ? null : snapshotOf(backend), pooled, means, status: backend === null && pooled.size === 0 ? "error" : "ready" });
    })();
  }, [active, key, tNs, playing, asks, meanNames]);
  return state;
}

/**
 * Not judged yet, because the pipeline behind the number is still working by design (reports held in
 * the RA's shuffle): a hollow ring and "Pending" in the neutral colour, never a verdict.
 */
export function PendingMark({ compact = false, title }: { compact?: boolean; title?: string }): React.JSX.Element {
  return (
    <span className={`lmark pending${compact ? " compact" : ""}`} data-level="pending" title={title ?? "Pending: not judged yet"} role={compact ? "img" : undefined} aria-label={compact ? `Pending${title ? `: ${title}` : ""}` : undefined}>
      <span className="hdot pending" aria-hidden="true" />
      {compact ? null : "Pending"}
    </span>
  );
}

/**
 * No verdict, on purpose: no band or expectation judges this question. A flat dash and "Not judged"
 * in the neutral colour, with the reason as its tooltip, so the reader sees a chip, never a blank.
 */
export function NotJudgedMark({ compact = false, title }: { compact?: boolean; title?: string }): React.JSX.Element {
  return (
    <span
      className={`lmark none${compact ? " compact" : ""}`}
      data-level="none"
      title={title ?? "Not judged: no standard sets a threshold"}
      tabIndex={compact ? undefined : 0}
      role={compact ? "img" : undefined}
      aria-label={compact ? `Not judged${title ? `: ${title}` : ""}` : undefined}
    >
      <span className="hdot none" aria-hidden="true" />
      {compact ? null : "Not judged"}
    </span>
  );
}

/** A question's verdict mark: its level, pending, or not judged. */
function SectionMark({ s, compact = false }: { s: ScmsSection; compact?: boolean }): React.JSX.Element | null {
  const title = compact ? `${s.question} ${s.answer}` : undefined;
  if (s.level) return <LevelMark level={s.level} compact={compact} title={title} />;
  if (s.pending) return <PendingMark compact={compact} title={title} />;
  if (s.notJudged) return <NotJudgedMark compact={compact} title={compact ? title : s.notJudged} />;
  return null;
}

/** A sparkline's ends, in the tile's own unit: "4.9–5.0", "120–340 ms". */
function rangeText(lo: number, hi: number, base: string | undefined, unit: string | undefined): string {
  const a = display(base ?? "", lo, unit ?? "");
  const b = display(base ?? "", hi, unit ?? "");
  // Zero needs no unit of its own: "0–57 µs", not "0 ms–57 µs".
  if (lo === 0) return `0–${b.value}${b.unit ? ` ${b.unit}` : ""}`;
  return a.unit === b.unit ? `${a.value}–${b.value}${b.unit ? ` ${b.unit}` : ""}` : `${displayText(base ?? "", lo, unit ?? "")}–${displayText(base ?? "", hi, unit ?? "")}`;
}

/**
 * A tile's trend over the windows that reported: the range written at the left, a line (dots when
 * there are one to three windows), or "steady" when it barely moved. The card it opens has the axes.
 */
function MiniSpark({ series, label, base, unit }: { series: Series; label: string; base?: string; unit?: string }): React.JSX.Element | null {
  const shape = useMemo(() => sparkShape(series), [series]);
  if (shape === null) return null;
  if (shape.kind === "steady") {
    return (
      <span className="scms-spark-row steady" data-testid="scms-spark-steady">
        <span className="scms-spark-range">
          steady at {displayText(base ?? "", shape.value, unit ?? "")} over {shape.n} {shape.n === 1 ? "window" : "windows"}
        </span>
      </span>
    );
  }
  const range = rangeText(shape.lo, shape.hi, base, unit);
  return (
    <span className="scms-spark-row" data-testid={`scms-spark-${shape.kind}`}>
      <span className="scms-spark-range mono" title="Lowest and highest window so far">
        {range}
      </span>
      <svg className="scms-spark" viewBox="0 0 120 24" preserveAspectRatio="none" role="img" aria-label={`${label}: ${shape.kind === "dots" ? `${shape.points.length} windows` : "trend over the windows that reported"}, ${range}`}>
        <line className="scms-spark-base" x1="0" x2="120" y1="23" y2="23" vectorEffect="non-scaling-stroke" />
        {shape.kind === "line" ? (
          <path d={shape.path} vectorEffect="non-scaling-stroke" />
        ) : (
          shape.points.map((p, i) => <circle key={i} cx={p.x} cy={p.y} r="2.2" vectorEffect="non-scaling-stroke" />)
        )}
      </svg>
    </span>
  );
}

function Tile({ t, overview, onOpen }: { t: ScmsTile; overview: Overview; onOpen: (base: string) => void }): React.JSX.Element {
  const spark = t.spark ? overview.series.get(t.spark) : undefined;
  // The tile shows the plain name; the standard term in brackets is in the tooltip.
  const short = t.title.replace(/\s*\([^)]*\)/g, "").trim() || t.title;
  const tip = [short !== t.title ? t.title : null, t.meaning, t.cue, t.pending, t.source, t.metric ? "Click to open its full chart." : null].filter(Boolean).join("\n\n");
  const body = (
    <>
      <span className="scms-tile-title">
        <span>{short}</span>
        {t.level === "healthy" ? <LevelMark level={t.level} compact /> : null}
      </span>
      <span className="scms-tile-value">
        <span className="scms-num" data-testid={`scms-value-${t.id}`}>
          {t.value}
        </span>
        {t.unit !== "" ? <span className="scms-unit">{t.unit}</span> : null}
        {t.level && t.level !== "healthy" ? <LevelMark level={t.level} /> : t.pending ? <PendingMark /> : null}
      </span>
      {t.level && t.level !== "healthy" && t.cue ? <span className={`scms-tile-cue ${t.level}`}>{t.cue.split(". Thresholds:")[0].split(/(?<=\.) /)[0]}</span> : null}
      {t.pending ? (
        <span className="scms-tile-cue pending" data-testid={`scms-pending-${t.id}`}>
          {t.pending}
        </span>
      ) : null}
      {t.split ? (
        <span className="scms-tile-split mono">
          {t.split.split(" · ").map((part) => (
            <span key={part} className="scms-split-part">
              {part}
            </span>
          ))}
        </span>
      ) : null}
      {spark ? <MiniSpark series={spark} label={t.title} base={t.metric} unit={t.sparkUnit} /> : null}
    </>
  );
  const common = {
    className: `scms-tile${t.level ? ` ${t.level}` : t.pending ? " pending" : ""}${t.metric ? " opens" : ""}`,
    title: tip,
    "data-testid": `scms-tile-${t.id}`,
    "data-level": t.level ?? (t.pending ? "pending" : "none"),
  };
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

/**
 * Where the reports are, as a left-to-right flow of steps that are each a subset of the one before:
 * each step a box with its count, and between two steps an arrow that says how many did not move on
 * and why ("45 not uploaded"), or what joined ("+378 beyond the uploads").
 */
function Funnel({ steps }: { steps: readonly FunnelStep[] }): React.JSX.Element {
  return (
    <div className="scms-block">
      <h5>Where the reports are</h5>
      <ol className="scms-flow" data-testid="scms-funnel" aria-label="Misbehaviour reports, from the vehicle to the Misbehaviour Authority">
        {steps.map((s, i) => (
          <li key={s.id} className={`scms-flow-item${i === 0 ? " first" : ""}`}>
            {i > 0 ? (
              <span className="scms-flow-arrow" aria-hidden={s.drop === undefined} data-testid={`scms-drop-${s.id}`}>
                <span className="scms-flow-line" aria-hidden="true">
                  →
                </span>
                {s.drop !== undefined ? <span className="scms-flow-drop">{s.drop}</span> : null}
              </span>
            ) : null}
            <span className="scms-step" title={s.hint} tabIndex={0} data-testid={`scms-step-${s.id}`} aria-label={`${s.label}: ${formatNumber(s.value)}. ${s.hint}`}>
              <span className="scms-step-num mono">{formatNumber(s.value)}</span>
              <span className="scms-step-label">{s.label}</span>
            </span>
          </li>
        ))}
      </ol>
    </div>
  );
}

function Bars({ b, onOpen, testId }: { b: ScmsBars; onOpen: (base: string) => void; testId: string }): React.JSX.Element {
  return (
    <div className="scms-block" data-testid={testId}>
      <h5 title={`${b.note}${b.limit ? ` The dashed line: ${b.limit.label}.` : ""}`}>
        {b.title}
        {b.metric && b.rows.length > 0 && b.idle === undefined ? (
          <button type="button" className="linklike scms-chart-link" onClick={() => onOpen(b.metric!)}>
            Full chart
          </button>
        ) : null}
      </h5>
      {b.rows.length === 0 ? (
        <p className="dim scms-note">{b.empty}</p>
      ) : b.idle !== undefined ? (
        <p className="scms-idle" data-testid={`${testId}-idle`}>
          {b.idle}
        </p>
      ) : (
        <>
          {b.caveat ? (
            <p className="scms-caveat" data-testid={`${testId}-caveat`} title={b.caveat}>
              <span className="scms-hatch-key" aria-hidden="true" />
              Hatched: a window the scenario compresses, not a deployment's time.
            </p>
          ) : null}
          <ul className="scms-bars">
            {b.rows.map((r) => (
              <li key={r.key} className={`scms-bar-row${r.hatched ? " hatched" : ""}`} title={r.hint ?? r.key} data-testid={`scms-bar-${r.key}`} data-width={barWidth(r.value, b.max).toFixed(1)}>
                <span className="scms-bar-label">
                  <span className="scms-bar-name">{r.label}</span>
                  {r.tag ? (
                    <span className="scms-bar-tag" data-testid={`scms-bar-tag-${r.key}`}>
                      {r.tag}
                    </span>
                  ) : null}
                </span>
                <span className="scms-bar-track">
                  <span className={`scms-bar${r.level ? ` ${r.level}` : ""}${r.hatched ? " hatched" : ""}`} style={{ width: `${barWidth(r.value, b.max)}%` }} />
                  {b.limit ? <span className="scms-bar-limit" style={{ left: `${b.limit.at * 100}%` }} title={b.limit.label} /> : null}
                </span>
                <span className="scms-bar-val mono">{r.display}</span>
              </li>
            ))}
            {b.axis ? (
              <li className="scms-bar-row scms-bar-axis" aria-hidden="true" data-testid={`${testId}-axis`}>
                <span />
                <span className="scms-axis">
                  <span>{b.axis.lo}</span>
                  <span>{b.axis.hi}</span>
                </span>
                <span />
              </li>
            ) : null}
          </ul>
          {b.total ? (
            <p className="scms-total" data-testid={`${testId}-total`}>
              {b.total}
            </p>
          ) : null}
        </>
      )}
    </div>
  );
}

/** A bar's width in % of its track: never past the track, never invisible. */
function barWidth(v: number, max: number): number {
  return Math.max(1, Math.min(100, (v / Math.max(1e-12, max)) * 100));
}

/** An answer's first sentence; the rest (caveats, references) one click away. */
function Answer({ text }: { text: string }): React.JSX.Element {
  const [more, setMore] = useState(false);
  const cut = text.search(/(?<=[.;]) (?=[A-Z])/);
  if (cut < 0) return <span>{text}</span>;
  return (
    <span>
      {text.slice(0, cut)}{" "}
      {more ? (
        <span className="dim">{text.slice(cut + 1)}</span>
      ) : (
        <button type="button" className="linklike scms-more" onClick={() => setMore(true)} title={text.slice(cut + 1)}>
          more
        </button>
      )}
    </span>
  );
}

function Panel({ s, overview, onOpen }:{ s: ScmsSection; overview: Overview; onOpen: (base: string) => void }): React.JSX.Element {
  return (
    <article className={`scms-panel${s.level ? ` ${s.level}` : s.pending ? " pending" : ""}`} id={`scms-${s.id}`} data-testid={`scms-section-${s.id}`} data-level={s.level ?? (s.pending ? "pending" : "none")} aria-labelledby={`scms-q-${s.id}`}>
      <header className="scms-panel-head">
        <h4 id={`scms-q-${s.id}`}>{s.question}</h4>
        <p className="scms-answer" data-testid={`scms-answer-${s.id}`}>
          <SectionMark s={s} />
          <Answer text={s.answer} />
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

/**
 * Every question's answer on one row, before the detail: its topic, its verdict and a few words.
 * Each opens its question below.
 */
function AnswerStrip({ sections }: { sections: readonly ScmsSection[] }): React.JSX.Element {
  return (
    <nav className="scms-strip" aria-label="Each question's answer, in brief" data-testid="scms-strip">
      {sections.map((s) => (
        <button
          key={s.id}
          type="button"
          className={`scms-strip-item${s.level ? ` ${s.level}` : ""}`}
          data-testid={`scms-strip-${s.id}`}
          data-level={s.level ?? (s.pending ? "pending" : "none")}
          title={`${s.question}\n${s.answer}${!s.level && !s.pending && s.notJudged ? `\n\n${s.notJudged}` : ""}`}
          onClick={() => {
            const el = document.getElementById(`scms-${s.id}`);
            el?.scrollIntoView({ block: "start", behavior: "smooth" });
          }}
        >
          <span className="scms-strip-topic">{s.topic}</span>
          <SectionMark s={s} />
          <span className="scms-strip-short">{s.short ?? ""}</span>
        </button>
      ))}
    </nav>
  );
}

export function ScmsOverview({
  families,
  overview,
  active,
  onOpen,
  onReport,
}: {
  families: readonly MetricFamily[];
  overview: Overview;
  active: boolean;
  onOpen: (f: MetricFamily) => void;
  /** Each question's verdict (the navigation's marks) and the metrics shown here; called when they change. */
  onReport?: (r: ScmsReport) => void;
}): React.JSX.Element | null {
  const [open, setOpen] = useState(() => readFlag(OPEN_KEY, true));
  const [raw, setRaw] = useState(() => readFlag(RAW_KEY, false));
  const src = useScmsSources(families, active);
  const tNs = useStudio((s) => s.run.tNs);
  const scenario = useStudio((s) => s.scenario);
  const finished = useStudio((s) => s.run.state === "finished");
  const reader = useMemo<MetricReader>(
    () => ({
      family: (b) => families.find((f) => f.base === b),
      latest: (n) => overview.latest.get(n) ?? null,
      series: (n) => overview.series.get(n),
      pooled: (m, d) => src.pooled.get(`${m}/${d}`),
      runMean: (n) => src.means.get(n) ?? null,
    }),
    [families, overview, src.pooled, src.means],
  );
  const has = hasCredentials(src.snapshot, reader);
  const o = has ? scmsOverview(src.snapshot, reader, { tNs: src.snapshot?.t ?? tNs, scenario, finished }) : null;
  const covered = o === null ? [] : coveredMetrics(o);
  const reportKey = o === null ? "" : `${o.sections.map((x) => `${x.id}:${x.level ?? (x.pending ? "pending" : "none")}:${x.answer}`).join("|")}#${covered.join(",")}`;
  useEffect(() => {
    if (onReport === undefined) return;
    onReport({
      verdicts: o === null ? [] : o.sections.map((x) => ({ id: x.id, question: x.question, answer: x.answer, level: x.level, pending: x.pending === true, ...(x.notJudged ? { notJudged: x.notJudged } : {}) })),
      covered,
    });
    // The key carries every verdict and metric; the sections themselves are rebuilt each render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [reportKey, onReport]);
  // Raw counts open below the six questions: when asked for, they are scrolled to, so the click
  // visibly does something.
  const rawRef = useRef<HTMLDivElement | null>(null);
  const rawAsked = useRef(false);
  useEffect(() => {
    if (!raw || !rawAsked.current) return;
    rawAsked.current = false;
    rawRef.current?.scrollIntoView({ block: "start", behavior: "smooth" });
  }, [raw]);
  if (o === null) return null;
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
          <h3 id="scms-title">{SCMS_TITLE}</h3>
        </button>
        <span className="scms-facts dim" data-testid="scms-facts">
          {o.system}
          {o.facts !== "" ? ` · ${o.facts}` : ""}
        </span>
        <span className="grow" />
        {!open ? (
          <span className="scms-mini" aria-label="Each question's verdict">
            {o.sections.map((s) => (
              <SectionMark key={s.id} s={s} compact />
            ))}
          </span>
        ) : null}
        {src.snapshot !== null ? (
          <span className="scms-actions">
            <button
              type="button"
              className="small"
              aria-pressed={raw}
              onClick={() => {
                rawAsked.current = !raw;
                setRaw(!raw);
                writeFlag(RAW_KEY, !raw);
                if (!open) setOpen(true);
              }}
              data-testid="scms-raw-toggle"
              title="Every authority's own counters, as the engine reports them, for debugging"
            >
              {raw ? "Hide engine counters" : "Engine counters ↓"}
            </button>
            <button
              type="button"
              className="small"
              onClick={() => openPanel("backend")}
              title="The certificate system's own view: the authorities as a diagram, every certificate, and every message between them"
              data-testid="scms-open-system"
            >
              Open certificate system
            </button>
          </span>
        ) : null}
      </header>
      {open ? (
        <div id="scms-body">
          {src.status === "loading" ? <p className="dim scms-intro">Reading the authorities' counts…</p> : null}
          <AnswerStrip sections={o.sections} />
          <div className="scms-grid">
            {o.sections.map((s) => (
              <Panel key={s.id} s={s} overview={overview} onOpen={openBase} />
            ))}
          </div>
          {raw && o.raw.length > 0 ? (
            <div className="scms-raw" data-testid="scms-raw" ref={rawRef}>
              <h4>Engine counters (for debugging)</h4>
              <p className="dim scms-note">Each authority's own counters, as the engine reports them. Hover a name for the engine's key; the questions above already use the ones that matter.</p>
              <div className="scms-raw-grid">
                {o.raw.map((e) => (
                  <dl key={e.id} className="scms-raw-entity">
                    <dt className="scms-raw-name">
                      {e.name} <span className="mono dim">{e.id}</span>
                    </dt>
                    {e.counts.length === 0 ? <dd className="dim">No counts.</dd> : null}
                    {e.counts.map(([k, v]) => {
                      const words = counterText(k);
                      return (
                        <dd key={k} title={`${k}${words?.means ? `: ${words.means}` : ""}`} data-testid={`scms-raw-${e.id}-${k}`}>
                          <span className={words ? "scms-raw-key" : "scms-raw-key mono dim"}>{words?.label ?? k.replace(/_/g, " ")}</span>
                          <span className="mono scms-raw-val" title={v}>
                            {v}
                          </span>
                        </dd>
                      );
                    })}
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
