/**
 * The PKI view (WP-E2, `docs/design/scms-real/00-gap-analysis-and-plan.md` §3.2): the certificate
 * system as the engine runs it, under the header like Metrics, with its tabs and the per-entity
 * view switch.
 *
 * It replaces the old Backend view for the US SCMS. While it is open it asks the engine
 * `inspect.trust` once a second (the newest `pki.state`, which is not on the event stream) and
 * hands the answer to the open tab. The view switch — ground truth, or "as RA", "as ACA", "as LA1"
 * ... — is what the tabs show *as*: an entity sees only the messages it sent or received and only
 * the plaintext it opened itself, and its own knowledge record instead of the ground-truth join.
 *
 * Vehicles, Cases and Revocation lists (WP-E3) are `../pki-records/`'s; they ask `inspect.pki`
 * themselves. The tab, the view and the map overlay's mode live in `usePkiView`, so the inspector
 * can open the panel on a vehicle and the map keeps its rings after the panel closes. While ground
 * truth is locked off for a blind evaluation, the record tabs ask for nothing in ground truth.
 *
 * A run with no PKI view (the European CCMS, or `security.scms.pki_view: false`) answers
 * `inspect.trust` with "not supported here", and the panel falls back to the single-provider
 * Backend diagram (`BackendView.tsx`), which the CCMS keeps (00 §3.3).
 */

import { useCallback, useEffect, useState } from "react";
import { JsonRpcError, RpcErrorCode } from "@vwp/protocol";

import { GROUND_TRUTH, trustOf, viewOptions, type TrustAnswer } from "../../lib/pki.js";
import { asCrls, entityLabel, viewChoices } from "../../lib/pkiRecords.js";
import { engine } from "../../state/engine.js";
import { useStudio } from "../../state/store.js";
import { CloseIcon, NetworkIcon } from "../../shell/Icons.js";
import { togglePanel } from "../../shell/route.js";
import { BackendPanel } from "../BackendView.js";
import { MapModePicker, RecordsBody } from "../pki-records/RecordsTabs.js";
import { RECORDS_TABS, chooseTab, usePki, usePkiView, type PkiTab, type RecordsTab } from "../pki-records/state.js";
import { FlowsTab } from "./FlowsTab.js";
import { HealthStrip } from "./HealthStrip.js";
import { TrustDomain } from "./TrustDomain.js";
import "../../styles/pki.css";

/** How often the open panel asks for a new trust-domain snapshot, wall-clock ms. */
const POLL_MS = 1000;

/** The workspace's sections (00 §3.2's tabs, in plain names; the standard term is the tooltip). */
export const PKI_TABS: readonly { readonly id: PkiTab; readonly label: string; readonly built: boolean; readonly term: string }[] = [
  { id: "trust", label: "Overview", built: true, term: "The trust domain: providers, authorities, the trust list and live traffic" },
  { id: "vehicles", label: "Vehicles", built: true, term: "End entities: enrolment, pseudonym certificate pools and their lifecycle" },
  { id: "cases", label: "Misbehaviour", built: true, term: "Misbehaviour cases: from report to revocation and enforcement" },
  { id: "crls", label: "Revocation", built: true, term: "Certificate revocation lists (CRLs) and the certificate trust list (CTL)" },
  { id: "flows", label: "Messages", built: true, term: "Flows: every request as a sequence diagram, every SPDU decoded" },
];
export type { PkiTab };

const isRecords = (t: PkiTab): t is RecordsTab => (RECORDS_TABS as readonly string[]).includes(t);

/** Arrow keys move along the tab bar (the WAI-ARIA tabs pattern), Home and End to its ends. */
function tabKeys(e: React.KeyboardEvent<HTMLDivElement>): void {
  const ids = PKI_TABS.map((t) => t.id);
  const at = ids.indexOf(usePkiView.getState().tab);
  const to = e.key === "ArrowRight" ? at + 1 : e.key === "ArrowLeft" ? at - 1 : e.key === "Home" ? 0 : e.key === "End" ? ids.length - 1 : null;
  if (to === null) return;
  e.preventDefault();
  const next = ids[(to + ids.length) % ids.length];
  chooseTab(next);
  e.currentTarget.querySelector<HTMLButtonElement>(`[data-testid="pki-tab-${next}"]`)?.focus();
}

/** The header's door to the panel. */
export function PkiButton(): React.JSX.Element {
  const open = useStudio((s) => s.panel === "backend");
  return (
    <button
      type="button"
      className={open ? "icon-button labelled on" : "icon-button labelled"}
      aria-pressed={open}
      aria-label="Certificate system"
      onClick={() => togglePanel("backend")}
      data-testid="backend-button"
      title="The certificate system (SCMS): its providers and their authorities, the trust list that binds them, and every request and revocation as it flows between them and the vehicles"
    >
      <NetworkIcon />
      <span>Certificates</span>
    </button>
  );
}

/** The panel. */
export function PkiPanel({ close }: { readonly close: () => void }): React.JSX.Element {
  const [trust, setTrust] = useState<TrustAnswer | null>(null);
  const [unsupported, setUnsupported] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const tab = usePkiView((s) => s.tab);
  const view = usePkiView((s) => s.view);
  const set = usePkiView((s) => s.set);
  const setView = useCallback((v: string) => set({ view: v }), [set]);
  const setTab = chooseTab;
  const connection = useStudio((s) => s.connection);
  const locked = useStudio((s) => s.groundTruthLocked);
  const records = isRecords(tab);
  // The record tabs also offer each provider's CRL generator and any vehicle they were opened as.
  const crls = asCrls(usePki(records ? { topic: "crls" } : null, 5000).data);

  const poll = useCallback(async (): Promise<void> => {
    try {
      const answer = trustOf(await engine.request("inspect.trust", {}, { quiet: true, timeoutMs: 4000 }));
      setTrust(answer);
      setUnsupported(false);
      setError(answer === null ? "The engine answered inspect.trust, but not with a trust domain." : null);
    } catch (err) {
      if (err instanceof JsonRpcError && err.code === RpcErrorCode.NOT_SUPPORTED_HERE) setUnsupported(true);
      else if (err instanceof JsonRpcError && err.code === RpcErrorCode.UNKNOWN_ID) setError("No trust-domain snapshot yet: the engine publishes one a simulated second.");
      else setError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    if (unsupported) return undefined;
    void poll();
    const id = window.setInterval(() => void poll(), POLL_MS);
    return () => window.clearInterval(id);
  }, [poll, connection, unsupported]);

  // A run with no PKI view: the CCMS keeps the single-provider Backend diagram.
  if (unsupported) return <BackendPanel close={close} />;

  const options: { id: string; label: string }[] = [...viewOptions(trust?.state ?? null)];
  if (records) {
    for (const c of viewChoices((crls?.providers ?? []).map((p) => p.label))) {
      if (!options.some((o) => o.id === c)) options.push({ id: c, label: `as ${entityLabel(c)}` });
    }
    if (!options.some((o) => o.id === view)) options.push({ id: view, label: `as ${entityLabel(view)}` });
  }
  if (locked) options[0] = { id: GROUND_TRUTH, label: "ground truth (locked)" };
  const viewKnown = options.some((o) => o.id === view);
  const activeView = viewKnown ? view : GROUND_TRUTH;

  return (
    <div className="fullpanel-inner pki-panel" data-testid="pki-panel">
      <div className="fullpanel-head pki-head">
        <h2 title="Security Credential Management System (SCMS): the US V2X public-key infrastructure">Certificate system</h2>
        <div className="pki-tabs" role="tablist" aria-label="Certificate system sections" onKeyDown={tabKeys}>
          {PKI_TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              className={tab === t.id ? "pki-tab on" : "pki-tab"}
              disabled={!t.built}
              title={t.term}
              tabIndex={tab === t.id ? 0 : -1}
              onClick={() => t.built && setTab(t.id)}
              data-testid={`pki-tab-${t.id}`}
            >
              {t.label}
            </button>
          ))}
        </div>
        <span className="grow" />
        <MapModePicker />
        <label className="pki-view" title="Show the system as ground truth, or as one entity knows it: only the messages it sent or received, only the plaintext it opened">
          <span className="dim">view</span>
          <select value={activeView} onChange={(e) => setView(e.target.value)} data-testid="pki-view">
            {options.map((o) => (
              <option key={o.id} value={o.id}>
                {o.label}
              </option>
            ))}
          </select>
        </label>
        {trust ? (
          <span className="dim mono" data-testid="pki-time" title="The snapshot's instant: simulated time and Time32">
            {trust.state.edition} · {trust.state.preset} · t {(trust.t_ns / 1e9).toFixed(0)} s
          </span>
        ) : null}
        <button type="button" className="icon-button" onClick={close} aria-label="Close the PKI view" title="Close (Esc)" data-testid="pki-close">
          <CloseIcon />
        </button>
      </div>
      {trust ? <HealthStrip trust={trust} view={locked && activeView === GROUND_TRUTH ? null : activeView} /> : null}
      {records ? (
        <RecordsBody tab={tab} />
      ) : trust === null ? (
        <div className="fullpanel-body pki-empty" data-testid="pki-empty">
          <p>No trust domain to show yet.</p>
          <p className="dim">
            A run publishes its PKI view once a simulated second when the scenario sets <code>security.protocol</code> to <code>protocol/scms/camp</code> (the US SCMS).
          </p>
          {error ? <p className="faint">The engine said: {error}</p> : null}
        </div>
      ) : tab === "flows" ? (
        <FlowsTab view={activeView} trust={trust} />
      ) : (
        <TrustDomain trust={trust} view={activeView} />
      )}
    </div>
  );
}
