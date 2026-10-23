# Measurement and detection defects (R-FS40 frozen position, privacy links, OF-3, R-UNSEC, old evidence)

Round: SCMS realism fix wave 3, 2026-10-22, from main `77cffb5b`. "Before" numbers are the
release binary of `77cffb5b` on the R-FS40 window (the full-scale benchmark's first 40 s:
`scenarios/real-world/midtown-scms-benchmark.yaml` with `duration_s: 40`, `warmup_s: 10`, no
fast-forward, seed `0xBE7C_0001`, `--record-level events`). It reproduces final-bench.md
exactly: 236 verdicts (`constantPositionFrozen` 165, `positionJump` 71), 229 reports about
honest devices, 2,873 link claims, 3,120 link decisions, 0 pseudonym changes. Analysis
scripts read the recording's `node.tx` channel (each BSM's claimed position, speed and
`t_generated`); they are not committed.

## 1. Honest `constantPositionFrozen` at full density

**The check is as specified.** [SM-MBR] v1.0 §2.3.1.2.3 ConstantPosition: "velocity is
non-zero but the position has not changed", over "BSMs from the same sender". The engine
reads it over two consecutive BSMs at the J2735 resolutions (1 cm, 0.02 m/s), within 0.5 s.
The document gives no other speed or time condition.

**The static hold is not the cause.** A held receiver reports 0 m/s with its held position,
so the check, which needs both speeds non-zero, cannot fire on it
(`a_stopped_smoothing_receiver_holds_still`).

**Cause: the receiver's propagation and the OBU's latency compensation disagreed.** Over
all 75,570 BSMs of the window, 8 consecutive pairs from 8 senders met the rule on the sender
side. Each was heard by about 20 receivers, which gives the 165 verdicts. Every pair is a car
slowing through creeping speed: first speed 0.4–1.2 m/s, second 0.3–0.5 m/s, just above the
0.3 m/s hold. For example, sender temp id `a4b4c7cc` sent (1310.039, 1711.053) at 0.709 m/s,
then (1310.032, 1711.057) at 0.341 m/s 100 ms later, 0.8 cm apart.

- The smoothing receiver carried its solution from one 10 Hz epoch to the next by the
  *later* measured velocity alone: `p_k+1 = p_k + v_k+1·Δt` (plus the small β correction).
- The node stamps each BSM at its own phase φ after the epoch and dead-reckons the fix to
  that instant by the fix's velocity (`Engine::dead_reckon_belief`, J2945/1 latency
  compensation).
- Two consecutive BSMs then differ by `v_k+1·(Δt + φ) − v_k·φ`. That is zero when the speed
  drops by about half in one epoch with φ near Δt, which the 0.1 m/s-per-axis white Doppler
  noise does near a stop.

**Fix (`gnss/error/gauss-markov` 1.2.0).** The solution is carried across an epoch by the
mean of the two measured velocities: `p_k+1 = p_k + ½(v_k + v_k+1)·Δt`. This is the
carrier-phase delta of Hatch 1982's smoothing: the integrated range change over the
interval, not the Doppler at its end. A Monte Carlo of stop-and-go creep (0–1.5 m/s at
1 m/s², the default σ, the 0.3 m/s hold, uniform φ) gives 509 frozen pairs in 396,000 with
the old propagation and 4 with the new one.

## 2. Privacy: link claims with no pseudonym change

**Cause.** The observer (Wiedersheim et al. 2010 tracker) makes a link decision on every
pseudonym it hears for the first time. Under sniffers at 10 % of the signals, most first
sightings are cars coming into a sniffer's range, and the tracker links them to some other
car's track that went silent. The observer cannot tell that apart from a pseudonym change,
and it should not be able to. The defect was in the run report: `privacy_links_claimed`,
documented as "links claimed across a pseudonym change", counted every claim.

**Fix.** The ground-truth join (`Phase2::observe_frame`, run report only) now classifies
each decision. It is across a change when the new pseudonym's vehicle was heard before
under another pseudonym.

| counter | meaning |
|---|---|
| `privacy_changes_heard` (new) | decisions across a change: the changes the observer could have bridged |
| `privacy_links_claimed` | claims at those decisions only |
| `privacy_links_spurious` (new) | claims where no pseudonym changed, always wrong |
| `privacy_links_correct` | unchanged (a correct link implies a change) |

The `linkability` metric (`privacy_metrics.rs`: correct links / pseudonym changes) was
already right. On R-FS40 the 2,873 claims are now all `privacy_links_spurious`.

## 3. OF-3: the t0 CRLs a device holds

The engine was right and the records were silent. Every device joined holding both
providers' t0 lists (`scms_deployment::every_provider_publishes_its_crl_at_the_start`), but
`crl_version` counts entries, which is 0 for an empty list.

- `node.security` rows carry `crls`, the signed CRLs the node's own receiver verified and
  holds: `{craca, series, counter, issue_date, next_crl}` per `(crlCraca, crlSeries)`
  (`RxTrust::held_crls`).
- `pki.flow` carries the pre-run fetch, one `SecuredCrl` PDU per list per device. It goes
  from the device's provider's CRL Store at the instant the device joins, marked `preload`
  like its pre-run enrolment (`ScmsRun::note_prerun_crl`).

An oracle rule can now check that every device holds each CRACA's current list from t0.

## 4. R-UNSEC: a roadside unit's own reports

**The US profile has no roadside reporter, so a unit's own detection is not filed.**

- [SM-MBR] §3 signs the report with "the end-entity certificate", a pseudonym certificate in
  CAMP's design ([CAMP122] §5.2.8.4).
- [SM-MBR] §2.3.1.4.1's ObsImplausible evidence is "a BSM from the sender and a BSM from the
  receiver": the reporter is itself a BSM sender, an OBE.
- The USDOT DSRC RSU Specification v4.1 (2016, p. 15) gives a unit certificates "to
  support" store-and-repeat and immediate-forward transmission, and it forwards received
  messages by PSID. It has no detection or reporting function.

Under strict filing (the default), `Phase2::files_report` now refuses a unit's report. It is
counted under `reports_not_filed_by["a roadside unit holds no pseudonym certificate to sign
an SM-MBR with"]` and is never sent sized-only on the backhaul (`reports_from_rsus` 0).
`modelled_reports: 1` (research) keeps the old sized-only path. A unit still relays
vehicles' reports.

## 5. The 43,125 s-old evidence

**Root cause: a handset judged frames its relevance check had admitted 43,125 s earlier.** A
VRU device takes its inbox at its own step. Its `generationTime` relevance check (IEEE 1609.2
§5.2, RS_BSP_168 2 s for a beacon) compared the BSM's generation time with the frame's
*arrival* time. The verified message was then stamped with the device's time at the step
(`VruDeviceRuntime::to_message(.., believed, ..)`).

Frames that arrived just before the realism base's fast-forward (60 s) waited in handsets'
inboxes until the first step after it (43,185 s). They passed the check on their arrival
times, and the US profile judged them at 43,185 s. Their reports had `generation_time`
43,185.016 s and evidence `st_tend` 59.912 s, which is the measured lag of 43,125.104 s, and
the MA's evidence-time rule set them aside. An OBU's verified message keeps its arrival time,
so its reports were never implausible.

**Fix.** The handset re-checks the relevance window when it takes a frame
(`VruDeviceRuntime::receive`), as an SDS judges relevance at processing time. A stale frame
is `NotRelevant(generation-time-too-old)` and is never judged.

**Evidence is now what the reporter judged** ([SM-MBR] §2.3.1.2.1 and §2.3.1.3.1).

- Before: `Phase2::real_mbr` attached the accused certificate's last two BSMs on the air at
  the moment the report was built. For a report stored for days, or one built after the
  sender kept transmitting, those were not the BSMs the reporter judged.
- Now: the on-air ring keeps each certificate's last 4 BSMs with their `generationTime`. When
  a report is generated, `Phase2::detect` takes the judged pair: the US profile's previous
  consecutive BSM (`UsSmMbr::consecutive_before`) and the judged one, matched by generation
  time (`judged_bsms`). The report attaches exactly those whenever it is built.
- If the judged BSMs are not on hand, the report is not filed: "too few of the BSMs the
  reporter judged on hand".
- `reports_evidence_moved_on` counts the real reports to which the old rule would have
  attached different BSMs.

## 6. Tests and faults

| Item | Test | Fault injected (then restored) |
|---|---|---|
| 1 | `v2xw-mobility` `a_creeping_receiver_never_sends_two_bsms_at_one_position_while_moving` | propagate by the later velocity alone: RED |
| 2 | `security_lifecycle::linkability_counts_only_links_across_a_pseudonym_change` (busy grid, a sniffer at every signal: no-change run 7 claims, all spurious; 10 s period: 100 changes heard, 93 claims across, 88 correct, 10 spurious) | count every claim as across a change: RED |
| 3 | `scms_deployment::a_device_records_the_t0_lists_it_holds_and_their_fetch` | `node.security` emitted without the node's lists: RED. Not injected: skipping `note_prerun_crl` |
| 4 | `report_signing::no_vehicle_report_goes_out_unsigned_across_the_week_rollover` (now asserts the units' detections are not filed, by reason, and `reports_from_rsus` 0) | the old roadside exemption in `files_report`: RED |
| 5 | `v2xw-node` `vru_device::a_frame_that_waited_past_its_relevance_window_is_not_judged`; `phase2::judged_evidence_tests` | the processing-time relevance check reverted to the arrival time: RED; take the ring's last entry: RED |

The regression digests `phase2-path` and `i-s1-phase2-60s` were re-blessed (the new
`node.security.crls`, the pre-run PDUs, and the units' reports no longer filed).

## 7. Open

- **`scms_wiring::detectors_file_real_us_reports_that_the_ma_opens_and_verifies` fails at its
  last assertion: "the MA opened no case".** Its 9 real reports are filed and opened, and
  their evidence verifies (every assertion before the last passes). In 120 s no case opens
  from vehicle reports alone: the corroborated MA needs 3 independent reporters, and the
  roadside units' sized-only reports, which it trusted as infrastructure, no longer reach it
  (R-UNSEC). Whether that test should run longer or assert on a vehicle-only quorum is for
  the next wave. It was not weakened here.
- **Full-density re-run owed.** The R-FS40 window (and the gnss-raw run for the old-evidence
  count) has not been re-run on this branch. Expected: `constantPositionFrozen` near 0,
  `privacy_links_claimed` 0 with 2,873 spurious, `reports_evidence_time_implausible` 0 on
  gnss-raw. The 71 `positionJump` verdicts at full density are not explained by item 1, and
  their cause was not looked for.
- The τ, hold and velocity-σ constants of the smoothing receiver remain assumptions.
