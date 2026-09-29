/**
 * The settings window's logic, apart from React: which fields there are, how they are grouped,
 * which match a search, which are edited or modified, and which validation error belongs to which.
 *
 * Kept free of the DOM and the store so the Node suite can check it exactly (`test/settings.test.ts`).
 */

import type { ValidationError } from "@vwp/protocol";

import {
  PHASE1_FIELDS,
  fieldsFromJsonSchema,
  fieldsFromPublished,
  pathOfPointer,
  type FieldStatus,
  type FormField,
  type PublishedFormField,
  type PublishedRow,
} from "../lib/schema.js";

/** A field of either source, with the engine's statement about it when the engine published one. */
export type Field = FormField &
  Partial<Pick<PublishedFormField, "status" | "statusNote" | "collection" | "path" | "hasDefault" | "defaultValue" | "range">>;

/** Where the fields came from, which the window says in one line. */
export type FieldSource = "published" | "schema" | "builtin";

/** The fields the window shows, from the best source the engine offered. */
export function settingsFields(
  published: readonly PublishedRow[],
  schema: unknown,
  doc: unknown,
): { readonly fields: readonly Field[]; readonly source: FieldSource } {
  if (published.length > 0) return { fields: fieldsFromPublished(published, schema, doc), source: "published" };
  const generated = schema ? fieldsFromJsonSchema(schema) : [];
  if (generated.length > 0) return { fields: generated, source: "schema" };
  return { fields: PHASE1_FIELDS, source: "builtin" };
}

/** The dotted path a scenario file spells the field with. */
export function fieldPath(f: Field): string {
  return f.path ?? pathOfPointer(f.pointer);
}

/**
 * Settings the engine reads nothing from, or has not classified. They sit behind the
 * "Unsupported" filter: editable, never silently accepted, but not in the way.
 */
export function isUnsupported(f: Field): boolean {
  return f.status === "not-implemented" || f.status === "unknown";
}

/** True when an unapplied edit touches this field (or anything inside it, for a collection). */
export function isEdited(f: Field, edits: readonly string[]): boolean {
  return edits.some((p) => p === f.pointer || p.startsWith(`${f.pointer}/`));
}

/** JSON with its keys sorted, so two equal objects written in another order compare equal. */
export function stableJson(value: unknown): string {
  if (value === undefined) return "undefined";
  return JSON.stringify(value, (_k, v: unknown) =>
    v !== null && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v as Record<string, unknown>).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
      : v,
  );
}

/**
 * True when the value is not the engine's default. A field with no published default (a list, a
 * map, an optional value) is never "modified": there is nothing to say it differs from.
 */
export function isModified(f: Field, value: unknown): boolean {
  if (f.hasDefault !== true || value === undefined) return false;
  return stableJson(value) !== stableJson(f.defaultValue);
}

/** A value as the default line prints it. */
export function describeValue(value: unknown): string {
  if (value === undefined) return "none";
  if (value === null) return "null";
  if (typeof value === "string") return value === "" ? "empty" : value;
  const text = JSON.stringify(value);
  return text.length > 80 ? `${text.slice(0, 77)}…` : text;
}

/**
 * Whether a field matches a search. Every word must begin a word somewhere in the field's title,
 * description, dotted path, group, unit or the engine's note about it — so "radio power" finds
 * `radio.tx_power_dbm` and "duration" finds `time.duration_s`, while "rate" does not find every
 * description that says "generated". A word with a dot or a slash in it is a path, and matches
 * anywhere in the dotted path or the pointer: `demand.rate`, `/time/`; a unit (`veh/h`) matches
 * inside the unit.
 */
export function matches(f: Field, query: string): boolean {
  const words = query.toLowerCase().split(/\s+/).filter((w) => w !== "");
  if (words.length === 0) return true;
  const path = `${fieldPath(f)} ${f.pointer}`.toLowerCase();
  const tokens = new Set(
    [f.label, f.help ?? "", fieldPath(f), f.group, f.unit ?? "", f.statusNote ?? ""]
      .join(" ")
      .toLowerCase()
      .split(/[^a-z0-9µ°]+/)
      .filter((t) => t !== ""),
  );
  const unit = (f.unit ?? "").toLowerCase();
  return words.every((w) => {
    if (w.includes(".") || w.startsWith("/")) return path.includes(w);
    // A unit is one word with a slash in it (`veh/h`, `m/s`), matched whole.
    if (unit !== "" && unit.includes(w)) return true;
    for (const t of tokens) if (t.startsWith(w)) return true;
    return false;
  });
}

/** Words for a section key the scenario spells as an abbreviation or a plural. */
const SECTION_LABELS: Record<string, string> = {
  vru: "Pedestrians and cyclists",
  rsus: "Roadside units",
  backend: "Backend (SCMS)",
  meta: "Description",
  seed: "Seed",
  time: "Time",
  phy: "Physical layer",
  mac: "Channel access",
  tiers: "Fidelity",
  gnss: "Positioning",
  crypto_mode: "Cryptography",
};

export function humanise(key: string): string {
  if (SECTION_LABELS[key]) return SECTION_LABELS[key];
  const spaced = key.replace(/[_-]+/g, " ").trim();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/** One heading under a group: the scenario section a field lives in. */
export interface SettingsSection {
  readonly id: string;
  readonly key: string;
  readonly label: string;
  readonly fields: readonly Field[];
}

/** One entry of the category tree. */
export interface SettingsGroup {
  readonly id: string;
  readonly name: string;
  readonly description: string;
  readonly sections: readonly SettingsSection[];
  readonly count: number;
}

/** The engine's published group list (`scenario.get` `groups`). */
export interface GroupMeta {
  readonly name: string;
  readonly description: string;
  readonly sections: readonly string[];
}

/**
 * Which section of its group a field sits under.
 *
 * A group that spans several top-level keys (Run: meta, seed, time) is sectioned by the key; a
 * group that is one key (Traffic is all of `actors`) by the level below, so "Traffic" opens into
 * vehicles, pedestrians and cyclists, roadside units and the backend rather than one long list.
 */
export function sectionKeyOf(f: Field, meta: GroupMeta | undefined): string {
  const parts = fieldPath(f).split(".");
  if (meta === undefined) return f.group;
  if (meta.sections.length > 1 || parts.length < 3) return parts[0] ?? f.group;
  return parts[1] ?? parts[0];
}

/** The words between a field's section and its own name, as a breadcrumb: `Demand ›`. */
export function breadcrumbOf(f: Field, sectionKey: string): string {
  const parts = fieldPath(f).split(".");
  const at = parts.indexOf(sectionKey);
  const middle = at >= 0 ? parts.slice(at + 1, -1) : [];
  return middle
    .filter((p) => p !== "*" && p !== "-")
    .map(humanise)
    .join(" › ");
}

function slug(text: string): string {
  return text.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
}

/**
 * The category tree, in the engine's group order (then any group it did not list, in field order),
 * with only the fields given — so a search or a filter prunes the tree to what it shows.
 */
export function buildTree(fields: readonly Field[], metas: readonly GroupMeta[]): SettingsGroup[] {
  const order: string[] = metas.map((m) => m.name);
  for (const f of fields) if (!order.includes(f.group)) order.push(f.group);
  const out: SettingsGroup[] = [];
  for (const name of order) {
    const meta = metas.find((m) => m.name === name);
    const inGroup = fields.filter((f) => f.group === name);
    if (inGroup.length === 0) continue;
    const sections: { key: string; fields: Field[] }[] = [];
    for (const f of inGroup) {
      const key = sectionKeyOf(f, meta);
      let section = sections.find((s) => s.key === key);
      if (!section) {
        section = { key, fields: [] };
        sections.push(section);
      }
      section.fields.push(f);
    }
    // In a group that is one scenario key, the fields directly under that key (`radio.rat`) are the
    // group's general settings: they come first, under "General" rather than the group's own name
    // again ("Radio › Radio").
    const general = meta !== undefined && meta.sections.length === 1 ? meta.sections[0] : null;
    sections.sort((a, b) => Number(b.key === general) - Number(a.key === general));
    out.push({
      id: `group-${slug(name)}`,
      name,
      description: meta?.description ?? "",
      count: inGroup.length,
      sections: sections.map((s) => ({
        id: `section-${slug(name)}-${slug(s.key)}`,
        key: s.key,
        label: s.key === name ? name : s.key === general ? "General" : humanise(s.key),
        fields: s.fields,
      })),
    });
  }
  return out;
}

/** A validation error's path as a pointer: the engine sends pointers, older builds dotted paths. */
export function errorPointer(path: string): string {
  if (path.startsWith("/")) return path;
  if (path === "") return "";
  return `/${path.replace(/\[(\d+)\]/g, ".$1").split(".").join("/")}`;
}

/**
 * Which field each validation error is about: the field whose pointer is the longest prefix of the
 * error's. An error no field covers (about the document as a whole) is returned apart, so the
 * window lists it at the top rather than dropping it.
 */
export function errorsByField(
  fields: readonly Field[],
  errors: readonly ValidationError[],
): { readonly byPointer: ReadonlyMap<string, ValidationError[]>; readonly unplaced: readonly ValidationError[] } {
  const byPointer = new Map<string, ValidationError[]>();
  const unplaced: ValidationError[] = [];
  for (const e of errors) {
    const p = errorPointer(e.path);
    let best: Field | null = null;
    for (const f of fields) {
      if (p === f.pointer || p.startsWith(`${f.pointer}/`)) {
        if (best === null || f.pointer.length > best.pointer.length) best = f;
      }
    }
    if (best === null) {
      unplaced.push(e);
      continue;
    }
    const list = byPointer.get(best.pointer) ?? [];
    list.push(e);
    byPointer.set(best.pointer, list);
  }
  return { byPointer, unplaced };
}

/** The words for a status, in the user's language. `wired` is the normal case and gets none. */
export const STATUS_BADGE: Record<FieldStatus, { label: string; cls: string } | null> = {
  wired: null,
  partial: { label: "partly applied", cls: "warn" },
  "not-implemented": { label: "not applied", cls: "off" },
  refused: { label: "limited choices", cls: "info" },
  descriptive: { label: "description", cls: "" },
  unknown: { label: "unclassified", cls: "off" },
};
