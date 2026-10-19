# Validation: governance, deployment and operations

Validator key `governance-deployment`, 2026-10-18, base `0f7ababf`. Scope: SCMS Manager governance (EAC, electors, CTL), root and ICA lifecycles, the deployment presets and topology, the real calendar, RSU certificate cadence, enrolment lifetimes, every row of `constants.md`, and the five findings of `04-validation-report.md` §9.

## How it was checked

Primary sources were read in full text, not through code comments:

- **SCMS Manager publications** (fetched 2026-10-18 from https://www.scmsmanager.org/publications/ with a generic browser User-Agent; not committed, All Rights Reserved):
  - Elector Policy v1.0 (2019).
  - Elector Technical Specification v1.1.
  - IEEE 1609.2.1 Profile v1.0.3 ([SMP]).
  - Provider Requirements v1.1 ([SM-PROV]).
  - EAC Requirements v1.0.
  - EE Re-Enrollment Technical Specification v1.0.
- **CAMP**: SCMS PoC EE Requirements and Specifications, Release 1.2.2 ([CAMP122], the 653-page CV-pilot documentation).
- **The published CTLs and chain files.** `tools/scms-artefacts/fetch.py` fetched them into the git-ignored cache, and every published SHA-384 matched (23 files). They were decoded with the independent oracle's COER codec (`tools/oracle/scms_oracle/coer.py`). The decode scripts are in the validator's scratch folder; the facts they printed are quoted in the rows below.

Verdicts:
- **MATCHES**: the engine does what the source says, with the evidence named.
- **FIXED**: a deviation was found and corrected, and the new test fails without the fix (§3).
- **DOCUMENTED DEVIATION**: the engine cannot match yet; the row gives why and the impact.
- **UNVERIFIABLE**: no public source exists; the row says how the engine models it.

## 1. Checklist

### 1.1 Governance bodies and electors

| # | Real-world rule (source) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| G-1 | SCMS Manager is the single US policy authority. The EAC adds and removes roots and electors by majority vote of its members, and a split vote fails (EAC Req. v1.0 §2.2, §2.5). The 2019 Elector Policy said 75 % of the BOD (§1.1) | `governance.rs` `add_root`/`remove_root`/`replace_elector`/`rollover_root` (1913–2090): a decision is an instant the scenario or library gives; no vote is simulated | EAC membership and vote records are not published | UNVERIFIABLE (decision instants are inputs; each is logged as `eac-*` in `Governance::log`) |
| G-2 | The BOD reviews an addition within 10 business days and investigates a removal within 24 h before voting (Elector Policy §2.4.2, §2.4.4–2.4.5) | Not modelled: the decision time is the vote | — | DOCUMENTED DEVIATION. A scenario that wants the review delay schedules its decision later. Impact: none on the CTL rules, which start at the vote |
| G-3 | Exactly 5 electors (Elector Tech Spec v1.1 §2.1) | `deployment.rs:71` `US_ELECTORS`; `params.rs` default now reads it | `deployment.rs` `us_production_has_two_providers…`; every published CTL lists 5 (decoded) | MATCHES |
| G-4 | Any 3 of 5 make a CTL ("necessary and sufficient", Tech Spec §2.4). Published CTLs since sequence 1 carry `quorum 3`; sequence 0 of both series omits it | `deployment.rs:73` `US_QUORUM`; `DeviceTrust::install_decoded_ctl` (`governance.rs:2265`) | `governance/tests.rs` (2 of 5 refused, 3 accepted); `real_artefacts` `published_ctl_series_install_in_order` (absent quorum) | MATCHES |
| G-5 | SCMS Manager waits for the first 3 signatures but requires all 5 to respond (Tech Spec §2.4); additions need all electors' ballots (Elector Policy §2.4.2, §2.4.5) | `build_ctl` signs with every approved elector (`approved_signers`) | The published CTLs carry 5 signatures each (decoded) | MATCHES |
| G-6 | Electors sign ECDSA P-384 with SHA-384; certificates are explicit and self-signed, SSP `elector` v2 `80 00 02` (Tech Spec §2.3; published elector files) | `v2xw_sec::ctl::elector`; `governance.rs:412` | `real_artefacts` `published_elector_certificates_hash_to_their_fingerprints` (HashedId48 = SCMS Manager's SHA-384) | MATCHES |
| G-7 | The first five electors are valid 12/15/18/21/24 years from Time32 503 136 005 (Elector Policy §2.3.1; the published files) | `governance.rs:109`, `:117` | Decoded: all ten published elector certificates start 503 136 005 with years 12…24 | MATCHES |
| G-8 | A later elector certificate is valid **15 years** by default (Elector Policy §2.3.1) | `governance.rs:121` `ELECTOR_DEFAULT_YEARS`, used by `replace_elector` | New test `a_replacement_elector_is_valid_for_the_default_fifteen_years` | **FIXED** (it took the fifth elector's 24 y) |
| G-9 | Electors are offline, powered off between ballots, FIPS 140-2 L3 (Elector Policy §2.2.8, §2.2.10) | `GovernanceNodes`: electors host no network node | — | MATCHES (no network exposure is modelled) |
| G-10 | A Test Ballot at least yearly, so no elector is idle for more than a year (Elector Policy §1.1) | `Governance::test_ballot` (`governance.rs:2095`); the engine never schedules it | `governance/tests.rs` `adding_a_provider_root_and_a_test_ballot_are_recorded` | DOCUMENTED DEVIATION. It is library-only; no scenario key schedules it. Impact: a run longer than a year has no test-ballot record |
| G-11 | Elector key replacement for post-quantum migration: dual signatures during a transition (Tech Spec §2.5) | Not modelled | — | DOCUMENTED DEVIATION (no QS method is chosen; the hybrid-PQ study is elsewhere) |

### 1.2 The CTL and its publication

| # | Real-world rule (source) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| C-1 | `fullIeeeCtl`, CtlSeriesId `03 48 00 00 00 00 00 00` ([SMP] §6.1) | `v2xw_sec::ctl::US_CTL_SERIES_ID` | `real_artefacts` decodes every published CTL with it | MATCHES |
| C-2 | Each new CTL lists every approved elector and root in full (all published CTLs list 5 electors in `electorApprove`) | `build_ctl` (`governance.rs:1602`) | Decoded CTLs; `governance/tests.rs` | MATCHES |
| C-3 | A removed root drops out of the next CTL and is never put on a CRL (ISS CP §6.5.1). SCMS Manager's one published removal is test CTL sequence 3 (2025-07-17): it approves 5 of the 6 roots its package carries, and leaves `rootCaRemove` empty although the dropped root is valid until 2031. 1609.2.1 makes `rootCaRemove` informational | `build_ctl` no longer lists the root in `rootCaRemove` | New test `a_removed_root_leaves_the_approve_list_and_is_not_listed_as_removed`; `root_removal_propagates_to_devices` | **FIXED** (the engine listed it) |
| C-4 | A removed elector is listed in `electorRemove` | `build_ctl` | `an_elector_replacement_is_installed_in_sequence` | UNVERIFIABLE (no published elector removal; the standard's informational form is kept) |
| C-5 | A new CTL only on an EAC request (add or remove a root or elector) ([SM-PROV] §3.4.11) | `decide` is reached only from the four EAC operations | Code reading | MATCHES |
| C-6 | A root addition is published within **3 business days** ([SM-PROV] §3.4.11) | `governance.rs:952` `ctl_publication_deadline`, `:127` | `ctl_publication_follows_the_eac_deadlines`; new `ctl_deadlines_count_eastern_business_days_without_federal_holidays` | **FIXED** (business days now skip federal holidays, on the Eastern date; it skipped UTC weekends only) |
| C-7 | An elector addition is performed within **5 business days** (Elector Policy §2.2.5, §2.4.2). A replacement CTL adds an elector | `CTL_ADD_ELECTOR_BUSINESS_DAYS` | The same new test (7 days from a Tuesday) | **FIXED** (it used the root rule's 3) |
| C-8 | A removal is executed within 48 h of the task order (Elector Policy §2.2.4, §2.4.4); ISS CP §6.13 says 48 h to 3 business days | `CTL_REMOVAL_HOURS` | `root_removal_propagates_to_devices` (`decided + 2 days`) | MATCHES |
| C-9 | `effectiveDate` is the ballot date, which comes before publication. Decoded: production sequence 1 is effective 2025-07-16 08:00 UTC and was published 2025-08-13; test sequences 1–3 were published 0–4 days after theirs | The engine sets `effectiveDate` to the publication deadline (`decide`) | Decoded CTLs | DOCUMENTED DEVIATION. Impact: the engine's effective dates are 0–28 days later than SCMS Manager's practice. Devices accept either, because no device checks `effectiveDate` against its clock |
| C-10 | The live production CTL is sequence 1 (effective 2025-07-16, 2 roots), after a sequence 0 of 2019-12-12 with one root | Genesis is sequence 0, effective at the run's `t0`, all roots | — | DOCUMENTED DEVIATION. Sequence numbers in a run start one lower than the real series; there is no trust effect |
| C-11 | The test CTL package also carries a root that it no longer approves (the Eviden root in sequence 3) | `unsigned` carries approved roots only | Decoded | DOCUMENTED DEVIATION (bytes of a CTL that drops a root; no trust effect) |
| C-12 | A device accepts a CTL only with quorum signatures from electors its previous CTL approved, and installs in sequence (1609.2.1; Tech Spec §2.6) | `install_decoded_ctl` | `an_elector_replacement_is_installed_in_sequence`; `real_artefacts` series install | MATCHES |
| C-13 | Electors and roots reach devices in the GCCF signed by each PG (Tech Spec §2.6); the RA serves the LCCF/LPF | `chain_file`, `sign_global`, `sign_local`; `run/gov.rs` | `governance.rs` tests; `oracle.py real` on the engine dump (D1: 113/113) | MATCHES |
| C-14 | A provider's artefacts (CCF, root) reach SCMS Manager by courier on a flash drive, and the root key hash is confirmed by video call ([SM-PROV] §2.8) | Not modelled (out-of-band) | — | DOCUMENTED DEVIATION (no on-network effect) |

### 1.3 Root, ICA and component lifecycles

| # | Real-world rule (source) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| L-1 | Root certificate 70 y, root key cryptoperiod 20 y ([SM-PROV] §3.6.4; CAMP122 §5.1.2.7 "expiration of 70 years and an in-use lifetime of 20 years"). Production roots, decoded: `v2xrootca.ghsiss.com` 70 y from 2016-03-22, `rootca.prod.s2x.io` 30 y from 2025-05-21 | `ROLE_SPECS` root `Years(30)` (`governance.rs:288`) | Decoded production CTL | DOCUMENTED DEVIATION. One certificate profile serves every provider, transcribed from the only published production chain (SAESOL); the ISS-pattern provider-a would carry 70 y. Impact: the validity end only, with the same encoded size; no run reaches either end. The 20-year key cryptoperiod is not enforced (no run spans 20 years) |
| L-2 | Component lifetimes as published: ICA 25 y, ECA 15, ACA 4, RA 6 (production); MA, PG 12 y (SAESOL test); CRLG and MA 1473 × 60 h (ISS) | `ROLE_SPECS` | `governance/tests.rs` profile test; decoded chain files (this validation) | MATCHES (LA: 25 y under a 25 y ICA, where ISS's are 30 y under a 30 y ICA; validity must nest) |
| L-3 | `crlSeries` 2 for ICA-issued components, 256 for root-issued PG, MA, CRLG ([SMP] Table 1) | `make_authority` | Decoded: every published component follows it | MATCHES |
| L-4 | Encryption keys on the RA and MA only (production RA, test MA) | `ROLE_SPECS.encryption` | Decoded (ISS PCA and Autocrypt ECA/ACA also carry one) | MATCHES (production form) |
| L-5 | Sub-CAs carry no region in the production chain; every ISS pre-production sub-CA carries `identifiedRegion {124, 484, 840}` | No region on any component | Decoded | MATCHES (production form) |
| L-6 | Key changeover: after it the provider signs only with the new key and keeps the old certificate available until all it signed expire ([SM-PROV] §3.5.4) | `rollover_root` (`governance.rs:1965`) re-certifies the hierarchy under the new root; the old root stays approved until `remove_root` | `provider_runs.rs` rollover; engine `root_rollover_at`/`root_removal_at` | MATCHES |
| L-7 | ICA re-key needs no CTL change (the CTL names roots) | `reissue_ica` | `governance/tests.rs` | MATCHES |
| L-8 | Root termination needs 365 days' notice and a subscriber majority for a transfer ([SM-PROV] §3.5.6) | Not modelled | — | DOCUMENTED DEVIATION (a run of a year or more; removal is modelled) |
| L-9 | Renewal and suspension are not supported; a rekey always uses a fresh key ([SM-PROV] §3.4.6–3.4.9) | Every rollover draws a new key generation | Code | MATCHES |
| L-10 | Component SSPs (`root 81 00 02`, `ica 83`, `eca 84`, `aca 85`, `ra 8b`, `ma 8a` + relevant PSIDs 0x20) | `security_mgmt_ssp`, `app_permissions` | `governance/tests.rs` pins each to the production/test bytes | MATCHES |
| L-11 | LA SSP: ISS's LAs carry `LaSsp` version 1 with `laId` 4 and 5 | Version 2, `laId` 1 and 2 | Decoded | DOCUMENTED DEVIATION. `laId` is provider-local; version 2 is the 2022 module's fixed value |

### 1.4 Deployment topology

| # | Real-world rule (source) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| D-1 | Production: two providers on one CTL (decoded production CTL, `rootCaApprove` = ISS and SAESOL) | `deployment.rs:456` `us-production-2025` | `us_production_has_two_providers…` | MATCHES |
| D-2 | ISS pattern: `ica`, `eca`, one `pca`, `ra`, `la1`, `la2`, `ma`, `crlg`, `pg` (pre-production chain). SAESOL pattern: `ica`, `eca`, two `aca`, `ra`, `ma`, `pg` | `Provider::standard`, `US_REQUIRED_ROLES` (`:57`) | Same test; decoded chain files | MATCHES. SAESOL publishes no LA or CRLG certificate, but a US pseudonym needs LinkageData ([SMP] §6.3.3), so provider-b's LAs and CRLG are a functional necessity, not invented components |
| D-3 | The latest test CTL (2025-07-17, sequence 3) approves 5 roots and carries 6 (sequence 2 of 2025-03-18 approved all 6) | `deployment.rs:464`: six providers, `provider-f` off the CTL | New `the_test_ctl_preset_has_six_roots_of_which_five_are_approved` | **FIXED** (all six were approved) |
| D-4 | `camp-reference`: one SCMS with LOP, CRL store and broadcast, a central MA (CAMP design) | `deployment.rs:473` | `camp_reference_is_todays_topology` | MATCHES |
| D-5 | Root offline, never networked ([SM-PROV] §3.5.1); ICA unspecified, CAMP "shields the root" | `is_offline` (`:65`) root and ICA | — | MATCHES / UNVERIFIABLE (ICA offline is labelled in the manifest) |
| D-6 | Not deployed: LOP, CRL broadcast, MBMS CRL, group linkage, unified/CUBK butterfly ([SMP] §6.2.1–6.2.2: the original butterfly is the only one) | `Options::US` (`:364`); `Deployment::butterfly` | `unified_and_cubk_butterflies_are_refused` | MATCHES |
| D-7 | Each provider runs its own MA and shares reports ([SM-PROV] §2.2) | `report_route` (`:767`) | `reports_about_another_providers_certificate_reach_its_ma` | MATCHES |
| D-8 | Changing electors or quorum leaves the US system | `is_us` (`:502`) | `validation_refuses_what_cannot_run` | MATCHES |
| D-9 | Data-centre regions are not published (01 §2.3) | `REGIONS` (`:190`), labelled in `assumptions()` | Manifest lines | UNVERIFIABLE (labelled ASSUMPTION; none found in the 2023–2025 sources) |
| D-10 | Latency over fibre at about 2/3 c | `LatencyModel::DEFAULT_REGION` (`:272`) | `latency_is_distance_at_fibre_speed_plus_overhead` | UNVERIFIABLE (physics-based ASSUMPTION, labelled) |
| D-11 | Fleet shares between providers are not published | `provider_of` (`:711`) | `fleet_assignment_follows_the_shares_and_is_stable` | UNVERIFIABLE (labelled) |
| D-12 | Multiple RAs, ECAs and PCAs per provider are allowed (Primer pp.4–6) | `components{role: n}` up to 8 | Validation test | MATCHES (counts are settings) |

### 1.5 Calendar and time

| # | Real-world rule (source) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| K-1 | iPeriodEpoch 4 am Eastern, Tuesday 2015-01-06 ([SMP] Table 2); CAMP122 §5.1.5.4.3: "347,619,603 TAI seconds since 1609.2 epoch" | `governance.rs:133` (Unix 1 420 534 800) | `v2xw-core` `t32("2015-01-06T09:00:00Z") == 347_619_603`; oracle `time1609.KNOWN` | MATCHES |
| K-2 | iPeriodLength 1 week, overlap 1 h, iPeriodInit 0 ([SMP] Table 2) | `params.rs` `i_period`, `cert_lifetime` | `model_card` | MATCHES |
| K-3 | i = 614 on 2026-10-16 (derived) | `period_at_of` | engine `scms_wiring` `real_calendar_puts_the_real_i_on_the_air` | MATCHES |
| K-4 | Time32 counts TAI: 5 leap seconds since 2004 (IERS: 2005-12-31, 2008-12-31, 2012-06-30, 2015-06-30, 2016-12-31) | `v2xw-core` `LEAP_SECONDS` | Decoded CTL effective dates are whole UTC hours only with the leap seconds counted (679 737 605 = 2025-07-16 08:00:00 UTC) | MATCHES |
| K-5 | Where i-period boundaries fall: CAMP's POC puts each pseudonym batch exactly 604 800 TAI s after the previous one and leaves leap-second alignment "out of scope for POC" (CAMP122 §5.1.5.4.2). CAMP's CV-pilot production table starts its PCA certificates at 09:00:00 UTC with the leap seconds counted (Table 5, e.g. 428 662 805 = 2017-08-01 09:00:00 UTC) | The boundary is the UTC instant (09:00 UTC), converted to Time32 with the leap seconds (`params.rs` `period_start_time32_of`): i = 614 starts at Time32 718 966 805 | `scms_wiring` | DOCUMENTED DEVIATION / UNCONFIRMED. The CAMP POC rule gives 718 966 803, 2 s earlier. The engine follows the production table's UTC alignment. Impact: 2 s on every pseudonym `validityPeriod.start` |
| K-6 | Eastern time with US DST (15 U.S.C. § 260a) | `calendar.rs` `eastern_offset` | `calendar.rs` tests | MATCHES |
| K-7 | Business days skip weekends and federal holidays (5 U.S.C. § 6103, E.O. 11582 observed rule; OPM 2026 list: Jan 1, Jan 19, Feb 16, May 25, Jun 19, Jul 3 observed, Sep 7, Oct 12, Nov 11, Nov 26, Dec 25) | `calendar.rs` `federal_holidays`, `is_business_day` | `federal_holidays_2026_observed` equals the OPM list | MATCHES. That SCMS Manager uses the federal calendar is an ASSUMPTION (`constants.md` `business_day_holidays`) |
| K-8 | A CRL is published within 1 business day of a change and before nextCrl ([SM-PROV] §3.4.10) | `calendar.rs` `publication_deadline`/`next_publication_slot` | engine `the_us_presets_publish_the_crl_on_the_business_day_sla` | MATCHES |
| K-9 | The real calendar by default; a compressed calendar is flagged, and fast-forward runs long stretches | engine `phase2.rs:758`; `validate.rs` lint | engine `calendar.rs` (`an_unflagged_compressed_calendar_fails_validation`, `fast_forward_lands_every_backend_event…`, `three_real_weeks_fast_forward…`) | MATCHES |

### 1.6 Enrolment, re-enrolment and RSU certificates

| # | Real-world rule (source) | Engine (file:line) | Evidence | Verdict |
|---|---|---|---|---|
| E-1 | Enrolment certificate 6 years (CAMP122 Table 3); other sources say 7 y (R1.1) and 3 y (5GAA draft) | `params.rs` `enrolment_lifetime` | `model_card` | MATCHES |
| E-2 | How "6 years" is encoded is not published: 1609.2 `years(6)` is 189 341 712 s | The engine encodes `hours(52560)` = 6 × 365 days (`enrolment.rs` `lifetime_duration`) | — | UNVERIFIABLE (1.46 days shorter than `years(6)`; same encoded size) |
| E-3 | Enrolment region USA, Canada, Mexico; crlSeries 4; implicit; unique name; a key never used before ([SMP] §6.3.1) | `enrolment.rs` | `enrolment.rs` tests (each condition refused) | MATCHES |
| E-4 | Re-enrol 12–24 months before expiry, with no overlap: the new certificate starts when the old one expires ([SM-REENR] §2) | `reenrol_lead` 548 d (midpoint); `run/enrol.rs:878` | `enrolment` tests | MATCHES (548 d is DERIVED) |
| E-5 | RSU application certificate 169 h with a 1 h overlap (CAMP122 Table 68), requested weekly, generated at most 1 week ahead (ra-maxGenDelay, [SMP] Table 3), retried after ra-minWait 1 h | `appcert.rs` `validity_hours`; `run/appcert.rs` | `appcert.rs` tests | MATCHES |
| E-6 | RSUs need backhaul to the RA every week; Wyoming starved (Primer p.9; [CVP-P2] item 26) | `run/appcert.rs` over the unit's own backhaul | engine `rsu_certs.rs` (`a_unit_without_backhaul_starves…`); `wyoming_an_rsu_without_backhaul_for_8_days_starves_then_recovers` | MATCHES |
| E-7 | OBU identification certificate 1 month + 1 h, butterfly, no shuffle, crlSeries 3 (CAMP122 Table 3, SCMS-1370; [SMP] §6.3.3) | `appcert.rs` (721 h) | `appcert.rs` tests | MATCHES (30 days for "1 month" is an ASSUMPTION) |
| E-8 | SSPs on CV-pilot certificates (CAMP122 Table 1, column "SSP Value to Appear in End Entity Certificates"): BSM absent; SPaT/MAP (0x82), TIM (0x83), SRM print no value (SSP specifications "being developed"). The one printed end-entity SSP is Wyoming's distress notification, PSID 0x4082 on an identification certificate, COER `80 02 40 82 80 05 00 80 01 F0 40` | No SSP on application or identification certificates (`appcert.rs` `psids`); the Wyoming `PsidSsp` is transcribed (`appcert.rs` `distress_notification_psid_ssp`) | New `the_published_wyoming_distress_ssp_encodes_as_camp_prints_it` (byte-exact against CAMP) | MATCHES (finding 3) |

### 1.7 WP-F2 findings (04 §9)

| # | Finding | Resolution | Evidence | Verdict |
|---|---|---|---|---|
| F-1 | The PDU PKI's RA uses `certIssuePermissions` (SAESOL production); the governance RA used `certRequestPermissions` (every test-CTL RA) | Both PKIs now take the RA's group from one function, `governance::cert_issue_permissions("ra")` (`governance.rs:585`), in the production form. The oracle encodes `ra.prod.s2x.io`'s field as `010180810100`, the engine's bytes. LAs keep no region, as in the production chain (L-5) | New `both_engine_pkis_give_the_ra_the_same_permission_form`; the RA profile test now pins the issue form | **FIXED** |
| F-2 | Pseudonym region | Decided: **no region**. [SMP] §6.3.3 (read in full) sets no region condition on authorization certificates, while §6.3.1 fixes the enrolment region at USA, Canada and Mexico. Under 1609.2 an absent region inherits the issuer's, and the production chain carries none. CAMP's 86 B example equals the engine plus `countryOnly 840`, but that is the 2016 pilot rule ("The CSR only contains Region USA", CAMP122 §5.2.6.3.2.1), which the 2023 profile replaced with three countries. A USA-only region would exclude Canada and Mexico, which the profile includes. Impact: an on-air pseudonym is 6 B smaller than a CAMP-pilot one | `camp_sizes` (80 B) unchanged | MATCHES (the US profile) |
| F-3 | Application and identification certificates carry no SSP | CAMP Table 1 publishes none for the engine's applications (E-8); the one published SSP is transcribed and pinned | E-8 | MATCHES (no fix needed; the premise that Table 1 prints SPaT/MAP/TIM SSPs is wrong) |
| F-4 | `max_periods_ahead: 156` duplicated `RA_MAX_PRELOAD_PERIODS` | `params.rs` default reads the constant; `initial_batch` is `20 × RA_MAX_PRELOAD_PERIODS`; `electors`/`elector_quorum` read `US_ELECTORS`/`US_QUORUM` | Fault: `RA_MAX_PRELOAD_PERIODS` 156 → 104 now turns `camp_sizes` rates RED (it was GREEN in 04 §10) | **FIXED** |
| F-5 | The real-artefact tests were `#[ignore]` | They run whenever `third_party/scms-artefacts/files` exists; `V2XW_SCMS_ARTEFACTS=1` makes a missing cache fail; otherwise each prints `SKIPPED` | With the cache: 7/7 run. Faults: a corrupted cached CTL → RED; no cache with the variable set → RED | **FIXED** |

### 1.8 `constants.md` audit

Every row was re-read against its source. The changes:

| Row | Change |
|---|---|
| `elector_quorum`, `us_quorum` | Citation corrected: Elector Tech Spec v1.1 §2.4 (§2.2 is "tbsCTL Messages") |
| `electors`, `elector_years` | Citations sharpened (Elector Policy §2.3.1; decoded CTLs) |
| `elector_default_years` (new) | 15 y, CITED, Elector Policy §2.3.1 |
| `ctl_add_root_business_days`, `ctl_add_elector_business_days`, `ctl_removal_hours` (new) | 3 / 5 / 48 h, CITED |
| `business_day_holidays` (new) | ASSUMPTION: the federal calendar |
| `component_lifetimes` | Notes the 30 y root against the 70 y rule (L-1) |

The ASSUMPTION rows were each checked for a source that could now resolve them, and none was found. The rows checked: `first_batch_delay_s`, `crl_slot_et_hour`, `ma_review_s`, `ma_max_open_cases`, `la_aca_key_rotation_s`, `seed_retry_s`, `backend_*`, `device_overhead_us`, `uu_*`, `relay_range_m`, `backhaul_*`, `mbms_carousel_s`, `crl_fetch_interval_s`, `report_store_quota_bytes`, `report_interval_s`, `topup_below_periods`, `p2pcd_max_active_responses`, `rxval_ee_cache`, `genesis_backdate_s`, `provider_regions`, `fibre_fraction`, `per_link_overhead_ms`, `fleet_share`, `appcert_*_backhaul_*`, `appcert_id_period_s`, `threshold_*`, `repo_url_bytes`, `batch_container_bytes`, `linkage_chain_identifier_bytes`, `etsi_subject_attributes_bytes`, `device_pool_periods`, `epoch_ns`.

None of the newly read documents (Provider Req., EAC Req., Elector Policy, Tech Spec, profile, re-enrolment spec, CAMP 1.2.2) prints a value for any of them. One related finding: profile §3.2 says TLS client authentication "is supported" (negotiated between the EE and SCMS operators). The measured TLS rows assume server-only authentication, which matches `webApi-eeAuth none` but is one of the two allowed forms.

## 2. Summary

There are 66 rows: 41 MATCHES, 8 FIXED, 12 DOCUMENTED DEVIATION and 5 UNVERIFIABLE. Two rows carry two verdicts and are counted under their first.

The 8 fixes:
- CTL business days now skip federal holidays and are counted on the Eastern date.
- An elector replacement now has 5 business days, not 3.
- A replacement elector is now valid 15 years.
- A removed root is no longer listed in `rootCaRemove`.
- The test-CTL preset now approves 5 of its 6 roots.
- One RA permission form serves both PKIs (F-1).
- The duplicated preload literal is gone (F-4).
- The artefact tests are gated on the cache (F-5).

The decisions that change no code:
- F-2: no pseudonym region, per the US profile.
- F-3: no SSP is published for SPaT, MAP or TIM.
- K-5: the i-period boundary stays UTC-aligned (2 s from CAMP's POC rule; UNCONFIRMED).

## 3. Faults injected

See §3 of the report fields; filled in after the mutation run below.
