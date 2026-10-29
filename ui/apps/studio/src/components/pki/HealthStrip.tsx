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

import { useEffect, useRef, useState } from "react";

import { GROUND_TRUTH, type TrustAnswer } from "../../lib/pki.js";
import { growing, type StatusShape } from "../../lib/pkiCert.js";
import { ageText, healthOf } from "../../lib/pkiTopology.js";
import { asCases, asVehicles, leftRun, type VehicleRow } from "../../lib/pkiRecords.js";
import { chooseTab, usePki, usePkiView, type PkiTab } from "../pki-records/state.js";
import { StatusGlyph } from "./CertStatus.js";

const POLL_MS = 4000;

/**
 * `info` is work in progress or a normal outcome (a top-up in flight, a case under investigation,
 * an attacker revoked); `warn` and `bad` are kept for real problems: a queue that keeps growing, a
 * fleet with no certificates, an overdue revocation list.
 */
type Tone = "ok" | "info" | "warn" | "bad" | "idle";

const TONE_SHAPE: Readonly<Record<Tone, StatusShape>> = { ok: "check", info: "info", warn: "clock", bad: "ban", idle: "dots" };

/** Polls of the waiting-requests count kept to tell a growing queue from routine work. */
const QUEUE_SAMPLES = 6;

export function HealthStrip({ trust, view }: { readonly trust: TrustAnswer; readonly view: string | null }): React.JSX.Element {
  const gt = view === GROUND_TRUTH;
  const vehicles = asVehicles(usePki(gt ? { topic: "vehicles", view: GROUND_TRUTH } : null, POLL_MS).data);
  const cases = asCases(usePki(gt ? { topic: "cases", view: GROUND_TRUTH } : null, POLL_MS).data);
  const h = healthOf(trust, vehicles?.vehicles ?? null, cases?.cases ?? null);
  const dash = gt ? "…" : "–";
  const onlyGt = gt ? undefined : "Ground truth only: an entity cannot see every vehicle's pool or every MA's cases.";
  const cadence = usePkiView((s) => s.crlCadence);
  // Only the vehicles still driving: one whose trip ended keeps its last pool, which is history.
  const inRun = vehicles ? vehicles.vehicles.filter((v) => !leftRun(v, vehicles.t_ns)) : null;
  // Valid now: what each vehicle can sign with this week, the definition Metrics' "valid pseudonym
  // certificates held" uses. The weeks downloaded ahead are said beside it, never mixed in.
  const validOf = (v: VehicleRow): number => (typeof v.security?.pool_valid === "number" ? v.security.pool_valid : v.usable);
  const certificates = inRun ? inRun.reduce((n, v) => n + validOf(v), 0) : null;
  const withAhead = inRun ? inRun.reduce((n, v) => n + Math.max(validOf(v), v.usable), 0) : null;
  const perVehicle = certificates !== null && inRun && inRun.length > 0 ? certificates / inRun.length : null;
  const every = h.crlNewest ? (cadence[h.crlNewest.provider] ?? null) : null;
  const nextIn = h.crlAgeS !== null && every !== null ? Math.max(0, every - h.crlAgeS) : null;
  // A revoked vehicle with nothing to sign with is revocation working, not a shortage.
  const starving = inRun ? inRun.filter((v) => v.enrolled && v.usable === 0 && v.revoked !== true && (v.truth?.revoked_certs ?? 0) === 0).length : 0;
  const jmax = inRun?.[0]?.jmax ?? null;
  // One sample per new snapshot: the queue warns only when it grew across the last three.
  const samples = useRef<number[]>([]);
  const [queueGrowing, setQueueGrowing] = useState(false);
  useEffect(() => {
    samples.current = [...samples.current, h.pending].slice(-QUEUE_SAMPLES);
    setQueueGrowing(growing(samples.current));
  }, [trust.t_ns, h.pending]);
  return (
    <div className="pki-health" role="group" aria-label="System health" data-testid="pki-health">
      <Stat
        tab="vehicles"
        label="pseudonym certificates valid now"
        value={certificates === null ? dash : certificates.toLocaleString()}
        sub={
          certificates === null
            ? "seen in ground truth only"
            : starving > 0
              ? `${starving} vehicle${starving === 1 ? " has" : "s have"} none left to sign with`
              : perVehicle !== null
                ? `about ${perVehicle.toFixed(0)} per vehicle${withAhead !== null && withAhead > certificates ? ` · ${withAhead.toLocaleString()} incl. weeks ahead` : ""}`
                : "no vehicle in the run has reported yet"
        }
        tip={
          onlyGt ??
          `Pseudonym certificates (PCs) the ${inRun?.length ?? 0} vehicles still in the run can sign with now: what is left of this week's batch${jmax !== null ? ` (${jmax} a week)` : ""}. The same count as Metrics' “valid pseudonym certificates held” (there as a per-vehicle mean). “Incl. weeks ahead” adds the batches already downloaded for later weeks. Vehicles that left the run are not counted.`
        }
        tone={certificates === null ? "idle" : starving > 0 ? "warn" : "ok"}
        testId="pki-health-certs"
      />
      <Stat
        tab="flows"
        label={h.pending === 1 ? "request waiting" : "requests waiting"}
        value={h.pending.toLocaleString()}
        sub={queueGrowing ? "growing: the authorities fall behind" : h.pending > 0 ? "routine: being served now" : "every authority keeps up"}
        tip="Requests queued at the authorities, plus certificate top-ups in flight at the vehicles. It warns only when the count has grown three snapshots running."
        tone={queueGrowing ? "warn" : h.pending > 0 ? "info" : "ok"}
        testId="pki-health-pending"
      />
      <Stat
        tab="cases"
        label={h.openCases === 1 ? "open misbehaviour case" : "open misbehaviour cases"}
        value={h.openCases === null ? dash : String(h.openCases)}
        sub={h.openCases === null ? "seen in ground truth only" : h.openCases > 0 ? "being investigated, normal" : "no case under investigation"}
        tip={onlyGt ?? "Misbehaviour cases the Misbehaviour Authority (MA) opened and has not yet enforced or refused. An open case is the system working, not a fault."}
        tone={h.openCases === null ? "idle" : h.openCases > 0 ? "info" : "ok"}
        testId="pki-health-cases"
      />
      <Stat
        tab="crls"
        label={h.crlAgeS === null ? "revocation list" : "newest revocation list"}
        value={h.crlAgeS === null ? "none yet" : `${ageText(h.crlAgeS)} ago`}
        sub={
          h.crlDueS === null
            ? "no list published yet"
            : h.crlDueS < 0
              ? `the next is overdue by ${ageText(-h.crlDueS)}`
              : `${h.crlNewest ? `${h.crlNewest.provider} v${h.crlNewest.version} · ` : ""}${nextIn !== null && every !== null ? `next within ${ageText(nextIn)} (every ${ageText(every)})` : `next due within ${ageText(h.crlDueS)}`}`
        }
        tip={`How long ago the newest certificate revocation list (CRL) of any provider was issued, and when the next one comes.${every !== null ? ` This scenario publishes a list every ${ageText(every)}.` : ""} Every list also prints a deadline by which it must be replaced (its nextCrl): the soonest is ${h.crlDueS === null ? "not set yet" : h.crlDueS < 0 ? `overdue by ${ageText(-h.crlDueS)}` : `in ${ageText(h.crlDueS)}`}.`}
        tone={h.crlOverdue ? "bad" : h.crlAgeS === null ? "idle" : "ok"}
        testId="pki-health-crl"
      />
      <Stat
        tab="vehicles"
        label="devices revoked"
        value={gt ? `${h.revoked} of ${h.devices}` : dash}
        sub={!gt ? "seen in ground truth only" : h.revoked > 0 ? "named on a published revocation list" : "no device on a published list"}
        tip={
          gt
            ? `Devices a published revocation list names (installed by the others or not), of the ${h.devices} devices the providers have enrolled: vehicles and roadside units, so one more than the vehicles listed if the run has a roadside unit. Every view uses this one definition. Revoking a misbehaving device is the system working; the engine does not publish which devices were honest, so this cannot tell a wrongful revocation apart.`
            : "Which devices a published list names is ground truth: no single authority sees it (the RA, for one, does not see revocations; it only blocks the enrolment certificate the MA points it to)."
        }
        tone={!gt ? "idle" : h.revoked > 0 ? "info" : "ok"}
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
