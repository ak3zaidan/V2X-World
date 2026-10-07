/**
 * The always-on agent's mark on the header's Agent button: it starts the page's reading of the
 * agent's feed (once, for the life of the page), shows that the agent is watching a run, counts the
 * warnings it posted since the agent panel was last open, and — while the panel is closed — shows
 * each new warning, and the end-of-run summary, as a short note under the header for a few seconds.
 */

import { useEffect, useState } from "react";

import { unseenCount, type Observation } from "../lib/agent.js";
import { openPanel } from "../shell/route.js";
import { useAgentWatch } from "../state/agent.js";
import { useStudio } from "../state/store.js";
import "../styles/agent.css";

/** How long a note stays up, milliseconds. Long enough to read one sentence twice. */
const NOTE_MS = 8000;

/** The last observation a note was considered for, across mounts. */
let lastNoted = 0;

function worthANote(o: Observation): boolean {
  return o.kind === "finished" || o.kind === "failed" || o.severity === "warning" || o.severity === "critical";
}

export function AgentBadge(): React.JSX.Element | null {
  const start = useAgentWatch((s) => s.start);
  const available = useAgentWatch((s) => s.available);
  const phase = useAgentWatch((s) => s.page?.phase ?? "idle");
  const unseen = useAgentWatch((s) => unseenCount(s.observations, s.seenSeq));
  const latest = useAgentWatch((s) => s.observations.at(-1) ?? null);
  const count = useAgentWatch((s) => s.observations.length);
  const markSeen = useAgentWatch((s) => s.markSeen);
  const panelOpen = useStudio((s) => s.panel === "agent");
  const [note, setNote] = useState<Observation | null>(null);
  const bootSeq = useAgentWatch((s) => s.bootSeq);

  useEffect(() => start(), [start]);
  // Everything is seen while the panel is open.
  useEffect(() => {
    if (panelOpen) markSeen();
  }, [panelOpen, count, markSeen]);

  // A note for each new warning (or the run's end) while the panel is closed. What the first
  // read after a reload found is old news (`bootSeq`), and the mark lives outside the component
  // so a remounted header does not show a note twice or swallow one.
  useEffect(() => {
    if (latest === null || bootSeq === null) return;
    if (latest.seq <= Math.max(bootSeq, lastNoted)) return;
    lastNoted = latest.seq;
    if (panelOpen || !worthANote(latest)) return;
    setNote(latest);
  }, [latest, bootSeq, panelOpen]);
  useEffect(() => {
    if (note === null) return;
    const t = setTimeout(() => setNote(null), NOTE_MS);
    return () => clearTimeout(t);
  }, [note]);

  if (available !== true) return null;
  const watching = phase === "watching" || phase === "analysing";
  return (
    <span className="agent-badge" data-testid="agent-badge" title={latest?.text ?? "The agent watches every run and sums it up at the end"}>
      <span className={watching ? "agent-live-dot on" : "agent-live-dot"} aria-label={watching ? "the agent is watching the run" : "the agent is idle"} />
      {unseen > 0 && !panelOpen ? <span className="agent-badge-count">{unseen}</span> : null}
      {note !== null && !panelOpen ? (
        <span
          role="status"
          className={`agent-note sev-${note.severity}`}
          data-testid="agent-note"
          onClick={(e) => {
            e.stopPropagation();
            setNote(null);
            openPanel("agent");
          }}
        >
          <b>Agent:</b> {note.text}
        </span>
      ) : null}
    </span>
  );
}
