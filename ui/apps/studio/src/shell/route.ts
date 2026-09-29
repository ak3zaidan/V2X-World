/**
 * The open panel, mirrored in the URL hash.
 *
 * `#settings`, `#metrics`, `#runs`… — so a reload keeps the panel you were in, a link can open one,
 * and the browser's Back button closes the one you opened. A panel opened from the page pushes a
 * history entry and closing it goes back over that entry; a panel the page was *loaded* with has no
 * entry of ours to go back over, so closing it rewrites the address instead of leaving the page.
 */

import { useEffect } from "react";

import { useStudio, type PanelId } from "../state/store.js";

export const PANEL_IDS: readonly PanelId[] = ["settings", "metrics", "runs", "compare", "commands", "details"];

export function panelFromHash(hash: string): PanelId | null {
  const id = hash.replace(/^#/, "");
  return (PANEL_IDS as readonly string[]).includes(id) ? (id as PanelId) : null;
}

/** Whether the history entry we are on was pushed by opening a panel from this page. */
let pushedByUs = false;

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
