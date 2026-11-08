/**
 * The scenario picker opens on the ready-made scenario that is running (`runningPreset`).
 *
 * A preset run on another city is another document, with another hash than the one the list
 * gives, so matching by hash alone found nothing after any run the picker had started itself:
 * the picker opened with no scenario chosen and its Run disabled (engine e2e, owner-flow: pick
 * "Rush hour", run it, then pick only a city).
 */

import { describe, expect, it } from "vitest";

import { runningPreset } from "../src/state/settings.js";
import type { ScenarioListItem } from "../src/state/store.js";

const PRESETS: ScenarioListItem[] = [
  { id: "midtown-normal", kind: "preset", name: "Normal traffic", hash: "aaa" },
  { id: "midtown-rush-hour", kind: "preset", name: "Evening rush hour", hash: "bbb" },
];

describe("the running preset", () => {
  it("is found by its hash when it runs as listed", () => {
    expect(runningPreset(PRESETS, "bbb", "something else")?.id).toBe("midtown-rush-hour");
  });

  it("is found by its name when it runs on another city", () => {
    expect(runningPreset(PRESETS, "ccc", "Evening rush hour")?.id).toBe("midtown-rush-hour");
  });

  it("is none for a scenario that is not a preset", () => {
    expect(runningPreset(PRESETS, "ddd", "my own scenario")).toBeUndefined();
    expect(runningPreset(PRESETS, null, null)).toBeUndefined();
  });
});
