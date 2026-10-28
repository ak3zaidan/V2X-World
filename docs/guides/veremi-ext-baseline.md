# Reference ML baseline on a VeReMi Extension export

WP-24 P24.3, 2026-10-28. This is the recipe for f2md.md §5 P11. XGBoost and an MLP are
trained, with fixed seeds, on check-score features from this engine's own VeReMi
Extension export. They are scored per reception on a run they never saw.

- Tool: `tools/realworld/veremi_baseline.py`.
- Checks: `tools/realworld/test_veremi_baseline.py`, 5 tests, each shown red under an
  injected fault.
- No F2MD Python is used, because it has no licence. The checks are this engine's TS 103 759
  defaults, re-stated in Python.

## 1. The export

`scenarios/benchmark/veremi-ext-legacy-export.yaml` and its `-s2` and `-s3` variants are
one 60 s run each, on different seeds:

- the LuST VeReMi subsection (`worlds/sumo/lust-veremi-sub.net.xml`, P24.1);
- 1 Hz beacons (`messages.rate_hz: 1`, as in the VeReMi Extension);
- internal Poisson demand at 60 veh/h per lane-km;
- 30 % attackers (the published share), spread evenly over the seven frozen legacy
  families that carry a VeReMi Extension code: 1 ConstPos, 2 ConstPosOffset, 4 RandomPos,
  6 ConstSpeedOffset, 7 RandomSpeed, 9 EventualStop, 12 DelayedMessages.

The runs are 60 s long because 120 s reached 672 MB, over the 450 MB `runtiny.sh` cap.

```
v2xw run scenarios/benchmark/veremi-ext-legacy-export.yaml    --veremi veremi-ext -o runs/vxA --no-verify
v2xw run scenarios/benchmark/veremi-ext-legacy-export-s2.yaml --veremi veremi-ext -o runs/vxB --no-verify
v2xw run scenarios/benchmark/veremi-ext-legacy-export-s3.yaml --veremi veremi-ext -o runs/vxC --no-verify
```

Measured with the `ac00603c` release build. The export digest is the sha256 over the
`veremi/` files in name order.

| run | vehicles | attackers (codes) | GT messages | receptions | export sha256 (16) |
|---|---|---|---|---|---|
| vxA | 33 | 11 (2:2, 4:2, 6:4, 7:2, 9:1) | 943 | 18,375 | `ed1372457882ab1f` |
| vxB | 33 | 9 (1:1, 2:1, 7:2, 9:3, 12:2) | 1,013 | 20,695 | `bd67beaf8d952a7f` |
| vxC | 27 | 3 (1:2, 7:1) | 743 | 11,387 | `e1e7df35cb958e86` |

- Re-running vxA gave the same export digest.
- The attackers are drawn per vehicle. vxC's 3 of 27 is 2.1 σ under the 30 % binomial mean.
- These are **small** runs: each attack code rests on one to four vehicles. The table in §4
  is the recipe's output, not a benchmark result. P24.5 makes the full two-hour windows.

## 2. Features

```
python3 -I tools/realworld/veremi_baseline.py features runs/vxA/veremi vxA.csv
```

Each type-3 reception gets eleven scores. Each score is about 1 at its firing threshold,
and is clipped to [0, 10] with a 1e-3 quantum. The constants are
`Ts103759Params::default` (`crates/v2xw-threat/src/ts103759.rs`):

| score | rule |
|---|---|
| range | distance to the receiver / 420 m |
| speed | \|v\| / 40 m/s |
| pos_cons | displacement / (40 m/s · Δt + 4 m) |
| speed_cons | Δv / (6 m/s² · Δt + 2.1) when speeding up; \|Δv\| / (10 m/s² · Δt + 6.2) when slowing down |
| pos_speed | \|d − v̄Δt\| / (4 m + ½ · 6 m/s² · Δt²) |
| pos_heading | angle(heading, displacement) / 90°, when d ≥ 1 m, v ≥ 1 m/s and Δt ≤ 1.1 s |
| beacon_freq | 0.9 s / Δt (the `f2md-veremi-1hz` profile) |
| sudden_app | on a pseudonym's first message: 30 m / distance |
| intersection | 2 m / the nearest of the receiver and every other pseudonym's claim within 2 s |
| cv_predict | \|claim − (previous claim + previous velocity · Δt)\| / (4 m + 1 m/s · Δt) |
| age | (rcvTime − sendTime) / 100 ms |

The feature vector holds the scores of this reception and of the two before it from the
same pseudonym at the same receiver (F2MD's "last 3 messages"). Missing slots are zero, and
one more feature counts the earlier messages present. That makes 34 numbers.

Only receiver-visible fields are read: own type-2 positions, and the claims, times and
pseudonyms in type-3 lines. The ground truth is read only for the label, joined on
`messageID`. A test checks that the features do not change when the ground truth does.

The VeReMi `PositionPlausibility` (the map check) is left out, because it needs the road
geometry, which the trace does not carry.

## 3. Training

```
DYLD_LIBRARY_PATH=<venv>/lib/python3.12/site-packages/sklearn/.dylibs OMP_NUM_THREADS=1 \
  <venv>/bin/python -I tools/realworld/veremi_baseline.py repro foldC.json --train vxA.csv vxB.csv --test vxC.csv --seed 0
```

`repro` trains twice and exits 1 unless both model hashes and every metric agree.

- **Trees:** `xgboost.XGBClassifier`, with n_estimators 200, max_depth 6, learning_rate 0.1,
  subsample 0.8, colsample_bytree 0.8, `tree_method="hist"`, `n_jobs=1` and
  `random_state=seed`. The hash is of the booster's JSON dump.
- **MLP:** scikit-learn `MLPClassifier` with layers (64, 32), relu, adam, alpha 1e-4,
  batch 256, lr 1e-3, 200 epochs and `random_state=seed`, on `StandardScaler` features.
  The hash is of its weights.
- **Split:** by run. The test run's vehicles and pseudonyms are never in training, and
  each run is the test once.
- **Labels:** binary, attack versus genuine, per reception. Per-code recall is reported.
- **Scores:** MCC, BM (TPR + TNR − 1), MK (PPV + NPV − 1), Cohen's κ, F1 and FPR, with the
  definitions of the `detection-scores` metric group.

Substitutions, recorded in each output JSON's `impl`:

- **torch** is not installed, so the MLP is scikit-learn's.
- **xgboost** 3.4.1 needs `libomp`. On macOS without Homebrew's copy, it loads with
  scikit-learn's bundled `sklearn/.dylibs/libomp.dylib` on `DYLD_LIBRARY_PATH`.
  - Run Python directly, not through `nice`. A SIP-protected launcher drops `DYLD_*`, and
    then the script falls back to `HistGradientBoostingClassifier`, which the output names.
  - BLAS threads are pinned to 1.
- Environment: a venv with numpy 2.x, scikit-learn 1.9.1 and xgboost 3.4.1, never the
  system Python.

## 4. Result (2026-10-28, seed 0; each fold reproduced bit for bit)

| test run (attack share) | model | MCC | BM | MK | κ | F1 | FPR | recall by code |
|---|---|---|---|---|---|---|---|---|
| vxA (0.33) | XGBoost | 0.632 | 0.503 | 0.795 | 0.574 | 0.669 | 0.002 | 2: 0.00, 4: 0.00, 6: 0.80, 7: 0.78, 9: 0.86 |
| vxA | MLP | 0.477 | 0.330 | 0.689 | 0.393 | 0.500 | 0.012 | 2: 0.04, 4: 0.06, 6: 0.41, 7: 0.67, 9: 0.73 |
| vxB (0.27) | XGBoost | 0.709 | 0.595 | 0.845 | 0.677 | 0.743 | 0.006 | 1: 0.97, 2: 0.00, 7: 0.97, 9: 0.95, 12: 0.02 |
| vxB | MLP | 0.835 | 0.778 | 0.895 | 0.827 | 0.868 | 0.010 | 1: 0.97, 2: 0.00, 7: 0.97, 9: 0.82, 12: 1.00 |
| vxC (0.13) | XGBoost | 0.853 | 0.888 | 0.820 | 0.852 | 0.872 | 0.027 | 1: 0.88, 7: 0.98 |
| vxC | MLP | 0.869 | 0.926 | 0.816 | 0.866 | 0.884 | 0.030 | 1: 0.94, 7: 0.98 |

What it says:

- **ConstPosOffset (2) is invisible to these features.** A constant 25 m offset
  (`attack_legacy.rs` `const_pos_offset_m`) moves every claim the same way. Its scores sit
  on the genuine ones: in vxA, pos_speed has p50 0.15 against 0.21 for genuine messages,
  and cv_predict 0.56 against 0.54. Only a check against something outside the sender
  could see it, such as the map or the receiver's own sensors.
- **Most misses are attacks the training runs never contained.** Fold A trains on vxB and
  vxC, which hold no RandomPos (4) and no ConstSpeedOffset (6). RandomPos is the most
  separable attack in the data: in vxA its cv_predict p50 is 10, the clip, against 0.54
  for genuine messages. Yet both models flag at most 6 % of it, because they learned
  "attack" only from codes 1, 2, 7, 9 and 12.
- **DelayedMessages (12) shows the same effect in fold B,** where it is absent from
  training. The MLP flags it from `age`, and the trees miss it.
- So a binary model trained on a few attack families does not generalise to other
  families. The VeReMi Extension's split mixes every attack into training, and
  P24.4/P24.5 must do the same before comparing numbers.
- **The spread between folds is large** (MCC 0.48–0.87). It comes from which attackers
  each 60 s run happens to draw. Only longer windows (P24.5) give a number to compare
  with the VeReMi Extension's.

## 5. Not done here

- **Serving through `PyDetector`** (`crates/v2xw-py/src/plugins.rs`) needs the `v2xw-py`
  build. This piece was a no-cargo piece. The trained models and their hashes are what
  such a detector would load.
- **G-ML and G-LEG** are unchanged: no engine code changed, and the export is opt-in
  (`--veremi`).
- **The F2MD-faithful attacks** (`f2md-19`, WP-07) and the comparison with the published
  VeReMi Extension numbers are P24.4 and P24.5.
