/**
 * The PKI view's map overlays (00-gap-analysis §3.2): a coloured ring around each vehicle on the
 * map, by provider, by credential state (healthy, top-up due, starving, CRL overdue, revoked,
 * untrusted provider), or around a case's reporters and accused.
 *
 * Mounted once beside the viewport, so the rings stay while the full-screen panel is closed. It
 * asks `inspect.pki` every two seconds while a mode is on, hands the viewer the rings
 * (`OverlayManager.setActorTags`) and draws the legend. A case's parties are ground truth: with
 * ground truth locked off, or seen as an entity, the mode draws nothing and the legend says why.
 */

import { useEffect, useMemo } from "react";

import { VEHICLE_STATE } from "../../lib/pkiCert.js";
import {
  CREDENTIAL_COLORS,
  CREDENTIAL_EMPHASIS,
  CREDENTIAL_LABEL,
  CREDENTIAL_ORDER,
  GROUND_TRUTH,
  MAP_MODE_LABEL,
  PARTY_COLORS,
  asCases,
  asCrls,
  asVehicles,
  byActor,
  mapTags,
  providerColor,
  type CredentialState,
} from "../../lib/pkiRecords.js";
import { engine } from "../../state/engine.js";
import { useEffectiveView, usePki, usePkiView } from "./state.js";
import { hex } from "./VehiclesTab.js";

export function PkiMapOverlay(): React.JSX.Element | null {
  const mode = usePkiView((s) => s.mapMode);
  const caseId = usePkiView((s) => s.caseId);
  const tagged = usePkiView((s) => s.tagged);
  const set = usePkiView((s) => s.set);
  const view = useEffectiveView();
  const on = mode !== "off";
  // Every vehicle is listed only in ground truth: seen as one entity the server lists none, and
  // with ground truth locked off nothing is asked for.
  const vehicles = asVehicles(usePki(on && view ? { topic: "vehicles", view } : null).data);
  const crls = asCrls(usePki(on && mode === "credential" ? { topic: "crls" } : null, 4000).data);
  const cases = asCases(usePki(on && mode === "case" && view === GROUND_TRUTH ? { topic: "cases", view } : null).data);
  const tags = useMemo(
    () =>
      mapTags(mode, {
        vehicles: vehicles?.vehicles ?? [],
        crls,
        cases: cases?.cases ?? [],
        selectedCase: caseId,
        view: view ?? "locked",
        nowNs: vehicles?.t_ns ?? 0,
      }),
    [mode, vehicles, crls, cases, caseId, view],
  );
  useEffect(() => {
    const overlays = engine.viewer?.overlays;
    // The records name engine nodes; the viewer draws actors, and the two ids differ.
    const onMap = byActor(tags, (n) => engine.nodes.get(n)?.actorId);
    overlays?.setActorTags(on ? onMap : null);
    set({ tagged: on ? onMap.size : 0 });
  }, [on, tags, set]);
  useEffect(() => () => engine.viewer?.overlays.setActorTags(null), []);
  if (!on) return null;
  const labels = (vehicles?.vehicles ?? []).map((v) => v.provider);
  const onMap = tagged;
  return (
    <div className="pkir-map-legend" data-testid="pki-map-legend" data-mode={mode} data-tagged={tags.size} data-on-map={onMap}>
      <b>{MAP_MODE_LABEL[mode]}</b> <span className="faint">{tags.size} vehicles{onMap < tags.size ? ` (${onMap} on the map)` : ""}</span>
      {mode === "provider"
        ? [...new Set(labels)].sort().map((p) => <span key={p}><i style={{ borderColor: hex(providerColor(p, labels)) }} />{p}</span>)
        : null}
      {mode === "credential"
        ? CREDENTIAL_ORDER.map((s: CredentialState) => (
            <span key={s} title={VEHICLE_STATE[s].meaning} data-testid="pki-map-legend-state" data-state={s}>
              <i className={CREDENTIAL_EMPHASIS.has(s) ? "double" : undefined} style={{ borderColor: hex(CREDENTIAL_COLORS[s]) }} />
              {CREDENTIAL_LABEL[s]}
            </span>
          ))
        : null}
      {mode === "case" ? (
        view === GROUND_TRUTH ? (
          <>
            <span><i style={{ borderColor: hex(PARTY_COLORS.reporter) }} />reporter</span>
            <span><i className="double" style={{ borderColor: hex(PARTY_COLORS.accused) }} />accused</span>
            <span className="faint">{caseId ?? "every case"} · ground truth</span>
          </>
        ) : (
          <span className="faint">who reported and who was accused is ground truth{view === null ? ", which is locked off" : ""}</span>
        )
      ) : null}
      <button type="button" className="pkir-link" onClick={() => set({ mapMode: "off" })} data-testid="pki-map-off">hide</button>
    </div>
  );
}
