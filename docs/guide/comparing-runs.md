# Comparing runs

Five commands read finished runs and tell you what differs or what went wrong. None of
them changes a run; three re-read a recording, one re-runs a scenario under several
schemes.

| Command | Question | Input | Output |
|---|---|---|---|
| [`v2xw compare`](#v2xw-compare) | what changed between two runs, and is it the settings or noise? | two run directories, digests, experiment directories or replication lists | a table (text, JSON, Markdown, CSV) |
| [`v2xw compare-signatures`](#v2xw-compare-signatures) | what does each signature scheme cost against ECDSA P-256? | one scenario | `comparison.json`, `.csv`, `.md` |
| [`v2xw ma-replay`](#v2xw-ma-replay) | how would other MA decision rules have done on these reports? | a run with report records | `ma-replay.csv` |
| [`v2xw check-traffic`](#v2xw-check-traffic) | did the traffic break an invariant? | a run with a full recording | a pass/fail table |
| [`v2xw energy`](#v2xw-energy) | what did the traffic emit and consume? | a full recording | a fleet table with health bands |

Every flag: [Command-line tool](../getting-started/cli.md).

## `v2xw compare`

```sh
v2xw compare runs/base runs/tx23                              # two run directories
v2xw compare prev.json runs/tx23 --markdown                   # a digest saved from the Studio or runs.digest
v2xw compare runs/a1,runs/a2,runs/a3 runs/b1,runs/b2,runs/b3  # replications pooled per side
v2xw compare runs/sweep-a runs/sweep-b --all --json           # experiment directories, every metric
```

A side is a run directory (`scenario.resolved.yaml` and `metrics.json`), a run digest
(`v2xw/run-digest/1`), an experiment output directory (every run under its `runs/` pooled
as replications), or a comma-separated list of those. The Studio's "compare with previous
run", the assistant and `v2xw compare` all print the same value
(`v2xw_server::compare::Comparison`), so the three cannot disagree.

### How a run becomes one number per metric

As the experiment runner does: proportions pool their counts, ratios of sums pool their two
sums, scalars and distributions are sample-count weighted means of each window's figure
(so a p95 is the typical window's p95), counts are summed, and a level metric aggregated
by its maximum (a CRL's entries as last published, a memory high-water mark) pools to its
largest value.

### When a change is "meaningful"

A row is shown (without `--all`) when its change is meaningful:

1. it crosses a health band of the metric, **or** it moves by at least 10 % of the
   baseline (`RELATIVE_FLOOR`) and by more than the unit's resolution; **and**
2. when the runs used different seeds, or either side has replications, the difference
   exceeds twice the combined standard error, estimated per run by batch means over its
   windows (Law, *Simulation Modeling and Analysis*, 5th ed., §9.5.3; up to 10 batches).

With the same seed and no replications, the runs share every random draw the settings did
not touch (common random numbers), so the difference is the settings'; the output says so.

The verdict column reads the metric's health direction (`v2xw_metrics::health`):

| Verdict | Meaning |
|---|---|
| `better` / `worse` | moved in the direction the metric's polarity calls better or worse |
| `changed` | moved, and the metric has no better direction (a density, a flow) |
| `same` | within the unit's resolution |
| `missing` | one side does not have it |

```text
  1 setting changed: Vehicles entering per hour 30.0 veh/h → 600.0 veh/h.
  15 of 36 metrics changed meaningfully: 3 better, 4 worse, 8 neutral.
  …
Settings changed:
  Vehicles entering per hour               30.0 veh/h → 600.0 veh/h

metric                       A              B         change         %  verdict
offered_load       32259 bit/s   130592 bit/s    98332 bit/s  +304.8 %  changed
half_duplex_rate             0        7.80e-3        7.80e-3            worse
…
```

`--json` is schema `v2xw/run-comparison/1`: both sides' identities, the settings that
differ, and a row per metric with `a`, `b`, `delta`, `pct`, `better`, `verdict`, health
levels, `meaningful` and `beyond_noise`.

## `v2xw compare-signatures`

```sh
v2xw compare-signatures scenarios/compare-signature-schemes.yaml \
    --schemes ecdsa-p256,ml-dsa-44,falcon-512,hybrid-mldsa44-ecdsa-p256 --duration 60
```

It is the experiment runner with one sweep axis,
[`security.signature`](../configuration/reference/security.md#s-security-signature), one
seed and one replication, and the opt-in signing-overhead metric group
(`metrics: [all, signing]`) on in every run. The run seed does not depend on the cell, so
every scheme sees the same traffic, channel draws and credential calendar. A stopped
comparison resumes through the runner's journal.

| File in `--out` | What |
|---|---|
| `comparison.json` | the schemes' registry facts and every row, value, difference and health level (`v2xw/signature-comparison/1`); the Studio's comparison view loads it |
| `comparison.csv` | the same, one line per (row, scheme) |
| `comparison.md` | a table for a paper or an issue: differences in brackets, called-out ones in bold |
| `runs/…` | each run's metrics, report and manifest |

The rows are the `sig_*` metrics: signature, signer-identifier and certificate bytes per
message and their airtime shares; signing and verification latency per scheme and engine
(`ml-dsa-44@cpu`, `ecdsa-p256@hsm`); signer and verifier utilisation, verification capacity
and headroom, dropped and late verifications; fragments per message and reassembly loss;
added end-to-end latency; the CBR the envelope adds; backend bytes and certificate-authority
load. A difference is called out when it crosses a health band, or moves by 10 % of the
baseline and more than the row's resolution. One realisation: no confidence interval is
claimed. Post-quantum schemes are size and cost models ([Security and
crypto](../models/security.md)).

## `v2xw ma-replay`

```sh
v2xw ma-replay runs/attack                                     # the built-in rule family
v2xw ma-replay runs/attack --policies scenarios/attack-fake-positions.yaml
```

Replays the misbehaviour reports a finished run recorded through a family of
Misbehaviour Authority decision rules, without re-simulating. The run must have
[`security.scms.report_records: true`](../configuration/reference/security.md#s-security-scms-report-records).

A rule reads only the `ma.received` channel: field for field what the deciding MA read from
each report (reporter and accused pseudonym certificates, subtypes, evidence window,
claimed position accuracy, issuer, and the Linkage Authorities' "same device" token).
Reports the MA set aside are dropped first. Ground truth (`gt.device_exposure`,
`pki.report_fate`) is joined only to score decisions, and a blindness check renames every
identifier in the MA view and requires the decisions to rename the same way.

| Rule | Revokes |
|---|---|
| `pseudonym-k` | a pseudonym certificate once `k` distinct reporters accused it (`k = 1` is kept as the unsound baseline) |
| `device-k` | a device (by its LA link) once `k` distinct reporters accused any of its pseudonyms within a window `w` |
| `catch` | a device when its reporters' accuracy-weighted factors sum to `θ` within `w`; a position report counts $\mathrm{clamp}(1 - 2a/(\text{excess}\cdot\tau), 0, 1)$ with `a` the claimed accuracy and τ = 1 m (after Kamel et al., "CaTch", IEEE WCNC 2019, re-implemented from the paper) |
| `cluster` | as `device-k`, but a device in a class (secMark phase, accuracy band, issuer) with `c` or more accused devices in `w` needs `m·k` reporters |
| `--policies` | every preset and named policy of the scenario's [`security.scms.ma_policy`](../configuration/reference/security.md), replayed through the run's own evaluator |

A parameterisation is `sound` when it needs corroboration (`k ≥ 2`, `θ ≥ 2`).
`ma-replay.csv` has one row per rule and parameter:

| Column | Meaning |
|---|---|
| `rule`, `params`, `sound` | the rule and its parameters |
| `decisions` | revocation decisions made |
| `honest_devices`, `honest_device_hours`, `honest_blocked`, `honest_blocked_per_1e6_dh` | false revocations, per 10⁶ honest device-hours |
| `attackers`, `attackers_revoked` | attackers in the run and how many were revoked |
| `tta_median_s`, `tta_max_s`, `attack_budget_s` | the attacker's time to act: attack start to revocation decision |

`ma-replay-attackers.csv` has one line per rule, parameter and attacker
(`attack_start_s`, `revoked_s`, `budget_s`) for paired comparisons across runs of one seed.

!!! note "Offline versus in-run"
    An offline replay shows no feedback: a revoked device keeps sending, the CRL's delays
    do not apply and reporters' reputations do not change. A sweep over
    `security.scms.ma_policy.preset` runs every policy in full and writes
    `ma-policy-family.csv` and `.md` ([Experiments and sweeps](experiments-and-sweeps.md#the-ma-policy-family)).

## `v2xw check-traffic`

```sh
v2xw check-traffic runs/midtown-normal
```

Audits the traffic of a finished run from what was recorded (the poses, lanes and signal
lamps the Studio replays), not from inside the engine, so a vehicle the engine thinks is
lawfully held at a red whose recorded lamp is green is caught. It needs `recording.mcap` at
`--record-level full` and `scenario.resolved.yaml` (the world is rebuilt from it), and
exits 1 when a row fails.

| Row | Fails when |
|---|---|
| `stopped_on_green` | a queue head stands at its stop line longer than `--green-stop-s` (6 s) on its own protected green with room ahead; a hold behind a standing exit queue (keep-clear, NY VTL §1175) is not counted |
| `stopped_on_permitted_green`, `stopped_at_clear_junction` | the same on a permitted green, or at an empty unsignalised junction |
| `phantom_stop`, `long_stop` | a vehicle stands mid-lane with nobody within 30 m, or longer than one signal cycle plus a margin (120 s off a signal); kerbside stops are excused |
| `red_entry` | a vehicle enters a junction on a recorded red |
| `lamp_phase_unknown` | a plan's recorded lamps match none of its phases |
| `overlap`, `jump` | two vehicles closer than the shorter body on one lane; a step longer than the speed explains |
| `decel`, `accel`, `jerk`, `speeding` | over 9 m/s² braking, 6 m/s² accelerating, jerk over 30 m/s³ in more than one step in a thousand, over 1.25 × the limit + 2 m/s |
| `ped_jump`, `ped_speed_jump`, `ped_heading_flip` | a pedestrian step no walker makes |

It also measures, with no pass mark, the stop-line saturation headway and start-up lost
time (HCM 7th ed. ch. 19: base 1,900 pc/h/ln is a 1.89 s headway, start-up lost time 2 s),
the longest stop, mean speed per road class against the limit, and gridlock teleports.
`--trace <actor>` prints one vehicle's every step on stderr.

## `v2xw energy`

```sh
v2xw energy runs/glosa/recording.mcap
v2xw energy runs/glosa/recording.mcap --emission-model moves-opmode --opmode-csv out/opmode.csv
```

Scores a recording's traffic for CO2, fuel, NOx, PM and EV energy without re-running it:
the energy provider reads only `gt.kinematics`, so any full recording can be scored, under a
different model or fleet than the run used. The `energy` section the run carried is the
starting point (from the recording's scenario attachment); `--spec` replaces it, and
`--emission-model` (`hbefa3`, `phemlight`, `moves-opmode`) and `--ev-model` (`kurczveil`,
`vt-cpem`) override the models. The fleet draw is keyed by the recording's master seed, so
a re-score reproduces the run's fleet.

```text
models      energy/emission/hbefa3 (combustion), energy/ev/kurczveil (electric); section from default
scored      12 vehicles from 4531 gt.kinematics records, 0 unmodelled
fleet       … g/km CO2, … L/100km fuel, - EV

class          km     CO2 kg    fuel L   NOx g    PM g   EV kWh  stops
passenger    2.45      1.063     0.457    0.43   0.018    0.000      7

health
  warn         co2_per_km             passenger  … (healthy 90..350)
  ok           nox_per_km             passenger  … (healthy 0..1)
  not-measured ev_energy_per_100km    passenger  - (healthy 9..25)
```

A `warn` is a value outside the band a realistic fleet would show, which on a short run
with stop-and-go traffic is often the traffic, not the model. The models and their sources:
[Energy and emissions](../models/energy.md).
