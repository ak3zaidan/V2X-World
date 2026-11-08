/**
 * The PDU decoder's words: what each SCMS message type is, and what each field of an IEEE 1609.2
 * / 1609.2.1 structure means, so a reader who knows V2X but not every ASN.1 member name can read a
 * decoded message. The decoder still draws every JER leaf the engine sent, in order; this only
 * adds a gloss beside the names it knows (`components/pki/PduDecoder.tsx`).
 *
 * Framework- and DOM-free (`test/pki-cert.test.ts`).
 */

/** Each SPDU type the engine decodes, as a reader would say it: what it is and who sends it to whom. */
export const PDU_TYPES: Readonly<Record<string, { readonly what: string; readonly flow: string }>> = {
  Ieee1609Dot2Data: { what: "IEEE 1609.2 secured message: the envelope that signs or encrypts what it carries", flow: "" },
  SignedCertificateRequest: { what: "A certificate request, signed with the requester's key", flow: "" },
  EeEcaCertRequestSpdu: { what: "Enrolment request: the vehicle asks for its long-term enrolment certificate", flow: "vehicle → ECA, through the DCM" },
  EcaEeCertResponseSpdu: { what: "Enrolment certificate: the ECA's answer, with the vehicle's enrolment certificate", flow: "ECA → vehicle" },
  EeRaCertRequestSpdu: {
    what: "Pseudonym certificate request: the vehicle sends its butterfly key seeds, signed with its enrolment certificate",
    flow: "vehicle → RA, through the proxy",
  },
  RaEeCertAckSpdu: { what: "The RA's acknowledgement: when and where the vehicle can download its batch", flow: "RA → vehicle" },
  RaAcaCertRequestSpdu: { what: "One expanded, shuffled certificate request: the ACA cannot tell which vehicle it is for", flow: "RA → ACA" },
  AcaRaCertResponseSpdu: { what: "The ACA's answer: a pseudonym certificate, encrypted so the RA cannot read it", flow: "ACA → RA" },
  AcaEeCertResponsePrivateSpdu: { what: "A pseudonym certificate, encrypted to the vehicle's butterfly key", flow: "ACA → vehicle, through the RA" },
  EeRaDownloadRequestSpdu: { what: "Download request: the vehicle fetches its batch of pseudonym certificates", flow: "vehicle → RA" },
  EncryptedIndividualPLV: { what: "Pre-linkage values from a Linkage Authority, encrypted to the ACA", flow: "LA → RA → ACA" },
  SecuredCrl: { what: "A signed certificate revocation list (CRL)", flow: "CRL Generator → every device" },
  SecuredMisbehaviorReport: { what: "A misbehaviour report, signed by the reporter and encrypted to the MA", flow: "vehicle → MA, through the RA" },
};

/** An SPDU type in plain words; an unknown type is described by its name. */
export function pduText(type: string): { readonly what: string; readonly flow: string } {
  return PDU_TYPES[type] ?? { what: type.replace(/([a-z])([A-Z])/g, "$1 $2"), flow: "" };
}

/** What each member of the 1609.2 / 1609.2.1 structures means, by its JER name. */
export const FIELD_GLOSS: Readonly<Record<string, string>> = {
  protocolVersion: "1609.2 version (3)",
  content: "what the envelope carries",
  signedData: "signed content",
  encryptedData: "encrypted content",
  unsecuredData: "plain bytes, not protected",
  signedCertificateRequest: "a certificate request, signed",
  hashId: "hash algorithm",
  tbsData: "the part that is signed",
  payload: "the signed content",
  data: "the inner message",
  extDataHash: "a hash of content sent separately",
  headerInfo: "header: kind of message, time, place",
  psid: "Provider Service ID: the kind of message",
  generationTime: "when it was made (µs since 2004, TAI)",
  expiryTime: "when it stops being valid",
  generationLocation: "where it was made",
  signer: "who signed it",
  digest: "the signer's certificate, as its HashedId8",
  certificate: "the signer's full certificate",
  self: "signed with the requester's own key",
  signature: "the signature",
  ecdsaNistP256Signature: "ECDSA signature on NIST P-256",
  ecdsaNistP384Signature: "ECDSA signature on NIST P-384",
  ecdsaBrainpoolP256r1Signature: "ECDSA signature on Brainpool P-256",
  rSig: "signature value r",
  sSig: "signature value s",
  "x-only": "a curve point, x coordinate only",
  "compressed-y-0": "a curve point, compressed",
  "compressed-y-1": "a curve point, compressed",
  recipients: "who can decrypt it",
  ciphertext: "the encrypted bytes",
  aes128ccm: "AES-128-CCM",
  nonce: "the encryption nonce",
  ccmCiphertext: "ciphertext with its authentication tag",
  certRecipInfo: "recipient named by its certificate",
  rekRecipInfo: "recipient named by a key it sent",
  pskRecipInfo: "recipient sharing a pre-shared key",
  signedDataRecipInfo: "recipient named by a signed message's key",
  recipientId: "the recipient, as a HashedId8",
  encKey: "the message key, encrypted to the recipient",
  eciesNistP256: "ECIES on NIST P-256",
  v: "ephemeral public key",
  c: "the wrapped message key",
  t: "authentication tag",
  version: "format version",
  type: "explicit (carries its key) or implicit (ECQV)",
  issuer: "who issued it",
  sha256AndDigest: "the issuer, as a HashedId8 (SHA-256)",
  sha384AndDigest: "the issuer, as a HashedId8 (SHA-384)",
  toBeSigned: "the certificate's signed contents",
  id: "who it names: a name, linkage data or none",
  name: "the holder's name",
  linkageData: "linkage data: lets a CRL revoke it without naming the vehicle",
  iCert: "its week (i-period)",
  "linkage-value": "its linkage value",
  cracaId: "the authority that would revoke it",
  crlSeries: "the revocation list series that would name it",
  validityPeriod: "when it is valid",
  start: "start (seconds since 2004, TAI)",
  duration: "how long",
  appPermissions: "what it may sign",
  certIssuePermissions: "what it may issue",
  certRequestPermissions: "what it may request",
  subjectPermissions: "the permissions granted",
  ssp: "service-specific permissions",
  opaque: "raw permission bytes",
  region: "where it is valid",
  identifiedRegion: "named regions",
  countryOnly: "a country (ISO 3166-1 numeric)",
  verifyKeyIndicator: "its public key, or how to rebuild it",
  verificationKey: "its public key",
  reconstructionValue: "ECQV value to rebuild its public key",
  encryptionKey: "a key to encrypt to its holder",
  publicKey: "the public key",
  supportedSymmAlg: "symmetric cipher it accepts",
  ecdsaNistP256: "an ECDSA NIST P-256 key",
  issueDate: "when the list was issued",
  nextCrl: "when the next list is due at the latest",
  priorityInfo: "priority",
  typeSpecific: "the list's entries",
  fullLinkedCertificateList: "a full list of linkage entries",
  deltaLinkedCertificateList: "only the changes since the last list",
  individual: "entries one vehicle at a time",
  groups: "entries for groups of vehicles",
  iRev: "the week revocation starts from",
  indexWithinI: "position within the week",
  jMax: "certificates per week",
  la1Id: "Linkage Authority 1's id",
  la2Id: "Linkage Authority 2's id",
  linkageSeed1: "LA1's seed at iRev",
  linkageSeed2: "LA2's seed at iRev",
  crlCraca: "the authority that signs the list",
  requestType: "what is being asked for",
  tbsCert: "the requested certificate's contents",
  publicKeys: "the keys to certify",
  verificationKeyBf: "butterfly key seed for signing",
  encryptionKeyBf: "butterfly key seed for encryption",
  expansionKey: "butterfly expansion key",
  caterpillarKey: "the butterfly seed key",
  requestHash: "a hash of the request it answers",
  nextDlTime: "when to download",
  acaResponse: "the ACA's response",
  ecaCertChain: "the ECA's chain",
  enrollmentCert: "the enrolment certificate",
};

/** The gloss of one JER member, or null for an index (`[0]`) or a name not listed. */
export function fieldGloss(key: string): string | null {
  if (key.startsWith("[")) return null;
  return FIELD_GLOSS[key] ?? null;
}

/** The arrow's label: the one SPDU it carried, else the step in words and how many SPDUs. */
export function arrowLabel(r: { readonly name: string; readonly pdus?: readonly { readonly type: string }[] }): string {
  const types = [...new Set((r.pdus ?? []).map((p) => p.type))];
  const n = r.pdus?.length ?? 0;
  if (types.length === 1 && n === 1) return types[0];
  if (n === 0) return wordsOf(r.name);
  return `${wordsOf(r.name)} · ${n} SPDUs`;
}

/** An engine step or stage name in words (`report_sent` → "report sent"). */
export function wordsOf(name: string): string {
  return name.replace(/[-_]+/g, " ");
}

/** How a message travelled, in words: `cellular-uu` is "cellular (Uu)", the LTE/5G link to the network. */
export function transportText(transport: string): string {
  switch (transport) {
    case "cellular-uu":
      return "cellular (Uu: the vehicle's LTE/5G link to the network)";
    case "backend-net":
      return "the authorities' backend network";
    case "rsu-backhaul":
      return "a roadside unit's wired backhaul";
    case "pc5":
    case "air":
      return "the safety radio channel";
    default:
      return transport.replace(/-/g, " ");
  }
}

/**
 * Why one message carries more than one secured message, when it is the known case: a vehicle's
 * misbehaviour report travels with its next certificate download request, one upload to the RA.
 */
export function piggyback(r: { readonly flow?: string; readonly pdus?: readonly { readonly type: string }[] }): string | null {
  const types = (r.pdus ?? []).map((p) => p.type);
  if (types.length < 2) return null;
  const report = types.some((t) => /Misbeh?aviou?rReport/i.test(t));
  const download = types.some((t) => /Download/i.test(t));
  if (report && download) return "Why two: the report rides with the vehicle's next certificate download request, so one upload to the RA carries both.";
  return null;
}

/** A label cut to `max` characters with an ellipsis; the full text stays in the tooltip. */
export function fitLabel(text: string, max: number): string {
  if (text.length <= max) return text;
  return `${text.slice(0, Math.max(1, max - 1))}…`;
}

