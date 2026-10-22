# R-USFP: why the US misbehaviour profile revoked honest vehicles

Round: SCMS realism fix wave, 2026-10-22, on main `176d6f6b`. Scenario
`scenarios/real-world/midtown-scms-realism.yaml` (the matrix base `M0`), seed `0xBE7C0001`,
`--record-level events`. "Before" is the matrix's own `M0` run. "After" is an opt-level-2
debug build of this branch. Analysis scripts work on the recordings' `node.tx` (the claimed
position, speed, heading and `t_generated` of every BSM), `ma.report`, `ma.decision` and
`gt.*` channels. They are not committed.

## 1. Cause

The detector is right, and the GNSS output it was fed was not realistic.

- **The check matches SM-MBR v1.0 §2.3.** RandomPosition is `|p₁ + v₁·Δt − p₂| > gpsDrift`
  over two consecutive BSMs of one certificate, with gpsDrift = 1 m, metres and seconds,
  heading in ENU. Over the recorded BSMs, the residual has the same size along-track and
  cross-track, and the same size at 0 m/s and at 20 m/s. A heading or unit error would
  grow with speed, so the prediction is not the problem. The 0.5 s gap limit,
  `generationTime` as the time base, and "verified BSMs only" are all as specified.
- **What fails is the input.** `gnss/error/gauss-markov` draws its white term (1.6 m per
  axis, the MathWorks `gpsSensor` default) fresh at every estimate, so it is new at every
  100 ms mobility step. Two BSMs 100 ms apart then differ by about 2.3 m per axis. Its own
  card said "receiver filtering dynamics are not modelled".
- **RTK did not help, because the white term is still there.** RTK scales the white term
  only to about 0.5 m per axis.

| consecutive BSMs, realism base (21,227 pairs) | RandomPosition residual p50 | p99 | over 1 m |
|---|---|---|---|
| raw fixes (before, `M0`) | 2.65 m | 25.8 m | 18,515 (87 %) |
| raw fixes, RTK (`gnss-rtk`) | 0.88 m | 9.3 m | 8,902 (42 %) |
| smoothed receiver (after) | 0.02 m | 1.16 m | 418, all from attacker node 62 |

Honest pairs after the fix: p90 is 0.11 m or less in every speed band, and none is over 1 m.

A deployed receiver outputs its navigation solution, not raw code fixes. It carries the
solution forward with its own Doppler velocity and blends the code fix in slowly: the
carrier/Doppler smoothing of code (Hatch 1982; RTCA DO-229 uses a 100 s time constant).
SM-MBR's 1 m gpsDrift assumes exactly that kind of output.

## 2. Fix

`gnss/error/gauss-markov` 1.1.0 adds two receiver-output settings, both off by default, so
every other scenario runs exactly as before.

- **`smoothing_tau_s`.** The output follows `p̂ₖ = p̂ₖ₋₁ + vₖΔt + β(zₖ − p̂ₖ₋₁ − vₖΔt)`,
  with β = 1 − e^(−Δt/τ). Reid 2019 measured receivers' outputs, so the whole fitted σ moves
  to the slow (bias) process. The white term becomes filter input noise, and about 4 cm of
  it reaches the output.
- **`static_hold_mps`.** This is u-blox's static hold: below the threshold the receiver
  holds its position and reports 0 m/s.

The hold is needed. Smoothing alone puts the SM-MBR ConstantPosition condition (speed above
0.02 m/s with less than 1 cm of movement) on a stopped car's Doppler noise. The
`a_stopped_smoothing_receiver_holds_still` test counts more than 1,000 such pairs without
the hold.

`midtown-scms-benchmark.yaml`, which every US preset inherits from, now sets τ = 100 s and a
0.3 m/s hold. Both values are ASSUMPTIONS on cited mechanisms. `matrix/gnss-raw.yaml` keeps
the raw fixes.

## 3. Same seed, before and after

| run | honest revoked | attackers revoked | reports at MA (about honest) | MA decisions | evidence-time refusals |
|---|---|---|---|---|---|
| M0 before | 26 | 1 of 3 | 7,488 (7,375) | 27 | 10 |
| `gnss-raw` (this build, raw fixes) | 26 | 1 | 7,488 (7,375) | 27 | 10 |
| **M0 after** | **0** | **2 of 3** | 235 (5) | 2 | 0 |
| `research-suite` after | 0 | 3 | 243 (8) | 3 | 0 |
| `att10` after (9 attackers) | 0 | 4 | 621 (5) | 4 | 0 |
| `sybil` after | 0 | 2 | 1,570 (5) | 2 | 0 |

`gnss-raw` reproduces the before run to the report on this build, so the receiver output
alone accounts for all of R-USFP.

**The attacker the US profile misses is a handset.** Attacker actor 103 (node 46) sends
PSMs. SM-MBR v1.0 defines reports for application PSID 0x20 (the BSM) only (§1, §2.3), and
`us_mbr` correctly skips PSMs. The research suite catches this attacker through its
stale/replay check.

This is a real detection gap of the US profile, and the matrix kept it from view. A
handset or other non-BSM device can lie about its position with no report defined against
it. In att10, the three handset attackers go unreported for the same reason. The other
two attackers that were not revoked are vehicles with short exposure. Actor 140 was heard
by only 2 reporters. Actor 160 was only in the second live window.

Every vehicle attacker heard by at least 3 reporters is revoked: 2 of 2 in M0, and 4 of 6
in att10.

## 4. The "27 decisions in every variant"

They are the R-USFP false positives themselves. M0 and att10 decide the same 27 **actors**:
the 26 honest vehicles plus attacker 119. The pseudonym digests differ between runs, so
comparing digests had shown only 7 in common. Every variant keeps the same seed, the same
traffic and the same per-node GNSS RNG streams. So the same honest vehicles' raw-GNSS jumps
are heard by the same RSUs.

RSUs are always-trusted reporters. Most vehicle reporters had gone far past the corroborating
MA's report budget of 30 (subject, event) pairs: 38 of 52 filed more than 30 reports. After the fix, decisions follow the
attackers: 2, 4 and 2.

## 5. The 10 reports with much older evidence

All 10 are in the raw-fix population. They reproduce exactly on `gnss-raw`: 10 reports, a
lag of 43,125.104 s. They vanish with the fix: 0 in every after run.

The lag equals the gap between a certificate's last BSM of the first live window (59.912 s)
and the moment of a report filed in the second window (43,185.016 s). So the report quotes
the certificate's BSM from before the fast-forward.

Which receiver path produced a report about a BSM older than its own 2 s relevance window
was not found by reading. NOT ROOT-CAUSED. The MA's evidence-time rule set those reports
aside correctly.

Possibly related, and noted for the evidence-integrity work: a real SM-MBR's evidence is
`Phase2::evidence`, the last BSMs **on the air** from that certificate when the report is
built. It is not the BSMs the reporter judged.

## 6. Residuals

- 5 reports about honest vehicles remain in each after run, from 4 reporters about one
  vehicle, none revoked. The most likely source is two BSMs generated inside one mobility
  step (a DCC or dynamics trigger, with Δt under 0.1 s) by a car slower than 1 m/s. They
  then move less than 1 cm. Not investigated further.
- Pre-fix numbers in the expectation tables are kept as measured. The rows whose verdict
  this changes are re-stated in `scms-matrix.yaml` and matrix.md.
