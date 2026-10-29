/**
 * The SCMS workspace's topology model: the deployment as a diagram — every provider a region, every
 * authority a node, the links the US SCMS defines between them, the vehicles and roadside units
 * as one client group per provider — and the messages of `inspect.trust` placed on those links.
 *
 * Pure functions of an engine answer, so the components draw and the tests check one thing. A
 * node's state is the engine's (`online`, `offline` by design, `faulted` by an injected outage)
 * or `not-deployed` when the provider runs no component of that role; a link is drawn when the
 * architecture defines it (01 §2, 03 §2–§6) or when a message crossed it, so a message on a link
 * the model did not expect is still drawn rather than dropped.
 *
 * Framework- and DOM-free (`test/pki-topology.test.ts`).
 */

import { GROUND_TRUTH, type PkiComponent, type PkiProvider, type PkiState, type TrustAnswer } from "./pki.js";

// ---------------------------------------------------------------------------------------------
// The roles, in plain words
// ---------------------------------------------------------------------------------------------

/** What the workspace says about one role: a plain name, the standard term and one sentence. */
export interface RoleInfo {
  readonly name: string;
  readonly term: string;
  readonly does: string;
}

/** Every SCMS role the engine runs, as the US deployment names it (CAMP SCMS PoC / IEEE 1609.2.1). */
export const ROLE_INFO: Readonly<Record<string, RoleInfo>> = {
  root: {
    name: "Root",
    term: "Root Certificate Authority (RCA)",
    does: "The trust anchor: kept offline and brought up only to sign; devices trust it because the trust list names it.",
  },
  ica: {
    name: "Intermediate",
    term: "Intermediate Certificate Authority (ICA)",
    does: "Kept offline; certifies the provider's online authorities so the root can stay locked away.",
  },
  eca: {
    name: "Enrolment",
    term: "Enrolment Certificate Authority (ECA)",
    does: "Issues each device its long-term enrolment certificate, the credential it later uses to ask for pseudonyms.",
  },
  dcm: {
    name: "Device setup",
    term: "Device Configuration Manager (DCM)",
    does: "At the factory or dealer, relays the device's enrolment request to the ECA and hands back its certificate and trust bundle.",
  },
  ra: {
    name: "Registration",
    term: "Registration Authority (RA)",
    does: "Checks the enrolment certificate, expands and shuffles certificate requests, and forwards them to the ACA without the device's identity.",
  },
  lop: {
    name: "Proxy",
    term: "Location Obscurer Proxy (LOP)",
    does: "Relays a device's requests and downloads so the RA never sees its network address.",
  },
  aca: {
    name: "Pseudonym CA",
    term: "Authorization Certificate Authority (ACA), the pseudonym CA; some documents call it the PCA",
    does: "Signs the pseudonym certificates, a week's batch per vehicle at a time (the policy's j-max, 20 in the US profile), without learning whose they are.",
  },
  la1: {
    name: "Linkage 1",
    term: "Linkage Authority 1 (LA1)",
    does: "Makes half of the pre-linkage values; with LA2's half a revoked device's certificates become linkable, never with one alone.",
  },
  la2: {
    name: "Linkage 2",
    term: "Linkage Authority 2 (LA2)",
    does: "Makes the other half of the pre-linkage values; it is run apart from LA1 so neither can link a device by itself.",
  },
  ma: {
    name: "Misbehaviour",
    term: "Misbehaviour Authority (MA)",
    does: "Collects misbehaviour reports, investigates with the ACA, LAs and RA, and decides whether a device is revoked.",
  },
  crlg: {
    name: "Revocation",
    term: "CRL Generator (CRLG)",
    does: "Signs and publishes the certificate revocation list (CRL).",
  },
  "crl-store": {
    name: "List store",
    term: "CRL Store",
    does: "Serves the published revocation list to devices over the network.",
  },
  "crl-broadcast": {
    name: "List relay",
    term: "CRL Broadcast",
    does: "Hands the revocation list to roadside units, which rebroadcast it over the air.",
  },
  pg: {
    name: "Policy",
    term: "Policy Generator (PG)",
    does: "Signs the global policy file with its own certificate, and hands the certificate trust list the electors signed on to devices.",
  },
  manager: {
    name: "SCMS Manager",
    term: "SCMS Manager",
    does: "Governs the system: sets policy and, on the electors' ballots, publishes the certificate trust list (CTL).",
  },
  elector: {
    name: "Elector",
    term: "Elector",
    does: "Holds an offline key; a quorum of electors' endorsements makes a trust-list change valid.",
  },
  devices: {
    name: "Vehicles and roadside units",
    term: "End entities (OBUs and RSUs)",
    does: "Enrol once, then download pseudonym batches, sign safety messages, report misbehaviour and install revocation lists.",
  },
};

/** Roles that come several to a system, numbered from 1 even when there is only the first. */
const NUMBERED_ROLES = new Set(["elector"]);

/** A role's standard acronym (`ACA`, `LA1`), from its term; null when the term has none. */
export function roleShort(role: string): string | null {
  const m = /\(([A-Z][A-Za-z0-9]*)[,)]/.exec(roleInfo(role).term);
  return m ? m[1] : null;
}

/**
 * The one name every view gives a component or device: the plain name with the standard acronym
 * beside it and a 1-based number (`Pseudonym CA (ACA) 2`, `Elector 1`, `Vehicle 1000000`), and the
 * provider after it when asked.
 */
export function componentName(id: string, withProvider = false): string {
  const dev = /^(ee|obu|rsu)\/(\w+)$/.exec(id);
  if (dev) return dev[1] === "rsu" ? `Roadside unit ${dev[2]}` : `Vehicle ${dev[2]}`;
  const [provider, role, n] = id.split("/");
  if (!role) return id;
  const info = roleInfo(role);
  const short = roleShort(role);
  const k = Number(n);
  const num = Number.isFinite(k) && (k > 0 || NUMBERED_ROLES.has(role)) ? ` ${k + 1}` : "";
  const base = `${info.name}${short && short !== info.name ? ` (${short})` : ""}${num}`;
  return withProvider && provider && provider !== "scms-manager" ? `${base}, ${provider}` : base;
}

/** Engine wording in the deployment's one set of names ("PCA" is the ACA). */
export function plainNames(text: string): string {
  return text
    .replace(/\bPCA\b/g, "Pseudonym CA (ACA)")
    .replace(/\bper LCI\b/g, "per linkage chain identifier (LCI)")
    .replace(/\bPLVs?\b/g, (m) => (m.endsWith("s") ? "pre-linkage values (PLVs)" : "pre-linkage value (PLV)"))
    .replace(/\bi-periods and j range\b/g, "weeks (i-periods) and certificates per week (j)")
    .replace(/\bthe caterpillar and cocoon public keys\b/g, "the vehicle's expandable public keys (caterpillar and cocoon keys)")
    .replace(/\biRev\b/g, "the revocation week (iRev)");
}

/** A role's plain words; an unknown role is named as the engine spells it. */
export function roleInfo(role: string): RoleInfo {
  return ROLE_INFO[role] ?? { name: role, term: role, does: "A component the workspace has no description for." };
}

/** The roles every provider of the US deployment runs (01 §2.1); one it lacks is "not deployed". */
export const PROVIDER_ROLES: readonly string[] = ["root", "ica", "eca", "dcm", "ra", "lop", "aca", "la1", "la2", "ma", "crlg", "crl-store", "crl-broadcast", "pg"];

/** The SCMS flows, in plain words, with their colour (Okabe–Ito, as the page's tokens are). */
export const FLOW_INFO: Readonly<Record<string, { readonly name: string; readonly color: string }>> = {
  enrolment: { name: "Enrolment", color: "var(--ok)" },
  provisioning: { name: "Pseudonym certificates", color: "var(--accent)" },
  report: { name: "Misbehaviour reports", color: "var(--warn)" },
  "linkage-resolution": { name: "Investigation", color: "var(--err)" },
  "crl-issuance": { name: "Revocation lists", color: "#cc79a7" },
};

/** Flows the diagram does not colour, named for the lists that show them. */
const FLOW_MORE: Readonly<Record<string, string>> = {
  topup: "Pseudonym top-ups",
  pregeneration: "Linkage values made in advance",
  preload: "Factory and dealer setup",
  reenrolment: "Re-enrolment",
  "rsu-application-cert": "Roadside unit certificate",
};

export function flowInfo(flow: string): { readonly name: string; readonly color: string } {
  return FLOW_INFO[flow] ?? { name: FLOW_MORE[flow] ?? flow.replace(/[-_]+/g, " "), color: "var(--text-dim)" };
}

/** Plain words for a message name (`pca-lookup-request` → "pca lookup request"). */
export function messageText(name: string): string {
  return name.replace(/-/g, " ");
}

// ---------------------------------------------------------------------------------------------
// Messages
// ---------------------------------------------------------------------------------------------

/** One message of `inspect.trust`'s `messages`. */
export interface TrustMessage {
  readonly id: string;
  readonly t: number;
  readonly from: string | null;
  readonly to: string | null;
  readonly name: string;
  readonly flow: string;
  readonly instance: string;
  readonly provider: string;
  readonly bytes: number | null;
}

/** The answer's messages, read defensively (an engine older than the field answers none). */
export function messagesOf(trust: TrustAnswer | null): TrustMessage[] {
  const raw = trust?.messages;
  if (!Array.isArray(raw)) return [];
  const out: TrustMessage[] = [];
  for (const m of raw) {
    if (typeof m !== "object" || m === null) continue;
    const r = m as Record<string, unknown>;
    if (typeof r.id !== "string" || typeof r.t !== "number") continue;
    out.push({
      id: r.id,
      t: r.t,
      from: typeof r.from === "string" ? r.from : null,
      to: typeof r.to === "string" ? r.to : null,
      name: typeof r.name === "string" ? r.name : "",
      flow: typeof r.flow === "string" ? r.flow : "",
      instance: typeof r.instance === "string" ? r.instance : "",
      provider: typeof r.provider === "string" ? r.provider : "",
      bytes: typeof r.bytes === "number" ? r.bytes : null,
    });
  }
  return out;
}

/** Whether an endpoint is a device (`ee/<n>`), which the diagram draws as its provider's client group. */
export function isDevice(end: string | null): boolean {
  return end !== null && end.startsWith("ee/");
}

/** The diagram node an endpoint lands on: a component id, or `devices/<provider>` for a device. */
export function nodeOf(end: string | null, otherEnd: string | null, fallbackProvider: string): string | null {
  if (end === null) return null;
  if (!isDevice(end)) return end;
  const other = otherEnd !== null && !isDevice(otherEnd) ? otherEnd.split("/")[0] : fallbackProvider;
  return `devices/${other}`;
}

/** Whether `view` observed a message: ground truth sees all, an entity what it sent or received. */
export function observedBy(m: Pick<TrustMessage, "from" | "to">, view: string): boolean {
  return view === GROUND_TRUTH || m.from === view || m.to === view;
}

// ---------------------------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------------------------

/** A node of the diagram. */
export interface TopoNode {
  readonly id: string;
  readonly kind: "component" | "ghost" | "devices";
  readonly role: string;
  readonly provider: string;
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
  readonly component: PkiComponent | null;
}

/** A provider's region box. */
export interface TopoRegion {
  readonly provider: PkiProvider;
  readonly x: number;
  readonly y: number;
  readonly w: number;
  readonly h: number;
}

/** A link: two node ids, and whether the architecture defines it (`false`: only a message showed it). */
export interface TopoLink {
  readonly key: string;
  readonly a: string;
  readonly b: string;
  readonly defined: boolean;
}

export interface Topology {
  readonly width: number;
  readonly height: number;
  readonly governance: { readonly x: number; readonly y: number; readonly w: number; readonly h: number };
  readonly regions: readonly TopoRegion[];
  readonly nodes: readonly TopoNode[];
  readonly links: readonly TopoLink[];
  /** Whether the regions sit side by side (wide) or stacked (narrow). */
  readonly sideBySide: boolean;
  /** Half the gap between two columns of nodes: where a routed link runs between them. */
  readonly gutter?: number;
}

/** Where each role sits in a region: column (0–5) and row (0–2); row 3 is the client group. */
const SLOT: Readonly<Record<string, readonly [number, number]>> = {
  root: [0, 0],
  ica: [1, 0],
  la1: [2, 0],
  la2: [3, 0],
  eca: [0, 1],
  ra: [1, 1],
  aca: [2, 1],
  ma: [3, 1],
  crlg: [4, 1],
  dcm: [0, 2],
  lop: [1, 2],
  pg: [3, 2],
  "crl-store": [4, 2],
  "crl-broadcast": [5, 2],
};
/** Free slots for a second instance of a role, or a role the layout does not know. */
const SPARE: readonly (readonly [number, number])[] = [
  [2, 2],
  [4, 0],
  [5, 0],
  [5, 1],
];

/** The links the US SCMS defines inside one provider (01 §2.1, 03 §2–§6), by role. */
export const DEFINED_LINKS: readonly (readonly [string, string])[] = [
  ["devices", "dcm"],
  ["dcm", "eca"],
  ["dcm", "ra"],
  ["devices", "lop"],
  ["lop", "ra"],
  ["ra", "aca"],
  ["ra", "la1"],
  ["ra", "la2"],
  ["ra", "eca"],
  ["ra", "ma"],
  ["ma", "aca"],
  ["ma", "la1"],
  ["ma", "la2"],
  ["ma", "crlg"],
  ["crlg", "crl-store"],
  ["crlg", "crl-broadcast"],
  ["crl-store", "devices"],
  ["crl-broadcast", "devices"],
  ["pg", "devices"],
];

export const NODE_W = 104;
export const NODE_H = 30;
const ROW_GAP = 52;
const REGION_HEAD = 34;
const PAD = 14;
const GOV_H = 64;
const GAP = 16;
/** A region narrower than this cannot hold six columns of nodes. */
const MIN_REGION_W = 6 * NODE_W + 5 * 8 + 2 * PAD;
/** The narrowest node, and so the narrowest region, the diagram draws (wider scrolls). */
export const MIN_NODE_W = 76;
const MIN_STACKED_W = 6 * MIN_NODE_W + 5 * 8 + 2 * PAD;

function linkKey(a: string, b: string): string {
  return a < b ? `${a}|${b}` : `${b}|${a}`;
}

/**
 * The diagram for a state at a width: the governance strip on top, then one region per provider
 * — side by side when they fit, stacked otherwise — each with its components in their slots, a
 * ghost for every role it does not run, and its client group along the bottom. The links are the
 * defined ones between nodes that exist, plus one per (sender, receiver) pair of `messages`.
 */
export function layoutTopology(state: PkiState, width: number, messages: readonly TrustMessage[] = []): Topology {
  const providers = state.providers;
  const n = Math.max(1, providers.length);
  const sideBySide = n > 1 && width >= n * MIN_REGION_W + (n - 1) * GAP;
  const regionW = Math.max(MIN_STACKED_W, sideBySide ? (width - (n - 1) * GAP) / n : width);
  // Nodes shrink to fit a narrow region (down to MIN_NODE_W), never grow past NODE_W.
  const nodeW = Math.max(MIN_NODE_W, Math.min(NODE_W, (regionW - 2 * PAD - 5 * 8) / 6));
  const regionH = REGION_HEAD + 3 * ROW_GAP + NODE_H + 18 + PAD;
  const totalW = sideBySide ? n * regionW + (n - 1) * GAP : regionW;
  const nodes: TopoNode[] = [];
  const regions: TopoRegion[] = [];
  const top = GOV_H + GAP;
  providers.forEach((p, k) => {
    const x0 = sideBySide ? k * (regionW + GAP) : 0;
    const y0 = sideBySide ? top : top + k * (regionH + GAP);
    regions.push({ provider: p, x: x0, y: y0, w: regionW, h: regionH });
    const colStep = (regionW - 2 * PAD - nodeW) / 5;
    const at = (col: number, row: number): { x: number; y: number } => ({ x: x0 + PAD + col * colStep, y: y0 + REGION_HEAD + row * ROW_GAP });
    const used = new Set<string>();
    const spare = [...SPARE];
    const sorted = [...p.components].sort((a, b) => a.instance - b.instance);
    for (const c of sorted) {
      let slot = SLOT[c.role];
      const key = slot ? `${slot[0]},${slot[1]}` : "";
      if (!slot || used.has(key)) {
        slot = spare.shift() ?? [5, 0];
      }
      used.add(`${slot[0]},${slot[1]}`);
      const { x, y } = at(slot[0], slot[1]);
      nodes.push({ id: c.id, kind: "component", role: c.role, provider: p.label, x, y, w: nodeW, h: NODE_H, component: c });
    }
    for (const role of PROVIDER_ROLES) {
      if (p.components.some((c) => c.role === role)) continue;
      const slot = SLOT[role];
      if (!slot || used.has(`${slot[0]},${slot[1]}`)) continue;
      const { x, y } = at(slot[0], slot[1]);
      nodes.push({ id: `${p.label}/${role}/not-deployed`, kind: "ghost", role, provider: p.label, x, y, w: nodeW, h: NODE_H, component: null });
    }
    const dy = y0 + REGION_HEAD + 3 * ROW_GAP;
    nodes.push({ id: `devices/${p.label}`, kind: "devices", role: "devices", provider: p.label, x: x0 + PAD, y: dy, w: regionW - 2 * PAD, h: NODE_H + 4, component: null });
  });
  // Governance: the manager and the electors across the top.
  const gov = state.governance;
  const managers = gov.filter((c) => c.role !== "elector");
  const electors = gov.filter((c) => c.role === "elector");
  let gx = PAD;
  for (const c of [...managers, ...electors]) {
    const w = c.role === "elector" ? 78 : NODE_W + 16;
    nodes.push({ id: c.id, kind: "component", role: c.role, provider: c.provider, x: gx, y: (GOV_H - NODE_H) / 2, w, h: NODE_H, component: c });
    gx += w + (c.role === "elector" ? 6 : 26);
  }
  // Links.
  const byId = new Map(nodes.map((nd) => [nd.id, nd]));
  const links = new Map<string, TopoLink>();
  const add = (a: string, b: string, defined: boolean): void => {
    if (a === b || !byId.has(a) || !byId.has(b)) return;
    const key = linkKey(a, b);
    const had = links.get(key);
    if (!had) links.set(key, { key, a, b, defined });
    else if (defined && !had.defined) links.set(key, { ...had, defined: true });
  };
  for (const p of providers) {
    const idOf = (role: string): string[] =>
      role === "devices" ? [`devices/${p.label}`] : nodes.filter((nd) => nd.provider === p.label && nd.role === role && nd.kind === "component").map((nd) => nd.id);
    for (const [ra, rb] of DEFINED_LINKS) for (const a of idOf(ra)) for (const b of idOf(rb)) add(a, b, true);
  }
  for (const m of messages) {
    const a = nodeOf(m.from, m.to, m.provider);
    const b = nodeOf(m.to, m.from, m.provider);
    if (a && b) add(a, b, false);
  }
  const height = sideBySide ? top + regionH : top + n * regionH + (n - 1) * GAP;
  const colGap = (regionW - 2 * PAD - nodeW) / 5 - nodeW;
  return { width: totalW, height, governance: { x: 0, y: 0, w: totalW, h: GOV_H }, regions, nodes, links: [...links.values()], sideBySide, gutter: Math.max(3, colGap / 2) };
}

/** A point. */
export interface Pt {
  readonly x: number;
  readonly y: number;
}

/** The centre of a node; for a client group, the point under `toward` (its links drop straight down). */
export function anchor(nd: TopoNode, toward: Pt): Pt {
  if (nd.kind === "devices") return { x: Math.min(nd.x + nd.w - 8, Math.max(nd.x + 8, toward.x)), y: nd.y + nd.h / 2 };
  return { x: nd.x + nd.w / 2, y: nd.y + nd.h / 2 };
}

/**
 * A link's path, as an SVG `d` and its three control points (start, bend, end). Links between
 * nodes of one row that skip a column bow upward so they do not run through the node between;
 * every other link is straight.
 */
export function linkPath(a: TopoNode, b: TopoNode): { readonly d: string; readonly p0: Pt; readonly c: Pt; readonly p1: Pt } {
  const ca = { x: a.x + a.w / 2, y: a.y + a.h / 2 };
  const cb = { x: b.x + b.w / 2, y: b.y + b.h / 2 };
  const p0 = anchor(a, cb);
  const p1 = anchor(b, p0.x === ca.x ? ca : p0);
  const sameRow = Math.abs(p0.y - p1.y) < 1;
  const skip = sameRow && Math.abs(p0.x - p1.x) > (a.w + b.w) / 2 + 60;
  const c = skip ? { x: (p0.x + p1.x) / 2, y: p0.y - 34 } : { x: (p0.x + p1.x) / 2, y: (p0.y + p1.y) / 2 };
  return { d: `M ${r(p0.x)} ${r(p0.y)} Q ${r(c.x)} ${r(c.y)} ${r(p1.x)} ${r(p1.y)}`, p0, c, p1 };
}

function r(v: number): string {
  return (Math.round(v * 10) / 10).toString();
}

/** Whether the segment p–q crosses node `n`'s box (shrunk by 2 px, so touching an edge does not count). */
function crosses(n: TopoNode, p: Pt, q: Pt): boolean {
  const x0 = n.x + 2;
  const x1 = n.x + n.w - 2;
  const y0 = n.y + 2;
  const y1 = n.y + n.h - 2;
  // Liang–Barsky clip of the segment against the box.
  let t0 = 0;
  let t1 = 1;
  const dx = q.x - p.x;
  const dy = q.y - p.y;
  for (const [pp, qq] of [
    [-dx, p.x - x0],
    [dx, x1 - p.x],
    [-dy, p.y - y0],
    [dy, y1 - p.y],
  ] as const) {
    if (pp === 0) {
      if (qq < 0) return false;
      continue;
    }
    const r0 = qq / pp;
    if (pp < 0) t0 = Math.max(t0, r0);
    else t1 = Math.min(t1, r0);
    if (t0 > t1) return false;
  }
  return true;
}

/**
 * A link as the diagram draws it, and the path a message dot follows along it. Inside one region a
 * link is straight (or bowed, `linkPath`) unless that line would run through another box; then it
 * runs up the gap beside its target. A link between two providers leaves its box sideways, runs
 * along the gap between columns and between the regions, and enters the other box the same way, so
 * it never crosses another provider's authorities.
 */
export function edgePath(t: Topology, a: TopoNode, b: TopoNode): string {
  const g = t.gutter ?? 6;
  const ra = t.regions.find((r) => r.provider.label === a.provider);
  const rb = t.regions.find((r) => r.provider.label === b.provider);
  if (ra && rb && ra !== rb && a.kind !== "devices" && b.kind !== "devices") {
    const ax = a.x + a.w;
    const bx = b.x + b.w;
    const ay = a.y + a.h / 2;
    const by = b.y + b.h / 2;
    const ga = ax + g;
    const gb = bx + g;
    // The lane: the gap above the row when the regions sit side by side, else the gap between the regions.
    const lane = Math.abs(ay - by) < 1 ? a.y - (ROW_GAP - NODE_H) / 2 : (Math.min(ra.y + ra.h, rb.y + rb.h) + Math.max(ra.y, rb.y)) / 2;
    return `M ${r(ax)} ${r(ay)} H ${r(ga)} V ${r(lane)} H ${r(gb)} V ${r(by)} H ${r(bx)}`;
  }
  const plain = linkPath(a, b);
  // A bowed link already clears the boxes between its ends.
  if (Math.abs(plain.c.y - (plain.p0.y + plain.p1.y) / 2) > 1) return plain.d;
  const others = t.nodes.filter((n) => n !== a && n !== b && n.kind !== "devices");
  if (!others.some((n) => crosses(n, plain.p0, plain.p1))) return plain.d;
  // Blocked: run up (or down) the gap to the right of the node that is not the client group, then in sideways.
  const [from, to] = b.kind === "devices" ? [b, a] : [a, b];
  const gx = to.x + to.w + g;
  const ty = to.y + to.h / 2;
  const start = from.kind === "devices" ? { x: Math.min(from.x + from.w - 8, Math.max(from.x + 8, gx)), y: from.y + from.h / 2 } : { x: from.x + from.w / 2, y: from.y + from.h / 2 };
  const d =
    from.kind === "devices"
      ? `M ${r(start.x)} ${r(start.y)} V ${r(ty)} H ${r(to.x + to.w)}`
      : `M ${r(start.x)} ${r(start.y)} H ${r(gx)} V ${r(ty)} H ${r(to.x + to.w)}`;
  if (from === a) return d;
  // The path runs from `a`: reverse the elbow.
  return from.kind === "devices"
    ? `M ${r(to.x + to.w)} ${r(ty)} H ${r(start.x)} V ${r(start.y)}`
    : `M ${r(to.x + to.w)} ${r(ty)} H ${r(gx)} V ${r(start.y)} H ${r(start.x)}`;
}

/** The ends of a message on the diagram (node ids), or null when either end is not on it. */
export function messageEnds(t: Pick<Topology, "nodes">, m: Pick<TrustMessage, "from" | "to" | "provider">): readonly [string, string] | null {
  const a = nodeOf(m.from, m.to, m.provider);
  const b = nodeOf(m.to, m.from, m.provider);
  if (!a || !b || a === b) return null;
  if (!t.nodes.some((n) => n.id === a) || !t.nodes.some((n) => n.id === b)) return null;
  return [a, b];
}

/** The path a message takes, sender to receiver, along its drawn link; null when either end is not on the diagram. */
export function messagePath(t: Topology, m: TrustMessage): string | null {
  const ends = messageEnds(t, m);
  if (!ends) return null;
  const byId = new Map(t.nodes.map((nd) => [nd.id, nd]));
  const a = byId.get(ends[0]);
  const b = byId.get(ends[1]);
  if (!a || !b) return null;
  return edgePath(t, a, b);
}

/** Messages per link in a batch, by link key: what the diagram thickens. */
export function linkLoad(messages: readonly TrustMessage[]): Map<string, number> {
  const out = new Map<string, number>();
  for (const m of messages) {
    const a = nodeOf(m.from, m.to, m.provider);
    const b = nodeOf(m.to, m.from, m.provider);
    if (!a || !b || a === b) continue;
    const k = linkKey(a, b);
    out.set(k, (out.get(k) ?? 0) + 1);
  }
  return out;
}

export { linkKey };

// ---------------------------------------------------------------------------------------------
// The health strip
// ---------------------------------------------------------------------------------------------

/** The header strip's numbers; null where the data to say it is not available in this view. */
export interface Health {
  /** Whether `revoked` counts the devices a published list names (ground truth) or the providers' own revocations. */
  readonly revokedByList: boolean;
  /** Usable pseudonym certificates across the vehicles (ground truth: their own pools). */
  readonly certificates: number | null;
  /** Requests queued at the authorities, plus top-ups in flight at the vehicles. */
  readonly pending: number;
  /** Misbehaviour cases opened and not yet enforced or refused. */
  readonly openCases: number | null;
  /** Seconds since the newest revocation list was issued, per provider, the oldest of them; null before any. */
  readonly crlAgeS: number | null;
  /** Whether a provider's newest list is past its nextCrl. */
  readonly crlOverdue: boolean;
  /**
   * Seconds until the soonest provider's next list is due (its newest list's `nextCrl` against the
   * snapshot's clock), negative when overdue; null before any list.
   */
  readonly crlDueS: number | null;
  /** Vehicles whose pools the certificate count covers; null outside ground truth. */
  readonly vehicles: number | null;
  readonly devices: number;
  readonly revoked: number;
}

/** The strip from the newest trust snapshot and, when the view allows, the vehicles and cases. */
export function healthOf(
  trust: TrustAnswer,
  vehicles:
    | readonly {
        readonly usable: number;
        readonly revoked?: boolean;
        readonly security?: { readonly topup_in_flight?: boolean } | null;
        readonly truth?: { readonly revoked_certs: number } | null;
      }[]
    | null,
  cases: readonly { readonly stages: readonly { readonly stage: string }[] }[] | null,
): Health {
  const s = trust.state;
  let pending = 0;
  for (const p of s.providers) for (const c of p.components) pending += c.queue?.depth ?? 0;
  for (const c of s.governance) pending += c.queue?.depth ?? 0;
  if (vehicles) for (const v of vehicles) if (v.security?.topup_in_flight === true) pending += 1;
  let age: number | null = null;
  let overdue = false;
  let due: number | null = null;
  for (const p of s.providers) {
    const mine = s.crls.filter((h) => h.provider === p.label);
    if (mine.length === 0) continue;
    const newest = mine.reduce((a, b) => (b.issued_at > a.issued_at ? b : a));
    const a = Math.max(0, (trust.t_ns - newest.issued_at) / 1e9);
    age = age === null ? a : Math.max(age, a);
    if (newest.sla_s < 0) overdue = true;
    due = due === null ? newest.sla_s : Math.min(due, newest.sla_s);
  }
  const open = cases ? cases.filter((c) => !c.stages.some((x) => x.stage === "enforced" || x.stage === "refused")).length : null;
  let devices = 0;
  let revoked = 0;
  for (const p of s.providers) {
    devices += p.devices.devices;
    revoked += p.devices.revoked;
  }
  // With the vehicles (ground truth), a device is revoked when a published list names it,
  // installed or not; without them, the providers' own count.
  if (vehicles) revoked = vehicles.filter((v) => v.revoked === true || (v.truth?.revoked_certs ?? 0) > 0).length;
  return {
    revokedByList: vehicles !== null,
    certificates: vehicles ? vehicles.reduce((n, v) => n + v.usable, 0) : null,
    pending,
    openCases: open,
    crlAgeS: age,
    crlOverdue: overdue,
    crlDueS: due,
    vehicles: vehicles ? vehicles.length : null,
    devices,
    revoked,
  };
}

/** A duration in plain words, coarse: "40 s", "12 min", "3 h", "2 d 4 h". */
export function ageText(s: number): string {
  const v = Math.round(s);
  if (v < 60) return `${v} s`;
  if (v < 3600) return `${Math.floor(v / 60)} min`;
  if (v < 86_400) return `${Math.floor(v / 3600)} h ${Math.floor((v % 3600) / 60)} min`;
  return `${Math.floor(v / 86_400)} d ${Math.floor((v % 86_400) / 3600)} h`;
}

// ---------------------------------------------------------------------------------------------
// A component's counters in words
// ---------------------------------------------------------------------------------------------

/** One engine counter in plain words; `unit` "s" values are drawn as a duration. */
export interface CounterText {
  readonly label: string;
  readonly means?: string;
  readonly unit?: "s";
}

/**
 * The counters the engine's components report (`pki.state` `counters`), in words. A counter not
 * here is shown only under "Raw counters"; so is one that repeats another (`case_open`) or is drawn
 * elsewhere on the card (`certificate`, the card itself).
 */
export const COUNTER_TEXT: Readonly<Record<string, CounterText>> = {
  gpf_version: { label: "Global policy file version (GPF)", means: "The system-wide policy every component and device follows" },
  gccf_version: { label: "Certificate chain file version (GCCF)", means: "The bundle of authority certificates devices are given" },
  lpf_version: { label: "Local policy file version (LPF)", means: "The provider's own policy for its devices" },
  policy_decisions: { label: "Policy decisions taken" },
  ctl_sequence: { label: "Trust list version it holds (CTL sequence)" },
  electors: { label: "Electors in the system" },
  quorum: { label: "Endorsements a trust-list change needs" },
  endorsements: {
    label: "Trust-list changes endorsed this run",
    means: "The trust list in force at the start was endorsed before the run, so it is not counted here",
  },
  certs_per_period: { label: "Pseudonym certificates per vehicle per week", means: "j-max: how many a vehicle may use in one i-period" },
  i_period_s: { label: "Length of one pseudonym week (i-period)", means: "The run compresses the calendar: a real i-period is one week", unit: "s" },
  files_signed: { label: "Policy and trust files signed" },
  certs_issued: { label: "Certificates issued" },
  devices_bootstrapped: { label: "Devices set up at the factory or dealer" },
  trust_bundles: { label: "Trust bundles handed out", means: "The roots, the trust list and the policy a new device starts with" },
  blocklisted: { label: "Enrolment certificates blocklisted", means: "Revoked devices: their requests for new certificates are refused" },
  blocklist: { label: "Enrolment certificates blocklisted", means: "Revoked devices: their requests for new certificates are refused" },
  enrolment_certs_issued: { label: "Enrolment certificates issued" },
  successors_issued: { label: "Replacement enrolment certificates issued", means: "Re-enrolment before the old one expires" },
  successors_refused: { label: "Replacement enrolment certificates refused" },
  batches_held: { label: "Pseudonym batches ready for download" },
  clipped_to_policy: { label: "Requests trimmed to the policy's limits" },
  enrolled_devices: { label: "Devices registered with it" },
  files_served: { label: "Policy files served to devices" },
  jobs_pending: { label: "Certificate jobs waiting at the Pseudonym CA (ACA)" },
  refused_blocklisted: { label: "Requests refused: device blocklisted" },
  refused_enrolment: { label: "Requests refused: enrolment certificate not accepted" },
  reports_in_shuffle: { label: "Misbehaviour reports waiting in the shuffle", means: "The RA holds and mixes reports before forwarding them, so the MA cannot tell who sent which" },
  requests_accepted: { label: "Certificate requests accepted" },
  shuffle_window_s: { label: "Shuffle window", means: "How long the RA holds requests and reports to mix them", unit: "s" },
  chains: { label: "Linkage chains it keeps", means: "One per device: the seeds its pseudonyms' linkage values come from" },
  plv_issued: { label: "Pre-linkage values issued", means: "Encrypted halves of each pseudonym's linkage value" },
  seeds_released: { label: "Linkage seeds released for revocation", means: "Handed to the MA so a CRL can name a device's pseudonyms" },
  lookups_answered: { label: "Investigation look-ups answered for the MA" },
  cases_open: { label: "Cases open now" },
  open_cases: { label: "Cases open now" },
  cases_total: { label: "Cases opened in total" },
  reports_received: { label: "Misbehaviour reports received" },
  revocations: { label: "Devices revoked" },
  cadence_s: { label: "Publishes a revocation list every", unit: "s" },
  entries: { label: "Entries it holds", means: "Revocations on its published list plus any waiting for the next one; the table below counts each published list's own entries" },
  versions_signed: { label: "Revocation list versions signed" },
  versions: { label: "Revocation list versions held" },
  awaiting_review: { label: "Cases waiting for an operator's review" },
  cases: { label: "Cases opened in total" },
  certificates_issued: { label: "Certificates issued" },
  decisions: { label: "Decisions taken (revoke or not)" },
  enrolments: { label: "Vehicles enrolled" },
  la_id: { label: "Its Linkage Authority number", means: "1 or 2: which half of every linkage value it supplies" },
  opened_reports: { label: "Misbehaviour reports it decrypted" },
  per_certificate_handles: { label: "Request handles it keeps, one per certificate", means: "What lets the RA, asked by the MA, find the enrolment certificate behind a pseudonym, without the ACA learning it" },
  plvs_issued: { label: "Pre-linkage values issued", means: "Its encrypted halves of linkage values (PLVs), one per certificate" },
  refused: { label: "Requests refused" },
  reports: { label: "Misbehaviour reports" },
  requests: { label: "Requests received" },
  successors: { label: "Replacement enrolment certificates issued" },
  enrolment_certificates_issued: { label: "Enrolment certificates issued" },
};

/** Counters never listed with the readable ones: the card shows them another way. */
const COUNTER_HIDDEN = new Set(["certificate", "case_open", "cadence_s"]);

/** A counter's words, or null when it belongs under "Raw counters" only. */
export function counterText(key: string): CounterText | null {
  return COUNTER_HIDDEN.has(key) ? null : (COUNTER_TEXT[key] ?? null);
}

/** One row of a card's recent messages: identical messages in a burst, counted once. */
export interface MessageGroup {
  readonly first: TrustMessage;
  readonly count: number;
  /** Simulated ns from the first to the last of the burst. */
  readonly span: number;
  readonly bytes: number;
}

/**
 * Consecutive messages (as listed) with the same name, sender and receiver within `windowNs` of
 * the burst's first, as one row: "14 × certificate response to the RA in 3 ms".
 */
export function groupMessages(ms: readonly TrustMessage[], windowNs = 1e9): MessageGroup[] {
  const out: { first: TrustMessage; count: number; lo: number; hi: number; bytes: number }[] = [];
  for (const m of ms) {
    const g = out[out.length - 1];
    if (g && g.first.name === m.name && g.first.from === m.from && g.first.to === m.to && Math.abs(m.t - g.first.t) <= windowNs) {
      g.count += 1;
      g.lo = Math.min(g.lo, m.t);
      g.hi = Math.max(g.hi, m.t);
      g.bytes += m.bytes ?? 0;
    } else out.push({ first: m, count: 1, lo: m.t, hi: m.t, bytes: m.bytes ?? 0 });
  }
  return out.map((g) => ({ first: g.first, count: g.count, span: g.hi - g.lo, bytes: g.bytes }));
}

// ---------------------------------------------------------------------------------------------
// The revocation-list schedule, said once
// ---------------------------------------------------------------------------------------------

/** A provider's own publication rule (`crl_publication`) in words. */
export function publicationRule(publication: string | null | undefined): string | null {
  switch (publication) {
    case "sla-1-business-day":
      return "a revocation must be on a published list within 1 business day of the decision";
    case "on-decision":
      return "a revocation is published as soon as it is decided";
    case "cadence":
      return "lists are published on a fixed schedule";
    default:
      return publication ? publication.replace(/-/g, " ") : null;
  }
}

/**
 * How a provider's revocation lists come out, as one sentence every view uses: the schedule the
 * run follows (the CRL Generator's `cadence_s`, which a scenario sets in place of the provider's
 * own rule), the provider's rule, and the deadline the newest list prints for the next (`nextCrl`,
 * seconds from now; negative when overdue).
 */
export function crlScheduleText(o: { readonly cadenceS: number | null; readonly publication: string | null; readonly dueS: number | null }): string {
  const rule = publicationRule(o.publication);
  const parts: string[] = [];
  if (o.cadenceS !== null && o.cadenceS > 0) {
    parts.push(
      `New revocations go out on a list every ${ageText(o.cadenceS)}` +
        (rule && o.publication !== "cadence" ? ` (the scenario's schedule; the provider's own rule, that ${rule}, is set aside)` : "") +
        ".",
    );
  } else if (rule) {
    parts.push(`${rule.charAt(0).toUpperCase()}${rule.slice(1)}.`);
  }
  if (o.dueS !== null) {
    parts.push(
      o.dueS < 0
        ? `The newest list's deadline for a replacement passed ${ageText(-o.dueS)} ago (overdue).`
        : `The next list is due within ${ageText(o.dueS)} at the latest (the deadline the newest list prints, its nextCrl).`,
    );
  }
  return parts.join(" ");
}

/** `ieee-1609.2-2022` → `IEEE 1609.2-2022`. */
export function editionText(edition: string): string {
  return edition.replace(/^ieee-/i, "IEEE ");
}

/** `us-production-2025` → `US SCMS (production, 2025)`; an unknown preset keeps its words. */
export function deploymentText(preset: string): string {
  const m = /^us-([a-z-]+?)(?:-(\d{4}))?$/i.exec(preset);
  if (!m) return preset.replace(/-/g, " ");
  return `US SCMS (${m[1].replace(/-/g, " ")}${m[2] ? `, ${m[2]}` : ""})`;
}

/** The MA's decision mode in words. */
export function maModeText(mode: string): string {
  if (mode === "automatic") return "MA decides automatically";
  if (mode === "manual-review") return "MA: an operator reviews each case";
  return `MA: ${mode.replace(/-/g, " ")}`;
}
