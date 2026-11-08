/**
 * The agent panel, kept minimal: a message list and one input box.
 *
 * - The person types anything ("set up busy traffic in Portland with DSRC and some attackers").
 *   The agent sets the settings up as the Settings window's draft and answers in a line; what it
 *   changed is one row of chips ("City → Portland, OR · Radio → DSRC"), each opening its
 *   setting, with Undo.
 * - The person presses Run. The engine's watcher follows the run and its alerts appear in the
 *   list as one-line cards (delivery collapsing, a busy channel, verify-queue drops, no vehicles,
 *   gridlock, attacks detected or missed); a click shows the numbers.
 * - Questions about the run are answered from its metrics. A reply's findings, runs and
 *   comparison tables are behind "Details"; nothing else is shown.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import {
  SUGGESTIONS,
  alertLine,
  chipLine,
  clock,
  foldEvents,
  isAlert,
  keyToPointer,
  layerStateLabel,
  notable,
  observationsOf,
  parseMarkdown,
  plain,
  verdictLabel,
  type AgentReport,
  type Chip,
  type Finding,
  type Inline,
  type JobView,
  type Observation,
  type RunRecord,
  type RunSummary,
} from "../lib/agent.js";
import { openMetricChart, revealSetting } from "../lib/links.js";
import { agentBusy, useAgent, useAgentWatch, type Turn } from "../state/agent.js";
import "../styles/agent.css";

const LAYER_TITLES: Record<string, string> = {
  traffic: "Traffic",
  channel: "Channel",
  access: "Access and queues",
  delivery: "Delivery",
  latency: "Latency",
  security: "Security",
  backend: "Credential backend",
  privacy: "Privacy",
  simulation: "Simulation",
};

function fmtNum(v: number | string): string {
  if (typeof v === "string") return v;
  if (!Number.isFinite(v)) return "n/a";
  const a = Math.abs(v);
  const s = a >= 100 ? v.toFixed(0) : a >= 10 ? v.toFixed(1) : a >= 0.01 || a === 0 ? v.toFixed(3) : v.toFixed(5);
  return s.includes(".") ? s.replace(/0+$/, "").replace(/\.$/, "") : s;
}

function InlineText({ inl, onFinding }: { inl: readonly Inline[]; onFinding: (id: string) => void }): React.JSX.Element {
  return (
    <>
      {inl.map((p, i) =>
        p.t === "bold" ? (
          <b key={i}>{p.s}</b>
        ) : p.t === "code" ? (
          <code key={i}>{p.s}</code>
        ) : p.t === "finding" ? (
          <button key={i} type="button" className="agent-finding-ref" onClick={() => onFinding(p.id)} title="Show this finding">
            {p.id}
          </button>
        ) : (
          <span key={i}>{p.s}</span>
        ),
      )}
    </>
  );
}

function Markdown({ text, onFinding }: { text: string; onFinding: (id: string) => void }): React.JSX.Element {
  const blocks = useMemo(() => parseMarkdown(text), [text]);
  return (
    <div className="agent-md">
      {blocks.map((b, i) =>
        b.t === "h" ? (
          <h4 key={i}>
            <InlineText inl={b.inl} onFinding={onFinding} />
          </h4>
        ) : b.t === "li" ? (
          <div key={i} className="agent-li">
            <span aria-hidden="true">•</span>
            <span>
              <InlineText inl={b.inl} onFinding={onFinding} />
            </span>
          </div>
        ) : b.t === "quote" ? (
          <div key={i} className="note">
            <InlineText inl={b.inl} onFinding={onFinding} />
          </div>
        ) : (
          <p key={i}>
            <InlineText inl={b.inl} onFinding={onFinding} />
          </p>
        ),
      )}
    </div>
  );
}

function FindingCard({ f, open, onToggle }: { f: Finding; open: boolean; onToggle: () => void }): React.JSX.Element {
  const where = f.location ?? f.where ?? {};
  const parts: string[] = [];
  if (where.nodes?.length) parts.push(`nodes ${where.nodes.join(", ")}`);
  if (where.regions?.length) parts.push(`regions ${where.regions.join(", ")}`);
  if (where.entities?.length) parts.push(`entities ${where.entities.join(", ")}`);
  if (where.distance_bins?.length) parts.push(`${where.distance_bins.join(", ")} m`);
  if (where.window_s) parts.push(`t ${fmtNum(where.window_s[0])}–${fmtNum(where.window_s[1])} s`);
  return (
    <div className={`agent-finding sev-${f.severity}`} id={`finding-${f.id}`} data-testid="agent-finding">
      <button type="button" className="agent-finding-head" onClick={onToggle} aria-expanded={open}>
        <span className={`sev-dot sev-${f.severity}`} aria-label={f.severity} />
        <span className="grow">{plain(f.title)}</span>
        <span className="faint mono">{f.id}</span>
      </button>
      {open ? (
        <div className="agent-finding-body">
          {f.detail ? <p>{f.detail}</p> : null}
          <table className="table agent-evidence">
            <tbody>
              {f.evidence.map((e, i) => (
                <tr key={i}>
                  <td>{e.label}</td>
                  <td className="mono">
                    {fmtNum(e.value)} {e.unit}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {parts.length > 0 ? <div className="dim">Where: {parts.join("; ")}</div> : null}
          {f.reference ? <div className="faint">Judged against: {f.reference}</div> : null}
          {f.links && f.links.length > 0 ? (
            <div className="agent-links">
              {[...new Set(f.links.map((l) => l.chart))].map((chart) => (
                <button key={chart} type="button" className="link-button" onClick={() => openMetricChart(chart)} title="Open this chart in the metrics panel">
                  Chart: {chart}
                </button>
              ))}
            </div>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}

function RunFindings({ run, openIds, toggle }: { run: RunRecord; openIds: ReadonlySet<string>; toggle: (id: string) => void }): React.JSX.Element {
  const list = notable(run.analysis.findings);
  const byLayer = new Map<string, Finding[]>();
  for (const f of list) byLayer.set(f.layer, [...(byLayer.get(f.layer) ?? []), f]);
  return (
    <div className="agent-run">
      <div className="agent-run-head">
        <b>{run.label}</b>
        <span className="faint mono">{run.analysis.run_id}</span>
        <span className="faint">{fmtNum(run.t_s)} s simulated</span>
      </div>
      {list.length === 0 ? <div className="dim">No bottleneck found in the measured layers.</div> : null}
      {[...byLayer.entries()].map(([layer, fs]) => (
        <div key={layer} className="agent-layer">
          <div className="agent-layer-title">{LAYER_TITLES[layer] ?? layer}</div>
          {fs.map((f) => (
            <FindingCard key={f.id} f={f} open={openIds.has(f.id)} onToggle={() => toggle(f.id)} />
          ))}
        </div>
      ))}
      {run.analysis.not_measured.length > 0 ? (
        <div className="faint">Not measured: {run.analysis.not_measured.map((l) => LAYER_TITLES[l] ?? l).join(", ")}.</div>
      ) : null}
    </div>
  );
}

function Report({ report }: { report: AgentReport }): React.JSX.Element {
  const [openIds, setOpen] = useState<ReadonlySet<string>>(() => new Set());
  const toggle = (id: string): void =>
    setOpen((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const copy = (): void => {
    void navigator.clipboard?.writeText(report.markdown);
  };
  // The overview is the reply itself; this is what is behind its "Details".
  return (
    <div className="agent-report" data-testid="agent-report">
      {report.ungrounded.length > 0 ? (
        <div className="note" data-testid="agent-ungrounded">
          The overview states {report.ungrounded.join(", ")}, which no analysis result contains. Trust the findings below over the prose.
        </div>
      ) : null}
      {report.runs.map((r, i) => (
        <RunFindings key={`${r.label}-${i}`} run={r} openIds={openIds} toggle={toggle} />
      ))}
      {report.comparison && report.comparison.deltas.length > 0 ? (
        <div className="agent-compare">
          <div className="agent-layer-title">Comparison</div>
          <table className="table">
            <thead>
              <tr>
                <th>Metric</th>
                <th>A</th>
                <th>B</th>
                <th>Change</th>
              </tr>
            </thead>
            <tbody>
              {report.comparison.deltas.map((d) => (
                <tr key={d.metric}>
                  <td>
                    <button type="button" className="link-button" onClick={() => openMetricChart(d.metric.split(/[.[]/)[0]!)}>
                      {d.metric}
                    </button>
                  </td>
                  <td>{fmtNum(d.a)}</td>
                  <td>{fmtNum(d.b)}</td>
                  <td className={d.change > 0 ? "up" : d.change < 0 ? "down" : ""}>{fmtNum(d.change)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
      <div className="agent-report-foot faint">
        {report.provider}; numbers from the analyst.
        <button type="button" className="link-button" onClick={copy} title="Copy the whole report as Markdown">
          Copy as Markdown
        </button>
      </div>
    </div>
  );
}

/**
 * A finished run as the agent sums it up: the verdict, one sentence, the overview, each layer's
 * state against its thresholds, and the problems with their numbers. Exported for the run-summary
 * overlay, which shows the same summary beside its own statistics.
 */
export function AgentRunSummaryView({ summary, pending, markdown }: { summary: RunSummary; pending?: boolean; markdown?: string }): React.JSX.Element {
  const [openIds, setOpen] = useState<ReadonlySet<string>>(() => new Set());
  const toggle = (id: string): void =>
    setOpen((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  const showFinding = (id: string): void => {
    setOpen((s) => new Set(s).add(id));
    requestAnimationFrame(() => document.getElementById(`finding-${id}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" }));
  };
  return (
    <div className="agent-summary" data-testid="agent-run-summary">
      <div className="agent-summary-head">
        <span className={`agent-verdict verdict-${summary.verdict}`} data-testid="agent-verdict">
          {verdictLabel(summary.verdict)}
        </span>
        <b className="grow">{summary.scenario || summary.run_id}</b>
        <span className="faint mono">
          {clock(summary.t_s)} / {clock(summary.t_end_s)}
        </span>
      </div>
      <Markdown text={summary.overview} onFinding={showFinding} />
      {pending ? <div className="faint agent-working">The agent is writing its overview…</div> : null}
      {summary.overview_note ? <div className="faint">{summary.overview_note}</div> : null}
      <div className="agent-layers" role="list" aria-label="Each layer against its thresholds">
        {summary.layers.map((l) => (
          <div key={l.layer} role="listitem" className={`agent-layer-chip state-${l.state}`} title={l.findings.length > 0 ? l.findings.join(", ") : layerStateLabel(l.state)}>
            <span className="agent-layer-name">{l.title}</span>
            <span className="agent-layer-state">{layerStateLabel(l.state)}</span>
          </div>
        ))}
      </div>
      {summary.problems.length > 0 ? (
        <div className="agent-layer">
          <div className="agent-layer-title">Problems, most severe first</div>
          {summary.problems.map((f) => (
            <FindingCard key={f.id} f={f} open={openIds.has(f.id)} onToggle={() => toggle(f.id)} />
          ))}
        </div>
      ) : null}
      <div className="agent-report-foot faint">
        Overview by {summary.overview_by === "analyst" ? "the deterministic analyst" : summary.overview_by}; every number is the analyst&apos;s, judged against the cited
        thresholds.
        {markdown ? (
          <button type="button" className="link-button" onClick={() => void navigator.clipboard?.writeText(markdown)} title="Copy the summary as Markdown">
            Copy as Markdown
          </button>
        ) : null}
      </div>
    </div>
  );
}

/** One applied setup: its chips on one line, each opening its setting, and Undo. */
function ChipLine({ setup, chips }: { setup: number; chips: readonly Chip[] }): React.JSX.Element {
  const undone = useAgent((s) => s.undone.has(setup));
  const undo = useAgent((s) => s.undo);
  const busy = useAgent((s) => agentBusy(s.turns));
  return (
    <div className={undone ? "agent-chips undone" : "agent-chips"} data-testid="agent-chips" title={chipLine(chips)}>
      {chips.map((c, i) => (
        <button key={`${c.key}-${i}`} type="button" className="agent-chip" onClick={() => revealSetting(keyToPointer(c.key))} title={`${c.key}: open in Settings`}>
          <span className="agent-chip-label">{c.label}</span> → {c.value}
        </button>
      ))}
      {undone ? (
        <span className="faint">Undone</span>
      ) : (
        <button type="button" className="link-button" onClick={() => void undo(setup)} disabled={busy} data-testid="agent-undo" title="Put the settings back as they were">
          Undo
        </button>
      )}
    </div>
  );
}

/** What the agent is doing right now, in one faint line. */
function Working({ view }: { view: JobView }): React.JSX.Element {
  const run = view.runs.at(-1);
  const text =
    run && !run.analysed
      ? `Running ${run.label} · ${clock(run.tS)} / ${clock(run.tEndS)}`
      : (view.plans.at(-1) ?? (view.tools > 0 ? `Working · step ${view.tools}` : "Thinking…"));
  return (
    <div className="agent-working" data-testid="agent-working">
      <span className="agent-live-dot on" aria-hidden="true" />
      <span className="agent-ellipsis">{plain(text)}</span>
    </div>
  );
}

function TurnView({ turn }: { turn: Turn }): React.JSX.Element {
  const view = useMemo(() => foldEvents(turn.events), [turn.events]);
  const report = view.report;
  const hasDetails = report !== null && (report.runs.length > 0 || (report.comparison?.deltas.length ?? 0) > 0 || report.ungrounded.length > 0);
  const failed = [...view.errors, ...(turn.error ? [turn.error] : [])];
  return (
    <>
      <div className="agent-msg user">{turn.prompt}</div>
      <div className="agent-msg bot" data-testid="agent-reply">
        {view.staged.map((s) => (
          <ChipLine key={s.setup} setup={s.setup} chips={s.chips} />
        ))}
        {turn.running && report === null ? <Working view={view} /> : null}
        {report ? <Markdown text={report.overview} onFinding={() => undefined} /> : null}
        {failed.map((e, i) => (
          <div key={i} className="agent-error" title={e}>
            {e}
          </div>
        ))}
        {hasDetails && report ? (
          <details className="agent-details">
            <summary>Details</summary>
            <Report report={report} />
          </details>
        ) : null}
      </div>
    </>
  );
}

/** A watcher alert: one line; the full sentence, its numbers and (at the run's end) the summary behind a click. */
function AlertCard({ o }: { o: Observation }): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const summary = useAgentWatch((s) => s.summary);
  const pending = useAgentWatch((s) => s.overviewPending);
  const markdown = useAgentWatch((s) => s.summaryMarkdown);
  const ended = o.kind === "finished" && summary !== null && summary.run_id === o.run_id && summary.generation === o.generation ? summary : null;
  const line = ended ? `Run finished · ${verdictLabel(ended.verdict)}: ${plain(ended.headline)}` : alertLine(o.text);
  return (
    <div className={`agent-alert sev-${o.severity}`} data-testid="agent-alert">
      <button type="button" className="agent-alert-head" onClick={() => setOpen(!open)} aria-expanded={open} title={plain(o.text)}>
        <span className={`sev-dot sev-${o.severity}`} aria-label={o.severity} />
        <span className="mono faint">{clock(o.t_s)}</span>
        <span className="agent-ellipsis">{line}</span>
      </button>
      {open ? (
        <div className="agent-alert-body">
          {ended ? (
            <AgentRunSummaryView summary={ended} pending={pending} markdown={markdown} />
          ) : (
            <>
              <div>{plain(o.text)}</div>
              {o.evidence.map((e, i) => (
                <div key={i} className="faint mono">
                  {e.label}: {fmtNum(e.value)} {e.unit === "ratio" || e.unit === "count" ? "" : e.unit}
                </div>
              ))}
              {o.finding ? <div className="faint mono">{o.finding}</div> : null}
            </>
          )}
        </div>
      ) : null}
    </div>
  );
}

interface Item {
  readonly at: number;
  readonly key: string;
  readonly turn?: Turn;
  readonly alert?: Observation;
}

export function AgentPanel(): React.JSX.Element {
  const status = useAgent((s) => s.status);
  const statusError = useAgent((s) => s.statusError);
  const turns = useAgent((s) => s.turns);
  const draft = useAgent((s) => s.draft);
  const undoError = useAgent((s) => s.undoError);
  const { setDraft, ask, stop, reset, refreshStatus } = useAgent.getState();
  const page = useAgentWatch((s) => s.page);
  const all = useAgentWatch((s) => s.observations);
  const arrived = useAgentWatch((s) => s.arrived);
  const busy = agentBusy(turns);
  const scroller = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  // The run on screen's alerts, and any alert that arrived while this page was open, placed
  // among the conversation's turns by when they arrived.
  const items = useMemo<Item[]>(() => {
    const current = page ? observationsOf(all, page.run_id, page.generation) : [];
    const shown = new Map<number, Observation>();
    for (const o of current) if (isAlert(o)) shown.set(o.seq, o);
    for (const o of all) if (isAlert(o) && (arrived.get(o.seq) ?? 0) > 0) shown.set(o.seq, o);
    const out: Item[] = [
      ...turns.map((t) => ({ at: t.at, key: `t${String(t.id).padStart(6, "0")}`, turn: t })),
      ...[...shown.values()].map((o) => ({ at: arrived.get(o.seq) ?? 0, key: `o${String(o.seq).padStart(6, "0")}`, alert: o })),
    ];
    return out.sort((a, b) => a.at - b.at || a.key.localeCompare(b.key));
  }, [turns, all, arrived, page]);

  // Keep the newest message in view.
  const lastEvents = turns.at(-1)?.events.length ?? 0;
  useEffect(() => {
    const el = scroller.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lastEvents, items.length]);

  const model = status?.model === true;
  const send = (): void => {
    if (!busy && model && draft.trim() !== "") void ask(draft);
  };
  const watching = page !== null && (page.phase === "watching" || page.phase === "analysing");

  return (
    <>
      <div className="panel-body agent-panel" data-testid="agent-panel" ref={scroller}>
        {statusError ? (
          <div className="agent-error" data-testid="agent-unavailable" title={statusError}>
            {statusError}
          </div>
        ) : status && !model ? (
          <div className="agent-hint" data-testid="agent-no-key" title={status.how_to_add_key ?? ""}>
            No model key: alerts still work. Add one to .env and restart the engine.
          </div>
        ) : null}
        {items.length === 0 ? (
          <div className="agent-empty">
            <div className="dim">Tell me what to set up, or ask about the run.</div>
            {model ? (
              <div className="agent-suggestions">
                {SUGGESTIONS.map((s) => (
                  <button key={s} type="button" className="agent-suggestion" onClick={() => void ask(s)} disabled={busy}>
                    {s}
                  </button>
                ))}
              </div>
            ) : null}
          </div>
        ) : (
          items.map((it) => (it.turn ? <TurnView key={it.key} turn={it.turn} /> : it.alert ? <AlertCard key={it.key} o={it.alert} /> : null))
        )}
        {undoError ? <div className="agent-error">{undoError}</div> : null}
      </div>
      <div className="panel-foot agent-foot">
        {watching ? <span className="agent-live-dot on" title={`Watching the run, ${clock(page.t_s)} / ${clock(page.t_end_s)}`} /> : null}
        <textarea
          rows={1}
          value={draft}
          placeholder={model ? "Set up, run or ask anything…" : "Add a model key to chat"}
          aria-label="Ask the agent"
          data-testid="agent-input"
          disabled={!model}
          onChange={(e) => setDraft(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault();
              send();
            }
          }}
        />
        {busy ? (
          <button type="button" onClick={() => void stop()} data-testid="agent-stop" title="Stop the agent; a run it started is paused">
            Stop
          </button>
        ) : (
          <button type="button" className="primary" onClick={send} disabled={!model || draft.trim() === ""} data-testid="agent-send">
            Send
          </button>
        )}
        {turns.length > 0 && !busy ? (
          <button type="button" className="agent-clear" onClick={() => void reset()} title="New conversation" aria-label="New conversation">
            ↺
          </button>
        ) : null}
      </div>
    </>
  );
}

