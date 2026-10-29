/**
 * One certificate, readable at a glance (`inspect.cert`): its state as a badge, what kind of
 * certificate it is in plain words with the standard term, its validity window as a bar with
 * "now" on it, what it may sign, where, its linkage and revocation series, and the chain up to the
 * root — each link named by its holder and itself a link to that certificate. Under it, the checks
 * a receiver runs, step by step, with the reason for any that failed; the raw decoded certificate
 * (JER) is one click away.
 *
 * The verdict and every reason are the engine's (`v2xw_sec` validation); `lib/pkiCert.ts` adds the
 * wording and the window's arithmetic.
 */

import { useEffect, useMemo, useState } from "react";

import { time32Date } from "../../lib/pki.js";
import {
  CERT_STATUS,
  certErrorText,
  certFacts,
  deviceName,
  certStatus,
  chainLinks,
  grouped,
  ruleText,
  whyNot,
  whySentence,
  windowFraction,
  windowWords,
  type ChainLink,
} from "../../lib/pkiCert.js";
import { roleInfo } from "../../lib/pkiTopology.js";
import { Term } from "./Term.js";
import { CloseIcon } from "../../shell/Icons.js";
import { usePkiView } from "../pki-records/state.js";
import { CertBadge, StatusGlyph } from "./CertStatus.js";
import { JerTree } from "./PduDecoder.js";
import { useCert } from "./useCert.js";

/** A HashedId8 that opens its certificate's card. */
export function CertLink({ id8, holder = null, label, testId }: { readonly id8: string; readonly holder?: string | null; readonly label?: string; readonly testId?: string }): React.JSX.Element {
  const set = usePkiView((s) => s.set);
  return (
    <button
      type="button"
      className="linklike mono pki-certlink"
      onClick={() => set({ cert: { id8, holder } })}
      title={`Open certificate ${id8}: what it is, its validity, permissions and chain of trust`}
      data-testid={testId}
    >
      {label ?? id8}
    </button>
  );
}



function holderText(holder: string | null): string | null {
  if (!holder) return null;
  if (holder.startsWith("ee/") || holder.startsWith("rsu/")) return deviceName(holder).replace(/^V/, "v").replace(/^R/, "r");
  const [provider, role, inst] = holder.split("/");
  const info = roleInfo(role ?? holder);
  return `${info.name}${Number(inst) > 0 ? ` ${Number(inst) + 1}` : ""} of ${provider}`;
}

function shortTerm(role: string): string {
  const m = /\(([^)]+)\)/.exec(roleInfo(role).term);
  return m ? m[1].split(",")[0] : roleInfo(role).term;
}

function ValidityBar({ start, end, now, status }: { readonly start: number; readonly end: number; readonly now: number | null; readonly status: string }): React.JSX.Element {
  const f = windowFraction({ start, end, stated: "" }, now);
  return (
    <div className={`pki-validity st-${status}`} data-testid="pki-cert-validity">
      <div className="pki-validity-bar" role="img" aria-label={f === null ? "validity window" : `${Math.round(f * 100)}% of its validity window has passed`}>
        <i className="used" style={{ width: `${((f ?? 0) * 100).toFixed(1)}%` }} />
        {f !== null ? <b className="now" style={{ left: `${(f * 100).toFixed(1)}%` }} title="now" /> : null}
      </div>
      <div className="pki-validity-ends mono">
        <span>{time32Date(start)}</span>
        <span>{time32Date(end)}</span>
      </div>
    </div>
  );
}

function Chain({ links }: { readonly links: readonly ChainLink[] }): React.JSX.Element {
  return (
    <ol className="pki-chainlist" data-testid="pki-cert-chain">
      {links.map((l, k) => (
        <li key={l.id8} className={k === 0 ? "leaf" : k === links.length - 1 ? "root" : undefined}>
          <span className="pki-chain-node" aria-hidden="true" />
          <span className="pki-chain-text">
            <b>{k === 0 ? "This certificate" : l.name}</b>
            {l.role ? <span className="dim"> ({shortTerm(l.role)})</span> : null}
            {l.holder && k > 0 ? <span className="dim"> · {l.holder.split("/")[0]}</span> : null}
            {k > 0 ? <CertLink id8={l.id8} holder={l.holder} label={grouped(l.id8)} /> : <span className="mono faint"> {grouped(l.id8)}</span>}
            {l.onCtl === true ? (
              <span className="pki-badge st-valid" style={{ color: "var(--ok)" }} title="Its root is on the certificate trust list (CTL): every device trusts it">
                <StatusGlyph shape="check" /> on the trust list
              </span>
            ) : l.onCtl === false ? (
              <span className="pki-badge st-untrusted" style={{ color: "var(--pki-untrusted)" }} title="The certificate trust list (CTL) does not name this root">
                <StatusGlyph shape="shield" /> not on the trust list
              </span>
            ) : null}
          </span>
          {k < links.length - 1 ? <span className="pki-chain-by faint">signed by</span> : null}
        </li>
      ))}
    </ol>
  );
}

/**
 * The card. `holder` is who holds the certificate (`provider-a/aca/0`, `ee/1000000`) when the
 * caller knows; `heading` replaces the card's own title row when it sits inside another card.
 */
export function CertificateCard({
  id8,
  holder = null,
  heading = true,
  testId = "pki-cert",
}: {
  readonly id8: string;
  readonly holder?: string | null;
  readonly heading?: boolean;
  readonly testId?: string;
}): React.JSX.Element {
  const { cert, error } = useCert(id8);
  const owners = usePkiView((s) => s.owners);
  const roots = usePkiView((s) => s.ctlRoots);
  const [raw, setRaw] = useState(false);
  const known = holder ?? owners[id8.toLowerCase()] ?? null;
  const role = known && !known.startsWith("ee/") ? (known.split("/")[1] ?? null) : null;
  const facts = useMemo(() => (cert ? certFacts(cert.tree, role) : null), [cert, role]);
  const status = cert ? certStatus(cert, facts?.validity ?? null) : "unknown";
  const links = useMemo(() => (cert ? chainLinks(cert.chain, owners, roots, (r) => roleInfo(r).name, known) : []), [cert, owners, roots, known]);
  const passed = cert ? cert.trace.filter((s) => s.status === "pass").length : 0;
  const failed = cert ? cert.trace.filter((s) => s.status === "fail") : [];
  const [checks, setChecks] = useState<boolean | null>(null);
  const showChecks = checks ?? failed.length > 0;
  const why = cert ? whySentence(cert, facts?.validity ?? null) : null;
  const engineWhy = cert ? whyNot(cert) : null;
  const words = facts ? windowWords(facts.validity, cert?.time32 ?? null) : null;
  const issuerHolder = facts && facts.issuer !== "self" ? (owners[facts.issuer] ?? null) : null;
  return (
    <section className={`pki-certcard st-${status}`} data-testid={testId} data-status={status}>
      {heading ? (
        <header className="pki-certcard-head">
          <CertBadge status={status} testId="pki-cert-status" />
          <h4>
            {facts ? facts.kind.name : "Certificate"}
            {facts ? <span className="dim"> ({facts.kind.short})</span> : null}
          </h4>
        </header>
      ) : (
        <CertBadge status={status} testId="pki-cert-status" />
      )}
      <p className="pki-certcard-id">
        <span className="mono" title="HashedId8: the low 8 bytes of the certificate's SHA-256, how messages and lists refer to it">
          {grouped(id8)}
        </span>
        {holderText(known) ? <span className="dim"> · held by {holderText(known)}</span> : null}
      </p>
      {error ? (
        <p className="faint" title={error} data-testid="pki-cert-error">
          {certErrorText(error)}
        </p>
      ) : null}
      {!cert && !error ? <p className="faint">Checking the certificate as a receiver would…</p> : null}
      {cert && facts ? (
        <>
          <p className="pki-certcard-about dim">{facts.kind.about}</p>
          {why ? (
            <p className="pki-certcard-why" style={{ color: CERT_STATUS[status].color }} data-testid="pki-cert-why" title={engineWhy ?? undefined}>
              <StatusGlyph shape={CERT_STATUS[status].shape} /> {why}
            </p>
          ) : null}
          {facts.validity ? (
            <div className="pki-certcard-row">
              <span className="k">Valid</span>
              <div>
                <ValidityBar start={facts.validity.start} end={facts.validity.end} now={cert.time32} status={status} />
                <span className="dim">
                  {words ? `${words} · ` : ""}lifetime {facts.validity.stated}
                </span>

              </div>
            </div>
          ) : null}
          <dl className="pki-pairs pki-certcard-facts">
            <dt>Issued by</dt>
            <dd>
              {facts.issuer === "self" ? (
                "itself (a root signs its own certificate)"
              ) : (
                <>
                  {issuerHolder ? <span>{holderText(issuerHolder)} </span> : null}
                  <CertLink id8={facts.issuer} holder={issuerHolder} label={grouped(facts.issuer)} testId="pki-cert-issuer" />
                </>
              )}
            </dd>
            <dt>Names</dt>
            <dd>{facts.subject}</dd>
            {facts.permissions.length > 0 ? (
              <>
                <dt title="Application permissions (appPermissions): the kinds of message it may sign, by PSID">
                  May sign (<Term k="psid">PSIDs</Term>)
                </dt>
                <dd>
                  <ul className="pki-perms" data-testid="pki-cert-psids">
                    {facts.permissions.map((p) => (
                      <li key={p}>{p}</li>
                    ))}
                  </ul>
                </dd>
              </>
            ) : null}
            {facts.issues ? (
              <>
                <dt>Authority</dt>
                <dd>{facts.issues}</dd>
              </>
            ) : null}
            {facts.region ? (
              <>
                <dt>Region</dt>
                <dd>{facts.region}</dd>
              </>
            ) : null}
            {facts.linkage ? (
              <>
                <dt title="Linkage data: lets the two Linkage Authorities' seeds, once published on a CRL, revoke every certificate of the vehicle from that week on, without ever naming it">
                  Linkage
                </dt>
                <dd>
                  <Term k="ij">
                    week <b className="mono">{facts.linkage.iCert}</b>
                  </Term>{" "}
                  · <Term k="lv">linkage value</Term>{" "}
                  <span className="mono" title={facts.linkage.value}>
                    {facts.linkage.value.slice(0, 12)}…
                  </span>
                </dd>
              </>
            ) : null}
            {facts.crlSeries !== null ? (
              <>
                <dt title="crlSeries and cracaId: which revocation list would name it, and the authority that signs that list">Revocation</dt>
                <dd>
                  would be listed on <Term k="crl-series">revocation list series</Term> <span className="mono">{facts.crlSeries}</span>
                  {facts.cracaId ? (
                    <>
                      {" "}
                      <span className="dim" title="cracaId (CRACA, the CRL-signing CA): the low 3 bytes of the hash of the certificate whose holder signs that list">
                        · signed by the CA ending <span className="mono">{facts.cracaId}</span> (<Term k="craca">CRACA</Term>)
                      </span>
                    </>
                  ) : null}
                </dd>
              </>
            ) : null}
            <dt title="Explicit: carries its public key. Implicit (ECQV): the key is reconstructed from the issuer's, which makes the certificate smaller">Key</dt>
            <dd>
              {facts.form === "implicit" ? (
                <>
                  <Term k="ecqv">implicit (ECQV)</Term>: the public key is rebuilt from the issuer's, which keeps the certificate small
                </>
              ) : facts.form === "explicit" ? (
                <>
                  <Term k="explicit">explicit</Term>: the certificate carries its public key
                </>
              ) : (
                (facts.form ?? "?")
              )}
              {facts.signature ? ` · ${facts.signature}` : ""}
              {facts.encryptionKey ? " · has an encryption key" : ""}
            </dd>
          </dl>
          <div className="pki-certcard-row">
            <span className="k">Chain of trust</span>
            <Chain links={links} />
          </div>
          <div className="pki-certcard-checks">
            <button type="button" className="linklike" aria-expanded={showChecks} onClick={() => setChecks(!showChecks)} data-testid="pki-cert-checks-toggle">
              {showChecks ? "▾" : "▸"} Checks a receiver runs: {passed} of {cert.trace.length} passed
              {failed.length > 0 ? `, ${failed.length} failed` : ""}
            </button>
            {showChecks ? (
              <ol className="pki-trace" data-testid="pki-cert-trace">
                {cert.trace.map((s) => {
                  const t = ruleText(s.rule);
                  return (
                    <li key={s.rule} className={`st-${s.status}`} data-rule={s.rule} title={`${s.rule}: ${t.means}`}>
                      <StatusGlyph shape={s.status === "pass" ? "check" : s.status === "fail" ? "ban" : "dots"} />
                      <b>{t.label}</b>
                      <span className="dim" title={s.detail.length > 0 ? `The engine says: ${s.detail.join("; ")}` : undefined}>
                        {s.status === "fail" ? (why ?? s.detail.join("; ")) : s.status === "pass" ? t.means : s.status}
                      </span>
                    </li>
                  );
                })}
              </ol>
            ) : null}
          </div>
          <button type="button" className="linklike" onClick={() => setRaw((r) => !r)} aria-expanded={raw} data-testid="pki-cert-raw-toggle">
            {raw ? "▾ Hide" : "▸ Show"} the raw certificate (decoded JER)
          </button>
          {raw ? <JerTree jer={cert.tree} testId="pki-cert-tree" /> : null}
        </>
      ) : null}
    </section>
  );
}

/** A card for a certificate opened from anywhere in the view (`usePkiView` `cert`), as a dialog. */
export function CertDrawer(): React.JSX.Element | null {
  const cert = usePkiView((s) => s.cert);
  const set = usePkiView((s) => s.set);
  useEffect(() => {
    if (!cert) return undefined;
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape" && !e.defaultPrevented) {
        e.preventDefault();
        set({ cert: null });
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [cert, set]);
  if (!cert) return null;
  return (
    <div className="pki-certdrawer" role="dialog" aria-label={`Certificate ${cert.id8}`} data-testid="pki-cert-drawer">
      <div className="pki-drawer-head">
        <h3>Certificate</h3>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={() => set({ cert: null })} aria-label="Close the certificate" data-testid="pki-cert-drawer-close" autoFocus>
          <CloseIcon />
        </button>
      </div>
      <div className="pki-drawer-body">
        <CertificateCard key={cert.id8} id8={cert.id8} holder={cert.holder} testId="pki-cert-drawer-card" />
      </div>
    </div>
  );
}
