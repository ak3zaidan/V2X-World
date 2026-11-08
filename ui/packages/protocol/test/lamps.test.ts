/**
 * The v1.2 `lamps` byte (§3.3.5): written into the bytes v1.0 reserved, read back, carried by the
 * pose buffer through keyframes, spawns and moved rows — and still ignored by a reader that knows
 * only the v1.0 names (`flags8`, `reserved`).
 */

import { describe, expect, it } from "vitest";
import {
  ActorLamps, PoseBuffer, decodeDelta, decodeKeyframe, deltaFrame, keyframeFrame, viewFrame,
} from "../src/index.js";

const row = (actorId: number, lamps: number) => ({
  actorId, xMm: actorId * 1000, yMm: 0, laneId: 0xffffffff, zCm: 0, headingBrad: 0, speedCq: 0,
  accelCq: 0, classIdx: 0, state: 8, verifiedNeighbors: 0, lamps,
});

describe("the lamps byte", () => {
  it("round-trips through a keyframe, a spawn and a moved row, into the pose buffer", () => {
    const kf = decodeKeyframe(viewFrame(keyframeFrame({
      simTimeNs: 0n, originXM: 0, originYM: 0, originZM: 0, gopIndex: 0,
      actors: [row(1, ActorLamps.BRAKE), row(2, ActorLamps.LOW_BEAM | ActorLamps.TURN_LEFT)], signals: [],
    }, 1n)));
    expect(Array.from(kf.actors.lamps)).toEqual([0x01, 0x12]);
    // The v1.0 name still reads the same bytes.
    expect(Array.from(kf.actors.flags8)).toEqual([0x01, 0x12]);
    const poses = new PoseBuffer(16);
    poses.applyKeyframe(kf);
    expect(poses.lamps[0]).toBe(ActorLamps.BRAKE);
    expect(poses.lamps[1]).toBe(0x12);

    const d = decodeDelta(viewFrame(deltaFrame({
      simTimeNs: 100_000_000n, gopIndex: 0, stepIndex: 1,
      moved: [{ slot: 0, dxMm: 0, dyMm: 0, dzMm: 0, headingBrad: 0, speedCq: 0, accelCq: 0, state: 8, verifiedNeighbors: 0, mflags: 0, lamps: ActorLamps.TURN_RIGHT }],
      spawns: [{
        slot: 2, actorId: 3, nodeId: 0xffffffff, xMm: 5000, yMm: 0, laneId: 0xffffffff, zCm: 0, headingBrad: 0,
        speedCq: 0, cause: 0, classIdx: 6, state: 8, verifiedNeighbors: 0, lamps: ActorLamps.EMERGENCY | ActorLamps.LOW_BEAM,
      }],
    }, 2n)));
    expect(d.moved.lamps[0]).toBe(ActorLamps.TURN_RIGHT);
    expect(d.moved.reserved[0]).toBe(ActorLamps.TURN_RIGHT);
    expect(d.spawns.lamps[0]).toBe(0x50);
    expect(poses.applyDelta(d).applied).toBe(true);
    expect(poses.lamps[0]).toBe(ActorLamps.TURN_RIGHT);
    expect(poses.lamps[2]).toBe(0x50);
  });

  it("is zero where a v1.0 writer left the byte reserved", () => {
    const kf = decodeKeyframe(viewFrame(keyframeFrame({
      simTimeNs: 0n, originXM: 0, originYM: 0, originZM: 0, gopIndex: 0,
      actors: [{ ...row(1, 0), lamps: undefined }], signals: [],
    }, 1n)));
    expect(kf.actors.lamps[0]).toBe(0);
  });
});
