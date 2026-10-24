# F2MD compared with the V2X World Simulator

Status: research note, 2026-10-22. Read-only study; no engine code changed.

Source read: `josephkamel/F2MD` at `e8ac8c5` (2025-11-15) and the `veins-f2md` submodule at
`340e3cb` (2022-09-29), cloned to the session scratchpad. The files read were
`src/veins/modules/application/f2md/**` (`F2MDParameters.h`, `mdEnumTypes/*`,
`mdAttacks/MDAttack.cc`, `mdAttacks/MDGlobalAttack.cc`, `mdChecks/{Legacy,CaTCh,Experi}Checks.cc`,
`mdApplications/*`, `mdReport/*`, `mdPCPolicies/PCPolicy.cc`, `mdSupport/{VeReMiPrintable,
RelativeOffset,RelativeOffsetConf,GeneralLib}.cc`, `kalmanLib/Kalman_SVI.cc`), the scenario
`f2md-networks/LuSTNanoScenario/omnetpp.ini`, the Python `misbehavior-authority-server`,
`machine-learning-server` and `attack-server`, and the `f2md-results/gui-plots` metric scripts.

Engine side compared: `crates/v2xw-threat` (`attack.rs`, `attack_legacy.rs`, `attack_ext.rs`,
`detect.rs`, `ts103759.rs`, `us_mbr.rs`, `ma.rs`, `ma_corroborate.rs`, `privacy.rs`, `poison.rs`),
`crates/v2xw-record/src/dataset/veremi.rs`, `crates/v2xw-metrics/src/detection.rs`,
`crates/v2xw-engine/src/pseudonym_policy.rs`, `crates/v2xw-py/src/plugins.rs`,
`docs/design/04-models.md` §14, `docs/design/research/R10-maps-mobility-veremi.md` §C1–C4.

## 1. Overview

F2MD (Kamel, Ansari, Petit, Kaiser, Ben Jemaa, Urien, "Simulation Framework for Misbehavior
Detection in Vehicular Networks", IEEE Trans. Veh. Technol., 2020, IEEE document 9056489) is a
Veins application layer (`JosephVeinsApp`) on OMNeT++ 5.6.1 with SUMO 1.5.0. It covers the
misbehaviour-detection chain from end to end:

- **Attacks** (`mdAttacks`): 19 local attacks and 1 global attack.
- **Sensor error** (`RelativeOffset*`): GNSS and odometry error, plus the confidence values
  broadcast with each message.
- **Local checks** (`mdChecks`), in three versions: Legacy (binary), CaTCh (scored by
  confidence) and Experi (CaTCh plus a stretched negative tail).
- **Node-side fusion** (`mdApplications`): Threshold, Aggregation, Behavioural, Cooperative,
  Experi, and machine learning (ML) through an HTTP call to a Python server.
- **Reports** (`mdReport`): OneMessage, Evidence, Protocol and BasicCheck, sent as JSON over
  HTTP.
- **Global misbehaviour authority (MA)**: a Python server.
- **Pseudonym change policies** (`mdPCPolicies`).
- **Output**: per-message detection statistics and the **VeReMi Extension** dataset writer.

It is the generator of VeReMi Extension (Kamel, Wolf, van der Heijden, Kaiser, Urien, Kargl,
ICC 2020). That dataset is the most-cited misbehaviour-detection benchmark after the original
VeReMi, so it matters mostly as a comparability target. As a realism target it matters much
less.

Its radio is Veins 802.11p (or C-V2X Mode 4 through SimuLTE). The scenario defaults are a
1 Hz beacon, 20 mW, 6 Mbps and `SimpleObstacleShadowing`. Its messages are Veins
`BasicSafetyMessage` objects: no J2735 encoding, no IEEE 1609.2 signature, no certificate.

## 2. Licence

| Part | Licence | Consequence |
|---|---|---|
| `veins-f2md` (the C++ code) | GPL-2.0 (`COPYING`; Veins is GPL-2.0-or-later). The F2MD files also carry "Copyright (c) 2013, 2018 IRT SystemX. All rights reserved." | **No code may be copied** into the Apache-2.0 engine. |
| Top-level `F2MD` (the Python servers, scripts) | **No licence file.** By default, all rights are reserved. | Nothing may be copied. |
| Numeric constants, enum names, the file format | Facts and interface descriptions | May be cited and matched. They are already cited in 04-models.md §14. |

Everything adopted below is **re-implemented from the published algorithm**, and its model
card cites the paper:

- F2MD itself: TVT 2020.
- CaTCh: Kamel, Kaiser, Ben Jemaa, Cincilla, Urien, *CaTCh: a confidence range tolerant
  misbehavior detection approach*, IEEE WCNC 2019.
- VeReMi Extension: ICC 2020.
- VeReMi: van der Heijden, Lukaseder, Kargl, SecureComm 2018.

The F2MD constants are cited as "veins-f2md `F2MDParameters.h`, read 2026-10-22". The VeReMi
Extension dataset's own licence must be checked before we redistribute any of it. Reading it
locally for comparison needs no redistribution.

## 3. Defects found in F2MD's source (do not port these; they explain some published numbers)

| # | Where | Defect | Effect on F2MD/VeReMi-Ext results |
|---|---|---|---|
| D1 | `GeneralLib.cc:22,44`, `RelativeOffset.cc`, `RelativeOffsetConf.cc` | Every random draw builds a new `mt19937` seeded from `CLOCK_REALTIME` nanoseconds | Runs cannot be reproduced and the OMNeT++ seed has no effect. Draws taken close together are correlated. |
| D2 | `MDAttack.cc:47,52,57,64` | `(int) RandomDouble(1-parVar, 1+parVar) * N` casts the factor before the multiply, so the factor is 0 or 1 | `ReplaySeqNum`, `DosMultipleFreq(Sybil)` and `SybilVehNumber` take only two values (2 or 6; 2 or 4; 1 or 5), each with probability 0.5. The intended ±55 % spread does not happen. |
| D3 | `MDAttack.cc:662-663` (GridSybil, default `SelfSybil=false`) | `setSenderSpeed(newAccel)` and `setSenderSpeedConfidence(newAccelConf)` | Each ghost's claimed speed is roughly its acceleration (about 0 m/s), so position/speed consistency catches it trivially. The published GridSybil (TrafficCongestionSybil) detection is inflated. |
| D4 | `LegacyChecks.cc:64`, `CaTChChecks.cc` | ProximityPlausibility iterates `realDynamicMap`, the **true** positions of every vehicle | The detector reads ground truth (an oracle). No real receiver has this. Our invariant I-T2 forbids it. |
| D5 | `VeReMiPrintable.cc` | The per-receiver trace's `sender` is `getSenderRealId()` | Pseudonyms link for free. The engine already writes `sender` from the pseudonym (`veremi.rs`). |
| D6 | `BehavioralApp.cc:53,81,102`, `CooperativeApp.cc` | `UpdatedTMO[TimeOutNum]` is used where `[indexTMO]` was meant, and `removePseudoTMO` shifts only `TimeOut`, not `PseudonymsToTMO` or `UpdatedTMO` | After the first removal, timeouts attach to the wrong pseudonyms. Behavioural-app results cannot be reproduced from the paper's description. |
| D7 | `MdAppTypes.h` | `AppNames` swaps the Behavioural and Cooperative entries relative to the enum | Output files are mislabelled. |
| D8 | `MDAttack.cc` RandomSpeedOffset | The y axis uses `RandomSpeedOffsetX` | Harmless with the defaults. |
| D9 | MA server | Five reaction levels are named (Warning, Ticket, PassiveRev, ActiveRev) but none is implemented | The MA is a report count (> 5 reports means "LocalAttacker"). |

## 4. Component table

The classification key: **EB** = the engine is better, **EQ** = equivalent, **FB** = the
framework is better (we should improve), **MI** = missing in the engine (we should add),
**NU** = not useful.

### 4.1 Attacks (F2MD `attackTypes`, defaults from `F2MDParameters.h`, `parVar` = 0.55 per-attacker spread)

The engine's legacy catalogue (`attack_legacy.rs`) is a port of our own Python `run.py`. Its
semantics are frozen by the ML corpus digest. **Several names match F2MD/VeReMi names, but the
behaviour behind them differs.** A paper that says "ConstPos" means F2MD's version.

| F2MD attack (VeReMi-Ext code) | F2MD behaviour and parameters | Engine counterpart | Class |
|---|---|---|---|
| ConstPos (1) | A fixed random point, uniform over the whole playground | `ConstPos`: freezes at the first honest position | FB: same name, different attack |
| ConstPosOffset (2) | True position + a constant (70 m ±55 %) per axis | `ConstPosOffset`: +25 m on both axes | FB: magnitude differs |
| RandomPos (3) | A uniform point anywhere on the playground, drawn every message | `RandomPos`: a ±60 m offset drawn every message | FB: semantics differ |
| RandomPosOffset (4) | True position + U(−35, +35) m per axis | none (the engine's `RandomPos` is the closest) | MI |
| ConstSpeed (5) | A constant speed vector, U(0, 40) per axis | none | MI |
| ConstSpeedOffset (6) | True speed + a constant (7 m/s ±55 %) per axis | `ConstSpeedOffset`: +12 m/s scalar | FB: magnitude |
| RandomSpeed (7) | A U(0, 40) speed vector per axis, drawn every message | `RandomSpeed`: scalar U(0, 40) | EQ |
| RandomSpeedOffset (8) | True speed ± (7 m/s ±55 %) per axis, random sign every message | none | MI |
| EventualStop (9) | Each beacon starts the stop with p = 0.05 (±55 %). From then on it repeats one frozen state with speed 0 and acceleration 0 (consistent) | `EventualStop`: freezes position after 6 s but claims creep at 0.8 + 2k m/s (inconsistent) | FB: semantics differ |
| Disruptive (10) | Replays a **random neighbour's** latest message under its own pseudonym | `Disruptive`: random falsification of position, speed and heading | FB: same name, different attack |
| DataReplay (11) | Follows one **neighbour** and replays its messages under its own pseudonym; switches target after `ReplaySeqNum` (6) | `DataReplay`: replays **its own** state from 5 samples back. `RelayReplay` (ext) retransmits another's frame verbatim | FB: neighbour replay is missing |
| StaleMessages (12, "DelayedMessages") | Sends its own message from 60 beacons earlier | `DelayedMessages`: a stale generation time | EQ |
| DoS (13) | Beacon interval ÷ `DosMultipleFreq` (4), honest content | `DoS`: repetitions per interval (4) | EQ (F2MD spaces the extra beacons; check ours do too) |
| DoSRandom (14) | High rate + uniform random position, speed, acceleration and heading | `DoSRandom`: high rate + a ±60 m position offset | FB: partial |
| DoSDisruptive (15) | High rate + random-neighbour replay | none | MI |
| GridSybil (16, "TrafficCongestionSybil") | `SybilVehNumber` (5) ghosts in a 2-column grid behind a target or itself, spaced by length + 5 m and width + 2 m with ±10 % jitter; rate × (n+1); a ghost is dropped if it would leave the road or the heading changes by more than 5° | legacy `Sybil` + ext `GhostVehicles` (5 ghosts, 5 m / 2 m, placed on lanes) | EQ, with the engine better on credentials (see §6) |
| DataReplaySybil (17) | Neighbour replay with a fresh pseudonym per target | none | MI |
| DoSRandomSybil (18) | High rate, random content, **a new pseudonym every message** | none | MI (bounded by the credential pool) |
| DoSDisruptiveSybil (19) | High rate, random-neighbour replay, new pseudonym every message | none | MI |
| MAStress (global, 20) | Floods the MA with fabricated reports | `poison.rs` report poisoning | EB (it is signed, budgeted and corroborated) |
| attack server (live launch over HTTP) | Starts attacks on running nodes at run time | scenario schedule plus the `v2xw-server` API | NU |
| Attacker share `LOCAL_ATTACKER_PROB` 0.05; VeReMi-Ext uses 0.30; `START_ATTACK` 10 s | — | `attack_intensity` and the attacker share in the scenario | EQ |

### 4.2 Sensor error and broadcast confidence

| F2MD | Engine | Class |
|---|---|---|
| `veremiConf`: position confidence U(3, 5) m per vehicle; heading confidence U(0, 20)° × e^(−0.1 v); speed confidence ≈ N(0, 0.0016) × v; acceleration confidence = Δ(speed confidence)/Δt. The error is a bounded polar random walk inside the confidence radius (`RelativeOffset::OffsetPosition`, steps of `STEP_G`). Draws seeded from the wall clock (D1). | `GaussMarkovGnss` with a cited correlation time; the broadcast semi-major axis taken from the belief | EB for realism. MI only as an *optional* `veremi-ext` preset for like-for-like runs. |

### 4.3 Local plausibility and consistency checks

F2MD thresholds come from `F2MDParameters.h`. Engine defaults are cited in 04-models.md §14.
Legacy means binary, CaTCh means a confidence-overlap score in [0, 1], and Experi is CaTCh
with a negative tail down to −20.

| F2MD check | F2MD rule and threshold | Engine counterpart | Class |
|---|---|---|---|
| RangePlausibility (ART) | d < 420 m. CaTCh scores the overlap of the two confidence circles with the range circle. | `ts103759` RangePlausibility (420 m); legacy `acceptanceRangeThreshold` (receiver range + 150 m) | EQ. The 420 m comes from Veins at 20 mW; our radios differ, so derive it from our link budget (§5 P9). |
| SpeedPlausibility | \|v\| < 40 m/s (CaTCh: ± conf/2) | `SpeedPlausibility` 40 m/s | EQ |
| PositionPlausibility (map) | distance to road > 2 m, skipped at speed ≤ `MAX_NON_ROUTE_SPEED` (−1, i.e. never skipped) | `PositionPlausibility` 2 m; legacy `mapOffRoad` 15 m | EQ |
| PositionConsistancy | d < 40 · Δt (CaTCh circle-circle) | `PositionConsistency` disp/(v_max·Δt + tol) | EQ |
| SpeedConsistancy | Δv < 3·Δt accelerating, < 4.5·Δt braking | `SpeedConsistency` and `AccelerationPlausibility` | EQ |
| PositionSpeedConsistancy | The distance must sit inside the [min, max] distance reachable under the acceleration and braking limits, with a speed-dependent margin 6.2 + 0.3571 v − 0.01694 v² (down) and 2.1 (up); Δt < 3.1 s | `PositionSpeedConsistency` \|disp − v̄·Δt\|/(4 + tol) | FB: F2MD's reachable-distance envelope is physically better at low rates (§5 P7) |
| PositionSpeedMaxConsistancy | max(v) − d/Δt > (4.5 + 4)·Δt or d/Δt − min(v) > (3 + 4)·Δt | none | MI (small) |
| PositionHeadingConsistancy | angle(heading, displacement) > 90° within 1.1 s; skipped if d < 1 m or v < 1 m/s | `PositionHeadingConsistency` (90°, 1.1 s) | EQ |
| BeaconFrequency | Δt < 0.9 s | `BeaconFrequency` = 0.9/Δt | **Engine defect** shown up by this comparison (§5 P1) |
| SuddenAppearence | first seen within 420 m (legacy); `MAX_SA_TIME` 2.1 s | `SuddenAppearance` (+ a listening-time guard) | EQ |
| Intersection | Overlap of the claimed **rectangles** (width × length, heading), scaled by (2.0 − Δt)/2.0, failing above 0.5; tested against the receiver itself and every neighbour within 2 s | `Intersection`: point distance < 2 m | FB (§5 P6) |
| ProximityPlausibility | A claim inside a 30 m × 3 m box ahead must have a **true** vehicle within 2 m (D4: oracle) | class-4 `PerceptionCrossCheck` on the node's own sensors, inside their coverage only | EB |
| kalmanPSCP / PSCS / PSCSP / PSCSS / PCC / PACS / SCC | Three Kalman variants (SVI: x, vx, y, vy with Q = diag(50, 10, 50, 10) and R from confidence; SC; SI). Score = 1 − ‖innovation‖ / (range · conf · Δt), with floors of 4 m and 1 m/s, Δt < 3.1 s | legacy `kalmanConsistency` (alpha-beta 0.5/0.3, a fusion feature only); nothing in `ts103759` | FB (§5 P5) |
| — | — | `ConfidencePlausibility`, `OversizedMessage`, `GenerationTimeOrder`, `ForeignRegion`, `CpmConsistency`, `SignatureInvalid`, `CertificateUnknown`, VRU and DENM checks, Sybil co-location | EB: F2MD has no envelope, CPM or DENM to check |

### 4.4 Detection apps and fusion

| F2MD | Rule | Engine | Class |
|---|---|---|---|
| ThresholdApp | Report if any check ≤ 0.28125 | each check fires at score ≥ 1 for `min_consecutive` (2) messages in a row | EQ |
| AggrigationApp | Mean of the current factor and up to 3 earlier factors within 10 s, against 0.28125. V2 returns 0 straight away if the current factor ≤ 0. | none (only consecutive runs) | MI (§5 P8) |
| BehavioralApp / CooperativeApp | A leaky bucket per pseudonym: TMO += 0.0005·e^(10(1−f_min)), −1 per message, report when TMO > 5.5066 (D6) | none | MI (§5 P8) |
| ExperiApp | Experimental thresholds, 10 s window, 3 messages | — | NU |
| MachineLearningApp | Sends each message's check vector over HTTP to Python (MLP, SVM, LR, XGBoost, LSTM, and a combined LSTM + dense model) | `v2xw-py` `PyDetector`: in-process and batched (Arrow); the `detnorm_*` ML contract | EB for the bridge. MI: a reference ML baseline (§5 P11). |
| Variable threshold (`MLVarThreshold`, `enableVarThre`) | The threshold adapts at run time | none | NU |

### 4.5 Reports and the global authority

| F2MD | Engine | Class |
|---|---|---|
| OneMessage, Evidence (with neighbours' messages for intersection evidence), Protocol (`ProtocolEnforcer`: re-reports every `CollectionPeriod` 10 s while the misbehaviour continues; dropped after 10 + 5 s of quiet), BasicCheck. Sent as JSON or XML over HTTP. | TS 103 759 ASN.1 misbehaviour report (`report.rs`, `ts103759.rs`) and the US SM MBR (`us_mbr.rs`), signed, with a report budget and evidence from the messages the reporter judged | EB |
| MA: more than 5 reports about a pseudonym means "LocalAttacker"; an aggregate over the last 20 predictions; web GUI | `ma.rs` + `ma_corroborate.rs` (k reporters, distinct seconds, window, reputation), `resolve.rs` (two-authority linkage), CRL/revocation loop | EB |

### 4.6 Pseudonym change policies

| F2MD (`PCPolicy.cc`) | Engine (`pseudonym_policy.rs`) | Class |
|---|---|---|
| Periodical 240 s | `time` (300 s, J2945/1) | EQ |
| DistanceBased 80 m | `distance` (2 km, NYC pilot) | EQ |
| Car2car: first change after U(800, 1500) m, then after ≥ 800 m plus U(120, 360) s, repeating | `c2c-cc`: the full RS_BSP_521–524 rule, including the 10–20 km and 25–35 km stages | EB |
| Disposable (every 10 messages), Random (p = 0.1 per beacon) | none | NU: no deployment does this. Add only as a comparability preset if a study needs it. |
| Unlimited fresh pseudonyms (up to 100 per vehicle, generated locally) | SCMS certificate batches, validity windows, concurrency limits | EB |
| — | `j2945-1`, `nyc-pilot`, `mix-zone`, `silent` periods; privacy observer with four linkability metrics | EB |

### 4.7 Metrics and dataset output

| F2MD | Engine | Class |
|---|---|---|
| **Per message**: TP, FP, TN, FN; accuracy, precision, recall, F1, **informedness, markedness, MCC, kappa**; MA detection rate and latency | `detection.rs`: four cells, recall, FPR, precision, F1, accuracy with confidence intervals, at the **Report** (subject) and **Vehicle** levels; `time_to_detect`, `time_to_decision`, `false_accusations` | FB: MCC, BM, MK and kappa are missing, and so is a **per-message** level (§5 P4) |
| VeReMi Extension writer: `traceJSON-<rx>-<module>-A<attackType>-<t0>-<slice>.json`. Type 2 (own GPS) and type 3 (received) carry `pos, pos_noise, spd, spd_noise, acl, acl_noise, hed, hed_noise`; type 3 adds `rcvTime, sendTime, sender, senderPseudo, messageID`. `traceGroundTruthJSON-<slice>.json` holds type 4 truth. Hourly slices (`VeReMiSliceTime` 3600). `attackType` = the F2MD enum 0–19. | `veremi.rs`: `traceJSON-<rx>.json`; type 2 has only `pos, spd`; type 3 has `pos, spd, RSSI` (no heading, no acceleration, no noise fields); `GroundTruthJSONlog.json` with `attackerType`; `sender` from the pseudonym (keep this) | FB (§5 P2, P3) |
| Scenarios: LuST (full, Mini, Nano), IRT SystemX, Ulm | SUMO `net.xml` importer, OSM, procedural world | EQ. MI: a VeReMi-Ext replica scenario (§5 P10). |

## 5. Prioritised port / re-implement list

None of this copies F2MD code. Each item is re-implemented from the algorithm with the
citation given.

| P | What exactly | Why it improves realism or comparability | Effort | How |
|---|---|---|---|---|
| **P1** | **Fix the TS 103 759 suite's beacon-frequency threshold.** `min_beacon_interval_s` = 0.9 s is F2MD's value for a **1 Hz** VeReMi beacon. US BSMs are sent at 10 Hz (J2945/1, 100 ms), and `tests/ts103759.rs::class_two_fires_on_a_sender_beaconing_faster_than_the_conforming_minimum` asserts that 0.1 s gaps fire. So every honest BSM sender would be flagged once the suite is wired in. Re-derive the 1 Hz-based windows (`MAX_TIME_DELTA` 3.1 s, `MAX_SA_TIME` 2.1 s) too. | Removes a guaranteed near-100 % false-positive rate on US traffic. | S | Take the minimum interval from the message profile: BSM 100 ms, and CAM `T_GenCamMin` 100 ms (EN 302 637-2), less a jitter tolerance. Or count messages over a window, as legacy `freq_max` does. Express the windows in nominal intervals. Make the test fail on a known violation and pass at 10 Hz (inject the fault, per our "checks that cannot fail" rule). |
| **P2** | **Correct the VeReMi `attackerType` codebook** in `veremi.rs::attacker_type_code`. VeReMi 1.0 uses bit codes 1 ConstPos, 2 ConstPosOffset, 4 RandomPos, 8 RandomPosOffset, **16 EventualStop**. Ours maps 16 to const-speed and 32 to eventual-stop, and the doc comment says "1–4". VeReMi Extension uses the F2MD enum 0–19 (§4.1). | Today a VeReMi reader silently mislabels our ConstSpeed as EventualStop. | S | Two explicit codebooks, `veremi-1` and `veremi-ext`, chosen by an export option. Map the engine families to codes **only where the semantics match** (see P3); everything else gets the "unnamed" code (1024). |
| **P3** | **VeReMi Extension line schema.** Add `hed`, `acl` and the `*_noise` fields (= the broadcast confidences) to types 2 and 3. Write type 4 truth to `traceGroundTruthJSON-<slice>.json`, use the `-A<code>` file naming, and support hourly slices. Keep the `sender` deviation (D5). | VeReMi-Ext detectors (most work since 2020) read heading, acceleration and confidence. Without them, those detectors cannot run on our output. | S–M | Extend `VeremiReception` and `VeremiTruth`. Take the confidences from the frame's position/speed/heading confidence (already joined beside the message, per the `lib.rs` seam notes). Add a byte-exact fixture test against a small VeReMi-Ext sample. |
| **P4** | **Metrics:** a `Message` detection level (each received message, labelled by whether its sender was attacking at send time, as F2MD and VeReMi-Ext do) and MCC, informedness, markedness and Cohen's kappa at every level. | Published F2MD/VeReMi numbers are per message; ours are per subject or vehicle, so they cannot be compared directly. | S | `detection.rs`: add the formulas to `ConfusionMatrix`, with no CI for the derived scores, matching the F1 note. Add a per-message matrix fed from the `det.observation` records and the ground-truth channel. |
| **P5** | **A constant-velocity Kalman consistency check** in `ts103759` (class 2). One filter per sender with state (x, vx, y, vy); R from the broadcast confidences, floored at 4 m and 1 m/s; score = normalised innovation squared against a χ² gate; Δt ≤ max window. It replaces F2MD's seven ad-hoc variants. | Catches slow drifts and replays that the pairwise checks miss. F2MD reports the Kalman checks as its strongest single features. | M | Re-implement from standard Kalman theory and cite the F2MD TVT paper for the feature set. Keep it out of the frozen `detnorm_*` columns (new check id). |
| **P6** | **An oriented-box intersection check:** the claimed length, width and heading → separating-axis overlap ratio, scaled down linearly with Δt over `MAX_DELTA_INTER` 2 s. Include the receiver's own footprint, taken from its own belief. | Two cars side by side in adjacent lanes are about 3.5 m apart, while two ghosts overlap. Point distance below 2 m misses length-wise overlaps and false-flags tight queues. | S–M | J2735 `VehicleSize` and CAM `vehicleLength`/`Width` are on the frame. Cite F2MD TVT 2020 for the formulation. |
| **P7** | **F2MD's reachable-distance envelope** for PositionSpeedConsistency (the min/max distance reachable in Δt under a_max = 3 and b_max = 4.5, capped at v_max, with the speed-dependent braking margin), plus **PositionSpeedMaxConsistency**. | More correct physics when messages are lost and Δt grows to 0.5–3 s (congestion, packet loss); avoids false positives under hard braking. | S | Re-derive the kinematics; cite the constants. |
| **P8** | **Node-side fusion options** besides consecutive runs: (a) the CaTCh confidence-overlap score as an alternative scoring mode; (b) a windowed mean (current + 3 within 10 s); (c) a behavioural leaky bucket (exponential penalty, −1 per message, firing threshold), all with F2MD's D6 bug fixed. | Lets a study run "F2MD Threshold vs Behavioural" side by side on the same traffic. The leaky bucket damps GNSS-spike false positives. | S–M | New `fusion` parameter on the detector host; each mode gets its own model card citing WCNC 2019 or TVT 2020. |
| **P9** | **Derive ART (`max_plausible_range_m`) from our own radio**, per environment (the range at the sensitivity floor plus a margin from the propagation model), instead of F2MD's 420 m (Veins, 20 mW). | US OBUs transmit up to 20–23 dBm with highway line of sight beyond 420 m, so honest far links would false-flag. | S | Compute it at build time from `v2xw-radio`'s link budget and record it on the card. |
| **P10** | **A VeReMi-Ext replica scenario:** the LuST subsection (≈ 1.61 km²), 07–09 h and 14–16 h, 30 % attackers, the 19 F2MD attacks (P12), 1 Hz beacon option, and an optional `veremi-ext` GNSS preset (§4.2). Publish our detector numbers next to the published VeReMi-Ext numbers. | Gives a direct benchmark against the literature. | M | Import the LuST `net.xml` through `v2xw-world::sumo`; verify the LuST licence first. The preset is opt-in and never the default. |
| **P11** | **A reference ML baseline:** a feature vector of all check scores over the last 3 messages → XGBoost and MLP via `PyDetector`, trained on our own export. | F2MD's ML results are the usual baseline; we need our own number on the same footing. | M | Training recipe under `docs/guides`; no F2MD Python is copied (it has no licence). |
| **P12** | **An F2MD-faithful attack profile** `f2md-19`, separate from the frozen legacy catalogue: the 19 attacks with F2MD semantics and the ±55 % per-attacker spread (applied correctly, unlike D2), including the missing ConstSpeed, RandomPosOffset, RandomSpeedOffset, neighbour DataReplay and Disruptive, DoSDisruptive, DataReplaySybil, DoSRandomSybil and DoSDisruptiveSybil. The Sybil variants draw from the attacker's **real credential pool**: when it runs out, the attack degrades and logs the refusal, as `attack_ext` already does. | Without it, "ConstPos" and "DataReplay" in our results are not the attacks a reader assumes. P2 depends on it for honest code mapping. | M (2–3 days) | A new module `attack_f2md.rs` behind the existing `Attacker` trait and firewall (neighbour replay uses only `AttackerView` receptions). Each attack's card cites ICC 2020 and TVT 2020 and lists the constants. |

Suggested order: P1 → P2 → P3 → P4 (all small, and they unblock honest comparison) → P12 → P5,
P6, P7 → P8, P9 → P10, P11.

**Status (WP-02, 2026-10-23): P1 and P9 are done** in `crates/v2xw-threat/src/ts103759.rs`.
The beacon thresholds come from a `BeaconProfile` (`bsm-j2945-1` by default,
`cam-en302637-2`, and `f2md-veremi-1hz` for comparison runs, which reproduces 0.9 / 3.1 /
2.1 s). The windows scale with the profile's *longest* conforming interval rather than the
nominal one, so a J2945/1 sender stretched by congestion control towards `vMaxITT` is still
compared. The acceptance range is a per-environment table derived from `v2xw-radio`
(04-models.md §14 gives the numbers); under TR 37.885's urban LOS law it is about 42 km, so
range plausibility does almost nothing in cities, and the card says so. The suite is still
not selectable in `detection.local` (the engine ships `detect/legacy-12` and
`detect/us-sm-mbr`), so these settings reach it through `Ts103759Params::apply`, not through
the scenario yet.

## 6. What the engine does that F2MD cannot

- **A real security envelope.** IEEE 1609.2 / TS 103 097 signatures, SCMS certificate chains,
  validity windows and regions, CRLs, and a post-quantum mode. That makes the following
  attacks and checks possible, and F2MD has none of them: InvalidSignature, ExpiredCert,
  NotYetValid, WrongRegionCert, `SignatureInvalid`, `CertificateUnknown`, `ForeignRegion`, and
  the four verification states.
- **Credential-bounded attackers.** A Sybil attacker has only the certificates its device
  holds. F2MD's attackers mint unlimited pseudonyms.
- **The firewall** (I-T1 to I-T3, enforced by types). Detectors run on belief only. F2MD's
  proximity check (D4) and its VeReMi `sender` field (D5) both leak ground truth.
- **Determinism.** Every draw comes from a seeded RNG domain and outputs are byte-reproducible.
  F2MD reseeds from the wall clock on every draw (D1).
- **Real report formats and the full loop.** TS 103 759 and the US SM MBR, MA corroboration,
  two-authority identity resolution, revocation and CRL distribution. F2MD's MA counts reports
  and never revokes.
- **Threat families beyond beacons:**
  - jamming (constant, reactive, random duty);
  - oversized floods;
  - fake CPM and the CPM-consistency check;
  - fake DENM hazards;
  - VRU impersonation and position spoofing;
  - selective dropping in forwarding;
  - a compromised RSU;
  - poisoned misbehaviour reports;
  - a passive privacy observer with linkability metrics.
- **Sensors.** The class-4 perception cross-check on the node's own sensors, with coverage
  gating. F2MD uses an oracle instead.
- **Message generation.** 10 Hz BSM, CAM generation rules, LTE-V2X and NR-V2X sidelink models,
  and DCC. F2MD defaults to a 1 Hz beacon.
- **Dataset hygiene.** The leakage linter (written against F2MD's `senderRealId` leak), dataset
  datasheets and quantisation grids.
