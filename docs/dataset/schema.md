# V2X-SCMS-MBD dataset: release schema v1.1

This is the layout of the V2X-SCMS-MBD release, version 1.1 (`release_schema` 1). It
describes the files the release holds, their columns and how the columns are filled. The
mini release (`v2x-scms-mbd-mini`) is built from runs of the V2X World Simulator with the
full US SCMS in the loop: version 1.1 is a one-run sample (§10), and the eleven-run design of
§10 is the planned full release. One run is written by the simulator's `dataset` exporter;
`tools/dataset/assemble_release.py` merges runs into this tree and
`tools/dataset/validate_release.py` checks it.

## 1. What the dataset is

Every run is published as **two observer views of the same traffic** plus the answers:

| View | Who sees it | One row is | Closest existing dataset |
|---|---|---|---|
| **local** | each vehicle's OBU | a message it received, a check that fired, a report it filed, a credential event | VeReMi, VeReMi Extension, VeReMi NextGen |
| **global** | the Misbehavior Authority (MA) | a report it decrypted, a decision, a CRL entry | DARE (the only report-level set) |
| **truth** | nobody in the system | who really sent what, which attack, what happened downstream | VeReMi `GroundTruth` file |

A message can be followed from the air, to a neighbour's check, to the report, to the
MA's decision, to the CRL, to the moment neighbours stop trusting the attacker.

Three rules hold across the release:

1. **No answer inside an observer file.** Truth lives in `truth/` (and in the joined
   `ml/` tables, which say so). A linter refuses a release in which any `local/` or
   `global/` column, at any nesting depth, is a truth key, or any string value is an
   engine report id or attack name.
2. **Join keys are computed from the bytes.** Anyone holding a message can compute its
   digest, so a key reveals nothing a real observer could not learn (§5).
3. **Splits are by run, never by message.** Each run is in exactly one split, and no
   scenario and seed appear in two splits.

## 2. Lessons from the existing datasets

| Dataset | Format | What is kept | What is fixed |
|---|---|---|---|
| VeReMi 1.0 (2018) | JSON per receiver (`type 2` own GPS, `type 3` received BSM) + `GroundTruthJSONlog` | per-receiver layout; truth in a separate file joined by message id | `sender` is the real module id inside a receiver file (a label leak) |
| VeReMi Ext (2020) | same, sliced, `-A<code>` file names, F2MD codes 0–19 | the attack codebook, kept as a crosswalk column | `senderRealId`; the attack code in the file name; random-message splits |
| VeReMi NextGen (2026) | JSON → Parquet; receiver and sender blocks inline | Parquet; a held-out test set; the receiver's own state on each row | `sender_id` and `attacker` written inline next to the features; no security fields; no report or MA layer |
| DARE (2020) | one JSON report: `Metadata`, `Messages` (evidence), `Checks` (scores) | report = metadata + evidence + check scores | `senderRealId`/`reportedRealId` inside the report; no revocation, so no outcome |

## 3. File layout

```
README.md                         dataset card (Hugging Face YAML header + datasheet)
croissant.json                    MLCommons Croissant metadata (with RAI fields)
manifest.json                     SHA-256, bytes and row count of every file
runs.parquet                      one row per run (§4)
schema/<table>.json               columns, units, visibility and a description per column
                                  (file name: the table with '/' as '_', e.g. local_rx.json)
local/{ego,rx,checks,reports,pki}/split=<s>/run=<id>/part-00000.parquet
global/{reports,decisions,crl}/split=<s>/run=<id>/part-00000.parquet
truth/{devices,pseudonyms,transmissions,receptions,reports,timeline,app_effects}/split=<s>/run=<id>/part-00000.parquet
ml/{local,global}/split=<s>/run=<id>/part-00000.parquet   joined, labelled training tables (§8)
compat/veremi/<run_id>.zip        VeReMi Extension JSON traces per receiver + ground truth
compat/dare/<run_id>.zip          DARE-style report JSON from global/reports; the real ids
                                  in a separate file (real_ids.json)
scenarios/<run_id>.yaml           the scenario and seed that regenerate the run
```

A table with no rows in a run (for example `ml/global` for a run in which no report
reached the MA) has no part file for that run.

**Format.** Apache Parquet, zstd. Lists and structs are native Parquet types, never JSON
strings, so the Hugging Face viewer can filter on them. Each table is a Hugging Face
config (`local_rx`, `global_reports`, `truth_timeline`, ...) with `train`, `validation`
and `test` splits over the `split=` partitions; `runs` is one config with one split.

**Conventions.**
- Times are `int64` ns since run start (`*_ns`); `runs.t0_utc` anchors them.
- On-wire clocks are kept raw: 1609.2 `generation_time_us` (TAI µs since 2004) and J2735
  `sec_mark_ms`.
- Positions come twice: `lat_deg`/`lon_deg` as broadcast, and `x_m`/`y_m` in local ENU
  metres from `runs.origin_lat_deg` / `origin_lon_deg` (a procedural world's origin is
  (0, 0)).
- Units are in the column name: `_m`, `_mps`, `_mps2`, `_deg`, `_dbm`, `_bytes`, `_ms`,
  `_us`, `_ns`.

## 4. `runs.parquet`

One row per run. It is the only place experiment settings appear, so a model cannot read
them off the rows.

| Column | Notes |
|---|---|
| `run_id` | partition key everywhere (`s01` in the v1.1 sample; `r01` ... `r11` in the planned full release) |
| `split` | `train`, `validation` or `test` |
| `tier` | `mini` |
| `heldout_attacks` | attacks absent from every train run (`[ReversedHeading, HeadingOffset]`) |
| `scenario_name` | the scenario's `meta.name` |
| `city`, `world_source` | world name and source (`procedural`) |
| `t0_utc`, `duration_s` | UTC instant of t = 0; simulated duration |
| `origin_lat_deg`, `origin_lon_deg` | local ENU origin |
| `vehicles`, `equipped_fraction`, `demand_veh_per_h` | fleet |
| `rat`, `bsm_rate_hz` | radio access technology; observed mean BSM rate per transmitting device |
| `scms_mode`, `calendar` | `in-loop`; `real` or `compressed` |
| `local_suite`, `ma_policy` | local detector ids; MA policy id |
| `attacker_fraction`, `attack_mix` | share of attacking vehicles; `list<struct{id, fraction, from_s, to_s, params}>` covering the attackers, jammers, compromised RSUs and the passive observer |
| `instrumented_receivers` | OBUs with `local/rx` and `local/ego` rows (§6) |
| `engine_version`, `git_sha`, `scenario_hash`, `seed` | regeneration |
| `release_schema` | this layout's version |

## 5. Identifiers

| Key | What it is | Who can compute it | Appears in |
|---|---|---|---|
| `spdu_digest` | first 8 bytes of SHA-256 over the signed message (1609.2 SPDU), hex | anyone holding the message | local, global (evidence), truth |
| `cert_id` | the signing pseudonym certificate's HashedId8 (1609.2), hex | anyone holding the message | local, global, truth |
| `report_id` | first 8 bytes of SHA-256 over the reporter-signed MBR, hex; null for a report never built for upload | reporter, MA | local, global, truth |
| `la_link` | opaque token, equal for two certificates of one device | the MA, **only after** asking the Linkage Authorities | global (nullable), truth |
| `receiver_ref` | first 16 hex of SHA-256 over the run seed and the device | that OBU | local, truth |
| `rx_id` | first 16 hex of SHA-256 over the run seed, receiver and reception | — | local, truth |
| `tx_id`, `device_id` | the engine's transmission and device ids | oracle only | truth only |

A verbatim replay has the same bytes, so it has the same `spdu_digest` as the original,
as it does for a real receiver. Truth resolves it per reception
(`truth/receptions.rx_id → tx_id`).

## 6. Local view (what an OBU knows)

`local/rx` and `local/ego` are written for `instrumented_receivers` OBUs per run (10 in
the mini release): the OBUs whose SHA-256 of seed and device id sorts first, so the choice
uses no random draw. `local/checks`, `local/reports` and `local/pki` are written for
**every** OBU, because the MA side depends on all of them.

**`local/ego`**: the instrumented OBU's own state, one row per own BSM, as that BSM
carried it. For an attacker that is what it broadcast (falsified), not its true belief.
`run_id, receiver_ref, t_ns, x_m, y_m, lat_deg, lon_deg, speed_mps, heading_deg,
accel_mps2, pos_accuracy_m, own_cert_id`

**`local/rx`**: one row per message the receiver decoded, every message type in one table
keyed by `msg_type`.

| Group | Columns |
|---|---|
| keys | `run_id, receiver_ref, rx_id, t_rx_ns, spdu_digest` |
| radio | `rssi_dbm`, `cbr` (the receiver's channel busy ratio at reception), `size_bytes` |
| security | `psid`, `cert_id`, `signer_kind` (`certificate` or `digest`), `issuer_id` (the signing certificate's issuer HashedId8), `verify_result` (`ok`, `bad_signature`, `expired`, `not_yet_valid`, `on_crl`, `unknown_issuer`, `not_checked`, `other`), `verify_latency_us`, `generation_time_us` |
| BSM content | `msg_type, msg_cnt, temp_id, sec_mark_ms, lat_deg, lon_deg, x_m, y_m, elevation_m`, `pos_accuracy` struct{`semi_major_m, semi_minor_m, orientation_deg`}, `speed_mps, heading_deg`, `accel` struct{`long_mps2, lat_mps2, vert_mps2, yaw_rate_dps`}, `brakes`, `transmission` (J2735 `TransmissionState` by name), `length_m, width_m, event_flags` |
| receiver context | `ego_x_m, ego_y_m, ego_speed_mps, ego_heading_deg` (the receiver's own last BSM before the reception, as VeReMi NextGen does; null before its first), `range_m` (from the receiver's position to the claimed position) |

**`local/checks`**: one row per check in a report the OBU filed, at the report's creation.
`run_id, receiver_ref, t_ns, rx_id, cert_id, suite, check, fired, score, excess`
- `suite` is `sm-mbr-us` (the deployed US CV-pilot checks), `research` (the legacy
  checks such as positionSpeedInconsistency, headingInconsistency, staleOrReplay,
  sybilCoLocation, beaconFrequency) or `ts103759` (the ETSI set).
- `fired` is true; `score` and `excess` are the reporter's detector score.
- `rx_id` names the reception the check ran on (the last evidence message) when the
  reporter is instrumented and that reception is in its `local/rx`; else null. Of an SPDU
  the reporter received more than once (a flood or a replay repeats one byte for byte),
  it is the last reception at or before `t_ns`.

**`local/reports`**: every MBR the OBU created, and what became of it in its store.
`run_id, receiver_ref, report_id, t_created_ns, subject_cert_id, mbr_type, subtypes[],
evidence_spdu_digests[], size_bytes, fate, t_uploaded_ns, path, weight_at_upload`
- `subtypes` are the reporter's own fired checks, known from creation; `mbr_type` is set
  for reports the MA decrypted.
- `evidence_spdu_digests` are the reporter's copy of the evidence it attached: the
  `spdu_digest` of every signed BSM in the report, equal to the MA's
  `global/reports.evidence[].spdu_digest`.
- `fate` is `uploaded` once an upload or delivery is recorded, else `evicted` (quota
  eviction or decay), `expired` or `pending_at_end`; `path` is `cellular` or `rsu_relay`.
- An `uploaded` row always names the bytes that left the device (`report_id`,
  `size_bytes`, `path`, `evidence_spdu_digests`), also for a report still in the RA's
  shuffle when the run ended, which no MA received.

**`local/pki`**: the OBU's own credential life.
`run_id, receiver_ref, t_ns, event` (`enrolled`, `batch_downloaded`, `pseudonym_change`,
`crl_downloaded`, `download_refused`), `cert_id, crl_series, crl_number, crl_entries,
bytes, latency_ms`. `bytes` and `latency_ms` are null: the simulator does not record them
per event.

## 7. Global view (what the MA knows)

**`global/reports`**: one row per MBR the MA decrypted. The fields are the US CV-pilot
report's (SM-MBR v1.0 and its ASN.1 v1.1: `EeMaApplicationSpecificMisbehaviorReport`,
`Psid20Mbr`, `SignedBsmsWithCertificate`), decoded.

| Group | Columns |
|---|---|
| keys | `run_id, ma_id, report_id, t_received_ns` |
| report header | `generation_time_us`, `generation_location` struct{`lat_deg, lon_deg, elevation_m`}, `psid` (0x20), `reporter_cert_id` |
| accusation | `subject_cert_id`, `subject_issuer` (the accused certificate's issuer HashedId8), `mbr_type` (`1VInconsistent`, `Implausible`, `ObsImplausible`), `subtypes` list<struct{`type, subtype, name, params`}> with `params` as `name=value` strings in the SM-MBR unit (`gpsDrift_mm=1000`) |
| evidence | `evidence` list<struct{`spdu_digest, generation_time_us, sec_mark_ms, msg_cnt, lat_deg, lon_deg, speed_mps, heading_deg, accel_long_mps2, brakes, pos_accuracy_m`}>; `evidence_start_ns` / `evidence_end_ns`: the generation times of the earliest and the latest attached BSM, in run time (equal when the report carries one BSM) |
| MA processing | `set_aside` (null, or why the MA set the report aside), `la_link` (null unless a decision's case named the report: the MA asks the Linkage Authorities only then), `la_link_t_ns` (that decision's time) |

The MA never learns the reporter's enrolment identity: the RA authenticates the upload but
cannot read it, so no reporter device id appears here. The simulator's own check names,
and fired checks that have no SM-MBR code, are not in `global/` because the MBR cannot
carry them; they are in `truth/reports`.

**`global/decisions`**: one row per MA decision.
`run_id, ma_id, t_ns, subject_kind` (`pseudonym` / `device`), `subject_cert_id, la_link,
decision` (`revoke`, `dismiss`, `monitor`), `rule, report_ids[], n_reports,
n_distinct_reporters`
- `report_ids` are the reports about the subject the deciding MA had received by `t_ns`.

**`global/crl`**: what the MA caused to be published.
`run_id, t_issued_ns, crl_series, crl_number, entry_kind` (`linkage_seed` / `hash_id`),
`la_link, hash_id, i_period, t_effective_ns`
- Rows are in issue order across providers; each provider numbers its own CRLs
  (`crl_number`).

## 8. Truth (the answers)

| Table | Key | Columns |
|---|---|---|
| `truth/devices` | `device_id` | `class` (vehicle / rsu / vru), `provider`, `gnss_class` (the device's BSM timing class), `label` (`benign` / `faulty` / `malicious`), `fault_kind`, `attack_id`, `attack_params[]` (`name=value`), `attack_start_ns`, `attack_end_ns`, `joined_ns`, `left_ns`, `la_link`, `receiver_ref`, `instrumented` |
| `truth/pseudonyms` | `cert_id` | `device_id, first_use_ns, last_use_ns, n_tx` |
| `truth/transmissions` | `tx_id` | `device_id, t_ns, spdu_digest, cert_id, label, device_label, attack_id, family`, crosswalk (`veremi_ext_code, veremi1_code, nextgen_name, dare_name`), `fields_falsified[], magnitude`, true `x_m, y_m, speed_mps, heading_deg, accel_mps2` at transmission, `gnss_error_m`, `fix_to_stamp_ms` |
| `truth/receptions` | `rx_id` | `tx_id, rx_device_id, tx_device_id, label, attack_id, true_distance_m` (one row per `local/rx` row) |
| `truth/reports` | `report_id` | `reporter_device_id, subject_cert_id, subject_device_id, subject_label, verdict` (`true_accusation` / `false_accusation`), `poisoned`, `fate, t_created_ns, t_at_ma_ns, reporter_checks[], nonstandard_triggers[]` |
| `truth/timeline` | `device_id` | per misbehaving or falsely accused device: `label, falsely_accused, attack_start_ns, first_falsified_ns, first_detection_ns, first_report_created_ns, first_report_uploaded_ns, first_report_at_ma_ns, decision_ns, crl_published_ns, crl_first_neighbour_ns, crl_median_neighbour_ns, crl_p95_neighbour_ns, n_neighbours, last_malicious_accepted_ns` |
| `truth/app_effects` | row | warnings an application raised because of a message from a faulty or malicious sender: `t_ns, app, receiver_device_id, tx_id, subject_cert_id, subject_device_id, subject_label, severity, driver_reaction` |

How the truth columns are filled:
- A message's `label` is `malicious` only when the message itself was falsified;
  `device_label` is the sender's. A frame the attack added (a flood's copy under the
  attacker's own pseudonym, `fields_falsified` `[repetitions]`; a Sybil ghost under
  another, `[signer]`) is falsified; a copy or ghost that claims another vehicle's state
  adds the fields it restates (`position`, `speed`, `heading`). Every falsified message is
  one `truth/transmissions` row with its label, also when the message reached the air
  seconds after it was falsified (a message held back by its own sender).
- A message more than one attack action edited (a DoSDisruptive beacon is restated as a
  neighbour and repeated; an evasion wraps a payload attack) carries the union of their
  fields in `fields_falsified`, sorted; `attack_id`, the crosswalk codes and `magnitude`
  are the primary attack's, the first action recorded on the message, which is the
  device's `truth/devices.attack_id`.
- `attack_start_ns` / `attack_end_ns` cover the scheduled attack window and the air times
  of the device's first and last falsified transmissions.
- `veremi_ext_code` / `veremi1_code` are the VeReMi Extension and VeReMi 1.0 attack codes:
  0 for a benign message, the F2MD code where the behaviour matches F2MD's, and **null**
  for an attack those codebooks have no code for. (The VeReMi trace files under
  `compat/veremi`, whose format needs an integer, write 1024 for it.)
- `faulty` is a first-class label: an honest device whose data is wrong beyond
  specification. Its `fault_kind` (`timestamp_lag`, `gnss_error`) comes from the device's
  BSM timing class.
- A compromised RSU is `malicious` with its kind as `attack_id`. Jammers have no device
  row (they send no messages); they, the compromised RSUs and the passive observer are
  listed in `runs.attack_mix`.
- `first_detection_ns` equals `first_report_created_ns` (see `local/checks`). CRL arrival
  is each neighbour's first `crl_downloaded` at or after the entry's issue; a neighbour is
  a receiver that decoded at least one of the device's messages.
- `truth/app_effects.tx_id` is the message the application names, else the sender's last
  transmission at or before the warning.
- `truth/reports.poisoned` is true for a forgery an attacker filed on purpose (an insider's
  `ReportPoisoning`: a car with valid credentials that behaves honestly on the air and
  accuses cars it heard of a position jump, attaching two genuine BSMs of each), false for every detection, a
  mistaken one included. A poisoned report against an honest car is a `false_accusation`,
  and the car is `falsely_accused` in `truth/timeline`. (Written after v1.1; the v1.1
  files have no such column.)
- A report a car relays through a roadside unit (a car without a cellular modem) has
  `path` `rsu_relay`. When a compromised unit replaces it with a forgery after taking it
  (`PoisonForwardedReports`), the car's row is still `uploaded` with the bytes it sent; the
  forgery never reaches an MA under the US SCMS, because a roadside unit holds no
  pseudonym certificate to sign an SM-MBR with, so on that path the unit's attack
  withholds the car's report from the MA while the car believes it delivered.
- Null by design: `truth/transmissions.fix_to_stamp_ms` and
  `truth/app_effects.driver_reaction` (the simulator does not record them).

**Labels exist at five levels:** message (`label`, `attack_id`), pseudonym
(`truth/pseudonyms` → `truth/devices`), device, report (true or false accusation), and
outcome (correct revocation, wrongful revocation, missed attacker: `truth/devices` against
`global/decisions`).

**`ml/` tables** are joined copies for people who only want to train:
- `ml/local`: `local/rx` plus `label`, `attack_id`, `family` and `veremi_ext_code` from
  truth;
- `ml/global`: `global/reports` plus `verdict` and `subject_label` from `truth/reports`.

Their joined columns say `JOINED` in `schema/`; `local/` and `global/` stay clean.

## 9. Attacks

Attackers run as `threat/attacker/<module>/<Kind>` in the scenario. The table lists the
families with the attacks of the simulator's catalogue; the mini release runs the ones
listed in §10.

| Family | Attacks | VeReMi Ext code / NextGen / DARE closest | Where it shows |
|---|---|---|---|
| Position | ConstPos, ConstPosOffset, RandomPos, Teleport, SineWavePos, SlowDrift, AlongRoadOffset, UniformRandomPos, PositionMirroring | 1, 2, 4, 3 (UniformRandomPos) / constantPositionOffset, randomPositionOffset, positionMirroring / position malfunction | local plausibility |
| Speed | ConstSpeedOffset, RandomSpeed, StopAndGo, ConstSpeed, RandomSpeedOffset, ZeroSpeedReport | 6, 7, 5 (ConstSpeed), 8 (RandomSpeedOffset) / constantSpeedOffset, randomSpeedOffset, suddenStop, zeroSpeedReport | local plausibility |
| Acceleration | FeignedBraking, AccelerationMultiplication | – / feignedBraking, accelerationMultiplication | local plausibility (`accelSet.long`) |
| Heading | ReversedHeading, HeadingOffset | – / reversedHeading | local plausibility |
| Inconsistent state | PosSpeedInconsistent, PosHeadingInconsistent, EventualStop | 9 / suddenStop / sudden stop | local consistency |
| Timing | DelayedMessages, OutOfOrder, DataReplay, NeighbourReplay | 12, 11 (NeighbourReplay) / timeDelayAttack, dataReplay / delayed, data replay | local timing and plausibility |
| Identity | Sybil, GhostVehicles, DataReplaySybil, DoSRandomSybil, DoSDisruptiveSybil | 16, 17, 18, 19 / trafficCongestionSybil / Sybil, traffic congestion | local and MA (one `la_link`, many certificates; the copies sign only with pseudonyms the device holds) |
| Credentials | InvalidSignature, ExpiredCert, NotYetValid, WrongRegionCert | – | security layer (`verify_result`) |
| Flooding | DoS, DoSRandom, Disruptive, OversizedFlood, DoSDisruptive | 13, 15 (DoSDisruptive) / dosAttack / DoS, disruptive | radio, CBR and rate (a DoSDisruptive copy goes on the air right behind its beacon, stamped at the multiplied rate: its own 1609.2 `generationTime` and `secMark`, k × 25 ms after the beacon's, and the random neighbour's position, speed and heading it claims, in its signed bytes) |
| Application | FakeHazard, VruImpersonation, VruPositionSpoof, FakeCpm | – | applications and perception cross-check |
| Suppression and relay | SelectiveDrop, RelayReplay | – / dataReplay (partly) | reception gaps, duplicate digests |
| Jamming | ConstantJamming, ReactiveJamming, RandomDutyJamming | – | radio only (no messages to label) |
| Compromised RSU | FalseCrl, SuppressForwardedReports, PoisonForwardedReports | – | PKI, report path |
| Against the MA | ReportPoisoning, BenignClassMimic (evasion) | – | global only |
| Privacy | PrivacyObserver (passive tracker; labels tracking success, not messages) | – | truth only |

The twelve F2MD / VeReMi NextGen behaviours run as `threat/attacker/f2md/<Kind>`. Where an
F2MD name is already a simulator name with a different behaviour, the F2MD one carries a
new name: `UniformRandomPos` is F2MD `RandomPos`, `NeighbourReplay` is F2MD `DataReplay`.
FeignedBraking, AccelerationMultiplication, PositionMirroring and ZeroSpeedReport have no
F2MD code (null in the tables). Every edit is recorded whatever its size, with its
`magnitude`, so VeReMi NextGen's conditional labels (acceleration changed by more than
0.25 m/s², position moved more than 1 m) can be applied by a consumer. A falsified stated
acceleration (more than 0.5 m/s² from the true one) labels the message malicious.
PositionMirroring's road-edge distance is a parameter (default half a 12 ft lane), not
measured from the map.

**Benign faults (label `faulty`):** BSM timing classes declared out of specification
(timestamp lag, GNSS error), set per device class in the scenario.

## 10. The mini release (v1.1)

**Version 1.1 is a one-run sample**, run `s01` (scenario `scenarios/dataset/v11-sample.yaml`),
in one split, `train`: the dataset base (`scenarios/dataset/base.yaml`) as it is, 75 s on
one procedural grid world with 60 vehicles from t = 0, every US SCMS entity, the compressed
SCMS calendar with a 30 s CRL cadence, MA review 0 s, the deployed receiver's smoothed GNSS,
the legacy-window MA rule and report records on.

It holds the ten main attacks, each on three attacker vehicles (27 in all), whose attacks
start at different points of their life: one from spawn (0 s), one between 15 and 25 s and
one at 35 or 40 s, each to the run's end, so how long an attacker's pseudonym has been heard
does not tell it apart from an honest one. Three of the ten are not put on the air by this
engine under their own names, and the nearest runnable behaviour stands in for them:

| Attack | In the sample as | Why |
|---|---|---|
| ConstPos, RandomPos, SlowDrift, RandomSpeed, EventualStop, DataReplay, Sybil | the legacy rendering of the same name | – |
| DoS | F2MD DoSDisruptive (the beacon rate multiplied, each copy a neighbour's claim) | the legacy flood needs extra frames per generation interval, which the host does not send |
| FakeHazard | VeReMi NextGen FeignedBraking (a false hard-braking claim in the BSM) | the host sends no attacker event messages |
| ReportPoisoning | the grid's roadside unit compromised, set to re-target half of the reports it relays at innocent cars from 15 s (`PoisonForwardedReports`) | the insider report filer is not wired into a run; the world has one roadside unit, so this is one unit, not three vehicles. **Inert in v1.1**: every car has a cellular modem, so no report is relayed through the unit and none is poisoned; `truth/devices` lists the unit as malicious, and no row elsewhere shows its effect |

Honest faulty devices come from the per-device BSM timing model with 15 % of devices out of
specification (`data/calibration/dataset-faulty-devices.yaml`: a stale-fix timestamp lag and
a GNSS random walk), labelled `faulty` in truth.

With one run there is one split: no test run, and nothing is held out
(`runs.heldout_attacks` is empty), so the held-out and cross-split checks of §11 report
"not applicable" instead of passing. Evaluate on the sample with folds grouped by `cert_id`.

### The planned full release

The eleven run files `scenarios/dataset/r1-...` to `r11-...` are the design of the full
mini release; they are committed but not recorded for v1.1. Eleven runs on one
procedural grid world: 60 vehicles from t = 0, every US SCMS entity, the compressed SCMS calendar with a 30 s CRL cadence, MA review 0 s, the deployed
receiver's smoothed GNSS, the legacy-window MA rule, report records on. Each run has its
own seed, so no two runs share traffic.

Every one of the 36 attacks runs on three vehicles (108 attacker vehicles in all), spread
over the runs, and the three start at different points of their life: one from spawn
(0 s), one about a quarter and one about half way into its run (15–25 s and 35–40 s in a
75 s run, 12–20 s and 28–32 s in a 60 s run), each to the run's end. How long an
attacker's pseudonym has been heard therefore does not tell it apart from an honest one.

| Run | Split | s | Attackers | Attacks (vehicles) and conditions |
|---|---|---|---|---|
| r01 | train | 75 | 12 | ConstPos, RandomPos, Teleport, ConstPosOffset, SineWavePos, SlowDrift, AlongRoadOffset, ConstSpeedOffset, RandomSpeed, StopAndGo, PosSpeedInconsistent, EventualStop |
| r02 | train | 75 | 12 | RandomSpeed, StopAndGo, PosSpeedInconsistent, PosHeadingInconsistent, EventualStop, DelayedMessages, OutOfOrder, DataReplay, ConstSpeedOffset, SlowDrift, AlongRoadOffset, SineWavePos; a compromised RSU poisoning forwarded reports |
| r03 | train | 75 | 11 | Sybil, InvalidSignature, ExpiredCert, NotYetValid, Disruptive, BenignClassMimic ×2, DelayedMessages, DataReplay, Teleport, RandomPos; a constant jammer (40–50 s); a compromised RSU suppressing forwarded reports; a passive privacy observer |
| r04 | validation | 75 | 12 | ConstPos, ConstPosOffset, PosHeadingInconsistent, OutOfOrder, Disruptive, Sybil, InvalidSignature, DataReplay, RandomSpeed, PosSpeedInconsistent, EventualStop, BenignClassMimic; a reactive jammer (40–50 s); a compromised RSU issuing a false CRL |
| r05 | test | 75 | 12 | ReversedHeading ×2, HeadingOffset, ConstPos, ConstPosOffset, PosHeadingInconsistent, OutOfOrder, Disruptive, ExpiredCert, NotYetValid, DelayedMessages, StopAndGo; a pulsed jammer (40–50 s) |
| r06 | train | 75 | 0 | no attacker; faulty honest devices (timestamp lag, GNSS error) |
| r07 | test | 75 | 9 | ReversedHeading, HeadingOffset ×2, RandomPos, Teleport, SineWavePos, SlowDrift, AlongRoadOffset, ConstSpeedOffset; faulty honest devices |
| r08 | train | 60 | 11 | FeignedBraking ×2, PositionMirroring ×2, ConstSpeed, NeighbourReplay, DoSDisruptive, DataReplaySybil ×2, Sybil, ExpiredCert |
| r09 | test | 60 | 9 | AccelerationMultiplication ×2, UniformRandomPos ×2, ZeroSpeedReport, RandomSpeedOffset, DoSRandomSybil, DoSDisruptiveSybil ×2 |
| r10 | train | 60 | 11 | FeignedBraking, PositionMirroring, ConstSpeed ×2, NeighbourReplay ×2, DoSDisruptive ×2, DataReplaySybil, InvalidSignature, NotYetValid |
| r11 | test | 60 | 9 | AccelerationMultiplication, UniformRandomPos, ZeroSpeedReport ×2, RandomSpeedOffset ×2, DoSRandomSybil ×2, DoSDisruptiveSybil |

The Heading family (ReversedHeading, HeadingOffset) is held out of train: its six
vehicles are in the test runs r05 and r07. The twelve F2MD / VeReMi NextGen behaviours
are split six and six: FeignedBraking, PositionMirroring, ConstSpeed, NeighbourReplay,
DoSDisruptive and DataReplaySybil in train (r08, r10), AccelerationMultiplication,
UniformRandomPos, ZeroSpeedReport, RandomSpeedOffset, DoSRandomSybil and
DoSDisruptiveSybil in test only (r09, r11): one behaviour of every family the twelve cover
on each side, so test holds six behaviours never seen in train, from families that were.

## 11. Quality gates

`tools/dataset/validate_release.py` runs these on the assembled release; its tests
(`tools/dataset/tests/`) break a fixture release to show each check can fail.

1. **schema**: columns, types and nullability per table, against `schema/`.
2. **joins**: evidence digests, `rx_id`s and `report_id`s resolve across the views.
3. **leakage**: no truth column, and no truth-only id value, in `local/` or `global/`.
4. **labels**: message, device and report labels agree; `attack_mix` fractions hold.
5. **time**: 0 ≤ t ≤ duration; per-receiver order; causal order across the views.
6. **units**: speeds, headings, radio values, latitude and longitude inside the run's area.
7. **splits**: one split per run; partitions agree with `runs.parquet`; held-out attacks
   absent from train; no scenario and seed in two splits. A release with no held-out attack
   or with one split says "not applicable" for the comparison it cannot make; the other
   parts are still checked.
8. **manifest**: SHA-256 and size of every file, and no unlisted file.
9. **shortcuts**: a random forest on metadata only (size, `msg_cnt`, signer kind,
   per-pseudonym rate and counts), with folds grouped by pseudonym, must stay at or below
   ROC-AUC 0.7 for attacks whose definition is not about rate or size.

## 12. Publication

- **Hugging Face** hosts the release (`akzaidan/v2x-scms-mbd-mini`); load a table with
  `load_dataset("akzaidan/v2x-scms-mbd-mini", "global_reports")`. The viewer works on
  every config; `compat/` holds zips.
- **Licence:** CC BY 4.0 for the data.
- **The card** follows Datasheets for Datasets; Croissant metadata sits beside it.

## 13. Not in v1.1

- `truth/kinematics` (10 Hz true state) and `wire/` (raw signed bytes): the exporter
  refuses `opts.wire: true`.
- A per-message check stream: `local/checks` holds the checks of filed reports only, and
  `local/check_scores` is not built.
- Separate `local/rx_<type>` tables: every message type is in `local/rx`.
- `local/pki.bytes` and `latency_ms`, `truth/transmissions.fix_to_stamp_ms`,
  `truth/app_effects.driver_reaction` (null in every row).
- Compromised-RSU FalseSpat, FalseMap and FalseCtl: declared in the catalogue, not driven.
- Worlds other than the procedural grid, and tiers other than `mini`.
- More than one run: v1.1 is the one-run sample `s01`, so it has no validation or test split
  and no held-out attack. The eleven runs of §10 are the planned full release.
- DoS and FakeHazard under their own names: the sample carries the nearest runnable
  behaviour for each (§10). ReportPoisoning now runs under its own name, as the insider
  report filer `threat/attacker/insider/ReportPoisoning` (parameters `report_prob`,
  `victim_pct`, `reports_per_interval`); v1.1 carries the compromised-unit stand-in, which
  poisons no report there (no report is relayed). The relay path's two exporter defects of
  v1.1 (a replaced report's upload without its `report_id` and evidence, and a second
  `local/ego` row for the relaying car at one instant) are fixed in the writer, checked by
  `scenarios/dataset/relay-poison.yaml`.
