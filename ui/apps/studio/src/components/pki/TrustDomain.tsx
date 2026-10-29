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
  counterConcept,
  counterText,
  crlScheduleShort,
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
import { openPanel } from "../../shell/route.js";
import { chooseTab, usePkiView } from "../pki-records/state.js";
import { certErrorText, certFacts, certStatus, grouped, windowWords } from "../../lib/pkiCert.js";
import { CertLink } from "./CertificateCard.js";
import { CertBadge } from "./CertStatus.js";
import { GlossText } from "./GlossText.js";
import { useCert } from "./useCert.js";
import { Topology } from "./Topology.js";
import { TrustChain } from "./TrustChain.js";

/** Messages the Overview remembers for the cards' "recent messages", newest last. */
const LOG_CAP = 600;
/** Messages a card lists. */
const CARD_MESSAGES = 8;

/** The trust list (CTL) in one line under the topology: version, roots, endorsements; history on demand. */
function TrustList({ trust }: { readonly trust: TrustAnswer }): React.JSX.Element {
  const s = trust.state;
  const ctl = ctlSummary(s);
  const [history, setHistory] = useState(false);
  return (
    <section className="pki-ctl-strip">
      <div className="pki-gov-row">
        <b title="The certificate trust list (CTL): the roots every device trusts. It changes only when a quorum of electors endorse the change.">Trust list</b>
        <span className="pki-ctl" data-testid="pki-ctl" title={`Roots (HashedId32): ${s.ctl.roots.map((r) => `${r.slice(0, 16)}…`).join(", ")} · ${s.ctl.bytes} B`}>
          v{ctl.sequence} · {ctl.roots} root{ctl.roots === 1 ? "" : "s"} · endorsed by {s.ctl.signers.length} of {s.ctl.electors} electors ({s.ctl.quorum} needed) · since {time32Date(s.ctl.effective)}
        </span>
        {s.ctl.pending > 0 ? (
          <span className="pill warn" title="A trust-list change the SCMS Manager decided, not yet in force">
            {s.ctl.pending} pending
          </span>
        ) : null}
        <span className="grow" />
        <button type="button" className="linklike" onClick={() => setHistory((h) => !h)} data-testid="pki-ctl-history-toggle" aria-expanded={history}>
          {history ? "▾" : "▸"} history ({s.ctl.history.length})
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
              <th>version</th>
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
          <dd>
            <b className="mono">{d.devices}</b>
            <span className="dim" title="No provider publishes its fleet share: this split is the scenario's setting">
              {" "}
              · {(provider.fleet_share * 100).toFixed(0)}% of the fleet (assumed)
            </span>
          </dd>
          <dt title="Hold an enrolment certificate (EC) from this provider's Enrolment CA">Enrolled</dt>
          <dd className="mono">{d.enrolled}</dd>
          <dt title="Have downloaded pseudonym certificates from the RA">With pseudonyms</dt>
          <dd className="mono">{d.provisioned}</dd>
          <dt title="The provider's own revocation counter. The strip's “devices revoked” counts the devices a published list names; the two differ while a revocation is on its way to the list.">
            Revoked
          </dt>
          <dd className="mono">{d.revoked}</dd>
          <dt title="The RA refuses their enrolment certificate after a revocation, so they get no new pseudonyms">Blocked</dt>
          <dd className="mono">{d.blocklisted}</dd>
        </dl>
        <button type="button" className="linklike" onClick={() => chooseTab("vehicles")} data-testid="pki-open-vehicles">
          Open the vehicles ›
        </button>
        <RecentMessages id={`devices/${provider.label}`} log={log} view={view} />
      </div>
    </aside>
  );
}

/** What one entity holds, as `view` may see it. */
export function KnowledgeCard({
  entity,
  view,
  skip = [],
  skipConcepts = [],
  bare = false,
}: {
  readonly entity: string;
  readonly view: string;
  /** Inside a disclosure that already names it: no heading, no box. */
  readonly bare?: boolean;
  readonly skip?: readonly string[];
  /** Concepts the card already counts (`counterConcept`): the record's second name for one is not repeated. */
  readonly skipConcepts?: readonly string[];
}): React.JSX.Element {
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
        ([key, v]) =>
          key !== "knows" &&
          (typeof v === "number" || typeof v === "string") &&
          !skip.includes(sameWords(key.replace(/_/g, " "))) &&
          !skip.includes(sameWords(counterText(key)?.label ?? "\u0000")) &&
          !skipConcepts.includes(counterConcept(key)),
      ).filter(([key], i, all) => all.findIndex(([other]) => counterConcept(other) === counterConcept(key)) === i)
    : [];
  return (
    <section className={bare ? "pki-knowledge bare" : "pki-card-section pki-knowledge"} data-testid="pki-knowledge">
      {bare ? null : (
        <h4>
          What {componentName(entity, true)} knows <span className="dim">{view === GROUND_TRUTH ? "(ground truth)" : "(its own view)"}</span>
        </h4>
      )}
      {error ? <p className="faint">The engine said: {error}</p> : null}
      {k ? (
        <>
          {knows.length > 0 ? (
            <ul className="pki-knows">
              {knows.map((x) => (
                <li key={x}>
                  <GlossText text={plainNames(x)} />
                </li>
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
              <li key={x}>
                <GlossText text={plainNames(x)} />
              </li>
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
  // One number per concept: a counter that repeats another under a second name stays raw only.
  const shownConcepts = new Set<string>();
  const readable = counters.flatMap(([k, v]) => {
    const t = counterText(k);
    if (!t || shownConcepts.has(counterConcept(k))) return [];
    shownConcepts.add(counterConcept(k));
    return [{ k, v, t }];
  });
  const mine = c.role === "crlg" ? state.crls.filter((h) => h.provider === c.provider) : [];
  const knows = view === GROUND_TRUTH || view === c.id;
  const info = roleInfo(c.role);
  // The Policy Generator reports the certificate it signs with as a counter, not as `cert`.
  const certId = c.cert ?? (typeof c.counters.certificate === "string" ? c.counters.certificate : null);
  const signedCtl = c.role === "elector" ? state.ctl.signers.includes(c.instance) : null;
  const due = mine.length > 0 ? Math.min(...mine.map((h) => h.sla_s)) : null;
  const ownsKnowledge = ["ra", "aca", "la1", "la2", "ma", "eca"].includes(c.role);
  const region = provider
    ? `${provider.region.name} (${provider.region.metro}, ${provider.region.lat_deg.toFixed(2)}°, ${provider.region.lon_deg.toFixed(2)}°)${provider.region.assumption ? ", assumed" : ""}`
    : null;
  return (
    <aside className="pki-drawer" data-testid="pki-card">
      <div className="pki-drawer-head">
        <h3 title={`${info.term} · ${c.id}${region ? ` · ${region}` : ""}`}>
          {componentName(c.id)} {c.provider !== "scms-manager" ? <span className="dim">· {c.provider}</span> : null}
        </h3>
        {c.status !== "online" ? (
          <span className={`pill status-${c.status}`} data-testid="pki-card-status" title={c.status === "offline" ? "By design: brought online only to sign" : c.status === "faulted" ? "An injected outage" : undefined}>
            {c.status}
          </span>
        ) : null}
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the card" data-testid="pki-card-close">
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        <p className="pki-does" data-testid="pki-card-does" title={info.term}>
          {info.does}
        </p>
        <dl className="pki-pairs">
          <dt>Certificate</dt>
          <dd data-testid="pki-card-cert-line">{certId ? <CertLine id8={certId} holder={c.id} /> : <span className="dim">{noCertText(c.role)}</span>}</dd>
          {signedCtl !== null ? (
            <>
              <dt>Trust list</dt>
              <dd data-testid="pki-card-elector-signed">
                {signedCtl ? "endorsed" : "did not endorse"} v{state.ctl.sequence}
              </dd>
            </>
          ) : null}
          {c.role === "crlg" ? (
            <>
              <dt>Publishing</dt>
              <dd data-testid="pki-card-crl-schedule" title={crlScheduleText({ cadenceS: cadence[c.provider] ?? null, publication: provider?.crl_publication ?? null, dueS: due })}>{crlScheduleShort({ cadenceS: cadence[c.provider] ?? null, publication: provider?.crl_publication ?? null, dueS: due })}</dd>
            </>
          ) : null}
          <dt>Messages</dt>
          <dd title={`${formatBytes(c.traffic.bytes_out)} out, ${formatBytes(c.traffic.bytes_in)} in`}>
            {c.traffic.sent.toLocaleString()} sent · {c.traffic.received.toLocaleString()} received
            <span className="dim" data-testid="pki-card-activity">
              {" "}
              · {activity ? `${activity.messages} in the last 2 s` : "idle now"}
            </span>
          </dd>
          {c.queue ? (
            <>
              <dt title="Requests waiting for one of its servers">Queue</dt>
              <dd>
                {c.queue.depth === 0 ? "empty" : `${c.queue.depth} waiting`}
                <span className="dim">
                  {" "}
                  · {c.queue.servers} server{c.queue.servers === 1 ? "" : "s"}
                  {c.queue.served > 0 ? ` · ${formatSpan(c.queue.waited_ns / c.queue.served)} average wait` : ""}
                </span>
              </dd>
            </>
          ) : null}
        </dl>
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
                    <td className={h.sla_s < 0 ? "mono warn" : "mono"}>{h.sla_s < 0 ? `overdue ${ageText(-h.sla_s)}` : ageText(h.sla_s)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </section>
        ) : null}
        <RecentMessages id={c.id} log={log} view={view} />
        {ownsKnowledge ? (
          knows ? (
            <details className="pki-more">
              <summary>What it knows</summary>
              <KnowledgeCard
                entity={c.id}
                view={view}
                bare
                skip={readable.map((r) => sameWords(r.t.label)).concat(counters.map(([k]) => sameWords(k.replace(/_/g, " "))))}
                skipConcepts={[...shownConcepts]}
              />
            </details>
          ) : (
            <p className="faint" data-testid="pki-knowledge-withheld">
              What it knows is its own: pick it in “Seen as” to see it.
            </p>
          )
        ) : null}
        {readable.length > 0 || ops.length > 0 || counters.length > 0 || c.node !== null ? (
          <details className="pki-more">
            <summary data-testid="pki-card-counters">Counters ({readable.length})</summary>
            <dl className="pki-pairs">
              {readable.map(({ k, v, t }) => (
                <div key={k} className="pki-pair" data-counter={k}>
                  <dt title={t.means}>{t.label}</dt>
                  <dd className="mono">{counterValue(v, t.unit)}</dd>
                </div>
              ))}
            </dl>
            <button type="button" className="linklike" aria-expanded={raw} onClick={() => setRaw((r) => !r)} data-testid="pki-card-raw-toggle">
              {raw ? "▾ Hide" : "▸ Show"} raw engine counters
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
          </details>
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
function CertLine({ id8, holder }: { readonly id8: string; readonly holder: string }): React.JSX.Element {
  const { cert, error } = useCert(id8);
  const facts = useMemo(() => (cert ? certFacts(cert.tree, null) : null), [cert]);
  const status = cert ? certStatus(cert, facts?.validity ?? null) : "unknown";
  const words = facts ? windowWords(facts.validity, cert?.time32 ?? null) : null;
  return (
    <span className="pki-certline">
      {cert ? <CertBadge status={status} compact /> : null}
      <CertLink id8={id8} holder={holder} label={grouped(id8)} testId="pki-card-cert-open" />
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
        <p className="pki-context dim" data-testid="pki-time" title={`Snapshot at simulated t = ${(trust.t_ns / 1e9).toFixed(0)} s. Click any box for what it does, its certificate and what it knows.`}>
          <span>
            <b title={`Deployment preset “${s.preset}”: which providers, policies and rules the run follows`}>{deploymentText(s.preset)}</b> ·{" "}
            <span title={`Certificate and message format: ${s.edition}`}>{editionText(s.edition)}</span> · {total} devices · click a box for details
          </span>
          <span className="grow" />
          <button type="button" className="linklike pki-metrics-link" onClick={() => openPanel("metrics")} data-testid="pki-open-metrics" title="The same system's numbers over the whole run, as charts">
            Charts in Metrics ›
          </button>
        </p>
        <Topology trust={trust} messages={fresh} view={view} selected={selected} onSelect={setSelected} />
        <TrustList trust={trust} />
        <TrustChain state={s} onSelect={setSelected} selected={selected} />
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
