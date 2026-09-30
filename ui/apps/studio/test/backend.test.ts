import { describe, expect, it } from "vitest";

import {
  columnHeads,
  edgeWidth,
  headlineCounts,
  isLive,
  layout,
  snapshotOf,
  systemTitle,
  type BackendEntity,
} from "../src/lib/backend.js";

function entity(id: string, tier: string, state: Record<string, unknown> = {}): BackendEntity {
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
    state,
  };
}

describe("backend snapshot", () => {
  it("reads the state out of an inspect.entity answer and refuses anything else", () => {
    const answer = {
      entity: "backend",
      t_ns: 3e9,
      role: "scms",
      state: { system: "scms", protocol: "protocol/scms/camp", t: 3e9, entities: [entity("ra", "ra")], edges: [], recent: [], flows: { "provision-batch": 2 } },
    };
    const snap = snapshotOf(answer);
    expect(snap?.system).toBe("scms");
    expect(snap?.entities.map((e) => e.id)).toEqual(["ra"]);
    expect(snap?.flows["provision-batch"]).toBe(2);
    expect(snapshotOf({ entity: "ra", state: { issued: 3 } })).toBeNull();
    expect(snapshotOf(null)).toBeNull();
  });

  it("lays out one column per tier that has an entity, in the design's order", () => {
    const entities = [entity("ee", "device"), entity("ra", "ra"), entity("root", "governance"), entity("pca", "ca"), entity("x", "no-such-tier")];
    const { boxes, width } = layout(entities);
    const col = (id: string): number => boxes.find((b) => b.id === id)?.column ?? -1;
    expect(col("root")).toBe(0);
    expect(col("pca")).toBe(1);
    expect(col("ra")).toBe(2);
    // An unknown tier is drawn with the devices rather than dropped.
    expect(col("ee")).toBe(3);
    expect(col("x")).toBe(3);
    expect(boxes.filter((b) => b.column === 3)[1].y).toBeGreaterThan(boxes.filter((b) => b.column === 3)[0].y);
    expect(columnHeads(entities).map((h) => h.label)).toEqual(["Governance", "Certificate authorities", "Registration and issuance", "Devices"]);
    expect(width).toBeGreaterThan(0);
  });

  it("shows what an entity has done before what it has not", () => {
    const e = entity("pca", "ca", { requests_waiting: 0, certs_issued: 120, batches: 6, name: "PCA" });
    expect(headlineCounts(e)).toEqual([
      ["certs issued", "120"],
      ["batches", "6"],
    ]);
    expect(headlineCounts(entity("la1", "privacy", { a: 0, b: 0, c: 0 }))).toEqual([
      ["a", "0"],
      ["b", "0"],
    ]);
  });

  it("draws no line for an unused pair and a live one only within its window", () => {
    expect(edgeWidth(0)).toBe(0);
    expect(edgeWidth(1)).toBe(1);
    expect(edgeWidth(1e9)).toBe(6);
    const edge = { from: "ra", to: "pca", messages: 3, bytes: 900, last_t: 1e9, last_step: "cert-request", transport: "backend-net", steps: {} };
    expect(isLive(edge, 2e9)).toBe(true);
    expect(isLive(edge, 4e9)).toBe(false);
    expect(isLive({ ...edge, messages: 0 }, 1e9)).toBe(false);
  });

  it("names both systems", () => {
    expect(systemTitle("scms")).toContain("SCMS");
    expect(systemTitle("ccms")).toContain("TS 102 941");
  });
});
