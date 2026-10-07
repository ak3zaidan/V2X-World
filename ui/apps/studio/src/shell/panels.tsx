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

import { AgentPanel } from "../components/AgentPanel.js";
import { ComparisonView } from "../components/ComparisonView.js";
import { CopilotPanel } from "../components/CopilotPanel.js";
import { MetricsPanel } from "../components/MetricsPanel.js";
import { RunBrowser } from "../components/RunBrowser.js";
import { RunDetails } from "../components/RunDetails.js";
import type { PanelId } from "../state/store.js";

export interface PanelSpec {
  readonly id: PanelId;
  readonly title: string;
  readonly kind: "fullscreen" | "sheet";
  readonly keepMounted?: boolean;
  /** The body. `close` closes the panel; `retarget` re-resolves the engine and reconnects. */
  readonly render: (ctx: { readonly close: () => void; readonly retarget: () => void }) => React.ReactNode;
}

export const PANELS: Readonly<Record<PanelId, PanelSpec>> = {
  // The settings window is rendered by the shell itself (it has its own header and footer and can
  // open in a window of its own), so its entry here only names it.
  settings: { id: "settings", title: "Settings", kind: "fullscreen", render: () => null },
  metrics: { id: "metrics", title: "Metrics", kind: "fullscreen", keepMounted: true, render: ({ close }) => <MetricsPanel close={close} /> },
  // The agent keeps its conversation in its own store (state/agent.ts), so the sheet can unmount.
  agent: { id: "agent", title: "Agent", kind: "sheet", render: () => <AgentPanel /> },
  runs: { id: "runs", title: "Runs and recordings", kind: "sheet", render: () => <RunBrowser /> },
  compare: { id: "compare", title: "Compare two runs", kind: "sheet", render: () => <ComparisonView /> },
  commands: { id: "commands", title: "Commands", kind: "sheet", render: () => <CopilotPanel /> },
  details: { id: "details", title: "Run details", kind: "sheet", render: ({ retarget }) => <RunDetails onRetarget={retarget} /> },
};
