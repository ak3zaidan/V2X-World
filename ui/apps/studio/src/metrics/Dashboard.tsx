/**
 * The metrics dashboard: every measurement of the run, full screen, grouped by the question a
 * researcher asks — channel load, delivery and reliability, latency, security and PKI, misbehaviour
 * detection, privacy, traffic, safety applications — searchable, with favourites pinned first.
 * Click a card to expand it.
 *
 * Opened by the header's Metrics button or the M key; Escape closes an expanded chart first and
 * then the dashboard. Its state is its address (`model.ts` `MetricsRoute`): a link, a reload or the
 * agent harness can open any chart, breakdown and range directly.
 *
 * It holds up on long, dense runs because it never draws or ships more than it shows: the overview
 * is binned by the engine to 240 points per line, an expanded chart is decimated to its pixel width,
 * breakdowns are pooled by the engine, and a node ranking of thousands is paged. Nothing polls while
 * the dashboard is closed.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { CloseIcon } from "../shell/Icons.js";
import { useStudio } from "../state/store.js";
import { MetricCard } from "./Card.js";
import { Expanded } from "./Expanded.js";
import { useCatalogue, useOverview } from "./hooks.js";
import {
  cardSeries,
  familiesOf,
  formatNumber,
  matchesSearch,
  metricsHash,
  observedCount,
  parseMetricsHash,
  seriesOf,
  DASHBOARD,
  EMPTY_SERIES,
  GROUPS,
  type GroupId,
  type MetricFamily,
  type MetricsRoute,
} from "./model.js";
import { usePins } from "./pins.js";
import { noteUnits } from "./tokens.js";
import "./metrics.css";

/** The run's radio technology, from the scenario the engine is running (`radio.rat`). */
function radioTechnology(scenario: unknown): string | null {
  const radio = (scenario as { radio?: { rat?: unknown } } | null)?.radio;
  return typeof radio?.rat === "string" ? radio.rat : null;
}

/** Whether a card has anything to show, from the overview. */
function measured(f: MetricFamily, series: ReadonlyMap<string, { t: Float64Array; v: Float64Array }>, latest: ReadonlyMap<string, number | null>): boolean {
  if (f.headline !== null) return observedCount(series.get(f.headline) ?? EMPTY_SERIES) > 0;
  return f.values.some((v) => (latest.get(v.series) ?? null) !== null);
}

export function MetricsDashboard({ close }: { close: () => void }): React.JSX.Element {
  const active = useStudio((s) => s.panel === "metrics");
  const run = useStudio((s) => s.run);
  const hello = useStudio((s) => s.hello);
  const rat = useStudio((s) => radioTechnology(s.scenario));
  const profileNode = run.profile === "node";
  const pins = usePins((s) => s.pins);
  const [route, setRouteState] = useState<MetricsRoute>(() => parseMetricsHash(location.hash) ?? DASHBOARD);
  const [query, setQuery] = useState(route.q ?? "");
  const search = useRef<HTMLInputElement | null>(null);
  const grid = useRef<HTMLDivElement | null>(null);

  // The address is the state: read it when it changes from outside (a link, Back, a reload).
  useEffect(() => {
    const read = (): void => {
      const r = parseMetricsHash(location.hash);
      if (r !== null) setRouteState(r);
    };
    window.addEventListener("hashchange", read);
    window.addEventListener("popstate", read);
    return () => {
      window.removeEventListener("hashchange", read);
      window.removeEventListener("popstate", read);
    };
  }, []);
  // Opened again from the header: the address is `#metrics` and says where it was left — unless
  // the address itself names a view, which wins.
  useEffect(() => {
    if (!active) return;
    const r = parseMetricsHash(location.hash);
    if (r !== null && (r.metric !== null || r.group !== null || r.q !== null)) setRouteState(r);
    else history.replaceState(history.state, "", `${location.pathname}${location.search}${metricsHash(route)}`);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active]);

  const setRoute = useCallback((patch: Partial<MetricsRoute>) => {
    setRouteState((r) => {
      const next = { ...r, ...patch };
      if (useStudio.getState().panel === "metrics") {
        history.replaceState(history.state, "", `${location.pathname}${location.search}${metricsHash(next)}`);
      }
      return next;
    });
  }, []);

  const catalogue = useCatalogue(active);
  useEffect(() => noteUnits(catalogue.defs), [catalogue.defs]);
  const families = useMemo(() => familiesOf(catalogue.defs), [catalogue.defs]);
  // A node-profile session is refused every ground-truth metric; not asking saves a refusal each.
  const asked = useMemo(() => families.filter((f) => !(profileNode && f.groundTruth)), [families, profileNode]);
  const lineNames = useMemo(() => asked.flatMap((f) => (f.headline === null ? [] : [f.headline])), [asked]);
  const barNames = useMemo(() => asked.flatMap((f) => (f.headline === null ? cardSeries(f) : [])), [asked]);
  const expanded = route.metric === null ? null : (families.find((f) => f.base === route.metric || seriesOf(f).includes(route.metric as string)) ?? null);
  // The overview is idle while a chart is expanded: nothing on screen would show it.
  const overview = useOverview(lineNames, barNames, active && expanded === null);

  const open = useCallback(
    (f: MetricFamily) => {
      setRoute({ metric: f.base, breakdown: null, from: null, to: null });
      grid.current?.scrollTo({ top: 0 });
    },
    [setRoute],
  );
  const back = useCallback(() => setRoute({ metric: null, breakdown: null, from: null, to: null }), [setRoute]);

  // Search follows the address, lightly: the address is rewritten when typing pauses.
  useEffect(() => {
    const id = setTimeout(() => {
      if ((route.q ?? "") !== query) setRoute({ q: query === "" ? null : query });
    }, 300);
    return () => clearTimeout(id);
  }, [query, route.q, setRoute]);

  // A group named in the address is scrolled to.
  useEffect(() => {
    if (!active || route.group === null || expanded !== null) return;
    document.getElementById(`metrics-group-${route.group}`)?.scrollIntoView({ block: "start" });
  }, [active, route.group, expanded, families.length]);

  const onKeyDown = (ev: React.KeyboardEvent<HTMLDivElement>): void => {
    if (ev.key === "Escape") {
      if (expanded !== null) {
        back();
        ev.preventDefault();
      } else if (query !== "" && document.activeElement === search.current) {
        setQuery("");
        ev.preventDefault();
      }
      return;
    }
    const target = ev.target as HTMLElement;
    const typing = target.closest("input, textarea, select, [contenteditable='true']") !== null;
    if (ev.key === "/" && !typing && !ev.metaKey && !ev.ctrlKey) {
      if (expanded !== null) back();
      search.current?.focus();
      ev.preventDefault();
    }
  };

  const ready = overview.status === "ready";
  const filtered = families.filter((f) => matchesSearch(f, query));
  const pinned = pins.map((p) => filtered.find((f) => f.base === p)).filter((f): f is MetricFamily => f !== undefined);
  const byGroup = GROUPS.map((g) => {
    const list = filtered.filter((f) => f.group === g.id);
    const shown = ready ? list.filter((f) => measured(f, overview.series, overview.latest)) : list;
    const missing = ready ? list.filter((f) => !measured(f, overview.series, overview.latest)) : [];
    return { g, shown, missing };
  }).filter((x) => x.shown.length + x.missing.length > 0);
  const measuredCount = ready ? families.filter((f) => measured(f, overview.series, overview.latest)).length : null;

  const scrollToGroup = (id: GroupId | "pinned"): void => {
    if (expanded !== null) back();
    setRoute({ group: id });
    requestAnimationFrame(() => document.getElementById(`metrics-group-${id}`)?.scrollIntoView({ block: "start", behavior: "smooth" }));
  };

  const runLine = `${hello?.scenarioName ?? "run"} · ${formatNumber(run.tNs / 1e9)} s${run.tEndNs > 0 ? ` of ${formatNumber(run.tEndNs / 1e9)} s` : ""} · ${run.state}${rat ? ` · ${rat}` : ""}`;

  return (
    <div className="fullpanel-inner metrics-panel" data-testid="metrics-panel" onKeyDown={onKeyDown}>
      <div className="fullpanel-head">
        <h2>Metrics</h2>
        <span className="dim" data-testid="metrics-run">
          {runLine}
          {measuredCount !== null ? ` · ${measuredCount} of ${families.length} measured` : ""}
        </span>
        <span className="grow" />
        <input
          ref={search}
          type="search"
          className="metrics-search"
          placeholder="Search metrics  /"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            if (expanded !== null && e.target.value !== "") back();
          }}
          aria-label="Search metrics by name, definition, unit or group"
          data-testid="metrics-search"
        />
        <button type="button" className="icon-button" onClick={close} aria-label="Close the metrics" title="Close (Esc)" data-testid="metrics-close">
          <CloseIcon />
        </button>
      </div>
      <div className="metrics-layout">
        <nav className="metrics-nav" aria-label="Metric groups">
          {pinned.length > 0 ? (
            <button type="button" className="linklike nav-item" onClick={() => scrollToGroup("pinned")}>
              <span>Pinned</span>
              <span className="dim">{pinned.length}</span>
            </button>
          ) : null}
          {byGroup.map(({ g, shown, missing }) => (
            <button
              key={g.id}
              type="button"
              className={`linklike nav-item${expanded?.group === g.id ? " current" : ""}`}
              onClick={() => scrollToGroup(g.id)}
              title={g.question}
              data-testid={`metrics-nav-${g.id}`}
            >
              <span>{g.label}</span>
              <span className="dim">{ready ? `${shown.length}/${shown.length + missing.length}` : shown.length}</span>
            </button>
          ))}
        </nav>
        <div className="metrics-main" ref={grid}>
          {catalogue.status === "error" && families.length === 0 ? (
            <p className="metrics-message" data-testid="metrics-message">
              The engine did not say what it measures{catalogue.error ? ` (${catalogue.error})` : ""}. It may still be starting; this page asks again when the connection or the run changes.
            </p>
          ) : catalogue.status === "loading" && families.length === 0 ? (
            <p className="metrics-message dim">Asking the engine what it measures…</p>
          ) : families.length === 0 ? (
            <p className="metrics-message" data-testid="metrics-message">
              This engine publishes no metric catalogue, so there is nothing to lay out. A run of the real engine measures delivery, latency, channel load and more.
            </p>
          ) : expanded !== null ? (
            <Expanded f={expanded} route={route} setRoute={setRoute} active={active} onBack={back} rat={rat} />
          ) : route.metric !== null ? (
            <p className="metrics-message" data-testid="metrics-message">
              This run does not measure <code>{route.metric}</code>, so the link has nothing to open.{" "}
              <button type="button" className="small" onClick={back}>
                All metrics
              </button>
            </p>
          ) : (
            <>
              {query !== "" && filtered.length === 0 ? <p className="metrics-message dim">No metric matches “{query}”.</p> : null}
              {pinned.length > 0 ? (
                <section id="metrics-group-pinned" className="mgroup" data-testid="metrics-group-pinned">
                  <h3>Pinned</h3>
                  <div className="mgrid">
                    {pinned.map((f) => (
                      <MetricCard key={f.base} f={f} overview={overview} runState={run.state} profileNode={profileNode} onOpen={open} />
                    ))}
                  </div>
                </section>
              ) : null}
              {byGroup.map(({ g, shown, missing }) => (
                <section key={g.id} id={`metrics-group-${g.id}`} className="mgroup" data-testid={`metrics-group-${g.id}`}>
                  <h3>{g.label}</h3>
                  <p className="dim mgroup-q">{g.question}</p>
                  {shown.length > 0 ? (
                    <div className="mgrid">
                      {shown.map((f) => (
                        <MetricCard key={f.base} f={f} overview={overview} runState={run.state} profileNode={profileNode} onOpen={open} />
                      ))}
                    </div>
                  ) : null}
                  {missing.length > 0 ? (
                    <p className="mgroup-missing dim" data-testid={`metrics-missing-${g.id}`}>
                      {run.state === "finished" ? "Not measured in this run" : "No data yet"}:{" "}
                      {missing.map((f, i) => (
                        <span key={f.base}>
                          {i > 0 ? ", " : ""}
                          <button type="button" className="linklike" onClick={() => open(f)} title={f.definition}>
                            {f.label}
                          </button>
                        </span>
                      ))}
                      .
                    </p>
                  ) : null}
                </section>
              ))}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
