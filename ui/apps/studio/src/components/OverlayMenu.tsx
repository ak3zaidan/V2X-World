/**
 * The `[overlays ▾]` menu of the 09-ui §6 wireframe.
 *
 * The list is the viewer's own catalogue (`OverlayManager.catalogue()`, which is the
 * `overlay.set {list:true}` shape of §6.7) merged with the engine's, so an overlay the engine knows
 * about but this build cannot draw is shown as unavailable rather than quietly missing. Ground-truth
 * overlays are labelled `GT` and can be locked off for blind evaluation (09-ui §6).
 */

import { useEffect, useRef, useState } from "react";
import { overlayLabel } from "@vwp/viewer";
import type { OverlayName } from "@vwp/protocol";

import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { OVERLAY_SOURCES, overlaySubject } from "../lib/provenance.js";

/**
 * The provenance trigger beside one overlay.
 *
 * An overlay is a value on screen like any other: it says something about the run, and a reader has
 * to be able to ask what. §6.7's `needs_channels` is the part that matters in practice — an overlay
 * whose channel is unsubscribed draws nothing, and without this the pane just looks broken.
 */
function OverlayWhy({ name, enabled }: { name: string; enabled: boolean }): React.JSX.Element {
  const setWhy = useStudio((s) => s.setWhy);
  const channels = OVERLAY_SOURCES[name as keyof typeof OVERLAY_SOURCES]?.channels ?? [];
  return (
    <button
      type="button"
      className="linklike why-dot"
      data-testid={`overlay-why-${name}`}
      aria-label={`What draws the ${name} overlay${channels.length > 0 ? `, from ${channels.join(", ")}` : ""}`}
      title={
        channels.length > 0
          ? `Drawn from the ${channels.join(", ")} event stream`
          : "Drawn from the world file, so it needs no event subscription"
      }
      onClick={(e) => {
        // The label wraps a checkbox, so a click here would otherwise toggle the overlay as well.
        e.preventDefault();
        e.stopPropagation();
        setWhy(overlaySubject(name, enabled));
      }}
    >
      why
    </button>
  );
}

/** The overlays whose records are streamed only while a radio is followed. */
const SELECTION_FED = ["tx_pulses", "links", "cbr_heatmap"];

export function OverlayMenu(): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const overlays = useStudio((s) => s.overlays);
  const serverOverlays = useStudio((s) => s.serverOverlays);
  const gtLocked = useStudio((s) => s.groundTruthLocked);
  const devDetails = useStudio((s) => s.devDetails);
  const following = useStudio((s) => s.selectedNode !== null || s.selectedActor !== null);
  const ref = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (ev: MouseEvent): void => {
      if (ref.current && !ref.current.contains(ev.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
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

  return (
    <div className="menu" ref={ref}>
      <button type="button" className="chip" onClick={() => setOpen((v) => !v)} data-testid="overlays-button">
        overlays ▾ <span className="dim">{onCount}</span>
      </button>
      {open ? (
        <div className="menu-pop" data-testid="overlays-menu">
          <label>
            <input type="checkbox" checked={gtLocked} onChange={(e) => engine.lockGroundTruth(e.target.checked)} />
            <span>Lock ground truth off (blind evaluation)</span>
          </label>
          <div className="sec">Draw on the map</div>
          {drawable.map((entry) => (
            <label key={entry.name} data-testid={`overlay-${entry.name}`}>
              <input
                type="checkbox"
                checked={overlays[entry.name as OverlayName] === true}
                disabled={gtLocked && entry.groundTruth}
                onChange={(e) => void engine.setOverlay(entry.name as OverlayName, e.target.checked)}
              />
              <span className="grow">{overlayLabel(entry.name as OverlayName)}</span>
              {entry.groundTruth ? <span className="gt-tag">GT</span> : null}
              <OverlayWhy name={entry.name} enabled={overlays[entry.name as OverlayName] === true} />
            </label>
          ))}
          {/* What this build cannot draw is for whoever is developing it, not a menu of dead
              checkboxes for everyone else: developer mode only. */}
          {devDetails && notDrawable.length > 0 ? (
            <>
              <div className="sec">Not available in this build</div>
              {notDrawable.map((entry) => (
                <label key={entry.name} className="disabled">
                  <input type="checkbox" checked={false} disabled readOnly />
                  <span className="grow">{overlayLabel(entry.name as OverlayName)}</span>
                  {entry.groundTruth ? <span className="gt-tag">GT</span> : null}
                  <OverlayWhy name={entry.name} enabled={false} />
                </label>
              ))}
            </>
          ) : null}
          {starved ? (
            <div className="note" style={{ margin: "4px 6px 6px" }} data-testid="overlays-starved">
              Transmissions, links and channel load are streamed only while a vehicle is followed. Click one
              and they fill.
            </div>
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
