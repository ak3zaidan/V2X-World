/**
 * The pure pieces behind the viewport, the inspector, the chase HUD and the Backend view
 * (2026-09-30 redesign): the node's security row and its pseudonym line, the run's radio counts,
 * the roadside-unit count, the backend's glance numbers and the metrics each entity links to.
 *
 * Each case names the defect it holds shut.
 */

import { describe, expect, it } from "vitest";

import { activeEntities, glance, metricsFor, type BackendEdge, type BackendEntity, type BackendSnapshot } from "../src/lib/backend.js";
import { radioBreakdown, radioSentence } from "../src/lib/format.js";
import { linkText, pseudonymLine, rsuCount, securityOf, untilText } from "../src/lib/security.js";

describe("the followed node's security row", () => {
  // The live engine answers `certs` with the node.security row and `crl` with its three CRL members.
  const answer = {
    node: 12,
    certs: {
      pseudonym: "a1b2c3d4e5f60718293a4b5c6d7e8f90",
      temp_id: "a1b2c3d4",
      cert_i: 3,
      cert_j: 17,
      pool_valid: 20,
      pool_preloaded: 40,
      pool_stored: 60,
      topup_in_flight: false,
      next_topup: 90_000_000_000,
      link: "cellular",
      link_up: true,
      changes_log: [],
    },
    crl: { entries: 4, version: 2, self_revoked: false },
  };

  it("reads the row and folds the CRL section into it", () => {
    const row = securityOf(answer);
    expect(row?.cert_i).toBe(3);
    expect(row?.crl_entries).toBe(4);
    expect(row?.crl_version).toBe(2);
    expect(row?.self_revoked).toBe(false);
  });

  it("is null when the engine has no credential state for the node", () => {
    expect(securityOf({ node: 1 })).toBeNull();
    expect(securityOf(null)).toBeNull();
    // The specification types `certs` as a list of certificates; a list is not a row.
    expect(securityOf({ certs: [{ digest: "ab" }] })).toBeNull();
  });

  // QA 2026-09-24: the HUD said "(indices pending — node.tx)" for a whole SCMS run.
  it("names the certificate's indices from the row, not from the last node.tx digest", () => {
    const line = pseudonymLine(securityOf(answer), { digest: "ffff", i: null, j: null });
    expect(line?.digest).toBe("a1b2c3d4e5f60718293a4b5c6d7e8f90");
    expect(line?.indices).toBe("i 3 · j 17");
    expect(line?.tempId).toBe("a1b2c3d4");
  });

  it("promises no indices when nothing carries them", () => {
    const line = pseudonymLine(null, { digest: "ffff", i: null, j: null });
    expect(line?.digest).toBe("ffff");
    expect(line?.indices).toBeNull();
    expect(pseudonymLine(null, null)).toBeNull();
  });

  it("says the backend access and the time to an instant in words", () => {
    expect(linkText({ link: "cellular", link_up: true })).toBe("cellular, up");
    expect(linkText({ link: "rsu-relay", link_up: false })).toBe("via a roadside unit, out of reach");
    expect(linkText({})).toBeNull();
    expect(untilText(90_000_000_000, 80_000_000_000)).toBe("in 10.0 s");
    expect(untilText(10, 20)).toBe("expired");
    expect(untilText(null, 0)).toBeNull();
  });
});

describe("what the run has on the map", () => {
  // QA 2026-09-24: "radios 82" beside "106 vehicles or roadside units" — two sources under words
  // that made them the same thing.
  it("counts radios by what carries them", () => {
    const b = radioBreakdown([{ kind: 0 }, { kind: 0 }, { kind: 1 }, { kind: 2 }]);
    expect(b).toEqual({ total: 4, vehicles: 2, vru: 1, roadside: 1 });
    expect(radioSentence(b)).toBe("4 radios: 2 on vehicles, 1 carried by pedestrians or cyclists, 1 roadside unit");
    expect(radioSentence(radioBreakdown([{ kind: 0 }]))).toBe("1 radio");
  });

  // QA 2026-09-24: the chip said "0 RSUs" for a unit placed by position_m.
  it("counts the scenario's roadside units, placed by site or by position", () => {
    expect(rsuCount({ actors: { rsus: [{ position_m: [548, 280, 0] }] } })).toBe(1);
    expect(rsuCount({ actors: { rsus: [{ site: 0 }, { site: 3 }] } })).toBe(2);
    expect(rsuCount({ actors: {} })).toBe(0);
    expect(rsuCount(null)).toBeNull();
  });
});

function entity(id: string, tier: string, extra: Partial<BackendEntity> = {}): BackendEntity {
  return {
    id,
    name: id.toUpperCase(),
    system: "scms",
    tier,
    online: true,
    node: null,
    role: "",
    queue: null,
    traffic: { received: 0, sent: 0, bytes_in: 0, bytes_out: 0 },
    ops: {},
    state: {},
    ...extra,
  };
}

function edge(from: string, to: string, messages: number, bytes: number, lastT: number): BackendEdge {
  return { from, to, messages, bytes, last_t: lastT, last_step: "x", transport: "backend-net", steps: {} };
}

describe("the Backend view at a glance", () => {
  const snap: BackendSnapshot = {
    system: "scms",
    protocol: "protocol/scms/camp",
    signature: "ecdsa-p256",
    t: 10_000_000_000,
    entities: [
      entity("ra", "ra", { queue: { depth: 3, servers: 1, served: 9, busy_ns: 0, waited_ns: 0, backlog_ns: 0, inbound: 0 } }),
      entity("pca", "ca"),
      entity("root", "governance", { online: false }),
      entity("ee", "device"),
    ],
    edges: [edge("ee", "ra", 10, 1000, 9_500_000_000), edge("ra", "pca", 5, 500, 1_000_000_000)],
    recent: [],
    flows: {},
  };

  it("sums the traffic and the waiting requests, and counts what moved in the last two seconds", () => {
    const g = glance(snap);
    expect(g).toEqual({ entities: 4, activeEntities: 2, edges: 2, liveEdges: 1, messages: 15, bytes: 1500, queued: 3, offline: 1 });
    expect([...activeEntities(snap)].sort()).toEqual(["ee", "ra"]);
  });

  it("links each tier to the metrics that measure its work", () => {
    expect(metricsFor({ tier: "revocation", id: "ma" })).toContain("revocation_latency_stage");
    expect(metricsFor({ tier: "device", id: "ee" })).toContain("cert_pool_valid");
    expect(metricsFor({ tier: "distribution", id: "crl" })).toContain("crl_entries");
    expect(metricsFor(null).length).toBeGreaterThan(0);
  });
});
