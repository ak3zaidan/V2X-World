/**
 * One name per vehicle across the studio. The map, the HUD and the camera chip know a vehicle by
 * its simulation node ("Vehicle 66"); the certificate records know it by its SCMS device id
 * (`ee/1000066`). Every PKI view names a vehicle by its node, as the HUD does, with the device id as
 * a small second line or a tooltip, so a vehicle on the map, its row in Vehicles and the accused in
 * a case read the same.
 *
 * The device-to-node pairs come from the engine's own answers (`inspect.pki` vehicles, case
 * parties and CRL owners carry both); nothing is guessed from the numbers. A device whose node no
 * answer has named yet (a vehicle of another provider that never drove here) is named by its
 * device id and says so.
 *
 * Framework- and DOM-free (`test/pki-names.test.ts`).
 */

const nodeByDevice = new Map<string, number>();
const deviceByNode = new Map<number, string>();

function isObj(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function note(device: unknown, node: unknown): void {
  if (typeof device !== "string" || !/^(ee|obu)\//.test(device)) return;
  if (typeof node !== "number" || !Number.isFinite(node)) return;
  nodeByDevice.set(device, node);
  deviceByNode.set(node, device);
}

/**
 * Remember every (device, node) pair an `inspect.pki` answer carries: the vehicles list, one
 * vehicle, the cases' reporters and accused, and a CRL's owners.
 */
export function noteDeviceNodes(answer: unknown): void {
  if (!isObj(answer)) return;
  note(answer.device, answer.node);
  if (Array.isArray(answer.vehicles)) for (const v of answer.vehicles) if (isObj(v)) note(v.device, v.node);
  if (Array.isArray(answer.cases)) {
    for (const c of answer.cases) {
      const truth = isObj(c) && isObj(c.truth) ? c.truth : null;
      if (!truth) continue;
      for (const list of [truth.accused, truth.reporters]) if (Array.isArray(list)) for (const p of list) if (isObj(p)) note(p.device, p.node);
    }
  }
  if (isObj(answer.owners)) for (const o of Object.values(answer.owners)) if (isObj(o)) note(o.device, o.node);
}

/** The simulation node of a device, when an answer has named it. */
export function nodeOfDevice(device: string): number | null {
  return nodeByDevice.get(device) ?? null;
}

/** The device id (`ee/1000066`) of a simulation node, when an answer has named it. */
export function deviceOfNode(node: number): string | null {
  return deviceByNode.get(node) ?? null;
}

/** `ee/1000066` → `1000066`; anything else as it is. */
export function deviceNumber(device: string): string {
  const m = /^(ee|obu|rsu)\/(\w+)$/.exec(device);
  return m ? m[2] : device;
}

/**
 * A device in the studio's one naming: `Vehicle 66` (its simulation node, as the HUD and the map
 * call it), `Roadside unit 3`, or `Vehicle (device 900105)` while no answer has named its node.
 * `node` overrides the remembered pair when the caller has it.
 */
export function vehicleName(device: string, node?: number | null): string {
  const m = /^(ee|obu|rsu)\/(\w+)$/.exec(device);
  if (!m) return device;
  if (m[1] === "rsu") return `Roadside unit ${m[2]}`;
  const n = node ?? nodeOfDevice(device);
  return n !== null && n !== undefined ? `Vehicle ${n}` : `Vehicle (device ${m[2]})`;
}

/** The second line under a vehicle's name: its SCMS device id, or null for a non-vehicle. */
export function deviceLine(device: string, node?: number | null): string | null {
  if (!/^(ee|obu)\//.test(device)) return null;
  const n = node ?? nodeOfDevice(device);
  return n !== null && n !== undefined ? `device ${deviceNumber(device)}` : null;
}

/** The tooltip that ties the two ids together. */
export function vehicleTitle(device: string, node?: number | null): string {
  const n = node ?? nodeOfDevice(device);
  return n !== null && n !== undefined
    ? `${vehicleName(device, n)}: simulation node ${n} (as the map and the HUD call it), SCMS device id ${deviceNumber(device)} (as the certificate records call it)`
    : `SCMS device id ${deviceNumber(device)}; no simulation node is known for it (it has not driven in this view's records)`;
}

/**
 * What a typed vehicle filter means: "66" or "Vehicle 66" is a simulation node, and a number the
 * records know as a device id (or `ee/…`) is that device. Returns the device id to ask for, or the
 * text as typed when it names neither.
 */
export function deviceForQuery(query: string): string {
  const q = query.trim();
  if (q === "") return "";
  if (/^(ee|obu)\//i.test(q)) return q.toLowerCase();
  const m = /^(?:vehicle\s*)?#?(\d+)$/i.exec(q);
  if (!m) return q;
  const n = Number(m[1]);
  const asDevice = `ee/${m[1]}`;
  if (nodeByDevice.has(asDevice)) return asDevice;
  return deviceByNode.get(n) ?? q;
}

/** Whether a typed filter picks this vehicle, by either number or by its name. */
export function vehicleMatches(query: string, device: string, node: number | null): boolean {
  const q = query.trim().toLowerCase();
  if (q === "") return true;
  const num = /^(?:vehicle\s*)?#?(\d+)$/.exec(q)?.[1];
  if (num !== undefined) return (node !== null && String(node) === num) || deviceNumber(device) === num;
  return device.toLowerCase().includes(q) || vehicleName(device, node).toLowerCase().includes(q);
}

/** Forget every pair (a new run, or a test). */
export function resetDeviceNodes(): void {
  nodeByDevice.clear();
  deviceByNode.clear();
}
