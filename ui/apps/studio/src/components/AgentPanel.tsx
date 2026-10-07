/**
 * The agent panel: say what to simulate and what to find out; the agent picks a shipped scenario,
 * changes only what the request implies, validates it with the engine's loader, runs it, watches
 * it, and reports — bottlenecks and inefficiencies in every layer, with the analyst's numbers.
 *
 * What it shows, per request: the agent's plan, the settings it changed (each one opens the
 * settings window at that setting), the run's progress, and the report — the model's overview,
 * whose [finding ids] open the finding, and the findings by layer, each with its numbers, where,
 * the threshold it was judged against and a link to its chart in the metrics panel.
 *
 * With no model key the panel says how to add one, and still offers what needs no model: analyse
 * the run on screen, or run a scenario as shipped and analyse it.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import {
  SUGGESTIONS,
  cellText,
  foldEvents,
  keyToPointer,
  notable,
  parseMarkdown,
  progressFraction,
  type AgentReport,
  type Finding,
  type Inline,
  type JobView,
  type RunRecord,
} from "../lib/agent.js";
import { openMetricChart, revealSetting } from "../lib/links.js";
import { agentBusy, useAgent, type Turn } from "../state/agent.js";
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
        <span className="grow">{f.title}</span>
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
  const showFinding = (id: string): void => {
    setOpen((s) => new Set(s).add(id));
    requestAnimationFrame(() => document.getElementById(`finding-${id}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" }));
  };
  const copy = (): void => {
    void navigator.clipboard?.writeText(report.markdown);
  };
  return (
    <div className="agent-report" data-testid="agent-report">
      <Markdown text={report.overview} onFinding={showFinding} />
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
        Overview by {report.provider}; every number in the findings is the analyst&apos;s.
        <button type="button" className="link-button" onClick={copy} title="Copy the whole report as Markdown">
          Copy as Markdown
        </button>
      </div>
    </div>
  );
}

function JobBody({ view, turn }: { view: JobView; turn: Turn }): React.JSX.Element {
  return (
    <div className="agent-job">
      {view.plans.map((p, i) => (
        <div key={`p${i}`} className="agent-plan">
          {p}
        </div>
      ))}
      {view.changes.map((c, i) => (
        <div key={`c${i}`} className="agent-changes" data-testid="agent-changes">
          <div className="dim">
            From <code>{c.base}</code>
            {c.reason ? ` — ${c.reason}` : ""}
            {c.valid ? null : <span className="err"> (refused by the loader)</span>}
          </div>
          {c.changes.length > 0 ? (
            <table className="table">
              <tbody>
                {c.changes.map((ch) => (
                  <tr key={ch.key} title={ch.why}>
                    <td>
                      <button type="button" className="link-button mono" onClick={() => revealSetting(keyToPointer(ch.key))} title="Open this setting in the settings window">
                        {ch.key}
                      </button>
                    </td>
                    <td className="faint">{cellText(ch.before)}</td>
                    <td>→ {cellText(ch.after)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : null}
          {c.errors.map((e, j) => (
            <div key={j} className="note err">
              {e.field ? <code>{e.field}</code> : null} {e.message}
            </div>
          ))}
        </div>
      ))}
      {view.runs.map((r, i) => (
        <div key={`r${i}`} className="agent-progress" data-testid="agent-progress">
          <div className="agent-progress-line">
            <span>{r.label}</span>
            <span className="grow" />
            <span className="mono faint">
              {fmtNum(r.tS)} / {fmtNum(r.tEndS)} s · {r.actors} actors{r.analysed ? " · analysed" : ""}
            </span>
          </div>
          <div className="agent-bar" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(progressFraction(r.tS, r.tEndS) * 100)}>
            <div style={{ width: `${progressFraction(r.tS, r.tEndS) * 100}%` }} />
          </div>
        </div>
      ))}
      {turn.running && view.report === null ? (
        <div className="faint agent-working">{view.statuses.at(-1) ?? (view.tools > 0 ? `Working… ${view.tools} step${view.tools === 1 ? "" : "s"} so far` : "Working…")}</div>
      ) : null}
      {view.report ? <Report report={view.report} /> : null}
      {view.errors.map((e, i) => (
        <div key={`e${i}`} className="note err">
          {e}
        </div>
      ))}
      {turn.error ? <div className="note err">{turn.error}</div> : null}
    </div>
  );
}

function TurnView({ turn }: { turn: Turn }): React.JSX.Element {
  const view = useMemo(() => foldEvents(turn.events), [turn.events]);
  return (
    <div className="agent-turn">
      <div className="agent-prompt">{turn.prompt}</div>
      <JobBody view={view} turn={turn} />
    </div>
  );
}

export function AgentPanel(): React.JSX.Element {
  const status = useAgent((s) => s.status);
  const statusError = useAgent((s) => s.statusError);
  const turns = useAgent((s) => s.turns);
  const draft = useAgent((s) => s.draft);
  const { setDraft, ask, stop, reset, refreshStatus, analyseCurrent, runScenario } = useAgent.getState();
  const busy = agentBusy(turns);
  const scroller = useRef<HTMLDivElement | null>(null);
  const input = useRef<HTMLTextAreaElement | null>(null);

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  // Keep the newest activity in view while a job runs.
  const lastEvents = turns.at(-1)?.events.length ?? 0;
  useEffect(() => {
    const el = scroller.current;
    if (el && busy) el.scrollTop = el.scrollHeight;
  }, [lastEvents, busy, turns.length]);

  const model = status?.model === true;
  const send = (): void => {
    if (!busy && model && draft.trim() !== "") void ask(draft);
  };

  return (
    <>
      <div className="panel-body agent-panel" data-testid="agent-panel" ref={scroller}>
        {statusError ? (
          <div className="note err" data-testid="agent-unavailable">
            {statusError}
          </div>
        ) : status && !model ? (
          <div className="note info" data-testid="agent-no-key">
            <strong>No model connected.</strong> {status.how_to_add_key}
          </div>
        ) : null}
        {turns.length === 0 ? (
          <div className="agent-empty">
            <p className="dim">
              Say what to simulate and what to find out. The agent picks a shipped scenario, changes only what you ask for, checks it with the engine&apos;s
              own loader, runs it and reports the bottlenecks in every layer — traffic, channel, queues, delivery, latency, security and privacy — with the
              numbers behind each.
            </p>
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
          turns.map((t) => <TurnView key={t.id} turn={t} />)
        )}
        {status !== null ? (
          <div className="agent-quick">
            <button type="button" onClick={() => void analyseCurrent()} disabled={busy} title="Analyse the run on screen with the deterministic analyst (no model needed)" data-testid="agent-analyse">
              Analyse this run
            </button>
            {!model ? (
              <button type="button" onClick={() => void runScenario("current")} disabled={busy} title="Run the current scenario to its end and analyse it (no model needed)">
                Run and analyse
              </button>
            ) : null}
            {turns.length > 0 ? (
              <button type="button" onClick={() => void reset()} disabled={busy} title="Start a new conversation">
                New conversation
              </button>
            ) : null}
          </div>
        ) : null}
      </div>
      <div className="panel-foot agent-foot">
        <textarea
          ref={input}
          rows={2}
          value={draft}
          placeholder={model ? "e.g. Run Manhattan at rush hour and tell me where the network struggles" : "Add a model key to ask in plain words"}
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
      </div>
    </>
  );
}
