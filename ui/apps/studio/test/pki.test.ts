/**
 * WP-E2: the PKI view's model (`src/lib/pki.ts`) on the engine's own answers.
 *
 * The answers are `test/fixtures/pki/pki-studio.json`: what `v2xw-server` answers `inspect.trust`,
 * `inspect.flows`, `inspect.flow`, `inspect.pdu` and `inspect.knowledge` on the WP-E1 fixtures
 * (a two-provider Manhattan run, and a whole in-run sequence with enrolment, provisioning, reports,
 * linkage resolution and CRL issuance). `crates/v2xw-server/tests/pki.rs` writes it and fails when
 * it goes stale, so these tests read what a real engine sends.
 */
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

import {
  canInspect,
  componentClasses,
  flowDetailOf,
  flowsOf,
  GROUND_TRUTH,
  hexRows,
  jerLeaves,
  pduOf,
  providerLayout,
  sequenceOf,
  stagesOf,
  time32Date,
  treeLines,
  trustOf,
  viewOptions,
  type FlowDetail,
  type TrustAnswer,
} from "../src/lib/pki.js";

const here = dirname(fileURLToPath(import.meta.url));
type Bundle = Record<string, Record<string, unknown>>;
const studio = JSON.parse(readFileSync(join(here, "fixtures/pki/pki-studio.json"), "utf8")) as { run: Bundle; sequence: Bundle };

function trust(which: "run" | "sequence", key = "newest"): TrustAnswer {
  const t = trustOf((studio[which]["inspect.trust"] as Record<string, unknown>)[key]);
  if (!t) throw new Error(`no trust answer ${which}/${key}`);
  return t;
}
function flow(which: "run" | "sequence", instance: string): FlowDetail {
  const d = flowDetailOf((studio[which]["inspect.flow"] as Record<string, unknown>)[instance]);
  if (!d) throw new Error(`no flow ${instance}`);
  return d;
}

describe("Trust domain", () => {
  it("reads every snapshot, with providers, components, region, CTL and electors", () => {
    const answers = studio.run["inspect.trust"] as Record<string, unknown>;
    expect(Object.keys(answers).length).toBeGreaterThan(3);
    for (const a of Object.values(answers)) expect(trustOf(a)).not.toBeNull();
    const t = trust("run");
    expect(t.state.providers.map((p) => p.label)).toEqual(["provider-a", "provider-b"]);
    for (const p of t.state.providers) {
      expect(p.region.assumption).toBe(true);
      expect(p.on_ctl).toBe(true);
      expect(p.components.length).toBeGreaterThan(10);
    }
    expect(t.state.ctl.roots).toHaveLength(2);
    expect(t.state.ctl.history.length).toBeGreaterThan(0);
    expect(t.state.governance.filter((c) => c.role === "elector")).toHaveLength(t.state.ctl.electors);
    expect(trustOf({ state: {} })).toBeNull();
  });

  it("puts the offline chain first and styles it offline; the rest online in request order", () => {
    const t = trust("run");
    for (const p of t.state.providers) {
      const { offline, online } = providerLayout(p);
      expect(offline.map((c) => c.role)).toEqual(["root", "ica"]);
      for (const c of offline) expect(componentClasses(c, t.activity)).toContain("status-offline");
      for (const c of online) expect(componentClasses(c, t.activity)).not.toContain("status-offline");
      const roles = online.map((c) => c.role);
      expect(roles.indexOf("ra")).toBeLessThan(roles.indexOf("aca"));
      expect(roles.indexOf("la1")).toBeLessThan(roles.indexOf("la2"));
      expect(roles.indexOf("ma")).toBeLessThan(roles.indexOf("crlg"));
    }
    for (const e of t.state.governance.filter((c) => c.role === "elector")) expect(componentClasses(e, t.activity)).toContain("status-offline");
  });

  it("lights exactly the components the engine says carried traffic in the window", () => {
    let lit = 0;
    for (const [key, a] of Object.entries(studio.run["inspect.trust"] as Record<string, unknown>)) {
      const t = trustOf(a);
      if (!t || key === "newest") continue;
      for (const p of t.state.providers) {
        for (const c of p.components) {
          const busy = componentClasses(c, t.activity).includes("busy");
          expect(busy, `${c.id} at ${key}`).toBe((t.activity[c.id]?.messages ?? 0) > 0);
          lit += busy ? 1 : 0;
        }
      }
    }
    expect(lit).toBeGreaterThan(10);
  });

  it("offers ground truth and every knowledge-bearing entity of every provider as views", () => {
    const opts = viewOptions(trust("run").state);
    expect(opts[0]).toEqual({ id: GROUND_TRUTH, label: "ground truth", provider: null });
    const ids = opts.slice(1).map((o) => o.id);
    for (const p of ["provider-a", "provider-b"]) for (const r of ["ra", "aca", "la1", "la2", "ma", "eca"]) expect(ids).toContain(`${p}/${r}/0`);
    expect(opts.find((o) => o.id === "provider-b/la1/0")?.label).toBe("as LA1 (b)");
    // One provider: no provider suffix.
    expect(viewOptions(trust("sequence").state).find((o) => o.id === "provider-a/ra/0")?.label).toBe("as RA");
  });
});

describe("Flows", () => {
  it("lists every instance with per-flow counts", () => {
    const all = flowsOf((studio.sequence["inspect.flows"] as Record<string, unknown>)[""]);
    expect(all?.total).toBe(8);
    expect(all?.flows).toMatchObject({ enrolment: 2, provisioning: 2, report: 2, "linkage-resolution": 1, "crl-issuance": 1 });
    const prov = flowsOf((studio.sequence["inspect.flows"] as Record<string, unknown>).provisioning);
    expect(prov?.instances.map((i) => i.flow)).toEqual(["provisioning", "provisioning"]);
  });

  it("draws a provisioning run as lanes EE → LOP → RA → LA1, LA2 → ACA with a hatched shuffle wait", () => {
    const d = flow("sequence", "provider-a/provisioning/2");
    const { lanes, rows } = sequenceOf(d);
    expect(lanes.map((l) => l.id)).toEqual(["ee/1000", "provider-a/lop/0", "provider-a/ra/0", "provider-a/la1/0", "provider-a/la2/0", "provider-a/aca/0"]);
    const arrows = rows.filter((r) => r.kind === "msg");
    expect(arrows).toHaveLength(d.messages);
    // In the engine's time order.
    const ts = arrows.map((r) => r.record.t);
    expect([...ts].sort((a, b) => a - b)).toEqual(ts);
    // The first arrow: the device's encrypted request, EE lane → LOP lane, carrying the SPDU.
    const first = arrows[0];
    expect(first?.kind === "msg" && [first.from, first.to]).toEqual([0, 1]);
    expect(first?.kind === "msg" && first.record.pdus?.map((p) => p.type)).toEqual(["EeRaCertRequestSpdu"]);
    // Exactly one hatched gap, closed by the `shuffled` stamp, as long as the engine's wait.
    const gaps = rows.filter((r) => r.kind === "gap");
    const shuffles = gaps.filter((g) => g.kind === "gap" && g.shuffle);
    expect(shuffles).toHaveLength(1);
    const at = rows.findIndex((r) => r.kind === "gap" && r.shuffle);
    const after = rows[at + 1];
    expect(after?.kind === "stage" && after.record.name).toBe("shuffled");
    const stages = stagesOf(d);
    const before = stages[stages.findIndex((s) => s.name === "shuffled") - 1];
    const shuffled = stages.find((s) => s.name === "shuffled");
    expect(before?.name).toBe("pre_linkage_ready");
    expect(shuffles[0]?.kind === "gap" && shuffles[0].ns).toBe((shuffled?.t ?? 0) - (before?.t ?? 0));
    expect(shuffles[0]?.kind === "gap" && shuffles[0].ns).toBeGreaterThan(50e9);
    // Every stage carries its own timestamp.
    expect(stages.map((s) => s.name)).toEqual(["requested", "proxy_forwarded", "acknowledged", "expanded", "pre_linkage_ready", "shuffled", "certified", "batch_ready", "downloaded", "installed"]);
  });

  it("an entity sees only what it sent or received", () => {
    const d = flow("sequence", "provider-a/provisioning/2");
    const seen = (view: string): string[] =>
      sequenceOf(d, view)
        .rows.filter((r) => r.kind === "msg" && r.observed)
        .map((r) => (r.kind === "msg" ? r.record.name : ""));
    expect(seen("provider-a/aca/0")).toEqual(["cert-request", "cert-response"]);
    expect(seen("provider-a/la1/0")).toEqual(["pre-linkage-request", "pre-linkage-response"]);
    expect(seen(GROUND_TRUTH)).toHaveLength(d.messages);
    // The RA never sees the LOP's leg to the device.
    expect(seen("provider-a/ra/0")).not.toContain("provisioning-request");
    // And it may not open a PDU on a message it did not handle.
    const req = d.records.find((r) => r.name === "provisioning-request-proxied");
    const pdu = req?.pdus?.[0];
    expect(pdu && req && canInspect(pdu, req, "provider-a/ra/0")).toBe(true);
    expect(pdu && req && canInspect(pdu, req, "provider-a/aca/0")).toBe(false);
  });

  it("marks the report shuffle and the MA's work in a misbehaviour flow", () => {
    const rep = sequenceOf(flow("sequence", "provider-a/report/5"));
    expect(rep.rows.some((r) => r.kind === "gap" && r.shuffle)).toBe(true);
    const lr = sequenceOf(flow("sequence", "provider-a/linkage-resolution/7"));
    expect(lr.lanes.map((l) => l.id)).toEqual(["provider-a/eca/0", "provider-a/ra/0", "provider-a/la1/0", "provider-a/la2/0", "provider-a/aca/0", "provider-a/ma/0"]);
  });
});

/** Every leaf of a JSON value with its path, computed independently of `treeLines`. */
function leaves(v: unknown, path: string, out: [string, string][]): [string, string][] {
  if (Array.isArray(v)) v.forEach((x, i) => leaves(x, `${path}[${i}]`, out));
  else if (typeof v === "object" && v !== null) for (const [k, x] of Object.entries(v)) leaves(x, `${path}.${k}`, out);
  else out.push([path, JSON.stringify(v ?? null)]);
  return out;
}

describe("PDU decoder", () => {
  const pdus = studio.sequence["inspect.pdu"] as Record<string, Record<string, unknown>>;

  it("draws every JER leaf of every layer once, in order, and nothing else", () => {
    let layersSeen = 0;
    let leavesSeen = 0;
    for (const [id, views] of Object.entries(pdus)) {
      const p = pduOf(views[GROUND_TRUTH]);
      expect(p, id).not.toBeNull();
      if (!p) continue;
      const all = [...p.layers, ...(p.opened && "layers" in p.opened ? p.opened.layers : [])];
      for (const l of all) {
        if (l.jer === undefined) continue;
        layersSeen++;
        const want = leaves(l.jer, "$", []);
        const drawn = treeLines(l.jer).filter((x) => x.value !== null);
        expect(drawn.map((x) => [x.path, x.value]), `${id} ${l.path}`).toEqual(want);
        expect(jerLeaves(l.jer).size).toBe(want.length);
        leavesSeen += want.length;
        // The hex beside the tree is the layer's bytes, 16 to a row.
        expect(hexRows(l.hex).map((r) => r.bytes.replace(/ /g, "")).join("")).toBe(l.hex.toLowerCase());
        expect(l.len * 2).toBe(l.hex.length);
      }
    }
    expect(layersSeen).toBeGreaterThan(40);
    expect(leavesSeen).toBeGreaterThan(1000);
  });

  it("shows plaintext in ground truth and to its opener only", () => {
    let sealed = 0;
    for (const [id, views] of Object.entries(pdus)) {
      const gt = pduOf(views[GROUND_TRUTH]);
      if (!gt?.opened) continue;
      sealed++;
      expect("layers" in gt.opened, `${id} in ground truth`).toBe(true);
      const by = gt.opened.by;
      for (const [view, answer] of Object.entries(views)) {
        if (view === GROUND_TRUTH) continue;
        const p = pduOf(answer);
        const visible = p?.opened ? "layers" in p.opened : false;
        expect(visible, `${id} as ${view}`).toBe(view === by);
        // The ciphertext is the same bytes in every view.
        expect(p?.hex).toBe(gt.hex);
      }
    }
    expect(sealed).toBeGreaterThan(5);
  });
});

describe("formatting", () => {
  it("Time32 counts TAI seconds from 2004: the US calendar epoch 2015-01-06 09:00 UTC", () => {
    // 2015-01-06T09:00Z is Unix 1420534800; three leap seconds (2005-12, 2008-12, 2012-06) lie
    // between it and 2004-01-01, so its Time32 is 347619600 + 3.
    expect(time32Date(347_619_603)).toBe("2015-01-06 09:00 UTC");
    expect(time32Date(0)).toBe("2004-01-01 00:00 UTC");
  });
});
