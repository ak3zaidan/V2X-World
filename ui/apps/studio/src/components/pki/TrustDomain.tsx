/**
 * The Trust domain tab (00 §3.2): who exists. The SCMS Manager and the electors, the CTL that binds
 * the providers (its sequence, roots, quorum, signers and history), and each provider as a band of
 * its components — the offline chain (root, ICA) hatched with a lock, then the online authorities
 * in the order a request meets them — each with its status, a dot while it carried traffic in the
 * last two simulated seconds and a pill with its queue depth. Under the providers, the devices.
 *
 * Selecting a component opens its card on the right: what the engine publishes about it, and its
 * certificate decoded with the validation trace a device runs (`inspect.cert`). In an entity's view
 * the tab also shows that entity's own knowledge record (`inspect.knowledge`), which carries no
 * ground-truth join.
 */

import { useEffect, useMemo, useState } from "react";

import {
  certOf,
  componentClasses,
  componentIndex,
  ctlSummary,
  devicesLine,
  formatBytes,
  formatSpan,
  GROUND_TRUTH,
  knowledgeOf,
  providerLayout,
  roleLabel,
  time32Date,
  type CertAnswer,
  type KnowledgeAnswer,
  type PkiActivity,
  type PkiComponent,
  type PkiProvider,
  type TrustAnswer,
} from "../../lib/pki.js";
import { engine } from "../../state/engine.js";
import { CloseIcon } from "../../shell/Icons.js";
import { JerTree } from "./PduDecoder.js";

function LockIcon(): React.JSX.Element {
  return (
    <svg className="pki-lock" viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
      <rect x="2" y="5.5" width="8" height="5.5" rx="1" fill="currentColor" />
      <path d="M4 5.5V4a2 2 0 0 1 4 0v1.5" fill="none" stroke="currentColor" strokeWidth="1.3" />
    </svg>
  );
}

/** One component box. */
function Box({
  c,
  activity,
  selected,
  viewing,
  onSelect,
}: {
  readonly c: PkiComponent;
  readonly activity: Readonly<Record<string, PkiActivity>>;
  readonly selected: boolean;
  readonly viewing: boolean;
  readonly onSelect: (id: string) => void;
}): React.JSX.Element {
  const cls = componentClasses(c, activity);
  if (selected) cls.push("selected");
  if (viewing) cls.push("viewing");
  const a = activity[c.id];
  const title = `${c.name} (${c.id}) — ${c.status}${a ? ` · ${a.messages} messages in the last 2 s` : ""}${c.queue && c.queue.depth > 0 ? ` · queue ${c.queue.depth}` : ""}`;
  return (
    <button type="button" className={cls.join(" ")} onClick={() => onSelect(c.id)} title={title} data-testid={`pki-comp-${c.id}`} data-status={c.status}>
      {c.status === "offline" ? <LockIcon /> : <i className="pki-dot" aria-hidden="true" />}
      <span className="r">
        {roleLabel(c.role)}
        {c.instance > 0 ? <sub>{c.instance}</sub> : null}
      </span>
      {c.queue && c.queue.depth > 0 ? <span className="pki-q">q{c.queue.depth}</span> : null}
    </button>
  );
}

function ProviderBand({
  p,
  activity,
  selected,
  view,
  onSelect,
}: {
  readonly p: PkiProvider;
  readonly activity: Readonly<Record<string, PkiActivity>>;
  readonly selected: string | null;
  readonly view: string;
  readonly onSelect: (id: string) => void;
}): React.JSX.Element {
  const { offline, online } = providerLayout(p);
  const box = (c: PkiComponent): React.JSX.Element => <Box key={c.id} c={c} activity={activity} selected={selected === c.id} viewing={view === c.id} onSelect={onSelect} />;
  return (
    <section className="pki-provider" data-testid={`pki-provider-${p.label}`}>
      <header>
        <b className="mono">{p.label}</b>
        {p.home ? <span className="pill">home</span> : null}
        <span className={p.on_ctl ? "pill ok" : "pill warn"} title={p.on_ctl ? "Its root is on the CTL: devices trust its certificates" : "Its root is not on the CTL: devices reject its certificates"}>
          {p.on_ctl ? "on CTL" : "not on CTL"}
        </span>
        <span className="dim" title={p.region.assumption ? "No provider publishes where it runs: this region is an assumption (01 §2.3)" : undefined} data-testid={`pki-region-${p.label}`}>
          region {p.region.name} ({p.region.metro}){p.region.assumption ? <em className="assume"> assumed</em> : null}
        </span>
        <span className="dim">
          MA {p.ma_mode}
          {p.ma_review_s !== null ? ` (${formatSpan(p.ma_review_s * 1e9)} review)` : ""}
        </span>
        <span className="dim">CRL {p.crl_publication}</span>
        <span className="dim">fleet {(p.fleet_share * 100).toFixed(0)}%<em className="assume"> assumed</em></span>
      </header>
      <div className="pki-chain">
        <div className="pki-offline" title="Offline by design: air-gapped, brought online only to sign">
          {offline.map((c, i) => (
            <span key={c.id} className="pki-chain-step">
              {i > 0 ? <span className="pki-arrow">→</span> : null}
              {box(c)}
            </span>
          ))}
        </div>
        {offline.length > 0 ? <span className="pki-arrow">→</span> : null}
        <div className="pki-online">{online.map(box)}</div>
      </div>
      <footer className="dim">
        devices {p.devices.devices} · enrolled {p.devices.enrolled} · provisioned {p.devices.provisioned} · revoked {p.devices.revoked} · blocklisted {p.devices.blocklisted}
      </footer>
    </section>
  );
}

function Governance({
  trust,
  selected,
  onSelect,
}: {
  readonly trust: TrustAnswer;
  readonly selected: string | null;
  readonly onSelect: (id: string) => void;
}): React.JSX.Element {
  const s = trust.state;
  const ctl = ctlSummary(s);
  const manager = s.governance.filter((c) => c.role === "manager");
  const electors = s.governance.filter((c) => c.role === "elector");
  const signers = new Set(s.ctl.signers);
  const [history, setHistory] = useState(false);
  return (
    <section className="pki-governance" data-testid="pki-governance">
      <div className="pki-gov-row">
        {manager.map((c) => (
          <Box key={c.id} c={c} activity={trust.activity} selected={selected === c.id} viewing={false} onSelect={onSelect} />
        ))}
        <span className="pki-electors" title="The electors endorse each CTL; offline, they sign ballots only when a CTL changes">
          <span className="dim">electors</span>
          {electors.map((c) => (
            <span key={c.id} className={signers.has(c.instance) ? "signed" : "unsigned"} title={signers.has(c.instance) ? "endorsed the current CTL" : "did not endorse the current CTL"}>
              <Box c={c} activity={trust.activity} selected={selected === c.id} viewing={false} onSelect={onSelect} />
            </span>
          ))}
          <span className="dim mono">{ctl.quorum}</span>
        </span>
        <span className="grow" />
        <span className="pki-ctl mono" data-testid="pki-ctl">
          CTL seq {ctl.sequence} · {ctl.roots} root{ctl.roots === 1 ? "" : "s"} · {s.ctl.bytes} B · effective {time32Date(s.ctl.effective)}
          {s.ctl.pending > 0 ? <span className="pill warn"> {s.ctl.pending} pending</span> : null}
        </span>
        <button type="button" className="linklike" onClick={() => setHistory((h) => !h)} data-testid="pki-ctl-history-toggle" aria-expanded={history}>
          history ({s.ctl.history.length})
        </button>
      </div>
      {ctl.offCtl.length > 0 ? (
        <p className="warn" data-testid="pki-off-ctl">
          Not on the CTL: {ctl.offCtl.join(", ")} — devices reject its certificates.
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
                <td className="mono">{h.signers.join(", ")}</td>
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
  onClose,
}: {
  readonly c: PkiComponent;
  readonly provider: PkiProvider | null;
  readonly activity: PkiActivity | undefined;
  readonly view: string;
  readonly crls: TrustAnswer["state"]["crls"];
  readonly onClose: () => void;
}): React.JSX.Element {
  const ops = Object.entries(c.ops);
  const counters = Object.entries(c.counters);
  const mine = c.role === "crlg" ? crls.filter((h) => h.provider === c.provider) : [];
  const knows = view === GROUND_TRUTH || view === c.id;
  return (
    <aside className="pki-drawer" data-testid="pki-card">
      <div className="pki-drawer-head">
        <h3>
          {c.name} <span className="dim mono">{c.id}</span>
        </h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={onClose} aria-label="Close the card" data-testid="pki-card-close">
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        <dl className="pki-pairs">
          <dt>role</dt>
          <dd className="mono">{c.role}</dd>
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
  const s = trust.state;
  const index = useMemo(() => componentIndex(s), [s]);
  const devices = devicesLine(s);
  const chosen = selected !== null ? (index.get(selected) ?? null) : null;
  const provider = chosen ? (s.providers.find((p) => p.label === chosen.provider) ?? null) : null;
  return (
    <div className={chosen ? "pki-body with-drawer" : "pki-body"} data-testid="pki-trust">
      <div className="pki-main">
        <Governance trust={trust} selected={selected} onSelect={setSelected} />
        {s.providers.map((p) => (
          <ProviderBand key={p.label} p={p} activity={trust.activity} selected={selected} view={view} onSelect={setSelected} />
        ))}
        <p className="pki-devices" data-testid="pki-devices">
          devices: {devices.total.devices} ({devices.perProvider.map((d) => `${d.label.replace(/^provider-/, "")} ${d.devices}`).join(" / ")}) · enrolled {devices.total.enrolled} · provisioned{" "}
          {devices.total.provisioned} · revoked {devices.total.revoked} · blocklisted {devices.total.blocklisted}
        </p>
        <p className="pki-legend dim">
          <span className="pki-key offline">
            <LockIcon /> offline (hatched)
          </span>
          <span className="pki-key busy">
            <i className="pki-dot" /> traffic in the last {trust.window_ns !== null ? formatSpan(trust.window_ns) : "2 s"}
          </span>
          <span className="pki-key faulted">faulted (outage)</span>
          <span>q = queue depth</span>
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
      {chosen ? <ComponentCard c={chosen} provider={provider} activity={trust.activity[chosen.id]} view={view} crls={s.crls} onClose={() => setSelected(null)} /> : null}
    </div>
  );
}
