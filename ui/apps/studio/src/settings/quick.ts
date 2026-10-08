/**
 * The quick settings' logic, apart from React: which published choice the form currently holds,
 * how a choice's patch lands in the draft, and how a fleet-mix toggle keeps the class shares
 * summing to one (the loader refuses any other sum, `validate.rs`).
 *
 * Every choice and every patch comes from the engine (`scenario.get` `quick`/`studies`,
 * `crates/v2xw-engine/src/scenario/guide.rs`); nothing here names a value of its own.
 */

import { getPointer, setPointer } from "../lib/schema.js";
import type { PatchWrite, QuickChoice } from "../state/store.js";
import { stableJson } from "./model.js";

/** True when every write of the patch already holds in `doc`. */
export function patchHolds(doc: unknown, patch: readonly PatchWrite[]): boolean {
  return patch.length > 0 && patch.every((w) => stableJson(getPointer(doc, w.pointer)) === stableJson(w.value));
}

/**
 * The index of the choice the document holds, or -1. The most specific match wins, so a choice
 * that sets the signature and its hardware is preferred to one that only sets the signature.
 */
export function currentChoice(doc: unknown, options: readonly QuickChoice[]): number {
  let best = -1;
  for (let i = 0; i < options.length; i++) {
    if (patchHolds(doc, options[i].patch) && (best < 0 || options[i].patch.length > options[best].patch.length)) best = i;
  }
  return best;
}

/** `doc` with every write of the patch applied. */
export function applyPatch<T>(doc: T, patch: readonly PatchWrite[]): T {
  let out = doc;
  for (const w of patch) out = setPointer(out, w.pointer, w.value);
  return out;
}

interface ClassSpec {
  readonly fraction: number;
  readonly [key: string]: unknown;
}

/** Rounded to 1e-12, well inside the loader's 1e-9 tolerance on the sum. */
function tidy(x: number): number {
  return Math.round(x * 1e12) / 1e12;
}

/** True when the fleet mix has a share for `cls`. */
export function hasClass(classes: unknown, cls: string): boolean {
  return classes !== null && typeof classes === "object" && cls in (classes as Record<string, unknown>);
}

/**
 * The fleet mix with `cls` switched on at `share` (taken from `absorb`, or from the largest class
 * when there is no `absorb`) or off (its share given back to `absorb`). An empty mix means every
 * vehicle is an `absorb` vehicle; switching the last extra class off returns to an empty mix.
 */
export function classesWith(
  classes: unknown,
  cls: string,
  share: number,
  absorb: string,
  on: boolean,
): Record<string, ClassSpec> {
  const src = classes !== null && typeof classes === "object" ? (classes as Record<string, ClassSpec>) : {};
  const wasEmpty = Object.keys(src).length === 0;
  const out: Record<string, ClassSpec> = {};
  for (const [k, v] of Object.entries(src)) out[k] = { ...v };
  if (wasEmpty) out[absorb] = { fraction: 1 };
  const donor = (): string => {
    if (out[absorb]) return absorb;
    let top = "";
    for (const [k, v] of Object.entries(out)) if (k !== cls && (top === "" || v.fraction > out[top].fraction)) top = k;
    return top;
  };
  if (on) {
    if (out[cls]) return out;
    const d = donor();
    if (d === "") return { [cls]: { fraction: 1 } };
    const take = Math.min(share, out[d].fraction);
    out[d] = { ...out[d], fraction: tidy(out[d].fraction - take) };
    if (out[d].fraction <= 0) delete out[d];
    out[cls] = { fraction: tidy(take) };
    return out;
  }
  const give = out[cls]?.fraction ?? 0;
  delete out[cls];
  if (Object.keys(out).length === 0) return {};
  const d = donor();
  out[d] = { ...(out[d] ?? { fraction: 0 }), fraction: tidy((out[d]?.fraction ?? 0) + give) };
  // Back to cars only: the empty mix says the same thing in the scenario file's own default.
  const keys = Object.keys(out);
  if (keys.length === 1 && keys[0] === absorb && Object.keys(out[absorb]).length === 1) return {};
  return out;
}

/** A run length in words: 90 → "1.5 min", 86400 → "1 day". */
export function describeSeconds(s: number): string {
  if (s >= 86_400) return `${+(s / 86_400).toFixed(1)} day${s === 86_400 ? "" : "s"}`;
  if (s >= 3_600) return `${+(s / 3_600).toFixed(1)} h`;
  if (s >= 60) return `${+(s / 60).toFixed(1)} min`;
  return `${s} s`;
}
