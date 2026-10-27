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

### Results

Pending.
