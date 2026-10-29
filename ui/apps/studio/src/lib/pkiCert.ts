/**
 * Certificates in plain words: the one visual language every PKI view uses for a certificate's
 * state, and a reading of one `inspect.cert` answer (an IEEE 1609.2 certificate in JER with the
 * validation trace a US device runs on it) into the facts a researcher looks for — what kind of
 * certificate it is, who issued it, when it is valid, what it may sign, where, how it is linked
 * for revocation, and the chain up to the root.
 *
 * Nothing here decides validity: the verdict and every reason are the engine's trace
 * (`v2xw_sec` validation, the same code the receivers run). What is added is wording and the
 * arithmetic of the validity window (Time32 start plus the 1609.2 `Duration`), which the card
 * draws as a bar with "now" on it.
 *
 * Framework- and DOM-free (`test/pki-cert.test.ts`).
 */

// ---------------------------------------------------------------------------------------------
// The status vocabulary
// ---------------------------------------------------------------------------------------------

/** A certificate's state, as every view shows it. */
export type CertStatus = "valid" | "expiring" | "expired" | "revoked" | "untrusted" | "not-yet-valid" | "unknown";

/** The shape that carries a state without colour. */
export type StatusShape = "check" | "clock" | "dash" | "ban" | "shield" | "hourglass" | "dots" | "info" | "out";

export interface StatusInfo {
  /** One or two words. */
  readonly label: string;
  /** What it means for a device, one sentence. */
  readonly meaning: string;
  /** The colour token the state is drawn in. */
  readonly color: string;
  readonly shape: StatusShape;
}

/**
 * The states, each a colour of the page's palette (Okabe-Ito, both themes) and a shape, so a state
 * reads in greyscale and to a colour-blind reader. Expired is grey on purpose: a pseudonym
 * certificate expiring is its normal end, not an alarm; revoked and untrusted are.
 */
export const CERT_STATUS: Readonly<Record<CertStatus, StatusInfo>> = {
  valid: { label: "Valid", meaning: "Receivers accept messages signed with it.", color: "var(--ok)", shape: "check" },
  expiring: { label: "Expiring soon", meaning: "Still accepted, but its validity window is almost over.", color: "var(--warn)", shape: "clock" },
  expired: { label: "Expired", meaning: "Its validity window has passed; receivers reject it. Normal at the end of a pseudonym's week.", color: "var(--text-dim)", shape: "dash" },
  revoked: { label: "Revoked", meaning: "A revocation list (CRL) names it; receivers that installed the list drop its messages.", color: "var(--err)", shape: "ban" },
  untrusted: { label: "Untrusted", meaning: "It does not chain to a root on the certificate trust list, or a signature fails; receivers reject it.", color: "var(--pki-untrusted)", shape: "shield" },
  "not-yet-valid": { label: "Not yet valid", meaning: "Downloaded ahead of time for a later week; it cannot sign yet.", color: "var(--accent)", shape: "hourglass" },
  unknown: { label: "Not checked", meaning: "The engine has not answered for this certificate yet.", color: "var(--text-faint)", shape: "dots" },
};

/** The order the legend lists the states in. */
export const STATUS_ORDER: readonly CertStatus[] = ["valid", "expiring", "not-yet-valid", "expired", "revoked", "untrusted"];

/**
 * A vehicle's credential state (`pkiRecords.credentialState`) in the same language: the shape and
 * colour of the certificate state that causes it, and what it means for the vehicle.
 */
export const VEHICLE_STATE: Readonly<Record<string, { readonly shape: StatusShape; readonly color: string; readonly meaning: string }>> = {
  revoked: { shape: "ban", color: "var(--err)", meaning: "A revocation list names it: receivers that installed the list drop its messages." },
  untrusted: { shape: "shield", color: "var(--pki-untrusted)", meaning: "Its provider's root is not on the trust list, so receivers reject it." },
  starving: { shape: "dash", color: "var(--err)", meaning: "It has no usable pseudonym certificate left, so it cannot sign safety messages." },
  "crl-overdue": { shape: "clock", color: "var(--warn)", meaning: "Its provider's next revocation list is late, so it may trust a revoked sender." },
  "topup-due": { shape: "clock", color: "var(--accent)", meaning: "It is asking, or about to ask, its RA for its next batch of pseudonym certificates. Routine." },
  between: { shape: "dots", color: "var(--text-dim)", meaning: "It holds usable pseudonym certificates but none is in use at this instant: it is changing pseudonym. Normal." },
  healthy: { shape: "check", color: "var(--ok)", meaning: "It signs with a valid pseudonym certificate and holds enough for the weeks ahead." },
  "ec-bad": {
    shape: "hourglass",
    color: "var(--warn)",
    meaning: "Its enrolment certificate fails a receiver's checks (open it for why). In the deployed SCMS its RA would then refuse it new pseudonym certificates.",
  },
  left: {
    shape: "out",
    color: "var(--text-faint)",
    meaning: "Its trip ended and it left the run; what it shows is what it held when it left. It does not count as a problem.",
  },
};

/**
 * Whether a queue is growing: each of the last `n` samples above the one before. A queue that is
 * merely non-empty (routine top-ups in flight) is work in progress, not a warning.
 */
export function growing(samples: readonly number[], n = 3): boolean {
  if (samples.length < n + 1) return false;
  const tail = samples.slice(-(n + 1));
  return tail.every((v, k) => k === 0 || v > tail[k - 1]);
}

/** A pool cell's state (`node.credentials`) in the shared vocabulary; null for an empty cell. */
export function cellStatus(state: string): CertStatus | null {
  switch (state) {
    case "active":
    case "valid":
      return "valid";
    case "downloaded":
      return "not-yet-valid";
    case "expired":
      return "expired";
    case "revoked":
      return "revoked";
    case "not-yet-available":
      return null;
    default:
      return "unknown";
  }
}

// ---------------------------------------------------------------------------------------------
// Reading a certificate
// ---------------------------------------------------------------------------------------------

/** One validation step as the engine traced it. */
export interface TraceStep {
  readonly rule: string;
  readonly status: string;
  readonly detail: readonly string[];
}

/** The parts of `inspect.cert`'s answer the card reads. */
export interface CertLike {
  readonly id8: string;
  readonly chain: readonly string[];
  readonly valid: boolean;
  readonly time32: number | null;
  readonly trace: readonly TraceStep[];
  readonly tree: unknown;
}

/** Each validation rule in plain words: what passing it means. */
export const RULE_TEXT: Readonly<Record<string, { readonly label: string; readonly means: string }>> = {
  "well-formed": { label: "Well-formed", means: "The bytes decode as an IEEE 1609.2 certificate." },
  "chain-to-root": { label: "Chains to a root", means: "Every issuer up the chain is known, ending at a root authority." },
  "root-on-ctl": { label: "Root is trusted", means: "The root is on the certificate trust list (CTL) the electors signed." },
  signatures: { label: "Signatures verify", means: "Each certificate's signature checks out against its issuer's key." },
  "issuer-permissions": { label: "Issuer was allowed", means: "Each issuer had the permissions it granted (no privilege escalation)." },
  "chain-length": { label: "Chain is short enough", means: "The chain fits the receiver's limit on chain length." },
  validity: { label: "Within its validity window", means: "The current time is between its start and its end." },
  region: { label: "Valid in this region", means: "Its region covers the place it is used." },
  crl: { label: "Not revoked", means: "No installed certificate revocation list (CRL) names it." },
};

/** A rule's plain words; an unknown rule keeps the engine's name. */
export function ruleText(rule: string): { readonly label: string; readonly means: string } {
  return RULE_TEXT[rule] ?? { label: rule.replace(/-/g, " "), means: "A check the engine ran." };
}

/** The state of a certificate from the engine's verdict and trace, and the window when known. */
export function certStatus(c: Pick<CertLike, "valid" | "trace" | "time32">, win: Validity | null = null): CertStatus {
  const failed = c.trace.filter((s) => s.status === "fail").map((s) => s.rule);
  if (failed.includes("crl")) return "revoked";
  if (failed.some((r) => r !== "validity" && r !== "crl" && r !== "region")) return "untrusted";
  if (failed.includes("validity")) {
    if (win && c.time32 !== null && c.time32 < win.start) return "not-yet-valid";
    return "expired";
  }
  if (!c.valid) return failed.length > 0 ? "untrusted" : "unknown";
  if (win && c.time32 !== null && win.end > win.start) {
    const left = win.end - c.time32;
    if (left <= Math.max(60, 0.1 * (win.end - win.start))) return "expiring";
  }
  return "valid";
}

/** The first failing step's reason in plain words, or null when it passed. */
export function whyNot(c: Pick<CertLike, "trace">): string | null {
  const f = c.trace.find((s) => s.status === "fail");
  if (!f) return null;
  const detail = f.detail.length > 0 ? `: ${f.detail.join("; ")}` : "";
  return `Fails “${ruleText(f.rule).label}”${detail}`;
}

/**
 * Why a certificate fails, as a sentence a reader takes in without the rule's name: the time
 * check in terms of its window (ended when, how long it lasted), any other check by what it
 * means. The engine's own words stay available as the check's detail. Null when it passes.
 */
export function whySentence(c: Pick<CertLike, "trace" | "time32">, v: Validity | null): string | null {
  const f = c.trace.find((s) => s.status === "fail");
  if (!f) return null;
  if (f.rule === "validity") {
    if (v && v.end === v.start) {
      return `It was issued with a lifetime of zero, so it was never valid: every receiver's time check fails${c.time32 !== null && c.time32 > v.end ? ` (its window closed ${spanWords(c.time32 - v.end)} ago)` : ""}.`;
    }
    if (v && c.time32 !== null && c.time32 > v.end) return `Its validity ended ${spanWords(c.time32 - v.end)} ago; it lasted ${spanWords(v.end - v.start)}.`;
    if (v && c.time32 !== null && c.time32 < v.start) return `It is not valid yet: its validity starts in ${spanWords(v.start - c.time32)}.`;
    return "It fails the time check: the receiver's clock is outside its validity window.";
  }
  const t = ruleText(f.rule);
  return `It fails the check “${t.label}”: ${t.means.replace(/^./, (x) => x.toLowerCase()).replace(/\.$/, "")}.`;
}

/** A certificate's validity window, Time32 seconds. */
export interface Validity {
  readonly start: number;
  readonly end: number;
  /** The duration as the certificate states it (`1 week`, `30 years`). */
  readonly stated: string;
}

/** IEEE 1609.2 §6.4.15 `Duration` choices, in seconds (a year is 31 556 952 s, 365.2425 days). */
const DURATION_S: Readonly<Record<string, number>> = {
  microseconds: 1e-6,
  milliseconds: 1e-3,
  seconds: 1,
  minutes: 60,
  hours: 3600,
  sixtyHours: 216_000,
  years: 31_556_952,
};

/** A number of seconds in the largest whole unit that fits (`7 days`, `10 140 min` → `7 d 1 h`). */
export function spanWords(s: number): string {
  const v = Math.round(Math.abs(s));
  if (v < 60) return `${v} s`;
  if (v < 3600) return `${Math.floor(v / 60)} min${v % 60 ? ` ${v % 60} s` : ""}`;
  if (v < 86_400) return `${Math.floor(v / 3600)} h${Math.floor((v % 3600) / 60) ? ` ${Math.floor((v % 3600) / 60)} min` : ""}`;
  if (v < 2 * 31_556_952) return `${Math.floor(v / 86_400)} d${Math.floor((v % 86_400) / 3600) ? ` ${Math.floor((v % 86_400) / 3600)} h` : ""}`;
  return `${Math.round(v / 31_556_952)} years`;
}

type Obj = Record<string, unknown>;
const isObj = (v: unknown): v is Obj => typeof v === "object" && v !== null && !Array.isArray(v);
const firstKey = (v: unknown): [string, unknown] | null => (isObj(v) ? (Object.entries(v)[0] ?? null) : null);

/** The window from a certificate's `toBeSigned.validityPeriod`, or null. */
export function validityOf(tbs: unknown): Validity | null {
  if (!isObj(tbs) || !isObj(tbs.validityPeriod)) return null;
  const vp = tbs.validityPeriod;
  const start = typeof vp.start === "number" ? vp.start : null;
  const d = firstKey(vp.duration);
  if (start === null || !d || typeof d[1] !== "number") return null;
  const per = DURATION_S[d[0]];
  if (per === undefined) return null;
  const s = d[1] * per;
  const unit = d[0] === "sixtyHours" ? "× 60 h" : d[0];
  const plain = `${d[1]} ${unit}`;
  const words = spanWords(s);
  return { start, end: start + s, stated: s >= 60 && words !== plain ? `${plain} (${words})` : plain };
}

/** Well-known PSIDs (IEEE 1609.12), as the engine's message sets use them. */
export const PSID_NAMES: Readonly<Record<number, string>> = {
  32: "Basic safety messages (BSM)",
  35: "SCMS security management",
  38: "Misbehaviour reports",
  39: "Vulnerable road users (PSM)",
  130: "Intersections (SPaT and MAP)",
  135: "Service advertisements (WSA)",
  256: "Certificate revocation lists (CRL)",
  2_113_685: "Signal status (SSM)",
  2_113_686: "Signal requests (SRM)",
  2_113_687: "MAP distribution",
};

/** A PSID in plain words, with its number. */
export function psidText(psid: number): string {
  const n = PSID_NAMES[psid];
  const hex = `0x${psid.toString(16)}`;
  return n ? `${n} · PSID ${psid}` : `PSID ${psid} (${hex})`;
}

/** ISO 3166-1 numeric codes a US deployment uses. */
const COUNTRY: Readonly<Record<number, string>> = { 840: "United States", 124: "Canada", 484: "Mexico" };

/** A 1609.2 `GeographicRegion` in plain words. */
export function regionText(region: unknown): string | null {
  const r = firstKey(region);
  if (!r) return null;
  const [kind, v] = r;
  if (kind === "identifiedRegion" && Array.isArray(v)) {
    const names = v.map((x) => {
      const k = firstKey(x);
      if (!k) return "?";
      if (k[0] === "countryOnly" && typeof k[1] === "number") return COUNTRY[k[1]] ?? `country ${k[1]}`;
      return k[0];
    });
    return [...new Set(names)].sort().join(", ");
  }
  if (kind === "circularRegion") return "a circle";
  if (kind === "rectangularRegion") return "rectangles";
  if (kind === "polygonalRegion") return "a polygon";
  return kind;
}

/** What a certificate is, in plain words with the standard term. */
export interface CertKind {
  readonly name: string;
  /** The standard term, short (`PC`, `EC`, `RCA`). */
  readonly short: string;
  readonly about: string;
}

const KIND_BY_ROLE: Readonly<Record<string, CertKind>> = {
  root: { name: "Root certificate", short: "RCA", about: "The trust anchor of a provider. Devices trust it because the trust list (CTL) names it." },
  ica: { name: "Intermediate CA certificate", short: "ICA", about: "Certifies the provider's online authorities, so the root can stay offline." },
  eca: { name: "Enrolment CA certificate", short: "ECA", about: "Lets the ECA issue enrolment certificates." },
  aca: { name: "Pseudonym CA certificate", short: "ACA", about: "Lets the ACA sign pseudonym certificates." },
  ra: { name: "Registration Authority certificate", short: "RA", about: "Signs the RA's messages to devices and authorities." },
  la1: { name: "Linkage Authority 1 certificate", short: "LA1", about: "Lets LA1 encrypt and sign its linkage values." },
  la2: { name: "Linkage Authority 2 certificate", short: "LA2", about: "Lets LA2 encrypt and sign its linkage values." },
  ma: { name: "Misbehaviour Authority certificate", short: "MA", about: "Devices encrypt misbehaviour reports to this key." },
  crlg: { name: "CRL Generator certificate", short: "CRLG", about: "Signs the certificate revocation lists." },
  dcm: { name: "Device setup certificate", short: "DCM", about: "Signs the bootstrap answers devices get at the factory or dealer." },
  pg: { name: "Policy Generator certificate", short: "PG", about: "Signs the global policy file." },
  elector: { name: "Elector certificate", short: "Elector", about: "Endorses changes to the certificate trust list." },
  lop: { name: "Proxy certificate", short: "LOP", about: "The Location Obscurer Proxy's certificate." },
};

const PSEUDONYM: CertKind = {
  name: "Pseudonym certificate",
  short: "PC",
  about: "An authorization certificate a vehicle signs its safety messages with. It names no one; a new one every few minutes keeps the vehicle from being tracked.",
};
const ENROLMENT: CertKind = {
  name: "Enrolment certificate",
  short: "EC",
  about: "The vehicle's long-term credential. It never signs safety messages; it proves the vehicle may ask the RA for pseudonym certificates.",
};
const APPLICATION: CertKind = {
  name: "Application certificate",
  short: "AC",
  about: "An identified certificate a roadside unit signs its messages with.",
};

/** The roles of a provider's components, read from a certificate's `id.name` (`aca.provider-a`). */
const NAME_ROLE = /^(root|rca|ica|eca|aca|pca|ra|la1|la2|ma|crlg|dcm|pg|elector|lop)[.-]/i;

/** The kind of certificate, from its role when the caller knows it, else from its contents. */
export function certKind(tbs: unknown, role: string | null = null): CertKind {
  if (role && KIND_BY_ROLE[role]) return KIND_BY_ROLE[role];
  const t = isObj(tbs) ? tbs : {};
  const id = isObj(t.id) ? t.id : {};
  if ("linkageData" in id) return PSEUDONYM;
  const name = typeof id.name === "string" ? id.name : "";
  const m = NAME_ROLE.exec(name);
  if (m) {
    const r = m[1].toLowerCase();
    const key = r === "rca" ? "root" : r === "pca" ? "aca" : r;
    if (KIND_BY_ROLE[key]) return KIND_BY_ROLE[key];
  }
  if ("certRequestPermissions" in t && !("certIssuePermissions" in t)) return ENROLMENT;
  if (/^(ee|obu|rsu)[-.]/i.test(name) && "appPermissions" in t) return APPLICATION;
  if ("certIssuePermissions" in t) return { name: "Authority certificate", short: "CA", about: "Lets its holder issue certificates." };
  return { name: "Certificate", short: "cert", about: "An IEEE 1609.2 certificate." };
}

/** The facts the card lists, every one taken from the certificate's own fields. */
export interface CertFacts {
  readonly kind: CertKind;
  /** Its `id`: a name, `linkage data`, or `none` (anonymous). */
  readonly subject: string;
  /** The issuer's HashedId8, or `self` for a root. */
  readonly issuer: string;
  readonly validity: Validity | null;
  /** The application permissions (what it may sign), in plain words. */
  readonly permissions: readonly string[];
  /** Whether it may issue certificates, or request them (enrolment). */
  readonly issues: string | null;
  readonly region: string | null;
  /** Linkage data, for a pseudonym: its i-period and linkage value. */
  readonly linkage: { readonly iCert: number; readonly value: string } | null;
  readonly crlSeries: number | null;
  readonly cracaId: string | null;
  /** `explicit` (carries its key) or `implicit` (ECQV: the key is reconstructed). */
  readonly form: string | null;
  readonly signature: string | null;
  readonly encryptionKey: boolean;
}

/** The card's facts from `inspect.cert`'s JER tree; `role` names the holder when the caller knows it. */
export function certFacts(tree: unknown, role: string | null = null): CertFacts {
  const t = isObj(tree) ? tree : {};
  const tbs = isObj(t.toBeSigned) ? t.toBeSigned : {};
  const id = isObj(tbs.id) ? tbs.id : {};
  const issuerK = firstKey(t.issuer);
  const issuer = !issuerK ? "?" : issuerK[0] === "self" ? "self" : typeof issuerK[1] === "string" ? issuerK[1].toLowerCase() : issuerK[0];
  const ld = isObj(id.linkageData) ? id.linkageData : null;
  const subject = typeof id.name === "string" ? id.name : ld ? "anonymous (linkage data)" : "none" in id ? "anonymous" : (firstKey(id)?.[0] ?? "?");
  const perms = Array.isArray(tbs.appPermissions)
    ? tbs.appPermissions.map((p) => (isObj(p) && typeof p.psid === "number" ? psidText(p.psid) : "?"))
    : [];
  const issues = Array.isArray(tbs.certIssuePermissions)
    ? "May issue certificates"
    : Array.isArray(tbs.certRequestPermissions)
      ? "May request pseudonym certificates (enrolment)"
      : null;
  const sig = firstKey(t.signature);
  const vki = firstKey(tbs.verifyKeyIndicator);
  return {
    kind: certKind(tbs, role),
    subject,
    issuer,
    validity: validityOf(tbs),
    permissions: perms,
    issues,
    region: regionText(tbs.region),
    linkage: ld ? { iCert: typeof ld.iCert === "number" ? ld.iCert : -1, value: String(ld["linkage-value"] ?? "") } : null,
    crlSeries: typeof tbs.crlSeries === "number" ? tbs.crlSeries : null,
    cracaId: typeof tbs.cracaId === "string" ? tbs.cracaId.toLowerCase() : null,
    form: typeof t.type === "string" ? t.type : vki ? (vki[0] === "reconstructionValue" ? "implicit" : "explicit") : null,
    signature: sig ? sig[0].replace(/Signature$/, "").replace(/^ecdsa/, "ECDSA ").replace(/NistP/, "P-").replace(/BrainpoolP/, "Brainpool P") : null,
    encryptionKey: isObj(tbs.encryptionKey),
  };
}

/** Where `now` sits in a window, 0..1 (clamped), or null when either is unknown. */
export function windowFraction(v: Validity | null, now: number | null): number | null {
  if (!v || now === null || v.end <= v.start) return null;
  return Math.min(1, Math.max(0, (now - v.start) / (v.end - v.start)));
}

// ---------------------------------------------------------------------------------------------
// The chain, in plain words
// ---------------------------------------------------------------------------------------------

interface OwnerComponent {
  readonly id: string;
  readonly cert: string | null;
  readonly counters?: Readonly<Record<string, unknown>>;
}

/** The parts of the trust snapshot the chain needs (structural, so `PkiState` fits). */
export interface OwnerSource {
  readonly governance: readonly OwnerComponent[];
  readonly providers: readonly { readonly components: readonly OwnerComponent[] }[];
  readonly ctl: { readonly roots: readonly string[] };
}

/**
 * Whose certificate each authority HashedId8 is: every component's `cert`, and the `certificate`
 * counter a component without a box of its own reports (the Policy Generator).
 */
export function ownersOf(s: OwnerSource): Record<string, string> {
  const out: Record<string, string> = {};
  const take = (c: OwnerComponent): void => {
    if (c.cert) out[c.cert.toLowerCase()] = c.id;
    const k = c.counters?.certificate;
    if (typeof k === "string" && !(k.toLowerCase() in out)) out[k.toLowerCase()] = c.id;
  };
  for (const c of s.governance) take(c);
  for (const p of s.providers) for (const c of p.components) take(c);
  return out;
}

/** The CTL's roots as HashedId8: a HashedId8 is the low-order 8 bytes of the SHA-256 (1609.2 §6.3.6). */
export function ctlRootIds(s: Pick<OwnerSource, "ctl">): string[] {
  return s.ctl.roots.map((r) => r.slice(-16).toLowerCase());
}

/** One link of a chain as the card draws it. */
export interface ChainLink {
  readonly id8: string;
  /** `Pseudonym CA`, `Root`, or `This certificate`. */
  readonly name: string;
  /** The holder's component id (`provider-a/aca/0`), when known. */
  readonly holder: string | null;
  readonly role: string | null;
  /** For the last link: whether the trust list names it (null when the list is not known). */
  readonly onCtl: boolean | null;
}

/** The chain, leaf first, each link named by its holder when the snapshot knows it. */
export function chainLinks(
  chain: readonly string[],
  owners: Readonly<Record<string, string>>,
  roots: readonly string[],
  roleName: (role: string) => string,
  leafHolder: string | null = null,
): ChainLink[] {
  return chain.map((raw, k) => {
    const id8 = raw.toLowerCase();
    const holder = k === 0 && leafHolder ? leafHolder : (owners[id8] ?? null);
    const role = holder && !holder.startsWith("ee/") ? (holder.split("/")[1] ?? null) : null;
    const name = role ? roleName(role) : k === 0 ? "This certificate" : k === chain.length - 1 ? "Root" : "Issuer";
    const last = k === chain.length - 1;
    return { id8, name, holder, role, onCtl: last && roots.length > 0 ? roots.includes(id8) : null };
  });
}

/** A device id in plain words: `ee/1000000` is "Vehicle 1000000", `rsu/3` "Roadside unit 3". */
export function deviceName(device: string): string {
  const m = /^(ee|obu|rsu)\/(\w+)$/.exec(device);
  if (!m) return device;
  return m[1] === "rsu" ? `Roadside unit ${m[2]}` : `Vehicle ${m[2]}`;
}

/** A HashedId8 in groups of four, which reads and compares more easily than sixteen digits. */
export function grouped(id8: string): string {
  return id8.replace(/(.{4})(?=.)/g, "$1 ");
}

/** "ends in 4 min", "ended 2 h ago", "starts in 3 d": the window against `now`, Time32 seconds. */
export function windowWords(v: Validity | null, now: number | null): string | null {
  if (!v || now === null) return null;
  if (now < v.start) return `starts in ${spanWords(v.start - now)}`;
  if (now > v.end) return `ended ${spanWords(now - v.end)} ago`;
  return `ends in ${spanWords(v.end - now)}`;
}

/** An `inspect.cert` failure in words: an id the run did not keep is not an error to show raw. */
export function certErrorText(error: string): string {
  return /unknown certificate|not in this run|-32006/i.test(error)
    ? "This certificate was not kept in the run's records, so it cannot be opened. The run keeps the certificates its devices and authorities used."
    : "The engine could not look this certificate up.";
}
