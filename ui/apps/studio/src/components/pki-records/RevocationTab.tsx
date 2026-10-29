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
import { formatBytes } from "../../lib/pki.js";
import { deviceName, grouped } from "../../lib/pkiCert.js";
import { crlScheduleText, maModeText } from "../../lib/pkiTopology.js";
import { CloseIcon } from "../../shell/Icons.js";
import { Term } from "../pki/Term.js";
import { chooseTab, usePki, usePkiView } from "./state.js";

export function RevocationTab({ view }: { readonly view: string | null }): React.JSX.Element {
  const crls = asCrls(usePki({ topic: "crls" }, 2000).data);
  const chosen = usePkiView((s) => s.crl);
  const cadence = usePkiView((s) => s.crlCadence);
  const set = usePkiView((s) => s.set);
  const [open, setOpen] = useState<string | null>(null);
  const rows = crls ? crlRows(crls) : [];
  const toggle = (key: string, newest: string): void => {
    setOpen(open === key ? null : key);
    if (open !== key) set({ crl: newest });
  };
  return (
    <div className={chosen ? "pkir-split pkir-crls" : "pkir-split pkir-crls single"} data-testid="pki-crls">
      <div className="pkir-table-wrap">
        <p className="pkir-pad pki-lead">
          A <b>certificate revocation list (CRL)</b> is how a revocation reaches the vehicles: each provider's CRL Generator signs a new list, and every
          list must be replaced before the deadline it prints. Times are the run's simulated calendar, in UTC. Click a row for its versions, and a version
          for what it revokes.
        </p>
        <table className="pkir-table pki-crltable" data-testid="pki-crl-table">
          <thead>
            <tr>
              <th>Provider</th>
              <th title="crlSeries (IEEE 1609.2 §7.3): which family of certificates the list covers">
                <Term k="crl-series">Series</Term>
              </th>
              <th title="Linkage entries revoke a vehicle's pseudonym certificates; hash entries revoke single certificates">Revokes</th>
              <th>Version</th>
              <th title="issueDate">Issued</th>
              <th title="nextCrl: the deadline by which a newer list must replace this one; the bar is how much of that window has gone">
                <Term k="nextcrl">Replace by</Term>
              </th>
              <th>Entries</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <Fragment key={r.key}>
                <tr
                  onClick={() => toggle(r.key, r.newest.id8)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      toggle(r.key, r.newest.id8);
                    }
                  }}
                  tabIndex={0}
                  aria-expanded={open === r.key}
                  className={open === r.key ? "sel" : undefined}
                  data-testid="pki-crl-row"
                  data-key={r.key}
                >
                  <td>
                    <span aria-hidden="true" className="faint">
                      {open === r.key ? "▾ " : "▸ "}
                    </span>
                    {r.provider}
                  </td>
                  <td className="mono">{r.series}</td>
                  <td>{r.kind === "linkage" ? "pseudonyms" : r.kind === "hash" ? "single certificates" : r.kind}</td>
                  <td className="mono">
                    v{r.newest.version}
                    {r.newest.delta ? " Δ" : ""}{" "}
                    <span className="faint">
                      · {r.versions.length} list{r.versions.length === 1 ? "" : "s"}
                    </span>
                  </td>
                  <td className="mono" title={time32Text(r.newest.issue_date)}>
                    {dateText(r.newest.issue_date)}
                  </td>
                  <td className={`mono ${r.newest.sla_now_s < 0 ? "bad" : ""}`} data-testid="pki-crl-sla" title={`${time32Text(r.newest.next_crl)} · ${slaText(r.newest.sla_now_s)}`}>
                    <SlaBar head={r.newest} />
                    {r.newest.sla_now_s < 0 ? slaText(r.newest.sla_now_s) : `in ${slaText(r.newest.sla_now_s)}`}
                  </td>
                  <td className="mono" title={`${formatBytes(r.newest.bytes)} signed`}>
                    {r.newest.entries}
                  </td>
                </tr>
                {open === r.key
                  ? r.versions.map((v) => (
                      <tr
                        key={`${r.key}/${v.id8}`}
                        className={`sub${chosen === v.id8 ? " sel" : ""}`}
                        onClick={() => set({ crl: v.id8 })}
                        onKeyDown={(e) => {
                          if (e.key === "Enter" || e.key === " ") {
                            e.preventDefault();
                            set({ crl: v.id8 });
                          }
                        }}
                        tabIndex={0}
                        data-testid="pki-crl-version"
                        data-id8={v.id8}
                      >
                        <td className="mono" colSpan={2} title={`HashedId8 ${v.id8}`}>
                          {grouped(v.id8)}
                        </td>
                        <td className="faint">issued at {secondsText(v.issued_at)}</td>
                        <td className="mono">
                          v{v.version}
                          {v.delta ? " Δ delta" : " full"}
                        </td>
                        <td className="mono">{dateText(v.issue_date)}</td>
                        <td className="mono">{dateText(v.next_crl)}</td>
                        <td className="mono" title={`${formatBytes(v.bytes)} signed`}>
                          {v.entries}
                        </td>
                      </tr>
                    ))
                  : null}
              </Fragment>
            ))}
          </tbody>
        </table>
        {rows.length === 0 ? <p className="dim pkir-pad">No revocation list (CRL) has been published yet. The first appears when a provider's CRL Generator signs one.</p> : null}
        {crls ? <Providers providers={crls.providers} heads={crls.crls} nowRows={rows} cadence={cadence} /> : null}
        {crls?.ctl ? <Ctl ctl={crls.ctl} /> : null}
      </div>
      {chosen ? <CrlCard id8={chosen} view={view} head={crls?.crls.find((c) => c.id8 === chosen) ?? null} /> : null}
    </div>
  );
}

/** A Time32 as the simulated date and time, to the minute (`2027-03-04 08:02`); the zone is said once, above. */
function dateText(t: number): string {
  return time32Text(t).slice(0, 16);
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
  const set = usePkiView((s) => s.set);
  // A CRL is public; only the join onto the run's devices needs ground truth.
  const v = asCrl(usePki({ topic: "crl", hashedid8: id8, view: view ?? "public" }, 0).data);
  const close = (
    <button type="button" className="icon-button" onClick={() => set({ crl: null })} aria-label="Close the list" data-testid="pki-crl-close">
      <CloseIcon />
    </button>
  );
  if (!v)
    return (
      <div className="pkir-card dim">
        Decoding the list {grouped(id8)}… {close}
      </div>
    );
  const gt = view === GROUND_TRUTH;
  const entries = v.linkage.length + v.hash.length;
  return (
    <div className="pkir-card" data-testid="pki-crl-card">
      <h3 className="pki-crlcard-head">
        Revocation list v{v.head.version}
        {v.head.delta ? " (delta)" : ""} <span className="dim">· {v.head.provider}, series {v.head.series}</span>
        <span className="grow" />
        {close}
      </h3>
      <div className="pkir-facts">
        <span>
          <b>{entries}</b> entr{entries === 1 ? "y" : "ies"}: {v.linkage.length} <Term k="linkage-entry">by linkage seed</Term> (whole vehicles), {v.hash.length}{" "}
          <Term k="hash-entry">by hash</Term> (single certificates)
        </span>
        <span>{formatBytes(v.len)} signed</span>
        {head ? (
          <span className={head.sla_now_s < 0 ? "bad" : undefined}>
            {head.sla_now_s < 0 ? `replacement ${slaText(head.sla_now_s)}` : `must be replaced within ${slaText(head.sla_now_s)}`}
          </span>
        ) : null}
        <span className="faint mono" title="HashedId8: how devices refer to this list">
          {grouped(id8)}
        </span>
      </div>
      <p className="pki-lead" data-testid="pki-crl-says">
        {entries === 0
          ? "An empty list: it revokes nothing, and says so with a signature, so a device knows it is up to date."
          : `${v.linkage.length > 0 ? `${v.linkage.length} linkage entr${v.linkage.length === 1 ? "y" : "ies"}: each lets a receiver recognise every pseudonym certificate of one vehicle from the revocation week on, without naming the vehicle. ` : ""}${v.hash.length > 0 ? `${v.hash.length} hash entr${v.hash.length === 1 ? "y" : "ies"}: each names one certificate by its hash. ` : ""}`}
      </p>
      {v.linkage.length > 0 ? (
        <table className="pkir-table compact" data-testid="pki-crl-entries">
          <thead>
            <tr>
              <th title="The two Linkage Authorities whose seeds the entry carries (la1Id, la2Id)">Linkage Authorities</th>
              <th title="iRev: the first week the entry revokes">
                <Term k="irev">From week</Term>
              </th>
              <th title="iMax: the last week the entry needs to cover">To week</th>
              <th title="jMax: certificates per week">
                <Term k="jmax">A week</Term>
              </th>
              <th title="The two Linkage Authorities' seeds at iRev: from them a receiver derives every linkage value of the vehicle">
                <Term k="seed">Seeds</Term>
              </th>
              {gt ? <th>Revokes (ground truth)</th> : null}
            </tr>
          </thead>
          <tbody>
            {v.linkage.map((e, k) => (
              <tr key={k} data-testid="pki-crl-entry">
                <td className="mono">
                  {e.la1_id} and {e.la2_id}
                </td>
                <td className="mono">{e.i_rev}</td>
                <td className="mono">{e.i_max}</td>
                <td className="mono">{e.jmax}</td>
                <td className="mono faint" title={`${e.ls1}\n${e.ls2}`}>
                  {e.ls1.slice(0, 8)}… {e.ls2.slice(0, 8)}…
                </td>
                {gt ? (
                  <td data-testid="pki-crl-matches">{(e.matches ?? []).length === 0 ? "none recorded" : summarize((e.matches ?? []).map((m) => v.owners?.[m] ?? null))}</td>
                ) : null}
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
      {v.hash.length > 0 ? (
        <ul className="mono">
          {v.hash.map((h) => (
            <li key={h.id10}>
              {h.id10} expires {dateText(h.expiry)}
              {gt && h.matches?.length ? ` · ${summarize(h.matches.map((m) => v.owners?.[m] ?? null))}` : ""}
            </li>
          ))}
        </ul>
      ) : null}
      {!gt && entries > 0 ? <p className="faint">Which vehicles these entries revoke is ground truth; choose “everything (ground truth)” in “Seen as” to see it.</p> : null}
      <details>
        <summary>The list as decoded (CrlContents, JER)</summary>
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
  return [...by]
    .map(([d, is]) => `${deviceName(d)}: ${is.length} certificate${is.length === 1 ? "" : "s"}${is.length ? ` (weeks ${Math.min(...is)}–${Math.max(...is)})` : ""}`)
    .join("; ");
}

function Providers({
  providers,
  nowRows,
  cadence,
}: {
  readonly providers: readonly ProviderView[];
  readonly heads: readonly CrlHead[];
  readonly nowRows: readonly { readonly provider: string; readonly newest: CrlHead }[];
  readonly cadence: Readonly<Record<string, number>>;
}): React.JSX.Element {
  return (
    <>
      <h4 className="pkir-pad">How each provider publishes</h4>
      <ul className="pkir-pad pki-provider-rules" data-testid="pki-crl-providers">
        {providers.map((p) => {
          const mine = nowRows.filter((r) => r.provider === p.label);
          const due = mine.length > 0 ? Math.min(...mine.map((r) => r.newest.sla_now_s)) : null;
          return (
            <li key={p.label} data-provider={p.label}>
              <b>{p.label}</b>
              {p.home ? " (home)" : ""}{" "}
              <span className={p.on_ctl ? "dim" : "bad"}>{p.on_ctl ? "· its root is on the trust list" : "· its root is NOT on the trust list"}</span>{" "}
              <span className="dim">
                · {maModeText(p.ma_mode)}
                {p.ma_review_s ? ` (review takes ${slaText(p.ma_review_s)})` : ""}
              </span>
              <br />
              <span data-testid="pki-crl-schedule">{crlScheduleText({ cadenceS: cadence[p.label] ?? null, publication: p.crl_publication, dueS: due })}</span>
            </li>
          );
        })}
      </ul>
    </>
  );
}

function Ctl({ ctl }: { readonly ctl: CtlView }): React.JSX.Element {
  return (
    <p className="pkir-pad faint" data-testid="pki-ctl">
      The certificate trust list (CTL), version {ctl.sequence}, names {ctl.roots.length} root{ctl.roots.length === 1 ? "" : "s"}; it is endorsed by {ctl.signers.length}{" "}
      of {ctl.electors} electors.{" "}
      <button type="button" className="linklike" onClick={() => chooseTab("trust")} data-testid="pki-ctl-overview">
        Its history and roots are on the Overview
      </button>
      .
    </p>
  );
}
