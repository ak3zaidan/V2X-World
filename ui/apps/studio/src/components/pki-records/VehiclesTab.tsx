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
import {
  CELL_LABEL,
  CREDENTIAL_LABEL,
  CREDENTIAL_ORDER,
  GROUND_TRUTH,
  asCrls,
  asVehicle,
  asVehicles,
  credentialState,
  fullGrid,
  overdueProviders,
  providerColor,
  secondsText,
  slaText,
  type CellState,
  type CredCell,
  type CredentialContext,
  type CredentialState,
  type VehicleDetail,
  type VehicleRow,
} from "../../lib/pkiRecords.js";
import { CertificateCard, CertLink } from "../pki/CertificateCard.js";
import { useCertStatus } from "../pki/useCert.js";
import { CertBadge, StatusGlyph } from "../pki/CertStatus.js";
import { Term } from "../pki/Term.js";
import { usePki, usePkiView } from "./state.js";
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

function crlCell(r: VehicleRow): string {
  return r.security?.crl_version !== undefined ? `v${r.security.crl_version}` : "none";
}

function crlTitle(r: VehicleRow): string {
  const n = r.security?.crl_entries ?? 0;
  return r.security?.crl_version !== undefined ? `Revocation list version ${r.security.crl_version}, ${n} entr${n === 1 ? "y" : "ies"}` : "No revocation list installed";
}

export function VehiclesTab({ view }: { readonly view: string | null }): React.JSX.Element {
  const node = usePkiView((s) => s.node);
  const set = usePkiView((s) => s.set);
  const [only, setOnly] = useState<CredentialState | null>(null);
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
  if (view === null) return <Locked />;
  const all = list?.vehicles ?? [];
  const labels = all.map((r) => r.provider);
  const stateOf = new Map(all.map((r) => [r.device, credentialState(r, ctx)]));
  const rank = (r: VehicleRow): number => CREDENTIAL_ORDER.indexOf(stateOf.get(r.device) ?? "healthy");
  // Problems first, then routine work, then the vehicles that are fine, then those that left.
  const sorted = [...all].sort((a, b) => rank(a) - rank(b) || a.node - b.node);
  const rows = only === null ? sorted : sorted.filter((r) => stateOf.get(r.device) === only);
  const chosen = all.find((r) => r.node === node) ?? null;
  const counts = new Map<CredentialState, number>();
  for (const st of stateOf.values()) counts.set(st, (counts.get(st) ?? 0) + 1);
  // Columns that never differ are one sentence, not a column of copies.
  const ctl = uniform(all, (r) => r.ctl_sequence ?? null);
  const crl = uniform(all, crlCell);
  const enrolled = uniform(all, (r) => r.enrolled);
  const allEnrolled = all.length > 0 && enrolled.same && enrolled.value === true;
  const same: string[] = [];
  if (allEnrolled) same.push("enrolled with its provider");
  if (all.length > 0 && ctl.same) same.push(ctl.value === null ? "no trust list (CTL) reported" : `trust list (CTL) version ${String(ctl.value)}`);
  if (all.length > 0 && crl.same) same.push(crl.value === "none" ? "no revocation list (CRL) installed" : `${crlTitle(all[0]).replace(/^Revocation list/, "revocation list (CRL)")} installed`);
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
            <button type="button" className={only === null ? "pki-chip on" : "pki-chip"} aria-pressed={only === null} onClick={() => setOnly(null)} data-testid="pki-chip-all">
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
            Every vehicle: {same.join(" · ")}.
          </p>
        ) : null}
        <table className="pkir-table pki-vtable" data-testid="pki-vehicles-table">
          <thead>
            <tr>
              <th>Vehicle</th>
              <th title="Its credential state, in the same shapes and colours as every certificate; problems are listed first">State</th>
              <th>Provider</th>
              {allEnrolled ? null : <th title="Holds an enrolment certificate from its provider's ECA">Enrolled</th>}
              <th title="Pseudonym certificates it can sign with now, and (+) how many weeks ahead it has downloaded">Usable</th>
              <th title="When it next asks its RA for a batch">Next top-up</th>
              {ctl.same ? null : <th title="The certificate trust list (CTL) version it holds">Trust list</th>}
              {crl.same ? null : <th title="The certificate revocation list (CRL) it installed">Revocation list</th>}
              <th title="Pseudonym certificates it has signed with so far">Pseudonyms used</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const st = stateOf.get(r.device) ?? "healthy";
              const gone = st === "left";
              const cls = [r.node === node ? "sel" : "", gone ? "left" : ""].filter(Boolean).join(" ");
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
                  <td title={`${r.device} · simulation node ${r.node}`}>{deviceName(r.device)}</td>
                  <td>
                    <VehicleStateBadge st={st} testId="pki-vehicle-state" />
                  </td>
                  <td>
                    <span className="pkir-swatch" style={{ background: hex(providerColor(r.provider, labels)) }} />
                    {r.provider}
                  </td>
                  {allEnrolled ? null : <td>{r.enrolled ? "yes" : "no"}</td>}
                  <td
                    className="mono"
                    title={`${r.usable} usable now; ${r.periods_ahead ?? 0} week(s) downloaded past this one${r.provisioned ? "" : "; it has not downloaded any pseudonym certificates yet"}`}
                  >
                    {r.usable}
                    <span className="faint"> +{r.periods_ahead ?? 0} wk</span>
                  </td>
                  <td className="mono">{gone ? "–" : nextTopup(r, list?.t_ns ?? 0)}</td>
                  {ctl.same ? null : <td className="mono">{r.ctl_sequence ?? "–"}</td>}
                  {crl.same ? null : (
                    <td className="mono" title={crlTitle(r)}>
                      {crlCell(r)}
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
          <p className="dim pkir-pad">No vehicle is in that state now.</p>
        ) : null}
        <p className="faint pkir-pad">
          {gt ? "Ground truth: “revoked” includes vehicles a published CRL revokes that have not installed it yet." : `As ${deviceName(view)}: only what that vehicle holds.`}
        </p>
      </div>
      {chosen ? (
        <VehicleCard device={chosen.device} view={gt ? GROUND_TRUTH : chosen.device} state={stateOf.get(chosen.device) ?? "healthy"} row={chosen} nowNs={list?.t_ns ?? 0} />
      ) : null}
    </div>
  );
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
}: {
  readonly d: VehicleDetail;
  readonly row: VehicleRow;
  readonly nowNs: number;
  readonly ec: ReturnType<typeof useCertStatus>;
  readonly left: boolean;
}): React.JSX.Element {
  const a = activeCell(d);
  const thisWeek = d.grid.held.filter((c) => c.i === d.current_i && (c.state === "valid" || c.state === "active")).length;
  const last = [...d.pseudonyms].sort((x, y) => y.since - x.since)[0];
  const revoked = d.revoked || row.security?.self_revoked === true;
  const gtRevoked = (d.truth?.revoked_certs ?? 0) > 0;
  const ecOk = ec.status === "valid" || ec.status === "expiring" || ec.status === "unknown";
  const n = row.security?.crl_entries ?? 0;
  return (
    <div className="pki-glance" data-testid="pki-vehicle-glance">
      <div className={ecOk ? "pki-tile" : "pki-tile attn"}>
        <span className="k" title="Enrolment certificate (EC): issued once by the provider's Enrolment CA (ECA); proves the vehicle may ask for pseudonym certificates">
          Enrolment certificate (EC)
        </span>
        {d.enrolment ? (
          <span className="pki-tile-cert">
            <CertBadge status={ec.status} compact testId="pki-glance-ec-status" />
            <CertLink id8={d.enrolment} holder={d.device} label={grouped(d.enrolment)} testId="pki-glance-ec" />
          </span>
        ) : (
          <b>none</b>
        )}
        <span className="sub" data-testid="pki-glance-ec-says">
          {!d.enrolled
            ? "not enrolled"
            : ecOk
              ? "enrolled with its provider"
              : ec.zeroLifetime
                ? "issued with a lifetime of zero, so a receiver's time check reads it as expired (open it)"
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
      <div className="pki-tile">
        <span
          className="k"
          title={`Pseudonym certificates it can still sign with: what is left of this week's ${d.jmax}, plus the ${d.jmax} a week it has downloaded for the weeks ahead`}
        >
          Pool
        </span>
        <b className="mono">{d.usable} usable</b>
        <span className="sub">
          {thisWeek} of this week's {d.jmax} left · {d.periods_ahead ?? 0} week{d.periods_ahead === 1 ? "" : "s"} downloaded ahead
        </span>
      </div>
      <div className="pki-tile">
        <span className="k" title="When it next asks its Registration Authority (RA) for a batch of pseudonym certificates">Next top-up</span>
        <b className="mono">{left ? "–" : nextTopup(row, nowNs)}</b>
        <span className="sub">
          {d.pseudonym_changes} pseudonym change{d.pseudonym_changes === 1 ? "" : "s"} so far
        </span>
      </div>
      <div className="pki-tile">
        <span className="k" title="Whether a certificate revocation list (CRL) names it">Revocation</span>
        {revoked ? (
          <b className="bad">revoked</b>
        ) : gtRevoked ? (
          <b className="warn" title="Ground truth: a published list revokes it, but it has not installed that list yet">
            revoked, not yet heard
          </b>
        ) : (
          <b className="ok">not revoked</b>
        )}
        <span className="sub">
          {row.security?.crl_version !== undefined ? `has list v${row.security.crl_version} (${n} entr${n === 1 ? "y" : "ies"})` : "no list installed"}
        </span>
      </div>
    </div>
  );
}

function VehicleCard({
  device,
  view,
  state,
  row,
  nowNs,
}: {
  readonly device: string;
  readonly view: string;
  readonly state: CredentialState;
  readonly row: VehicleRow;
  readonly nowNs: number;
}): React.JSX.Element {
  const d = asVehicle(usePki({ topic: "vehicle", device, view }, 3000).data);
  const panelNow = usePkiView((s) => s.nowNs);
  // The EC's own state, from the same answer its card reads, so the tile, the header and the card agree.
  const ec = useCertStatus(d?.enrolment ?? null);
  if (!d) return <div className="pkir-card dim">Loading the certificates of {deviceName(device)}…</div>;
  const left = state === "left";
  const ecBad = d.enrolled && !(ec.status === "valid" || ec.status === "expiring" || ec.status === "unknown");
  // A vehicle whose enrolment certificate fails is never "healthy", whatever its pool says.
  const shown: CredentialState | "ec-bad" = ecBad && (state === "healthy" || state === "between" || state === "topup-due") ? "ec-bad" : state;
  return (
    <div className="pkir-card" data-testid="pki-vehicle-card">
      <h3 className="pki-vehicle-head">
        <span title={`${d.device} · simulation node ${d.node}`}>{deviceName(d.device)}</span> <span className="dim">· {d.provider}</span>
        <VehicleStateBadge st={shown} testId="pki-vehicle-card-state" />
      </h3>
      <p className="faint pki-vstate-meaning">{VEHICLE_STATE[shown].meaning}</p>
      <AtAGlance d={d} row={row} nowNs={nowNs} ec={ec} left={left} />
      <VehicleTimeline d={d} nowNs={Math.max(panelNow, nowNs)} />
      <h4>
        Certificates by week{" "}
        <span className="faint" title="The current i-period: one week from the 2015-01-06 04:00 ET epoch (compressed in a compressed calendar)">
          (this is week i = {d.current_i}; {d.jmax} certificates a week)
        </span>
      </h4>
      <CredentialGrid d={d} />
      <h4>Pseudonym history</h4>
      <ol className="pkir-history pki-history" data-testid="pki-pseudonyms">
        {d.pseudonyms.map((p, k) => (
          <li key={`${p.id8}/${p.since}`}>
            <CertLink id8={p.id8} holder={d.device} label={p.id8} />
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
              <title>{`week ${c.i}, certificate ${c.j}: ${CELL_LABEL[c.state as CellState] ?? c.state}${c.id8 ? ` · ${c.id8} (click for its card)` : ""}`}</title>
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
          ? `week i ${hover.i} · certificate j ${hover.j} · ${CELL_LABEL[hover.state as CellState] ?? hover.state}${hover.id8 ? ` · ${hover.id8}` : ""}${hover.used ? " · has signed messages" : ""}`
          : `weeks ${g.iMin}–${g.iMax} × j 0–${g.jmax - 1}${hidden > 0 ? ` · ${hidden} later weeks, not yet available, not drawn` : ""} · click a certificate for its card`}
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
