/**
 * The Backend view's data: what `inspect.entity {entity: "backend"}` answers, and the pure
 * functions that turn it into a diagram.
 *
 * The engine publishes one `backend.state` snapshot a simulated second while a credential system
 * runs (`v2xw_proto::view::BackendView`): every entity of the US SCMS or the European CCMS with its
 * queue, traffic, cryptographic operations and its own counts, the devices pooled as `ee`, the
 * roadside units as `rsu`, and every pair that has exchanged a message. Nothing here computes a
 * number; it only places boxes and picks which counts to show first.
 */

/** One entity's queue, as `v2xw_proto::view::QueueView` serialises it. */
export interface BackendQueue {
  readonly depth: number;
  readonly servers: number;
  readonly served: number;
  readonly busy_ns: number;
  readonly waited_ns: number;
  readonly backlog_ns: number;
  readonly inbound: number;
}

/** One authority, the roadside units, or the pooled devices. */
export interface BackendEntity {
  readonly id: string;
  readonly name: string;
  readonly system: string;
  readonly tier: string;
  readonly online: boolean;
  readonly node: number | null;
  readonly role: string;
  readonly queue: BackendQueue | null;
  readonly traffic: { readonly received: number; readonly sent: number; readonly bytes_in: number; readonly bytes_out: number };
  readonly ops: Readonly<Record<string, number>>;
  readonly state: Readonly<Record<string, unknown>>;
}

/** The traffic between two entities since the run began. */
export interface BackendEdge {
  readonly from: string;
  readonly to: string;
  readonly messages: number;
  readonly bytes: number;
  readonly last_t: number;
  readonly last_step: string;
  readonly transport: string;
  readonly steps: Readonly<Record<string, number>>;
}

/** One recent message. */
export interface BackendStep {
  readonly t: number;
  readonly from: string;
  readonly to: string;
  readonly flow: string;
  readonly step: string;
  readonly bytes: number;
  readonly transport: string;
}

/** The whole snapshot. */
export interface BackendSnapshot {
  readonly system: "scms" | "ccms" | string;
  readonly protocol: string;
  /** The signature scheme every certificate and signed message uses: `ecdsa-p256` or a hybrid post-quantum one. */
  readonly signature: string;
  readonly t: number;
  readonly entities: readonly BackendEntity[];
  readonly edges: readonly BackendEdge[];
  readonly recent: readonly BackendStep[];
  readonly flows: Readonly<Record<string, number>>;
}

/** Reads a snapshot out of an `inspect.entity` answer, or `null` if it is not one. */
export function snapshotOf(answer: unknown): BackendSnapshot | null {
  if (typeof answer !== "object" || answer === null) return null;
  const state = (answer as { state?: unknown }).state;
  if (typeof state !== "object" || state === null) return null;
  const s = state as Partial<BackendSnapshot>;
  if (!Array.isArray(s.entities) || !Array.isArray(s.edges)) return null;
  return {
    system: typeof s.system === "string" ? s.system : "",
    protocol: typeof s.protocol === "string" ? s.protocol : "",
    signature: typeof s.signature === "string" ? s.signature : "ecdsa-p256",
    t: typeof s.t === "number" ? s.t : 0,
    entities: s.entities,
    edges: s.edges,
    recent: Array.isArray(s.recent) ? s.recent : [],
    flows: typeof s.flows === "object" && s.flows !== null ? s.flows : {},
  };
}

/**
 * The diagram's columns, left to right: who governs, who certifies, who registers and issues,
 * who keeps the device private, who revokes, what distributes, and the devices. A researcher reads
 * a flow left to right as it goes from policy to the vehicle.
 */
export const TIERS: readonly { readonly id: string; readonly label: string }[] = [
  { id: "governance", label: "Governance" },
  { id: "ca", label: "Certificate authorities" },
  { id: "ra", label: "Registration and issuance" },
  { id: "privacy", label: "Privacy" },
  { id: "revocation", label: "Misbehaviour and revocation" },
  { id: "distribution", label: "Distribution" },
  { id: "device", label: "Devices" },
];

/** A placed box. */
export interface Placed {
  readonly id: string;
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
  readonly column: number;
}

/** Box and gap sizes, in CSS pixels. */
export const BOX = { w: 168, h: 74, gapX: 44, gapY: 16, head: 28 } as const;

/**
 * Places every entity: one column per tier that has an entity, entities in the order the engine
 * lists them within a column. Tiers with nothing in them take no room, so the CCMS's six
 * authorities do not leave a hole where the SCMS's LAs would be.
 */
export function layout(entities: readonly BackendEntity[]): { boxes: Placed[]; width: number; height: number } {
  const known = new Set(TIERS.map((t) => t.id));
  const columns = TIERS.map((t) => entities.filter((e) => (known.has(e.tier) ? e.tier : "device") === t.id)).filter(
    (c) => c.length > 0,
  );
  const boxes: Placed[] = [];
  let height = 0;
  columns.forEach((column, ci) => {
    column.forEach((e, ri) => {
      const x = ci * (BOX.w + BOX.gapX);
      const y = BOX.head + ri * (BOX.h + BOX.gapY);
      boxes.push({ id: e.id, x, y, w: BOX.w, h: BOX.h, column: ci });
      height = Math.max(height, y + BOX.h);
    });
  });
  const width = columns.length * BOX.w + Math.max(0, columns.length - 1) * BOX.gapX;
  return { boxes, width, height };
}

/** The column headings the layout produced, with their x. */
export function columnHeads(entities: readonly BackendEntity[]): { label: string; x: number }[] {
  const known = new Set(TIERS.map((t) => t.id));
  const present = TIERS.filter((t) => entities.some((e) => (known.has(e.tier) ? e.tier : "device") === t.id));
  return present.map((t, i) => ({ label: t.label, x: i * (BOX.w + BOX.gapX) }));
}

/** An edge's stroke width: one pixel for a message, growing with the log of the count. */
export function edgeWidth(messages: number): number {
  if (messages <= 0) return 0;
  return Math.min(6, 1 + Math.log10(messages) * 1.6);
}

/** Whether an edge carried a message within `windowNs` of the snapshot: drawn as live. */
export function isLive(edge: BackendEdge, t: number, windowNs = 2_000_000_000): boolean {
  return edge.messages > 0 && t - edge.last_t <= windowNs;
}

/**
 * The path of an edge between two boxes: from the side of `a` facing `b` to the side of `b`
 * facing `a`, bowed so two edges between one pair in opposite directions do not overlap.
 */
export function edgePath(a: Placed, b: Placed, reverse: boolean): string {
  const bow = reverse ? 10 : -10;
  if (a.column === b.column) {
    const x = a.x + a.w;
    const y1 = a.y + a.h / 2;
    const y2 = b.y + b.h / 2;
    const cx = x + 36 + Math.abs(y2 - y1) * 0.08;
    return `M ${x} ${y1 + bow / 2} C ${cx} ${y1 + bow / 2}, ${cx} ${y2 + bow / 2}, ${x} ${y2 + bow / 2}`;
  }
  const forward = a.x < b.x;
  const x1 = forward ? a.x + a.w : a.x;
  const x2 = forward ? b.x : b.x + b.w;
  const y1 = a.y + a.h / 2 + bow / 2;
  const y2 = b.y + b.h / 2 + bow / 2;
  const mx = (x1 + x2) / 2;
  return `M ${x1} ${y1} C ${mx} ${y1}, ${mx} ${y2}, ${x2} ${y2}`;
}

/** Turns a state key into words: `certs_issued` → `certs issued`. */
export function label(key: string): string {
  return key.replace(/_/g, " ");
}

/** A count for a box: an integer with thin spaces, a boolean as yes/no, a hex digest shortened. */
export function formatValue(v: unknown): string {
  if (typeof v === "number") return Number.isInteger(v) ? v.toLocaleString("en-US") : v.toFixed(2);
  if (typeof v === "boolean") return v ? "yes" : "no";
  if (typeof v === "string") return v.length > 12 ? `${v.slice(0, 8)}…` : v;
  if (v === null || v === undefined) return "–";
  return JSON.stringify(v);
}

/**
 * The two counts a box shows under its name: the first two numeric state entries that are not
 * zero, so a box says what the entity has actually done. A box with nothing done yet shows its
 * first two, zeros included.
 */
export function headlineCounts(e: BackendEntity): [string, string][] {
  const numeric = Object.entries(e.state).filter(([, v]) => typeof v === "number") as [string, number][];
  const nonZero = numeric.filter(([, v]) => v !== 0);
  const pick = (nonZero.length > 0 ? nonZero : numeric).slice(0, 2);
  return pick.map(([k, v]) => [label(k), formatValue(v)]);
}

/** Bytes as B, kB or MB. */
export function formatBytes(b: number): string {
  if (b < 1_000) return `${b} B`;
  if (b < 1_000_000) return `${(b / 1_000).toFixed(1)} kB`;
  return `${(b / 1_000_000).toFixed(2)} MB`;
}

/** The panel's heading for a snapshot's `system`. */
export function systemTitle(system: string): string {
  if (system === "scms") return "US SCMS (IEEE 1609.2.1, CAMP and USDOT design)";
  if (system === "ccms") return "European CCMS (ETSI TS 102 941)";
  return system === "" ? "Backend" : system;
}

/** A simulated instant, ns, as seconds with one decimal. */
export function formatT(ns: number): string {
  return `${(ns / 1e9).toFixed(1)} s`;
}

/** The ids of entities that sent or received a message within `windowNs` of the snapshot. */
export function activeEntities(s: BackendSnapshot, windowNs = 2_000_000_000): Set<string> {
  const out = new Set<string>();
  for (const e of s.edges) {
    if (isLive(e, s.t, windowNs)) {
      out.add(e.from);
      out.add(e.to);
    }
  }
  return out;
}

/** The whole system in a few numbers: what the Backend view's top strip shows. */
export interface Glance {
  readonly entities: number;
  readonly activeEntities: number;
  readonly edges: number;
  readonly liveEdges: number;
  readonly messages: number;
  readonly bytes: number;
  /** Requests waiting in every entity's queue now. */
  readonly queued: number;
  /** Entities kept offline (air-gapped). */
  readonly offline: number;
}

export function glance(s: BackendSnapshot): Glance {
  let messages = 0;
  let bytes = 0;
  let liveEdges = 0;
  for (const e of s.edges) {
    messages += e.messages;
    bytes += e.bytes;
    if (isLive(e, s.t)) liveEdges++;
  }
  let queued = 0;
  let offline = 0;
  for (const e of s.entities) {
    queued += e.queue?.depth ?? 0;
    if (!e.online) offline++;
  }
  return {
    entities: s.entities.length,
    activeEntities: activeEntities(s).size,
    edges: s.edges.length,
    liveEdges,
    messages,
    bytes,
    queued,
    offline,
  };
}

/**
 * The metrics that measure an entity's work, by the tier the engine puts it in; `null` asks for the
 * credential system as a whole. Names are the engine's metric catalogue (`v2xw-metrics`); the view
 * offers only those the run actually carries.
 *
 *  * certificate authorities, registration and issuance: the valid pool the devices hold and how
 *    often they change pseudonym, which is what the issuance chain exists to keep up;
 *  * privacy (the linkage authorities, the shuffle): linkability and the change rate;
 *  * misbehaviour and revocation: the time to detect, to decide and each revocation stage, the
 *    detector's quality and its false accusations, and the CRL it produces;
 *  * distribution: the CRL's size and the devices' backend reach;
 *  * devices and roadside units: pool, reach, verification rate and the unverified share.
 */
export function metricsFor(e: Pick<BackendEntity, "tier" | "id"> | null): string[] {
  const pool = ["cert_pool_valid", "pseudonym_change_rate"];
  const revocation = ["time_to_detect", "time_to_decision", "revocation_latency_stage", "false_accusations", "det_precision", "det_recall", "crl_entries"];
  const distribution = ["crl_entries", "crl_bytes", "backend_link_up"];
  const device = ["cert_pool_valid", "backend_link_up", "verify_rate", "unverified_ratio", "pseudonym_change_rate"];
  if (e === null) return [...new Set([...pool, "backend_link_up", "revocation_latency_stage", "crl_entries", "linkability"])];
  switch (e.tier) {
    case "governance":
    case "ca":
      return pool;
    case "ra":
      return [...pool, "backend_link_up"];
    case "privacy":
      return ["linkability", "pseudonym_change_rate"];
    case "revocation":
      return revocation;
    case "distribution":
      return distribution;
    default:
      return device;
  }
}
