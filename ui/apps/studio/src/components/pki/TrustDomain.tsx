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
  componentIndex,
  ctlSummary,
  formatBytes,
  formatSimT,
  formatSpan,
  GROUND_TRUTH,
  knowledgeOf,
  time32Date,
  type KnowledgeAnswer,
  type PkiActivity,
  type PkiComponent,
  type PkiProvider,
  type TrustAnswer,
} from "../../lib/pki.js";
import {
  ageText,
  componentName,
  counterText,
  crlScheduleText,
  deploymentText,
  editionText,
  flowInfo,
  groupMessages,
  isDevice,
  messagesOf,
  messageText,
  observedBy,
  plainNames,
  roleInfo,
  type TrustMessage,
} from "../../lib/pkiTopology.js";
import { engine } from "../../state/engine.js";
import { CloseIcon } from "../../shell/Icons.js";
import { chooseTab, usePkiView } from "../pki-records/state.js";
import { certErrorText, certFacts, certStatus, grouped, windowWords } from "../../lib/pkiCert.js";
import { CertificateCard } from "./CertificateCard.js";
import { CertBadge } from "./CertStatus.js";
import { useCert } from "./useCert.js";
import { Topology } from "./Topology.js";
import { TrustChain } from "./TrustChain.js";

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
      <p className="pki-ctl-says">
        The <b>trust list (CTL)</b> names the {ctl.roots === 1 ? "root" : `${ctl.roots} roots`} every device trusts. It changes only when {s.ctl.quorum} of the {s.ctl.electors} electors endorse the change; this version is endorsed
        by {s.ctl.signers.length}.
      </p>
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
  const provider = end.split("/")[0];
  return componentName(end, !isDevice(end) && provider !== home);
}

/** The messages one node sent or received, newest first, as the view may see them. */
function RecentMessages({ id, log, view }: { readonly id: string; readonly log: readonly TrustMessage[]; readonly view: string }): React.JSX.Element {
  const devices = id.startsWith("devices/") ? id.slice("devices/".length) : null;
  const home = devices ?? id.split("/")[0];
  const touches = (m: TrustMessage): boolean =>
    devices !== null ? (isDevice(m.from) || isDevice(m.to)) && [m.from, m.to].some((e) => e !== null && e.startsWith(`${devices}/`)) : m.from === id || m.to === id;
  const mine = groupMessages(log.filter((m) => observedBy(m, view) && touches(m)).reverse()).slice(0, CARD_MESSAGES);
  return (
    <section className="pki-card-section" data-testid="pki-card-messages">
      <h4>Recent messages</h4>
      {mine.length === 0 ? (
        <p className="faint">None since the view opened{view === GROUND_TRUTH ? "" : " that this view observed"}.</p>
      ) : (
        <ul className="pki-recent">
          {mine.map(({ first: m, count, span, bytes }) => {
            const out = devices !== null ? isDevice(m.from) : m.from === id;
            return (
              <li key={m.id} title={`${m.instance} · ${m.from ?? "?"} → ${m.to ?? "?"}`} data-testid="pki-recent-row" data-count={count}>
                <i className="pki-swatch" style={{ background: flowInfo(m.flow).color }} />
                <span className="mono faint">{formatSimT(m.t)}</span>
                {count > 1 ? <b className="mono">{count} ×</b> : null}
                <span>{messageText(m.name)}</span>
                <span>
                  {out ? "to" : "from"} {endText(out ? m.to : m.from, home)}
                </span>
                {count > 1 ? <span className="faint">within {formatSpan(span)}</span> : null}
                {m.bytes !== null ? <span className="faint mono">{formatBytes(count > 1 ? bytes : m.bytes)}</span> : null}
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
          {info.name} <span className="dim">· {provider.label}</span>
        </h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the card" data-testid="pki-card-close">
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        <p className="pki-does">{info.does}</p>
        <dl className="pki-pairs">
          <dt>Devices</dt>
          <dd className="mono">{d.devices}</dd>
          <dt title="Hold an enrolment certificate (EC) from this provider's Enrolment CA">Enrolled</dt>
          <dd className="mono">{d.enrolled}</dd>
          <dt title="Provisioned: have downloaded pseudonym certificates from the RA">With pseudonym certificates</dt>
          <dd className="mono">{d.provisioned}</dd>
          <dt title="Named by a revocation list after a misbehaviour case">Revoked</dt>
          <dd className="mono">{d.revoked}</dd>
          <dt title="Blocklisted: the RA refuses their enrolment certificate after a revocation, so they get no new pseudonyms">Blocked from new certificates</dt>
          <dd className="mono">{d.blocklisted}</dd>
          <dt>Share of the fleet</dt>
          <dd className="mono">
            {(provider.fleet_share * 100).toFixed(0)}%{" "}
            <em className="assume" title="No provider publishes its fleet share: this split is the scenario's setting">
              assumed
            </em>
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

/** What one entity holds, as `view` may see it. */
export function KnowledgeCard({ entity, view, skip = [] }: { readonly entity: string; readonly view: string; readonly skip?: readonly string[] }): React.JSX.Element {
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
  // A count the component's card already lists (its "cases open" is the record's "open cases") is not repeated.
  const counts = k
    ? Object.entries(k.holds).filter(
        ([key, v]) => key !== "knows" && (typeof v === "number" || typeof v === "string") && !skip.includes(sameWords(key.replace(/_/g, " "))) && !skip.includes(sameWords(counterText(key)?.label ?? "\u0000")),
      )
    : [];
  return (
    <section className="pki-card-section pki-knowledge" data-testid="pki-knowledge">
      <h4>
        What {componentName(entity, true)} knows{" "}
        <span className="dim">{view === GROUND_TRUTH ? "(seen as everything: with which vehicle each record belongs to)" : "(its own view)"}</span>
      </h4>
      {error ? <p className="faint">The engine said: {error}</p> : null}
      {k ? (
        <>
          {knows.length > 0 ? (
            <ul className="pki-knows">
              {knows.map((x) => (
                <li key={x}>{plainNames(x)}</li>
              ))}
            </ul>
          ) : null}
          {counts.length > 0 ? (
            <dl className="pki-pairs">
              {counts.map(([key, v]) => (
                <div key={key} className="pki-pair">
                  <dt title={counterText(key)?.means}>{counterText(key)?.label ?? key.replace(/_/g, " ")}</dt>
                  <dd className="mono">{String(v)}</dd>
                </div>
              ))}
            </dl>
          ) : null}
          <p className="dim">Cannot see:</p>
          <ul className="pki-cannot" data-testid="pki-cannot-see">
            {k.cannot_see.map((x) => (
              <li key={x}>{plainNames(x)}</li>
            ))}
          </ul>
          {k.truth ? (
            <p className="dim" data-testid="pki-truth-join">
              Only ground truth links these to vehicles: {Object.keys(k.truth).map((x) => x.replace(/_/g, " ")).join(", ")}.
            </p>
          ) : null}
        </>
      ) : null}
    </section>
  );
}

/** A counter's value in its unit. */
function counterValue(v: unknown, unit: "s" | undefined): string {
  if (typeof v === "number" && unit === "s") return ageText(v);
  if (typeof v === "number") return v.toLocaleString();
  if (typeof v === "boolean") return v ? "yes" : "no";
  return typeof v === "object" ? JSON.stringify(v) : String(v);
}

/** Words of a counter, compared without order ("cases open" is "open cases"). */
function sameWords(a: string): string {
  return a.toLowerCase().replace(/[^a-z ]/g, " ").split(/\s+/).filter(Boolean).sort().join(" ");
}

function ComponentCard({
  c,
  provider,
  activity,
  view,
  state,
  log,
  onClose,
}: {
  readonly c: PkiComponent;
  readonly provider: PkiProvider | null;
  readonly activity: PkiActivity | undefined;
  readonly view: string;
  readonly state: TrustAnswer["state"];
  readonly log: readonly TrustMessage[];
  readonly onClose: () => void;
}): React.JSX.Element {
  const [raw, setRaw] = useState(false);
  const cadence = usePkiView((s) => s.crlCadence);
  const ops = Object.entries(c.ops);
  const counters = Object.entries(c.counters);
  const readable = counters.flatMap(([k, v]) => {
    const t = counterText(k);
    return t ? [{ k, v, t }] : [];
  });
  const mine = c.role === "crlg" ? state.crls.filter((h) => h.provider === c.provider) : [];
  const knows = view === GROUND_TRUTH || view === c.id;
  const info = roleInfo(c.role);
  // The Policy Generator reports the certificate it signs with as a counter, not as `cert`.
  const certId = c.cert ?? (typeof c.counters.certificate === "string" ? c.counters.certificate : null);
  const signedCtl = c.role === "elector" ? state.ctl.signers.includes(c.instance) : null;
  const due = mine.length > 0 ? Math.min(...mine.map((h) => h.sla_s)) : null;
  const ownsKnowledge = ["ra", "aca", "la1", "la2", "ma", "eca"].includes(c.role);
  return (
    <aside className="pki-drawer" data-testid="pki-card">
      <div className="pki-drawer-head">
        <h3 title={`${info.term} · ${c.id}`}>
          {componentName(c.id)} {c.provider !== "scms-manager" ? <span className="dim">· {c.provider}</span> : null}
        </h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the card" data-testid="pki-card-close">
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        {/* 1. What it is for, in one sentence, and its standard name. */}
        <p className="pki-does" data-testid="pki-card-does">
          {info.does}
        </p>
        <dl className="pki-pairs">
          <dt>Standard name</dt>
          <dd>{info.term}</dd>
          <dt>Status</dt>
          <dd className={`status-${c.status}`} data-testid="pki-card-status">
            {c.status}
            {c.status === "offline" ? " (by design: brought online only to sign)" : c.status === "faulted" ? " (an injected outage)" : ""}
          </dd>
          {/* 2. Its certificate in one line; the full card is further down. */}
          <dt>Its certificate</dt>
          <dd data-testid="pki-card-cert-line">{certId ? <CertLine id8={certId} /> : noCertText(c.role)}</dd>
          {signedCtl !== null ? (
            <>
              <dt>Trust list</dt>
              <dd data-testid="pki-card-elector-signed">
                {signedCtl ? "endorsed" : "did not endorse"} the trust list in force (CTL version {state.ctl.sequence}, endorsed by {state.ctl.signers.length} of{" "}
                {state.ctl.electors} electors)
              </dd>
            </>
          ) : null}
          {c.role === "crlg" ? (
            <>
              <dt>Publishing</dt>
              <dd data-testid="pki-card-crl-schedule">{crlScheduleText({ cadenceS: cadence[c.provider] ?? null, publication: provider?.crl_publication ?? null, dueS: due })}</dd>
            </>
          ) : null}
        </dl>
        {/* 3. What it knows, and what it cannot see. */}
        {ownsKnowledge ? (
          knows ? (
            <KnowledgeCard entity={c.id} view={view} skip={readable.map((r) => sameWords(r.t.label)).concat(counters.map(([k]) => sameWords(k.replace(/_/g, " "))))} />
          ) : (
            <p className="faint" data-testid="pki-knowledge-withheld">
              What {componentName(c.id, true)} holds is its own: choose it, or everything (ground truth), in “Seen as” to see it.
            </p>
          )
        ) : null}
        {/* 4. Its activity. */}
        <section className="pki-card-section">
          <h4>Activity</h4>
          <dl className="pki-pairs">
            {provider ? (
              <>
                <dt>Region</dt>
                <dd>
                  {provider.region.name} ({provider.region.metro}, {provider.region.lat_deg.toFixed(2)}°, {provider.region.lon_deg.toFixed(2)}°)
                  {provider.region.assumption ? (
                    <em className="assume" title="No provider publishes where its data centres are: this region is an assumption">
                      {" "}
                      assumed
                    </em>
                  ) : null}
                </dd>
              </>
            ) : null}
            <dt>Messages</dt>
            <dd>
              {c.traffic.sent.toLocaleString()} sent, {c.traffic.received.toLocaleString()} received{" "}
              <span className="dim">
                ({formatBytes(c.traffic.bytes_out)} out, {formatBytes(c.traffic.bytes_in)} in)
              </span>
            </dd>
            <dt title="The last 2 s of simulated time">Last 2 s</dt>
            <dd data-testid="pki-card-activity">{activity ? `${activity.messages} message${activity.messages === 1 ? "" : "s"}, ${formatBytes(activity.bytes)}` : "idle"}</dd>
            {c.queue ? (
              <>
                <dt title="Requests waiting for one of its servers">Work queue</dt>
                <dd>
                  {c.queue.depth === 0 ? "nothing waiting" : `${c.queue.depth} waiting`}
                  <span className="dim">
                    {" "}
                    · {c.queue.servers} server{c.queue.servers === 1 ? "" : "s"} · {c.queue.served.toLocaleString()} served
                    {c.queue.served > 0 ? ` · average wait ${formatSpan(c.queue.waited_ns / c.queue.served)}` : ""}
                  </span>
                </dd>
              </>
            ) : null}
            {readable.map(({ k, v, t }) => (
              <div key={k} className="pki-pair" data-counter={k}>
                <dt title={t.means}>{t.label}</dt>
                <dd className="mono">{counterValue(v, t.unit)}</dd>
              </div>
            ))}
          </dl>
        </section>
        {mine.length > 0 ? (
          <section className="pki-card-section">
            <h4>Its revocation lists</h4>
            <table className="pki-table">
              <thead>
                <tr>
                  <th title="crlSeries: which family of certificates the list covers">Series</th>
                  <th>Revokes</th>
                  <th>Version</th>
                  <th>Entries</th>
                  <th>Size</th>
                  <th title="nextCrl: the deadline by which a newer list must replace it">Replace within</th>
                </tr>
              </thead>
              <tbody>
                {mine.map((h) => (
                  <tr key={h.id8}>
                    <td className="mono">{h.series}</td>
                    <td>{h.kind === "linkage" ? "pseudonyms" : h.kind === "hash" ? "single certificates" : h.kind}</td>
                    <td className="mono">{h.version}</td>
                    <td className="mono">{h.entries}</td>
                    <td className="mono">{formatBytes(h.bytes)}</td>
                    <td className={h.sla_s < 0 ? "mono warn" : "mono"}>{h.sla_s < 0 ? `overdue ${ageText(-h.sla_s)}` : ageText(h.sla_s)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ) : null}
        <RecentMessages id={c.id} log={log} view={view} />
        {certId ? (
          <section className="pki-card-section">
            <h4>The certificate it signs with</h4>
            <CertificateCard id8={certId} holder={c.id} />
          </section>
        ) : null}
        {ops.length > 0 || counters.length > 0 || c.queue || c.node !== null ? (
          <section className="pki-card-section">
            <button type="button" className="linklike" aria-expanded={raw} onClick={() => setRaw((r) => !r)} data-testid="pki-card-raw-toggle">
              {raw ? "▾ Hide" : "▸ Show"} engine counters (raw, as the engine reports them)
            </button>
            {raw ? (
              <dl className="pki-pairs pki-raw" data-testid="pki-card-raw">
                <div className="pki-pair">
                  <dt className="mono">id</dt>
                  <dd className="mono">{c.id}</dd>
                </div>
                {c.node !== null ? (
                  <div className="pki-pair">
                    <dt className="mono">node</dt>
                    <dd className="mono">{c.node}</dd>
                  </div>
                ) : null}
                {c.queue ? (
                  <div className="pki-pair">
                    <dt className="mono">queue</dt>
                    <dd className="mono">
                      depth {c.queue.depth} · servers {c.queue.servers} · served {c.queue.served} · waited {formatSpan(c.queue.waited_ns)} · busy {formatSpan(c.queue.busy_ns)}
                    </dd>
                  </div>
                ) : null}
                {counters.map(([k, v]) => (
                  <div key={k} className="pki-pair">
                    <dt className="mono">{k}</dt>
                    <dd className="mono">{typeof v === "object" ? JSON.stringify(v) : String(v)}</dd>
                  </div>
                ))}
                {ops.map(([k, v]) => (
                  <div key={`op-${k}`} className="pki-pair">
                    <dt className="mono" title="Cryptographic operations, as the engine's cost model counts them">
                      crypto: {k}
                    </dt>
                    <dd className="mono">{v}</dd>
                  </div>
                ))}
              </dl>
            ) : null}
          </section>
        ) : null}
      </div>
    </aside>
  );
}

/** Why a component has no certificate of its own, by role. */
function noCertText(role: string): string {
  if (role === "crl-store" || role === "crl-broadcast") return "none of its own: it passes on lists the CRL Generator signed";
  if (role === "manager") return "none in this run: the SCMS Manager sets policy and decides trust-list changes, which the electors sign";
  if (role === "lop") return "none of its own: it only forwards requests, hiding the vehicle's network address";
  return "none of its own in this run";
}

/** A certificate's state in one line: the shared badge, its id, and how long it has left. */
function CertLine({ id8 }: { readonly id8: string }): React.JSX.Element {
  const { cert, error } = useCert(id8);
  const facts = useMemo(() => (cert ? certFacts(cert.tree, null) : null), [cert]);
  const status = cert ? certStatus(cert, facts?.validity ?? null) : "unknown";
  const words = facts ? windowWords(facts.validity, cert?.time32 ?? null) : null;
  return (
    <span className="pki-certline">
      {cert ? <CertBadge status={status} compact /> : null}
      <span className="mono">{grouped(id8)}</span>
      {error ? <span className="faint"> · {certErrorText(error)}</span> : words ? <span className="dim"> · {words}</span> : !cert ? <span className="faint"> · checking…</span> : null}
    </span>
  );
}

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
      // A certificate opened over the card closes first (its drawer listens for the same key).
      if (e.key === "Escape" && !e.defaultPrevented && usePkiView.getState().cert === null) {
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
        <p className="pki-lead" data-testid="pki-overview-lead">
          Who runs the certificate system and what is happening now. Each provider runs its own authorities; the dots are messages travelling
          between them as the run goes. <b>Click any box</b> for what it does, its certificate and what it knows.
        </p>
        <p className="pki-context dim" data-testid="pki-time" title={`Snapshot at simulated t = ${(trust.t_ns / 1e9).toFixed(0)} s`}>
          <span title={`Deployment preset “${s.preset}”: which providers, policies and rules the run follows`}>
            Deployment: <b>{deploymentText(s.preset)}</b>
          </span>{" "}
          ·{" "}
          <span title={`Certificate and message format: ${s.edition}`}>
            Certificate standard: <b>{editionText(s.edition)}</b>
          </span>
        </p>
        <Topology trust={trust} messages={fresh} view={view} selected={selected} onSelect={setSelected} />
        <TrustChain state={s} onSelect={setSelected} selected={selected} />
        <TrustList trust={trust} />
        <p className="pki-devices dim" data-testid="pki-devices" title="Vehicles and roadside units, all providers: how many hold an enrolment certificate, how many have pseudonym certificates, how many are revoked, and how many enrolment certificates their RA refuses">
          Vehicles and roadside units: <b>{total}</b> ({s.providers.map((p) => `${p.label} ${p.devices.devices}`).join(", ")}) · {s.providers.reduce((n, p) => n + p.devices.enrolled, 0)} enrolled ·{" "}
          {s.providers.reduce((n, p) => n + p.devices.provisioned, 0)} with pseudonym certificates · {s.providers.reduce((n, p) => n + p.devices.revoked, 0)} revoked ·{" "}
          {s.providers.reduce((n, p) => n + p.devices.blocklisted, 0)} blocked from new certificates
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
        <ComponentCard c={chosen} provider={provider} activity={trust.activity[chosen.id]} view={view} state={s} log={log} onClose={() => setSelected(null)} />
      ) : devicesOf ? (
        <DevicesCard provider={devicesOf} log={log} view={view} onClose={() => setSelected(null)} />
      ) : null}
    </div>
  );
}
