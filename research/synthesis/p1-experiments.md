# Paper 1 experiments

Each experiment states its falsification test (paper plan 2026-10 §1.3) before it runs and reports
pass or fail as measured. The §1.1 qualifiers apply to every sentence: the rule set is the
**published CV-Pilot** SM-MBR v1.0 / ASN.1 v1.1, never "deployed"; an MA result reads "**would
revoke under rule R, given trigger rate T**"; the chain is "first for the US SCMS chain".

## P1-E5, P1-E6, P1-E7 (engine validation track, 2026-10-26)

Binary: the integrator's release build of `f4982788`, copied to the scratchpad. Scenarios:
`scenarios/real-world/paper1/p1-e5-*`, `p1-e6-*`, `p1-e7-*` (plus the existing `p1-attack`,
`p1-q0`, `p1-q2`, `p1-q4`). Rows: `P1.E5.*`, `P1.E6.*`, `P1.E7.*` in
`crates/v2xw-engine/tests/expectations/scms-paper1.yaml`, committed with `measured: pending`
before the first run. Every run is on the Midtown map (the city map of the Paper 1 base) with a
**small fleet** (600 to 5,400 veh/h for 60 to 120 s), so every number below is a small-fleet
number, not a full-scale one.

### Pre-registration

**P1-E5, receiver-side gaps at density.**
- Design: 3 radio technologies (DSRC 802.11p, LTE-V2X PC5 mode 4, NR-V2X PC5 mode 2) x 3 loads
  (600, 1,800, 5,400 veh/h), 60 s (30 s warm-up, 30 s live), honest, CV-pilot timing classes at
  25 % locked, `--record-level receptions`. Measured by `tools/realworld/paper1/e5gaps.py`.
- Definitions: a stream is one pseudonym as one receiver decoded it; a gap is the time between
  two consecutive decoded BSMs of a stream on the receiver's clock; a gap over 10 s closes the
  stream. Tampa (`nm7w-nvbm`, RSU-received, one night) on the same definitions:
  34,186 gaps, 226 temp IDs, share in [50, 150) ms 0.759, share <= 0.5 s 0.953, p50 105 ms,
  p90 303 ms.
- **Falsification test:** the DSRC low-load RSU-received gap distribution must be within KS
  0.0747 of Tampa's. The tolerance was fixed before any E5 run: the 95th percentile of the KS
  distance between two random halves of Tampa's temp IDs (400 splits, seed 20261026). Secondary
  bands: one-interval share 0.759 +- 0.15; share <= 0.5 s 0.953 +- 0.05.
- Fault proof: the same KS on the engine's gaps with every second decoded BSM dropped must
  exceed the tolerance.
- Theory rows: the channel busy ratio at least doubles from load 1 to load 3; the reception
  ratio within 300 m and the share of verified BSMs the US profile can pair (secMark dt <= 0.5 s)
  fall by under 20 % and 10 %.
- Disclosure: one 40 s DSRC probe at 1,800 veh/h was run before the rows to learn the output
  format and peak memory (416 MB). Its gaps were not computed.

**P1-E6, store and upload crowd-out.**
- Design: one change at a time against `p1-q4` (the attack base, 3 % ConstPos from 20 s, at a
  4,500 B quota where the store crowds out): gpsDrift 0.5 / 2 / 5 m, locked share 1 / 0
  (free-class share 0 / 1), 10 / 60 RSUs, cellular share 0 / 1, plus quotas 0, 2,300, 4,500,
  16,384 and the default 65,536 B. `--record-level events`.
- **Falsification tests:** an attack-free run loses zero attack evidence (and names no
  attacker); a zero-quota run loses all of it and delivers nothing.
- **Offline replay** (`tools/realworld/paper1/e6replay.py`, added after the E6 runs started and
  before any E6 result was read): the default-quota run's fate log replayed through the §2.2
  store at 0, 2,300, 4,500 and 16,384 B. Its own falsification test: it must reproduce the
  engine at a zero quota exactly, and be within 0.05 of the engine's attack-evidence lost share
  at the other quotas (deletions within 20 % at 4,500 B). Only if it passes does the replay
  extend the quota sweep.
- Theory rows: attack-evidence lost share non-increasing in quota; fewer honest triggers
  (larger gpsDrift, more locked devices) crowd out no more; fewer RSUs or no cellular upload no
  less.

**P1-E7, the MA rule family surface.**
- Design: `p1-attack` at the default quota with the free-class share 0, 10, 25, 50, 75 and
  100 %, two master seeds each, then `v2xw ma-replay` (37 settings in 4 families).
- **Falsification test:** with the free class at 0 %, every sound rule (k >= 2, theta >= 2)
  revokes zero honest devices, on both seeds.
- Fault proof: at 100 % free some sound rule blocks an honest device; and some sound rule
  revokes an attacker at 0 % free.
- Reported: honest devices blocked per 10^6 device-hours against the attacker's time to act,
  per rule and free share. A zero is reported with its rule-of-three upper bound
  (3 / device-hours), because a small fleet has few device-hours.

### Changes to the plan after pre-registration (each committed before the result it affects was read)

- **E5 loads 2 and 3** (`f71eb1ae`): the 1,800 veh/h 60 s run hit the 1.2 GB run-slot cap, so loads
  2 and 3 are 1,800 and 3,600 veh/h over 40 s (10 s live). Load 1 stays at 600 veh/h over 60 s.
- **E5 fading follow-up** (`9a1209cf`): after E5-D1 failed its KS test, a pre-registered
  hypothesis run with the high propagation tier (Nakagami-m fading).
- **E6 offline replay** (`4244e34d`): written after the E6 runs started, before any E6 result
  was read.
- **E7C** (`c04b8efb`): after E7-F0 failed its null, a pre-registered re-run on the calibrated
  receiver (`gnss.velocity_noise_mps: 0`, as the final P1-E4 run) and on an engine with the
  stop fix (`ffaebbf9`, below).

### Results

**Verdict.** 57 rows: 43 pass, 2 fixed, 2 wrong expectations (corrected with evidence in the
row), 10 open.
- **P1-E5 fails** its pre-registered KS test: the engine's RSU-received gaps are cleaner than
  Tampa's.
- **P1-E6 passes** both falsification tests, and the offline store replay passes its own.
- **P1-E7's null failed** on the as-written scenarios. The cause was one engine defect, fixed
  here, plus an uncalibrated receiver. On the calibrated re-run the null holds for every rule
  with k >= 3 and fails only for k = 2 rules, through real-data-consistent ConstantPosition.

Rows are judged by `tools/realworld/matrix.py`'s judge on each run's `realism.json`. All passing
rows went red under `tools/realworld/variants_faults.py` (37 red, 0 silent). Real faults also went
red: the E5 drop-every-second-BSM KS (0.777), zero quota (all lost), the all-free MA runs, and the
new unit test without its fix. Same seed gives a byte-identical record digest (E5-D1/D1b,
E7-F0/F0b, E7C-F0/F0b).

**P1-E5: fail as pre-registered.**
- **The falsification test fails.** The DSRC low-load RSU-received gap distribution is KS 0.129
  from Tampa, over the 0.0747 tolerance. The secondary bands pass: one-interval share 0.898 (band
  0.609-0.909), share <= 0.5 s 0.981.
- **The shape of the miss.** The engine's RSU gaps are too clean. Engine against Tampa:
  - one interval 0.898 vs 0.759; two intervals 0.060 vs 0.112; 3-5 intervals 0.023 vs 0.065;
  - p90 151 vs 303 ms;
  - under 50 ms: 0 vs 0.016.
- **Losses are all-or-nothing.** Within 300 m only 0.35 of BSM receptions succeed, but the gaps
  inside a contact are short. So the losses are whole links that buildings block (96,529
  below-sensitivity losses), not intermittent ones.
- **The fading hypothesis is refuted.** It was pre-registered after the failure. With the high
  propagation tier (Nakagami-m fading plus diffraction), KS is 0.149 and the one-interval share
  rises to 0.919, because diffraction opens more links.
- **Not separated (open).** Tampa's extra multi-interval gaps may be sender-side: msgCnt is +1 on
  only 78 % of kept pairs. Its sub-50 ms gaps may be duplicate logging. The public RSU log cannot
  tell these apart.
- **LTE-V2X and NR-V2X.** KS is 0.48. Their gaps sit on the exact 100 ms SPS grid (p50 100 ms),
  while Tampa's DSRC gaps are spread by sender clocks (p50 105 ms).
- **Reception within 300 m.** DSRC 0.35, LTE-V2X 0.94, NR-V2X 0.66 at low load. LTE-V2X sends
  twice the frames (18,970 vs 9,586) and books no loss as below-sensitivity. Both are worth an
  audit; not judged here.
- **Load.** The small fleets never load the channel: mean CBR is 0.003 to 0.034 at every load.
  - The CBR rows pass for DSRC (0.0026 -> 0.0144).
  - Reception within 300 m stays flat (0.346 -> 0.346).
  - The judged-pair share rises slightly (0.968 -> 0.980, ratio 1.013, over the 1.005 top). That
    row fails. The load 2-3 runs have a 10 s live window against 30 s, so duration is a confound.
  - LTE-V2X: CBR 0.016 -> 0.073; reception 0.943 -> 0.913; judged 0.978 -> 0.977. All three rows
    pass.
  - NR-V2X: CBR 0.005 -> 0.029; reception 0.657 -> 0.648 (both pass); judged 0.958 -> 0.965,
    which fails at a 1.007 ratio for the same duration confound.
  - So at these loads no radio's reception or pairing changes beyond 3 %. The density effect P1-E5
    asks about needs fleets the 1.2-2.5 GB slots do not hold at `--record-level receptions`.

**P1-E6: both falsification tests pass.**
- **The nulls.** An attack-free run at 4,500 B loses 0 attack evidence and names no attacker. A
  zero quota loses 272 of 272 attack reports and delivers 0.
- **The quota curve.** Attack-evidence lost share at 0 / 2,300 / 4,500 / 16,384 / 65,536 B is
  1.0 / 0.099 / 0.081 / 0.015 / 0, non-increasing as SM-MBR §2.2 implies.
- **Who fills the store.** Honest reports fill it: 1,709 of 1,981 reports (86 %) accuse honest
  devices at 75 % free.
- **The offline replay passes its own test.**
  - Exact at 0 B; within 0.026 of the engine at 2,300 and 4,500 B; exact at 16,384 B.
  - Deletions at 4,500 B: 84 against the engine's 94.
  - It under-counts, which fits the engine re-storing reports whose upload failed.
  - Extended sweep (replay only): 1 report 0.096, 8 reports 0.026, 32 reports 0.
- **gpsDrift.** It moves the detector as theory says: RandomPosition verdicts 17,369 / 6,603 /
  10 at 0.5 / 1 / 5 m. Honest *filed reports* fall only 10 % at 5 m (1,708 -> 1,534).
  - The 30 % row is a **wrong expectation**: the remaining honest reports are ConstantPosition
    (11,755 verdicts, unchanged by gpsDrift).
  - At 0.5 m the lost share falls (0.081 -> 0.055: 22 -> 15 of 272), against the
    non-decreasing row. **Open**: small counts, and more reports also revoke attackers sooner.
- **Free-class share.** Every OBU locked: honest reports 1,708 -> 77, lost share 0.081 -> 0.064.
  Every OBU free: honest reports 2,080, lost share 0.082. Both rows pass.
- **Cellular share.** This is the strongest lever. At 4,500 B the lost share is 0.238 / 0.081 / 0
  at cellular share 0 / 0.8 / 1: **all attack-evidence loss is on the RSU-relay path**.
  - Delivered falls only 18 % with no modem (1,661 -> 1,364), because 30 RSUs relay most reports
    within 120 s. The 30 % row is a wrong expectation.
- **RSU density: confounded (open).** RSUs are nodes 0..N-1, so changing their count renumbers
  every vehicle. Every keyed draw then changes: access class, timing class and demand. At 10
  RSUs relay uploads fall 273 -> 98, but cellular uploads rise 1,597 -> 1,834. Both upload rows
  fail. A common-random-numbers RSU sweep needs vehicle draws keyed by something other than the
  node index.

**P1-E7: the null fails on the as-written scenarios; on the calibrated receiver with the stop fix
it holds for every rule with k >= 3, theta >= 3 or clustering, and fails for k = 2.**
- **E7 (p1-attack family, default receiver).** At 0 % free, sound rules would block up to 11 and
  30 honest devices (seeds 1 and 2), given those trigger rates. The fault and attacker rows pass.
  Root causes, from the MA's received reports:
  - **38 MaxAcceleration reports.** Locked devices' :00/:50 grid hops make a 50 ms secMark dt.
    The scenario's default GNSS white velocity noise (sd 0.14 m/s between fixes) then reads as
    over 10 m/s². CV-pilot locked devices differ by sd 0.045 m/s, and the final P1-E4 run
    already sets `velocity_noise_mps: 0`.
  - **32 RandomPosition reports: an engine defect, fixed (`ffaebbf9`).** `bsm_timing` added its
    fitted position-error walk (stationary sd about 0.7 m) only while the vehicle moved. Every stop
    or start then moved the reported position by the whole walk; one car flipped 1.1 m between two
    points while creeping. CV-pilot locked devices show 0.00 % RandomPosition below 5 m/s (T4).
    The walk now holds while standing. The test
    `a_stop_keeps_the_error_walk_on_the_reported_position` is red without the fix (6.11 m).
- **E7C (calibrated receiver plus the fix).**
  - Honest RandomPosition reports at the MA: 32 -> 0. MaxAcceleration: 38 -> 1.
  - At 0 % free, k = 2 rules only would still block 1 honest device per seed, given these trigger rates (`pseudonym-k`
    k=2, `device-k` k=2, `catch` theta=2), so P1.E7C.zero-* fail.
  - The device is accused by ConstantPosition at creep speed: 0.9 cm moved at 0.62 m/s. This class
    of trigger is real-data-consistent: Tampa locked devices trip ConstantPosition at 13.0 per
    ID-minute (T5).
  - Every rule with k >= 3 would block 0 honest devices at 0 % free on both seeds. That is under 1.2 x
    10^6 per 10^6 device-hours, the rule-of-three bound on 2.43 honest device-hours.
- **The E7C surface.** Honest devices each rule would block, pooled over seeds, with attackers it
  would revoke (offline replay of the MA, small fleet):

  | free % | seeds | honest dev-h | k=1 (unsound) | pseudonym k=2 | k=3 | k=5 | k=10 | device k=20 | catch 3 | catch 10 | cluster k3 c3 | cluster k5 c3 |
  |---|---|---|---|---|---|---|---|---|---|---|---|---|
  | 0 | 2 | 2.43 | 5 (5/5) | 2 (5/5) | 0 (5/5) | 0 (4/5) | 0 (1/5) | 0 (0/5) | 0 (5/5) | 0 (1/5) | 0 (5/5) | 0 (4/5) |
  | 10 | 1 | 1.09 | 10 (3/3) | 7 (3/3) | 6 (3/3) | 4 (2/3) | 0 (1/3) | 0 (0/3) | 6 (3/3) | 0 (1/3) | 1 (2/3) | 0 (1/3) |
  | 25 | 1 | 1.09 | 22 (3/3) | 16 (3/3) | 14 (3/3) | 10 (2/3) | 0 (1/3) | 0 (0/3) | 14 (3/3) | 0 (1/3) | 0 (2/3) | 0 (0/3) |
  | 50 | 1 | 1.09 | 34 (3/3) | 29 (3/3) | 25 (3/3) | 17 (2/3) | 1 (1/3) | 0 (0/3) | 23 (3/3) | 1 (1/3) | 3 (2/3) | 0 (0/3) |
  | 75 | 1 | 1.09 | 40 (3/3) | 36 (3/3) | 29 (3/3) | 20 (3/3) | 2 (1/3) | 0 (0/3) | 29 (3/3) | 2 (1/3) | 2 (2/3) | 0 (0/3) |
  | 100 | 2 | 2.43 | 123 (5/5) | 114 (5/5) | 102 (5/5) | 77 (5/5) | 13 (0/5) | 0 (0/5) | 102 (5/5) | 12 (0/5) | 24 (2/5) | 0 (0/5) |

- **Reading.** "Would revoke under rule R, given trigger rate T": at the CV-pilot locked rate, a
  2-reporter rule would still block honest devices through ConstantPosition. From 10 % free, every
  count rule with k <= 5 (and catch theta=3) would block honest devices; k = 10 and theta = 10
  hold to 25 % free and would block some from 50 %. Only `device-k` k = 20 (which would revoke no attacker
  in 120 s) and `cluster` k=5 c=3 stay at 0 throughout. The cluster rule k=3 c=3 is the best
  trade-off at low free share: 0 honest and 5 of 5 attackers revoked at 0 %, with tta median
  18-25 s.

**Qualifiers.**
- Every MA number is a small-fleet, 120 s Midtown number: about 60 honest devices and 1.1
  honest device-hours per run. A per-10^6-device-hour figure is therefore an extrapolation; its
  zero bound is 1.2 x 10^6.
- The rule set is the published CV-Pilot SM-MBR v1.0 on secMark. No US MA rule is published.
- The RA shuffle is cut to 10 s (stated assumption, as in the validation runs).

### Reproduce

```sh
# runs (one per run slot; E5 at --record-level receptions, E6/E7 at events), then per run:
python3 -I tools/realworld/paper1/summ.py <run> [--ma]          # E7: after v2xw ma-replay <run>
python3 -I tools/realworld/paper1/e5gaps.py run <run> --tampa <raw>/tampa.csv
python3 -I tools/realworld/paper1/e5gaps.py tol <raw>/tampa.csv  # the 0.0747 tolerance
python3 -I tools/realworld/paper1/e6replay.py <E6-A> <out> <quota-bytes>
python3 tools/realworld/matrix.py check crates/v2xw-engine/tests/expectations/scms-paper1.yaml <runs>
python3 tools/realworld/variants_faults.py crates/v2xw-engine/tests/expectations/scms-paper1.yaml <runs>
```

E7C runs need an engine build with `ffaebbf9`; E7 and E5/E6 used the `f4982788` release build.
