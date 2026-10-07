/**
 * The followed radio's credential state, as the inspector and the chase HUD show it.
 *
 * The engine publishes one `node.security` row per node per telemetry window while a credential
 * system runs (`v2xw_engine::sec_records::NodeSecurityView`): the pseudonym certificate in use with
 * its i-period and j index, the pool of certificates it holds, whether a top-up is in flight, the
 * backend access, the CRL it installed and whether it found itself on it. `inspect.node` answers it
 * under `certs` (the row, with its recent changes) and `crl` (entries, version, self-revoked).
 *
 * The chase HUD used to read its pseudonym from `node.tx` alone, which carries the digest but not
 * the indices, and said "(indices pending — node.tx)" for the whole run even under the SCMS
 * lifecycle, whose `node.security` row carries both every second. Here the row is read first.
 *
 * Framework- and DOM-free, so it is tested in plain Node (`test/security.test.ts`).
 */

/** What a `node.security` row says, every member optional because an engine may leave any out. */
export interface NodeSecurityRow {
  readonly t?: number;
  readonly protocol?: string;
  /** The pseudonym certificate's digest, hex; `null` when the node holds none it can sign with. */
  readonly pseudonym?: string | null;
  readonly temp_id?: string | null;
  readonly cert_i?: number | null;
  readonly cert_j?: number | null;
  readonly cert_valid_until?: number | null;
  readonly pool_valid?: number;
  readonly pool_preloaded?: number;
  readonly pool_stored?: number;
  readonly pool_last_period?: number | null;
  readonly changes?: number;
  readonly topup_in_flight?: boolean;
  /**
   * When the next top-up request is due, ns: the start of the i-period at which the pool would
   * fall to `topup_below_periods` periods ahead. Absent from an engine that predates it, and `null`
   * when no top-up will be asked for (a revoked device, top-ups turned off).
   */
  readonly next_topup?: number | null;
  readonly link?: string;
  readonly link_up?: boolean;
  readonly outbox_reports?: number;
  readonly reports_uploaded?: number;
  readonly crl_entries?: number;
  readonly crl_version?: number;
  readonly self_revoked?: boolean;
  /** Recent pseudonym changes, newest first (`sec.pseudonym` rows). */
  readonly changes_log?: readonly Record<string, unknown>[];
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

/**
 * The security row out of an `inspect.node` answer, or `null` when the engine has none for the node
 * (no credential system in the run, or a node that has not closed a telemetry window yet).
 *
 * `certs` is the row itself on the live engine; the specification types it as a list of
 * certificates, so a list is accepted and ignored rather than misread. `crl` fills the three CRL
 * members the engine moves out of `certs`.
 */
export function securityOf(answer: unknown): NodeSecurityRow | null {
  if (!isObj(answer)) return null;
  const certs = answer.certs;
  const crl = answer.crl;
  if (!isObj(certs) && !isObj(crl)) return null;
  const row: Record<string, unknown> = isObj(certs) ? { ...certs } : {};
  if (isObj(crl)) {
    if (typeof crl.entries === "number") row.crl_entries = crl.entries;
    if (typeof crl.version === "number") row.crl_version = crl.version;
    if (typeof crl.self_revoked === "boolean") row.self_revoked = crl.self_revoked;
  }
  return row as NodeSecurityRow;
}

/** Whether two rows say the same thing, so a once-a-second poll does not re-render for nothing. */
export function sameSecurity(a: NodeSecurityRow | null, b: NodeSecurityRow | null): boolean {
  if (a === b) return true;
  if (a === null || b === null) return false;
  return JSON.stringify(a) === JSON.stringify(b);
}

/** The pseudonym the HUD names: its digest, and its indices when anything published them. */
export interface PseudonymLine {
  readonly digest: string;
  /** "i 12 · j 3", or `null` when no source carries the indices (a run with no credential system). */
  readonly indices: string | null;
  /** The temporary id the messages carry (BSM `id`, CAM `stationID`), hex. */
  readonly tempId: string | null;
}

/**
 * Which pseudonym to name, from the security row and from the last `sec.cert` / `node.tx` event.
 *
 * The row wins when it names one: it is the node's own store, published every window. A `sec.cert`
 * event carries indices too. A `node.tx` digest carries none, and then the line has none — it does
 * not promise that they are coming, because in a run with no credential system they never will.
 */
export function pseudonymLine(
  row: NodeSecurityRow | null,
  event: { readonly digest: string; readonly i: number | null; readonly j: number | null } | null,
): PseudonymLine | null {
  if (row && typeof row.pseudonym === "string" && row.pseudonym !== "") {
    const i = typeof row.cert_i === "number" ? row.cert_i : null;
    const j = typeof row.cert_j === "number" ? row.cert_j : null;
    return {
      digest: row.pseudonym,
      indices: i !== null && j !== null ? `i ${i} · j ${j}` : null,
      tempId: typeof row.temp_id === "string" && row.temp_id !== "" ? row.temp_id : null,
    };
  }
  if (event) {
    return {
      digest: event.digest,
      indices: event.i !== null && event.j !== null ? `i ${event.i} · j ${event.j}` : null,
      tempId: null,
    };
  }
  return null;
}

/** The backend access in words: "cellular, up", "via a roadside unit, out of reach". */
export function linkText(row: NodeSecurityRow): string | null {
  if (typeof row.link !== "string" || row.link === "") return null;
  const via = row.link === "cellular" ? "cellular" : row.link === "rsu-relay" ? "via a roadside unit" : row.link;
  if (row.link === "offline") return "offline";
  if (typeof row.link_up !== "boolean") return via;
  return `${via}, ${row.link_up ? "up" : "out of reach"}`;
}

/**
 * The roadside units a scenario document places (`actors.rsus`), or `null` when the document does
 * not say (none loaded, or an engine whose document has no such list).
 */
export function rsuCount(doc: unknown): number | null {
  if (!isObj(doc)) return null;
  const actors = doc.actors;
  if (!isObj(actors)) return null;
  const rsus = actors.rsus;
  if (rsus === undefined) return 0;
  return Array.isArray(rsus) ? rsus.length : null;
}

/** Seconds from `nowNs` to `atNs`, as "in 12.3 s" or "now"; `null` when there is no instant. */
export function untilText(atNs: number | null | undefined, nowNs: number): string | null {
  if (typeof atNs !== "number" || !Number.isFinite(atNs)) return null;
  const s = (atNs - nowNs) / 1e9;
  if (s <= 0) return "expired";
  if (s < 120) return `in ${s.toFixed(1)} s`;
  if (s < 7200) return `in ${(s / 60).toFixed(1)} min`;
  return `in ${(s / 3600).toFixed(1)} h`;
}
