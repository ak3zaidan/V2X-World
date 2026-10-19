/**
 * The workspace's Overview (00 §3.2 "who exists" and "what is happening now"): the deployment as a
 * live topology — the SCMS Manager and the electors, each provider as a region with its
 * authorities (offline ones hatched with a lock, ones it does not run as ghosts), its vehicles and
 * roadside units as a client group, the links between them and every message travelling its link
 * as it happens (`Topology.tsx`). Under it, the certificate trust list (CTL) that binds the
 * providers.
 *
 * Selecting a node opens its card on the right: what the role does in one sentence, its status
 * and traffic, its certificate decoded with the validation trace a device runs (`inspect.cert`),
 * what it knows (`inspect.knowledge`; in an entity's view only that entity's own record, with no
 * ground-truth join) and the messages it sent or received recently.
 */

import { useEffect, useMemo, useRef, useState } from "react";

import {
  certOf,
  componentIndex,
  ctlSummary,
  formatBytes,
  formatSimT,
  formatSpan,
  GROUND_TRUTH,
  knowledgeOf,
  time32Date,
  type CertAnswer,
  type KnowledgeAnswer,
  type PkiActivity,
  type PkiComponent,
  type PkiProvider,
  type TrustAnswer,
} from "../../lib/pki.js";
import { flowInfo, isDevice, messagesOf, messageText, observedBy, roleInfo, type TrustMessage } from "../../lib/pkiTopology.js";
import { engine } from "../../state/engine.js";
import { CloseIcon } from "../../shell/Icons.js";
import { chooseTab } from "../pki-records/state.js";
import { JerTree } from "./PduDecoder.js";
import { Topology } from "./Topology.js";

/** Messages the Overview remembers for the cards' "recent messages", newest last. */
const LOG_CAP = 600;
/** Messages a card lists. */
const CARD_MESSAGES = 14;

/** The CTL line under the topology: sequence, roots, quorum, signers, history. */
function TrustList({ trust }: { readonly trust: TrustAnswer }): React.JSX.Element {
  const s = trust.state;
  const ctl = ctlSummary(s);
  const [history, setHistory] = useState(false);
  return (
    <section className="pki-ctl-strip">
      <div className="pki-gov-row">
        <span className="pki-ctl mono" data-testid="pki-ctl" title="The certificate trust list (CTL): the roots every device trusts, signed by a quorum of electors">
          CTL seq {ctl.sequence} · {ctl.roots} root{ctl.roots === 1 ? "" : "s"} · {s.ctl.bytes} B · effective {time32Date(s.ctl.effective)}
        </span>
        <span className="dim" title="Elector endorsements a trust-list change needs, of the electors there are">
          quorum {ctl.quorum} · endorsed by {s.ctl.signers.length}
        </span>
        {s.ctl.pending > 0 ? (
          <span className="pill warn" title="A trust-list change the SCMS Manager decided, not yet in force">
            {s.ctl.pending} pending
          </span>
        ) : null}
        <span className="grow" />
        <button type="button" className="linklike" onClick={() => setHistory((h) => !h)} data-testid="pki-ctl-history-toggle" aria-expanded={history}>
          history ({s.ctl.history.length})
        </button>
      </div>
      {ctl.offCtl.length > 0 ? (
        <p className="warn" data-testid="pki-off-ctl">
          Not on the trust list: {ctl.offCtl.join(", ")} — devices reject its certificates.
        </p>
      ) : null}
      {history ? (
        <table className="pki-table" data-testid="pki-ctl-history">
          <thead>
            <tr>
              <th>sequence</th>
              <th>effective</th>
              <th>roots</th>
              <th>endorsed by electors</th>
            </tr>
          </thead>
          <tbody>
            {s.ctl.history.map((h) => (
              <tr key={h.sequence}>
                <td className="mono">{h.sequence}</td>
                <td className="mono">{time32Date(h.effective)}</td>
                <td className="mono">{h.roots}</td>
                <td className="mono">{h.signers.map((x) => x + 1).join(", ")}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
      <div className="pki-roots dim mono" title="Each root as the CTL lists it: HashedId32 (SHA-256 of its certificate)">
        {s.ctl.roots.map((r) => (
          <span key={r}>{r.slice(0, 16)}…</span>
        ))}
      </div>
    </section>
  );
}

/** The other end of a message, in plain words. */
function endText(end: string | null, home: string): string {
  if (end === null) return "?";
  if (isDevice(end)) return end;
  const [provider, role] = end.split("/");
  return `${roleInfo(role ?? end).name}${provider !== home ? ` (${provider})` : ""}`;
}

/** The messages one node sent or received, newest first, as the view may see them. */
function RecentMessages({ id, log, view }: { readonly id: string; readonly log: readonly TrustMessage[]; readonly view: string }): React.JSX.Element {
  const devices = id.startsWith("devices/") ? id.slice("devices/".length) : null;
  const home = devices ?? id.split("/")[0];
  const touches = (m: TrustMessage): boolean =>
    devices !== null ? (isDevice(m.from) || isDevice(m.to)) && [m.from, m.to].some((e) => e !== null && e.startsWith(`${devices}/`)) : m.from === id || m.to === id;
  const mine = log.filter((m) => observedBy(m, view) && touches(m)).slice(-CARD_MESSAGES).reverse();
  return (
    <section className="pki-card-section" data-testid="pki-card-messages">
      <h4>Recent messages</h4>
      {mine.length === 0 ? (
        <p className="faint">None since the view opened{view === GROUND_TRUTH ? "" : " that this view observed"}.</p>
      ) : (
        <ul className="pki-recent">
          {mine.map((m) => {
            const out = devices !== null ? isDevice(m.from) : m.from === id;
            return (
              <li key={m.id} title={`${m.instance} · ${m.from ?? "?"} → ${m.to ?? "?"}`}>
                <i className="pki-swatch" style={{ background: flowInfo(m.flow).color }} />
                <span className="mono faint">{formatSimT(m.t)}</span>
                <span aria-label={out ? "sent to" : "received from"}>{out ? "→" : "←"}</span>
                <span>{endText(out ? m.to : m.from, home)}</span>
                <span className="mono">{messageText(m.name)}</span>
                {m.bytes !== null ? <span className="faint mono">{formatBytes(m.bytes)}</span> : null}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}

/** The card of a provider's client group. */
function DevicesCard({ provider, log, view, onClose }: { readonly provider: PkiProvider; readonly log: readonly TrustMessage[]; readonly view: string; readonly onClose: () => void }): React.JSX.Element {
  const info = roleInfo("devices");
  const d = provider.devices;
  return (
    <aside className="pki-drawer" data-testid="pki-card">
      <div className="pki-drawer-head">
        <h3 title={info.term}>
          {info.name} <span className="dim mono">{provider.label}</span>
        </h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the card" data-testid="pki-card-close">
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        <p className="pki-does">{info.does}</p>
        <dl className="pki-pairs">
          <dt>devices</dt>
          <dd className="mono">{d.devices}</dd>
          <dt>enrolled</dt>
          <dd className="mono">{d.enrolled}</dd>
          <dt>provisioned</dt>
          <dd className="mono">{d.provisioned}</dd>
          <dt>revoked</dt>
          <dd className="mono">{d.revoked}</dd>
          <dt title="Enrolment certificates the RA refuses after a revocation">blocklisted</dt>
          <dd className="mono">{d.blocklisted}</dd>
          <dt>fleet share</dt>
          <dd className="mono">
            {(provider.fleet_share * 100).toFixed(0)}% <em className="assume">assumed</em>
          </dd>
        </dl>
        <button type="button" className="linklike" onClick={() => chooseTab("vehicles")} data-testid="pki-open-vehicles">
          Open the vehicles
        </button>
        <RecentMessages id={`devices/${provider.label}`} log={log} view={view} />
      </div>
    </aside>
  );
}

/** A certificate's decode and validation trace. */
function CertCard({ id8 }: { readonly id8: string }): React.JSX.Element {
  const [cert, setCert] = useState<CertAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [tree, setTree] = useState(false);
  useEffect(() => {
    let live = true;
    setCert(null);
    setError(null);
    engine
      .request("inspect.cert", { hashedid8: id8 }, { quiet: true, timeoutMs: 8000 })
      .then((a) => live && setCert(certOf(a)))
      .catch((err: unknown) => live && setError(err instanceof Error ? err.message : String(err)));
    return () => {
      live = false;
    };
  }, [id8]);
  return (
    <section className="pki-card-section" data-testid="pki-cert">
      <h4>
        Certificate <span className="mono">{id8}</span>
        {cert ? <span className={cert.valid ? "pill ok" : "pill warn"}>{cert.valid ? "valid" : "invalid"}</span> : null}
      </h4>
      {error ? <p className="faint">The engine said: {error}</p> : null}
      {cert ? (
        <>
          <p className="dim mono" title="This certificate, its issuers, and the root's HashedId8">
            chain {cert.chain.join(" → ")}
          </p>
          <ol className="pki-trace" data-testid="pki-cert-trace">
            {cert.trace.map((s) => (
              <li key={s.rule} className={`st-${s.status}`}>
                <span className="mono">{s.rule}</span> <b>{s.status}</b>
                {s.detail.length > 0 ? <span className="dim"> — {s.detail.join("; ")}</span> : null}
              </li>
            ))}
          </ol>
          <button type="button" className="linklike" onClick={() => setTree((t) => !t)} aria-expanded={tree}>
            {tree ? "hide" : "show"} the decoded certificate
          </button>
          {tree ? <JerTree jer={cert.tree} testId="pki-cert-tree" /> : null}
        </>
      ) : null}
    </section>
  );
}

/** What one entity holds, as `view` may see it. */
export function KnowledgeCard({ entity, view }: { readonly entity: string; readonly view: string }): React.JSX.Element {
  const [k, setK] = useState<KnowledgeAnswer | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setError(null);
    engine
      .request("inspect.knowledge", { entity, view }, { quiet: true, timeoutMs: 8000 })
      .then((a) => live && setK(knowledgeOf(a)))
      .catch((err: unknown) => live && setError(err instanceof Error ? err.message : String(err)));
    return () => {
      live = false;
    };
  }, [entity, view]);
  const knows = k && Array.isArray(k.holds.knows) ? (k.holds.knows as unknown[]).map(String) : [];
  const counts = k ? Object.entries(k.holds).filter(([key, v]) => key !== "knows" && (typeof v === "number" || typeof v === "string")) : [];
  return (
    <section className="pki-card-section pki-knowledge" data-testid="pki-knowledge">
      <h4>
        What {entity} knows{" "}
        <span className="dim">{view === GROUND_TRUTH ? "(ground truth: with the join onto devices)" : "(its own view)"}</span>
      </h4>
      {error ? <p className="faint">The engine said: {error}</p> : null}
      {k ? (
        <>
          {knows.length > 0 ? (
            <ul className="pki-knows">
              {knows.map((x) => (
                <li key={x}>{x}</li>
              ))}
            </ul>
          ) : null}
          {counts.length > 0 ? (
            <dl className="pki-pairs">
              {counts.map(([key, v]) => (
                <div key={key} className="pki-pair">
                  <dt>{key.replace(/_/g, " ")}</dt>
                  <dd className="mono">{String(v)}</dd>
                </div>
              ))}
            </dl>
          ) : null}
          <p className="dim">Cannot see:</p>
          <ul className="pki-cannot" data-testid="pki-cannot-see">
            {k.cannot_see.map((x) => (
              <li key={x}>{x}</li>
            ))}
          </ul>
          {k.truth ? (
            <p className="dim" data-testid="pki-truth-join">
              Ground-truth join: {Object.keys(k.truth).join(", ")} (shown only in ground truth).
            </p>
          ) : null}
        </>
      ) : null}
    </section>
  );
}

function ComponentCard({
  c,
  provider,
  activity,
  view,
  crls,
  log,
  onClose,
}: {
  readonly c: PkiComponent;
  readonly provider: PkiProvider | null;
  readonly activity: PkiActivity | undefined;
  readonly view: string;
  readonly crls: TrustAnswer["state"]["crls"];
  readonly log: readonly TrustMessage[];
  readonly onClose: () => void;
}): React.JSX.Element {
  const ops = Object.entries(c.ops);
  const counters = Object.entries(c.counters);
  const mine = c.role === "crlg" ? crls.filter((h) => h.provider === c.provider) : [];
  const knows = view === GROUND_TRUTH || view === c.id;
  const info = roleInfo(c.role);
  return (
    <aside className="pki-drawer" data-testid="pki-card">
      <div className="pki-drawer-head">
        <h3 title={info.term}>
          {info.name}
          {c.instance > 0 ? ` ${c.instance + 1}` : ""} <span className="dim mono">{c.id}</span>
        </h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the card" data-testid="pki-card-close">
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        <p className="pki-does" data-testid="pki-card-does">
          {info.does}
        </p>
        <dl className="pki-pairs">
          <dt>role</dt>
          <dd>
            {info.term} <span className="faint mono">{c.role}</span>
          </dd>
          <dt>status</dt>
          <dd className={`mono status-${c.status}`} data-testid="pki-card-status">
            {c.status}
            {c.status === "offline" ? " (by design: brought online only to sign)" : c.status === "faulted" ? " (an injected outage)" : ""}
          </dd>
          {provider ? (
            <>
              <dt>region</dt>
              <dd>
                {provider.region.name} ({provider.region.metro}, {provider.region.lat_deg.toFixed(2)}°, {provider.region.lon_deg.toFixed(2)}°)
                {provider.region.assumption ? <em className="assume"> assumed</em> : null}
              </dd>
            </>
          ) : null}
          {c.node !== null ? (
            <>
              <dt>node</dt>
              <dd className="mono">{c.node}</dd>
            </>
          ) : null}
          <dt>traffic</dt>
          <dd className="mono">
            {c.traffic.sent} sent · {c.traffic.received} received · {formatBytes(c.traffic.bytes_out)} out · {formatBytes(c.traffic.bytes_in)} in
          </dd>
          <dt>last 2 s</dt>
          <dd className="mono" data-testid="pki-card-activity">
            {activity ? `${activity.messages} messages, ${formatBytes(activity.bytes)}` : "idle"}
          </dd>
          {c.queue ? (
            <>
              <dt>queue</dt>
              <dd className="mono">
                depth {c.queue.depth} · {c.queue.servers} servers · served {c.queue.served} · waited {formatSpan(c.queue.waited_ns)} · busy {formatSpan(c.queue.busy_ns)}
              </dd>
            </>
          ) : null}
        </dl>
        {ops.length > 0 ? (
          <section className="pki-card-section">
            <h4>Cryptographic operations</h4>
            <dl className="pki-pairs">
              {ops.map(([k, v]) => (
                <div key={k} className="pki-pair">
                  <dt className="mono">{k}</dt>
                  <dd className="mono">{v}</dd>
                </div>
              ))}
            </dl>
          </section>
        ) : null}
        {counters.length > 0 ? (
          <section className="pki-card-section">
            <h4>Counters</h4>
            <dl className="pki-pairs">
              {counters.map(([k, v]) => (
                <div key={k} className="pki-pair">
                  <dt>{k.replace(/_/g, " ")}</dt>
                  <dd className="mono">{typeof v === "object" ? JSON.stringify(v) : String(v)}</dd>
                </div>
              ))}
            </dl>
          </section>
        ) : null}
        {mine.length > 0 ? (
          <section className="pki-card-section">
            <h4>CRL heads</h4>
            <table className="pki-table">
              <thead>
                <tr>
                  <th>series</th>
                  <th>kind</th>
                  <th>version</th>
                  <th>entries</th>
                  <th>bytes</th>
                  <th>next CRL in</th>
                </tr>
              </thead>
              <tbody>
                {mine.map((h) => (
                  <tr key={h.id8}>
                    <td className="mono">{h.series}</td>
                    <td>{h.kind}</td>
                    <td className="mono">{h.version}</td>
                    <td className="mono">{h.entries}</td>
                    <td className="mono">{h.bytes}</td>
                    <td className={h.sla_s < 0 ? "mono warn" : "mono"}>{formatSpan(h.sla_s * 1e9)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ) : null}
        <RecentMessages id={c.id} log={log} view={view} />
        {c.cert ? <CertCard id8={c.cert} /> : <p className="faint">It holds no certificate of its own.</p>}
        {["ra", "aca", "la1", "la2", "ma", "eca"].includes(c.role) ? (
          knows ? (
            <KnowledgeCard entity={c.id} view={view} />
          ) : (
            <p className="faint" data-testid="pki-knowledge-withheld">
              What {c.id} holds is its own: switch the view to ground truth or to {c.id} to see it.
            </p>
          )
        ) : null}
      </div>
    </aside>
  );
}

/** The tab. */
export function TrustDomain({ trust, view }: { readonly trust: TrustAnswer; readonly view: string }): React.JSX.Element {
  const [selected, setSelected] = useState<string | null>(null);
  const [fresh, setFresh] = useState<TrustMessage[]>([]);
  const [log, setLog] = useState<TrustMessage[]>([]);
  const seen = useRef<Set<string>>(new Set());
  const s = trust.state;
  const index = useMemo(() => componentIndex(s), [s]);

  // Each answer's messages not seen before: the topology animates them, the cards list them.
  useEffect(() => {
    const novel = messagesOf(trust).filter((m) => !seen.current.has(m.id));
    if (novel.length === 0) return;
    for (const m of novel) seen.current.add(m.id);
    if (seen.current.size > 4 * LOG_CAP) seen.current = new Set([...seen.current].slice(-2 * LOG_CAP));
    setFresh(novel);
    setLog((old) => [...old, ...novel].slice(-LOG_CAP));
  }, [trust]);

  // Escape closes the card.
  useEffect(() => {
    if (selected === null) return undefined;
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape" && !e.defaultPrevented) {
        // Marked handled, so the shell's Escape (close the panel) leaves the panel open.
        e.preventDefault();
        setSelected(null);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [selected]);

  const chosen = selected !== null ? (index.get(selected) ?? null) : null;
  const provider = chosen ? (s.providers.find((p) => p.label === chosen.provider) ?? null) : null;
  const devicesOf = selected?.startsWith("devices/") ? (s.providers.find((p) => `devices/${p.label}` === selected) ?? null) : null;
  const drawer = chosen !== null || devicesOf !== null;
  const total = s.providers.reduce((n, p) => n + p.devices.devices, 0);
  return (
    <div className={drawer ? "pki-body with-drawer" : "pki-body"} data-testid="pki-trust">
      <div className="pki-main">
        <Topology trust={trust} messages={fresh} view={view} selected={selected} onSelect={setSelected} />
        <TrustList trust={trust} />
        <p className="pki-devices dim" data-testid="pki-devices">
          devices: {total} ({s.providers.map((p) => `${p.label.replace(/^provider-/, "")} ${p.devices.devices}`).join(" / ")}) · enrolled{" "}
          {s.providers.reduce((n, p) => n + p.devices.enrolled, 0)} · provisioned {s.providers.reduce((n, p) => n + p.devices.provisioned, 0)} · revoked{" "}
          {s.providers.reduce((n, p) => n + p.devices.revoked, 0)} · blocklisted {s.providers.reduce((n, p) => n + p.devices.blocklisted, 0)}
        </p>
        {view !== GROUND_TRUTH ? <KnowledgeCard entity={view} view={view} /> : null}
        <details className="pki-assumptions">
          <summary className="dim">Assumptions ({s.assumptions.length})</summary>
          <ul>
            {s.assumptions.map((a) => (
              <li key={a} className="dim">
                {a}
              </li>
            ))}
          </ul>
        </details>
      </div>
      {chosen ? (
        <ComponentCard c={chosen} provider={provider} activity={trust.activity[chosen.id]} view={view} crls={s.crls} log={log} onClose={() => setSelected(null)} />
      ) : devicesOf ? (
        <DevicesCard provider={devicesOf} log={log} view={view} onClose={() => setSelected(null)} />
      ) : null}
    </div>
  );
}
