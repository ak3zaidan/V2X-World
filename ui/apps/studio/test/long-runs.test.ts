/**
 * What a run of days needs from the page's clock, axes and duration control.
 *
 * Shown able to fail: with `simClock`'s day branch removed, 26 hours reads `26:00:00.000` and the
 * first assertion fails; with `axisTimeSplits` returning uPlot's decimal steps, no tick of a day's
 * axis falls on an hour.
 */

import { describe, expect, it } from "vitest";

import { axisTime, axisTimeSplits, localClock, simClock } from "../src/lib/format.js";
import { DURATION_PRESETS, durationEdits } from "../src/lib/duration.js";

describe("a clock that counts days", () => {
  it("reads hours as before and days from a day on", () => {
    expect(simClock(90_061.5e9)).toBe("1d 01:01:01");
    expect(simClock(26 * 3600e9)).toBe("1d 02:00:00");
    expect(simClock(3723.25e9)).toBe("01:02:03.250");
  });

  it("reads the run's own clock: t0 plus the simulated time, local mean time at the map", () => {
    // 2027-03-04T05:00:00Z is local midnight at 75° W; a Thursday.
    const t0 = Date.UTC(2027, 2, 4, 5, 0, 0) / 1000;
    expect(localClock(t0, -75, 0)).toBe("Thu 00:00");
    expect(localClock(t0, -75, 8.25 * 3600e9)).toBe("Thu 08:15");
    expect(localClock(t0, -75, 50 * 3600e9)).toBe("Sat 02:00");
    expect(localClock(0, -75, 0)).toBe("");
  });
});

describe("a time axis over hours and days", () => {
  it("puts its ticks on round hours and labels them in the span's unit", () => {
    const ticks = axisTimeSplits(0, 86400);
    expect(ticks.length).toBeGreaterThanOrEqual(4);
    for (const t of ticks) expect(t % 3600).toBe(0);
    expect(axisTime(6 * 3600, 86400)).toBe("6:00");
    expect(axisTime(30 * 3600, 3 * 86400)).toBe("1d 06h");
    expect(axisTime(42, 300)).toBe("42");
  });
});

describe("the duration control", () => {
  it("offers minutes to days, and asks for a metric a minute on a long run the scenario left at the default", () => {
    const spans = DURATION_PRESETS.map((p) => p.s);
    expect(Math.min(...spans)).toBeLessThanOrEqual(60);
    expect(Math.max(...spans)).toBeGreaterThanOrEqual(2 * 86400);
    expect(durationEdits({ time: { duration_s: 300 } }, 300)).toEqual([{ pointer: "/time/duration_s", value: 300 }]);
    expect(durationEdits({ time: {} }, 86400)).toEqual([
      { pointer: "/time/duration_s", value: 86400 },
      { pointer: "/time/metric_period_s", value: 60 },
    ]);
    // A period the scenario chose is the scenario's.
    expect(durationEdits({ time: { metric_period_s: 10 } }, 86400)).toHaveLength(1);
  });
});
