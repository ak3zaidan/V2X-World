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
 * Open one with `openPanel(id)` from `shell/route.ts`; the URL hash follows.
 */

import { useEffect, useRef } from "react";

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

/**
 * Focus in: when a panel opens, focus moves into it, unless something in it already took it (the
 * settings window focuses its search box). Focus back to the control that opened it is
 * `shell/route.ts`'s job, because only it knows what that was.
 */
function useFocusIn(host: React.RefObject<HTMLElement | null>, active: boolean): void {
  useEffect(() => {
    if (!active) return;
    const el = host.current;
    if (el && !el.contains(document.activeElement)) {
      const first = el.querySelector<HTMLElement>(
        'input:not([type="hidden"]), select, textarea, button:not([disabled]), [tabindex]:not([tabindex="-1"])',
      );
      (first ?? el).focus();
    }
  }, [host, active]);
}

/** A full-screen panel's frame. `hidden` keeps it mounted and out of sight. */
export function FullPanel({
  id,
  title,
  hidden,
  children,
}: {
  id: PanelId;
  title: string;
  hidden: boolean;
  children: React.ReactNode;
}): React.JSX.Element {
  const host = useRef<HTMLDivElement | null>(null);
  useFocusIn(host, !hidden);
  return (
    <div
      ref={host}
      className="fullpanel"
      role="dialog"
      aria-label={title}
      hidden={hidden}
      tabIndex={-1}
      data-testid={`panel-${id}`}
    >
      {children}
    </div>
  );
}

/** A sheet's frame: a title, a close button and the body. */
export function Sheet({ spec, close, retarget }: { spec: PanelSpec; close: () => void; retarget: () => void }): React.JSX.Element {
  const host = useRef<HTMLElement | null>(null);
  useFocusIn(host, true);
  return (
    <aside ref={host} className="sheet" role="dialog" aria-label={spec.title} tabIndex={-1} data-testid={`panel-${spec.id}`}>
      <div className="sheet-head">
        <h2>{spec.title}</h2>
        <span className="grow" />
        <button type="button" className="icon-button" onClick={close} aria-label={`Close ${spec.title}`} title="Close (Esc)" data-testid="sheet-close">
          <CloseIcon />
        </button>
      </div>
      <div className="sheet-body">{spec.render({ close, retarget })}</div>
    </aside>
  );
}
