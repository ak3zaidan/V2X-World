/**
 * The Backend view: every entity of the credential system the run is using — the US SCMS or the
 * European CCMS — as a diagram with live counts, and the traffic between them and the vehicles.
 *
 * It is one header button and one full-screen panel. While the panel is open it asks the engine
 * `inspect.entity {entity: "backend"}` once a second; the engine answers from the newest
 * `backend.state` snapshot at or before the clock (`v2xw_proto::view::BackendView`). The panel
 * computes nothing: the boxes are the engine's entities, the lines are the pairs that have
 * exchanged a message, the numbers are the engine's counters. Clicking a box shows everything the
 * engine publishes about that entity; clicking a line shows its protocol steps.
 *
 * Self-contained on purpose — its own file, its own stylesheet, one line in `App.tsx` — so the
 * page's layout can change around it.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import {
  columnHeads,
  edgePath,
  edgeWidth,
  formatBytes,
  formatT,
  formatValue,
  headlineCounts,
  isLive,
  label,
  layout,
  snapshotOf,
  systemTitle,
  type BackendEdge,
  type BackendEntity,
  type BackendSnapshot,
} from "../lib/backend.js";
import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import "../styles/backend.css";

/** How often the open panel asks for a new snapshot, in wall-clock milliseconds. */
const POLL_MS = 1000;

type Selection = { readonly kind: "entity"; readonly id: string } | { readonly kind: "edge"; readonly from: string; readonly to: string } | null;

export function BackendView(): React.JSX.Element {
  const [open, setOpen] = useState(false);
  return (
    <>
      <button
        type="button"
        onClick={() => setOpen(true)}
        data-testid="backend-button"
        title="The credential system's authorities and roadside units, with the traffic between them and the vehicles"
      >
        Backend
      </button>
      {open ? <BackendPanel onClose={() => setOpen(false)} /> : null}
    </>
  );
}

function BackendPanel({ onClose }: { readonly onClose: () => void }): React.JSX.Element {
  const [snapshot, setSnapshot] = useState<BackendSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selection, setSelection] = useState<Selection>(null);
  const connection = useStudio((s) => s.connection);

  const poll = useCallback(async (): Promise<void> => {
    try {
      const answer = await engine.request("inspect.entity", { entity: "backend", limit: 40 }, { quiet: true, timeoutMs: 4000 });
      const snap = snapshotOf(answer);
      setSnapshot(snap);
      setError(snap === null ? "The engine answered, but not with a backend snapshot." : null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    void poll();
    const id = window.setInterval(() => void poll(), POLL_MS);
    return () => window.clearInterval(id);
  }, [poll, connection]);

  useEffect(() => {
    const onKey = (ev: KeyboardEvent): void => {
      if (ev.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="backend-overlay" role="dialog" aria-modal="true" aria-label="Backend" data-testid="backend-panel">
      <div className="backend-head">
        <h2>{snapshot ? systemTitle(snapshot.system) : "Backend"}</h2>
        {snapshot ? (
          <span className="backend-sub" data-testid="backend-time">
            {snapshot.protocol} · {snapshot.signature} · state at {formatT(snapshot.t)} · {snapshot.entities.length} entities ·{" "}
            {snapshot.edges.length} links in use
          </span>
        ) : null}
        <span className="spacer grow" />
        <button type="button" onClick={onClose} data-testid="backend-close" title="Close (Esc)">
          Close
        </button>
      </div>
      {snapshot === null ? (
        <div className="backend-empty" data-testid="backend-empty">
          <p>No backend to show.</p>
          <p className="dim">
            A run publishes its credential system once a simulated second when the scenario sets{" "}
            <code>security.protocol</code> to <code>protocol/scms/camp</code> (the US SCMS) or{" "}
            <code>protocol/etsi/ts102941</code> (the European CCMS), and the clock has run past its first second.
          </p>
          {error ? <p className="faint">The engine said: {error}</p> : null}
        </div>
      ) : (
        <div className="backend-body">
          <Diagram snapshot={snapshot} selection={selection} onSelect={setSelection} />
          <Details snapshot={snapshot} selection={selection} />
        </div>
      )}
    </div>
  );
}

function Diagram({
  snapshot,
  selection,
  onSelect,
}: {
  readonly snapshot: BackendSnapshot;
  readonly selection: Selection;
  readonly onSelect: (s: Selection) => void;
}): React.JSX.Element {
  const placed = useMemo(() => layout(snapshot.entities), [snapshot.entities]);
  const heads = useMemo(() => columnHeads(snapshot.entities), [snapshot.entities]);
  const byId = useMemo(() => new Map(placed.boxes.map((b) => [b.id, b])), [placed]);
  const entities = useMemo(() => new Map(snapshot.entities.map((e) => [e.id, e])), [snapshot.entities]);
  const pad = 12;
  const selectedId = selection?.kind === "entity" ? selection.id : null;
  return (
    <div className="backend-diagram">
      <svg
        width={placed.width + pad * 2 + 60}
        height={placed.height + pad * 2}
        viewBox={`${-pad} ${-pad} ${placed.width + pad * 2 + 60} ${placed.height + pad * 2}`}
        data-testid="backend-diagram"
      >
        <defs>
          <marker id="backend-arrow" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
            <path d="M 0 0 L 8 4 L 0 8 z" className="backend-arrowhead" />
          </marker>
        </defs>
        {heads.map((h) => (
          <text key={h.label} x={h.x} y={14} className="backend-colhead">
            {h.label}
          </text>
        ))}
        {snapshot.edges.map((e) => {
          const a = byId.get(e.from);
          const b = byId.get(e.to);
          if (!a || !b || a === b) return null;
          const reverse = snapshot.edges.some((o) => o.from === e.to && o.to === e.from) && e.from > e.to;
          const on = selection?.kind === "edge" && selection.from === e.from && selection.to === e.to;
          const touches = selectedId !== null && (e.from === selectedId || e.to === selectedId);
          const cls = ["backend-edge", isLive(e, snapshot.t) ? "live" : "", on || touches ? "on" : "", e.transport].join(" ");
          return (
            <path
              key={`${e.from}>${e.to}`}
              d={edgePath(a, b, reverse)}
              className={cls}
              strokeWidth={edgeWidth(e.messages)}
              markerEnd="url(#backend-arrow)"
              onClick={() => onSelect({ kind: "edge", from: e.from, to: e.to })}
              data-testid={`backend-edge-${e.from}-${e.to}`}
            >
              <title>
                {entities.get(e.from)?.name ?? e.from} → {entities.get(e.to)?.name ?? e.to}: {e.messages.toLocaleString("en-US")} messages,{" "}
                {formatBytes(e.bytes)}, last {e.last_step} ({e.transport})
              </title>
            </path>
          );
        })}
        {placed.boxes.map((p) => {
          const e = entities.get(p.id);
          if (!e) return null;
          const counts = headlineCounts(e);
          const depth = e.queue?.depth ?? 0;
          return (
            <g
              key={p.id}
              transform={`translate(${p.x} ${p.y})`}
              className={["backend-box", selectedId === p.id ? "on" : "", e.online ? "" : "offline"].join(" ")}
              onClick={() => onSelect({ kind: "entity", id: p.id })}
              data-testid={`backend-entity-${p.id}`}
            >
              <title>{e.role}</title>
              <rect width={p.w} height={p.h} rx={6} />
              <text x={8} y={17} className="backend-name">
                {e.name.length > 24 ? `${e.name.slice(0, 23)}…` : e.name}
              </text>
              {counts.map(([k, v], i) => (
                <text key={k} x={8} y={36 + i * 16} className="backend-count">
                  <tspan className="backend-count-v">{v}</tspan> {k.length > 20 ? `${k.slice(0, 19)}…` : k}
                </text>
              ))}
              {depth > 0 ? (
                <g transform={`translate(${p.w - 8} 10)`}>
                  <rect x={-30} y={-7} width={30} height={15} rx={7} className="backend-queue" />
                  <text x={-15} y={4} className="backend-queue-t" textAnchor="middle">
                    {depth}
                  </text>
                  <title>{depth} requests waiting in this entity's queue</title>
                </g>
              ) : null}
            </g>
          );
        })}
      </svg>
      <p className="backend-legend dim">
        Lines are the pairs that have exchanged a message since the run began; thicker is more, bright is within the last two
        simulated seconds, dashed is over the air. A number in a pill is a queue. Click a box or a line for everything the
        engine reports about it.
      </p>
    </div>
  );
}

function Details({ snapshot, selection }: { readonly snapshot: BackendSnapshot; readonly selection: Selection }): React.JSX.Element {
  if (selection?.kind === "entity") {
    const e = snapshot.entities.find((x) => x.id === selection.id);
    if (e) return <EntityDetails entity={e} snapshot={snapshot} />;
  }
  if (selection?.kind === "edge") {
    const edge = snapshot.edges.find((x) => x.from === selection.from && x.to === selection.to);
    if (edge) return <EdgeDetails edge={edge} snapshot={snapshot} />;
  }
  return (
    <aside className="backend-side" data-testid="backend-side">
      <h3>Protocol flows</h3>
      <dl className="kv">
        {Object.entries(snapshot.flows).map(([k, v]) => (
          <Fragmentless key={k} k={label(k)} v={formatValue(v)} />
        ))}
      </dl>
      <Recent snapshot={snapshot} filter={null} />
    </aside>
  );
}

function EntityDetails({ entity: e, snapshot }: { readonly entity: BackendEntity; readonly snapshot: BackendSnapshot }): React.JSX.Element {
  const edgesIn = snapshot.edges.filter((x) => x.to === e.id);
  const edgesOut = snapshot.edges.filter((x) => x.from === e.id);
  const name = (id: string): string => snapshot.entities.find((x) => x.id === id)?.name ?? id;
  return (
    <aside className="backend-side" data-testid="backend-side">
      <h3>{e.name}</h3>
      <p className="dim">{e.role}</p>
      {!e.online ? <p className="warn">Offline: kept air-gapped, as the design has it.</p> : null}
      <h4>State</h4>
      <dl className="kv" data-testid="backend-entity-state">
        {Object.entries(e.state).map(([k, v]) => (
          <Fragmentless key={k} k={label(k)} v={formatValue(v)} />
        ))}
      </dl>
      <h4>Traffic</h4>
      <dl className="kv">
        <Fragmentless k="received" v={`${formatValue(e.traffic.received)} (${formatBytes(e.traffic.bytes_in)})`} />
        <Fragmentless k="sent" v={`${formatValue(e.traffic.sent)} (${formatBytes(e.traffic.bytes_out)})`} />
      </dl>
      {e.queue ? (
        <>
          <h4>Queue</h4>
          <dl className="kv">
            <Fragmentless k="waiting" v={formatValue(e.queue.depth)} />
            <Fragmentless k="servers" v={formatValue(e.queue.servers)} />
            <Fragmentless k="served" v={formatValue(e.queue.served)} />
            <Fragmentless k="mean wait" v={e.queue.served > 0 ? `${(e.queue.waited_ns / e.queue.served / 1e6).toFixed(2)} ms` : "–"} />
            <Fragmentless k="busy" v={`${(e.queue.busy_ns / 1e9).toFixed(2)} s`} />
          </dl>
        </>
      ) : null}
      {Object.keys(e.ops).length > 0 ? (
        <>
          <h4>Cryptographic operations</h4>
          <dl className="kv">
            {Object.entries(e.ops).map(([k, v]) => (
              <Fragmentless key={k} k={k} v={formatValue(v)} />
            ))}
          </dl>
        </>
      ) : null}
      <h4>Links</h4>
      <ul className="backend-links">
        {edgesIn.map((x) => (
          <li key={`in-${x.from}`}>
            from {name(x.from)}: {formatValue(x.messages)} ({x.transport})
          </li>
        ))}
        {edgesOut.map((x) => (
          <li key={`out-${x.to}`}>
            to {name(x.to)}: {formatValue(x.messages)} ({x.transport})
          </li>
        ))}
        {edgesIn.length + edgesOut.length === 0 ? <li className="faint">none yet</li> : null}
      </ul>
      <Recent snapshot={snapshot} filter={e.id} />
    </aside>
  );
}

function EdgeDetails({ edge, snapshot }: { readonly edge: BackendEdge; readonly snapshot: BackendSnapshot }): React.JSX.Element {
  const name = (id: string): string => snapshot.entities.find((x) => x.id === id)?.name ?? id;
  return (
    <aside className="backend-side" data-testid="backend-side">
      <h3>
        {name(edge.from)} → {name(edge.to)}
      </h3>
      <dl className="kv">
        <Fragmentless k="messages" v={formatValue(edge.messages)} />
        <Fragmentless k="bytes" v={formatBytes(edge.bytes)} />
        <Fragmentless k="carried over" v={edge.transport} />
        <Fragmentless k="last" v={`${edge.last_step} at ${formatT(edge.last_t)}`} />
      </dl>
      <h4>Protocol steps</h4>
      <dl className="kv">
        {Object.entries(edge.steps).map(([k, v]) => (
          <Fragmentless key={k} k={k} v={formatValue(v)} />
        ))}
      </dl>
    </aside>
  );
}

function Recent({ snapshot, filter }: { readonly snapshot: BackendSnapshot; readonly filter: string | null }): React.JSX.Element | null {
  const rows = snapshot.recent.filter((r) => filter === null || r.from === filter || r.to === filter).slice(-15).reverse();
  if (rows.length === 0) return null;
  return (
    <>
      <h4>Latest messages</h4>
      <ol className="backend-recent">
        {rows.map((r, i) => (
          <li key={`${r.t}-${i}`}>
            <span className="faint">{formatT(r.t)}</span> {r.from} → {r.to} <span className="dim">{r.step}</span>{" "}
            <span className="faint">{formatBytes(r.bytes)}</span>
          </li>
        ))}
      </ol>
    </>
  );
}

/** One key and value of a `<dl>`. */
function Fragmentless({ k, v }: { readonly k: string; readonly v: string }): React.JSX.Element {
  return (
    <>
      <dt>{k}</dt>
      <dd>{v}</dd>
    </>
  );
}
