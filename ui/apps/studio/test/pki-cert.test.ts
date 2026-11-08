/**
 * The certificate card's model (`src/lib/pkiCert.ts`), the decoder's words (`src/lib/pkiPdu.ts`),
 * the signing state the inspector and HUD badge (`src/lib/security.ts` `signingStatus`) and the
 * case headline (`src/lib/pkiCases.ts`).
 *
 * The certificates are what `v2xw-server` answered `inspect.cert` on `scenarios/credential-
 * lifecycle.yaml` (2026-10-28): the ACA's own certificate, a vehicle's pseudonym certificate and
 * an enrolment certificate that a successor replaced (its validity check fails).
 */
import { describe, expect, it } from "vitest";

import { caseHeadline } from "../src/lib/pkiCases.js";
import {
  CERT_STATUS,
  STATUS_ORDER,
  VEHICLE_STATE,
  cellStatus,
  certFacts,
  certStatus,
  chainLinks,
  ctlRootIds,
  ownersOf,
  psidText,
  regionText,
  ruleText,
  spanWords,
  validityOf,
  whyNot,
  windowFraction,
  windowWords,
} from "../src/lib/pkiCert.js";
import { fieldGloss, pduText } from "../src/lib/pkiPdu.js";
import { CREDENTIAL_LABEL, type CaseView } from "../src/lib/pkiRecords.js";
import { signingStatus } from "../src/lib/security.js";

const pass = (rule: string) => ({ rule, status: "pass", detail: [] as string[] });
const RULES = ["well-formed", "chain-to-root", "root-on-ctl", "signatures", "issuer-permissions", "chain-length", "validity", "region", "crl"];

const ACA = {
  id8: "5d5ccf0f22a066dd",
  chain: ["5d5ccf0f22a066dd", "1cd35a5f7482d70b", "0a709666ff1d66de"],
  valid: true,
  time32: 731232071,
  trace: RULES.map(pass),
  tree: {
    issuer: { sha256AndDigest: "1CD35A5F7482D70B" },
    signature: { ecdsaNistP256Signature: { rSig: { "x-only": "35" }, sSig: "8B" } },
    toBeSigned: {
      appPermissions: [{ psid: 35, ssp: { opaque: "850001" } }],
      certIssuePermissions: [{ subjectPermissions: { all: null } }],
      cracaId: "1D66DE",
      crlSeries: 2,
      encryptionKey: { publicKey: { eciesNistP256: { "compressed-y-1": "DF" } }, supportedSymmAlg: "aes128Ccm" },
      id: { name: "aca.provider-a" },
      region: { identifiedRegion: [{ countryOnly: 124 }, { countryOnly: 484 }, { countryOnly: 840 }] },
      validityPeriod: { duration: { years: 30 }, start: 699675053 },
      verifyKeyIndicator: { verificationKey: { ecdsaNistP256: { "compressed-y-0": "C8" } } },
    },
    type: "explicit",
    version: 3,
  },
};

const PC = {
  id8: "f03991b3c038b181",
  chain: ["f03991b3c038b181", "5d5ccf0f22a066dd", "1cd35a5f7482d70b", "0a709666ff1d66de"],
  valid: true,
  time32: 731232071,
  trace: RULES.map(pass),
  tree: {
    issuer: { sha256AndDigest: "5D5CCF0F22A066DD" },
    toBeSigned: {
      appPermissions: [{ psid: 32 }, { psid: 38 }],
      cracaId: "1D66DE",
      crlSeries: 1,
      id: { linkageData: { iCert: 1, "linkage-value": "21AD34291C7B41479C" } },
      validityPeriod: { duration: { minutes: 10140 }, start: 731232065 },
      verifyKeyIndicator: { reconstructionValue: { "compressed-y-0": "E3" } },
    },
  },
};

const EC_SUPERSEDED = {
  id8: "43f4f5cd1a447f28",
  chain: ["43f4f5cd1a447f28", "de5db80bcfa6b333", "1cd35a5f7482d70b", "0a709666ff1d66de"],
  valid: false,
  time32: 731232071,
  trace: RULES.map((r) => (r === "validity" ? { rule: r, status: "fail", detail: ["cert 0: not valid at the policy time"] } : pass(r))),
  tree: {
    issuer: { sha256AndDigest: "DE5DB80BCFA6B333" },
    toBeSigned: {
      certRequestPermissions: [{ subjectPermissions: { all: null } }],
      id: { name: "ee-000f4240.provider-a" },
      region: { identifiedRegion: [{ countryOnly: 840 }, { countryOnly: 124 }, { countryOnly: 484 }] },
      validityPeriod: { duration: { hours: 0 }, start: 731231914 },
    },
  },
};

describe("the certificate states", () => {
  it("give every state a label, a meaning, a colour token and a distinct shape", () => {
    const shapes = new Set<string>();
    for (const s of STATUS_ORDER) {
      const i = CERT_STATUS[s];
      expect(i.label.length).toBeGreaterThan(0);
      expect(i.meaning.length).toBeGreaterThan(10);
      expect(i.color).toMatch(/^var\(--/);
      shapes.add(i.shape);
    }
    // Shape alone tells every state apart: the language reads without colour.
    expect(shapes.size).toBe(STATUS_ORDER.length);
  });
  it("cover every vehicle credential state", () => {
    for (const s of Object.keys(CREDENTIAL_LABEL)) expect(VEHICLE_STATE[s], s).toBeDefined();
  });
  it("map the pool's cells onto them", () => {
    expect(cellStatus("active")).toBe("valid");
    expect(cellStatus("downloaded")).toBe("not-yet-valid");
    expect(cellStatus("expired")).toBe("expired");
    expect(cellStatus("revoked")).toBe("revoked");
    expect(cellStatus("not-yet-available")).toBeNull();
  });
});

describe("reading a certificate", () => {
  it("names an authority's certificate by its holder, with its window, permissions and region", () => {
    const f = certFacts(ACA.tree, "aca");
    expect(f.kind.short).toBe("ACA");
    expect(f.issuer).toBe("1cd35a5f7482d70b");
    expect(f.subject).toBe("aca.provider-a");
    expect(f.permissions).toEqual(["SCMS security management · PSID 35"]);
    expect(f.issues).toBe("May issue certificates");
    expect(f.region).toBe("Canada, Mexico, United States");
    expect(f.crlSeries).toBe(2);
    expect(f.cracaId).toBe("1d66de");
    expect(f.form).toBe("explicit");
    expect(f.signature).toBe("ECDSA P-256");
    expect(f.encryptionKey).toBe(true);
    expect(f.validity?.end).toBe(699675053 + 30 * 31_556_952);
    // Without the holder, the name says what it is.
    expect(certFacts(ACA.tree).kind.short).toBe("ACA");
  });
  it("reads a pseudonym certificate: anonymous, linked, implicit, a week long", () => {
    const f = certFacts(PC.tree);
    expect(f.kind.short).toBe("PC");
    expect(f.subject).toMatch(/anonymous/);
    expect(f.linkage).toEqual({ iCert: 1, value: "21AD34291C7B41479C" });
    expect(f.permissions).toEqual(["Basic safety messages (BSM) · PSID 32", "Misbehaviour reports · PSID 38"]);
    expect(f.form).toBe("implicit");
    expect(f.validity).toEqual({ start: 731232065, end: 731232065 + 10140 * 60, stated: "10140 minutes (7 d 1 h)" });
  });
  it("reads an enrolment certificate", () => {
    expect(certFacts(EC_SUPERSEDED.tree).kind.short).toBe("EC");
  });
  it("words an unknown PSID, region and rule by what the engine sent", () => {
    expect(psidText(4000)).toBe("PSID 4000 (0xfa0)");
    expect(regionText({ circularRegion: {} })).toBe("a circle");
    expect(regionText(undefined)).toBeNull();
    expect(ruleText("new-rule").label).toBe("new rule");
    expect(ruleText("root-on-ctl").label).toBe("Root is trusted");
  });
});

describe("a certificate's state", () => {
  it("is the engine's verdict: valid when every step passed", () => {
    expect(certStatus(ACA, certFacts(ACA.tree).validity)).toBe("valid");
    expect(whyNot(ACA)).toBeNull();
  });
  it("is expired when the validity step fails after its window, with the engine's reason", () => {
    const v = validityOf(EC_SUPERSEDED.tree.toBeSigned);
    expect(certStatus(EC_SUPERSEDED, v)).toBe("expired");
    expect(whyNot(EC_SUPERSEDED)).toBe("Fails “Within its validity window”: cert 0: not valid at the policy time");
  });
  it("is not yet valid when the validity step fails before its window", () => {
    expect(certStatus({ ...EC_SUPERSEDED, time32: 731231000 }, validityOf(EC_SUPERSEDED.tree.toBeSigned))).toBe("not-yet-valid");
  });
  it("is revoked when the CRL step fails, untrusted when the chain or a signature does", () => {
    const fail = (rule: string) => ({ ...ACA, valid: false, trace: RULES.map((r) => (r === rule ? { rule, status: "fail", detail: [] } : pass(r))) });
    expect(certStatus(fail("crl"))).toBe("revoked");
    expect(certStatus(fail("root-on-ctl"))).toBe("untrusted");
    expect(certStatus(fail("signatures"))).toBe("untrusted");
  });
  it("is expiring in the last tenth of its window", () => {
    const v = validityOf(PC.tree.toBeSigned)!;
    expect(certStatus({ ...PC, time32: v.end - 30 }, v)).toBe("expiring");
    expect(certStatus({ ...PC, time32: v.start + 60 }, v)).toBe("valid");
  });
  it("places now in its window, in words", () => {
    const v = validityOf(PC.tree.toBeSigned)!;
    expect(windowFraction(v, v.start)).toBe(0);
    expect(windowFraction(v, v.end + 10)).toBe(1);
    expect(windowWords(v, v.start + 6)).toBe("ends in 7 d");
    expect(windowWords(v, v.end + 7200)).toBe("ended 2 h ago");
    expect(windowWords(v, v.start - 90)).toBe("starts in 1 min 30 s");
    expect(spanWords(31_556_952 * 30)).toBe("30 years");
  });
});

describe("the chain of trust", () => {
  const state = {
    governance: [{ id: "scms-manager/elector/0", cert: "886c6c4024e03687" }],
    providers: [
      {
        components: [
          { id: "provider-a/root/0", cert: "0a709666ff1d66de" },
          { id: "provider-a/ica/0", cert: "1cd35a5f7482d70b" },
          { id: "provider-a/aca/0", cert: "5d5ccf0f22a066dd" },
          { id: "provider-a/pg/0", cert: null, counters: { certificate: "9e778f27359609d6" } },
        ],
      },
    ],
    ctl: { roots: ["c23da18ccd8ea4807246c63d79de2548dad9777b3e299e980a709666ff1d66de"] },
  };
  it("knows every authority's certificate, the Policy Generator's from its counter", () => {
    const o = ownersOf(state);
    expect(o["5d5ccf0f22a066dd"]).toBe("provider-a/aca/0");
    expect(o["9e778f27359609d6"]).toBe("provider-a/pg/0");
    expect(ctlRootIds(state)).toEqual(["0a709666ff1d66de"]);
  });
  it("names each link, leaf to root, and says whether the trust list names the root", () => {
    const links = chainLinks(PC.chain, ownersOf(state), ctlRootIds(state), (r) => r.toUpperCase(), "ee/1000000");
    expect(links.map((l) => l.name)).toEqual(["This certificate", "ACA", "ICA", "ROOT"]);
    expect(links[0].holder).toBe("ee/1000000");
    expect(links[3].onCtl).toBe(true);
    expect(chainLinks(PC.chain, {}, ["ffff"], (r) => r)[3].onCtl).toBe(false);
  });
});

describe("the decoder's words", () => {
  it("say what each SCMS message is and who sends it", () => {
    expect(pduText("EeRaCertRequestSpdu").flow).toBe("vehicle → RA, through the proxy");
    expect(pduText("SomeNewSpdu").what).toBe("Some New Spdu");
  });
  it("gloss known fields, never an index", () => {
    expect(fieldGloss("psid")).toMatch(/kind of message/);
    expect(fieldGloss("[0]")).toBeNull();
    expect(fieldGloss("noSuchField")).toBeNull();
  });
});

describe("the signing state", () => {
  it("follows the node's own row", () => {
    expect(signingStatus(null, 0)).toBeNull();
    expect(signingStatus({ self_revoked: true, pool_valid: 5 }, 0)).toBe("revoked");
    expect(signingStatus({ pool_valid: 0 }, 0)).toBe("expired");
    expect(signingStatus({ pseudonym: "ab", pool_valid: 3, cert_valid_until: 100e9 }, 50e9)).toBe("expiring");
    expect(signingStatus({ pseudonym: "ab", pool_valid: 3, cert_valid_until: 100e9 }, 10e9)).toBe("valid");
    expect(signingStatus({ pseudonym: "ab", pool_valid: 3, cert_valid_until: 100e9 }, 200e9)).toBe("expired");
  });
});

describe("a case's headline", () => {
  const st = (stage: string, t: number, detail: Record<string, unknown> = {}) => ({ case: "provider-a/case/1", provider: "provider-a", stage, t, instance: "i", detail });
  it("times each beat from the first report", () => {
    const c: CaseView = {
      case: "provider-a/case/1",
      provider: "provider-a",
      instance: "i",
      stages: [st("reports-in", 10e9), st("decision", 12e9), st("crl-published", 40e9), st("enforced", 45e9, { installed: 3, of: 9 }), st("enforced", 47e9, { installed: 5, of: 9 })],
      steps: [],
    };
    const h = caseHeadline(c);
    expect(h.toDecision).toBe(2e9);
    expect(h.toList).toBe(30e9);
    expect(h.toEnforced).toBe(35e9);
    expect(h.enforced).toEqual({ installed: 5, of: 9 });
    expect(h.refused).toBe(false);
  });
});
