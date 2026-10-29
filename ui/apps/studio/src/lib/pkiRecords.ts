/**
 * The PKI view's Vehicles, Cases and Revocation-lists tabs and its map overlays (WP-E3): what
 * `inspect.pki` answers (`v2xw_server::pki`, from the engine's `node.credentials`, `pki.case`,
 * `pki.flow`, `pki.state` and `pki.knowledge` records), and the pure functions that turn it
 * into a grid, a board, a table and per-vehicle map colours.
 *
 * Nothing here invents a number. The grid is the vehicle's own `node.credentials`, rebuilt from
 * the cells it holds and the grid's bounds; the board's stages are the MA's `pki.case`
 * transitions; the CRL entries are what the real decoder read from the published bytes. Ground
 * truth (who reported, who was accused, which certificates a CRL revokes) is in a `truth` member
 * that the server sends only in the ground-truth view, and every function here that uses it
 * takes the view explicitly, so a blind evaluation cannot leak it through a colour.
 *
 * Framework- and DOM-free, so it is tested in plain Node (`test/pki-records.test.ts`).
 */

/** The ground-truth view's name, as the server spells it. */
export const GROUND_TRUTH = "ground-truth";

// ---------------------------------------------------------------------------------------------
// Vehicles
// ---------------------------------------------------------------------------------------------

/** A cell's state, as `node.credentials` names it. */
export type CellState = "active" | "valid" | "downloaded" | "expired" | "revoked" | "not-yet-available";

/** One cell of the i × j grid. */
export interface CredCell {
  readonly i: number;
  readonly j: number;
  readonly state: CellState | string;
  readonly id8?: string;
  readonly used?: boolean;
}

/** The facts of the vehicle's own `node.security` row the table shows. */
export interface VehicleSecurity {
  readonly t?: number;
  readonly next_topup?: number | null;
  readonly topup_in_flight?: boolean;
  readonly crl_version?: number;
  readonly crl_entries?: number;
  readonly self_revoked?: boolean;
  readonly link?: string;
  readonly link_up?: boolean;
  readonly pool_valid?: number;
}

/** One row of `inspect.pki {topic: "vehicles"}`. */
export interface VehicleRow {
  readonly device: string;
  readonly node: number;
  readonly provider: string;
  readonly t: number;
  readonly enrolled: boolean;
  readonly enrolment?: string | null;
  readonly provisioned: boolean;
  readonly active?: string | null;
  readonly current_i: number;
  readonly jmax: number;
  readonly pool: Readonly<Record<string, number>>;
  readonly usable: number;
  readonly last_period?: number | null;
  readonly periods_ahead?: number | null;
  readonly pseudonym_changes: number;
  readonly used: number;
  readonly ctl_sequence?: number | null;
  /** Revoked as the vehicle itself knows it: a CRL it installed names it. */
  readonly revoked: boolean;
  readonly security?: VehicleSecurity | null;
  /** Ground truth only: how many of its certificates a published CRL revokes. */
  readonly truth?: { readonly revoked_certs: number; readonly revoked_by_crls: readonly string[] };
}

/** `inspect.pki {topic: "vehicle"}`. */
export interface VehicleDetail extends Omit<VehicleRow, "truth"> {
  readonly grid: {
    readonly i_min: number;
    readonly i_max: number;
    readonly jmax: number;
    /** Cells in the full grid. */
    readonly cells: number;
    /** The cells it holds; every other cell in the bounds is `not-yet-available`. */
    readonly held: readonly CredCell[];
  };
  readonly pseudonyms: readonly { readonly id8: string; readonly since: number }[];
  readonly history: readonly string[];
  readonly records: number;
  readonly first_t: number;
  readonly view: string;
  readonly truth?: {
    readonly revoked_certs: number;
    readonly revoked_by_crls: readonly string[];
    readonly revoked_by?: Readonly<Record<string, readonly string[]>>;
  };
}

/** `inspect.pki {topic: "vehicles"}`. */
export interface VehiclesAnswer {
  readonly t_ns: number;
  readonly time32: number;
  readonly view: string;
  readonly vehicles: readonly VehicleRow[];
}

/** The full grid, columns by i-period and rows by j, rebuilt from the held cells and the bounds. */
export interface Grid {
  readonly iMin: number;
  readonly iMax: number;
  readonly jmax: number;
  /** `columns[i - iMin][j]`. */
  readonly columns: readonly (readonly CredCell[])[];
}

/**
 * The vehicle's whole i × j grid, as `node.credentials` lists it: every cell in the bounds, the
 * held ones as recorded and the rest `not-yet-available`. Flattened column by column it is the
 * record's `cells` array exactly (`(i, j)` ascending).
 */
export function fullGrid(d: Pick<VehicleDetail, "grid">): Grid {
  const { i_min: iMin, i_max: iMax, jmax } = d.grid;
  const held = new Map<string, CredCell>();
  for (const c of d.grid.held) held.set(`${c.i}/${c.j}`, c);
  const columns: CredCell[][] = [];
  for (let i = iMin; i <= iMax; i++) {
    const col: CredCell[] = [];
    for (let j = 0; j < jmax; j++) col.push(held.get(`${i}/${j}`) ?? { i, j, state: "not-yet-available", used: false });
    columns.push(col);
  }
  return { iMin, iMax, jmax, columns };
}

/** The grid's cells in `node.credentials` order. */
export function flatCells(g: Grid): CredCell[] {
  return g.columns.flat();
}

/** Plain words for a cell state, for the legend and a cell's title. */
export const CELL_LABEL: Readonly<Record<CellState, string>> = {
  active: "in use now (signing)",
  valid: "valid this week",
  downloaded: "downloaded for a later week",
  expired: "expired",
  revoked: "revoked",
  "not-yet-available": "not downloaded yet",
};

/** A vehicle's credential state, the map overlay's colour (00-gap-analysis §3.2). */
export type CredentialState = "healthy" | "topup-due" | "starving" | "crl-overdue" | "revoked" | "untrusted";

export const CREDENTIAL_LABEL: Readonly<Record<CredentialState, string>> = {
  healthy: "healthy",
  "topup-due": "top-up due",
  starving: "starving (no usable certificate)",
  "crl-overdue": "CRL overdue",
  revoked: "revoked",
  untrusted: "untrusted provider",
};

/** What {@link credentialState} needs beyond the row: the clock and the published lists. */
export interface CredentialContext {
  readonly nowNs: number;
  /** Providers whose root is on the CTL. A provider not listed counts as on it. */
  readonly offCtl: ReadonlySet<string>;
  /** Providers whose newest CRL is past its `nextCrl`. */
  readonly crlOverdue: ReadonlySet<string>;
  /** Ground truth may decide the colour (a vehicle revoked by a CRL it has not installed). */
  readonly groundTruth: boolean;
}

/**
 * One vehicle's credential state, in the order a researcher triages: revoked first, then an
 * untrusted provider, no usable certificate, an overdue CRL, and a top-up that is due.
 *
 * "Revoked" is the vehicle's own knowledge (a CRL it installed names it) outside ground truth,
 * and in ground truth also any vehicle a published CRL revokes whether or not it has heard.
 */
export function credentialState(row: VehicleRow, ctx: CredentialContext): CredentialState {
  if (row.revoked || row.security?.self_revoked === true) return "revoked";
  if (ctx.groundTruth && (row.truth?.revoked_certs ?? 0) > 0) return "revoked";
  if (ctx.offCtl.has(row.provider)) return "untrusted";
  if (!row.enrolled || row.usable === 0 || !row.active) return "starving";
  if (ctx.crlOverdue.has(row.provider)) return "crl-overdue";
  const next = row.security?.next_topup;
  if (row.security?.topup_in_flight === true || (typeof next === "number" && next <= ctx.nowNs) || row.periods_ahead === 0) {
    return "topup-due";
  }
  return "healthy";
}

// ---------------------------------------------------------------------------------------------
// Revocation lists
// ---------------------------------------------------------------------------------------------

/** One CRL version, as `pki.state` listed it, with its SLA clock against the newest state. */
export interface CrlHead {
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
  /** Seconds from the newest state to `nextCrl`; negative is overdue. */
  readonly sla_now_s: number;
  readonly id8: string;
  readonly decodable: boolean;
}

export interface CtlView {
  readonly sequence: number;
  readonly quorum: number;
  readonly electors: number;
  readonly roots: readonly string[];
  readonly effective: number;
  readonly signers: readonly number[];
  readonly bytes: number;
  readonly pending: number;
  readonly history: readonly { readonly sequence: number; readonly effective: number; readonly roots: number; readonly signers: readonly number[] }[];
}

export interface ProviderView {
  readonly label: string;
  readonly on_ctl: boolean;
  readonly home: boolean;
  readonly crl_publication: string;
  readonly ma_mode: string;
  readonly ma_review_s?: number | null;
  readonly devices: { readonly devices: number; readonly enrolled: number; readonly provisioned: number; readonly revoked: number; readonly blocklisted: number };
  readonly region?: { readonly name: string; readonly metro: string; readonly assumption: boolean };
  readonly fleet_share?: number;
}

/** `inspect.pki {topic: "crls"}`. */
export interface CrlsAnswer {
  readonly t_ns: number;
  readonly time32: number;
  readonly crls: readonly CrlHead[];
  readonly ctl: CtlView | null;
  readonly providers: readonly ProviderView[];
  readonly edition?: string | null;
  readonly assumptions?: readonly string[] | null;
}

/** One linkage entry as the device installs it. */
export interface LinkageEntry {
  readonly la1_id: number;
  readonly la2_id: number;
  readonly i_rev: number;
  readonly i_max: number;
  readonly jmax: number;
  readonly ls1: string;
  readonly ls2: string;
  /** Ground truth only: the run's certificates this entry revokes. */
  readonly matches?: readonly string[];
}

/** `inspect.pki {topic: "crl"}`. */
export interface CrlAnswer {
  readonly head: Omit<CrlHead, "sla_now_s" | "decodable">;
  readonly len: number;
  readonly contents: unknown;
  readonly linkage: readonly LinkageEntry[];
  readonly hash: readonly { readonly id10: string; readonly expiry: number; readonly matches?: readonly string[] }[];
  readonly view: string;
  /** Ground truth only: who holds each matched certificate. */
  readonly owners?: Readonly<Record<string, { device: string; node: number | null; provider: string | null; i: number | null; j: number | null; enrolment: boolean } | null>>;
}

/** One row of the Revocation-lists tab: a provider × series × kind, newest version first. */
export interface CrlSeriesRow {
  readonly key: string;
  readonly provider: string;
  readonly series: number;
  readonly kind: string;
  /** Every version, newest first (full and delta lists both). */
  readonly versions: readonly CrlHead[];
  readonly newest: CrlHead;
}

/** The CRL heads grouped one row per provider × series × kind, in provider then series order. */
export function crlRows(a: Pick<CrlsAnswer, "crls">): CrlSeriesRow[] {
  const by = new Map<string, CrlHead[]>();
  for (const h of a.crls) {
    const key = `${h.provider}/${h.series}/${h.kind}`;
    const list = by.get(key) ?? [];
    list.push(h);
    by.set(key, list);
  }
  const rows: CrlSeriesRow[] = [];
  for (const [key, list] of by) {
    const versions = [...list].sort((x, y) => y.issued_at - x.issued_at || y.version - x.version || Number(x.delta) - Number(y.delta));
    rows.push({ key, provider: list[0].provider, series: list[0].series, kind: list[0].kind, versions, newest: versions[0] });
  }
  return rows.sort((x, y) => x.provider.localeCompare(y.provider) || x.series - y.series || x.kind.localeCompare(y.kind));
}

/** Providers whose newest list of any series is past its `nextCrl` at the newest state. */
export function overdueProviders(a: Pick<CrlsAnswer, "crls">): Set<string> {
  const out = new Set<string>();
  for (const r of crlRows(a)) if (r.newest.sla_now_s < 0) out.add(r.provider);
  return out;
}

/** "6 d 23 h" for an SLA clock in seconds; "overdue 2 h" when negative. */
export function slaText(s: number): string {
  const neg = s < 0;
  let v = Math.abs(Math.round(s));
  const d = Math.floor(v / 86_400);
  v -= d * 86_400;
  const h = Math.floor(v / 3600);
  v -= h * 3600;
  const m = Math.floor(v / 60);
  const body = d > 0 ? `${d} d ${h} h` : h > 0 ? `${h} h ${m} min` : m > 0 ? `${m} min` : `${v} s`;
  return neg ? `overdue ${body}` : body;
}

/** IEEE 1609.2 `Time32` (TAI seconds since 2004-01-01 00:00:00 UTC) as an ISO date, leap seconds aside. */
export function time32Text(t: number): string {
  // 2004-01-01T00:00:00Z is 1072915200 Unix seconds. Time32 counts TAI seconds (1609.2 §6.4.15),
  // which since 2017-01-01 run five ahead of UTC (the leap seconds of 2005, 2008, 2012, 2015 and
  // 2016); the label is to the minute, so the five are taken off and earlier dates are left.
  return new Date((1_072_915_200 + t - 5) * 1000).toISOString().slice(0, 16).replace("T", " ") + " UTC";
}

// ---------------------------------------------------------------------------------------------
// Cases
// ---------------------------------------------------------------------------------------------

/** The board's columns, in the order `pki_view::CASE_STAGES` lists the stages. */
export const BOARD_COLUMNS = [
  { stage: "reports-in", label: "Reports in" },
  { stage: "corroborated", label: "Corroborated" },
  { stage: "decision", label: "Decision" },
  { stage: "plv-lookup", label: "PLV lookup" },
  { stage: "la-seeds", label: "LA seeds" },
  { stage: "crl-published", label: "CRL published" },
  { stage: "enforced", label: "Enforced" },
] as const;

export type BoardStage = (typeof BOARD_COLUMNS)[number]["stage"];

/** One `pki.case` record. */
export interface CaseStage {
  readonly case: string;
  readonly provider: string;
  readonly stage: string;
  readonly t: number;
  readonly instance: string;
  readonly detail: Readonly<Record<string, unknown>>;
}

/** One investigation step: a backend message or a stage stamp of the case's instances. */
export interface CaseStep {
  readonly kind: "msg" | "stage" | string;
  readonly id: string;
  readonly t: number;
  readonly flow: string;
  readonly name: string;
  readonly from?: string | null;
  readonly to?: string | null;
  readonly bytes?: number | null;
  readonly transport?: string | null;
  readonly pdus: readonly { readonly id: string; readonly type: string; readonly len: number; readonly sealed_to?: string | null; readonly opened_by?: string | null }[];
}

export interface CaseParty {
  readonly device: string;
  readonly node: number | null;
  readonly provider: string | null;
  readonly reports?: number;
}

/** One case of `inspect.pki {topic: "cases"}`. */
export interface CaseView {
  readonly case: string;
  readonly provider: string;
  readonly instance: string;
  readonly crl_instance?: string | null;
  /** The MA's own table; empty for a view that is not the case's MA nor ground truth. */
  readonly stages: readonly CaseStage[];
  readonly steps: readonly CaseStep[];
  /** Ground truth only. */
  readonly truth?: { readonly accused: readonly CaseParty[]; readonly reporters: readonly CaseParty[] };
}

export interface CasesAnswer {
  readonly t_ns: number;
  readonly view: string;
  readonly cases: readonly CaseView[];
}

/** Where a case sits on the board: the furthest column it reached, or `refused`. */
export function boardColumn(c: Pick<CaseView, "stages">): BoardStage | "refused" | null {
  if (c.stages.some((s) => s.stage === "refused")) return "refused";
  let best = -1;
  for (const s of c.stages) best = Math.max(best, BOARD_COLUMNS.findIndex((b) => b.stage === s.stage));
  return best < 0 ? null : BOARD_COLUMNS[best].stage;
}

/**
 * Each stage the case entered, in board order, with the time to the next stage it entered. A stage
 * recorded more than once (`enforced`, once per enforcing vehicle) counts from its first record:
 * when the case entered it.
 */
export function stageTimes(c: Pick<CaseView, "stages">): { stage: string; t: number; dwellNs: number | null }[] {
  const order = (s: string): number => {
    const i = BOARD_COLUMNS.findIndex((b) => b.stage === s);
    return i < 0 ? BOARD_COLUMNS.length : i;
  };
  const sorted = [...c.stages].sort((a, b) => order(a.stage) - order(b.stage) || a.t - b.t);
  const list = sorted.filter((s, k) => k === 0 || sorted[k - 1].stage !== s.stage);
  return list.map((s, k) => ({ stage: s.stage, t: s.t, dwellNs: k + 1 < list.length ? list[k + 1].t - s.t : null }));
}

/** One report's evidence: `1VInconsistent/ConstantPosition, verified`, or why it was modelled. */
export function evidenceText(e: unknown): string {
  const r = (e ?? {}) as Record<string, unknown>;
  if (typeof r.type_name === "string") {
    const subs = Array.isArray(r.subtypes) ? r.subtypes.map((s) => String((s as Record<string, unknown>).name)).join("+") : "";
    const verified = r.verified === true ? "verified" : r.verified === false ? "NOT verified" : "unverified";
    return `${r.type_name}/${subs}, ${verified}${r.shared === true ? ", shared by the other MA" : ""}`;
  }
  if (typeof r.modelled === "string") {
    const checks = Array.isArray(r.checks) ? r.checks.join("+") : "";
    return `modelled (${r.modelled})${checks ? `: ${checks}` : ""}`;
  }
  return "unknown";
}

/**
 * "Enforced 7 of 13 vehicles": the engine stamps `enforced` once per vehicle that installs the
 * entry, with the running count, so the newest record (the highest count) is the current one.
 */
export function enforcement(c: Pick<CaseView, "stages">): { installed: number; of: number } | null {
  const s = c.stages
    .filter((x) => x.stage === "enforced")
    .reduce<CaseStage | undefined>((best, x) => (best === undefined || Number(x.detail.installed) > Number(best.detail.installed) ? x : best), undefined);
  const installed = Number(s?.detail.installed);
  const of = Number(s?.detail.of);
  return s && Number.isFinite(installed) && Number.isFinite(of) ? { installed, of } : null;
}

/**
 * What the receiver of one investigation message learns, in plain words, by the message's name
 * (the kernel's step names, `pki.flow` `name`). A step it does not know is described by its name.
 */
export const STEP_LEARNS: Readonly<Record<string, string>> = {
  "pca-lookup-request": "the accused certificate's linkage value is under investigation",
  "pca-lookup-response": "the RA's request handle and the two LCIs, sealed for the LAs (it cannot open them)",
  "blocklist-request": "the request handle; the RA maps it to the enrolment certificate and blocklists it",
  "blocklist-notice": "that enrolment certificate is blocklisted (no re-enrolment)",
  "blocklist-response": "the blocklisting is done",
  "seed-request": "an LCI sealed for it; the LA finds the chain and releases its seed at iRev",
  "seed-response": "that LA's linkage seed at iRev, so with the other LA's it can revoke the device",
  "crl-append": "the entry to list: (la1Id, la2Id, ls1, ls2, iRev, jMax)",
  "crl-publish": "the signed CRL (public)",
  "crl-broadcast": "the signed CRL, for the roadside units to rebroadcast (public)",
};

/** Who learned what in one case: one line per receiving entity, in time order. */
export function whoLearned(c: Pick<CaseView, "steps">): { who: string; what: string; t: number; from: string }[] {
  const out: { who: string; what: string; t: number; from: string }[] = [];
  for (const s of c.steps) {
    if (s.kind !== "msg" || !s.to) continue;
    out.push({ who: s.to, from: s.from ?? "", t: s.t, what: STEP_LEARNS[s.name] ?? s.name });
  }
  return out;
}

/** The roles a view can take, in the order a case walks them. */
export const VIEW_ROLES = ["ma", "aca", "ra", "la1", "la2", "eca", "crlg"] as const;

/**
 * The views to offer: ground truth, then each provider's components (`provider/role/0`). The
 * providers come from the public trust-domain state, so offering them needs no ground truth.
 */
export function viewChoices(providers: readonly string[]): string[] {
  const out: string[] = [GROUND_TRUTH];
  for (const p of [...new Set(providers)].sort()) for (const r of VIEW_ROLES) out.push(`${p}/${r}/0`);
  return out;
}

// ---------------------------------------------------------------------------------------------
// Map overlays
// ---------------------------------------------------------------------------------------------

/** What the PKI map overlay colours vehicles by. */
export type PkiMapMode = "off" | "provider" | "credential" | "case";

export const MAP_MODE_LABEL: Readonly<Record<PkiMapMode, string>> = {
  off: "Off",
  provider: "Provider",
  credential: "Credential state",
  case: "Case parties",
};

/** Okabe–Ito hues, colour-blind safe, one per provider in label order. */
export const PROVIDER_COLORS: readonly number[] = [0x0072b2, 0xe69f00, 0x009e73, 0xcc79a7, 0x56b4e9, 0xd55e00];

export const CREDENTIAL_COLORS: Readonly<Record<CredentialState, number>> = {
  healthy: 0x009e73,
  "topup-due": 0xe69f00,
  starving: 0xd55e00,
  "crl-overdue": 0x56b4e9,
  revoked: 0xcc79a7,
  untrusted: 0x6b6b6b,
};

export const PARTY_COLORS = { reporter: 0x0072b2, accused: 0xd55e00 } as const;

/** One provider's colour: its place in the sorted labels. */
export function providerColor(provider: string, providers: readonly string[]): number {
  const sorted = [...new Set(providers)].sort();
  const i = sorted.indexOf(provider);
  return PROVIDER_COLORS[(i < 0 ? sorted.length : i) % PROVIDER_COLORS.length];
}

export interface MapTag {
  readonly color: number;
  readonly emphasis?: boolean;
}

/**
 * The map's rings, by node id, for one mode.
 *
 * `case` names people only ground truth knows (the MA holds linkage values, not identities), so
 * outside ground truth it returns nothing; `credential` uses ground truth only through
 * {@link credentialState}'s `groundTruth`.
 */
export function mapTags(
  mode: PkiMapMode,
  input: {
    readonly vehicles: readonly VehicleRow[];
    readonly crls?: Pick<CrlsAnswer, "crls" | "providers"> | null;
    readonly cases?: readonly CaseView[];
    readonly selectedCase?: string | null;
    readonly view: string;
    readonly nowNs: number;
  },
): Map<number, MapTag> {
  const out = new Map<number, MapTag>();
  const gt = input.view === GROUND_TRUTH;
  if (mode === "provider") {
    const labels = input.vehicles.map((v) => v.provider);
    for (const v of input.vehicles) out.set(v.node, { color: providerColor(v.provider, labels) });
  } else if (mode === "credential") {
    const offCtl = new Set((input.crls?.providers ?? []).filter((p) => !p.on_ctl).map((p) => p.label));
    const ctx: CredentialContext = { nowNs: input.nowNs, offCtl, crlOverdue: input.crls ? overdueProviders(input.crls) : new Set(), groundTruth: gt };
    for (const v of input.vehicles) out.set(v.node, { color: CREDENTIAL_COLORS[credentialState(v, ctx)] });
  } else if (mode === "case" && gt) {
    const chosen = (input.cases ?? []).filter((c) => !input.selectedCase || c.case === input.selectedCase);
    for (const c of chosen) {
      for (const r of c.truth?.reporters ?? []) if (r.node !== null) out.set(r.node, { color: PARTY_COLORS.reporter });
      for (const a of c.truth?.accused ?? []) if (a.node !== null) out.set(a.node, { color: PARTY_COLORS.accused, emphasis: true });
    }
  }
  return out;
}

/**
 * The rings re-keyed from engine node id (what the PKI records name) to the viewer's `actor_id`
 * (what the pose stream carries; the two differ). A node with no actor on the map is dropped.
 */
export function byActor(tags: ReadonlyMap<number, MapTag>, actorOf: (node: number) => number | null | undefined): Map<number, MapTag> {
  const out = new Map<number, MapTag>();
  for (const [node, tag] of tags) {
    const a = actorOf(node);
    if (typeof a === "number") out.set(a, tag);
  }
  return out;
}

// ---------------------------------------------------------------------------------------------
// Small readers
// ---------------------------------------------------------------------------------------------

/** "provider-a/aca/0" → "ACA (provider-a)"; "ee/1000019" → "vehicle ee/1000019". */
export function entityLabel(id: string): string {
  if (id.startsWith("ee/")) return `vehicle ${id}`;
  const [provider, role, n] = id.split("/");
  if (!role) return id;
  return `${role.toUpperCase()}${n && n !== "0" ? ` ${n}` : ""} (${provider})`;
}

/** Seconds of simulated time, one decimal. */
export function secondsText(ns: number): string {
  return `${(ns / 1e9).toFixed(1)} s`;
}

/** The answer narrowed to a topic's shape, or null if it is not one. */
export function asVehicles(v: unknown): VehiclesAnswer | null {
  return isObj(v) && Array.isArray(v.vehicles) ? (v as unknown as VehiclesAnswer) : null;
}
export function asVehicle(v: unknown): VehicleDetail | null {
  return isObj(v) && isObj(v.grid) && Array.isArray(v.grid.held) ? (v as unknown as VehicleDetail) : null;
}
export function asCases(v: unknown): CasesAnswer | null {
  return isObj(v) && Array.isArray(v.cases) ? (v as unknown as CasesAnswer) : null;
}
export function asCrls(v: unknown): CrlsAnswer | null {
  return isObj(v) && Array.isArray(v.crls) ? (v as unknown as CrlsAnswer) : null;
}
export function asCrl(v: unknown): CrlAnswer | null {
  return isObj(v) && Array.isArray(v.linkage) && isObj(v.head) ? (v as unknown as CrlAnswer) : null;
}

function isObj(v: unknown): v is Record<string, any> { // eslint-disable-line @typescript-eslint/no-explicit-any
  return typeof v === "object" && v !== null && !Array.isArray(v);
}
