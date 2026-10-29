/**
 * The SCMS providers as the window edits them: one card per provider the deployment runs, each
 * with the number of servers of every role, its share of vehicles, region, trust-list state, MA
 * and revocation rule.
 *
 * Under a preset (`us-production-2025`, …) the document's `providers` list holds only the changes,
 * matched by name; under `custom` it is the providers themselves. Either way the window shows every
 * provider with its effective values, and an edit writes the smallest change: a value equal to the
 * preset's is dropped, and an entry left with nothing but its name is removed.
 *
 * What each preset deploys is the engine's (`v2xw-proto` `Deployment::preset`); the engine does not
 * publish it, so it is restated here and listed as owed in the report.
 */

import { getPointer } from "../lib/schema.js";

export const PRESET_POINTER = "/security/scms/deployment/preset";
export const PROVIDERS_POINTER = "/security/scms/deployment/providers";
export const DEFAULT_PRESET = "us-production-2025";

/** The roles every US provider runs (`US_REQUIRED_ROLES`). */
export const US_ROLES = ["root", "ica", "eca", "ra", "aca", "la1", "la2", "ma", "crlg", "pg"] as const;
/** Roles only the CAMP reference design runs. */
export const EXTRA_ROLES = ["lop", "crl_store", "crl_broadcast"] as const;

/** The limits the engine checks (`Deployment::validate`): one root and one ICA, at least one RA and ACA, at most 8. */
export function roleLimits(role: string): { min: number; max: number } {
  if (role === "root" || role === "ica") return { min: 1, max: 1 };
  if (role === "ra" || role === "aca") return { min: 1, max: 8 };
  return { min: 0, max: 8 };
}

export interface ProviderValues {
  readonly name: string;
  readonly components: Readonly<Record<string, number>>;
  readonly region: string;
  readonly fleet_share: number;
  readonly on_ctl: boolean;
  readonly ma_mode: string;
  readonly ma_review_s: number;
  readonly crl_publication: string;
}

function standard(name: string, aca: number, share: number): ProviderValues {
  const components: Record<string, number> = {};
  for (const r of US_ROLES) components[r] = r === "aca" ? aca : 1;
  return {
    name,
    components,
    region: "us-east",
    fleet_share: share,
    on_ctl: true,
    ma_mode: "manual-review",
    ma_review_s: 28_800,
    crl_publication: "sla-1-business-day",
  };
}

/** The providers a preset deploys, home provider first. */
export function presetProviders(preset: string): ProviderValues[] {
  switch (preset) {
    case "us-test-ctl-2025":
      return ["a", "b", "c", "d", "e", "f"].map((s) => ({ ...standard(`provider-${s}`, 1, 1 / 6), on_ctl: s !== "f" }));
    case "camp-reference": {
      const p = standard("provider-a", 1, 1);
      return [{ ...p, components: { ...p.components, lop: 1, crl_store: 1, crl_broadcast: 1 }, crl_publication: "cadence" }];
    }
    case "custom":
      return [];
    default:
      return [standard("provider-a", 1, 0.5), standard("provider-b", 2, 0.5)];
  }
}

/** A provider a custom deployment adds: a copy of the US home provider with no vehicles. */
export function customBase(name: string): ProviderValues {
  return { ...standard(name, 1, 0) };
}

export type Override = Record<string, unknown> & { name: string };

export function presetOf(doc: unknown): string {
  const p = getPointer(doc, PRESET_POINTER);
  return typeof p === "string" && p !== "" ? p : DEFAULT_PRESET;
}

export function overridesOf(doc: unknown): Override[] {
  const list = getPointer(doc, PROVIDERS_POINTER);
  if (!Array.isArray(list)) return [];
  return list.filter((x): x is Override => x !== null && typeof x === "object" && typeof (x as { name?: unknown }).name === "string");
}

/** A provider's values: the base, with the document's entry for it on top. */
export function effective(base: ProviderValues, o: Override | undefined): ProviderValues {
  if (!o) return base;
  const comps = o.components !== null && typeof o.components === "object" ? (o.components as Record<string, unknown>) : {};
  const components: Record<string, number> = { ...base.components };
  for (const [k, v] of Object.entries(comps)) if (typeof v === "number") components[k] = v;
  const str = (k: string, d: string): string => (typeof o[k] === "string" ? (o[k] as string) : d);
  const num = (k: string, d: number): number => (typeof o[k] === "number" ? (o[k] as number) : d);
  return {
    name: base.name,
    components,
    region: str("region", base.region),
    fleet_share: num("fleet_share", base.fleet_share),
    on_ctl: typeof o.on_ctl === "boolean" ? o.on_ctl : base.on_ctl,
    ma_mode: str("ma_mode", base.ma_mode),
    ma_review_s: num("ma_review_s", base.ma_review_s),
    crl_publication: str("crl_publication", base.crl_publication),
  };
}

/** One provider card: its effective values, its preset base and the document's entry. */
export interface ProviderCard {
  readonly base: ProviderValues;
  readonly values: ProviderValues;
  readonly entry: Override | undefined;
  readonly home: boolean;
}

/** Every provider the deployment runs, as cards. */
export function providerCards(doc: unknown): ProviderCard[] {
  const preset = presetOf(doc);
  const entries = overridesOf(doc);
  if (preset === "custom") {
    return entries.map((e, i) => {
      const base = customBase(e.name);
      return { base, values: effective(base, e), entry: e, home: i === 0 };
    });
  }
  return presetProviders(preset).map((base, i) => {
    const entry = entries.find((e) => e.name === base.name);
    return { base, values: effective(base, entry), entry, home: i === 0 };
  });
}

/** Entries naming a provider the preset does not run: the engine refuses them. */
export function strayEntries(doc: unknown): string[] {
  const preset = presetOf(doc);
  if (preset === "custom") return [];
  const names = presetProviders(preset).map((p) => p.name);
  return overridesOf(doc)
    .map((e) => e.name)
    .filter((n) => !names.includes(n));
}

/** Drop a key whose value is the base's, and an empty `components`. */
function tidy(entry: Override, base: ProviderValues, custom: boolean): Override {
  const out: Override = { name: entry.name };
  for (const [k, v] of Object.entries(entry)) {
    if (k === "name" || v === undefined) continue;
    if (k === "components" && v !== null && typeof v === "object") {
      const comps: Record<string, number> = {};
      for (const [r, n] of Object.entries(v as Record<string, unknown>)) {
        if (typeof n !== "number") continue;
        if (n === (base.components[r] ?? 0)) continue;
        comps[r] = n;
      }
      if (Object.keys(comps).length > 0) out.components = comps;
      continue;
    }
    // A custom provider keeps its share even at 0: it is what the shares add up from.
    if (!(custom && k === "fleet_share") && (base as unknown as Record<string, unknown>)[k] === v) continue;
    out[k] = v;
  }
  return out;
}

/**
 * The providers list after setting `key` (`components/aca`, `region`, …) of provider `name` to
 * `value` (`undefined` puts the base back).
 */
export function writeProvider(doc: unknown, name: string, key: string, value: unknown): Override[] | undefined {
  const preset = presetOf(doc);
  const custom = preset === "custom";
  const cards = providerCards(doc);
  const card = cards.find((c) => c.values.name === name);
  const base = card?.base ?? customBase(name);
  const entries = overridesOf(doc);
  const at = entries.findIndex((e) => e.name === name);
  const current: Override = at >= 0 ? { ...entries[at] } : { name };
  if (key.startsWith("components/")) {
    const role = key.slice("components/".length);
    const comps = { ...((current.components as Record<string, unknown> | undefined) ?? {}) };
    if (value === undefined) delete comps[role];
    else comps[role] = value;
    current.components = comps;
  } else if (value === undefined) {
    delete current[key];
  } else {
    current[key] = value;
  }
  // A review time is only read under manual review, and the engine wants the mode said with it.
  if (key === "ma_review_s" && value !== undefined) current.ma_mode = "manual-review";
  if (key === "ma_mode" && value === "automatic") delete current.ma_review_s;
  const next = tidy(current, base, custom);
  const keep = custom || Object.keys(next).length > 1;
  const out = entries.slice();
  if (at >= 0) {
    if (keep) out[at] = next;
    else out.splice(at, 1);
  } else if (keep) {
    out.push(next);
  }
  return out.length === 0 ? undefined : out;
}

/** The sum of every provider's share of vehicles. */
export function shareTotal(cards: readonly ProviderCard[]): number {
  return cards.reduce((n, c) => n + c.values.fleet_share, 0);
}

/** What is wrong with the providers, in plain words, before the engine is asked. */
export function providerProblems(cards: readonly ProviderCard[], preset: string): string[] {
  const out: string[] = [];
  if (preset === "custom" && cards.length === 0) out.push("Add at least one provider.");
  if (cards.length > 9) out.push("At most 9 providers.");
  const total = shareTotal(cards);
  if (cards.length > 0 && Math.abs(total - 1) > 1e-6) out.push(`Shares of vehicles add up to ${Math.round(total * 1000) / 10} %; they must add up to 100 %.`);
  if (cards[0] && !cards[0].values.on_ctl) out.push(`${cards[0].values.name} is the home provider and must be on the trust list.`);
  return out;
}

/** Whether the provider runs every role the US requires. */
export function isUsProvider(v: ProviderValues): boolean {
  return US_ROLES.every((r) => (v.components[r] ?? 0) >= 1) && EXTRA_ROLES.every((r) => (v.components[r] ?? 0) === 0);
}
