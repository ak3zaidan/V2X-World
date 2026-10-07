/**
 * The scenario picker: what happens, then where.
 *
 * Step 1 is a ready-made scenario — the traffic, the radio, the network and what goes wrong, by
 * its plain name and one line. Step 2 is a real city from the engine's library (`cities.list`),
 * searchable by name and state, grouped by state, each with the area its map covers. Run loads the
 * scenario on the city (`scenario.load {path, city}`) and starts it. Any scenario runs on any city:
 * the engine moves its roadside units to the city's signalised intersections, scales its demand to
 * the city's network, and keeps its local time of day (`crates/v2xw-engine/src/city.rs`).
 */

import { useEffect, useMemo, useRef, useState } from "react";
import type { CityEntry } from "@vwp/protocol";

import { engine } from "../state/engine.js";
import { cityById, cityLabel, filterCities, groupByState, refreshCities, useCities } from "../state/cities.js";
import { presetsOf } from "../state/settings.js";
import { useStudio, type ScenarioListItem } from "../state/store.js";
import { describeError } from "../lib/errors.js";

/** A scenario description's first sentence: the one line the list has room for. */
export function firstSentence(text: string): string {
  const t = text.trim();
  const m = /^(.+?[.!?])(\s|$)/.exec(t);
  return m ? m[1] : t;
}

/** The city a ready-made scenario runs on when none is chosen (`scenario.list`'s `city`). */
function defaultCity(item: ScenarioListItem | undefined): string | null {
  const c = (item as { city?: unknown } | undefined)?.city;
  return typeof c === "string" ? c : null;
}

export function ScenarioPicker({ onClose }: { onClose: () => void }): React.JSX.Element {
  const list = useStudio((s) => s.scenarioList);
  const runningHash = useStudio((s) => s.scenarioExtras.runningHash);
  const presets = presetsOf(list);
  const cities = useCities((s) => s.cities);
  const current = useCities((s) => s.current);
  const available = useCities((s) => s.available);
  const attribution = useCities((s) => s.attribution);

  const running = presets.find((p) => p.hash !== undefined && p.hash === runningHash);
  const [scenarioId, setScenarioId] = useState<string | null>(running?.id ?? null);
  const [cityId, setCityId] = useState<string | null>(current);
  const [cityTouched, setCityTouched] = useState(false);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [note, setNote] = useState<{ text: string; err: boolean } | null>(null);
  const search = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    void refreshCities();
  }, []);
  // Until the user picks a city, the chosen scenario's own default stands in.
  const scenario = presets.find((p) => p.id === scenarioId);
  // A scenario with no default city (a synthetic road, such as the highway) runs on its own road
  // network unless a city is picked.
  const ownRoad = scenario !== undefined && defaultCity(scenario) === null;
  useEffect(() => {
    if (cityTouched) return;
    setCityId(scenario ? defaultCity(scenario) : current);
  }, [scenario, cityTouched, current]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const shown = useMemo(() => groupByState(filterCities(cities, query)), [cities, query]);
  const city = cityById(cities, cityId);

  const go = (andRun: boolean): void => {
    if (!scenario) return;
    setBusy(true);
    setNote(null);
    const params: { path: string; city?: string; validate: boolean } = { path: scenario.id, validate: true };
    if (city) params.city = city.id;
    void engine
      .request("scenario.load", params)
      .then(async (res) => {
        if (!res.valid) {
          useStudio.getState().setValidation({ valid: false, errors: res.errors ?? [], warnings: [] });
          throw new Error(`${scenario.name ?? scenario.id} does not load on ${city ? cityLabel(city) : "this city"}: each problem is marked in Settings.`);
        }
        useStudio.getState().setDraft(null);
        await engine.refreshScenario();
        if (andRun) {
          await engine.startRun();
          await refreshCities();
          onClose();
        } else {
          await refreshCities();
          setNote({ text: `Loaded ${scenario.name ?? scenario.id}${city ? ` on ${cityLabel(city)}` : ""}. Press Run to start it.`, err: false });
        }
      })
      .catch((err: unknown) => setNote({ text: describeError(err), err: true }))
      .finally(() => setBusy(false));
  };

  const cityRow = (c: CityEntry): React.JSX.Element => (
    <button
      key={c.id}
      type="button"
      role="option"
      aria-selected={c.id === cityId}
      className={c.id === cityId ? "picker-row on" : "picker-row"}
      disabled={!c.present}
      data-testid="picker-city"
      data-city={c.id}
      title={c.present ? `${c.description} — ${c.area_km2.toFixed(1)} km², ${c.highway_preset}` : `Not downloaded on the engine's machine: ${c.fetch}`}
      onClick={() => {
        setCityId(c.id);
        setCityTouched(true);
      }}
    >
      <span className="picker-row-title">
        <span className="grow">{c.name}</span>
        {c.id === current ? <span className="faint">running</span> : null}
        {!c.present ? <span className="faint">not downloaded</span> : null}
      </span>
      <span className="picker-row-desc">{c.description}</span>
    </button>
  );

  return (
    <div className="picker-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="picker" role="dialog" aria-modal="true" aria-label="Choose a scenario and a city" data-testid="scenario-picker">
        <div className="picker-head">
          <span className="picker-title">New run</span>
          <span className="grow" />
          <button type="button" className="icon-button" onClick={onClose} title="Close (Esc)" aria-label="Close">
            ✕
          </button>
        </div>
        <div className="picker-body">
          <section className="picker-col" aria-label="Scenario">
            <div className="picker-step">
              <span className="step-num">1</span> Scenario <span className="faint">— what happens</span>
            </div>
            <div className="picker-list" role="listbox" aria-label="Scenarios" data-testid="picker-scenarios">
              {presets.length === 0 ? <div className="dim menu-note">This engine lists no ready-made scenarios.</div> : null}
              {presets.map((item, i) => (
                <div key={item.id}>
                  {item.group && item.group !== presets[i - 1]?.group ? <div className="sec group">{item.group}</div> : null}
                  <button
                    type="button"
                    role="option"
                    aria-selected={item.id === scenarioId}
                    className={item.id === scenarioId ? "picker-row on" : "picker-row"}
                    data-testid="picker-scenario"
                    data-preset={item.name ?? item.id}
                    onClick={() => {
                      setScenarioId(item.id);
                      search.current?.focus();
                    }}
                  >
                    <span className="picker-row-title">
                      <span className="grow">{item.name ?? item.id}</span>
                      {item === running ? <span className="faint">running</span> : null}
                    </span>
                    {item.description ? <span className="picker-row-desc">{firstSentence(item.description)}</span> : null}
                  </button>
                </div>
              ))}
            </div>
          </section>
          <section className={scenario ? "picker-col" : "picker-col waiting"} aria-label="City">
            <div className="picker-step">
              <span className="step-num">2</span> City <span className="faint">— where</span>
            </div>
            {!available ? (
              <div className="dim menu-note">This engine has no city library; the scenario runs on the world it names.</div>
            ) : (
              <>
                <input
                  ref={search}
                  className="picker-search"
                  type="search"
                  placeholder={`Search ${cities.length} cities by name or state`}
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  data-testid="picker-city-search"
                  aria-label="Search cities"
                />
                <div className="picker-list" role="listbox" aria-label="Cities" data-testid="picker-cities">
                  {ownRoad ? (
                    <button
                      type="button"
                      role="option"
                      aria-selected={cityId === null}
                      className={cityId === null ? "picker-row on" : "picker-row"}
                      data-testid="picker-own-road"
                      onClick={() => {
                        setCityId(null);
                        setCityTouched(true);
                      }}
                    >
                      <span className="picker-row-title">The scenario's own road</span>
                      <span className="picker-row-desc">The synthetic network this scenario was built on, rather than a real city.</span>
                    </button>
                  ) : null}
                  {shown.length === 0 ? <div className="dim menu-note">No city matches “{query}”.</div> : null}
                  {shown.map((g) => (
                    <div key={g.state}>
                      <div className="sec group">{g.state}</div>
                      {g.cities.map(cityRow)}
                    </div>
                  ))}
                </div>
              </>
            )}
          </section>
        </div>
        <div className="picker-foot">
          <div className="picker-summary" data-testid="picker-summary">
            {scenario ? (
              <>
                <b>{scenario.name ?? scenario.id}</b>
                {city ? (
                  <>
                    {" "}on <b>{cityLabel(city)}</b>
                    <span className="faint"> · {city.description}</span>
                  </>
                ) : null}
              </>
            ) : (
              <span className="faint">Choose a scenario, then a city.</span>
            )}
            {note ? (
              <div className={note.err ? "err-text" : "dim"} role="status" data-testid="picker-message">
                {note.text}
              </div>
            ) : null}
          </div>
          <span className="grow" />
          <button type="button" className="button" disabled={!scenario || busy} onClick={() => go(false)} data-testid="picker-load" title="Load it; the run on screen keeps playing until you press Run">
            Load
          </button>
          <button type="button" className="button primary" disabled={!scenario || busy} onClick={() => go(true)} data-testid="picker-run">
            {busy ? "Starting…" : "Run"}
          </button>
        </div>
        {attribution ? <div className="picker-attrib faint">Map data {attribution}</div> : null}
      </div>
    </div>
  );
}
