/**
 * The SCMS menus (docs/design/scms-real/08-settings-ux-and-gaps.md §4.2): maps edited a row per key,
 * `[from, to]` pairs, the engine's sub-headings, relevance and the duration readout. The rows are
 * shaped like the engine's published surface (`crates/v2xw-engine/src/scenario/publish.rs`).
 */

import { describe, expect, it } from "vitest";

import { fieldsFromPublished } from "../src/lib/schema.js";
import { buildResearchTree, controlKind, irrelevantReason, type Field } from "../src/settings/model.js";
import { durationWords } from "../src/lib/duration.js";

const ptr = (path: string): string => `/${path.replace(/\[\]/g, "/-").replace(/\.\*/g, "/*").split(".").join("/")}`;
const row = (path: string, extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  "x-path": path,
  "x-pointer": ptr(path),
  "x-group": "Security",
  "x-status": "wired",
  title: path.split(".").pop(),
  ...extra,
});
const opt = (value: string, label = value, extra: Record<string, unknown> = {}) => ({ value, label, description: `${label}.`, available: true, ...extra });

const ROLES = [opt("ra", "Registration Authority"), opt("ma", "Misbehaviour Authority"), opt("lop", "LOP", { us: false })];

/** A schema with the map nodes' `x-key-options` and the pair's `x-pair`, as `publish.rs` writes them. */
const SCHEMA = {
  properties: {
    actors: {
      properties: {
        backend: {
          properties: {
            entities: { "x-pointer": "/actors/backend/entities", "x-keyed": true, "x-key-options": ROLES, title: "Per-server hardware" },
          },
        },
      },
    },
    security: {
      properties: {
        scms: {
          properties: {
            deployment: {
              properties: {
                providers: {
                  "x-pointer": "/security/scms/deployment/providers",
                  items: {
                    properties: {
                      components: { "x-pointer": "/security/scms/deployment/providers/-/components", "x-keyed": true, "x-key-options": ROLES, title: "Servers it runs" },
                    },
                  },
                },
              },
            },
          },
        },
        pseudonym_change: {
          properties: { silent_period_s: { "x-pointer": "/security/pseudonym_change/silent_period_s", "x-pair": true } },
        },
      },
    },
  },
};

const ROWS = [
  row("actors.backend.entities.*.profile", { kind: "enum", "x-options": [opt("backend/xeon")] }),
  row("actors.backend.entities.*.service_model", { kind: "enum", "x-options": [opt("service/mmc"), opt("service/mm1")] }),
  row("security.scms.deployment.providers[].name", { kind: "enum", "x-options": [opt("provider-a")], "x-options-open": true }),
  row("security.scms.deployment.providers[].components.*", { kind: "integer", unit: "servers", minimum: 0, maximum: 8 }),
  row("security.pseudonym_change.silent_period_s[]", { kind: "number", unit: "s", minimum: 0, maximum: 60 }),
  row("security.pseudonym_change.strategy", {
    kind: "enum",
    "x-options": [opt("time", "Every few minutes"), opt("mix-zone", "In a mix zone")],
    "x-subgroup": { id: "lifecycle", label: "Certificate lifecycle", order: 3 },
    "x-subgroup-blurb": "How a vehicle gets and uses its certificates.",
  }),
  row("security.pseudonym_change.mix_zone_radius_m", {
    kind: "number",
    unit: "m",
    "x-relevant-when": { path: "security.pseudonym_change.strategy", in: ["mix-zone"] },
    "x-subgroup": { id: "lifecycle", label: "Certificate lifecycle", order: 3 },
  }),
  row("detection.ma.params", { kind: "json", "x-any": true, "x-typed-children": true }),
  row("detection.ma.params.window_s", { kind: "number", unit: "s", exclusiveMinimum: 0, maximum: 86400 }),
  row("security.scms.deployment.preset", {
    kind: "enum",
    "x-options": [opt("us-production-2025", "US system (2025)", { us: true })],
    "x-subgroup": { id: "deployment", label: "Deployment", order: 1 },
  }),
];

const fields = (): Field[] => fieldsFromPublished(ROWS, SCHEMA, {}) as Field[];
const at = (p: string): Field => {
  const f = fields().find((x) => x.pointer === p);
  if (!f) throw new Error(`no field at ${p}`);
  return f;
};

describe("SCMS menus", () => {
  it("edits a map with published keys a row per key, not as JSON", () => {
    const f = at("/actors/backend/entities");
    expect(f.kind).toBe("map");
    expect(controlKind(f)).toBe("map");
    expect(f.keyChoices?.map((c) => c.value)).toEqual(["ra", "ma", "lop"]);
    expect(f.keyChoices?.[2].us).toBe(false);
    expect(f.valueFields?.map((v) => v.key)).toEqual(["/profile", "/service_model"]);
  });

  it("edits a map inside a list element as a field of the row editor", () => {
    const f = at("/security/scms/deployment/providers");
    expect(f.kind).toBe("list");
    const comp = f.itemFields?.find((x) => x.key === "/components");
    expect(comp?.kind).toBe("map");
    expect(comp?.valueFields?.[0]).toMatchObject({ key: "", kind: "integer", min: 0, max: 8 });
  });

  it("draws a [from, to] list as a range pair with the element's unit and range", () => {
    const f = at("/security/pseudonym_change/silent_period_s");
    expect(f.kind).toBe("pair");
    expect(controlKind(f)).toBe("pair");
    expect([f.min, f.max, f.unit]).toEqual([0, 60, "s"]);
  });

  it("files SCMS fields under the engine's sub-headings, in the engine's order, with the help", () => {
    const tree = buildResearchTree(fields().filter((f) => f.pointer.startsWith("/security/")));
    const scms = tree.find((g) => g.name === "Certificates & SCMS");
    expect(scms?.subs.map((s) => s.label).slice(0, 2)).toEqual(["Deployment", "Certificate lifecycle"]);
    expect(scms?.subs.find((s) => s.label === "Certificate lifecycle")?.blurb).toMatch(/certificates/);
  });

  it("greys a field whose condition does not hold, and says which value it needs", () => {
    const all = fields();
    const radius = all.find((f) => f.pointer.endsWith("mix_zone_radius_m")) as Field;
    expect(irrelevantReason(radius, { security: { pseudonym_change: { strategy: "time" } } }, all)).toBe(
      "Only used with “In a mix zone”.",
    );
    expect(irrelevantReason(radius, { security: { pseudonym_change: { strategy: "mix-zone" } } }, all)).toBeUndefined();
  });

  it("edits a parameter block through its typed fields, with no JSON box", () => {
    const all = fields();
    expect(all.some((f) => f.pointer === "/detection/ma/params")).toBe(false);
    expect(at("/detection/ma/params/window_s").kind).toBe("number");
  });

  it("reads a duration in words", () => {
    expect(durationWords(604800)).toBe("1 week");
    expect(durationWords(608400)).toBe("1 week 1 h");
    expect(durationWords(189216000)).toBe("6 years");
    expect(durationWords(30)).toBe("");
  });
});
