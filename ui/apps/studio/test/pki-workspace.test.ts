/**
 * The SCMS workspace redesign on the engine's fixtures: the topology (`lib/pkiTopology.ts`), the
 * case step diagram (`lib/pkiCases.ts`) and the vehicle lifecycle (`lib/pkiVehicle.ts`).
 *
 * `pki-studio.json` is what `v2xw-server` answers the Studio (its `inspect.trust` carries the
 * window's messages), `pki-tabs.json` what it answers `inspect.pki`, `pki-run.jsonl` the records
 * the engine published (`crates/v2xw-server/tests/pki.rs` checks the first two against the third).
 */

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, expect, it } from "vitest";

import { trustOf, type TrustAnswer } from "../src/lib/pki.js";
import { caseSteps } from "../src/lib/pkiCases.js";
import { BOARD_COLUMNS, asCases, asVehicle, boardColumn, enforcement } from "../src/lib/pkiRecords.js";
import {
  PROVIDER_ROLES,
  healthOf,
  layoutTopology,
  linkKey,
  linkLoad,
  messagePath,
  messagesOf,
  nodeOf,
  observedBy,
  roleInfo,
  type TopoNode,
} from "../src/lib/pkiTopology.js";
import { lifecycleOf } from "../src/lib/pkiVehicle.js";

const dir = (name: string): string => fileURLToPath(new URL(`./fixtures/pki/${name}`, import.meta.url));
interface Row { channel: string; t: number; record: Record<string, any> } // eslint-disable-line @typescript-eslint/no-explicit-any
const rows: Row[] = readFileSync(dir("pki-run.jsonl"), "utf8").trim().split("\n").map((l) => JSON.parse(l) as Row);
const studio = JSON.parse(readFileSync(dir("pki-studio.json"), "utf8")) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
const tabs = JSON.parse(readFileSync(dir("pki-tabs.json"), "utf8")) as Record<string, any>; // eslint-disable-line @typescript-eslint/no-explicit-any
const trusts: TrustAnswer[] = Object.values(studio.run["inspect.trust"] as Record<string, unknown>).map((a) => trustOf(a)!);
const newest = trustOf(studio.run["inspect.trust"].newest)!;

const overlap = (a: TopoNode, b: TopoNode): boolean => a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;

describe("the topology", () => {
  for (const width of [1400, 1000, 600]) {
    it(`places every component once, a ghost for each role not run and a client group per provider, without overlap at ${width} px`, () => {
      const t = layoutTopology(newest.state, width, messagesOf(newest));
      const comps = t.nodes.filter((n) => n.kind === "component");
      const all = [...newest.state.governance, ...newest.state.providers.flatMap((p) => p.components)];
      expect(comps.map((n) => n.id).sort()).toEqual(all.map((c) => c.id).sort());
      for (const p of newest.state.providers) {
        const roles = new Set(p.components.map((c) => c.role));
        const ghosts = t.nodes.filter((n) => n.kind === "ghost" && n.provider === p.label).map((n) => n.role);
        expect(ghosts.sort()).toEqual(PROVIDER_ROLES.filter((r) => !roles.has(r)).sort());
        expect(t.nodes.filter((n) => n.kind === "devices" && n.provider === p.label)).toHaveLength(1);
      }
      for (let i = 0; i < t.nodes.length; i++)
        for (let j = i + 1; j < t.nodes.length; j++) expect(overlap(t.nodes[i], t.nodes[j]), `${t.nodes[i].id} / ${t.nodes[j].id}`).toBe(false);
      for (const n of t.nodes) {
        expect(n.x + n.w).toBeLessThanOrEqual(t.width + 0.01);
        expect(n.y + n.h).toBeLessThanOrEqual(t.height + 0.01);
      }
      // Regions sit side by side only when they fit.
      expect(t.sideBySide).toBe(width >= 1400);
    });
  }

  it("draws a ghost, in the role's place, for a role a provider does not run", () => {
    const p = newest.state.providers[1];
    const dcm = p.components.find((c) => c.role === "dcm")!;
    const state = { ...newest.state, providers: newest.state.providers.map((x) => (x === p ? { ...x, components: x.components.filter((c) => c !== dcm) } : x)) };
    const full = layoutTopology(newest.state, 1000).nodes.find((n) => n.id === dcm.id)!;
    const ghost = layoutTopology(state, 1000).nodes.find((n) => n.kind === "ghost" && n.provider === p.label && n.role === "dcm");
    expect(ghost).toBeDefined();
    expect([ghost!.x, ghost!.y]).toEqual([full.x, full.y]);
    // And no link runs to a role that is not there.
    expect(layoutTopology(state, 1000).links.some((l) => l.a === ghost!.id || l.b === ghost!.id)).toBe(false);
  });

  it("draws a link for every message of every window, and each message has a path", () => {
    let messages = 0;
    for (const a of trusts) {
      const ms = messagesOf(a);
      const t = layoutTopology(a.state, 1400, ms);
      const keys = new Set(t.links.map((l) => l.key));
      const load = linkLoad(ms);
      for (const m of ms) {
        const k = linkKey(nodeOf(m.from, m.to, m.provider)!, nodeOf(m.to, m.from, m.provider)!);
        expect(keys.has(k), `${m.id} ${m.from} → ${m.to}`).toBe(true);
        expect(messagePath(t, m), m.id).not.toBeNull();
      }
      expect([...load.values()].reduce((x, y) => x + y, 0)).toBe(ms.length);
      messages += ms.length;
    }
    expect(messages).toBeGreaterThan(20);
  });

  it("lists the same messages the run recorded, in the window before each snapshot", () => {
    const states = rows.filter((r) => r.channel === "pki.state").map((r) => Number(r.record.t));
    const recorded = rows.filter((r) => r.channel === "pki.flow" && r.record.kind === "msg").map((r) => r.record);
    states.forEach((t, i) => {
      const lo = i === 0 ? t - 2e9 : states[i - 1];
      const want = recorded.filter((m) => m.t > lo && m.t <= t).map((m) => m.id).sort();
      const got = messagesOf(trustOf(studio.run["inspect.trust"][String(t)])).map((m) => m.id).sort();
      expect(got, `at ${t}`).toEqual(want);
    });
  });

  it("an entity's view observes only what it sent or received", () => {
    const ms = trusts.flatMap((a) => messagesOf(a));
    const view = "provider-a/ra/0";
    const seen = ms.filter((m) => observedBy(m, view));
    expect(seen.length).toBeGreaterThan(0);
    expect(seen.length).toBeLessThan(ms.length);
    for (const m of seen) expect(m.from === view || m.to === view).toBe(true);
  });

  it("names every role the engine runs in plain words with its standard term", () => {
    for (const c of [...newest.state.governance, ...newest.state.providers.flatMap((p) => p.components)]) {
      const r = roleInfo(c.role);
      expect(r.does, c.role).not.toBe("A component the workspace has no description for.");
      expect(r.term.length).toBeGreaterThan(2);
    }
  });
});

describe("the health strip", () => {
  it("adds the queues, the cases still open and the newest list's age", () => {
    const vehicles = tabs.vehicles.vehicles as { usable: number; security?: { topup_in_flight?: boolean } }[];
    const cases = asCases(tabs.cases["ground-truth"])!.cases;
    const h = healthOf(newest, vehicles, cases);
    const queued = [...newest.state.governance, ...newest.state.providers.flatMap((p) => p.components)].reduce((n, c) => n + (c.queue?.depth ?? 0), 0);
    expect(h.pending).toBe(queued + vehicles.filter((v) => v.security?.topup_in_flight === true).length);
    expect(h.certificates).toBe(vehicles.reduce((n, v) => n + v.usable, 0));
    expect(h.openCases).toBe(cases.filter((c) => boardColumn(c) !== "enforced" && boardColumn(c) !== "refused").length);
    const issued = newest.state.crls.map((c) => c.issued_at);
    expect(issued.length).toBeGreaterThan(0);
    expect(h.crlAgeS).not.toBeNull();
    expect(h.crlAgeS!).toBeGreaterThanOrEqual((newest.t_ns - Math.max(...issued)) / 1e9 - 1e-9);
    expect(healthOf(newest, null, null).certificates).toBeNull();
  });
});

describe("the case step diagram", () => {
  const cases = asCases(tabs.cases["ground-truth"])!.cases;
  it("reaches exactly the stages pki.case recorded, in board order, ending at the board column", () => {
    expect(cases.length).toBeGreaterThan(0);
    for (const c of cases) {
      const steps = caseSteps(c);
      const recorded = new Set(rows.filter((r) => r.channel === "pki.case" && r.record.case === c.case).map((r) => String(r.record.stage)));
      expect(new Set(steps.filter((s) => s.reached).map((s) => s.stage)), c.case).toEqual(recorded);
      expect(steps.slice(0, BOARD_COLUMNS.length).map((s) => s.stage)).toEqual(BOARD_COLUMNS.map((b) => b.stage));
      const furthest = [...steps].reverse().find((s) => s.reached && s.stage !== "refused")?.stage;
      const col = boardColumn(c);
      if (col !== "refused") expect(furthest).toBe(col);
      // Waits run forward.
      for (const s of steps) if (s.waitNs !== null) expect(s.waitNs).toBeGreaterThanOrEqual(0);
    }
  });

  // Where each investigation message belongs (1609.2.1 §6.3.5 / CAMP: lookup and blocklist at the ACA
  // and RA, seeds and the same-device check at the LAs, the list at the CRL generator), written out
  // here rather than read from the code under test.
  const BELONGS: Record<string, string> = {
    "pca-lookup-request": "plv-lookup",
    "pca-lookup-response": "plv-lookup",
    "blocklist-request": "plv-lookup",
    "blocklist-response": "plv-lookup",
    "blocklist-notice": "plv-lookup",
    "same-device-request": "la-seeds",
    "same-device-response": "la-seeds",
    "seed-request": "la-seeds",
    "seed-response": "la-seeds",
    "crl-append": "crl-published",
    "crl-publish": "crl-published",
    "crl-broadcast": "crl-published",
  };
  it("places every investigation message under one stage, by name where the table names it", () => {
    let named = 0;
    for (const c of cases) {
      const steps = caseSteps(c);
      const placed = steps.flatMap((s) => s.messages.map((m) => [s.stage, m] as const));
      expect(placed.map(([, m]) => m.id).sort()).toEqual(c.steps.filter((m) => m.kind === "msg").map((m) => m.id).sort());
      for (const [stage, m] of placed) {
        if (BELONGS[m.name]) {
          expect(stage, m.name).toBe(BELONGS[m.name]);
          named++;
        }
      }
      const enf = enforcement(c);
      if (enf) expect(steps.find((s) => s.stage === "enforced")!.detail).toContain(`installed by ${enf.installed}`);
    }
    expect(named).toBeGreaterThan(5);
  });

  it("an LA's view of a case says what LA1 learned and names no device", () => {
    const as = asCases(tabs.cases["provider-a/la1/0"])!.cases;
    expect(as.length).toBeGreaterThan(0);
    const text = JSON.stringify(as.map((c) => caseSteps(c)));
    expect(text).toContain("LA1");
    expect(text).not.toContain("ee/");
  });
});

describe("the vehicle lifecycle", () => {
  it("has one pseudonym span per pseudonym and counts the grid's held certificates per week", () => {
    const devices = Object.keys(tabs.vehicle as Record<string, unknown>);
    expect(devices.length).toBeGreaterThan(2);
    for (const dev of devices) {
      const d = asVehicle(tabs.vehicle[dev])!;
      const life = lifecycleOf(d, [], d.t);
      expect(life.events.filter((e) => e.lane === "pseudonym").map((e) => e.label)).toEqual([...d.pseudonyms].sort((a, b) => a.since - b.since).map((p) => p.id8));
      const held = d.grid.held.filter((c) => c.state !== "not-yet-available").length;
      expect(life.batches.reduce((n, b) => n + b.certs, 0)).toBe(held);
      for (const b of life.batches) expect(b.certs).toBeLessThanOrEqual(d.jmax);
      expect(life.t1).toBeGreaterThan(life.t0);
      if (d.revoked) expect(life.events.some((e) => e.lane === "revocation")).toBe(true);
    }
    // A cell not yet available is not a held certificate.
    const d = asVehicle(tabs.vehicle[devices[0]])!;
    const extra = { ...d, grid: { ...d.grid, held: [...d.grid.held, { i: d.grid.i_max, j: 0, state: "not-yet-available" }] } };
    expect(lifecycleOf(extra, [], d.t).batches).toEqual(lifecycleOf(d, [], d.t).batches);
  });

  it("puts the factory steps apart and calls later downloads top-ups", () => {
    const d = asVehicle(tabs.vehicle[Object.keys(tabs.vehicle)[0]])!;
    const flow = (flowName: string, t: number): Parameters<typeof lifecycleOf>[1][number] => ({
      instance: `p/${flowName}/${t}`,
      provider: "p",
      flow: flowName,
      first_t: t,
      last_t: t + 1,
      messages: 1,
      stages: 0,
      bytes: 0,
      pdus: 0,
      devices: [d.device],
      device_count: 1,
      last_stage: null,
    });
    const life = lifecycleOf(d, [flow("preload", 0), flow("provisioning", 5e9), flow("provisioning", 9e9), flow("report", 7e9)], d.t);
    expect(life.beforeRun).toHaveLength(1);
    expect(life.events.filter((e) => e.lane === "download").map((e) => e.label)).toEqual(["top-up", "top-up"]);
    expect(life.events.filter((e) => e.lane === "report")).toHaveLength(1);
  });
});
