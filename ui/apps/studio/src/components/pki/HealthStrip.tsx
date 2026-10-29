/**
 * The workspace's health strip: five numbers an operator watches, each with its unit, a status
 * shape and one line of what it means — usable pseudonym certificates across the fleet, requests
 * waiting at the authorities, open misbehaviour cases, the age of the newest revocation list (and
 * when the next is due) and revoked devices. Each is a button to the section that explains it.
 *
 * The certificates and the cases are ground truth (the vehicles' own pools, the MAs' case tables
 * joined); seen as one entity, or with ground truth locked, the strip shows only what the trust
 * snapshot carries and a dash for the rest (`lib/pkiTopology.ts` `healthOf`), and says why.
 */

import { GROUND_TRUTH, type TrustAnswer } from "../../lib/pki.js";
import { type StatusShape } from "../../lib/pkiCert.js";
import { ageText, healthOf } from "../../lib/pkiTopology.js";
import { asCases, asVehicles } from "../../lib/pkiRecords.js";
import { chooseTab, usePki, type PkiTab } from "../pki-records/state.js";
import { StatusGlyph } from "./CertStatus.js";

const POLL_MS = 4000;

type Tone = "ok" | "warn" | "bad" | "idle";

const TONE_SHAPE: Readonly<Record<Tone, StatusShape>> = { ok: "check", warn: "clock", bad: "ban", idle: "dots" };

export function HealthStrip({ trust, view }: { readonly trust: TrustAnswer; readonly view: string | null }): React.JSX.Element {
  const gt = view === GROUND_TRUTH;
  const vehicles = asVehicles(usePki(gt ? { topic: "vehicles", view: GROUND_TRUTH } : null, POLL_MS).data);
  const cases = asCases(usePki(gt ? { topic: "cases", view: GROUND_TRUTH } : null, POLL_MS).data);
  const h = healthOf(trust, vehicles?.vehicles ?? null, cases?.cases ?? null);
  const dash = gt ? "…" : "–";
  const onlyGt = gt ? undefined : "Ground truth only: an entity cannot see every vehicle's pool or every MA's cases.";
  const perVehicle = h.certificates !== null && h.vehicles ? h.certificates / h.vehicles : null;
  return (
    <div className="pki-health" role="group" aria-label="System health" data-testid="pki-health">
      <Stat
        tab="vehicles"
        label="pseudonym certificates ready"
        value={h.certificates === null ? dash : h.certificates.toLocaleString()}
        sub={h.certificates === null ? "seen in ground truth only" : perVehicle !== null ? `about ${perVehicle.toFixed(0)} per vehicle, this week and later` : "no vehicle has reported yet"}
        tip={onlyGt ?? "Usable pseudonym certificates (authorization certificates, PCs) the vehicles hold, this week and later"}
        tone={h.certificates === null ? "idle" : h.certificates === 0 && (h.vehicles ?? 0) > 0 ? "bad" : "ok"}
        testId="pki-health-certs"
      />
      <Stat
        tab="flows"
        label={h.pending === 1 ? "request waiting" : "requests waiting"}
        value={h.pending.toLocaleString()}
        sub={h.pending > 0 ? "queued at the authorities or top-ups in flight" : "every authority is keeping up"}
        tip="Requests queued at the authorities, plus certificate top-ups in flight at the vehicles"
        tone={h.pending > 0 ? "warn" : "ok"}
        testId="pki-health-pending"
      />
      <Stat
        tab="cases"
        label={h.openCases === 1 ? "open misbehaviour case" : "open misbehaviour cases"}
        value={h.openCases === null ? dash : String(h.openCases)}
        sub={h.openCases === null ? "seen in ground truth only" : h.openCases > 0 ? "reported, not yet revoked or refused" : "no case under investigation"}
        tip={onlyGt ?? "Misbehaviour cases opened and not yet enforced or refused"}
        tone={h.openCases === null ? "idle" : h.openCases > 0 ? "warn" : "ok"}
        testId="pki-health-cases"
      />
      <Stat
        tab="crls"
        label="since the newest revocation list"
        value={h.crlAgeS === null ? "none yet" : ageText(h.crlAgeS)}
        sub={h.crlDueS === null ? "no list published yet" : h.crlDueS < 0 ? `overdue by ${ageText(-h.crlDueS)}` : `next list due within ${ageText(h.crlDueS)}`}
        tip="Time since the newest certificate revocation list (CRL) was issued, the oldest across providers, and when the next is due at the latest"
        tone={h.crlOverdue ? "bad" : h.crlAgeS === null ? "idle" : "ok"}
        testId="pki-health-crl"
      />
      <Stat
        tab="vehicles"
        label="devices revoked"
        value={`${h.revoked} of ${h.devices}`}
        sub={h.revoked > 0 ? (h.revokedByList ? "a published list names them" : "their providers revoked them") : "no device revoked"}
        tip={h.revokedByList ? "Devices a published revocation list names (ground truth: installed or not), of all devices" : "Devices the providers count as revoked, of all devices"}
        tone={h.revoked > 0 ? "bad" : "ok"}
        testId="pki-health-revoked"
      />
    </div>
  );
}

function Stat({
  tab,
  label,
  value,
  sub,
  tip,
  tone,
  testId,
}: {
  readonly tab: PkiTab;
  readonly label: string;
  readonly value: string;
  readonly sub: string;
  readonly tip: string;
  readonly tone: Tone;
  readonly testId: string;
}): React.JSX.Element {
  return (
    <button type="button" className={`pki-stat tone-${tone}${tone === "warn" || tone === "bad" ? ` ${tone}` : ""}`} title={tip} onClick={() => chooseTab(tab)} data-testid={testId} data-tone={tone}>
      <span className="pki-stat-mark" aria-hidden="true">
        <StatusGlyph shape={TONE_SHAPE[tone]} size={13} />
      </span>
      <span className="pki-stat-text">
        <span className="pki-stat-main">
          <b className="mono">{value}</b>
          <span>{label}</span>
        </span>
        <span className="pki-stat-sub">{sub}</span>
      </span>
    </button>
  );
}
