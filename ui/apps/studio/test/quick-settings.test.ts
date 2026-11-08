/**
 * The quick settings' logic (`src/settings/quick.ts`) and the level the engine publishes per field.
 * Patches are shaped like `crates/v2xw-engine/src/scenario/guide.rs`'s.
 */

import { describe, expect, it } from "vitest";

import { fieldsFromPublished } from "../src/lib/schema.js";
import { isBasic } from "../src/settings/model.js";
import { applyPatch, classesWith, currentChoice, describeSeconds } from "../src/settings/quick.js";

const sum = (c: Record<string, { fraction: number }>): number => Object.values(c).reduce((a, v) => a + v.fraction, 0);

describe("fleet-mix toggle", () => {
  it("takes a share from cars and gives it back, keeping the sum at one", () => {
    const on = classesWith({}, "truck", 0.03, "passenger", true);
    expect(on).toEqual({ passenger: { fraction: 0.97 }, truck: { fraction: 0.03 } });
    const both = classesWith(on, "motorcycle", 0.02, "passenger", true);
    expect(Math.abs(sum(both) - 1)).toBeLessThan(1e-9);
    const off = classesWith(both, "truck", 0.03, "passenger", false);
    expect(off.truck).toBeUndefined();
    expect(Math.abs(sum(off) - 1)).toBeLessThan(1e-9);
    expect(classesWith(off, "motorcycle", 0.02, "passenger", false)).toEqual({});
  });

  it("takes from the largest class when there is no passenger class", () => {
    const out = classesWith({ bus: { fraction: 0.2 }, delivery: { fraction: 0.8 } }, "truck", 0.1, "passenger", true);
    expect(out.delivery.fraction).toBeCloseTo(0.7);
    expect(Math.abs(sum(out) - 1)).toBeLessThan(1e-9);
  });
});

describe("published choices", () => {
  const opts = [
    { label: "US", patch: [{ path: "security.envelope", pointer: "/security/envelope", value: "ieee1609.2" }] },
    {
      label: "EU",
      patch: [
        { path: "security.envelope", pointer: "/security/envelope", value: "etsi103097" },
        { path: "messages.sets", pointer: "/messages/sets", value: ["cam"] },
      ],
    },
  ];

  it("finds the choice the document holds, and none for a mixed one", () => {
    const eu = applyPatch({ security: { envelope: "ieee1609.2" } }, opts[1].patch);
    expect(currentChoice(eu, opts)).toBe(1);
    expect(currentChoice({ security: { envelope: "ieee1609.2" } }, opts)).toBe(0);
    expect(currentChoice({ security: { envelope: "etsi103097" }, messages: { sets: ["bsm"] } }, opts)).toBe(-1);
  });

  it("reads the level, the explanation and the recommendation off a published row", () => {
    const [f] = fieldsFromPublished(
      [{ "x-pointer": "/seed", "x-path": "seed", kind: "integer", "x-level": "basic", "x-explain": "Why.", "x-recommended": "Any." }],
      null,
      {},
    );
    expect(f.level).toBe("basic");
    expect(f.explain).toBe("Why.");
    expect(f.recommended).toBe("Any.");
    expect(isBasic(f, true)).toBe(true);
    expect(isBasic({ ...f, level: "advanced" }, true)).toBe(false);
    expect(isBasic({ ...f, level: undefined }, false)).toBe(true);
  });

  it("says a run length in words", () => {
    expect(describeSeconds(90)).toBe("1.5 min");
    expect(describeSeconds(86_400)).toBe("1 day");
    expect(describeSeconds(604_800)).toBe("7 days");
  });
});
