/**
 * The metrics a reader pinned: shown first on the dashboard, and the first two in the header's
 * live summary. Per browser, remembered across reloads; a private window or blocked storage just
 * starts with none.
 */

import { create } from "zustand";

const KEY = "vwp.metrics.pins";

/** What the header summarises before anything is pinned: the two numbers a V2X study leads with. */
export const DEFAULT_SUMMARY = ["pdr", "cbr"] as const;

function read(): string[] {
  try {
    const raw = localStorage.getItem(KEY);
    const list: unknown = raw === null ? [] : JSON.parse(raw);
    return Array.isArray(list) ? list.filter((x): x is string => typeof x === "string").slice(0, 64) : [];
  } catch {
    return [];
  }
}

function write(pins: readonly string[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(pins));
  } catch {
    /* storage is a convenience here */
  }
}

interface PinState {
  pins: readonly string[];
  toggle: (base: string) => void;
}

export const usePins = create<PinState>((set, get) => ({
  pins: typeof localStorage === "undefined" ? [] : read(),
  toggle: (base) => {
    const pins = get().pins.includes(base) ? get().pins.filter((p) => p !== base) : [...get().pins, base];
    write(pins);
    set({ pins });
  },
}));
