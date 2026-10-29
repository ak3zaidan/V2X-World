/**
 * The certificate views' polish pass (the fresh-eyes review's fixes): one credential state per
 * vehicle with the ones that left the run apart, the revocation schedule said once, a failing
 * certificate's reason as a sentence, a case as four beats, links that never cross another
 * provider's boxes, and a glossary entry behind every term a view marks.
 */

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { trustOf } from "../src/lib/pki.js";
import { beatNow, evidencePlain, storyBeats } from "../src/lib/pkiCases.js";
import { VEHICLE_STATE, certErrorText, deviceName, growing, whySentence } from "../src/lib/pkiCert.js";
import { glossary, GLOSSARY } from "../src/lib/pkiGlossary.js";
import { arrowLabel, fitLabel, wordsOf } from "../src/lib/pkiPdu.js";
import { CREDENTIAL_LABEL, CREDENTIAL_ORDER, LEFT_RUN_NS, STEP_LEARNS, asCases, credentialState, leftRun, type CredentialContext, type VehicleRow } from "../src/lib/pkiRecords.js";
import { componentName, crlScheduleText, edgePath, groupMessages, layoutTopology, messagesOf, type TrustMessage } from "../src/lib/pkiTopology.js";

const dir = (name: string): string => fileURLToPath(new URL(`./fixtures/pki/${name}`, import.meta.url));
const studio = JSON.parse(readFileSync(dir("pki-studio.json"), "utf8")) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
const tabs = JSON.parse(readFileSync(dir("pki-tabs.json"), "utf8")) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
const newest = trustOf(studio.run["inspect.trust"].newest)!;

const row: VehicleRow = {
  device: "ee/1",
  node: 1,
  provider: "p",
  t: 0,
  enrolled: true,
  provisioned: true,
  active: "aa",
  current_i: 5,
  jmax: 20,
  pool: { active: 1, valid: 19 },
  usable: 20,
  periods_ahead: 1,
  pseudonym_changes: 0,
  used: 1,
  revoked: false,
  security: { t: 99e9, next_topup: 200e9 },
};
const ctx: CredentialContext = { nowNs: 100e9, offCtl: new Set(), crlOverdue: new Set(), groundTruth: true };

describe("a vehicle's state", () => {
  it("is 'left the run' once its security record stops updating, not starving or between", () => {
    const gone = { ...row, active: null, security: { t: 100e9 - LEFT_RUN_NS - 1, next_topup: 10e9 } };
    expect(leftRun(gone, ctx.nowNs)).toBe(true);
    expect(credentialState(gone, ctx)).toBe("left");
    // A live vehicle a second behind the snapshot is still in the run.
    expect(credentialState(row, ctx)).toBe("healthy");
    // Revocation still wins: a revoked vehicle that left is reported revoked.
    expect(credentialState({ ...gone, revoked: true }, ctx)).toBe("revoked");
  });

  it("orders every state for triage, and every state has a label, a shape and a meaning", () => {
    expect([...CREDENTIAL_ORDER].sort()).toEqual(Object.keys(CREDENTIAL_LABEL).sort());
    expect(CREDENTIAL_ORDER[0]).toBe("revoked");
    expect(CREDENTIAL_ORDER.at(-1)).toBe("left");
    for (const s of CREDENTIAL_ORDER) expect(VEHICLE_STATE[s]?.meaning.length, s).toBeGreaterThan(20);
    expect(VEHICLE_STATE["ec-bad"]?.shape).toBe("hourglass");
  });

  it("names devices and components one way everywhere", () => {
    expect(deviceName("ee/1000039")).toBe("Vehicle 1000039");
    expect(componentName("provider-b/aca/1", true)).toBe("Pseudonym CA (ACA) 2, provider-b");
    expect(componentName("scms-manager/elector/0")).toBe("Elector 1");
  });
});

describe("the revocation schedule", () => {
  it("says the run's schedule, the provider's rule it replaces and the deadline in one sentence", () => {
    const s = crlScheduleText({ cadenceS: 30, publication: "sla-1-business-day", dueS: 6 * 86_400 + 23 * 3600 });
    expect(s).toContain("every 30 s");
    expect(s).toContain("1 business day");
    expect(s).toContain("within 6 d 23 h");
    expect(crlScheduleText({ cadenceS: null, publication: "on-decision", dueS: -120 })).toBe(
      "A revocation is published as soon as it is decided. The newest list's deadline for a replacement passed 2 min ago (overdue).",
    );
  });
});

describe("why a certificate fails", () => {
  const trace = (rule: string): { rule: string; status: string; detail: string[] }[] => [
    { rule: "chain-to-root", status: "pass", detail: [] },
    { rule, status: "fail", detail: ["cert 0: not valid at the policy time"] },
  ];
  it("puts the time check in terms of its window, a zero lifetime included", () => {
    expect(whySentence({ trace: trace("validity"), time32: 1000 }, { start: 760, end: 760, stated: "0 hours" })).toBe(
      "It was issued with a lifetime of zero, so it was never valid: every receiver's time check fails (its window closed 4 min ago).",
    );
    expect(whySentence({ trace: trace("validity"), time32: 1000 }, { start: 0, end: 940, stated: "15 min" })).toBe("Its validity ended 1 min ago; it lasted 15 min 40 s.");
    expect(whySentence({ trace: [], time32: 1000 }, null)).toBeNull();
  });
  it("says an id the run did not keep in words, not as an RPC error", () => {
    expect(certErrorText("inspect.cert failed: unknown certificate id 886c (1000 certificates recorded) (-32006)")).toMatch(/^This certificate was not kept/);
  });
});

describe("a case's story", () => {
  const cases = asCases(tabs.cases["ground-truth"])!.cases;
  it("is four beats, the same words on the list and the card", () => {
    expect(cases.length).toBeGreaterThan(0);
    for (const c of cases) {
      const beats = storyBeats(c);
      expect(beats.map((b) => b.key)).toEqual(["reports-in", "decision", "crl-published", "enforced"]);
      expect(beats.map((b) => b.title)).toContain(beatNow(c));
      // A beat is done only once its MA stage is recorded, and the times only grow.
      const reached = new Set(c.stages.map((s) => s.stage));
      expect(beats[2].done).toBe(reached.has("crl-published"));
      const at = beats.filter((b) => b.at !== null).map((b) => b.at as number);
      expect([...at].sort((x, y) => x - y)).toEqual(at);
    }
  });
  it("puts evidence and what each party learns in words, without the spec's tuple", () => {
    expect(evidencePlain({ type_name: "1VInconsistent", subtypes: [{ name: "ConstantPosition" }], verified: false, shared: true })).toBe(
      "one sender's messages contradict each other: same position while moving; the MA could NOT verify it; shared by the other provider's MA",
    );
    for (const what of Object.values(STEP_LEARNS)) expect(what).not.toMatch(/\b(LCI|iRev|la1Id|ls1)\b/);
  });
});

describe("the topology's links", () => {
  for (const width of [1240, 1580]) {
    it(`never run a cross-provider link through another box at ${width} px`, () => {
      const t = layoutTopology(newest.state, width, messagesOf(newest));
      const byId = new Map(t.nodes.map((n) => [n.id, n]));
      const providers = new Set(newest.state.providers.map((p) => p.label));
      const cross = t.links.filter((l) => {
        const a = byId.get(l.a)!;
        const b = byId.get(l.b)!;
        return providers.has(a.provider) && providers.has(b.provider) && a.provider !== b.provider && a.kind !== "devices" && b.kind !== "devices";
      });
      // A synthetic one when the fixture's window has none: the two MAs sharing a report.
      const pairs = cross.length > 0 ? cross.map((l) => [byId.get(l.a)!, byId.get(l.b)!]) : [[byId.get("provider-a/ma/0")!, byId.get("provider-b/ma/0")!]];
      for (const [a, b] of pairs) {
        const d = edgePath(t, a, b);
        expect(d, d).toMatch(/^M [\d.]+ [\d.]+ H [\d.]+ V [\d.]+ H [\d.]+ V [\d.]+ H [\d.]+$/);
        // Walk the segments: none enters a box other than its two ends.
        const nums = d.match(/-?[\d.]+/g)!.map(Number);
        const pts: { x: number; y: number }[] = [{ x: nums[0], y: nums[1] }];
        const cmds = d.match(/[HV]/g)!;
        cmds.forEach((c, k) => {
          const v = nums[2 + k];
          const p = pts[pts.length - 1];
          pts.push(c === "H" ? { x: v, y: p.y } : { x: p.x, y: v });
        });
        for (const n of t.nodes) {
          if (n === a || n === b || n.kind === "devices") continue;
          for (let k = 1; k < pts.length; k++) {
            const [p, q] = [pts[k - 1], pts[k]];
            const inX = Math.max(p.x, q.x) > n.x + 2 && Math.min(p.x, q.x) < n.x + n.w - 2;
            const inY = Math.max(p.y, q.y) > n.y + 2 && Math.min(p.y, q.y) < n.y + n.h - 2;
            expect(inX && inY, `${a.id}→${b.id} crosses ${n.id}`).toBe(false);
          }
        }
      }
    });
  }
});

describe("the messages", () => {
  it("label an arrow by its one SPDU, else by the step and the count, and fit it", () => {
    expect(arrowLabel({ name: "cert-request", pdus: [{ type: "EeRaCertRequestSpdu" }] })).toBe("EeRaCertRequestSpdu");
    expect(arrowLabel({ name: "report", pdus: [{ type: "SecuredMisbehaviorReport" }, { type: "EeRaDownloadRequestSpdu" }] })).toBe("report · 2 SPDUs");
    expect(wordsOf("report_sent")).toBe("report sent");
    expect(fitLabel("SecuredMisbehaviorReport", 10)).toBe("SecuredMi…");
  });
  it("group a burst of identical messages into one row", () => {
    const m = (id: string, t: number): TrustMessage => ({ id, t, from: "a/ra/0", to: "ee/1", name: "cert-response", flow: "provisioning", instance: "i", provider: "a", bytes: 100 });
    const g = groupMessages([m("1", 0), m("2", 1e8), m("3", 2e8), { ...m("4", 3e8), name: "other" }]);
    expect(g.map((x) => x.count)).toEqual([3, 1]);
    expect(g[0].bytes).toBe(300);
  });
  it("warn of a queue only when it keeps growing", () => {
    expect(growing([0, 3, 3, 3])).toBe(false);
    expect(growing([1, 2, 3, 4])).toBe(true);
  });
});

describe("the glossary", () => {
  it("has an entry for every term a certificate view marks", () => {
    const root = fileURLToPath(new URL("../src/components/", import.meta.url));
    const files: string[] = [];
    const walk = (d: string): void => {
      for (const f of readdirSync(d)) {
        const p = join(d, f);
        if (statSync(p).isDirectory()) walk(p);
        else if (p.endsWith(".tsx")) files.push(p);
      }
    };
    walk(root);
    const keys = files.flatMap((f) => [...readFileSync(f, "utf8").matchAll(/<Term k="([^"]+)"/g)].map((x) => x[1]));
    expect(keys.length).toBeGreaterThan(10);
    for (const k of keys) expect(glossary(k), k).not.toBeNull();
    for (const [k, e] of Object.entries(GLOSSARY)) expect(e.means.length, k).toBeGreaterThan(30);
  });
});
