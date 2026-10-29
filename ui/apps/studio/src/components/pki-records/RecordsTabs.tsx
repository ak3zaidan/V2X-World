/**
 * The PKI view's record tabs (WP-E3, 00-gap-analysis §3.2) — Vehicles, Cases and Revocation
 * lists — and the map overlay's mode picker. The panel that holds them, its tab bar and its view
 * switch are `../pki/PkiPanel.tsx` (WP-E2).
 *
 * The view applies to every tab: in ground truth the tabs join the records onto the run's
 * devices; seen as an entity they show what it holds and nothing it cannot know. While ground
 * truth is locked off for a blind evaluation, the ground-truth view asks for nothing.
 */

import { MAP_MODE_LABEL, type PkiMapMode } from "../../lib/pkiRecords.js";
import { CasesTab } from "./CasesTab.js";
import { RevocationTab } from "./RevocationTab.js";
import { VehiclesTab } from "./VehiclesTab.js";
import { useEffectiveView, usePkiView, type RecordsTab } from "./state.js";
import "../../styles/pki-records.css";

/** One record tab, seen in the effective view. */
export function RecordsBody({ tab }: { readonly tab: RecordsTab }): React.JSX.Element {
  const view = useEffectiveView();
  return (
    <div className="fullpanel-body pkir-body">
      {tab === "vehicles" ? <VehiclesTab view={view} /> : tab === "cases" ? <CasesTab view={view} /> : <RevocationTab view={view} />}
    </div>
  );
}

/** What the map overlay colours vehicles by. */
export function MapModePicker(): React.JSX.Element {
  const mode = usePkiView((s) => s.mapMode);
  const set = usePkiView((s) => s.set);
  return (
    <label className="pkir-picker" title="Colour the vehicles on the map behind this panel: by provider, by credential state, or the chosen case's reporters and accused">
      <span className="dim">Colour map by</span>
      <select value={mode} onChange={(e) => set({ mapMode: e.target.value as PkiMapMode })} data-testid="pki-map-mode">
        {(Object.keys(MAP_MODE_LABEL) as PkiMapMode[]).map((m) => (
          <option key={m} value={m}>{MAP_MODE_LABEL[m]}</option>
        ))}
      </select>
    </label>
  );
}
