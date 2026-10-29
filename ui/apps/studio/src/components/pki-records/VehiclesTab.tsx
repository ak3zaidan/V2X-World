/**
 * The PKI view's Vehicles tab (00-gap-analysis §3.2): every vehicle's provider, enrolment and
 * provisioning state, pool, next top-up, installed CRL and CTL; selecting one shows its
 * certificate life at a glance — a summary of what it holds now (its enrolment certificate, the
 * pseudonym certificate it signs with, its pool, its next top-up and whether it is revoked), the
 * lifecycle on the run's clock, its pool week by week (the i × j grid: weeks across, the
 * certificates of a week down), its pseudonym history and the active certificate's card with the
 * validation a US device runs on it, step by step. Every certificate id opens its card.
 *
 * Everything is `inspect.pki {topic: "vehicles" | "vehicle"}` and `inspect.cert`; the grid is
 * the vehicle's own `node.credentials`, rebuilt cell for cell (`lib/pkiRecords.ts` `fullGrid`).
 */

import { useMemo, useState } from "react";

import { CERT_STATUS, VEHICLE_STATE, cellStatus, deviceName, grouped } from "../../lib/pkiCert.js";
import { vehicleMatches } from "../../lib/pkiNames.js";
import {
  CELL_LABEL,
  CREDENTIAL_LABEL,
  CREDENTIAL_ORDER,
  GROUND_TRUTH,
  asCases,
  asCrls,
  asVehicle,
  asVehicles,
  credentialState,
  fullGrid,
  installedList,
  installedListText,
  overdueProviders,
  providerColor,
  revocationStory,
  secondsText,
  slaText,
  type CellState,
  type CredCell,
  type CredentialContext,
  type CredentialState,
  type CrlHead,
  type VehicleDetail,
  type VehicleRow,
} from "../../lib/pkiRecords.js";
import { CertificateCard, CertLink } from "../pki/CertificateCard.js";
import { useCertStatus } from "../pki/useCert.js";
import { CertBadge, StatusGlyph } from "../pki/CertStatus.js";
import { Term } from "../pki/Term.js";
import { VehicleName } from "../pki/VehicleName.js";
import { chooseTab, usePki, usePkiView } from "./state.js";
import { VehicleTimeline } from "./VehicleTimeline.js";

const CELL_PX = 14;
/** Weeks shown past the last one the vehicle holds a certificate for. */
const WEEKS_PAST_HELD = 3;
const GRID_HEAD = 16;

/** A vehicle's state as a badge in the certificates' shared language. */
export function VehicleStateBadge({ st, testId }: { readonly st: CredentialState | "ec-bad"; readonly testId?: string }): React.JSX.Element {
  const v = VEHICLE_STATE[st];
  return (
    <span className={`pkir-state ${st} pki-vstate`} style={{ color: v.color }} title={v.meaning} data-testid={testId} data-state={st}>
      <StatusGlyph shape={v.shape} />
      {st === "ec-bad" ? "enrolment certificate failing" : CREDENTIAL_LABEL[st]}
    </span>
  );
}

/** A column whose value is the same in every row is said once above the table instead. */
function uniform<T>(rows: readonly VehicleRow[], f: (r: VehicleRow) => T): { readonly same: boolean; readonly value: T | undefined } {
  if (rows.length === 0) return { same: true, value: undefined };
  const v = f(rows[0]);
  return { same: rows.every((r) => f(r) === v), value: v };
}

function crlTitle(r: VehicleRow, heads: readonly CrlHead[] | undefined): string {
  const l = installedList(r.security, heads);
  if (!l) return "No revocation list installed";
  return `${l.provider ? `${l.provider}'s` : "A"} revocation list, version ${l.version}, ${l.entries} entr${l.entries === 1 ? "y" : "ies"}${l.provider ? " (matched to the published list by version and entry count)" : ""}`;
}

/** Pseudonym certificates it can sign with now (the Metrics "valid certificates held"), and with the weeks downloaded ahead. */
function poolOf(r: Pick<VehicleRow, "usable" | "security" | "periods_ahead">): { readonly now: number; readonly withAhead: number; readonly ahead: number } {
  const now = typeof r.security?.pool_valid === "number" ? r.security.pool_valid : r.usable;
  return { now, withAhead: Math.max(now, r.usable), ahead: r.periods_ahead ?? 0 };
}

function aheadText(weeks: number): string {
  return `${weeks} week${weeks === 1 ? "" : "s"} ahead`;
}

/** A vehicle's certificates in the table: valid now, then with the weeks ahead; a revoked vehicle's are held but refused. */
function PoolCell({ r, revoked }: { readonly r: VehicleRow; readonly revoked: boolean }): React.JSX.Element {
  const p = poolOf(r);
  if (revoked)
    return (
      <td className="faint" data-testid="pki-vehicle-pool" data-revoked="1" title="It still holds these, but a published revocation list names it, so receivers refuse every one of them">
        <span className="mono">{p.withAhead}</span> held · refused
      </td>
    );
  return (
    <td
      data-testid="pki-vehicle-pool"
      title={`${p.now} valid now: what it can sign with this week. ${p.withAhead} including the ${aheadText(p.ahead)} it has already downloaded.${r.provisioned ? "" : " It has not downloaded any pseudonym certificates yet."}`}
    >
      <b className="mono">{p.now}</b> now
      {p.withAhead > p.now ? (
        <span className="faint">
          {" "}
          · <span className="mono">{p.withAhead}</span> incl. {aheadText(p.ahead)}
        </span>
      ) : null}
    </td>
  );
}

export function VehiclesTab({ view }: { readonly view: string | null }): React.JSX.Element {
  const node = usePkiView((s) => s.node);
  const set = usePkiView((s) => s.set);
  const [only, setOnly] = useState<CredentialState | null>(null);
  const [find, setFind] = useState("");
  const gt = view === GROUND_TRUTH;
  const list = asVehicles(usePki(view ? { topic: "vehicles", view } : null).data);
  const crls = asCrls(usePki({ topic: "crls" }, 4000).data);
  const ctx: CredentialContext = useMemo(
    () => ({
      nowNs: list?.t_ns ?? 0,
      offCtl: new Set((crls?.providers ?? []).filter((p) => !p.on_ctl).map((p) => p.label)),
      crlOverdue: crls ? overdueProviders(crls) : new Set(),
      groundTruth: gt,
    }),
    [list?.t_ns, crls, gt],
  );
  // The chosen vehicle's enrolment certificate, checked as a receiver would: its row and its card
  // say the same thing about it (the list itself carries no EC validity).
  const ec = useCertStatus((list?.vehicles ?? []).find((r) => r.node === node)?.enrolment ?? null);
  if (view === null) return <Locked />;
  const all = list?.vehicles ?? [];
  const labels = all.map((r) => r.provider);
  const stateOf = new Map(all.map((r) => [r.device, credentialState(r, ctx)]));
  const rank = (r: VehicleRow): number => CREDENTIAL_ORDER.indexOf(stateOf.get(r.device) ?? "healthy");
  // Problems first, then routine work, then the vehicles that are fine, then those that left.
  const sorted = [...all].sort((a, b) => rank(a) - rank(b) || a.node - b.node);
  const rows = (only === null ? sorted : sorted.filter((r) => stateOf.get(r.device) === only)).filter((r) => vehicleMatches(find, r.device, r.node));
  const chosen = all.find((r) => r.node === node) ?? null;
  const counts = new Map<CredentialState, number>();
  for (const st of stateOf.values()) counts.set(st, (counts.get(st) ?? 0) + 1);
  // Columns that never differ are one sentence, not a column of copies. A vehicle that left the run
  // has no next top-up, so the top-up column is judged on the vehicles still driving.
  const heads = crls?.crls;
  const driving = all.filter((r) => stateOf.get(r.device) !== "left");
  const now = list?.t_ns ?? 0;
  const ctl = uniform(all, (r) => r.ctl_sequence ?? null);
  // The lists and top-ups of the vehicles still driving: one that left keeps its last record, and a
  // revoked one is blocked from top-ups (its card and its state say so), so neither unfolds a column.
  const crl = uniform(driving, (r) => installedListText(installedList(r.security, heads)));
  const topup = uniform(
    driving.filter((r) => stateOf.get(r.device) !== "revoked"),
    (r) => nextTopup(r, now),
  );
  const enrolled = uniform(all, (r) => r.enrolled);
  const allEnrolled = all.length > 0 && enrolled.same && enrolled.value === true;
  const same: string[] = [];
  if (allEnrolled) same.push("enrolled with its provider");
  if (all.length > 0 && ctl.same) same.push(ctl.value === null ? "no trust list (CTL) reported" : `trust list (CTL) version ${String(ctl.value)}`);
  if (driving.length > 0 && crl.same) same.push(listSentence(installedList(driving[0].security, heads)));
  if (driving.length > 0 && topup.same && topup.value !== undefined) same.push(`${topupSentence(topup.value)}${(counts.get("revoked") ?? 0) > 0 ? " (revoked vehicles get none)" : ""}`);
  return (
    <div className={chosen ? "pkir-split" : "pkir-split single"} data-testid="pki-vehicles">
      <div className="pkir-table-wrap">
        <p className="pkir-pad pki-lead">
          Each vehicle enrols once (an <b>enrolment certificate</b>, its long-term credential), then signs its safety messages with short-lived{" "}
          <b>pseudonym certificates</b> it downloads a week at a time. Choose a vehicle to see the certificates it holds now, its certificate life on the
          run's clock, its pool week by week and the checks a receiver runs on its certificate.
        </p>
        {all.length > 0 ? (
          <div className="pkir-pad pki-vsummary" data-testid="pki-vehicles-summary" role="group" aria-label="Show only the vehicles in one state">
            <button
              type="button"
              className={only === null ? "pki-chip on" : "pki-chip"}
              aria-pressed={only === null}
              onClick={() => setOnly(null)}
              data-testid="pki-chip-all"
              title={`The ${all.length} vehicles with a certificate record. The strip's device count also includes roadside units, which hold certificates but are not listed here.`}
            >
              All <b className="mono">{all.length}</b>
            </button>
            {CREDENTIAL_ORDER.filter((s) => (counts.get(s) ?? 0) > 0).map((s) => (
              <button
                key={s}
                type="button"
                className={only === s ? "pki-chip on" : "pki-chip"}
                aria-pressed={only === s}
                onClick={() => setOnly(only === s ? null : s)}
                title={`Show only these. ${VEHICLE_STATE[s].meaning}`}
                data-testid="pki-chip"
                data-state={s}
              >
                <VehicleStateBadge st={s} /> <b className="mono">{counts.get(s)}</b>
              </button>
            ))}
          </div>
        ) : null}
        {same.length > 0 ? (
          <p className="pkir-pad faint" data-testid="pki-vehicles-same">
            Every vehicle{(counts.get("left") ?? 0) > 0 ? " still in the run" : ""}: {same.join(" · ")}.
          </p>
        ) : null}
        {all.length > 0 ? (
          <label className="pkir-pad pki-vfind">
            <span className="dim">Find a vehicle</span>
            <input
              type="search"
              value={find}
              onChange={(e) => setFind(e.target.value)}
              placeholder="66, or its device id"
              aria-label="Find a vehicle by its number on the map or its device id"
              title="The number the map and the HUD show (Vehicle 66), or the SCMS device id the certificate records use (1000066)"
              data-testid="pki-vehicle-find"
            />
          </label>
        ) : null}
        <table className="pkir-table pki-vtable" data-testid="pki-vehicles-table">
          <thead>
            <tr>
              <th title="Its number on the map and in the HUD, with its SCMS device id under it">Vehicle</th>
              <th title="Its credential state, in the same shapes and colours as every certificate; problems are listed first">State</th>
              <th>Provider</th>
              {allEnrolled ? null : <th title="Holds an enrolment certificate from its provider's ECA">Enrolled</th>}
              <th title="Pseudonym certificates (PCs) valid now: what it can sign with this week, the same count as Metrics' “valid pseudonym certificates held”. Then the same including the weeks it has already downloaded ahead.">
                Pseudonym certificates
              </th>
              {topup.same ? null : <th title="When it next asks its RA for a batch">Next top-up</th>}
              {ctl.same ? null : <th title="The certificate trust list (CTL) version it holds">Trust list</th>}
              {crl.same ? null : <th title="The certificate revocation list (CRL) it installed, and whose list it is">Revocation list</th>}
              <th title="Pseudonym certificates it has signed with so far">Pseudonyms used</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const st0 = stateOf.get(r.device) ?? "healthy";
              const st = r.node === node ? withEc(st0, ec, r.enrolled) : st0;
              const gone = st === "left";
              const cls = [r.node === node ? "sel" : "", gone ? "left" : ""].filter(Boolean).join(" ");
              const lst = installedList(r.security, heads);
              return (
                <tr
                  key={r.device}
                  className={cls || undefined}
                  onClick={() => set({ node: r.node })}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      set({ node: r.node });
                    }
                  }}
                  tabIndex={0}
                  aria-selected={r.node === node}
                  data-testid="pki-vehicle-row"
                  data-node={r.node}
                  data-state={st}
                >
                  <td>
                    <VehicleName device={r.device} node={r.node} testId="pki-vehicle-name" />
                  </td>
                  <td>
                    <VehicleStateBadge st={st} testId="pki-vehicle-state" />
                  </td>
                  <td>
                    <span className="pkir-swatch" style={{ background: hex(providerColor(r.provider, labels)) }} />
                    {r.provider}
                  </td>
                  {allEnrolled ? null : <td>{r.enrolled ? "yes" : "no"}</td>}
                  <PoolCell r={r} revoked={st === "revoked"} />
                  {topup.same ? null : <td className="mono">{gone ? "–" : nextTopup(r, now)}</td>}
                  {ctl.same ? null : <td className="mono">{r.ctl_sequence ?? "–"}</td>}
                  {crl.same ? null : (
                    <td title={crlTitle(r, heads)} data-testid="pki-vehicle-crl">
                      {installedListText(lst)}
                    </td>
                  )}
                  <td className="mono">{r.pseudonym_changes + (r.active ? 1 : 0)}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {all.length === 0 ? (
          <p className="dim pkir-pad">No vehicle has published its credentials yet: each vehicle reports its certificates once a simulated second after it enters the run.</p>
        ) : rows.length === 0 ? (
          <p className="dim pkir-pad">{find.trim() ? `No vehicle${only ? " in that state" : ""} matches “${find.trim()}”.` : "No vehicle is in that state now."}</p>
        ) : null}
        <p className="faint pkir-pad">
          {gt ? "Ground truth: “revoked” includes vehicles a published CRL revokes that have not installed it yet." : `As ${deviceName(view)}: only what that vehicle holds.`}
        </p>
      </div>
      {chosen ? (
        <VehicleCard
          device={chosen.device}
          view={gt ? GROUND_TRUTH : chosen.device}
          state={stateOf.get(chosen.device) ?? "healthy"}
          row={chosen}
          nowNs={list?.t_ns ?? 0}
          ec={ec}
          heads={heads ?? null}
        />
      ) : null}
    </div>
  );
}

/** The installed list in the "Every vehicle" sentence. */
function listSentence(l: ReturnType<typeof installedList>): string {
  if (!l) return "no revocation list (CRL) installed";
  return `has installed ${l.provider ? `${l.provider}'s ` : "the "}revocation list (CRL) version ${l.version} (${l.entries} entr${l.entries === 1 ? "y" : "ies"})`;
}

/** The next top-up in the "Every vehicle" sentence. */
function topupSentence(v: string): string {
  if (v === "none") return "no top-up planned";
  if (v === "due" || v === "in flight") return `next pseudonym top-up ${v}`;
  if (v === "–") return "no top-up reported";
  return `next pseudonym top-up ${v}`;
}

/**
 * An enrolment certificate issued with a lifetime of zero is the engine's rounding of a lifetime
 * under one hour (a scenario's compressed calendar), not the vehicle's fault: it is shown as a
 * simulator limitation and does not decide the vehicle's state.
 */
function ecFails(ec: { readonly status: string; readonly zeroLifetime?: boolean }): boolean {
  return !(ec.status === "valid" || ec.status === "expiring" || ec.status === "unknown") && ec.zeroLifetime !== true;
}

/** A vehicle whose enrolment certificate fails is never "healthy", whatever its pool says. */
function withEc(state: CredentialState, ec: { readonly status: string; readonly zeroLifetime?: boolean }, enrolled: boolean): CredentialState | "ec-bad" {
  const bad = enrolled && ecFails(ec);
  return bad && (state === "healthy" || state === "between" || state === "topup-due") ? "ec-bad" : state;
}

function nextTopup(r: VehicleRow, now: number): string {
  if (r.security?.topup_in_flight) return "in flight";
  const n = r.security?.next_topup;
  if (n === undefined) return "–";
  if (n === null) return "none";
  return n <= now ? "due" : `in ${slaText((n - now) / 1e9)}`;
}

/** The active certificate's place in the grid: its week and index, from the cell marked active. */
function activeCell(d: Pick<VehicleDetail, "grid" | "active">): CredCell | null {
  return d.grid.held.find((c) => c.state === "active") ?? (d.active ? (d.grid.held.find((c) => c.id8 === d.active) ?? null) : null);
}

/** What the vehicle holds now, in five tiles. */
function AtAGlance({
  d,
  row,
  nowNs,
  ec,
  left,
  heads,
}: {
  readonly d: VehicleDetail;
  readonly row: VehicleRow;
  readonly nowNs: number;
  readonly ec: ReturnType<typeof useCertStatus>;
  readonly left: boolean;
  readonly heads: readonly CrlHead[] | null;
}): React.JSX.Element {
  const a = activeCell(d);
  const thisWeek = d.grid.held.filter((c) => c.i === d.current_i && (c.state === "valid" || c.state === "active")).length;
  const last = [...d.pseudonyms].sort((x, y) => y.since - x.since)[0];
  const revoked = d.revoked || row.security?.self_revoked === true;
  const gtRevoked = (d.truth?.revoked_certs ?? 0) > 0;
  const ecOk = !ecFails(ec);
  // A lifetime of zero is the engine's rounding (engine request: minutes or seconds), not a fault.
  const ecLimit = ec.zeroLifetime && ec.status !== "valid" && ec.status !== "expiring" && ec.status !== "unknown";
  const pool = poolOf({ usable: d.usable, security: row.security, periods_ahead: d.periods_ahead });
  const validNow = typeof row.security?.pool_valid === "number" ? pool.now : thisWeek;
  // Which list and which case name it is ground truth: asked only there.
  const story = useRevocationStory(d.device, d.view === GROUND_TRUTH && (revoked || gtRevoked) ? (d.truth?.revoked_by_crls ?? []) : null, heads);
  const own = installedList(row.security, heads);
  return (
    <div className="pki-glance" data-testid="pki-vehicle-glance">
      <div className={ecLimit ? "pki-tile limit" : ecOk ? "pki-tile" : "pki-tile attn"} data-testid="pki-glance-ec-tile" data-limit={ecLimit ? "1" : undefined}>
        <span className="k" title="Enrolment certificate (EC): issued once by the provider's Enrolment CA (ECA); proves the vehicle may ask for pseudonym certificates">
          Enrolment certificate (EC)
        </span>
        {d.enrolment ? (
          <span className="pki-tile-cert">
            {ecLimit ? (
              <span className="pki-limit-chip" title="A limit of the simulator in this scenario, not a fault of the vehicle" data-testid="pki-glance-ec-limit">
                <StatusGlyph shape="info" /> simulator limit
              </span>
            ) : (
              <CertBadge status={ec.status} compact testId="pki-glance-ec-status" />
            )}
            <CertLink id8={d.enrolment} holder={d.device} label={grouped(d.enrolment)} testId="pki-glance-ec" />
          </span>
        ) : (
          <b>none</b>
        )}
        <span className="sub" data-testid="pki-glance-ec-says">
          {!d.enrolled
            ? "not enrolled"
            : ecLimit
              ? "lifetime not modelled in this scenario (open it for why)"
              : ecOk
                ? "enrolled with its provider"
                : `a receiver reads it as ${CERT_STATUS[ec.status].label.toLowerCase()} (open it)`}
        </span>
      </div>
      <div className="pki-tile">
        <span className="k" title="The pseudonym certificate (PC) it signs its safety messages with now">Signing with (PC)</span>
        {d.active && !left ? (
          <CertLink id8={d.active} holder={d.device} label={grouped(d.active)} testId="pki-glance-pc" />
        ) : left ? (
          <b className="dim" data-testid="pki-glance-left">
            nothing: it left the run
          </b>
        ) : d.usable > 0 ? (
          <b className="dim" data-testid="pki-glance-between" title="No certificate is active at this instant, but it holds usable ones: it is changing to its next pseudonym">
            between pseudonyms
          </b>
        ) : (
          <b className="bad">no certificate</b>
        )}
        <span className="sub">
          {a && !left ? (
            <Term k="ij">
              week {a.i}, certificate {a.j + 1} of {d.jmax}
            </Term>
          ) : left ? (
            `its last record is from ${secondsText(row.security?.t ?? d.t)}`
          ) : d.active ? (
            "—"
          ) : d.usable > 0 ? (
            `${d.usable} usable to change to`
          ) : (
            "nothing usable left"
          )}
          {last && d.active === last.id8 && !left ? ` · since ${secondsText(last.since)}` : last ? ` · last one from ${secondsText(last.since)}` : ""}
        </span>
      </div>
      <div className="pki-tile" data-testid="pki-glance-pool" data-revoked={revoked || gtRevoked ? "1" : undefined}>
        <span
          className="k"
          title={`Pseudonym certificates it can sign with now: what is left of this week's ${d.jmax}. The second line adds the weeks it has already downloaded ahead.`}
        >
          Pseudonym certificates
        </span>
        {revoked || gtRevoked ? (
          <>
            <b className="mono dim">{pool.withAhead} held · refused</b>
            <span className="sub">a published revocation list names this vehicle, so receivers refuse every one of them</span>
          </>
        ) : (
          <>
            <b className="mono">{validNow} valid now</b>
            <span className="sub">
              {thisWeek} of this week's {d.jmax} left
              {pool.ahead > 0 ? ` · ${pool.withAhead} including the ${aheadText(pool.ahead)}` : " · none downloaded for the weeks ahead yet"}
            </span>
          </>
        )}
      </div>
      <div className="pki-tile">
        <span className="k" title="When it next asks its Registration Authority (RA) for a batch of pseudonym certificates">Next top-up</span>
        <b className="mono">{left ? "–" : revoked || gtRevoked ? "none: blocked" : nextTopup(row, nowNs)}</b>
        <span className="sub">
          {d.pseudonym_changes} pseudonym change{d.pseudonym_changes === 1 ? "" : "s"} so far
        </span>
      </div>
      <div className={revoked || gtRevoked ? "pki-tile attn-bad" : "pki-tile"} data-testid="pki-glance-revocation">
        <span className="k" title="Whether a certificate revocation list (CRL) names it, and so whether the other vehicles refuse its messages">
          Revocation
        </span>
        {revoked || gtRevoked ? (
          <>
            <b className="bad">
              <StatusGlyph shape="ban" /> revoked
            </b>
            <span className="sub" data-testid="pki-glance-revoked-by">
              {story && story.lists.length > 0
                ? `on ${story.lists[0].provider}'s list v${story.lists[0].version}, published at ${secondsText(story.lists[0].issued_at)}`
                : "a published list names it"}
              {story?.refusing ? ` · ${story.refusing.installed} of ${story.refusing.of} vehicles refuse it` : ""}
              {story?.caseId ? (
                <>
                  {" · "}
                  <button type="button" className="linklike" onClick={() => openCase(story.caseId as string)} data-testid="pki-glance-case">
                    {caseLabel(story.caseId)} ›
                  </button>
                </>
              ) : null}
            </span>
          </>
        ) : (
          <b className="ok">
            <StatusGlyph shape="check" /> not revoked
          </b>
        )}
        <span className="sub" data-testid="pki-glance-own-list" title="As a receiver: the list it checks other vehicles' messages against">
          {own ? `Its own copy of the list: ${installedListText(own)} (${own.entries} entr${own.entries === 1 ? "y" : "ies"})` : "It has not installed a revocation list yet"}
        </span>
      </div>
    </div>
  );
}

/** "provider-b/case/333" → "case 333". */
function caseLabel(id: string): string {
  return `case ${id.split("/").pop() ?? id}`;
}

function openCase(id: string): void {
  usePkiView.getState().set({ caseId: id });
  chooseTab("cases");
}

/** The lists that revoke a vehicle and its case (ground truth), asked only for a revoked vehicle. */
function useRevocationStory(device: string, revokedBy: readonly string[] | null, heads: readonly CrlHead[] | null): ReturnType<typeof revocationStory> {
  const cases = asCases(usePki(revokedBy !== null ? { topic: "cases", view: GROUND_TRUTH } : null, 4000).data);
  return revokedBy === null ? null : revocationStory(device, revokedBy, heads, cases?.cases ?? null);
}

function VehicleCard({
  device,
  view,
  state,
  row,
  nowNs,
  ec,
  heads,
}: {
  readonly device: string;
  readonly view: string;
  readonly state: CredentialState;
  readonly row: VehicleRow;
  readonly nowNs: number;
  /** Its enrolment certificate's state, from the same answer its card reads, so the row, the header and the card agree. */
  readonly ec: ReturnType<typeof useCertStatus>;
  readonly heads: readonly CrlHead[] | null;
}): React.JSX.Element {
  const d = asVehicle(usePki({ topic: "vehicle", device, view }, 3000).data);
  const panelNow = usePkiView((s) => s.nowNs);
  if (!d) return <div className="pkir-card dim">Loading the certificates of {deviceName(device, row.node)}: its enrolment certificate, pool and history…</div>;
  const left = state === "left";
  const shown = withEc(state, ec, d.enrolled);
  return (
    <div className="pkir-card" data-testid="pki-vehicle-card">
      <h3 className="pki-vehicle-head">
        <VehicleName device={d.device} node={d.node} testId="pki-vehicle-card-name" /> <span className="dim">· {d.provider}</span>
        <VehicleStateBadge st={shown} testId="pki-vehicle-card-state" />
      </h3>
      <p className="faint pki-vstate-meaning">{VEHICLE_STATE[shown].meaning}</p>
      <AtAGlance d={d} row={row} nowNs={nowNs} ec={ec} left={left} heads={heads} />
      <VehicleTimeline d={d} nowNs={Math.max(panelNow, nowNs)} />
      <h4>
        Certificates by week{" "}
        <span className="faint" title="The current i-period: one week from the 2015-01-06 04:00 ET epoch (compressed in a compressed calendar)">
          (this is <Term k="ij">week {d.current_i}</Term>; {d.jmax} certificates a week)
        </span>
      </h4>
      <CredentialGrid d={d} />
      <h4>Pseudonym history</h4>
      <ol className="pkir-history pki-history" data-testid="pki-pseudonyms">
        {d.pseudonyms.map((p, k) => (
          <li key={`${p.id8}/${p.since}`}>
            <CertLink id8={p.id8} holder={d.device} label={grouped(p.id8)} />
            <span className="faint">
              {" "}
              from {secondsText(p.since)}
              {k + 1 < d.pseudonyms.length ? ` for ${secondsText(d.pseudonyms[k + 1].since - p.since)}` : p.id8 === d.active ? " · in use now" : ""}
            </span>
          </li>
        ))}
      </ol>
      {d.active ? (
        <>
          <h4>The certificate it signs with now</h4>
          <CertificateCard id8={d.active} holder={d.device} testId="pki-vehicle-cert" />
        </>
      ) : null}
    </div>
  );
}

/** The i × j grid: one column per week, one row per j, coloured by the cell's state. */
export function CredentialGrid({ d }: { readonly d: Pick<VehicleDetail, "grid" | "current_i" | "truth"> & { readonly device?: string } }): React.JSX.Element {
  const full = useMemo(() => fullGrid(d), [d]);
  const set = usePkiView((s) => s.set);
  const [hover, setHover] = useState<CredCell | null>(null);
  // The pool horizon is years of weeks, almost all not yet available: show the weeks it holds and
  // a few past them, and say how many more there are.
  const lastHeld = Math.max(full.iMin, ...d.grid.held.filter((c) => c.state !== "not-yet-available").map((c) => c.i));
  const shown = Math.min(full.columns.length, Math.max(lastHeld - full.iMin + 1 + WEEKS_PAST_HELD, 12));
  const g = { ...full, columns: full.columns.slice(0, shown), iMax: full.iMin + shown - 1 };
  const hidden = full.columns.length - shown;
  const w = g.columns.length * CELL_PX;
  const h = g.jmax * CELL_PX;
  const revoked = d.truth?.revoked_by ?? {};
  const nowX = 1 + (d.current_i - g.iMin) * CELL_PX;
  return (
    <div className="pkir-grid pki-weekgrid">
      <svg width={w + 2} height={GRID_HEAD + h + 14} role="img" aria-label="Certificate pool by week and index" data-testid="pki-grid">
        {d.current_i >= g.iMin && d.current_i <= g.iMax ? (
          <>
            <rect x={nowX - 1} y={GRID_HEAD - 3} width={CELL_PX + 1} height={h + 5} className="now-col" />
            <text x={nowX + CELL_PX / 2} y={GRID_HEAD - 6} textAnchor="middle" className="axis now-label">
              now
            </text>
          </>
        ) : null}
        {g.columns.map((col, x) =>
          col.map((c) => (
            <rect
              key={`${c.i}/${c.j}`}
              x={1 + x * CELL_PX}
              y={GRID_HEAD + 1 + c.j * CELL_PX}
              width={CELL_PX - 2}
              height={CELL_PX - 2}
              rx={2}
              className={`cell ${c.state}${c.used ? " used" : ""}${c.id8 && revoked[c.id8] ? " gt-revoked" : ""}${c.id8 ? " has-id" : ""}`}
              data-state={c.state}
              data-i={c.i}
              data-j={c.j}
              onMouseEnter={() => setHover(c)}
              onClick={c.id8 ? () => set({ cert: { id8: c.id8 as string, holder: d.device ?? null } }) : undefined}
            >
              <title>{`week ${c.i}, certificate ${c.j + 1} of ${g.jmax}: ${CELL_LABEL[c.state as CellState] ?? c.state}${c.id8 ? ` · ${grouped(c.id8)} (click for its card)` : ""}`}</title>
            </rect>
          )),
        )}
        <line x1={nowX - 0.5} x2={nowX - 0.5} y1={GRID_HEAD} y2={GRID_HEAD + h + 2} className="now" />
        <text x={1} y={GRID_HEAD + h + 12} className="axis">
          week {g.iMin}
        </text>
        <text x={w} y={GRID_HEAD + h + 12} className="axis" textAnchor="end">
          week {g.iMax}
        </text>
      </svg>
      <div className="pkir-legend pki-cell-legend">
        {(Object.keys(CELL_LABEL) as CellState[]).map((s) => {
          const st = cellStatus(s);
          return (
            <span key={s} title={st ? undefined : "An empty slot: the vehicle has not downloaded this week's batch yet"}>
              <svg width="12" height="12" aria-hidden="true">
                <rect x="0.5" y="0.5" width="11" height="11" rx="2" className={`cell ${s}`} />
              </svg>
              {CELL_LABEL[s]}
            </span>
          );
        })}
        {Object.keys(revoked).length > 0 ? (
          <span>
            <svg width="12" height="12" aria-hidden="true">
              <rect x="0.5" y="0.5" width="11" height="11" rx="2" className="cell valid gt-revoked" />
            </svg>
            revoked by a published CRL (ground truth)
          </span>
        ) : null}
      </div>
      <div className="faint mono pkir-hover">
        {hover
          ? `week ${hover.i} · certificate ${hover.j + 1} of ${g.jmax} · ${CELL_LABEL[hover.state as CellState] ?? hover.state}${hover.id8 ? ` · ${grouped(hover.id8)}` : ""}${hover.used ? " · has signed messages" : ""}`
          : `weeks ${g.iMin}–${g.iMax} across, the week's ${g.jmax} certificates down${hidden > 0 ? ` · ${hidden} later weeks, not yet available, not drawn` : ""} · click a certificate for its card`}
      </div>
    </div>
  );
}

export function Locked(): React.JSX.Element {
  return (
    <div className="pkir-card dim" data-testid="pki-locked">
      Ground truth is locked off for a blind evaluation. Choose an entity in “view” to see what it knows.
    </div>
  );
}

export function hex(c: number): string {
  return `#${c.toString(16).padStart(6, "0")}`;
}
