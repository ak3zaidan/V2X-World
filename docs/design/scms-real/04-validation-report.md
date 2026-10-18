# SCMS realism: real-world cross-check report (WP-F2)

Written 2026-10-18 at commit `fd9c444` plus this package's tests. This report compares the engine's US SCMS with the real deployment wherever there is something real to compare against: the published CTL and certificate-chain files, CAMP Release 1.2.2's example sizes and design sizing, Brecht et al.'s CRL estimate, and what the three CV pilots did. It lists every UNCONFIRMED item and how the engine handles it, and gives every gap-table row of `00-gap-analysis-and-plan.md` §1 a final status.

The owner's summary page is https://claude.ai/artifact/DXpbYbBq1y57Cigb5D36or. It is private until shared.

The rules of `00` §4.1 apply throughout:
- Every claimed match is checked by a test or by the independent A0 oracle (`tools/oracle`, asn1tools plus `cryptography`, written without reading the Rust).
- Every new check was shown to fail under an injected fault (§10).

**Verdict in one paragraph.** Every published SCMS Manager artefact decodes, re-encodes and verifies, both in the engine and in the oracle:
- 6 CTLs, 2 chain files and 10 elector certificates.
- Every elector signature (P-384).
- Every certificate signature up to its root.

The engine's component certificates have the same fields as the published certificate of the same role. Each one encodes to the same length as its published twin except for the name. CAMP's example sizes are matched only where the field set is the same. Where it is not, the difference is explained field by field:
- **Pseudonym:** 80 B against CAMP's 86 B. A USA region alone accounts for the 6 B.
- **CRLs:** 33.1 B per linkage entry, under Brecht's ≈ 40 B.
- **Sizing:** CAMP's fleet-sizing rates follow from the engine's per-device counts to within 1 % (0.54 rather than 0.5 requests/s).
- **Pilots:** Wyoming starvation, the NYC weekly top-off and the THEA 3-year preload are reproduced by tests.

Five findings need an owner decision (§9).

## 1. How to reproduce

| What | Command (project venv for Python; every cargo command through `heavy.sh`) | Result |
|---|---|---|
| Fetch the published artefacts into the gitignored cache (sha256 and SCMS Manager SHA-384 pinned) | `python3 -I tools/scms-artefacts/fetch.py` | `PASS (23 files, 23 fetched, 16 checked against published SHA-384)` |
| Real CTL and chain verification, Rust | `cargo test -p v2xw-sec --test real_artefacts -- --ignored` | 7/7 |
| The same files, oracle | `python -I tools/oracle/oracle.py real third_party/scms-artefacts/files/{production-*,test-*}/*.oer` | `real: PASS (164/164 checks over 8 files)` |
| Certificate sizes against CAMP and the published files | `V2XW_SIZES_OUT=f.json cargo test -p v2xw-proto --test camp_sizes`, then `python -I tools/realworld/sizes.py f.json --real third_party/scms-artefacts/files` | 2/2; `sizes: PASS (26 engine and 41 published certificates decoded by the oracle; 0 failures)` |
| CRL size against Brecht | `V2XW_CRL_OUT=c.oer cargo test -p v2xw-sec --test crl_size_brecht`, then `python -I tools/realworld/crl_sizes.py c.oer` | 2/2; `crl: PASS (330818 B for 10000 entries …)` |
| Pilot behaviours | `cargo test -p v2xw-proto --test appcert --test provisioning_b3a --test pregeneration`; `cargo test -p v2xw-engine --test rsu_certs` | 25/25; see §6 |
| One real week on the release binary | `v2xw run scenarios/credential-weeks.yaml --record-level metrics` (binary sha256 `99ef9b4e…45afc6`, manifest commit `a86a5c0`, the code of `fd9c444`) | 3,003 s wall, integrity VERIFIED; §4.2 |

The artefact tests are still `#[ignore]`. The owner's decision is that tests fetch the files into a gitignored cache and run whenever the cache is present. That switch is not made here (§9, item 5).

## 2. Real CTL and certificate-chain verification (gap row 14)

The engine (WP-A5 `ctl`/`chain`, WP-B5 governance) and the oracle (`scms_oracle/real.py`) each check the following, independently:

| Artefact | Engine check (`real_artefacts.rs`) | Oracle check | Result |
|---|---|---|---|
| Production CTL 2025-08-13 (2,671 B): roots `rootca.prod.s2x.io`, `v2xrootca.ghsiss.com`; 5 production electors | Decoded; each elector's HashedId48 equals SCMS Manager's printed fingerprint; ≥ 3 of 5 distinct trusted electors sign; `tbsCtlHash` = SHA-384(tbsCtl) | COER re-encodes byte for byte; 5 elector self-signatures; 5 CTL signatures (P-384/SHA-384) over `H(H(tbsData) ‖ H(elector))`; 5 `tbsCtlHash` | Both pass |
| Production CTL 2025-08 (the earlier zip) and test CTLs 2023, 2024-04-19, 2025-03-18, 2025-07-17 | The series installs in order (each one by the prior trust's quorum); test-2025-07-17 roots equal the five published root files; one root on that CTL (`rca.plugfest.c-its-pki.org`) is not approved | As above for each: 90 elector checks in all | Both pass |
| Production chain `cert-chain_0.oer`: 6 certificates (root, ICA, ECA, RA, 2 ACAs) | Every signature verified up to the CTL root; chain rules applied | 6 canonical-as-published checks; root self-signature; 5 issuer signatures | Both pass |
| Test chain (2025-07-17): 23 certificates from ISS, SAESOL and Autocrypt | As above | As above; one SAESOL ACA certificate is transmitted non-canonically (extension bitmap `02 06 80` where the 2022 module has 4 additions); its ICA signature verifies over the canonical re-encoding | Both pass; the non-canonical encoding is a real-world quirk the engine tolerates and the oracle names |
| Fault tests | Flipping any of 206 sampled bytes of the production CTL breaks it; a flipped chain-file signature byte fails | — | Both red when they should be |

Fault injected today: the CTL signature check was replaced by "a signer certificate is present" (`ctl.rs`). `flipping_any_byte_of_the_production_ctl_breaks_it` went RED ("flipping byte 1053 left the CTL intact"). WP-A5 injected 26 more faults.

One published-profile deviation is tolerated and counted, not hidden. Every published US RA certificate carries the group `{all, minChainLength 0, chainLengthRange 0, eeType app}` under an ICA whose `all` group has `minChainLength 2`. A strict 1609.2 chain check rejects it. Deployed devices accept these RAs, so the engine's device policy `ChainPolicy::us_device` tolerates exactly those two findings. The credential-weeks run counts them in `receiver_validation.tolerated` (336).

## 3. Certificate sizes against CAMP Table 3 (gap rows 1, 13)

**Sources.** CAMP Table 3 ("Example Size in Bytes (Certs are Not Fixed Size)", [CAMP122] pp. 34–36) was transcribed here from the PDF. CAMP adds that its "FQDN range was 14-23 bytes, and at most 2 PSIDs … were used" (Table 9 note, p. 63). The published certificates come from the chain files of §2.

**Method.** The engine's bytes come from the real COER encoder (`camp_sizes.rs`, the default profile). The oracle decodes every certificate, re-encodes it byte for byte, and sizes each field on its own (`tools/realworld/sizes.py`).

| Certificate | CAMP Table 3 | Engine | Published (min–max, n) | Reconciliation |
|---|---|---|---|---|
| OBE pseudonym | 86 (CV-pilot Table 9: 91) | **80** | none published | Fields: linkageData 13, cracaId 3, crlSeries 2, validity 7, appPermissions 8 (PSID 0x20, 0x26, no SSP), reconstruction value 34, issuer 9, framing 4. **With an identifiedRegion countryOnly 840 the engine's pseudonym is exactly 86 B** (oracle what-if). Whether US pseudonyms carry a region is UNCONFIRMED (§7, U1) |
| OBE / RSE enrolment | 87 (RSE in Table 9: 109) | 99 | none | Name `ee-000003e8.provider-a` (24 B where CAMP's example request uses `obeenr`, 8 B) plus the three-country region (12 B) and wildcard `certRequestPermissions` (4 B), per [SMP] §6.3.1. The size follows from the engine's name format, which is UNCONFIRMED (B2) |
| OBE identification | 89 | 70 | none | No region and no SSP; 2 PSIDs (10 B) |
| RSE application | 89 | 65 | none | One PSID (0x82), no SSP, no region. The real SPaT/MAP SSPs are not modelled (§9, item 3) |
| Root CA | 211 | 206 | 203–218 (6) | Same fields and encoding as `rootca.prod.s2x.io` except the name (23 B vs 20 B) |
| ICA | 195 | 179 | 176–200 (4) | Same encoding as `ica.prod.s2x.io` and SAESOL test except the name |
| ECA | 150 | 171 | 172–235 (4) | Same encoding as `eca.prod.s2x.io` except the name. CAMP's 150 B is smaller than every published ECA |
| RA | 217 | 192 | 193–217 (4) | Same encoding as `ra.prod.s2x.io` except the name (`certIssuePermissions` form; see §9 item 1) |
| ACA (PCA) | 216 | 156 | 157–216 (6) | Same encoding as the first `aca.prod.s2x.io` except the name. ISS and Autocrypt PCAs add a region and an encryption key (215–216 B, CAMP's size) |
| LA1 / LA2 | 205 | 154 | 178 (2, ISS pre-production) | Same fields as ISS's LAs except the region (ISS puts an identifiedRegion on every sub-CA, the production chain on none) |
| MA | 205 | 190 | 200–202 (2) | Same encoding as SAESOL test except the name |
| CRLG | 190 | 172 | 184 (1) | Same encoding as ISS pre-production except the name |
| PG | 172 | 155 (governance) | 159–163 (2) | Same encoding as both published PGs except the name |
| DCM | 219 | 152 | none published | No real counterpart to compare |
| Elector | 166 | (P-384, governance) | 208 / 213 (10) | The real electors are P-384 (52 B key, 99 B signature). CAMP's 166 B describes P-256 electors |

The governance hierarchy (WP-B5) names its components `*.provider-a.sim`, which adds 4 B. The PDU PKI the devices see uses `*.provider-a`. Both are pinned.

**SPDU sizes measured on the real encoder** (CAMP marks the response size UNCONFIRMED, 03 §12):

| SPDU | Bytes |
|---|---|
| `AcaEeCertResponsePrivateSpdu`, one certificate | 454–470 |
| `EeRaCertRequestSpdu` | 385–424 |
| `RaEeCertAckSpdu` | 292–294 |
| `EeRaDownloadRequestSpdu` | 319 |
| `EcaEeCertResponseSpdu` | 958 |
| `EeEcaCertRequestSpdu` | 235 |

So a THEA-style 3-year download is 3,120 × ≈ 460 B ≈ 1.44 MB per vehicle before zip framing.

**Faults injected:**
- Rust:
  - an extra PSID on every pseudonym (`pdu_prov.rs`) → RED, 83 B
  - a longer RA name (`pdu.rs`) → RED
  - a second relevant PSID on the MA → RED, 192 B
- Oracle script:
  - an RA without its encryptionKey → RED, "matches no published ra"
  - a longer SSP → RED, "some field other than the name encodes to a different length"
  - a truncated certificate → RED, decode
  - trailing bytes → RED, re-encode

## 4. CAMP sizing rates scaled to the fleet (gap rows 18, 22)

### 4.1 Per-device counts measured, then scaled

CAMP's design sizing is for 17 M new vehicles a year ([CAMP122] §6.3.3.4, §6.3.4.4, §6.3.6.4; 03 §12). It rests on these per-vehicle assumptions:
- one provisioning request in the vehicle's life
- 156 batch files at first download
- then one `.info` poll and one new file a week, from "half of new cars plus all existing cars"

`camp_sizing_rates_follow_from_the_engines_per_device_counts` measures those counts on the engine's real SPDU path, then scales them:

| Quantity | Engine, measured per device | Scaled to the fleet | CAMP |
|---|---|---|---|
| Provisioning requests | 1 (`ra-maxReqs` = 1) | 0.54 req/s | ≈ 0.5 req/s |
| `.info` polls, year 1 / 3 / 5 / 10 | 1 a week | 14.1 / 70.3 / 126.5 / 267.0 req/s | 14 / 70 / 126 / 267 |
| Batch files, year 1 | 156 at first download (THEA-style preload; RA horizon 3 years) | 84.0 files/s | 85 |
| Batch files, year 10 (170 M fleet) | + 1 a week | 337.0 files/s | ~337 |

The rates agree because the engine's devices behave as CAMP's sizing assumes. The test is the check that they do, measured from the SPDUs the RA receives, not taken from a parameter.
- Faults injected: a 2-year RA horizon (`max_periods_ahead` 104) → RED, 104 files. A top-off without `.info` → RED: each top-off became a new butterfly request, 5 instead of 1.
- The first fault attempt hit a second, unused constant (`RA_MAX_PRELOAD_PERIODS`) and stayed GREEN. The constant is duplicated (§9, item 4).

### 4.2 One real week on the release binary (`credential-weeks`)

The scenario runs on the real calendar from Tue 2026-10-20 08:50 UTC, for one week. Fast-forward skips 604,500 s. It uses the two-provider `us-production-2025` deployment, manual MA review, the business-day CRL and modelled crypto (I-S1 equivalence, WP-F3).

| Observable | Value | Comment |
|---|---|---|
| Vehicles (by provider) | 601 (308 / 294) | Seeded fleet split |
| Top-offs at the week rollover | 213 started, 188 completed, 3,760 certificates (20 each), 2 refused because the device was blocklisted | A revoked device cannot top off: the RA blocklist works as deployed |
| Misbehaviour reports | 55,147 sent, 54,432 over cellular, 714 from RSUs, 20,800 shared between the providers' MAs | — |
| MA cases and revocations | 19 cases, 19 revocations (provider-a 10, provider-b 9): 15 attackers, **4 honest** | Security-relevant: under the default manual review, 4 of 19 revocations in a week hit honest vehicles. Recorded, not hidden (`revoked_honest`) |
| Revocation stage times (run report) | report shuffle 333 s; MA decision 29,133 s (8 h manual review); CRL published 104,778 s (≈ 29.1 h: the next business-day slot); enforced 104,795 s | Publication is bounded by the [SM-PROV] one-business-day SLA, as configured |
| CRLs | 19 issued, 8 versions published, 477 downloads, 5,290 installs, 166 verified by vehicles, none refused | — |
| Receiver validation | 2,022,145 SPDUs checked, none refused; 336 tolerated RA-profile findings (§2) | — |
| Credential gaps | 1 vehicle starved for 19.75 s | — |

## 5. CRL size against Brecht (gap row 29)

Brecht et al. (T-ITS 2018 §VI-F) estimate about 40 B per linkage-CRL entry and about 400 KB per 10,000 entries.

**Method.** `crl_size_brecht.rs` builds series-1 full linkage CRLs with the CRL Generator's own `crl::crl_contents`:
- one LA pair and jmax 20
- each revoked device's `iMax` spread over the 156-week preload horizon, the worst case for group overhead

`tools/realworld/crl_sizes.py` has the oracle decode the 10,000-entry file. The re-encoding must be byte-identical, with 10,000 entries and 20,000 distinct 16-byte seeds.

| Entries | 0 | 1 | 10 | 100 | 1,000 | 10,000 |
|---|---|---|---|---|---|---|
| `CrlContents` bytes | 27 | 76 | 418 | 3,838 | 33,818 | **330,818** |

- **Linkage CRLs.** That is 33.08 B per entry at 10,000: two 16-byte seeds and a 1-byte preamble, plus the iMax-group overhead shared out. It sits under Brecht's ≈ 40 B and ≈ 400 KB. Brecht's figure is a design estimate. 03 §14 item 7 records that no live US CRL size has been published, so this agreement is with the estimate only.
- **Hash CRLs** (series 3/4) cost 15 B per entry: HashedId10, Time32 and a preamble.
- **The signed CRL** adds the 1609.2 envelope with the CRLG certificate. In the E1 fixture run, a SecuredCrl with one entry is 328 B.

Faults injected:
- every entry listed twice (`crl.rs`) → RED, 66.08 B per entry
- oracle side: a duplicated seed, a dropped entry and a truncated file → all RED

## 6. Pilot behaviours (gap rows 22, 24)

| Pilot fact (source) | Engine test | What it shows |
|---|---|---|
| **Wyoming:** rural RSUs could not keep 1-week application certificates fresh (01 §6; [CVP-LOG] p.16) | `proto/tests/appcert.rs::wyoming_an_rsu_without_backhaul_for_8_days_starves_then_recovers` and its 6-day control. Engine level: `engine/tests/rsu_certs.rs::a_unit_without_backhaul_starves_when_its_week_lapses_and_signs_nothing` | After 8 days without backhaul, the RSU's week-1 certificate expires at start(2) + 1 h. Nothing replaces it until the hourly retry gets through after the backhaul returns. Starvation lasts more than 20 h, there are ≥ 168 hourly `BackhaulDown` retries, no certificate exists to sign SPaT/MAP, and the RSU recovers. A 6-day outage does not starve. In the engine both starved units withhold every SPaT and MAP and count `rsus_starved` = 2 |
| **NYC:** 60 certificates a week for taxis, loaded one week ahead and topped off weekly, with weekly top-off monitoring ([CVP-P2] items 21, 24; [CVP-LOG] p.16) | `provisioning_b3a.rs::the_nyc_taxi_profile_installs_60_certificates_a_week`; `pregeneration.rs::a_weekly_top_off_is_an_info_download_on_the_one_request` | There is one butterfly request in all. Each week is a `.info` download, then that week's batch file (names checked: `<hash>.info`, `<hash>_<i hex>.zip`). That gives 60 × (weeks + 1) usable implicit certificates and 240 issued for 4 weeks. Every certificate is the same size |
| **THEA:** 3 years preloaded at once ([CVP-LOG] p.16; `ra-maxPreloadTime` ≤ 3 years) | `provisioning_b3a.rs::a_thea_style_three_year_preload_is_one_request_for_156_periods`; `pregeneration.rs::the_preload_horizon_is_held_between_two_weeks_and_three_years`; §4.1 | One request and one ack, 156 batch files, 156 private responses and 2 × 156 signed download requests (both hops). A request for 157 periods is clipped to the 3-year maximum |

These tests pass (25/25 proto; the engine result is in §10). Their fault proofs were done by WP-B3b, RAPRE and RSUC. The 03 §11 change rule for NYC ("every 2 km or 5 min") is a rotation preset (gap row 40) and is not re-checked here.

## 7. Everything UNCONFIRMED, and how the engine handles it

"Setting" means a scenario can change it. "Labelled" means `constants.md` lists it as ASSUMPTION with its reason and code location.

| # | Item (research ref) | Engine handling |
|---|---|---|
| U1 | Whether US pseudonym certificates carry a region (§3) | No region. With USA countryOnly the size is CAMP's 86 B exactly. Owner decision: §9 item 2 |
| U2 | Unified/CUBK butterfly response key (02 §15.1) | Refused. Only the original butterfly is implemented (owner decision) |
| U3 | The 1609.2 §5.1.3.4 linkage body text (02 §15.2) | Implemented from CAMP code and field sizes. CAMP `lv.txt`/`glv.txt` vectors pass |
| U4 | Group linkage `jValue`/ei relation (02 §15.3) | Off by default. The group linkage option is implemented and labelled |
| U5 | 1609.2 Annex D beyond D.6; any 1609.2.1 vector annex; a 1609.2-profile ECQV vector (02 §14–15) | Dual implementation with the oracle (500 chains) plus CAMP SEC 4 vectors. The standards are not bought (owner decision 2) |
| U6 | AES-CCM nonce rule (02 §15.5) | A fresh random 12-byte nonce. D.6.1 vectors pass |
| U7 | D.6.2 P1 misprint corrected in 2025? (02 §15.6) | The real P1 from CAMP `ecies.py`. The printed P1 is a fault test |
| U8 | The S8 `h_i` change adopted in 1609.2.1-2026? (02 §15.7) | Not adopted. The 2022 edition is the default |
| U9 | LCI, PLV-request and LA-key formats (02 §15.8; 03 §9) | Real AES-CCM PLVs, `symmRecipInfo` (row 12). The LCI is a 16 B ASSUMPTION; MA–LA formats are modelled |
| U10 | 1609.2.1 v3 URL paths and production ports (03 §2, §14.3) | CAMP REST paths. The file names `<hash>_<i>.zip` and `.info` are real |
| U11 | Semantics of `*-maxAge` and `ra-maxGenDelay` (03 §6.2, §14.5) | Read as in 03 §6.2. Cited values; the semantics are labelled |
| U12 | Misbehaviour format: J3287 content (03 §14.6; 01 §1.4) | SCMS Manager MBR v1.0 / ASN v1.1 (public, deployed). J3287 is not modelled |
| U13 | Real CRL sizes and cadence from a live provider (03 §14.7) | Real encoder sizes (§5). Cadence: one-business-day SLA, slot 10:00 ET (ASSUMPTION) |
| U14 | Provider data-centre placement (01 §2.3; 03 §13) | Assumed public-cloud metros, labelled (owner decision 6) |
| U15 | LA1/LA2 run by separate organisations (01 §2.2) | Separate entities with separate keys. The organisational split is not claimed |
| U16 | LOP deployed (01 §2.2) | Off by default; on in `camp-reference` (row 33) |
| U17 | ICA role and lifetime details (01 §2.2) | The published chain's ICA profile is copied (§3) |
| U18 | Eviden ↔ `rca.plugfest.c-its-pki.org` mapping; BlackBerry/ESCRYPT status (01 §2.1) | Neutral provider labels. Test-CTL roots are used only as data (§2) |
| U19 | TMC/back-end signing on CRL series 3 (01 §5) | Application certificates on series 3, labelled |
| U20 | As-built pilot scale; production revocations; MA at scale; OEM production use (01 §6–7) | Scale is a scenario input. The manual MA is the default (owner decision 7). Automatic mode is selectable |
| U21 | Elector Requirements and CTL Update Process documents (01 §1.3) | EAC timings from the 2024 texts (row 31) |
| U22 | Voting threshold change (75 % BOD vs EAC majority) (01 §1.3) | Governance follows the 2024 EAC text. Electors use quorum 3 of 5 (cited) |
| U23 | J3287 re-profiling by SCMS Manager (01 §1.4) | As U12 |
| U24 | Whole-certificate hash rule text, explicit roots in practice (02 §4, §6) | Both are confirmed by the real artefacts (§2): HashedId48/32 recomputed, roots explicit |
| U25 | The canonicalId text form and successor file name (B2) | Our own text form, labelled |
| U26 | The rekRecipInfo reading for the ACA response (A4) | Implemented as read. The oracle decrypts it |
| U27 | The `SignedEncryptedCertificateResponse` size (03 §12) | Now measured: 454–470 B per certificate (§3) |
| U28 | The constants without a source (`constants.md`: 34 ASSUMPTION rows, e.g. `first_batch_delay_s`, `ma_review_s`, `backend_servers`, `repo_url_bytes`, `batch_container_bytes`, `crl_slot_et_hour`, `provider_regions`) | Each one is a setting with its reason and code location, checked against the code by proto `model_card` and engine `scms_constants` |

## 8. Final status of every gap-table row

Statuses:
- **vector**: identical, proved by an official test vector
- **real artefact**: identical to the real published files
- **dual**: identical against the independent oracle only
- **setting**: modelled or chosen, with an UNCONFIRMED source and labelled

| Row | Subject | Status | Evidence |
|---|---|---|---|
| 1 | COER of 1609.2, canonicalisation | real artefact | Published CTLs and chains re-encode byte for byte (§2). asn1tools cross-check: 8,757/9,000 values; the NESTED exemption is documented |
| 2 | 1609.2.1 ASN.1 / ScmsPdu | real artefact + dual | Published files decode as ScmsPdu (§2). `check_pdus.py` 23/23, `check_jer.py` 102/102 |
| 3 | HashedId3/8/10/32/48 | vector + real artefact | `SHA-256("")` example; elector HashedId48 equal SCMS Manager's printed fingerprints (§2) |
| 4 | Time32/64 TAI | vector + real artefact | IANA leap table; the CTL `effectiveDate` decodes to July 2025 |
| 5 | ECDSA P-256 1609 digest | vector + real artefact | CAVP; every published chain signature verifies (§2) |
| 6 | P-384 electors | vector + real artefact | CAVP P-384; 30 production and 60 test elector signatures verify (§2) |
| 7 | ECIES | vector | D.6.2 ×4 with the real P1, D.6.3 ×7, D.6.4 ×4, RFC 4231 |
| 8 | AES-128-CCM | vector | D.6.1 ×6, CAVP; the nonce rule is a setting (U6) |
| 9 | ECQV-1609 | dual | 500 oracle chains; CAMP SEC 4 vectors for the algebra; no public 1609 vector (U5) |
| 10 | Butterfly | vector | CAMP `bfkeyexp.txt`, the OpenSCMS vector; prefix-swap fault |
| 11 | Linkage | vector | CAMP `lv.txt`, `glv.txt`; group linkage is a setting (U4) |
| 12 | PLV transport | dual | The oracle decrypts the RA-forwarded PLVs; key rotation is ASSUMPTION |
| 13 | Authority certificates | real artefact (field sets) | Each component encodes like its published twin except the name (§3); the DCM has no counterpart; the LA has no region (§9 item 1) |
| 14 | Published artefacts | real artefact | §2 |
| 15 | PDUs under PSID 35 | dual | `check_pdus.py`, `check_appcerts.py`, `check_mbr.py` |
| 16 | Factory enrolment | dual + setting | Profile per [SMP] §6.3.1; the name format is UNCONFIRMED (U25) |
| 17 | Re-enrolment | setting (cited values) | 18-month lead, 6 y lifetime (owner decision 10) |
| 18 | Provisioning request/ack | dual | The oracle decrypts and verifies; ra-maxReqs = 1 (§4.1) |
| 19 | RA → ACA | dual | One request per certificate; ACA-blindness test |
| 20 | Shuffle | setting (cited) | CAMP 1.2.2 default, Brecht preset; release-at-count-or-age test |
| 21 | ACA → EE private response | dual | Same-ACA rule fault; real file names |
| 22 | Download parameters | setting (cited) + pilots | [SMP] Table 3 values; NYC and THEA reproduced (§6) |
| 23 | TLS transport | setting (measured) | OpenSSL handshake sizes (`constants.md`) |
| 24 | RSU application / OBU identification | dual + pilots | `check_appcerts.py` 18/18; Wyoming (§6) |
| 25 | US misbehaviour report | dual | `check_mbr.py` against SM-MBR-ASN v1.1 |
| 26 | Report shuffle | setting (cited) | — |
| 27 | MA investigation, multi-provider | setting (model) | Flow stamps; 20,800 shared reports in the week run (§4.2) |
| 28 | MA modes | setting | Manual by default (owner decision 7); the honest-revocation count is recorded (§4.2) |
| 29 | CRL issuance | dual + design estimate | `check_run_crl.py` 19/19; Brecht size (§5); no live CRL to compare (U13) |
| 30 | CRL distribution | setting | Composite CRL via the RA; overdue rule tested |
| 31 | CTL lifecycle | real artefact | The published CTL series installs in order (§2); quorum tests |
| 32 | PG files | setting | The GPF/LPF payload layout is our own (B5) |
| 33 | LOP | setting | Off by default (U16) |
| 34 | Implicit pseudonym signing | dual | `d·G = e·P_U + Q_ACA` at install |
| 35 | Receiver validation | dual | One-rule-per-test matrix; the RA profile is tolerated (§2) |
| 36 | Revocation from the certificate's own linkageData | dual | The forged-relay test |
| 37 | Node CRL verification | dual | Altered-byte test |
| 38 | P2PCD | dual | asn1tools byte match |
| 39 | Crypto timing | setting (measured) | Per-primitive pricing; `unpriced` = 0 in the week run |
| 40 | Certificate change | setting (cited) | J2945/1 default, NYC preset |
| 41 | Two providers, components | setting | Deployment preset; the CTL lists its roots |
| 42 | Cross-provider trust | dual | Every receiver/signer provider pair verifies (§4.2) |
| 43 | Time grid | vector (derived) | i = 614 on 2026-10-16; the calendar lint |
| 44 | Fast-forward | dual (self-equivalence) | Equivalence test; byte-identical at 1 and 4 threads |
| 45 | Uncited constants | setting | `constants.md`, checked by `model_card` and `scms_constants` |
| 46 | UI | — | Playwright and server tests (E2, E3) |

## 9. Findings for the owner

1. **The engine's two component PKIs disagree on the RA's permission field.**
   - The PDU PKI's RA (what devices see) has a 6-byte `certIssuePermissions`, like SAESOL production (`ra.prod.s2x.io`).
   - The governance RA uses `certRequestPermissions`, like every test-CTL RA (ISS, SAESOL test, Autocrypt).
   - Both have a real precedent, but one provider would not use both. The engine LAs also carry no region, where the only published LAs (ISS pre-production) do.
   - Suggested fix: derive both PKIs from one per-provider profile, and pick the production form.
2. **Pseudonym region.**
   - With USA countryOnly the engine's pseudonym is exactly CAMP's 86 B.
   - Without it, the engine's on-air BSM certificate is 6 B smaller than the deployed one may be. That affects airtime (WP-F3 frame digests) and region checks.
   - It is UNCONFIRMED (U1). Choose: add the region (closer to CAMP's example), or keep the current form.
3. **Application and identification certificates carry no SSP.** The real SPaT/MAP/TIM SSPs ([CAMP122] Table 1) are not modelled, which explains part of the 19–24 B gap to CAMP. An attacker study of SSP misuse needs them.
4. **Duplicated constant.** `params.rs` has `RA_MAX_PRELOAD_PERIODS = 156`, and also `max_periods_ahead: 156` as a literal in the default. Changing the constant alone has no effect on the default; it is only a validation bound and the THEA preset. The first fault injection (§4.1) found this.
5. **Real-artefact tests are still `#[ignore]`.** The owner decided they should run whenever the cache is present. Making them skip silently when the cache is absent would be a check that cannot fail. The safer form is a required `V2XW_SCMS_ARTEFACTS=1` (or a present cache) that turns a missing file into a failure. This is left to the merge agent, so the 7 tests are not weakened here.

Also notable from the week run (§4.2): 4 of the 19 revocations in one week were honest vehicles under the default manual-review MA. The count is recorded (`revoked_honest`). It is a benchmark target, not a defect of this package.

## 10. Faults injected by this package

Each Rust fault was injected with `tools/mutate/mutate.py`, which restores the file with a fresh mtime. Each script fault was injected on a copy of the input.

| Check | Fault | Verdict |
|---|---|---|
| `real_artefacts` | CTL signature check replaced by "a signer is present" | RED |
| `camp_sizes` sizes | Extra PSID on pseudonyms | RED (83 B) |
| `camp_sizes` sizes | RA name lengthened | RED |
| `camp_sizes` sizes | Second MA relevant PSID | RED (192 B) |
| `camp_sizes` rates | `max_periods_ahead` 156 → 104 | RED (104 files) |
| `camp_sizes` rates | Top-off without `.info` | RED (5 requests) |
| `camp_sizes` rates | `RA_MAX_PRELOAD_PERIODS` 156 → 104 | GREEN: the constant is not on the default path (§9 item 4) |
| `sizes.py` | RA without encryptionKey / longer SSP / truncated / trailing bytes | 4 × RED |
| `crl_size_brecht` | Every entry listed twice | RED (66.08 B per entry) |
| `crl_sizes.py` | Duplicated seed / dropped entry / truncated | 3 × RED |
