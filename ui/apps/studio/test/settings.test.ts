/**
 * The settings window's logic (`src/settings/model.ts`) and the draft's rebase (`state/store.ts`).
 *
 * The rows below are shaped exactly like the engine's published surface (`scenario.get
 * {with_schema:true}` `fields`, from `crates/v2xw-engine/src/scenario/publish.rs`): `x-pointer`,
 * `x-path`, `x-group`, `x-status`, `x-status-note`, `default`, `x-range`.
 */

import { describe, expect, it } from "vitest";

import { fieldsFromPublished } from "../src/lib/schema.js";
import {
  breadcrumbOf,
  buildTree,
  errorPointer,
  errorsByField,
  isEdited,
  isModified,
  isUnsupported,
  matches,
  sectionKeyOf,
  settingsFields,
  stableJson,
  type Field,
} from "../src/settings/model.js";
import { rebaseDraft } from "../src/state/store.js";

const row = (path: string, extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  "x-path": path,
  "x-pointer": `/${path.split(".").join("/")}`,
  "x-status": "wired",
  "x-status-note": "",
  kind: "number",
  ...extra,
});

const ROWS = [
  row("time.duration_s", { "x-group": "Run", title: "Duration", unit: "s", default: 60, description: "How long the run lasts.", "x-range": "(0, ∞)" }),
  row("seed", { "x-group": "Run", title: "Seed", kind: "string", default: "0x0" }),
  row("actors.vehicles.demand.rate_veh_per_h", { "x-group": "Traffic", title: "Rate", unit: "veh/h", description: "Vehicles per hour offered to the network." }),
  row("actors.vehicles.equipped_fraction", { "x-group": "Traffic", title: "Equipped fraction", default: 1 }),
  row("actors.vru.pedestrians", { "x-group": "Traffic", title: "Pedestrians", default: 0 }),
  row("radio.rat", { "x-group": "Radio", title: "Rat", kind: "enum", enum: ["dsrc-80211p", "lte-v2x-pc5"], default: "dsrc-80211p" }),
  row("radio.tiers.phy", { "x-group": "Radio", title: "Phy", kind: "enum", enum: ["abstract", "medium", "high"], default: "medium", "x-status": "partial", "x-status-note": "Only the medium tier is read." }),
  row("radio.legacy_knob", { "x-group": "Radio", title: "Legacy knob", "x-status": "not-implemented", "x-status-note": "Read by nothing." }),
];
const GROUPS = [
  { name: "Run", description: "What this scenario is.", sections: ["meta", "seed", "time"] },
  { name: "Traffic", description: "What moves.", sections: ["actors"] },
  { name: "Radio", description: "The radio stack.", sections: ["radio"] },
];
const DOC = {
  seed: "0x0",
  time: { duration_s: 30 },
  actors: { vehicles: { demand: { rate_veh_per_h: 600 }, equipped_fraction: 1 }, vru: { pedestrians: 0 } },
  radio: { rat: "dsrc-80211p", tiers: { phy: "medium" }, legacy_knob: 3 },
};

const fields = (): Field[] => fieldsFromPublished(ROWS, null, DOC);
const byPointer = (p: string): Field => {
  const f = fields().find((x) => x.pointer === p);
  if (!f) throw new Error(`no field ${p}`);
  return f;
};

describe("the settings surface", () => {
  it("carries the engine's default, path and range onto each field", () => {
    const d = byPointer("/time/duration_s");
    expect(d.path).toBe("time.duration_s");
    expect(d.hasDefault).toBe(true);
    expect(d.defaultValue).toBe(60);
    expect(d.range).toBe("(0, ∞)");
    // No default published: never "modified", because there is nothing to differ from.
    expect(byPointer("/actors/vehicles/demand/rate_veh_per_h").hasDefault).toBe(false);
  });

  it("prefers the published surface, and says which source it used", () => {
    expect(settingsFields(ROWS, null, DOC).source).toBe("published");
    expect(settingsFields([], null, DOC).source).toBe("builtin");
  });
});

describe("search", () => {
  it("finds a setting by title, description, path or pointer, every word required", () => {
    const hits = (q: string): string[] => fields().filter((f) => matches(f, q)).map((f) => f.pointer);
    expect(hits("duration")).toEqual(["/time/duration_s"]);
    expect(hits("duration_s")).toEqual(["/time/duration_s"]);
    expect(hits("rate_veh")).toEqual(["/actors/vehicles/demand/rate_veh_per_h"]);
    expect(hits("vehicles per hour")).toEqual(["/actors/vehicles/demand/rate_veh_per_h"]);
    expect(hits("demand.rate")).toEqual(["/actors/vehicles/demand/rate_veh_per_h"]);
    expect(hits("/radio/tiers")).toEqual(["/radio/tiers/phy"]);
    expect(hits("veh/h")).toEqual(["/actors/vehicles/demand/rate_veh_per_h"]);
    expect(hits("medium tier")).toEqual(["/radio/tiers/phy"]);
    expect(hits("duration radio")).toEqual([]);
  });

  it("matches the start of a word, not the inside of one", () => {
    // "rate" is inside "generated" and "accelerate"; neither is a match for a search about rates.
    const f: Field = { pointer: "/x", label: "X", group: "G", kind: "number", help: "Generated at an accelerated pace." };
    expect(matches(f, "rate")).toBe(false);
    expect(matches(f, "gener")).toBe(true);
  });
});

describe("edited and modified", () => {
  it("marks an edit to a leaf, and to anything inside a collection", () => {
    const d = byPointer("/time/duration_s");
    expect(isEdited(d, ["/time/duration_s"])).toBe(true);
    expect(isEdited(d, ["/time/duration_s_other"])).toBe(false);
    const c: Field = { pointer: "/events", label: "Events", group: "Timeline", kind: "json" };
    expect(isEdited(c, ["/events/0/t"])).toBe(true);
  });

  it("calls a value modified when it is not the engine's default, comparing JSON by content", () => {
    const d = byPointer("/time/duration_s");
    expect(isModified(d, 30)).toBe(true);
    expect(isModified(d, 60)).toBe(false);
    expect(isModified(d, undefined)).toBe(false);
    expect(stableJson({ b: 1, a: [1, { d: 2, c: 3 }] })).toBe(stableJson({ a: [1, { c: 3, d: 2 }], b: 1 }));
  });

  it("puts the settings the engine reads nothing from behind the Unsupported filter", () => {
    expect(isUnsupported(byPointer("/radio/legacy_knob"))).toBe(true);
    expect(isUnsupported(byPointer("/radio/tiers/phy"))).toBe(false);
    // A key the engine refuses whatever its value (`detection.responder`) is unsupported too.
    const refused: Field = { ...byPointer("/radio/legacy_knob"), pointer: "/detection/responder", status: "refused" };
    expect(isUnsupported(refused)).toBe(true);
    expect(isUnsupported({ ...refused, status: "partial" })).toBe(false);
  });
});

describe("the category tree", () => {
  it("follows the engine's group order, sections a one-key group by the level below, and puts its own leaves first as General", () => {
    const tree = buildTree(fields(), GROUPS);
    expect(tree.map((g) => g.name)).toEqual(["Run", "Traffic", "Radio"]);
    const run = tree[0];
    expect(run.sections.map((s) => s.key)).toEqual(["time", "seed"]);
    const traffic = tree[1];
    expect(traffic.sections.map((s) => s.label)).toEqual(["Vehicles", "Pedestrians and cyclists"]);
    const radio = tree[2];
    expect(radio.sections[0].label).toBe("General");
    expect(radio.sections[0].fields.map((f) => f.pointer)).toEqual(["/radio/rat", "/radio/legacy_knob"]);
    expect(radio.sections[1].label).toBe("Fidelity");
  });

  it("prunes to what a search shows", () => {
    const tree = buildTree(fields().filter((f) => matches(f, "rate")), GROUPS);
    expect(tree.map((g) => g.name)).toEqual(["Traffic"]);
    expect(tree[0].count).toBe(1);
  });

  it("titles a deep field with the words between its section and its name", () => {
    const rate = byPointer("/actors/vehicles/demand/rate_veh_per_h");
    const key = sectionKeyOf(rate, GROUPS[1]);
    expect(key).toBe("vehicles");
    expect(breadcrumbOf(rate, key)).toBe("Demand");
    expect(breadcrumbOf(byPointer("/time/duration_s"), "time")).toBe("");
  });
});

describe("validation errors at the field", () => {
  it("places each error on the field whose pointer is its longest prefix, and keeps the rest apart", () => {
    const { byPointer: placed, unplaced } = errorsByField(fields(), [
      { path: "/time/duration_s", message: "outside the allowed range" },
      { path: "radio.tiers.phy", message: "conflicts" },
      { path: "/nowhere", message: "about the document" },
    ]);
    expect(placed.get("/time/duration_s")?.map((e) => e.message)).toEqual(["outside the allowed range"]);
    expect(placed.get("/radio/tiers/phy")?.map((e) => e.message)).toEqual(["conflicts"]);
    expect(unplaced.map((e) => e.message)).toEqual(["about the document"]);
    expect(errorPointer("events[2].t")).toBe("/events/2/t");
  });
});

describe("the draft", () => {
  it("keeps the user's edits when the engine's document is re-fetched under them", () => {
    const before = { time: { duration_s: 30 }, seed: "0x0" };
    const edited = { time: { duration_s: 12 }, seed: "0x0" };
    const refetched = { time: { duration_s: 30 }, seed: "0x0", extra: true };
    expect(rebaseDraft(before, refetched, edited)).toEqual({ time: { duration_s: 12 }, seed: "0x0", extra: true });
  });

  it("takes the engine's document when there are no edits, or no draft yet", () => {
    const doc = { a: 1 };
    expect(rebaseDraft({ a: 0 }, doc, { a: 0 })).toBe(doc);
    expect(rebaseDraft(null, doc, null)).toBe(doc);
  });
});
