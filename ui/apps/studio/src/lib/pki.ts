/**
 * The PKI view's model (WP-E2, `docs/design/scms-real/00-gap-analysis-and-plan.md` §3.2): the
 * Trust domain and Flows tabs, read from the engine's PKI records (WP-E1,
 * `v2xw_proto::scms::pki_view`) through `inspect.trust`, `inspect.flows`, `inspect.flow`,
 * `inspect.pdu`, `inspect.cert` and `inspect.knowledge`.
 *
 * Everything here is a pure function of an engine answer, so the components draw and the tests
 * check one thing. The model computes nothing the engine did not publish: a component's status is
 * the engine's (`online`, `offline` by design, `faulted` by an injected outage), its activity is
 * the engine's two-second traffic delta, a flow's order is the engine's timestamps. What it adds
 * is layout — which lane a message runs between, which gap is a shuffle wait — and the per-entity
 * view's rule for what an entity observed.
 */

// ---------------------------------------------------------------------------------------------
// Shapes (the subset the view reads; every other field is kept on the raw object)
// ---------------------------------------------------------------------------------------------

/** A component's queue (`v2xw_proto::kernel::QueueView`). */
export interface PkiQueue {
  readonly depth: number;
  readonly servers: number;
  readonly served: number;
  readonly inbound: number;
  readonly waited_ns: number;
  readonly busy_ns: number;
  readonly backlog_ns: number;
}

/** A component's message and byte counts. */
export interface PkiTraffic {
  readonly sent: number;
  readonly received: number;
  readonly bytes_in: number;
  readonly bytes_out: number;
}

/** One component (`ComponentState`). */
export interface PkiComponent {
  readonly id: string;
  readonly provider: string;
  readonly role: string;
  readonly instance: number;
  readonly name: string;
  readonly status: "online" | "offline" | "faulted" | string;
  readonly node: number | null;
  readonly cert: string | null;
  readonly queue: PkiQueue | null;
  readonly traffic: PkiTraffic;
  readonly ops: Readonly<Record<string, number>>;
  readonly counters: Readonly<Record<string, unknown>>;
}

/** A provider's region, always an assumption. */
export interface PkiRegion {
  readonly name: string;
  readonly metro: string;
  readonly lat_deg: number;
  readonly lon_deg: number;
  readonly assumption: boolean;
}

/** A provider's devices. */
export interface PkiDevices {
  readonly devices: number;
  readonly enrolled: number;
  readonly provisioned: number;
  readonly revoked: number;
  readonly blocklisted: number;
}

/** One provider (`ProviderState`). */
export interface PkiProvider {
  readonly label: string;
  readonly index: number;
  readonly home: boolean;
  readonly region: PkiRegion;
  readonly on_ctl: boolean;
  readonly ma_mode: string;
  readonly ma_review_s: number | null;
  readonly crl_publication: string;
  readonly fleet_share: number;
  readonly components: readonly PkiComponent[];
  readonly devices: PkiDevices;
}

/** One past CTL. */
export interface PkiCtlEntry {
  readonly sequence: number;
  readonly effective: number;
  readonly roots: number;
  readonly signers: readonly number[];
}

/** The CTL (`CtlState`). */
export interface PkiCtl {
  readonly sequence: number;
  readonly quorum: number;
  readonly electors: number;
  readonly roots: readonly string[];
  readonly effective: number;
  readonly signers: readonly number[];
  readonly bytes: number;
  readonly history: readonly PkiCtlEntry[];
  readonly pending: number;
}

/** One CRL head. */
export interface PkiCrlHead {
  readonly provider: string;
  readonly series: number;
  readonly kind: string;
  readonly delta: boolean;
  readonly version: number;
  readonly issue_date: number;
  readonly next_crl: number;
  readonly entries: number;
  readonly bytes: number;
  readonly issued_at: number;
  readonly sla_s: number;
  readonly id8: string;
}

/** `pki.state`, as `inspect.trust` returns it (no certificates, no CRL bytes). */
export interface PkiState {
  readonly schema: number;
  readonly t: number;
  readonly time32: number;
  readonly edition: string;
  readonly preset: string;
  readonly governance: readonly PkiComponent[];
  readonly providers: readonly PkiProvider[];
  readonly ctl: PkiCtl;
  readonly crls: readonly PkiCrlHead[];
  readonly assumptions: readonly string[];
}

/** One component's traffic over the activity window. */
export interface PkiActivity {
  readonly messages: number;
  readonly bytes: number;
}

/** `inspect.trust`'s answer. */
export interface TrustAnswer {
  readonly t_ns: number;
  readonly state: PkiState;
  /** The span the activity covers, ns, or null for the first snapshot. */
  readonly window_ns: number | null;
  readonly activity: Readonly<Record<string, PkiActivity>>;
  readonly certificates_known: number;
  readonly history: number;
  /** The messages that crossed a link in the window (raw; `pkiTopology.messagesOf` reads them). */
  readonly messages: readonly unknown[];
  /** Messages of the window beyond the answer's cap, not listed. */
  readonly messages_omitted: number;
  /** False when the server's log no longer reaches back to the window's start. */
  readonly messages_complete: boolean;
}

/** One SPDU a flow message carried, without its bytes. */
export interface FlowPdu {
  readonly id: string;
  readonly type: string;
  readonly len: number;
  readonly from: string;
  readonly to: string;
  readonly t: number;
  readonly id8?: string;
  readonly sealed_to?: string;
  readonly opened?: { readonly by: string; readonly error?: string };
}

/** One `pki.flow` record. */
export interface FlowRecord {
  readonly kind: "msg" | "stage" | "pdu" | string;
  readonly id: string;
  readonly t: number;
  readonly provider: string;
  readonly flow: string;
  readonly instance: string;
  readonly name: string;
  readonly from?: string;
  readonly to?: string;
  readonly bytes?: number;
  readonly transport?: string;
  readonly pdus?: readonly FlowPdu[];
}

/** One row of `inspect.flows`. */
export interface FlowRow {
  readonly instance: string;
  readonly provider: string;
  readonly flow: string;
  readonly first_t: number;
  readonly last_t: number;
  readonly messages: number;
  readonly stages: number;
  readonly bytes: number;
  readonly pdus: number;
  readonly devices: readonly string[];
  readonly device_count: number;
  readonly last_stage: string | null;
}

/** `inspect.flows`'s answer. */
export interface FlowsAnswer {
  readonly instances: readonly FlowRow[];
  readonly total: number;
  readonly flows: Readonly<Record<string, number>>;
}

/** `inspect.flow`'s answer. */
export interface FlowDetail extends FlowRow {
  readonly records: readonly FlowRecord[];
  readonly dropped: number;
}

/** One decoded layer of an SPDU (`decode_tree`). */
export interface PduLayer {
  readonly path: string;
  readonly type: string;
  readonly hex: string;
  readonly len: number;
  readonly content?: string;
  readonly jer?: unknown;
}

/** `inspect.pdu`'s answer. */
export interface PduAnswer {
  readonly id: string;
  readonly type: string;
  readonly len: number;
  readonly from: string;
  readonly to: string;
  readonly t: number;
  readonly id8: string | null;
  readonly hex: string | null;
  readonly layers: readonly PduLayer[];
  readonly sealed_to: string | null;
  /** The plaintext, when the view may see it; `hidden` says why not otherwise. */
  readonly opened:
    | { readonly by: string; readonly hex: string | null; readonly error: string | null; readonly layers: readonly PduLayer[] }
    | { readonly by: string; readonly hidden: string }
    | null;
  readonly view: string;
}

/** One certificate validation step. */
export interface CertStep {
  readonly rule: string;
  readonly status: "pass" | "fail" | "skipped" | string;
  readonly detail: readonly string[];
}

/** `inspect.cert`'s answer. */
export interface CertAnswer {
  readonly id8: string;
  readonly chain: readonly string[];
  readonly valid: boolean;
  readonly time32: number | null;
  readonly trace: readonly CertStep[];
  readonly tree: unknown;
}

/** `inspect.knowledge`'s answer. */
export interface KnowledgeAnswer {
  readonly entity: string;
  readonly role: string;
  readonly t: number;
  readonly view: string;
  readonly holds: Readonly<Record<string, unknown>>;
  readonly cannot_see: readonly string[];
  /** The ground-truth join; present only in the ground-truth view. */
  readonly truth?: Readonly<Record<string, unknown>>;
}

// ---------------------------------------------------------------------------------------------
// Reading answers
// ---------------------------------------------------------------------------------------------

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const num = (v: unknown, d = 0): number => (typeof v === "number" && Number.isFinite(v) ? v : d);
const str = (v: unknown, d = ""): string => (typeof v === "string" ? v : d);
const arr = (v: unknown): readonly unknown[] => (Array.isArray(v) ? v : []);

function component(v: unknown): PkiComponent | null {
  if (!isObj(v) || typeof v.id !== "string" || typeof v.role !== "string") return null;
  const t = isObj(v.traffic) ? v.traffic : {};
  const q = isObj(v.queue) ? v.queue : null;
  return {
    id: v.id,
    provider: str(v.provider),
    role: v.role,
    instance: num(v.instance),
    name: str(v.name, v.role),
    status: str(v.status, "online"),
    node: typeof v.node === "number" ? v.node : null,
    cert: typeof v.cert === "string" ? v.cert : null,
    queue: q
      ? {
          depth: num(q.depth),
          servers: num(q.servers, 1),
          served: num(q.served),
          inbound: num(q.inbound),
          waited_ns: num(q.waited_ns),
          busy_ns: num(q.busy_ns),
          backlog_ns: num(q.backlog_ns),
        }
      : null,
    traffic: { sent: num(t.sent), received: num(t.received), bytes_in: num(t.bytes_in), bytes_out: num(t.bytes_out) },
    ops: isObj(v.ops) ? (Object.fromEntries(Object.entries(v.ops).map(([k, x]) => [k, num(x)])) as Record<string, number>) : {},
    counters: isObj(v.counters) ? v.counters : {},
  };
}

const components = (v: unknown): PkiComponent[] => arr(v).map(component).filter((c): c is PkiComponent => c !== null);

/** `inspect.trust`'s answer, or null for anything else. */
export function trustOf(answer: unknown): TrustAnswer | null {
  if (!isObj(answer) || !isObj(answer.state)) return null;
  const s = answer.state;
  if (!Array.isArray(s.providers) || !isObj(s.ctl)) return null;
  const ctl = s.ctl;
  const providers: PkiProvider[] = arr(s.providers)
    .filter(isObj)
    .map((p) => {
      const r = isObj(p.region) ? p.region : {};
      const d = isObj(p.devices) ? p.devices : {};
      return {
        label: str(p.label),
        index: num(p.index),
        home: p.home === true,
        region: { name: str(r.name), metro: str(r.metro), lat_deg: num(r.lat_deg), lon_deg: num(r.lon_deg), assumption: r.assumption !== false },
        on_ctl: p.on_ctl === true,
        ma_mode: str(p.ma_mode),
        ma_review_s: typeof p.ma_review_s === "number" ? p.ma_review_s : null,
        crl_publication: str(p.crl_publication),
        fleet_share: num(p.fleet_share),
        components: components(p.components),
        devices: { devices: num(d.devices), enrolled: num(d.enrolled), provisioned: num(d.provisioned), revoked: num(d.revoked), blocklisted: num(d.blocklisted) },
      };
    });
  const activity: Record<string, PkiActivity> = {};
  if (isObj(answer.activity)) {
    for (const [id, a] of Object.entries(answer.activity)) {
      if (isObj(a)) activity[id] = { messages: num(a.messages), bytes: num(a.bytes) };
    }
  }
  return {
    t_ns: num(answer.t_ns, num(s.t)),
    state: {
      schema: num(s.schema),
      t: num(s.t),
      time32: num(s.time32),
      edition: str(s.edition),
      preset: str(s.preset),
      governance: components(s.governance),
      providers,
      ctl: {
        sequence: num(ctl.sequence),
        quorum: num(ctl.quorum),
        electors: num(ctl.electors),
        roots: arr(ctl.roots).map((x) => str(x)),
        effective: num(ctl.effective),
        signers: arr(ctl.signers).map((x) => num(x)),
        bytes: num(ctl.bytes),
        history: arr(ctl.history)
          .filter(isObj)
          .map((h) => ({ sequence: num(h.sequence), effective: num(h.effective), roots: num(h.roots), signers: arr(h.signers).map((x) => num(x)) })),
        pending: num(ctl.pending),
      },
      crls: arr(s.crls)
        .filter(isObj)
        .map((c) => ({
          provider: str(c.provider),
          series: num(c.series),
          kind: str(c.kind),
          delta: c.delta === true,
          version: num(c.version),
          issue_date: num(c.issue_date),
          next_crl: num(c.next_crl),
          entries: num(c.entries),
          bytes: num(c.bytes),
          issued_at: num(c.issued_at),
          sla_s: num(c.sla_s),
          id8: str(c.id8),
        })),
      assumptions: arr(s.assumptions).map((x) => str(x)),
    },
    window_ns: typeof answer.window_ns === "number" ? answer.window_ns : null,
    activity,
    certificates_known: num(answer.certificates_known),
    history: num(answer.history),
    messages: Array.isArray(answer.messages) ? answer.messages : [],
    messages_omitted: num(answer.messages_omitted),
    messages_complete: answer.messages_complete !== false,
  };
}

function flowRow(v: Obj): FlowRow {
  return {
    instance: str(v.instance),
    provider: str(v.provider),
    flow: str(v.flow),
    first_t: num(v.first_t),
    last_t: num(v.last_t),
    messages: num(v.messages),
    stages: num(v.stages),
    bytes: num(v.bytes),
    pdus: num(v.pdus),
    devices: arr(v.devices).map((x) => str(x)),
    device_count: num(v.device_count),
    last_stage: typeof v.last_stage === "string" ? v.last_stage : null,
  };
}

/** `inspect.flows`'s answer, or null. */
export function flowsOf(answer: unknown): FlowsAnswer | null {
  if (!isObj(answer) || !Array.isArray(answer.instances)) return null;
  const flows: Record<string, number> = {};
  if (isObj(answer.flows)) for (const [k, v] of Object.entries(answer.flows)) flows[k] = num(v);
  return { instances: answer.instances.filter(isObj).map(flowRow), total: num(answer.total), flows };
}

/** `inspect.flow`'s answer, or null. */
export function flowDetailOf(answer: unknown): FlowDetail | null {
  if (!isObj(answer) || !Array.isArray(answer.records) || typeof answer.instance !== "string") return null;
  const records: FlowRecord[] = answer.records.filter(isObj).map((r) => ({
    kind: str(r.kind),
    id: str(r.id),
    t: num(r.t),
    provider: str(r.provider),
    flow: str(r.flow),
    instance: str(r.instance),
    name: str(r.name),
    ...(typeof r.from === "string" ? { from: r.from } : {}),
    ...(typeof r.to === "string" ? { to: r.to } : {}),
    ...(typeof r.bytes === "number" ? { bytes: r.bytes } : {}),
    ...(typeof r.transport === "string" ? { transport: r.transport } : {}),
    pdus: arr(r.pdus)
      .filter(isObj)
      .map((p) => ({
        id: str(p.id),
        type: str(p.type),
        len: num(p.len),
        from: str(p.from),
        to: str(p.to),
        t: num(p.t),
        ...(typeof p.id8 === "string" ? { id8: p.id8 } : {}),
        ...(typeof p.sealed_to === "string" ? { sealed_to: p.sealed_to } : {}),
        ...(isObj(p.opened) ? { opened: { by: str(p.opened.by), ...(typeof p.opened.error === "string" ? { error: p.opened.error } : {}) } } : {}),
      })),
  }));
  return { ...flowRow(answer), records, dropped: num(answer.dropped) };
}

function layers(tree: unknown): PduLayer[] {
  if (!isObj(tree)) return [];
  return arr(tree.layers)
    .filter(isObj)
    .map((l) => ({
      path: str(l.path),
      type: str(l.type),
      hex: str(l.hex),
      len: num(l.len),
      ...(typeof l.content === "string" ? { content: l.content } : {}),
      ...("jer" in l ? { jer: l.jer } : {}),
    }));
}

/** `inspect.pdu`'s answer, or null. */
export function pduOf(answer: unknown): PduAnswer | null {
  if (!isObj(answer) || typeof answer.id !== "string" || typeof answer.type !== "string") return null;
  let opened: PduAnswer["opened"] = null;
  const o = answer.opened;
  if (isObj(o)) {
    opened =
      typeof o.hidden === "string"
        ? { by: str(o.by), hidden: o.hidden }
        : { by: str(o.by), hex: typeof o.hex === "string" ? o.hex : null, error: typeof o.error === "string" ? o.error : null, layers: layers(o.tree) };
  }
  return {
    id: answer.id,
    type: answer.type,
    len: num(answer.len),
    from: str(answer.from),
    to: str(answer.to),
    t: num(answer.t),
    id8: typeof answer.id8 === "string" ? answer.id8 : null,
    hex: typeof answer.hex === "string" ? answer.hex : null,
    layers: layers(answer.tree),
    sealed_to: typeof answer.sealed_to === "string" ? answer.sealed_to : null,
    opened,
    view: str(answer.view, GROUND_TRUTH),
  };
}

/** `inspect.cert`'s answer, or null. */
export function certOf(answer: unknown): CertAnswer | null {
  if (!isObj(answer) || typeof answer.id8 !== "string" || !Array.isArray(answer.trace)) return null;
  return {
    id8: answer.id8,
    chain: arr(answer.chain).map((x) => str(x)),
    valid: answer.valid === true,
    time32: typeof answer.time32 === "number" ? answer.time32 : null,
    trace: answer.trace.filter(isObj).map((s) => ({ rule: str(s.rule), status: str(s.status), detail: arr(s.detail).map((x) => str(x)) })),
    tree: answer.tree,
  };
}

/** `inspect.knowledge`'s answer, or null. */
export function knowledgeOf(answer: unknown): KnowledgeAnswer | null {
  if (!isObj(answer) || typeof answer.entity !== "string" || !isObj(answer.holds)) return null;
  return {
    entity: answer.entity,
    role: str(answer.role),
    t: num(answer.t),
    view: str(answer.view, GROUND_TRUTH),
    holds: answer.holds,
    cannot_see: arr(answer.cannot_see).map((x) => str(x)),
    ...(isObj(answer.truth) ? { truth: answer.truth } : {}),
  };
}

// ---------------------------------------------------------------------------------------------
// The Trust domain
// ---------------------------------------------------------------------------------------------

/** The view that sees everything. */
export const GROUND_TRUTH = "ground-truth";

/** Short role labels as the SCMS documents print them. */
const ROLE_LABEL: Readonly<Record<string, string>> = {
  root: "RCA",
  ica: "ICA",
  eca: "ECA",
  dcm: "DCM",
  ra: "RA",
  lop: "LOP",
  aca: "ACA",
  la1: "LA1",
  la2: "LA2",
  ma: "MA",
  pg: "PG",
  crlg: "CRLG",
  "crl-store": "CRL store",
  "crl-broadcast": "CRL bcast",
  manager: "SCMS Manager",
  elector: "Elector",
};

/** A component role's short label (`RA`, `LA1`, `RCA`). */
export function roleLabel(role: string): string {
  return ROLE_LABEL[role] ?? role.toUpperCase();
}

/**
 * The order the Trust domain lays a provider's components out in: the offline chain first (root,
 * then the ICA it certifies), then the online components in the order a request meets them —
 * enrolment (DCM, ECA), provisioning (LOP, RA, LAs, ACA), misbehaviour (MA, CRLG and its
 * distribution), policy (PG).
 */
export const ROLE_ORDER: readonly string[] = ["root", "ica", "dcm", "eca", "lop", "ra", "la1", "la2", "aca", "ma", "crlg", "crl-store", "crl-broadcast", "pg"];

function roleRank(role: string): number {
  const i = ROLE_ORDER.indexOf(role);
  return i < 0 ? ROLE_ORDER.length : i;
}

/** A provider's components split into the offline chain and the online rest, each in [`ROLE_ORDER`]. */
export function providerLayout(p: PkiProvider): { readonly offline: readonly PkiComponent[]; readonly online: readonly PkiComponent[] } {
  const sorted = [...p.components].sort((a, b) => roleRank(a.role) - roleRank(b.role) || a.instance - b.instance);
  return { offline: sorted.filter((c) => c.status === "offline"), online: sorted.filter((c) => c.status !== "offline") };
}

/** The CSS state classes of one component box: its status, and `busy` while it has traffic. */
export function componentClasses(c: PkiComponent, activity: Readonly<Record<string, PkiActivity>>): string[] {
  const out = ["pki-comp", `status-${c.status}`];
  if ((activity[c.id]?.messages ?? 0) > 0) out.push("busy");
  if (c.queue && c.queue.depth > 0) out.push("queued");
  return out;
}

/** Every component of the state, governance first, by id. */
export function componentIndex(state: PkiState): ReadonlyMap<string, PkiComponent> {
  const m = new Map<string, PkiComponent>();
  for (const c of state.governance) m.set(c.id, c);
  for (const p of state.providers) for (const c of p.components) m.set(c.id, c);
  return m;
}

/** The roles whose own knowledge the engine publishes (`pki.knowledge`). */
export const KNOWLEDGE_ROLES: readonly string[] = ["ra", "aca", "la1", "la2", "ma", "eca"];

/** One choice of the view switch. */
export interface ViewOption {
  readonly id: string;
  readonly label: string;
  readonly provider: string | null;
}

/**
 * The view switch's choices: ground truth, then each knowledge-bearing component of every
 * provider ("as RA", "as ACA", "as LA1" ...), labelled with its provider when there is more than one.
 */
export function viewOptions(state: PkiState | null): ViewOption[] {
  const out: ViewOption[] = [{ id: GROUND_TRUTH, label: "ground truth", provider: null }];
  if (!state) return out;
  const many = state.providers.length > 1;
  for (const p of state.providers) {
    const comps = [...p.components].filter((c) => KNOWLEDGE_ROLES.includes(c.role)).sort((a, b) => KNOWLEDGE_ROLES.indexOf(a.role) - KNOWLEDGE_ROLES.indexOf(b.role));
    for (const c of comps) out.push({ id: c.id, label: `as ${roleLabel(c.role)}${many ? ` (${p.label.replace(/^provider-/, "")})` : ""}`, provider: p.label });
  }
  return out;
}

/** The devices line under the providers: totals and each provider's share. */
export function devicesLine(state: PkiState): { readonly total: PkiDevices; readonly perProvider: readonly { readonly label: string; readonly devices: number }[] } {
  const total = { devices: 0, enrolled: 0, provisioned: 0, revoked: 0, blocklisted: 0 };
  for (const p of state.providers) {
    total.devices += p.devices.devices;
    total.enrolled += p.devices.enrolled;
    total.provisioned += p.devices.provisioned;
    total.revoked += p.devices.revoked;
    total.blocklisted += p.devices.blocklisted;
  }
  return { total, perProvider: state.providers.map((p) => ({ label: p.label, devices: p.devices.devices })) };
}

/** Which providers' roots the CTL lists, and whether every listed root belongs to a known provider. */
export function ctlSummary(state: PkiState): { readonly sequence: number; readonly roots: number; readonly quorum: string; readonly onCtl: readonly string[]; readonly offCtl: readonly string[] } {
  return {
    sequence: state.ctl.sequence,
    roots: state.ctl.roots.length,
    quorum: `${state.ctl.quorum} of ${state.ctl.electors}`,
    onCtl: state.providers.filter((p) => p.on_ctl).map((p) => p.label),
    offCtl: state.providers.filter((p) => !p.on_ctl).map((p) => p.label),
  };
}

// ---------------------------------------------------------------------------------------------
// The Flows tab: the sequence diagram
// ---------------------------------------------------------------------------------------------

/** One lane of the sequence diagram. */
export interface Lane {
  readonly id: string;
  readonly label: string;
  /** The provider, when it is not the instance's own. */
  readonly foreign: string | null;
}

/** One row of the sequence diagram: an arrow, a stage stamp or a lone PDU. */
export type SeqRow =
  | {
      readonly kind: "msg";
      readonly record: FlowRecord;
      readonly from: number;
      readonly to: number;
      /** Whether the chosen view's entity sent or received it (always, in ground truth). */
      readonly observed: boolean;
      readonly dt: number;
    }
  | { readonly kind: "stage"; readonly record: FlowRecord; readonly lane: number; readonly observed: boolean; readonly dt: number }
  | { readonly kind: "gap"; readonly ns: number; readonly shuffle: boolean; readonly stage: string | null };

/** A gap at least this long between two rows is drawn as a break with its duration. */
export const GAP_NS = 1_000_000_000;

/** The lane role order: the device, then the backend in the order a request meets it. */
const LANE_ORDER: readonly string[] = ["ee", "dcm", "eca", "lop", "ra", "la1", "la2", "aca", "ma", "crlg", "crl-store", "crl-broadcast", "pg", "manager", "elector", "root", "ica"];

function laneRole(id: string): string {
  if (id.startsWith("ee/")) return "ee";
  const parts = id.split("/");
  return parts.length >= 2 ? (parts[1] ?? id) : id;
}

function laneLabel(id: string): string {
  if (id.startsWith("ee/")) return `Vehicle ${id.slice(3)}`;
  const parts = id.split("/");
  const role = parts[1] ?? id;
  const inst = Number(parts[2] ?? "0");
  return `${roleLabel(role)}${inst > 0 ? ` #${inst}` : ""}`;
}

/** Whether `entity` (a view id) is the participant `id`. */
function isEntity(view: string, id: string | undefined): boolean {
  return id !== undefined && (id === view || (!view.includes("/") && laneRole(id) === view));
}

/** The lanes of one instance: every sender, receiver and stamping node, in [`LANE_ORDER`]. */
export function lanesOf(detail: FlowDetail): Lane[] {
  const ids = new Set<string>();
  for (const r of detail.records) {
    if (r.kind === "pdu") continue;
    if (r.from) ids.add(r.from);
    if (r.to) ids.add(r.to);
  }
  const rank = (id: string): number => {
    const i = LANE_ORDER.indexOf(laneRole(id));
    return i < 0 ? LANE_ORDER.length : i;
  };
  return [...ids]
    .sort((a, b) => rank(a) - rank(b) || a.localeCompare(b, "en", { numeric: true }))
    .map((id) => {
      const provider = id.startsWith("ee/") ? null : (id.split("/")[0] ?? null);
      return { id, label: laneLabel(id), foreign: provider !== null && provider !== detail.provider ? provider : null };
    });
}

/**
 * The sequence diagram of one instance as `view` sees it: one row per message (an arrow between
 * two lanes) and stage stamp (a mark on its lane), in the engine's time order (a stamp before the
 * messages of its own instant), with a gap row
 * wherever the next row is [`GAP_NS`] or more later. A gap that ends at a `shuffled` stamp is the
 * shuffle wait (00 §3.2: "the shuffle wait shows as a hatched gap"). Outside ground truth a row the
 * viewing entity neither sent, received nor stamped is marked unobserved.
 */
export function sequenceOf(detail: FlowDetail, view: string = GROUND_TRUTH): { readonly lanes: readonly Lane[]; readonly rows: readonly SeqRow[]; readonly t0: number } {
  const lanes = lanesOf(detail);
  const index = new Map(lanes.map((l, i) => [l.id, i]));
  const truth = view === GROUND_TRUTH;
  // The engine's time order; at one instant a stage stamp comes before the messages that leave
  // at it (the RA stamps `shuffled`, then sends the batch to the ACA).
  const recs = detail.records
    .filter((r) => r.kind === "msg" || r.kind === "stage")
    .map((r, i) => ({ r, i }))
    .sort((a, b) => a.r.t - b.r.t || (a.r.kind === b.r.kind ? 0 : a.r.kind === "stage" ? -1 : 1) || a.i - b.i)
    .map((x) => x.r);
  const t0 = recs[0]?.t ?? detail.first_t;
  const rows: SeqRow[] = [];
  let prev = t0;
  for (const r of recs) {
    const gap = r.t - prev;
    if (gap >= GAP_NS) rows.push({ kind: "gap", ns: gap, shuffle: r.kind === "stage" && r.name === "shuffled", stage: r.kind === "stage" ? r.name : null });
    if (r.kind === "msg") {
      rows.push({
        kind: "msg",
        record: r,
        from: index.get(r.from ?? "") ?? 0,
        to: index.get(r.to ?? "") ?? 0,
        observed: truth || isEntity(view, r.from) || isEntity(view, r.to),
        dt: r.t - t0,
      });
    } else {
      rows.push({ kind: "stage", record: r, lane: index.get(r.from ?? "") ?? 0, observed: truth || isEntity(view, r.from), dt: r.t - t0 });
    }
    prev = r.t;
  }
  return { lanes, rows, t0 };
}

/** The stage stamps of one instance with their offsets from its start: the per-stage timestamps. */
export function stagesOf(detail: FlowDetail): { readonly name: string; readonly by: string; readonly t: number; readonly dt: number; readonly step: number }[] {
  const stages = detail.records.filter((r) => r.kind === "stage");
  const t0 = detail.records[0]?.t ?? detail.first_t;
  let last = t0;
  return stages.map((s) => {
    const row = { name: s.name, by: s.from ?? "", t: s.t, dt: s.t - t0, step: s.t - last };
    last = s.t;
    return row;
  });
}

/** Whether the view may open a PDU in the decoder: ground truth, or an entity that sent or received it. */
export function canInspect(pdu: FlowPdu, record: FlowRecord, view: string): boolean {
  return view === GROUND_TRUTH || isEntity(view, record.from) || isEntity(view, record.to) || isEntity(view, pdu.from) || isEntity(view, pdu.to);
}

// ---------------------------------------------------------------------------------------------
// The PDU decoder
// ---------------------------------------------------------------------------------------------

/** One line of a JER tree as the decoder draws it. */
export interface TreeLine {
  /** `$.content.signedData...`, the JSON path of the value. */
  readonly path: string;
  readonly depth: number;
  /** The member name or `[i]`. */
  readonly key: string;
  /** The leaf's value as JSON text, or null for an object or array (its children follow). */
  readonly value: string | null;
  /** For an object or array, how many children. */
  readonly children: number;
}

/** A JER value flattened depth-first into the lines the decoder draws, every leaf once, in order. */
export function treeLines(jer: unknown, key = "$", depth = 0, path = "$"): TreeLine[] {
  if (Array.isArray(jer)) {
    const out: TreeLine[] = [{ path, depth, key, value: null, children: jer.length }];
    jer.forEach((v, i) => out.push(...treeLines(v, `[${i}]`, depth + 1, `${path}[${i}]`)));
    return out;
  }
  if (isObj(jer)) {
    const keys = Object.keys(jer);
    const out: TreeLine[] = [{ path, depth, key, value: null, children: keys.length }];
    for (const k of keys) out.push(...treeLines(jer[k], k, depth + 1, `${path}.${k}`));
    return out;
  }
  return [{ path, depth, key, value: JSON.stringify(jer ?? null), children: 0 }];
}

/** The leaves of a JER value as `path → JSON text`: what the decoder must show, and nothing else. */
export function jerLeaves(jer: unknown, path = "$"): Map<string, string> {
  const out = new Map<string, string>();
  for (const l of treeLines(jer, "$", 0, path)) if (l.value !== null) out.set(l.path, l.value);
  return out;
}

/** Hex as rows of 16 bytes with their offsets (`0000  03 81 00 ...`). */
export function hexRows(hex: string, perRow = 16): { readonly offset: string; readonly bytes: string }[] {
  const out: { offset: string; bytes: string }[] = [];
  const clean = hex.replace(/[^0-9a-fA-F]/g, "").toLowerCase();
  for (let i = 0; i < clean.length; i += perRow * 2) {
    const chunk = clean.slice(i, i + perRow * 2);
    out.push({ offset: (i / 2).toString(16).padStart(4, "0"), bytes: chunk.replace(/(..)(?!$)/g, "$1 ") });
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------------------------

/** A simulated time in ns as seconds with milliseconds (`60.236 s`). */
export function formatSimT(ns: number): string {
  return `${(ns / 1e9).toFixed(3)} s`;
}

/** A duration in ns, at the precision it deserves (`11 ms`, `60.0 s`, `2 d 1 h`). */
export function formatSpan(ns: number): string {
  const a = Math.abs(ns);
  if (a < 1) return "0 s";
  if (a < 1e3) return `${ns} ns`;
  if (a < 1e6) return `${(ns / 1e3).toFixed(0)} µs`;
  if (a < 1e9) return `${(ns / 1e6).toFixed(a < 1e7 ? 1 : 0)} ms`;
  const s = ns / 1e9;
  if (Math.abs(s) < 120) return `${s.toFixed(1)} s`;
  if (Math.abs(s) < 7200) return `${(s / 60).toFixed(1)} min`;
  if (Math.abs(s) < 172_800) return `${(s / 3600).toFixed(1)} h`;
  return `${Math.floor(s / 86_400)} d ${Math.floor((s % 86_400) / 3600)} h`;
}

/** Bytes, short (`1.4 kB`). */
export function formatBytes(n: number): string {
  if (n < 1000) return `${n} B`;
  if (n < 1e6) return `${(n / 1e3).toFixed(1)} kB`;
  if (n < 1e9) return `${(n / 1e6).toFixed(1)} MB`;
  return `${(n / 1e9).toFixed(2)} GB`;
}

/** `Time32` (TAI seconds since 2004-01-01 00:00:00 UTC, IEEE 1609.2 §6.3.11) as a UTC date. */
export function time32Date(t: number): string {
  // 2004-01-01T00:00:00Z is 1072915200 Unix seconds. Time32 counts TAI seconds, so it runs ahead
  // of UTC by every leap second inserted since 2004 (IERS Bulletin C: at the ends of 2005-12,
  // 2008-12, 2012-06, 2015-06 and 2016-12; none since). The engine converts the same way
  // (`v2xw_core::time::utc_unix_to_tai_since_1609_epoch`).
  const LEAPS_UNIX = [1_136_073_600, 1_230_768_000, 1_341_100_800, 1_435_708_800, 1_483_228_800];
  let unix = 1_072_915_200 + t;
  for (const l of LEAPS_UNIX) if (unix - 1 >= l) unix -= 1;
  const d = new Date(unix * 1000);
  return `${d.toISOString().slice(0, 16).replace("T", " ")} UTC`;
}
