/**
 * One vehicle's whole history as steps: its factory setup and enrolment, every pseudonym batch
 * and top-up, each revocation-list download, each report it sent, each pseudonym change, its
 * revocation, and its radio activity (BSMs sent and heard) in ten-second blocks.
 *
 * The SCMS steps come from the engine's flow records (`inspect.flows {device}`) and the vehicle's
 * certificate record (`inspect.pki {topic: "vehicle"}`). The radio blocks come from the radio's
 * once-a-second report, which the engine sends only for the selected vehicle, so they are kept
 * here from the moment the vehicle is first selected.
 */

import type { FlowRecord, FlowRow } from "./pki.js";

export type StepKind = "setup" | "enrolment" | "download" | "topup" | "crl" | "report" | "policy" | "pseudonym" | "revoked" | "radio" | "other";

export interface HistoryStep {
  readonly id: string;
  readonly kind: StepKind;
  readonly t: number;
  readonly end: number;
  readonly title: string;
  readonly summary: string;
  /** The flow instance to open for the detail, when the step is an SCMS exchange. */
  readonly instance: string | null;
  readonly flow?: FlowRow;
  readonly pseudonym?: string;
  readonly radio?: RadioBlock;
  /** The step's own messages, when it is known without a flow instance (the factory steps). */
  readonly records?: readonly FlowRecord[];
}

/** The kinds as the filter chips name them, in order. */
export const KIND_INFO: Readonly<Record<StepKind, { readonly label: string; readonly color: string }>> = {
  setup: { label: "Factory setup", color: "#8b9cff" },
  enrolment: { label: "Enrolment", color: "#8b9cff" },
  download: { label: "Certificates", color: "#4fc3a1" },
  topup: { label: "Certificates", color: "#4fc3a1" },
  crl: { label: "Revocation lists", color: "#e0b350" },
  report: { label: "Reports", color: "#ef7d6b" },
  policy: { label: "Policy", color: "#9aa4b2" },
  pseudonym: { label: "Pseudonym changes", color: "#5fb2ff" },
  revoked: { label: "Revocation", color: "#ef5350" },
  radio: { label: "BSMs", color: "#7d8796" },
  other: { label: "Other", color: "#9aa4b2" },
};

const FLOW_STEP: Readonly<Record<string, { readonly kind: StepKind; readonly title: string }>> = {
  preload: { kind: "setup", title: "Factory and dealer setup: enrolled and given its first certificates" },
  enrolment: { kind: "enrolment", title: "Got its enrolment certificate (long-term identity)" },
  reenrolment: { kind: "enrolment", title: "Renewed its enrolment certificate" },
  provisioning: { kind: "download", title: "Requested and received pseudonym certificates" },
  topup: { kind: "topup", title: "Topped up its pseudonym certificates" },
  "crl-distribution": { kind: "crl", title: "Downloaded a revocation list" },
  report: { kind: "report", title: "Sent a misbehaviour report" },
  "policy-distribution": { kind: "policy", title: "Received a new policy file" },
};

/** One second of the radio's report for the vehicle. */
export interface RadioSample {
  readonly t: number;
  readonly sent: number;
  readonly heard: number;
  readonly fullCert: number;
  readonly digestMisses: number;
  readonly neighbours: number;
  readonly verified: number;
  readonly revokedNeighbours: number;
  readonly cbr: number;
}

/** Ten seconds of radio activity. */
export interface RadioBlock {
  readonly seconds: number;
  readonly sent: number;
  readonly heard: number;
  readonly fullCert: number;
  readonly digestMisses: number;
  readonly neighbours: number;
  readonly verified: number;
  readonly revokedNeighbours: number;
  readonly cbrMax: number;
}

const samples = new Map<number, RadioSample[]>();

/** Keeps one radio report for `node` (at most one per simulated second). */
export function noteRadioSample(node: number, s: RadioSample): void {
  const list = samples.get(node) ?? [];
  const last = list[list.length - 1];
  if (last && Math.floor(last.t / 1e9) === Math.floor(s.t / 1e9)) list[list.length - 1] = s;
  else if (!last || s.t > last.t) list.push(s);
  else {
    // The run was rewound: start over.
    list.length = 0;
    list.push(s);
  }
  samples.set(node, list);
}

export function radioSamples(node: number): readonly RadioSample[] {
  return samples.get(node) ?? [];
}

const BLOCK_NS = 10e9;

function radioBlocks(list: readonly RadioSample[]): HistoryStep[] {
  const by = new Map<number, RadioSample[]>();
  for (const s of list) {
    const k = Math.floor(s.t / BLOCK_NS);
    by.set(k, [...(by.get(k) ?? []), s]);
  }
  return [...by.entries()].map(([k, ss]) => {
    const sum = (f: (s: RadioSample) => number): number => ss.reduce((a, s) => a + f(s), 0);
    const last = ss[ss.length - 1];
    const block: RadioBlock = {
      seconds: ss.length,
      sent: sum((s) => s.sent),
      heard: sum((s) => s.heard),
      fullCert: sum((s) => s.fullCert),
      digestMisses: sum((s) => s.digestMisses),
      neighbours: last.neighbours,
      verified: last.verified,
      revokedNeighbours: last.revokedNeighbours,
      cbrMax: Math.max(...ss.map((s) => s.cbr)),
    };
    return {
      id: `radio-${k}`,
      kind: "radio" as const,
      t: k * BLOCK_NS,
      end: (k + 1) * BLOCK_NS,
      title: `Broadcast ${Math.round(block.sent)} BSMs, heard ${Math.round(block.heard)}`,
      summary: `${block.neighbours} vehicles around · ${block.verified} verified${block.revokedNeighbours > 0 ? ` · ${block.revokedNeighbours} revoked` : ""}`,
      instance: null,
      radio: block,
    };
  });
}

/** Every step, oldest first. */
export function historySteps(
  flows: readonly FlowRow[],
  detail: { readonly pseudonyms: readonly { readonly id8: string; readonly since: number }[]; readonly revoked: boolean; readonly t: number } | null,
  radio: readonly RadioSample[],
): HistoryStep[] {
  const steps: HistoryStep[] = [];
  for (const f of flows) {
    const k = FLOW_STEP[f.flow] ?? { kind: "other" as const, title: f.flow.replace(/[-_]+/g, " ") };
    const dur = f.last_t - f.first_t;
    steps.push({
      id: f.instance,
      kind: k.kind,
      t: f.first_t,
      end: f.last_t,
      title: k.title,
      summary: `${f.messages} message${f.messages === 1 ? "" : "s"} · ${kb(f.bytes)}${dur > 0 ? ` · took ${durText(dur)}` : ""}${f.device_count > 1 ? ` · with ${f.device_count - 1} other vehicles` : ""}`,
      instance: f.instance,
      flow: f,
    });
  }
  const ps = [...(detail?.pseudonyms ?? [])].sort((a, b) => a.since - b.since);
  ps.forEach((p, i) =>
    steps.push({
      id: `ps-${p.id8}`,
      kind: "pseudonym",
      t: p.since,
      end: p.since,
      title: i === 0 ? "Started signing with a pseudonym certificate" : "Changed to a new pseudonym certificate",
      summary: `certificate ${p.id8.slice(0, 8)}…${i > 0 ? " · its temporary ID and certificate change together" : ""}`,
      instance: null,
      pseudonym: p.id8,
    }),
  );
  if (detail?.revoked) {
    steps.push({ id: "revoked", kind: "revoked", t: detail.t, end: detail.t, title: "Found itself on a revocation list", summary: "It stops signing: other vehicles now refuse its messages", instance: null });
  }
  steps.push(...radioBlocks(radio));
  return steps.sort((a, b) => a.t - b.t || order(a.kind) - order(b.kind));
}

function order(k: StepKind): number {
  return ["setup", "enrolment", "download", "pseudonym", "topup", "crl", "report", "policy", "revoked", "radio", "other"].indexOf(k);
}

export function kb(bytes: number): string {
  return bytes >= 1024 ? `${(bytes / 1024).toFixed(1)} kB` : `${bytes} B`;
}

export function durText(ns: number): string {
  if (ns < 1e6) return `${Math.round(ns / 1e3)} µs`;
  if (ns < 1e9) return `${Math.round(ns / 1e6)} ms`;
  if (ns < 120e9) return `${(ns / 1e9).toFixed(1)} s`;
  if (ns < 7200e9) return `${(ns / 60e9).toFixed(1)} min`;
  return `${(ns / 3600e9).toFixed(1)} h`;
}

/** The run clock as m:ss (or h:mm:ss), for the step list. */
export function clockText(ns: number): string {
  const s = Math.max(0, Math.floor(ns / 1e9));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}
