/**
 * The Flows tab (00 §3.2): a sequence diagram of one flow instance, chosen from a list filtered by
 * flow type and vehicle (`inspect.flows`, `inspect.flow`).
 *
 * Lanes are the participants the engine logged — the device, then the backend in the order a
 * request meets it (DCM, ECA, LOP, RA, LA1, LA2, ACA, MA, CRLG, ...). Each arrow is one wire
 * message, labelled with the SPDU it carried (`EeRaCertRequestSpdu`, ...) or its step name, its
 * bytes and its time; each diamond a stage stamp. A gap of a second or more is drawn as a break
 * with its length, hatched when it is the shuffle wait (the gap a `shuffled` stamp closes).
 * Clicking an arrow opens the PDU decoder. In an entity's view the messages that entity neither
 * sent nor received are drawn faint and cannot be opened, because it never saw them.
 */

import { useCallback, useEffect, useMemo, useState } from "react";

import {
  canInspect,
  flowDetailOf,
  flowsOf,
  formatBytes,
  formatSimT,
  formatSpan,
  GROUND_TRUTH,
  sequenceOf,
  stagesOf,
  type FlowDetail,
  type FlowRecord,
  type FlowsAnswer,
  type SeqRow,
  type TrustAnswer,
} from "../../lib/pki.js";
import { engine } from "../../state/engine.js";
import { CloseIcon } from "../../shell/Icons.js";
import { PduDecoder } from "./PduDecoder.js";

const POLL_MS = 2000;
const LANE_W = 128;
const ROW_H = 30;
const GAP_H = 26;
const HEAD_H = 34;

function rowHeight(r: SeqRow): number {
  return r.kind === "gap" ? GAP_H : ROW_H;
}

/** The arrow's label: the SPDU(s) it carried, else the step name. */
function arrowLabel(r: FlowRecord): string {
  const types = [...new Set((r.pdus ?? []).map((p) => p.type))];
  if (types.length === 0) return r.name;
  const n = r.pdus?.length ?? 0;
  return `${types.join(", ")}${n > 1 ? ` ×${n}` : ""}`;
}

function Sequence({
  detail,
  view,
  selected,
  onPick,
}: {
  readonly detail: FlowDetail;
  readonly view: string;
  readonly selected: string | null;
  readonly onPick: (r: FlowRecord) => void;
}): React.JSX.Element {
  const { lanes, rows, t0 } = useMemo(() => sequenceOf(detail, view), [detail, view]);
  const width = Math.max(lanes.length, 2) * LANE_W;
  const height = HEAD_H + rows.reduce((h, r) => h + rowHeight(r), 0) + 12;
  const x = (i: number): number => LANE_W / 2 + i * LANE_W;
  let y = HEAD_H;
  return (
    <svg className="pki-seq" width={width} height={height} viewBox={`0 0 ${width} ${height}`} role="img" aria-label={`Sequence diagram of ${detail.instance}`} data-testid="pki-seq">
      <defs>
        <pattern id="pki-hatch" width="6" height="6" patternUnits="userSpaceOnUse" patternTransform="rotate(45)">
          <rect width="6" height="6" className="pki-hatch-bg" />
          <line x1="0" y1="0" x2="0" y2="6" className="pki-hatch-line" strokeWidth="2" />
        </pattern>
        <marker id="pki-arrowhead" viewBox="0 0 8 8" refX="7" refY="4" markerWidth="7" markerHeight="7" orient="auto-start-reverse">
          <path d="M0 0 L8 4 L0 8 z" className="pki-arrowhead" />
        </marker>
      </defs>
      {lanes.map((l, i) => (
        <g key={l.id} data-testid={`pki-lane-${l.id}`}>
          <text x={x(i)} y={14} textAnchor="middle" className={l.id === view ? "pki-lane-label viewing" : "pki-lane-label"}>
            {l.label}
          </text>
          {l.foreign ? (
            <text x={x(i)} y={27} textAnchor="middle" className="pki-lane-sub">
              {l.foreign}
            </text>
          ) : null}
          <line x1={x(i)} x2={x(i)} y1={HEAD_H - 4} y2={height - 6} className="pki-lifeline" />
        </g>
      ))}
      {rows.map((r, k) => {
        const top = y;
        y += rowHeight(r);
        if (r.kind === "gap") {
          return (
            <g key={`gap${k}`} data-testid={r.shuffle ? "pki-gap-shuffle" : "pki-gap"}>
              <rect x={4} y={top + 3} width={width - 8} height={GAP_H - 6} className={r.shuffle ? "pki-gap shuffle" : "pki-gap"} fill={r.shuffle ? "url(#pki-hatch)" : undefined} />
              <text x={width / 2} y={top + GAP_H / 2 + 4} textAnchor="middle" className="pki-gap-label">
                {r.shuffle ? "shuffle wait" : "wait"} {formatSpan(r.ns)}
              </text>
            </g>
          );
        }
        if (r.kind === "stage") {
          const cx = x(r.lane);
          const cy = top + ROW_H / 2;
          return (
            <g key={r.record.id} className={r.observed ? "pki-stage" : "pki-stage unobserved"} data-testid={`pki-stage-${r.record.name}`}>
              <path d={`M${cx} ${cy - 5} L${cx + 5} ${cy} L${cx} ${cy + 5} L${cx - 5} ${cy} z`} className="pki-diamond" />
              <text x={cx + 9} y={cy + 4} className="pki-stage-label">
                {r.record.name} · +{formatSpan(r.dt)}
              </text>
            </g>
          );
        }
        const x1 = x(r.from);
        const x2 = x(r.to);
        const cy = top + ROW_H - 8;
        const self = r.from === r.to;
        const clickable = r.observed;
        const cls = ["pki-msg", r.observed ? "" : "unobserved", selected === r.record.id ? "selected" : "", (r.record.pdus?.length ?? 0) > 0 ? "has-pdu" : ""].filter(Boolean).join(" ");
        const mid = (x1 + x2) / 2;
        return (
          <g
            key={r.record.id}
            className={cls}
            onClick={clickable ? () => onPick(r.record) : undefined}
            data-testid={`pki-msg-${r.record.id}`}
            data-observed={r.observed ? "1" : "0"}
            role={clickable ? "button" : undefined}
            tabIndex={clickable ? 0 : undefined}
            onKeyDown={clickable ? (e) => (e.key === "Enter" || e.key === " ") && onPick(r.record) : undefined}
          >
            <title>
              {r.record.name}: {r.record.from} → {r.record.to} · {r.record.bytes ?? 0} B · {formatSimT(r.record.t)}
              {r.record.transport ? ` · ${r.record.transport}` : ""}
              {r.observed ? "" : ` — not seen by ${view}`}
            </title>
            <rect x={Math.min(x1, x2) - 4} y={top + 1} width={Math.abs(x2 - x1) + 8 || 60} height={ROW_H - 2} className="pki-msg-hit" />
            {self ? (
              <path d={`M${x1} ${cy - 6} h24 v8 h-24`} className="pki-msg-line" markerEnd="url(#pki-arrowhead)" fill="none" />
            ) : (
              <line x1={x1} x2={x2} y1={cy} y2={cy} className="pki-msg-line" markerEnd="url(#pki-arrowhead)" />
            )}
            <text x={self ? x1 + 30 : mid} y={cy - 5} textAnchor={self ? "start" : "middle"} className="pki-msg-label">
              {arrowLabel(r.record)}
              <tspan className="pki-msg-meta">
                {" "}
                {r.record.bytes ?? 0} B · +{formatSpan(r.record.t - t0)}
              </tspan>
            </text>
          </g>
        );
      })}
    </svg>
  );
}

function MessageCard({
  record,
  view,
  onPdu,
  onClose,
}: {
  readonly record: FlowRecord;
  readonly view: string;
  readonly onPdu: (id: string) => void;
  readonly onClose: () => void;
}): React.JSX.Element {
  return (
    <aside className="pki-drawer" data-testid="pki-msg-card">
      <div className="pki-drawer-head">
        <h3 className="mono">{record.name}</h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close" data-testid="pki-msg-close">
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        <dl className="pki-pairs">
          <dt>from → to</dt>
          <dd className="mono">
            {record.from} → {record.to}
          </dd>
          <dt>left at</dt>
          <dd className="mono">{formatSimT(record.t)}</dd>
          <dt>bytes</dt>
          <dd className="mono">{record.bytes ?? 0}</dd>
          {record.transport ? (
            <>
              <dt>transport</dt>
              <dd className="mono">{record.transport}</dd>
            </>
          ) : null}
        </dl>
        {(record.pdus ?? []).length === 0 ? <p className="faint">It carried no SPDU of its own: a backend control message whose bytes the engine counts but does not encode.</p> : null}
        <ul className="pki-pdus">
          {(record.pdus ?? []).map((p) => (
            <li key={p.id}>
              <button type="button" className="linklike mono" disabled={!canInspect(p, record, view)} onClick={() => onPdu(p.id)} data-testid={`pki-pdu-${p.id}`}>
                {p.type}
              </button>{" "}
              <span className="dim mono">
                {p.len} B{p.sealed_to ? ` · encrypted to ${p.sealed_to}` : ""}
              </span>
            </li>
          ))}
        </ul>
      </div>
    </aside>
  );
}

/** The tab. */
export function FlowsTab({ view }: { readonly view: string; readonly trust: TrustAnswer }): React.JSX.Element {
  const [flow, setFlow] = useState<string>("");
  const [device, setDevice] = useState<string>("");
  const [list, setList] = useState<FlowsAnswer | null>(null);
  const [instance, setInstance] = useState<string | null>(null);
  const [detail, setDetail] = useState<FlowDetail | null>(null);
  const [message, setMessage] = useState<FlowRecord | null>(null);
  const [pdu, setPdu] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const pollList = useCallback(async () => {
    try {
      const params: { flow?: string; device?: string; limit: number } = { limit: 300 };
      if (flow) params.flow = flow;
      if (device.trim()) params.device = device.trim();
      const a = flowsOf(await engine.request("inspect.flows", params, { quiet: true, timeoutMs: 4000 }));
      setList(a);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [flow, device]);

  useEffect(() => {
    void pollList();
    const id = window.setInterval(() => void pollList(), POLL_MS);
    return () => window.clearInterval(id);
  }, [pollList]);

  const chosen = instance ?? list?.instances[0]?.instance ?? null;

  useEffect(() => {
    if (chosen === null) {
      setDetail(null);
      return undefined;
    }
    let live = true;
    const load = async (): Promise<void> => {
      try {
        const d = flowDetailOf(await engine.request("inspect.flow", { instance: chosen }, { quiet: true, timeoutMs: 6000 }));
        if (live) setDetail(d);
      } catch (err) {
        if (live) setError(err instanceof Error ? err.message : String(err));
      }
    };
    void load();
    const id = window.setInterval(() => void load(), POLL_MS);
    return () => {
      live = false;
      window.clearInterval(id);
    };
  }, [chosen]);

  // A new view closes what the old one had open: the new entity may never have seen it.
  useEffect(() => {
    setMessage(null);
    setPdu(null);
  }, [view]);

  const pick = useCallback(
    (r: FlowRecord) => {
      const pdus = r.pdus ?? [];
      const only = pdus.length === 1 ? pdus[0] : undefined;
      setMessage(r);
      setPdu(only && canInspect(only, r, view) ? only.id : null);
    },
    [view],
  );

  const flowNames = Object.keys(list?.flows ?? {});
  const stages = detail ? stagesOf(detail) : [];
  const drawer = pdu !== null || message !== null;

  return (
    <div className={drawer ? "pki-body with-drawer" : "pki-body"} data-testid="pki-flows">
      <div className="pki-flows">
        <div className="pki-flow-list">
          <div className="pki-flow-filters">
            <select value={flow} onChange={(e) => setFlow(e.target.value)} data-testid="pki-flow-type" aria-label="Flow type">
              <option value="">every flow</option>
              {flowNames.map((f) => (
                <option key={f} value={f}>
                  {f} ({list?.flows[f] ?? 0})
                </option>
              ))}
            </select>
            <input value={device} onChange={(e) => setDevice(e.target.value)} placeholder="vehicle (device id)" aria-label="Vehicle" data-testid="pki-flow-device" />
          </div>
          {error ? <p className="faint">The engine said: {error}</p> : null}
          <p className="dim">
            {list ? `${list.total} instance${list.total === 1 ? "" : "s"}${list.total > list.instances.length ? `, newest ${list.instances.length}` : ""}` : "…"}
          </p>
          <ul data-testid="pki-flow-instances">
            {(list?.instances ?? []).map((i) => (
              <li key={i.instance}>
                <button
                  type="button"
                  className={i.instance === chosen ? "pki-flow-row on" : "pki-flow-row"}
                  onClick={() => {
                    setInstance(i.instance);
                    setMessage(null);
                    setPdu(null);
                  }}
                  data-testid={`pki-flow-${i.instance}`}
                >
                  <span className="mono">{i.instance}</span>
                  <span className="dim">
                    {formatSimT(i.first_t)} · {i.messages} msg · {formatBytes(i.bytes)}
                    {i.device_count > 0 ? ` · ${i.device_count === 1 ? (i.devices[0] ?? "") : `${i.device_count} devices`}` : ""}
                  </span>
                  {i.last_stage ? <span className="pill">{i.last_stage}</span> : null}
                </button>
              </li>
            ))}
          </ul>
        </div>
        <div className="pki-flow-main">
          {detail ? (
            <>
              <h3 className="pki-flow-title">
                <span className="mono">{detail.instance}</span>{" "}
                <span className="dim">
                  {detail.messages} messages · {detail.stages} stages · {formatBytes(detail.bytes)} · {formatSpan(detail.last_t - detail.first_t)}
                  {view !== GROUND_TRUTH ? ` · as ${view}` : ""}
                  {detail.dropped > 0 ? ` · ${detail.dropped} records not kept` : ""}
                </span>
              </h3>
              <div className="pki-seq-wrap">
                <Sequence detail={detail} view={view} selected={message?.id ?? null} onPick={pick} />
              </div>
              {stages.length > 0 ? (
                <table className="pki-table pki-stages" data-testid="pki-stages">
                  <thead>
                    <tr>
                      <th>stage</th>
                      <th>stamped by</th>
                      <th>at</th>
                      <th>since start</th>
                      <th>since previous</th>
                    </tr>
                  </thead>
                  <tbody>
                    {stages.map((s, i) => (
                      <tr key={`${s.name}${i}`}>
                        <td className="mono">{s.name}</td>
                        <td className="mono">{s.by}</td>
                        <td className="mono">{formatSimT(s.t)}</td>
                        <td className="mono">{formatSpan(s.dt)}</td>
                        <td className="mono">{formatSpan(s.step)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              ) : null}
            </>
          ) : (
            <p className="dim">{list && list.total === 0 ? "No flow recorded yet." : "Choose a flow instance."}</p>
          )}
        </div>
      </div>
      {pdu !== null ? (
        <PduDecoder id={pdu} view={view} onClose={() => setPdu(null)} />
      ) : message !== null ? (
        <MessageCard record={message} view={view} onPdu={setPdu} onClose={() => setMessage(null)} />
      ) : null}
    </div>
  );
}
