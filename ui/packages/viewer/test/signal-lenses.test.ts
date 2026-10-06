/**
 * Signal lens shapes: a separate turn face has arrow lenses and shows the flashing yellow arrow for a
 * permissive turn (MUTCD 2009 §4D.06, §4D.18, §4D.20); a head over a through or shared lane, or a
 * turn lane on the through movement's own group, has round lenses; a pedestrian head has the hand
 * and the walking person (§4E.04).
 */

import { createHash } from "node:crypto";
import { describe, expect, it } from "vitest";
import { decodeWorld, encodeWorld, type SignalBlock, type WorldInit, type WorldLaneInit } from "@vwp/protocol";
import { WorldRenderer } from "../src/world-render.js";
import { LENS } from "../src/signals.js";
import { DARK_THEME } from "../src/theme.js";

const sha256 = (b: Uint8Array): Uint8Array => new Uint8Array(createHash("sha256").update(b).digest());

/**
 * One eastbound approach ending at x = 0: lane 0 (right) goes straight on, lane 1 (left) only turns
 * left; a westbound-facing right-turn-only lane 2 on the same edge's right; their connectors; a
 * crosswalk lane for a pedestrian head. `leftGroup` is the left lane's signal group (the through
 * lane's is 1).
 */
function approachWorld(leftGroup: number, rightGroup: number) {
  const W = 3.3;
  const lane = (laneId: number, laneType: number, indexInEdge: number, points: [number, number, number][], edgeId = 1): WorldLaneInit => ({
    laneId, edgeId, junctionId: laneType === 5 ? 7 : 0xffffffff, strName: 0, widthM: laneType === 6 ? 3 : W,
    speedLimitMps: 13.9, allowedClasses: 0b0100_1111, laneType, indexInEdge, points,
  });
  const lanes: WorldLaneInit[] = [
    lane(0, 0, 1, [[-80, -W * 1.5, 0], [0, -W * 1.5, 0]]),
    lane(1, 0, 2, [[-80, -W * 0.5, 0], [0, -W * 0.5, 0]]),
    lane(2, 0, 0, [[-80, -W * 2.5, 0], [0, -W * 2.5, 0]]),
    // Connectors: straight on from lane 0, left from lane 1, right from lane 2.
    lane(10, 5, 0, [[0, -W * 1.5, 0], [10, -W * 1.5, 0], [20, -W * 1.5, 0]], 100),
    lane(11, 5, 0, [[0, -W * 0.5, 0], [8, 2, 0], [12, 12, 0]], 101),
    lane(12, 5, 0, [[0, -W * 2.5, 0], [5, -9.5, 0], [7, -18, 0]], 102),
    // A crosswalk across the approach.
    lane(20, 6, 0, [[-4, -12, 0], [-4, 2, 0]], 200),
  ];
  const signals = [
    { signalId: 3, junctionId: 7, laneId: 0, xM: 0, yM: -W * 1.5, zM: 5.2, kind: 0, group: 1 },
    { signalId: 3, junctionId: 7, laneId: 1, xM: 0, yM: -W * 0.5, zM: 5.2, kind: 0, group: leftGroup },
    { signalId: 3, junctionId: 7, laneId: 2, xM: 0, yM: -W * 2.5, zM: 5.2, kind: 0, group: rightGroup },
    { signalId: 3, junctionId: 7, laneId: 20, xM: -4, yM: 2, zM: 2.6, kind: 1, group: 4 },
  ];
  const init: WorldInit = {
    originLatDeg: 40.75, originLonDeg: -73.98, originAltM: 10,
    bboxMinXM: -100, bboxMinYM: -100, bboxMaxXM: 100, bboxMaxYM: 100, bboxMinZM: -1, bboxMaxZM: 20,
    lanes, buildings: [], junctions: [{ junctionId: 7, strName: 0, xM: 5, yM: 0, zM: 0, control: 2, laneCount: 3 }],
    signals, sites: [], crossings: [], landuse: [], strings: [""],
    provenanceJson: JSON.stringify({ source: "synthetic" }),
  };
  const bytes = encodeWorld(init, sha256);
  return decodeWorld(bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer);
}

function block(rows: readonly [number, number][]): SignalBlock {
  return {
    count: rows.length,
    signalId: Uint32Array.from(rows.map((r) => r[0])),
    timeToChangeDs: Uint16Array.from(rows.map(() => 50)),
    phase: Uint8Array.from(rows.map((r) => r[1])),
    reserved: new Uint8Array(rows.length),
  };
}

const groupId = (controller: number, group: number): number => (controller + 1) * 65536 + group;
const lit = (c: [number, number, number] | null): number => (c ? c[0] + c[1] + c[2] : 0);

describe("signal lens shapes", () => {
  it("gives a separately signalled turn lane arrow lenses, and every other head balls or symbols", () => {
    const w = new WorldRenderer({ theme: DARK_THEME });
    w.setWorld(approachWorld(2, 3));
    expect(w.signals.count).toBe(4);
    for (let k = 0; k < 3; k++) {
      expect(w.signals.lensShape(0, k), "through lane").toBe(LENS.BALL);
      expect(w.signals.lensShape(1, k), "left-only lane on its own group").toBe(LENS.LEFT_ARROW);
      expect(w.signals.lensShape(2, k), "right-only lane on its own group").toBe(LENS.RIGHT_ARROW);
    }
    expect(w.signals.lensShape(3, 0)).toBe(LENS.HAND);
    expect(w.signals.lensShape(3, 1)).toBeNull();
    expect(w.signals.lensShape(3, 2)).toBe(LENS.WALKER);
    const names = w.signals.group.children.map((c) => c.name);
    expect(names).toEqual(expect.arrayContaining([
      "world/signal-lamps", "world/signal-lamps-left-arrow", "world/signal-lamps-right-arrow",
      "world/signal-lamps-hand", "world/signal-lamps-walker",
    ]));
    w.dispose();
  });

  it("keeps round lenses on a turn lane that runs on the through movement's group", () => {
    const w = new WorldRenderer({ theme: DARK_THEME });
    w.setWorld(approachWorld(1, 1));
    for (const h of [0, 1, 2]) for (let k = 0; k < 3; k++) expect(w.signals.lensShape(h, k), `head ${h}`).toBe(LENS.BALL);
    w.dispose();
  });

  it("shows a permissive turn as the flashing yellow arrow and a protected one as the green arrow", () => {
    const w = new WorldRenderer({ theme: DARK_THEME });
    w.setWorld(approachWorld(2, 3));
    // Through permissive green (5), left permissive (5), right protected (6).
    w.applySignalKeyframe(block([[groupId(3, 1), 5], [groupId(3, 2), 5], [groupId(3, 3), 6]]));
    // The round-lens through head: a plain green ball.
    expect(w.signals.headState(0)?.aspect.name).toBe("green");
    expect(lit(w.signals.lampColor(0, 2))).toBeGreaterThan(5 * lit(w.signals.lampColor(0, 1)));
    // The left arrow face: amber, flashing; no green arrow.
    expect(w.signals.headState(1)?.aspect.name).toBe("amber-flashing");
    w.signals.update(0.1);
    const on = lit(w.signals.lampColor(1, 1));
    w.signals.update(0.6);
    const off = lit(w.signals.lampColor(1, 1));
    expect(on).toBeGreaterThan(5 * off);
    expect(lit(w.signals.lampColor(1, 2))).toBeLessThan(on / 5);
    // The right arrow face, protected: the green arrow.
    expect(w.signals.headState(2)?.aspect.name).toBe("green");
    expect(lit(w.signals.lampColor(2, 2))).toBeGreaterThan(5 * lit(w.signals.lampColor(2, 0)));
    w.dispose();
  });
});
