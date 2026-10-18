/**
 * The PKI view's tab, view and selection, and the map overlay's mode (WP-E3).
 *
 * Kept outside the panel so the map overlay keeps colouring vehicles after the full-screen panel
 * closes (the map is behind it), and so the inspector can open the panel on a vehicle.
 */

import { useEffect, useState } from "react";
import { create } from "zustand";

import { engine } from "../../state/engine.js";
import { useStudio } from "../../state/store.js";
import { GROUND_TRUTH, type PkiMapMode } from "../../lib/pkiRecords.js";

export type PkiTab = "trust" | "flows" | "vehicles" | "cases" | "crls";
/** The tabs WP-E3 draws (`RecordsTabs.tsx`); the others are WP-E2's (`../pki/`). */
export type RecordsTab = Exclude<PkiTab, "trust" | "flows">;
export const RECORDS_TABS: readonly RecordsTab[] = ["vehicles", "cases", "crls"];

const TAB_KEY = "vwp.pki.tab";

/** The tab the viewer last chose (per browser), else Trust domain. */
function rememberedTab(): PkiTab {
  try {
    const v = window.localStorage.getItem(TAB_KEY);
    if (v === "trust" || v === "flows" || v === "vehicles" || v === "cases" || v === "crls") return v;
  } catch {
    // Storage blocked: the default tab.
  }
  return "trust";
}

/** Choose a tab and remember it for this browser. */
export function chooseTab(tab: PkiTab): void {
  usePkiView.getState().set({ tab });
  try {
    window.localStorage.setItem(TAB_KEY, tab);
  } catch {
    // Storage blocked: the tab is not remembered.
  }
}

export interface PkiViewState {
  tab: PkiTab;
  /** `ground-truth`, or the entity every tab is seen as. */
  view: string;
  /** The Vehicles tab's chosen vehicle, by node id. */
  node: number | null;
  /** The Cases tab's chosen case. */
  caseId: string | null;
  /** The Revocation-lists tab's chosen CRL, by HashedId8. */
  crl: string | null;
  mapMode: PkiMapMode;
  /** Rings the map overlay last drew, for its legend. */
  tagged: number;
  set: (patch: Partial<Omit<PkiViewState, "set">>) => void;
}

export const usePkiView = create<PkiViewState>((set) => ({
  tab: rememberedTab(),
  view: GROUND_TRUTH,
  node: null,
  caseId: null,
  crl: null,
  mapMode: "off",
  tagged: 0,
  set: (patch) => set(patch),
}));

/**
 * The view to ask for, or `null` while ground truth is chosen but locked off for a blind
 * evaluation (09-ui §6): then nothing that only ground truth knows is asked for at all.
 */
export function useEffectiveView(): string | null {
  const view = usePkiView((s) => s.view);
  const locked = useStudio((s) => s.groundTruthLocked);
  return locked && view === GROUND_TRUTH ? null : view;
}

/** One `inspect.pki` answer, re-asked every `intervalMs` while `enabled`. */
export function usePki(
  params: { topic: "vehicles" | "vehicle" | "cases" | "crls" | "crl"; device?: string; hashedid8?: string; view?: string } | null,
  intervalMs = 2000,
): { data: unknown; error: string | null } {
  const [data, setData] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);
  const connection = useStudio((s) => s.connection);
  const key = params ? JSON.stringify(params) : "";
  useEffect(() => {
    if (!params) {
      setData(null);
      return undefined;
    }
    let live = true;
    const ask = async (): Promise<void> => {
      try {
        const answer = await engine.request("inspect.pki", params, { quiet: true, timeoutMs: 5000 });
        if (live) {
          setData(answer);
          setError(null);
        }
      } catch (err) {
        if (live) setError(err instanceof Error ? err.message : String(err));
      }
    };
    void ask();
    const id = intervalMs > 0 ? window.setInterval(() => void ask(), intervalMs) : 0;
    return () => {
      live = false;
      if (id) window.clearInterval(id);
    };
    // `key` stands for `params`, which callers build inline.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, intervalMs, connection]);
  return { data, error };
}

/** Open the PKI view on one vehicle (from the inspector or the HUD). */
export function openVehicle(node: number): void {
  usePkiView.getState().set({ tab: "vehicles", node });
}
