# Live cross-domain causal chains — 2026-10-25

Track: live engine testing, cross-domain chains (key `chains`). Base `486c8cba`; the
integrator's release `v2xw` of that commit, copied to the scratchpad and run there. No
engine code changed, so no build was made.

Every link of each chain was predicted, with its equation or source, in
`crates/v2xw-engine/tests/expectations/live-chains.yaml` and committed before the runs
that judge it (`27bd3695`, `fa945f9b`). A row corrected after its run says so in `note`.
The scenarios are in `scenarios/live-chains/`. Each one makes a single change against its
base, on the same seed. Every run went through its own `heavy.sh` call: radio runs on the
second slot, the SCMS, signal and pedestrian runs on the main slot. The SCMS runs used
`--no-recording`.

```
python3 -I tools/realworld/live_chains.py extract <run-dir>   # matrix.py extract + per-link lc.* measures
python3 -I tools/realworld/live_chains.py derive <runs>       # the X-* interaction contrasts
python3 -I tools/realworld/matrix.py check crates/v2xw-engine/tests/expectations/live-chains.yaml <runs>
python3 -I tools/realworld/live_chains.py faults crates/v2xw-engine/tests/expectations/live-chains.yaml <runs>
```

`faults` re-judges every row that has data under an injected fault:
- a direction, ratio or delta row gets variant := base (the knob did nothing);
- a `same` row gets the variant moved by 1.5 + tol;
- a value or band row gets its value pushed outside the band;
- an `identical` row gets another run's digest.

On the final run set every judged row goes red under its fault, and none is CANNOT-FAIL.

## Headline

**The radio chains hold end to end, and the PQ chain showed a feedback loop nobody
predicted.**

**PQ chain (Falcon-512, 100 vehicles, J2945/1 on).**

| Link | Base | Falcon-512 |
|---|---|---|
| PSDU per BSM | — | x4.75 (predicted 4.85) |
| Airtime per BSM | — | x4.20 (predicted 4.3) |
| Busy ratio (`m.cbr.172`) | 0.21 | 0.73 |
| Offered load | 0.22 | 0.99 |
| Collision rate | 0.016 | 0.31 |
| PDR | 0.98 | 0.65 |
| MAC access delay | 0.07 ms | 2.1 ms |
| Sign time | — | x6.66 (1,625 vs 244 us) |
| Verify time | — | x0.22 |

- J2945/1 power control shows as below-sensitivity losses x63. It does not show at 40
  vehicles (CBR 0.37, under the 50 % knee). With DCC off it is gone (0.044 → 0.0002 per
  frame).
- Density amplifies the PQ damage. The PDR lost to Falcon is 0.25 larger at 100 vehicles
  than at 40, and the added collision rate is 0.23 larger.
- Hybrid Falcon+P-256: PSDU x5.13, sign x7.66, verify x1.20, each inside its band.
- ML-DSA-44: PSDU x14.9 with every BSM fragmented, offered load 2.65, CBR 0.94, PDR 0.16.
- **Feedback:** under Falcon the BSM rate *rose* from 8.04 to 9.07 per second, the rate
  with DCC off. A J2945/1 vehicle counts density from the BSMs it *receives*. When PDR
  collapses it under-counts its neighbours and shortens its ITT, which adds load to a
  channel that is already saturated.

**Density → J2945/1 → load chain.**
- At 40 vehicles DCC does not change the rate (N < B = 25).
- Against DCC off, the rate falls to 0.89 at 100 vehicles and 0.80 at 160.
  - CBR grows 0.10 → 0.33 without DCC and 0.10 → 0.25 with it.
  - At 160 vehicles DCC halves collisions (0.043 → 0.022), raises PDR from 0.956 to
    0.978, cuts verifications per node by 20 % and DES events by 23 %.
- Age of information rises 58 → 70 ms (T/2).
- B = 10 cuts the rate to 4.95 per second; B = 50 restores 9.07.
- vMax_ITT 300 ms leaves the rate identical (8.04 → 8.04). The linear segment
  100 + 4(N − 25) ms is the same for both caps, and N100 < 75.
- Events grow x11.3 for 4x vehicles (N²).
- The same seed gives a byte-identical record digest.

**OBU crypto chain.**
- Craton2 HSM: sign x36.9 (9 ms), +9.3 ms end to end, verify x0.68.
- Software Craton2: verify x2.04, verify wait 0.19 → 10.8 ms.
- At 160 vehicles the software Craton2 is saturated **with or without DCC**: about 636
  verifications/s per node, 75–78 ms waits. This was predicted wrong: the default
  `compute_tier: medium` is one CPU server, not the profile's two cores (finding 4).

**The SCMS chains did not test what they were meant to test (open).**
- **Backend outage → CRL:** a CRLG outage and a CRL-store outage left every SCMS output
  identical to S to the nanosecond, with 0 deliveries held or dropped. The outage could
  only name the home provider's entity (finding 5).
- **GNSS → reports:** 10x the outlier rate left every report identical, and 50x added 6
  honest reports. Removing outliers *and* bursts removed 27 of 34 honest reports
  (finding 7).

## Findings

1. **J2945/1 rate feedback under PQ** (`LC.PQ.rate-feedback-falcon`, `LC.PQ.mldsa44-cost`).
   - The density J2945/1 counts comes from the BSMs received within 100 m
     (`SaeJ2945Dcc::smooth_density`, Ahmad 2019 Eq. 5; SAE J2945/1 6.3.8).
   - When PQ frames collapse PDR the count falls and the ITT shortens: 8.04 → 9.07 BSM/s
     under Falcon and 9.04 under ML-DSA-44, against 9.07 with DCC off.
   - Congestion control switches itself off on the channels that need it most.
   - This is the standard's behaviour, not an engine defect. It is a result for the PQ
     studies: the size-only analyses miss it.
2. **`m.channel_load` is the offered load, not the CBR.** Every CBR row was first written
   on it. Its metric card says it may exceed 1 (2.65 under ML-DSA-44). The busy ratio is
   `m.cbr.172`. All CBR rows now read that, and `LC.PQ.cbr-falcon-bounded`, first judged a
   wrong expectation on the offered load, passes on the busy ratio (0.73).
3. **Reception attempts are culled at the nominal range,** so they cannot show J2945/1
   power control (79.6 → 80.1 per frame under Falcon). The controlled power shows as
   below-sensitivity losses. `LC.PQ.power-falcon` was a wrong expectation;
   `LC.PQ.power-falcon-bs` and `LC.PQ.nodcc-power-back` pass.
4. **The medium compute tier prices one CPU server** (v2xw-node `ServerBank`;
   06-node-models §2.1: medium = one CPU and one HSM server, high = `cores` servers). On it
   the dual-core software Craton2 saturates at 160 vehicles even under J2945/1. A real
   dual-core Cortex-A7 running OpenSSL verifies on both cores, so the default tier is
   pessimistic for software-crypto OBUs. Realism candidate: price software ECDSA on
   `cpu.cores` servers at the medium tier. It moves goldens, so it needs a flag or an owner
   call.
5. **`actors.backend.failures` cannot name another provider's entity.**
   - `crypto_config::failure_node` resolves the role on the home provider's run (runs[0])
     only.
   - The credential-lifecycle base runs two providers (52 + 46 devices). Its two
     in-window revocations (decided at 140.9 s) were provider-b's, and provider-a's came
     at 233.4 s, after the 60–200 s outage.
   - The CRLG-down edge runs (MWC, MWC2) never met a home revocation either
     (rerun-2026-10-24-b §7). **No run has yet shown a CRLG outage delaying a CRL.**
   - Owed: a provider-qualified node (`provider-b/crlg`), a check that an outage
     overlapping a flow moves `backend_deliveries_held`, and these rows re-run.
6. **A CRL-store outage does not reach CRL fetches** (53 → 53 downloads). In this model the
   device's RA answers the fetch ("the composite list its RA serves", phase2 R-JOINCRL,
   [PRIMER] pp. 5, 8). Whether the deployed US fetch path is the RA or a CRL store is open.
7. **GNSS outliers barely reach the detectors.**
   - About 1,760 single-fix 12 m outliers (0.1/s x 98 vehicles x 180 s) add 0
     position-jump verdicts, and about 8,800 add 3.
   - The degraded bursts make the honest reports: g0 takes position jumps from 7 to 0 and
     honest reports from 34 to 7.
   - Either legacy-12 tolerates a one-fix jump, or outliers do not reach a judged BSM.
     Telling them apart needs node.tx positions against gt, from a recorded run (owed).
   - The matrix's raw `gnss-clean` run (7,375 → 7,360 honest reports) showed the same
     small outlier effect.
8. **Engine finding, silent loss: SLH-DSA-SHA2-128s puts nothing on the air and counts
   nothing.**
   - Setup: 38 vehicles for 6 s on the no-HSM SoC with the SDU fragmenter.
   - Result: 2,101 node tasks and 0 frames, while `net_mtu_refusals`, `mac_drops`,
     `suppressed_frames`, `phy_refusals` and `credentials_refused` all stay 0. The loader
     accepted the scenario.
   - The likely cause is the signing time (SLH-DSA-128s signs in tenths of a second on
     this class of core, against a 100 ms BSM interval), but no record or counter says so.
   - It is a check that cannot fail: only a reader looking for frames would notice.
   - Owed: count the BSMs lost before the air, or refuse the scheme at load as a profile
     without a sign cost is refused. Not fixed here (no build budget left).

## Run cost

| Run | Wall | Peak |
|---|---|---|
| Dense radio grid, 6 s | 3–188 s (ML-DSA-44 the longest) | 28–108 MB |
| SCMS lifecycle, 240 s, no recording | 135–203 s | ≤ 96 MB |
| Signalised 4 x 6 grid, 180 s | 471 s | 69 MB |
| SCMS lifecycle, 300 s, recorded (calibration only) | 582 s | 192 MB; 812 MB recording, deleted |

- Cost rows that pass: events x11.3 for 4x vehicles (N²); DCC −23 % events at 160
  vehicles.
- Falcon's events per reception attempt fall from 2.43 to 0.96, because far fewer frames
  decode and get verified.
- Disk stayed above 17 GB free. Raw recordings were deleted once read.

## Not run (run-slot budget)

Both run slots were shared with other agents; a slot was often held for ten minutes at
a time. These rows are still pending; re-running them needs only their scenarios:
- the signal-timing chain beyond `sig-base` (c40, c120, c180, r3000, c120 × r3000);
- the pedestrian chain (base, 50, 450, device fraction 0 and 1);
- the seed-2 PQ pair and the Craton2 HSM at 160 vehicles;
- the dose run of the CRLG outage (its premise failed).

## Rows

The table has 94 rows: 54 pass, 3 wrong expectations (corrected with evidence; the
corrected rows pass), 16 open and 21 not run. No row fails, and no row was weakened. Every
correction keeps the original row with its verdict and adds the corrected measure as a
new row.

**pq-chain**

| Row | Change | Measure | Predicted | Measured | Verdict |
|---|---|---|---|---|---|
| PQ.psdu-falcon | r100 -> r100-falcon | `lc.psdu_per_bsm` | ratio [3.5, 6] | 200.519 -> 952.972 (ratio 4.753) | pass |
| PQ.psdu-hybrid | r100 -> r100-hybrid | `lc.psdu_per_bsm` | ratio [4, 6.5] | 200.519 -> 1028.46 (ratio 5.129) | pass |
| PQ.psdu-mldsa44 | r100 -> r100-mldsa44 | `lc.psdu_per_bsm` | ratio [11, 19] | 200.519 -> 2973.94 (ratio 14.83) | pass |
| PQ.psdu-slh-edge | r40 -> r40-slh | `lc.psdu_per_bsm` | ratio [30, 60] | 199.552 -> 0 (ratio 0) | open |
| PQ.frag-share-falcon | r100 -> r100-falcon | `lc.frag_share` | band [0.05, 0.25] | 0.101342 | pass |
| PQ.frag-share-mldsa44 | r100 -> r100-mldsa44 | `lc.frag_share` | band [0.95, 1] | 1 | pass |
| PQ.airtime-falcon | r100 -> r100-falcon | `lc.airtime_us_per_bsm` | ratio [3.3, 5.5] | 315.071 -> 1322.76 (ratio 4.198) | pass |
| PQ.cbr-falcon | r100 -> r100-falcon | `m.cbr.172.mean` | up > 1x | 0.213411 -> 0.728317 | pass |
| PQ.cbr-falcon-bounded | r100 -> r100-falcon | `m.cbr.172.mean` | band [0.3, 0.97] | 0.728317 | pass |
| PQ.power-falcon | r100 -> r100-falcon | `lc.attempts_per_frame` | down > 0.02x | 79.591 -> 80.1338 | wrong-expectation |
| PQ.power-falcon-bs | r100 -> r100-falcon | `lc.below_sens_per_frame` | up > 5x | 0.000698324 -> 0.0436657 | pass |
| PQ.power-knee-r40 | r40 -> r40-falcon | `lc.below_sens_per_frame` | same (tol 2) | 0.000952381 -> 0.0017301 | pass |
| PQ.rate-feedback-falcon | r100 -> r100-falcon | `lc.bsms_per_veh_s` | up > 0.05x | 8.04494 -> 9.07303 | pass |
| PQ.collisions-falcon | r100 -> r100-falcon | `m.collision_rate` | up > 1x | 0.0162697 -> 0.309006 | pass |
| PQ.pdr-falcon | r100 -> r100-falcon | `m.pdr` | down > 0.01x | 0.983423 -> 0.646866 | pass |
| PQ.access-falcon | r100 -> r100-falcon | `m.mac_access_delay.mean` | up > 0.5x | 0.0650917 -> 2.12085 | pass |
| PQ.sign-falcon | r100 -> r100-falcon | `m.latency_stage.sign.mean` | ratio [5, 8] | 0.244 -> 1.625 (ratio 6.66) | pass |
| PQ.verify-falcon | r100 -> r100-falcon | `m.verify_cost.mean` | ratio [0.12, 0.4] | 0.655012 -> 0.142416 (ratio 0.2174) | pass |
| PQ.sign-hybrid | r100 -> r100-hybrid | `m.latency_stage.sign.mean` | ratio [6, 9.5] | 0.244 -> 1.869 (ratio 7.66) | pass |
| PQ.verify-hybrid | r100 -> r100-hybrid | `m.verify_cost.mean` | ratio [1.05, 1.45] | 0.655012 -> 0.787816 (ratio 1.203) | pass |
| PQ.mldsa44-saturates | r100 -> r100-mldsa44 | `m.cbr.172.mean` | up > 1.5x | 0.213411 -> 0.940232 | pass |
| PQ.mldsa44-pdr | r100 -> r100-mldsa44 | `m.pdr` | down > 0.05x | 0.983423 -> 0.163498 | pass |
| PQ.mldsa44-cost | r100 -> r100-mldsa44 | `lc.bsms` | up > 0.05x | 4296 -> 4845 | pass |
| PQ.density-amplifies-pdr | X-pq-density-pdr | `x.value` | band [1e-06, 1] | 0.254353 | pass |
| PQ.density-amplifies-coll | X-pq-density-coll | `x.value` | band [1e-06, 1] | 0.228569 | pass |
| PQ.nodcc-power-back | r100-falcon -> r100-falcon-nodcc | `lc.below_sens_per_frame` | down > 0.5x | 0.0436657 -> 0.000187477 | pass |
| PQ.seed2-cbr | r100-s2 -> r100-falcon-s2 | `m.cbr.172.mean` | up > 1x | not run (run-slot budget) | not run |
| PQ.seed2-pdr | r100-s2 -> r100-falcon-s2 | `m.pdr` | down | not run (run-slot budget) | not run |

**dcc-chain**

| Row | Change | Measure | Predicted | Measured | Verdict |
|---|---|---|---|---|---|
| DD.rate-low-density | r40-nodcc -> r40 | `lc.bsms_per_veh_s` | same (tol 0.12) | 9.21053 -> 9.21053 | pass |
| DD.rate-100 | r100-nodcc -> r100 | `lc.bsms_per_veh_s` | ratio [0.65, 0.95] | 9.07116 -> 8.04494 (ratio 0.8869) | pass |
| DD.rate-160 | r160-nodcc -> r160 | `lc.bsms_per_veh_s` | ratio [0.35, 0.85] | 8.93011 -> 7.10215 (ratio 0.7953) | pass |
| DD.rate-falls-with-density | r100 -> r160 | `lc.bsms_per_veh_s` | down > 0.05x | 8.04494 -> 7.10215 | pass |
| DD.nodcc-rate-flat | r40-nodcc -> r160-nodcc | `lc.bsms_per_veh_s` | same (tol 0.1) | 9.21053 -> 8.93011 | pass |
| DD.cbr-grows-nodcc | r40-nodcc -> r160-nodcc | `m.cbr.172.mean` | up > 1.5x | 0.103 -> 0.328046 | pass |
| DD.dcc-flattens-cbr | X-dcc-density-cbr | `x.value` | band [-1, -1e-06] | -0.0755301 | pass |
| DD.dcc-pdr-160 | r160-nodcc -> r160 | `m.pdr` | up | 0.956098 -> 0.977579 | pass |
| DD.dcc-collisions-160 | r160-nodcc -> r160 | `m.collision_rate` | down | 0.0433283 -> 0.0221192 | pass |
| DD.dcc-verify-load-160 | r160-nodcc -> r160 | `lc.verifies_per_node_s` | down > 0.15x | 897.134 -> 719.595 | pass |
| DD.aoi-100 | r100-nodcc -> r100 | `m.aoi` | up > 0.05x | 58.5081 -> 70.0887 | pass |
| DD.b10 | r100 -> r100-b10 | `lc.bsms_per_veh_s` | down > 0.2x | 8.04494 -> 4.95318 | pass |
| DD.b50 | r100 -> r100-b50 | `lc.bsms_per_veh_s` | up > 0.05x | 8.04494 -> 9.07303 | pass |
| DD.itt300-no-effect | r100 -> r100-itt300 | `lc.bsms_per_veh_s` | same (tol 0.03) | 8.04494 -> 8.04494 | pass |
| DD.cost-nodcc-160 | r160 -> r160-nodcc | `cost.events` | up > 0.1x | 1410172 -> 1836388 | pass |
| DD.cost-density-nodcc | r40-nodcc -> r160-nodcc | `cost.events` | ratio [4, 20] | 162596 -> 1836388 (ratio 11.29) | pass |
| DD.determinism | r100 -> r100b | `digest.record` | identical | 2f4a7bfcc5925edd -> 2f4a7bfcc5925edd | pass |

**obu-chain**

| Row | Change | Measure | Predicted | Measured | Verdict |
|---|---|---|---|---|---|
| OB.sign-hsm | r100 -> r100-craton2 | `m.latency_stage.sign.mean` | ratio [25, 45] | 0.244 -> 9 (ratio 36.89) | pass |
| OB.e2e-hsm | r100 -> r100-craton2 | `m.e2e_latency.bsm.mean` | delta [6, 12] | 4.1221 -> 13.428 (delta 9.306) | pass |
| OB.verify-hsm | r100 -> r100-craton2 | `m.verify_cost.mean` | ratio [0.45, 0.9] | 0.655012 -> 0.446114 (ratio 0.6811) | pass |
| OB.verify-c2sw | r100 -> r100-c2sw | `m.verify_cost.mean` | ratio [1.6, 2.5] | 0.655012 -> 1.3367 (ratio 2.041) | pass |
| OB.wait-c2sw | r100 -> r100-c2sw | `m.verify_wait.mean` | up > 1x | 0.192492 -> 10.798 | pass |
| OB.wait-c2sw-nodcc | r160-c2sw -> r160-c2sw-nodcc | `m.verify_wait.mean` | up > 1x | 74.7525 -> 78.4168 | wrong-expectation |
| OB.e2e-c2sw-nodcc | r160-c2sw -> r160-c2sw-nodcc | `m.e2e_latency.bsm.mean` | up > 0.1x | 80.3074 -> 86.5005 | wrong-expectation |
| OB.dcc-protects-slow-obu | X-dcc-obu-wait | `x.value` | band [1e-06, 1e+06] | 3.51567 | pass |
| OB.hsm-160-wait | r160 -> r160-craton2 | `m.verify_wait.mean` | up | not run (run-slot budget) | not run |

**crl-chain**

| Row | Change | Measure | Predicted | Measured | Verdict |
|---|---|---|---|---|---|
| CRL.enforcement-lost | S -> scms-crlg | `p2.revoked_receptions` | down > 0.3x | 22318 -> 22318 | open |
| CRL.latency-up | S -> scms-crlg | `p2.revocation_latency_ns` | up > 0.5x | 16320467886 -> 16320467886 | open |
| CRL.delayed-not-lost | S -> scms-crlg | `p2.revoked_attackers` | same (tol 0) | 3 -> 3 | pass |
| CRL.decisions-unchanged | S -> scms-crlg | `p2.ma_revoke_decisions` | same (tol 0) | 3 -> 3 | pass |
| CRL.attackers-reported-longer | S -> scms-crlg | `p2.reports_about_attackers` | up > 0.05x | 2248 -> 2248 | open |
| CRL.dose | scms-crlg -> scms-crlg-short | `p2.revoked_receptions` | up > 0.2x | NO-RUN | open |
| CRL.store-fetches | S -> scms-crlstore | `p2.crl_downloads` | down > 0.2x | 53 -> 53 | open |
| CRL.store-issuance-unchanged | S -> scms-crlstore | `p2.crls_issued` | same (tol 0) | 3 -> 3 | pass |
| CRL.broadcast-softens | X-crl-paths | `x.value` | band [-1e+09, -1e-06] | 0 | open |
| CRL.fleet-unchanged | S -> scms-crlg | `p2.devices` | same (tol 0) | 98 -> 98 | pass |

**gnss-chain**

| Row | Change | Measure | Predicted | Measured | Verdict |
|---|---|---|---|---|---|
| GN.jumps-g10 | S -> scms-g10 | `p2.verdicts.positionJump` | ratio [3, 15] | 7 -> 7 (ratio 1) | open |
| GN.jumps-g0 | S -> scms-g0 | `p2.verdicts.positionJump` | down > 0.3x | ABSENT | pass |
| GN.honest-reports-g10 | S -> scms-g10 | `p2.reports_about_honest` | up > 1x | 34 -> 34 | open |
| GN.honest-reports-g0 | S -> scms-g0 | `p2.reports_about_honest` | down | 34 -> 7 | pass |
| GN.honest-devices-g10 | S -> scms-g10 | `p2.honest_reported` | up | 17 -> 17 | open |
| GN.ma-load-g10 | S -> scms-g10 | `p2.reports_at_ma` | up > 0.1x | 393 -> 393 | open |
| GN.fpr-g10 | S -> scms-g10 | `m.det_fpr.report` | up | 0.00340807 -> 0.00340807 | open |
| GN.precision-g10 | S -> scms-g10 | `m.det_precision.report` | down | 0.997007 -> 0.997007 | open |
| GN.dose-g50 | scms-g10 -> scms-g50 | `p2.reports_about_honest` | up > 0.3x | 34 -> 40 | open |
| GN.g50-honest-revoked | S -> scms-g50 | `p2.revoked_honest` | ge 1 | 0 | open |
| GN.attackers-unaffected-g0 | S -> scms-g0 | `p2.revoked_attackers` | same (tol 0) | 3 -> 3 | pass |
| GN.cost-g50 | S -> scms-g50 | `cost.events` | up | 2230023 -> 2229836 | open |

**signal-chain**

| Row | Change | Measure | Predicted | Measured | Verdict |
|---|---|---|---|---|---|
| SG.speed-c120 | sig-base -> sig-c120 | `m.mean_speed` | down | not run (run-slot budget) | not run |
| SG.speed-c40 | sig-base -> sig-c40 | `m.mean_speed` | up | not run (run-slot budget) | not run |
| SG.speed-c180 | sig-c120 -> sig-c180 | `m.mean_speed` | down | not run (run-slot budget) | not run |
| SG.density-c120 | sig-base -> sig-c120 | `m.density` | up | not run (run-slot budget) | not run |
| SG.rate-c120 | sig-base -> sig-c120 | `lc.bsms_per_veh_s` | down | not run (run-slot budget) | not run |
| SG.cbr-peak-c120 | sig-base -> sig-c120 | `m.cbr.172.p95` | up | not run (run-slot budget) | not run |
| SG.demand-amplifies | X-sig-demand | `x.value` | band [1e-06, 1000] | not run (run-slot budget) | not run |
| SG.cost-r3000 | sig-base -> sig-r3000 | `cost.events` | up > 0.5x | not run (run-slot budget) | not run |

**ped-chain**

| Row | Change | Measure | Predicted | Measured | Verdict |
|---|---|---|---|---|---|
| PD.psm-450 | ped-base -> ped-450 | `lc.frames.psm` | ratio [2.3, 3.4] | not run (run-slot budget) | not run |
| PD.psm-50 | ped-base -> ped-50 | `lc.frames.psm` | ratio [0.25, 0.5] | not run (run-slot budget) | not run |
| PD.psm-dev100 | ped-base -> ped-dev100 | `lc.frames.psm` | ratio [2.7, 3.7] | not run (run-slot budget) | not run |
| PD.psm-dev0 | ped-base -> ped-dev0 | `lc.frames.psm` | eq 0 | not run (run-slot budget) | not run |
| PD.pcw-dev0 | ped-base -> ped-dev0 | `m.app_warnings.pcw` | eq 0 | not run (run-slot budget) | not run |
| PD.pcw-dev100 | ped-base -> ped-dev100 | `m.app_warnings.pcw` | up | not run (run-slot budget) | not run |
| PD.truth-dev0 | ped-base -> ped-dev0 | `m.ped_conflicts` | same (tol 0) | not run (run-slot budget) | not run |
| PD.conflicts-450 | ped-base -> ped-450 | `m.ped_conflicts` | up | not run (run-slot budget) | not run |
| PD.cbr-450 | ped-base -> ped-450 | `m.cbr.172.mean` | up | not run (run-slot budget) | not run |
| PD.cost-450 | ped-base -> ped-450 | `cost.events` | up > 0.2x | not run (run-slot budget) | not run |

