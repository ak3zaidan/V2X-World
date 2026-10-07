/**
 * The dashboard's data hooks: when to ask the engine, and what to keep.
 *
 * All of them are idle while the dashboard is hidden (`active` false): the panel stays mounted
 * between two looks, and a hidden dashboard polling the engine would cost a live run for nothing.
 * While a run plays they refresh at most every {@link LIVE_REFRESH_MS}; while it is paused or
 * finished they refresh when the shown time moves (a seek, a step) and otherwise not at all.
 *
 * An answer is kept unless the *inputs* changed while it was on its way. A clock tick in between
 * is not a reason to throw it away: while a run plays the clock moves every 200 ms, and a refresh
 * that took longer than that would otherwise never land.
 */

import { useEffect, useRef, useState } from "react";

import { useStudio } from "../state/store.js";
import { fetchCatalogue, fetchGroups, fetchSeries, metricPeriodNs, type PooledSpan, type Side } from "./data.js";
import { appendSeries, chooseBinNs, latestOf, EMPTY_SERIES, type GroupRow, type Series, type SeriesDef } from "./model.js";

/** How often a playing run's charts refresh. The engine's metric period is 1 s by default. */
export const LIVE_REFRESH_MS = 2000;

/** How many points an overview card asks the engine for, across the whole run so far. */
export const OVERVIEW_POINTS = 240;

/** Identifies one run: a new run (or `Run again`) starts every hook over. */
export function useRunKey(): string {
  return useStudio((s) => `${s.run.runId}#${s.run.generation}`);
}

/** The shown simulated time in ns, and whether the run is playing. */
function useClock(): { tNs: number; playing: boolean } {
  const tNs = useStudio((s) => s.run.tNs);
  const playing = useStudio((s) => s.run.state === "running" || s.run.state === "seeking");
  return { tNs, playing };
}

/**
 * Whether a refresh is due: the inputs changed, or the clock moved and (while playing) the last
 * refresh is at least {@link LIVE_REFRESH_MS} old. Kept in a ref so it survives re-renders.
 */
function useDue(key: string, active: boolean): (tNs: number, playing: boolean) => boolean {
  const last = useRef<{ key: string; tNs: number; wall: number }>({ key: "", tNs: -1, wall: 0 });
  return (tNs: number, playing: boolean): boolean => {
    if (!active) return false;
    const now = Date.now();
    const l = last.current;
    const due = l.key !== key || (l.tNs !== tNs && (!playing || now - l.wall >= LIVE_REFRESH_MS));
    if (due) last.current = { key, tNs, wall: now };
    return due;
  };
}

/** A ref that always holds the latest value, for "are these inputs still current?" checks. */
function useCurrent<T>(value: T): { readonly current: T } {
  const ref = useRef(value);
  ref.current = value;
  return ref;
}

export type LoadState = "loading" | "ready" | "error";

/** The run's catalogue, fetched once per run and again if it came back empty. */
export function useCatalogue(active: boolean): { status: LoadState; defs: SeriesDef[]; error: string | null } {
  const runKey = useRunKey();
  const connection = useStudio((s) => s.connection);
  const runState = useStudio((s) => s.run.state);
  const [state, setState] = useState<{ status: LoadState; defs: SeriesDef[]; error: string | null }>({
    status: "loading",
    defs: [],
    error: null,
  });
  const have = useRef<string>("");
  const current = useCurrent(runKey);
  useEffect(() => {
    if (!active) return;
    // One good answer per run is enough: the catalogue is the scenario's, fixed for the run.
    if (have.current === runKey) return;
    const requested = runKey;
    fetchCatalogue()
      .then((defs) => {
        if (current.current !== requested) return;
        if (defs.length > 0) have.current = requested;
        setState({ status: "ready", defs, error: null });
      })
      .catch((err: unknown) => {
        if (current.current !== requested) return;
        setState((s) => ({ ...s, status: s.defs.length > 0 ? "ready" : "error", error: err instanceof Error ? err.message : String(err) }));
      });
    // `connection` and `runState` re-ask after a failure: the engine may simply not have been up.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, runKey, connection, runState]);
  return state;
}

export interface Overview {
  /** Each card series across the run so far, binned by the engine to about {@link OVERVIEW_POINTS}. */
  readonly series: ReadonlyMap<string, Series>;
  /** Each series' value in its newest window, at the engine's own period. */
  readonly latest: ReadonlyMap<string, number | null>;
  /** Series the engine refused (a ground-truth metric in a node-profile session). */
  readonly refused: ReadonlySet<string>;
  /** The bin the overview was drawn at, in seconds. */
  readonly binS: number;
  /** The simulated time the overview reaches, in seconds. */
  readonly toS: number;
  readonly status: LoadState;
}

const EMPTY_OVERVIEW: Overview = { series: new Map(), latest: new Map(), refused: new Set(), binS: 1, toS: 0, status: "loading" };

/**
 * The overview cards' series. One call for the lines (`names`, across the run so far), one for the
 * newest windows of those and of `latestOnly` (a breakdown-only metric's per-value series, which a
 * card shows as bars of their newest values and never as lines).
 */
export function useOverview(names: readonly string[], latestOnly: readonly string[], active: boolean): Overview {
  const runKey = useRunKey();
  const { tNs, playing } = useClock();
  const [overview, setOverview] = useState<Overview>(EMPTY_OVERVIEW);
  const key = `${runKey}|${names.join(",")}|${latestOnly.join(",")}`;
  const due = useDue(key, active && names.length + latestOnly.length > 0);
  const inFlight = useRef(false);
  const current = useCurrent(key);
  // Bumped when an answer lands for inputs that changed meanwhile, so the new inputs are asked
  // for even if nothing else moves (a paused run).
  const [kick, setKick] = useState(0);

  useEffect(() => {
    if (inFlight.current || !due(tNs, playing)) return;
    inFlight.current = true;
    const requested = key;
    const period = metricPeriodNs();
    const spanS = tNs / 1e9;
    const binNs = chooseBinNs(spanS, period, OVERVIEW_POINTS);
    const newest = [...names, ...latestOnly];
    void (async () => {
      try {
        const [whole, tail] = await Promise.all([
          fetchSeries(names, { fromNs: 0, toNs: tNs, binNs, limit: OVERVIEW_POINTS * 2 + 4 }),
          // The newest windows at the engine's own period, so the headline number is a window's
          // value and not the mean of the last overview bin.
          fetchSeries(newest, { fromNs: Math.max(0, tNs - 4 * period), toNs: tNs, binNs: period, limit: 8 }),
        ]);
    if (current.current !== requested) return;
        const latest = new Map<string, number | null>();
        for (const n of newest) {
          latest.set(n, latestOf(tail.series.get(n) ?? EMPTY_SERIES) ?? latestOf(whole.series.get(n) ?? EMPTY_SERIES));
        }
        const refused = new Set([...whole.refused, ...tail.refused]);
        setOverview({ series: whole.series, latest, refused, binS: binNs / 1e9, toS: spanS, status: "ready" });
      } catch {
        if (current.current === requested) setOverview((o) => ({ ...o, status: o.series.size > 0 ? "ready" : "error" }));
      } finally {
        inFlight.current = false;
        if (current.current !== requested) setKick((k) => k + 1);
      }
    })();
    // `due` is a fresh closure each render over refs; `tNs` and `key` are what trigger.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tNs, key, active, playing, kick]);

  // A new run: drop the previous run's numbers at once rather than showing them under its name.
  useEffect(() => setOverview(EMPTY_OVERVIEW), [runKey]);
  return overview;
}

/**
 * Series at the engine's full resolution, from the start of the run, fetched incrementally and
 * kept while the inputs stay the same. What the expanded chart's statistics, table and CSV use.
 */
export function useFullSeries(
  names: readonly string[],
  active: boolean,
  side: Side = "a",
): { series: ReadonlyMap<string, Series>; refused: ReadonlySet<string>; status: LoadState } {
  const runKey = useRunKey();
  const clock = useClock();
  const compareT = useStudio((s) => s.compare?.tNs ?? 0);
  const compareLabel = useStudio((s) => s.compare?.label ?? "");
  const tNs = side === "a" ? clock.tNs : compareT;
  const playing = side === "a" ? clock.playing : false;
  const key = `${side}|${side === "a" ? runKey : compareLabel}|${names.join(",")}`;
  const store = useRef<{ key: string; series: Map<string, Series>; refused: Set<string>; lastS: number }>({
    key: "",
    series: new Map(),
    refused: new Set(),
    lastS: 0,
  });
  const [version, setVersion] = useState(0);
  const [status, setStatus] = useState<LoadState>("loading");
  const due = useDue(key, active && names.length > 0);
  const inFlight = useRef(false);
  const current = useCurrent(key);
  const [kick, setKick] = useState(0);

  useEffect(() => {
    if (inFlight.current || !due(tNs, playing)) return;
    const s = store.current;
    // A new key, or the clock went back (a seek into the past, a new run on this socket): what is
    // held is another timeline, so start again from zero.
    const fresh = s.key !== key || tNs / 1e9 < s.lastS;
    if (fresh) {
      store.current = { key, series: new Map(), refused: new Set(), lastS: 0 };
      setStatus("loading");
    }
    const from = fresh ? 0 : store.current.lastS;
    inFlight.current = true;
    const period = metricPeriodNs(side);
    void fetchSeries(names, { fromNs: from * 1e9, ...(side === "a" ? { toNs: tNs } : {}), binNs: period, limit: 1_000_000 }, side)
      .then(({ series, refused }) => {
        if (store.current.key !== key) return;
        const target = store.current;
        let last = target.lastS;
        for (const n of names) {
          const more = series.get(n) ?? EMPTY_SERIES;
          target.series.set(n, appendSeries(target.series.get(n) ?? EMPTY_SERIES, more));
          if (more.t.length > 0) last = Math.max(last, more.t[more.t.length - 1]);
        }
        for (const r of refused) target.refused.add(r);
        target.lastS = last;
        setStatus("ready");
        setVersion((v) => v + 1);
      })
      .catch(() => {
        if (store.current.key === key) setStatus(store.current.series.size > 0 ? "ready" : "error");
      })
      .finally(() => {
        inFlight.current = false;
        if (current.current !== key) setKick((k) => k + 1);
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tNs, key, active, playing, kick]);

  const snapshot = useVersioned(store.current, version);
  return { series: snapshot.key === key ? snapshot.series : new Map(), refused: snapshot.refused, status };
}

/** A copy of the store's maps per fetch, so a consumer's memo sees new data as a new identity. */
function useVersioned(
  value: { key: string; series: Map<string, Series>; refused: Set<string> },
  version: number,
): { key: string; series: ReadonlyMap<string, Series>; refused: ReadonlySet<string> } {
  const ref = useRef<{ version: number; key: string; copy: { key: string; series: ReadonlyMap<string, Series>; refused: ReadonlySet<string> } } | null>(null);
  if (ref.current === null || ref.current.version !== version || ref.current.key !== value.key) {
    ref.current = { version, key: value.key, copy: { key: value.key, series: new Map(value.series), refused: new Set(value.refused) } };
  }
  return ref.current.copy;
}

/** `value`, once it has stopped changing for `ms`. */
export function useDebounced<T>(value: T, ms: number): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setSettled(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return settled;
}

/**
 * One grouped breakdown over a range. `toNs` omitted follows the run's end live. Debounced, so a
 * brush being dragged asks once when it settles, not on every pointer move; while the new answer
 * is on its way the previous one stays on screen, marked `stale`.
 */
export function useGroups(
  metric: string | null,
  dim: string,
  range: { fromNs: number; toNs?: number },
  where: Readonly<Record<string, string>>,
  active: boolean,
  side: Side = "a",
): { rows: GroupRow[]; pooled: PooledSpan | null; status: LoadState; error: string | null; stale: boolean } {
  const runKey = useRunKey();
  const { tNs, playing } = useClock();
  const [state, setState] = useState<{ key: string; rows: GroupRow[]; pooled: PooledSpan | null; status: LoadState; error: string | null }>({
    key: "",
    rows: [],
    pooled: null,
    status: "loading",
    error: null,
  });
  const whereKey = JSON.stringify(where);
  const key = `${runKey}|${side}|${metric}|${dim}|${range.fromNs}|${range.toNs ?? "live"}|${whereKey}`;
  const settled = useDebounced(key, 200);
  const follows = range.toNs === undefined;
  const due = useDue(settled, active && metric !== null && settled === key);
  const current = useCurrent(key);
  const clock = follows ? tNs : 0;
  useEffect(() => {
    // A fixed range answers once; a live one refreshes as the run moves.
    if (metric === null || settled !== key || !due(clock, playing)) return;
    const requested = key;
    fetchGroups(metric, dim, range, where, side)
      .then(({ rows, pooled }) => {
        if (current.current === requested) setState({ key: requested, rows, pooled, status: "ready", error: null });
      })
      .catch((err: unknown) => {
        if (current.current === requested) {
          setState({ key: requested, rows: [], pooled: null, status: "error", error: err instanceof Error ? err.message : String(err) });
        }
      });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [settled, key, clock, active]);
  return { rows: state.rows, pooled: state.pooled, status: state.status, error: state.error, stale: state.key !== key };
}
