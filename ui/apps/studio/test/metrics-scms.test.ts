/**
 * The credential-system overview's words and numbers (`src/metrics/scmsModel.ts`), against a real
 * Backend snapshot: the conformance deployment's two providers at 546 s
 * (`fixtures/metrics/scms-backend.json`, from `scenarios/scms-audit-mbpipe.yaml`).
 */

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { snapshotOf } from "../src/lib/backend.js";
import type { HealthBand } from "../src/metrics/health.js";
import { familiesOf, type GroupRow, type SeriesDef } from "../src/metrics/model.js";
import { briefSentences } from "../src/metrics/summaryModel.js";
import { countOf, cueText, duration, firstProvider, hasCredentials, meanOf, providerOf, roleOf, scmsOverview, shown, stageLabel, stageRank, type MetricReader } from "../src/metrics/scmsModel.js";

const snap = snapshotOf(JSON.parse(readFileSync(new URL("./fixtures/metrics/scms-backend.json", import.meta.url), "utf8")))!;

const poolBand: HealthBand = {
  better: "higher",
  judged: "",
  caution: 5,
  unhealthy: 1,
  healthy_note: "vehicles hold enough valid pseudonym certificates to keep changing them",
  caution_note: "vehicles are down to a handful",
  unhealthy_note: "vehicles have run out",
  source: "engineering choice: 20 certificates a week (Whyte et al., IEEE VNC 2013)",
  basis: "engineering",
};
const fprBand: HealthBand = { ...poolBand, better: "lower", caution: 0.01, unhealthy: 0.05, healthy_note: "at most 1 % wrongly flagged", caution_note: "above 1 %", unhealthy_note: "above 5 %" };

function def(name: string, unit: string, health: HealthBand | null = null, dims: string[] = ["t"]): SeriesDef {
  return { name, base: name.replace(/\.p\d+$/, ""), unit, dims, agg: "mean", visibility: "PUBLIC", definition: `${name} definition.`, notAccounted: [], health };
}

function reader(values: Record<string, number>, pooled: Record<string, GroupRow[]> = {}, defs: SeriesDef[] = []): MetricReader {
  const families = familiesOf(defs);
  return {
    family: (b) => families.find((f) => f.base === b),
    latest: (s) => values[s] ?? null,
    series: () => undefined,
    pooled: (m, d) => pooled[`${m}/${d}`],
  };
}

const defs = [
  def("cert_pool_valid", "count", poolBand),
  def("crl_entries", "count"),
  def("det_recall", "ratio", null, ["t", "level"]),
  def("det_fpr", "ratio", fprBand, ["t", "level"]),
  def("false_accusations", "count", null, ["t", "level"]),
  def("security_overhead", "ratio"),
  def("verify_wait", "ms"),
  def("verify_wait.p95", "ms"),
  def("loss_rate", "ratio", null, ["t", "cause"]),
  def("revocation_latency_stage", "s", null, ["t", "stage"]),
];

describe("reading the Backend snapshot", () => {
  it("names a role and a provider whatever the prefix", () => {
    expect(roleOf("provider-b/crl-store")).toBe("crl-store");
    expect(roleOf("ra")).toBe("ra");
    expect(providerOf({ id: "provider-b/ra", state: {} }, "provider-a")).toBe("provider-b");
    expect(providerOf({ id: "ra", state: {} }, "provider-a")).toBe("provider-a");
  });

  it("adds a count across providers and keeps each provider's share", () => {
    const issued = countOf(snap, ["pca"], ["certs_issued"], "provider-a")!;
    expect(issued.total).toBe(640 + 204);
    expect(Object.fromEntries(issued.byProvider)).toEqual({ "provider-a": 640, "provider-b": 204 });
    expect(countOf(snap, ["pca"], ["no_such_count"], "a")).toBeNull();
    expect(countOf(null, ["pca"], ["certs_issued"], "a")).toBeNull();
  });
});

describe("numbers as a reader wants them", () => {
  it("shows a ratio as a percentage and a long time in a long unit", () => {
    expect(shown(0.974, "ratio")).toEqual({ value: "97.4", unit: "%" });
    expect(shown(0.0123, "ms")).toEqual({ value: "0.0123", unit: "ms" });
    expect(duration(0.25)).toEqual({ value: "250", unit: "ms" });
    expect(duration(86_400)).toEqual({ value: "24", unit: "h" });
    expect(duration(604_800)).toEqual({ value: "7", unit: "days" });
    expect(shown(2048, "B")).toEqual({ value: "2", unit: "KiB" });
    expect(shown(null, "s")).toEqual({ value: "—", unit: "" });
  });

  it("states a cue's thresholds, basis and source", () => {
    const t = cueText(fprBand, "caution", "ratio");
    expect(t).toContain("healthy up to 1 %");
    expect(t).toContain("unhealthy above 5 %");
    expect(t).toContain("no standard sets it");
    expect(t).toContain("Whyte");
  });

  it("words a revocation stage in plain English, and orders the steps as a revocation runs", () => {
    expect(stageLabel("decision->issued")).toBe("MA decides → on a signed CRL");
    expect(stageLabel("weird_one->other")).toBe("weird one → other");
    const keys = ["issued->published", "detect->report_sent", "report_received->decision", "odd->thing", "published->downloaded"];
    expect([...keys].sort((a, b) => stageRank(a) - stageRank(b))).toEqual([
      "detect->report_sent",
      "report_received->decision",
      "issued->published",
      "published->downloaded",
      "odd->thing",
    ]);
  });

  it("names the home provider from the scenario, else beside its sibling", () => {
    expect(firstProvider({ security: { scms: { deployment: { providers: [{ name: "acme" }] } } } }, snap)).toBe("acme");
    expect(firstProvider(null, snap)).toBe("provider-a");
    expect(firstProvider(null, null)).toBe("home provider");
  });
});

describe("the five questions", () => {
  const m = reader(
    { cert_pool_valid: 38.5, crl_entries: 0, security_overhead: 0.42, "verify_wait.p95": 0.8 },
    {
      "det_recall/level": [{ key: "report", value: 0.9, lo: null, hi: null } as GroupRow, { key: "vehicle", value: 0.5 } as GroupRow],
      "false_accusations/level": [{ key: "vehicle", value: 0 } as GroupRow, { key: "report", value: 3 } as GroupRow],
      "det_fpr/level": [{ key: "report", value: 0.02 } as GroupRow],
      "loss_rate/cause": [
        { key: "collision", value: 0.2 } as GroupRow,
        { key: "unknown-signer", value: 0.01 } as GroupRow,
        { key: "signature-invalid", value: 0.002 } as GroupRow,
      ],
    },
    defs,
  );
  const o = scmsOverview(snap, m, { tNs: snap.t, scenario: { security: { scms: { deployment: { providers: [{ name: "provider-a" }, { name: "provider-b" }] } } } } });
  const section = (id: string) => o.sections.find((s) => s.id === id)!;
  const tile = (sid: string, id: string) => section(sid).tiles.find((t) => t.id === id);

  it("says which system, how many providers and which signature", () => {
    expect(o.system).toContain("US SCMS");
    expect(o.providers).toEqual(["provider-a", "provider-b"]);
    expect(o.facts).toBe("2 providers: provider-a, provider-b · ECDSA P-256 signatures");
    expect(o.sections.map((s) => s.id)).toEqual(["certs", "revocation", "misbehaviour", "cost", "backend"]);
  });

  it("answers whether devices get certificates, with the provider split", () => {
    const enrolled = tile("certs", "enrolled")!;
    expect(enrolled.value).toBe("23 of 23");
    expect(enrolled.level).toBe("healthy");
    expect(enrolled.split).toBe("provider-a 16 · provider-b 7");
    expect(tile("certs", "issued")!.value).toBe("844");
    const pool = tile("certs", "cert_pool_valid")!;
    expect(pool.value).toBe("38.5");
    expect(pool.level).toBe("healthy");
    expect(pool.cue).toContain("Whyte");
    expect(tile("certs", "starved")!.level).toBe("healthy");
    expect(section("certs").answer).toBe("Yes: all 23 devices are enrolled, vehicles hold 38.5 valid certificates each on average, none has run out.");
  });

  it("judges a pool below its band, and says so in the answer", () => {
    const low = scmsOverview(snap, reader({ cert_pool_valid: 3 }, {}, defs), { tNs: snap.t, scenario: null });
    const s = low.sections.find((x) => x.id === "certs")!;
    expect(s.level).toBe("caution");
    expect(s.answer.startsWith("Mostly:")).toBe(true);
  });

  it("follows a report from the vehicle to the MA", () => {
    const f = section("misbehaviour").funnel!;
    expect(f.map((x) => [x.id, x.value])).toEqual([
      ["filed", 195],
      ["sent", 150],
      ["shuffle", 150],
      ["ma", 0],
      ["cases", 0],
      ["revoked", 0],
    ]);
    expect(f[1]!.hint).toContain("74 over cellular, 76 relayed by roadside units; 2 waiting for coverage");
  });

  it("reads detection quality pooled over the run, and judges an honest device revoked", () => {
    const recall = tile("misbehaviour", "recall")!;
    expect(recall.value).toBe("50");
    expect(recall.unit).toBe("%");
    expect(recall.source).toContain("level = vehicle");
    expect(tile("misbehaviour", "honest-revoked")!.level).toBe("healthy");
    expect(tile("misbehaviour", "fpr")!.level).toBe("caution");
    expect(section("misbehaviour").answer).toBe("Vehicles have filed 195 misbehaviour reports, 50 % of attackers caught, no honest vehicle revoked.");
    const bad = scmsOverview(snap, reader({}, { "false_accusations/level": [{ key: "vehicle", value: 2 } as GroupRow] }, defs), { tNs: snap.t, scenario: null });
    expect(bad.sections.find((x) => x.id === "misbehaviour")!.level).toBe("unhealthy");
  });

  it("says nothing has been revoked, and that the CRL is still published", () => {
    expect(section("revocation").answer).toBe("Nothing has been revoked yet; the CRL is still published on schedule (2 so far).");
    expect(tile("revocation", "crl-versions")!.meaning).toContain("every 24 h");
    expect(section("revocation").bars!.rows).toEqual([]);
  });

  it("lists only the security checks that rejected messages, largest first", () => {
    const b = section("cost").bars!;
    expect(b.rows.map((r) => r.key)).toEqual(["unknown-signer", "signature-invalid"]);
    expect(b.rows[0]!.display).toBe("1 %");
    expect(section("cost").answer).toBe("Security takes 42 % of the bytes on air; 95 % of messages wait under 0.8 ms to be verified; 1.2 % of receptions are rejected by a security check.");
  });

  it("reads a bursty rate as its mean over the windows that reported, not its idle newest window", () => {
    const series = { t: new Float64Array([1, 2, 3, 4]), v: new Float64Array([100, Number.NaN, 300, 0]) };
    expect(meanOf(series)).toBeCloseTo(400 / 3);
    const r: MetricReader = { ...reader({ bytes_backend: 0 }, {}, [def("bytes_backend", "B/s")]), series: () => series };
    const t = scmsOverview(null, r, { tNs: 4e9, scenario: null }).sections.find((s) => s.id === "cost")!.tiles.find((x) => x.id === "bytes_backend")!;
    expect(t.value).toBe("133");
    expect(t.source).toContain("mean of the windows that reported");
  });

  it("names the busiest authority against the 80 % ceiling", () => {
    const b = section("backend");
    expect(b.tiles[0]!.id).toBe("backend-busiest");
    expect(b.level).toBe("healthy");
    expect(b.bars!.limit!.at).toBe(0.8);
  });

  it("keeps every authority's raw counts", () => {
    const ma = o.raw.find((r) => r.id === "ma")!;
    expect(ma.counts.find(([k]) => k === "decision_rule")![1]).toBe("threat/ma/corroborated");
  });

  it("shows the overview for a run with a credential system only", () => {
    expect(hasCredentials(snap, reader({}))).toBe(true);
    expect(hasCredentials(null, reader({}))).toBe(false);
    expect(hasCredentials(null, reader({}, {}, [def("crl_entries", "count")]))).toBe(true);
  });
});

describe("the run in brief", () => {
  it("reads as three to five sentences", () => {
    const s = briefSentences({
      judged: { unhealthy: 1, caution: 0, atTimes: 1, healthy: 9, worstLabel: "Verification wait" },
      delivery: { pdr: 0.94, latencyMs: 3.2 },
      credentials: { enrolled: 23, devices: 23, pool: 38.5, starved: 0 },
      misbehaviour: { filed: 195, revoked: 2, honestRevoked: 0, slowestStageS: 86_400 },
      bottleneck: { stage: "verification", stageShare: 0.41, entity: "Registration Authority", utilisation: 0.03 },
    });
    expect(s.length).toBeGreaterThanOrEqual(3);
    expect(s.length).toBeLessThanOrEqual(5);
    expect(s[0]).toBe("Of 11 metrics with a published healthy range, 9 stayed healthy, 1 was out of range at times and 1 was unhealthy on average; the worst is verification wait.");
    expect(s[1]).toBe("Messages within 100 m were delivered 94 % of the time, in 3.2 ms end to end on average.");
    expect(s[2]).toBe("All 23 devices were enrolled and vehicles held 38.5 valid pseudonym certificates on average at the lowest point; none ran out.");
    expect(s[3]).toBe("Vehicles filed 195 misbehaviour reports and 2 devices were revoked, none of them honest; the slowest revocation step took 24 h (p95).");
    expect(s[4]).toBe("Verification takes the largest share of latency (41 %).");
  });

  it("leaves out what the run did not measure, and still says something", () => {
    const s = briefSentences({ judged: null, delivery: null, credentials: null, misbehaviour: null, bottleneck: null });
    expect(s).toEqual(["No metric with a published healthy range has been measured yet."]);
  });
});
