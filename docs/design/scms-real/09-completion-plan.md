# SCMS completion plan (2026-10-30)

The owner's goal: an SCMS that is complete end to end, tested end to end, free of known
defects, and identical to the US SCMS pipeline wherever the US system is observable, so it
can be written up in a publication. This file is the work list for the build waves that
deliver it, the decisions the owner delegated, and what was deliberately left out.

The owner's rule for every choice: follow the SCMS architecture first, then what makes
sense for a simulation; where a difference is negligible for this engine's results, skip it
and record it in §5.

Sources of the list: the rule register (06-validation-summary.md §2.1, last rewritten
2026-10-23), the expectation tables under crates/v2xw-engine/tests/expectations/, the
BUILD-STATUS sections of 2026-10-28 and 2026-10-29, and a code audit of the SCMS crates.

## 1. Defects (wave A)

| Id | Defect | Evidence |
|---|---|---|
| SC-D1 | Some RSUs air a new CRL 0.146 s after publication, before their backhaul could deliver it; some backhaul messages are counted twice | live-rsu LR.S.l240.bc4.fix.lag-min/lag-max (FAIL), LR.S.dbl.*; the ignored live_rsu.rs tests cover it |
| SC-D2 | A mid-run joiner holds the t0 CRL, which can be overdue: 755 honest refusals | scms-stress MWC.recovery (R-JOINCRL) |
| SC-D3 | CRL Generator and CRL-store entries stay 0 through 46 ks of a 2-day run | scms-final-run FB2.crl |
| SC-D4 | Answers sent to a down RA are lost: no backend-to-backend retry | scms-edge-cases E3 |
| SC-D5 | Slow GNSS drift has no streak gate, so honest devices are reported (88 cases at full scale) | scms-stress FS.mbr-airtime / FS.honest-revocations (R-USFP) |
| SC-D6 | An honest vehicle with its clock 83 min ahead is revoked | scms-edge-cases E2 |
| SC-D7 | The hybrid PQ signature is added to every SPDU even under the certificate cycle: 0 frames | scms-final-bench PQHMC.*, PQHFC.ccsig (R-CCSIG) |
| SC-D8 | The P-384 profile keeps P-256 operation costs | live-scms LS.S384.verify |
| SC-D9 | A report burst on resuming after a fast-forward; frequent backend stops in a second fast-forward | scms-final-run FB2.honest-reports (R-FFBURST), FB2.ff-ticks |
| SC-D10 | The BSM timing slip repeats a secMark; 26 truth rows at (0, 0) | scms-paper1 P1.RP.gt-vs-sm.* (TM-1), P1.VR.full-truth-pos (VR-2) |
| SC-D11 | Attack harness: CRL refusals scored as signature failures; Sybil has no onset on the release build; RandomSpeed ignores intensity; the expired-certificate host passes validity 0..MAX | live-atk F2-F5 rows |
| SC-D12 | ECA and ACA each carry two certificates; provider-b's ICA count is off | audit-gov GOV.L.onecert, GOV.L.counters |
| SC-D13 | No record shows an RSU installing a CRL or carrying its application certificate; 282 report-store order violations; privacy link claims stamped late; one unverified-with-certificate reception | audit-mbpipe AU.DIST.rsu, AU.STORE.order; audit-prov PV.rsu-cert; audit-sim-metrics RS.time-order; audit-db1-3 OA.RX.cached-verifies |
| SC-D14 | An enrolment certificate shorter than 1 h is written as `hours(0)`; `crl_version` is an entry count, not a version; server memory grows over long runs | BUILD-STATUS 10-29 engine requests 1 and 14; measure.md:73 |

## 2. Built work to finish and merge (wave B)

| Id | Work | Where it is |
|---|---|---|
| SC-M1 | MA policy: 8 presets, per-provider policy, family sweep and `ma-replay --policies` (08 §7 B1-B3); B4, the optional Python hook | branch worktree-wf_ab880052-ba6-4 |
| SC-M2 | Server sites, capacity, 1 to 5 ICA levels (08 §5-§8 C1a-C3a; C3b wip snapshot 538a10fa) | branch worktree-wf_ab880052-ba6-5 |
| SC-M3 | infoStatus device flow (WP-35 P35.2, unverified wip 13ae0463) | branch worktree-wf_0343ab94-a4a-1 |
| SC-M4 | WP-19 leftovers: the WSA MessageCodec arm, the 10 Hz WSA rate, WSAs on the control channel, the P19.5 live-rsu rows | WP-19 notes in 00-adoption-plan.md |
| SC-M5 | WP-34 SPS-aligned BSMs for LTE-V2X: the release measurement that closes lte-latency-budget | scms-matrix lte-aligned-* rows |

## 3. Real-pipeline replacements (wave C)

| Id | Today | Real US SCMS | Change |
|---|---|---|---|
| SC-P1 | The engine maps a reported digest to its device from its own provisioning records | The ACA (PCA) looks the certificate up in its own issuance table | A PCA issuance table keyed by (i, linkage value) that the lookup reads; the engine's map stays only as labelled ground truth for checks |
| SC-P2 | Cases open as `direct`; the MA-LA same-device exchange is skipped | The MA links reports through the PCA and both LAs | Every case runs the real lookup and linking path |
| SC-P3 | The MA de-duplicates decisions by the engine's device identity | The MA knows only what the PCA and LAs return | De-duplicate by linkage chain identifier / linkage value |
| SC-P4 | Before a case the MA reads the CRL Generator's unpublished entries | The MA checks the published list | Read the last published CRL |
| SC-P5 | A shared report is not re-verified by the receiving MA | Each MA verifies what it acts on | Re-verify the evidence at the receiving MA (format stays modelled: unpublished) |
| SC-P6 | The default local detector is the research Legacy12 suite; SM-MBR runs only when named | US OBUs detect and file SM-MBR subtypes | `detect/us-sm-mbr` is the default under a US deployment |
| SC-P7 | BrakeAcceleration never fires; MaxAcceleration comes from speed differences | SM-MBR §2.3 reads `brakes` and `accelSet` | Carry both fields in the receiver's observation |
| SC-P8 | Component CRLs (series 2, 256) are never issued | The US profile Table 1 has a provider CRL of revoked components | Issue them, revoke a component, and show devices refusing what it signs |
| SC-P9 | Emergency identification certificates exist but are never used | Privileged vehicles hold identification certificates | Wire them for emergency vehicles (WP-20) |
| SC-P10 | A vehicle whose provisioning fails falls back to a bootstrap stand-in with no linkage value | A device without credentials cannot sign | Remove the fallback (count the vehicle as unable to sign) |
| SC-P11 | Region checks for countryOnly / countryAndRegions fall through | The receiver checks generationLocation against the signer's region | Country polygons (Natural Earth, public domain) for USA, Canada, Mexico |
| SC-P12 | Availability and stochastic service times are hooks only | Real servers have outages and variable service times | Build both, off by default, parameters labelled ASSUMPTION |

## 4. Rule gaps (wave D) and records (wave E)

Rule gaps, each with the spec in rule-gap-specs-2026-10-27.md: gov C-9 (CTL effectiveDate
is the ballot date), C-10 (genesis sequence setting), C-11 (dropped root carried in the
test CTL), G-2 (board review deadlines), G-10 (yearly test ballot), L-8 (365-day root
termination notice), G-11 (dual signatures in a transition, labelled ASSUMPTION), L-11
(LaSsp v1, laId 4/5 as the ISS certificates); proto H10 (indexWithinI past 255, labelled
ASSUMPTION), B9 (request bytes), D11 (download-maxAge, ra-maxAge under a labelled reading),
B5 (RA acknowledgement within 1 s, measured); veh C6 (2 km from the last change point).
Observability: the 2026-10-29 UI engine requests (both lists in BUILD-STATUS).

Wave E, validation, runs after every merge, on a quiet machine, one long run at a time:
the full-fleet runs (FB4 unbroken, FB2 two days, the final bench under its memory cap), every
open and blank expectation row (live-chains, scms-paper1, live-studio), the ignored long
tests, the conformance kit, the Studio e2e specs, sensitivity sweeps over the unpublished
values of §6, and a rewrite of the rule register from current evidence (including the
relabels of rule-gap-specs §3).

## 5. Decisions the owner delegated (2026-10-30)

| Question | Decision | Why |
|---|---|---|
| Real 8 h MA review and one-business-day CRL SLA by default? | Yes on the real calendar (already so); automatic decisions on a compressed calendar (already so) | The US behaviour; a compressed week makes an 8 h review meaningless |
| SCMS through RSUs on the service channel with WSA by default? | Yes, `net.rsu_ip.air: sch`, after the P19.5 rows are measured; goldens re-blessed with the cause | USDOT RSU Spec v4.1 and SM-MBR §4: SCMS traffic rides a WSA-advertised service, safety channel 172 carries safety messages |
| Overdue-CRL scope | Keep `craca` | IEEE 1609.2's validity model is per CRACA (each certificate names its cracaId and crlSeries); CAMP122 Table 67's "all" reading stays selectable |
| RSUs run misbehaviour detection? | No (accept the DETPRIV reading of veh M13) | SM-MBR and RSU Spec v4.1 assign reporting to OBUs |
| Default BSM stamp profile | Unchanged (chain off) | Negligible for SCMS results; Paper 1 selects its profile per scenario |
| LTE-V2X BSM generation aligned to the SPS reservation by default? | Yes after SC-M5's measurement | A real LTE-V2X stack generates into its reservation; misaligned generation is what breaks the 100 ms budget |
| Hold late-stamped records to the next tick? | No | A logging artefact, explained in fix round 5; no effect on results |
| Alternating channel access by default? | No (continuous) | J2945/1 single-channel operation on 172 |
| Buy the paywalled standards (IEEE 1609.2.1 body, 1609.2 §5.3.8, J2945/1, J2945/9, J3287)? | Owner action (a purchase); until then the rows stay "unverifiable" with their labelled readings | Not something the build can do |

## 6. Skipped or kept as modelled, with the reason

| Item | Kept as | Why it is acceptable |
|---|---|---|
| Internal MA-PCA-LA-RA messages as modelled enums with derived sizes | Modelled | Their encodings (LCI, PLV request, LA keys) are unpublished; sizes are derived and charged |
| MA-to-MA sharing format | Modelled (re-verified, SC-P5) | No published spec |
| Policy file (GPF/LPF) payload layout | Engine layout inside a real signed envelope | CAMP's ASN.1 is not public |
| Threshold ACA signs with one key | Kept | Research mode only, not the US system |
| Epidemic V2V CRL exchange | Not built | Not part of the US deployment |
| Out-of-band CRL requests (missingCrlIdentifier) | Not built | US devices download CRLs from the RA; no effect on US runs |
| Delta CRLs | Off | The US profile distributes full lists |
| Unified and compact butterfly | Refused | Not deployed in the US |
| Unpublished numbers (data-centre sites, fleet shares, MA thresholds and review time, store quota, report rate, backend capacity) | Labelled ASSUMPTION | Covered by the sensitivity sweeps of wave E |
| Couriered key ceremony steps (gov C-14) | Not modelled | Off-network, no effect on a run |

## 7. Status

Dated notes go under each id as its wave lands.

### Waves C and D (SC-P1..SC-P12, the §4 rule gaps, the UI engine requests)

- 2026-10-30: build specs written in `10-completion-specs.md`, re-grepped on `5c5dfc4e`.
  Each item gives its file:line today, its design and source, accept tests each with a
  named fault, its settings with menu values, and its golden moves and size. The file ends
  with two three-agent schedules (C: real pipeline; D: rule gaps and UI requests) in which
  no two agents of a wave share a hot file.
- Refresh of `rule-gap-specs-2026-10-27.md`:
  - proto D11's request-age reading is already enforced (the 5 s SCMS-1203 check); only
    `ra-maxAge` is left;
  - proto B9 drops TLS, which is already charged;
  - veh R10 becomes SC-P11. Under ISS, end-entity certificates inherit the ICA region
    (1609.2 `region` "omitted" rule), so the check reaches every provider-a device;
  - proto F9 is proposed for §6 (EU profile only).
- Specs only: no code was changed and nothing was run.

### SC-D1

- 2026-10-30 (wave A): fixed in code before this wave (run.rs `on_flow_timer`, 2026-10-27:
  a unit in the certificate cycle relays the `RsuCrlFile` it installed, `rsu_crl_held`,
  and nothing before its first install; step 4 of `on_backend_step` books nothing down a
  cycle unit's line). Both ignored `live_rsu.rs` tests pass on `5c5dfc4e`. They could not
  see the defect: the 120 s run publishes one list. New ignored test
  `a_later_list_reaches_the_air_only_through_a_unit_that_downloaded_it`
  (`scms-lat2000-bc4-240.yaml`, lists at 80.004 and 210.004 s, a tick 0.146 s after the
  second): no unit installs and no vehicle downloads a case before its publication + 2 s
  (min 2.096 s and 2.146 s; 134 vehicle downloads of the later list). Fault: cycle units
  air `broadcast_crl()` again -> red. The frame metric `s.crl_first_frame_lag_s_*` cannot
  tell a frame's list: a unit keeps relaying the older list while its download is on the
  line, so LR.S.l240.bc4.fix.lag-max is a wrong expectation, not a defect. Rows:
  live-rsu LR.S.l240.bc4.fix.*, LR.S.dbl.unexplained, LR.S.lat2000bc3.crl-lag.

### SC-D2

- 2026-10-30 (wave A): fixed in code before this wave, by `2a14d6c4` (2026-10-24, R-JOINCRL):
  `Phase2::prerun_crl` hands a joiner the composite its RA serves at its join once that is
  newer than the `t0` snapshot, and marks its copy as of it. The 755 refusals were measured
  on `77cffb5b`, before the fix. The MWC runs on `f4982788` (release) and `a925db58+` (debug)
  refuse 0 messages of any kind, so W5 has 0 crl-overdue refusals; MW went 16,618 -> 0.
  Test `join_crl_tests::a_device_joining_after_a_publication_holds_the_newest_list` passes
  on `5c5dfc4e`. Fault: `newer = false` (every joiner gets the `t0` lists) -> red. Row
  scms-stress MWC.recovery is now fixed. Kept as modelled and noted: the joiner holds its RA's
  list from the instant it joins, not after its first connection's round trip. That is
  seconds against a two-week tolerance, so negligible.

### SC-D3

- 2026-10-30 (wave A): not an engine defect. The benchmark is honest traffic only, so it
  should have 0 entries. FB3 ran the same file to 172,800 s: 2 CRL versions (the `t0` lists),
  0 issued, 0 cases and 0 revocations, with 1,447 honest reports at the MAs (FB3.pki,
  FB3.honest-reports). No reissue is due within 2 days (nextCrl = issue + 7 d). FB2.crl now
  passes, judged on FB3. One thing found: the census keys `scms.crlg_entries` and
  `scms.crl_store_entries` count the home provider only. New diagnostic keys
  `scms.crlg_entries_all` and `scms.crl_store_entries_all` sum every provider (they reach
  no record or digest). Tests: `phase2.rs`
  `the_census_counts_the_entries_the_crl_generators_and_stores_hold` (path run, attacker
  revoked) and `with_no_attacker_nothing_is_revoked` (census 0).

### SC-D4

- 2026-10-30 (wave A): fixed in code before this wave, by `dcee6cb7` (2026-10-21). The
  E3 row was written on main `d3039b53` (10-18) and never updated. In `ScmsRun::step`, a
  message one SCMS component sends to another that is down waits in the sender's queue
  and is retried every `BACKEND_RETRY` (10 s) until the receiver is back
  (`FaultPlan::held`). A down component's own timers (the shuffle release, the weekly
  pre-generation) run when it restarts (`deferred_timers`). An outage that never ends
  still loses both. The 10 s is an ASSUMPTION: no SCMS text specifies inter-component
  retry, so SCMS-522's period is reused. All 13 `ee_retry.rs` tests pass on `5c5dfc4e`.
  Fault: component messages to a down node are dropped again -> red (see the commit).
  Row scms-edge-cases E3.ra-outage-loses-backend-answers is now fixed.
- 2026-10-30 (wave A), a gap that remained: a report another provider's MA shares with
  this one (`ScmsRun::shared_report_at_ma`) comes from a node outside this run's
  `NodeSet`. So `step` treated it as a device's request and dropped it while this MA was
  down (mod.rs `step`, the `component(d.from)` test). It is now held and retried like any
  component message. New test `provider_runs.rs`
  `a_report_shared_with_a_down_ma_arrives_when_it_is_back`. It only changes a run with an
  MA outage under report sharing, and no shipped scenario has one, so no golden moves.

### SC-D6

- **2026-10-30 (wave A): closed, no engine change needed.** What the real system does: IEEE
  1609.2 §5.2 lets the receiving SDS discard an SPDU whose `generationTime` is too far in
  the past or the future before verifying it; the BSM profile sets the window (public
  value: C2C-CC BSP RS_BSP_168, 40 ms ahead and 2 s old; J2945/1's own is paywalled, so
  ASSUMPTION for the US). A discarded BSM is unverified, and SM-MBR judges only verified
  BSMs of one certificate, so it is not evidence and its sender is not reported. No
  published rule (SM-MBR §2.3, CAMP-EE, the MA policy) revokes a device for its clock, so
  none was added. The window landed in `rxval::GenerationTimeWindow` (`8da5b7c1`,
  2026-10-21), after E2's new3 run; the honest revocations of new3 were that missing check.
  Re-run on 5c5dfc4e's engine: `edge/clock-skew-ahead` against `scms-conformance`, cases
  5 / 5, attackers revoked 5 / 5, honest revoked 0 / 0, reports about honest devices
  13 / 28; 186,368 BSMs discarded as in the future and 141,407 as too old. Fault (window
  opened to a year): reports about honest devices 13 -> 11,401. Row E2 in
  `scms/scms-edge-cases.yaml` updated.

### SC-D7

- **2026-10-30 (wave A): closed, already fixed (R-CCSIG, wave 3).** The certificate cycle
  signs a node's messages with ECDSA and checks the PQ signature once per certificate it
  learns, as the NDSS 2024 Partially-Hybrid design does (ccsig-memory §1, faults F1 and F2
  red; test `cert_cycle_signature::hybrid_mldsa44_messages_are_ecdsa_and_only_the_certificate_is_hybrid`).
  The final-bench rows were never moved after the fix; they now point at the final-run
  re-measurements: ML-DSA-44 cycle 0 -> 50,183 frames, MTU refusals 50,180 -> 2 (the
  relayed reports), BSM PSDU 814 B; Falcon-512 cycle PSDU 1,045 -> 380 B (the NDSS
  prediction); verify cost 0.630 ms against ECDSA's 0.628.

### SC-D8

- **2026-10-30 (wave A): fixed.** Cause: `HardwareProfile::priced_op`
  (`crates/v2xw-node/src/profile.rs`) charged a P-384 operation that the profile does not
  price as its P-256 sibling, at exactly P-256's cost, on every profile but
  `obu/generic-automotive-soc-no-hsm` (live-scms LS.S384.verify 0.422132 -> 0.422136 ms).
  The real system: US end entities sign BSMs with P-256; P-384 is the electors' curve
  (Elector Technical Specification §2.3) and an option 1609.2 allows per certificate, so a
  P-384 BSM run is a research variant, but it must not cost what P-256 costs. Fix: such an operation is now
  the profile's own P-256 figure times the P-384 / P-256 cost ratio on the first catalogue
  reference that publishes both (the profile's `crypto_anchors`, then the catalogue's
  device anchors), the catalogue's own `CostRatio` rule (04-models.md §9.4):
  `HardwareProfile::curve_ratio`, applied in `op_cost`, the HSM range draws
  (`server.rs`) and the cost facts, flagged derived. CRATON2 (Pi 3B Cortex-A53 anchor):
  verify x8.24, sign x4.55. Key generation has no P-384 reference row and stays at P-256
  (flagged). Test `curve_pricing::a_profile_without_p384_scales_p256_by_the_reference_ratio_and_says_so`
  replaces the test that pinned the P-256 cost; fault `curve_ratio` -> 1.0 turns it red.
  No golden moves: no pinned run signs with P-384. Recorded, not modelled: an HSM's own
  P-384 / P-256 ratio is unpublished for V2X parts (Thales T-2000 1.5, T-5000 1.0, OpenSSL
  on Cortex-A 4.5-8.9), so the software ratio of the device's own anchor is an ASSUMPTION
  and likely pessimistic for a hardware accelerator.

### SC-D9

- **2026-10-30 (wave A): closed, already fixed or not a defect.** The resume burst
  (R-FFBURST) had two causes, both fixed in finalize 4 (testing/fast-forward.md §1): the
  mobility clocks and the GNSS per-second hazards read the frozen window as elapsed time
  (`Mobility::resume_after_pause`, `GnssModel::resume_after_pause`; tests
  `calendar::the_traffic_resumes_after_a_fast_forward_as_if_no_time_had_passed` and
  `gauss_markov::a_receiver_resumed_after_a_pause_continues_its_error_as_if_no_time_had_passed`).
  FB3 at full scale: 404 reports in the rollover window against FB2's 10,357, 0 cases.
  The frequent stops are real wake sources (`stops_by_cause`): hourly conditional CRL
  polls of each cellular device, the 60 s jump cap and kernel events; R-FFSLOW does not
  reproduce after new3. FB2.honest-reports is `fixed` and FB2.ff-ticks a corrected
  expectation. Left for the validation wave: FB4.ff-stops (cost per stop).

### SC-D10

- **2026-10-30 (wave A).** TM-1 (the repeated secMark): already fixed in `cca82fcd`, the v2
  BSM timing model (a free OBU's own BSM clock; a slip k > 0 moves the position k fixes
  later and keeps the timestamp). Its re-run rows pass (`P1.TM1.sm-repeat` 0 against v1's
  17 %, `P1.TM1.gt-vs-sm.v2` 5,318 -> 5,935); the three v1-engine rows
  `P1.RP.gt-vs-sm.v10.*` are marked fixed and point at them.
- VR-2 (26 VeReMi truth rows at (0, 0)): cause, `v2xw-record` `dataset/veremi.rs`
  joined a reception to its sender's latest `gt.kinematics` record *at or before* it,
  and a vehicle's node is created when the mobility step that spawns it is dispatched
  (`Engine::absorb` -> `create_node(.., now)`), so it can send its first BSM inside that
  step, before the vehicle's first record. Fix (exporter, no engine record moves): with no
  record at or before the reception, the first record within 0.2 s after it
  (`FORWARD_JOIN_NS`, two mobility steps, ASSUMPTION) is carried back along its own
  velocity by the engine's published extrapolation rule (03-interfaces §3, I-M4), which is
  where the engine itself placed the node. Test
  `veremi::tests::a_first_bsm_before_the_first_kinematics_record_gets_the_carried_back_truth`;
  fault (no forward branch) red. Recorded, not changed: the node is live up to one step
  before its vehicle's first record; moving node creation would re-time every spawn and
  move every golden for an effect only this export sees.

### SC-D5

- **2026-10-30 (wave A): fixed (`146e87b1`).** The 88 full-scale cases were R-USFP (raw
  GNSS white noise), fixed earlier by the smoothed receiver (testing/usfp.md). What was
  left at full density: every honest `positionJump` compared two BSMs with lost ones
  between them (memory-density.md §2.2: no pair as sent breaks the rule). That came from
  `UsSmMbr::checks` (`crates/v2xw-threat/src/us_mbr.rs`), which paired a BSM with the
  previous one *received* within `max_gap_s`. SM-MBR v1.0 judges "two consecutive BSMs"
  and the ASN.1 v1.1 two BSMs "sent 0.1 seconds apart". Neither has a streak gate, so none
  was added: one would leave the US rules. A receiver tells consecutive BSMs apart by J2735
  `msgCnt`, so the sender now carries it (`BsmExtras::msg_cnt`) and the profile pairs only
  `msgCnt` successors within `max_gap_s`. The new choice
  `detection.local[].params.consecutive` takes `msg_count` (the default) or `received`
  (the old reading). Every Paper 1 scenario sets `received`, so Paper 1 keeps the reading
  it was measured on; its p1-attack recording is byte-identical to the release binary's.
- Evidence: `stress/fs-40` (full fleet, 40 s live), same binary, received -> msg_count.
  Verdicts went 8 -> 0 (positionJump 3, constantPositionFrozen 5) and reports about honest
  devices 8 -> 0. Attackers are still caught: on p1-attack, reports about attackers went
  126 -> 122, about the same attacker, with 2 cases in both. Fault: `in_sequence` always
  true makes `us_mbr::tests::only_bsms_consecutive_on_the_air_are_a_pair` red. No golden
  moved: the pinned runs use the research suite.
- Left: the matrix, realism and live-atk US-profile rows were measured on the old reading,
  so they are re-measured in the validation wave.
