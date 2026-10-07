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

import type { AgentEvent, AgentStatus, EventsPage } from "../lib/agent.js";
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
                ? "This engine has no agent. Start it with `v2xw serve --scenario <file>` (or put `v2xw serve --attach <engine>` in front of it) to use the agent."
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
