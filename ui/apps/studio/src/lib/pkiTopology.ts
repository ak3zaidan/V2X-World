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
    term: "Authorization Certificate Authority (ACA, the pseudonym CA)",
    does: "Signs the pseudonym certificates, 20 a week per device, without learning whose they are.",
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
    name: "List broadcast",
    term: "CRL Broadcast",
    does: "Hands the revocation list to roadside units, which rebroadcast it over the air.",
  },
  pg: {
    name: "Policy",
    term: "Policy Generator (PG)",
    does: "Signs the global policy file and distributes the certificate trust list to devices.",
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

export function flowInfo(flow: string): { readonly name: string; readonly color: string } {
  return FLOW_INFO[flow] ?? { name: flow, color: "var(--text-dim)" };
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
  const mgr = managers.find((c) => c.role === "manager");
  if (mgr) for (const e of electors) add(e.id, mgr.id, true);
  for (const m of messages) {
    const a = nodeOf(m.from, m.to, m.provider);
    const b = nodeOf(m.to, m.from, m.provider);
    if (a && b) add(a, b, false);
  }
  const height = sideBySide ? top + regionH : top + n * regionH + (n - 1) * GAP;
  return { width: totalW, height, governance: { x: 0, y: 0, w: totalW, h: GOV_H }, regions, nodes, links: [...links.values()], sideBySide };
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

/** The path a message takes, sender to receiver, or null when either end is not on the diagram. */
export function messagePath(t: Topology, m: TrustMessage): string | null {
  const byId = new Map(t.nodes.map((nd) => [nd.id, nd]));
  const a = byId.get(nodeOf(m.from, m.to, m.provider) ?? "");
  const b = byId.get(nodeOf(m.to, m.from, m.provider) ?? "");
  if (!a || !b || a === b) return null;
  return linkPath(a, b).d;
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
  readonly devices: number;
  readonly revoked: number;
}

/** The strip from the newest trust snapshot and, when the view allows, the vehicles and cases. */
export function healthOf(
  trust: TrustAnswer,
  vehicles: readonly { readonly usable: number; readonly security?: { readonly topup_in_flight?: boolean } | null }[] | null,
  cases: readonly { readonly stages: readonly { readonly stage: string }[] }[] | null,
): Health {
  const s = trust.state;
  let pending = 0;
  for (const p of s.providers) for (const c of p.components) pending += c.queue?.depth ?? 0;
  for (const c of s.governance) pending += c.queue?.depth ?? 0;
  if (vehicles) for (const v of vehicles) if (v.security?.topup_in_flight === true) pending += 1;
  let age: number | null = null;
  let overdue = false;
  for (const p of s.providers) {
    const mine = s.crls.filter((h) => h.provider === p.label);
    if (mine.length === 0) continue;
    const newest = mine.reduce((a, b) => (b.issued_at > a.issued_at ? b : a));
    const a = Math.max(0, (trust.t_ns - newest.issued_at) / 1e9);
    age = age === null ? a : Math.max(age, a);
    if (newest.sla_s < 0) overdue = true;
  }
  const open = cases ? cases.filter((c) => !c.stages.some((x) => x.stage === "enforced" || x.stage === "refused")).length : null;
  let devices = 0;
  let revoked = 0;
  for (const p of s.providers) {
    devices += p.devices.devices;
    revoked += p.devices.revoked;
  }
  return {
    certificates: vehicles ? vehicles.reduce((n, v) => n + v.usable, 0) : null,
    pending,
    openCases: open,
    crlAgeS: age,
    crlOverdue: overdue,
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
