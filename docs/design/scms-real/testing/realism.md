# SCMS runtime realism: what an operator would see

Written 2026-10-18 on `wf_1faba59c-7f5-4`, based on main `d3039b53`; resumed 2026-10-19 (the
owed runs, §6, and the new findings R-SERIAL, R-USFP, R-TRUSTLEAK, R-EVID). This package runs real
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
| Case board "reports in" | report arrival, 8 h before the decision | at the decision (base); 28,800 s before the decision (base-new, L4) | FIXED (R-CASE) |
| Case board "corroborated" | the case opening, 8 h before the decision (CV-pilot manual MA) | at the decision, review 0 s (base); review 28,800.001 s (base-new, L5) | FIXED (R-CORR) |
| Cases run concurrently (gap row 27) | a case opens when its evidence is corroborated | the 3rd case waited 28,800 s behind the 2nd (base-new, L6) | OPEN (R-SERIAL) |
| Fast-forward backend stops | ≤ 1 a minute when idle (D2): ~2,900 in 2 days | **255,658** (base); **4,290** (base-new), same top-ups (1,100) and enforcement instant (147,573.8 s); events 513,447 → 262,079 | FIXED (R-FF) |
| Peak memory | < 2.5 GB cap | 154 MB | PASS |

## 3. Results: configuration changes (same seed, against `base`)

| Change | Expected | Measured | Verdict |
|---|---|---|---|
| `certs_per_period` 60 (NYC taxis) | top-off certificates ×3; still 1 request; response size unchanged; download bytes ≈×3; BSM frames unchanged | 3,300 (×3.00); median 1; 470 B; Uu downlink ×2.93; 21,301 = 21,301; CPU 64→91 s | PASS |
| MA `automatic` | decision 8 h earlier; **publication unchanged**: decided Tue 17:00 EDT, after that day's slot, so it still goes out at Wednesday's slot; same cases | decision −28,800 s; published Δ 0; 3 cases | PASS |
| `crl_publication: on-decision` | published at the decision; enforced ≈ 9 h earlier | 115,183.75 s; enforced −32,334 s; 3 CRL versions instead of 2; downloads 49→98 | PASS |
| Both regions `us-west` | +16.7 ms per device–RA leg: haversine NYC–Ashburn 354.5 km is 2.77 ms, NYC–Boardman OR 3,691.7 km is 19.47 ms (2/3 c + 1 ms); counts unchanged | report arrival +16.70 ms; counts identical | PASS |
| `ma_report_sharing: false` | 0 shared; revocations same or fewer | 0 shared; 3 attackers still revoked: every attacker was reported by its own provider's devices too | PASS |
| provider-b `on_ctl: false` (rogue or test-CTL provider) | nothing B certifies verifies; nobody honest revoked | main: **panic in 5.7 s** ("the home provider has an approved root"); branch: runs (66 s), 0 honest revoked, 2 attackers revoked, but **7** provider-b messages verified at provider-a receivers (V6) | FIXED (R-OFFCTL); OPEN (R-TRUSTLEAK) |
| 2× vehicle demand | per-device requests 1; checks ×(n…n²); cost up | median 1; checks ×2.21; CPU ×1.52; 50 honest devices reported, 0 revoked | PASS |
| `detect/us-sm-mbr` | checks every BSM (×5–12); 0 unmapped reports; ConstantPosition catches the 3 attackers; more honest false positives (1 m drift, no streak gate, 12 m GNSS outliers) | checks ×5.72 (77,336); 0 unmapped; reports 7,885 (×25; 3,620 from RSUs); honest reported ×2 (36); **0 attackers and 4 honest revoked**; wall 180 s, 229 MB | V8 checks/mapped/FP PASS; catch FAIL → OPEN (R-SERIAL, R-USFP) |

Run cost (wall / CPU s; backend ticks):
- base: 61.6 / 64.3; 255,658
- base-new (branch, opt-2 debug binary): 52.7 / 53.8; 4,290; peak 127 MB
- us-profile: 179.7 / 101.9; 4,312; off-ctl: 66.2 / 52.6; 4,286
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

**Fixed** (all built, tested, fault-proved and measured; R-OFFCTL and R-FF verified on the resume, R-CORR added then):
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
  - Measured: 255,658 → 4,290 backend stops (base-new), the same 1,100 top-up certificates
    and the same enforcement instant. Engine test
    `phase2::fast_forward_wake_tests::a_crl_version_wakes_the_fast_forward_only_until_it_has_crossed`.
- **R-CORR.** The case board stamped `corroborated` at the decision. Under manual review
  that hid the 8 h review between the case opening and the operator's go-ahead.
  - `Case::opened` is stamped when the MA's pipeline opens the case; `corroborated` uses it.
  - Measured: review 0 s (base) → 28,800.001 s (base-new, row L5).

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
- **R-SERIAL (new, engine).** `Phase2::start_cases_of` opens one case per provider at a
  time (`while !case_open()`), though the proto MA runs up to `max_open_cases` (1,000)
  concurrently (gap row 27). Under the 8 h review each queued case waits 8 h per case
  ahead of it.
  - base-new: the 3rd case opened 28,800 s after its reports (row L6).
  - us-profile: provider-a's attacker case opened 57,598 s late, behind two honest cases.
    Its decision would fall at 172,805 s, after the 172,800 s horizon, so it never
    revoked (V8).
  - The fix is to loop until `request_revocation` refuses at the cap. A case's CRL entry
    must then be the one matching its own `(i, lv)` (`matches_any_index`), not
    `crlg.entries.last()`. Not done here: it changes case timing in every SCMS run, so the
    regression digests (`phase2-path`, `scms_scale`) must be re-blessed with heavy runs.
- **R-USFP (new).** Under `detect/us-sm-mbr` the MA revoked 4 honest vehicles, each on
  RandomPosition evidence (1 m gpsDrift, no streak gate).
  - The cause is GNSS outliers on honest senders (the Gauss-Markov model with 12 m jumps).
  - This is the expected cost of the strict thresholds, not a code defect. An attack study
    can use it directly: the profile alone lets an urban-canyon fleet be revoked.
  - It is undecided whether a real OBU's fused position ever jumps > 1 m between BSMs
    100 ms apart. A filtered-position model is owed before the rate is trusted.
- **R-EVID (new).** Every recorded case evidence says `verified: false`, base included.
  - The independent oracle agrees: `tools/pdu_check` decoded the 158 recorded opened
    reports of base-new. Every certificate chains, every signer names the evidence
    certificate, and all 158 fail on the BSM signature.
  - The cause: the benchmark runs `crypto_mode: modeled`, so on-air BSM signatures are
    not real ECDSA. The engine MA decides on the modelled reason codes, not this verdict,
    so no decision is wrong.
  - But the record reads as "the MA acted on unverified evidence". The verdict should be
    labelled modelled in that mode, or the realism base should run `crypto_mode: real`.
- **R-TRUSTLEAK (new, security).** With provider-b off the CTL, provider-a receivers
  verified 7 provider-b messages (`verified_by_provider` `provider-a<-provider-b`).
  - P2PCD did not cause it: learning was refused, with `learn_unanchored` 13 at
    provider-a and 0 learned.
  - Provider attribution is consistent: 0 devices whose flows and credentials disagree.
  - Not root-caused. The next suspect is the modelled-crypto receive path accepting a
    known issuer before its chain is anchored.
- **R-UNSEC (cause found).** All the "no active pseudonym certificate" reports come from
  RSUs.
  - us-profile: 3,620 unsecured = 3,620 RSU-filed; base: 83 of the 137 RSU reports.
  - A roadside unit is an SCMS device holding application certificates only. It passes
    `real_mbr`'s enrolment check and fails in `secure_mbr`.
  - The reason should say so. Whether a US RSU may sign an SM-MBR report with its
    application certificate is not settled by [SM-MBR] (signer = "the EE certificate").
- **Unmapped reports.** 148 of 320 research-suite reports name checks with no SM-MBR subtype
  (staleOrReplay, positionSpeedInconsistency), so they travel the modelled path. A real US
  device could not send them. The US profile removes this (§3).

## 5. Fault proofs

Rust faults were injected with `tools/mutate/mutate.py`.

| Check | Fault | Result |
|---|---|---|
| `ma_cases::the_case_board_dates_reports_in_from_the_arrival_not_the_decision` | arrivals ignored | RED |
| `provider_runs::a_shared_report_is_recorded_as_sent_by_the_other_providers_ma` | foreign components skipped | RED |
| `provider_runs::a_provider_off_the_ctl_still_runs_under_its_own_root` | home root must be approved | RED |
| `phase2::fast_forward_wake_tests::a_crl_version_wakes_the_fast_forward_only_until_it_has_crossed` | crossed version held to `now + 1` | RED |
| `ma_cases::the_case_board_dates_corroboration_from_the_case_opening_not_the_decision` | `corroborated` at the decision | RED |
| `runtime_realism.py check` L5 / edge default | L5 against main's base (review 0 s) / `edge.ee->ra` = 0 | 2 × RED |
| `us_mbr::tests` | MaxSpeed 120 m/s | RED |
| `us_mbr::tests` | gpsDrift 2 m | RED |
| `scenario::world_and_detector_inputs…` | US profile accepts any key | RED |
| `runtime_realism.py check` (input faults) | sealed SPDU opened by the RA / RA holds a linkage value / 2 requests per OBU / cert60 tops up ×2 / west leg not slower | 5 × RED |

## 6. Owed

- R-SERIAL: concurrent cases in the engine driver, with per-case CRL entry matching, then
  re-bless the regression digests. It is the cause of V8's missed attacker.
- R-TRUSTLEAK: root-cause the 7 verified off-CTL messages. A security fix comes first.
- R-EVID: label the evidence verdict under modelled crypto, or a `crypto_mode: real`
  realism base.
- R-UNSEC: RSU-filed reports are now labelled "a roadside unit holds no pseudonym certificate to sign an SM-MBR with" (07 §7 item 2); whether RSUs sign SM-MBR reports is still the owner's decision.
- R-USFP: a filtered-position GNSS option, so US-profile false positives can be trusted.
- R-LOP and R-CRLFETCH, above.
- The full quarter benchmark re-run on this branch. Every per-device number above is
  scale-free; the radio numbers are a fifteenth of rush-hour density.
