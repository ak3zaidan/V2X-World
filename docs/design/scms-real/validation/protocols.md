# Validation: protocols and backend flows

Area: the IEEE 1609.2.1 `ScmsPdu` messages and their signing and encryption layering, the
transport, enrolment and re-enrolment, pseudonym provisioning, RSU application and OBU
identification certificates, misbehaviour reporting, MA investigation, CRL issuance, and
CRL/CTL distribution. Checked against the primary sources in 01–03 (source tags as in
03 §0; [ASN-CRL] is the 1609.2-2022 `Ieee1609Dot2CrlBaseTypes` module kept with provenance
at `tools/oracle/asn1/ieee1609.2-2022/`). Code comments were not used as sources.

Verdicts:
- **MATCHES**: the engine does what the source says, with the evidence named.
- **FIXED**: a deviation found here and corrected, with a test that fails without the fix.
- **DEVIATION**: documented deviation. The engine cannot match yet; the reason and the impact are given.
- **UNVERIFIABLE**: no public source fixes the value; how it is modelled is given.

"Oracle" means the independent Python oracle (`tools/oracle`, `tools/pdu_check`,
`tools/conformance`). Its earlier results are in 04 §8 and 06 §6 (zero disagreements).
Paths are relative to `crates/` unless they start with `docs/` or `tools/`.

## Summary

| Verdict | Count |
|---|---|
| MATCHES | 90 |
| FIXED | 5 |
| DEVIATION | 7 |
| UNVERIFIABLE | 7 |
| **Total** | **109** |

Each item is counted by its first verdict. Five MATCHES rows (D4, E6, G7, G8, H7) also name a parameter whose value no public source fixes (for example the MA case cap, the 8 h review, the 10:00 ET slot). Those values are labelled ASSUMPTION in `constants.md`.

The fixes:
- **R2, device model.** A replayed older CRL no longer erases a newer revocation (item I3).
- **R2, engine node.** A stale CRL can no longer move the node's overdue clock back (item I4).
- **B4.** Every chain of a revoked enrolment is now revoked, not only the reported one (item G6).
- **`indexWithinI`.** It now counts the delta CRL as its own CRL, as the standard says (item H4).
- **SCMS-509.** A weekly pre-generation job armed before the blocklist now generates nothing (item D14).

The deviations that matter most for attack studies:
- **No EE retry.** The EE request retry (SCMS-522/523) is not modelled (B6), so an RA outage loses a request silently.
- **Modelled CRL size.** The engine's vehicle CRL download is charged by the 40 B/entry design size, not the encoded composite CRL (I7).

## A. Envelopes and the PDU catalogue (03 §3.1–3.3; 02 §7–9)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| A1 | Every EE↔SCMS message is `Ieee1609Dot2Data` carrying `ScmsPdu {version 2, content}` under SecurityMgmtPsid 35; signed SPDUs have header psid only; signer `SignerSingleCert` ([ASN-2.1] Protocol) | `v2xw-proto/src/scms/pdu.rs` (wrappers `signed`, `signed_encrypted` :1276, `signed_encrypted_cert_request` :1350) | Oracle `check_pdus.py` 23/23 (04 §8 row 15); `pdu_tests.rs` | MATCHES |
| A2 | `EeEcaCertRequestSpdu`: SignedCertRequest, `SignerSelf` (new key, or canonical key with `canonicalId`) | `scms/enrolment.rs`, `run/enrol.rs` | `enrolment.rs::the_bootstrap_issues_the_us_enrolment_certificate…`; oracle | MATCHES |
| A3 | `EcaEeCertResponseSpdu`: Signed by the ECA, `requestHash`, `ecaCertChain`, `certificate`, `privateKeyInfo` (32 B reconstruction value) | `scms/enrolment.rs` | Oracle check_pdus; the device reconstructs its key (`enrolment_tests.rs`) | MATCHES |
| A4 | `EeRaCertRequestSpdu` = `EeRa1609Dot2AuthenticatedCertRequestSpdu`: signed by the enrolment certificate, then encrypted to the RA's encryptionKey; tbsCert cracaId `000000`, crlSeries 0, appPermissions present | `scms/pdu_prov.rs` | Oracle decrypts with the RA key and verifies (04 row 18); `pdu_prov_tests.rs` | MATCHES |
| A5 | `RaEeCertAckSpdu`: Signed RA; `requestHash` HashedId8, `firstI` present for butterfly, `nextDlTime` | `scms/pdu_prov.rs`, `run/provision.rs` | `pdu_prov_tests.rs`; oracle | MATCHES |
| A6 | `.info`: CAMP form, one COER Time32 (next batch time), filename `[0-9A-F]{16}.info` ([CAMP122] §6.3, Table 79). The 1609.2.1 form is the unsecured `RaEeCertInfoSpdu` | `pdu_prov.rs:921` `info_body`; `info_filename` | `pregeneration.rs::the_info_answer_is_one_time32_the_device_acts_on` | MATCHES (CAMP form, the deployed pilot protocol) |
| A7 | `EeRaDownloadRequestSpdu`: `{generationTime, filename}` signed by the enrolment certificate and encrypted to the RA; carried Base64 in `Download-Req` ([CAMP122] §6.3) | `pdu.rs:1107`, `pdu_prov.rs` `ee_download_request_spdu` | `pregeneration.rs::an_info_request_naming_another_devices_file_is_refused`; oracle | MATCHES |
| A8 | `EeRaSuccessorEnrollmentCertRequestSpdu`: inner self-signed by the new key, outer signed by the current enrolment certificate, encrypted to the RA | `run/enrol.rs` | `enrolment.rs::a_successor_requested_at_the_lead_takes_over_exactly_at_expiry`; oracle | MATCHES |
| A9 | `RaAcaCertRequestSpdu`: SignedCertRequest by the RA; `flags` (butterflyExplicit, cubk), `linkageInfo {encPlv1, encPlv2}`, `certEncKey`, dummy LV with the real iCert | `run/provision.rs`, `pdu_prov.rs` | Oracle (04 row 19); `privacy.rs::the_pca_holds_no_device_identity_only_a_request_hash` | MATCHES |
| A10 | `AcaRaCertResponseSpdu`: Signed by the ACA, `requestHash`, `acaResponse.private` | `pdu_prov.rs` | Oracle | MATCHES |
| A11 | `AcaEeCertResponsePrivateSpdu`: encrypted to `certEncKey`, then signed by **the same ACA** that issued the certificate (otherwise invalid) | `pdu_prov.rs:431`; `appcert.rs:587` | Oracle decrypts with h_ij; wrong-ACA mutation refused (04 row 21; `appcert.rs::the_ee_installs_a_genuine_response_and_refuses_another_signer_or_a_tampered_one`) | MATCHES |
| A12 | US profile: original butterfly only; unified and compact-unified (CUBK) not used ([SMP] §2.1.21; owner decision) | `scms/deployment.rs:594-599` refuses `unified`/`cubk` | `deployment.rs::an_invalid_deployment_is_refused` | MATCHES |
| A13 | `SecuredCrl`: Signed `CrlContents`, psid 256, every other header field absent, signer's PSID-256 `CrlSsp` lists the series and `issuerIsCraca` ([ASN-2] Crl; 02 §11.1) | `run/crl.rs:230` `issue_secured_crls`; `v2xw-sec/src/crl.rs` | `check_run_crl.py` 19/19 (04 row 29); `crl_issuance.rs` | MATCHES |
| A14 | `CompositeCrlSpdu` (`cert.compositeCrl` {crl SEQUENCE OF SecuredCrl, homeCtl}), unsecured container ([ASN-2.1] CertManagement) | `v2xw-sec/src/ctl.rs` `encode_composite_crl`/`decode_composite_crl` | Real published artefacts decode (04 §2) | MATCHES (format); see I7 for the engine's charged size |
| A15 | `MultiSignedCtlSpdu`/`CtlSignatureSpdu` (elector-signed, PSID 35) and `CertificateChainSpdu` | `v2xw-sec/src/ctl.rs`, `scms/governance.rs` | Published CTLs and chains re-encode byte for byte and verify (04 §2) | MATCHES |
| A16 | `CertificateManagementInformationStatusSpdu` (`infoStatus`): what an EE polls cheaply to see whether a CRL/CTL/CCF changed ([ASN-2.1]) | Not built; devices fetch the CRL on a timer instead | — | DEVIATION: the poll's few hundred bytes are not on the air; a CRL fetch is charged instead, which over-counts bytes when nothing changed. A study of fetch load should add it |
| A17 | MBR is outside 1609.2.1 (`EeMaInterfacePdu ::= NULL`); the US format is SCMS Manager MBR v1.0 | `scms/mbr.rs` | Oracle `check_mbr.py` against SM-MBR-ASN v1.1 (04 row 25) | MATCHES |

## B. Transport and sessions (03 §2, §4.2)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| B1 | HTTPS over TCP; binary OER bodies, `application/octet-stream` ([CAMP122] §6.1) | `v2xw-engine/src/backend.rs` (TCP/TLS session model; bodies are the encoded SPDUs) | 04 row 23; `constants.md` handshake sizes | MATCHES (bytes); HTTP headers are not counted, see B9 |
| B2 | TLS 1.2/1.3, server authentication only, ECDHE-ECDSA suites ([SMP] §3.2, [SM-EESEC] §5.2.3) | `backend.rs` handshake flights | OpenSSL-measured flight sizes (`constants.md`) | MATCHES |
| B3 | Session lifetime at most 5 minutes; no resumption ([SMP] §3.2) | `backend.rs:263` `TLS_SESSION_MAX` = 300 s; every new session pays a full handshake | `backend.rs` unit tests (cap) | MATCHES |
| B4 | EE client authentication at the application layer: an enrolment-signed `Download-Req` with Time32, RA tolerance 5 s ([CAMP122] SCMS-1203) | `params.rs` `download_request_tolerance` = 5 s; `run/provision.rs` (download), `run/report.rs:342` (report upload) | `ma_cases.rs::the_ra_and_the_ma_refuse_unauthenticated_misattributed_and_tampered_uploads` | MATCHES |
| B5 | RA TCP ACK within 1 s (SCMS-521) | Backend M/M/c queue; no explicit deadline | `provisioning_in_run.rs::a_fleet_starting_together_makes_the_backend_queue` (the queue time is measured, not bounded) | UNVERIFIABLE as a bound: no deployed figure; the queue gives the time the load causes |
| B6 | EE retries after 10 s without a response, at most 10 retries per 60 min ([CAMP122] SCMS-522/523) | Not modelled: a request delivered to an offline RA is dropped (`fault.rs` `Offline`), and the device does not ask again until its next trigger | `faults.dropped` counts the loss | DEVIATION: adding the timer changes every flow's event stream and the pinned digests, so it is left to a package that re-blesses them. Impact: an RA-outage or DoS study under-counts the retry load (up to 10 per device-hour) and over-counts lost requests |
| B7 | Order on every connection: LPF and LCCF first, then requests, then batches, then `.info`, in one HTTPS session ([CAMP122] §3 step 8, SCMS-709) | `run/gov.rs` (policy on the next connection) and `run/provision.rs`; the engine opens one session per contact | `governance.rs::a_policy_decision_reaches_devices_on_their_next_connection_and_the_ra_enforces_it` | MATCHES (policy before requests); the in-session ordering of the other files is not checked separately |
| B8 | Production errors are coarse (HTTP 500), plus 404, 408 (retry after minWait) and 412 ([SMP] §2.1.20, §4.3.1) | `msg.rs` `Refusal` reasons, recorded per device; "not ready" polls follow the 408 rule | `pregeneration.rs::a_batch_that_never_comes_is_polled_every_five_minutes_at_most_one_hundred_more_times` | MATCHES (408 behaviour); the finer `Refusal` reasons are a simulation observable, which a real production RA would not show |
| B9 | REST paths on port 8892 (RA) and 8894 (CRL store) ([CAMP122] §6.2–6.3); the 1609.2.1 paths are UNCONFIRMED (03 §4.1) | `enrolment.rs:78` `RA_URL` `https://ra.provider-a:8892/`; flows are message types, not HTTP requests | — | UNVERIFIABLE: no HTTP request line or headers are modelled (tens of bytes per request). The provider label is neutral (owner decision) |
| B10 | The LOP strips IP/MAC; specified but likely not deployed (01 §2.2) | `deployment.rs` `lop: false` by default; on in `camp-reference` | `deployment.rs::us_production_hosts_both_providers_on_one_ctl`; `governance.rs::every_device_to_ra_message_goes_through_the_lop` | MATCHES |
| B11 | CRL store download "does not authenticate the downloading entity" ([CAMP122] §6.2.3) | `run/crl.rs` `CrlDownloadRequest` carries no signature | `flows.rs::crl_distribution_emits_its_declared_stages_in_order` | MATCHES |

## C. Enrolment and re-enrolment (03 §5)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| C1 | Bootstrap in a secure environment (DCM) with a canonical key (direct authorisation) ([SMP] §2.1.7) | `run/enrol.rs`, `scms/enrolment.rs` | `enrolment.rs::the_bootstrap_issues_the_us_enrolment_certificate_and_registers_it_with_the_ra` | MATCHES |
| C2 | Bootstrap installs the elector, root, ECA, RA and MA certificates and the RA URL ([BRECHT] §V-A; [CAMP122] §5.2.13.3) | `run/enrol.rs`; `DeviceState.ra_url`, `trust` | `governance.rs::the_dcm_bootstrap_installs_the_trust_list_chain_and_policy` | MATCHES |
| C3 | Enrolment request rules (US): region USA/Canada/Mexico only (840/124/484), crlSeries 4, `id.name` present and unique, type implicit, validity inside the ECA's ([SMP] §6.3.1) | `scms/enrolment.rs` | `enrolment_tests.rs` (840/124/484; a region with 250 refused; cracaId 000000) | MATCHES |
| C4 | Key never used before with that operator ([SMP] §6.3.1) | `run/enrol.rs:163`, `:230` `seen_keys` | `enrolment_tests.rs` | MATCHES |
| C5 | Wildcard permissions: `certRequestPermissions` one group, `subjectPermissions all`; no LinkageData ([SMP] §4.4) | `scms/enrolment.rs:106` | `enrolment_tests.rs`; oracle | MATCHES |
| C6 | The RA serves only registered enrolment certificates and must support blocking ([SMP] §2.1.8) | `RaState.registered`; `run/provision.rs` refusal | `enrolment.rs::the_ra_serves_only_registered_enrolment_certificates` | MATCHES |
| C7 | The ECA refuses a second bootstrap of a name it issued | `EcaState.names` | `enrolment.rs::the_eca_refuses_a_second_bootstrap_of_a_name_it_already_issued` | MATCHES |
| C8 | Enrolment lifetime 6 years (CAMP 1.2.2; owner decision) | `params.rs` `enrolment_lifetime` 6 × 365 d | `model_card.rs`; `constants.md` | MATCHES |
| C9 | Re-enrolment 12–24 months before expiry, through the RA, with no validity overlap ([SM-REENR] §1–3) | `params.rs` `reenrol_lead` 548 d (18 months); `run/enrol.rs` | `enrolment.rs::a_successor_requested_at_the_lead_takes_over_exactly_at_expiry` | MATCHES |
| C10 | A second successor request before download invalidates the first, and blocklists it if already issued ([SM-REENR]) | `run/enrol.rs:752`, `:929` `successors_blocklisted` | `enrolment.rs::a_second_successor_request_before_download_supersedes_the_first_and_blocklists_it` | MATCHES |
| C11 | A blocklisted enrolment gets no successor | `run/enrol.rs:733`, `:823` | `governance.rs::a_revoked_enrolment_gets_neither_a_top_up_nor_a_successor` | MATCHES |
| C12 | Existing pseudonym batches stay downloadable with the new enrolment certificate ([SM-REENR]) | `run/enrol.rs` registers the successor with the RA, linked to the old one | `governance.rs::an_expired_enrolment_cannot_get_a_top_up_until_it_is_renewed` | MATCHES |
| C13 | Pilot practice: re-enrolment needed a return to a secure environment ([CVP-P2] item 27) | Online re-enrolment through the RA, as the SCMS Manager standard specifies | — | UNVERIFIABLE: the pilots' manual process is not modelled; the production standard is |

## D. Pseudonym provisioning and top-up (03 §6, §9, §11)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| D1 | US time grid: i-period 1 week from 2015-01-06 04:00 ET; certificate 1 week + 1 h; 20 per period ([SMP] Table 2; [CAMP122] Table 3) | `params.rs` `i_period` 10,080 min, `cert_lifetime` 10,140 min, `certs_per_period` 20 | 04 row 43 (i = 614 on 2026-10-16); `scms_wiring.rs` | MATCHES |
| D2 | One request (`ra-maxReqs` = 1), again only "when parameters change" ([SMP] Table 3; [CAMP122] SCMS-520) | `run/provision.rs` (`ra-maxReqs` check against the LPF version) | `pregeneration.rs::a_second_butterfly_request_is_refused_until_the_policy_changes` | MATCHES |
| D3 | Original butterfly: signing and encryption expansion (AES-128), cocoon keys at the RA | `v2xw-sec` butterfly, `run/provision.rs` | CAMP `bfkeyexp.txt` and OpenSCMS vectors (04 row 10) | MATCHES |
| D4 | LA PLVs: 9 B, `EncryptedIndividualPLV` under an LA↔ACA symmetric key, rotated per `shared_key_update_interval` (local) | `pdu_prov.rs`; `params.rs` `la_aca_key_rotation_s` = 7 d | Oracle decrypts the RA-forwarded PLVs (04 row 12); CAMP `lv.txt` | MATCHES (format); rotation period UNVERIFIABLE (local configuration, labelled ASSUMPTION in `constants.md`) |
| D5 | Shuffle default CAMP 1.2.2: 1,000 requests or a 2-day maximum age; Brecht 10,000 or one day as a preset (owner decision 5) | `params.rs` `shuffle_threshold`/`shuffle_window`, `ShufflePreset` | `provisioning_b3a.rs::the_shuffle_releases_at_the_age_when_the_count_is_not_reached`, `…_at_the_count_before_the_age`, `the_presets_carry_the_cited_numbers` | MATCHES |
| D6 | One `RaAcaCertRequest` per certificate after the shuffle; the ACA never sees j or the EE identity | `run/provision.rs` | `pregeneration.rs::the_aca_can_link_no_certificate_to_a_device_or_to_another`; `privacy.rs` | MATCHES |
| D7 | ACA inserts the real LV, cracaId and crlSeries 1 | `run/provision.rs` | Oracle; `provisioning.rs::the_pca_issued_exactly_the_requested_certificates` | MATCHES |
| D8 | Device derives each private key: `d = e·b + r`, `d·G = e·P_U + Q_ACA` | `pdu_prov.rs` | 04 row 34; `provisioning.rs::every_provisioned_credential_is_usable_by_the_device` | MATCHES |
| D9 | Batch file `<16 hex requestHash>_<i hex>.zip` (upper case, `[0-9A-F]{1,8}`), entries `i_j` in hex ([CAMP122] Table 79; example `AB09281C9867DE53_F.zip`) | `pdu_prov.rs` `parse_download_filename`, `:940` `batch_entry_name` | `pdu_prov_tests.rs` (9 hex digits refused; `266_13`) | MATCHES |
| D10 | `download-minWait` 5 min, `download-maxReqs` 100 ([SMP] Table 3; 408 rule) | `params.rs` `download_poll_interval` 300 s, `max_download_polls` 100 | `pregeneration.rs::a_batch_that_never_comes_is_polled_every_five_minutes_at_most_one_hundred_more_times` | MATCHES |
| D11 | `download-maxAge` 1 h, `ra-maxAge` 1 week, `eca-*` parameters ([SMP] Table 3) | Not enforced | — | UNVERIFIABLE: their semantics are in paywalled 1609.2.1 Tables 5–7 (03 §6.2, §14 item 5). Read as maximum request age, a 1-hour limit would only refuse a download request older than an hour, which the engine never sends |
| D12 | `ra-maxPreloadTime` 2 weeks – 3 years; THEA 3 years, NYC 1 week ([SMP] Table 3; [CVP-LOG] p.16) | `params.rs` `max_periods_ahead` 156, `RA_MIN/MAX_PRELOAD_PERIODS` 2/156 | `pregeneration.rs::the_preload_horizon_is_held_between_two_weeks_and_three_years`; `provisioning_b3a.rs` THEA and NYC | MATCHES |
| D13 | The RA keeps the horizon generated and adds a week every week, within `ra-maxGenDelay` 1 week ([PRIMER] p.6) | `run/provision.rs:348` `pregen_due`, weekly job | `pregeneration.rs::the_ra_keeps_three_years_generated_and_adds_a_week_at_each_boundary`, `on_the_real_calendar_…` | MATCHES |
| D14 | "Generation stops for blacklisted devices" ([CAMP122] SCMS-509), including a weekly job armed before the decision | `run/provision.rs:426`: the job checks the blocklist when it runs; `RaState.pregen_blocked` counts it | **New** `pregeneration.rs::a_weekly_job_armed_before_the_blocklist_generates_nothing` (fails without the check: a week is expanded for the revoked device) | FIXED |
| D15 | A blocklisted device is refused a top-off at login (SCMS-1204) | `run/provision.rs` `.info` handler | `pregeneration.rs::a_device_with_an_account_is_still_refused_a_top_off_once_blocklisted`; `provisioning.rs::a_blocklisted_device_is_refused_a_top_up` | MATCHES |
| D16 | Top-up is incremental by `.info` then batch download; the RA updates `.info` for every EE ([CAMP122] §5.2.7.8, SCMS-576) | `run/provision.rs` | `pregeneration.rs::a_weekly_top_off_is_an_info_download_on_the_one_request` | MATCHES |
| D17 | First-batch delay "to accommodate shuffling, certificate generation, and certificate encryption"; no numeric bound ([CAMP122] §5.2.7.6.7.1) | `params.rs` `first_batch_delay` 60 s after the shuffle releases | — | UNVERIFIABLE: no published bound (03 §12); the shuffle's own wait (up to 2 days) dominates |
| D18 | Batch download resumes with `Range`, even after an IP change ([SMP] §4.3.2) | Not modelled: a download is one transfer | — | DEVIATION: an interrupted download costs a full retransfer instead of the remainder. Matters only for lossy-link studies |
| D19 | NYC 60 certificates a week for taxis; weekly top-off ([CVP-P2] item 24) | `provisioning_b3a.rs` profile | `provisioning_b3a.rs::the_nyc_taxi_profile_installs_60_certificates_a_week` | MATCHES |

## E. RSU application and OBU identification certificates (03 §7)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| E1 | RSU application certificate: 1 week + 1 h (169 h), 1 (+1 in the overlap) concurrent, not pre-generated, requested weekly, no butterfly, no shuffle; signed by the enrolment certificate and encrypted to the RA; the PCA encrypts to the RSE and signs ([CAMP122] Table 3, Table 68, SCMS-1213) | `scms/appcert.rs:7`, `:99`; `run/appcert.rs` | `appcert.rs::an_rsu_application_certificate_is_implicit_series_3_without_linkage_data`; `check_appcerts.py` 18/18 | MATCHES |
| E2 | Series 3, no LinkageData; a certificate with butterfly and LinkageData is series 1, one without is 3 ([SMP] §6.3.3) | `scms/appcert.rs` | as E1 | MATCHES |
| E3 | The RA refuses stockpiling: a second request, and one beyond `ra-maxGenDelay` | `run/appcert.rs` | `appcert.rs::the_ra_refuses_stockpiling_a_second_request_and_one_beyond_max_gen_delay` | MATCHES |
| E4 | Only the requester's enrolment certificate may download its file | `run/appcert.rs` | `appcert.rs::only_the_requesters_enrolment_certificate_may_download_its_file` | MATCHES |
| E5 | Wyoming: an RSU without backhaul for longer than its certificate's life stops signing validly ([PRIMER] p.9; [CVP-P2] item 26) | `run/appcert.rs`, engine RSU cycle | `appcert.rs::wyoming_an_rsu_without_backhaul_for_8_days_starves_then_recovers` and its 6-day control; `v2xw-engine/tests/rsu_certs.rs` | MATCHES |
| E6 | OBU identification certificate: 1 month + 1 h, original butterfly, no shuffle, series 3 ([CAMP122] Table 3, SCMS-1370) | `scms/appcert.rs:99`: 30 days + 1 h (721 h) | `appcert.rs::an_identification_certificate_uses_the_original_butterfly_and_series_3` | MATCHES; "1 month" read as 30 days (1609.2 `Duration` has no month), UNVERIFIABLE beyond that |
| E7 | RSU SPaT/MAP/TIM SSPs ([CAMP122] Table 1) | Not carried | — | DEVIATION (04 §9 item 3): an SSP-misuse study needs them; the certificates are 19–24 B shorter than CAMP's examples |

## F. Misbehaviour reporting (03 §8.1–8.2)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| F1 | `EndEntityMaInterfacePDU` → `EeMaApplicationSpecificMisbehaviorReport {version 1, generationTime, generationLocation, psid, contents}` ([SM-MBR] §1; [SM-MBR-ASN]) | `scms/mbr.rs` | `mbr.rs::the_open_type_layout_is_x696`; oracle `check_mbr.py` | MATCHES |
| F2 | Reporting PSID 0x26; first application PSID 0x20 (BSM) | `mbr.rs:79` `MBR_PSID` | `mbr.rs::the_ma_refuses_tampered_misaddressed_and_wrong_psid_reports` | MATCHES |
| F3 | Types/subtypes and defaults: 1VInconsistent (ConstantPosition, RandomPosition gpsDrift 1 m); Implausible (MaxSpeed 90 m/s, MaxAcceleration 10 m/s², BrakeAcceleration); ObsImplausible Distance 3000 m + 10 m, **off by default** ([SM-MBR] §2.3) | `mbr.rs:263-286` | `mbr.rs::the_evidence_count_rules_hold`; oracle | MATCHES |
| F4 | Evidence `SignedBsmsWithCertificate` (1..MAX signed BSMs + the signing certificate) | `mbr.rs` | `mbr.rs::evidence_the_accuser_signed_does_not_verify` | MATCHES |
| F5 | Signed by a pseudonym certificate, headerInfo generationTime only, full certificate as signer; encrypted to the MA whose `MaSsp.relevantPsids` contains the PSID ([SM-MBR] §3; [ASN-2.1] MaSsp) | `mbr.rs:572-581` | `mbr.rs::a_device_reports_and_the_ma_opens_it`; misaddressed report refused | MATCHES |
| F6 | One HTTPS POST per report through the RA, with a `Download-Req` filename `misbehavior-report` signed by the enrolment key ([SM-MBR] §4) | `mbr.rs:85`; `run/report.rs:342-376` | `ma_cases.rs::a_real_report_reaches_the_ma_through_the_ra_and_drives_the_revocation`, `…refuse_unauthenticated_misattributed_and_tampered_uploads` | MATCHES |
| F7 | EE queue: weight 1.0, ×0.9 daily at 05:00 Eastern; upload highest weight first (ties newer first); over quota delete lowest (ties older); delete after a TCP-confirmed upload; unsent reports older than a week may be deleted ([SM-MBR] §2.2) | `mbr.rs:783-876` `MbrQueue`; `scms/calendar.rs` | `mbr.rs::the_queue_decays_at_five_eastern_and_orders_uploads`; `v2xw-engine` phase2 unit test | MATCHES |
| F8 | RA→MA report shuffle: 10,000 reports or 1 day ([CAMP122] SCMS-765) | `params.rs` `report_shuffle_*` = `BatchPolicy::CAMP_SHUFFLE` | `lockstep.rs::the_report_shuffle_closes_at_its_count_before_its_window` | MATCHES |
| F9 | J3287 / ETSI TS 103 759 formats | Threat-crate reports adapted to SM-MBR | — | UNVERIFIABLE: J3287 is paywalled (03 §14 item 6); the deployed SM-MBR format is used |

## G. MA investigation and revocation (03 §8.2; [BRECHT] §VI)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| G1 | "Link" set {MA, PCA, one LA}: MA → PCA LV → (sealed) chain identifiers; MA → LA "same device?", one bit ([BRECHT] §VI-B/C) | `run/revoke.rs` (`PcaLookup*`, `SameDevice*`, :264) | `resolution.rs::a_linkage_authority_answers_one_bit_and_the_ma_needs_both`, `two_reports_about_two_devices_do_not_resolve` | MATCHES |
| G2 | "Revoke" set {MA, PCA, RA, LA1, LA2}: PCA → request hash; RA → blocklist + LCIs; LA1/LA2 → ls(i) for the revocation period; CRLG publishes ([BRECHT] §VI-D) | `run/revoke.rs` | `revocation_path.rs::a_report_becomes_a_vehicle_that_cannot_sign`; `lockstep.rs::a_decided_certificate_is_revoked_through_both_linkage_authorities` | MATCHES |
| G3 | The LA releases `ls(i)`, never `ls(0)`: revocation is forward-only | `run/revoke.rs:441` | `backward_privacy.rs` | MATCHES |
| G4 | No single entity learns both identity and LV | `privacy.rs` invariants | `privacy.rs` (5 tests) | MATCHES |
| G5 | The RA blocklists the enrolment (never revealed to the MA) and tells the ECA ([CAMP122] SCMS-507; [5GAA] (257)) | `run/revoke.rs` `BlocklistRequest`, `BlocklistNotice` | `resolution.rs::two_reports_about_one_device_resolve_to_that_device_and_blocklist_only_it` | MATCHES |
| G6 | **B4.** The RA blocklists the *enrolment*, so it must return the chain identifiers of **every** request it accepted from it (a second request after a parameter change, [CAMP122] SCMS-520, has its own chains). Brecht §VI-D describes the per-request return because a device has one request; revoking only the reported chain leaves the other batch valid for its whole life | Was: one LCI pair per case. Now `run/revoke.rs:327`: the RA returns every distinct `(LCI1, LCI2)` of the device's requests, the reported one first; the MA asks both LAs per chain (`Case.chains`, `chain_seeds`); one `CrlAppend` carries one entry per chain; seed retries and the t-of-n committee path cover every chain | `redteam_pki.rs::every_chain_of_a_revoked_enrolment_is_revoked` (was the pinned finding: 2 certificates escaped; now 0 escape, 2 CRL entries, device B untouched). Fails with the RA returning only the reported pair | FIXED |
| G7 | Concurrent cases | `run/mod.rs` `MaState.cases`, cap `max_open_cases` 1,000 | `ma_cases.rs::a_hundred_concurrent_cases_all_complete_with_every_stage_stamped`, `a_full_case_table_refuses_…` | MATCHES; the cap is UNVERIFIABLE (labelled ASSUMPTION) |
| G8 | MA practice: manual review by an operator in the pilots; automatic selectable (owner decision 7; [PRIMER] pp. 6, 8) | `params.rs` `MaMode::ManualReview { review: 8 h }` default | `ma_cases.rs::manual_review_waits_for_the_operator_and_automatic_does_not` | MATCHES; the 8 h review time is UNVERIFIABLE (labelled) |
| G9 | Each provider runs its own MA and shares reports | `v2xw-engine` per-provider runs | 04 row 27 (20,800 shared reports in the week run) | MATCHES |
| G10 | Investigation ends cleanly when the PCA never issued the LV | `run/revoke.rs` `PcaLookupResponse { found: None }` | `resolution.rs::a_linkage_value_the_pca_never_issued_resolves_to_nothing` | MATCHES |
| G11 | Certificates from two requests of one device still resolve to that device | LA owner map | `resolution.rs::certificates_from_two_different_provisioning_requests_still_resolve_to_one_device` | MATCHES |

## H. CRL issuance (02 §11; 03 §3.4, §10)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| H1 | `CrlContents` version 1, `crlCraca` = HashedId8 of the CRACA (the ICA under `issuerIsCraca`), `nextCrl` > `issueDate` | `run/crl.rs:230` | `check_run_crl.py`; `crl_issuance.rs::a_revocation_publishes_a_real_secured_crl_that_revokes_exactly_the_device` | MATCHES |
| H2 | Series: 1 pseudonym (linkage seeds), 3 RSU application + OBU identification (HashedId10), 4 enrolment (hash), 2 components, 256 root-signed ([SMP] Table 1) | `run/crl.rs` series 1, 3, 4 | `crl_issuance.rs`; `appcert.rs` | MATCHES for 1, 3, 4; series 2 and 256 are governance-driven (CTL/component revocation), outside this area |
| H3 | Linkage entries grouped `JMaxGroup → LAGroup → IMaxGroup`, seeds walked to `iRev`, exhausted entries pruned at `iMax` | `run/crl.rs` `linkage_revocations`; `v2xw-sec/src/crl.rs` | `crl_issuance.rs::a_later_crl_walks_earlier_seeds_forward_to_its_i_rev`; Brecht size (04 §5) | MATCHES |
| H4 | `indexWithinI` "set to 0 for the first CRL issued for … crlCraca, crlSeries, and iRev, and increments by 1 every time a new full **or delta** CRL is issued" ([ASN-CRL] `ToBeSignedLinkageValueCrl`) | Was: the full and the delta of one publication carried the same value. Now `run/crl.rs:251`: the full takes the next value and its delta the one after | `crl_issuance.rs::a_second_revocation_issues_a_full_and_a_delta_crl` (new assertion: full 1, delta 2; fails with the old shared counter) | FIXED |
| H5 | `crlSerial` "increments by 1 every time a new full or delta CRL is issued" for the series ([ASN-CRL] `ToBeSignedHashIdCrl`) | `run/crl.rs` `hash_serial` per series (+1 per full hash CRL; no hash deltas are issued) | `crl_issuance.rs` | MATCHES |
| H6 | Full and delta CRLs | `params.rs` `CrlIssuance.deltas` = true | `crl_issuance.rs::a_second_revocation_issues_a_full_and_a_delta_crl` | MATCHES |
| H7 | Publication within 1 business day of a change and before `nextCrl`; US federal holidays ([SM-PROV] §3.4.9–3.4.10) | `run/crl.rs` `business_day_sla`, `scms/calendar.rs:202` `next_publication_slot`; set per provider by the deployment (`v2xw-engine/src/phase2.rs` `apply_crl_publication_of`) | `crl_issuance.rs::a_friday_evening_decision_publishes_on_monday`, `…federal_holiday…`, `…tuesday_morning…`; `redteam_pki.rs::the_revocation_window_on_the_real_calendar` | MATCHES; the 10:00 ET slot is UNVERIFIABLE (labelled) |
| H8 | `nextCrl − issueDate` | `params.rs` `next_crl` 7 d | — | UNVERIFIABLE: 7 d keeps the "at least weekly" fetch current; the draft CP allows monthly ([5GAA] (254)). Labelled ASSUMPTION |
| H9 | `jmax` is a Uint8: at most 255 certificates per period ([CAMP122] Table 68 note) | `linkage_revocations` clamps `jmax` to `u8` | `v2xw-sec` CRL tests | MATCHES |
| H10 | `indexWithinI` is a Uint8: at most 256 CRLs per (CRACA, series, iRev) | Saturates at 255; `issueDate` then orders the lists (`CrlOrder`, I3) | — | DEVIATION: a real CRLG would have to stop issuing at 255 in one week. Reached only by publish-on-decision runs with more than 127 publications in a week |

## I. CRL and CTL distribution and the device (03 §10; 02 §11.2; [CAMP122] §5.2.10.2.3)

| # | Real-world rule (source) | Engine | Evidence | Verdict |
|---|---|---|---|---|
| I1 | The device verifies the `SecuredCrl` (signer chain, `CrlSsp`, signature, contents) and acts only on the signed bytes | `run/crl.rs:131` `device_check_crl`; node `v2xw-node/src/rxval.rs` `verify_crl_inner` | `crl_issuance.rs::a_secured_crl_altered_in_transit_is_refused_by_the_device`; `redteam_pki.rs::an_rsu_edited_crl_is_refused_whole`; `rx_validation.rs::a_signed_crl_is_accepted_and_one_altered_byte_is_refused_by_the_node` | MATCHES |
| I2 | A CRL entry matches a certificate only if its `cracaId`/`crlSeries` match the CRL's | node `stores.rs` `check_from` | `rx_validation.rs::a_linkage_entry_revokes_only_a_pseudonym_of_the_same_craca` | MATCHES |
| I3 | **R2, device model.** A device must not let a replayed earlier CRL roll back a newer one: 1609.2 numbers every CRL of a series (`crlSerial`; `iRev` + `indexWithinI`), so a list whose counter is not above the held one's for that CRACA and series is stale ([ASN-CRL]). What real devices do is unpublished; refusing is the reading of the counters' purpose | Was: `dev.crl = list` for any list that verified. Now `run/crl.rs:90` `CrlOrder` (counter, then `issueDate` as a tie-break); the `CrlDownload` handler refuses a different list that is not newer (`DeviceState.crls_stale`, a `pdu_rejections` row "stale CRL"), and the same list heard again is still installed | `redteam_pki.rs::a_replayed_older_crl_is_refused_and_the_newer_revocation_holds` (was the pinned finding: A un-revoked at C and un-silenced; now 1 stale refusal at each of C and A, A still revoked and silent, v2 re-heard accepted). Fails without the order check | FIXED |
| I4 | **R2, engine node.** The node's freshness rule refused only a *full* CRL with an older `issueDate`. An earlier list issued in the same second was accepted, and so was any old delta, which reset the stored `nextCrl` and so the overdue clock (a replayed old delta could push a node into "distrust everything" early) | Now `v2xw-node/src/rxval.rs:363` `crl_order`: any CRL, full or delta, whose (counter, `issueDate`) is not above the installed one's is `CrlStale` (`rxval.rs:987`); the node stays additive per series | **New** `rx_validation.rs::an_older_crl_full_or_delta_is_stale_even_in_the_same_second`. Fails with the old rule | FIXED |
| I5 | The OBE drops its own matching certificates and stops transmitting for that PSID/SSP when it finds itself revoked ([CAMP122] §5.2.10.2.3) | `DeviceState.silenced`; node own-revocation check | `revocation_path.rs::a_report_becomes_a_vehicle_that_cannot_sign` | MATCHES |
| I6 | Fetch "as often as possible but no later than once a week", on every RA connection ([CAMP122] §3 item 10) | `v2xw-engine/src/phase2.rs:349` `CRL_FETCH_INTERVAL_S` 1 h over cellular | 04 §4.2 week run | MATCHES (inside the bound) |
| I7 | The RA serves the composite CRL (every series + home CTL) ([SMP] §4.4.8) | Engine vehicles are charged `crl_bytes` = envelope + CRLG certificate + 40 B per entry (Brecht's design figure, `msg.rs` `crl`); the proto flows charge the real `SecuredCrl` length | 04 §5: real 33.1 B/entry at 10,000 entries; one-entry `SecuredCrl` 328 B | DEVIATION: at 10,000 entries the engine over-charges by about 20 %, and it omits the CTL and hash CRLs in the composite. Airtime and Uu bytes of CRL delivery are approximate by that much |
| I8 | Overdue: 2 weeks past `nextCrl`, distrust everything and warn ([CAMP122] Table 67) | `v2xw-node/src/rxval.rs:89` `OVERDUE_CRL_TOLERANCE_S` | `rx_validation.rs::two_weeks_past_next_crl_every_message_is_refused` | MATCHES |
| I9 | RSU/satellite broadcast is pass-through: the RSU does not re-sign | `run/crl.rs` `CrlAirBroadcast` | `revocation_path.rs::the_two_distribution_paths_are_reported_separately` | MATCHES |
| I10 | `If-Modified-Since` and 412 on the CRL/CTL/CCF downloads ([SMP] §4.3.2) | The device fetches the whole CRL each poll (see A16) | — | DEVIATION: a 412 "already current" answer is not modelled; poll bytes are over-counted when nothing changed |
| I11 | CTL: m-of-N electors, quorum 3 of 5; US `CtlSeriesId` `0348000000000000` ([SMP] §6.1) | `scms/governance.rs` | `governance` quorum tests; published CTL series installs in order (04 row 31) | MATCHES |
| I12 | CRL distribution through the RA is the default; RSU broadcast and MBMS are options | `deployment.rs` options | `deployment.rs` tests | MATCHES |

## J. Choices made where the real behaviour is unpublished

- **Stale CRLs (R2).** A device refuses a different CRL of the same `(crlCraca, crlSeries)` unless its counter is higher. The counter is `crlSerial`, or `iRev · 256 + indexWithinI`, with `issueDate` as a tie-break. A repeat of the held CRL is not a refusal. Basis: 1609.2 defines both counters to increase with every CRL of the series. No published device specification says what to do with a lower one.
- **Every chain (B4).** The RA returns every chain pair of the blocklisted enrolment. Basis: the RA blocklists the enrolment, not a request ([CAMP122] SCMS-507, SCMS-1204), and the MA's decision is to revoke the device. The MA still sees only opaque LCIs, and the LAs still learn nothing beyond the chains they hold.
- **Counter order.** Within one publication, the full CRL takes the lower `indexWithinI` and the delta the higher one. A device that holds the delta has the same state as the full, so refusing the full afterwards loses nothing.

## K. Faults injected

All five faults were applied at once, so one build covered them. They sit in disjoint code paths, and each test was checked to fail at its own assertion. The tool follows the `tools/mutate/mutate.py` contract: the bytes are restored, then the mtime is set to now. The tree was clean afterwards, and a clean run of the suites followed.

| Fault | Test | Result |
|---|---|---|
| R2 device: the stale check is disabled (`false && …` in the `CrlDownload` handler) | `redteam_pki::a_replayed_older_crl_is_refused_and_the_newer_revocation_holds` | RED: "v1 is refused as older than v2" (0 ≠ 1) |
| R2 node: back to an `issueDate`-only rule | `rx_validation::an_older_crl_full_or_delta_is_stale_even_in_the_same_second` | RED: "an earlier list of the same second" (accepted) |
| B4: the RA returns only the reported request's chains | `redteam_pki::every_chain_of_a_revoked_enrolment_is_revoked` | RED: "one entry per chain pair" (1 ≠ 2) |
| H4: the delta shares the full's `indexWithinI` | `crl_issuance::a_second_revocation_issues_a_full_and_a_delta_crl` | RED: "full 1, then its delta 2" (1 ≠ 2) |
| D14: no blocklist check when the weekly job runs | `pregeneration::a_weekly_job_armed_before_the_blocklist_generates_nothing` | RED: "the job ran into the blocklist" (0 ≠ 1) |

Clean runs before and after the faults passed. They covered `v2xw-proto` (lib and 13–16 integration suites: redteam_pki, crl_issuance, pregeneration, resolution, revocation_path, lockstep, ma_cases, flows, threshold, governance, privacy, wire_sizes, pki_view, plus appcert, mbr and enrolment on the second run) and `v2xw-node` rx_validation.

## L. Not run here

- The engine regression digests (`v2xw-engine` `phase2`, `scms_scale`). They are pinned for the default single-chain path: no size changes, and B4 changes bytes only when a device has two requests. The `indexWithinI` change does alter the bytes of every delta CRL, and its signature. The merge agent should run them and re-bless, with that reason, if the record digest moves.
- The conformance run (06): its oracle does not check `indexWithinI` or freshness. An independent oracle rule for both is owed (this package touches the Rust, so by 00 §4.1 it may not extend the oracle).
