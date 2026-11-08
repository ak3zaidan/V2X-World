/**
 * The certificate views' glossary: every SCMS term or acronym a view shows, in one sentence a
 * student who knows V2X but not the SCMS can follow. `components/pki/Term.tsx` draws a term with
 * this sentence on hover and on keyboard focus, so no view has to explain an acronym inline.
 *
 * Framework- and DOM-free (`test/pki-glossary.test.ts`).
 */

export interface GlossaryEntry {
  /** The term as the standard writes it. */
  readonly term: string;
  /** One plain sentence. */
  readonly means: string;
}

export const GLOSSARY: Readonly<Record<string, GlossaryEntry>> = {
  ij: {
    term: "i and j",
    means:
      "Pseudonym certificates come in weeks: i is the week (the i-period) and j the certificate's place within that week's batch. A vehicle may use any of the week's j-max certificates, all valid at once.",
  },
  jmax: { term: "j-max", means: "How many pseudonym certificates a vehicle gets for each week, all valid at the same time; the policy sets it." },
  psid: {
    term: "PSID",
    means: "Provider Service Identifier (IEEE 1609.12): a number naming an application, such as 32 for basic safety messages. A certificate lists the PSIDs it may sign for.",
  },
  craca: {
    term: "CRACA",
    means: "The CRL-signing CA: the authority whose revocation list would name this certificate. A certificate carries the last 3 bytes of that authority's certificate hash (cracaId).",
  },
  "crl-series": {
    term: "CRL series",
    means: "Which family of revocation lists covers a certificate. A receiver only needs the lists of the series its certificates name.",
  },
  ecqv: {
    term: "Implicit certificate (ECQV)",
    means:
      "Elliptic Curve Qu-Vanstone: the certificate carries no public key; a receiver rebuilds it from the issuer's key and a value in the certificate. It keeps pseudonym certificates small.",
  },
  explicit: { term: "Explicit certificate", means: "The certificate carries its public key and the issuer's signature over it." },
  lci: {
    term: "LCI",
    means: "Linkage chain identifier: a Linkage Authority's private handle on one vehicle's chain of linkage values. Only that Linkage Authority can open it.",
  },
  lv: {
    term: "Linkage value (LV)",
    means:
      "A value in each pseudonym certificate, built from two Linkage Authorities' seeds. Alone it reveals nothing; with both seeds a receiver can recognise all of one vehicle's certificates.",
  },
  plv: {
    term: "PLV",
    means: "Pre-linkage value: each Linkage Authority's half of a linkage value, encrypted so only the Pseudonym CA can combine them.",
  },
  irev: { term: "iRev", means: "The first week a revocation covers: every certificate of the vehicle from that week on is revoked." },
  seed: {
    term: "Linkage seed",
    means: "A Linkage Authority's secret for one vehicle and week. Publishing both Linkage Authorities' seeds on a CRL revokes every later certificate of that vehicle.",
  },
  butterfly: {
    term: "Butterfly keys (caterpillar and cocoon keys)",
    means:
      "The vehicle sends one seed key (the caterpillar); the RA expands it into many keys (cocoons) so the Pseudonym CA can sign a week of certificates without learning they belong to one vehicle.",
  },
  jij: {
    term: "J_ij (cocoon encryption key)",
    means:
      "A one-time public key the RA expands from the vehicle's caterpillar key, one per certificate (week i, number j). The Pseudonym CA encrypts each certificate to it, so only the vehicle can open its answer and the RA cannot read it.",
  },
  "la-aca-key": {
    term: "LA-ACA key",
    means: "The Pseudonym CA's encryption key the Linkage Authorities seal their pre-linkage values to, so only the Pseudonym CA can open them and the RA, which carries them, cannot.",
  },
  shuffle: {
    term: "Shuffle",
    means: "The RA holds requests and reports for a while and mixes them before passing them on, so the next authority cannot tell which came from which vehicle.",
  },
  hashedid8: { term: "HashedId8", means: "The last 8 bytes of a certificate's SHA-256 hash: how messages and lists refer to a certificate." },
  ctl: { term: "CTL", means: "Certificate trust list: the root certificates every device trusts, signed by a quorum of electors." },
  crl: { term: "CRL", means: "Certificate revocation list: the signed list of certificates (or vehicles, by linkage seed) that receivers must refuse." },
  delta: { term: "Delta CRL", means: "A revocation list carrying only the changes since the previous full list." },
  "hash-entry": { term: "Hash entry", means: "A CRL entry that revokes one certificate by its hash; used for certificates without linkage values, such as an authority's." },
  "linkage-entry": {
    term: "Linkage entry",
    means: "A CRL entry carrying two linkage seeds: it revokes every pseudonym certificate of one vehicle from week iRev on, without naming the vehicle.",
  },
  nextcrl: { term: "nextCrl", means: "The deadline printed on a revocation list by which a newer list must replace it. Providers may publish sooner." },
  spdu: { term: "SPDU", means: "Secured protocol data unit: an IEEE 1609.2 message wrapper, signed, encrypted or both." },
  jer: { term: "JER", means: "JSON encoding rules (ITU-T X.697): the ASN.1 message written out as readable JSON." },
  evidence: {
    term: "Misbehaviour evidence",
    means: "What a reporting vehicle saw: the check that failed (for example a position that cannot be right) and the messages that show it.",
  },
  "1vinconsistent": {
    term: "1VInconsistent",
    means: "A single-vehicle check (SAE J3287): one sender's messages contradict themselves, for example a speed that does not match its positions.",
  },
  constantposition: { term: "ConstantPosition", means: "A check subtype: the sender reports the same position while claiming to move." },
  election: {
    term: "Elector",
    means: "One of the people or bodies who must endorse any change to the trust list (CTL); a change needs a quorum of them.",
  },
};

/** Where each glossary term is spelled in the engine's sentences. */
const TERM_PATTERNS: readonly (readonly [RegExp, string])[] = [
  [/\bJ_ij\b/, "jij"],
  [/\bLA-ACA key\b/, "la-aca-key"],
  [/\bcaterpillar and cocoon (?:public )?keys\b|\bcaterpillar key\b|\bcocoon keys?\b/, "butterfly"],
  [/\bPLVs?\b/, "plv"],
  [/\bLCIs?\b/, "lci"],
  [/\biRev\b/, "irev"],
  [/\bHashedId8\b/, "hashedid8"],
  [/\bSPDUs?\b/, "spdu"],
  [/\bCTL\b/, "ctl"],
  [/\bCRLs?\b/, "crl"],
  [/\bj-max\b|\bjmax\b/, "jmax"],
];

/**
 * A sentence cut into plain text and glossary terms, in order: what `GlossText` draws. A term is
 * matched where the engine spells it (`J_ij`, `PLVs`, `caterpillar and cocoon keys`).
 */
export function glossTerms(text: string): { readonly text: string; readonly term: string | null }[] {
  const out: { text: string; term: string | null }[] = [];
  let rest = text;
  while (rest.length > 0) {
    let best: { at: number; len: number; term: string } | null = null;
    for (const [re, term] of TERM_PATTERNS) {
      const m = re.exec(rest);
      if (m && (best === null || m.index < best.at)) best = { at: m.index, len: m[0].length, term };
    }
    if (best === null) {
      out.push({ text: rest, term: null });
      break;
    }
    if (best.at > 0) out.push({ text: rest.slice(0, best.at), term: null });
    out.push({ text: rest.slice(best.at, best.at + best.len), term: best.term });
    rest = rest.slice(best.at + best.len);
  }
  return out;
}

/** The entry for a key (case and spaces ignored), or null. */
export function glossary(key: string): GlossaryEntry | null {
  return GLOSSARY[key.toLowerCase().replace(/[\s_]+/g, "-")] ?? null;
}

/** Evidence names as the engine spells them (`1VInconsistent/ConstantPosition`), in words. */
export function evidenceWords(name: string): string {
  const parts = name.split("/").map((p) => {
    const g = glossary(p.replace(/[^A-Za-z0-9]/g, ""));
    if (p === "1VInconsistent") return "one sender's messages contradict each other";
    if (p === "ConstantPosition") return "same position while moving";
    return g ? g.term : p.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase();
  });
  return parts.join(": ");
}
