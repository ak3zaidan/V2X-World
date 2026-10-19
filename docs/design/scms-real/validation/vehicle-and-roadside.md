# Validation: on-vehicle and roadside behaviour

Area: what an OBU, a pedestrian's handset and an RSU do with SCMS credentials. That covers
receiver validation, verification policy, signer identifiers and certificate attachment,
P2PCD, pseudonym pools and changes, misbehaviour detection and the device report queue,
VRU devices and RSUs. The checklist is built from the research set (01, 02 §11–12, 03 §6,
§8, §10–11) and the documents it cites. Code comments were not used as sources. Branch
`wf_fcf1da54-4cd-5`, base `0f7ababf`.

Source keys are the ones in 03 §0: [SMP] SCMS Manager 1609.2.1 Profile v1.0.3; [SM-MBR] SCMS
Manager CV-Pilot misbehaviour report spec v1.0 / ASN v1.1; [CAMP122] CAMP EE Requirements
1.2.2; [PRIMER] USDOT SCMS Technical Primer FHWA-JPO-19-775; [CVP-P2] CV Pilot lessons
learned 2018; [EE-REQ] SCMS Manager EE Requirements v2.04; 1609.2 means IEEE 1609.2-2022
(clause 8 as written by 1609.2a-2017).

## Summary

| Verdict | Count |
|---|---|
| MATCHES | 41 |
| FIXED | 7 |
| DOCUMENTED DEVIATION | 8 |
| UNVERIFIABLE | 11 |

Six deviations were fixed on this branch. Each fix has a test that was shown to fail with the
fault put back (§3):

1. **PSM permissions (benchmark §9.1).** Pedestrian handsets requested the vehicles'
   permission profile, `{0x20, 0x26}`. Every receiver refused every PSM (`psid-not-permitted`),
   and the `signatureVerification` detector reported the sender. Now each device has its own
   permission profile, carried through the `EeRaCertRequest` and the RA→ACA request: a handset
   gets `{0x26, 0x27}` (PSM and misbehaviour report) and a vehicle never gets 0x27.
2. **Unverifiable evidence at the MA.** A report whose only reason is a bad signature names a
   certificate that never signed the message. Anyone can put a digest on a forged frame, so
   such reports let one transmitter frame any honest certificate. The MA now sets them aside
   and counts them (`reports_evidence_unverifiable`).
3. **Stated accuracy of PSMs and VAMs.** Detectors judged a PSM against no stated accuracy
   (`accuracy_unjoined`), although a PSM carries J2735 `PositionalAccuracy`. They now read it,
   and a VAM's confidence ellipse is read the way a CAM's is.
4. **PSM `msgCnt` across a pseudonym change.** A handset kept counting `msgCnt` across a
   change, which links the old and new pseudonyms. It now restarts at a random value, as the
   vehicle's BSM already did.
5. **US pseudonym-change rules.** `time: 300` was a 5-minute change alone. J2945/1 CERTCHG
   needs 5 min **and** 2 km. The rule is now selectable as `j2945-1` (and the NYC pilot's
   `nyc-pilot`, 2 km **or** 5 min), and the Midtown benchmark uses it.
6. **Report frames checked as beacons.** A neighbour's detector checked a report uploaded
   over the air as if it were the uploader's own beacon. It now checks safety messages only.

Benchmark re-measurement, first 60 s of `midtown-scms-benchmark-quarter.yaml` (live, no
fast-forward, seed 0xBE7C0001), §4: MBR airtime went from 29.3 % to 0 % of all
airtime (15 honest reports in 60 s, none over the air), and receivers refused no PSM.

## 1. Checklist

Verdicts: **M** MATCHES, **F** FIXED, **D** DOCUMENTED DEVIATION, **U** UNVERIFIABLE.

### 1.1 Receiver validation (1609.2 §5.1.2, §5.2; 02 §12)

| # | Real-world rule | Engine | Evidence | Verdict |
|---|---|---|---|---|
| R1 | The signer chains, link by link, to a root on the installed CTL (§5.1.2.1, §5.1.2.3) | `v2xw-node/src/rxval.rs:740` `validate_signer` → `v2xw-sec/src/chain.rs` | `rx_validation.rs::rule_untrusted_root`, `an_unknown_issuer_concludes_nothing`; `chain_rules.rs`, `chain_xcheck.rs` (oracle cross-check) | M |
| R2 | Each subordinate's permissions are covered by an issuer `certIssuePermissions` entry whose `eeType` allows `app` (§5.1.2.4) | `chain.rs` `PermissionNotCovered`, `EeTypeNotAllowed` | `rule_issuer_lacking_cert_issue_permissions` | M |
| R3 | SSP rules: opaque SSP needs `sspRange all` or identical; bitmap rule (§6.4.28; S1 note) | `chain.rs` `SspNotCovered` | `chain_rules.rs` SSP cases | M |
| R4 | Chain length: `mcr_i ≤ mcr_s+1` and `mcr_i+clr_i ≥ mcr_s+clr_s+1`; at most 8 certificates (PICS) | `chain.rs:303` `chain_length_ok`, `MAX_CHAIN_LENGTH` = 8 (`chain.rs:58`) | `rule_chain_length_exceeded` | M |
| R5 | Validity and region inside the issuer's (§5.1.2.4) | `chain.rs` `ValidityNotContained`, `RegionNotContained` | `chain_rules.rs` | M |
| R6 | Every certificate in the chain is current (§5.1.2.4a, "strongly recommended") | `chain.rs` `Expired` | `rule_generation_time_outside_validity_and_expired` | M |
| R7 | The SPDU's PSID is in the signer's `appPermissions` (§5.2) | `rxval.rs:771` | `rule_psid_not_in_app_permissions`; engine `vru.rs::under_the_scms_a_handset_signs_psms_that_every_receiver_accepts` | M (the check). Its input was wrong for PSMs, see V1 |
| R8 | `generationTime` inside the signer's validity (§5.2, SDEE profile) | `rxval.rs:777` | `rule_generation_time_outside_validity_and_expired` | M |
| R9 | `generationLocation` inside the signer's circular, rectangular or polygonal region (§5.2) | `rxval.rs:786` | `rule_generation_location_outside_region` | M |
| R10 | Identified (country) regions checked against a location | counted in `RxStats::region_unchecked`, not refused | rxval module doc | D: the build has no country polygons. US pseudonyms carry no region in the published profile ([SMP] §4.4), so on the benchmark `region_unchecked` = 0 |
| R11 | Published US RA profile `{all, minChainLength 0, chainLengthRange 0}` accepted | `ChainPolicy::us_device` (`chain.rs:155`), findings counted in `tolerated` | benchmark: 1,780 tolerated in 60 s | D: the profile as published breaks the strict §5.1.2.4 rule for the EE under the RA. A real device accepts it, so the engine tolerates it and counts each case |
| R12 | Hash-CRL revocation (series 2, 3, 4, 256) matched on `crlCraca` and `crlSeries` (§5.1.3; 03 §10) | `rxval.rs:827` | `a_revoked_aca_revokes_every_pseudonym_under_it` | M |
| R13 | Linkage revocation reads the certificate's own `linkageData`, matches `cracaId`/`crlSeries`, walks the seeds `iCert − iRev` steps for j = 0…jmax−1 (02 §11.2) | `stores.rs:565` `CrlGate` | `a_linkage_entry_revokes_only_a_pseudonym_of_the_same_craca`; CAMP `lv.txt` vectors (02 §10.1) | M |
| R14 | A CRL is accepted only if signed by a CRL signer with matching `CrlSsp`, chained to the root (03 §10) | `rxval.rs` `verify_secured_crl` | `a_signed_crl_is_accepted_and_one_altered_byte_is_refused_by_the_node`, `a_crl_signer_whose_certificate_the_ica_did_not_sign_is_refused` | M |
| R15 | Overdue rule: when the CRACA's CRL is more than `overdue_CRL_tolerance` = 2 weeks past `nextCrl`, its messages are refused ([CAMP122] Table 67) | `rxval.rs:89` `OVERDUE_CRL_TOLERANCE_S` = 1,209,600 s, applied at `rxval.rs:767` | `two_weeks_past_next_crl_every_message_is_refused` | M |
| R16 | A device keeps validated CA certificates and validates a pseudonym certificate once; later SPDUs cost only their signature check (1609.2 §5.2: the chain part does not change until revocation does) | `rxval.rs` `EeCache`, 4,096 entries (`rxval.rs:97`), LRU | `first_sight_pays_the_ee_link_and_reconstruction_and_a_repeat_only_the_signature`, `the_validated_cache_evicts_the_least_recently_used_signer` | M. The size is an implementation choice (U for the number; Vanetza is cited as a stack that does the same) |
| R17 | The cache is invalidated when the certificate leaves the peer cache, and when a CRL governing it changes | `rxval.rs:584` `forget`; `install_crl` invalidation | `a_certificate_the_peer_cache_forgets_is_validated_again`, `a_fresh_crl_sends_the_signers_it_governs_back_to_first_sight` | M |
| R18 | Validation work is priced per operation actually performed | `rxval::price` through `PrimitiveCatalogue::price_all`; `unpriced` counter | benchmark: `unpriced` = 0 in 60 s | M |

### 1.2 Verification policy, signer identifier, certificate attachment

| # | Real-world rule | Engine | Evidence | Verdict |
|---|---|---|---|---|
| S1 | A receiver may verify on demand or prioritise when load exceeds capacity (no single policy is mandated by J2945/1) | `v2xw-node/src/policy.rs` `VerifyAll`, `OnDemand`, `Prioritized` | `policy.rs` unit tests; engine `vru.rs::a_vru_device_runs_the_scenarios_verification_policy` | U: deployed OBUs' policies are not published. All three are modelled, and each is labelled on its card |
| S2 | J2945/1: full certificate on a cadence and a digest otherwise. `CertAttachInt` 450 ms (constants.md, from Rostami et al. 2018 Table 1) | `v2xw-sec/src/envelope.rs:158` `SAE_450MS` (the node default) | `envelope.rs` `choose` tests | M for the default. The Midtown benchmark overrides it to 1,000 ms ("once a second"). The two published readings disagree and J2945/1 itself is paywalled. Re-check the scenario value when the primary text is read |
| S3 | The first message under a new certificate carries the certificate | `SignerIdPolicy::choose` (`last_attached` None → certificate), keyed per certificate | envelope tests | M |
| S4 | A certificate is attached in the next BSM when a new neighbour is heard | `secure.rs:542` `note_new_neighbour` (`signer_id_policy.on_new_neighbour`) | `runtime_behaviour.rs` | M. It is optional, and off in the benchmark |
| S5 | A digest that misses the peer cache cannot be verified until the certificate arrives | `runtime.rs` `learn_or_request` → `record_p2pcd_request`; inline P2PCD `on_unknown_signer_digest` | `unknown_signer.rs` | M |
| S6 | The full-certificate share in the benchmark was 25 % against 10 % at 1 Hz (benchmark §9.5) | P2PCD inline answers and new-neighbour attachment add certificates during the fill | benchmark §9.5 | U: plausible during the fill. To be re-measured in steady state |

### 1.3 P2PCD (1609.2 clause 8)

| # | Real-world rule | Engine | Evidence | Verdict |
|---|---|---|---|---|
| P1 | Out-of-band request: an unknown issuer starts a `p2pcdLearningRequest` (HashedId3) in the next SPDU, one per SPDU (§8.2.4.1.1, §6.3.9) | `v2xw-node/src/p2pcd.rs` `on_unknown_issuer`, `take_header` | `p2pcd_exchange.rs::a_new_aca_certificate_propagates_out_of_band`; `p2pcd_air.rs` | M |
| P2 | `requestActiveTimeout` and `observedRequestTimeout` 0.25 s (Annex C.2.1.3.1) | `v2xw-sec/src/p2pcd.rs:124` `RECOMMENDED` | `p2pcd.rs` unit test pinning the values; `a_request_is_sent_once_per_request_active_timeout`, `a_peers_request_suppresses_and_withdraws_ours` | M |
| P3 | A responder answers only for CA certificates in the chain of a certificate it signed with within 60 s (`currentlyUsedTriggerCertificateTime`) | `note_signed_with` | `only_a_recently_used_chain_is_served` | M |
| P4 | Random back-off up to `maxResponseBackoff` 0.25 s; no response once `responseCountThreshold` (3) responses were observed; `responseActiveTimeout` 0.25 s | `due_responses` | `a_responder_backs_off_and_stops_at_the_threshold`, `several_responders_back_off_and_stop_at_the_threshold`, `a_response_leaves_at_its_backoff_instant_not_at_a_step` | M |
| P5 | Inline flavour: requests go into `inlineP2pcdRequest`; the responder attaches its own certificate or one requested CA certificate in `requestedCertificate` (§8.2.4.1.2, §8.2.4.2.3) | `take_header` | `a_new_aca_certificate_propagates_inline`, `inline_answers_with_the_own_certificate_first_then_a_ca` | M |
| P6 | Everyone learns from observed responses (§8.2.1) | `NodeSecurity::p2pcd_learn`, validated against an installed root before use | `a_forged_response_is_refused_and_the_request_goes_on` | M |
| P7 | Maximum concurrent responses and inline entries are implementation-specific | 8 and 8 | — | U: labelled ASSUMPTION |
| P8 | Which flavour US devices run | out-of-band default | — | U: no US profile publishes it |

### 1.4 Pseudonym pool and change

| # | Real-world rule | Engine | Evidence | Verdict |
|---|---|---|---|---|
| C1 | 20 concurrently valid pseudonyms per week ([PRIMER] p.8; [CAMP122] Table 67 `certs_per_i_period`) | `v2xw-proto/src/scms/params.rs:532` | `provisioning_b3a.rs::the_presets_carry_the_cited_numbers`; benchmark `cert_pool_valid` 20 | M |
| C2 | Validity 1 week + 1 h of overlap (10,140 min) ([SMP] §2.1.17 Table 2) | `params.rs:531` | `stores.rs::a_change_in_the_overlap_moves_to_the_new_period` | M |
| C3 | A device on the road holds its current and next week (top-up by `.info` and batch download, `ra-maxReqs` 1) | `DEVICE_POOL_PERIODS` = 2 (`scms/mod.rs:1100`), `topup_below_periods` 1 | benchmark §5: 0 starved, top-up at the rollover | M |
| C4 | J2945/1 CERTCHG: change once 5 min have passed **and** the vehicle is ≥ 2 km from the last change ([CVP-P2] item 25; [PRIMER] pp.8–9) | `RotationPolicy::J2945_1` existed in `stores.rs` but no scenario could select it. `time: 300` gave the 5-minute condition alone | new `scms_constants.rs::the_us_pseudonym_change_rules_are_selectable_with_both_conditions`; `stores.rs::the_two_published_rotation_rules_differ_and_both_are_expressible` | **F**: `security.pseudonym_change.strategy: j2945-1` (`wiring.rs` `rotation_policy`). The Midtown benchmark now uses it |
| C5 | NYC pilot: every 2 km **or** 5 min, whichever comes first ([PRIMER] pp.8–9) | `RotationPolicy::NYC_PILOT` | same test | **F**: `nyc-pilot` |
| C6 | The 2 km is "from the last change point" | measured as distance travelled since the change | — | D: travelled distance ≥ displacement, so a change can come slightly earlier on a winding route |
| C7 | At a change every identifier changes together: certificate, temporary ID (first 4 octets of the HashedId8), link-layer address, `msgCnt` restarted (J2735 lets `MsgCount` restart on an identity change) | vehicle: `runtime.rs` (restart from `node/identity-change`); handset: **counted on** before this branch | new `vru_device.rs::a_pseudonym_change_restarts_the_psm_msg_count` | **F** for the handset (`vru.rs` `step_timed`). M for the vehicle |
| C8 | Pool rotation goes through the whole pool before reusing a pseudonym; an exhausted or revoked pool stops transmission ([CAMP-EE §2.2.10.2]) | `stores.rs:275` `rotate` | `rotation_goes_round_the_whole_pool_before_reusing_a_pseudonym`, `a_revoked_own_certificate_leaves_the_node_unable_to_sign`, `vru_device.rs::a_device_with_no_credential_stops_transmitting` | M |
| C9 | Silent periods: not required by J2945/1 | `silent_period_s` optional, off by default | `pseudonym_policy.rs` | M (off). The option is for research |
| C10 | Path history across a change | not cleared | — | U: whether J2945/1 clears `PathHistory` at a change could not be checked (paywalled). A tracker can chain points across the change. Recorded for the privacy study |
| C11 | Pseudonym-change rule for J2945/9 handsets | the scenario's rule, shared with vehicles | — | U: no public source. Under `j2945-1` a pedestrian rarely reaches 2 km |

### 1.5 Misbehaviour detection, reports and the device report queue

| # | Real-world rule | Engine | Evidence | Verdict |
|---|---|---|---|---|
| M1 | US report format: `EeMaApplicationSpecificMisbehaviorReport`, PSID 0x26, application PSID 0x20; types 1VInconsistent (ConstantPosition, RandomPosition with 1 m drift), Implausible (MaxSpeed 90 m/s, MaxAcceleration 10 m/s², BrakeAcceleration), ObsImplausible off ([SM-MBR] §1, §2.3) | `v2xw-proto/src/scms/mbr.rs`; engine mapping `phase2.rs` `us_subtype` | `tests/mbr.rs` (asn1tools oracle `tools/pdu_check/check_mbr.py`); `scms_wiring.rs::detectors_file_real_us_reports_that_the_ma_opens_and_verifies` | M for the format |
| M2 | Only detections that have a v1.1 subtype can be filed as a US report | other legacy-12 checks go on a "modelled" path (`reports_mbr_unmapped`) | benchmark: 20,421 of 23,112 reports unmapped before the fix | D: kept so that attack studies of the research detectors still reach the MA. Each one is counted and its evidence is labelled "no SM-MBR v1.1 subtype". A strict-US mode that drops them is the realistic setting still to add |
| M3 | Detector thresholds compared with [SM-MBR] §2.3 defaults: legacy-12 acceleration 12 m/s² against SM-MBR MaxAcceleration 10 m/s²; no MaxSpeed (90 m/s) check in legacy-12 | `v2xw-threat/src/detect.rs:477` | — | D: the benchmark runs the research suite (VeReMi/F2MD), not the CV-pilot detector. The report claims SM-MBR's 10 m/s² threshold for a check that fired at 12 m/s² |
| M4 | A position check allows for the sender's stated accuracy (CaTch, Kamel et al. 2019); a PSM states J2735 `PositionalAccuracy` | BSM and CAM joined. **PSM and VAM were not** (`accuracy_unjoined`) | baseline: 33,943 of 82,148 checked messages unjoined. After the fix: 0 of 81,973 | **F**: `run.rs` notes PSM and VAM accuracy; `phase2.rs` `broadcast_accuracy_95_m` reads a VAM like a CAM |
| M5 | The MA processes a report only if its evidence verifies (the evidence is the accused's signed BSMs and their certificate, [SM-MBR] §1; "valid reporter signature" and valid chains are processing preconditions, [5GAA] (263)) | before: a report with only `signatureVerification` was counted against the named certificate | new `security_lifecycle.rs::the_ma_sets_aside_a_report_whose_only_evidence_is_a_bad_signature`; benchmark revocation of honest devices | **F**: `phase2.rs` `ingest_arrivals` sets them aside (`reports_evidence_unverifiable`) |
| M6 | The reporter still detects and files a report about a message that failed verification | filed (modelled path) | `a_forged_signature_is_on_the_air_and_reported` (unchanged) | D: a CV-pilot OBU has no subtype for it and would not file it (see M2). The MA now discards it, so it can no longer frame a certificate |
| M7 | One report per HTTPS POST to the RA ([SM-MBR] §4); no bundling | one upload per report | `scms_wiring.rs` | M |
| M8 | Store: weight 1.0, ×0.9 daily at 05:00 Eastern, highest weight first, newer first on ties; over quota delete the lowest (older first); unsent reports older than 1 week may be deleted ([SM-MBR] §2.2) | `mbr.rs` `MbrQueue` (`DECAY_HOUR_ET` = 5, `max_age_s` 7 d); engine `ReportStore` | `phase2::report_store_tests`; `stores.rs::the_outbox_sheds_reports_after_a_week` | M |
| M9 | Store quota | 64 KiB (`phase2.rs:1034`) | — | U: [SM-MBR] leaves it device-specific. ASSUMPTION |
| M10 | Report rate per reporter and accused | at most one per second (`REPORT_INTERVAL_S`, `phase2.rs:343`) | — | U: no published US rate limit. The value is the legacy engine's step |
| M11 | Upload "whenever in range of an RSU offering SCMS connectivity" ([SM-MBR] §4), over IPv6 on a service channel advertised by WSA (1609.3) | relayed uploads are broadcast frames on the safety channel (ch 172), 1,162 B each | baseline: 4,306 relay frames = 6.89 s of 23.52 s airtime | D: the engine has no service channel. The report airtime counted on ch 172 is an upper bound. In a real deployment it lands on an SCH |
| M12 | Report signed under the reporter's pseudonym and encrypted to the MA whose `MaSsp.relevantPsids` covers 0x20; upload authenticated by an enrolment-signed `Download-Req` ([SM-MBR] §3–4) | `mbr.rs` `secure_mbr`, `upload_header` | `tests/mbr.rs`, privacy split tests | M |
| M13 | RSU reports: an RSU is a reporter like a vehicle | `arm_rsu_detector` | — | U: whether deployed RSUs run MBD and report is not published |
| M14 | Detection reads safety messages, the BSM for the US checks ([SM-MBR] §2.3). A relayed report, a CRL, a P2PCD response or a WSA states nothing about its sender's motion | before: every delivered signed frame was checked as a vehicle beacon. A report uploaded over the air was checked with no stated accuracy, and a handset's upload was checked against the vehicle lane map | new `security_lifecycle.rs::a_relayed_report_is_not_checked_as_a_beacon`. Baseline: 545 `mapOffRoad` verdicts in 60 s, which match the 686 verified messages with no stated accuracy (MBR frames) | **F**: `phase2.rs` `detect` skips them (`messages_not_checked`) |

### 1.6 VRU devices

| # | Real-world rule | Engine | Evidence | Verdict |
|---|---|---|---|---|
| V1 | A J2945/9 handset signs PSMs under PSID 0x27 (`psid-vulnerable-road-users-safety-applications`, IEEE 1609.12). Its certificates must permit 0x27 and, since it files reports, 0x26. A vehicle OBU's do not permit 0x27. [EE-REQ] p.12 lists "PSM devices" as device group 1 | before: every device requested the fleet profile `{0x20, 0x26, + vehicle services}`, so every PSM was refused | new `pdu_layer.rs::a_device_profile_reaches_the_aca_and_every_certificate_it_installs`, `vru.rs::under_the_scms_a_handset_signs_psms_that_every_receiver_accepts` | **F**: `DeviceState::psids`, `ScmsRun::set_device_psids`, `ScmsState::pseudonym_psids_of` (EE request and RA→ACA); `LifecycleParams::vru_psids`; `Phase2::provision(.., vru)`. The exact handset profile is an ASSUMPTION (not published) |
| V2 | The PSM carries `id` = the first 4 octets of the pseudonym's HashedId8, `msgCnt`, position, accuracy and speed (J2735) | `vru.rs` `encode_payload` | `vru.rs::equipped_pedestrians_send_psms_that_other_nodes_hear` | M |
| V3 | PSM rate | 1 Hz, 100 ms floor | `vru_device.rs` | U: J2945/9 rate rules not read (paywalled). The design set's default |
| V4 | A handset with no valid credential stops transmitting | `vru.rs` | `a_device_with_no_credential_stops_transmitting` | M |
| V5 | Receivers drop the vehicle-kinematic and map checks for a self-declared VRU | `detect.rs` VRU branch | `detectors.rs` | M (design choice, cited CaTch/F2MD) |

### 1.7 RSUs

| # | Real-world rule | Engine | Evidence | Verdict |
|---|---|---|---|---|
| U1 | An RSU signs with an RSU application certificate (CRL series 3, HashedId10), renewed on its own cadence ([SMP] §2.1.21; 03 §7) | `appcert.rs` path, `rsu_app_certs_preloaded` | `rsu_certs.rs` | M |
| U2 | RSU roles: SPaT/MAP broadcast, provisioning proxy, report forward, CRL repeat (RSU spec v4.1 §2.2; 01 §8) | `v2xw-node/src/rsu.rs` `RsuRole` | `rsu_runtime.rs`, `rsu.rs` unit tests | M |
| U3 | RSUs verify every frame they hear | `wiring::build_rsu` `verify-all` | `rsu_runtime.rs` | U: deployed policy not published |
| U4 | Store-and-forward when the backhaul is down; items age out after a week | `ForwardQueue` | `the_forward_queue_holds_then_drains`, `a_held_item_ages_out_after_a_week` | M (the week is an ASSUMPTION matching [SM-MBR] §2.2) |
| U5 | WSA advertising the SCMS service | no WSA encoder, so the frame is unsized and counted | `spat_and_map_are_sized_by_the_size_model_and_a_wsa_is_not` | D: no WSA, so an IP service channel cannot be advertised either (see M11) |

## 2. The misbehaviour-report flood (benchmark §9.1): cause and fix

Baseline, first 60 s, release binary `a71c9f3419b1d486`: 82,148 messages checked, 27,049
verdicts (26,455 `signatureVerification`, 545 `mapOffRoad`, 49 `positionSpeedInconsistency`,
13 `positionJump`), 23,112 reports, all about honest devices. 32,052 receptions were refused
`psid-not-permitted`. MBR airtime was 6.89 s of 23.52 s (29.3 %), and the MA made 73 revoke
decisions about honest devices.

The causes, in order of weight:

1. **The PSM permission defect (V1)**, 97.8 % of verdicts. Every PSM failed `appPermissions`,
   the receiver handed the detector an `Invalid` message, and `signatureVerification` fired
   at the hard-fail score on each one.
2. **The MA acted on unverifiable evidence (M5).** This is what turned those reports into
   honest revocations.
3. **Thresholds against stated accuracy (M4).** PSMs had no stated accuracy joined. Vehicles'
   BSMs did. The remaining `mapOffRoad` verdicts are vehicles reported off the motor-vehicle
   lanes by more than 15 m beyond their stated 95 % radius. That is the urban-canyon case the
   research suite is known to raise, and it is left as a measured false-positive rate.
4. **Rate limit and bundling.** One report per (reporter, subject) per second and one POST
   per report. Neither has a published US value (M10, M7), so neither was changed.
5. **The channel the uploads use (M11).** This is a modelling limit that overstates
   safety-channel airtime.

After the fixes (same scenario and seed, binary built from this branch with the test
profile, commit `80522078`, which predates fix M14; M14 cannot change this run because no
report went on the air):

| Quantity (60 s) | Baseline | This branch |
|---|---|---|
| Receptions refused `psid-not-permitted` | 32,052 | 0 |
| Verification states seen by detectors | Verified 48,891, Invalid 28,715, Unverified 4,542 | Verified 77,391, Invalid 0, Unverified 4,582 |
| Verdicts | 27,049 (signatureVerification 26,455, mapOffRoad 545, positionSpeedInconsistency 49, positionJump 13) | 21 (positionSpeedInconsistency 21, positionJump 6; one verdict can fire both) |
| Reports filed | 23,112 (14,888 Uu, 4,306 over the air, 3,285 from RSUs) | 15 (8 Uu, 0 over the air, 2 from RSUs; 3 real SM-MBR, 7 unmapped) |
| Honest devices reported | 147 | 4 |
| MA revoke decisions (all honest) | 73 | 0 |
| Messages with no stated accuracy | 33,943 | 0 |
| Airtime, total | 23.52 s | 16.63 s |
| Airtime BSM / PSM / MBR | 13.59 / 3.05 / 6.89 s (57.8 / 13.0 / 29.3 %) | 13.59 / 3.05 / 0 s (81.7 / 18.3 / 0 %) |

The BSM and PSM frame counts are identical (41,894 and 8,280), so the traffic and radio are
the same. Only the security outcome changed. The remaining 21 verdicts are honest GNSS
behaviour seen by the research suite. They are 2.6 per 10,000 checked messages, and none led
to a decision in the window.

## 3. Faults injected

| Test | Fault | Result |
|---|---|---|
| `pdu_layer.rs::a_device_profile_reaches_the_aca_and_every_certificate_it_installs` | `ScmsState::pseudonym_psids_of` returns the fleet list for every device | RED: the handset's certificates list `[32, 38]` |
| `vru.rs::under_the_scms_a_handset_signs_psms_that_every_receiver_accepts` | `Phase2::provision` without `set_device_psids` | RED: 1,831 `psid-not-permitted`, 1,672 `signatureVerification` verdicts, 1,439 reports |
| `security_lifecycle.rs::the_ma_sets_aside_a_report_whose_only_evidence_is_a_bad_signature` | `ingest_arrivals` without the evidence check | RED: 48 at the MA, 0 set aside |
| `vru_device.rs::a_pseudonym_change_restarts_the_psm_msg_count` | no `restart_psm_count` at a change | RED: `msgCnt` 3 -> 3 (counted on) |
| `scms_constants.rs::the_us_pseudonym_change_rules_are_selectable_with_both_conditions` | `rotation_policy` without the `j2945-1` / `nyc-pilot` arms | RED: `j2945-1` gave 5 min alone |
| `security_lifecycle.rs::a_relayed_report_is_not_checked_as_a_beacon` | `detect` without the non-station skip | RED: `messages_not_checked` 0 with 89 reports relayed over the air |

Faults were applied and restored by a script that restores the bytes and sets each file's
mtime to now (the `tools/mutate/mutate.py` rule). The working tree was clean after each
run.

## 4. Runs

| Run | Binary | Wall / CPU | Peak |
|---|---|---|---|
| Baseline, 60 s live | `target/release/v2xw` copied, sha256 prefix `a71c9f3419b1d486` (main, 2026-10-18 17:00) | 154 s / 128 s | 341 MB |
| This branch, 60 s live | `cargo build --profile test -p v2xw-cli` at `80522078` (workspace crates at test-profile opt levels) | 453 s / 326 s | 278 MB |

Scenario: `meta.base: scenarios/real-world/midtown-scms-benchmark-quarter.yaml`,
`time.duration_s: 60`, `time.fast_forward: []`, and the world path pointed at the main
checkout's `worlds/cache/manhattan.osm.xml`. Run with `--record-level metrics --no-verify`
through the run lock. The baseline's base file still had `strategy: time`, this branch's had
`j2945-1`. Neither changes a pseudonym inside 60 s (`pseudonym_changes` = 0 in both).

Agents may not build `--release` (owner's rule), so the branch's run used the test-profile
binary. The engine is deterministic and the profile changes speed, not results. A release
re-run belongs to the merge.

Suites run clean on this branch (test profile, after every fix): `v2xw-proto` `pdu_layer`;
`v2xw-node` `rx_validation`, `p2pcd_exchange`, `vru_device`, `runtime_behaviour`;
`v2xw-engine` `security_lifecycle` (15), `vru` (6), `scms_wiring` (4), `unknown_signer`,
`p2pcd_air`, `scms_constants`, `scenario`. Not run: the engine's lib tests, `phase2.rs`,
`scms_scale` (regression digests), and the remaining engine suites.

## 5. Open items for the merge agent

1. **Regression digests.** `tests/regression/phase2-path.json` and `i-s1-phase2-60s.json`
   may change: PSM/VAM accuracy is now noted, the MA sets aside signature-only reports,
   and the detectors skip report frames. Re-bless them after the merge if
   `scms_scale` reports a change. They were not run here because of the build budget.
2. **The full benchmark** (`midtown-scms-benchmark-quarter.yaml`, 172,800 s) on a release
   build. Expect the §9.1 numbers to collapse as in §2, and re-measure §9.5 (full-certificate
   share) in steady state.
3. **Strict US report profile (M2, M3, M6).** Add a setting that files only SM-MBR v1.1
   subtypes, with the [SM-MBR] §2.3 thresholds (MaxSpeed 90 m/s, MaxAcceleration 10 m/s²,
   RandomPosition 1 m), and use it as the US default once attack studies have an
   alternative. It was left out here because existing attack tests reach the MA through
   unmapped research detectors.
4. **Report uploads on a service channel (M11).** This needs multi-channel operation in the
   radio, which is outside this area.
5. **Paywalled primaries** worth buying, because they would turn U into M: SAE J2945/1
   (CertAttachInt 450 ms or 1 s; PathHistory at a change), SAE J2945/9 (PSM rate; handset
   permissions and pseudonym change).
