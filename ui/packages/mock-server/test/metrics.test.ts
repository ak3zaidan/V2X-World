/**
 * The mock's `metrics.query` store: binned series over any range, a seek back rewriting the tail,
 * and grouped breakdowns in the real engine's row shape (`[key, value, lo, hi, n]`).
 */

import { describe, expect, it } from "vitest";

import { MOCK_CATALOGUE, MockMetricHistory, catalogueAnswer } from "../src/metrics.js";

const S = 1_000_000_000;

describe("mock metric history", () => {
  it("bins what was sent, with gaps as null", () => {
    const h = new MockMetricHistory();
    h.record(1 * S, [{ name: "pdr", value: 0.9 }]);
    h.record(2 * S, [{ name: "pdr", value: 0.8 }]);
    h.record(4 * S, [{ name: "pdr", value: 0.6 }]);
    expect(h.series(["pdr"], 0, 4 * S, S, 100)).toEqual([[0, null], [S, 0.9], [2 * S, 0.8], [3 * S, null], [4 * S, 0.6]]);
    expect(h.series(["pdr"], 0, 4 * S, 2 * S, 100)).toEqual([[0, 0.9], [2 * S, 0.8], [4 * S, 0.6]]);
    expect(h.series(["pdr"], 0, 4 * S, S, 2)).toHaveLength(2);
  });

  it("forgets the future on a seek back", () => {
    const h = new MockMetricHistory();
    for (let t = 1; t <= 5; t++) h.record(t * S, [{ name: "cbr", value: t / 10 }]);
    h.truncateAfter(2 * S);
    expect(h.series(["cbr"], 0, 5 * S, S, 100).filter((r) => r[1] !== null)).toHaveLength(2);
  });

  it("groups by a declared dimension only, in the engine's row shape", () => {
    const h = new MockMetricHistory();
    for (let t = 1; t <= 5; t++) h.record(t * S, [{ name: "pdr", value: 0.9 }]);
    const bins = h.groups("pdr", "dist_bin", 0, 5 * S, []);
    expect(bins.length).toBeGreaterThan(3);
    for (const row of bins) {
      expect(row).toHaveLength(5);
      const [, v, lo, hi] = row as [string, number, number, number, number];
      expect(lo).toBeLessThanOrEqual(v);
      expect(hi).toBeGreaterThanOrEqual(v);
    }
    // Delivery falls with distance.
    expect(bins[0][1] as number).toBeGreaterThan(bins[bins.length - 1][1] as number);
    expect(h.groups("pdr", "node", 0, 5 * S, [7, 8, 9]).map((r) => r[0])).toEqual(["7", "8", "9"]);
    expect(h.groups("pdr", "stage", 0, 5 * S, [])).toEqual([]);
    expect(h.groups("pdr", "dist_bin", 10 * S, 20 * S, [])).toEqual([]);
  });

  it("publishes a catalogue row per metric with its unit and dimensions", () => {
    const rows = catalogueAnswer() as { name: string; unit: string; dims: string[]; base: string }[];
    expect(rows.map((r) => r.name)).toEqual(MOCK_CATALOGUE.map((m) => m.name));
    expect(rows.every((r) => r.unit !== "" && r.dims.includes("t") && r.base === r.name)).toBe(true);
  });
});
