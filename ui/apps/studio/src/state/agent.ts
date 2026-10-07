/**
 * The agent panel's state: the conversation, the job in flight, and the polling that follows it.
 *
 * Its own small store rather than more fields on the studio store, so closing the panel (a sheet,
 * which unmounts) loses nothing, and so no other part of the page re-renders on an agent event.
 *
 * A job is followed by polling `GET /agent/events?job=&after=` every half second until the host
 * says it is done (lib/agent.ts explains why not a stream). Every request goes to the engine
 * origin the page is connected to, where `v2xw serve` serves the agent beside `/rpc`; in
 * development Vite proxies `/agent` there.
 */

import { create } from "zustand";

import {
  AGENT_SERVE_HINT,
  mergeObservations,
  type AgentEvent,
  type AgentStatus,
  type EventsPage,
  type Observation,
  type RunSummary,
  type SummaryPage,
  type WatchPage,
} from "../lib/agent.js";
import { useStudio } from "./store.js";

export interface Turn {
  readonly id: number;
  /** What the person asked, or what the button did. */
  readonly prompt: string;
  readonly kind: "ask" | "run" | "analyse";
  readonly job: number | null;
  readonly events: readonly AgentEvent[];
  readonly running: boolean;
  /** A failure to start or follow the job (the job's own errors arrive as events). */
  readonly error: string | null;
}

interface AgentState {
  status: AgentStatus | null;
  /** Why the status could not be read: the engine is not `v2xw serve`, or it is unreachable. */
  statusError: string | null;
  turns: Turn[];
  draft: string;
  setDraft: (s: string) => void;
  refreshStatus: () => Promise<void>;
  ask: (prompt: string) => Promise<void>;
  runScenario: (base: string) => Promise<void>;
  analyseCurrent: () => Promise<void>;
  stop: () => Promise<void>;
  reset: () => Promise<void>;
}

const POLL_MS = 500;

function url(path: string): string {
  const base = useStudio.getState().target.baseUrl;
  return `${base && base !== window.location.origin ? base.replace(/\/$/, "") : ""}${path}`;
}

async function call<T>(path: string, init?: RequestInit): Promise<{ ok: boolean; status: number; body: T | { error?: string } }> {
  const r = await fetch(url(path), {
    ...init,
    headers: { "content-type": "application/json", ...(init?.headers ?? {}) },
  });
  let body: unknown = null;
  try {
    body = await r.json();
  } catch {
    body = { error: `HTTP ${r.status}` };
  }
  return { ok: r.ok, status: r.status, body: body as T };
}

let nextTurn = 1;

export const useAgent = create<AgentState>((set, get) => {
  const patch = (id: number, f: (t: Turn) => Partial<Turn>): void =>
    set((s) => ({ turns: s.turns.map((t) => (t.id === id ? { ...t, ...f(t) } : t)) }));

  /** Follows one job to its end. */
  const follow = async (id: number, job: number): Promise<void> => {
    let after = 0;
    for (;;) {
      let page: EventsPage;
      try {
        const r = await call<EventsPage>(`/agent/events?job=${job}&after=${after}`);
        if (!r.ok) {
          patch(id, () => ({ running: false, error: (r.body as { error?: string }).error ?? `HTTP ${r.status}` }));
          return;
        }
        page = r.body as EventsPage;
      } catch (e) {
        patch(id, () => ({ running: false, error: `Lost the agent: ${String(e)}` }));
        return;
      }
      if (page.events.length > 0) {
        after = page.next;
        patch(id, (t) => ({ events: [...t.events, ...page.events] }));
      }
      const done = page.events.some((e) => e.kind === "done");
      if (done || !page.running) {
        // One more read after `running` drops, so the last events are not missed.
        if (!done) {
          const r = await call<EventsPage>(`/agent/events?job=${job}&after=${after}`).catch(() => null);
          const tail = r && r.ok ? (r.body as EventsPage).events : [];
          if (tail.length > 0) patch(id, (t) => ({ events: [...t.events, ...tail] }));
        }
        patch(id, () => ({ running: false }));
        void get().refreshStatus();
        return;
      }
      await new Promise((r) => setTimeout(r, POLL_MS));
    }
  };

  const start = async (kind: Turn["kind"], prompt: string, path: string, body: unknown): Promise<void> => {
    const id = nextTurn++;
    set((s) => ({ turns: [...s.turns, { id, prompt, kind, job: null, events: [], running: true, error: null }] }));
    try {
      const r = await call<{ job: number }>(path, { method: "POST", body: JSON.stringify(body) });
      if (!r.ok) {
        patch(id, () => ({ running: false, error: (r.body as { error?: string }).error ?? `HTTP ${r.status}` }));
        return;
      }
      const job = (r.body as { job: number }).job;
      patch(id, () => ({ job }));
      await follow(id, job);
    } catch (e) {
      patch(id, () => ({ running: false, error: `The agent could not be reached: ${String(e)}` }));
    }
  };

  return {
    status: null,
    statusError: null,
    turns: [],
    draft: "",
    setDraft: (draft) => set({ draft }),
    refreshStatus: async () => {
      try {
        const r = await call<AgentStatus>("/agent/status");
        if (r.ok) set({ status: r.body as AgentStatus, statusError: null });
        else
          set({
            status: null,
            statusError:
              r.status === 404
                ? AGENT_SERVE_HINT
                : ((r.body as { error?: string }).error ?? `HTTP ${r.status}`),
          });
      } catch (e) {
        set({ status: null, statusError: `The engine could not be reached: ${String(e)}` });
      }
    },
    ask: async (prompt) => {
      const p = prompt.trim();
      if (p === "") return;
      set({ draft: "" });
      await start("ask", p, "/agent/ask", { prompt: p });
    },
    runScenario: async (base) => start("run", `Run ${base === "current" ? "the current scenario" : base} and analyse it`, "/agent/run", { base }),
    analyseCurrent: async () => start("analyse", "Analyse the run on screen", "/agent/analyse", {}),
    stop: async () => {
      await call("/agent/stop", { method: "POST", body: "{}" }).catch(() => null);
    },
    reset: async () => {
      const r = await call("/agent/reset", { method: "POST", body: "{}" }).catch(() => null);
      if (r === null || r.ok) set({ turns: [] });
    },
  };
});

/** Whether a turn is in flight. */
export function agentBusy(turns: readonly Turn[]): boolean {
  return turns.some((t) => t.running);
}

// ---------------------------------------------------------------------------------------------
// The always-on watcher. The server's agent follows every run by itself (crates/v2xw-cli
// agent_host.rs); the page only reads what it noticed. Polling starts once, when the header
// mounts, and never stops: the feed is small and a read costs the engine nothing.

interface WatchState {
  /** `null` until the first read; `false` when this engine has no agent. */
  available: boolean | null;
  /** Why not, in words. */
  unavailable: string | null;
  page: Omit<WatchPage, "observations"> | null;
  /** Every observation since the engine started, oldest first. */
  observations: Observation[];
  /** The last finished run's summary. */
  summary: RunSummary | null;
  summaryMarkdown: string;
  /** The model is still writing the summary's overview. */
  overviewPending: boolean;
  /** The highest `seq` the person has seen (the agent panel was open). */
  seenSeq: number;
  /** The last `seq` the first successful read found: older lines are not news. */
  bootSeq: number | null;
  /** Bumped when a new summary arrives, so a run-summary overlay can open itself once. */
  summaryArrivals: number;
  markSeen: () => void;
  start: () => void;
  refreshSummary: () => Promise<void>;
}

const WATCH_MS = 1500;
const WATCH_RETRY_MS = 10_000;
let watchStarted = false;

export const useAgentWatch = create<WatchState>((set, get) => {
  const summaryKey = (s: RunSummary | null): string => (s === null ? "" : `${s.run_id}#${s.generation}`);

  const poll = async (): Promise<void> => {
    let delay = WATCH_MS;
    try {
      const after = get().observations.length;
      const r = await call<WatchPage>(`/agent/watch?after=${after}`);
      if (r.status === 404) {
        set({ available: false, unavailable: AGENT_SERVE_HINT });
        delay = WATCH_RETRY_MS;
      } else if (!r.ok) {
        set({ available: false, unavailable: (r.body as { error?: string }).error ?? `HTTP ${r.status}` });
        delay = WATCH_RETRY_MS;
      } else {
        const page = r.body as WatchPage;
        const { observations, ...rest } = page;
        // A restarted engine numbers its feed from 1 again.
        const held = page.next < get().observations.length ? [] : get().observations;
        const merged = mergeObservations(held, observations);
        const wasPending = get().overviewPending;
        set((s) => ({
          available: true,
          unavailable: null,
          page: rest,
          observations: merged,
          overviewPending: page.overview_pending,
          bootSeq: s.bootSeq ?? (merged.at(-1)?.seq ?? 0),
        }));
        const want = page.summary_run_id === null ? "" : `${page.summary_run_id}#${page.summary_generation ?? 0}`;
        if (want !== "" && (want !== summaryKey(get().summary) || (wasPending && !page.overview_pending))) {
          await get().refreshSummary();
        }
      }
    } catch (e) {
      set({ available: false, unavailable: `The engine could not be reached: ${String(e)}` });
      delay = WATCH_RETRY_MS;
    }
    setTimeout(() => void poll(), delay);
  };

  return {
    available: null,
    unavailable: null,
    page: null,
    observations: [],
    summary: null,
    summaryMarkdown: "",
    overviewPending: false,
    seenSeq: 0,
    bootSeq: null,
    summaryArrivals: 0,
    markSeen: () => {
      const last = get().observations.at(-1)?.seq ?? 0;
      if (last !== get().seenSeq) set({ seenSeq: last });
    },
    start: () => {
      if (watchStarted) return;
      watchStarted = true;
      void poll();
    },
    refreshSummary: async () => {
      try {
        const r = await call<SummaryPage>("/agent/summary");
        if (!r.ok) return;
        const body = r.body as SummaryPage;
        if (body.summary === null) return;
        const fresh = summaryKey(body.summary) !== summaryKey(get().summary);
        set((s) => ({
          summary: body.summary,
          summaryMarkdown: body.markdown ?? "",
          overviewPending: body.overview_pending === true,
          summaryArrivals: fresh ? s.summaryArrivals + 1 : s.summaryArrivals,
        }));
      } catch {
        // The next poll tries again.
      }
    },
  };
});
