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
  getPointer,
  pathOfPointer,
  type FieldStatus,
  type FormField,
  type PublishedFormField,
  type PublishedRow,
  type Subgroup,
} from "../lib/schema.js";
import { overlayOf } from "./scmsOverlay.js";

/** A field of either source, with the engine's statement about it when the engine published one. */
export type Field = FormField &
  Partial<
    Pick<
      PublishedFormField,
      "status" | "statusNote" | "collection" | "path" | "hasDefault" | "defaultValue" | "range" | "level" | "explain" | "recommended"
    >
  >;

/** Where the fields came from, which the window says in one line. */
export type FieldSource = "published" | "schema" | "builtin";

/** The fields the window shows, from the best source the engine offered. */
export function settingsFields(
  published: readonly PublishedRow[],
  schema: unknown,
  doc: unknown,
): { readonly fields: readonly Field[]; readonly source: FieldSource } {
  if (published.length > 0) return { fields: fieldsFromPublished(published, schema, doc).map(withOverlay), source: "published" };
  const generated = schema ? fieldsFromJsonSchema(schema) : [];
  if (generated.length > 0) return { fields: generated, source: "schema" };
  return { fields: PHASE1_FIELDS, source: "builtin" };
}

/** A field with the window's own words, default and relevance for it (`scmsOverlay.ts`). */
export function withOverlay(f: Field): Field {
  const o = overlayOf(f.pointer);
  if (!o) return f;
  const engineDefault = f.hasDefault === true;
  return {
    ...f,
    ...(o.label ? { label: o.label } : {}),
    ...(o.help ? { explain: o.help } : {}),
    ...(o.unit !== undefined ? (o.unit === "" ? { unit: undefined } : { unit: o.unit }) : {}),
    ...(o.defaultValue !== undefined && !engineDefault ? { hasDefault: true, defaultValue: o.defaultValue } : {}),
    ...(o.relevantWhen && !f.relevantWhen ? { relevantWhen: o.relevantWhen } : {}),
    ...(o.level ? { level: o.level } : {}),
    ...(o.pair && (f.kind === "json" || f.kind === "list") ? { kind: "pair" as const } : {}),
    ...(o.min !== undefined && f.min === undefined ? { min: o.min } : {}),
    ...(o.max !== undefined && f.max === undefined ? { max: o.max } : {}),
  };
}

/** The dotted path a scenario file spells the field with. */
export function fieldPath(f: Field): string {
  return f.path ?? pathOfPointer(f.pointer);
}

/**
 * Settings the engine reads nothing from, refuses outright, or has not classified. They sit behind
 * the "Unsupported" filter with the engine's reason: editable, never silently accepted, but not in
 * the way. `refused` is here because every key the engine files under it changes nothing it
 * computes: `detection.responder` and `detection.perception_tier` are refused whatever their value,
 * and `time.des_resolution` only refuses a resolution the run cannot keep.
 */
export function isUnsupported(f: Field): boolean {
  return f.status === "not-implemented" || f.status === "unknown" || f.status === "refused";
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
  // Words both whole and split at underscores, so `duration_s` and `duration` both find
  // `time.duration_s`.
  const words_ = [f.label, f.help ?? "", fieldPath(f), f.group, f.unit ?? "", f.statusNote ?? ""]
    .join(" ")
    .toLowerCase()
    .split(/[^a-z0-9µ°_]+/)
    .filter((t) => t !== "");
  const tokens = new Set([...words_, ...words_.flatMap((t) => t.split("_")).filter((t) => t !== "")]);
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
  traffic: "Signals, rerouting and gridlock",
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
  // Certificates & SCMS sub-headings.
  lifecycle: "Certificates, shuffle, downloads, revocation and MA",
  deployment: "Deployment and providers",
  protocol: "Credential system",
  pseudonym_change: "Pseudonym change",
  signer_id_policy: "Certificate or digest in each message",
  threshold: "Threshold committees",
  misbehaviour_authority: "Misbehaviour Authority decision",
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

/**
 * Whether a field belongs on the Basic page. A build that publishes no levels at all shows every
 * field on both pages rather than an empty Basic page.
 */
export function isBasic(f: Field, anyLevels: boolean): boolean {
  return !anyLevels || f.level === "basic";
}

/**
 * The window's sections, in the order a researcher works through a study: what runs, what
 * moves, the radio, the MAC, congestion control, messages, the network layer, security, the
 * backend, attackers, and what is written out.
 *
 * Only *where a field is shown* is decided here; which fields exist, their choices, defaults and
 * ranges are the engine's. A field no rule claims falls back to the engine's own group, shown
 * after these, so a key the engine adds later is never dropped.
 */
export interface ResearchSection {
  readonly id: string;
  readonly name: string;
  readonly blurb: string;
  /** The scenario key whose two-part fields (`radio.rat`) are this section's "General". */
  readonly home?: string;
}

export const RESEARCH_SECTIONS: readonly ResearchSection[] = [
  { id: "run", name: "Run", blurb: "Length, step, seed and timeline of the run.", home: "time" },
  { id: "traffic", name: "Traffic", blurb: "Vehicles, people, roadside units and how they move.", home: "actors" },
  { id: "world", name: "World & Weather", blurb: "The map, its buildings and terrain, and the weather." },
  { id: "radio", name: "Radio & Channel", blurb: "Radio technology, power, antennas, propagation and fidelity.", home: "radio" },
  { id: "mac", name: "Medium Access", blurb: "How radios share the channel: 802.11p EDCA or C-V2X sidelink scheduling." },
  { id: "dcc", name: "Congestion Control", blurb: "How radios back off when the channel is busy." },
  { id: "messages", name: "Messages", blurb: "Which messages are sent, how often, and the applications using them.", home: "messages" },
  { id: "net", name: "Networking", blurb: "WSMP or GeoNetworking/BTP, forwarding and fragmentation.", home: "net" },
  { id: "security", name: "Security & Crypto", blurb: "Signature schemes, verification policy, crypto hardware and certificate caches.", home: "security" },
  {
    id: "scms",
    name: "Certificates & SCMS",
    blurb:
      "The certificate system: providers and authorities, pseudonym certificates per week and when they change, the RA shuffle, downloads, enrolment, revocation lists and the Misbehaviour Authority.",
  },
  { id: "backend", name: "Backend & Cellular", blurb: "The SCMS backend and the cellular and IP links that reach it." },
  { id: "attackers", name: "Attackers", blurb: "Attackers, jammers, eavesdroppers and misbehaviour detection.", home: "threats" },
  { id: "output", name: "Output", blurb: "What is measured and which files the run writes.", home: "metrics" },
];

/** Words in a dotted path, split at dots, brackets and underscores. */
function pathWords(path: string): Set<string> {
  return new Set(path.toLowerCase().split(/[.[\]_-]+/).filter((w) => w !== ""));
}

/** Which research section a field belongs in, or null for "the engine's own group". */
export function researchSectionOf(f: Field): string | null {
  const path = fieldPath(f).toLowerCase();
  const top = (path.split(".")[0] ?? "").replace(/\[\]$/, "");
  const words = pathWords(path);
  const has = (...w: string[]): boolean => w.some((x) => words.has(x));
  // Topic words first: a congestion-control or MAC knob is found wherever the engine files it.
  if (has("dcc", "congestion", "cbr")) return "dcc";
  // `radio.sidelink.*` is the C-V2X scheduler (sensing, reservation, candidate share,
  // selection window): medium access, kept together under one heading.
  if (
    has("mac", "edca", "csma", "aifs", "aifsn", "cw", "sps", "harq", "retransmissions", "sensing", "reservation") ||
    path.startsWith("radio.sidelink.") ||
    path.includes("channel_access") ||
    path.includes("access_categories")
  )
    return "mac";
  // The certificate system, wherever the engine files its parts: the deployment and the
  // lifecycle knobs, the credential protocol, when a vehicle changes pseudonym and attaches
  // its certificate, threshold committees, the SCMS backend entities and the MA's decision.
  if (
    path.startsWith("security.scms") ||
    path.startsWith("security.protocol") ||
    path.startsWith("security.pseudonym_change") ||
    path.startsWith("security.signer_id_policy") ||
    path.startsWith("security.threshold") ||
    path.startsWith("actors.backend") ||
    path.startsWith("detection.ma")
  )
    return "scms";
  if (has("uu", "cellular", "backhaul", "tcp", "gnb", "enb") || path.startsWith("net.backend") || path.startsWith("actors.backend") || path.startsWith("nodes.backend"))
    return "backend";
  if (top === "meta" || top === "schema" || top === "seed" || top === "time" || top === "events" || top === "experiment") return "run";
  if (top === "actors" || top === "traffic" || top === "mobility" || top === "energy" || path.startsWith("scale.traffic")) return "traffic";
  if (top === "city" || top === "world" || top === "weather") return "world";
  if (top === "radio" || path.startsWith("scale.radio")) return "radio";
  if (top === "messages" || top === "apps") return "messages";
  if (top === "net") return "net";
  if (top === "security" || top === "nodes" || top === "crypto_mode") return "security";
  if (top === "threats" || top === "detection") return "attackers";
  if (top === "metrics" || top === "exporters") return "output";
  if (top === "scale") return "traffic";
  return null;
}

/** One sub-heading of a research section. */
export interface ResearchSub {
  readonly key: string;
  readonly label: string;
  /** The help under an engine sub-heading (`x-subgroup-blurb`). */
  readonly blurb?: string;
  readonly fields: readonly Field[];
}

/** One section of the window, with its fields split into sub-headings. */
export interface ResearchGroup {
  readonly id: string;
  readonly name: string;
  readonly blurb: string;
  readonly subs: readonly ResearchSub[];
  readonly count: number;
}

/** The sub-heading a field sits under inside its section: the scenario key below the top one. */
export function subKeyOf(f: Field, section: ResearchSection | undefined): string {
  const parts = fieldPath(f).split(".").map((p) => p.replace(/\[\]$/, ""));
  // The engine's SCMS sub-headings (`x-subgroup`, 08-settings-ux-and-gaps §3) win in the SCMS
  // section: their order and help live with the engine.
  if (section?.id === "scms" && f.subgroup) return `sg:${f.subgroup.id}`;
  if (section?.id === "scms") {
    // `security.scms.lifecycle.*` under "lifecycle", `security.scms.pki_view` under General,
    // `security.pseudonym_change.*` under its own key, the backend under "backend".
    if (parts[0] === "security" && parts[1] === "scms") return parts.length >= 4 ? parts[2] : "";
    if (parts[0] === "security") return parts[1] ?? "";
    if (parts[0] === "detection") return "misbehaviour_authority";
    return parts[1] ?? parts[0];
  }
  if (parts.length >= 3 && section?.home === parts[0]) return parts[1];
  const top = parts[0] ?? f.group;
  return section?.home === top ? "" : top;
}

/**
 * The window's sections with the fields given, in research order then any engine group no rule
 * claimed. Inside a section the engine's field order is kept per sub-heading, and the section's
 * own general fields (`radio.rat`) come first.
 */
export function buildResearchTree(fields: readonly Field[]): ResearchGroup[] {
  const buckets = new Map<string, Field[]>();
  const extra: { name: string; fields: Field[] }[] = [];
  for (const f of fields) {
    const id = researchSectionOf(f);
    if (id === null) {
      let e = extra.find((x) => x.name === f.group);
      if (!e) {
        e = { name: f.group, fields: [] };
        extra.push(e);
      }
      e.fields.push(f);
    } else {
      const list = buckets.get(id) ?? [];
      list.push(f);
      buckets.set(id, list);
    }
  }
  const make = (id: string, name: string, blurb: string, list: readonly Field[], section?: ResearchSection): ResearchGroup => {
    const subs: { key: string; fields: Field[] }[] = [];
    for (const f of list) {
      const key = subKeyOf(f, section);
      let s = subs.find((x) => x.key === key);
      if (!s) {
        s = { key, fields: [] };
        subs.push(s);
      }
      s.fields.push(f);
    }
    subs.sort((a, b) => Number(b.key === "") - Number(a.key === ""));
    // The engine's sub-headings in the engine's order, after the section's general fields.
    const sg = (key: string): Subgroup | undefined => (key.startsWith("sg:") ? list.find((f) => `sg:${f.subgroup?.id}` === key)?.subgroup : undefined);
    const rank = (key: string): number => (key === "" ? -1 : sg(key)?.order ?? 1000);
    subs.sort((a, b) => rank(a.key) - rank(b.key));
    return {
      id: `group-${id}`,
      name,
      blurb,
      count: list.length,
      subs: subs.map((s) => {
        const g = sg(s.key);
        return {
          key: s.key,
          label: g ? g.label : s.key === "" ? "General" : humanise(s.key),
          ...(g?.blurb ? { blurb: g.blurb } : {}),
          fields: s.fields,
        };
      }),
    };
  };
  const out: ResearchGroup[] = [];
  for (const s of RESEARCH_SECTIONS) {
    const list = buckets.get(s.id);
    if (list && list.length > 0) out.push(make(s.id, s.name, s.blurb, list, s));
  }
  // An engine group no rule claimed keeps its own section; its id never collides with a
  // research section's (a React key and an anchor both use it).
  const taken = new Set(RESEARCH_SECTIONS.map((s) => s.id));
  for (const e of extra) {
    const id = taken.has(slug(e.name)) ? `engine-${slug(e.name)}` : slug(e.name);
    taken.add(id);
    out.push(make(id, e.name, "", e.fields));
  }
  return out;
}

/**
 * Why a field does not apply to the document as it stands (`x-relevant-when`), or `undefined` when
 * it does: "Applies only when Failure mode is Slow". The field is drawn greyed with this sentence,
 * never hidden, and its value is kept (08-settings-ux-and-gaps §4.2 item 5).
 *
 * The condition names a field at a fixed pointer; a condition inside a list element is not drawn
 * here (the loader still refuses the combination).
 */
export function irrelevantReason(f: Field, doc: unknown, fields: readonly Field[]): string | undefined {
  const w = f.relevantWhen;
  if (!w) return undefined;
  const target = `/${w.path.split(".").join("/")}`;
  const value = getPointer(doc, target);
  const is = (x: unknown): boolean => x === value || ((value === undefined || value === null) && x === null);
  const of = fields.find((x) => x.pointer === target);
  const name = of?.label ?? w.path;
  if (w.notIn) return w.notIn.some(is) ? `Applies only when ${name} is set.` : undefined;
  const holds = w.in.some(is);
  if (holds) return undefined;
  const words = w.in
    .map((x) => of?.choices?.find((c) => c.value === x)?.label ?? (x === null ? "unset" : String(x)))
    .join(" or ");
  return `Applies only when ${name} is ${words}.`;
}

/**
 * What a row's control is, for `data-kind` on the row (08-settings-ux-and-gaps §4.2 item 9): the
 * check that keeps every SCMS setting on a menu reads it, so it names what the reader sees.
 */
export function controlKind(f: Field): string {
  switch (f.kind) {
    case "enum":
      return "select";
    case "boolean":
      return "toggle";
    case "number":
    case "integer":
      return "number";
    case "multi":
      return "checks";
    case "list":
      return "list";
    case "map":
      return f.keyChoices ? "map" : "json";
    case "pair":
      return "pair";
    case "json":
      return "json";
    default:
      return "text";
  }
}

/** The engine's US-system verdict on a `scenario.validate` answer (`us_system`), when it gave one. */
export function usSystemOf(res: unknown): { usSystem?: { isUs: boolean; reasons: string[] } } {
  const u = (res as { us_system?: unknown } | null)?.us_system;
  if (u === null || typeof u !== "object") return {};
  const o = u as { is_us?: unknown; reasons?: unknown };
  if (typeof o.is_us !== "boolean") return {};
  const reasons = Array.isArray(o.reasons) ? o.reasons.filter((r): r is string => typeof r === "string") : [];
  return { usSystem: { isUs: o.is_us, reasons } };
}

/** The first sentence of an explanation, for the one line under a control. */
export function oneLine(text: string | undefined): string {
  if (!text) return "";
  const t = text.replace(/[`*]/g, "").replace(/\s+/g, " ").trim();
  const m = /^(.+?[.!?])(\s|$)/.exec(t);
  const first = m ? m[1] : t;
  return first.length > 160 ? `${first.slice(0, 157)}…` : first;
}
