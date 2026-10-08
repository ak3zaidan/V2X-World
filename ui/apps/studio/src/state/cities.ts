/**
 * The city library as the engine lists it (`cities.list`): every real city a scenario can run on,
 * and the city the running scenario is on. A scenario says what happens and a city says where;
 * the picker (`components/ScenarioPicker.tsx`) chooses one, then the other.
 */

import { create } from "zustand";
import type { CityEntry } from "@vwp/protocol";

import { engine } from "./engine.js";

interface CitiesState {
  /** Every city, in the registry's order (grouped by state). */
  readonly cities: readonly CityEntry[];
  /** The id of the city the running scenario is on, when it is a library city. */
  readonly current: string | null;
  /** The id of the city the staged scenario is on. */
  readonly staged: string | null;
  /** The map data's attribution line. */
  readonly attribution: string;
  /** Whether the engine answered at all (an older engine has no `cities.list`). */
  readonly available: boolean;
  set: (patch: Partial<Omit<CitiesState, "set">>) => void;
}

export const useCities = create<CitiesState>((set) => ({
  cities: [],
  current: null,
  staged: null,
  attribution: "",
  available: false,
  set: (patch) => set(patch),
}));

/** Asks the engine for the library again; failures leave the picker without a city list. */
export async function refreshCities(): Promise<void> {
  try {
    const res = await engine.request("cities.list", {}, { quiet: true });
    useCities.getState().set({
      cities: res.cities ?? [],
      current: res.current ?? null,
      staged: res.staged ?? null,
      attribution: res.attribution ?? "",
      available: true,
    });
  } catch {
    useCities.getState().set({ available: false });
  }
}

/** The city with this id. */
export function cityById(cities: readonly CityEntry[], id: string | null | undefined): CityEntry | undefined {
  return id ? cities.find((c) => c.id === id) : undefined;
}

/** "Houston, TX" — what the header and the picker show for a city. */
export function cityLabel(c: CityEntry): string {
  return c.region === "europe" ? c.name : `${c.name}, ${c.state}`;
}

/**
 * The header's one-line name: the district a name gives in brackets, else the city, with the state
 * code — "New York (Midtown Manhattan)" → "Midtown Manhattan, NY", "Portland" → "Portland, OR".
 * The same rule as the agent's chips (`catalog::city_short_name`).
 */
export function cityShortLabel(c: CityEntry): string {
  const inner = /\(([^)]+)\)\s*$/.exec(c.name);
  const name = inner ? inner[1]! : c.name;
  return c.region === "europe" || !c.state ? name : `${name}, ${c.state}`;
}

/** Cities whose name, id, state or state code contains `query` (case-insensitive), in order. */
export function filterCities(cities: readonly CityEntry[], query: string): CityEntry[] {
  const q = query.trim().toLowerCase();
  if (q === "") return [...cities];
  return cities.filter((c) =>
    [c.name, c.id, c.state, c.state_name, c.description].some((f) => f.toLowerCase().includes(q)),
  );
}

/** The cities grouped by state, each group in the registry's order. */
export function groupByState(cities: readonly CityEntry[]): { state: string; cities: CityEntry[] }[] {
  const out: { state: string; cities: CityEntry[] }[] = [];
  for (const c of cities) {
    const last = out[out.length - 1];
    if (last && last.state === c.state_name) last.cities.push(c);
    else out.push({ state: c.state_name, cities: [c] });
  }
  return out;
}
