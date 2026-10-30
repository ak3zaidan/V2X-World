/**
 * The keys of the parameter blocks the engine reads as JSON but checks key by key, so the window
 * can show one box per key instead of a JSON text box. Each table is the engine's parser, read off
 * its source (the file named on each), and a key the parser does not read is never written: what a
 * block already holds beyond these is kept and shown as it is. Change them there, then here.
 */

import type { Choice } from "../lib/schema.js";

export interface ParamSpec {
  readonly key: string;
  readonly label: string;
  readonly unit?: string;
  readonly kind: "number" | "integer" | "position" | "choice";
  readonly choices?: readonly Choice[];
  readonly min?: number;
  readonly max?: number;
  /** Shown only for these models (the block's sibling `id`). */
  readonly onlyFor?: readonly string[];
}

const when: ParamSpec[] = [
  { key: "from_s", label: "On from", unit: "s", kind: "number", min: 0 },
  { key: "to_s", label: "Off at", unit: "s", kind: "number", min: 0 },
];

/** `v2xw-engine` `run/jamming.rs` `JammerParams`; `path_m`, `speed_mps`, `loop_path` and `at` stay in the other keys. */
const EMITTER: ParamSpec[] = [
  { key: "position_m", label: "Position", unit: "m", kind: "position" },
  { key: "follow_node", label: "Follow node", kind: "integer", min: 0 },
  { key: "power_dbm", label: "Power", unit: "dBm", kind: "number" },
  ...when,
  { key: "period_ms", label: "Period", unit: "ms", kind: "number", min: 0, onlyFor: ["attacker/jammer/pulsed", "adjacent"] },
  { key: "duty", label: "Duty", unit: "0 to 1", kind: "number", min: 0, max: 1, onlyFor: ["attacker/jammer/pulsed", "adjacent"] },
  { key: "trigger_dbm", label: "Wakes on frames above", unit: "dBm", kind: "number", onlyFor: ["attacker/jammer/reactive"] },
];

const choice = (value: string, label: string, description: string): Choice => ({ value, label, description, available: true });

export const PARAM_SPECS: Readonly<Record<string, readonly ParamSpec[]>> = {
  "/threats/jammers/-/params": EMITTER,
  "/radio/adjacent_channel/-/params": EMITTER,
  // `v2xw-engine` `phase2.rs`, the compromised-unit attack's parameters.
  "/threats/compromised_rsu_attack/params": [
    {
      key: "kind",
      label: "What the unit does",
      kind: "choice",
      choices: [
        choice("SuppressForwardedReports", "Drops reports it should forward", "Misbehaviour reports relayed through it never reach the backend."),
        choice("PoisonForwardedReports", "Corrupts reports it forwards", "Reports relayed through it arrive altered."),
        choice("FalseCrl", "Broadcasts a false revocation list", "It puts a forged CRL on the air."),
      ],
    },
    { key: "suppress_prob", label: "Chance it drops a report", unit: "0 to 1", kind: "number", min: 0, max: 1 },
    { key: "poison_prob", label: "Chance it corrupts a report", unit: "0 to 1", kind: "number", min: 0, max: 1 },
    { key: "fabricated_entries", label: "Fake entries in its CRL", kind: "number", min: 1 },
    ...when,
  ],
};
