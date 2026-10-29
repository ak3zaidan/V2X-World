/**
 * The PKI view's Vehicles, Cases and Revocation-lists tabs and map overlays (WP-E3) on the
 * engine's fixtures: `fixtures/pki/pki-run.jsonl` (the records a two-provider Manhattan run
 * published) and `fixtures/pki/pki-tabs.json` (what the server's `inspect.pki` answers on that
 * slice, `crates/v2xw-server/tests/pki.rs`).
 *
 * The acceptance of 00-gap-analysis WP-E3: the i × j grid matches `node.credentials`, the case
 * board's stages match `pki.case`, and the knowledge toggle hides what only ground truth knows.
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import {
  BOARD_COLUMNS,
  GROUND_TRUTH,
  STEP_LEARNS,
  asCases,
  asCrl,
  asCrls,
  asVehicle,
  asVehicles,
  boardColumn,
  credentialState,
  crlRows,
  enforcement,
  evidenceText,
  flatCells,
  fullGrid,
  mapTags,
  overdueProviders,
  slaText,
  stageTimes,
  time32Text,
  viewChoices,
  whoLearned,
  type CredentialContext,
  type VehicleRow,
} from "../src/lib/pkiRecords.js";

const dir = (name: string): string => fileURLToPath(new URL(`./fixtures/pki/${name}`, import.meta.url));
interface Row { channel: string; t: number; record: Record<string, any> } // eslint-disable-line @typescript-eslint/no-explicit-any
const rows: Row[] = readFileSync(dir("pki-run.jsonl"), "utf8").trim().split("\n").map((l) => JSON.parse(l) as Row);
const tabs = JSON.parse(readFileSync(dir("pki-tabs.json"), "utf8")) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any

const lastCredentials = new Map<string, Record<string, unknown>>();
for (const r of rows) if (r.channel === "node.credentials") lastCredentials.set(String(r.record.device), r.record);

describe("the Vehicles tab", () => {
  it("rebuilds every vehicle's i × j grid cell for cell from node.credentials", () => {
    expect(lastCredentials.size).toBeGreaterThanOrEqual(4);
    for (const [device, rec] of lastCredentials) {
      const d = asVehicle(tabs.vehicle[device]);
      expect(d, device).not.toBeNull();
      const g = fullGrid(d!);
      expect(flatCells(g)).toEqual(rec.cells);
      expect(g.columns.length * g.jmax).toBe((rec.cells as unknown[]).length);
      expect(d!.active).toBe(rec.active);
      expect(d!.history).toEqual(rec.history);
    }
  });

  it("colours a vehicle revoked in ground truth only while ground truth is on", () => {
    const list = asVehicles(tabs.vehicles)!;
    const crls = asCrls(tabs.crls)!;
    const base: Omit<CredentialContext, "groundTruth"> = { nowNs: list.t_ns, offCtl: new Set(), crlOverdue: overdueProviders(crls) };
    const gtRevoked = list.vehicles.filter((v) => (v.truth?.revoked_certs ?? 0) > 0);
    expect(gtRevoked.length).toBeGreaterThan(0);
    for (const v of gtRevoked) {
      expect(credentialState(v, { ...base, groundTruth: true })).toBe("revoked");
      // The vehicle has not installed a list naming it: blind, it is not revoked.
      if (!v.revoked) expect(credentialState(v, { ...base, groundTruth: false })).not.toBe("revoked");
    }
  });

  it("orders credential states as triage reads them", () => {
    const row: VehicleRow = {
      device: "ee/1", node: 1, provider: "p", t: 0, enrolled: true, provisioned: true, active: "aa", current_i: 5, jmax: 20,
      pool: { active: 1, valid: 19 }, usable: 20, periods_ahead: 1, pseudonym_changes: 0, used: 1, revoked: false,
      security: { next_topup: 100 },
    };
    const ctx: CredentialContext = { nowNs: 50, offCtl: new Set(), crlOverdue: new Set(), groundTruth: false };
    expect(credentialState(row, ctx)).toBe("healthy");
    expect(credentialState(row, { ...ctx, nowNs: 100 })).toBe("topup-due");
    expect(credentialState({ ...row, usable: 0 }, ctx)).toBe("starving");
    // No active certificate but usable ones in the pool: changing pseudonym, not starving.
    expect(credentialState({ ...row, active: null }, ctx)).toBe("between");
    expect(credentialState({ ...row, active: null, usable: 0 }, ctx)).toBe("starving");
    expect(credentialState(row, { ...ctx, crlOverdue: new Set(["p"]) })).toBe("crl-overdue");
    expect(credentialState(row, { ...ctx, offCtl: new Set(["p"]) })).toBe("untrusted");
    expect(credentialState({ ...row, revoked: true }, { ...ctx, offCtl: new Set(["p"]) })).toBe("revoked");
  });
});

describe("the Cases tab", () => {
  const recorded = new Map<string, Record<string, unknown>[]>();
  for (const r of rows) if (r.channel === "pki.case") recorded.set(String(r.record.case), [...(recorded.get(String(r.record.case)) ?? []), r.record]);
  const board = asCases(tabs.cases[GROUND_TRUTH])!;

  it("puts every case in the column of the furthest pki.case stage it reached, with the recorded times", () => {
    expect(board.cases.length).toBe(recorded.size);
    expect(board.cases.length).toBeGreaterThan(0);
    for (const c of board.cases) {
      const rec = recorded.get(c.case)!;
      const furthest = Math.max(...rec.map((s) => BOARD_COLUMNS.findIndex((b) => b.stage === s.stage)));
      expect(boardColumn(c)).toBe(BOARD_COLUMNS[furthest].stage);
      const times = stageTimes(c);
      // One row per stage, at its first record: `enforced` repeats once per enforcing vehicle.
      const firsts = rec.filter((s, k) => rec.findIndex((x) => x.stage === s.stage) === k);
      expect(times.map((s) => [s.stage, s.t])).toEqual(firsts.map((s) => [s.stage, s.t]));
    }
  });

  it("counts enforcement up to the newest vehicle, not the first (the frozen 'n of N')", () => {
    let counted = 0;
    for (const c of board.cases) {
      const rec = recorded.get(c.case)!.filter((s) => s.stage === "enforced");
      if (rec.length === 0) continue;
      const newest = rec[rec.length - 1].detail as { installed: number; of: number };
      expect(enforcement(c)).toEqual({ installed: newest.installed, of: newest.of });
      expect(newest.installed).toBe(rec.length);
      counted = Math.max(counted, rec.length);
    }
    expect(counted, "some case is enforced by more than one vehicle in the fixture").toBeGreaterThan(1);
  });

  it("puts each report's SM-MBR evidence in words, or why it was modelled", () => {
    const texts = board.cases.flatMap((c) => {
      const ev = c.stages.find((s) => s.stage === "reports-in")?.detail.evidence;
      return Array.isArray(ev) ? ev.map(evidenceText) : [];
    });
    expect(texts.length).toBe(board.cases.length);
    // Re-recorded 2026-10-23: the run's two cases are provider-a's, each opened on a real
    // report provider-b's MA shared. A roadside unit no longer files a report (R-UNSEC), so
    // the run has no modelled evidence and no case of the reporter's own MA any more.
    expect(texts).toContain("1VInconsistent/ConstantPosition, verified, shared by the other MA");
    expect(texts.every((t) => t === "1VInconsistent/ConstantPosition, verified, shared by the other MA")).toBe(true);
    expect(texts).not.toContain("unknown");
    // The two forms the run no longer produces, as the 2026-10-22 recording carried them
    // (`pki.case` `reports-in` of provider-b/linkage-resolution/61 and
    // provider-a/linkage-resolution/500).
    expect(
      evidenceText({ modelled: null, report: 34, shared: false, subtypes: [{ name: "ConstantPosition", subtype: 1 }], type: 1, type_name: "1VInconsistent", verified: true }),
    ).toBe("1VInconsistent/ConstantPosition, verified");
    expect(
      evidenceText({
        checks: ["positionSpeedInconsistency", "constantPositionFrozen", "staleOrReplay"],
        modelled: "a roadside unit holds no pseudonym certificate to sign an SM-MBR with",
        report: 437,
        shared: false,
        subtypes: [],
        type: null,
        type_name: null,
        verified: null,
      }),
    ).toBe("modelled (a roadside unit holds no pseudonym certificate to sign an SM-MBR with): positionSpeedInconsistency+constantPositionFrozen+staleOrReplay");
  });

  it("names who learned what from each step", () => {
    for (const c of board.cases) {
      const learned = whoLearned(c);
      expect(learned.map((l) => l.who)).toEqual(expect.arrayContaining([`${c.provider}/aca/0`, `${c.provider}/la1/0`, `${c.provider}/la2/0`, `${c.provider}/crlg/0`]));
      // Every message a case sends is one the tab can put in words.
      const unknown = c.steps.filter((s) => s.kind === "msg" && STEP_LEARNS[s.name] === undefined).map((s) => s.name);
      expect(unknown).toEqual([]);
    }
  });

  it("hides every identity, and the parties' map rings, outside ground truth", () => {
    expect(board.cases.every((c) => (c.truth?.accused.length ?? 0) > 0 && (c.truth?.reporters.length ?? 0) > 0)).toBe(true);
    const gtRings = mapTags("case", { vehicles: [], cases: board.cases, view: GROUND_TRUTH, nowNs: 0 });
    expect(gtRings.size).toBeGreaterThan(0);
    // Even handed ground-truth cases, a view that is not ground truth draws no party.
    expect(mapTags("case", { vehicles: [], cases: board.cases, view: "provider-a/ma/0", nowNs: 0 }).size).toBe(0);
    for (const view of Object.keys(tabs.cases).filter((v) => v !== GROUND_TRUTH)) {
      const as = asCases(tabs.cases[view])!;
      expect(JSON.stringify(as)).not.toContain('"ee/');
      expect(as.cases.every((c) => c.truth === undefined)).toBe(true);
      expect(mapTags("case", { vehicles: [], cases: as.cases, view, nowNs: 0 }).size).toBe(0);
      // Nor do the steps carry any PDU bytes or plaintext, in any view.
      expect(JSON.stringify(as)).not.toMatch(/"hex"|"plaintext"/);
    }
    expect(JSON.stringify(board)).not.toMatch(/"hex"|"plaintext"/);
  });

  it("offers each provider's components as views, never a device", () => {
    const v = viewChoices(asCrls(tabs.crls)!.providers.map((p) => p.label));
    expect(v[0]).toBe(GROUND_TRUTH);
    expect(v).toContain("provider-b/la2/0");
    expect(v.some((x) => x.startsWith("ee/"))).toBe(false);
  });
});

describe("the Revocation-lists tab", () => {
  const crls = asCrls(tabs.crls)!;

  it("groups every CRL version one row per provider × series × kind, newest first", () => {
    const r = crlRows(crls);
    expect(r.reduce((n, x) => n + x.versions.length, 0)).toBe(crls.crls.length);
    for (const x of r) {
      expect(x.versions[0]).toBe(x.newest);
      for (let k = 1; k < x.versions.length; k++) expect(x.versions[k - 1].issued_at).toBeGreaterThanOrEqual(x.versions[k].issued_at);
    }
  });

  it("joins a linkage entry onto the run's vehicles in ground truth only", () => {
    let matched = 0;
    for (const [id8, views] of Object.entries(tabs.crl as Record<string, Record<string, unknown>>)) {
      const gt = asCrl(views[GROUND_TRUTH])!;
      const other = asCrl(views.other)!;
      expect(other.linkage.length, id8).toBe(gt.linkage.length);
      expect(other.owners).toBeUndefined();
      expect(other.linkage.every((e) => e.matches === undefined)).toBe(true);
      for (const e of gt.linkage) {
        for (const m of e.matches ?? []) {
          expect(gt.owners?.[m]?.device, m).toMatch(/^ee\//);
          matched++;
        }
      }
    }
    expect(matched).toBeGreaterThan(0);
  });

  it("reads the SLA clock and Time32 the way the record does", () => {
    expect(slaText(604_790)).toBe("6 d 23 h");
    expect(slaText(-7_260)).toBe("overdue 2 h 1 min");
    // Time32 0 is 2004-01-01 00:00:00 TAI.
    expect(time32Text(5)).toBe("2004-01-01 00:00 UTC");
  });
});
