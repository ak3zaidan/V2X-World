/**
 * The PKI view (00-gap-analysis §3.2): the certificate-system panel with its tabs — Trust domain
 * (the backend diagram, `BackendView.tsx`), Vehicles, Cases and Revocation lists — the view
 * selector (ground truth, or what one entity knows) and the map overlay's mode.
 *
 * The view applies to every tab: in ground truth the tabs join the records onto the run's
 * devices; seen as an entity they show what it holds and nothing it cannot know. While ground
 * truth is locked off for a blind evaluation, the ground-truth view asks for nothing.
 */

import { BackendPanel } from "../BackendView.js";
import { CloseIcon } from "../../shell/Icons.js";
import { useStudio } from "../../state/store.js";
import { GROUND_TRUTH, MAP_MODE_LABEL, asCrls, entityLabel, viewChoices, type PkiMapMode } from "../../lib/pkiRecords.js";
import { CasesTab } from "./CasesTab.js";
import { RevocationTab } from "./RevocationTab.js";
import { VehiclesTab } from "./VehiclesTab.js";
import { useEffectiveView, usePki, usePkiView, type PkiTab } from "./state.js";
import "../../styles/pki-records.css";

const TABS: readonly { readonly id: PkiTab; readonly label: string }[] = [
  { id: "trust", label: "Trust domain" },
  { id: "vehicles", label: "Vehicles" },
  { id: "cases", label: "Cases" },
  { id: "crls", label: "Revocation lists" },
];

export function PkiPanel({ close }: { readonly close: () => void }): React.JSX.Element {
  const tab = usePkiView((s) => s.tab);
  const set = usePkiView((s) => s.set);
  return (
    <div className="pkir-panel" data-testid="pki-panel">
      <div className="pkir-tabs" role="tablist" data-testid="pki-tabs">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            className={tab === t.id ? "on" : undefined}
            onClick={() => set({ tab: t.id })}
            data-testid={`pki-tab-${t.id}`}
          >
            {t.label}
          </button>
        ))}
        <span className="grow" />
        {tab !== "trust" ? <ViewPicker /> : null}
        <MapModePicker />
        {tab !== "trust" ? (
          <button type="button" className="icon-button" onClick={close} aria-label="Close the PKI view" title="Close (Esc)" data-testid="pki-close">
            <CloseIcon />
          </button>
        ) : null}
      </div>
      {tab === "trust" ? <BackendPanel close={close} /> : <Body tab={tab} />}
    </div>
  );
}

function Body({ tab }: { readonly tab: Exclude<PkiTab, "trust"> }): React.JSX.Element {
  const view = useEffectiveView();
  return (
    <div className="fullpanel-body pkir-body">
      {tab === "vehicles" ? <VehiclesTab view={view} /> : tab === "cases" ? <CasesTab view={view} /> : <RevocationTab view={view} />}
    </div>
  );
}

/** Ground truth, or one entity that took part in a case. */
function ViewPicker(): React.JSX.Element {
  const view = usePkiView((s) => s.view);
  const set = usePkiView((s) => s.set);
  const locked = useStudio((s) => s.groundTruthLocked);
  // The entities are each provider's components, from the public trust-domain state.
  const crls = asCrls(usePki({ topic: "crls" }, 5000).data);
  const choices = viewChoices((crls?.providers ?? []).map((p) => p.label));
  if (!choices.includes(view)) choices.push(view);
  return (
    <label className="pkir-picker">
      <span className="faint">view</span>
      <select value={view} onChange={(e) => set({ view: e.target.value })} data-testid="pki-view">
        {choices.map((c) => (
          <option key={c} value={c}>
            {c === GROUND_TRUTH ? (locked ? "ground truth (locked)" : "ground truth") : `as ${entityLabel(c)}`}
          </option>
        ))}
      </select>
    </label>
  );
}

function MapModePicker(): React.JSX.Element {
  const mode = usePkiView((s) => s.mapMode);
  const set = usePkiView((s) => s.set);
  return (
    <label className="pkir-picker">
      <span className="faint">map</span>
      <select value={mode} onChange={(e) => set({ mapMode: e.target.value as PkiMapMode })} data-testid="pki-map-mode">
        {(Object.keys(MAP_MODE_LABEL) as PkiMapMode[]).map((m) => (
          <option key={m} value={m}>{MAP_MODE_LABEL[m]}</option>
        ))}
      </select>
    </label>
  );
}
