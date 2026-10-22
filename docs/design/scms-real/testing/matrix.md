# SCMS configuration matrix

Round: SCMS realism testing, 2026-10-22, key `matrix`, on main `2004bb87` (after the PKI,
FLOWS and POLICY fix wave). Table:
`crates/v2xw-engine/tests/expectations/scms-matrix.yaml` (schema
`v2xw/scms-run-expectations/1`, so the sweep's loader passes over it, like
`scms-effects.yaml`). Checker: `tools/realworld/matrix.py`.

## Method

- **Base `M0`.** `scenarios/real-world/midtown-scms-realism.yaml`: Midtown at a fifteenth
  of rush hour (95 devices: 69 vehicles, 26 handsets), `us-production-2025` (two providers
  50/50), the US SM-MBR detector profile, the corroborating MA with the pilots' manual
  review (8 h), the business-day CRL slot, 30 RSUs on fibre, DSRC under J2945/1, cellular
  Uu at 80 % penetration, 3 % constant-position attackers. Real calendar from Mon
  2026-10-19 21:00 UTC; live 0–240 s and 43,170–43,230 s (the 614 → 615 rollover);
  fast-forward to 172,800 s. Seed `0xBE7C0001`.
- **Variants.** 23 one-change files in `scenarios/real-world/matrix/` plus the 8 realism
  variants of `scenarios/real-world/realism/`, each on the same seed. `pq-hybrid` is
  judged against `no-hsm`, because no OBU HSM publishes Falcon and the loader refuses
  the hybrid on the base OBU.
- **Expectations first.** All 98 rows, each with its source, were committed (`019108e4`)
  before any run they judge.
- **Running.** One run per `heavy.sh` call through the run lock, `--record-level events`,
  `/usr/bin/time -l` for wall time and peak memory. Binary: an opt-level-2 debug build of
  this branch (`b2`, with the two fixes below). The pre-fix base `M0-b1` (the same build
  without them) is kept for the before/after.
- **Reading.** `matrix.py extract` is `runtime_realism.py extract` (run report, PKI view
  records: messages per entity pair, PDU sizes, per-device requests, flow latencies, case
  stages, separation of duties, sealed-recipient check, run cost) plus the pooled metric
  samples of `metrics.json` (`m.*`: PDR, latency stages, CBR, AoI, overhead, verify load,
  certificate pool, detection quality, revocation stages). `matrix.py check` judges every
  row and prints one line each.

Reproduce:
```
v2xw run scenarios/real-world/matrix/<variant>.yaml -o runs/<variant> --record-level events
python -I tools/realworld/matrix.py extract runs/<variant>
python -I tools/realworld/matrix.py check crates/v2xw-engine/tests/expectations/scms-matrix.yaml runs [--fill]
```

## Totals

| Table | Rows | Pass | Fixed (a bug, then pass) | Expectation wrong, corrected | Open |
|---|---|---|---|---|---|
| This matrix (`scms-matrix.yaml`), 32 runs + the pre-fix base | 98 | 82 | 1 | 5 | 10 |
| Realism rows re-run (`tools/realworld/runtime_expectations.yaml`) on this round's runs | 62 | 49 | 0 | 1 (V8, see below) | 12 |

All 32 runs completed through the run lock: wall 25–410 s, peak 127–447 MB.

## What I fixed

1. **The SCMS runs had become 13 times slower** (`7a8e7ef0`).
   - Symptom: the realism base took 2,074 s of wall time (1,746 CPU-s). It had taken 53 s before
     the fix wave.
   - Cause, from a `sample` profile: 97 % of the time was in
     `DeviceState::credential_is_usable`, a P-256 base-point multiplication.
     `Phase2::report_waits_for_signer` → `ScmsRun::mbr_signer` called it for every pseudonym
     of every device holding a report, at every backend step of the fast-forward.
   - Fix: `UsableMemo` keys each answer by `(i, j)`, the derived private key and the certified
     key. A credential whose secret or certificate changes is checked again, so the memo
     cannot change a result.
   - Measured: 164 s wall. All 573 measurements are identical to the pre-fix run except AoI
     (fix 2).
   - Test: `v2xw-proto tests/provisioning.rs::the_usability_check_answers_a_repeat_from_its_memo`.
     It goes red with the memo lookup removed. With the memo keyed by `(i, j)` alone, the
     swapped-randomiser test goes red.
   - Note: other agents' runs with older binaries pay the same cost.
2. **The headline `aoi` read 1,693,079 ms (28 minutes)** (`7a8e7ef0`).
   - Cause: `Link::age_over` integrates the neighbourhood age. The first update after a
     silence charged the piece before it with the whole silence, held from the pre-silence
     delivery. A fast-forward window is a silence of 43,000 s. The every-link pool already
     restarted after `aoi_restart_s`; the neighbourhood path did not.
   - Fix: a piece more than `aoi_restart_s` after the delivery it holds is left out.
   - Measured: 81.6 ms (handsets 824 ms at 1 Hz).
   - Test: `v2xw-metrics awareness::tests::neighbourhood_aoi_restarts_after_a_silence_rather_than_charging_it`,
     red under the injected fault. The whole `v2xw-metrics` suite passes (273 tests).

## Findings by area

- **Metrics on the base are plausible.**
  - PDR 0.364 (82 % of losses below sensitivity among buildings).
  - BSM latency: mean 12.4 ms, p95 14.6 ms, under the 100 ms budget (signing in the
    Craton2's secure element is 8.6 ms of it).
  - CBR p95 1.4 %, peak AoI 114 ms, AoI 82 ms.
  - 2.5 % of receptions unverified; security is 46 % of a BSM's bytes; 17 % of BSMs carry
    the full certificate.
  - Certificates valid in the pool: mean 25.7. No signature or parse failures, no backend
    errors, no starvation.
- **The SCMS architecture holds in every configuration.**
  - One request per OBU; 20 certificates per device at the top-off (60 under cert60, ×3.00).
  - Sealed messages are opened only by their recipient. No separation-of-duties violation,
    including under the CAMP topology.
  - A device talks only to its RA and the DCM.
  - I-S1 holds: real and modelled crypto give the same records (topped-up certificates,
    revocations, frames and sizes identical; CPU 186 s against 164 s).
- **Environment.**
  - LTE-V2X: PDR 0.88 and BSM latency mean 68 ms; NR-V2X: 0.61 and 36 ms. The SCMS PDUs and
    flows are unchanged.
  - RSUs 30 → 5 → 0: RSU-filed reports 3,616 → 503 → 0; top-offs 55 → 53 → 51 (the
    relay-only vehicles).
  - No cellular: 47 top-offs. No vehicle enforces the CRL within the run: it is published at
    147,581 s, inside the fast-forward, and only cellular fetches run there.
  - Cellular backhaul adds 72 ms to the RSU's renewal. TLS 1.2 changes nothing on this Uu
    model.
  - Half the fleet: 780 certificates topped up and verifications ×0.47. Twice the fleet:
    CBR p95 4.5 % and wall time ×2.
- **Post-quantum at density** (edge 11, owed until now). Hybrid Falcon-512 + P-256 on the
  same Midtown fleet, against `no-hsm`:
  - security bytes ×9.84, 7,968 fragments, channel load ×4.07, PDR −5.5 %, verify stage
    +42 %;
  - nobody starves.
  - **Open:** under the hybrid scheme there is no top-off (0 against 1,100 certificates,
    because pre-generation is off for lack of a PQ butterfly) and no RSU CRL install, and
    receivers check no CRL chain (`rv.crls_accepted` 0). The PQ path is a modelled one, and
    it is labelled as such.

## Open (not fixed, with evidence)

1. **R-USFP — FIXED after this round** (`usfp.md`). The cause *was* the GNSS input, but not
   its scale. The raw 10 Hz fixes each carried fresh 1.6 m white noise (RTK: about 0.5 m), so
   consecutive honest BSMs disagreed with their own velocity by 2.65 m at the median. The
   US presets' receivers now output a smoothed solution with a static hold. Same seed,
   after: 0 honest revoked, 2 of 3 attackers (the third is a handset, and SM-MBR defines
   PSID 0x20 only), 235 reports at the MA. Rows re-stated: M0-attackers-revoked,
   M0-honest-revoked, M0-false-accusation, rtk-*, sybil-not-revoked and offctl-honest,
   plus a new raw-honest row. Rows whose variant was not re-run keep their pre-fix
   numbers. The original finding:
   - Under the US SM-MBR default, the base revokes 26 honest vehicles and 1 of 3 attackers.
     7,200 of the 7,488 reports are RandomPosition (`positionJump`).
   - RTK (0.26 m) still revokes 26, with 15 % fewer reports. GNSS with no outliers and no
     bursts revokes 25.
   - So 07 §8.3's explanation (raw GNSS drift) is refuted. The research suite revokes 0
     honest and 3 of 3 attackers.
   - Next suspect: the claimed position and `generationTime` pair that the 1 m check
     extrapolates over, which is not GNSS. It needs a receptions-level run.
2. **R-TRUSTLEAK grew** from 7 to 280 provider-b messages verified at provider-a receivers,
   with provider-b off the CTL.
3. **A constant 27 MA decisions** in M0, att10 (9 attackers, 5 reported, 3 revoked) and
   sybil. **Explained** (`usfp.md` §4): M0 and att10 decide the same 27 actors, the R-USFP
   false positives (same seed, traffic, GNSS streams and always-trusted RSU reporters).
   After the fix: 2, 4 and 2.
4. **LTE-V2X BSM latency p95 is 112 ms**, over the 100 ms budget. The BSM is not generated
   aligned with its SPS reservation.
5. **Realism rows Z1/Z3/Z4 (sizes).**
   - provider-b (SAESOL form) matches CAMP Table 3 exactly: response 454/470, ack 292/294,
     ECA response 958.
   - provider-a (the ISS form from the fix wave) is 501/517, 304/306 and 1,064.
   - Most likely its published component certificates are longer. That is not yet checked
     against the published ISS chain by the oracle.
   - Z5 (CRL 924 B) and A1 (MBR airtime 7.8 %) are consequences of R-USFP's 27 revocations
     and 7,744 reports.
6. **L5:** the case board shows the review as 28,799.2 s, not ≥ 28,800 s. The review clock
   starts at report receipt, and `corroborated` is stamped up to 0.8 s later.
7. **V8:** the row compares the US profile with `base`, and the US profile is now the base.
   Against `research-suite`, 77,433 / 13,523 checks = ×5.73, inside [5, 12]: pass once
   re-based.
8. **Not re-run here:**
   - the scms-knobs sweep rows, which another agent ran through `scms_knobs_runs` during
     this round;
   - the effects table's 20 credential-weeks runs;
   - the conformance oracle run.

## Per-run headline measurements

| run | wall s | MB | PDR | BSM ms | CBR p95 | AoI ms | verified | sec B | top-off certs | reports | attackers rev. | honest rev. | enforced s |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| M0-b1 | 2074 | 221 | 0.364 | 12.4 | 0.014 | 1693079 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| M0 | 164 | 223 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| no-hsm | 159 | 226 | 0.363 | 3.7 | 0.014 | 70 | 77445 | 2446825 | 1100 | 7498 | 2 | 28 | 147590 |
| pq-hybrid | 25 | 161 | 0.343 | 6.7 | 0.059 | 81 | 72398 | 24072007 | 0 | 6706 | 2 | 27 | 147590 |
| gnss-rtk | 147 | 196 | 0.364 | 12.4 | 0.014 | 81 | 77344 | 2446127 | 1100 | 6395 | 1 | 26 | 147594 |
| gnss-clean | 160 | 223 | 0.364 | 12.4 | 0.014 | 82 | 77270 | 2446111 | 1100 | 7475 | 1 | 25 | 147595 |
| on-demand | 36 | 149 | 0.364 | 12.1 | 0.014 | 82 | 37580 | 2446453 | 1100 | 3632 | 1 | 22 | 147595 |
| camp | 158 | 234 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 27 | - |
| test-ctl | 122 | 182 | 0.364 | 12.4 | 0.014 | 81 | 77292 | 2447860 | 1100 | 6038 | 2 | 20 | 104415 |
| split-90-10 | 161 | 232 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 27 | 147597 |
| lte | 381 | 447 | 0.880 | 68.0 | - | 113 | 257141 | 4722994 | 1100 | 26077 | 2 | 35 | 147602 |
| nr | 234 | 334 | 0.605 | 35.9 | - | 88 | 124927 | 2355248 | 1100 | 13020 | 2 | 32 | 147594 |
| nyc-pilot | 156 | 223 | 0.364 | 12.3 | 0.013 | 81 | 76888 | 2446111 | 1100 | 7432 | 1 | 29 | 147595 |
| silent | 162 | 223 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| full-cert | 169 | 233 | 0.362 | 12.4 | 0.018 | 83 | 79085 | 3863553 | 1100 | 7770 | 1 | 29 | 147595 |
| rsu5 | 263 | 181 | 0.334 | 12.3 | 0.016 | 72 | 45972 | 2446291 | 1060 | 4016 | 1 | 23 | 147707 |
| rsu0 | 233 | 180 | 0.323 | 12.3 | 0.014 | 81 | 40520 | 2445854 | 1020 | 3883 | 1 | 24 | 147717 |
| pen0 | 250 | 200 | 0.359 | 12.4 | 0.041 | 82 | 76849 | 2446453 | 940 | 6803 | 1 | 26 | - |
| backhaul-cellular | 160 | 221 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| fleet-half | 82 | 134 | 0.366 | 12.4 | 0.022 | 89 | 36486 | 1404446 | 780 | 3383 | 2 | 12 | 147726 |
| att10 | 160 | 210 | 0.364 | 12.4 | 0.014 | 81 | 77301 | 2446453 | 1100 | 7154 | 3 | 24 | 147595 |
| sybil | 167 | 245 | 0.361 | 12.7 | 0.023 | 81 | 92543 | 3842239 | 1100 | 9192 | 1 | 26 | 147595 |
| ma-legacy | 160 | 220 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 28 | 147601 |
| tls12 | 160 | 220 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| real-crypto | 186 | 211 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| cert60 | 410 | 278 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 3300 | 7488 | 1 | 26 | 147595 |
| crl-on-decision | 161 | 226 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 115195 |
| fleet2x | 333 | 438 | 0.347 | 12.3 | 0.045 | 79 | 190342 | 4875824 | 1960 | 18736 | 2 | 72 | 147637 |
| ma-auto | 160 | 215 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| no-sharing | 160 | 212 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 4568 | 1 | 22 | 147595 |
| off-ctl | 100 | 127 | 0.362 | 12.3 | 0.014 | 79 | 77420 | 2447575 | 1100 | 2074 | 1 | 7 | 147595 |
| region-west | 161 | 214 | 0.364 | 12.4 | 0.014 | 82 | 77283 | 2446453 | 1100 | 7488 | 1 | 26 | 147595 |
| research-suite | 41 | 148 | 0.364 | 12.4 | 0.014 | 82 | 77430 | 2446453 | 1100 | 310 | 3 | 0 | 147574 |

## Every row

| Row | Change | Expected (source) | Measured | Verdict |
|---|---|---|---|---|
| `M0-pdr` | base | `m.pdr` band [0.2, 0.9] (urban DSRC to 500 m among Midtown buildings: most pairs past ~150 m are NLOS and below sensitivity (Mangel et al. 2011 NLOS; 04-models PH...) | 0.363836 | pass |
| `M0-bsm-latency` | base | `m.e2e_latency.bsm.p95` le 100.0 (SAE J2945/1 §6.3: BSM latency budget 100 ms; DSRC channel access is sub-ms at low CBR) | 14.639 | pass |
| `M0-cbr` | base | `m.cbr.172.p95` le 0.6 (a fifteenth of rush hour stays far below the J2945/1 congestion-control range (CBR 50-80 %)) | 0.0143 | pass |
| `M0-aoi-peak` | base | `m.aoi_peak.mean` band [100.0, 1000.0] (10 Hz BSMs: a neighbour's peak age is at least one interval (100 ms), a few intervals with loss) | 114.487 | pass |
| `M0-aoi-mean` | base | `m.aoi` band [50.0, 5000.0] (time-average age of a 10 Hz stream with ~36 % PDR is a few hundred ms (Kaul et al. 2011); the fast-forward days carry no radio and must n...) | 81.6119 | fixed — pre-fix M0-b1: 1,693,079 ms, the fast-forward silence charged as age; after: 81.6 ms |
| `M0-unverified` | base | `m.unverified_ratio` le 0.1 (prioritised verification sheds only when the verifier falls behind; at CBR < 5 % the OBU's verifier keeps up (verify_wait ~0)) | 0.025131 | pass |
| `M0-sec-overhead` | base | `m.security_overhead.bsm` band [0.3, 0.7] (1609.2 BSM: ~40 B payload against ~100 B security (digest) and ~170 B (certificate once a second): security is about half the frame) | 0.458282 | pass |
| `M0-full-cert-share` | base | `m.full_cert_share` band [0.05, 0.3] (J2945/1 §6.1.2.2: certificate once a second, digest otherwise (1 in 10 at 10 Hz), plus the new-neighbour rule) | 0.173771 | pass |
| `M0-pool` | base | `m.cert_pool_valid.mean` band [15.0, 60.0] (20 pseudonyms a week, 2 weeks preloaded, weekly top-off (CAMP 1.2.2 Table 67; NYC/THEA pilots)) | 25.6987 | pass |
| `M0-no-sig-fail` | base | `p2.spdu_signature_failures` eq 0 (honest SCMS PDUs all verify (IEEE 1609.2.1)) | 0 | pass |
| `M0-no-parse-fail` | base | `p2.spdu_parse_failures` eq 0 (every SPDU decodes (1609.2.1-2022 COER)) | 0 | pass |
| `M0-no-backend-error` | base | `p2.backend_errors` eq 0 (an on-time backend answers every request) | 0 | pass |
| `M0-no-starve` | base | `p2.vehicles_starved` eq 0 (two weeks preloaded plus weekly top-off: no device runs out in two days) | 0 | pass |
| `M0-attackers-revoked` | base | `p2.revoked_attackers` eq 3 (3 % constant-position attackers are caught by the SM-MBR ConstantPosition check and revoked after shuffle + review (SM-MBR §2.3; realism.md)) | 1 | open — only 2 of 3 attackers reported, 1 revoked, among 27 decisions; the research suite revokes 3 of 3 (research-attackers). R-USFP: 7,200 of the 7,488 reports at the MA are RandomPosition (positionJump), 7,375 of them about honest vehicles; 36 honest reported, 26 revoked. gnss-rtk still revokes 26, so the raw GNSS error is not the cause (07 section 8.3 said it was); see matrix.md 'Open' |
| `M0-honest-revoked` | base | `p2.revoked_honest` eq 0 (the pilots revoked no honest device (CV pilot evaluations); an honest OBU's filtered position does not jump 1 m between consecutive BSMs) | 26 | open — R-USFP: 7,200 of the 7,488 reports at the MA are RandomPosition (positionJump), 7,375 of them about honest vehicles; 36 honest reported, 26 revoked. gnss-rtk still revokes 26, so the raw GNSS error is not the cause (07 section 8.3 said it was); see matrix.md 'Open' |
| `M0-false-accusation` | base | `m.false_accusations.vehicle` eq 0 (as M0-honest-revoked, the metric's view) | 26 | open — R-USFP: 7,200 of the 7,488 reports at the MA are RandomPosition (positionJump), 7,375 of them about honest vehicles; 36 honest reported, 26 revoked. gnss-rtk still revokes 26, so the raw GNSS error is not the cause (07 section 8.3 said it was); see matrix.md 'Open' |
| `M0-enforced` | base | `stage.enforced_s` band [147500.0, 147800.0] (reports shuffled one day, 8 h review, next business-day 10:00 ET slot (Wed 2026-10-21 10:00 EDT = 147,600 s), then the first fetch (reali...) | 147595 | pass |
| `M0-req-per-obu` | base | `perdev.EeRaCertRequestSpdu.median` eq 1 (ra-maxReqs = 1: one provisioning request per device over its life (CAMP 1.2.2 §6.3.3.4)) | 1 | pass |
| `M0-topups` | base | `p2.certs_topped_up` eq 1100 (55 devices present at the 614 -> 615 rollover, 20 certificates each (realism P3)) | 1100 | pass |
| `M0-separation` | base | `separation.violations` empty (no component pairs a device identity with a linkage value (CAMP 1.2.2 §5.2)) | [] | pass |
| `M0-sealed` | base | `sealed.mismatch` eq 0 (sealed SPDUs are opened only by their recipient (02 §8)) | 0 | pass |
| `M0-ee-peers` | base | `topology.ee_peers` subset ['ra', 'dcm'] (a device talks only to its RA, and to the DCM at the factory (01 §2.3)) | dcm,ra | pass |
| `M0-memory` | base | `cost.peak_mb` le 2500.0 (the owner's 8 GB machine; realism base 127-229 MB) | 222.758 | pass |
| `nohsm-verify-slower` | no-hsm vs M0 | `m.latency_stage.verify.mean` up > 20% (software ECDSA-P256 verify on an automotive application SoC is slower than the Craton2's verify engine (anchors::DEVICES; C4 primitive pr...) | 0.50653 -> 0.584345 | wrong-expectation — direction holds (+15 %); the pooled verify stage also counts the 30 RSUs and 26 handsets, whose hardware this setting does not change, and a vehicle's repeat costs 400 us (Craton2) against 645 us (SoC) |
| `nohsm-backend-same` | no-hsm vs M0 | `p2.certs_topped_up` same (identical) (receiver hardware does not change what the RA issues) | 1100 -> 1100 | pass |
| `nohsm-pdr` | no-hsm vs M0 | `m.pdr` same ±5% (crypto time is not on the air: the PHY outcome is unchanged within a few % (only transmit instants shift)) | 0.363836 -> 0.362838 | pass |
| `pq-security-bytes` | pq-hybrid vs no-hsm | `air.security` ratio [4.0, 20.0] (a Falcon-512 signature is ~666 B and its key 897 B, against 64 B + 33 B for P-256: hybrid security bytes several times ECDSA's (effects P...) | 2446825 -> 24072007 (ratio 9.838) | pass |
| `pq-fragments` | pq-hybrid vs no-hsm | `r.fragments_sent` ge 1 (a hybrid certificate frame exceeds the 1,400-octet WSMP MTU and is fragmented (post-quantum.yaml)) | 7968 | pass |
| `pq-channel-load` | pq-hybrid vs no-hsm | `m.channel_load.mean` up > 100% (airtime grows with the frame: channel load at least doubles (effects PQH: x4.2)) | 0.00361934 -> 0.0147291 | pass |
| `pq-pdr-down` | pq-hybrid vs no-hsm | `m.pdr` down (longer frames: more collisions, more fragments to lose (a fragmented SDU needs every fragment)) | 0.362838 -> 0.343057 | pass |
| `pq-provisioned` | pq-hybrid vs no-hsm | `p2.vehicles_starved` eq 0 (hybrid credentials are issued (CredentialFactory PQ path); nobody starves in two days) | 0 | pass |
| `pq-verify-load` | pq-hybrid vs no-hsm | `m.latency_stage.verify.mean` up (hybrid verify = ECDSA verify + Falcon verify (the two published costs together)) | 0.584345 -> 0.827119 | pass |
| `rtk-honest` | gnss-rtk vs M0 | `p2.revoked_honest` eq 0 (with 0.26 m errors consecutive BSMs agree to well inside the 1 m gpsDrift (SM-MBR RandomPosition): no honest report reaches the MA) | 26 | open — RTK leaves 26 honest revoked: the false positives are not GNSS error. R-USFP: 7,200 of the 7,488 reports at the MA are RandomPosition (positionJump), 7,375 of them about honest vehicles; 36 honest reported, 26 revoked. gnss-rtk still revokes 26, so the raw GNSS error is not the cause (07 section 8.3 said it was); see matrix.md 'Open' |
| `rtk-honest-reports` | gnss-rtk vs M0 | `p2.reports_about_honest` down > 50% (as rtk-honest) | 7375 -> 6248 | open — -15 % only. R-USFP: 7,200 of the 7,488 reports at the MA are RandomPosition (positionJump), 7,375 of them about honest vehicles; 36 honest reported, 26 revoked. gnss-rtk still revokes 26, so the raw GNSS error is not the cause (07 section 8.3 said it was); see matrix.md 'Open' |
| `rtk-attackers` | gnss-rtk vs M0 | `p2.revoked_attackers` eq 3 (ConstantPosition does not depend on the honest GNSS error) | 1 | open — as M0-attackers-revoked |
| `clean-honest-reports` | gnss-clean vs M0 | `p2.reports_about_honest` down (removing the 12 m outliers and the bursts removes the biggest jumps; the 1.6 m white noise remains) | 7375 -> 7360 | pass |
| `ond-verifications` | on-demand vs M0 | `rv.checked` down > 50% (on-demand verifies only what an application acts on (Krishnan & Weimerskirch 2011; 07 §2: 756/s -> 46/s)) | 77283 -> 37580 | pass |
| `ond-reports` | on-demand vs M0 | `p2.reports_at_ma` down > 30% (SM-MBR judges only verified BSMs (07 §8.2): detection is nearly blind) | 7488 -> 3632 | pass |
| `ond-backend-same` | on-demand vs M0 | `p2.certs_topped_up` same (identical) (the receiver policy does not change provisioning) | 1100 -> 1100 | pass |
| `camp-lop` | camp vs M0 | `edge.ee->lop` ge 1 (the CAMP reference design puts the LOP between the device and the RA (CAMP 1.2.2 §5.2)) | 7848 | pass |
| `camp-one-provider` | camp vs M0 | `verified.from_b` eq 0 (one SCMS: no second provider's pseudonyms on the air) | 0 | pass |
| `camp-no-sharing` | camp vs M0 | `p2.reports_shared` eq 0 (a central MA has no peer to share with) | 0 | pass |
| `camp-topups` | camp vs M0 | `p2.certs_topped_up` same (identical) (the topology does not change the batch (20 a week) or who is present at the rollover) | 1100 -> 1100 | pass |
| `camp-separation` | camp vs M0 | `separation.violations` empty (separation of duties holds in the CAMP design too (the LOP sees addresses, not linkage)) | [] | pass |
| `testctl-same-topups` | test-ctl vs M0 | `p2.certs_topped_up` same (identical) (the test trust list approves both providers' roots: issuance is unchanged) | 1100 -> 1100 | pass |
| `testctl-attackers` | test-ctl vs M0 | `p2.revoked_attackers` same (identical) (as testctl-same-topups: detection and revocation are unchanged) | 1 -> 2 | wrong-expectation — us-test-ctl-2025 deploys six providers at 1/6 each (devices 10/21/18/11/14/...), so the fleet split and MA routing change, not only the trust list |
| `split-b-down` | split-90-10 vs M0 | `verified.from_b` ratio [0.05, 0.45] (provider-b's fleet share 50 % -> 10 %: its signed BSMs fall to about a fifth) | 56582 -> 12393 (ratio 0.219) | pass |
| `split-a-up` | split-90-10 vs M0 | `verified.from_a` ratio [1.4, 2.4] (provider-a's share 50 % -> 90 %: about x1.8) | 22692 -> 66881 (ratio 2.947) | wrong-expectation — the base's verified traffic is not split like its devices (49/46 devices, 29 %/71 % of verified BSMs from a/b); 90/10 gives a 84 %: x2.95 is that shift |
| `split-topups` | split-90-10 vs M0 | `p2.certs_topped_up` ratio [0.9, 1.1] (the split moves devices between providers; the total top-off is the same 20 per device) | 1100 -> 1100 (ratio 1) | pass |
| `split-req` | split-90-10 vs M0 | `perdev.EeRaCertRequestSpdu.median` eq 1 (ra-maxReqs = 1 at either provider) | 1 | pass |
| `lte-latency` | lte vs M0 | `m.e2e_latency.bsm.mean` up (LTE-V2X mode 4 SPS waits for its reserved subframe (selection window up to 100 ms, mean ~half the RRI) where DSRC contends sub-ms (3GPP T...) | 12.3571 -> 68.0006 | pass |
| `lte-latency-budget` | lte vs M0 | `m.e2e_latency.bsm.p95` le 100.0 (SAE J3161/1: the BSM latency budget is still 100 ms) | 112.184 | open — LTE-V2X BSM p95 112 ms (mean 68 ms) exceeds the 100 ms budget: the BSM is generated without regard to the SPS reservation, which J3161/1 has the application align with; DSRC p95 14.6 ms |
| `lte-backend-same` | lte vs M0 | `p2.certs_topped_up` same (identical) (the access radio does not change the backend flows (cellular Uu and RSU backhaul unchanged)) | 1100 -> 1100 | pass |
| `lte-sizes` | lte vs M0 | `pdu.AcaEeCertResponsePrivateSpdu.max` same (identical) (the SCMS PDUs are radio-independent) | 517 -> 517 | pass |
| `nr-backend-same` | nr vs M0 | `p2.certs_topped_up` same (identical) (as lte-backend-same) | 1100 -> 1100 | pass |
| `nr-pdr` | nr vs M0 | `m.pdr` band [0.1, 0.95] (NR-V2X PC5 is plausible in the same urban geometry) | 0.604931 | pass |
| `nyc-changes` | nyc-pilot vs M0 | `p2.pseudonym_changes` up (NYC pilot: 5 min OR 2 km ([PRIMER] pp.8-9); a vehicle alive across the fast-forward is 43,000 s old at the second window and changes; J29...) | 0 -> 65 | pass |
| `nyc-backend-same` | nyc-pilot vs M0 | `p2.certs_topped_up` same (identical) (the change strategy uses the pool the RA issued; the top-off is weekly whatever the strategy) | 1100 -> 1100 | pass |
| `silent-frames` | silent vs M0 | `p2.pseudonym_silenced_frames` up (a silent period suppresses transmissions around each change (Huang et al. 2005); the base's J2945/1 rule silences nothing) | 0 -> 0 | wrong-expectation — `silent` means no scheduled change (wiring.rs rotation_policy), not a silent period: the run is the base's (23,549 frames, 0 changes) |
| `fullcert-bytes` | full-cert vs M0 | `air.security` ratio [1.2, 2.5] (a 77 B implicit certificate replaces the 8 B digest on 9 of 10 BSMs: ~+69 B on ~100 B of security) | 2446453 -> 3863553 (ratio 1.579) | pass |
| `fullcert-share` | full-cert vs M0 | `m.full_cert_share` ge 0.95 (digest_otherwise false: every signed frame carries the certificate) | 1 | pass |
| `fullcert-p2pcd` | full-cert vs M0 | `m.p2pcd_requests` le 0.0 (with a certificate on every frame no receiver needs to ask for one (1609.2a §8.2.4)) | 0 | pass |
| `fullcert-backend-same` | full-cert vs M0 | `p2.certs_topped_up` same (identical) (attachment is a transmit choice) | 1100 -> 1100 | pass |
| `rsu5-app-certs` | rsu5 vs M0 | `p2.rsu_app_certs_renewed` eq 5 (one weekly application-certificate renewal per RSU (B3b, 169 h)) | 5 | pass |
| `rsu5-reports` | rsu5 vs M0 | `p2.reports_from_rsus` down (fewer RSUs hear and file fewer reports) | 3616 -> 503 | pass |
| `rsu0-no-rsu-reports` | rsu0 vs M0 | `p2.reports_from_rsus` eq 0 (no RSU, nothing filed by one) | 0 | pass |
| `rsu0-topups` | rsu0 vs M0 | `p2.topups_completed` le 55 (the 20 % with no cellular reach the RA only through an RSU (relay); with none they cannot top off) | 51 | pass |
| `rsu0-no-app-certs` | rsu0 vs M0 | `p2.rsu_app_certs_renewed` eq 0 (no RSU) | 0 | pass |
| `pen0-no-cellular` | pen0 vs M0 | `access.cellular_vehicles` eq 0 (penetration 0: no device has a cellular link) | 0 | pass |
| `pen0-no-cell-uploads` | pen0 vs M0 | `p2.reports_uploaded_cellular` eq 0 (reports go up through the RSUs only) | 0 | pass |
| `pen0-topups` | pen0 vs M0 | `p2.topups_completed` le 55 (top-off only within an RSU's reach (provisioning-proxy role)) | 47 | pass |
| `bhcell-rsu-cert` | backhaul-cellular vs M0 | `flow.rsu-application-cert.median_s` up (cellular backhaul has more delay than fibre: the RSU's renewal round trips take longer) | 60.0268 -> 60.0984 | pass |
| `bhcell-counts` | backhaul-cellular vs M0 | `p2.rsu_app_certs_renewed` same (identical) (the backhaul changes delay, not the number of renewals) | 30 -> 30 | pass |
| `half-topups` | fleet-half vs M0 | `p2.certs_topped_up` down > 20% (fewer vehicles present at the rollover; still 20 each) | 1100 -> 780 | pass |
| `half-req` | fleet-half vs M0 | `perdev.EeRaCertRequestSpdu.median` eq 1 (per-device behaviour is fleet-independent) | 1 | pass |
| `half-verify` | fleet-half vs M0 | `rv.checked` down (fewer neighbours, fewer verifications) | 77283 -> 36486 | pass |
| `att10-revoked` | att10 vs M0 | `p2.revoked_attackers` up (about 7 attackers instead of 3, all ConstantPosition) | 1 -> 3 | pass |
| `att10-crl` | att10 vs M0 | `m.crl_entries` up (one CRL entry per revoked device: the CRL grows (~33 B an entry, Brecht)) | 27 -> 27 | open — 27 CRL entries in M0, att10 and sybil alike: att10 revokes 3 of 9 attackers (5 reported) and 24 honest. A constant 27 decisions across three detection settings is unexplained |
| `att10-cases` | att10 vs M0 | `p2.cases_opened` up (one case per accused device (gap row 27: cases run side by side)) | 27 -> 27 | open — as att10-crl: 27 cases in M0, att10, sybil |
| `sybil-not-revoked` | sybil vs M0 | `p2.revoked_attackers` eq 0 (the SM-MBR PSID-0x20 profile has no Sybil check (us_mbr.rs: no heading, staleness, Sybil, map or frequency check): a known detection gap) | 1 | wrong-expectation — the Sybil ghosts are signed under the attacker's own pseudonyms and their positions jump, so RandomPosition catches it (2,050 reports about attackers against 308 in M0); 1 of 3 revoked |
| `malegacy-cases` | ma-legacy vs M0 | `p2.cases_opened` changes (the legacy window revokes on three trusted reporters within a window; the case count is the policy's (scms-realism.yaml R-MA)) | 27 -> 29 | pass |
| `tls12-same` | tls12 vs M0 | `p2.certs_topped_up` same (identical) (TLS 1.2 adds a round trip, not a message; the top-off is unchanged) | 1100 -> 1100 | pass |
| `tls12-bytes` | tls12 vs M0 | `access.backend_bytes` same (identical) (cell-capacity-mm1 carries no TCP or TLS (base tcp_connections 0; scms-knobs: only packet-pf does), so the TLS version cannot show) | 17478789 -> 17478789 | pass |
| `real-same-topups` | real-crypto vs M0 | `p2.certs_topped_up` same (identical) (I-S1: real and modelled crypto give the same records (scms_scale.rs)) | 1100 -> 1100 | pass |
| `real-same-revocations` | real-crypto vs M0 | `p2.revocations` same (identical) (I-S1) | 27 -> 27 | pass |
| `real-same-frames` | real-crypto vs M0 | `r.frames_transmitted` same (identical) (I-S1: identical frames) | 23549 -> 23549 | pass |
| `real-same-sizes` | real-crypto vs M0 | `pdu.EeRaCertRequestSpdu.max` same (identical) (I-S1: the real encoder sizes do not depend on the crypto mode) | 422 -> 422 | pass |
| `real-cost` | real-crypto vs M0 | `cost.cpu_s` up (real P-256 arithmetic for every certificate (~0.4 CPU-s a device on release)) | 165.79 -> 190.48 | pass |
| `cert60-x3` | cert60 vs M0 | `p2.certs_topped_up` ratio [2.99, 3.01] (60 a week instead of 20 (NYC taxis, CVP-P2 item 21)) | 1100 -> 3300 (ratio 3) | pass |
| `ond-crl-earlier` | crl-on-decision vs M0 | `stage.enforced_s` down > 10% (publishing at the decision skips the wait for Wednesday's 10:00 slot (realism: -32,334 s)) | 147595 -> 115195 | pass |
| `fleet2x-req` | fleet2x vs M0 | `perdev.EeRaCertRequestSpdu.median` eq 1 (per-device behaviour is fleet-independent) | 1 | pass |
| `maauto-decision` | ma-auto vs M0 | `stage.decision_s` delta [-28900.0, -28700.0] (no 8 h review (D3; realism: -28,800 s)) | 115187 -> 86387.9 (delta -2.88e+04) | pass |
| `maauto-published` | ma-auto vs M0 | `stage.published_s` same ±0% (decided Tue 17:00 EDT, after that day's slot: still Wednesday's slot) | 147582 -> 147582 | pass |
| `nosharing-shared` | no-sharing vs M0 | `p2.reports_shared` eq 0 (sharing off) | 0 | pass |
| `offctl-honest` | off-ctl vs M0 | `p2.revoked_honest` le 0 (a provider off the trust list cannot get anyone honest revoked) | 7 | open — 7 honest revoked: the US-profile false positives (R-USFP), not the trust change. R-TRUSTLEAK grew: 280 provider-b messages verified at provider-a receivers (was 7) |
| `west-counts` | region-west vs M0 | `p2.certs_topped_up` same (identical) (a farther region changes latency, not counts) | 1100 -> 1100 | pass |
| `research-honest` | research-suite vs M0 | `p2.revoked_honest` eq 0 (the research suite's streak gate and stated-accuracy tolerance (realism: 0 honest)) | 0 | pass |
| `research-attackers` | research-suite vs M0 | `p2.revoked_attackers` eq 3 (realism: 3 of 3) | 3 | pass |

Totals: fixed 1, open 10, pass 82, wrong-expectation 5 of 98 rows
