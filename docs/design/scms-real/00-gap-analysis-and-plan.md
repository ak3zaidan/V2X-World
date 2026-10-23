# 00 — SCMS realism: gap analysis and build plan

Status: plan, 2026-10-16, awaiting owner review. No code changed.
Inputs: `01-deployment-and-governance.md` (01), `02-cryptography-and-formats.md` (02) and `03-protocols-and-operations.md` (03) in this folder, plus an audit of the engine at `main` 70981b7. The audit read targeted ranges of:

- `crates/v2xw-sec` (`crypto.rs`, `cert.rs`, `envelope.rs`, `butterfly.rs`, `linkage.rs`, `primitive.rs`, `hashedid.rs`)
- `crates/v2xw-proto/src/scms` (`mod.rs`, `params.rs`, `governance.rs`, `msg.rs`, `run.rs`)
- `crates/v2xw-node/src` (`secure.rs`, `stores.rs`, `runtime.rs`)
- `crates/v2xw-engine/src` (`phase2.rs`, `backend.rs`, `run.rs`)
- `crates/v2xw-threat/src` (`ma.rs`, `resolve.rs`)
- `ui/apps/studio/src` (`components/BackendView.tsx`, `lib/backend.ts`, `lib/security.ts`, `components/Inspector.tsx`)

Goal (owner, 2026-10-16): SCMS in the engine must match the US system as it is actually deployed. That covers which entities exist, how many, where they are, how they talk, every byte of cryptography and every format, and all of it validated. The Studio must track and show it properly.

Reading rule for this document:
- **Identical** means bit-exact where a standard fixes the bits, and parameter-exact where SCMS Manager or a pilot fixes the number.
- Where the sources say UNCONFIRMED, the engine must make the value a named, flagged setting. It must never become a silent constant.

---

## 0. Summary

### 0.1 The ten gaps that matter most

| # | Gap | Why it matters |
|---|---|---|
| G1 | **Vehicles, RSUs and VRUs sign under explicit certificates from a per-node stand-in issuer** (`v2xw-node/src/secure.rs` `NodeSecurity::provision`, one local authority per node). The real system uses ECQV implicit certificates issued by the ACA under ICA → Root, with that root on the CTL. | No chain exists, so trust cannot be decided. An explicit certificate is also about 60 B larger on the air than the real 86 B implicit one (03 §12), which inflates BSM airtime whenever the full certificate is attached. |
| G2 | **The node verify path checks one signature and nothing else** (`SecCtx`/`verify_parsed`): no chain, validity, region, PSID/SSP or chain-length checks. `envelope.rs` `verify_plan` exists, but it only plans costs, stops one level above the trust anchor, and is not called by the node. | Untrusted, expired, out-of-region or wrong-PSID messages are accepted. |
| G3 | **Revocation checks a side-channel value.** The certificate on the air carries a linkage value from seeds derived from the node index (`secure.rs` `device_linkage`, LA ids 1 and 2). The CRL check instead uses a different value that the engine looks up from the SCMS provisioning record (`engine/src/run.rs` ≈4534) and passes in `RxFrame.claimed_linkage`. | The CRL never acts on the bits in the certificate. A real receiver has only those bits. |
| G4 | **Backend messages are Rust structs with modelled sizes** (`proto/src/scms/msg.rs`). Encryption is a type-level gate (`SealedForPca::open` compares node ids). There is no ECIES, AES-CCM, KDF2 or MAC1 in `v2xw-sec`; ECIES exists only as a size descriptor (`scheme/kem.rs`). | None of the IEEE 1609.2.1 PDUs (`ScmsPdu`, PSID 35) exist. Encryption costs size and time only. Nothing can be checked against a standard. |
| G5 | **The governance artefacts use an ad-hoc format** (`governance.rs` `AuthorityCert::tbs_hash` hashes a role string and a node index; `SignedCtl` is custom). Electors are 3 with quorum 2 on P-256. The real system has 5 electors, quorum 3, ECDSA P-384, a `FullIeeeTbsCtl` with HashedId48/32 lists, and CtlSeriesId `0348000000000000`. | The trust anchor itself is wrong in count, curve and format. |
| G6 | **CRLs are not 1609.2 CRLs.** The proto CRL is a custom digest over an entry list (`run.rs` `crl_digest`). The engine accepts a CRL by set membership against the generator's in-memory list (`phase2.rs` ≈3646), not by signature. Only linkage entries exist. There are no series 2/3/4/256, no `crlCraca`, `iRev`, `indexWithinI`, `jmax` groups or `nextCrl`, and no hash CRLs for RSU certificates. | The CRL is the only revocation channel a real device has. |
| G7 | **The topology is one of each role at fixed node ids** (`params.rs` `ScmsNodes` 1–11, governance 20–28). It has no providers, no locations, and a LOP, CRL store and CRL broadcast that the US system does not deploy. | Production today is 2 providers on one CTL, each with its own full tree, MA and PG (01 §2). |
| G8 | **The time base is wrong in two ways.** I-period 0 starts at run start (`ScmsParams.epoch = 0`) instead of 2015-01-06 04:00 Eastern (≈ i = 614 now). `Time32` ignores leap seconds (`core/src/time.rs` ≈255), so it is 5 s off TAI. Scenarios also compress calendars (`i_period_s: 60` in five scenarios). | Certificate validity, CRL `iRev` and every Time32 field differ from the real bits. |
| G9 | **The misbehaviour and revocation path is simplified.** The MA works one case at a time (`run.rs` `MaState::case: Option<Case>`). The two-report LA investigation (`ScmsRun::investigate`) has no caller outside tests. The report is not the SCMS Manager MBR v1.0 format (PSID 0x26, weight decay ×0.9 at 05:00 ET). CRLs publish on decision by default (`publish_on_cadence: false`), not on the 1-business-day SLA. | Revocation latency, the headline Phase 2 result, is measured on a path that differs from the real one. |
| G10 | **Validation is only self-referential.** Butterfly and linkage are pinned to the legacy Python port's own outputs (`sec/tests/legacy_vectors.rs`). No IEEE 1609.2a Annex D.6 vector, CAMP vector, NIST CAVP vector or real published SCMS artefact is used anywhere (02 §13). SHA-256 and AES cost zero time (`primitive.rs` ≈1541: "charges it as zero"). | A wrong bit would pass every test today. |

Several things are already right and are kept:
- The butterfly expansion formula and prefixes.
- The linkage seed, PLV and LV formulas.
- The 1609.2 signing digest `H(H(tbs)‖H(cert))`, with x-only `rSig`.
- HashedId3/8/10 over canonical COER.
- The certificate rotation and pool model.
- The node's real ECDSA P-256 sign and verify.
- The OBE stopping transmission when it finds itself revoked.
- The ECQV algebra in `crypto.rs::ecqv`. It is correct, but nothing outside `v2xw-sec` calls it.
- The engine's IEEE 1609.2 ASN.1, which is already the **2025** module set (`Ieee1609Dot2` major 2, minor 7; forge commit 77e2c822, `third_party/asn1/etsi/PROVENANCE.md`).

### 0.2 Decisions the owner must make (details in §5)

1. Which standard edition is the wire default: 2022 (what the SCMS Manager profile v1.0.3 profiles) or 2025/2026.
2. Whether to buy IEEE 1609.2-2025 and 1609.2.1-2026. They contain the Annex D vectors, the unified/CUBK key formula, the v3 URL paths and the parameter semantics.
3. Whether presets name the real providers and hostnames or use neutral labels.
4. How to handle the CAMP test-vector repository, which has no licence file.
5. The shuffle threshold conflict: 1,000 plus a 2-day age (CAMP 1.2.2) or 10,000 per day (Brecht).
6. Data-centre placement, which no source publishes.
7. The default MA mode: pilot-style manual review, or automatic detection to CRL.
8. Real one-week calendars plus a fast-forward mode, instead of compressed periods.
9. New crate dependencies, and a Python oracle in dev tooling.

---

## 1. Gap table

Columns:
- **Real** is the real-world element, with its source in this folder.
- **Today** is what the engine does, with the file.
- **Gap** is the difference.
- **Identical requires** is what the engine must do to match.
- **Validated by** is how the result will be proved.

"Oracle" means the independent Python reference of WP-A0. A different agent writes it from the standards without reading the Rust. It uses `asn1tools` for the OER codec and the `cryptography` package for the primitives.

### 1.1 Encoding and primitives

| # | Real | Today | Gap | Identical requires | Validated by |
|---|---|---|---|---|---|
| 1 | COER of the 1609.2 modules, with canonicalisation (compressed points, x-only rSig) before hashing (02 §3) | Generated by `rasn-compiler` from the 2025 modules; the encoder emits only compressed points (`hashedid.rs` notes) | Small. The edition differs from the deployed profile (2022). No independent cross-check | Pin the edition per owner decision 1. Encode every hashed structure canonically | Oracle round-trip of every PDU type. Decode real published certificates and CTLs (row 14) |
| 2 | IEEE 1609.2.1 ASN.1 (`Ieee1609Dot2Dot1*`: Protocol, EeRa, AcaRa, AcaEe, EcaEe, CertManagement), `ScmsPdu` version 2 (03 §3) | Absent from `third_party/asn1` and `v2xw-msg` | Whole layer missing | Commit the forge modules with provenance and generate bindings. Risk: `rasn-compiler` 0.16 already fails on `WITH COMPONENTS` in TS 102 941 (PROVENANCE §1), and 1609.2.1 uses parameterised types heavily. Fallback: hand-written COER for the profiled subset, generated tables checked against the oracle | Oracle encodes the same values and bytes match. Fuzzed decode/encode round-trip |
| 3 | HashedId3/8/10/32/48: low-order bytes of the hash of the canonical encoding (02 §4) | Correct for P-256/SHA-256 certificates (`hashedid.rs`). Governance uses its own digest over `tbs_hash‖sig` (`governance.rs` `digest`) | Governance digest is wrong. HashedId32 (root on the CTL) and HashedId48 (elector, SHA-384) are absent | One digest path for every artefact. Add SHA-384 for P-384 electors | 1609.2 example `SHA-256("")` → `a495991b7852b855`. Real CTL `rootCaApprove` HashedId32 values recomputed from the embedded root certificates (row 14) |
| 4 | Time32 = TAI seconds since 2004-01-01, Time64 = TAI µs; 5 leap seconds so far (02 §4) | UTC without leap seconds (`core/src/time.rs` ≈255) | Off by 5 s from the real bits | Leap-second table and a TAI conversion in `v2xw-core::time`. Keep UTC for display | Known instants: 2015-01-06 09:00 UTC → Time32 = 347 619 603 (derived, 3 leap seconds by then); the oracle converts the same instants; the real CTL `effectiveDate` 679 737 605 decodes to ≈ 16 Jul 2025 (01 §3.1) |
| 5 | ECDSA P-256 over `H(H(tbsData)‖H(signer cert or ""))` (02 §7) | Implemented (`envelope.rs` `signing_digest`, x-only rSig) | Only self-checked | Keep. Add the signer-input rules for `self` and the chain form | NIST CAVP SigVer P-256/SHA-256. Vanetza `calculate_message_hash` cross-check on fixtures. Oracle |
| 6 | ECDSA P-384 with SHA-384 for electors (01 §3.1) | Absent; electors are P-256 | Missing curve | `p384` support in `v2xw-sec` | NIST CAVP P-384 SigVer. Verify the real elector signatures on the published CTL (row 14) |
| 7 | ECIES per 1609.2a §5.3.5: KDF2 (X9.63), K1 16 B XOR, K2 32 B, MAC1 = HMAC-SHA-256 truncated to 128 bits, non-DHAES, P1 per RecipientInfo type (02 §8.2) | Absent. Size-only descriptor `ecies-p256` (`scheme/kem.rs`) | Whole primitive missing | `sec/src/ecies.rs`, with P1 computed for `certRecipInfo`, `signedDataRecipInfo` and `rekRecipInfo` | 1609.2a D.6.2, all 4 vectors, using the **real** P1 from CAMP `ecies.py`. Mutation: the printed (wrong) P1 must fail (02 §8.2 erratum). D.6.3 MAC1 ×7, D.6.4 KDF2 ×4, RFC 4231 |
| 8 | AES-128-CCM: 12-byte nonce, 16-byte tag, no AAD (02 §8.3) | Absent | Missing | `sec/src/ccm.rs` over the existing `aes128.rs`, or the `ccm` crate | 1609.2a D.6.1 ×6. NIST CAVP CCM DVPT |
| 9 | ECQV per 1609.2: e = full 256-bit `SHA-256(H(tbsCert)‖H(issuer cert))`, no 255-bit truncation (02 §6) | `crypto.rs::ecqv` has the algebra. `cert_hash_1609` hashes whatever bytes the caller passes, and nothing outside `v2xw-sec` calls it | Not on any certificate path. The input is not pinned to `tbsCert` | Issue and reconstruct with `tbsCert` canonical COER as input. Run the SEC 4 retry when the result is the point at infinity | No public 1609.2-profile vector exists (02 §14). Three checks instead: (a) the CAMP `implicit.txt` SEC 4 vectors with a SEC 4 hash switch, testing the algebra; (b) dual implementation against the oracle; (c) reconstruct any implicit certificate in the real published chain files and verify the next signature in the chain (row 14). Mutation: SEC 4 truncation, or hashing the whole certificate, must fail (c) |
| 10 | Butterfly expansion f(k, x), prefixes `00000000`/`FFFFFFFF`, i and j as 32-bit fields (02 §9.2) | Implemented (`butterfly.rs`), explicit variant only; the cocoon encryption key J is never used | Implicit variant missing (B as R_U). No response encryption | Implicit issuance with B as R_U, and private-key recovery `d = e·b + r` | CAMP `bfkeyexp.txt` (sign and encryption), OpenSCMS encryption vector (i = 7590, j = 0). Mutation: swapping the prefixes must fail |
| 11 | Linkage: `ls(i)`, PLV (9 B), LV; group linkage (02 §10) | Individual linkage implemented (`linkage.rs`) and pinned to legacy outputs only. No group linkage | Never checked against CAMP | Keep the formulas. Add group linkage (gs, gplv, glv) as an option | CAMP `lv.txt` (i = 0, 1; la_id 0x8E09/0x59CE → lv(0,8) = `688ACF70E25EC01F32`), `glv.txt`. Mutation: dropping the 0^112 pad must fail |
| 12 | PLV transport: `EncryptedIndividualPLV`, AES-CCM under a key shared between that LA and the ACA, `symmRecipInfo` (02 §10.3) | `SealedForPca<T>`, a node-id check | Not encrypted | Real AES-CCM with HashedId8 of the shared key. Shared-key rotation as a parameter | Oracle decrypts the bytes the RA forwards. Privacy test: RA code holds only ciphertext (keep `tests/privacy.rs` intent) |
| 13 | 1609.2 certificates for every authority, with `certIssuePermissions`, `cracaId`, `crlSeries`, region and names matching the FQDN (03 §9) | Ad-hoc `AuthorityCert` (role string, node index, 20 y/5 y uncited lifetimes) | Format and lifetimes wrong | Real `Certificate` for RCA (explicit, self-signed), elector (explicit, P-384), ICA, ECA, RA, ACA, LA, MA, PG, CRLG. Whether sub-CAs are explicit or implicit is read from the published chain files. Lifetimes from 01 §2.2 (ISS pre-production: 30 y sub-CAs, MA/CRLG ≈ 10 y; root certificate 70 y, key 20 y) | Decode real chain files with our decoder. Our certificates re-decoded by the oracle. CAMP Table 3 sizes as a ±few-byte sanity band (RA 217 B, ICA 195 B, …) |
| 14 | Published artefacts: production CTL (13 Aug 2025), test CTL (17 Jul 2025), ISS/SAESOL/Autocrypt chain files (01 §2.2, §3.1) | Not used | No real-world cross-check | A fixture corpus of the published files with sha256 provenance | **Real-world check:** decode each CTL, recompute HashedId48/32, verify ≥ 3 elector P-384 signatures with the published elector keys, and verify every chain-file signature (and implicit reconstruction) up to the root. This is the strongest validation available without paywalled text |

### 1.2 Protocols

| # | Real | Today | Gap | Identical requires | Validated by |
|---|---|---|---|---|---|
| 15 | Every EE↔SCMS message is an `Ieee1609Dot2Data` carrying a `ScmsPdu` under PSID 35, with the protection pattern per PDU in 03 §3.2 | `ScmsMsg` enum with sizes from `sizes.rs` | No real bytes; protection is modelled | Real PDUs for every row of 03 §3.2 that the US profile uses. Signer `SignerSingleCert`, header psid only | Oracle decodes every PDU a run emits (sampled) and verifies every signature and decryption |
| 16 | Enrolment at the factory: secure environment (DCM), canonical key, implicit enrolment certificate, region USA/Canada/Mexico (840/124/484), `crlSeries` 4, wildcard `certRequestPermissions`, `id.name` unique (03 §5) | `EnrolmentCert` struct; DCM → ECA flow with modelled stages | Format, region, series and permissions missing | `EeEcaCertRequestSpdu` → `EcaEeCertResponseSpdu` with `privateKeyInfo`. RA registration of the enrolment certificate (direct authorisation). Bootstrap installs the elector, root and RA certificates and the RA URL | Oracle verifies; chain validation (row 29) accepts the enrolment certificate for PSID 35 requests only |
| 17 | Re-enrolment 12–24 months before expiry, with no overlap, through the RA (`eeRaSuccessorEnrollmentCertRequest`). Enrolment lifetime 6 y (CAMP 1.2.2) or 3 y (5GAA draft) (03 §5) | `reenrol_lead` = 1 week, uncited; lifetime 6 y, uncited | Lead time wrong by about 2 orders of magnitude | Defaults: 18-month lead (inside the 12–24 range), 6 y lifetime, both settings | Unit test on the calendar. Scenario: a device crosses its expiry with no gap and no overlap |
| 18 | Provisioning: `EeRaCertRequest` (original butterfly, A, ck, H, ek) signed by the enrolment certificate, then encrypted to the RA; `RaEeCertAck` (requestHash, firstI, nextDlTime); `ra-maxReqs` = 1 (03 §1, §6) | Modelled request and acknowledgement | No real PDUs or encryption | Real PDUs. The RA keeps j and the device's file folder | Oracle decrypts the request with the RA key and verifies the enrolment signature |
| 19 | RA → ACA: one `RaAcaCertRequestSpdu` per certificate after the shuffle, with `linkageInfo`, `certEncKey` = J, a dummy LV and cracaId `000000` (03 §3.3) | Modelled `CertRequestItem` | As row 15 | Real PDUs. The ACA inserts the real LV, cracaId and crlSeries 1 | Oracle. Invariant: the ACA never sees j or the EE identity (type test) |
| 20 | Shuffle: CAMP 1.2.2 `shuffle_threshold` 1,000 plus `max_cert_request_age` 2 days, vs Brecht 10,000 per day (03 §11 conflict) | 10,000 / 1 day, cited as "CAMP-EE §2.2.7" (`service.rs` `CAMP_SHUFFLE`) | Default disputed; citation not reconciled with 03 | Both as presets. Default per owner decision 5 | Card cites both. Test: the release happens at the threshold or at the age, whichever comes first |
| 21 | ACA → EE: `AcaEeCertResponsePrivateSpdu`, encrypted to J and then signed by **the same ACA** that issued the certificate. The RA zips one i-period per file `<16-hex requestHash>_<i hex>.zip` holding files `i_j` (03 §4.2, 02 §9.4) | Credentials handed over as structs | Missing | Real encrypt-then-sign. Same-ACA validity rule. Real file naming. `.info` returns nextDlTime | Oracle decrypts with h_ij and checks the same-ACA rule. Mutation: an ACA-signed response from a different ACA must be rejected |
| 22 | Download parameters: `download-minWait` 5 min, `maxReqs` 100, `maxAge` 1 h; `ra-maxPreloadTime` 2 weeks – 3 years; `ra-maxGenDelay` 1 week; RA ACK within 1 s, EE retry after 10 s, at most 10 per 60 min; Download-Req header with a 5 s clock tolerance (03 §2, §6.2) | `first_batch_delay` 60 s, `download_poll_interval` 60 s, `max_download_polls` 64, all uncited | Constants invented | Use the profile numbers. Preload horizon per fleet (THEA 3 years, NYC 1 week) | Card cites SMP Table 3. Scenario reproduces the NYC weekly top-off and the THEA 3-year preload |
| 23 | Transport: HTTPS, TLS 1.2/1.3, server authentication only, at most 5 min per session, no resumption; all checks and requests in one session (03 §2) | TCP handshake model in `backend.rs`; TLS cost not explicit | Session limits not modelled | Model the TLS handshake (ECDHE-ECDSA), the 5-minute cap and no resumption, with session cost on the access link | Byte and RTT count per session against RFC 8446/5246 handshake sizes. Unit test for the cap |
| 24 | RSU application certificates: 1 week + 1 h, not pre-generated, requested weekly, no butterfly, revoked by hash on series 3. OBU identification certificates: 1 month + 1 h, series 3 (03 §7) | RSUs and VRUs use the same stand-in pseudonym path (`rsu.rs`/`vru.rs` → `NodeSecurity::provision`) | Whole credential type missing | Application and identification certificate flows. RSU starvation when backhaul is down (Wyoming) | Scenario: an RSU without backhaul for 8 days stops signing SPaT/MAP validly |
| 25 | Misbehaviour report: SCMS Manager MBR v1.0, PSID 0x26 about 0x20 BSMs, types and subtypes per 03 §8.1; signed by a pseudonym certificate, encrypted to the MA whose `MaSsp.relevantPsids` covers 0x20, POSTed one per request through the RA; EE queue weighted, ×0.9 daily at 05:00 ET, highest weight first | `v2xw-threat` report types (TS 103 759, J3287 references) carried as structs | US format missing | `EeMaApplicationSpecificMisbehaviorReport` encoder, evidence `SignedBsmsWithCertificate`, weight queue | Oracle decodes against SM-MBR-ASN v1.1. Unit test of weight decay and eviction order |
| 26 | RA → MA report shuffle: 10,000 or 1 day (03 §8.2) | Same numbers (`report_shuffle_*`) | Fine | Keep; cite SCMS-765 correctly | — |
| 27 | MA investigation and revocation (Brecht §VI): LV → PLVs at the ACA; one LA answers "same device?"; then ACA → RA (request hash) → RA blocklist and LCI pair → LA1/LA2 release ls(i) → CRLG. Each provider runs its own MA and shares reports with the others (01 §1.4) | One case at a time (`MaState::case`); two-report `investigate` unused; one MA | Concurrency, multi-provider sharing and the investigation flow missing | A case table with concurrent cases, both the "link" and the "revoke" collaboration sets, a per-provider MA, and inter-provider report forwarding | Flow test stamps every stage per case. 100 concurrent cases finish. Privacy test: no single entity gets both identity and LV |
| 28 | MA decision in practice: CV-pilot MVP was an operator reviewing reports by hand; end-to-end detection → CRL only demonstrated in June 2026 (01 §1.4, §7) | `CorroboratedMa`, automatic | Default is more automated than deployed reality | MA modes: `manual-review` (operator delay distribution, a setting) and `automatic` (today's pipelines) | Card states the source of each mode. Scenario compares the two |
| 29 | CRL issuance: `SecuredCrl` (PSID 256, header fields ABSENT), `CrlContents` v1, series 1 linkage CRL (`iRev`, `indexWithinI`, `JMaxGroup`/`LAGroup`/`IMaxGroup`), hash CRLs for series 2/3/4/256, `CrlSsp` signer checks, `nextCrl`; publication within 1 business day and before `nextCrl` (02 §11, 03 §10) | Custom signed digest; daily cadence; publish-on-decision default | Format, series and SLA all missing | Real CRLs per provider and per series. Business-day calendar (US federal holidays). Full and delta CRLs | Oracle decodes and verifies. CRL size vs Brecht ≈40 B per entry (≈400 KB per 10,000). Mutation: an entry outside the signed bytes must fail on the device |
| 30 | CRL distribution: composite CRL (`CompositeCrlSpdu`, all series plus the home CTL) from the RA on every connection, `If-Modified-Since`; the EE fetches at least weekly; after 2 weeks overdue it distrusts everything and warns (03 §10). The RSU/satellite CRL broadcast is specified but not used | CRL store node, broadcast node, MBMS option | Default distribution not as deployed | Composite CRL through the RA as the default; broadcast and MBMS as options; overdue tolerance on the node | Scenario: a device offline for 15 days goes to "CRL overdue, distrust" |
| 31 | CTL lifecycle: an EAC vote and an elector ceremony; add ≤ 3 business days, removal ≤ 48 h; electors exercised at least yearly (01 §3.1) | Static CTL at start | No lifecycle | Governance events: add or remove a root or elector, a new sequence number, devices installing the new CTL by quorum | Unit test: a CTL signed by 2 of 5 is rejected and 3 of 5 accepted. Root removal makes that provider's devices untrusted after install |
| 32 | PG: GPF/GCCF fetched at least daily by RA, ACA and LA; LPF/LCCF served by the RA (03 §6.3) | Present in `governance.rs` with ad-hoc signing | Format | Real signed files (CAMP formats; mark the 1609.2.1 CCF as the US profile choice) | Oracle verifies signatures |
| 33 | LOP: not in 1609.2.1 or the profile; likely not deployed (01 §2.2, §7) | Always present on every EE path | Specified-but-not-deployed component modelled as real | Off by default; on in the `camp-reference` preset | Preset test: the default topology has no LOP node |

### 1.3 Vehicle-side verification and timing

| # | Real | Today | Gap | Identical requires | Validated by |
|---|---|---|---|---|---|
| 34 | The device signs with the implicit pseudonym certificate the ACA issued, using `d = e·b + r` | The node issues its own explicit certificate (`secure.rs` stand-in; engine joint 1 in `phase2.rs`) | G1 | The protocol hands the node its real certificates and private-key reconstruction values; the stand-in issuer is deleted | Device check at install: `d·G == e·P_U + Q_ACA` for every certificate. Test that no `Issuer` remains in `secure.rs` |
| 35 | Receiver validation per 1609.2 §5.1.2 and §5.2 (02 §12): chain to a CTL root, consistency (permissions, SSP, chain length, validity and region inside the issuer's), PSID in `appPermissions`, generation time inside validity, location inside region, revocation | Signature only (`verify_parsed`); `verify_plan` unused on the node | G2 | `sec/src/chain.rs` validator, called on the node with a CA certificate cache. Cost charged per operation actually performed | Conformance matrix: one test per rule, each with a case that must fail (wrong PSID, expired, out of region, chain length exceeded, issuer lacking `certIssuePermissions`, untrusted root, revoked ICA). Oracle-generated negative cases |
| 36 | The revocation check reads the certificate's own `linkageData` and `iCert`, matches `cracaId`/`crlSeries` against the CRL's `crlCraca`/`crlSeries`, and walks seeds forward `iCert − iRev` steps, j = 0…jmax−1 (02 §11.2) | Reads `RxFrame.claimed_linkage` | G3 | Remove `claimed_linkage` and `claimed_cert_period` from `RxFrame`; the node decodes them from the certificate bytes | Test: a CRL entry for the provisioned LV revokes the on-air certificate; a forged frame whose side channel claims revocation has no effect (the field no longer exists) |
| 37 | The node accepts a CRL only if its signature verifies under a CRL signer with a matching `CrlSsp` | Engine set membership (`phase2.rs` ≈3646) | G6 | Node-side `SecuredCrl` verification | Mutation: an RSU that alters one byte is rejected by the node, not by the engine |
| 38 | P2PCD (1609.2 §8): `p2pcdLearningRequest` / `inlineP2pcdRequest` HashedId3 in the header; a peer answers with `requestedCertificate` | Counted only (`stores.rs` `record_p2pcd_request`) | Exchange missing | Real header fields, response timer, rate limits, and learning of unknown CA certificates | Scenario: a new ACA certificate propagates through a fleet; count the frames and the time to learn it |
| 39 | Crypto timing: every primitive that runs costs time on the device and the backend (SHA-256, AES for CRL expansion and butterfly, ECIES, CCM, P-384) | SHA-256 and AES cost zero; ECQV uses the ECDSA verify cost as a proxy (`primitive.rs`) | Under-costed | Cost entries per hardware profile from measured benchmarks (cited, quantised), with a published HSM figure where one exists | Benchmarks recorded with machine id. Model card cites them. A test that no primitive executed in a run is unpriced |
| 40 | Certificate change: J2945/1 CERTCHG every 5 min unless < 2 km; NYC every 2 km or 5 min (03 §11) | Rotation policy in `stores.rs` | Check defaults | US default as J2945/1; NYC as a preset | Unit test of the policy |

### 1.4 Deployment, calendar and constants

| # | Real | Today | Gap | Identical requires | Validated by |
|---|---|---|---|---|---|
| 41 | 2 production providers on one CTL; 5 electors; each provider has RCA, ICA, ECA, RA, ACA (SAESOL ×2), LA1, LA2, MA, PG, CRLG (01 §2) | One of each, fixed ids | G7 | Topology from a deployment preset (§2) | Preset test counts components. CTL lists exactly the preset's roots |
| 42 | Cross-provider trust is purely CTL plus CCF: an OBU on provider A verifies an RSU on provider B (01 §9) | Not possible | Missing | Per-device trust store from the CTL; the CCF downloaded from the RA | Scenario: mixed fleet with 2 providers; cross-provider verification succeeds; removing B's root from the CTL makes B's devices untrusted |
| 43 | Time grid: epoch 2015-01-06 04:00 Eastern, 168 h periods, 1 h overlap, 20 certificates per week (03 §6.1) | Epoch at run start; five scenarios use `i_period_s: 60` | G8 | Real epoch and real start date by default. Compressed periods only behind an explicit `calendar: compressed` flag that every output labels | Test: a run starting 2026-10-16 provisions i = 614 (derived). Lint: a compressed calendar without the flag fails scenario validation |
| 44 | Long-horizon effects (weekly top-up, CRL every business day, re-enrolment over years) happen on real calendars | Only reachable through compression | Missing | Fast-forward mode: the engine skips quiet stretches of radio time, as an event-skipping run with no traffic simulation, while the backend runs on the real calendar | Equivalence test: backend state after fast-forward equals the same events run in full on a small fleet |
| 45 | Uncited constants (`params.rs`, `governance.rs`): elector count/quorum, root and authority lifetimes, reenrol lead, poll intervals, first-batch delay, backend link latency 10 ms / 1 Gbit/s, backend overhead 1 ms, servers 4, seed retry 30 s | Marked uncited or `todo-calibrate` | Several now have sources (01–03) | Replace with the cited value or keep as a flagged setting with the reason | `tests/model_card.rs` extended: every parameter has a source or a calibration plan |

### 1.5 UI

| # | Real need | Today | Gap | Requires | Validated by |
|---|---|---|---|---|---|
| 46 | See providers, offline and online components, CTL/CRL contents, flows per request, certificates per vehicle, cases and revocations | `BackendView.tsx`: one box per role in 7 tier columns, devices pooled as one `ee` box, counters and edges only. Inspector shows the pseudonym digest, pool count and CRL entry count | No per-provider view, no certificate decode, no case tracking, no flow timeline, no per-entity knowledge | §3 | Playwright tests on fixtures. Engine-schema tests (`publish::tests`) |

---

## 2. Deployment realism

### 2.1 What the engine models by default (`security.scms.deployment: us-production-2025`)

| Entity | Count | Online? | Location (default) | Source |
|---|---|---|---|---|
| SCMS Manager (EAC, Policy WG) | 1 | Governance actor; acts in business days | n/a (no network node) | 01 §1.1 |
| Electors | 5, quorum 3, P-384, certificates staggered 12/15/18/21/24 y | Offline (ceremony events only) | n/a | 01 §3.1 |
| Providers | 2: "provider-a" (ISS/OmniTrust pattern: `ica`, `eca`, `pca`, `ra`, `la1`, `la2`, `ma`, `crlg`, `pg`) and "provider-b" (SAESOL S2X pattern: `ica`, `eca`, `aca` ×2, `ra`, `ma`, `pg`) | — | — | 01 §2.1–2.2 [A] |
| Root CA per provider | 1; root certificate 70 y, key 20 y | Offline | n/a | 01 §3.2 |
| ICA per provider | 1 | Offline between certifications (UNCONFIRMED; modelled offline) | n/a | 01 §2.2 |
| ECA per provider | 1 | Online, reached only at enrolment | Provider cloud region | 01 §2.2 |
| RA per provider | 1 (setting: n) | Online; the only server EEs talk to routinely | Provider cloud region | 01 §2.3 |
| ACA per provider | A: 1, B: 2 | Online, behind the RA | Provider cloud region | 01 §2.2 [A] |
| LA1, LA2 per provider | 2 | Online, internal | Provider cloud region (separate operator UNCONFIRMED; modelled as separate hosts) | 01 §2.2 |
| MA per provider | 1 | Online; mode `manual-review` by default (see decision 7) | Provider cloud region | 01 §1.4, §7 |
| PG per provider | 1 | Online, publishes daily | Provider cloud region | 01 §2.2 |
| CRLG / CRL signer per provider | 1 | Online | Provider cloud region | 01 §2.2 |
| DCM | 1 per OEM/vendor | At the factory; bootstrap is a pre-run event | Factory | 01 §5 |
| OBUs | From the scenario; assigned to providers by `fleet.provider_share` (default 50/50, UNCONFIRMED market share) | — | Road | — |
| RSUs | From the scenario; application certificates weekly over backhaul | — | Road | 01 §4 |

Links (all TLS 1.2/1.3 between online components; application-layer encryption where a relay must not read):

| Link | Carries | Default |
|---|---|---|
| EE → RA | requests, downloads, reports, composite CRL, CCF | cellular Uu or RSU backhaul (existing `backend.rs` access models) + internet to the provider region |
| RA ↔ ACA, RA ↔ LA1/LA2, RA ↔ MA, MA ↔ ACA/LA/CRLG, PG → RA/ACA/LA | backend flows | intra-region: latency setting, default from the region model; bandwidth setting |
| LA ↔ ACA | shared PLV key (rotation `shared_key_update_interval`) | key event, no per-request traffic |
| MA(A) ↔ MA(B) | report sharing | inter-provider link |
| ECA ← DCM | enrolment | factory event |
| Offline roots and electors | none | ceremonies with business-day delays |

**Placement.** No source publishes data-centre locations (01 §2.3, 03 §13). The engine therefore models a named region per provider, `region: us-east` by default and settable. Latency comes from great-circle distance to the scenario's city at a configured fibre speed (2/3 c) plus a per-hop overhead. This default is labelled ASSUMPTION in the run manifest. A `flat` option keeps today's fixed latency.

### 2.2 Settings (all in `security.scms.deployment`)

- `preset`: one of
  - `us-production-2025` (default)
  - `us-test-ctl-2025`: 6 roots (Autocrypt, Chunghwa Telecom, ISS, Microsec, SAESOL, Eviden)
  - `camp-reference`: one SCMS with LOP, CRL store and broadcast, a central MA, no providers; today's topology, kept for comparison
  - `custom`
- `providers[]`: `name`, `components{role: count}`, `region`, `ma_mode`, `crl_publication` (`sla-1-business-day` | `cadence` | `on-decision`), `fleet_share`.
- `electors`, `quorum` (defaults 5 / 3). Changing them flags the run as non-US.
- `options`: `lop`, `crl_broadcast`, `mbms_crl`, `group_linkage`, `identification_certs`, `butterfly: original | unified | cubk`. Defaults: off, off, off, off, on for emergency vehicles, original.
- `calendar`: `real` (default) | `compressed` (labelled). `start_date` defaults to the scenario date. `fast_forward` is on for runs longer than a day.
- `fleet`: `certs_per_week` (20; taxis 60), `preload` (THEA 3 y | NYC 1 w | `ra-maxPreloadTime` setting), `change_policy` (J2945/1 | NYC).

### 2.3 Specified but not deployed: model it, default it off

| Element | Status in the US system | Engine default |
|---|---|---|
| LOP | Not in 1609.2.1 or the profile; likely absent | off (on in `camp-reference`) |
| CRL broadcast (RSU, satellite) | Not used; CRLs come from the RA | off |
| Distribution centre (DC–EE) | Not profiled | off |
| SAS / OAuth, ACPC, X.509 enrolment, ISO 21177 | Excluded by the profile | not modelled except ACPC as a later option |
| Unified and compact unified butterfly | Excluded by the profile; CUBK key formula UNCONFIRMED | option, marked UNCONFIRMED until the text is read |
| Single central MA | CAMP design; deployed reality is one MA per provider | off |
| CRL series 4 (enrolment) | Specified; "may not need to be used" (RA blocklist instead) | series exists; RA blocklist is the default action |
| Group linkage | Specified; no deployment evidence | off |
| Automatic MBD → CRL at scale | Demonstrated June 2026 only | `manual-review` default, `automatic` option |

---

## 3. The UI: a redesigned PKI view

### 3.1 What it must answer

1. Who exists. Providers, their components, offline versus online, and the CTL that binds them.
2. What is happening now. Flows in flight, queues, and which links are busy.
3. What a given vehicle holds. Enrolment certificate, provider, pool by i and j, the active certificate, top-up state, CRL and CTL status.
4. What happened to a given request or case. Every PDU in order, with its time, bytes, sender, receiver, and decoded contents.
5. Who knows what. A ground-truth view against each entity's own knowledge. For example, the RA knows the enrolment identity and j but not the certificate; the ACA knows the LV but not the identity.

### 3.2 Layout (wireframe)

```
┌ header (unchanged) ───────────────────────────────────────────────────────────────┐
├ PKI ▸ [Trust domain] [Flows] [Vehicles] [Cases] [Revocation lists]   view: ◉ ground truth ○ as RA ○ as ACA ○ as LA1 … ┤
│                                                                                    │
│ Trust domain                                                                       │
│ ┌ SCMS Manager ─ EAC ─┐   electors ▢▢▢▢▢ (offline, P-384, 3 of 5)   CTL seq 1 · 2 roots│
│ ├ provider-a ─────────────────────────────────────────────────────────────────────┤ │
│ │ [RCA]▒ → [ICA]▒ → [ECA] [RA ●q3] [PCA ●] [LA1] [LA2] [MA ●2 cases] [PG] [CRLG]  │ │
│ ├ provider-b ─────────────────────────────────────────────────────────────────────┤ │
│ │ [RCA]▒ → [ICA]▒ → [ECA] [RA ●] [ACA][ACA] [LA1] [LA2] [MA] [PG] [CRLG]          │ │
│ └ devices: OBU 412 (a 205 / b 207) · RSU 18 · starving 3 · revoked 1 · CRL overdue 0┘ │
│   ▒ = offline (hatched, lock icon); ● = traffic in last 2 s; lines = links by flow    │
│                                                                                    │
│ right drawer (on selection): component card — certificate (decoded), queue, ops,    │
│   counters, PDUs sent/received, links to metrics                                   │
└────────────────────────────────────────────────────────────────────────────────────┘
```

- **Flows tab.** A sequence diagram for one flow instance, chosen from a list filtered by type and vehicle. Lanes are EE, RA, LA1, LA2, ACA, MA and CRLG. Each arrow is labelled with the PDU name, for example `EeRa1609Dot2AuthenticatedCertRequestSpdu`, plus its bytes and time. The shuffle wait shows as a hatched gap. Clicking an arrow opens the PDU decoder, which shows the hex beside the ASN.1 tree. For encrypted parts it shows the ciphertext and, in ground-truth mode only, the plaintext.
- **Vehicles tab.** A table of vehicles: provider, enrolment status, valid pool, next top-up, last CRL, and CTL sequence. Selecting a vehicle shows an i × j grid: weeks across and 20 slots down, coloured by state (downloaded, active, expired, revoked, not yet available). Below the grid is the active certificate's decode and its chain-validation steps, each passed or failed: chain to the root on the CTL, ACA permissions, validity, region, PSID/SSP, CRL. The same component is used for any certificate on the air: clicking a received message in `MessagePanel` opens it.
- **Cases tab.** A board of misbehaviour cases. The columns are Reports in → Corroborated → Decision (manual or automatic) → PLV lookup → LA seeds → CRL published → Enforced (n of N vehicles installed). A card shows the reporters, the evidence type and subtype (SM-MBR §2.3), the time spent in each stage, and the provider of each party.
- **Revocation lists tab.** One row per provider × series (1, 2, 3, 4, 256), showing issueDate, nextCrl, size, entries, and the SLA clock. Expanding a linkage entry shows (la1Id, la2Id, iRev, iMax) and the certificates it matches in the run (ground truth only). The CTL history appears on the same tab.
- **Map overlays.** Vehicle colour by credential state: healthy, top-up due, starving, CRL overdue, revoked, or untrusted provider. RSUs show their SCMS-service range.

### 3.3 Engine side

- New snapshot topics, each keyed by stable ids (`provider/role/index`) and certificates by HashedId8:
  - `pki.state`: providers, components, queues, CTL, and CRL heads, each second.
  - `pki.flow`: one record per PDU with flow instance id, sender, receiver, PDU type, bytes and a hex pointer.
  - `pki.case`: stage transitions.
  - `node.credentials`: the i × j grid summary.
- New inspect queries:
  - `inspect.pdu {id}`: hex plus ASN.1 JSON from the real decoder.
  - `inspect.cert {hashedid8}`: decode plus the chain-validation trace.
  - `inspect.knowledge {entity}`: what that entity holds.
- `BackendView.tsx` becomes the Trust-domain tab. The CCMS (ETSI) system keeps a single-provider rendering of the same components.

---

## 4. Phased build plan

### 4.1 Rules for every wave

- **Agents.** At most 3 per wave, and each wave is merged before the next starts. A merge agent closes every wave and runs the full affected suites once.
- **Heavy commands.** All cargo, tests, simulations and pnpm run through `.claude/pm/heavy.sh`, one at a time machine-wide (no concurrent builds). Agents develop against `cargo check -p <crate>` and run targeted tests only at milestones.
- **Environment.** Worktrees use the shared `target/agents` env and the codegen-units isolation of the parallel-Rust memory note.
- **Mutation proof.** Every acceptance test must be proved able to fail. The agent injects the named fault, watches the test go red, reverts, and reports the faults it injected. Inject and revert with `tools/mutate/mutate.py`, which sets the reverted file's mtime to now. A revert that restores an older mtime (`shutil.copy` to a `.bak`, then `shutil.move` back) leaves cargo's faulted build fresh, so later "clean" runs still test the fault; this is what made the D2 equivalence test look intermittent. Each agent keeps its scripts in its own scratchpad subfolder: the scratchpad is shared, and a `c.sh` or `ct.sh` another agent rewrites runs cargo in that agent's worktree.
- **Vectors.** Values are transcribed with citations. Nothing is fetched at test time, and the corpus carries sha256 provenance.
- **The oracle.** It is written once (WP-A0) by an agent that does not read the Rust, and it is extended only by agents that do not touch the Rust being checked.
- **Estimates.** Given in agent-hours of focused work: S ≈ 3 h, M ≈ 6 h, L ≈ 10 h, XL ≈ 16 h. Wall time is longer because of the single build lock.

### 4.2 Work packages

#### Phase A: cryptographic core and formats

**WP-A0: vector corpus and independent oracle** (M, no dependencies)
- Scope:
  - `tests/vectors/` holds transcribed vectors:
    - IEEE 1609.2a D.6.1–D.6.4
    - CAMP `lv`, `glv`, `bfkeyexp`, `ecies` (with the real P1), `kdf`, `mac1`, `aesccm`, `implicit`
    - Selected NIST CAVP files: ECDSA P-256/P-384 SigVer, CCM DVPT, SHA, X9.63
    - RFC 4231
  - `tools/oracle/` is a Python package (`asn1tools` with the committed ASN.1, `cryptography`). It covers COER, HashedId, Time32, ECDSA, ECIES, CCM, ECQV-1609, butterfly and linkage, CRL matching and chain validation. It runs as a script that prints a short verdict.
- Files: `crates/v2xw-sec/tests/vectors/*`, `tools/oracle/*`, `docs/design/scms-real/vectors-provenance.md`.
- Acceptance: the oracle reproduces every vector in the corpus by itself (ECIES must fail with the printed P1), and runs in CI-free mode under `heavy.sh`.

**WP-A1: COER and the 1609.2.1 ASN.1 layer** (L, no dependencies)
- Scope:
  - Commit the 1609.2.1 forge modules for the edition the owner chooses (decision 1), with provenance.
  - Generate bindings, or write the profiled subset by hand if `rasn-compiler` fails on parameterised types.
  - Pin the canonicalisation rules.
- Files: `third_party/asn1/etsi/ieee1609.2.1/*`, `crates/v2xw-msg/build.rs`, `crates/v2xw-msg/src/sec_types.rs` (or a new `scms_types.rs`).
- Acceptance:
  - Every PDU type in 03 §3.2 used by the US profile round-trips.
  - For 200 fuzzed values per type, our bytes equal the oracle's bytes.
  - The 1609.2 empty-string HashedId examples pass.

**WP-A2: primitives** (M, no dependencies)
- Scope: in `v2xw-sec`:
  - ECIES with the P1 rules
  - AES-128-CCM
  - KDF2 and MAC1
  - ECDSA P-384 with SHA-384
  - Time32/Time64 in TAI, with the leap-second table in `v2xw-core::time`
- Files: `crates/v2xw-sec/src/{ecies,ccm,kdf,p384}.rs`, `crates/v2xw-core/src/time.rs`.
- Acceptance: these vectors pass, each with its injected-fault failure:
  - D.6.1 ×6, D.6.2 ×4 (real P1), D.6.3 ×7, D.6.4 ×4
  - CAVP P-384 SigVer and CCM DVPT samples
  - Time32 known instants (§1 row 4)
- New crates (`p384`, `ccm` or own, `hmac`) per decision 9.

**WP-A3: certificates and chain validation** (L, depends on A1 and A2)
- Scope:
  - Implicit (ECQV-1609) and explicit issuance and verification for every certificate type in 01 §4.
  - `chain.rs` implements the 02 §12 rules: permissions coverage, SSP rules, the chain-length inequalities, validity and region containment, the same-ACA response rule, and the CTL-root anchor.
  - Replace `verify_plan`'s one-level chain with this validator, keeping the cost-plan output.
- Files: `crates/v2xw-sec/src/{cert.rs, crypto.rs ecqv, chain.rs, envelope.rs}`.
- Acceptance:
  - CAMP `implicit.txt` passes with the SEC 4 hash switch.
  - Dual implementation against the oracle on 500 random chains.
  - One negative test per 02 §12 rule.
  - Mutation: SEC 4 truncation, or hashing the whole certificate in 1609 mode, fails against the oracle.

**WP-A4: butterfly and linkage, official** (M, depends on A0 and A3)
- Scope:
  - Retarget the tests from legacy vectors to CAMP and OpenSCMS vectors; keep the legacy tests as a regression.
  - Add the implicit butterfly variant (B as R_U), the cocoon encryption key J and h_ij, and the original-variant response decryption.
  - Add group linkage behind an option.
- Files: `crates/v2xw-sec/src/{butterfly,linkage}.rs`, `tests/official_vectors.rs`.
- Acceptance:
  - `bfkeyexp` (signing and encryption), the OpenSCMS encryption vector, `lv` (i = 0, 1) and `glv` all pass.
  - Mutations: swapped prefix, dropped pad, 32-bit la_id. Each fails.

**WP-A5: CRL, CTL and real-artefact cross-check** (L, depends on A1, A2 and A3)
- Scope:
  - `SecuredCrl` and `CrlContents` encode and decode for full and delta, hash and linkage CRLs (series 1/2/3/4/256), including `iPeriodInfo` if the edition is 2025.
  - `CrlSsp` checks.
  - `FullIeeeTbsCtl`, `CtlSignatureSpdu`, `MultiSignedCtl`, `CompositeCrl`, CCF.
  - A fixture corpus of the published SCMS Manager production and test CTL zips and the provider chain files, with sha256 provenance.
- Files: `crates/v2xw-sec/src/{crl.rs, ctl.rs}`, `crates/v2xw-sec/tests/real_artefacts.rs`, `third_party/scms-artefacts/`.
- Acceptance:
  - Real production and test CTLs decode; at least 3 elector P-384 signatures verify; `rootCaApprove` hashes equal our HashedId32 of the embedded roots.
  - Every published chain file verifies to its root, including implicit reconstruction where present. This is the real-world ECQV check.
  - Mutation: flip one byte of a real CTL and verification fails.

#### Phase B: protocols

**WP-B1: SCMS PDU layer and `run.rs` split** (L, depends on A1–A3)
- Scope:
  - First, a behaviour-free split of `run.rs` (3,306 lines) into `run/{enrol,provision,report,revoke,crl,gov}.rs`, so that later agents do not collide.
  - Then replace `ScmsMsg` payloads with real `ScmsPdu` bytes in the 1609.2.1 wrappers (Signed, Encrypted, SignedEncrypted, EncryptedSigned, SymmEncryptedSingleRecipient), under PSID 35.
  - Sizes come from the encoder, with invariant I-S3 kept.
- Files: `crates/v2xw-proto/src/scms/{msg.rs, run/*.rs, sizes.rs}`.
- Acceptance:
  - `tests/flows.rs` stage lists are unchanged.
  - Every PDU a flow test emits is decoded and verified by the oracle.
  - The size model still equals the encoder output.

**WP-B2: enrolment, registration and re-enrolment** (M, depends on B1)
- Scope:
  - DCM bootstrap in the secure environment: the canonical key, the implicit enrolment certificate (region 840/124/484, series 4, wildcard `certRequestPermissions`), installation of the elector, root, RA and MA certificates, and RA registration.
  - Successor enrolment 12–24 months before expiry with no overlap.
- Files: `scms/run/enrol.rs`, `scms/params.rs`.
- Acceptance: the oracle verifies enrolment PDUs; calendar tests on the lead time and on the absence of any overlap.

**WP-B3a: pseudonym provisioning and top-up** (XL, depends on B1 and A4)
- Scope:
  - The original butterfly `EeRaCertRequest`, the RA cocoon expansion, LA PLV generation, and `EncryptedIndividualPLV` under LA↔ACA keys.
  - One `RaAcaCertRequest` per certificate, the shuffle presets, ACA ECQV issuance with the real LV, `cracaId` and series 1.
  - `AcaEeCertResponsePrivateSpdu` (encrypt to J, then same-ACA sign), batch naming and `.info`.
  - The download parameters of 03 §6.2, preload policies, and the TLS session model (≤ 5 min, no resumption).
- Files: `scms/run/provision.rs`, `scms/params.rs`, `crates/v2xw-engine/src/backend.rs` (TLS).
- Acceptance:
  - The device derives every private key, and `d·G = e·P_U + Q_ACA` holds for all 20 × n certificates.
  - The oracle decrypts responses with h_ij.
  - A wrong-ACA signature is rejected (mutation).
  - The NYC weekly top-off and THEA 3-year preload scenarios produce the cited counts.

**WP-B3b: RSU application and OBU identification certificates** (M, depends on B1)
- Scope: weekly application certificates over RSU backhaul with no butterfly and series 3; identification certificates for emergency vehicles (1 month + 1 h); starvation behaviour.
- Files: `scms/run/provision.rs` (non-butterfly branch).
- Acceptance: the Wyoming scenario (backhaul down 8 days) shows starvation; certificates carry series 3 and no LinkageData.

**WP-B4: misbehaviour, investigation and revocation** (L, depends on B1 and A5)
- Scope:
  - The SM-MBR v1.0 encoder (PSID 0x26, types and evidence), signed by a pseudonym and encrypted to the MA chosen by `relevantPsids`.
  - The EE weight queue (×0.9 at 05:00 ET) and per-report POST through the RA.
  - A concurrent MA case table, with the two-report "link" flow wired in.
  - MA modes `manual-review` and `automatic`.
  - The Brecht §VI-D revocation chain, and the RA blocklist.
  - Per-provider CRLG issuing real `SecuredCrl` series 1 (linkage) and 3/2 (hash), on the business-day SLA calendar, with full and delta CRLs.
  - Inter-provider report sharing.
- Files: `scms/run/{report,revoke,crl}.rs`, `crates/v2xw-threat/src/{report.rs, ma*.rs}` (adapter only).
- Acceptance:
  - The oracle decodes MBRs and CRLs.
  - 100 concurrent cases complete.
  - SLA tests: decisions on a Friday evening and on a federal holiday publish on the next business day.
  - Privacy invariant tests are unchanged.

**WP-B5: governance operations** (M, depends on A5)
- Scope:
  - Replace `governance.rs` artefacts with real certificates and CTL.
  - 5 electors with staggered validity; ceremonies; EAC add and remove timings; CTL install by quorum on devices; PG GPF/GCCF and LPF/LCCF signed with real keys.
  - Lifetimes from 01.
- Files: `scms/governance.rs`.
- Acceptance: 2-of-5 rejected and 3-of-5 accepted; root removal propagates; no `AuthorityCert` remains.

#### Phase C: vehicle-side verification

**WP-C1: credential hand-off; stand-in issuer removed** (L, depends on B3a and B3b)
- Scope:
  - The node receives its certificates and reconstruction values from the protocol.
  - `NodeSecurity` signs with `d = e·b + r`.
  - RSUs and VRUs get their own credential types.
  - The engine joint 1 and `device_linkage` are deleted.
  - The air certificate becomes the 86 B implicit one (airtime changes; record the delta in BUILD-STATUS).
- Files: `crates/v2xw-node/src/{secure.rs, rsu.rs, vru.rs, runtime.rs}`, `crates/v2xw-engine/src/{phase2.rs, run.rs}`.
- Acceptance: grep test that no `Issuer` or `device_linkage` remains; real-mode and modelled-mode equivalence (I-S1) holds; BSM size with an attached certificate equals the encoder output.

**WP-C2: full receiver validation** (L, depends on C1, A3 and A5)
- Scope:
  - `chain.rs` on the node with a CA cache seeded from the CCF.
  - Validity, region, PSID and SSP checks.
  - Revocation from the certificate's own `linkageData`, with `cracaId`/`crlSeries` matching and hash CRLs.
  - Node-side `SecuredCrl` verification and the 2-week overdue rule.
  - `RxFrame.claimed_linkage` and `claimed_cert_period` removed; `phase2.rs` set-membership check removed.
- Files: `crates/v2xw-node/src/{runtime.rs, stores.rs, secure.rs}`, `crates/v2xw-engine/src/{run.rs, phase2.rs}`.
- Acceptance:
  - The conformance matrix of §1 row 35, one red case per rule.
  - Mutation: an altered CRL byte is rejected by the node.
  - The Phase 2 honest-revocation regression (`phase2-manhattan` with attackers removed revokes 0) still holds.

**WP-C3: P2PCD exchange** (M, depends on C2)
- Scope: real `p2pcdLearningRequest`/`inlineP2pcdRequest` and `requestedCertificate`, response timers and rate limits.
- Files: `crates/v2xw-node/src/{runtime.rs, stores.rs}`, `crates/v2xw-sec/src/envelope.rs`.
- Acceptance: the new-ACA-certificate propagation scenario; count of frames and time to learn.

**WP-C4: crypto timing** (M, depends on A2; can run in any wave after A2)
- Scope: benchmarks for SHA-256, AES-128, ECIES, CCM, ECQV reconstruction and P-384 on the reference profiles; cost tables with citations; a test that every executed primitive is priced.
- Files: `crates/v2xw-sec/src/primitive.rs`, `crates/v2xw-sec/benches/*`.
- Acceptance: no zero-cost primitive that runs; the model card cites a source per number.

#### Phase D: deployment topology

**WP-D1: providers and placement** (L, depends on B4, B5 and C2)
- Scope:
  - Replace `ScmsNodes` fixed ids with a deployment built from `security.scms.deployment`: presets, providers, component counts, regions and the latency model, offline components, and fleet assignment.
  - CTL and CCF-based cross-provider trust.
  - Map `camp-reference` to today's topology.
- Files: `scms/params.rs`, `crates/v2xw-engine/src/{phase2.rs, scenario*.rs}`, schema.
- Acceptance: preset component counts; a mixed-fleet cross-provider scenario; root removal makes one provider untrusted; `camp-reference` reproduces today's Phase 2 numbers within noise.

**WP-D2: real calendar and fast-forward** (M, depends on D1)
- Scope: the real epoch and start date; the `calendar: compressed` flag and lint; the fast-forward mode; scenario migrations (the five `i_period_s: 60` files gain the flag or move to real time).
- Files: `crates/v2xw-engine/src/*`, `scenarios/*.yaml`.
- Acceptance: i = 614 on 2026-10-16; fast-forward equivalence on a small fleet; the lint fails on an unflagged compressed calendar (mutation).

**WP-D3: constant and citation audit** (S, depends on D1)
- Scope: every row of §1.4 row 45; resolve the shuffle conflict per the decision; update the model cards.
- Acceptance: `tests/model_card.rs` passes with no uncited parameter lacking a plan.

#### Phase E: UI

**WP-E1: engine publication and inspection** (M, depends on D1)
- Scope: `pki.state`, `pki.flow`, `pki.case`, `node.credentials`; `inspect.pdu`, `inspect.cert`, `inspect.knowledge`; JSON fixtures committed for the UI agents.
- Files: `crates/v2xw-proto/src/view.rs`, `crates/v2xw-engine/src/{publish*, inspect*}`, `crates/v2xw-server`.
- Acceptance: schema tests (touch `build.rs` first); each query answers on a recorded run.

**WP-E2: Trust domain and Flows tabs** (L, depends on E1)
- Files: `ui/apps/studio/src/components/pki/*`, `lib/pki.ts`, retiring `BackendView.tsx`.
- Acceptance: Playwright on fixtures; offline styling; sequence diagram for a provisioning run; PDU decoder shows the oracle-matching tree.

**WP-E3: Vehicles, Cases and Revocation-list tabs, and map overlays** (L, depends on E1)
- Files: `components/pki/*`, `Inspector.tsx`, `ObuHud.tsx`, `lib/security.ts`.
- Acceptance: Playwright; the i × j grid matches `node.credentials`; the case board stages match `pki.case`; the knowledge toggle hides plaintext outside ground truth.

#### Phase F: end-to-end validation

**WP-F1: conformance run** (L, depends on all of B–D)
- Scope: one scenario on the real calendar with fast-forward, through these steps:
  1. factory enrolment
  2. provisioning and a week rollover
  3. RSU application certificate renewal
  4. a misbehaviour report, case, CRL, and enforcement on both providers' vehicles
  5. a CTL change
  - Every PDU is captured and checked by the oracle (decode, signature, decryption).
- Acceptance: zero oracle disagreements; a published list of the injected faults and their red results.

**WP-F2: real-world cross-checks report** (M, depends on F1)
- Scope:
  - Real CTL and chain verification (from A5) in the report.
  - Sizes against CAMP Table 3.
  - CAMP sizing rates scaled to the fleet (0.5 requests/s per 17 M vehicles a year; polls).
  - CRL size against Brecht.
  - Pilot behaviours: Wyoming starvation, NYC top-off, the THEA preload.
  - Everything UNCONFIRMED listed with how it was handled.
- Output: `docs/design/scms-real/04-validation-report.md`, plus a published artifact for the owner.

**WP-F3: scale and regression** (M, depends on D1 and C2)
- Scope: a fleet-scale run with modelled crypto and a small run with real crypto; I-S1 equivalence; wall-time budget; the Phase 2 honest-revocation regression on the new topology.
- Acceptance: equivalence holds; no regression beyond the documented airtime change from implicit certificates.

### 4.3 Waves

| Wave | Packages (≤ 3) | Why together | Estimate (agent-h) |
|---|---|---|---|
| 1 | [x] A0, [x] A1, [x] A2 (merged 2026-10-16) | Independent: tooling, the ASN.1 layer and primitives touch different crates | 22 |
| 2 | [x] A3, [x] A4, [x] A5 (merged 2026-10-16) | All need A1/A2. Different files: `cert`/`chain`, `butterfly`/`linkage`, `crl`/`ctl` | 26 |
| 3 | [x] B1, [x] B5, [x] C4 (merged 2026-10-17) | B1 must land first for B2–B4. B5 is `governance.rs` only. C4 is `primitive.rs` only, pulled forward because it is independent | 22 |
| 4 | [x] B2, [x] B3a, [x] B3b (merged 2026-10-17) | All on the split `run/*` modules from B1 | 28 |
| 5 | [x] B4, [x] C1 (merged 2026-10-17) | B4 needs A5 and B1; C1 needs B3a/b. Different crates | 20 |
| 6 | [x] C2, [x] C3 (merged 2026-10-17) | Node-side; C3 follows C2's validator inside the same wave only after C2's first merge point. Otherwise move C3 to wave 7 | 16 |
| 7 | [x] D1 (merged 2026-10-17), then [x] D2 and [x] D3 (merged 2026-10-17) | D1 starts first; D2 and D3 start when D1's schema is committed | 19 |
| 8 | [x] E1, [x] F3 (merged 2026-10-17) | E1 is publication; F3 is scale testing on the new topology | 12 |
| 9 | E2, E3 | Both UI, on E1's fixtures; separate component folders | 20 |
| 10 | F1, F2 | Final validation | 16 |
| 11 | [x] PKI, [x] FLOWS, [x] POLICY (merged 2026-10-21) | Fix wave for the config-testing deviations 1–7: PKI profiles and trust, backend flows, receiver and MA policy. Different modules | ≈ 24 |
| 12 | [x] ORACLE, [x] STRESS, [x] MATRIX (merged 2026-10-22) | Testing wave: independent record oracle, multi-week and PQ stress, the same-seed configuration matrix. New files, two small engine fixes | ≈ 24 |
| 13 | [x] TRUST, [x] REPORTS, [x] DETECT (merged 2026-10-22) | Fix wave for the testing wave's open defects: trust and CRLs, report filing and PQ frames, the US detection profile's inputs. Different modules | ≈ 18 |
| | | **Total** | **≈ 201 agent-hours** over 10 waves, plus one merge agent per wave (≈ 2 h each) |

Wave 1 status (2026-10-16, merged into `main`; details in BUILD-STATUS.md):
- [x] **A0** (merge `5d5c0cf`): the Python oracle is in `tools/oracle`, with a vector corpus of 18 transcribed files plus 9 oracle-emitted files in `crates/v2xw-sec/tests/vectors/`. `selftest` passes (412/412), `mutate` turns all 24 faults red, and `verify-emitted` passes (10/10). Still open: no public 1609.2 implicit-certificate vector exists, the oracle does not compile the 2025 edition, and the real CTL files are not vendored (WP-A5 adds them).
- [x] **A1** (merge `ae74afa`): the 1609.2.1-2022 bindings build through rasn-compiler, so the fallback was not needed. Adds patch 0005 (pduFunctionalType constraint), `scms_types` (WireEdition, canonicalisation) and the asn1tools COER cross-check (8,757/9,000 values re-encode byte for byte; NESTED exempt). Still open: the rasn nested-extension-addition bug and the Psid ≥ 2^63 bug, and checking this layer against the A0 oracle.
- [x] **A2** (merge `13e2687`): Time32/64 now count TAI, using the IANA leap-second table, and the sec crate adds KDF2, MAC1, CCM, ECIES and P-384. All 21 D.6 vectors pass, plus the CAVP samples. Still open: `civil_at` ignores a leap second during a run, and the A0 oracle has not cross-checked these primitives.

Wave 2 status (2026-10-16, merged into `main`; details in BUILD-STATUS.md):
- [x] **A3** (merge `c789875`): ECQV-1609 implicit and explicit issuance and verification (P-256/P-384), whole-cert hashing and HashedId3/8/10/32, the `chain` module covering every 02 §12 rule, and a `verify_plan` that walks the whole chain (new rejects PsidNotPermitted, ChainInconsistent, ChainTooLong). Matches the oracle on 500 chains bit for bit; 32 injected faults go red. Still open: no public 1609.2-profile ECQV vector, the oracle cannot resolve implicit issuers, geometric regions pass only when identical to the issuer's, and per-type certificate profiles (gap row 13).
- [x] **A4** (merge `9d158d6`): butterfly B/J/h_ij helpers and the implicit variant, the encrypted ACA response (rekRecipInfo), and group linkage (off by default), against CAMP, OpenSCMS and the oracle; 7 faults go red. Still open: the unified/CUBK response key is unconfirmed (both refused), the rekRecipInfo reading and the group jValue/ei relation are unconfirmed, and the group_linkage scenario option is not wired (B phase).
- [x] **A5** (merge `dfd7a61`): real 1609.2.1 CTLs (quorum 3 of 5 P-384 electors), CCF, composite CRL and 1609.2 CRLs (series 1 linkage, 2/3/4/256 hash, full and delta). All 6 published CTLs and both chain files verify; 26 faults go red. Still open: the owner must decide whether the SCMS Manager files ("All Rights Reserved") may be vendored, so the 7 real-artefact tests stay `#[ignore]`; not yet wired into node or engine (B4/C2).

Wave 3 status (2026-10-17, merged into `main`; details in BUILD-STATUS.md):
- [x] **B1** (merge `77b6511`): `scms/run.rs` split into `run/{mod,enrol,provision,report,revoke,crl,gov}.rs` with no behaviour change, and a new `scms/pdu.rs` with the 1609.2.1 wrappers, a deterministic provider PKI and ECQV enrolment certificates. Enrolment and provisioning now exchange real EeEcaCertRequest, EcaEeCertResponse, EeRaCertRequest and RaEeCertAck SPDUs, each hop charged its encoded length (on by default, off under hybrid/KEM). `check_pdus.py` passes 6/6 SPDUs and 9/9 certificates. Still open: the download request, `.info`, RaAcaCertRequest, the ACA private response (B3a), the full enrolment profile and successor enrolment (B2), MBR (B4); the device-side requestHash/signer checks have no fault test; the check rules need independent re-derivation; PKI component profiles are stand-ins (row 13).
- [x] **B5** (merge `f36a066`): governance rebuilt on `ctl`/`chain`: five P-384 electors, genesis CTL signed 5/5 with 3-of-5 device quorum, P-256 root, ICA and 11 component certificates copied from the published US chain files, GCCF/LCCF and signed GPF/LPF, full chain rules on install, and the EAC operations with SLA publication. The oracle's `real` check passes 120/120. Still open: chain::validate rejects the published US RA certificates (tolerated and counted, needs a decision), the GPF/LPF payload layout is our own, federal holidays are not modelled, and run.rs does not yet schedule EAC events.
- [x] **C4** (merge `4d5da74`): `primitive.rs` prices SHA-256/384 per compression, AES per block, CCM as 2⌈n/16⌉+2 AES calls, ECDH, ECIES, ECQV, butterfly and linkage by recipe, and P-384, with `anchors::DEVICES` mapping all 12 hardware profiles. Still open: chain validation is not yet charged through `price_all` (C2), the AES key schedule and ECQV decompression are unpriced, and no vendor publishes CRATON2/SXF1800 SHA/AES/ECIES figures.

Wave 4 status (2026-10-17, merged into `main`; details in BUILD-STATUS.md):
- [x] **B2** (merge `d7aff97`): `scms/enrolment.rs` builds the US enrolment profile (id.name, region 840/124/484, crlSeries 4, wildcard certRequestPermissions) with canonical-key bootstrap and the ECA's SMP §6.3.1 checks. The DCM relays the enrolment response and registers the certificate with the RA. Successor enrolment runs device → RA → ECA → RA → device `<hash>.zip`, with no overlap, 6-year lifetime and an 18-month lead. Still open: the canonicalId text form and successor file name are UNCONFIRMED, the RA→ECA forward is modelled, refusal counters are not in the PKI view.
- [x] **B3a** (merge `82a79b0`): `scms/pdu_prov.rs` covers sealed PLVs (AES-CCM, LA-ACA key), one RaAcaCertRequest per certificate, implicit ECQV issuance with real LV and CRLG cracaId, the private response to J_ij, `<hash>_<i>.zip` batch files and signed download requests. Defaults: shuffle 1,000 / 2 days, poll 5 min. `check_pdus.py` passes 23/23. Still open: RA weekly pre-generation, `.info` flow, preload policies, the TLS session model, ACA-blindness type test.
- [x] **B3b** (merge `14577d2`): `scms/appcert.rs` and `run/appcert.rs` issue RSU application (169 h, no butterfly) and OBU identification (butterfly at (i,0)) certificates by ECQV with crlSeries 3, with ra-maxGenDelay, retry, poll caps and backhaul outages. `check_appcerts.py` passes 18/18. Still open: node/engine wiring (C1), series-3 revocation (B4), several labelled period and permission assumptions.

Wave 5 status (2026-10-17, merged into `main`; details in BUILD-STATUS.md):
- [x] **B4** (merge `ab09467`): `scms/mbr.rs` builds the US misbehaviour report (SM-MBR v1.0 / ASN v1.1). It is signed under PSID 0x26, encrypted to the MA and uploaded in an enrolment-signed Download-Req, and the MA checks the evidence. The device queue decays ×0.9 at 05:00 ET. `scms/calendar.rs` adds ET/DST, federal holidays and business-day SLA slots. Real SecuredCrls (full/delta linkage, series 3/4 hash) are signed by the CRLG (issuerIsCraca, so the CRACA is the ICA). The MA case table runs cases concurrently, in manual or automatic mode. `check_mbr.py` passes 4/4 and `check_run_crl.py` 19/19. Still open: per-provider MA (D1), device delta-CRL use, hash-CRL revocation flow, the report queue in run devices, and engine calls to `submit_mbr`.
- [x] **C1** (merge `50b824c`): nodes sign only with SCMS-issued implicit credentials, with the import check `d·G = e·P_U + Q_ACA`. The stand-in issuer and the linkage side channel are removed, and receivers revoke on the certificate's own linkageData. `CredentialFactory` covers RSUs, ETSI, PQ and no-SCMS runs. The certificate on the air is now 77 B. Still open: RSU certificates come from the factory (B3b not wired), ETSI explicit ATs, batch certificates permit PSID 0x20 only, and receiver validation above the ACA (C2).

Wave 6 status (2026-10-17, C2 merged into `main` with two follow-up packages; details in BUILD-STATUS.md):
- [x] **C2** (merge `f622d60`): `v2xw-node/src/rxval.rs` validates every SPDU on OBU, RSU and VRU nodes: chain to an installed root, validity, generationTime, region, PSID, hash revocation up the chain, and the 2-week overdue-CRL rule. Refusals are counted by rule. Vehicles verify the signed CRL themselves, and validation time is charged to the frame. The ACA gains certIssuePermissions. After the merge, an entry dated after the CRL's iRev counts as unsigned, which closes a forged-relay gap. Still open: RSU CRL install, identified-region checks, group linkage, the RA certRequestPermissions check, and an oracle check of node verdicts.
- [x] **RAPRE**, a follow-up to B3a (merge `a84fe9a`): one request per device (ra-maxReqs = 1), RA pre-generation over ra-maxPreloadTime (2 weeks to 3 years), the `.info` top-off, ACA blindness (per-certificate handles, LA-sealed chain IDs), the TLS session model and the THEA/NYC preload presets. After the merge, pre-generation is off under a hybrid scheme, because there is no post-quantum butterfly. Still open: per-fleet preload wiring in the engine, and calibrated TLS sizes.
- [x] **WIRE**, engine wiring for B4/B5/C1 (merge `ef01e01`): the real US weekly calendar from `time.t0` (i = 614 on 2026-10-16), the PDU PKI under the governance root, real SM-MBR reports with BSM evidence, the MA decision mode (`ma_review_s`), the MbrQueue report store, and scheduled root rollover and removal. After the merge, the genesis hierarchy is back-dated one year, so a pseudonym from a week that began before t0 still lies inside its ACA's validity. Still open: RSU application certificates from B3b, merging the two RaAcaCertRequest paths, real report bytes on the cellular leg, and the ICA after a root rollover.
- [x] **C3** (merge `bade343`, wave 7 integration): P2PCD per 1609.2a §8.2.4. HeaderInfo carries the learning/inline requests and `requestedCertificate`, the caCerts Peer2PeerPDU (PSID 136) matches asn1tools byte for byte, and `v2xw-node/src/p2pcd.rs` runs the out-of-band (US default) and inline (ETSI default) flavours. A certificate is learned only after its CA path validates, and activity is recorded on `node.p2pcd`. Still open: out-of-band responses over the engine radio and back-off wake-ups, RSU/VRU preprocessing, the 2016 base text, and the PSID 136 mapping.

Wave 7 status (2026-10-17, D1, its vehicle flows, D2 and the C3 air follow-up merged into `main`, then D3 with wave 8; details in BUILD-STATUS.md):
- [x] **D1** (merge `bbab5a5`): `scms/deployment.rs` adds the presets (us-production-2025, us-test-ctl-2025, camp-reference, custom), per-provider components, a great-circle latency model, fleet assignment and MA routing. Every provider's root is on one shared-elector CTL, each provider has its own ICA, components and PDU PKI, and the engine gains `security.scms.deployment` (the default). `oracle.py real` passes 113/113 on the two-provider CTL, GCCF and LCCF. Still open: vehicle flows on provider-b (one ScmsRun per provider), the fleet split beyond RSUs, per-provider MA and CRL options, and adding the city-to-region latency.
  - [x] **D1 vehicle flows** (merge `e6c0911`): one `ScmsRun` per provider (`ScmsRun::new_peer`, same CTL byte for byte, own PKI, nodes and seed); vehicles assigned by the seeded fleet split; per-provider MA and CRL Generator; MA-to-MA report sharing over the inter-provider link (format modelled: no published spec); every RA serves every provider's signed CRL after the link and vehicles verify each under its CRACA; the MA verifies foreign evidence under every provider's CAs; device legs add the city-to-region leg. `tests/scms_providers.rs` (Manhattan, 6 attackers): both providers' vehicles verify each other, 1,494 reports shared, 3 revocations per provider, all four receiver/signer pairs refuse revoked senders, no honest revocation. Still open: per-provider `crl_publication`/`ma_mode` settings (the run's single `ma_review_s` applies to all), RSU CRL broadcast and MBMS carry only the home CRL, the Backend view shows the home run only, a home root rollover does not re-certify the peers' copies of its components, extra ACA instances take no load.
  - Integration fixes (`43c675e`, `d50bdfd`): the roadside CRL broadcast now relays every provider's list and signed CRL (ASSUMPTION), and each node installs its own provider's chain file under `home-provider`. MBMS still carries only the home CRL.
- [x] **D2** (merge `b280dfd`): `time.calendar` (`real` by default, or `compressed`): a non-week `i_period_s` is refused unless the scenario says `compressed`, and compressed runs are labelled in the manifest and `RunReport.calendar`. On the real calendar the home provider's CRL rule reaches the CRL Generator (US presets: one business day). `time.fast_forward` windows jump the backend to its next due event (`Phase2::next_wake`, now over every provider's run). `tests/calendar.rs` passes 6/6, and each test was shown to fail under its fault. The equivalence test's "intermittent" failure (2 of 5 runs, a top-up 100 ms late) came from the D2 agent's own fault-injection revert, not the engine: the revert restored `run.rs` with an older mtime, so cargo kept the build with the "fast-forward stops one step late" fault until an unrelated edit to `run.rs`. WP-DET reproduced this and found no non-determinism in the engine. `tests/calendar.rs::the_lifecycle_run_is_byte_identical_at_one_and_four_threads` now checks record streams at 1 and 4 threads and on a repeat. Still open: revocation is not in the equivalence test, `credential-weeks.yaml` has never been run, and `phase2-manhattan`/`revocation-latency` compress cadences that the lint does not catch.
- [x] **AIR**, a C3 follow-up (merge `90b7101`): P2PCD learning responses are radio frames (`MsgType::P2pcd`, PSID 136) sent at their exact back-off instant, VRU and RSU preprocess received SPDUs, `security.scms.chain_file: home-provider` exists, and one `RaAcaCertRequest` builder and one ACA issuance serve every certificate (pseudonym requests carry B_ij). `p2pcd_air` passes 3/3. Still open: RSU application certificates from the B3b weekly cycle (item 2: never called), RSU CRL install, a dedicated starvation counter, and an independent re-derivation of the `check_pdus.py` rule the agent edited.
- [x] **D3** (merge `cf06907`, wave 8 integration): `docs/design/scms-real/constants.md` lists every SCMS default as CITED, DERIVED, MEASURED or ASSUMPTION with its code location and source, checked against the code by proto `model_card` and engine `scms_constants`. Enrolment lifetime is cited (CAMP 1.2.2), the report size comes from the real encoder (524 B report, 1,119 B upload), TLS flights are measured (`tools/scms_constants/tls_handshake_sizes.py`) and charged by `scms_tcp_config`, and the 5 s CRL broadcast is bounded by RSU v4.1. Still open: relay range, backhaul, MA review and four wire sizes stay ASSUMPTION; the library's RSU certificate cycle backhaul differs from the engine's; the TLS 1.3 client Finished is not charged.

Wave 8 status (2026-10-17, merged into `main`; details in BUILD-STATUS.md):
- [x] **E1** (merge `5f8cb0a`): `v2xw-proto/src/scms/pki_view.rs` builds `pki.state`, `pki.flow` (real SPDU hex, plaintext opened with the recipient's key, linked to its wire step), `pki.case`, `node.credentials` and `pki.knowledge` (ground truth in its own field). The engine publishes all five (`security.scms.pki_view: false` turns them off), and the server answers `inspect.pdu`, `inspect.cert` and `inspect.knowledge` (NODE-only sessions refused ground truth). UI fixtures are in `ui/apps/studio/test/fixtures/pki`; `tools/pki_view/check_jer.py` agrees with asn1tools on 102/102 layers. Still open: pre-drive provisioning SPDUs are flow `preload` off the run clock; devices cannot open `AcaEeCertResponsePrivate`; knowledge rows capped at 64; the 2025 edition is not distinguished.
- [x] **F3** (merge `bf6c218`): `tests/scms_scale.rs` checks I-S1 (real vs modelled crypto give the same records and frames on the two-provider path), pinned regression digests in `tests/regression/` (re-blessed after the D3/E1 merges, frame digests unchanged), camp-reference vs us-production-2025 honest revocation, a `long_` fleet-scale test, and `05-scale-and-regression.md`. Dependencies build at opt-level 2 and ten hot crates at opt-level 1 in test (phase2 about 25 min to 6.5 min). Still open: release-build fleet costs, cross-platform digest equality, and the 600 s 24,000 veh/h horizon.

Follow-up packages (2026-10-18, merged into `main`; details in BUILD-STATUS.md):
- [x] **DET**, a D2 follow-up (merge `5097db3`): the D2 "intermittent" failure was a stale faulted build: an mtime-losing revert left cargo running the faulted binary. No engine non-determinism was found. `tools/mutate/mutate.py` injects a fault and restores the file with a fresh mtime (§4.1). `calendar` checks byte-identical record streams at 1 and 4 threads. Trip files are named by content and written atomically. The proto `threshold` expectation was corrected. Still open: the `pki_view` change digest uses std `DefaultHasher`.
- [x] **VCOST**, a C2 follow-up (merge `72678b1`): `rxval::EeCache`, an LRU of 4,096 entries keyed by HashedId8. A fresh CRL invalidates matching certificates and CA paths by cracaId/crlSeries, and peer-cache eviction forgets validated state. Repeat frames cost 400 µs; a first sight adds the priced SHA-256 and ECQV work. This fixes the three `foundations` failures. Still open: capacity is uncalibrated, there is no time-based expiry, and invalidation drops the whole entry (an upper bound).
- [x] **RSUC**, the C1/AIR item 2 follow-up (merge `f8e51a8`, fix `ef23a0b`): RSUs run the B3b weekly application-certificate cycle (preload, backhaul renewal, starvation counters), fetch and install their RA's composite CRL, and refuse revoked senders (`rsu_revoked_receptions`). It adds per-provider `ma_mode`/`ma_review_s`/`crl_publication`, `<provider>/<role>` Backend entities, every provider's CRL on MBMS, and peer copies on root rollover. The library backhaul default now matches the engine's. Still open: RSU CRL install has no transit time, extra ACA instances take no load, peer settings are absent from the library presets, CRL/P2PCD frames are signed under a PSID 0x82-only certificate, and the RSU fields have no UI.

Phase E UI status (2026-10-18, E2 and E3 merged into `main`; details in BUILD-STATUS.md):
- [x] **E2** (merge `8fc115a`): the Studio's Backend view is now the PKI view (`components/pki/`, `lib/pki.ts`), with a view switch for ground truth or "as RA/ACA/LA1/LA2/MA/ECA". The Trust domain and Flows tabs come from three new server queries: `inspect.trust`, `inspect.flows` and `inspect.flow`. Flow answers carry no bytes or plaintext. The PDU decoder's trees match `check_jer.py`. CCMS runs keep the old diagram. Still open: `inspect.trust` is asked without `t_ns`, so after a backward seek the tab can show a later snapshot.
- [x] **E3** (merge `c464bdd`, digests re-blessed in `c14ec7d`): the Vehicles (i × j grid, pseudonym history), Cases (board, steps, who learned what) and Revocation lists (decoded entries, SLA clock, CTL) tabs, from `inspect.pki`. There are also map rings by provider, credential state or case parties, and links from the Inspector and HUD. Proto adds `crl_entries`/`crl_matches`, and the MA's `pki.knowledge` adds `truth.cases`. Still open: `enforced` is stamped once, evidence type is not recorded, RSU reporters show no provider, and the "who learned what" text is a fixed table.

Phase F status (2026-10-18, F1 and F2 merged into `main` with the cleanup package; details in BUILD-STATUS.md):
- [x] **F1** (merge `2907e26`): `scenarios/scms-conformance.yaml` runs on the real calendar with two fast-forward windows through factory enrolment, provisioning and the 614→615 rollover, RSU renewal, SM-MBR → case → CRL → refusals on both providers' vehicles, and a root-rollover CTL. `tests/conformance.rs` captures every PDU for `tools/conformance/check_run.py`. On the merged main that passes 19,183/19,183 checks with 0 disagreements, and 18 capture faults and the engine faults all went red (`06-conformance-run.md`). The run found and fixed two engine defects: the last PKI-view interval was not recorded, and the PLV was not sealed as Ieee1609Dot2Data-Unsecured. Still open: two real-artefact chain-length deviations (owner/SCMS Manager decision), the crlSeries 4-vs-0 check, and the CRL-broadcast and P2PCD frames, which are not tapped.
- [x] **F2** (merge `832d81a`): `04-validation-report.md` plus an owner artifact. Certificate sizes match CAMP Table 3 and the published chains (pseudonym 80 B, 86 B with a region), a linkage CRL costs 33.08 B per entry (Brecht ≈ 40 B), and CAMP's sizing rates are within 1 %. Pinned by `camp_sizes` and `crl_size_brecht`. Still open: the five owner findings in report §9 (RA permission forms, US pseudonym region, no SSPs, the duplicated preload constant, and `#[ignore]` real-artefact tests).
- [x] **CLEAN**, an E/C follow-up (merge `dc6ec05`): method counts come from the real lists, the `feed` crypto test is fixed, and so is the chase-camera bug. The `pki_view` digest is a truncated SHA-256. `enforced` is stamped per vehicle, and each case's report evidence is recorded. Still open: the `sensitivity` DefaultHasher, CCA preamble detection, and a flaky render-perf allocation test.

Validation status (2026-10-18, five validators merged into `main`; checklists in `validation/`, details in BUILD-STATUS.md):
- [x] **VAL-CRYPTO** (merge `0f18380f`): 80 requirements (61 match, 7 fixed, 6 deviations, 6 unverifiable). Received certificates are hashed in canonical form, generationTime < expiryTime is enforced, the device-facing MA is root-issued on series 256, components take their published lifetimes, and new `profiles.py` checks 24/24. Owner: CF-CRACA, CF-IPERIOD.
- [x] **VAL-PROTO** (merge `843f73f0`): 109 items (90/5/7/7). Stale CRLs are refused (R2) at the device and the node, the RA returns every chain of a blocklisted enrolment (B4), indexWithinI counts each delta, and SCMS-509 applies at job time. Open: EE retry, infoStatus/412, Range resume.
- [x] **VAL-GOV** (merge `aee0f110`): 70 rows (45/8/11/6). CTL business days skip federal holidays, the elector deadline is 5 days, a replacement elector is valid 15 y, a removed root is no longer listed, and the test CTL approves 5 of 6 roots. F2 §9 is resolved.
- [x] **VAL-VEH** (merge `3c1f5736`): 67 requirements (41/7/8/11). Each device has its own PSID profile (handsets 0x26/0x27), the MA sets aside signature-only evidence, detectors skip relayed frames, a pseudonym change resets PSM msgCnt, and the `j2945-1`/`nyc-pilot` strategies are added. MBR airtime in the first 60 s fell from 29.3 % to 0 %.
- [x] **VAL-UI** (merge `0c8a5e34`): one "Certificate system" workspace (Overview diagram, health strip, Vehicles timeline, Misbehaviour steps, Revocation), and `inspect.trust` lists the window's messages. Open: the enforced n/N count, and messages missed at high speed.

Fix wave status (2026-10-21, merged into `main`; details in BUILD-STATUS.md and 07-config-testing-report.md §7–§8):
- [x] **PKI** (merge `a8c411da`): `governance::CertProfile` with the ISS form (70 y root, root-issued CRL signer, root as CRACA) and the SAESOL form (ICA-issued CRL signer); us-production-2025 provider-a is ISS. Devices check a new CTL's 3-of-5 quorum, a composite CRL skips only an unverifiable CRL, uncompressed keys are accepted after an on-curve check, enrolment certificates carry the CRACA. Open: Studio PKI fixtures, an oracle check of the ISS profile.
- [x] **FLOWS** (merge `8250023b`): backend messages to a down component are held and retried every 10 s, and its timers run on restart; RSU-filed unsigned reports carry their own label; CRL polls are conditional (412 when current) and interrupted downloads resume with Range/206. Single-channel MAC documented. Open: per-channel MAC queues and WSA bytes, the infoStatus body.
- [x] **POLICY** (merge `e2c77632`): US SM-MBR judges only verified BSMs; verify-on-demand with detection warns in validate and settings; a 1609.2 generationTime window on every receiver and an MA evidence-time rule; the dead-reckoning clock mix fixed (clock-skew edge: 19 honest revoked → 0). Open: the US default revokes 26 honest vehicles in the realism base (R-USFP, raw-GNSS drift).

Testing wave status (2026-10-22, merged into `main`; details in BUILD-STATUS.md and `testing/oracle-final.md`, `testing/stress.md`, `testing/matrix.md`):
- [x] **ORACLE** (merge `2dab66bf`): `oracle.py records` (30 rules over pki.flow/pki.state/credential records: CRL freshness, downloads, report signing, cracaId, CTL order, RA blacklist) and `oracle.py ctl-files`. Base 65,980/69,051 and two-provider variant 206,798/212,189 checks pass; every red check is OF-1 (a second, unissued enrolment certificate signs later requests) or OF-2 (1,695 of 2,991 cellular report uploads carry no SPDU). Both reproduced in `tests/oracle_final.rs` (#[ignore] until the engine is fixed). Expectations moved to `tests/expectations/scms/`.
- [x] **STRESS** (merge `02926e03`): F-OFF fixed (a switched-off device reaches no SCMS). Multi-week (33 d) runs prove the 2-week overdue-CRL rule in-run; PQ at quarter-fleet density (ML-DSA-44 ×12.6 airtime). Open: R-NOCRL (no CRL before a provider's first revocation), R-PQMBR, the full-scale benchmark.
- [x] **MATRIX** (merge `0301425a`): 32 same-seed runs, 98 rows (82 pass, 10 open). Fixed the `credential_is_usable` cost (2,074 s → 164 s, same output) and the AoI fast-forward silence (1,693,079 ms → 81.6 ms; phase2-path digest re-blessed). Open: R-USFP is not GNSS noise, R-TRUSTLEAK (280 off-trust-list messages verify), LTE-V2X BSM p95 112 ms.

Fix wave 2 status (2026-10-22, merged into `main`; details in BUILD-STATUS.md and `testing/usfp.md`, `testing/pq-profile.md`):
- [x] **TRUST** (merge `1700c6d2`): R-TRUSTLEAK fixed: sized-only frames (RSU CRL broadcasts, relayed reports) carry the signer certificate and receivers validate it to the CTL roots (realism off-ctl: 280 off-trust-list messages verified, now 0). R-NOCRL: every CRACA signs a CRL at t0 and devices join holding them. OF-1: one aged enrolment certificate from the ECA exchange. O-1: full CRLs only (deltas off). Integration fix `3471f2fc`: a mid-run joiner's pre-run exchange runs at its certificate's start (`preload_at`). Open: no fault test on the OBU/VRU fallback; RSU senders missing from verified_by_provider.
- [x] **REPORTS** (merge `6e0e7e14`): OF-2: a detection with no SM-MBR upload is not filed (`reports_not_filed`; `modelled_reports: 1` for research). R-PQMBR: over-MTU relay reports and CRL frames are refused and counted, and fragmented under generic-sdu. PQ hybrid top-ups complete after the request shuffle; `testing/pq-profile.md` lists the PQ gaps. Studio fixtures re-recorded. Open: R-UNSEC (an RSU's own reports are sized-only); RSUs learn no CRL under PQ.
- [x] **DETECT** (merge `fb2a9ed2`; was R-USFP): fixed at its cause, the raw 10 Hz GNSS fixes' fresh white noise (2.65 m median consecutive-BSM residual against SM-MBR's 1 m). `gnss/error/gauss-markov` 1.1.0 smoothing receiver (τ 100 s, static hold 0.3 m/s) in every US preset. Realism base, same seed: honest revoked 26 → 0, attackers 1 → 2 of 3 (the third a handset: SM-MBR covers PSID 0x20 only), the constant 27 decisions and the 10 old-evidence reports explained and gone (`testing/usfp.md`). Open: the 43,125 s-old evidence path is not root-caused; the τ and hold constants are assumptions.

Final test wave status (2026-10-22, merged into `main`; details in BUILD-STATUS.md, `testing/oracle-final.md` §8 and `testing/final-bench.md`):
- [x] **ORACLE-FINAL** (merge `e6033e12`): HEAD `88e24b09` passes every rule on conf (67,451/67,451) and mbr (211,785/211,785); OF-1, OF-2, O-1 confirmed fixed; two oracle fixes so the ctl-files and CRL faults can go red. Open: OF-3 (no record of a device holding a t0 CRL), O-3 (RSU backhaul reports carry no bytes), O-2.
- [x] **BENCH** (merge `887520b6`): full-scale run stopped by the 2.5 GB cap at ~155 s (windows reported); hybrid ML-DSA-44 airtime ×13.0; determinism byte-identical at 4 vs 1 threads. Open: R-CCSIG (cert cycle keeps the hybrid signature on every message), memory ~1.4–2 MB per node, the privacy observer's 2,873 link claims with 0 pseudonym changes.

Fix wave 3 status (2026-10-23, merged into `main`; details in BUILD-STATUS.md, `testing/ccsig-memory.md`, `testing/measure.md` and `testing/rerun-2026-10-22.md`):
- [x] **PQMEM** (merge `0a125a98`): R-CCSIG fixed (Partially-Hybrid cycle messages are ECDSA, PQ check once per learned certificate; reverts `45f0942f`), RSUs fetch a CRL under PQ, per-node credential de-duplication. Its tests and 5 faults were run on main, all faults red. Open: the full-scale benchmark still exceeds 2.5 GB (allocator retention), PQHFC/PQHMC not re-run, VRU handsets price ECDSA.
- [x] **DETPRIV** (merge `f812677e`): R-FS40 (GNSS mean-speed propagation), privacy link counting, OF-3 (CRLs held and pre-run downloads recorded), R-UNSEC (an RSU's own reports are not filed), old evidence re-checked when judged. Open: `scms_wiring` MA-case test and `pq_relay` relayed-report test fail (owner decision), full-density R-FS40 and gnss-raw re-runs owed.
- [x] **RERUNS** (merge `4a62ed5b`): the 6 failing tests re-stated on measured causes, M-RX1 fragment fix, 34 matrix/realism runs, `component_certs.py` 10/10 roles. Open: R-JOINCRL, MWP.off-no-backend, node.rx refusal label, enrolment name label, L5, LTE BSM p95.

Final runs part 2 status (2026-10-23, merged into `main`; details in BUILD-STATUS.md and `testing/final-run.md`):
- [x] **BENCH** (merge `430f40ee`): 27 expectation rows written before the runs (16 pass, 2 fixed, 2 corrected, 7 open). R-CCSIG confirmed at density: Falcon cycle BSMs 380 B (was 1,045 B), ML-DSA-44 cycle transmits (814 B), RSUs install the CRL under PQ. Full-scale benchmark: both live windows completed, peak 3,109 MB, stopped by hand in the second fast-forward (47,340 of 172,800 s), so no end-of-run report. Open: slow second fast-forward, R-FFHOLD, R-FFBURST, full-scale memory, rollover pseudonym count, `ped_wait`, R-PQUNSEC (owner decision), 47 Invalid states.

Finalize 4 status (2026-10-23, merged into `main`; details in BUILD-STATUS.md, `testing/fast-forward.md` and `testing/memory-density.md`):
- [x] **TESTS** (merge `2f6c664d`): `scms_wiring` runs its scenario's 300 s (105 verified MA reports, revoke at 186.8 s); `pq_relay` gets a relay-only attacker so the relay path runs (3 revocations); PKI Studio fixtures re-recorded. Both tests pass. Open: 5 mobility defects classified, `reports_uploaded_relay` counts on-air reports.
- [x] **FF** (merge `66d65542`): lost edits replayed; R-FFBURST fixed (gridlock after the gap, first GNSS fix with a 43,000 s dt); `ped_wait` and pseudonym rate skip the frozen window; `stops_by_cause` added. R-FFHOLD is not a defect. Open: R-FFSLOW, FB2 re-run.
- [x] **MEM** (merge `58e0b270`, includes `wip/finalize4-mem`): peak 2,074 MB to 1,027 MB at full fleet, 100 s; shared governance certificates; apex turn rule in the benchmark (positionJump 123 to 35). Open: the 2.5 GB target at the resume peak is projected, not measured; per-device backend state needs owner decisions.

Critical path: A1 → A3 → A5 → B4 → C2 → D1 → E1 → E2/E3 → F1. Two things could stretch it:
- `rasn-compiler` cannot handle the 1609.2.1 modules (WP-A1 fallback adds about 8 h).
- The published artefacts reveal format details the research did not see (WP-A5 may add a fix-up package).

---

## 5. Owner decisions

| # | Decision | Options | Recommendation |
|---|---|---|---|
| 1 | Wire edition | 1609.2-2022/1609.2.1-2022, which SCMS Manager profile v1.0.3 profiles and which is deployed; or 2025/2026, the newest, already used for 1609.2 here | Default 2022 for the US preset, 2025 as a setting. The ASN.1 differences are small (02 §1) |
| 2 | Buy IEEE 1609.2-2025 and 1609.2.1-2026 | Buy, or work without them | Buy. They settle the unified/CUBK formula, Annex D vectors (possibly an ECQV-1609 vector), the v3 URL paths, the CCM nonce rule and the parameter semantics. Without them those items stay UNCONFIRMED and are validated only by dual implementation and real artefacts |
| 3 | Provider names in presets | Real names and real hostnames, as on the CTL; or neutral `provider-a`/`provider-b` with a documented mapping | Neutral ids in the engine, with the real-world mapping in the preset's documentation. Our simulated certificates should not carry real organisations' hostnames as if they issued them |
| 4 | CAMP vector repository (no licence file) | Transcribe the values with citations; fetch at test time; or skip | Transcribe the specific values with citations (published test values), and keep a provenance note |
| 5 | Request-shuffle default | CAMP 1.2.2 (1,000 plus a 2-day age), the newer pilot document; or Brecht (10,000 per day) | CAMP 1.2.2 by default, with Brecht as a preset |
| 6 | Placement | Assumed region per provider (e.g. `us-east`) with a distance latency model; or flat latency | Assumed region, labelled ASSUMPTION; change it if a provider publishes locations |
| 7 | Default MA mode | `manual-review`, the deployed pilot reality with an operator-delay setting; or `automatic` | `manual-review` for the US preset; keep today's automatic pipeline as an option and as the research default for detection studies |
| 8 | Calendar | Real weeks plus fast-forward; or keep compressed periods | Real by default; compressed only with the visible flag |
| 9 | Dependencies and tooling | `p384`, `hmac`, possibly `ccm`; Python `asn1tools` and `cryptography` for the oracle (dev only) | Approve. The oracle is the main defence against "checks that cannot fail" |
| 10 | Enrolment lifetime | 6 y (CAMP 1.2.2) or 3 y (5GAA draft CP) | 6 y default, settable |
| 11 | Real interoperability capture | Ask a provider or OmniAir for test-CTL credentials or a plugfest capture | Optional. It is the only way to validate against a live commercial SCMS beyond published files |

---

## 6. Traceability

- Every row of §1 cites 01, 02 or 03 for the real side and a file for today.
- Every work package closes one or more rows:
  - A0–A5: rows 1–14
  - B1–B5: rows 15–33
  - C1–C4: rows 34–40
  - D1–D3: rows 41–45
  - E1–E3: row 46
  - F1–F3: validate all rows
- The validation report (WP-F2) lists every row with its final status: identical (vector), identical (real artefact), identical (dual implementation only), or setting (UNCONFIRMED source).
