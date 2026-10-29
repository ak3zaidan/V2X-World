/**
 * The run summary: how the run performed, in one dialog over the simulator.
 *
 * It opens by itself when a run this page watched finishes, and from the header's or the metrics
 * dashboard's "Run summary" button at any time (mid-run it summarises the run so far). It says:
 *
 *  1. the scenario, in one line;
 *  2. the headline results a protocol evaluation leads with — delivery, latency, channel load,
 *     security overhead, credentials and revocation, traffic flow;
 *  3. every metric outside its healthy band, worst first, with how bad, when and (where the metric
 *     has a node dimension) which nodes;
 *  4. the bottlenecks — the latency stage that dominates, the loss cause that dominates, backend
 *     entities near saturation;
 *  5. the agent's analysis (`AgentSlot.tsx`), when the agent has one.
 *
 * Every number is the engine's (`metrics.query`, `inspect.entity`), every threshold the published
 * health band's (`crates/v2xw-metrics/src/health.rs`). Nothing is judged that has no band, and a
 * metric the run did not measure is listed as not measured, never as healthy.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import { snapshotOf } from "../lib/backend.js";
import { CloseIcon } from "../shell/Icons.js";
import { openPanel } from "../shell/route.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { AgentSlot } from "./AgentSlot.js";
import { fetchCatalogue, fetchGroups, fetchSeries, metricPeriodNs } from "./data.js";
import { classify, isCumulative, judge, judgedSeries, LEVEL_LABEL, noteOf, outOfBand, whenText, withExpectations, worseLevel, type HealthLevel, type Judgement } from "./health.js";
import { BandTable, HealthDot, HealthLine } from "./HealthBadge.js";
import { useRunKey } from "./hooks.js";
import { chooseBinNs, clockText, displayText, familiesOf, formatNumber, worstFirst, type MetricFamily } from "./model.js";
import { countOf, firstProvider, linkabilityReference, scenarioWindows, stageSpan, POOLED } from "./scmsModel.js";

/** Metrics the credential overview pools over the run (no per-window value, but a run total). */
const POOLED_IN_OVERVIEW: readonly string[] = POOLED.map((p) => p.metric);
import {
  baseOf,
  briefSentences,
  causeName,
  credentialRows,
  detectionPending,
  dominant,
  entityLoads,
  reduceSeries,
  scenarioLine,
  scenarioPurpose,
  stageName,
  ENTITY_UTILISATION_LIMIT,
  HEADLINES,
  SUMMARY_POINTS,
  type CredentialRow,
  type EntityLoad,
} from "./summaryModel.js";
import { closeRunSummary, useRunSummary } from "./summaryStore.js";
import { openRunCompare, useCanComparePrevious } from "./RunCompare.js";
import "./metrics.css";


interface SummaryData {
  readonly families: readonly MetricFamily[];
  readonly judgements: readonly Judgement[];
  /** Banded metrics the run did not measure. */
  readonly unmeasured: readonly MetricFamily[];
  readonly headline: ReadonlyMap<string, number | null>;
  readonly units: ReadonlyMap<string, string>;
  readonly where: ReadonlyMap<string, readonly { key: string; value: number | null }[]>;
  readonly stage: { key: string; share: number } | null;
  readonly cause: { key: string; share: number } | null;
  readonly entities: readonly EntityLoad[];
  readonly atS: number;
  /** The run in three to five sentences (`summaryModel.ts` `briefSentences`). */
  readonly brief: readonly string[];
  /** The credential tile's rows from the authorities' counts (`summaryModel.ts` `credentialRows`). */
  readonly credentialRows: readonly CredentialRow[];
}

type Load = { status: "idle" | "loading" } | { status: "error"; error: string } | { status: "ready"; data: SummaryData };

async function collect(tNs: number, scenario: unknown, finished = false): Promise<SummaryData> {
  const defs = await fetchCatalogue();
  // Judged against this run's own expectation, as the dashboard judges it (`health.ts` `expectedBand`).
  const families = withExpectations(familiesOf(defs), scenario);
  const known = new Set(defs.map((d) => d.name));
  const units = new Map(defs.map((d) => [d.name, d.unit] as const));
  const banded = families.filter((f) => f.health !== null && known.has(judgedSeries(f.base, f.health)));
  const headSeries = HEADLINES.flatMap((g) => g.items.map((i) => i.series)).filter((s) => known.has(s));
  // time_to_decision is read as the overview reads it (the mean of the windows with a decision).
  const names = [...new Set([...banded.map((f) => judgedSeries(f.base, f.health!)), ...headSeries, ...(known.has("time_to_decision") ? ["time_to_decision"] : [])])];
  const period = metricPeriodNs();
  const binNs = chooseBinNs(tNs / 1e9, period, SUMMARY_POINTS);
  const { series } = await fetchSeries(names, { fromNs: 0, toNs: tNs, binNs, limit: SUMMARY_POINTS * 2 + 4 });

  const judgements: Judgement[] = [];
  const unmeasured: MetricFamily[] = [];
  for (const f of banded) {
    const s = series.get(judgedSeries(f.base, f.health!));
    const j = s ? judge(f.base, f.unit, f.health!, s, f.label, isCumulative(f.definition)) : null;
    if (j === null) unmeasured.push(f);
    else judgements.push(j);
  }
  const headline = new Map<string, number | null>();
  // A bin wider than the metric period holds the mean of its windows, so a total over the run is
  // that sum scaled by the windows per bin.
  const perBin = Math.max(1, binNs / period);
  for (const g of HEADLINES) {
    for (const i of g.items) {
      if (!known.has(i.series)) continue;
      const v = reduceSeries(series.get(i.series) ?? { t: new Float64Array(0), v: new Float64Array(0) }, i.reduce);
      headline.set(i.series, v !== null && i.reduce === "sum" ? Math.round(v * perBin) : v);
    }
  }

  // Where: the worst nodes of each out-of-band metric that has a node dimension.
  const where = new Map<string, { key: string; value: number | null }[]>();
  const out = outOfBand(judgements).slice(0, 8);
  await Promise.all(
    out.map(async (j) => {
      const f = families.find((x) => x.base === j.base);
      if (!f || !f.breakdowns.includes("node")) return;
      try {
        const { rows } = await fetchGroups(j.base, "node", { fromNs: 0, toNs: tNs });
        where.set(j.base, worstFirst(rows, f.polarity).slice(0, 3));
      } catch {
        /* no breakdown on this engine */
      }
    }),
  );

  const optional = async <T,>(f: () => Promise<T>): Promise<T | null> => {
    try {
      return await f();
    } catch {
      return null;
    }
  };
  const [stageRows, causeRows, backend, accusations, recallRows, revocationRows] = await Promise.all([
    families.some((f) => f.base === "latency_stage_share") ? optional(() => fetchGroups("latency_stage_share", "stage", { fromNs: 0, toNs: tNs })) : null,
    families.some((f) => f.base === "pdr_by_cause") ? optional(() => fetchGroups("pdr_by_cause", "cause", { fromNs: 0, toNs: tNs })) : null,
    optional(() => engine.request("inspect.entity", { entity: "backend", limit: 40 }, { quiet: true, timeoutMs: 4000 })),
    families.some((f) => f.base === "false_accusations" && f.breakdowns.includes("level")) ? optional(() => fetchGroups("false_accusations", "level", { fromNs: 0, toNs: tNs })) : null,
    families.some((f) => f.base === "det_recall" && f.breakdowns.includes("level")) ? optional(() => fetchGroups("det_recall", "level", { fromNs: 0, toNs: tNs })) : null,
    families.some((f) => f.base === "revocation_latency_stage" && f.breakdowns.includes("stage")) ? optional(() => fetchGroups("revocation_latency_stage", "stage", { fromNs: 0, toNs: tNs })) : null,
  ]);
  const decisionS = reduceSeries(series.get("time_to_decision") ?? { t: new Float64Array(0), v: new Float64Array(0) }, "mean");
  const toListS = revocationRows ? stageSpan(revocationRows.rows, "decision", "published") : null;
  const compressed = [...scenarioWindows(scenario).values()].some((x) => x.compressed);
  const snap = backend === null ? null : snapshotOf(backend);
  const stage = stageRows ? dominant(stageRows.rows) : null;
  const entities = snap ? entityLoads(snap, tNs) : [];

  // The brief: the same judgements the dialog lists, the headline numbers, the authorities' counts.
  const first = firstProvider(scenario, snap);
  const count = (roles: string[], keys: string[]): number | null => countOf(snap, roles, keys, first)?.total ?? null;
  const healthyThroughout = judgements.filter((j) => j.level === "healthy" && j.worstLevel === "healthy").length;
  const outs = outOfBand(judgements);
  const honest = accusations?.rows.find((r) => r.key === "vehicle")?.value ?? null;
  // Detection recall per attacking vehicle (else per report), and its band's verdict.
  const recallV = recallRows?.rows.find((r) => r.key === "vehicle")?.value ?? recallRows?.rows.find((r) => r.key === "report")?.value ?? null;
  const recallBand = families.find((f) => f.base === "det_recall")?.health ?? null;
  const brief = briefSentences({
    judged:
      judgements.length === 0
        ? null
        : {
            unhealthy: judgements.filter((j) => j.level === "unhealthy").length,
            caution: judgements.filter((j) => j.level === "caution").length,
            atTimes: judgements.filter((j) => j.level === "healthy" && j.worstLevel !== "healthy").length,
            healthy: healthyThroughout,
            worstLabel: outs[0]?.label ?? null,
            worstHow: outs[0] ? howText(outs[0]) : null,
            worstNote: outs[0]?.base === "linkability" && linkabilityReference(scenario) !== null ? "expected without a silent period, as Wiedersheim et al. found at WONS 2010" : null,
          },
    delivery: { pdr: headline.get("pdr[100m]") ?? null, latencyMs: headline.get("e2e_latency") ?? null },
    credentials: {
      enrolled: count(["ee"], ["enrolled"]),
      devices: count(["ee"], ["devices", "stations"]),
      pool: headline.get("cert_pool_valid") ?? null,
      starved: count(["ee"], ["vehicles_starved"]),
    },
    misbehaviour: {
      filed: count(["ee"], ["reports_filed"]),
      revoked: count(["ma"], ["revocations"]),
      honestRevoked: honest,
      slowestStageS: headline.get("revocation_latency_stage.p95") ?? null,
      pending: detectionPending({
        revoked: count(["ma"], ["revocations"]) ?? 0,
        inShuffle: count(["ra"], ["reports_in_shuffle"]) ?? 0,
        atMa: count(["ma"], ["reports_received"]) ?? 0,
        filed: count(["ee"], ["reports_filed"]) ?? 0,
        shuffleS: snap?.entities.find((e) => e.id.split("/").pop() === "ra")?.state["shuffle_window_s"] as number | undefined,
        finished,
      }),
      recall: recallV,
      decisionS,
    },
    bottleneck: { stage: stage ? stageName(stage.key) : null, stageShare: stage?.share ?? null, entity: entities[0]?.name ?? null, utilisation: entities[0]?.utilisation ?? null },
  });
  return {
    families,
    judgements,
    unmeasured,
    headline,
    units,
    where,
    stage,
    cause: causeRows ? dominant(causeRows.rows) : null,
    entities,
    atS: tNs / 1e9,
    brief,
    credentialRows: credentialRows({
      revoked: count(["ma"], ["revocations"]),
      crlEntries: count(["crlg"], ["entries"]) ?? count(["ca"], ["ca_crl_entries"]),
      crlVersions: count(["crlg"], ["versions_signed"]) ?? count(["ca"], ["ca_crl_sequence"]),
      filed: count(["ee"], ["reports_filed"]),
      inShuffle: count(["ra"], ["reports_in_shuffle"]),
      atMa: count(["ma"], ["reports_received"]),
      finished,
      recall: recallV,
      recallLevel: recallBand ? classify(recallBand, recallV) : null,
      decisionS,
      toListS,
      compressed,
    }),
  };
}

/** Opens the summary by itself once per run, when a run this page saw playing finishes. */
function useAutoOpen(): void {
  const runKey = useRunKey();
  const state = useStudio((s) => s.run.state);
  const sawPlaying = useRef<string>("");
  useEffect(() => {
    if (state === "running" || state === "seeking" || state === "paused") sawPlaying.current = runKey;
    if (state === "finished" && sawPlaying.current === runKey && useRunSummary.getState().autoOpenedFor !== runKey) {
      useRunSummary.getState().noteAutoOpened(runKey);
    }
  }, [state, runKey]);
}

/** The header's door to the summary: "Summary", marked once the run has finished. */
export function RunSummaryButton(): React.JSX.Element | null {
  const state = useStudio((s) => s.run.state);
  const open = useRunSummary((s) => s.open);
  if (state === "idle" || state === "loading") return null;
  return (
    <button
      type="button"
      className={`icon-button labelled${open ? " on" : ""}${state === "finished" ? " rsum-ready" : ""}`}
      aria-pressed={open}
      onClick={() => useRunSummary.getState().setOpen(!open)}
      data-testid="run-summary-button"
      title={state === "finished" ? "How the run performed: headline results, anything unhealthy, bottlenecks" : "How the run is performing so far"}
    >
      <span>Summary</span>
    </button>
  );
}

export function RunSummary(): React.JSX.Element | null {
  useAutoOpen();
  const open = useRunSummary((s) => s.open);
  if (!open) return null;
  return <SummaryDialog />;
}

function SummaryDialog(): React.JSX.Element {
  const run = useStudio((s) => s.run);
  const scenario = useStudio((s) => s.scenario);
  const hello = useStudio((s) => s.hello);
  const runKey = useRunKey();
  const [load, setLoad] = useState<Load>({ status: "idle" });
  const [expanded, setExpanded] = useState<string | null>(null);
  const host = useRef<HTMLDivElement | null>(null);
  const tNs = run.tNs;
  const tRef = useRef(tNs);
  tRef.current = tNs;
  const scenarioRef = useRef(scenario);
  scenarioRef.current = scenario;
  const finishedRef = useRef(run.state === "finished");
  finishedRef.current = run.state === "finished";

  const refresh = useCallback(() => {
    setLoad({ status: "loading" });
    collect(tRef.current, scenarioRef.current, finishedRef.current)
      .then((data) => setLoad({ status: "ready", data }))
      .catch((e: unknown) => setLoad({ status: "error", error: e instanceof Error ? e.message : String(e) }));
  }, []);
  // Once on open, and again when the run finishes or a new run starts under it.
  useEffect(() => {
    refresh();
  }, [refresh, runKey, run.state === "finished"]);

  useEffect(() => {
    host.current?.focus();
    const onKey = (ev: KeyboardEvent): void => {
      if (ev.key === "Escape" && !ev.defaultPrevented) {
        ev.preventDefault();
        ev.stopPropagation();
        closeRunSummary();
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, []);

  const showChart = (base: string): void => {
    closeRunSummary();
    useStudio.getState().setMetricsFocus({ title: base, metrics: [base] });
    openPanel("metrics");
  };

  const finished = run.state === "finished";
  const canCompare = useCanComparePrevious();
  const data = load.status === "ready" ? load.data : null;
  const judged = data?.judgements ?? [];
  const out = outOfBand(judged);
  const counts = { unhealthy: 0, caution: 0, healthy: 0 } as Record<HealthLevel, number>;
  for (const j of judged) counts[j.level]++;
  const healthy = judged.filter((j) => j.level === "healthy" && j.worstLevel === "healthy");
  const atTimes = counts.healthy - healthy.length;
  const overall: HealthLevel = counts.unhealthy > 0 ? "unhealthy" : counts.caution > 0 || atTimes > 0 ? "caution" : "healthy";

  return (
    <div className="rsum-backdrop" onMouseDown={(e) => e.target === e.currentTarget && closeRunSummary()} data-testid="run-summary-backdrop">
      <div ref={host} className="rsum" role="dialog" aria-modal="true" aria-labelledby="rsum-title" tabIndex={-1} data-testid="run-summary">
        <header className="rsum-head">
          <h2 id="rsum-title">Run summary</h2>
          <span className={`rsum-state ${finished ? "done" : ""}`}>{finished ? "Finished" : `Run so far · ${clockText(tNs / 1e9)}`}</span>
          <span className="grow" />
          <button type="button" className="small" onClick={refresh} disabled={load.status === "loading"} data-testid="run-summary-refresh">
            {load.status === "loading" ? "Reading…" : "Refresh"}
          </button>
          <button
            type="button"
            className="small"
            onClick={() => {
              closeRunSummary();
              openPanel("metrics");
            }}
          >
            All metrics
          </button>
          {canCompare ? (
            <button
              type="button"
              className="small"
              onClick={() => {
                closeRunSummary();
                openRunCompare();
              }}
              data-testid="run-summary-compare"
              title="What changed in the settings and in every metric since the previous run"
            >
              Compare with previous run
            </button>
          ) : null}
          <button type="button" className="icon-button" onClick={closeRunSummary} aria-label="Close the run summary" title="Close (Esc)" data-testid="run-summary-close">
            <CloseIcon />
          </button>
        </header>
        <div className="rsum-body">
          <p className="rsum-scenario" data-testid="run-summary-scenario">
            {scenarioLine(scenario, run, hello?.scenarioName ?? null)}
          </p>
          {scenarioPurpose(scenario) ? <p className="dim rsum-purpose">{scenarioPurpose(scenario)}</p> : null}

          {load.status === "error" ? (
            <p className="metrics-message" data-testid="run-summary-error">
              The engine did not answer ({load.error}). It may have stopped; press Refresh to ask again.
            </p>
          ) : data === null ? (
            <p className="dim">Reading the run's metrics…</p>
          ) : (
            <>
              <div className={`rsum-verdict ${overall}`} data-testid="run-summary-verdict" data-level={overall}>
                <HealthDot level={overall} />
                {judged.length === 0 ? (
                  <span>No metric with a published healthy range was measured in this run.</span>
                ) : (
                  <span className="rsum-verdict-text">
                    <strong data-testid="run-summary-verdict-head">
                      {verdictHead(counts.unhealthy, counts.caution, atTimes, judged.length)}
                      {out[0] ? `. Weakest: ${out[0].label} (${howText(out[0])})` : ""}.
                    </strong>
                    <span className="rsum-verdict-counts dim" data-testid="run-summary-verdict-counts">
                      Over the whole run, of {judged.length} metrics with a published healthy range: {verdictCounts(counts.unhealthy, counts.caution, atTimes, healthy.length)}.
                    </span>
                  </span>
                )}
              </div>

              <section className="rsum-section rsum-brief" aria-label="The run in brief" data-testid="run-summary-brief">
                <h3>In brief</h3>
                <p>{data.brief.join(" ")}</p>
              </section>

              <section className="rsum-section" aria-label="Headline results">
                <h3>Headline results</h3>
                <div className="rsum-tiles">
                  {HEADLINES.map((g) => {
                    const extra = g.id === "credentials" ? data.credentialRows : [];
                    // Detection reads from the authorities' rule (pending while reports wait in the shuffle), never twice.
                    const items = g.items.filter((i) => (data.headline.get(i.series) ?? null) !== null && !(i.series === "det_recall" && extra.some((r) => r.id === "detection")) && !(i.series === "crl_entries" && extra.some((r) => r.id === "crl-entries")));
                    return (
                      <div key={g.id} className="rsum-tile" data-testid={`run-summary-tile-${g.id}`}>
                        <h4>{g.title}</h4>
                        {items.length === 0 && extra.length === 0 ? (
                          <p className="dim">Not measured in this run.</p>
                        ) : (
                          <dl>
                            {items.map((i) => {
                              const v = data.headline.get(i.series) ?? null;
                              const unit = data.units.get(i.series) ?? "";
                              const f = data.families.find((x) => x.base === baseOf(i.series));
                              const lv = f?.health && judgedSeries(f.base, f.health) === i.series ? classify(f.health, v) : null;
                              return (
                                <div key={i.series} className="rsum-item">
                                  <dt>{i.label}</dt>
                                  <dd>
                                    {lv ? <HealthDot level={lv} /> : null}
                                    <span className="mono">{v === null ? "—" : displayText(baseOf(i.series), v, unit)}</span>
                                  </dd>
                                </div>
                              );
                            })}
                            {extra.map((r) => (
                              <div key={r.id} className="rsum-item" data-testid={`run-summary-cred-${r.id}`}>
                                <dt>{r.label}</dt>
                                <dd>
                                  {r.level ? <HealthDot level={r.level} /> : null}
                                  <span className="mono">{r.value}</span>
                                </dd>
                              </div>
                            ))}
                          </dl>
                        )}
                      </div>
                    );
                  })}
                </div>
              </section>

              <section className="rsum-section" aria-label="Outside the healthy range" data-testid="run-summary-outside">
                <h3>Outside the healthy range</h3>
                {out.length === 0 ? (
                  <p className="dim">{judged.length === 0 ? "Nothing was judged." : "Nothing: every judged metric stayed in its healthy range in every window."}</p>
                ) : (
                  <ol className="rsum-list">
                    {out.map((j) => {
                      const nodes = data.where.get(j.base);
                      const open = expanded === j.base;
                      return (
                        <li key={j.base} className={`rsum-row ${worse(j)}`} data-testid={`run-summary-out-${j.base}`} data-level={worse(j)}>
                          <div className="rsum-row-head">
                            <HealthDot level={worse(j)} />
                            <strong>{j.label}</strong>
                            <span className="rsum-level">{levelWords(j)}</span>
                            <span className="grow" />
                            <button type="button" className="linklike" onClick={() => setExpanded(open ? null : j.base)} aria-expanded={open}>
                              {open ? "Hide thresholds" : "Thresholds"}
                            </button>
                            <button type="button" className="linklike" onClick={() => showChart(j.base)}>
                              Chart
                            </button>
                          </div>
                          <HealthLine band={j.band} value={j.level === "healthy" ? j.worst : j.mean} unit={j.unit} bare />
                          <p className="dim rsum-when">
                            {j.cumulative
                              ? `Over the whole run ${displayText(j.base, j.mean, j.unit)}${j.level === "healthy" ? ", healthy" : ""}; `
                              : j.level === "healthy"
                                ? "Run mean healthy; "
                                : `Run mean ${displayText(j.base, j.mean, j.unit)}; `}
                            {whenText(j, data.atS)}.
                            {nodes && nodes.length > 0 ? ` Worst nodes: ${nodes.map((n) => `${n.key} (${displayText(j.base, n.value, j.unit)})`).join(", ")}.` : ""}
                          </p>
                          {open ? <BandTable band={j.band} unit={j.unit} /> : null}
                        </li>
                      );
                    })}
                  </ol>
                )}
              </section>

              <section className="rsum-section" aria-label="Bottlenecks" data-testid="run-summary-bottlenecks">
                <h3>Bottlenecks</h3>
                <Bottlenecks data={data} />
              </section>

              <AgentSlot />

              {healthy.length > 0 || data.unmeasured.length > 0 ? (
                <section className="rsum-section" aria-label="Healthy and not measured">
                  <h3>Healthy throughout</h3>
                  {healthy.length > 0 ? (
                    <ul className="rsum-healthy">
                      {healthy.map((j) => (
                        <li key={j.base} title={noteOf(j.band, "healthy")}>
                          <HealthDot level="healthy" /> {j.label} <span className="dim mono">{displayText(j.base, j.mean, j.unit)}</span>
                        </li>
                      ))}
                    </ul>
                  ) : (
                    <p className="dim">None.</p>
                  )}
                  {data.unmeasured.length > 0 ? (
                    <p className="dim" data-testid="run-summary-unmeasured">
                      No window reported a value, so not judged here: {data.unmeasured.map((f) => f.label).join(", ")}.
                      {data.unmeasured.some((f) => POOLED_IN_OVERVIEW.includes(f.base))
                        ? " Detection and revocation metrics are pooled over the whole run instead: their run totals, and whether they can be judged yet, are in the metrics page's Credentials at a glance."
                        : ""}
                    </p>
                  ) : null}
                </section>
              ) : null}
            </>
          )}
        </div>
      </div>
    </div>
  );
}

/**
 * The run's verdict in two or three words, over the whole run: "Healthy throughout", "Healthy on
 * average, out of range at times", "Mostly healthy" (at most one in six judged metrics caution or
 * unhealthy on average), "Several metrics out of range".
 */
function verdictHead(unhealthy: number, caution: number, atTimes: number, n: number): string {
  if (unhealthy + caution + atTimes === 0) return "Healthy throughout";
  if (unhealthy + caution === 0) return "Healthy on average, out of range at times";
  if (unhealthy + caution <= Math.max(1, Math.floor(n / 6))) return "Mostly healthy";
  return "Several metrics out of range";
}

/** The four categories in the words the health strip and the rows use: "1 unhealthy · 1 caution · 4 healthy, out of range at times · 12 healthy throughout". */
function verdictCounts(unhealthy: number, caution: number, atTimes: number, healthy: number): string {
  const parts: string[] = [];
  if (unhealthy > 0) parts.push(`${unhealthy} unhealthy`);
  if (caution > 0) parts.push(`${caution} caution`);
  if (atTimes > 0) parts.push(`${atTimes} healthy, out of range at times`);
  parts.push(`${healthy} healthy throughout`);
  return parts.join(" · ");
}

/** A judged metric's level in words, its run mean first: "Caution on average, unhealthy at times", "Unhealthy all run". */
function levelWords(j: Judgement): string {
  if (j.level === "healthy") return `${LEVEL_LABEL[j.worstLevel]} at times`;
  const all = j.level === "unhealthy" ? j.unhealthyShare >= 1 : j.cautionShare + j.unhealthyShare >= 1;
  const base = `${LEVEL_LABEL[j.level]} ${all ? "all run" : "on average"}`;
  return worseLevel(j.level, j.worstLevel) !== j.level ? `${base}, ${j.worstLevel} at times` : base;
}

/** "unhealthy all run, 90.9 %": how the weakest metric did, for the verdict and the brief. */
function howText(j: Judgement): string {
  return `${levelWords(j).toLowerCase()}, ${j.cumulative ? "" : j.level === "healthy" ? "worst " : "mean "}${displayText(j.base, j.level === "healthy" ? j.worst : j.mean, j.unit)}`;
}

/** A row's colour: the worse of the run mean's level and the worst window's. */
function worse(j: Judgement): HealthLevel {
  return worseLevel(j.level, j.worstLevel);
}

function Bottlenecks({ data }: { data: SummaryData }): React.JSX.Element {
  // A dominant stage or loss cause is a fact about where time and messages go, not a verdict: it is
  // shown neutral. Only an entity past its utilisation ceiling or a metric out of band is coloured.
  const lines: { key: string; level: HealthLevel | "info"; text: React.ReactNode }[] = [];
  if (data.stage !== null) {
    lines.push({
      key: "stage",
      level: "info",
      text: (
        <>
          <strong>{stageName(data.stage.key)}</strong> takes the largest share of end-to-end latency: {Math.round(data.stage.share * 100)} %.
        </>
      ),
    });
  }
  if (data.cause !== null) {
    lines.push({
      key: "cause",
      level: "info",
      text: (
        <>
          Most lost messages were lost to <strong>{causeName(data.cause.key)}</strong>: {Math.round(data.cause.share * 100)} % of losses.
        </>
      ),
    });
  }
  const busy = data.entities.filter((e) => e.utilisation >= ENTITY_UTILISATION_LIMIT * 0.75).slice(0, 4);
  for (const e of busy) {
    lines.push({
      key: `entity-${e.id}`,
      level: e.utilisation > ENTITY_UTILISATION_LIMIT ? "unhealthy" : "caution",
      text: (
        <>
          Backend <strong>{e.name}</strong> was busy {Math.round(e.utilisation * 100)} % of the time
          {e.meanWaitMs !== null ? `, requests waited ${formatNumber(e.meanWaitMs)} ms on average` : ""}
          {e.utilisation > ENTITY_UTILISATION_LIMIT ? " — past the 80 % planning ceiling, where queues grow without bound." : "."}
        </>
      ),
    });
  }
  const worstOut = outOfBand(data.judgements).filter((j) => j.level === "unhealthy").slice(0, 2);
  for (const j of worstOut) {
    const nodes = data.where.get(j.base);
    if (nodes && nodes.length > 0) {
      lines.push({
        key: `nodes-${j.base}`,
        level: "unhealthy",
        text: (
          <>
            <strong>{j.label}</strong> is worst at node {nodes[0]!.key} ({displayText(j.base, nodes[0]!.value, j.unit)}).
          </>
        ),
      });
    }
  }
  if (lines.length === 0) {
    const anyBackend = data.entities.length > 0;
    return (
      <p className="dim">
        No bottleneck stands out{anyBackend ? `: the busiest backend entity was ${data.entities[0]!.name} at ${Math.round(data.entities[0]!.utilisation * 100)} %` : ""}.
      </p>
    );
  }
  return (
    <ul className="rsum-bottlenecks">
      {lines.map((l) => (
        <li key={l.key}>
          <span className={`hdot ${l.level}`} aria-hidden="true" /> {l.text}
        </li>
      ))}
    </ul>
  );
}
