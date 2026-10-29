/**
 * How trust flows, as one line a reader follows left to right: the electors sign the certificate
 * trust list (CTL); the CTL names each provider's root; the root certifies the intermediate and
 * any authority it signs directly; the intermediate certifies the rest; those issue the vehicles'
 * enrolment and pseudonym certificates and sign the revocation lists. Who certifies whom is read
 * from each authority's own certificate (`inspect.cert` chain), not assumed. The topology beside it shows who talks to
 * whom; this shows who vouches for whom. Every box opens the same card as the topology's.
 *
 * Built from `inspect.trust` alone: the components, their certificates and the CTL.
 */

import { useEffect, useState } from "react";

import { certOf, type PkiComponent, type PkiProvider, type PkiState } from "../../lib/pki.js";
import { componentName, roleInfo } from "../../lib/pkiTopology.js";
import { engine } from "../../state/engine.js";
import { LockIcon } from "./Topology.js";

/** Each authority certificate's issuer (HashedId8), asked once per certificate for the page's life. */
const ISSUER_CACHE = new Map<string, string | null>();

/** The issuer of every component certificate in `state`, as the engine's chain for it says. */
function useIssuers(state: PkiState): ReadonlyMap<string, string | null> {
  const [, bump] = useState(0);
  const ids = state.providers.flatMap((p) => p.components.map((c) => c.cert).filter((x): x is string => typeof x === "string"));
  const key = ids.join(",");
  useEffect(() => {
    let live = true;
    const missing = key.split(",").filter((id) => id && !ISSUER_CACHE.has(id));
    for (const id of missing) {
      ISSUER_CACHE.set(id, null);
      engine
        .request("inspect.cert", { hashedid8: id }, { quiet: true, timeoutMs: 8000 })
        .then((a) => {
          const c = certOf(a);
          ISSUER_CACHE.set(id, c && c.chain.length > 1 ? c.chain[1].toLowerCase() : null);
          if (live) bump((n) => n + 1);
        })
        .catch(() => ISSUER_CACHE.delete(id));
    }
    return () => {
      live = false;
    };
  }, [key]);
  return ISSUER_CACHE;
}

/** The authorities under a root, in the order the chain lists them, and what each one's certificate lets it sign. */
const ISSUED: readonly { readonly role: string; readonly signs: string }[] = [
  { role: "dcm", signs: "devices' setup answers" },
  { role: "eca", signs: "enrolment certificates" },
  { role: "aca", signs: "pseudonym certificates" },
  { role: "ra", signs: "its answers to vehicles" },
  { role: "ma", signs: "its investigation requests; reports are encrypted to it" },
  { role: "la1", signs: "linkage values" },
  { role: "la2", signs: "linkage values" },
  { role: "crlg", signs: "revocation lists" },
];

function Box({
  c,
  label,
  sub,
  onSelect,
  selected,
}: {
  readonly c: PkiComponent | null;
  readonly label: string;
  readonly sub: string;
  readonly onSelect: (id: string) => void;
  readonly selected: string | null;
}): React.JSX.Element {
  if (!c) {
    return (
      <span className="pki-tc-box ghost" title={`${label}: not deployed by this provider`}>
        <b>{label}</b>
        <span>not deployed</span>
      </span>
    );
  }
  const info = roleInfo(c.role);
  return (
    <button
      type="button"
      className={`pki-tc-box status-${c.status}${selected === c.id ? " selected" : ""}`}
      onClick={() => onSelect(c.id)}
      title={`${info.term}${c.status === "offline" ? " — offline by design" : ""}\n${info.does}`}
      data-testid={`pki-tc-${c.id}`}
    >
      <b>
        {c.status === "offline" ? <LockIcon /> : null}
        {label}
      </b>
      <span>{sub}</span>
    </button>
  );
}

function ProviderChain({
  p,
  issuers,
  onSelect,
  selected,
}: {
  readonly p: PkiProvider;
  readonly issuers: ReadonlyMap<string, string | null>;
  readonly onSelect: (id: string) => void;
  readonly selected: string | null;
}): React.JSX.Element {
  const one = (role: string): PkiComponent | null => p.components.find((c) => c.role === role && c.instance === 0) ?? null;
  const root = one("root");
  const ica = one("ica");
  const rootId = root?.cert?.toLowerCase() ?? null;
  // Every instance of each role (a second Pseudonym CA too), placed under the certificate that signed it.
  type Entry = { readonly x: (typeof ISSUED)[number]; readonly c: PkiComponent | null };
  const all: Entry[] = ISSUED.flatMap((x): Entry[] => {
    const cs = p.components.filter((c) => c.role === x.role).sort((a, b) => a.instance - b.instance);
    return cs.length > 0 ? cs.map((c) => ({ x, c })) : [{ x, c: null }];
  });
  const byRoot = all.filter(({ c }) => c?.cert && rootId !== null && issuers.get(c.cert) === rootId);
  const underIca = all.filter((e) => !byRoot.includes(e));
  const box = ({ x, c }: { readonly x: (typeof ISSUED)[number]; readonly c: PkiComponent | null }): React.JSX.Element => (
    <Box key={c?.id ?? x.role} c={c} label={c ? componentName(c.id) : roleInfo(x.role).name} sub={`signs ${x.signs}`} onSelect={onSelect} selected={selected} />
  );
  return (
    <div className={`pki-tc-provider${p.on_ctl ? "" : " off-ctl"}`} data-testid={`pki-tc-provider-${p.label}`}>
      <div className="pki-tc-row">
        <span className="pki-tc-name mono">{p.label}</span>
        <span className={`pki-tc-arrow${p.on_ctl ? "" : " broken"}`} title={p.on_ctl ? "The trust list names this provider's root" : "The trust list does not name this root: devices reject everything under it"}>
          {p.on_ctl ? "names →" : "✕ not named"}
        </span>
        <Box c={root} label="Root (RCA)" sub="offline, signs once" onSelect={onSelect} selected={selected} />
        <span className="pki-tc-arrow">certifies →</span>
        <Box c={ica} label="Intermediate (ICA)" sub="offline" onSelect={onSelect} selected={selected} />
        <span className="pki-tc-arrow">certifies →</span>
        <span className="pki-tc-fan" data-testid={`pki-tc-under-ica-${p.label}`}>
          {underIca.map(box)}
        </span>
      </div>
      {byRoot.length > 0 ? (
        <div className="pki-tc-row pki-tc-direct">
          <span className="pki-tc-arrow" title="These authorities' certificates are signed by the root itself, not by the intermediate">
            the root also certifies directly →
          </span>
          <span className="pki-tc-fan" data-testid={`pki-tc-under-root-${p.label}`}>
            {byRoot.map(box)}
          </span>
        </div>
      ) : null}
    </div>
  );
}

export function TrustChain({ state, onSelect, selected }: { readonly state: PkiState; readonly onSelect: (id: string) => void; readonly selected: string | null }): React.JSX.Element {
  const ctl = state.ctl;
  const issuers = useIssuers(state);
  return (
    <section className="pki-trustchain" data-testid="pki-trustchain" aria-label="How trust flows">
      <h3 className="pki-section-title">
        How trust flows <span className="dim">who vouches for whom: a device accepts a certificate only if this chain holds, end to end</span>
      </h3>
      <div className="pki-tc-ctl">
        <span className="pki-tc-box ctl" title="The certificate trust list (CTL): the roots every device trusts, signed by a quorum of electors">
          <b>Trust list (CTL)</b>
          <span>
            version {ctl.sequence} · signed by {ctl.signers.length} of {ctl.electors} electors ({ctl.quorum} needed)
          </span>
        </span>
      </div>
      {state.providers.map((p) => (
        <ProviderChain key={p.label} p={p} issuers={issuers} onSelect={onSelect} selected={selected} />
      ))}
      <p className="faint pki-tc-foot">
        The Enrolment CA (ECA) then issues each vehicle its enrolment certificate, and the Pseudonym CA (ACA) its pseudonym certificates; a receiver checks
        a pseudonym certificate back up this chain to a root the trust list names.
      </p>
    </section>
  );
}
