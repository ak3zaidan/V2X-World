# Validation: cryptography and data formats

Written 2026-10-18 at `0f7ababf` plus this package. Area: certificates (explicit and ECQV-1609 implicit, every `ToBeSignedCertificate` field per certificate type), HashedId and canonicalisation, the SPDU signing digest and header rules, ECDSA P-256/P-384, ECIES (P1 erratum), AES-CCM, KDF2/MAC1, butterfly expansion and the response encryption, linkage and group linkage, CRL and CTL structures and signatures, COER.

## Summary

| Verdict | Count |
|---|---|
| MATCHES | 61 |
| FIXED | 7 (rows 3, 20–24, 43; five findings) |
| DOCUMENTED DEVIATION | 6 |
| UNVERIFIABLE | 6 |
| **Total requirements** | **80** |

**What was wrong and is now fixed. Each fix has a test that fails without it (§4).**
- **CF-CANON.** The receive path hashed a received certificate as transmitted, not in canonical form. 1609.2 lets a sender send uncompressed points and an R-form `rSig`. The affected identities were the HashedId8 and HashedId10 (`hashedid::certificate_digest`, used by the peer cache, the trust store, the hash-CRL store and the parsed signer), the signer handle's signer input, and the node's §5.3.1 signer input (`NodeSecurity::verify_parsed`). The node's signer input was also always SHA-256. So the same certificate could have two identities, and a peer's SPDU failed to verify when the peer sent its certificate uncompressed.
- **CF-EXPIRY.** The rule "generationTime strictly before expiryTime" was not enforced on either side. A sender could produce an SPDU that expires when it is generated, and a receiver accepted one.
- **CF-MA.** The device-facing MA certificate was issued by the ICA, on CRL series 2, for 3 years. [SMP] Table 1 and both published US MAs (ISS pre-production and SAESOL test) are root-issued, on series 256, for 10–12 years. The MA is now root-issued, on series 256, with the root as CRACA, for 12 years.
- **CF-LIFE.** The device-facing RA, ACA, LAs, DCM and CRLG were valid for 3 years, a lifetime no published certificate has. They now take the lifetime of the published certificate of their role, from one table (`governance::ROLE_SPECS`): RA 6 y, ACA 4 y, LA 25 y, DCM 6 y, CRLG 1473 × 60 h.
- **CF-RULE.** The two PDU checkers written by the Rust authors (`tools/pdu_check/check_pdus.py`, `check_appcerts.py`) were missing these rules, now added:
  - the declared hash algorithms;
  - every `Uint8(2)` version field;
  - the request `tbsCert` permission constraints;
  - `ecaCertChain`;
  - `privateKeyInfo`.

  Every other rule was re-derived from the ASN.1 text and confirmed (§3).

**What is still different, and why.** These are documented deviations; §1 gives each one's impact.
- **CF-CRACA.** The device-facing CRL signer is ICA-issued, so the ICA is the CRACA. That matches SAESOL's production chain, where the sub-CAs name the ICA. The published ISS CRLG and [SMP] Table 1 have it root-issued instead. The governance hierarchy mixes both forms, so a governance CRL could never revoke a governance sub-CA.
- **CF-IPERIOD.** The engine starts i-periods on the UTC wall-clock grid. The 1609.2-2025 `IPeriodInfo` text implies a uniform Time32 grid, so every validity start since 2017 is 2 s late.
- **CF-UNCOMP.** On the node path, a peer's explicit certificate with an uncompressed key is refused.
- **CF-ENR-REQ.** The enrolment request carries `crlSeries` 4, which overrides the ASN.1 constraint `(0)`; this follows [SMP] §6.3.1.
- **CF-RSU-SSP.** RSU application certificates carry no SSP.
- **The published RA profile.** The published RA permission group (04 §2) is tolerated, and each acceptance is counted.

Two smaller choices sit inside fixed rows. The LAs are valid 25 y rather than the published 30 y (CF-LIFE-LA, row 22). The CRL signer stays ICA-issued (CF-CRACA, row 24).

**New oracle coverage.**
- `tools/realworld/profiles.py` compares every engine certificate with the published certificate of its role, field by field. End-entity certificates are checked against the profile rules. It goes red on the pre-fix corpus with exactly the 6 certificates that CF-MA and CF-LIFE fixed (§4).
- `crates/v2xw-sec/tests/canonical_receive.rs` re-encodes SCMS Manager's production chain in a legal transmitted form. Every HashedId8 must still equal the one its subordinates name.
- `tools/pdu_check/mutate_rules.py` shows that every added checker rule can go red.

## 0. Sources and method

Sources:
- [ASN-2] = `tools/oracle/asn1/ieee1609.2-2022/*.asn` (the normative per-field comments).
- [ASN-2.1] = `tools/oracle/asn1/ieee1609.2.1-2022/*.asn`.
- [ASN-2025] = `third_party/asn1/etsi/ieee1609.2/Ieee1609Dot2CrlBaseTypes.asn` (2025 edition).
- 02/03/01 = the research files in this folder (S4 = IEEE 1609.2a-2017 full text, Annex D.6).
- [SMP] = the SCMS Manager Profile, via 01/03.
- Published artefacts: SCMS Manager's CTL and chain files, fetched into the gitignored cache by `tools/scms-artefacts/fetch.py` (23 files, sha256 and SCMS Manager SHA-384 pinned).

Code comments were not used as sources.

Evidence types:
- **vector**: an official or CAMP test vector, transcribed in `crates/v2xw-sec/tests/vectors` with provenance.
- **oracle**: the independent A0 oracle (`tools/oracle`, asn1tools plus `cryptography`), or one of the oracle-based checkers run on the engine's emitted bytes.
- **real**: the published SCMS Manager artefacts.
- **test**: a Rust test.

Checker results at this package's head (corpora from the default `ScmsParams` runs of `camp_sizes`, `pdu_layer`, `enrolment`, `appcert`, `crl_issuance`):
- `oracle.py selftest`: 412/412 vectors in 27 checks.
- `oracle.py mutate`: 24 faults, 0 silent.
- `oracle.py real`: 164/164 checks over 8 published files.
- `check_pdus.py`: provisioning 24/24 SPDUs, enrolment 25/25 SPDUs.
- `check_appcerts.py`: 18/18 SPDUs, 3/3 keys.
- `check_run_crl.py`: 19/19 checks.
- `check_mbr.py`: 4/4 reports.
- `profiles.py`: 24/24 engine certificates match their published role or profile rule; the two DCMs are UNVERIFIABLE.

## 1. Requirement checklist

### 1.1 Encoding and base types

| # | Real-world rule (citation) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| 1 | Every hashed or signed structure is COER (X.696), modules `AUTOMATIC TAGS` ([ASN-2]; 02 §3) | `v2xw-msg` rasn bindings generated from the committed modules | oracle: `v2xw-msg/tests/scms_coer_xcheck.rs`, 45 types × 200 asn1tools values, byte-identical; real: 8 published files re-encode byte for byte (`oracle.py real`); every emitted SPDU round-trips canonically (`check_pdus.py`) | MATCHES |
| 2 | Canonicalisation before hashing: points compressed, `rSig` x-only, for Certificate (tbs and signature), ToBeSignedData, HeaderInfo.encryptionKey ([ASN-2] `CertificateBase`, `HeaderInfo`, `EcdsaP256Signature` notes; 02 §3) | send: `ec::Point::to_ecc_point` only emits compressed points; `cert.rs:557–690` `canonical`, `canonical_coer` | oracle: every engine certificate is canonical as emitted (`profiles.py`, `sizes.py`) | MATCHES |
| 3 | Receive side: a certificate's identity and signer input are its canonical form, whatever was transmitted ("at the sender's discretion", [ASN-2] `EccP256CurvePoint`) | `hashedid.rs:80` `certificate_digest`/`certificate_hashed_id10` → `cert::cert_id8`/`cert_id10`; `envelope.rs` `SignerHandle::new`; `v2xw-node/src/secure.rs` `verify_parsed` | test: `canonical_receive.rs` (self-made chain; and the published production chain when the cache is present), `secure.rs` `a_peer_certificate_sent_uncompressed_verifies_over_its_canonical_form` | **FIXED** (CF-CANON) |
| 4 | An uncompressed key in an explicit certificate is legal on the wire ([ASN-2] `EccP256CurvePoint`) | `cert.rs:410` `public_key_material` refuses it on the node's explicit-key path; `cert::verification_key` accepts it | review | DOCUMENTED DEVIATION (CF-UNCOMP): only explicit-certificate SPDU signers are affected; every US end-entity certificate is implicit ([SMP] §6.3.1, §6.3.3), and the implicit path accepts both forms (row 3) |
| 5 | HashedId3/8/10 = low-order 3/8/10 bytes of the hash; `SHA-256("")` → HashedId8 `a495991b7852b855` ([ASN-2] BaseTypes; 02 §4) | `hashedid.rs:40–60` | vector: `hashedid::the_published_example_from_the_standard_reproduces`; oracle `ieee-hashedid` (5); emit fault `hashedid-high-bytes` RED | MATCHES |
| 6 | Whole-certificate hash algorithm from the issuer field: SHA-384 for `sha384AndDigest` / `self(sha384)` ([ASN-2] `IssuerIdentifier`; 5.3.9.2) | `cert.rs:690` `whole_cert_hash_alg`; `sig1609.rs` `cert_hashed_id48` | real: every elector's HashedId48 equals SCMS Manager's published fingerprint; `rootCaApprove` HashedId32 values recomputed (`real_artefacts.rs`, 7/7) | MATCHES (receive-side HashedId8 was always SHA-256 → fixed with row 3) |
| 7 | HashedId32/48 = full SHA-256/SHA-384 (CTL `rootCaApprove`, `electorApprove`) | `sig1609.rs` | real: as row 6 | MATCHES |
| 8 | Time32 = TAI s since 2004-01-01, Time64 = TAI µs; 5 leap seconds (02 §4) | `v2xw-core/src/time.rs:283–345` | test: `time32_is_tai_known_instants`, `time32_inverse_round_trips_around_leap_seconds`; oracle `time32` (7); real: CTL `effectiveDate`s decode to the publication dates | MATCHES |
| 9 | `Duration` years = 31 556 952 s ([ASN-2] `Duration`) | `ctl.rs:82`, `envelope.rs:1576` | oracle: `check_pdus.py` `UNIT_S`; `profiles.py` | MATCHES |
| 10 | i-periods: "each time period starts exactly iPeriodLength after the previous time period started", and a certificate of `iCert` starts no earlier than `startOfNextIPeriod` (Time32) ([ASN-2025] `IPeriodInfo`); epoch 2015-01-06 04:00 ET ([SMP] Table 2) | `v2xw-proto/src/scms/params.rs:616` `period_start_time32_of`: the TAI instant of the UTC 09:00 boundary | review; `v2xw-engine/tests/scms_wiring.rs` pins 718 966 805 for i = 614 | DOCUMENTED DEVIATION (CF-IPERIOD): a uniform Time32 grid from the epoch's Time32 (347 619 603) gives 718 966 803. Every validity start and `startOfNextIPeriod` since 2017 is 2 s later than the grid, and the periods spanning 2015-06-30 and 2016-12-31 are 604 801 s long. A one-line change (start = epoch32 + p·len) and a re-pinned engine test. Owner decision: which time base the deployment uses (1609.2.1 body text is paywalled) |
| 11 | PSIDs: CRL 256 (`CrlPsid`), SCMS management 35 (`SecurityMgmtPsid`) | `crl.rs` `CRL_PSID`; `pdu.rs` `SECURITY_MGMT_PSID` | oracle: `check_pdus.py`, `check_run_crl.py` | MATCHES |
| 12 | Edition: 1609.2-2022/1609.2.1-2022 by default, 2025 where it differs (`iPeriodInfo`) (owner decision 1) | `v2xw-msg` `WireEdition`; `crl.rs:351` | test: `crl::contents_rules_and_editions` | MATCHES |

### 1.2 Certificates

| # | Real-world rule (citation) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| 13 | `CertificateBase.version` 3; implicit = `reconstructionValue`, no signature; explicit = `verificationKey` + signature ([ASN-2]) | `cert.rs` `CERT_VERSION`, `issue_implicit` (`cert.rs:834`), `issue_explicit_cert` (`cert.rs:763`) | test: `cert::only_the_explicit_form_carries_a_signature`; oracle decodes all 26 engine certificates (`profiles.py`) | MATCHES |
| 14 | At least one of appPermissions / certIssuePermissions / certRequestPermissions ([ASN-2]) | `cert.rs` | test: `a_certificate_needs_at_least_one_permission` | MATCHES |
| 15 | Explicit signature over `H(H(COER tbs) ‖ H(canonical issuer cert, or "" if self))`, H by the issuer's curve ([ASN-2] `CertificateBase.signature`) | `sig1609.rs:153` `digest_1609`; `cert.rs:763` | real: every published chain verifies to its root (Rust and oracle); oracle `chain_explicit.json` | MATCHES |
| 16 | ECQV-1609: `e = SHA-256(SHA-256(canonical tbs) ‖ SHA-256(canonical issuer))`, full 256 bits, no SEC 4 truncation; `P_U = R_U + kG`, `r = ek + d_CA`, retry on O ([S4] §5.3.2; 02 §6) | `cert.rs:822–890`; `crypto.rs:1201` | vector: CAMP `implicit.txt` (SEC 4 algebra); oracle `ecqv_1609.json` and `ecqv_certs.rs` (6 tests incl. the retry); every implicit key in every corpus re-derived by the oracle (`check_pdus.py`, `check_appcerts.py`) | MATCHES (no public 1609.2-profile ECQV vector exists, 02 §14; dual implementation is the strongest check available) |
| 17 | Root CA: explicit, self-signed SHA-256, P-256; crlSeries 0, cracaId 000000; app {35 root, 256 CrlSsp isCraca [256]}; issue {all 3/−1, 35, 38, 256 all} (published roots, 01 §2.2) | `governance.rs` `ROLE_SPECS`; root of the device-facing PKI = governance root | real/oracle: `profiles.py`, field for field with `rootca.prod.s2x.io`, 30 y | MATCHES |
| 18 | ICA: root-issued, series 2, app {35 ica}, issue {all 2/0 app+enrol, {35, 256} 1/−1} | as row 17 | `profiles.py` vs `ica.prod.s2x.io` (25 y) | MATCHES |
| 19 | ECA: ICA-issued, series 2, app {35 eca}, issue {{35 eca SSP} app, all enrol} | as row 17 | `profiles.py` vs `eca.prod.s2x.io` (15 y) | MATCHES |
| 20 | RA: ICA-issued, series 2, encryption key, `{all, 0, 0, app}` group; lifetime 6 y (SAESOL production) | `pdu.rs` components (production `certIssuePermissions` form) | `profiles.py`; `pki_profile.rs` | **FIXED** lifetime (CF-LIFE: was 3 y) |
| 21 | ACA: ICA-issued, series 2, issue {all 1/0 app}; 4 y (SAESOL production) | `pdu.rs` | `profiles.py`; `pki_profile.rs` | **FIXED** lifetime (CF-LIFE: was 3 y) |
| 22 | LA1/LA2: ICA-issued, series 2, app {35 la + laId} (ISS pre-production; 30 y under a 30 y ICA) | `pdu.rs` | `profiles.py`; `pki_profile.rs` | **FIXED** lifetime (CF-LIFE: was 3 y). DOCUMENTED (CF-LIFE-LA): 25 y, the production ICA's full lifetime, as the published LAs carry their ICA's |
| 23 | MA: **root-issued, series 256**, encryption key, app {35 ma relevantPsids}; 12 y (SAESOL test) / 1473 × 60 h (ISS) ([SMP] Table 1) | `pdu.rs` `components`, `issue_on(&root, …, SERIES_ROOT_SIGNED)` | `pki_profile.rs`; `profiles.py` (baseline RED: issuer ICA, series 2, 3 y) | **FIXED** (CF-MA) |
| 24 | CRL signer (CRLG): CrlSsp {v1, issuerIsCraca, [4, 3, 1, 2]} (ISS, byte for byte); root-issued on series 256 ([SMP] Table 1; ISS) | `pdu.rs` CRLG under the ICA, series 2; lifetime now 1473 × 60 h | `profiles.py`; `check_run_crl.py` | **FIXED** lifetime (CF-LIFE). DOCUMENTED DEVIATION (CF-CRACA, §2) for its issuer |
| 25 | PG: root-issued, series 256, app {35 pg}; 12 y | governance only | `profiles.py` vs SAESOL test PG | MATCHES |
| 26 | DCM | `pdu.rs`, 6 y | no published DCM certificate | UNVERIFIABLE (modelled as the RA's lifetime and the 1609.2.1 `DcmSsp`) |
| 27 | Elector: explicit P-384 self-signed, SSP elector v2, staggered lifetimes (published electors) | `governance.rs` electors; `ctl.rs:506` | real: `elector_certificate_is_the_published_form`; the 10 published electors verify | MATCHES |
| 28 | Enrolment: implicit, ECA-issued, `id.name`, crlSeries 4, region 840/124/484, certRequestPermissions {all}, no appPermissions, 6 y ([SMP] §4.4.1, §6.3.1; owner decision) | `enrolment.rs` | oracle: `check_pdus.py` `check_us_enrolment_tbs`, `profiles.py` | MATCHES |
| 29 | Enrolment `cracaId` in the issued certificate: "may have different values, set by the ECA" ([ASN-2.1] `EeEcaCertRequest` note) | engine keeps 000000 | review | UNVERIFIABLE (CF-ENR-CRACA): no published enrolment certificate. With 000000 a series-4 hash CRL can never match a certificate under the 1609.2 rule, so enrolment revocation is the RA's blocklist only. That is what 01 §4 reports ("CRL series 4, but in practice the RA's blocklist") |
| 30 | EeEcaCertRequest `tbsCert` constraint `cracaId '000000'H, crlSeries (0)` ([ASN-2.1]) versus [SMP] §6.3.1 request validity "crlSeries = 4" | request carries 4 | `06-conformance-run.md` | DOCUMENTED DEVIATION (CF-ENR-REQ): the profile overrides the ASN.1 subtype constraint. COER encodes both in the same 2 bytes |
| 31 | Pseudonym: implicit, ACA-issued, `linkageData {iCert, lv}`, crlSeries 1, 1 week + 1 h ([SMP] §2.1.17 Table 2, §6.3.3) | `pdu.rs:1017` `minutes(10140)` = 169 h | `profiles.py` EE rules; `check_pdus.py` (lv = plv1 ⊕ plv2, iCert = i, series 1, key re-derived) | MATCHES (the Duration choice is not canonical in 1609.2; no published pseudonym to compare) |
| 32 | Pseudonym region | absent | 04 §3 U1 | UNVERIFIABLE (a countryOnly 840 would make it CAMP's 86 B) |
| 33 | Identification: implicit, ACA-issued, id none, series 3, 1 month + 1 h (03 §7) | `appcert.rs` `hours(721)` | `check_appcerts.py`; `profiles.py` | MATCHES |
| 34 | RSU application: implicit, ACA-issued, id none, series 3, 1 week + 1 h, not butterfly (03 §7) | `appcert.rs` `hours(169)`, PSID 0x82 | `check_appcerts.py`; `profiles.py` | MATCHES |
| 35 | RSU application SSPs (SPaT/MAP/TIM service permissions) | none carried | 04 §9 item 3 | DOCUMENTED DEVIATION (CF-RSU-SSP): 1–4 B per certificate; the receiver SSP check is not exercised for these PSIDs |
| 36 | `cracaId` = HashedId3 of the CRACA; the CRL signer's CrlSsp decides which certificate that is ([ASN-2] CrlSsp; 02 §11.1) | `pdu.rs` `craca_id` (the ICA) | `check_run_crl.py`; `profiles.py` | DOCUMENTED DEVIATION (CF-CRACA, §2) |
| 37 | Chain validation (02 §12): permissions coverage, eeType, SSP consistency, chain-length inequalities, validity and region containment, same-ACA response, CTL anchor, ≤ 8 | `chain.rs:540` `validate` | test: `chain_rules.rs` (18), `chain_xcheck.rs` (500 oracle chains, same verdict) | MATCHES |
| 38 | Published RA `{all, 0, 0}` under an ICA whose `all` group has minChainLength 2: a strict check refuses it, deployed devices accept it | `ChainPolicy::us_device` tolerates exactly those two findings and counts them | test: `the_published_ra_profile_is_refused_strictly_and_accepted_by_a_us_device` | DOCUMENTED (04 §2; a real-world quirk the engine reproduces and records) |

### 1.3 The signed SPDU

| # | Real-world rule (citation) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| 39 | `Ieee1609Dot2Data.protocolVersion` 3 | `envelope.rs` | oracle `check_pdus.py` | MATCHES |
| 40 | Signing digest `H(H(COER canonical tbsData) ‖ H(canonical signer cert, or ""))`; the EE certificate even when `signer` is a chain ([ASN-2] `SignedData`; 02 §7) | `envelope.rs:1058` `signing_digest`; parse re-encodes `tbsData` | test: `overhead.rs` `a_signed_pdu_round_trips_and_verifies`; oracle: every SPDU of every corpus verifies (`check_pdus`, `check_appcerts`, conformance `check_run.py`) | MATCHES (receive-side signer input: row 3) |
| 41 | `SignedData.hashId` is the signer key's hash | sha256 on every SPDU | oracle: CF-RULE in both checkers | MATCHES |
| 42 | `rSig` x-only on send; x-only, compressed or uncompressed R accepted on receive ([ASN-2] `EcdsaP256Signature`) | `sig1609.rs:380`; `envelope.rs` parse | review; oracle `check_run.py` | MATCHES |
| 43 | "If both generationTime and expiryTime are present, the signed SPDU is invalid if generationTime is not strictly earlier than expiryTime" ([ASN-2] `HeaderInfo`) | `envelope.rs` `header_info` (send) and `parse` (receive), `EXPIRY_AFTER_GENERATION_RULE` | test: `canonical_receive.rs` `an_spdu_that_expires_at_or_before_its_generation_is_invalid` | **FIXED** (CF-EXPIRY) |
| 44 | `p2pcdLearningRequest` and `inlineP2pcdRequest` mutually exclusive ([ASN-2] `HeaderInfo`) | `envelope.rs:995, :1159` | test: `p2pcd_wire.rs` `a_receiver_refuses_both_request_fields_and_an_etsi_learning_request`; asn1tools byte match of the learning response | MATCHES |
| 45 | SCMS SPDUs: `Ieee1609Dot2Data-Signed{Tbs, 35}`: psid 35, generationTime/expiryTime/location/p2pcd/missingCrl/encryptionKey ABSENT, SignerSingleCert ([ASN-2.1] Protocol) | `pdu.rs:1425` | oracle `check_pdus.py`, `check_appcerts.py` | MATCHES |
| 46 | `SignedCertificateRequest`: signature over COER `tbsRequest`; signer input "" (self) or the enrolment certificate; `hashAlgorithmId` ([ASN-2.1] Protocol) | `pdu.rs` | oracle `check_pdus.py` `open_cert_request` (+ CF-RULE hashAlgorithmId) | MATCHES |
| 47 | `requestHash` = low 8 bytes of SHA-256 over the request SPDU as sent, with no `rSig` canonicalisation ([ASN-2.1] `EcaEeCertResponse`) | `pdu.rs` | oracle: `check_pdus.py` | MATCHES |
| 48 | Every 1609.2.1 PDU `version Uint8 (2)`; `CrlContents.version` 1 | `pdu.rs`, `crl.rs` | oracle: CF-RULE `v2`; asn1tools decodes with `check_constraints=True` | MATCHES |
| 49 | EeRaCertRequest / RaAcaCertRequest `tbsCert`: cracaId 000000, crlSeries 0, appPermissions PRESENT, certIssue/RequestPermissions ABSENT; EeRa: verificationKey ([ASN-2.1]) | `pdu.rs:1003` | oracle: CF-RULE `tbs_request_permissions` | MATCHES |
| 50 | `EcaEeCertResponse.ecaCertChain` = the ECA certificate and its chain up to and including the root ([ASN-2.1]) | `enrolment.rs` | oracle: CF-RULE in `check_pdus.py` | MATCHES |
| 51 | `AcaEeCertResponse.privateKeyInfo` present iff implicit (or explicit butterfly) ([ASN-2.1]) | `pdu.rs` | oracle: CF-RULE | MATCHES |

### 1.4 ECDSA, ECIES, CCM, KDF2, MAC1

| # | Real-world rule (citation) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| 52 | ECDSA P-256 / SHA-256 (FIPS 186-4) | `p256` crate via `crypto.rs`, `sig1609.rs` | vector: NIST CAVP SigVer 30 + range 6 (oracle); `ecdsa_with_k_matches_the_oracle_signatures` | MATCHES |
| 53 | ECDSA P-384 / SHA-384 for electors (01 §3.1) | `p384.rs` | vector: `cavp.rs` `sigver_p384_sha384_all_fifteen`; real: every elector signature on 6 published CTLs | MATCHES |
| 54 | ECIES ([S4] §5.3.5): `Z = x(vR)`, non-DHAES, `K = KDF2(Z, 48, P1)`, `c = k ⊕ K1`, `t = MAC1(K2, c)` | `ecies.rs:108–160` | vector: D.6.2 ×4 with the real P1; `d6_2_the_printed_p1_does_not_reproduce`; oracle fault `ecies-dhaes` RED | MATCHES |
| 55 | P1 per RecipientInfo: certRecipInfo SHA-256(canonical cert); signedDataRecipInfo SHA-256(the Ieee1609Dot2Data); rekRecipInfo SHA-256("") (02 §8.1) | `ecies.rs:58` | test: `rek_p1_is_the_hash_of_the_empty_string`; oracle: every encrypted SPDU opens with the oracle's P1 (`check_pdus.py`, `check_appcerts.py`) | MATCHES (signedDataRecipInfo is not used by any US flow) |
| 56 | Fresh ephemeral per encryption, V ≠ O, constant-time tag check | `ecies.rs`; randomness from a seeded SHA-256 DRBG (`pdu.rs:30`) | test: `ecies::round_trip_and_rejections` | MATCHES (determinism is a simulation property, labelled) |
| 57 | recipientId: certRecipInfo HashedId8(cert), rekRecipInfo HashedId8(COER PublicEncryptionKey), symmRecipInfo HashedId8(COER SymmetricEncryptionKey) (02 §8.1) | `pdu.rs` | oracle: `check_pdus.py`, `check_appcerts.py` | MATCHES |
| 58 | AES-128-CCM: 12-byte nonce, 16-byte tag, no AAD ([S4] D.6.1) | `ccm.rs` | vector: D.6.1 ×6; NIST CCM 70 (oracle); `cavp.rs` `ccm_dvpt128_no_aad`; oracle fault `ccm-counter-1` RED | MATCHES |
| 59 | CCM nonce construction ([S4] §5.3.8 body text) | random 12 bytes per encryption (`pdu.rs:223`) | — | UNVERIFIABLE (body text paywalled; never reused under one key) |
| 60 | KDF2 = ANSI X9.63 with SHA-256 | `kdf.rs:24` | vector: D.6.4 ×4, NIST X9.63 ×20 | MATCHES |
| 61 | MAC1 = HMAC-SHA-256, 128-bit tag | `kdf.rs:44` | vector: D.6.3 ×7, RFC 4231 ×7 | MATCHES |
| 62 | The encrypted plaintext is an `Ieee1609Dot2Data` (Tbe wrapped as unsecured); EncryptedSigned = Signed(Unsecured(Encrypted)) ([ASN-2.1] Protocol notes) | `pdu.rs` | oracle: `check_pdus.py` `open_plv`, `check_appcerts.py` `open_signed_any` | MATCHES |

### 1.5 Butterfly keys

| # | Real-world rule (citation) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| 63 | `x_sign = 0^32‖i‖j‖0^32`, `x_enc = 1^32‖i‖j‖0^32`, i and j 32-bit; `f = ⊕`-Davies–Meyer over x+1..x+3, mod n (02 §9.2) | `butterfly.rs:96–160` | vector: CAMP `bfkeyexp` (sign and encryption), OpenSCMS (i = 7590, j = 0); swapped-prefix fault RED (A4) | MATCHES |
| 64 | Cocoon keys `B = A + f1·G`, `J = H + f2·G`; implicit issuance with B as R_U, `d = e·b + r` (02 §9.3) | `butterfly.rs`; `pdu.rs` | oracle: `oracle_butterfly_cocoon_keys_and_both_aca_variants_reproduce`; `check_pdus.py` re-derives B, J and d for every certificate | MATCHES |
| 65 | Original-variant response: encrypted to J with rekRecipInfo, signed by the ACA that issued the inner certificate (02 §9.4; [ASN-2.1]) | `pdu.rs` | test: `the_original_variant_response_is_bit_exact_and_opens_only_with_h_ij`; oracle: same-ACA rule in both checkers | MATCHES |
| 66 | Explicit butterfly: `U = B + c·G`, `privateKeyInfo = c` | `butterfly.rs` | oracle: `official_vectors.rs` (both ACA variants) | MATCHES |
| 67 | Unified / compact unified refused (owner decision; §9.3.4.2 key derivation unpublished) | `butterfly.rs` | test: `the_unified_variants_are_refused_until_their_key_is_confirmed` | MATCHES (decision) |

### 1.6 Linkage values

| # | Real-world rule (citation) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| 68 | `ls(i) = SHA-256(la_id ‖ ls(i−1) ‖ 0^112)[0:16]`, la_id 16 bits (02 §10.1) | `linkage.rs:175` | vector: CAMP `lv` (i = 0, 1); fault `lv-no-pad` RED | MATCHES |
| 69 | `plv = (AES_ls(la_id‖j‖0^80) ⊕ input)[0:9]`; `lv = plv1 ⊕ plv2` | `linkage.rs:205, :221` | vector: CAMP lv(0, 8) = `688ACF70E25EC01F32`; oracle: every pseudonym's lv = PLV1 ⊕ PLV2 of the PLVs forwarded (`check_pdus.py`) | MATCHES |
| 70 | Group linkage gs/gplv/glv and the encrypted index (CAMP `glv`) | `linkage.rs:455–` | vector: CAMP `glv` (i = 0, 1) | MATCHES |
| 71 | `GroupLinkageValue.jValue` (4 B) and its relation to CAMP's ei(j, k) | — | — | UNVERIFIABLE (02 §15 item 3) |
| 72 | PLV transport: `EncryptedIndividualPLV {version 2, laId, SymmEncryptedSingleRecipient}`, AES-CCM under the LA–ACA key, recipientId HashedId8 of that key ([ASN-2.1] AcaRa) | `pdu.rs` | oracle: `check_pdus.py` `open_plv` | MATCHES |
| 73 | LCI, PLV request and LA key formats | modelled | — | UNVERIFIABLE (NULL interfaces in 1609.2.1; 02 §10.3) |

### 1.7 CRLs and CTLs

| # | Real-world rule (citation) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| 74 | `CrlContents` v1, nextCrl > issueDate, series 1 linkage and others hash, jmax ≥ 1, iMax ≥ iRev, `iPeriodInfo` only in 2025 ([ASN-2] CrlBaseTypes) | `crl.rs:351` `check_crl` | test: `contents_rules_and_editions`; oracle `check_run_crl.py` | MATCHES |
| 75 | SecuredCrl: psid 256, every other header field absent, signer's CrlSsp lists the series, crlCraca by isCraca / issuerIsCraca ([ASN-2] Crl, CrlSsp) | `crl.rs:479` `verify_crl` | test: `crl_ssp_and_header_rules`, `crl_ssp_encoding_matches_the_published_root_ssp`; oracle `check_run_crl.py` | MATCHES |
| 76 | Device matching: cracaId and crlSeries equal; hash entries by canonical HashedId10; linkage entries walk forward from iRev to iMax over j < jmax (02 §11.2) | `crl.rs:529` `revokes`; `envelope.rs` `CrlStore` | test: `revocation_matches_forward_and_never_backward`, `a_crl_entry_with_a_wide_index_range_still_catches_a_high_index`; oracle `oracle-crl`; `canonical_receive.rs` (hash entry and transmitted form) | MATCHES |
| 77 | A revoked CA revokes everything that chains to it ([S4] §5.1.3.1) | `v2xw-node/src/rxval.rs` | test: `rx_validation.rs` `a_revoked_aca_revokes_every_pseudonym_under_it` | MATCHES |
| 78 | CTL: `FullIeeeTbsCtl`, signatures by ≥ quorum electors the prior CTL trusts, `tbsCtlHash = SHA-384(tbsCtl)`, HashedId48 electors and HashedId32 roots (03 §3.3) | `ctl.rs:519` `verify_ctl` | real: 6 published CTLs verify in order; byte flips break them; `two_of_five_is_rejected_three_of_five_accepted` | MATCHES |
| 79 | CertificateChain and CompositeCrl SPDUs ([ASN-2.1] CertManagement) | `ctl.rs:343, :399` | real: both chain files decode and verify; test: `chain_file_and_composite_crl_round_trip` | MATCHES |
| 80 | The non-canonical SAESOL ACA in the 2025-07-17 test chain (extension bitmap `02 06 80`) is tolerated: its signature verifies over the canonical re-encoding | `ctl.rs` | real: oracle names it (`KNOWN_NONCANONICAL`), Rust verifies it | MATCHES |

## 2. Finding CF-CRACA (documented deviation, needs an owner decision)

The published artefacts show two consistent arrangements:

- **ISS, pre-production.** Published certificates: root, ICA, ECA, PCA, RA, LA1, LA2, MA, CRLG, PG.
  - The CRLG is **root-issued**, on series 256, with CrlSsp `{issuerIsCraca, [4, 3, 1, 2]}`, so the CRACA is the **root**.
  - Every sub-CA names the root as its `cracaId`.
  - [SMP] Table 1 agrees: "CRL signer: signed by root".
- **SAESOL, production.** Published certificates: root, ICA, ECA, RA and 2 ACAs; no CRLG.
  - Every sub-CA names the **ICA** as its `cracaId`.
  - So SAESOL's CRL signer must have the ICA as CRACA, which means an ICA-issued signer with issuerIsCraca.
  - That is inferred: SAESOL's CRLG is not published.

The engine uses each arrangement in a different place:
- **Device-facing hierarchy.** The engine takes the SAESOL form. The CRLG is ICA-issued, on series 2, with issuerIsCraca. Every pseudonym, identification, application and component certificate names the ICA. This hierarchy is internally consistent.
- **Governance hierarchy (WP-B5).** The engine takes ISS's root-issued CRLG but SAESOL's ICA-CRACA sub-CAs. A CRL signed by the governance CRLG has crlCraca = root, so it can never match a governance sub-CA, whose cracaId names the ICA. Component revocation is therefore not expressible in that hierarchy.

To match ISS exactly, the engine would need three changes:
- move the device-facing CRLG under the root, on series 256;
- set every certificate's cracaId to HashedId3(root);
- re-pin the tests that encode the ICA as CRACA (`check_pdus.py`, `check_appcerts.py`, `check_run_crl.py`, `tools/conformance/check_run.py`, and `v2xw-engine/tests/scms_wiring.rs:158`).

This was not done here, for two reasons: the build budget, and because SAESOL's production chain is the deployment the default profile clones. **Recommendation:** keep the device-facing form, and give the governance sub-CAs the CRACA their CRL signer can reach (the root), or move the governance CRLG under the ICA.

## 3. Re-derivation of the PDU checker rules

The rules of `tools/pdu_check/check_pdus.py` and `check_appcerts.py` were written by the agents that wrote the Rust. Each rule was re-derived here from [ASN-2.1]/[ASN-2] constraints and comments and from 02/03 only.

**Confirmed as stated:**
- COER canonical round trip of every layer;
- protocolVersion 3, ScmsPdu version 2;
- header {psid 35} only (stricter than the ASN.1, which leaves the extension fields unconstrained; sound for the engine's output);
- SignerSingleCert / SignerSelf;
- the §5.3.1 digest with the signer's curve hash;
- the chain to the emitted root;
- certRecipInfo to the RA certificate with P1 = SHA-256(canonical RA certificate);
- rekRecipInfo with P1 = SHA-256("") and recipientId HashedId8(COER PublicEncryptionKey);
- symmRecipInfo with recipientId HashedId8(COER SymmetricEncryptionKey);
- the PLV as Ieee1609Dot2Data-Unsecured;
- requestHash over the SPDU as sent;
- firstI iff butterfly;
- the same-ACA response rule;
- lv = PLV1 ⊕ PLV2;
- `B_ij`, `J_ij` and `d = e·b + r` by the oracle's own expansion;
- the enrolment profile of [SMP] §6.3.1 (with CF-ENR-REQ);
- successor enrolment: no overlap, same name and region, a new key;
- download file names (CAMP form; the 1609.2.1 path table is paywalled).

**Missing, added (CF-RULE):**
- `SignedData.hashId` and `SignedCertificateRequest.hashAlgorithmId` equal the signer key's hash.
- The `version` field of EeEcaCertRequest, EcaEeCertResponse, EeRaCertRequest, RaEeCertAck, RaEeCertInfo, RaAcaCertRequest, AcaRaCertResponse and AcaEeCertResponse. The oracle's asn1tools decode also enforces these; the rule is kept explicit.
- The EeRaCertRequest and RaAcaCertRequest `tbsCert` permission constraints, and EeRaCertRequest's `verificationKey`.
- `ecaCertChain` from the ECA certificate up to the emitted root.
- `privateKeyInfo` present for an implicit certificate.

**Corrected:** none. No existing rule contradicted the standard text. The one conflict between the US profile and the ASN.1 (EeEcaCertRequest `crlSeries` 4) is already documented as CF-ENR-REQ.

## 4. Fault injection (every new check shown able to fail)

| Check | Fault | Result |
|---|---|---|
| `canonical_receive.rs` (3 tests), `secure.rs` node test, `pki_profile.rs` | the pre-fix sources: `swap.sh unfix` restores `hashedid.rs`, `envelope.rs`, `pdu.rs`, `governance.rs` and the one-line `secure.rs` change from `0f7ababf`, with the new tests kept | RED, 5/5: the transmitted root's HashedId8 differs; the sender signs an SPDU whose expiry equals its generation; the published chain's transmitted ICA hashes differently; the node returns `Invalid` for the uncompressed peer certificate; the RA lifetime is `years(3)` |
| the same tests | the fixed sources | GREEN: all 5 pass, and so do the full suites of `v2xw-sec` (including `real_artefacts` and the published-chain test, with the cache present), `v2xw-node` and `v2xw-proto` |
| `profiles.py` | the pre-fix engine corpus (build run 1) | RED: `15/24` before the documented choices were annotated, `18/24` after, exactly ACA, CRLG, LA1, LA2, MA, RA (lifetime; MA also issuer, series and CRACA) |
| `profiles.py` | the fixed corpus | PASS: 24/24 (the two DCMs are UNVERIFIABLE) |
| CF-RULE (both checkers) | `tools/pdu_check/mutate_rules.py`: hashId expected sha384; version 3 required; certIssuePermissions required; ecaCertChain reversed; acaEeCertResponse privateKeyInfo dropped (data faults applied after the signature checks) | 12/12 RED, clean runs PASS |

## 5. Reproduce

```
python3 -I tools/scms-artefacts/fetch.py                       # the published files, sha256-pinned
<venv>/bin/python -I tools/oracle/oracle.py selftest            # 412/412
cargo test -p v2xw-sec --test canonical_receive                 # runs the published-chain test when the cache is present
cargo test -p v2xw-node --lib a_peer_certificate_sent_uncompressed
cargo test -p v2xw-proto --test pki_profile
V2XW_SIZES_OUT=f.json cargo test -p v2xw-proto --test camp_sizes
<venv>/bin/python -I tools/realworld/profiles.py f.json third_party/scms-artefacts/files
V2XW_PDU_OUT=d cargo test -p v2xw-proto --test pdu_layer && <venv>/bin/python -I tools/pdu_check/check_pdus.py d
V2XW_APPCERT_OUT=a cargo test -p v2xw-proto --test appcert && <venv>/bin/python -I tools/pdu_check/check_appcerts.py a
<venv>/bin/python -I tools/pdu_check/mutate_rules.py <dir holding pdu/ enr/ app/>
```
