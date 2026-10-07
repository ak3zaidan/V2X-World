/**
 * The camera picker: which way the scene is looked at, with a one-line hint for each view.
 *
 * It used to be a native `<select>` reading "Map / Chase / Dashboard / Free / Roadside unit", which
 * told a first-time researcher the names and nothing else: "Dashboard" read as the metrics page, and
 * a view that needs a vehicle failed silently into the log (owner review, 2026-10-06). A native
 * option cannot carry a second line, so this is a small menu: each view has a name a person
 * recognises, one sentence of what it shows, and — when it cannot be used yet — why not, in place
 * of the hint. A view the viewer refused is said beside the button for a few seconds rather than
 * written to a log nobody reads.
 */

import { useEffect, useState } from "react";
import { CAMERA_MODES, type CameraMode } from "@vwp/viewer";

import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { MenuButton } from "../shell/Menu.js";
import { CameraIcon } from "../shell/Icons.js";

/** A view's name and the sentence that says what it shows. Exported for the free-camera help. */
export const CAMERA_VIEWS: Readonly<Record<string, { readonly label: string; readonly hint: string }>> = {
  map: { label: "Map", hint: "The whole area from above. Click a vehicle to follow it." },
  chase: { label: "Chase", hint: "Ride just behind the selected vehicle." },
  dashboard: { label: "Driver's view", hint: "The road as the selected vehicle's driver sees it." },
  free: { label: "Free", hint: "Move the camera anywhere, not tied to a vehicle." },
  rsu: { label: "Roadside unit", hint: "Look out from a roadside radio mast." },
};

/** The name of a view, for any other part of the page that mentions one. */
export function cameraLabel(mode: CameraMode): string {
  return CAMERA_VIEWS[mode]?.label ?? mode;
}

/** Why a view cannot be chosen right now, or `null` when it can. */
function unavailable(mode: CameraMode, vehicles: number, sites: number | null): string | null {
  if ((mode === "chase" || mode === "dashboard") && vehicles === 0) {
    return "Needs a vehicle on the road. Press Run first.";
  }
  if (mode === "rsu" && sites === 0) return "This map has no roadside units.";
  return null;
}

export function CameraPicker(): React.JSX.Element {
  const mode = useStudio((s) => s.cameraMode);
  const sites = useStudio((s) => s.world?.sites ?? null);
  // The store flushes at a few hertz; the live vehicle count is read from the stream on render.
  useStudio((s) => s.simTimeNs);
  const vehicles = engine.client?.poses.count ?? 0;
  const [refused, setRefused] = useState<string | null>(null);

  useEffect(() => {
    if (refused === null) return;
    const id = window.setTimeout(() => setRefused(null), 5000);
    return () => window.clearTimeout(id);
  }, [refused]);

  const choose = (next: CameraMode): void => {
    engine.setCameraMode(next);
    const applied = useStudio.getState().cameraMode;
    setRefused(
      applied === next
        ? null
        : next === "rsu"
          ? "No roadside unit to look from, so the view stayed as it was."
          : `${cameraLabel(next)} needs a vehicle on the road, so the view stayed as it was.`,
    );
  };

  return (
    <>
      <MenuButton
        className="vp-button"
        title={`Camera: ${cameraLabel(mode)}. Choose how to look at the scene ( [ and ] switch views )`}
        testId="camera-mode"
        menuTestId="camera-menu"
        align="left"
        label={
          <>
            <CameraIcon />
            <span data-testid="camera-mode-label" data-mode={mode}>
              {cameraLabel(mode)}
            </span>
            <span className="caret" aria-hidden="true">▾</span>
          </>
        }
      >
        {(close) => (
          <>
            <div className="sec">Camera</div>
            {CAMERA_MODES.map((m) => {
              const why = unavailable(m, vehicles, sites);
              return (
                <button
                  key={m}
                  type="button"
                  role="menuitemradio"
                  aria-checked={m === mode}
                  className="menu-item two-line"
                  disabled={why !== null}
                  data-testid={`camera-mode-${m}`}
                  onClick={() => {
                    close();
                    choose(m);
                  }}
                >
                  <span className="check-mark" aria-hidden="true">{m === mode ? "✓" : ""}</span>
                  <span className="grow">
                    <span className="menu-item-title">{cameraLabel(m)}</span>
                    <span className="menu-item-hint">{why ?? CAMERA_VIEWS[m]?.hint ?? ""}</span>
                  </span>
                </button>
              );
            })}
            <div className="menu-rule" />
            <div className="menu-note faint">
              <kbd>[</kbd> <kbd>]</kbd> switch views · <kbd>F</kbd> frame the traffic
            </div>
          </>
        )}
      </MenuButton>
      {refused !== null ? (
        <span className="vp-note" role="status" data-testid="camera-refused">
          {refused}
        </span>
      ) : null}
    </>
  );
}
