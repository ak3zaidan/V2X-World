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
  asCases,
  asCrl,
  asCrls,
  crlRows,
  secondsText,
  slaText,
  time32Text,
  type CrlHead,
  type ProviderView,
} from "../../lib/pkiRecords.js";
import { formatBytes } from "../../lib/pki.js";
import { deviceName, grouped } from "../../lib/pkiCert.js";
import { crlScheduleShort, crlScheduleText, maModeText } from "../../lib/pkiTopology.js";
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
        <table className="pkir-table pki-crltable" data-testid="pki-crl-table">
          <thead>
            <tr>
              <th>Provider</th>
              <th title="crlSeries (IEEE 1609.2 §7.3): which family of certificates the list covers">
                <Term k="crl-series">Series</Term>
              </th>
              <th title="Linkage entries revoke all of a vehicle's pseudonym certificates; hash entries revoke single certificates">Kind</th>
              <th>Version</th>
              <th title="issueDate">Issued</th>
              <th title="When the next list is expected: the scenario's publishing schedule when it has one. The deadline every list prints (its nextCrl) is in the tooltip and in the list's card.">
                Next list
              </th>
              <th title="How many vehicles or certificates the newest list revokes; click to see which">Revokes</th>
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
                  <td>{kindText(r.kind)}</td>
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
                  <NextList head={r.newest} cadenceS={cadence[r.provider] ?? null} nowNs={crls?.t_ns ?? 0} />
                  <td title={`${formatBytes(r.newest.bytes)} signed`}>
                    {r.newest.entries > 0 ? (
                      <button
                        type="button"
                        className="linklike mono"
                        onClick={(e) => {
                          e.stopPropagation();
                          set({ crl: r.newest.id8 });
                        }}
                        data-testid="pki-crl-entries-link"
                        title="Open the newest list: who each entry revokes"
                      >
                        {r.newest.entries} {r.kind === "linkage" ? `vehicle${r.newest.entries === 1 ? "" : "s"}` : `certificate${r.newest.entries === 1 ? "" : "s"}`} ›
                      </button>
                    ) : (
                      <span className="faint">nobody</span>
                    )}
                  </td>
                </tr>
                {open === r.key ? (
                  <tr className="sub pki-crl-versions-row">
                    <td colSpan={7}>
                      <table className="pkir-table compact pki-crl-versions" data-testid="pki-crl-versions">
                        <thead>
                          <tr>
                            <th>Version</th>
                            <th title="issueDate, in the run's calendar">Issued</th>
                            <th title="nextCrl: the deadline by which a newer list must replace this one">
                              <Term k="nextcrl">Replace by</Term>
                            </th>
                            <th>Entries</th>
                            <th title="A full list carries every entry; a delta list only the changes since the last full one">
                              <Term k="delta">Type</Term>
                            </th>
                            <th title="HashedId8: how devices refer to this list">Id</th>
                          </tr>
                        </thead>
                        <tbody>
                          {r.versions.map((v) => (
                            <tr
                              key={`${r.key}/${v.id8}`}
                              className={chosen === v.id8 ? "sel" : undefined}
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
                              <td className="mono">v{v.version}</td>
                              <td className="mono" title={`at ${secondsText(v.issued_at)} of the run`}>
                                {dateText(v.issue_date)}
                              </td>
                              <td className="mono">{dateText(v.next_crl)}</td>
                              <td className="mono" title={`${formatBytes(v.bytes)} signed`}>
                                {v.entries}
                              </td>
                              <td>{v.delta ? "delta: changes only" : "full list"}</td>
                              <td className="mono faint" title={`HashedId8 ${grouped(v.id8)}`}>
                                {grouped(v.id8).slice(0, 9)}…
                              </td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </td>
                  </tr>
                ) : null}
              </Fragment>
            ))}
          </tbody>
        </table>
        {rows.length === 0 ? <p className="dim pkir-pad">No revocation list (CRL) has been published yet. The first appears when a provider's CRL Generator signs one.</p> : null}
        {crls ? <Providers providers={crls.providers} heads={crls.crls} nowRows={rows} cadence={cadence} /> : null}
      </div>
      {chosen ? <CrlCard id8={chosen} view={view} head={crls?.crls.find((c) => c.id8 === chosen) ?? null} /> : null}
    </div>
  );
}

/** A Time32 as the simulated date and time, to the minute (`2027-03-04 08:02`); the zone is said once, above. */
function dateText(t: number): string {
  return time32Text(t).slice(0, 16);
}

/** A list's kind in words. */
function kindText(kind: string): string {
  return kind === "linkage" ? "whole vehicles" : kind === "hash" ? "single certificates" : kind;
}

/**
 * When the next list comes. With a scenario schedule (`cadence_s`) that is what a reader needs: "in
 * 12 s (every 30 s)"; the deadline the list prints (nextCrl, often days away in a compressed run)
 * goes to the tooltip. Without one, the deadline is the only promise, and it is shown. An overdue
 * list is always said.
 */
function NextList({ head, cadenceS, nowNs }: { readonly head: CrlHead; readonly cadenceS: number | null; readonly nowNs: number }): React.JSX.Element {
  const deadline = `Deadline printed on the list (nextCrl): ${time32Text(head.next_crl)}, ${head.sla_now_s < 0 ? slaText(head.sla_now_s) : `in ${slaText(head.sla_now_s)}`}.`;
  if (head.sla_now_s < 0)
    return (
      <td className="mono bad" data-testid="pki-crl-sla" title={deadline}>
        {slaText(head.sla_now_s)}
      </td>
    );
  if (cadenceS !== null && cadenceS > 0) {
    const since = Math.max(0, (nowNs - head.issued_at) / 1e9);
    return (
      <td
        data-testid="pki-crl-sla"
        data-basis="schedule"
        title={`The scenario's CRL Generator gathers new revocations and publishes them every ${slaText(cadenceS)}; with nothing new it publishes no list (this one is ${slaText(since)} old). ${deadline}`}
      >
        <span className="mono">within {slaText(cadenceS)}</span> <span className="faint">of a revocation</span>
      </td>
    );
  }
  return (
    <td className="mono" data-testid="pki-crl-sla" data-basis="deadline" title={deadline}>
      by {dateText(head.next_crl)} <span className="faint">(in {slaText(head.sla_now_s)})</span>
    </td>
  );
}

function CrlCard({ id8, view, head }: { readonly id8: string; readonly view: string | null; readonly head: CrlHead | null }): React.JSX.Element {
  const set = usePkiView((s) => s.set);
  // A CRL is public; only the join onto the run's devices needs ground truth.
  const v = asCrl(usePki({ topic: "crl", hashedid8: id8, view: view ?? "public" }, 0).data);
  // Which case put each vehicle on the list: the cases' accused, ground truth only.
  const cases = asCases(usePki(view === GROUND_TRUTH ? { topic: "cases", view: GROUND_TRUTH } : null, 4000).data);
  const caseOf = new Map<string, string>();
  for (const c of cases?.cases ?? []) for (const a of c.truth?.accused ?? []) if (!caseOf.has(a.device)) caseOf.set(a.device, c.case);
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
        <span
          data-testid="pki-crl-says"
          title={
            entries === 0
              ? "An empty list: it revokes nothing, and says so with a signature, so a device knows it is up to date."
              : "A linkage entry lets a receiver recognise every pseudonym certificate of one vehicle from the revocation week on, without naming it. A hash entry names one certificate."
          }
        >
          {entries === 0 ? (
            <b>empty</b>
          ) : (
            <>
              {v.linkage.length > 0 ? (
                <>
                  <b>{v.linkage.length}</b> <Term k="linkage-entry">{v.linkage.length === 1 ? "vehicle" : "vehicles"}</Term>
                </>
              ) : null}
              {v.linkage.length > 0 && v.hash.length > 0 ? " · " : null}
              {v.hash.length > 0 ? (
                <>
                  <b>{v.hash.length}</b> <Term k="hash-entry">{v.hash.length === 1 ? "certificate" : "certificates"}</Term>
                </>
              ) : null}
            </>
          )}
        </span>
        <span className="dim">{formatBytes(v.len)}</span>
        {head ? (
          <span className={head.sla_now_s < 0 ? "bad" : "dim"} title="nextCrl: the deadline by which a newer list must replace this one">
            {head.sla_now_s < 0 ? `replacement ${slaText(head.sla_now_s)}` : `replace within ${slaText(head.sla_now_s)}`}
          </span>
        ) : null}
        <span className="faint mono" title="HashedId8: how devices refer to this list">
          {grouped(id8)}
        </span>
      </div>
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
                  <td data-testid="pki-crl-matches">
                    {(e.matches ?? []).length === 0 ? "none recorded" : <Owners owners={(e.matches ?? []).map((m) => v.owners?.[m] ?? null)} caseOf={caseOf} />}
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

/** Who an entry revokes, each vehicle by name with its case one click away (ground truth). */
function Owners({
  owners,
  caseOf,
}: {
  readonly owners: readonly ({ device: string; node?: number | null; i: number | null; j: number | null; provider: string | null } | null)[];
  readonly caseOf: ReadonlyMap<string, string>;
}): React.JSX.Element {
  const devices = [...new Set(owners.filter((o) => o !== null).map((o) => o.device))];
  return (
    <>
      {summarize(owners)}
      {devices.map((d) => {
        const c = caseOf.get(d);
        return c ? (
          <span key={d}>
            {" · "}
            <button
              type="button"
              className="linklike"
              onClick={() => {
                usePkiView.getState().set({ caseId: c });
                chooseTab("cases");
              }}
              data-testid="pki-crl-case"
              title="Open the misbehaviour case that put it on the list"
            >
              case {c.split("/").pop()} ›
            </button>
          </span>
        ) : null;
      })}
    </>
  );
}

function summarize(owners: readonly ({ device: string; node?: number | null; i: number | null; j: number | null; provider: string | null } | null)[]): string {
  const by = new Map<string, { node: number | null; weeks: number[] }>();
  for (const o of owners) {
    if (!o) continue;
    const e = by.get(o.device) ?? { node: o.node ?? null, weeks: [] };
    if (o.i !== null) e.weeks.push(o.i);
    by.set(o.device, e);
  }
  return [...by]
    .map(([d, { node, weeks: is }]) => `${deviceName(d, node)}: ${is.length} certificate${is.length === 1 ? "" : "s"}${is.length ? ` (weeks ${Math.min(...is)}–${Math.max(...is)})` : ""}`)
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
  const dueOf = (label: string): number | null => {
    const mine = nowRows.filter((r) => r.provider === label);
    return mine.length > 0 ? Math.min(...mine.map((r) => r.newest.sla_now_s)) : null;
  };
  const line = (p: ProviderView): React.JSX.Element => {
    const o = { cadenceS: cadence[p.label] ?? null, publication: p.crl_publication, dueS: dueOf(p.label) };
    return (
      <>
        <span data-testid="pki-crl-schedule" title={crlScheduleText(o)}>
          Publishes {crlScheduleShort(o)}
        </span>
        <span className="dim">
          {" "}
          · {maModeText(p.ma_mode)}
          {p.ma_review_s ? ` (review ${slaText(p.ma_review_s)})` : ""}
        </span>
        {!p.on_ctl ? <span className="bad"> · root NOT on the trust list</span> : null}
      </>
    );
  };
  // Providers that publish the same way are said once; the deadline is compared to the hour.
  const shape = (p: ProviderView): string =>
    JSON.stringify([p.on_ctl, p.ma_mode, p.ma_review_s ?? null, p.crl_publication, cadence[p.label] ?? null, Math.round((dueOf(p.label) ?? 0) / 3600)]);
  if (providers.length > 1 && new Set(providers.map(shape)).size === 1) {
    return (
      <p className="pkir-pad pki-provider-rules-one" data-testid="pki-crl-providers" data-same="1">
        <b>{providers.length === 2 ? "Both providers" : `All ${providers.length} providers`}</b> {line(providers[0])}
      </p>
    );
  }
  return (
    <ul className="pkir-pad pki-provider-rules" data-testid="pki-crl-providers">
      {providers.map((p) => (
        <li key={p.label} data-provider={p.label}>
          <b>{p.label}</b> {line(p)}
        </li>
      ))}
    </ul>
  );
}
