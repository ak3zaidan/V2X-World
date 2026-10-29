/**
 * The layers menu: what is drawn over the scene (the `[overlays ▾]` menu of the 09-ui §6
 * wireframe).
 *
 * The list is the viewer's own catalogue (`OverlayManager.catalogue()`, which is the
 * `overlay.set {list:true}` shape of §6.7) merged with the engine's, so an overlay the engine knows
 * about but this build cannot draw is shown — in developer mode — as unavailable rather than
 * quietly missing.
 *
 * # Why it reads the way it does
 *
 * It used to be a mono-spaced `overlays ▾ 3` chip opening a flat list of wire names made pretty —
 * "Tx pulses", "Cbr heatmap", "Belief vs truth (GT)" — each with a "why" link into the inspector's
 * Why tab. A first-time researcher could not tell what most of them drew, and the Why tab is gone
 * (owner review, 2026-10-06). Now each layer has a plain name and one line saying what it shows,
 * grouped by what a researcher is looking for: the radio, security, and the map itself. Layers
 * drawn from ground truth — what no radio in the simulation can know — say so in words, and can be
 * locked off together for a blind evaluation (09-ui §6).
 */

import { useEffect, useRef, useState } from "react";
import { overlayLabel } from "@vwp/viewer";
import type { OverlayName } from "@vwp/protocol";

import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { OVERLAY_SOURCES } from "../lib/provenance.js";
import { LayersIcon } from "../shell/Icons.js";
import { MapModeLayerRow } from "./pki-records/RecordsTabs.js";

type Group = "radio" | "security" | "map";

/** A layer's name, the line under it, and the group it is listed in. */
const LAYER_COPY: Readonly<Partial<Record<OverlayName, { label: string; hint: string; group: Group }>>> = {
  tx_pulses: { label: "Transmissions", hint: "A ring each time a radio sends a message.", group: "radio" },
  links: { label: "Received messages", hint: "A line from sender to receiver for each message heard.", group: "radio" },
  cbr_heatmap: { label: "Channel load", hint: "How busy the radio channel is around each radio.", group: "radio" },
  coverage: { label: "Roadside coverage", hint: "Where a roadside unit is likely to be heard.", group: "radio" },
  rsu_range: { label: "Roadside range rings", hint: "The nominal range of each roadside unit.", group: "radio" },
  attackers_gt: { label: "Attackers", hint: "Which vehicles really are attackers.", group: "security" },
  reported: { label: "Reported vehicles", hint: "Vehicles named in a misbehaviour report (a suspicion).", group: "security" },
  revoked: { label: "Revoked vehicles", hint: "Vehicles whose certificates were revoked.", group: "security" },
  detections: { label: "Detections", hint: "Where a misbehaviour detector flagged a message.", group: "security" },
  belief_vs_truth_gt: { label: "Claimed vs true position", hint: "A line from where a vehicle says it is to where it is.", group: "security" },
  backend_flows: { label: "Backend traffic", hint: "Messages between the certificate authorities.", group: "security" },
  trajectories_gt: { label: "Paths driven", hint: "The route each vehicle has actually taken.", group: "map" },
  buildings: { label: "Buildings", hint: "Building volumes from the map.", group: "map" },
  lane_markings: { label: "Lane markings", hint: "Lane lines and road edges.", group: "map" },
  signal_state: { label: "Traffic signals", hint: "Each signal's light and time to its next change.", group: "map" },
  labels: { label: "Name tags", hint: "Text labels over vehicles and roadside units.", group: "map" },
  density: { label: "Traffic density", hint: "How crowded each patch of road is.", group: "map" },
  focus_region: { label: "Detailed-simulation area", hint: "The area the scenario simulates in full detail.", group: "map" },
};

const GROUPS: readonly { id: Group; title: string }[] = [
  { id: "radio", title: "Radio" },
  { id: "security", title: "Security" },
  { id: "map", title: "Map" },
];

/** A layer's plain name, falling back to the viewer's for one this table does not know. */
function layerLabel(name: string): string {
  return LAYER_COPY[name as OverlayName]?.label ?? overlayLabel(name as OverlayName).replace(/ \(GT\)$/, "");
}

/** The overlays whose records are streamed only while a radio is followed. */
const SELECTION_FED = ["tx_pulses", "links", "cbr_heatmap"];

export function OverlayMenu(): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const overlays = useStudio((s) => s.overlays);
  const serverOverlays = useStudio((s) => s.serverOverlays);
  const gtLocked = useStudio((s) => s.groundTruthLocked);
  const devDetails = useStudio((s) => s.devDetails);
  const lighting = useStudio((s) => s.lighting);
  const setLighting = useStudio((s) => s.setLighting);
  const following = useStudio((s) => s.selectedNode !== null || s.selectedActor !== null);
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (ev: MouseEvent): void => {
      if (ref.current && !ref.current.contains(ev.target as Node)) setOpen(false);
    };
    const onKey = (ev: KeyboardEvent): void => {
      if (ev.key === "Escape") {
        ev.preventDefault();
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  const catalogue = engine.overlayCatalogue();
  const drawable = catalogue.filter((c) => c.available);
  const notDrawable = catalogue.filter((c) => !c.available);
  const onCount = Object.values(overlays).filter(Boolean).length;

  const serverOnly = serverOverlays.filter((s) => !catalogue.some((c) => c.name === s.name));
  // Three overlays are fed only while a radio is followed (at full scale their records are millions
  // a simulated second). The note says so when it explains what the reader sees: one of them is on
  // and nothing is followed.
  const starved = !following && SELECTION_FED.some((n) => overlays[n as OverlayName] === true);

  const row = (entry: (typeof catalogue)[number]): React.JSX.Element => {
    const name = entry.name as OverlayName;
    const copy = LAYER_COPY[name];
    const source = OVERLAY_SOURCES[name]?.summary;
    return (
      <label key={entry.name} data-testid={`overlay-${entry.name}`} className="layer-row" title={source}>
        <input
          type="checkbox"
          checked={overlays[name] === true}
          disabled={gtLocked && entry.groundTruth}
          onChange={(e) => void engine.setOverlay(name, e.target.checked)}
        />
        <span className="grow">
          <span className="menu-item-title">
            {layerLabel(entry.name)}
            {entry.groundTruth ? (
              <span className="gt-tag" title="Drawn from ground truth: no radio in the simulation can know this">
                truth
              </span>
            ) : null}
          </span>
          {copy ? <span className="menu-item-hint">{copy.hint}</span> : null}
        </span>
      </label>
    );
  };

  const grouped = GROUPS.map((g) => ({ ...g, rows: drawable.filter((e) => (LAYER_COPY[e.name as OverlayName]?.group ?? "map") === g.id) }));

  return (
    <div className="menu" ref={ref}>
      <button
        type="button"
        className={open ? "vp-button on" : "vp-button"}
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        data-testid="overlays-button"
        title="Choose what is drawn over the scene"
      >
        <LayersIcon />
        <span>Layers</span>
        {onCount > 0 ? <span className="count">{onCount}</span> : null}
      </button>
      {open ? (
        <div className="menu-pop layers-pop" data-testid="overlays-menu">
          {grouped.map((g) =>
            g.rows.length === 0 ? null : (
              <div key={g.id} className="layer-group">
                <div className="sec">{g.title}</div>
                {g.rows.map(row)}
              </div>
            ),
          )}
          {starved ? (
            <div className="note layers-note" data-testid="overlays-starved">
              Transmissions, received messages and channel load are drawn only around a followed vehicle. Click one
              on the map.
            </div>
          ) : null}
          <div className="menu-rule" />
          <MapModeLayerRow />
          <label
            className="layer-row"
            title="How street-level views are lit. The map view is always drawn in daylight so the streets stay readable."
          >
            <span className="grow">
              <span className="menu-item-title">Lighting</span>
              <span className="menu-item-hint">Map view is always lit as day.</span>
            </span>
            <select
              value={lighting}
              onChange={(e) => setLighting(e.target.value as typeof lighting)}
              data-testid="lighting"
              aria-label="Lighting"
            >
              <option value="day">Day</option>
              <option value="night">Night</option>
              <option value="clock">Follow the clock</option>
            </select>
          </label>
          <label className="layer-row" title="Turn off and lock every layer drawn from ground truth, so what is on screen is only what the radios could know">
            <input type="checkbox" checked={gtLocked} onChange={(e) => engine.lockGroundTruth(e.target.checked)} />
            <span className="grow">
              <span className="menu-item-title">Blind evaluation</span>
              <span className="menu-item-hint">Hide every layer marked “truth”.</span>
            </span>
          </label>
          {/* What this build cannot draw is for whoever is developing it, not a menu of dead
              checkboxes for everyone else: developer mode only. */}
          {devDetails && notDrawable.length > 0 ? (
            <>
              <div className="sec">Not available in this build</div>
              {notDrawable.map((entry) => (
                <label key={entry.name} className="disabled" title={OVERLAY_SOURCES[entry.name as OverlayName]?.summary}>
                  <input type="checkbox" checked={false} disabled readOnly />
                  <span className="grow">{layerLabel(entry.name)}</span>
                  {entry.groundTruth ? <span className="gt-tag">truth</span> : null}
                </label>
              ))}
            </>
          ) : null}
          {devDetails && serverOnly.length > 0 ? (
            <>
              <div className="sec">The engine offers these, but this build cannot draw them</div>
              {serverOnly.map((entry) => (
                <label key={entry.name} className="disabled" title={entry.description}>
                  <input type="checkbox" checked={false} disabled readOnly />
                  <span>{entry.name}</span>
                </label>
              ))}
            </>
          ) : null}
        </div>
      ) : null}
    </div>
  );
}
