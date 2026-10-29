/**
 * The credential system's older parameter block (`security.protocol.params`): the same lifecycle
 * keys as `security.scms.lifecycle`, where a typed field wins over the block (the engine's guide:
 * "a typed field above wins over the same key here"). The window shows the block's value greyed in
 * the typed field it feeds, and moves the block into those fields in one click.
 */

import { getPointer } from "../lib/schema.js";

export const PARAMS_POINTER = "/security/protocol/params";
const LIFECYCLE = "/security/scms/lifecycle/";

/** The block's value for a lifecycle field, while the field itself is unset. */
export function inheritedValue(doc: unknown, pointer: string): unknown {
  if (!pointer.startsWith(LIFECYCLE)) return undefined;
  const key = pointer.slice(LIFECYCLE.length);
  if (key.includes("/")) return undefined;
  const params = getPointer(doc, PARAMS_POINTER);
  if (params === null || typeof params !== "object") return undefined;
  return (params as Record<string, unknown>)[key];
}

/** The block's keys that have a typed field, given the pointers the window shows. */
export function movableKeys(doc: unknown, pointers: ReadonlySet<string>): string[] {
  const params = getPointer(doc, PARAMS_POINTER);
  if (params === null || typeof params !== "object") return [];
  return Object.keys(params as Record<string, unknown>).filter((k) => pointers.has(`${LIFECYCLE}${k}`));
}

/**
 * The writes that move those keys into their fields: each field gets the block's value unless it
 * already holds one (which won anyway), and the key leaves the block. A block left empty is removed.
 */
export function moveIntoFields(doc: unknown, keys: readonly string[]): { pointer: string; value: unknown }[] {
  const params = { ...((getPointer(doc, PARAMS_POINTER) as Record<string, unknown> | undefined) ?? {}) };
  const out: { pointer: string; value: unknown }[] = [];
  for (const k of keys) {
    const ptr = `${LIFECYCLE}${k}`;
    if (getPointer(doc, ptr) === undefined) out.push({ pointer: ptr, value: params[k] });
    delete params[k];
  }
  out.push({ pointer: PARAMS_POINTER, value: Object.keys(params).length === 0 ? undefined : params });
  return out;
}
