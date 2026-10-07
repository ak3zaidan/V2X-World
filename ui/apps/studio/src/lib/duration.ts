/**
 * The run-duration presets and what choosing one edits (`components/RunDuration.tsx`), apart from
 * React and the store so the Node suite can check them (`test/long-runs.test.ts`).
 */

import { getPointer } from "./schema.js";

/** The spans offered, seconds. */
export const DURATION_PRESETS: readonly { readonly s: number; readonly label: string }[] = [
  { s: 60, label: "1 minute" },
  { s: 300, label: "5 minutes" },
  { s: 900, label: "15 minutes" },
  { s: 3600, label: "1 hour" },
  { s: 6 * 3600, label: "6 hours" },
  { s: 86400, label: "1 day" },
  { s: 2 * 86400, label: "2 days" },
  { s: 7 * 86400, label: "1 week" },
];

/** From this span on, the control also asks for a metric a minute. */
export const LONG_RUN_S = 6 * 3600;

/** The edits a choice of `seconds` makes to `doc`: the duration, and a minute's metric period for a long run. */
export function durationEdits(doc: unknown, seconds: number): { readonly pointer: string; readonly value: unknown }[] {
  const edits: { pointer: string; value: unknown }[] = [{ pointer: "/time/duration_s", value: seconds }];
  if (seconds >= LONG_RUN_S && getPointer(doc, "/time/metric_period_s") === undefined) {
    edits.push({ pointer: "/time/metric_period_s", value: 60 });
  }
  return edits;
}

