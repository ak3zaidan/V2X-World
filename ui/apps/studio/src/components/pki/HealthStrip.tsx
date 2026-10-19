/**
 * The workspace's health strip: five numbers an operator watches — usable pseudonym certificates
 * across the fleet, requests waiting at the authorities, open misbehaviour cases, the age of the
 * newest revocation list and revoked devices — each a button to the tab that explains it.
 *
 * The certificates and the cases are ground truth (the vehicles' own pools, the MAs' case tables
 * joined); seen as one entity, or with ground truth locked, the strip shows only what the trust
 * snapshot carries and a dash for the rest (`lib/pkiTopology.ts` `healthOf`).
 */

import { GROUND_TRUTH, type TrustAnswer } from "../../lib/pki.js";
import { ageText, healthOf } from "../../lib/pkiTopology.js";
import { asCases, asVehicles } from "../../lib/pkiRecords.js";
import { chooseTab, usePki, type PkiTab } from "../pki-records/state.js";

const POLL_MS = 4000;

export function HealthStrip({ trust, view }: { readonly trust: TrustAnswer; readonly view: string | null }): React.JSX.Element {
  const gt = view === GROUND_TRUTH;
  const vehicles = asVehicles(usePki(gt ? { topic: "vehicles", view: GROUND_TRUTH } : null, POLL_MS).data);
  const cases = asCases(usePki(gt ? { topic: "cases", view: GROUND_TRUTH } : null, POLL_MS).data);
  const h = healthOf(trust, vehicles?.vehicles ?? null, cases?.cases ?? null);
  const dash = gt ? "…" : "–";
  const onlyGt = gt ? undefined : "Ground truth only";
  return (
    <div className="pki-health" role="group" aria-label="System health" data-testid="pki-health">
      <Stat
        tab="vehicles"
        label="certificates available"
        value={h.certificates === null ? dash : h.certificates.toLocaleString()}
        tip={onlyGt ?? "Usable pseudonym certificates (authorization certificates) the vehicles hold, this week and later"}
        testId="pki-health-certs"
      />
      <Stat
        tab="flows"
        label="pending requests"
        value={h.pending.toLocaleString()}
        tip="Requests queued at the authorities, plus certificate top-ups in flight at the vehicles"
        tone={h.pending > 0 ? "warn" : undefined}
        testId="pki-health-pending"
      />
      <Stat
        tab="cases"
        label="open cases"
        value={h.openCases === null ? dash : String(h.openCases)}
        tip={onlyGt ?? "Misbehaviour cases opened and not yet enforced or refused"}
        tone={(h.openCases ?? 0) > 0 ? "warn" : undefined}
        testId="pki-health-cases"
      />
      <Stat
        tab="crls"
        label="revocation list age"
        value={h.crlAgeS === null ? "none yet" : ageText(h.crlAgeS)}
        tip="Time since the newest certificate revocation list (CRL) was issued, the oldest across providers"
        tone={h.crlOverdue ? "bad" : undefined}
        testId="pki-health-crl"
      />
      <Stat
        tab="vehicles"
        label="revoked devices"
        value={`${h.revoked} of ${h.devices}`}
        tip={h.revokedByList ? "Devices a published revocation list names (ground truth: installed or not), of all devices" : "Devices the providers count as revoked, of all devices"}
        tone={h.revoked > 0 ? "bad" : undefined}
        testId="pki-health-revoked"
      />
    </div>
  );
}

function Stat({
  tab,
  label,
  value,
  tip,
  tone,
  testId,
}: {
  readonly tab: PkiTab;
  readonly label: string;
  readonly value: string;
  readonly tip: string;
  readonly tone?: "warn" | "bad";
  readonly testId: string;
}): React.JSX.Element {
  return (
    <button type="button" className={`pki-stat${tone ? ` ${tone}` : ""}`} title={tip} onClick={() => chooseTab(tab)} data-testid={testId}>
      <b className="mono">{value}</b>
      <span>{label}</span>
    </button>
  );
}
