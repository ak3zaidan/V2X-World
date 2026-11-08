/**
 * The city picker's list logic (`src/state/cities.ts`): search by name or state, grouped by state.
 */

import { describe, expect, it } from "vitest";
import type { CityEntry } from "@vwp/protocol";

import { cityLabel, cityShortLabel, filterCities, groupByState } from "../src/state/cities.js";

describe("the header's city name", () => {
  const c = (id: string, name: string, state: string, region: string): CityEntry => ({
    id,
    name,
    state,
    state_name: state,
    region,
    role: "largest",
    bbox: [0, 0, 0.01, 0.01],
    area_km2: 2,
    utc_offset_h: 0,
    highway_preset: "urban-us",
    fetched: "2026-10-07",
    description: name,
    present: true,
    fetch: `worlds/fetch.sh ${id}`,
  });
  it("is one short line: the district in brackets, else the city, with the state code", () => {
    expect(cityShortLabel(c("manhattan", "New York (Midtown Manhattan)", "NY", "northeast"))).toBe("Midtown Manhattan, NY");
    expect(cityShortLabel(c("portland", "Portland", "OR", "west"))).toBe("Portland, OR");
    expect(cityShortLabel(c("berlin", "Berlin-Mitte", "BE", "europe"))).toBe("Berlin-Mitte");
  });
});

const city = (id: string, name: string, state: string, state_name: string, region = "south"): CityEntry => ({
  id,
  name,
  state,
  state_name,
  region,
  role: "largest",
  bbox: [0, 0, 0.01, 0.01],
  area_km2: 2.2,
  utc_offset_h: -6,
  highway_preset: `urban-us-${state.toLowerCase()}`,
  fetched: "2026-10-07",
  description: `Downtown ${name}`,
  present: true,
  fetch: `worlds/fetch.sh ${id}`,
});

const LIBRARY = [
  city("austin", "Austin", "TX", "Texas"),
  city("dallas", "Dallas", "TX", "Texas"),
  city("houston", "Houston", "TX", "Texas"),
  city("seattle", "Seattle", "WA", "Washington", "west"),
  city("berlin", "Berlin-Mitte", "BE", "Berlin, Germany", "europe"),
];

describe("the city picker's list", () => {
  it("finds a city by its name, its state's name or its state's code, ignoring case", () => {
    expect(filterCities(LIBRARY, "hous").map((c) => c.id)).toEqual(["houston"]);
    expect(filterCities(LIBRARY, "texas").map((c) => c.id)).toEqual(["austin", "dallas", "houston"]);
    expect(filterCities(LIBRARY, "wa").map((c) => c.id)).toContain("seattle");
    expect(filterCities(LIBRARY, "  ")).toHaveLength(LIBRARY.length);
    expect(filterCities(LIBRARY, "atlantis")).toEqual([]);
  });

  it("groups by state in the library's order", () => {
    const groups = groupByState(LIBRARY);
    expect(groups.map((g) => g.state)).toEqual(["Texas", "Washington", "Berlin, Germany"]);
    expect(groups[0].cities.map((c) => c.id)).toEqual(["austin", "dallas", "houston"]);
  });

  it("labels a US city with its state and a foreign one by name", () => {
    expect(cityLabel(LIBRARY[2])).toBe("Houston, TX");
    expect(cityLabel(LIBRARY[4])).toBe("Berlin-Mitte");
  });
});
