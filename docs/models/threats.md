# Threats, detection and the misbehaviour authority

Code: `crates/v2xw-threat` (models), `crates/v2xw-engine/src/phase2.rs` and
`ma_policy.rs` (what a scenario can select and how it is wired), jammer emission in
`crates/v2xw-radio/src/jamming.rs`.

## The firewall

Three rules, each enforced by a type:

- An attacker receives an `attack::AttackerView`: no world, no actor index, no actor id.
  What it may observe and do is declared by its capability (`capability.rs`).
- Detectors and MA pipelines run on belief only: every input is an `obs::ObservedMessage`,
  the receiver's own `SelfBelief` and its own map store.
- Every action that changes bytes on the air is logged host-side on `gt.attack.action`
  with the true actor id (`attack::log_actions`), which is what labels a dataset;
  `is_falsified` decides the label.

A detector that read ground truth would not crash or look wrong; it would score
perfectly. That is why the boundary is a type and not a convention.

## Attackers (`threats.attackers`)

Each entry names a model, a population (exactly one of `fraction`, `count`, `actor_ids`),
an optional `schedule` (`from_s`, `to_s`) and `params` (`phase2::ATTACKER_PARAMS`, for
example `intensity`, `const_pos_offset_m`, `delay_s`, `sybil_ghosts`). The timeline's
`attack.wave` restricts falsification to windows.

```yaml
threats:
  attackers:
    - id: threat/attacker/legacy/ConstPos
      fraction: 0.02
      schedule: {from_s: 30.0, to_s: 300.0}
```

`threat/attacker/legacy/<Kind>` renders one kind of the base catalogue
(`attack::AttackKind`), in the VeReMi and VeReMi Extension tradition:

| Family | Kinds |
|---|---|
| position | `ConstPos`, `ConstPosOffset`, `RandomPos`, `Teleport`, `SineWavePos` |
| speed | `ConstSpeedOffset`, `RandomSpeed`, `StopAndGo` |
| heading | `ReversedHeading`, `HeadingOffset` |
| timing | `DataReplay`, `DelayedMessages`, `OutOfOrder` |
| stealth | `SlowDrift`, `AlongRoadOffset` |
| identity | `Sybil` (ghost identities under valid pseudonyms) |
| credential | `InvalidSignature`, `ExpiredCert`, `NotYetValid` |
| combined | `Disruptive`, `PosSpeedInconsistent`, `PosHeadingInconsistent`, `EventualStop` |

The catalogue also defines `DoS`, `DoSRandom`, `VruImpersonation`, `VruPositionSpoof`,
`FakeHazard` and `SelectiveDrop`; the engine refuses them by name with the reason (its
node sends one frame per generation, a BSM carries no station type, attacker DENMs are
not sent, vehicles relay nothing), rather than accepting a setting that would do nothing.

`threat/attacker/f2md/<Kind>` renders the F2MD (VeReMi Extension; Kamel et al., IEEE T-VT
69(6), 2020) and VeReMi NextGen behaviours whose semantics the base catalogue does not
have, under names that do not collide with it:

| Source | Kinds |
|---|---|
| F2MD | `UniformRandomPos` (F2MD `RandomPos`), `ConstSpeed`, `RandomSpeedOffset`, `NeighbourReplay` (F2MD `DataReplay`: a neighbour's messages), `DoSDisruptive`, `DataReplaySybil`, `DoSRandomSybil`, `DoSDisruptiveSybil` |
| VeReMi NextGen | `FeignedBraking`, `AccelerationMultiplication`, `PositionMirroring`, `ZeroSpeedReport` |

Each edits only the fields its definition names and records every edit as a
`gt.attack.action` with its magnitude. A flooding copy is a ghost under the attacker's own
pseudonym at the multiplied rate; a Sybil copy is signed with one of the concurrent valid
pseudonyms the attacker holds, never a fabricated certificate. The F2MD kinds carry their
F2MD attack code into the VeReMi exporter's `veremi-ext` codebook.

`threat/attacker/evasion/BenignClassMimic` falsifies while writing the fields an MA can
use to infer a device class (the BSM `secMark` phase relative to the GNSS epoch, the stated
`PositionalAccuracy`) so that it looks like a benign class, to test MA rules that
discount reports against such classes.

## Jammers (`threats.jammers`)

| Id | Behaviour |
|---|---|
| `attacker/jammer/constant` | always on between `from_s` and `to_s` |
| `attacker/jammer/pulsed` | on for `duty` of every `period_ms` |
| `attacker/jammer/reactive` | transmits when it hears a frame above `trigger_dbm` |

A jammer enters the interference sums as a transmitter, so reception loss follows from
SINR. `at: busiest-junction` places it on any city.

## Compromised roadside units

`threats.compromised_rsus` names units and `threats.compromised_rsu_attack`
(`threat/attacker/compromised-rsu`) what they do: `SuppressForwardedReports` and
`PoisonForwardedReports` on the reporting path, and `FalseCrl` (a forged list on the
air). The model also declares `FalseSpat`, `FalseMap` and `FalseCtl`, which the engine
refuses.

## Library models not selectable from a scenario

`v2xw-threat` also implements ghost vehicles, relay replay, oversized flooding, wrong-region
certificates, phantom CPM objects and the three jamming behaviours as attacker logic
(`attack_ext`), misbehaviour-report poisoning by colluders (`poison`, `report::forge`), and
two-authority identity resolution (`resolve`, `threat/ma/two-authority`). They are tested
in the crate but not wired into the engine's scenario path.

## Local detectors (`detection.local`)

| Id | Suite | Notes |
|---|---|---|
| `detect/legacy-12` | twelve plausibility and consistency checks (position/speed consistency, position jump, heading consistency, stale or replay, frozen position, implausible acceleration, Sybil co-location, acceptance range, beacon frequency, signature verification, certificate validity, map off-road), two gated checks and a Kalman consistency feature | VeReMi and F2MD lineage; normalised outputs feed the dataset's detector columns |
| `detect/us-sm-mbr` | exactly the PSID 0x20 (BSM) report subtypes of the SCMS Manager *Misbehavior Report and Application Specification for CV Pilot Deployment* v1.0 §2.3, with its default thresholds (1 m GPS drift, 90 m/s, 10 m/s², brake/acceleration; acceptance range off) | the default under a US deployment |
| `detect/ts103759` | ETSI TS 103 759 observation classes 1-5: implausible values; inconsistency with the same station's earlier messages; with the map and local environment; with on-board sensors (perception cross-check); with other stations' messages (physical intersection, CPM consistency), plus envelope checks | thresholds cited per check; a check whose input the receiver lacks is absent from the verdict, not scored zero |

### Check names

The legacy and US suites name each check with `v2xw_threat::DetectorId`. The name below is
how the `reason_codes` and `detnorm_*` dataset columns and the `det.observation` records
spell it.

| Check | Fires when | Raised by |
|---|---|---|
| `positionSpeedInconsistency` | the claimed displacement does not match the claimed speed over the interval | `detect/legacy-12` |
| `positionJump` | the claimed position moved further than any plausible motion allows | `detect/legacy-12`; `detect/us-sm-mbr` as 1VInconsistent/RandomPosition |
| `headingInconsistency` | the claimed heading disagrees with the bearing of the claimed motion | `detect/legacy-12` |
| `staleOrReplay` | the generation time is old, or the claim repeats a stored one | `detect/legacy-12` |
| `constantPositionFrozen` | the claimed position is frozen while the claimed speed is not zero | `detect/legacy-12`; `detect/us-sm-mbr` as 1VInconsistent/ConstantPosition |
| `implausibleAcceleration` | the implied acceleration exceeds what a vehicle can do | `detect/legacy-12`; `detect/us-sm-mbr` as Implausible/MaxAcceleration (10 m/s²) |
| `sybilCoLocation` | several distinct certificates claim nearly one point and one heading | `detect/legacy-12` |
| `acceptanceRangeThreshold` | the claimed position is further away than this receiver can hear | `detect/legacy-12`; `detect/us-sm-mbr` as ObsImplausible/Distance (off by default) |
| `beaconFrequency` | the sender transmits faster than the message rate allows | `detect/legacy-12` |
| `signatureVerification` | the signature did not verify | `detect/legacy-12` |
| `certValidity` | the certificate is expired or not yet valid | `detect/legacy-12` |
| `mapOffRoad` | the claimed position is not on any road the node's map knows | `detect/legacy-12` |
| `vruImpersonation` | a beacon declaring a vulnerable-road-user type does not move like one | `detect/legacy-12`, gated: only when station types are in play |
| `denmPlausibility` | an event message announces a hazard the sender's own claimed speed contradicts | `detect/legacy-12`, gated: only when the event-message layer is in play |
| `kalmanConsistency` | the residual against a constant-velocity tracker is large; a soft feature that never raises a report on its own | `detect/legacy-12` |
| `implausibleSpeed` | the claimed speed is above the US MaxSpeed threshold (90 m/s) | `detect/us-sm-mbr` only; no dataset column |
| `brakeAcceleration` | the BSM says its brakes are on and its `accelSet.long` is not negative | `detect/us-sm-mbr` only; no dataset column |

The US profile judges only BSMs the receiver verified, compares each with the previous one
from the same certificate, and checks nothing else. `detect/ts103759` reports ETSI
observation classes rather than these names.

The position-plausibility check follows F2MD (Kamel et al., IEEE T-VT 69(6), 2020).
Detectors fire `det.observation` records; reports follow the US MBR format
(`v2xw_proto::scms::mbr`) or TS 103 759 under the ETSI PKI.

## The misbehaviour authority (`detection.ma`)

| Id | Decision rule |
|---|---|
| `threat/ma/corroborated` | correlates reports about one pseudonym into events (all observations within `event_window_s`), counts an event only when at least `event_min_reporters` trusted reporters witnessed it, revokes when `min_events` corroborated events fall inside `window_s` with at least `min_reporters` distinct reporters; many witnesses of one message count once |
| `threat/ma/legacy-window` | k = 3 trusted reporters, 4 distinct seconds, a 3 s span, inside a 15 s window, with a reporter budget of 30 and a reputation cap of 40 |
| `threat/ma/policy` (`security.scms.ma_policy`) | the decision rule as one declarative policy: a preset fills every field, explicit fields override it |

A reporter counts as trusted only if it is not revoked, has not itself been reported
beyond the reputation cap and has not exceeded its report budget, which bounds what a
colluding coalition can achieve; the gate can be turned off to measure its value. The
deployed US MA rule is not published (the CV pilots used operator review), so the rule is
a family, not one assumed answer. `v2xw ma-replay <run>` replays a run's recorded reports
through a family of rules without re-simulating, reporting honest devices blocked per 10⁶
device-hours and the attacker's time to action (needs `security.scms.report_records: true`).

## Privacy observer (`threats.eavesdropper`)

`threat/observer/passive-privacy` is a passive adversary: receivers at a share of the
signalised junctions that only listen and try to link a vehicle's pseudonyms across
changes. Its tracker, coverage, records and the `linkability` metric are on
[Privacy](privacy.md).

## Attack search

`v2xw_experiment::foundry` is a MAP-Elites search over attack parameters on the same run
executor as sweeps, with an evaluation of whether a mutation operator beats random search
(`foundry_eval`).

## Modelled versus simplified

| Modelled | Simplified or absent |
|---|---|
| attackers that change bytes on the air under valid or invalid credentials, logged on ground truth | `DoS`, `DoSRandom`, `VruImpersonation`, `VruPositionSpoof`, `FakeHazard`, `SelectiveDrop` and three compromised-RSU kinds are refused by name |
| jammers as transmitters in the interference sums | |
| three local detector suites on belief only | a check whose input the receiver lacks is absent from the verdict, not scored |
| the MA decision as a family of rules with reporter trust gates, replayable offline (`v2xw ma-replay`) | the deployed US MA rule is unpublished; no single rule is asserted |
| | ghost vehicles, relay replay, flooding, wrong-region certificates, phantom CPM objects, report poisoning and two-authority resolution are library models not wired into the scenario path |
