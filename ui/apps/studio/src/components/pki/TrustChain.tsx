/**
 * How trust flows, as one line a reader follows left to right: the electors sign the certificate
 * trust list (CTL); the CTL names each provider's root; the root certifies the intermediate; the
 * intermediate certifies the online authorities; those issue the vehicles' enrolment and
 * pseudonym certificates and sign the revocation lists. The topology beside it shows who talks to
 * whom; this shows who vouches for whom. Every box opens the same card as the topology's.
 *
 * Built from `inspect.trust` alone: the components, their certificates and the CTL.
 */

import { type PkiComponent, type PkiProvider, type PkiState } from "../../lib/pki.js";
import { roleInfo } from "../../lib/pkiTopology.js";
import { LockIcon } from "./Topology.js";

/** The authorities the intermediate certifies, in the order the chain lists them, and what each one's certificate lets it sign. */
const ISSUED: readonly { readonly role: string; readonly signs: string }[] = [
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

function ProviderChain({ p, onSelect, selected }: { readonly p: PkiProvider; readonly onSelect: (id: string) => void; readonly selected: string | null }): React.JSX.Element {
  const one = (role: string): PkiComponent | null => p.components.find((c) => c.role === role && c.instance === 0) ?? null;
  const root = one("root");
  const ica = one("ica");
  return (
    <div className={`pki-tc-provider${p.on_ctl ? "" : " off-ctl"}`} data-testid={`pki-tc-provider-${p.label}`}>
      <span className="pki-tc-name mono">{p.label}</span>
      <span className={`pki-tc-arrow${p.on_ctl ? "" : " broken"}`} title={p.on_ctl ? "The trust list names this provider's root" : "The trust list does not name this root: devices reject everything under it"}>
        {p.on_ctl ? "names →" : "✕ not named"}
      </span>
      <Box c={root} label="Root (RCA)" sub="offline, signs once" onSelect={onSelect} selected={selected} />
      <span className="pki-tc-arrow">certifies →</span>
      <Box c={ica} label="Intermediate (ICA)" sub="offline" onSelect={onSelect} selected={selected} />
      <span className="pki-tc-arrow">certifies →</span>
      <span className="pki-tc-fan">
        {ISSUED.map((x) => {
          const c = one(x.role);
          return <Box key={x.role} c={c} label={`${roleInfo(x.role).name}`} sub={`signs ${x.signs}`} onSelect={onSelect} selected={selected} />;
        })}
      </span>
    </div>
  );
}

export function TrustChain({ state, onSelect, selected }: { readonly state: PkiState; readonly onSelect: (id: string) => void; readonly selected: string | null }): React.JSX.Element {
  const ctl = state.ctl;
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
        <ProviderChain key={p.label} p={p} onSelect={onSelect} selected={selected} />
      ))}
      <p className="faint pki-tc-foot">
        The Enrolment CA then issues each vehicle its enrolment certificate, and the Pseudonym CA its pseudonym certificates; a receiver checks
        a pseudonym certificate back up this chain to a root the trust list names.
      </p>
    </section>
  );
}
