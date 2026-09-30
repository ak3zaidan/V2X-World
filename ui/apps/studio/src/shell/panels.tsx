/**
 * The shell's panels: what can open over the main view, and how.
 *
 * This is the plug-in point the other tracks build into (docs/design/14-studio-shell.md §5). A
 * panel is an id, a title, a kind and a body:
 *
 *  * **fullscreen** — covers everything under the header, which stays, so the run state, the clock
 *    and the primary action are never hidden. Settings and Metrics.
 *  * **sheet** — slides over the viewport's left edge and leaves the time controls and the
 *    inspector usable. Runs, Compare, Commands, Details.
 *
 * `keepMounted` keeps a panel's body alive after it is first closed, for a body that holds state a
 * reader would not want to lose between two looks (the metrics' chosen plots).
 *
 * Open one with `openPanel(id)` from `shell/route.ts`; the URL hash follows. The frames a panel is
 * drawn in (`FullPanel`, `Sheet`) are in `shell/PanelFrame.tsx`.
 */

import { ComparisonView } from "../components/ComparisonView.js";
import { CopilotPanel } from "../components/CopilotPanel.js";
import { PlotsStrip } from "../components/PlotsStrip.js";
import { RunBrowser } from "../components/RunBrowser.js";
import { RunDetails } from "../components/RunDetails.js";
import type { PanelId } from "../state/store.js";
import { CloseIcon } from "./Icons.js";

export interface PanelSpec {
  readonly id: PanelId;
  readonly title: string;
  readonly kind: "fullscreen" | "sheet";
  readonly keepMounted?: boolean;
  /** The body. `close` closes the panel; `retarget` re-resolves the engine and reconnects. */
  readonly render: (ctx: { readonly close: () => void; readonly retarget: () => void }) => React.ReactNode;
}

/**
 * The metrics panel. Today it hosts the measurements strip that used to sit under the viewport,
 * given the whole screen; the metrics-dashboard track replaces this body.
 */
function MetricsBody({ close }: { close: () => void }): React.JSX.Element {
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

export const PANELS: Readonly<Record<PanelId, PanelSpec>> = {
  // The settings window is rendered by the shell itself (it has its own header and footer and can
  // open in a window of its own), so its entry here only names it.
  settings: { id: "settings", title: "Settings", kind: "fullscreen", render: () => null },
  metrics: { id: "metrics", title: "Metrics", kind: "fullscreen", keepMounted: true, render: ({ close }) => <MetricsBody close={close} /> },
  runs: { id: "runs", title: "Runs and recordings", kind: "sheet", render: () => <RunBrowser /> },
  compare: { id: "compare", title: "Compare two runs", kind: "sheet", render: () => <ComparisonView /> },
  commands: { id: "commands", title: "Commands", kind: "sheet", render: () => <CopilotPanel /> },
  details: { id: "details", title: "Run details", kind: "sheet", render: ({ retarget }) => <RunDetails onRetarget={retarget} /> },
};
