/**
 * What a modelled SCMS message carries.
 *
 * Some back-end messages are not encoded as IEEE 1609.2 bytes: the engine sends them with the
 * right meaning and a size summed from their parts (`v2xw-proto` `scms/msg.rs` `ScmsSizes`).
 * The message card shows these parts, with an example of each, so a reader sees what is sent
 * rather than only a byte count. The sizes are the engine's: change them there, then here.
 */

/** One part of a modelled message. */
export interface MessagePart {
  readonly part: string;
  /** Its size in bytes; null when it varies (shown as the remainder of the total). */
  readonly bytes: number | null;
  readonly example: string;
}

/** A modelled message: what it is for, and its parts in order. */
export interface ModelledMessage {
  readonly purpose: string;
  readonly parts: readonly MessagePart[];
}

/** The signed envelope with a certificate digest as signer (IEEE 1609.2 SignedData, ECDSA P-256). */
const ENV_DIGEST: MessagePart = {
  part: "Signed envelope (signer = certificate digest, ECDSA P-256 signature)",
  bytes: 93,
  example: "version 3 · PSID 0x23 · generationTime · signer HashedId8 3f9a…c2 · signature r,s",
};
/** The signed envelope with the signer's full certificate attached (the certificate is its own part). */
const ENV_CERT: MessagePart = {
  part: "Signed envelope (signer = full certificate, ECDSA P-256 signature)",
  bytes: 87,
  example: "version 3 · PSID 0x23 · generationTime · signature r,s",
};
const LCI: MessagePart = { part: "Linkage chain identifier (sealed)", bytes: 16, example: "lci 8e41…07d2" };
const HASH8 = (what: string, example: string): MessagePart => ({ part: what, bytes: 8, example });
const I_PERIOD: MessagePart = { part: "Certificate week (i-period)", bytes: 4, example: "i = 615" };

const TABLE: Record<string, ModelledMessage> = {
  "file-request": {
    purpose: "The device tells the RA which policy and chain files it holds, so the RA can send newer ones.",
    parts: [
      ENV_DIGEST,
      { part: "Local Policy File: file id + version held", bytes: 5, example: "LPF · version 3" },
      { part: "Local Certificate Chain File: file id + version held", bytes: 5, example: "LCCF · version 13" },
    ],
  },
  "file-response": {
    purpose: "The RA sends whichever of the two files is newer than the device's; nothing else if both are current.",
    parts: [
      ENV_DIGEST,
      { part: "Newer Local Policy File (only if newer)", bytes: null, example: "LPF v4: certs per week 20, weeks ahead 2, CRL cadence 1 d, …" },
      { part: "Newer Local Certificate Chain File (only if newer)", bytes: null, example: "LCCF v14: root, ICA, ACA, RA, MA certificates" },
    ],
  },
  "info-response": {
    purpose: "The RA answers \"when is my next batch ready?\" with one time; the device downloads the batch then.",
    parts: [{ part: "nextDlTime (Time32, seconds since 2004-01-01 TAI)", bytes: 4, example: "0x2F1C8A40 = 2026-10-29 08:02:00" }],
  },
  "trust-bundle": {
    purpose: "At the factory the DCM gives a new device everything it needs to trust the system.",
    parts: [
      ENV_CERT,
      { part: "DCM certificate", bytes: null, example: "dcm.provider-a" },
      { part: "Elector certificates (trust anchors)", bytes: null, example: "5 electors, P-384" },
      { part: "Certificate trust list (CTL)", bytes: null, example: "sequence 1 · 2 roots · quorum 3 of 5" },
      { part: "Local Certificate Chain File", bytes: null, example: "root, ICA, ECA, RA, ACA, MA" },
      { part: "Local Policy File", bytes: null, example: "8 policy fields" },
    ],
  },
  "enrolment-registration": {
    purpose: "The DCM tells the RA a new enrolment certificate is legitimate, so the RA accepts requests signed with it.",
    parts: [ENV_DIGEST, HASH8("Enrolment certificate digest (HashedId8)", "a4c1…9e02")],
  },
  "provisioning-refused": {
    purpose: "The RA refuses a certificate request and says why.",
    parts: [ENV_DIGEST, { part: "Error code", bytes: 1, example: "Blocklisted | EnrolmentExpired | UnknownEnrolment | RequestLimit" }, HASH8("Request digest (HashedId8)", "77d0…41ab")],
  },
  "enrol-refused": {
    purpose: "The ECA refuses an enrolment request and says why.",
    parts: [ENV_DIGEST, { part: "Error code", bytes: 1, example: "Blocklisted" }, HASH8("Request digest (HashedId8)", "77d0…41ab")],
  },
  "blocklist-notice": {
    purpose: "The RA tells the ECA a device is blocklisted, so it cannot re-enrol.",
    parts: [ENV_DIGEST, HASH8("Enrolment certificate digest (HashedId8)", "a4c1…9e02")],
  },
  "blocklist-request": {
    purpose: "The MA asks the RA to blocklist the device behind a certificate request.",
    parts: [ENV_DIGEST, { part: "Request handle (SHA-256 of the RA → ACA request)", bytes: 32, example: "5b0e…f31c" }],
  },
  "blocklist-response": {
    purpose: "The RA returns the linkage pointers of every request from that device, so all its certificates can be revoked.",
    parts: [ENV_DIGEST, { part: "Per request: LA1 and LA2 chain identifiers (16 B each)", bytes: null, example: "(lci1 8e41…, lci2 c09f…) × 2" }],
  },
  "pca-lookup-request": {
    purpose: "The MA asks the ACA which request produced a reported certificate.",
    parts: [ENV_DIGEST, I_PERIOD, { part: "Linkage value of the reported certificate", bytes: 9, example: "lv 1d7e4c90a2b3f58e61" }],
  },
  "pca-lookup-response": {
    purpose: "The ACA answers with the request handle and one sealed pointer for each linkage authority.",
    parts: [ENV_DIGEST, { part: "Request handle", bytes: 32, example: "5b0e…f31c" }, LCI, LCI],
  },
  "same-device-request": {
    purpose: "The MA asks a linkage authority whether two certificates belong to one device.",
    parts: [ENV_DIGEST, LCI, LCI],
  },
  "same-device-response": {
    purpose: "The linkage authority answers yes or no, nothing more.",
    parts: [ENV_DIGEST, { part: "Same device?", bytes: 1, example: "true" }],
  },
  "seed-request": {
    purpose: "The MA asks a linkage authority for its seed from the revocation week on.",
    parts: [ENV_DIGEST, LCI, I_PERIOD],
  },
  "seed-response": {
    purpose: "The linkage authority returns its half of the revocation seed.",
    parts: [ENV_DIGEST, { part: "Linkage seed ls(i)", bytes: 16, example: "ls 4af0…9c1d" }, { part: "Linkage authority id", bytes: 2, example: "LA1" }, I_PERIOD],
  },
  "crl-append": {
    purpose: "The MA hands the CRL Generator one revocation entry (both seeds).",
    parts: [ENV_DIGEST, { part: "One linkage entry: the two seeds and their group", bytes: 40, example: "iRev 615 · ls1 4af0… · ls2 b812…" }],
  },
  "crl-request": {
    purpose: "The device asks for the revocation list if it changed since the one it holds.",
    parts: [ENV_DIGEST, { part: "Time of the list it holds (Time32)", bytes: 4, example: "If-Modified-Since 2026-10-29 07:00:00" }],
  },
  "pre-linkage-request": {
    purpose: "The RA asks a linkage authority for pre-linkage values for one device's certificates.",
    parts: [ENV_DIGEST, LCI, I_PERIOD, { part: "Certificates per week", bytes: 1, example: "20" }],
  },
  "pre-linkage-response": {
    purpose: "The linkage authority returns pre-linkage values, each encrypted so only the ACA can read it.",
    parts: [ENV_DIGEST, { part: "Encrypted pre-linkage values (AES-128-CCM), one per certificate", bytes: null, example: "plv × 20, 9 B each + nonce + tag" }],
  },
  "policy-update": {
    purpose: "A new policy file travels from the SCMS Manager toward the RA.",
    parts: [ENV_CERT, { part: "Signer certificate", bytes: null, example: "pg.provider-a" }, { part: "Version", bytes: 4, example: "4" }, { part: "8 policy fields (tag + uint32 each)", bytes: 40, example: "certs per week 20, weeks ahead 2, …" }],
  },
};

/** What a modelled message carries, or null when the engine encodes it (or it is unknown). */
export function modelledMessage(name: string): ModelledMessage | null {
  const base = name.replace(/-proxied$|-forward$/, "");
  return TABLE[name] ?? TABLE[base] ?? (base === "policy-publish" ? (TABLE["policy-update"] ?? null) : null);
}
