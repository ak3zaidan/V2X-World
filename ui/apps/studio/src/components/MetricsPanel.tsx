import { CloseIcon } from "../shell/Icons.js";
import { PlotsStrip } from "./PlotsStrip.js";

/**
 * The metrics panel. Today it hosts the measurements strip that used to sit under the viewport,
 * given the whole screen; the metrics-dashboard track replaces this body.
 */
export function MetricsPanel({ close }: { close: () => void }): React.JSX.Element {
  return (
    <div className="fullpanel-inner metrics-panel" data-testid="metrics-panel">
      <div className="fullpanel-head">
        <h2>Metrics</h2>
        <span className="dim">What the engine is measuring in this run, as it arrives. Click a plot&apos;s name or value to see where it comes from.</span>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={close} aria-label="Close the metrics" title="Close (Esc)" data-testid="metrics-close">
          <CloseIcon />
        </button>
      </div>
      <div className="fullpanel-body">
        <PlotsStrip />
      </div>
    </div>
  );
}
