/**
 * The Backend view: every entity of the credential system the run is using — the US SCMS or the
 * European CCMS — as a diagram with live counts, and the traffic between them and the vehicles.
 *
 * A full-screen shell panel (`shell/panels.tsx`), like Metrics: it opens under the header, which
 * stays, so the run's state, its clock and Pause are never hidden behind it. (It used to be an
 * overlay of its own at `inset: 0`, over the header.) While it is open it asks the engine
 * `inspect.entity {entity: "backend"}` once a second; the engine answers from the newest
 * `backend.state` snapshot at or before the clock (`v2xw_proto::view::BackendView`). The panel
 * computes nothing: the boxes are the engine's entities, the lines are the pairs that have
 * exchanged a message, the numbers are the engine's counters.
 *
 * # Reading it at a glance
 *
 * A strip across the top says what the whole system is doing — how many entities and links are
 * active in the last two simulated seconds, how many messages and bytes it has moved, how many
 * requests are waiting, and the protocol flows by count. Each box carries a dot that lights while
 * the entity has traffic and a pill with its queue depth. Clicking a box or a line opens everything
 * the engine publishes about it on the right, with a link to the metrics that measure it.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import {
  activeEntities,
  columnHeads,
  edgePath,
  edgeWidth,
  formatBytes,
  formatT,
  formatValue,
  glance,
  headlineCounts,
  isLive,
  label,
  layout,
  metricsFor,
  snapshotOf,
  systemTitle,
  type BackendEdge,
  type BackendEntity,
  type BackendSnapshot,
} from "../lib/backend.js";
import { engine } from "../state/engine.js";
import { CloseIcon, NetworkIcon } from "../shell/Icons.js";
import { openPanel, togglePanel } from "../shell/route.js";
import { useStudio } from "../state/store.js";
import "../styles/backend.css";

/** How often the open panel asks for a new snapshot, in wall-clock milliseconds. */
const POLL_MS = 1000;

type Selection = { readonly kind: "entity"; readonly id: string } | { readonly kind: "edge"; readonly from: string; readonly to: string } | null;

/** The header's door to the panel. */
export function BackendButton(): React.JSX.Element {
  const open = useStudio((s) => s.panel === "backend");
  return (
    <button
      type="button"
      className={open ? "icon-button labelled on" : "icon-button labelled"}
      aria-pressed={open}
      aria-label="Backend"
      onClick={() => togglePanel("backend")}
      data-testid="backend-button"
      title="The credential system's authorities and roadside units, with the traffic between them and the vehicles"
    >
      <NetworkIcon />
      <span>Backend</span>
    </button>
  );
}

/**
 * Ask the metrics panel to show `metrics`, titled `title`, and open it. Only metrics this run
 * actually carries are asked for; with none, nothing opens and the caller says so.
 */
function showMetrics(title: string, metrics: readonly string[]): void {
  useStudio.getState().setMetricsFocus({ title, metrics });
  openPanel("metrics");
}

export function BackendPanel({ close }: { readonly close: () => void }): React.JSX.Element {
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

  return (
    <div className="fullpanel-inner backend-panel" data-testid="backend-panel">
      <div className="fullpanel-head">
        <h2>{snapshot ? systemTitle(snapshot.system) : "Backend"}</h2>
        {snapshot ? (
          <span className="dim" data-testid="backend-time">
            {snapshot.protocol} · {snapshot.signature} · state at {formatT(snapshot.t)}
          </span>
        ) : null}
        <span className="grow" />
        <button type="button" className="icon-button" onClick={close} aria-label="Close the backend" title="Close (Esc)" data-testid="backend-close">
          <CloseIcon />
        </button>
      </div>
      {snapshot === null ? (
        <div className="fullpanel-body backend-empty" data-testid="backend-empty">
          <p>No backend to show.</p>
          <p className="dim">
            A run publishes its credential system once a simulated second when the scenario sets{" "}
            <code>security.protocol</code> to <code>protocol/scms/camp</code> (the US SCMS) or{" "}
            <code>protocol/etsi/ts102941</code> (the European CCMS), and the clock has run past its first second.
            The ready-made <b>credential-lifecycle</b> and <b>ccms-lifecycle</b> scenarios do.
          </p>
          {error ? <p className="faint">The engine said: {error}</p> : null}
        </div>
      ) : (
        <>
          <Glance snapshot={snapshot} />
          <div className="backend-body">
            <Diagram snapshot={snapshot} selection={selection} onSelect={setSelection} />
            <Details snapshot={snapshot} selection={selection} onSelect={setSelection} />
          </div>
        </>
      )}
    </div>
  );
}

/** The strip across the top: the whole system in six numbers and its flows. */
function Glance({ snapshot }: { readonly snapshot: BackendSnapshot }): React.JSX.Element {
  const g = useMemo(() => glance(snapshot), [snapshot]);
  const flows = Object.entries(snapshot.flows)
    .filter(([, v]) => typeof v === "number" && v > 0)
    .sort((a, b) => (b[1] as number) - (a[1] as number))
    .slice(0, 6);
  return (
    <div className="backend-glance" data-testid="backend-glance">
      <Tile k="entities active" v={`${g.activeEntities} of ${g.entities}`} hint="Entities that sent or received a message in the last two simulated seconds" />
      <Tile k="links active" v={`${g.liveEdges} of ${g.edges}`} hint="Pairs that exchanged a message in the last two simulated seconds, of those that ever have" />
      <Tile k="messages" v={formatValue(g.messages)} hint="Every message between entities and devices since the run began" />
      <Tile k="carried" v={formatBytes(g.bytes)} hint="Their bytes, all transports" />
      <Tile k="waiting" v={formatValue(g.queued)} tone={g.queued > 0 ? "warn" : undefined} hint="Requests waiting in the entities' queues now" />
      {g.offline > 0 ? <Tile k="offline" v={String(g.offline)} hint="Entities kept air-gapped, as the design has them (the root CA, the elector)" /> : null}
      {flows.length > 0 ? (
        <div className="backend-flows" title="Protocol flows completed or in progress, by count">
          {flows.map(([k, v]) => (
            <span key={k} className="backend-flow">
              <b>{formatValue(v)}</b> {label(k)}
            </span>
          ))}
        </div>
      ) : null}
    </div>
  );
}

function Tile({ k, v, hint, tone }: { readonly k: string; readonly v: string; readonly hint: string; readonly tone?: "warn" }): React.JSX.Element {
  return (
    <div className={tone ? `backend-tile ${tone}` : "backend-tile"} title={hint} data-testid={`backend-tile-${k.replace(/\s+/g, "-")}`}>
      <span className="v">{v}</span>
      <span className="k">{k}</span>
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
  const active = useMemo(() => activeEntities(snapshot), [snapshot]);
  const pad = 12;
  const selectedId = selection?.kind === "entity" ? selection.id : null;
  const naturalWidth = placed.width + pad * 2 + 60;
  return (
    <div className="backend-diagram">
      {/* Scaled to the panel's width, so every column — the vehicles on the right included — is on
          screen at once; below 70 % of its natural size the text would be too small to read, and
          the panel scrolls instead. On a wide screen it grows to 140 %, so a full-screen panel is
          filled by the diagram rather than by empty space under an 11 px one. */}
      <svg
        width="100%"
        style={{ maxWidth: naturalWidth * 1.4, minWidth: naturalWidth * 0.7, display: "block" }}
        viewBox={`${-pad} ${-pad} ${naturalWidth} ${placed.height + pad * 2}`}
        preserveAspectRatio="xMinYMin meet"
        data-testid="backend-diagram"
        onClick={(ev) => {
          if (ev.target === ev.currentTarget) onSelect(null);
        }}
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
          const dim = selectedId !== null && !touches;
          const cls = ["backend-edge", isLive(e, snapshot.t) ? "live" : "", on || touches ? "on" : "", dim ? "dim" : "", e.transport].join(" ");
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
          const lit = active.has(p.id);
          return (
            <g
              key={p.id}
              transform={`translate(${p.x} ${p.y})`}
              className={["backend-box", selectedId === p.id ? "on" : "", e.online ? "" : "offline", lit ? "lit" : ""].join(" ")}
              onClick={() => onSelect({ kind: "entity", id: p.id })}
              data-testid={`backend-entity-${p.id}`}
              data-active={lit ? "true" : "false"}
            >
              <title>{e.role}</title>
              <rect width={p.w} height={p.h} rx={6} />
              <circle cx={12} cy={13} r={4} className="backend-dot" />
              <text x={22} y={17} className="backend-name">
                {e.name.length > 21 ? `${e.name.slice(0, 20)}…` : e.name}
              </text>
              {counts.map(([k, v], i) => (
                <text key={k} x={10} y={38 + i * 16} className="backend-count">
                  <tspan className="backend-count-v">{v}</tspan> {k.length > 20 ? `${k.slice(0, 19)}…` : k}
                </text>
              ))}
              {depth > 0 ? (
                <g transform={`translate(${p.w - 8} 13)`}>
                  <rect x={-30} y={-7} width={30} height={15} rx={7} className="backend-queue" />
                  <text x={-15} y={4} className="backend-queue-t" textAnchor="middle">
                    {depth}
                  </text>
                  <title>{depth} requests waiting in this entity&apos;s queue</title>
                </g>
              ) : null}
            </g>
          );
        })}
      </svg>
      <p className="backend-legend dim">
        <span className="backend-key"><i className="dot lit" /> active in the last 2 s</span>
        <span className="backend-key"><i className="line live" /> link in use</span>
        <span className="backend-key"><i className="line air" /> over the air</span>
        <span className="backend-key"><i className="pill" /> requests waiting</span>
        <span className="backend-key"><i className="box offline" /> kept offline</span>
        <span className="faint">Thicker lines carried more. Click a box or a line for everything the engine reports about it.</span>
      </p>
    </div>
  );
}

function Details({
  snapshot,
  selection,
  onSelect,
}: {
  readonly snapshot: BackendSnapshot;
  readonly selection: Selection;
  readonly onSelect: (s: Selection) => void;
}): React.JSX.Element {
  if (selection?.kind === "entity") {
    const e = snapshot.entities.find((x) => x.id === selection.id);
    if (e) return <EntityDetails entity={e} snapshot={snapshot} onSelect={onSelect} />;
  }
  if (selection?.kind === "edge") {
    const edge = snapshot.edges.find((x) => x.from === selection.from && x.to === selection.to);
    if (edge) return <EdgeDetails edge={edge} snapshot={snapshot} onSelect={onSelect} />;
  }
  const busiest = [...snapshot.entities]
    .sort((a, b) => b.traffic.received + b.traffic.sent - (a.traffic.received + a.traffic.sent))
    .filter((e) => e.traffic.received + e.traffic.sent > 0)
    .slice(0, 6);
  return (
    <aside className="backend-side" data-testid="backend-side">
      <h3>Busiest entities</h3>
      {busiest.length === 0 ? <p className="faint">Nothing has moved yet.</p> : null}
      <ul className="backend-rank">
        {busiest.map((e) => (
          <li key={e.id}>
            <button type="button" className="linklike" onClick={() => onSelect({ kind: "entity", id: e.id })}>
              {e.name}
            </button>
            <span className="mono">{formatValue(e.traffic.received + e.traffic.sent)}</span>
          </li>
        ))}
      </ul>
      <MetricsLink title="the credential system" metrics={metricsFor(null)} />
      <Recent snapshot={snapshot} filter={null} />
    </aside>
  );
}

/** "Metrics for …": the measurements that follow this entity, in the full-screen metrics panel. */
function MetricsLink({ title, metrics }: { readonly title: string; readonly metrics: readonly string[] }): React.JSX.Element | null {
  const tick = useStudio((s) => s.seriesTick);
  // Only what this run measures; a link to a plot that will stay empty is a dead end.
  const live = useMemo(() => {
    void tick;
    const have = new Set(engine.metrics.names());
    return metrics.filter((m) => have.has(m));
  }, [metrics, tick]);
  if (live.length === 0) return null;
  return (
    <button
      type="button"
      className="backend-metrics-link"
      data-testid="backend-metrics-link"
      title={`Open the metrics panel on ${live.join(", ")}`}
      onClick={() => showMetrics(title, live)}
    >
      Metrics for {title} <span className="faint">({live.length})</span> →
    </button>
  );
}

function EntityDetails({
  entity: e,
  snapshot,
  onSelect,
}: {
  readonly entity: BackendEntity;
  readonly snapshot: BackendSnapshot;
  readonly onSelect: (s: Selection) => void;
}): React.JSX.Element {
  const edgesIn = snapshot.edges.filter((x) => x.to === e.id);
  const edgesOut = snapshot.edges.filter((x) => x.from === e.id);
  const name = (id: string): string => snapshot.entities.find((x) => x.id === id)?.name ?? id;
  const state = Object.entries(e.state).filter(([, v]) => v !== null && v !== undefined);
  return (
    <aside className="backend-side" data-testid="backend-side">
      <h3>{e.name}</h3>
      <p className="dim">{e.role}</p>
      {!e.online ? <p className="warn">Offline: kept air-gapped, as the design has it.</p> : null}
      <MetricsLink title={e.name} metrics={metricsFor(e)} />
      {state.length > 0 ? (
        <>
          <h4>State</h4>
          <dl className="kv" data-testid="backend-entity-state">
            {state.map(([k, v]) => (
              <Pair key={k} k={label(k)} v={formatValue(v)} />
            ))}
          </dl>
        </>
      ) : null}
      <h4>Traffic</h4>
      <dl className="kv">
        <Pair k="received" v={`${formatValue(e.traffic.received)} (${formatBytes(e.traffic.bytes_in)})`} />
        <Pair k="sent" v={`${formatValue(e.traffic.sent)} (${formatBytes(e.traffic.bytes_out)})`} />
      </dl>
      {e.queue ? (
        <>
          <h4>Queue</h4>
          <dl className="kv">
            <Pair k="waiting" v={formatValue(e.queue.depth)} />
            <Pair k="servers" v={formatValue(e.queue.servers)} />
            <Pair k="served" v={formatValue(e.queue.served)} />
            {e.queue.served > 0 ? <Pair k="mean wait" v={`${(e.queue.waited_ns / e.queue.served / 1e6).toFixed(2)} ms`} /> : null}
            <Pair k="busy" v={formatDuration(e.queue.busy_ns)} />
          </dl>
        </>
      ) : null}
      {Object.keys(e.ops).length > 0 ? (
        <>
          <h4>Cryptographic operations</h4>
          <dl className="kv">
            {Object.entries(e.ops).map(([k, v]) => (
              <Pair key={k} k={k} v={formatValue(v)} />
            ))}
          </dl>
        </>
      ) : null}
      {edgesIn.length + edgesOut.length > 0 ? (
        <>
          <h4>Links</h4>
          <ul className="backend-links">
            {edgesIn.map((x) => (
              <li key={`in-${x.from}`}>
                <button type="button" className="linklike" onClick={() => onSelect({ kind: "edge", from: x.from, to: x.to })}>
                  from {name(x.from)}
                </button>{" "}
                <span className="mono">{formatValue(x.messages)}</span> <span className="faint">{x.transport}</span>
              </li>
            ))}
            {edgesOut.map((x) => (
              <li key={`out-${x.to}`}>
                <button type="button" className="linklike" onClick={() => onSelect({ kind: "edge", from: x.from, to: x.to })}>
                  to {name(x.to)}
                </button>{" "}
                <span className="mono">{formatValue(x.messages)}</span> <span className="faint">{x.transport}</span>
              </li>
            ))}
          </ul>
        </>
      ) : null}
      <Recent snapshot={snapshot} filter={e.id} />
    </aside>
  );
}

function EdgeDetails({
  edge,
  snapshot,
  onSelect,
}: {
  readonly edge: BackendEdge;
  readonly snapshot: BackendSnapshot;
  readonly onSelect: (s: Selection) => void;
}): React.JSX.Element {
  const name = (id: string): string => snapshot.entities.find((x) => x.id === id)?.name ?? id;
  return (
    <aside className="backend-side" data-testid="backend-side">
      <h3>
        <button type="button" className="linklike" onClick={() => onSelect({ kind: "entity", id: edge.from })}>
          {name(edge.from)}
        </button>{" "}
        →{" "}
        <button type="button" className="linklike" onClick={() => onSelect({ kind: "entity", id: edge.to })}>
          {name(edge.to)}
        </button>
      </h3>
      <dl className="kv">
        <Pair k="messages" v={formatValue(edge.messages)} />
        <Pair k="bytes" v={formatBytes(edge.bytes)} />
        <Pair k="carried over" v={edge.transport} />
        <Pair k="last" v={`${edge.last_step} at ${formatT(edge.last_t)}`} />
      </dl>
      {Object.keys(edge.steps).length > 0 ? (
        <>
          <h4>Protocol steps</h4>
          <dl className="kv">
            {Object.entries(edge.steps).map(([k, v]) => (
              <Pair key={k} k={k} v={formatValue(v)} />
            ))}
          </dl>
        </>
      ) : null}
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

/** A span of simulated time at a precision that shows a backend's microseconds of cryptography. */
function formatDuration(ns: number): string {
  if (ns >= 1e9) return `${(ns / 1e9).toFixed(2)} s`;
  if (ns >= 1e6) return `${(ns / 1e6).toFixed(2)} ms`;
  return `${(ns / 1e3).toFixed(1)} µs`;
}

/** One key and value of a `<dl>`. */
function Pair({ k, v }: { readonly k: string; readonly v: string }): React.JSX.Element {
  return (
    <>
      <dt>{k}</dt>
      <dd>{v}</dd>
    </>
  );
}
