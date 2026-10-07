# The driving-model library

Every way a vehicle in this engine can follow the one ahead, and steer across its lane, is
a plug-in behind a trait, registered in one list, selectable per vehicle class in the
scenario, and carrying a model card with its sources. This page lists what is there, how to
select it, what the cooperative models measure, and how to add another.

Code: `crates/v2xw-mobility/src/carfollowing/` (longitudinal), `src/coop.rs` (what V2X
delivered), `src/lateral/` (sub-lane), `crates/v2xw-engine/src/driving.rs` (scenario
wiring), `crates/v2xw-engine/src/run/coop.rs` (the network-to-controller feed).

## The models

| Short name | Id | What it is | Primary source | Reads V2X |
|---|---|---|---|---|
| `idm` | `mobility/car-following/idm` | Intelligent Driver Model, the engine's default (`urban-hcm` drivers) | Treiber, Hennecke & Helbing 2000; Kesting et al. 2010 | no |
| `eidm` | `mobility/car-following/eidm` | Extended IDM: improved IDM + constant-acceleration heuristic + jerk bound | Salles, Kaufmann & Reuss 2020; Treiber & Kesting 2013 §11.3.7 | no |
| `krauss` | `mobility/car-following/krauss` | Krauß's stochastic safe-speed model, SUMO's default | Krauß 1998 (DLR 98-08) | no |
| `gipps` | `mobility/car-following/gipps` | Gipps's behavioural model, Aimsun's | Gipps 1981, TR-B 15(2) | no |
| `w99` | `mobility/car-following/w99` | Wiedemann 99 psycho-physical thresholds, Vissim's (presets `vissim`, `sumo`) | PTV Vissim manual; w99-demo / SUMO `MSCFModel_W99` | no |
| `acc` | `mobility/car-following/acc` | Production ACC, the PATH multi-mode controller | Milanés & Shladover 2014, TR-C 48; Xiao et al. 2017, 2018 | no |
| `cacc-milanes` | `mobility/car-following/cacc-milanes` | PATH field CACC, 0.6 s gap | Milanés & Shladover 2014 | predecessor speed |
| `cacc-ploeg` | `mobility/car-following/cacc-ploeg` | CACC with acceleration feed-forward, 0.7 s gap | Ploeg et al. 2011, IEEE ITSC | predecessor speed + acceleration |
| `cacc-path` | `mobility/car-following/cacc-path` | Leader–predecessor platoon, 5 m constant spacing | Rajamani 2012 ch. 7; Plexe (Segata et al. 2014) | predecessor + platoon leader |
| `sublane-sl2015` (lateral) | `mobility/lateral/sublane-sl2015` | Sub-lane lateral positioning: pass a cyclist within a wide lane, filter where lawful | Semrau & Erdmann 2016 (SUMO SL2015) | — |

Every card says which of its numbers are primary, which are secondary (quoted from a
reproduction, not re-read), which follow another tool's source code (SUMO's is EPL-2.0 and
was **read, never vendored**), and which are choices with a calibration plan. `v2xw
models` and the manifest list them with everything else.

## Selecting models

```yaml
actors:
  vehicles:
    driving:
      car_following: { model: krauss }                # every class unless by_class says otherwise
      by_class:
        truck:     { model: gipps }
        passenger: { model: cacc-ploeg, share: 0.4 }  # 40 % of cars are cooperative
        bus:       { model: w99, params: { preset: sumo } }
      cooperative: { max_age_s: 0.3, association_gate_m: 5.0 }
      lateral:   { model: sl2015, params: { filter: { motorcycles_share: true } } }
```

* Leaving `driving` out changes nothing: the run is byte-for-byte the run it was before the
  library existed, and the scenario's content hash is unchanged (the block is skipped when
  serialised at its default).
* `params` is the model's own parameter struct (`KraussParams`, `W99Params`, `CaccParams`,
  …); unknown keys are refused at validation, naming the key.
* `share` splits a class: each vehicle draws once, from its own keyed stream, whether it
  drives this model or the default — a penetration-rate study is one number.
* `lateral.params.filter.motorcycles_share` only takes effect where the jurisdiction's rules
  allow filtering (`v2xw_mobility::rules`); every preset this build knows prohibits it, so
  it is a what-if switch for a lane-splitting jurisdiction.

## Cooperative driving that depends on the network

SUMO's CACC decides it may drive cooperatively by checking whether its predecessor's
*model id* is CACC; no message is ever sent. Here a CACC or platoon law drives on what the
simulated network actually delivered:

1. Every BSM or CAM a cooperative vehicle's node delivers — after its own security stack
   checked it, so invalid and revoked messages never reach the controller — is offered to
   the follower's `CoopLink` (`crates/v2xw-engine/src/run/coop.rs`).
2. It is kept only if it **associates by position** with the follower's radar target (or
   its platoon head): the claimed position against where that vehicle truly was at the
   claimed generation time, within `association_gate_m`. GNSS error, pseudonym changes, a
   forged position or a ghost vehicle therefore reach the platoon as they would on the road.
3. The leader's acceleration comes from the message's own octets (`accelSet.long`,
   `longitudinalAcceleration`); without the payload the link differentiates claimed speeds.
4. A message older than `max_age_s` (by generation time) is not used: the vehicle drives the
   `acc` law at its 1.1 s gap instead — **graceful degradation**, per vehicle, per step.

### Measuring it

`v2xw_mobility::carfollowing::platoon` runs one platoon through a controlled channel
(Bernoulli or Gilbert–Elliott loss, fixed latency) and reports:

| Metric | Definition |
|---|---|
| `accel_amplification` | `max_i ‖a_i‖₂/‖a_{i−1}‖₂` over followers — string stable at ≤ 1 (Ploeg, van de Wouw & Nijmeijer 2014, IEEE T-CST 22(2)) |
| `gap_error_amplification` | the same on the spacing error's ∞-norm |
| `rms_gap_error_m`, `max_gap_error_m` | deviation from the gap held before the disturbance |
| `throughput_veh_h` | time-averaged platoon flow `3600·v̄/(gap + length)` |
| `cooperative_share` | fraction of follower steps on the cooperative law |
| `min_gap_m`, `max_decel_mps2` | safety |

```text
cargo run -p v2xw-mobility --example platoon_sweep            # loss × latency × model table
cargo run -p v2xw-mobility --example platoon_sweep -- --burst 4
```

### What the sweep shows (2026-10-07, this build)

Eight vehicles at 25 m/s, leader brakes 2 m/s² for 3 s, 10 Hz beacons, 10 ms latency,
independent loss. "Gain" is the acceleration L2 gain (string stable at ≤ 1); "flow" is
veh/h/lane.

| Model | loss 0 gain / flow | loss 0.1 | loss 0.3 | loss 0.5 |
|---|---|---|---|---|
| `acc` (radar only) | 0.967 / 2,582 | same | same | same |
| `cacc-ploeg` | **0.966 / 3,643** | 0.970 / 3,643 | 1.029 / 3,614 | 1.043 / 3,558 |
| `cacc-ploeg`, abrupt switch | 0.966 / 3,643 | 1.142 / 3,293 | 1.099 / 2,816 | 1.057 / 2,683 |
| `cacc-milanes` | 1.003 / 3,872 | 1.015 / 3,875 | 1.010 / 3,739 | 1.078 / 3,625 |
| `cacc-milanes`, abrupt switch | 1.003 / 3,872 | 1.249 / 3,646 | 1.095 / 2,980 | 1.085 / 2,724 |
| `cacc-path` (5 m spacing) | 1.022 / 8,745 | 1.206 / 3,244 | 1.054 / 2,690 | 1.015 / 2,632 |

What it says:

* With every message arriving, Ploeg's feed-forward CACC damps the disturbance and carries
  41 % more than ACC; the 5 m PATH platoon carries 3.4 times as much.
* **Switching abruptly to ACC on a lost message is what destabilises a CACC platoon**:
  10 % loss takes Ploeg's gain from 0.97 to 1.14 and costs 10 % of the flow. With the
  graceful ramp (`degrade_ramp_s`, 5 s) the same 10 % loss leaves it string stable at
  full flow. PATH keeps the abrupt switch by default: ramping a 5 m platoon's fallback up
  from a 0.2 s gap produced collisions at 50 % loss.
* Latency is judged by message age, so `coop.max_age_s` (0.3 s) sets a hard limit: at
  300 ms one-way latency every message is stale on arrival and every CACC drives as ACC.
  The constant-spacing PATH law is the most latency-sensitive: at 200 ms its RMS gap error
  is 22 m against 0.4 m at 10 ms.
* The ACC baseline is string stable in this test (gain 0.967). Milanés & Shladover 2014
  found the production ACCs they measured string-unstable; the multi-mode model's heavily
  damped gap-closing mode is what makes it stable here, and that difference is recorded as
  a limitation rather than tuned away.

The controlled channel isolates the controller. In a full run the same `CoopLink` is fed by
the radio, MAC, congestion control and security stack, so a scenario that jams the channel,
congests it, revokes a certificate or forges an acceleration moves the platoon's numbers —
that is the experiment no other open tool can run end to end.

## How to add another car-following model

1. **Write the module** `crates/v2xw-mobility/src/carfollowing/<name>.rs`:
   * a `Params` struct (`#[serde(default, deny_unknown_fields)]`, `Default` = the cited set);
   * the model struct holding its params and its `ModelCard`;
   * `impl Model` (returns the card) and `impl CarFollowing`:
     * `accel(ego, leader, lane, w)` — start from `common::situation(...)` so the speed
       limit, weather and fog apply exactly as for every other model; a `leader` with
       `is_vehicle() == false` is a stop line or signal;
     * `profile(class)` — the per-class driver your calibration gives;
     * `begin_step(now, dt)` if the model is discrete-time or draws per-step noise (keep a
       `common::StepClock`; draw with `common::keyed_uniform(seed, MODEL_ID, actor, step, slot)`,
       never a thread-local or an ad-hoc generator);
     * `cooperative()` if it reads V2X (return your `Arc<CoopLink>`; read it with
       `link.fresh(ego, leader, clock.now())` and degrade on `None`);
   * `pub fn card(&Params) -> ModelCard`: every parameter with its `Source` — `Paper`,
     `Standard`, `Code` (another tool's source, read only), or `TodoCalibrate` **with** a
     calibration plan; mark anything not re-read against the primary as secondary in the
     source's note;
   * a test that the model stops behind a standing obstacle without collision
     (`testkit::run_follow`), plus one that pins a hand computation.
2. **Register it**: add `pub mod <name>;` to `carfollowing/mod.rs` and one `CfFactory` entry
   to `library::FACTORIES` (kept in id order — a test checks). That entry is what makes it
   selectable in a scenario, listed in the manifest and the model registry, and built by the
   engine. Nothing in the engine changes.
3. **Document it**: a row in the table above and in `carfollowing/mod.rs`.
4. **Measure it**: `platoon::run` with `CfChoice::of("<name>")` works for any model, and
   `fd.rs`'s fundamental-diagram validation is the place for a capacity check.

## How to add another lateral model

Implement `lateral::LateralModel` (`decide` returns the offset to steer for and the lateral
speed; `filter` returns the pair rule the leader search uses), add a `LateralKind` variant
and a line in `LateralSpec::build`. The engine calls the trait only: it passes the
same-lane neighbours from the frozen snapshot (`lateral::decide_in_lane`), moves the
vehicle at the decided lateral speed when no lane change is in progress, and gives the
neighbour query the filter so a vehicle the ego may pass is not its leader.

## Determinism

* No model reads a clock; discrete models read the step from `begin_step`, written before
  any `accel` of the step.
* Per-step noise (Krauss dawdling, W99 thresholds, the class-share draw) is a ChaCha stream
  keyed by `(master seed, model plug-in domain, actor)` positioned at the step index: a pure
  function of `(seed, model, actor, step)`, unaffected by thread count or by how many other
  actors drew, and identical for a hypothetical evaluation (MOBIL) and the real one.
* The cooperative link is written only between mobility steps, by one writer, in node-id
  order, and read only during the mobility step; its maps are `BTreeMap`s.
