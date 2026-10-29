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

import { VEHICLE_STATE, cellStatus, grouped } from "../../lib/pkiCert.js";
import {
  CELL_LABEL,
  CREDENTIAL_LABEL,
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
import { StatusGlyph } from "../pki/CertStatus.js";
import { usePki, usePkiView } from "./state.js";
import { VehicleTimeline } from "./VehicleTimeline.js";

const CELL_PX = 14;
/** Weeks shown past the last one the vehicle holds a certificate for. */
const WEEKS_PAST_HELD = 3;
const GRID_HEAD = 16;

/** A vehicle's state as a badge in the certificates' shared language. */
export function VehicleStateBadge({ st, testId }: { readonly st: CredentialState; readonly testId?: string }): React.JSX.Element {
  const v = VEHICLE_STATE[st];
  return (
    <span className={`pkir-state ${st} pki-vstate`} style={{ color: v.color }} title={v.meaning} data-testid={testId}>
      <StatusGlyph shape={v.shape} />
      {CREDENTIAL_LABEL[st]}
    </span>
  );
}

export function VehiclesTab({ view }: { readonly view: string | null }): React.JSX.Element {
  const node = usePkiView((s) => s.node);
  const set = usePkiView((s) => s.set);
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
  const rows = list?.vehicles ?? [];
  const labels = rows.map((r) => r.provider);
  const chosen = rows.find((r) => r.node === node) ?? null;
  const counts = new Map<CredentialState, number>();
  for (const r of rows) {
    const st = credentialState(r, ctx);
    counts.set(st, (counts.get(st) ?? 0) + 1);
  }
  return (
    <div className="pkir-split" data-testid="pki-vehicles">
      <div className="pkir-table-wrap">
        <p className="pkir-pad pki-lead">
          Each vehicle enrols once (an <b>enrolment certificate</b>, its long-term credential), then signs its safety messages with short-lived{" "}
          <b>pseudonym certificates</b> it downloads a week at a time. Choose a vehicle to see its certificates.
        </p>
        {rows.length > 0 ? (
          <p className="pkir-pad pki-vsummary" data-testid="pki-vehicles-summary">
            {rows.length} vehicle{rows.length === 1 ? "" : "s"}:
            {(Object.keys(VEHICLE_STATE) as CredentialState[])
              .filter((s) => (counts.get(s) ?? 0) > 0)
              .map((s) => (
                <span key={s} className="pki-vsummary-item">
                  <VehicleStateBadge st={s} /> <b className="mono">{counts.get(s)}</b>
                </span>
              ))}
          </p>
        ) : null}
        <table className="pkir-table" data-testid="pki-vehicles-table">
          <thead>
            <tr>
              <th>Vehicle</th>
              <th>Provider</th>
              <th title="Holds an enrolment certificate from its provider's ECA">Enrolled</th>
              <th title="Pseudonym (authorization) certificates it can sign with, this week and later">Usable</th>
              <th title="Weeks (i-periods) of certificates it holds past this one">Weeks ahead</th>
              <th title="When it next asks the RA for a batch">Next top-up</th>
              <th title="The certificate revocation list (CRL) it installed">Revocation list</th>
              <th title="The certificate trust list (CTL) sequence it holds">Trust list</th>
              <th title="Pseudonyms it has signed with">Pseudonyms</th>
              <th>State</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => {
              const st = credentialState(r, ctx);
              return (
                <tr
                  key={r.device}
                  className={r.node === node ? "sel" : undefined}
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
                >
                  <td className="mono">
                    {r.device}
                    <span className="faint"> · node {r.node}</span>
                  </td>
                  <td>
                    <span className="pkir-swatch" style={{ background: hex(providerColor(r.provider, labels)) }} />
                    {r.provider}
                  </td>
                  <td>
                    {r.enrolled ? "yes" : "no"}
                    {r.provisioned ? "" : " · not provisioned"}
                  </td>
                  <td className="mono">{r.usable}</td>
                  <td className="mono">{r.periods_ahead ?? "–"}</td>
                  <td className="mono">{nextTopup(r, list?.t_ns ?? 0)}</td>
                  <td className="mono">{r.security?.crl_version !== undefined ? `v${r.security.crl_version} · ${r.security.crl_entries ?? 0} entries` : "–"}</td>
                  <td className="mono">{r.ctl_sequence ?? "–"}</td>
                  <td className="mono">{r.pseudonym_changes + (r.active ? 1 : 0)}</td>
                  <td>
                    <VehicleStateBadge st={st} testId="pki-vehicle-state" />
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {rows.length === 0 ? (
          <p className="dim pkir-pad">No vehicle has published its credentials yet: each vehicle reports its certificates once a simulated second after it enters the run.</p>
        ) : null}
        <p className="faint pkir-pad">
          {gt ? "Ground truth: “revoked” includes vehicles a published CRL revokes that have not installed it yet." : `As ${view}: only what that vehicle holds.`}
        </p>
      </div>
      {chosen ? (
        <VehicleCard device={chosen.device} view={gt ? GROUND_TRUTH : chosen.device} state={credentialState(chosen, ctx)} row={chosen} nowNs={list?.t_ns ?? 0} />
      ) : (
        <div className="pkir-card dim pki-choose">
          <p>Choose a vehicle in the table to see:</p>
          <ul>
            <li>the certificates it holds now and the one it signs with,</li>
            <li>its certificate life on the run's clock: enrolment, downloads, pseudonym changes, reports, revocation,</li>
            <li>its pool week by week, and</li>
            <li>the checks a receiver runs on its certificate.</li>
          </ul>
        </div>
      )}
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
function AtAGlance({ d, row, nowNs }: { readonly d: VehicleDetail; readonly row: VehicleRow; readonly nowNs: number }): React.JSX.Element {
  const a = activeCell(d);
  const thisWeek = d.grid.held.filter((c) => c.i === d.current_i && (c.state === "valid" || c.state === "active")).length;
  const last = [...d.pseudonyms].sort((x, y) => y.since - x.since)[0];
  const revoked = d.revoked || row.security?.self_revoked === true;
  const gtRevoked = (d.truth?.revoked_certs ?? 0) > 0;
  return (
    <div className="pki-glance" data-testid="pki-vehicle-glance">
      <div className="pki-tile">
        <span className="k" title="Enrolment certificate (EC): issued once by the provider's ECA; proves the vehicle may ask for pseudonym certificates">
          Enrolment certificate (EC)
        </span>
        {d.enrolment ? <CertLink id8={d.enrolment} holder={d.device} label={grouped(d.enrolment)} testId="pki-glance-ec" /> : <b>none</b>}
        <span className="sub">{d.enrolled ? "enrolled with its provider" : "not enrolled"}</span>
      </div>
      <div className="pki-tile">
        <span className="k" title="The pseudonym certificate (PC) it signs its safety messages with now">Signing with (PC)</span>
        {d.active ? <CertLink id8={d.active} holder={d.device} label={grouped(d.active)} testId="pki-glance-pc" /> : <b className="bad">no certificate</b>}
        <span className="sub">
          {a ? `week i = ${a.i}, certificate j = ${a.j}` : "—"}
          {last ? ` · since ${secondsText(last.since)}` : ""}
        </span>
      </div>
      <div className="pki-tile">
        <span className="k" title="Pseudonym certificates it can still sign with, this week and the weeks it has downloaded">Pool</span>
        <b className="mono">{d.usable} usable</b>
        <span className="sub">
          {thisWeek} of {d.jmax} left this week · {d.periods_ahead ?? 0} week{d.periods_ahead === 1 ? "" : "s"} ahead
        </span>
      </div>
      <div className="pki-tile">
        <span className="k" title="When it next asks its Registration Authority (RA) for a batch of pseudonym certificates">Next top-up</span>
        <b className="mono">{nextTopup(row, nowNs)}</b>
        <span className="sub">{d.pseudonym_changes} pseudonym change{d.pseudonym_changes === 1 ? "" : "s"} so far</span>
      </div>
      <div className="pki-tile">
        <span className="k" title="Whether a certificate revocation list (CRL) names it">Revocation</span>
        {revoked ? (
          <b className="bad">revoked</b>
        ) : gtRevoked ? (
          <b className="warn" title="Ground truth: a published list revokes it, but it has not installed that list yet">revoked, not yet heard</b>
        ) : (
          <b className="ok">not revoked</b>
        )}
        <span className="sub">
          {row.security?.crl_version !== undefined ? `list v${row.security.crl_version} installed (${row.security.crl_entries ?? 0} entries)` : "no list installed"}
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
  if (!d) return <div className="pkir-card dim">Loading {device}’s certificates…</div>;
  return (
    <div className="pkir-card" data-testid="pki-vehicle-card">
      <h3 className="pki-vehicle-head">
        <span className="mono">{d.device}</span> <span className="dim">· {d.provider}</span>
        <VehicleStateBadge st={state} />
      </h3>
      <p className="faint pki-vstate-meaning">{VEHICLE_STATE[state].meaning}</p>
      <AtAGlance d={d} row={row} nowNs={nowNs} />
      <VehicleTimeline d={d} />
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
