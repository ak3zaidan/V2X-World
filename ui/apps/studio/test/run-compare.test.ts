/**
 * "Compare with previous run"'s pure half (`src/metrics/runCompareModel.ts`): which rows show,
 * how a change is written and coloured, and that the agent is handed both runs, the settings
 * change and the meaningful differences.
 */

import { describe, expect, it } from "vitest";
import type { RunMetricRow, RunsCompareResult } from "@vwp/protocol";

import { agentPrompt, deltaText, exportName, pctText, rowLabel, sideText, tally, verdictClass, visibleRows } from "../src/metrics/runCompareModel.js";

const row = (o: Partial<RunMetricRow>): RunMetricRow => ({
  name: "pdr",
  base: "pdr",
  unit: "ratio",
  better: "higher",
  a: 0.8,
  b: 0.9,
  delta: 0.1,
  pct: 12.5,
  verdict: "better",
  level_a: "caution",
  level_b: "healthy",
  meaningful: true,
  beyond_noise: null,
  ...o,
});

const header = (id: string, seed: number) => ({
  run_id: id,
  label: "manhattan",
  seed,
  city: null,
  scenario_hash: "",
  t_reached_s: 60,
  t_end_s: 60,
  finished: true,
  source: "live",
  replications: 1,
});

const comparison: RunsCompareResult = {
  schema: "v2xw/run-comparison/1",
  a: header("aaaaaaaa-1", 1),
  b: header("bbbbbbbb-2", 1),
  same_seed: true,
  noise_note: "Both runs used seed 1: the differences are caused by the settings change.",
  settings: [{ path: "network.radio.tx_power_dbm", label: "Transmit power", group: "Radio", unit: "dBm", before: 20, after: 23, text: "20 dBm → 23 dBm" }],
  rows: [row({}), row({ name: "cbr", meaningful: false, verdict: "same", delta: 0, pct: 0, level_a: null, level_b: null })],
  summary: ["1 setting changed: Transmit power 20 dBm → 23 dBm."],
};

describe("run comparison model", () => {
  it("shows meaningful rows unless asked for all, and filters by label", () => {
    expect(visibleRows(comparison.rows, false, "").map((r) => r.name)).toEqual(["pdr"]);
    expect(visibleRows(comparison.rows, true, "").map((r) => r.name)).toEqual(["pdr", "cbr"]);
    expect(visibleRows(comparison.rows, true, "CBR").map((r) => r.name)).toEqual(["cbr"]);
  });

  it("writes and colours a change by its verdict", () => {
    expect(deltaText(row({ delta: -3.2, unit: "ms" }))).toMatch(/^−3\.2/);
    expect(pctText(row({ pct: 12.5 }))).toBe("+13 %");
    expect(pctText(row({ pct: -2.5 }))).toBe("−2.5 %");
    expect(pctText(row({ pct: null }))).toBe("");
    expect(verdictClass(row({ verdict: "worse" }))).toBe("worse");
    expect(verdictClass(row({ verdict: "changed" }))).toBe("neutral");
    expect(rowLabel("latency.p95")).toMatch(/\(p95\)$/);
  });

  it("counts and names", () => {
    expect(tally(comparison.rows)).toEqual({ meaningful: 1, better: 1, worse: 0, total: 2 });
    expect(exportName(comparison, "csv")).toBe("compare-aaaaaaaa-vs-bbbbbbbb.csv");
    expect(sideText(comparison.a)).toBe("manhattan · seed 1 · 60 s");
  });

  it("hands the agent both runs, the setting change and the meaningful rows", () => {
    const p = agentPrompt(comparison);
    expect(p).toContain("aaaaaaaa-1");
    expect(p).toContain("bbbbbbbb-2");
    expect(p).toContain("Transmit power");
    expect(p).toContain("- pdr:");
    expect(p).not.toContain("- cbr:");
  });
});
