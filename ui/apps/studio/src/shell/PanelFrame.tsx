/**
 * The frames the shell draws its panels in (`shell/panels.tsx` says which panel uses which): a
 * full-screen panel under the header, and a sheet over the viewport's left edge. Both move focus
 * into themselves when they open.
 */

import { useEffect, useRef } from "react";

import type { PanelId } from "../state/store.js";
import { CloseIcon } from "./Icons.js";
import type { PanelSpec } from "./panels.js";

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
