/**
 * Every roadside unit the `Hello` node table lists is on the map (§3.1.3).
 *
 * A world file's sites are where a unit may be mounted; a scenario can also place a unit by
 * `position_m`, on no site. Such a unit was on no layer of the page — not drawn, not clickable, no
 * coverage ring, and the `rsu` camera refused for want of a mast (QA, 2026-09-24: the developer chip
 * said "0 RSUs" beside one). The viewer now reads the node table's kind-2 rows.
 */

import { describe, expect, it } from "vitest";
import type { HelloMessage } from "@vwp/protocol";
import { Viewer } from "../src/scene.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";
import { makeGridWorld } from "./support/fixture.js";

const CANVAS = { width: 640, height: 360, clientWidth: 640, clientHeight: 360 } as unknown as ViewerCanvas;

interface Row {
  nodeId: number;
  kind: number;
  x: number;
  y: number;
  z: number;
}

/** A Hello whose node table is `rows`, with the other fields `applyHello` reads. */
function hello(rows: Row[]): HelloMessage {
  return {
    actorCapacity: 64,
    mobilityStepNs: 100_000_000n,
    helloFlags: 0x01,
    runId: new Uint8Array(16),
    strings: [""],
    nodes: {
      count: rows.length,
      nodeId: Uint32Array.from(rows.map((r) => r.nodeId)),
      actorId: Uint32Array.from(rows.map(() => 0xffffffff)),
      kind: Uint8Array.from(rows.map((r) => r.kind)),
      posXM: Float32Array.from(rows.map((r) => r.x)),
      posYM: Float32Array.from(rows.map((r) => r.y)),
      posZM: Float32Array.from(rows.map((r) => r.z)),
    },
    classes: {
      count: 0,
      strName: new Uint32Array(0),
      lengthM: new Float32Array(0),
      widthM: new Float32Array(0),
      heightM: new Float32Array(0),
      colorRgba: new Uint32Array(0),
      category: new Uint8Array(0),
    },
  } as unknown as HelloMessage;
}

function viewer(): Viewer {
  return new Viewer({ canvas: CANVAS, theme: "dark", autoStart: false, createRenderer: (c) => new NullRenderer(c) });
}

describe("§3.1.3 — the node table's roadside units are on the map", () => {
  it("a unit placed by position is a mast of its own, whichever of the Hello and the world comes first", () => {
    const grid = makeGridWorld({ blocks: 4, blockM: 120, buildingsPerBlock: 0 });
    const worldSites = grid.world.sites.count;
    expect(worldSites).toBeGreaterThan(0);
    // Mid-block, far from every junction's site; a vehicle's radio (kind 0) is not a unit.
    const rows: Row[] = [
      { nodeId: 0, kind: 2, x: 30, y: -170, z: 6 },
      { nodeId: 7, kind: 0, x: 0, y: 0, z: 1.5 },
    ];

    // The Hello first, as the engine sends them.
    const a = viewer();
    a.applyHello(hello(rows));
    a.setWorld(grid.world);
    const w = a.worldRenderer;
    expect(w.siteCount, "the placed unit was not added to the sites").toBe(worldSites + 1);
    expect(w.siteNodeIds[worldSites]).toBe(0);
    expect(w.siteKinds[worldSites]).toBe(0);
    expect([...w.sitePositions.subarray(worldSites * 3, worldSites * 3 + 3)]).toEqual([30, -170, 6]);
    expect(w.sitesGroup.children.some((c) => c.name === "world/rsu-masts")).toBe(true);

    // The world first: the same result.
    const b = viewer();
    b.setWorld(grid.world);
    b.applyHello(hello(rows));
    expect(b.worldRenderer.siteCount).toBe(worldSites + 1);

    // Applied again (the Studio re-attaching the viewer), nothing is doubled.
    b.applyHello(hello(rows));
    expect(b.worldRenderer.siteCount).toBe(worldSites + 1);
    expect(b.worldRenderer.sitesGroup.children.filter((c) => c.name === "world/rsu-masts")).toHaveLength(1);
  });

  it("a unit on a world site is that site, and lends it its node id", () => {
    const grid = makeGridWorld({ blocks: 4, blockM: 120, buildingsPerBlock: 0 });
    const site = grid.world.sites.at(0);
    const v = viewer();
    v.setWorld(grid.world);
    // A world whose site carries no node: the unit on it gives it one, so a click selects the unit.
    v.worldRenderer.siteNodeIds[0] = 0xffffffff;
    v.applyHello(hello([{ nodeId: 3, kind: 2, x: site.xM + 0.5, y: site.yM, z: site.zM + 6 }]));
    expect(v.worldRenderer.siteCount).toBe(grid.world.sites.count);
    expect(v.worldRenderer.siteNodeIds[0]).toBe(3);
  });
});
