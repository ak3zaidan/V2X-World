/**
 * Whether the run summary is open. Its own small store so any control (the header, the metrics
 * dashboard, the agent panel) can open it without the shell's panel route, and so it can sit over
 * whatever else is open.
 */

import { create } from "zustand";

interface SummaryState {
  open: boolean;
  /** The run key (`runId#generation`) the summary last opened itself for, so it opens once per run. */
  autoOpenedFor: string;
  setOpen: (open: boolean) => void;
  noteAutoOpened: (runKey: string) => void;
}

export const useRunSummary = create<SummaryState>((set) => ({
  open: false,
  autoOpenedFor: "",
  setOpen: (open) => set({ open }),
  noteAutoOpened: (autoOpenedFor) => set({ autoOpenedFor, open: true }),
}));

export function openRunSummary(): void {
  useRunSummary.getState().setOpen(true);
}

export function closeRunSummary(): void {
  useRunSummary.getState().setOpen(false);
}
