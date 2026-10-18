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
 * A run with no PKI view (the European CCMS, or `security.scms.pki_view: false`) answers
 * `inspect.trust` with "not supported here", and the panel falls back to the single-provider
 * Backend diagram (`BackendView.tsx`), which the CCMS keeps (00 §3.3).
 */

import { useCallback, useEffect, useState } from "react";
import { JsonRpcError, RpcErrorCode } from "@vwp/protocol";

import { GROUND_TRUTH, trustOf, viewOptions, type TrustAnswer } from "../../lib/pki.js";
import { engine } from "../../state/engine.js";
import { useStudio } from "../../state/store.js";
import { CloseIcon, NetworkIcon } from "../../shell/Icons.js";
import { togglePanel } from "../../shell/route.js";
import { BackendPanel } from "../BackendView.js";
import { FlowsTab } from "./FlowsTab.js";
import { TrustDomain } from "./TrustDomain.js";
import "../../styles/pki.css";

/** How often the open panel asks for a new trust-domain snapshot, wall-clock ms. */
const POLL_MS = 1000;

/** The tabs of 00 §3.2, in its order. */
export const PKI_TABS = [
  { id: "trust", label: "Trust domain", built: true },
  { id: "flows", label: "Flows", built: true },
  { id: "vehicles", label: "Vehicles", built: false },
  { id: "cases", label: "Cases", built: false },
  { id: "crls", label: "Revocation lists", built: false },
] as const;
export type PkiTab = (typeof PKI_TABS)[number]["id"];

const TAB_KEY = "vwp.pki.tab";

function rememberedTab(): PkiTab {
  try {
    const v = window.localStorage.getItem(TAB_KEY);
    if (v === "trust" || v === "flows") return v;
  } catch {
    // Storage blocked: the default tab.
  }
  return "trust";
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
  const [tab, setTabState] = useState<PkiTab>(rememberedTab);
  const [view, setView] = useState<string>(GROUND_TRUTH);
  const connection = useStudio((s) => s.connection);

  const setTab = useCallback((t: PkiTab) => {
    setTabState(t);
    try {
      window.localStorage.setItem(TAB_KEY, t);
    } catch {
      // Storage blocked: the tab is not remembered.
    }
  }, []);

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

  const options = viewOptions(trust?.state ?? null);
  const viewKnown = options.some((o) => o.id === view);
  const activeView = viewKnown ? view : GROUND_TRUTH;

  return (
    <div className="fullpanel-inner pki-panel" data-testid="pki-panel">
      <div className="fullpanel-head pki-head">
        <h2>PKI</h2>
        <div className="pki-tabs" role="tablist" aria-label="PKI view">
          {PKI_TABS.map((t) => (
            <button
              key={t.id}
              type="button"
              role="tab"
              aria-selected={tab === t.id}
              className={tab === t.id ? "pki-tab on" : "pki-tab"}
              disabled={!t.built}
              title={t.built ? undefined : "Not built yet (WP-E3)"}
              onClick={() => t.built && setTab(t.id)}
              data-testid={`pki-tab-${t.id}`}
            >
              {t.label}
            </button>
          ))}
        </div>
        <span className="grow" />
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
      {trust === null ? (
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
