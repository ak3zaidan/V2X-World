/**
 * The PKI view's Revocation-lists tab (00-gap-analysis §3.2): one row per provider × series ×
 * kind with its issueDate, nextCrl, size, entries and SLA clock; expanding a row lists every
 * version, and choosing one decodes it — each linkage entry as (la1Id, la2Id, iRev, iMax, jMax)
 * with its seeds and, in ground truth only, the run's certificates it revokes and whose they
 * are. The CTL and its history are below.
 *
 * The entries are what `v2xw_sec::crl::decode_crl` read from the published bytes, and a match
 * is the walk a device makes (`v2xw_sec::crl::revokes`); nothing here decodes or matches.
 */

import { Fragment, useState } from "react";

import {
  GROUND_TRUTH,
  asCrl,
  asCrls,
  crlRows,
  secondsText,
  slaText,
  time32Text,
  type CrlHead,
  type CtlView,
  type ProviderView,
} from "../../lib/pkiRecords.js";
import { usePki, usePkiView } from "./state.js";

export function RevocationTab({ view }: { readonly view: string | null }): React.JSX.Element {
  const crls = asCrls(usePki({ topic: "crls" }, 2000).data);
  const chosen = usePkiView((s) => s.crl);
  const set = usePkiView((s) => s.set);
  const [open, setOpen] = useState<string | null>(null);
  const rows = crls ? crlRows(crls) : [];
  return (
    <div className="pkir-split" data-testid="pki-crls">
      <div className="pkir-table-wrap">
        <table className="pkir-table" data-testid="pki-crl-table">
          <thead>
            <tr>
              <th>Provider</th><th title="crlSeries (IEEE 1609.2 §7.3)">Series</th><th title="Linkage-based entries (pseudonym certificates) or hash-based entries (other certificates)">Kind</th>
              <th>Version</th><th title="issueDate">Issued</th><th title="nextCrl: the latest instant the next list is due">Next due</th>
              <th>Entries</th><th>Size</th><th title="The publication SLA clock: time left until nextCrl; negative is overdue">Due in</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <Fragment key={r.key}>
                <tr
                  onClick={() => {
                    setOpen(open === r.key ? null : r.key);
                    if (open !== r.key) set({ crl: r.newest.id8 });
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      setOpen(open === r.key ? null : r.key);
                      if (open !== r.key) set({ crl: r.newest.id8 });
                    }
                  }}
                  tabIndex={0}
                  aria-expanded={open === r.key}
                  className={open === r.key ? "sel" : undefined}
                  data-testid="pki-crl-row"
                  data-key={r.key}
                >
                  <td>{r.provider}</td>
                  <td className="mono">{r.series}</td>
                  <td>{r.kind === "linkage" ? "pseudonyms (linkage)" : r.kind === "hash" ? "certificates (hash)" : r.kind}</td>
                  <td className="mono">v{r.newest.version}{r.newest.delta ? " Δ" : ""} <span className="faint">({r.versions.length} lists)</span></td>
                  <td className="mono">{time32Text(r.newest.issue_date)}</td>
                  <td className="mono">{time32Text(r.newest.next_crl)}</td>
                  <td className="mono">{r.newest.entries}</td>
                  <td className="mono">{r.newest.bytes} B</td>
                  <td className={`mono ${r.newest.sla_now_s < 0 ? "bad" : ""}`} data-testid="pki-crl-sla">
                    <SlaBar head={r.newest} />
                    {slaText(r.newest.sla_now_s)}
                  </td>
                </tr>
                {open === r.key
                  ? r.versions.map((v) => (
                      <tr
                        key={`${r.key}/${v.id8}`}
                        className={`sub${chosen === v.id8 ? " sel" : ""}`}
                        onClick={() => set({ crl: v.id8 })}
                        data-testid="pki-crl-version"
                        data-id8={v.id8}
                      >
                        <td className="mono" colSpan={3}>{v.id8}</td>
                        <td className="mono">v{v.version}{v.delta ? " Δ delta" : " full"}</td>
                        <td className="mono">{time32Text(v.issue_date)}</td>
                        <td className="mono">{time32Text(v.next_crl)}</td>
                        <td className="mono">{v.entries}</td>
                        <td className="mono">{v.bytes} B</td>
                        <td className="mono faint">issued at {secondsText(v.issued_at)}</td>
                      </tr>
                    ))
                  : null}
              </Fragment>
            ))}
          </tbody>
        </table>
        {rows.length === 0 ? <p className="dim pkir-pad">No revocation list (CRL) has been published yet.</p> : null}
        {crls ? <Providers providers={crls.providers} /> : null}
        {crls?.ctl ? <Ctl ctl={crls.ctl} /> : null}
      </div>
      {chosen ? <CrlCard id8={chosen} view={view} head={crls?.crls.find((c) => c.id8 === chosen) ?? null} /> : <div className="pkir-card dim">Choose a list to see its entries.</div>}
    </div>
  );
}

/** How much of the list's validity window (issueDate to nextCrl) has gone, as a bar. */
function SlaBar({ head }: { readonly head: CrlHead }): React.JSX.Element {
  const window = Math.max(1, head.next_crl - head.issue_date);
  const gone = Math.min(1, Math.max(0, 1 - head.sla_now_s / window));
  return (
    <span className={`pkir-sla${head.sla_now_s < 0 ? " over" : gone > 0.8 ? " near" : ""}`} aria-hidden="true">
      <i style={{ width: `${(gone * 100).toFixed(1)}%` }} />
    </span>
  );
}

function CrlCard({ id8, view, head }: { readonly id8: string; readonly view: string | null; readonly head: CrlHead | null }): React.JSX.Element {
  // A CRL is public; only the join onto the run's devices needs ground truth.
  const v = asCrl(usePki({ topic: "crl", hashedid8: id8, view: view ?? "public" }, 0).data);
  if (!v) return <div className="pkir-card dim">Decoding {id8}…</div>;
  const gt = view === GROUND_TRUTH;
  return (
    <div className="pkir-card" data-testid="pki-crl-card">
      <h3 className="mono">{id8} <span className="dim">· {v.head.provider} series {v.head.series} v{v.head.version}{v.head.delta ? " (delta)" : ""}</span></h3>
      <div className="pkir-facts">
        <span>{v.len} bytes</span>
        <span>{v.linkage.length} linkage entries</span>
        <span>{v.hash.length} hash entries</span>
        {head ? <span>SLA {slaText(head.sla_now_s)}</span> : null}
      </div>
      {v.linkage.length > 0 ? (
        <table className="pkir-table compact" data-testid="pki-crl-entries">
          <thead>
            <tr><th>la1Id</th><th>la2Id</th><th>iRev</th><th>iMax</th><th>jMax</th><th>seeds at iRev</th>{gt ? <th>Revokes (ground truth)</th> : null}</tr>
          </thead>
          <tbody>
            {v.linkage.map((e, k) => (
              <tr key={k} data-testid="pki-crl-entry">
                <td className="mono">{e.la1_id}</td>
                <td className="mono">{e.la2_id}</td>
                <td className="mono">{e.i_rev}</td>
                <td className="mono">{e.i_max}</td>
                <td className="mono">{e.jmax}</td>
                <td className="mono faint">{e.ls1.slice(0, 8)}… {e.ls2.slice(0, 8)}…</td>
                {gt ? (
                  <td className="mono" data-testid="pki-crl-matches">
                    {(e.matches ?? []).length === 0
                      ? "none recorded"
                      : summarize((e.matches ?? []).map((m) => v.owners?.[m] ?? null))}
                  </td>
                ) : null}
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
      {v.hash.length > 0 ? (
        <ul className="mono">
          {v.hash.map((h) => (
            <li key={h.id10}>{h.id10} expires {time32Text(h.expiry)}{gt && h.matches?.length ? ` · ${summarize(h.matches.map((m) => v.owners?.[m] ?? null))}` : ""}</li>
          ))}
        </ul>
      ) : null}
      {!gt ? <p className="faint">Which vehicles these entries revoke is ground truth; choose the ground-truth view to see it.</p> : null}
      <details>
        <summary>CrlContents (JER)</summary>
        <pre className="pkir-pre">{JSON.stringify(v.contents, null, 1)}</pre>
      </details>
    </div>
  );
}

function summarize(owners: readonly ({ device: string; i: number | null; j: number | null; provider: string | null } | null)[]): string {
  const by = new Map<string, number[]>();
  for (const o of owners) {
    if (!o) continue;
    const list = by.get(o.device) ?? [];
    if (o.i !== null) list.push(o.i);
    by.set(o.device, list);
  }
  return [...by].map(([d, is]) => `${d}: ${is.length} cert${is.length === 1 ? "" : "s"}${is.length ? ` (weeks ${Math.min(...is)}–${Math.max(...is)})` : ""}`).join("; ");
}

function Providers({ providers }: { readonly providers: readonly ProviderView[] }): React.JSX.Element {
  return (
    <>
      <h4 className="pkir-pad">Providers</h4>
      <table className="pkir-table compact" data-testid="pki-crl-providers">
        <tbody>
          {providers.map((p) => (
            <tr key={p.label}>
              <td>{p.label}{p.home ? " (home)" : ""}</td>
              <td>{p.on_ctl ? "on the CTL" : "not on the CTL"}</td>
              <td>CRL publication {p.crl_publication}</td>
              <td>MA {p.ma_mode}{p.ma_review_s ? ` (review ${p.ma_review_s} s)` : ""}</td>
              <td className="faint">{p.region ? `${p.region.name}${p.region.assumption ? " (assumed)" : ""}` : ""}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}

function Ctl({ ctl }: { readonly ctl: CtlView }): React.JSX.Element {
  return (
    <>
      <h4 className="pkir-pad">Certificate trust list</h4>
      <p className="pkir-pad mono" data-testid="pki-ctl">
        sequence {ctl.sequence} · {ctl.roots.length} roots · signed by {ctl.signers.length} of {ctl.electors} electors (quorum {ctl.quorum}) · effective {time32Text(ctl.effective)} · {ctl.bytes} B{ctl.pending ? ` · ${ctl.pending} pending` : ""}
      </p>
      <table className="pkir-table compact">
        <thead><tr><th>Sequence</th><th>Effective</th><th>Roots</th><th>Signers</th></tr></thead>
        <tbody>
          {ctl.history.map((h) => (
            <tr key={h.sequence}>
              <td className="mono">{h.sequence}</td>
              <td className="mono">{time32Text(h.effective)}</td>
              <td className="mono">{h.roots}</td>
              <td className="mono">{h.signers.join(", ")}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </>
  );
}
