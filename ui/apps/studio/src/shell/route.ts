/**
 * The open panel, mirrored in the URL hash.
 *
 * `#settings`, `#metrics`, `#runs`… — so a reload keeps the panel you were in, a link can open one,
 * and the browser's Back button closes the one you opened. A panel opened from the page pushes a
 * history entry and closing it goes back over that entry; a panel the page was *loaded* with has no
 * entry of ours to go back over, so closing it rewrites the address instead of leaving the page.
 */

import { useEffect } from "react";

import { PANEL_IDS, useStudio, type PanelId } from "../state/store.js";

export { PANEL_IDS };

/**
 * The panel a hash opens. A panel may carry a sub-address after a `/` or a `?` — the metrics
 * dashboard's `#metrics/pdr/dist_bin?from=30&to=90` (`metrics/model.ts` `MetricsRoute`) — which is
 * the panel's own to read.
 */
export function panelFromHash(hash: string): PanelId | null {
  const id = hash.replace(/^#/, "").split(/[/?]/, 1)[0];
  return (PANEL_IDS as readonly string[]).includes(id) ? (id as PanelId) : null;
}

/** Whether the history entry we are on was pushed by opening a panel from this page. */
let pushedByUs = false;

/**
 * Whether a close's `history.back()` has not landed yet. The traversal is asynchronous: a panel
 * opened in the meantime (Escape on Metrics and a click on Backend, or Run closing the settings
 * window and the next edit opening it again) pushed its entry, and then the back landed on the
 * address with no panel and its `popstate` closed the panel just opened. The settings window
 * vanished under the test's next keystroke and the Backend button did nothing (e2e, 2026-10-06).
 * While a back is pending, an open only sets the store; the back's own `popstate` then pushes the
 * entry for whatever panel is open by then.
 */
let pendingBack = false;

/** The control that had focus when the first panel opened; it gets focus back when it closes. */
let opener: HTMLElement | null = null;

function restoreFocus(): void {
  const el = opener;
  opener = null;
  // After React has unmounted the panel, or the focus would land in a node about to go.
  if (el !== null) requestAnimationFrame(() => {
    if (el.isConnected) el.focus();
  });
}

/** Open a panel, or `null` to close whatever is open. */
export function openPanel(id: PanelId | null): void {
  const current = useStudio.getState().panel;
  if (current === id) return;
  if (id === null) {
    closePanel();
    return;
  }
  if (current === null) {
    const active = document.activeElement;
    // A menu item that opened the panel is about to unmount; its menu button is the opener.
    const menu = active instanceof HTMLElement ? active.closest(".menu")?.querySelector<HTMLElement>("[aria-haspopup]") : null;
    opener = menu ?? (active instanceof HTMLElement && active !== document.body ? active : null);
  }
  if (current !== null) {
    // Replacing one panel with another is one step, not two: Back closes it.
    history.replaceState(history.state, "", `#${id}`);
  } else if (pendingBack) {
    // The last close's back has not landed; its popstate pushes this panel's entry.
  } else {
    history.pushState(history.state, "", `#${id}`);
    pushedByUs = true;
  }
  useStudio.getState().setPanel(id);
}

export function closePanel(): void {
  if (useStudio.getState().panel === null) return;
  useStudio.getState().setPanel(null);
  restoreFocus();
  if (pushedByUs) {
    pushedByUs = false;
    pendingBack = true;
    history.back();
  } else {
    history.replaceState(history.state, "", `${location.pathname}${location.search}`);
  }
}

export function togglePanel(id: PanelId): void {
  openPanel(useStudio.getState().panel === id ? null : id);
}

/** Keep the store and the address in step: read the hash on load and on every Back or Forward. */
export function usePanelRoute(): void {
  useEffect(() => {
    const read = (): void => {
      if (pendingBack) {
        // Our own close landed. A panel opened while it was in flight gets its entry now.
        pendingBack = false;
        const open = useStudio.getState().panel;
        if (open !== null) {
          history.pushState(history.state, "", `#${open}`);
          pushedByUs = true;
          return;
        }
      }
      const id = panelFromHash(location.hash);
      if (id === null) pushedByUs = false;
      useStudio.getState().setPanel(id);
    };
    read();
    window.addEventListener("popstate", read);
    window.addEventListener("hashchange", read);
    return () => {
      window.removeEventListener("popstate", read);
      window.removeEventListener("hashchange", read);
    };
  }, []);
}
