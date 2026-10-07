/**
 * Cross-panel links: open the settings window at one setting, or the metrics panel at one chart.
 *
 * The agent panel (and anything else) asks for a target; the panel that owns it opens and reveals
 * it. A pending target is kept here until a consumer takes it, because the consumer may not be
 * mounted yet when the link is followed (the settings window mounts when it opens). A
 * `CustomEvent` is also dispatched on `window` for a consumer that is already mounted:
 *
 *  - `vwp:reveal-setting` `{ pointer }` — the settings window scrolls to and focuses the field.
 *  - `vwp:metric-focus` `{ metric }` — the metrics panel expands that chart. The chart id is the
 *    metric's base name (`cbr`, `pdr`, `mac_access_delay`), which is what the analyst links by.
 */

import { useEffect } from "react";

import { openPanel } from "../shell/route.js";

let pendingSetting: string | null = null;
let pendingMetric: string | null = null;

/** Opens the settings window at the field a JSON Pointer names (`/radio/rat`). */
export function revealSetting(pointer: string): void {
  pendingSetting = pointer;
  openPanel("settings");
  window.dispatchEvent(new CustomEvent("vwp:reveal-setting", { detail: { pointer } }));
}

/** Opens the metrics panel at one chart. */
export function openMetricChart(metric: string): void {
  pendingMetric = metric;
  openPanel("metrics");
  window.dispatchEvent(new CustomEvent("vwp:metric-focus", { detail: { metric } }));
}

/** Takes the pending setting, once. */
export function takePendingSetting(): string | null {
  const p = pendingSetting;
  pendingSetting = null;
  return p;
}

/** Takes the pending chart, once. */
export function takePendingMetric(): string | null {
  const m = pendingMetric;
  pendingMetric = null;
  return m;
}

/**
 * For the settings window: call `reveal(pointer)` for a link followed before it mounted and for
 * every one followed while it is open. `ready` holds the call back until the fields are known.
 */
export function useRevealSetting(reveal: (pointer: string) => void, ready: boolean): void {
  useEffect(() => {
    if (!ready) return;
    const p = takePendingSetting();
    if (p !== null) requestAnimationFrame(() => reveal(p));
    const on = (e: Event): void => {
      const pointer = (e as CustomEvent<{ pointer: string }>).detail?.pointer;
      if (typeof pointer === "string") {
        takePendingSetting();
        requestAnimationFrame(() => reveal(pointer));
      }
    };
    window.addEventListener("vwp:reveal-setting", on);
    return () => window.removeEventListener("vwp:reveal-setting", on);
  }, [reveal, ready]);
}

/** For the metrics panel: the same, for a chart. */
export function useMetricFocus(focus: (metric: string) => void): void {
  useEffect(() => {
    const m = takePendingMetric();
    if (m !== null) focus(m);
    const on = (e: Event): void => {
      const metric = (e as CustomEvent<{ metric: string }>).detail?.metric;
      if (typeof metric === "string") {
        takePendingMetric();
        focus(metric);
      }
    };
    window.addEventListener("vwp:metric-focus", on);
    return () => window.removeEventListener("vwp:metric-focus", on);
  }, [focus]);
}
