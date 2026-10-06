/**
 * §3.3.5 — the activity byte: what a road user is doing rides in the bytes v1.0 reserved
 * (`Keyframe.actors.flags8`, `Delta.moved.reserved`, `Delta.spawns.reserved`). The decoder
 * exposes it as `activity`, on the same bytes, and an encoder that leaves it out writes 0.
 */

import { describe, expect, it } from "vitest";

import {
  ActorActivity,
  ActorState,
  decodeDelta,
  decodeKeyframe,
  deltaFrame,
  inCarriageway,
  keyframeFrame,
  viewFrame,
} from "../src/index.js";

const actor = (actorId: number, flags8?: number) => ({
  actorId,
  xMm: 1000,
  yMm: 2000,
  laneId: 0xffffffff,
  zCm: 0,
  headingBrad: 0,
  speedCq: 0,
  accelCq: 0,
  classIdx: 0,
  state: ActorState.EQUIPPED,
  verifiedNeighbors: 0,
  ...(flags8 === undefined ? {} : { flags8 }),
});

describe("§3.3.5 — the activity byte", () => {
  it("is read from a keyframe's flags8", () => {
    const frame = keyframeFrame(
      {
        simTimeNs: 1_000_000_000n,
        originXM: 0,
        originYM: 0,
        originZM: 0,
        gopIndex: 0,
        actors: [actor(1, ActorActivity.WAITING_AT_KERB), actor(2)],
        signals: [],
      },
      1n,
    );
    const k = decodeKeyframe(viewFrame(frame));
    expect(Array.from(k.actors.activity)).toEqual([ActorActivity.WAITING_AT_KERB, ActorActivity.NONE]);
    expect(Array.from(k.actors.flags8)).toEqual(Array.from(k.actors.activity));
  });

  it("is read from a delta's moved and spawn rows", () => {
    const frame = deltaFrame(
      {
        simTimeNs: 1_100_000_000n,
        gopIndex: 0,
        stepIndex: 1,
        moved: [
          {
            slot: 0,
            dxMm: 140,
            dyMm: 0,
            dzMm: 0,
            headingBrad: 0,
            speedCq: 179,
            accelCq: 0,
            state: ActorState.EQUIPPED,
            verifiedNeighbors: 0,
            mflags: 0,
            activity: ActorActivity.CROSSING_MIDBLOCK,
          },
          {
            slot: 1,
            dxMm: 10,
            dyMm: 0,
            dzMm: 0,
            headingBrad: 0,
            speedCq: 10,
            accelCq: 0,
            state: ActorState.EQUIPPED,
            verifiedNeighbors: 0,
            mflags: 0,
          },
        ],
        spawns: [
          {
            slot: 2,
            actorId: 9,
            nodeId: 0xffffffff,
            xMm: 0,
            yMm: 0,
            laneId: 7,
            zCm: 0,
            headingBrad: 0,
            speedCq: 0,
            cause: 0,
            classIdx: 0,
            state: 0,
            verifiedNeighbors: 0,
            activity: ActorActivity.CROSSING_AGAINST_SIGNAL,
          },
        ],
      },
      2n,
    );
    const d = decodeDelta(viewFrame(frame));
    expect(Array.from(d.moved.activity)).toEqual([ActorActivity.CROSSING_MIDBLOCK, ActorActivity.NONE]);
    expect(Array.from(d.spawns.activity)).toEqual([ActorActivity.CROSSING_AGAINST_SIGNAL]);
  });

  it("names which activities are in the carriageway", () => {
    expect(inCarriageway(ActorActivity.CROSSING_ON_WALK)).toBe(true);
    expect(inCarriageway(ActorActivity.CROSSING_MIDBLOCK)).toBe(true);
    expect(inCarriageway(ActorActivity.WAITING_MIDBLOCK)).toBe(false);
    expect(inCarriageway(ActorActivity.WAITING_AT_KERB)).toBe(false);
    expect(inCarriageway(ActorActivity.NONE)).toBe(false);
  });
});
