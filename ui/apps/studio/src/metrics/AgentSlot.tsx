/**
 * The run summary's slot for the agent's analysis.
 *
 * The agent harness (`state/agent.ts`, served by `v2xw serve` beside `/rpc`) runs the deterministic
 * run analyst (`crates/v2xw-copilot/src/analyst.rs`) and, with a model attached, writes an overview
 * grounded in its findings. This slot shows the newest of those for the run on screen: the model's
 * overview when there is one, and the analyst's findings, most severe first. When nothing has been
 * analysed yet it offers the button that asks; when the engine has no agent it says so.
 *
 * This is the one place the summary reads the agent, so the agent track can change what it feeds
 * in here without touching the rest of the summary.
 */

import { useEffect } from "react";

import type { AgentEvent, Finding, Severity } from "../lib/agent.js";
import { agentBusy, useAgent } from "../state/agent.js";

const SEVERITY_RANK: Readonly<Record<Severity, number>> = { critical: 3, warning: 2, notice: 1, info: 0 };
const SEVERITY_LEVEL: Readonly<Record<Severity, string>> = { critical: "unhealthy", warning: "caution", notice: "caution", info: "healthy" };

/** The newest analysis among the agent's turns: its findings and, if a model wrote one, its overview. */
export function newestAnalysis(turns: readonly { readonly events: readonly AgentEvent[] }[]): { findings: readonly Finding[]; overview: string | null } | null {
  for (let i = turns.length - 1; i >= 0; i--) {
    const events = turns[i]!.events;
    let findings: readonly Finding[] | null = null;
    let overview: string | null = null;
    for (const e of events) {
      if (e.kind === "analysed") findings = e.analysis.findings;
      if (e.kind === "report") {
        overview = e.report.overview.trim() === "" ? null : e.report.overview;
        const last = e.report.runs[e.report.runs.length - 1];
        if (last) findings = last.analysis.findings;
      }
    }
    if (findings !== null || overview !== null) return { findings: findings ?? [], overview };
  }
  return null;
}

export function AgentSlot(): React.JSX.Element {
  const turns = useAgent((s) => s.turns);
  const status = useAgent((s) => s.status);
  const statusError = useAgent((s) => s.statusError);
  const busy = agentBusy(turns);
  useEffect(() => {
    if (status === null && statusError === null) void useAgent.getState().refreshStatus();
  }, [status, statusError]);
  const analysis = newestAnalysis(turns);
  const findings = analysis ? [...analysis.findings].sort((a, b) => SEVERITY_RANK[b.severity] - SEVERITY_RANK[a.severity] || a.id.localeCompare(b.id)) : [];

  return (
    <section className="rsum-section" data-testid="run-summary-agent">
      <h3>Agent analysis</h3>
      {statusError !== null && analysis === null ? (
        <p className="dim">{statusError}</p>
      ) : analysis === null ? (
        <p className="dim">
          {busy ? "The agent is analysing the run…" : "The agent has not analysed this run yet."}{" "}
          {!busy ? (
            <button type="button" className="small" onClick={() => void useAgent.getState().analyseCurrent()} data-testid="run-summary-analyse">
              Analyse this run
            </button>
          ) : null}
        </p>
      ) : (
        <>
          {analysis.overview !== null ? <p className="rsum-overview">{analysis.overview}</p> : null}
          {findings.length === 0 ? (
            <p className="dim">The run analyst found nothing to report.</p>
          ) : (
            <ul className="rsum-findings">
              {findings.slice(0, 12).map((f) => (
                <li key={f.id} className={SEVERITY_LEVEL[f.severity]}>
                  <span className={`hdot ${SEVERITY_LEVEL[f.severity]}`} aria-hidden="true" />
                  <span className="rsum-sev">{f.severity}</span> <strong>{f.title}</strong>
                  {f.detail ? <span className="dim"> — {f.detail}</span> : null}
                </li>
              ))}
            </ul>
          )}
          {findings.length > 12 ? <p className="dim">and {findings.length - 12} more in the Agent panel.</p> : null}
        </>
      )}
    </section>
  );
}
