/**
 * A vehicle's history (`lib/vehicleHistory.ts`): every step of its life in the run as a vertical
 * stepper, newest at the bottom, with filters by kind. Clicking a step opens its detail in place:
 * when it happened and how long it took, who it talked to, every message with its size, link and
 * contents (real SPDUs by type; modelled messages part by part), each stage's time, and the total
 * overhead. Opened from the vehicle panel's History button.
 */

import { Fragment, useEffect, useMemo, useState } from "react";

import { flowDetailOf, flowsOf, formatSimT, stagesOf, type FlowRecord, type FlowRow } from "../lib/pki.js";
import { pduText, transportText, wordsOf } from "../lib/pkiPdu.js";
import { entityLabel, type VehicleDetail } from "../lib/pkiRecords.js";
import { KIND_INFO, clockText, durText, historySteps, kb, radioSamples, type HistoryStep, type StepKind } from "../lib/vehicleHistory.js";
import { CloseIcon } from "../shell/Icons.js";
import { engine } from "../state/engine.js";
import { usePki } from "./pki-records/state.js";
import { ModelledParts } from "./pki/FlowsTab.js";

type Filter = "all" | "certs" | "crl" | "report" | "pseudonym" | "radio";
const FILTERS: readonly { readonly id: Filter; readonly label: string; readonly kinds: readonly StepKind[] }[] = [
  { id: "all", label: "Everything", kinds: [] },
  { id: "certs", label: "Certificates", kinds: ["setup", "enrolment", "download", "topup", "policy"] },
  { id: "pseudonym", label: "Pseudonym changes", kinds: ["pseudonym"] },
  { id: "crl", label: "Revocation lists", kinds: ["crl", "revoked"] },
  { id: "report", label: "Reports", kinds: ["report"] },
  { id: "radio", label: "BSMs", kinds: ["radio"] },
];

export function VehicleHistory({ node, device, onClose }: { readonly node: number; readonly device: string; readonly onClose: () => void }): React.JSX.Element {
  const [flows, setFlows] = useState<readonly FlowRow[]>([]);
  const [filter, setFilter] = useState<Filter>("all");
  const [open, setOpen] = useState<string | null>(null);
  const [tick, setTick] = useState(0);
  const detail = usePki({ topic: "vehicle", device }, 4000).data as VehicleDetail | null;
  const preloadId = flows.find((f) => f.flow === "preload")?.instance ?? null;
  const [factory, setFactory] = useState<readonly FlowRecord[] | null>(null);
  useEffect(() => {
    if (preloadId === null) return undefined;
    let live = true;
    engine
      .request("inspect.flow", { instance: preloadId }, { quiet: true, timeoutMs: 8000 })
      .then((a) => live && setFactory((flowDetailOf(a)?.records ?? []).filter((r) => r.from === device || r.to === device)))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [preloadId, device]);

  useEffect(() => {
    let live = true;
    const ask = (): void => {
      engine
        .request("inspect.flows", { device, limit: 400 }, { quiet: true, timeoutMs: 5000 })
        .then((a) => live && setFlows(flowsOf(a)?.instances ?? []))
        .catch(() => undefined);
      setTick((n) => n + 1);
    };
    ask();
    const id = window.setInterval(ask, 3000);
    return () => {
      live = false;
      window.clearInterval(id);
    };
  }, [device]);

  useEffect(() => {
    const key = (e: KeyboardEvent): void => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [onClose]);

  const steps = useMemo(
    () => withFactory(historySteps(flows, detail && Array.isArray(detail.pseudonyms) ? detail : null, radioSamples(node)), factory),
    // `tick` re-reads the radio samples kept outside React.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [flows, detail, node, tick, factory],
  );
  const counts = useMemo(() => {
    const c = new Map<Filter, number>();
    for (const f of FILTERS) c.set(f.id, f.id === "all" ? steps.length : steps.filter((s) => f.kinds.includes(s.kind)).length);
    return c;
  }, [steps]);
  const kinds = FILTERS.find((f) => f.id === filter)?.kinds ?? [];
  const shown = filter === "all" ? steps : steps.filter((s) => kinds.includes(s.kind));

  return (
    <div className="vh-backdrop" onClick={onClose} data-testid="vehicle-history">
      <div className="vh" role="dialog" aria-label={`Vehicle ${node} history`} onClick={(e) => e.stopPropagation()}>
        <header className="vh-head">
          <div>
            <h2>Vehicle {node} · history</h2>
            <span className="faint">
              device {device.replace(/^ee\//, "")}
              {detail?.provider ? ` · ${detail.provider}` : ""}
            </span>
          </div>
          <button type="button" className="icon-button" onClick={onClose} aria-label="Close">
            <CloseIcon />
          </button>
        </header>
        <nav className="vh-filters">
          {FILTERS.map((f) => (
            <button key={f.id} type="button" className={f.id === filter ? "vh-chip on" : "vh-chip"} onClick={() => setFilter(f.id)} disabled={f.id !== "all" && (counts.get(f.id) ?? 0) === 0}>
              {f.label} <span className="vh-count">{counts.get(f.id) ?? 0}</span>
            </button>
          ))}
        </nav>
        {filter === "radio" || (filter === "all" && radioSamples(node).length === 0) ? (
          <p className="vh-note faint">BSM activity is kept from when you selected this vehicle: the radio reports it once a simulated second while the run plays.</p>
        ) : null}
        <ol className="vh-steps">
          {shown.length === 0 ? <li className="faint vh-empty">Nothing yet. Play the run and steps appear here.</li> : null}
          {shown.map((s) => (
            <li key={s.id} className={open === s.id ? "vh-step open" : "vh-step"}>
              <button type="button" className="vh-step-head" onClick={() => setOpen(open === s.id ? null : s.id)} aria-expanded={open === s.id}>
                <span className="vh-time mono">{s.records && s.t === 0 ? "factory" : s.t === 0 && s.kind === "setup" ? "before" : clockText(s.t)}</span>
                <span className="vh-dot" style={{ background: KIND_INFO[s.kind].color }} />
                <span className="vh-text">
                  <span className="vh-title">{s.title}</span>
                  <span className="vh-summary">{s.summary}</span>
                </span>
                <span className="vh-kind" style={{ color: KIND_INFO[s.kind].color }}>
                  {KIND_INFO[s.kind].label}
                </span>
              </button>
              {open === s.id ? <StepDetail step={s} device={device} /> : null}
            </li>
          ))}
        </ol>
      </div>
    </div>
  );
}

function StepDetail({ step, device }: { readonly step: HistoryStep; readonly device: string }): React.JSX.Element {
  if (step.radio) {
    const r = step.radio;
    return (
      <div className="vh-detail">
        <Facts
          rows={[
            ["When", `${formatSimT(step.t)} – ${formatSimT(step.end)} (${r.seconds} s reported)`],
            ["BSMs sent", `${Math.round(r.sent)} (about ${(r.sent / Math.max(1, r.seconds)).toFixed(1)} a second), ${Math.round(r.fullCert)} with the full certificate, the rest with its 8-byte digest`],
            ["Messages heard", `${Math.round(r.heard)} from ${r.neighbours} vehicles around it`],
            ["Neighbours verified", `${r.verified} of ${r.neighbours}${r.revokedNeighbours > 0 ? `, ${r.revokedNeighbours} on a revocation list` : ""}`],
            ["Unknown certificates", r.digestMisses > 0 ? `${Math.round(r.digestMisses)} messages named a certificate it had not seen yet (it waits for the full one)` : "none"],
            ["Channel busy (peak)", `${(r.cbrMax / 10).toFixed(1)} %`],
          ]}
        />
      </div>
    );
  }
  if (step.pseudonym) {
    return (
      <div className="vh-detail">
        <Facts
          rows={[
            ["When", formatSimT(step.t)],
            ["Certificate", step.pseudonym],
            ["What changes", "the certificate it signs with, and with it the temporary ID and MAC address in its BSMs, so listeners cannot link old and new messages"],
            ["Exchanged", "nothing: the certificate was already in its pool"],
          ]}
        />
      </div>
    );
  }
  if (step.kind === "revoked") {
    return (
      <div className="vh-detail">
        <Facts rows={[["When", formatSimT(step.t)], ["What happened", "a revocation list it installed names its linkage value, so it stops signing; receivers refuse its messages as Revoked"]]} />
      </div>
    );
  }
  if (step.records) return <Messages records={step.records} device={device} stages={[]} before />;
  return step.instance ? <FlowDetailView instance={step.instance} device={device} row={step.flow ?? null} /> : <div className="vh-detail faint">No more detail.</div>;
}

function FlowDetailView({ instance, device, row }: { readonly instance: string; readonly device: string; readonly row: FlowRow | null }): React.JSX.Element {
  const [d, setD] = useState<ReturnType<typeof flowDetailOf>>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    engine
      .request("inspect.flow", { instance }, { quiet: true, timeoutMs: 6000 })
      .then((a) => live && setD(flowDetailOf(a)))
      .catch((e: unknown) => live && setErr(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [instance]);
  if (err) return <div className="vh-detail faint">Could not load this step: {err}</div>;
  if (!d) return <div className="vh-detail faint">Loading…</div>;
  const msgs = d.records.filter((r) => r.kind === "msg" || r.kind === "pdu");
  // A shared flow keeps only this vehicle's messages when it has any.
  const mine = msgs.filter((m) => m.from === device || m.to === device);
  return <Messages records={mine.length > 0 ? mine : msgs} device={device} stages={stagesOf(d)} total={msgs.length} pdus={row?.pdus ?? 0} />;
}

function Messages({
  records,
  device,
  stages,
  total,
  pdus,
  before,
}: {
  readonly records: readonly FlowRecord[];
  readonly device: string;
  readonly stages: ReturnType<typeof stagesOf>;
  readonly total?: number;
  readonly pdus?: number;
  readonly before?: boolean;
}): React.JSX.Element {
  const [inside, setInside] = useState<string | null>(null);
  const list = records;
  const bytes = list.reduce((a, m) => a + (m.bytes ?? 0), 0);
  const peers = [...new Set(list.flatMap((m) => [m.from, m.to]).filter((x): x is string => !!x && x !== device))];
  const links = [...new Set(list.map((m) => m.transport).filter((x): x is string => !!x))];
  const t0 = list[0]?.t ?? 0;
  const t1 = list[list.length - 1]?.t ?? t0;
  const secured = pdus ?? list.reduce((a, m) => a + (m.pdus ?? []).length, 0);
  return (
    <div className="vh-detail">
      <Facts
        rows={[
          ["When", before ? "before the run, at the factory and the dealer (not on the run's clock)" : `${formatSimT(t0)}${t1 > t0 ? ` – ${formatSimT(t1)} (took ${durText(t1 - t0)})` : ""}`],
          ["Talked with", peers.map(entityLabel).join(", ") || "—"],
          ["Over", before ? "the factory's secure connection" : links.map(transportText).join(", ") || "the backend network"],
          ["Overhead", `${list.length} message${list.length === 1 ? "" : "s"}, ${kb(bytes)}${secured > 0 ? `, ${secured} secured (SPDUs)` : ""}`],
          ...(total !== undefined && total > list.length ? [["Shared step", `${total} messages in all; ${list.length} are this vehicle's`] as const] : []),
        ]}
      />
      <h4>Messages</h4>
      <table className="vh-msgs">
        <thead>
          <tr>
            {before ? null : <th>At</th>}
            <th>From → to</th>
            <th>Message</th>
            <th className="num">Size</th>
            <th>Inside</th>
          </tr>
        </thead>
        <tbody>
          {groups(list).map(({ m, n }) => {
            const spdus = m.pdus ?? [];
            return (
              <Fragment key={m.id}>
                <tr>
                  {before ? null : <td className="mono nowrap">{m.t === t0 ? "start" : `+${durText(m.t - t0)}`}</td>}
                  <td className="nowrap" title={`${entityLabel(m.from ?? "?")} → ${entityLabel(m.to ?? "?")}`}>
                    {shortName(m.from ?? "?", device)} → {shortName(m.to ?? "?", device)}
                  </td>
                  <td>
                    {m.kind === "pdu" ? pduText(m.name).what.split(":")[0] : wordsOf(m.name)}
                    {n > 1 ? <b className="vh-times"> × {n}</b> : null}
                  </td>
                  <td className="num mono">{n > 1 ? `${kb(m.bytes ?? 0)} each` : kb(m.bytes ?? 0)}</td>
                  <td>
                    {spdus.length > 0 ? (
                      <span className="mono dim" title={spdus.map((p) => pduText(p.type).what).join("\n")}>
                        {spdus.map((p) => p.type).join(", ")}
                      </span>
                    ) : (
                      <button type="button" className="linkbutton" onClick={() => setInside(inside === m.id ? null : m.id)}>
                        {inside === m.id ? "hide" : "what's inside"}
                      </button>
                    )}
                  </td>
                </tr>
                {inside === m.id ? (
                  <tr className="vh-inside">
                    <td colSpan={before ? 4 : 5}>
                      <ModelledParts name={m.name} total={m.bytes ?? 0} />
                    </td>
                  </tr>
                ) : null}
              </Fragment>
            );
          })}
        </tbody>
      </table>
      {stages.length > 0 ? (
        <>
          <h4>Stages</h4>
          <ol className="vh-stages">
            {stages.map((st, i) => (
              <li key={i}>
                <span className="mono">{st.dt === 0 ? "start" : `+${durText(st.dt)}`}</span> {wordsOf(st.name)} <span className="faint">· {entityLabel(st.by)}</span>
              </li>
            ))}
          </ol>
        </>
      ) : null}
    </div>
  );
}

/** Consecutive identical messages (same sender, receiver, kind and size) as one row with a count. */
function groups(list: readonly FlowRecord[]): { readonly m: FlowRecord; readonly n: number }[] {
  const out: { m: FlowRecord; n: number }[] = [];
  for (const m of list) {
    const last = out[out.length - 1];
    if (last && last.m.from === m.from && last.m.to === m.to && last.m.name === m.name && last.m.bytes === m.bytes) last.n += 1;
    else out.push({ m, n: 1 });
  }
  return out;
}

const ROLE_SHORT: Readonly<Record<string, string>> = { ra: "RA", aca: "ACA", pca: "ACA", dcm: "DCM", eca: "ECA", ma: "MA", la1: "LA1", la2: "LA2", crlg: "CRL Generator", lop: "LOP", crl_store: "CRL Store", pg: "Policy Generator" };

function shortName(id: string, device: string): string {
  if (id === device) return "This vehicle";
  const m = /^[^/]+\/([a-z0-9_]+)\/\d+$/.exec(id);
  return m ? (ROLE_SHORT[m[1]] ?? m[1].toUpperCase()) : entityLabel(id);
}

/** The factory steps of this vehicle, split out of the shared setup flow: enrolment, the request for its first pseudonyms, and their download. */
function withFactory(steps: readonly HistoryStep[], factory: readonly FlowRecord[] | null): HistoryStep[] {
  if (!factory || factory.length === 0) return [...steps];
  const enrol = factory.filter((r) => /Eca|Dcm|Enrol/i.test(r.name));
  const ask = factory.filter((r) => /EeRaCertRequest|RaEeCertAck/.test(r.name));
  const crls = factory.filter((r) => /Crl/i.test(r.name) && !enrol.includes(r));
  const dl = factory.filter((r) => !enrol.includes(r) && !ask.includes(r) && !crls.includes(r));
  const certs = dl.filter((r) => /CertResponse/.test(r.name)).length;
  const mk = (id: string, kind: StepKind, title: string, recs: readonly FlowRecord[]): HistoryStep => ({
    id,
    kind,
    t: 0,
    end: 0,
    title,
    summary: `${recs.length} message${recs.length === 1 ? "" : "s"} · ${kb(recs.reduce((a, r) => a + (r.bytes ?? 0), 0))}`,
    instance: null,
    records: recs,
  });
  const out = steps.filter((s) => s.kind !== "setup");
  const add: HistoryStep[] = [];
  if (enrol.length) add.push(mk("f-enrol", "enrolment", "Enrolled at the factory: got its enrolment certificate (long-term identity) from the ECA", enrol));
  if (ask.length) add.push(mk("f-ask", "download", "Asked the RA for pseudonym certificates (butterfly key request)", ask));
  if (dl.length) add.push(mk("f-dl", "download", `Downloaded its first ${certs} pseudonym certificates`, dl));
  if (crls.length) add.push(mk("f-crl", "crl", `Received the current revocation list${crls.length === 1 ? "" : `s (${crls.length}, one per provider)`}`, crls));
  return [...add, ...out];
}

function Facts({ rows }: { readonly rows: readonly (readonly [string, string])[] }): React.JSX.Element {
  return (
    <dl className="vh-facts">
      {rows.map(([k, v]) => (
        <div key={k}>
          <dt>{k}</dt>
          <dd>{v}</dd>
        </div>
      ))}
    </dl>
  );
}
