# 06 — The SCMS conformance run (WP-F1)

Package WP-F1 of [00 §4.2](00-gap-analysis-and-plan.md): one scenario on the real calendar,
every PDU captured and checked by the independent oracle (decode, signature, decryption).
Results of 2026-10-18 (worktree commit range `b1bb8b7..`, base `fd9c444`).

## 1. What runs

`scenarios/scms-conformance.yaml` (overlay on `credential-lifecycle.yaml`): the default
deployment `us-production-2025` (two providers on one CTL), the real US calendar, every
lifecycle number cited (the base's compressed values are removed), real cryptography, the
1609.2 / 1609.2.1-2022 wire edition, 200 veh/h on a 5 × 8 procedural grid (5 m/s), two RSUs,
10 % constant-position liars.

| Window (s from t0 = Tue 2026-10-20 08:50 UTC) | What happens |
|---|---|
| before t0 | factory enrolment (EeEcaCertRequest via the DCM) and the first batches for every vehicle; RSU enrolment and application certificates (`ScmsRun::preload`) |
| 0–660, radio on | liars from 60 s, detectors, real SM-MBR reports; the SCMS Manager decides the home root rollover at 120 s; i-period 614 → 615 at 600 s: vehicle top-ups, RSU application-certificate renewal (week 616) |
| 660–196,200, fast-forward | report shuffle, MA manual review, business-day CRL, hourly CRL fetches |
| 196,200–196,500, radio on | the revoked liars' frames refused by vehicles of both providers |
| 196,500–432,900, fast-forward | the rollover CTL reaches its publication deadline and is published |
| 432,900–433,200, radio on | the run ends under the new CTL |

## 2. How to run it

```
V2XW_CONFORMANCE_OUT=<dir> cargo test -p v2xw-engine --test conformance     # ~8 min
<oracle venv>/bin/python -I tools/conformance/check_run.py <dir> --json o.json  # ~8 min
<oracle venv>/bin/python -I tools/conformance/check_run.py <dir> --mutate --sample 1
```

`tests/conformance.rs` checks the five lifecycle steps from the engine's report and records
(factory enrolment by both providers; top-ups past the rollover; RSU renewal to week 616;
SM-MBR, case, CRL, and refusals by receivers of both providers; a published CTL of
sequence 1 held by both providers) and that every SPDU each provider's run emitted is in
the `pki.flow` recording exactly once, with its bytes. The capture is that recording (SPDUs,
certificates, CRL heads), every CTL / chain / policy file, a sample of the air (one 1609.2
SPDU per node per 5 s), the recipients' private keys, and the engine's own view (`truth.json`).

## 3. The oracle and where its rules come from

`tools/conformance/check_run.py` uses only the WP-A0 oracle's primitives (asn1tools COER on
the committed 2022 modules; its ECDSA, ECQV-1609, butterfly expansion, linkage values, ECIES,
KDF2, MAC1, AES-CCM). Its rules were written by the WP-F1 agent from the standard text, and
committed (`b1bb8b7`) before any engine fix:

| Rule | Source |
|---|---|
| canonical COER of every layer; protocolVersion 3; ScmsPdu version 2; the inner alternative each SPDU type names | [ASN-2.1] Protocol, the interface modules |
| `Ieee1609Dot2Data-Signed`: psid, the six ABSENT header fields, SignerSingleCert, hash of the signer's curve | [ASN-2.1] Protocol |
| SignedCertificateRequest digest (signer input empty for SignerSelf); canonicalId → canonical key | [ASN-2.1] Protocol, EcaEeInterface comments |
| digest H(H(tbs) ‖ H(signer)), ECDSA, implicit keys by ECQV-1609 | 02 §6, §7 |
| chains to a root on a CTL the oracle verified; per-pair consistency (`chain.check_pair`) | 02 §12 |
| **NEW** chain length per covering group, and "minChainLength 0 in certIssuePermissions is invalid" | [ASN-2] PsidGroupPermissions comment; 02 §12 |
| signer roles from SecurityMgmtSsp / CrlSsp; CRL crlCraca and series | [ASN-2.1] SecurityMgmtSsp; 02 §11 |
| one recipient of the SPDU's form; recipientId and P1 per form; ECIES; AES-128-CCM | 02 §8.1–8.3 |
| **NEW** an encrypted bare Tbe is encapsulated as `Ieee1609Dot2Data-Unsecured` (applies to the sealed PLV) | [ASN-2.1] `Ieee1609Dot2Data-Encrypted` note; 02 §8.3 |
| `EncryptedSigned` = Signed{Unsecured{Encrypted}} | [ASN-2.1] note on that type |
| requestHash = HashedId8 of the named SPDU as sent | [ASN-2.1] EeRa, AcaRa, EcaEe comments |
| the private response is signed by the ACA that issued its certificate | [ASN-2.1] AcaEeCertResponsePrivateSpdu; 02 §12 |
| **NEW** the RA's cocoon keys are one device's B_ij = A + f1(ck,i,j)G and J_ij = H + f2(ek,i,j)G (same i, j), the uploaded caterpillar is the device's | 02 §9.1–9.2 |
| **NEW** the device's reconstructed key e·b_ij + r matches the certificate; linkage-value = PLV1 ⊕ PLV2; iCert as requested; crlSeries 1 / 3 / 4 | 02 §6, §10; [SMP] Table 1 |
| SM-MBR: to the MA certificate, psid 0x26, headerInfo {psid, generationTime}, implicit pseudonym signer, evidence BSMs verify under signingCertificate | [SM-MBR] §3; 03 §8.1 |
| CTL: elector self-signatures, tbsCtlHash = SHA-384(tbsCtl), quorum of the electors the previous CTL approved, series and sequence + 1 | [ASN-2.1] CertManagement; 03 §10 |
| **NEW** enforcement: the devices the published linkage CRLs revoke (oracle-computed over every pseudonym it decrypted) are liars the MA decided on, and every decision before a later CRL of the device's CRACA is on it | 02 §11.2; [SM-PROV] §3.4.9 |
| **NEW** air sample: generationTime inside the signer's validity, psid in its appPermissions | 02 §12 |
| **NEW** EeEcaCertRequest crlSeries = 4 (US profile), overriding the ASN.1 constraint (0) | [SMP] §6.3.1 via 03 §5 |

Independence: the agent wrote the capture hooks in the engine test and the fixes in §5, but
no oracle rule was taken from the Rust. The two published-RA entries in §4 were settled from
the published artefacts themselves (fetched into the ignored cache by
`tools/scms-artefacts/fetch.py`), after a comment in `governance.rs` pointed at the second form.

## 4. Known deviations of the real artefacts (counted, not disagreements)

* The production chain (`cert-chain_0.oer`, 2025-08-13) RA carries certIssuePermissions
  {all, minChainLength 0, range 0, app}; the 2022 text says such a certificate is invalid.
* The 2025-07-17 test chain RAs (all three providers) carry no certIssuePermissions and
  certRequestPermissions {all, 0, 0, app}, which the chain-length text read literally does not
  allow directly under the ICA's {all, 2, 0} group. The engine's governance RA uses this form,
  its PDU-layer RA the production form.

## 5. Engine defects the run found, and their fixes

| # | Finding | Red result | Fix |
|---|---|---|---|
| F1 | SPDUs emitted after the last 1 s view tick never reached the recording | `tests/conformance.rs`: provider-b `EeRaDownloadRequestSpdu` emitted 872, captured 871 (and one SM-MBR) | `Engine::finish_run` flushes the pending `pki.*` records (`Phase2::final_pki`), only when something is pending |
| F2 | every `EncryptedIndividualPLV` encrypted the bare 9-byte PLV | oracle: 4,960 PLVs and every RaAcaCertRequest "does not decode" as Ieee1609Dot2Data | the LA seals `Ieee1609Dot2Data-Unsecured{PLV}`; the ACA opens it; `check_pdus.py` (B3a) updated |

## 6. Result: zero disagreements

Clean run (worktree HEAD after the fixes; `tests/conformance.rs` 2/2 in 442 s):

* engine: 58 devices (provider-a 31, provider-b 27) enrolled at the factory; 12/12 top-ups
  past the rollover; 2 RSU renewals to week 616; 1,313 real SM-MBRs, 4 cases, 4 CRLs; refused
  receptions of revoked senders for all four receiver/sender provider pairs; 1 CTL published
  (sequence 1, three roots) and held by both providers; 1,903,789 steps fast-forwarded.
* oracle: **`conformance: PASS (19183/19183 checks; 0 disagreements)`**: 58 EeEcaCertRequest,
  116 EcaEeCertResponse, 118 EeRaCertRequest, 118 RaEeCertAck, 1,591 EeRaDownloadRequest,
  4,960 EncryptedIndividualPLV, 2,486 each of RaAcaCertRequest / AcaRaCertResponse /
  AcaEeCertResponsePrivate (2,480 pseudonyms and 6 RSU application certificates decrypted
  and their private keys reconstructed), 1,983 SM-MBR, 4 SecuredCrl + 2 CRL heads, 4 CTLs, 4
  chain files, 2,765 air SPDUs, completeness, enforcement. The CRLs revoke exactly devices
  1000018, 1000027 and 1000028; the fourth decided liar (1000039, Time32 719853606) was
  decided after its provider's last CRL (719762405) and is pending, as the SLA allows.
  Known deviations of the real artefacts (§4): 2 + 3 chains.

## 7. Injected faults and their red results

Capture faults (`check_run.py --mutate --sample 1`; the two enforcement faults on the full
capture, whose unmutated check is PASS):

| Fault | Red result (first disagreement) |
|---|---|
| flip the last byte of an EcaEeCertResponse | signature does not verify |
| flip a ciphertext byte of an EeRaCertRequest | AES-CCM tag does not verify |
| flip the tag of an EeRaDownloadRequest | AES-CCM tag does not verify |
| flip an EeEcaCertRequest signature | signature does not verify |
| flip a sealed PLV | AES-CCM tag does not verify |
| flip an AcaRaCertResponse | signature does not verify |
| flip a SecuredCrl signature | signature does not verify |
| flip an SM-MBR ciphertext | AES-CCM tag does not verify |
| flip a CTL elector signature | signature does not verify (4) |
| flip an air SPDU signature | signature does not verify |
| RA private key off by one bit | ECIES tag does not verify (24) |
| MA private key off by one bit | ECIES tag does not verify (3) |
| LA–ACA keys zeroed | no key for recipientId (83) |
| device caterpillars' ek replaced | the uploaded caterpillar is none of the keys it holds (84) |
| the engine's `opened` plaintext replaced | the engine opened different plaintext |
| one RaEeCertAck dropped from the capture | emitted 64, captured 63 |
| a revoked liar relabelled honest | honest devices revoked: [1000018] |
| an undecided liar given a decision at t0 | device 1000048 absent from a CRL issued at 719762405 |

All 18 red. Engine faults (`tools/mutate/mutate.py`, capture rerun, then the oracle):

| Fault | Engine test | Oracle |
|---|---|---|
| ECIES P1 = SHA-256("") for certRecipInfo (`v2xw-sec/src/ecies.rs`; sender and receiver agree, the wire does not) | **green** (2/2: the engine is self-consistent) | RED: ECIES tag does not verify on EeRaCertRequest, EeRaDownloadRequest, SM-MBR |
| ACA linkage value PLV1 ⊕ PLV1 (`pdu_prov.rs`) | RED: no provider-a receiver refused a revoked sender | RED: linkage-value is not PLV1 XOR PLV2 (80) |
| before F1: the last view interval unrecorded | RED: emitted 872, captured 871 | RED: completeness |
| before F2: the bare PLV sealed | green | RED: Ieee1609Dot2Data does not decode (4,960 PLVs) |

`mutate.py` restored both injected engine faults (fresh mtime; the tree is clean).

## 8. Still open

* The air sample covers the SPDUs nodes encode (BSM); CRL broadcast and P2PCD frames are
  sized, not encoded, so they are not tapped.
* The oracle opens PDUs with the recipients' private keys as the engine exports them; a
  wrong export goes red (faults above), but the keys are not derived independently.
* Two published RA certificate forms fail the literal chain-length text (§4): a question for
  the owner or SCMS Manager, not an engine bug.
* The sample mode (3N of each type outside the sampled devices) only speeds up the fault
  runs; the verdict of record is the full run.
* `tools/pdu_check/check_pdus.py` was updated for F2; its B3a corpus was not re-run here.
