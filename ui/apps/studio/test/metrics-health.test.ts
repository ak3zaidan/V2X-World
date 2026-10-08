/**
 * Health bands and the run summary's pure half (`src/metrics/health.ts`, `src/metrics/summaryModel.ts`):
 * classification matches the engine's `HealthBand::classify`, a run's windows are judged and ranked,
 * and the summary's scenario line, reductions, dominant shares and backend loads read correctly.
 */

import { describe, expect, it } from "vitest";

import { bandOf, classify, judge, limitText, outOfBand, verdictLine, verdictParts, whenText, type HealthBand } from "../src/metrics/health.js";
import { familiesOf, type Series } from "../src/metrics/model.js";
import { attackerShare, baseOf, dominant, entityLoads, reduceSeries, scenarioLine } from "../src/metrics/summaryModel.js";

/** The engine's cbr band, as `crates/v2xw-metrics/src/health.rs` publishes it. */
const CBR: HealthBand = {
  better: "lower",
  judged: "",
  caution: 0.5,
  unhealthy: 0.68,
  healthy_note: "below 0.50: congestion control has nothing to do",
  caution_note: "above 0.50: SAE J2945/1 congestion control is lowering power and message rate",
  unhealthy_note: "above the 0.68 ETSI congestion target; vehicles will throttle their messages",
  source: "ETSI TS 102 687",
  basis: "standard",
};

const PDR: HealthBand = { ...CBR, better: "higher", judged: "[100m]", caution: 0.95, unhealthy: 0.9 };

function series(values: readonly number[]): Series {
  return { t: Float64Array.from(values.map((_, i) => i)), v: Float64Array.from(values) };
}

describe("classify", () => {
  it("matches the engine's boundaries: a value at a threshold is the better level", () => {
    expect(classify(CBR, 0.5)).toBe("healthy");
    expect(classify(CBR, 0.51)).toBe("caution");
    expect(classify(CBR, 0.68)).toBe("caution");
    expect(classify(CBR, 0.71)).toBe("unhealthy");
    expect(classify(CBR, null)).toBeNull();
    expect(classify(CBR, Number.NaN)).toBeNull();
    expect(classify(PDR, 0.95)).toBe("healthy");
    expect(classify(PDR, 0.92)).toBe("caution");
    expect(classify(PDR, 0.89)).toBe("unhealthy");
  });

  it("writes the owner's example line", () => {
    expect(verdictLine(CBR, 0.71, "ratio")).toBe("0.71 — above the 0.68 ETSI congestion target; vehicles will throttle their messages");
  });

  it("reads a band off a catalogue row and refuses a malformed one", () => {
    expect(bandOf(CBR)).toEqual(CBR);
    expect(bandOf({ better: "sideways", caution: 1, unhealthy: 2 })).toBeNull();
    expect(bandOf({ better: "lower", caution: "x", unhealthy: 2 })).toBeNull();
    expect(bandOf(null)).toBeNull();
  });

  it("carries the band from the catalogue to the card", () => {
    const [f] = familiesOf([{ name: "cbr", base: "cbr", unit: "ratio", dims: ["t"], agg: "mean", visibility: "NODE", definition: "", notAccounted: ["x"], health: CBR }]);
    expect(f!.health).toEqual(CBR);
  });
});

describe("judge", () => {
  it("finds the worst window, the time out of band and a severity that ranks", () => {
    const j = judge("cbr", "ratio", CBR, series([0.3, 0.4, 0.6, 0.75, Number.NaN, 0.45]))!;
    expect(j.windows).toBe(5);
    expect(j.worst).toBe(0.75);
    expect(j.worstAtS).toBe(3);
    expect(j.worstLevel).toBe("unhealthy");
    expect(j.level).toBe("healthy"); // mean 0.5
    expect(j.unhealthyShare).toBeCloseTo(0.2);
    expect(j.cautionShare).toBeCloseTo(0.2);
    expect(j.outFromS).toBe(2);
    expect(j.outToS).toBe(3);
    expect(whenText(j)).toContain("worst 0.75 at 3 s");
    const bad = judge("e2e", "ms", { ...CBR, caution: 50, unhealthy: 100 }, series([120, 130]))!;
    const fine = judge("x", "ratio", CBR, series([0.1, 0.2]))!;
    expect(outOfBand([fine, j, bad]).map((x) => x.base)).toEqual(["e2e", "cbr"]);
  });

  it("does not judge a series with no observed window", () => {
    expect(judge("cbr", "ratio", CBR, series([Number.NaN]))).toBeNull();
  });
});

describe("the expanded view's verdict line", () => {
  it("names the threshold that decides the level, then the run's shares and worst window", () => {
    expect(limitText(CBR, "unhealthy", "ratio")).toBe("Above 0.68 limit");
    expect(limitText(CBR, "caution", "ratio")).toBe("Above 0.5 target");
    expect(limitText(PDR, "unhealthy", "ratio")).toBe("Below 0.9 limit");
    const lat: HealthBand = { ...CBR, caution: 500, unhealthy: 600 };
    expect(limitText(lat, "healthy", "ms")).toBe("Within 500 ms target");
    const j = judge("lat", "ms", lat, series([100, 700, 834, 650, 550]));
    expect(verdictParts(lat, "unhealthy", "ms", j)).toEqual(["Above 600 ms limit", "unhealthy 60 % of run", "caution 20 %", "worst 834 ms at 2 s"]);
    const ok = judge("lat", "ms", lat, series([100, 200]));
    expect(verdictParts(lat, "healthy", "ms", ok)).toEqual(["Within 500 ms target", "healthy in all 2 windows", "worst 200 ms at 1 s"]);
    expect(verdictParts(lat, "healthy", "ms", null)).toEqual(["Within 500 ms target"]);
  });
});

describe("the summary's pure half", () => {
  it("says the scenario in one line, from what the scenario and the run say", () => {
    const scenario = {
      meta: { name: "manhattan-5min" },
      radio: { rat: "dsrc-80211p" },
      security: { protocol: { id: "protocol/scms/camp" } },
      threats: { attackers: [{ fraction: 0.05 }] },
    };
    expect(scenarioLine(scenario, { tNs: 300e9, tEndNs: 300e9, actors: 113, nodes: 121 }, null)).toBe(
      "manhattan-5min: 300 s simulated, 113 road users at the end, DSRC (IEEE 802.11p), US SCMS credentials, 5 % attackers.",
    );
    expect(scenarioLine(null, { tNs: 0, tEndNs: 0, actors: 0, nodes: 0 }, "fixture")).toBe("fixture.");
    expect(attackerShare({ threats: { attackers: [{ fraction: 0.7 }, { fraction: 0.6 }] } })).toBe(1);
  });

  it("reduces a run's windows, skipping the unobserved ones", () => {
    const s = series([1, Number.NaN, 3, 2]);
    expect(reduceSeries(s, "mean")).toBe(2);
    expect(reduceSeries(s, "max")).toBe(3);
    expect(reduceSeries(s, "min")).toBe(1);
    expect(reduceSeries(s, "last")).toBe(2);
    expect(reduceSeries(s, "sum")).toBe(6);
    expect(reduceSeries(series([Number.NaN]), "mean")).toBeNull();
    expect(baseOf("e2e_latency.p99")).toBe("e2e_latency");
    expect(baseOf("pdr[100m]")).toBe("pdr");
  });

  it("names the dominant share of a breakdown", () => {
    const rows = [
      { key: "airtime", value: 0.2, lo: null, hi: null, n: 10 },
      { key: "verify_queue", value: 0.6, lo: null, hi: null, n: 10 },
      { key: "sign", value: 0.2, lo: null, hi: null, n: 10 },
      { key: "none", value: null, lo: null, hi: null, n: 0 },
    ];
    expect(dominant(rows)).toEqual({ key: "verify_queue", value: 0.6, share: 0.6 });
    expect(dominant([])).toBeNull();
  });

  it("computes backend utilisation from busy time over servers and elapsed time", () => {
    const entity = (id: string, busy: number, servers: number) => ({
      id,
      name: id.toUpperCase(),
      system: "scms",
      tier: "ra",
      online: true,
      node: null,
      role: "",
      queue: { depth: 3, servers, served: 10, busy_ns: busy, waited_ns: 50e6, backlog_ns: 0, inbound: 0 },
      traffic: { received: 0, sent: 0, bytes_in: 0, bytes_out: 0 },
      ops: {},
      state: {},
    });
    const loads = entityLoads(
      { system: "scms", protocol: "", signature: "", t: 0, entities: [entity("pca", 45e9, 1), entity("ra", 90e9, 1), entity("la", 60e9, 2)], edges: [], recent: [], flows: {} },
      100e9,
    );
    expect(loads.map((l) => [l.id, l.utilisation])).toEqual([
      ["ra", 0.9],
      ["pca", 0.45],
      ["la", 0.3],
    ]);
    expect(loads[0]!.meanWaitMs).toBe(5);
  });
});
