/**
 * The PKI view's Vehicles tab (00-gap-analysis §3.2): every vehicle's provider, enrolment and
 * provisioning state, pool, next top-up, installed CRL and CTL; selecting one shows its i × j
 * grid (weeks across, the 20 certificates of a week down), its pseudonym history and the active
 * certificate's decode with the validation a US device runs on it, step by step.
 *
 * Everything is `inspect.pki {topic: "vehicles" | "vehicle"}` and `inspect.cert`; the grid is
 * the vehicle's own `node.credentials`, rebuilt cell for cell (`lib/pkiRecords.ts` `fullGrid`).
 */

import { useEffect, useMemo, useState } from "react";

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
  type VehicleDetail,
  type VehicleRow,
} from "../../lib/pkiRecords.js";
import { engine } from "../../state/engine.js";
import { usePki, usePkiView } from "./state.js";

const CELL_PX = 11;
/** Weeks shown past the last one the vehicle holds a certificate for. */
const WEEKS_PAST_HELD = 3;

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
  return (
    <div className="pkir-split" data-testid="pki-vehicles">
      <div className="pkir-table-wrap">
        <table className="pkir-table" data-testid="pki-vehicles-table">
          <thead>
            <tr>
              <th>Vehicle</th><th>Provider</th><th>Enrolled</th><th>Usable pool</th><th>Weeks ahead</th>
              <th>Next top-up</th><th>CRL installed</th><th>CTL</th><th>Pseudonyms</th><th>State</th>
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
                  data-testid="pki-vehicle-row"
                  data-node={r.node}
                >
                  <td className="mono">{r.device}<span className="faint"> · node {r.node}</span></td>
                  <td><span className="pkir-swatch" style={{ background: hex(providerColor(r.provider, labels)) }} />{r.provider}</td>
                  <td>{r.enrolled ? "yes" : "no"}{r.provisioned ? "" : " · not provisioned"}</td>
                  <td className="mono">{r.usable}</td>
                  <td className="mono">{r.periods_ahead ?? "–"}</td>
                  <td className="mono">{nextTopup(r, list?.t_ns ?? 0)}</td>
                  <td className="mono">{r.security?.crl_version !== undefined ? `v${r.security.crl_version} · ${r.security.crl_entries ?? 0} entries` : "–"}</td>
                  <td className="mono">{r.ctl_sequence ?? "–"}</td>
                  <td className="mono">{r.pseudonym_changes + (r.active ? 1 : 0)}</td>
                  <td><span className={`pkir-state ${st}`} data-testid="pki-vehicle-state">{CREDENTIAL_LABEL[st]}</span></td>
                </tr>
              );
            })}
          </tbody>
        </table>
        {rows.length === 0 ? <p className="dim pkir-pad">No vehicle has published its credentials yet.</p> : null}
        <p className="faint pkir-pad">
          {gt
            ? "Ground truth: “revoked” includes vehicles a published CRL revokes that have not installed it yet."
            : `As ${view}: only what that vehicle holds.`}
        </p>
      </div>
      {chosen ? <VehicleCard device={chosen.device} view={gt ? GROUND_TRUTH : chosen.device} /> : <div className="pkir-card dim">Choose a vehicle.</div>}
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

function VehicleCard({ device, view }: { readonly device: string; readonly view: string }): React.JSX.Element {
  const d = asVehicle(usePki({ topic: "vehicle", device, view }, 3000).data);
  if (!d) return <div className="pkir-card dim">Loading {device}…</div>;
  return (
    <div className="pkir-card" data-testid="pki-vehicle-card">
      <h3 className="mono">{d.device} <span className="dim">· {d.provider}</span></h3>
      <div className="pkir-facts">
        <span>enrolment <b className="mono">{d.enrolment ?? "none"}</b></span>
        <span>week i = <b className="mono">{d.current_i}</b></span>
        <span>active <b className="mono">{d.active ?? "none"}</b></span>
        <span>records <b className="mono">{d.records}</b></span>
      </div>
      <CredentialGrid d={d} />
      <h4>Pseudonym history</h4>
      <ol className="pkir-history mono" data-testid="pki-pseudonyms">
        {d.pseudonyms.map((p) => (
          <li key={`${p.id8}/${p.since}`}>{p.id8} <span className="faint">from {secondsText(p.since)}</span></li>
        ))}
      </ol>
      {d.active ? <CertTrace id8={d.active} /> : null}
    </div>
  );
}

/** The i × j grid: one column per week, one row per j, coloured by the cell's state. */
export function CredentialGrid({ d }: { readonly d: Pick<VehicleDetail, "grid" | "current_i" | "truth"> }): React.JSX.Element {
  const full = useMemo(() => fullGrid(d), [d]);
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
  return (
    <div className="pkir-grid">
      <svg width={w + 2} height={h + 14} role="img" aria-label="Certificate pool by week and index" data-testid="pki-grid">
        {g.columns.map((col, x) =>
          col.map((c) => (
            <rect
              key={`${c.i}/${c.j}`}
              x={1 + x * CELL_PX}
              y={1 + c.j * CELL_PX}
              width={CELL_PX - 1}
              height={CELL_PX - 1}
              className={`cell ${c.state}${c.used ? " used" : ""}${c.id8 && revoked[c.id8] ? " gt-revoked" : ""}`}
              data-state={c.state}
              data-i={c.i}
              data-j={c.j}
              onMouseEnter={() => setHover(c)}
            />
          )),
        )}
        <line x1={1 + (d.current_i - g.iMin) * CELL_PX - 0.5} x2={1 + (d.current_i - g.iMin) * CELL_PX - 0.5} y1={0} y2={h + 2} className="now" />
        <text x={1} y={h + 12} className="axis">week {g.iMin}</text>
        <text x={w} y={h + 12} className="axis" textAnchor="end">week {g.iMax}</text>
      </svg>
      <div className="pkir-legend">
        {(Object.keys(CELL_LABEL) as CellState[]).map((s) => (
          <span key={s}><i className={`cell ${s}`} />{CELL_LABEL[s]}</span>
        ))}
        {Object.keys(revoked).length > 0 ? <span><i className="cell gt-revoked" />revoked by a published CRL (ground truth)</span> : null}
      </div>
      <div className="faint mono pkir-hover">
        {hover ? `i ${hover.i} · j ${hover.j} · ${hover.state}${hover.id8 ? ` · ${hover.id8}` : ""}${hover.used ? " · signed with" : ""}` : `weeks ${g.iMin}–${g.iMax} × j 0–${g.jmax - 1}${hidden > 0 ? ` · ${hidden} later weeks, not yet available, not drawn` : ""}`}
      </div>
    </div>
  );
}

interface CertAnswer {
  readonly valid?: boolean;
  readonly chain?: readonly string[];
  readonly trace?: readonly { readonly rule: string; readonly status: string; readonly detail: readonly string[] }[];
  readonly tree?: unknown;
}

/** The active certificate's chain-validation trace, as `inspect.cert` answers it. */
function CertTrace({ id8 }: { readonly id8: string }): React.JSX.Element {
  const [a, setA] = useState<CertAnswer | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    engine
      .request("inspect.cert", { hashedid8: id8 }, { quiet: true, timeoutMs: 5000 })
      .then((v) => live && setA(v as CertAnswer))
      .catch((e: unknown) => live && setErr(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
    };
  }, [id8]);
  if (err) return <p className="faint">inspect.cert: {err}</p>;
  if (!a) return <p className="dim">Validating {id8}…</p>;
  return (
    <div data-testid="pki-cert-trace">
      <h4>Active certificate <span className={a.valid ? "ok" : "bad"}>{a.valid ? "valid" : "not valid"}</span></h4>
      <div className="faint mono">chain {(a.chain ?? []).join(" → ")}</div>
      <ul className="pkir-trace">
        {(a.trace ?? []).map((s) => (
          <li key={s.rule} className={s.status}>
            <b>{s.status}</b> {s.rule}{s.detail.length ? <span className="faint"> — {s.detail.join("; ")}</span> : null}
          </li>
        ))}
      </ul>
      <details>
        <summary>Decoded (JER)</summary>
        <pre className="pkir-pre">{JSON.stringify(a.tree, null, 1)}</pre>
      </details>
    </div>
  );
}

export function Locked(): React.JSX.Element {
  return (
    <div className="pkir-card dim" data-testid="pki-locked">
      Ground truth is locked off for a blind evaluation. Choose an entity to see what it knows.
    </div>
  );
}

export function hex(c: number): string {
  return `#${c.toString(16).padStart(6, "0")}`;
}
