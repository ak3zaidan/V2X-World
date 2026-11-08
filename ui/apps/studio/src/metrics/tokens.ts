/**
 * The small shared state the dashboard's components read that is not a component: a series
 * colour from the theme (`metrics.css` defines `--viz-1` … `--viz-8` per theme), and the units the
 * header's live summary knows.
 */

/** The categorical colour of slot 1–8, as the current theme defines it. */
export function slotColour(slot: number): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(`--viz-${slot}`).trim();
  return v === "" ? "#3987e5" : v;
}

/** The units the header summary knows without a catalogue; the dashboard fills in the rest. */
export const summaryUnits = new Map<string, string>([
  ["pdr", "ratio"],
  ["cbr", "ratio"],
]);

/** Called by the dashboard when it has the catalogue, so a pinned metric's unit is known. */
export function noteUnits(rows: readonly { readonly name: string; readonly unit: string }[]): void {
  for (const r of rows) summaryUnits.set(r.name, r.unit);
}
