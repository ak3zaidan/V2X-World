/**
 * The credential-system overview's words and numbers (`src/metrics/scmsModel.ts`), against a real
 * Backend snapshot: the conformance deployment's two providers at 546 s
 * (`fixtures/metrics/scms-backend.json`, from `scenarios/scms-audit-mbpipe.yaml`).
 */

import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

import { snapshotOf } from "../src/lib/backend.js";
import type { HealthBand } from "../src/metrics/health.js";
import { expectedBand, isCumulative, judge, whenText, withExpectations, type Judgement } from "../src/metrics/health.js";
import { clockText, dataView, display, familiesOf, type GroupRow, type SeriesDef } from "../src/metrics/model.js";
import { briefSentences, credentialRows } from "../src/metrics/summaryModel.js";
import {
  countOf,
  coveredMetrics,
  cueText,
  duration,
  firstProvider,
  hasCredentials,
  meanOf,
  providerOf,
  roleOf,
  scmsOverview,
  sharePct,
  shown,
  stageLabel,
  stageRank,
  topupOutcome,
  type MetricReader,
} from "../src/metrics/scmsModel.js";

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
    // Under a millisecond reads in µs (and under a microsecond in ns), never as "0.0123 ms" or "5.30e-5 ms".
    expect(shown(0.0123, "ms")).toEqual({ value: "12.3", unit: "µs" });
    expect(shown(5.3e-5, "ms")).toEqual({ value: "53", unit: "ns" });
    expect(shown(0.43, "ms")).toEqual({ value: "430", unit: "µs" });
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
    // No privacy metric in this reader, so no privacy question: a question appears only with its data.
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
    // The reports between two steps are accounted for on the arrow, not left for the reader to subtract.
    expect(f[1]!.drop).toBe("45 not uploaded (2 wait for coverage)");
    expect(f[3]!.drop).toBe("150 wait in the shuffle");
    expect(f[0]!.drop).toBeUndefined();
    // When the RA and the MA hold more than the vehicles count as sent (shared reports), the gap is
    // not reports left behind: no arrow claims it, and the step's hint says why.
    const shared = { ...snap, entities: snap.entities.map((e) => (e.id === "ma" ? { ...e, state: { ...e.state, reports_received: 300 } } : e)) };
    const g = scmsOverview(shared, m, { tNs: snap.t, scenario: null }).sections.find((x) => x.id === "misbehaviour")!.funnel!;
    expect(g[1]!.drop).toBeUndefined();
    expect(g[1]!.hint).toContain("not reports left behind");
  });

  it("calls a low recall pending, not a problem, while the reports wait in the RA's shuffle", () => {
    const recallBand: HealthBand = { ...poolBand, caution: 0.9, unhealthy: 0.5, healthy_note: "most attackers revoked", caution_note: "some missed", unhealthy_note: "fewer than half the attackers are detected" };
    const r = reader({}, { "det_recall/level": [{ key: "vehicle", value: 0 } as GroupRow] }, [def("det_recall", "ratio", recallBand, ["t", "level"])]);
    const mb = scmsOverview(snap, r, { tNs: snap.t, scenario: null }).sections.find((x) => x.id === "misbehaviour")!;
    const t = mb.tiles.find((x) => x.id === "recall")!;
    expect(t.level).toBeNull();
    expect(t.cue).toBeUndefined();
    expect(t.pending).toBe("150 reports wait in the RA's shuffle, released every 2 days; detection is judged once they reach the MA.");
    expect(t.title).toBe("Attackers revoked (detection recall)");
    expect(mb.level).toBeNull();
    expect(mb.pending).toBe(true);
    expect(mb.answer).toContain("150 reports wait in the RA's shuffle");
    // Once the MA has revoked someone, the same recall is judged.
    const later = { ...snap, entities: snap.entities.map((e) => (e.id === "ma" ? { ...e, state: { ...e.state, revocations: 1 } } : e)) };
    const judged = scmsOverview(later, r, { tNs: snap.t, scenario: null }).sections.find((x) => x.id === "misbehaviour")!;
    expect(judged.tiles.find((x) => x.id === "recall")!.level).toBe("unhealthy");
    expect(judged.pending).toBeUndefined();
  });

  it("names recall by what it counts: revoked per vehicle, reported per report", () => {
    const r = reader({}, { "det_recall/level": [{ key: "report", value: 0.9 } as GroupRow] }, defs);
    const t = scmsOverview(snap, r, { tNs: snap.t, scenario: null }).sections.find((x) => x.id === "misbehaviour")!.tiles.find((x) => x.id === "recall")!;
    expect(t.title).toBe("Attackers reported (detection recall, per report)");
  });

  it("reads detection quality pooled over the run, and judges an honest device revoked", () => {
    const recall = tile("misbehaviour", "recall")!;
    expect(recall.value).toBe("50");
    expect(recall.unit).toBe("%");
    expect(recall.source).toContain("level = vehicle");
    expect(tile("misbehaviour", "honest-revoked")!.level).toBe("healthy");
    expect(tile("misbehaviour", "fpr")!.level).toBe("caution");
    expect(section("misbehaviour").answer).toBe("Vehicles have filed 195 misbehaviour reports; 50 % of attackers revoked; no honest vehicle revoked.");
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
    // One decimal for every reason, so "1.0 %" and "0.2 %" compare at a glance.
    expect(b.rows.map((r) => r.display)).toEqual(["1.0 %", "0.2 %"]);
    expect(section("cost").answer).toBe(
      "Security takes 42 % of the bytes on air; 95 % of messages wait under 800 µs to be verified; 1.2 % of receptions are rejected by a security check, a rate no standard judges.",
    );
    expect(tile("cost", "verify-wait")!.value).toBe("800");
    expect(tile("cost", "verify-wait")!.unit).toBe("µs");
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

  it("says in one sentence that every authority is idle, and names each one's provider", () => {
    const b = section("backend").bars!;
    expect(b.idle).toMatch(/^Every authority is idle more than 99(\.9)? % of the time/);
    for (const r of b.rows) expect(r.label).toMatch(/\((provider-a|provider-b)\)$/);
  });

  it("asks whether vehicles reach the backend under the backend question, and names it when it fails", () => {
    const reachBand: HealthBand = { ...poolBand, caution: 0.95, unhealthy: 0.8, unhealthy_note: "many vehicles cannot reach the backend" };
    const r = reader({ backend_link_up: 0.723, cert_pool_valid: 38.5 }, {}, [...defs, def("backend_link_up", "ratio", reachBand)]);
    const o2 = scmsOverview(snap, r, { tNs: snap.t, scenario: null });
    const certs = o2.sections.find((x) => x.id === "certs")!;
    const backend = o2.sections.find((x) => x.id === "backend")!;
    expect(certs.tiles.some((t) => t.id === "backend_link_up")).toBe(false);
    expect(certs.answer.startsWith("Yes:")).toBe(true);
    expect(backend.tiles[0]!.id).toBe("backend_link_up");
    expect(backend.level).toBe("unhealthy");
    expect(backend.answer.startsWith("27.7 % of vehicles cannot reach the backend right now")).toBe(true);
  });

  it("names the flagged tile when a certificate answer is not a plain yes", () => {
    const starved = { ...snap, entities: snap.entities.map((e) => (e.id === "ee" ? { ...e, state: { ...e.state, vehicles_starved: 3 } } : e)) };
    const c = scmsOverview(starved, reader({ cert_pool_valid: 38.5 }, {}, defs), { tNs: snap.t, scenario: null }).sections.find((x) => x.id === "certs")!;
    expect(c.answer).toBe("Not all: 3 vehicles ran out of valid certificates. Otherwise all 23 devices are enrolled, vehicles hold 38.5 valid certificates each on average.");
  });

  it("says what the RA's files are, and why a provider sent none", () => {
    const t = tile("certs", "downloads")!;
    expect(t.title).toBe("Policy and chain files served (by the RA)");
    expect(t.meaning).toContain("provider-a's devices already held the current versions");
  });

  it("explains a vehicle CRL count of zero instead of leaving a bare 0", () => {
    const t = tile("revocation", "crl-installs")!;
    expect(t.value).toBe("0");
    expect(t.meaning).toBe("No vehicle holds a revoked device's entry yet: the lists published so far are empty (nothing is revoked), so there is nothing to install. Roadside units have installed 2 lists.");
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
    expect(s[2]).toBe("All 23 devices were enrolled and the vehicles' mean pool never fell below 38.5 valid pseudonym certificates; none ran out.");
    expect(s[3]).toBe("Vehicles filed 195 misbehaviour reports and 2 devices were revoked, none of them honest; the slowest revocation step took 24 h (p95).");
    expect(s[4]).toBe("Verification takes the largest share of latency (41 %).");
  });

  it("leaves out what the run did not measure, and still says something", () => {
    const s = briefSentences({ judged: null, delivery: null, credentials: null, misbehaviour: null, bottleneck: null });
    expect(s).toEqual(["No metric with a published healthy range has been measured yet."]);
  });
});

describe("the polish pass: one rule, one number format, the run's own expectation", () => {
  const linkBand: HealthBand = {
    ...poolBand,
    better: "lower",
    caution: 0.2,
    unhealthy: 0.5,
    healthy_note: "an observer links few pseudonym changes",
    caution_note: "an observer links more than 1 in 5 pseudonym changes",
    unhealthy_note: "an observer links most pseudonym changes: pseudonyms are not protecting vehicles",
  };
  const privacyDefs = [...defs, def("linkability", "ratio", linkBand), def("pseudonym_change_rate", "1/(veh·h)")];

  it("asks whether a vehicle can be tracked, and answers from linkability", () => {
    const o = scmsOverview(snap, reader({ cert_pool_valid: 38.5, linkability: 0.87, pseudonym_change_rate: 12.4 }, {}, privacyDefs), { tNs: snap.t, scenario: null });
    expect(o.sections.map((s) => s.id)).toEqual(["certs", "revocation", "misbehaviour", "privacy", "cost", "backend"]);
    const p = o.sections.find((s) => s.id === "privacy")!;
    expect(p.question).toBe("Can a vehicle be tracked across pseudonym changes?");
    expect(p.level).toBe("unhealthy");
    expect(p.answer).toBe(
      "Yes, mostly: an eavesdropper links 87 % of pseudonym changes back to the same vehicle, so pseudonyms are not protecting vehicles here; vehicles change pseudonym 12.4 times per vehicle-hour.",
    );
    expect(p.tiles.find((t) => t.id === "pseudonym_change_rate")!.unit).toBe("per vehicle-hour");
    const fine = scmsOverview(snap, reader({ linkability: 0.05 }, {}, privacyDefs), { tNs: snap.t, scenario: null }).sections.find((s) => s.id === "privacy")!;
    expect(fine.level).toBe("healthy");
    expect(fine.answer.startsWith("Rarely: an eavesdropper links only 5 %")).toBe(true);
  });

  it("judges backend reach against the scenario's cellular penetration, and says so", () => {
    const band: HealthBand = { ...poolBand, caution: 0.99, unhealthy: 0.9, source: "engineering choice" };
    const scenario = { net: { uu: { params: { penetration: 0.6 } } } };
    const b = expectedBand("backend_link_up", band, scenario);
    expect(b.caution).toBeCloseTo(0.594);
    expect(b.unhealthy).toBeCloseTo(0.54);
    expect(b.source).toContain("cellular penetration of 60 %");
    // Every other band, and a fully equipped scenario, stay as published.
    expect(expectedBand("cert_pool_valid", band, scenario)).toBe(band);
    expect(expectedBand("backend_link_up", band, { net: { uu: { params: { penetration: 1 } } } })).toBe(band);
    const fams = withExpectations(familiesOf([def("backend_link_up", "ratio", band)]), scenario);
    const r: MetricReader = { family: (x) => fams.find((f) => f.base === x), latest: (s) => (s === "backend_link_up" ? 0.723 : null), series: () => undefined, pooled: () => undefined };
    const backend = scmsOverview(snap, r, { tNs: snap.t, scenario }).sections.find((s) => s.id === "backend")!;
    expect(backend.tiles.find((t) => t.id === "backend_link_up")!.level).toBe("healthy");
  });

  it("writes a top-up count as requests and their outcome", () => {
    expect(topupOutcome(9, 9, 0, 0)).toBe("All completed.");
    expect(topupOutcome(9, 7, 2, 0)).toBe("7 completed, 2 in flight.");
    expect(topupOutcome(0, 0, 0, 0)).toBe("None needed yet.");
  });

  it("writes small shares with one decimal, never an exponent", () => {
    expect(sharePct(0.011)).toBe("1.1 %");
    expect(sharePct(0.00498)).toBe("0.5 %");
    expect(sharePct(0.0004)).toBe("< 0.1 %");
    expect(sharePct(0.42)).toBe("42 %");
  });

  it("reads a ratio that can pass 1 as a multiple, and simulated time as a clock", () => {
    expect(display("envelope_overhead", 1.26, "ratio")).toEqual({ value: "1.26", unit: "× payload" });
    expect(display("unverified_ratio", 0.00499, "ratio")).toEqual({ value: "0.499", unit: "%" });
    expect(clockText(8754)).toBe("2 h 26 min");
    expect(clockText(433_200)).toBe("5 days");
    expect(clockText(48)).toBe("48 s");
  });

  it("says when a judged metric reported in only part of a long run", () => {
    const j = { unhealthyShare: 1, cautionShare: 0, worst: 0.6, worstAtS: 400, unit: "ratio", outFromS: 296, outToS: 592, firstS: 296, lastS: 592, windows: 10 } as unknown as Judgement;
    expect(whenText(j, 52_014)).toContain("all fall between 4 min 56 s and 9 min 52 s of a 14 h 27 min run");
    expect(whenText(j, 560)).not.toContain("fall between");
  });

  it("opens a small chart on the windows that reported when they are a sliver of the run", () => {
    const s = { t: new Float64Array([300, 400, 500, 600]), v: new Float64Array([1, 2, 3, 4]) };
    const v = dataView(s, 433_200);
    expect(v.zoomed).toBe(true);
    expect(v.from).toBeLessThan(300);
    expect(v.to).toBeGreaterThan(600);
    expect(v.to).toBeLessThanOrEqual(700);
    expect(dataView(s, 800)).toMatchObject({ from: 0, to: 800, zoomed: false });
    // A few windows days later do not stretch the view back over the whole run.
    const t = [...Array.from({ length: 40 }, (_, i) => 300 + i * 10), 200_000, 300_000];
    const late = dataView({ t: new Float64Array(t), v: new Float64Array(t.length).fill(1) }, 433_200);
    expect(late.zoomed).toBe(true);
    expect(late.to).toBeLessThan(1000);
    expect(late.inside).toBe(38);
    expect(late.windows).toBe(42);
  });

  it("names the metrics the overview shows, so their cards below do not say \"no data\"", () => {
    const o = scmsOverview(snap, reader({ cert_pool_valid: 38.5 }, { "det_recall/level": [{ key: "vehicle", value: 0.5 } as GroupRow] }, defs), { tNs: snap.t, scenario: null });
    const c = coveredMetrics(o);
    expect(c).toContain("det_recall");
    expect(c).toContain("cert_pool_valid");
    expect(c).not.toContain("loss_rate");
  });

  it("gives the summary's credential tile the authorities' counts, with detection pending by the overview's rule", () => {
    const pending = credentialRows({ revoked: 0, crlVersions: 3, filed: 452, inShuffle: 320, recall: 0 });
    expect(pending.map((r) => r.id)).toEqual(["revoked", "crl-versions", "filed", "detection"]);
    expect(pending[3]!.value).toBe("pending: 320 in the RA's shuffle");
    expect(credentialRows({ revoked: 2, crlVersions: null, filed: null, inShuffle: 320, recall: 0.5 }).find((r) => r.id === "detection")!.value).toBe("50 %");
    const brief = briefSentences({
      judged: null,
      delivery: null,
      credentials: null,
      misbehaviour: { filed: 452, revoked: 0, honestRevoked: null, slowestStageS: null, pendingInShuffle: 320 },
      bottleneck: null,
    });
    expect(brief[1]).toBe("Vehicles filed 452 misbehaviour reports and no device was revoked; 320 reports wait in the RA's shuffle, so detection is not judged yet.");
  });

  it("judges a cumulative metric on its whole-run figure, not on a mean of its windows", () => {
    const band: HealthBand = { ...poolBand, better: "lower", caution: 0.2, unhealthy: 0.5 };
    const s = { t: new Float64Array([0, 60, 120, 180]), v: new Float64Array([0.94, 0.7, 0.4, 0.238]) };
    expect(isCumulative("The share of pseudonym changes a global passive observer bridges correctly, cumulative over the run.")).toBe(true);
    expect(isCumulative("Mean end-to-end latency over the window.")).toBe(false);
    const j = judge("linkability", "ratio", band, s, "Linkability", true)!;
    expect(j.mean).toBe(0.238);
    expect(j.level).toBe("caution");
    expect(j.cumulative).toBe(true);
    expect(judge("linkability", "ratio", band, s, "Linkability")!.level).toBe("unhealthy");
  });
});
