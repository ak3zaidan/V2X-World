# SCMS runtime realism: what an operator would see

Written 2026-10-18 on `wf_1faba59c-7f5-4`, based on main `d3039b53`. This package runs real
simulations and compares each runtime number with the real US deployment:
- request and download rates
- sizes
- airtime
- calendar latencies
- the recorded topology
- separation of duties

It also adds the strict US misbehaviour profile.

## 1. Method

- **Scenario.** `scenarios/real-world/midtown-scms-realism.yaml` is the Midtown benchmark's
  quarter file with these changes:
  - 600 veh/h, 100 pedestrians and 10 cyclists (95 devices; 69 vehicles, 26 handsets)
  - 3 % constant-position attackers from 20 s, so that a case, a CRL and enforcement happen
  - the PKI view on

  Everything else is the benchmark: the real calendar from Mon 2026-10-19 21:00 UTC,
  `us-production-2025`, manual MA review, the business-day CRL, 30 RSUs, DSRC/J2945/1, the
  4× fill, live 0–60 s and 43,185–43,215 s (the 614→615 rollover), and fast-forward to
  172,800 s.
- **Variants.** `scenarios/real-world/realism/*.yaml` each change one setting and use the
  same seed `0xBE7C0001`.
- **Running.** Each run goes through the run lock with `--record-level events`.
  - Runs `base` and the main-binary variants use the release binary of `d3039b53`
    (`target/release/v2xw`).
  - Runs `base-new`, `us-profile` and `off-ctl` use this branch's opt-level-2 debug binary.
- **Reading the runs.** `tools/realworld/runtime_realism.py extract` reads `run-report.json`
  and the `pki.flow`, `pki.knowledge`, `pki.case` and `pki.state` records.
- **Expectations.** Every expectation was written down before its run:
  - `tools/realworld/runtime_expectations.yaml`: 60 rows, each with a source
  - `crates/v2xw-engine/tests/expectations/scms-realism.yaml`: the one row the catalogue
    sweep can plan. The sweep publishes only 4 SCMS settings, and its loader refuses rows of
    any other shape.

  `runtime_realism.py check` evaluates every row.
- **The checker can fail.** Five input faults were injected and all five went RED (§5).

Reproduce:
```
v2xw run scenarios/real-world/midtown-scms-realism.yaml -o out/base --record-level events --no-verify
python -I tools/realworld/runtime_realism.py extract out/base --json out/base/realism.json   # per run
python -I tools/realworld/runtime_realism.py check tools/realworld/runtime_expectations.yaml out
```

## 2. Results: the base run against the real deployment

| Check | Expected (source) | Measured (run) | Verdict |
|---|---|---|---|
| EE peers | RA only, plus the DCM at the factory (01 §2.3; [SM-MBR] §4) | ra, dcm, **lop**, **ma** (base) | ma: FIXED (R-SHARE); lop: OPEN (R-LOP) |
| EE→ACA, EE→LA | none (CAMP122 §5.2) | 0, 0 | PASS |
| Shared report sender | the other provider's MA (Provider Req. §2.2) | `ee/5` (base) → `ma` (base-new) | FIXED (R-SHARE) |
| Components on record | the US roles (00 §2.1) | + lop, crl-store, crl-broadcast | OPEN (R-LOP) |
| MA resolution path | MA→ACA→RA→LA1/LA2 (BRECHT §VI-C) | ma→aca 3, ma→ra 3, ma→la1/la2 3 each | PASS |
| Separation of duties (recorded `holds`) | no entity pairs a device identity with a linkage id; the ground-truth `device` never in `holds` | RA {enrolment, request hash}; ACA {lv, handle}; LA {lci}; MA {subject_lv}; ECA {canonical_id}; 0 violations | PASS |
| Sealed SPDUs | opened only by their recipient (02 §8) | 8,427 checked, 0 mismatched | PASS |
| Provisioning requests | 1 per OBU over its life (`ra-maxReqs`) | median 1; RSUs 3 (2 preloaded + 1 weekly renewal) | PASS |
| CAMP rate (17 M vehicles a year) | ≈ 0.5 req/s (CAMP122 §6.3.3.4) | 1 per device → 0.54 req/s | PASS |
| Weekly top-off | one `.info` and one file per device per week, 20 certificates | 55 of 55 devices present, 1,100 certificates | PASS |
| Top-off latency | inside the 1 h overlap | 60.1 s (download poll), from 43,200.02 s | PASS |
| Certificate availability | none starved | 0 starved | PASS |
| Sizes (04 §3) | response 454–470, request 385–424, ack 292–294, ECA response 958 | 454–470, 385–422, 292–294, 958 | PASS |
| CRL size | 328 B with 1 entry, +~33 B per entry (04 §5; Brecht ≈ 40 B) | 328 / 361 B | PASS |
| MBR size | 2 signed BSMs + certificate, signed and encrypted | 694–876 B | PASS |
| MBR airtime share | ≪ 29.3 % (pre-fix) | 0.33 % (16 relay frames) | PASS |
| Revocations | 3 attackers, 0 honest | 3 / 0 (18 honest devices were reported, none revoked) | PASS |
| Report shuffle | 10,000 reports or 1 day (CAMP122 §5.2.8.4) | released at 86,383.7 s | PASS |
| MA decision | shuffle + 8 h review (D3) | 115,183.7 s | PASS |
| CRL publication | next business-day slot ([SM-PROV]) | 147,560.8 s (Wed 10:00 EDT) | PASS |
| Enforcement | after the first fetch | 147,573.8 s | PASS |
| Case board "reports in" | report arrival, 8 h before the decision | at the decision (base); fix proved by its unit test, base-new NOT RUN: the build lock was held by another agent's 35-min test at the 3 h stop (§6) | FIXED (R-CASE) |
| Fast-forward backend stops | ≤ 1 a minute when idle (D2): ~2,900 in 2 days | **255,658** (base); fix committed UNVERIFIED, base-new NOT RUN: the build lock was held by another agent's 35-min test at the 3 h stop (§6) | OPEN (R-FF fix unverified) |
| Peak memory | < 2.5 GB cap | 154 MB | PASS |

## 3. Results: configuration changes (same seed, against `base`)

| Change | Expected | Measured | Verdict |
|---|---|---|---|
| `certs_per_period` 60 (NYC taxis) | top-off certificates ×3; still 1 request; response size unchanged; download bytes ≈×3; BSM frames unchanged | 3,300 (×3.00); median 1; 470 B; Uu downlink ×2.93; 21,301 = 21,301; CPU 64→91 s | PASS |
| MA `automatic` | decision 8 h earlier; **publication unchanged**: decided Tue 17:00 EDT, after that day's slot, so it still goes out at Wednesday's slot; same cases | decision −28,800 s; published Δ 0; 3 cases | PASS |
| `crl_publication: on-decision` | published at the decision; enforced ≈ 9 h earlier | 115,183.75 s; enforced −32,334 s; 3 CRL versions instead of 2; downloads 49→98 | PASS |
| Both regions `us-west` | +16.7 ms per device–RA leg: haversine NYC–Ashburn 354.5 km is 2.77 ms, NYC–Boardman OR 3,691.7 km is 19.47 ms (2/3 c + 1 ms); counts unchanged | report arrival +16.70 ms; counts identical | PASS |
| `ma_report_sharing: false` | 0 shared; revocations same or fewer | 0 shared; 3 attackers still revoked: every attacker was reported by its own provider's devices too | PASS |
| provider-b `on_ctl: false` (rogue or test-CTL provider) | nothing B certifies verifies; nobody honest revoked | main: **panic in 5.7 s** ("the home provider has an approved root"); fix committed UNVERIFIED | OPEN (R-OFFCTL fix unverified) |
| 2× vehicle demand | per-device requests 1; checks ×(n…n²); cost up | median 1; checks ×2.21; CPU ×1.52; 50 honest devices reported, 0 revoked | PASS |
| `detect/us-sm-mbr` | checks every BSM (×5–12); 0 unmapped reports; ConstantPosition catches the 3 attackers; more honest false positives (1 m drift, no streak gate, 12 m GNSS outliers) | NOT RUN: the build lock was held by another agent's 35-min test at the 3 h stop (§6) (profile and its unit tests are in; the run is owed) | OPEN |

Run cost (wall / CPU s; backend ticks):
- base: 61.6 / 64.3; 255,658
- cert60: 87.1 / 90.5
- ma-auto: 277.8 / 135.5; ticks and events identical to base. The doubled CPU is not
  explained by the run's own counts. The machine was shared with other agents' runs.
- crl-on-decision: 225.5 / 155.8; **578,617 ticks**. Publishing 32,377 s earlier added
  322,959 stops, which is 10 per second, one per mobility step. This is how the R-FF cause
  was found.
- region-west: 84.7 / 75.6
- no-sharing: 74.6 / 69.6
- fleet2x: 90.7 / 97.7

## 4. What was fixed, and what is open

**Fixed** (R-SHARE and R-CASE: built, tested and fault-proved; R-OFFCTL and R-FF: committed in `UNVERIFIED` commit, not yet compiled or run, §6):
- **R-SHARE.** `pki_view::component_ids` named only the run's own provider's components.
  - A report the other provider's MA shares was therefore recorded as sent by `ee/5` (that
    MA's node id).
  - The recorded topology showed devices talking to the foreign MA.
  - Now every provider's components carry their own labels.
- **R-CASE.** The case board's `reports-in` was stamped at the decision. Under manual review
  this hid the 8 h wait: the three stages carried one timestamp.
  - The MA now keeps each report's arrival time (`MaState::report_arrived`).
  - `reports-in` is the earliest arrival among the case's reports.
- **R-OFFCTL.** `Governance::home_root` required an *approved* root. Any run with a provider
  off the CTL panicked on the first use of that provider's own root.
  - Such a provider now runs under its own unapproved root, which nobody else trusts.
  - This is the configuration an attack study needs for a rogue or test-CTL SCMS.
- **R-FF.** `Phase2::next_wake` offered every CRL version ever published, held to
  `now + 1`.
  - From the first publication on, fast-forward therefore stopped at every 100 ms step.
  - That is the open benchmark §9.2 (1.1 M ticks); here it was 255,658 of an expected
    ~2,900.
  - Only versions still crossing the inter-provider link are wakes now. `visible_versions`
    already serves the ones that have arrived.
  - **Not yet built or run.** Proof owed: base-new against base, and an engine fast test (§6).

**Added: the strict US profile `detect/us-sm-mbr`** (`v2xw_threat::us_mbr`):
- Subtypes and defaults are transcribed from the SCMS Manager PDF (fetched 2026-10-18, not
  committed: All Rights Reserved). The ASN.1 numbering was checked against the v1.1 module:
  - ConstantPosition = 1 and RandomPosition = 2 (gpsDrift 1 m)
  - MaxSpeed = 1 (90 m/s) and MaxAcceleration = 2 (10 m/s²)
  - ObsImplausible Distance: 3000 m + 10 m, **off** by default
- It checks every BSM against the previous one from the same certificate, with no streak
  gate.
- Reports carry the profile's thresholds. The new check `implausibleSpeed` maps to
  MaxSpeed and has no fingerprint column, so `detnorm_*` is unchanged.
- Deviations:
  - MaxAcceleration uses the implied acceleration, because the observation has no
    `accelSet`.
  - BrakeAcceleration is not checked, because there is no brake status.
  - "Consecutive" is ≤ 0.5 s apart (ASSUMPTION).
- Engine option: `detection.local: [{id: detect/us-sm-mbr, params: {...}}]`. The profile
  must be listed alone.

**Open:**
- **R-LOP.** Under `us-production-2025` the PKI view still lists `lop`, `crl-store` and
  `crl-broadcast` as online components.
  - Every EE↔RA message is recorded as two hops through `provider-x/lop/0`: 670 EE→LOP and
    670 LOP→RA here.
  - The deployment itself runs no LOP (`deployment.rs` asserts count 0, and the hop is
    transparent: 0 latency, no queue).
  - So the timing is right, but the recorded architecture is not the US one.
  - The cause is that the runner always routes through `nodes.lop` (≈ 40 sites in
    `run/provision.rs`).
  - The fix is to elide the transparent hop when publishing `pki.flow` and omit undeployed
    roles in `pki.state`. It touches the conformance capture and the Studio, so it is left
    to its own package.
- **R-CRLFETCH.** Device CRL downloads (49 here) do not appear in `pki.flow`. The recorded
  topology lacks the EE→MA/CRL-store `download-crl` edge (03 §1 item 6).
- **R-CORR.** `corroborated` is still stamped at the decision. Under manual review,
  corroboration happens when the request enters `awaiting_review`, which is not timestamped.
- **R-UNSEC.** 83 of 320 reports went the modelled path: "no active pseudonym certificate"
  for the reporter. Not investigated. A candidate is reporters at the rollover instant.
- **Unmapped reports.** 148 of 320 research-suite reports name checks with no SM-MBR subtype
  (staleOrReplay, positionSpeedInconsistency), so they travel the modelled path. A real US
  device could not send them. The US profile removes this (§3).

## 5. Fault proofs

Rust faults were injected with `tools/mutate/mutate.py`.

| Check | Fault | Result |
|---|---|---|
| `ma_cases::the_case_board_dates_reports_in_from_the_arrival_not_the_decision` | arrivals ignored | RED |
| `provider_runs::a_shared_report_is_recorded_as_sent_by_the_other_providers_ma` | foreign components skipped | RED |
| `provider_runs::a_provider_off_the_ctl_still_runs_under_its_own_root` | home root must be approved | NOT RUN (§6) |
| `us_mbr::tests` | MaxSpeed 120 m/s | RED |
| `us_mbr::tests` | gpsDrift 2 m | RED |
| `scenario::world_and_detector_inputs…` | US profile accepts any key | RED |
| `runtime_realism.py check` (input faults) | sealed SPDU opened by the RA / RA holds a linkage value / 2 requests per OBU / cert60 tops up ×2 / west leg not slower | 5 × RED |

## 6. Owed

- **First:** build and test the UNVERIFIED commit (R-OFFCTL, R-FF). Run `cargo test -p v2xw-proto --test provider_runs`, then F6 (in `scratchpad/realism/step2.sh`). Build the binary. Run `us-profile`, `off-ctl` and `base-new` and check the TBD rows of `runtime_expectations.yaml` (V6, V8, C1, L4, T4).

- An engine fast test for R-FF: a short calendar run with a CRL published early in a long
  fast-forward, asserting the backend stops stay near one a minute.
- R-LOP, R-CRLFETCH, R-CORR and R-UNSEC above.
- The full quarter benchmark re-run on this branch. Every per-device number above is
  scale-free; the radio numbers are a fifteenth of rush-hour density.
