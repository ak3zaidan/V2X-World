/**
 * How long the next run lasts, from a minute to a week, in one control.
 *
 * The duration is `time.duration_s` in the settings window, a number in seconds under a heading a
 * reader has to find; a run of a day is 86400 of them. This is the same setting as a short list of
 * spans a person thinks in, applied for the next run as the settings window applies it (the engine
 * validates it, and the run on screen is not touched until Run is pressed).
 *
 * A span of six hours or more also asks for a metric every minute (`time.metric_period_s: 60`)
 * when the scenario has not chosen a period itself, and says so: a day of every metric at the
 * default one second is tens of millions of samples, which the charts, the engine's memory and the
 * metrics file all pay for (docs/design/findings/long-runs.md). The control is self-contained, so
 * it can sit in the transport bar or among quick settings alike.
 */

import { useCallback, useState } from "react";

import { useStudio } from "../state/store.js";
import { applyDraft } from "../state/settings.js";
import { getPointer, setPointer } from "../lib/schema.js";
import { describeError } from "../lib/errors.js";
import { durationNs } from "../lib/format.js";

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

export function RunDuration(): React.JSX.Element | null {
  const scenario = useStudio((s) => s.scenario);
  const draft = useStudio((s) => s.draft);
  const replay = useStudio((s) => s.replay);
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<string | null>(null);
  const doc = draft ?? scenario;
  const current = getPointer(doc, "/time/duration_s");
  const currentS = typeof current === "number" ? current : undefined;

  const choose = useCallback(
    async (seconds: number) => {
      const base = (useStudio.getState().draft ?? useStudio.getState().scenario) as Record<string, unknown> | null;
      if (base === null) return;
      let next = base;
      const edits = durationEdits(base, seconds);
      for (const e of edits) next = setPointer(next, e.pointer, e.value);
      useStudio.getState().setDraft(next);
      setBusy(true);
      setNote(null);
      try {
        const said = await applyDraft();
        setNote(
          edits.length > 1
            ? `${said} Metrics are sampled once a minute for a run this long; set time.metric_period_s to change that.`
            : said,
        );
      } catch (err) {
        setNote(`The duration was not applied: ${describeError(err)}`);
      } finally {
        setBusy(false);
      }
    },
    [],
  );

  // A recording has its own span, and no next run to set one for.
  if (replay !== null || doc === null) return null;
  const known = DURATION_PRESETS.some((p) => p.s === currentS);
  return (
    <span className="run-duration" data-testid="run-duration">
      <select
        className="tb-duration"
        aria-label="Duration of the next run"
        title="How long the next run lasts in simulated time; applied for the next run, which Run starts"
        value={currentS === undefined ? "" : String(currentS)}
        disabled={busy}
        data-testid="run-duration-select"
        onChange={(e) => {
          const v = Number(e.target.value);
          if (Number.isFinite(v) && v > 0) void choose(v);
        }}
      >
        {!known && currentS !== undefined ? <option value={String(currentS)}>{durationNs(currentS * 1e9)}</option> : null}
        {currentS === undefined ? <option value="">duration</option> : null}
        {DURATION_PRESETS.map((p) => (
          <option key={p.s} value={String(p.s)}>
            {p.label}
          </option>
        ))}
      </select>
      {note !== null ? (
        <span className="timebar-notice" role="status" data-testid="run-duration-note">
          {note}
          <button type="button" className="linklike" onClick={() => setNote(null)} aria-label="Dismiss">
            ✕
          </button>
        </span>
      ) : null}
    </span>
  );
}
