/**
 * The header's live summary: the newest value of two measurements, inside the Metrics button.
 *
 * What replaced the bottom plots strip. The pinned metrics lead; with nothing pinned it is delivery
 * and channel busy ratio, the two numbers a V2X study leads with. Read from the stream's own buffer
 * (`engine.metrics`), so it costs no request, and absent when the stream carries neither.
 */

import { engine } from "../state/engine.js";
import { useStudio } from "../state/store.js";
import { formatWithUnit } from "./model.js";
import { DEFAULT_SUMMARY, usePins } from "./pins.js";

/** The units the summary knows without a catalogue; the dashboard fills in the rest. */
const units = new Map<string, string>([
  ["pdr", "ratio"],
  ["cbr", "ratio"],
]);

/** Called by the dashboard when it has the catalogue, so a pinned metric's unit is known here. */
export function noteUnits(rows: readonly { readonly name: string; readonly unit: string }[]): void {
  for (const r of rows) units.set(r.name, r.unit);
}

export function MetricsSummary(): React.JSX.Element | null {
  // Re-render on the store's 5 Hz series beat; the values themselves are read from the buffer.
  useStudio((s) => s.seriesTick);
  const pins = usePins((s) => s.pins);
  const names = [...pins, ...DEFAULT_SUMMARY].filter((n, i, all) => all.indexOf(n) === i && engine.metrics.has(n)).slice(0, 2);
  if (names.length === 0) return null;
  return (
    <span className="metrics-summary" data-testid="metrics-summary" aria-label="Newest values">
      {names.map((n) => (
        <span key={n} className="metrics-summary-item">
          <span className="dim">{n}</span> {formatWithUnit(engine.metrics.latest(n), units.get(n) ?? "")}
        </span>
      ))}
    </span>
  );
}
