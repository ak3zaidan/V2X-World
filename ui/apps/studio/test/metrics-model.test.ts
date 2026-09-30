/**
 * The metrics dashboard's pure half (`src/metrics/model.ts`): the cards built from the catalogue,
 * the groups, the worst-first order, the statistics of a range, the display downsampling, the CSV
 * and the addresses the agent harness links to.
 */

import { describe, expect, it } from "vitest";

import {
  appendSeries,
  chooseBinNs,
  decimate,
  familiesOf,
  formatWithUnit,
  groupOf,
  histogram,
  matchesSearch,
  metricsHash,
  niceTicks,
  parseMetricsHash,
  quantile7,
  rangeStats,
  seriesCsv,
  seriesFromRows,
  toCsv,
  worstFirst,
  DASHBOARD,
  type Series,
  type SeriesDef,
} from "../src/metrics/model.js";

/** Catalogue rows shaped as `crates/v2xw-server/src/live.rs` `metric_series` publishes them. */
function row(name: string, base: string, extra: Partial<SeriesDef> = {}): SeriesDef {
  return { name, base, unit: "ms", dims: ["t"], agg: "mean", visibility: "NODE", definition: "The delay.", notAccounted: ["x"], ...extra };
}

const CATALOGUE: SeriesDef[] = [
  row("e2e_latency", "e2e_latency", { dims: ["t", "flow", "msg_type"], definition: "Mean over the window. End-to-end delay.", source: "08 §2.1" }),
  row("e2e_latency.p50", "e2e_latency", { definition: "The p50 (type-7 quantile) over the window. End-to-end delay." }),
  row("e2e_latency.p95", "e2e_latency", { definition: "The p95 (type-7 quantile) over the window. End-to-end delay." }),
  row("e2e_latency.p99", "e2e_latency", { definition: "The p99 (type-7 quantile) over the window. End-to-end delay." }),
  row("e2e_latency[bsm]", "e2e_latency", { definition: "For msg_type = bsm. Mean over the window." }),
  row("latency_stage[sign]", "latency_stage", { dims: ["t", "flow", "msg_type", "stage"], definition: "For stage = sign. Time in a stage." }),
  row("latency_stage[airtime]", "latency_stage", { dims: ["t", "flow", "msg_type", "stage"], definition: "For stage = airtime. Time in a stage." }),
  row("pdr", "pdr", { unit: "ratio", dims: ["t", "radius", "dist_bin"] }),
  row("mean_speed", "mean_speed", { unit: "m/s", visibility: "GT" }),
];

function series(points: readonly (readonly [number, number])[]): Series {
  return { t: Float64Array.from(points.map((p) => p[0])), v: Float64Array.from(points.map((p) => p[1])) };
}

describe("cards from the catalogue", () => {
  const families = familiesOf(CATALOGUE);
  const byBase = new Map(families.map((f) => [f.base, f]));

  it("makes one card per metric, with its series behind it", () => {
    expect(families.map((f) => f.base)).toEqual(["e2e_latency", "latency_stage", "pdr", "mean_speed"]);
    const e2e = byBase.get("e2e_latency");
    expect(e2e?.headline).toBe("e2e_latency");
    expect(e2e?.percentiles).toEqual(["e2e_latency.p50", "e2e_latency.p95", "e2e_latency.p99"]);
    expect(e2e?.values).toEqual([{ key: "bsm", series: "e2e_latency[bsm]" }]);
    expect(e2e?.valueDim).toBe("msg_type");
    expect(e2e?.definition).toBe("End-to-end delay.");
    expect(e2e?.source).toBe("08 §2.1");
  });

  it("knows a breakdown-only metric has no headline, and which dimension its series are", () => {
    const stage = byBase.get("latency_stage");
    expect(stage?.headline).toBeNull();
    expect(stage?.valueDim).toBe("stage");
    expect(stage?.values.map((v) => v.key)).toEqual(["sign", "airtime"]);
    expect(stage?.definition).toBe("Time in a stage.");
  });

  it("offers every dimension but time and run as a breakdown, and tags ground truth", () => {
    expect(byBase.get("pdr")?.breakdowns).toEqual(["radius", "dist_bin"]);
    expect(byBase.get("latency_stage")?.breakdowns).toEqual(["flow", "msg_type", "stage"]);
    expect(byBase.get("mean_speed")?.groundTruth).toBe(true);
    expect(byBase.get("pdr")?.groundTruth).toBe(false);
  });

  it("groups by the question a researcher asks, and places unknown metrics by name", () => {
    expect(groupOf("cbr")).toBe("channel");
    expect(groupOf("pdr")).toBe("delivery");
    expect(groupOf("e2e_latency")).toBe("latency");
    expect(groupOf("crl_bytes")).toBe("security");
    expect(groupOf("linkability")).toBe("privacy");
    expect(groupOf("mean_speed")).toBe("traffic");
    expect(groupOf("ttc_min")).toBe("safety");
    expect(groupOf("det_recall")).toBe("misbehaviour");
    // A metric another track adds later lands somewhere sensible, not in "other".
    expect(groupOf("fcw_warning_lead_time")).toBe("safety");
    expect(groupOf("pedestrian_delay")).toBe("latency");
    expect(groupOf("jaywalk_count")).toBe("traffic");
    expect(groupOf("something_new")).toBe("other");
  });

  it("finds a card by any word of its name, label, group, unit or definition", () => {
    const e2e = byBase.get("e2e_latency");
    if (!e2e) throw new Error("no card");
    expect(matchesSearch(e2e, "latency")).toBe(true);
    expect(matchesSearch(e2e, "end-to-end delay")).toBe(true);
    expect(matchesSearch(e2e, "p95")).toBe(true);
    expect(matchesSearch(e2e, "msg_type")).toBe(true);
    expect(matchesSearch(e2e, "channel busy")).toBe(false);
  });
});

describe("worst first", () => {
  const rows = [
    { key: "3", value: 0.9 },
    { key: "1", value: 0.5 },
    { key: "2", value: null },
    { key: "10", value: 0.5 },
  ];
  it("puts the lowest first where higher is better, the highest where lower is", () => {
    expect(worstFirst(rows, "higher-better").map((r) => r.key)).toEqual(["1", "10", "3", "2"]);
    expect(worstFirst(rows, "lower-better").map((r) => r.key)).toEqual(["3", "1", "10", "2"]);
  });
});

describe("statistics of a range", () => {
  it("matches the type-7 quantile R and v2xw-metrics use", () => {
    const sorted = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    expect(quantile7(sorted, 0.5)).toBeCloseTo(5.5, 12);
    expect(quantile7(sorted, 0.95)).toBeCloseTo(9.55, 12);
    expect(quantile7(sorted, 0.99)).toBeCloseTo(9.91, 12);
    expect(quantile7([7], 0.95)).toBe(7);
  });

  it("covers only the windows in the range and skips the unobserved ones", () => {
    const s = series([
      [0, 100],
      [1, 1],
      [2, Number.NaN],
      [3, 3],
      [4, 2],
      [5, -100],
    ]);
    const st = rangeStats(s, 1, 4);
    expect(st).not.toBeNull();
    expect(st?.n).toBe(3);
    expect(st?.min).toBe(1);
    expect(st?.max).toBe(3);
    expect(st?.mean).toBeCloseTo(2, 12);
    expect(st?.p50).toBe(2);
    expect(rangeStats(s, 2, 2)).toBeNull();
  });
});

describe("downsampling for display", () => {
  it("keeps every spike of a long series and never more than four points a bucket", () => {
    // Ten hours of a 1 s metric, flat at 0.9 with one spike of 0 at 20,000 s.
    const n = 36_000;
    const t = new Float64Array(n);
    const v = new Float64Array(n);
    for (let i = 0; i < n; i++) {
      t[i] = i;
      v[i] = i === 20_000 ? 0 : 0.9;
    }
    const [xs, ys] = decimate({ t, v }, 0, n, 500);
    expect(xs.length).toBeLessThanOrEqual(4 * 500 + 4);
    expect(ys).toContain(0);
    expect(Math.min(...(ys.filter((y) => y !== null) as number[]))).toBe(0);
    // Time order is kept, so the line is drawn left to right.
    for (let i = 1; i < xs.length; i++) expect(xs[i]).toBeGreaterThanOrEqual(xs[i - 1]);
  });

  it("draws a ten-hour run of eight series in a bounded number of points", () => {
    // What an expanded chart does on every range change: decimate each series to the plot's
    // width, and compute the range's statistics from the full series.
    const n = 36_000;
    const all: Series[] = [];
    for (let k = 0; k < 8; k++) {
      const t = new Float64Array(n);
      const v = new Float64Array(n);
      for (let i = 0; i < n; i++) {
        t[i] = i;
        v[i] = Math.sin(i / (50 + k)) + k;
      }
      all.push({ t, v });
    }
    const started = performance.now();
    let points = 0;
    for (const s of all) {
      points += decimate(s, 0, n, 1200)[0].length;
      expect(rangeStats(s, 0, n)?.n).toBe(n);
    }
    const ms = performance.now() - started;
    expect(points).toBeLessThanOrEqual(8 * (4 * 1200 + 2));
    // eslint-disable-next-line no-console -- the measured number is evidence the report quotes
    console.log(`ten hours x 8 series: ${points} points drawn of ${8 * n}, decimation and statistics in ${ms.toFixed(1)} ms`);
  });

  it("returns a short series as it is, gaps included", () => {
    const [xs, ys] = decimate(series([[0, 1], [1, Number.NaN], [2, 3]]), 0, 2, 100);
    expect(xs).toEqual([0, 1, 2]);
    expect(ys).toEqual([1, null, 3]);
  });

  it("asks the engine for whole metric periods, never finer than one", () => {
    expect(chooseBinNs(60, 1e9, 240)).toBe(1e9);
    expect(chooseBinNs(3600, 1e9, 240)).toBe(15e9);
    expect(chooseBinNs(36_000, 1e9, 240)).toBe(150e9);
    expect(chooseBinNs(10, 1e8, 240)).toBe(1e8);
  });
});

describe("series from the engine", () => {
  it("reads a time-binned answer, null as not observed", () => {
    const [a, b] = seriesFromRows(
      [
        [0, 0.5, null],
        [1_000_000_000, 0.75, 3],
      ],
      2,
    );
    expect([...a.t]).toEqual([0, 1]);
    expect([...a.v]).toEqual([0.5, 0.75]);
    expect(Number.isNaN(b.v[0])).toBe(true);
    expect(b.v[1]).toBe(3);
  });

  it("appends an increment, replacing the window that was still open", () => {
    const base = series([[0, 1], [1, 2], [2, 3]]);
    const more = series([[2, 30], [3, 4]]);
    const out = appendSeries(base, more);
    expect([...out.t]).toEqual([0, 1, 2, 3]);
    expect([...out.v]).toEqual([1, 2, 30, 4]);
  });
});

describe("CSV", () => {
  it("quotes what needs quoting and ends lines with CRLF", () => {
    expect(toCsv(["a", "b"], [["x,y", 1], ['say "hi"', null]])).toBe('a,b\r\n"x,y",1\r\n"say ""hi""",\r\n');
  });

  it("writes every instant of the range at full precision, units in the header", () => {
    const a = series([[0, 0.1], [1, 0.123456789012], [2, 0.3]]);
    const b = series([[1, 5], [3, 7]]);
    const csv = seriesCsv(["pdr", "e2e_latency"], ["ratio", "ms"], [a, b], 1, 3);
    expect(csv).toBe("t_s,pdr (ratio),e2e_latency (ms)\r\n1,0.123456789012,5\r\n2,0.3,\r\n3,,7\r\n");
  });
});

describe("numbers carry units", () => {
  it("formats a delay with its unit and a ratio as a bare number", () => {
    expect(formatWithUnit(12.346, "ms")).toBe("12.35 ms");
    expect(formatWithUnit(0.9421, "ratio")).toBe("0.942");
    expect(formatWithUnit(12_345, "count")).toBe("12,345 count");
    expect(formatWithUnit(null, "ms")).toBe("—");
  });
});

describe("addresses", () => {
  it("round-trips every form the agent harness writes", () => {
    const cases = [
      { ...DASHBOARD },
      { ...DASHBOARD, group: "latency" },
      { ...DASHBOARD, q: "verify wait" },
      { ...DASHBOARD, metric: "e2e_latency" },
      { ...DASHBOARD, metric: "latency_stage[airtime]" },
      { ...DASHBOARD, metric: "pdr", breakdown: "dist_bin" },
      { ...DASHBOARD, metric: "pdr", breakdown: "node", from: 30, to: 90.5 },
    ];
    for (const r of cases) expect(parseMetricsHash(metricsHash(r))).toEqual(r);
  });

  it("reads the documented examples", () => {
    expect(metricsHash({ metric: "pdr", breakdown: "node", from: 30, to: 90 })).toBe("#metrics/pdr/node?from=30&to=90");
    expect(parseMetricsHash("#metrics/latency_stage%5Bairtime%5D")?.metric).toBe("latency_stage[airtime]");
    expect(parseMetricsHash("#settings")).toBeNull();
    // A backwards range is read the right way round; a nonsense one is ignored.
    expect(parseMetricsHash("#metrics/pdr?from=90&to=30")).toMatchObject({ from: 30, to: 90 });
    expect(parseMetricsHash("#metrics/pdr?from=-3&to=abc")).toMatchObject({ from: null, to: null });
  });
});

describe("breakdown helpers", () => {
  it("draws round ticks", () => {
    expect(niceTicks(0, 1, 5)).toEqual([0, 0.2, 0.4, 0.6, 0.8, 1]);
    expect(niceTicks(0, 1050, 6)).toEqual([0, 200, 400, 600, 800, 1000]);
  });

  it("bins thousands of node values and loses none", () => {
    const values = Array.from({ length: 5000 }, (_, i) => (i % 97) / 97);
    const h = histogram(values, 24);
    expect(h.counts.reduce((a, b) => a + b, 0)).toBe(5000);
    expect(h.edges.length).toBe(h.counts.length + 1);
  });
});
