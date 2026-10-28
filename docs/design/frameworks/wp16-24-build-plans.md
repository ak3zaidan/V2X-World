# Build plans for WP-16 to WP-24

Status: prep, 2026-10-27. This is read-only on code: nothing was built or run. It turns
§2 of `00-adoption-plan.md` (waves 6-8) into work a build agent can start on straight
away. Each package lists what the code is today (`file:line` at `4aa798bb`), the decision
to build against, the exact files and functions to change, its settings, its tests (each
with an injected fault), its expectation rows, which goldens move, and pieces of about
three agent-hours each.

How to read the citations:
- **[cached]**: the source text is in the repo or its cache.
- **[repo]**: the value is already cited in a repo document, which is named.
- **[verify]**: the source is real and named, but its text is not cached. The piece that
  uses the value caches the source first and pins the clause and page. Until then, no
  number from it goes on a card.

No clause below was guessed. Where the clause is not known, only the document is named.

## 0. Rules shared by every piece

1. **Load.** At most 3 build agents at a time, each in its own worktree with the
   per-worktree cargo config, and one heavy cargo command at a time through `heavy.sh`
   (memory: lean agent builds, parallel Rust agents on 8 GB). A piece is sized so that it
   needs at most two heavy commands: one crate test, and one engine test file. A piece
   that needs a city run uses the 450 MB/one-thread `runtiny.sh` wrapper.
2. **Checks that can fail.** Every new test or held figure is shown red under its named
   fault before it counts. Apply the fault as a temporary edit, run, revert, and list it
   in the commit message as `fault: <what> -> <test> red`.
3. **Defaults.** New behaviour is off by default and the default path stays
   byte-identical (rule 4 of the adoption plan). The one exception is WP-23, where
   flattening is a stated defect (see that section). Each package ends by proposing the
   realistic default to the owner. It never flips the default itself.
4. **Settings checklist.** Every new key needs all of these, or the publish tests fail.
   This is the WP-05 lesson: `publish::every_number_publishes_a_unit`.
   - the field and its serde default in `crates/v2xw-engine/src/scenario/schema.rs`;
   - its bounds and choices in `scenario/validate.rs`;
   - its label and choices in `scenario/options.rs`;
   - its unit in `scenario/publish.rs`;
   - a row in `scenario/guide.rs` where the key is user-facing;
   - a line in `docs/guides/configuration-reference.md`.
5. **Determinism.** A new draw uses a keyed hash, in the pattern of
   `v2xw-mobility/src/carfollowing/common.rs:137` `keyed_uniform`. It does not take a new
   position in a shared stream. Prefer that to adding a `RngDomain` variant
   (`v2xw-core/src/rng.rs:142`).
6. **Belief only.** A detector or driver reaction triggered by V2X reads only what the
   unit verified (invariants I-T1 to I-T3). A scenario director (WP-21) and a physical
   law (WP-18, WP-22) may read ground truth. Their cards say so.
7. **Goldens** referred to below:
   - **G-SCMS**: the SCMS record digests in `crates/v2xw-engine/tests/regression/*.json`,
     re-blessed with `V2XW_BLESS_SCMS_DIGESTS=1`;
   - **G-ML**: the frozen ML corpus digest and the `detnorm_*` columns;
   - **G-LEG**: the legacy attack catalogue;
   - **G-TV**: the held `v2xw-trafficval` checks (`calibration/*`, `invariants/*`,
     `cities/*`, `safety/*`);
   - **G-SENS**: the sensitivity expectation tables in
     `crates/v2xw-engine/tests/expectations/*.yaml`. Rows have the form
     `{setting, change, metric, expect, min_change, source}` (see the head of
     `catalogue.yaml`). A metric is a fingerprint name (`traffic.*`, `prr`,
     `cbr_mean`, …) or any numeric leaf of `RunReport`, written `report.*`
     (`sensitivity/sweep.rs:17`). A row below marked "new RunReport leaf" needs that
     field added by the named piece. WP-16's `report.mac_guard_deferrals` is one, added
     in P16.3.

### 0.1 Shared-file map (merge planning)

| File | 16 | 17 | 18 | 19 | 20 | 21 | 22 | 23 | 24 |
|---|---|---|---|---|---|---|---|---|---|
| `v2xw-radio/src/mac.rs` | **W** | | | | | | | | |
| `v2xw-net/src/stack/wave.rs` | **W** | | | w | | | | | |
| `v2xw-engine/src/run.rs` | **W** (MAC driver, `tuned`) | | | w (relay) | w (EV command) | **W** (`on_mobility_step`, `on_control`) | | | |
| `v2xw-engine/src/run/channels.rs` | **W** | | | r | | | | | |
| `v2xw-engine/src/actuated.rs`, new `nema.rs` | | **W** | | | | | | r | |
| `v2xw-engine/src/infra.rs`, `priority.rs` | | **W** | | | | | | | |
| `v2xw-world/src/model.rs` | | w (`SignalPlan`) | w (vegetation) | | | | | w (programme table) | |
| `v2xw-world/src/osm.rs` | | | **W** | | | | | | |
| `v2xw-world/src/sumo.rs` | | | | | | | | **W** | r |
| `v2xw-radio/src/obstacle.rs`, `prop.rs` | | | **W** | | | | | | |
| `v2xw-radio/src/ip_transport.rs`, `engine/src/backend.rs` | | | | **W** | | | | | |
| `v2xw-msg` (WSA, BSM Part II, CAM) | | | | **W** (new WSA) | **W** (`j2735/bsm.rs`, `cam.rs`) | | | | |
| `v2xw-mobility/src/engine.rs` | | | | | **W** | | **W** | | |
| `v2xw-mobility/src/rules.rs`, `lateral/` | | | | | **W** | | | | |
| `v2xw-mobility/src/classes.rs` | | | | | | | **W** | | |
| `v2xw-engine/src/timeline.rs` | | | | | | **W** | | | |
| `v2xw-interop/src/xval.rs`, `traci.rs` | | | | | | | | **W** | |
| scenario `schema/validate/options/publish` | w | w | w | w | w | **W** | w | w | w |

**W** marks heavy edits and w light ones. Every package appends its own block to the
scenario files, and the integrator merges those blocks; that has worked for waves 1-5.
Two pairs edit the same functions and must not run in the same round:
- WP-20 and WP-22: `MobilityEngine::step`, pass 6.
- WP-16 and WP-21: `run.rs`.

### 0.2 Dependencies and schedule (pieces, 3 agents per round)

```
WP-13 (done) ─► WP-16 ─► WP-19
WP-17 ─► WP-23
WP-08 (merged, its own tests red) ─► WP-20 (red-light tag reuse)
WP-03 (done), WP-10 (done), WP-07 (BLOCKED: owner decision) ─► WP-24 (P24.4-P24.5 only)
WP-05 (merged) ─► WP-18 (osm.rs sequencing)
WP-11 (done) ─► WP-21 (TTC from v2xw-metrics::ssm)
```

| Round | Agent A | Agent B | Agent C |
|---|---|---|---|
| R1 | P16.1 | P17.1 (sources, no cargo) | P22.1 (sources and table) |
| R2 | P16.2 | P17.2 | P22.2 |
| R3 | P16.3 | P17.3 | P22.3 |
| R4 | P16.4 | P17.4 | P18.1 (sources, no cargo) |
| R5 | P21.1 | P17.5 | P18.2 |
| R6 | P21.2 | P17.6 | P18.3 |
| R7 | P21.3 | P17.7 | P18.4 |
| R8 | P19.1 | P17.8 | P18.5 |
| R9 | P19.2 | P17.9 | P20.1 |
| R10 | P19.3 | P17.10 | P20.2 |
| R11 | P19.4 | P23.1 | P20.3 |
| R12 | P19.5 | P23.2 | P20.4 |
| R13 | P24.1 | P23.3 | P20.5 |
| R14 | P24.2 | P24.3 | (WP-07 if unblocked) |

Source-only pieces (P17.1, P18.1, P22.1, P19.1's cache step, P24.1) run no cargo. They
can share a round with a build piece without load. The owner asked for more agents, so
these are where the extra agents go: one more source-only agent per round adds no build
load.

---

## WP-16 IEEE 1609.4 alternating access in the MAC

**Today.**
- The 1609.4 schedule is `ChannelSchedule` in `v2xw-net/src/stack/wave.rs:51-108`, with
  `interval`, `next_opportunity_ns`, `access_delay_ns` and `usable_fraction`. The
  constants are at `:18-25`: sync 100 ms, CCH 50 ms, guard 4 ms, `CCH_CHANNEL` 178.
- It is used in one place, as an application-layer jitter: `run.rs:5410-5417` holds
  every frame of an `alternating` run to the next CCH interval.
- There is no SCH queue and no check that a frame fits before the guard.
- `v2xw-radio` does not depend on `v2xw-net` (`Cargo.toml`), so `mac.rs` cannot call
  `wave.rs` today.
- The EDCA MAC keeps its state per `(node, channel)` already: `EdcaOcbMac.nodes:
  BTreeMap<(u32,u16), NodeState>` (`mac.rs:307-316`), with four AC queues
  (`NodeState`, `:262-285`). Per-channel EDCA queues therefore exist. What is missing is
  the gate.
- Grants go through `next_poll_at` (`:661`) and `poll` (`:673-772`), driven by the
  engine in `run.rs:6459` `schedule_mac_timer` and `:6466` `on_mac_timer`, reading
  `:6774`.
- The MAC card says "No IEEE 1609.4 channel switching" (`mac.rs:980`).
- Reception: `run.rs:9710` `tuned(node, ch)` and its callers at `:5487` (tx drop) and
  `:7154` (`untuned_arrivals`) are time-independent.

**Decision.**
1. **Move the schedule.** `ChannelSchedule` moves into `v2xw-radio` as the new module
   `v2xw-radio/src/wave_schedule.rs`. `wave.rs` re-exports it under the same names, so
   no caller changes.
2. **Gate in the EDCA MAC.** The gate lives in `EdcaOcbMac` as an optional
   `AlternatingGate { sched, cch: ChannelId, sch: ChannelId }`.
3. **What a grant must satisfy.** A grant at `due` on channel `ch` needs both:
   - (a) `sched.interval(due)` is the interval of `ch`. It is never `Guard`.
   - (b) `due + air_time(sdu)` is at or before the end of that interval.

   This is Veins' `timeLeftInSlot` rule (behaviour reference only).
4. **When the gate fails.** The backoff is frozen, as on a busy medium, until the next
   start of that channel's interval. AIFS is then re-run from that instant. A frame is
   never dropped for this.
5. **Which channels pair up.** The CCH interval belongs to the run's primary channel
   (`ChannelPlan.primary`) and the SCH interval to `ChannelPlan.service` (WP-13).
6. **Who alternates.** Only units with one radio:
   - A two-radio unit runs radio 1 continuously on the primary and radio 2 continuously
     on the service channel.
   - A unit that alternates is tuned to the primary only in CCH intervals and to the
     service channel only in SCH intervals. `tuned` becomes `tuned_at(node, ch, t)`.
7. **The medium tier** (`SlottedMac`, `mac.rs:1034`) refuses `alternating` in
   validation in this package. Gating it is a follow-up.
8. **Status of the numbers.** The 100/50/4 ms constants are UNVERIFIED at the primary
   level (`04-models.md:668`, R1 §F, `11-open-questions.md:47`). The card keeps that
   status. Purchasing IEEE 1609.4-2016 is the open action.

**Files and functions.**
- New `v2xw-radio/src/wave_schedule.rs`: `ChannelSchedule`, plus `interval_end_ns(t)`
  and `fits(t, air_ns, cch: bool) -> Result<(), u64 /*next start*/>`.
- `v2xw-net/src/stack/wave.rs`: `pub use v2xw_radio::wave_schedule::*`. The existing
  tests stay.
- `v2xw-radio/src/mac.rs`:
  - `EdcaOcbMac::with_alternating(sched, cch, sch)`;
  - the gate in `next_poll_at` (`:661`) and `poll` (`:673`);
  - `NodeState` gains `gate_deferrals` and `guard_deferrals`;
  - the card limitation at `:980` is rewritten.
- `v2xw-engine/src/run.rs`:
  - delete the deferral at `:5410-5417`;
  - build the MAC with the gate (wiring, where the EDCA MAC is constructed);
  - `tuned` (`:9710`) becomes `tuned_at`, at the `:5487` and `:7154` call sites;
  - report counters at `:549` and nearby.
- `v2xw-engine/src/run/channels.rs:60` `tuned`: add the time argument for single-radio
  classes when the access is alternating.
- `scenario/validate.rs:1960`, `:6965` (`net.channel_access`):
  - `alternating` with a channel map and one radio is now meaningful;
  - refuse `alternating` together with the `medium` tier.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `net.channel_access` (exists) | `continuous` | choice | IEEE 1609.4-2016 §6.2.5 [repo: `wave.rs:4`; constants UNVERIFIED] |
| report `mac_gate_deferrals`, `mac_guard_deferrals`, `straddling_frames` | n/a | count | invariant: `straddling_frames` must be 0 |

There are no new knobs. The interval lengths stay constants until the standard is
cached.

**Tests (Accept → test → fault).**

| Accept | Test | Injected fault |
|---|---|---|
| No frame straddles a guard | `mac::tests::alternating_gate_never_grants_into_a_guard`: 10 000 random enqueue instants and sizes; every grant has `interval(at) != Guard` and `at + air <= interval_end` | remove check (b): red with a straddling grant |
| The same in a dense run | `tests/alternating_mac.rs::dense_run_has_no_straddling_frame`: 120 s, 80 vehicles, 1 200 m grid (the plan's 10 minutes, scaled to stay inside a piece), `report.straddling_frames == 0`, recomputed from the tx records | same fault: red |
| Continuous mode unchanged | `mac::tests::continuous_gate_is_the_ungated_mac`: identical grant sequence with and without a `Continuous` gate; plus `bsm_timing` and `phase2` unchanged (G-SCMS) | apply the gate in continuous mode: red |
| SCH frames queue across the CCH interval | `mac::tests::sch_frame_waits_for_the_sch_interval`: enqueued at 10 ms, granted at ≥ 54 ms + AIFS; the queue keeps FIFO order | key the gate on the wrong channel: red |
| Single-radio receivers hear the SCH only in the SCH interval | `tests/alternating_mac.rs::single_radio_hears_spat_only_in_sch`: RSU with 2 radios sends SPaT continuously on the service channel; 1-radio OBU PDR within ±0.05 of 0.46 (`usable_fraction`) | `tuned_at` ignores the time: PDR ≈ 1, red |
| BSMs held to the CCH (existing) | `tests/geonet.rs:71` keeps passing through the MAC path | n/a |

**Expectation rows** (`expectations/networking.yaml`; family `test-networking-dsrc`):
```yaml
- {setting: net.channel_access, change: alternating, metric: e2e_ms_mean, expect: up,
   source: "a message generated in the SCH interval waits for the next CCH interval (IEEE 1609.4-2016 6.2.5; constants UNVERIFIED)"}   # exists
- {setting: net.channel_access, change: alternating, metric: report.mac_guard_deferrals, expect: up,
   source: "no transmission in the guard interval, and a frame that would cross the interval end waits (IEEE 1609.4-2016 6.2.5; Veins Mac1609_4 behaviour)"}
- {setting: net.channel_access, change: alternating, metric: prr, expect: down,
   source: "frames held over the SCH interval contend together at the next CCH start (Eichler, IEEE VTC 2007-Fall [verify])"}
```

**Goldens.** None on the default path. Runs with `alternating` change, because the frame
timing moves from jitter to MAC deferral: `scenarios/sensitivity/test-networking-dsrc.yaml`
and the `geonet.rs` alternating test. List them in the commit.

**Pieces.**
- **P16.1** (3 h): move the schedule into `v2xw-radio`, re-export it, add
  `interval_end_ns` and `fits`, with unit tests. No behaviour change. Heavy commands:
  `cargo test -p v2xw-radio wave_schedule`, `-p v2xw-net stack::wave`.
- **P16.2** (3 h): the MAC gate, its counters, the three `mac::tests`, the card text,
  and the faults.
- **P16.3** (3 h):
  - engine wiring;
  - remove the app-layer deferral;
  - `tuned_at`;
  - validation (medium tier refused);
  - report fields and units.
- **P16.4** (3 h): the `tests/alternating_mac.rs` pair, the expectation rows, and a
  status note in the adoption plan.

---

## WP-17 NEMA dual-ring 8-phase controller (size L, split into 10 pieces)

**Today.**
- `v2xw-engine/src/actuated.rs` (647 lines) is a **single-ring** NEMA TS 2 basic actuated
  controller (`:12-25`). It works by moving the plan's clock:
  - `step_with` (`:285-372`) shifts `SignalPlan.offset_s` to gap out, extend, max out or
    rest;
  - the plan is a clock, `SignalPlan::phase_at` (`v2xw-world/src/model.rs:2568`), and a
    phase is a full state vector over `controlled` movements (`SignalPhase`,
    `model.rs:2465`).
- The plan is read in 10 source files besides `model.rs` itself: `infra.rs` (SPaT), `priority.rs`, `app_truth.rs`,
  `actuated.rs`, mobility `engine.rs`, `audit.rs`, `calibration.rs`, `meso`,
  `signal_fixed_time.rs`, and world `walk.rs`.
- SPaT `minEndTime`/`maxEndTime` come from `infra.rs:287-346` `spat_with` plus the
  priority `Outlook`. Without one they equal the fixed plan's change times (CTI 4501
  guidance, as `infra.rs:289` cites).
- The settings are global: `traffic.signals` → `SignalControl` (`schema.rs:440-519`),
  with `SignalControlKind::{FixedTime, Actuated}` (`:405-413`). The defaults cite the
  2008 manual, FHWA-HOP-08-024 (`schema.rs:491-495`).

**Decision.**
1. **The plan stays a clock, by re-projection.** A new module `v2xw-engine/src/nema.rs`
   holds the dual-ring state.
2. **Each step**, it rewrites the junction's `SignalPlan.phases` into the *composite
   interval sequence* that runs from now:
   - the current composite interval, with its remaining time;
   - then the predicted intervals under the current calls, each ring at its max;
   - `cycle_s` set to their sum, and `offset_s` set so that `phase_at(now)` lands in
     composite 0.
3. **Why re-projection.** All of them keep calling `phase_at` and `states_at`
   unchanged, and "the SPaT is the signal by construction" (`actuated.rs:5-10`) still
   holds. A live-state override would change every consumer.
4. **The SPaT bounds** come from the controller's outlook:
   - `minEndTime` = the earliest gap-out or barrier time;
   - `maxEndTime` = the max-out or force-off time;
   - `likelyTime` = the current projection.

   `ActuatedOutlook` (`actuated.rs:94`) generalises to a per-group outlook.
5. **`World::validate`** (`model.rs:3995`: durations sum to `cycle_s`, offset in range)
   must hold after every rewrite. Assert it in debug builds.
6. **The content hash.** `hash::content_hash` (`v2xw-world/src/hash.rs:236`) covers
   signal plans (item 9). It is computed once, at load. The single-ring controller's
   offset moves already rely on that, and the rewrite must too: never re-hash at run
   time. Check that the UI's world payload (`serde_vwp`) is served from the load-time
   plan and that the lamps come from live `phase_at`.
7. **Phase numbering.** NEMA numbers come from the geometry:
   - ring 1 = {1,2,3,4} and ring 2 = {5,6,7,8};
   - barrier between {1,2,5,6} and {3,4,7,8};
   - 2/6 are main-street throughs, 1/5 main-street lefts, 4/8 side throughs, 3/7 side
     lefts;
   - pedestrian phases go with the parallel through.

   The source is the FHWA Signal Timing Manual 2nd ed. (NCHRP Report 812, 2015), its
   dual-ring diagram [verify: pin the figure in P17.1]. Junctions that do not map (more
   than 4 legs, offset T) fall back to the single-ring actuated controller and are
   counted.
8. **Sources.** Re-implement from the STM 2nd ed. and NEMA TS 2-2016 [verify; paywalled,
   so the STM is the working text]. SUMO's `NEMAController.cpp` is never read for code.
   Only SUMO's documented parameter names are used, for WP-23 import parity.

**Files and functions.**
- New `v2xw-engine/src/nema.rs`:
  - `NemaPhase { number, movements, min_green_s, max_green_s, passage_s, yellow_s, red_clear_s, walk_s, ped_clear_s }`;
  - `Ring`, `RingState { phase, interval: Green|Yellow|RedClear|Walk|PedClear, held_s, last_call_s }`;
  - `NemaController::step(&mut self, world, calls, now_s, dt_s)`;
  - `project(&self, plan: &mut SignalPlan, now_s)`;
  - `outlook(group)`.
- New `v2xw-engine/src/nema/assign.rs`: `assign_phases(world, plan) -> Result<Assignment, Unmapped>`.
- `actuated.rs`: `ActuatedControllers::new` (`:138`) routes plans to `NemaController` when
  `control: nema`. Detector call sets `calls` and `extension_calls` (`:232`, `:253`) are
  shared.
- `infra.rs:296` `spat_with`: take the NEMA outlook, and map protected, permissive and FYA
  states to J2735 `MovementPhaseState`.
- `priority.rs:213` `step`: on a NEMA plan, request an early green or an extension
  through the ring controller instead of moving the clock (P17.9).
- `schema.rs:405` `SignalControlKind::Nema`, plus a `SignalControl.nema: Option<NemaSettings>`
  block. Also `validate.rs`, `options.rs`, `publish.rs`.
- `v2xw-world/src/model.rs`: no struct change is needed. The projection writes the
  existing fields.

**Settings** (`traffic.signals.*`; SUMO's NEMA names in brackets for WP-23 [verify against
the SUMO "NEMA" documentation page, behaviour reference only]).

| Key | Default | Unit | Source |
|---|---|---|---|
| `control` | `fixed-time` (adds `nema`) | choice | n/a |
| `nema.ring1` [`ring1`] | `[1,2,3,4]` | phase list | STM 2nd ed., dual-ring diagram [verify] |
| `nema.ring2` [`ring2`] | `[5,6,7,8]` | phase list | same |
| `nema.barrier_phases` [`barrierPhases`] | `[2,6]` | pair | same |
| `nema.barrier2_phases` [`barrier2Phases`] | `[4,8]` | pair | same |
| `nema.lefts` | `lead-lead` (`lead-lead`, `lag-lag`, `lead-lag`) | choice | STM 2nd ed., left-turn phasing [verify] |
| `nema.left_turn_mode` | `protected-permissive` (`protected`, `permissive`) | choice | STM 2nd ed. [verify] |
| `nema.permissive_display` | `fya` (`green-ball`) | choice | MUTCD 2009 ch. 4D, left-turn signal faces [verify] |
| `nema.min_green_s`, `max_green_s`, `passage_s` per phase class (`through`, `left`) | through: the existing 7 / 45 / 2.5; left: pinned in P17.1 | s | STM 2nd ed. timing tables [verify]; the existing defaults cite FHWA-HOP-08-024 [repo `schema.rs:491`] |
| `nema.yellow_s`, `red_clear_s` | from the plan's own amber and all-red phases (as today) | s | ITE change-interval formula Y = t + v/(2a + 2Gg), R = (W+L)/v [verify: ITE 2020 guideline] |
| `nema.walk_s`, `ped_speed_mps` | 7 s; 1.07 m/s (3.5 ft/s) | s, m/s | MUTCD 2009 §4E.06 [verify] |
| `nema.min_recall` [`minRecall`] | `[2,6]` | phases | STM 2nd ed. [verify] |
| `nema.max_recall` [`maxRecall`], `nema.ped_recall` | `[]` | phases | n/a |
| `nema.coordinate_phases` [`coordinatePhases`] | `[2,6]` | phases | STM 2nd ed., coordination [verify] |
| `nema.coordination` [`coordinate-mode`] | `off` (`on`) | choice | n/a |
| `nema.force_off` [`fixForceOff`] | `floating` (`fixed`) | choice | STM 2nd ed. [verify] |
| `nema.controller_type` [`controllerType`] | `ts2` (`ts1`) | choice | recorded only; same behaviour here, as the card states |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Two concurrent throughs gap out independently | `nema::tests::concurrent_throughs_gap_out_independently`: calls continue on 2 and stop on 6; lagging 5 is called; 6 ends and 5 is served while 2 continues | one shared ring timer: 2 and 6 end together, red |
| Ring pointers meet at each barrier | `nema::tests::rings_cross_barriers_together`: 1 h of random calls under 5 seeds; no instant has ring 1 in {1,2} and ring 2 in {7,8}; both clearances end on the same instant at every crossing | skip the barrier wait: red |
| No conflicting greens | `nema::tests::no_conflicting_greens` against the junction's `ConflictMatrix`, over random calls on generated 4-leg and T junctions | swap the 3/7 assignment: red |
| FHWA 8-phase example reproduces | `nema::tests::stm_example_timing`: max recall on every phase gives the example's cycle and splits exactly; the example is identified in P17.1 | drop the yellow from the split: red |
| SPaT stays consistent with the controller | `tests/nema_spat.rs`: every published group change happened inside [`minEndTime`, `maxEndTime`] (100 %), and `likelyTime` is within 1 step of the projected change while calls are unchanged | publish `max` as `min`: red |
| Single-ring scenarios unchanged | the existing actuated tests and `phase2`/`bsm_timing` digests (G-SCMS) unchanged with `control: actuated` | n/a |
| Pedestrians served | `nema::tests::ped_recall_serves_walk`: walk ≥ 7 s and clearance = crossing length / 1.07 m/s every cycle | no ped recall: walk missing, red |
| Coordination | `nema::tests::coordinated_phases_hold_their_offset`: with `coordination: on`, the coordinated green ends at the yield point mod cycle ±1 step every cycle | force-offs ignored: drift, red |
| FYA and permissive in SPaT | `infra::tests::fya_maps_to_permissive_movement_allowed` | swap the protected and permissive mapping: red |
| Defects stay visible | `invariants/nema-safety` (new G-TV check): 600 s Manhattan with `control: nema`, 0 conflicting greens and 0 auditor `red-entry` defects that are not tagged violations | assignment fault: red |

**Expectation rows** (`expectations/traffic.yaml`, family `traffic-grid`):
```yaml
- {setting: traffic.signals.control, change: nema, metric: traffic.stopped_share, expect: down,
   source: "concurrent phases serve both directions' throughs and lefts in one barrier group instead of in sequence (FHWA STM 2nd ed., NCHRP 812 [verify chapter])"}
- {setting: traffic.signals.nema.max_recall, change: "[1,2,3,4,5,6,7,8]", metric: report.signal_gap_outs, expect: down,   # new RunReport leaf (P17.10)
   source: "max recall holds every phase to its maximum, so none gaps out (FHWA STM 2nd ed. [verify])"}
```

**Goldens.** None by default. A later owner decision to make `nema` the US default moves
G-TV (`calibration/saturation-flow`, `cities/*`) and every SPaT-carrying digest. That is
out of scope here.

**Pieces.**
- **P17.1** (3 h, no cargo): cache the sources and write
  `docs/design/signals/nema-constants.md`:
  - the STM 2nd ed. PDF (public, TRB), MUTCD 2009 ch. 4D/4E (public, FHWA), and the
    SUMO NEMA documentation page;
  - in the constants file, every default above with its page;
  - the 8-phase example the test reproduces;
  - the left-turn min and max green.
  The ITE guideline is paywalled: record the formula's secondary source if the primary
  cannot be cached.

  **Sources resolved, 2026-10-27 (sources track, no code; `l.N` = this file at commit 081fcab2).** STM = NCHRP Report 812, *Signal
  Timing Manual* 2nd ed. (2015), public PDF at
  <https://transops.s3.amazonaws.com/uploaded_files/Signal%20Timing%20Manual%20812.pdf>
  (sha256 `87d89878dd35…25cc5`, read on 2026-10-27 and not cached, since it is 16 MB).
  Pages are the printed ones.
  - *Dual ring (l.303, `ring1` row).* STM §5.1.2, **Exhibit 5-3** "Basic Ring-and-Barrier
    Diagram", p. 5-4. It has two rings, two barriers and eight phases, with 1, 2, 5, 6
    compatible and 3, 4, 7, 8 compatible. Numbering is from §5.1.1, Exhibit 5-1, pp. 5-2–5-3.
    2/6 are the major throughs, 4/8 the minor throughs, 1/5 the major lefts and 3/7 the
    minor lefts. "Pedestrian phases … are generally assigned the same phase number as the
    adjacent, parallel vehicular phases." Verified.
  - *SUMO names (l.330).* Checked against <https://sumo.dlr.de/docs/Simulation/NEMA.html>
    (last edited 2026-05-04). These exist: `ring1`, `ring2`, `barrierPhases`,
    `barrier2Phases`, `coordinate-mode`, `minRecall`, `maxRecall`, `fixForceOff`,
    `controllerType`, `latchingDetectors`, `crossPhaseSwitching:X`. **`coordinatePhases`
    does not exist.** In SUMO the coordinated phases are `barrier2Phases` with
    `coordinate-mode=true`, so the `nema.coordinate_phases` bracket must change for WP-23.
    SUMO's own defaults: `minRecall` is all phases when unset, and `fixForceOff=false`
    (floating).
  - *`lefts` lead-lead.* STM §5.1.3.6, p. 5-9: "The most common left-turn phase sequence
    is the lead-lead sequence". Verified.
  - *`left_turn_mode`.* STM §5.1.3 and Exhibit 5-4, p. 5-5 list five options. The STM picks
    no default, so `protected-permissive` stays an ASSUMPTION choice.
  - *`permissive_display` fya.* MUTCD 2009 §4D.20 ¶03 item 4: "During the permissive
    left-turn movement, a flashing left-turn YELLOW ARROW signal indication shall be
    displayed". The permissive-only case is §4D.18 ¶03, and STM §4.3.1.3 is p. 4-19.
    Verified. Note that the MUTCD 11th ed. (Dec 2023) supersedes 2009. It was not checked.
  - *Min / max / passage (l.343).* Minimum green is **Exhibit 6-4**, p. 6-6: through on a
    major arterial >40 mph 10–15 s, ≤40 mph 7–15 s; minor arterial 4–10 s;
    collector/local 2–10 s; **left turn, any facility, 2–5 s**. Maximum green is
    **Exhibit 6-9**, p. 6-10: through >40 mph 50–70 s, ≤40 mph 40–60 s, minor arterial
    30–50 s, collector 20–40 s; **left turn 15–30 s**. Passage is §6.1.5 with Exhibit
    6-12, p. 6-14 (speed × zone length), and p. 6-12: "Setback detectors typically have
    passage times between 2 and 3 seconds". The existing through 7 / 45 / 2.5 sits inside
    the ≤40 mph ranges. Proposed left defaults are **4 / 25 / 2.5 s**. They lie inside the
    STM ranges, but the exact values are an ASSUMPTION.
  - *Yellow / red (l.344).* STM Equation 6-1, p. 6-3, cites ITE: Y = t + 1.47v/(2a + 2Gg)
    (v in mph), with t = 1.0 s and a = 10 ft/s² ("recommended … in NCHRP 731"). Exhibit 6-2,
    p. 6-4: 25→3.0, 30→3.2, 35→3.6, 40→3.9, 45→4.3, 50→4.7, 55→5.0, 60→5.4 s, plus 0.1 s per
    1 % downgrade. Red clearance is Exhibit 6-3, p. 6-5, a speed × width table "reduced by 1
    second" per NCHRP 731. Back-calculation fits R = (W + 20 ft)/(1.47v) − 1, e.g. 30 mph
    and 50 ft give 0.6, and 25 mph and 30 ft give 0.4. The ITE 2020 primary stays paywalled,
    so the secondary is STM Eq. 6-1 / Exh. 6-3.
  - *Walk / ped speed (l.345).* MUTCD 2009 §4E.06: ¶11 says walk "should be at least 7
    seconds" (Guidance), ¶12 allows 4 s (Option), ¶07 sets clearance at 3.5 ft/s, and ¶14
    checks walk plus clearance at 3 ft/s. STM pp. 6-16–6-17 agree. Verified.
  - *`min_recall` [2,6] (l.346): a mismatch.* STM **Exhibit 6-22**, p. 6-22, "Typical
    Settings … at a Fully-Actuated Intersection", gives Minimum Recall *None* and **Soft
    Recall on 2 and 6**. So the default as written does not match the STM. Either add
    `nema.soft_recall: [2,6]` with `min_recall: []`, or keep `[2,6]` labelled as a
    deviation. P17.2 decides.
  - *Coordinate phases (l.348).* STM §5.1.1, p. 5-3, says the manual follows the convention
    that phases 2 and 6 "are the coordinated phases when under coordinated operations".
    Verified.
  - *Force-off (l.350).* STM §7.3.4 "Force-Offs" (guidance in §7.4.4), p. 7-11, and Exhibit 7-14, define fixed and
    floating force-offs and name no default. "Floating" rests on SUMO's default and the
    STM's statement that floating favours the coordinated phases. That is secondary, so the
    entry stays labelled.
  - *Expectation sources (l.371, l.373).* STM §5.1.2, p. 5-4, covers compatible phases
    timing together. Max recall is STM Exhibit 6-21, p. 6-22. SUMO's docs say `maxRecall`
    phases "run for their maxDur".
  - **Not resolved.** NEMA TS 2-2016 (paywalled). The ITE 2020 guideline (paywalled). The
    **8-phase worked example** for `stm_example_timing`: the STM has no numeric
    max-recall worked example, only diagrams (Exhibits 5-5 to 5-26, 7-12 Splits, p. 7-11).
    The test must build its own example from Exhibits 6-2/6-4/6-9 and state that it does.
- **P17.2** (3 h): the schema, validation, options, publish and guide rows for
  `control: nema` and the `nema.*` block, with no behaviour (validation refuses to run
  `nema` until P17.5 lands).

  **Done 2026-10-28 (P17.2).** Decisions where the table above was open or wrong, each
  checked against its source:
  - *Barriers follow SUMO's documented names.* SUMO's NEMA page says `barrierPhases` is
    "usually 4,8" and `barrier2Phases` "usually 2,6" (and `barrier2Phases` are the
    coordinated phases in coordinate mode). The table had the two reversed, so the defaults
    are `barrier_phases: [4,8]` and `barrier2_phases: [2,6]`.
  - *Recall follows STM Exhibit 6-22* (p. 6-22): `min_recall: []` and a new
    `soft_recall: [2,6]`. SUMO's own `minRecall` default (every phase) is not taken.
  - *`controller_type` is `ts2` or `type170`*, as SUMO documents it. There is no `ts1`.
    The two differ only in the reference point of the coordination offset (start of green,
    or start of yellow), so the key is refused unless `coordination` is on, and P17.8 acts
    on it.
  - *`coordinate_phases` has no SUMO name.* WP-23 maps SUMO's `barrier2Phases` onto it
    when `coordinate-mode` is true.
  - *Through timing is the existing `traffic.signals.min_green_s`, `max_green_s` and
    `passage_s`.* The left-turn phases get `nema.left_min_green_s`, `left_max_green_s` and
    `left_passage_s` (4 / 25 / 2.5). No setting is duplicated.
  - *Absent means "from somewhere else".* With `yellow_s` and `red_clear_s` absent, each
    plan's own amber and all-red apply. With `walk_s` and `ped_speed_mps` absent,
    `world.signals.pedestrian_*` applies, else MUTCD 7 s and 1.067 m/s. Bounds are MUTCD
    2009 §4D.26 (yellow 3-6 s, red at most 6 s) and §4E.06 (walk 4-30 s, speed 0.8-1.5 m/s).
  - *Rings are written in ascending order.* Ring 1 must come from phases 1-4 and ring 2
    from 5-8. Lead or lag comes only from `lefts`, so a scenario cannot state the sequence
    twice. A barrier is a ring-1 phase then a ring-2 phase from that barrier's group.
    `ped_recall` names throughs only (STM §5.1.1). `coordinate_phases` is one phase, or a
    concurrent pair.
  - *Dead settings are refused*, as the codebase does elsewhere: coordination-only keys
    without `coordination`, left-phase timing under `permissive`, and the permissive face
    under `protected`. A `nema` block under another `control` is refused too.
  Status `traffic.signals.nema`: Refused, until P17.5.
- **P17.3** (3 h): `assign_phases` from junction geometry, with unit tests on generated
  4-leg, T and one-way junctions, and the fallback count. Main street = the higher road
  class, then more lanes.

  **Done 2026-10-28 (P17.3).** `v2xw-engine/src/nema/assign.rs`: `assign_phases(world,
  plan)`, its pure core `assign_approaches`, `assign_all` with
  `AssignReport::fallback_counts`, and `Assignment::concurrent_foes`. The STM leaves
  these open; the choices are:
  - Two approaches are one street when their headings are opposite within 45°. At most
    two streets, at least 45° apart. Otherwise the junction is `Unmapped`:
    `too-many-approaches` (more than 4), `not-two-streets` (a Y or a skewed offset),
    `unknown-movement`, or `skewed-crossing`.
  - Main street ties after class and lanes go to the street with more green in the
    junction's own plan, then to the lower edge id.
  - Through 2 (and 4) is the approach heading most nearly north-east,
    `cos(h − 45°)`. Eastbound and northbound are 2 and 4 on a compass grid.
  - A left whose approach has no opposing through rides its own through (one-way
    streets, the stem of a T). Otherwise it takes the number beside the opposing through
    (5 beside 2, 1 beside 6, 7 beside 4, 3 beside 8) and records that through as
    `permissive_with`.
  - A crosswalk takes the number of the parallel through on whose right-hand side it
    lies (STM §5.1.1). A crosswalk across a single street gets a pedestrian-only 4 or 8.
  - The generated conflict matrices record opposing lefts as a level conflict, so
    `concurrent_foes` flags 1+5 and 3+7 as `opposing_lefts`. The tests allow that pair
    only.
  - Open, for P17.6: a left from a lane it shares with the through is given its own phase
    here. US practice gives a protected left phase only to an exclusive left lane, so
    P17.6 should demote a shared-lane left to permissive, using the lane geometry.
  One-way layouts are tested through the pure core, because the graph builder makes
  two-way streets only.
- **P17.4** (3 h): the ring state machine as a pure unit (no world):
  - min, extension, max, gap-out;
  - yellow and red clearance;
  - barrier synchronisation and rest;
  - the first three tests and their faults.

  **Done 2026-10-28 (P17.4).** `nema.rs`: `PhaseTiming`, `RingState`, `Interval`
  (`Green`, `Yellow`, `RedClear`, `Idle`), `Calls`, `RingConfig::from_settings`, and the
  pure `DualRing::step`. Walk and PedClear are left to P17.7: they lie inside the green.
  The behaviour, where the STM leaves the detail open:
  - The max timer starts on the first conflicting call and does not reset.
  - A gap with a conflicting call waiting is latched. The phase is then committed to end
    and is not re-extended, so a partner held at the barrier ends as a gap-out.
  - A ring moves within its side on its own. Both rings start their clearances on the
    same step at a barrier, and both start their next greens on the step the later
    clearance ends; the earlier ring dwells in red.
  - A call only behind the rings on this side (say 1 while 2 runs) makes the rings pass
    straight through the empty far side and back.
  - On entering a side, each ring serves its first called phase. A ring with no call
    there serves its barrier phase (dual entry for the throughs, `barrier_phases` and
    `barrier2_phases`), or `Idle` red when that phase is absent.
  - Start-up is in 2 and 6.
  - Soft recall calls its phases only when no other phase has a call.
  - Lead and lag order each barrier group of a ring.
  Tests: `a_green_runs_its_minimum_extends_to_its_maximum_and_rests`,
  `concurrent_throughs_gap_out_independently`, `rings_cross_barriers_together` (1 h, 5
  seeds, keyed SplitMix arrivals, plus a check that every interval ran in full) and
  `no_conflicting_greens` (assignment plus rings on a generated crossroads and tee). The
  STM 8-phase example test waits for P17.5's projection.
- **P17.5** (3 h): `project` into `SignalPlan`, `World::validate` after the rewrite, and
  the engine wiring through `ActuatedControllers::new` and the step. Drivers obey it.

  **Done 2026-10-28 (P17.5).** `nema/control.rs`: `NemaControllers` over every plan that
  maps (`assign_all`). The rest keep the single-ring controller, counted by reason in
  `ActuatedControllers::nema_fallbacks`. `control: nema` now loads; the default stays
  fixed-time.
  - **When the plan is rewritten.** Only when the shown vector changes, when composite 0
    is within two steps of its projected end, or at most once a second when the calls
    behind the prediction change. Rewriting every step is not needed, because the clock
    runs on by itself between rewrites.
  - **How it predicts.** A forward run of a copy of the rings, at 0.5 s steps, for up to
    240 s or one whole cycle back to now's vector. Called phases that are not green now
    are assumed extended (so they run to their maximum). A current green keeps its
    current detector state.
  - **What each movement shows.** A left with its own phase is green-yield beside its
    own through's green, and amber in that through's yellow (protected-permissive only).
  - **Timing.** Yellow is `nema.yellow_s`, else the plan's amber, else STM Eq. 6-1 at the
    fastest approach. Red is `nema.red_clear_s`, else the plan's all-red, else 0.
  - **Crosswalks.** Each crosswalk walks for `walk_s`, then shows its clearance as amber
    (crossing length / `ped_speed_mps`). Every crosswalk phase is on pedestrian recall
    until P17.7, and its minimum green holds walk + clearance + the MUTCD 3 s buffer,
    less Y + R.
  - **Invariants.** `plan_invariants` (World::validate's per-plan rules) is asserted in
    debug after every rewrite.
  - **SPaT feed.** `IntersectionFeed::sync` rebuilds the group timelines when the
    phases change. Fixed-time and single-ring plans never change them, so the default
    path is untouched.
  - **Refused until later pieces.** Priority requests at NEMA junctions are skipped
    (P17.9). The loader refuses `left_turn_mode` other than protected-permissive (P17.6),
    `ped_recall` (P17.7), and `coordination`, `green_wave_speed_mps` and
    `short_link_clearance_m` under nema (P17.8).
  - **Pedestrian fallback.** Walk and speed fall back to `world.signals.pedestrian_*`
    (set in `Engine::build_with_world`), then to MUTCD 7 s and 1.067 m/s.
  - **STM example.** `nema::tests::stm_example_timing` is built from Exhibits 6-2, 6-3
    and 6-9 (the STM has no numeric example). It gives cycle 132.3 s and splits
    24 / 55.1 / 19 / 34.2 s under max recall.
- **P17.6** (3 h): left-turn modes, lead/lag, and FYA display, with the SPaT
  `MovementPhaseState` mapping in `infra.rs` and the FYA test.

  **Done 2026-10-28 (P17.6).** `nema::control::left_rules` gives each movement a `Rule`:
  an `own` phase that protects it and a `permit` phase in whose green it turns, yielding.
  - The three modes run. Lead/lag was already ring order (P17.4).
  - **The FYA window is the opposing through's green; the circular green is the left's
    own approach's.** MUTCD 2009 §4D.20 and §4D.18. Under FYA the left yields to the
    traffic it watches, so the lead-lag "yellow trap" goes away. Under a circular green
    the left sees its own head's yellow while the opposing through runs on. The
    behaviour moves, not just the face. Test: `fya_ends_the_lead_lag_yellow_trap`.
  - **A protected-permissive left that shares its lane with the through or the right**
    gets no arrow and runs permissive in its own approach's circular green, counted in
    `NemaControllers::shared_lane_lefts`. A protected left phase needs an exclusive
    left-turn lane in US practice (STM 2nd ed. ch. 5, left-turn phasing [verify the
    clause]): a waiting left would block the through behind it. Under `protected` the
    arrow is kept, as asked.
  - Consequence: the procedural grids, whose lefts share the inner lane, run two-phase
    (2+6, 4+8) under the defaults.
  - A permissive-only left calls its window's phase.
  - SPaT already mapped green-yield to permissive-Movement-Allowed (5) and green to
    protected-Movement-Allowed (6). `infra::tests::fya_maps_to_permissive_movement_allowed`
    pins that mapping and its agreement with `SignalState::j2735_phase`.
  - **Open.** The steady yellow after an FYA window is sent as protected-clearance (8),
    because `SignalState` has a single amber. J2735's permissive-clearance (7) needs a
    world state, a model change. A left needs its own signal group to show its FYA
    separately in SPaT, and the generated worlds give a head group per approach, not per
    left lane.
- **P17.7** (3 h): pedestrian phases (walk and clearance timing, calls from waiting
  walkers in `v2xw-world/src/walk.rs`) and recall, with the ped test.

  **Done 2026-10-28 (P17.7).** Pedestrian intervals live in the ring machine. Each phase
  with crosswalks gets one interval, `RingConfig::ped[p] = (walk_s, clearance)`, timed for
  the longest of its crosswalks at `ped_speed_mps`.
  - **Calls.** A standing pedestrian (under 0.3 m/s) within `PUSH_BUTTON_REACH_M` = 2.5 m
    of a crosswalk's kerb end presses its button. MUTCD 2009 §4E.08 sets the button's
    placement [verify the paragraph]. The call is latched: it waits for the phase's next
    green, unless the walk now showing lets the pedestrian start.
  - **Which greens walk.** A green walks when it is called or on `ped_recall`. That green
    holds walk + clearance + `max(0, 3 s − Y − R)` (the MUTCD buffer may overlap the
    vehicle change interval). Any other green does not walk, and the crosswalk shows
    don't walk.
  - **Pedestrian recycle.** A green resting with no conflicting call serves a new walk
    on a call, at once. Without it a pedestrian beside a resting main street would wait
    for a vehicle elsewhere to cycle the controller; typical controller practice
    [verify against the STM's rest-in-walk text].
  - **Where the calls come from.** The engine passes standing `VehicleClass::Pedestrian`
    actors through `ActuatedControllers::step_with_pedestrians` when the dual ring is on.
    The single ring is unchanged.
  - **Change from P17.5.** P17.5's blanket pedestrian recall is gone. With
    `ped_recall: []` (the default) crosswalks walk only when called, so a grid with
    crosswalks and no pedestrians no longer holds the long pedestrian minimums.
  - **Tests.** `nema::tests::ped_recall_serves_walk` (recall walks every cycle; a call
    from a single step is latched and served) and
    `nema::control::tests::a_pedestrian_at_the_kerb_calls_the_walk` (TR 36.885 urban
    grid).
- **P17.8** (3 h): coordination (cycle, offset, force-offs fixed or floating, permissive
  window, yield point), the `green_wave_speed_mps` interplay (`actuated.rs:385`
  `coordinate`), and the coordination test.

  **Done 2026-10-28 (P17.8).** `nema::Coordination` lives on `RingConfig`, and
  `DualRing::clock_s` is the shared clock; `NemaControllers::step` sets it to `now_s`.
  - **Cycle and offset.** They are the load-time plan's own. `green_wave_speed_mps` has
    already set the offset (`actuated::coordinate`), so a green wave reaches the rings
    through coordination. Without coordination it is refused under nema.
  - **Splits.** They keep the plan's division of the cycle between the streets. The main
    group is the load-time phases in which a main-street movement is not red, held to
    20-80 % of the cycle. Each left's split is its maximum green and clearance, at most a
    third of its group; the through takes the rest. This is this build's choice: the STM
    computes splits from volumes, which a plan does not carry.
  - **Yield point.** The end of the coordinated green: the TS 2 offset references the
    start of that green, so the yield point is offset + split − Y − R; Type 170's offset
    references its yellow, so the yield point is at the offset (SUMO's `controllerType`
    text).
  - **The coordinated phases.** They are always called and hold to the yield point. They
    may end only in the permissive window, which lasts until the next phase in the ring
    could no longer time its minimum before its force-off. A call that misses the window
    waits a cycle.
  - **Force-offs.** Floating ends a non-coordinated green at split − Y − R. Fixed ends it
    at its point in the cycle, with the splits laid end to end from the coordinated
    clearance (STM §7.3.4, Exhibit 7-14). Force-offs are counted on `RingCounters::force_offs`
    and logged with the max-outs on `traffic.control`. Unused time returns early to the
    coordinated phases, which hold.
  - **Tests.** `nema::tests::coordinated_phases_hold_their_offset`: saturated calls, both
    force-off modes, and the yield at 13 + 40.4 mod 100 s ±1 step in every cycle.
    `tests/nema_control.rs::coordinated_rings_with_a_green_wave_run_the_grid`.
  - **Open.** Offset transition, when a junction joins coordination off its offset, is a
    plain wait for the next yield point; there is no shortway or dwell transition. There
    is no early-yield or "max-in-coordination" option, and splits come from the plan,
    not from volumes.
- **P17.9** (3 h):
  - `priority.rs` on NEMA (early green and extension per ring, capped by
    `PriorityParams`);
  - the outlook through to `spat_with`;
  - `tests/nema_spat.rs`;
  - a NEMA adapter function `NemaSettings::from_sumo_params(&BTreeMap<String,String>)`,
    for WP-23.
- **P17.10** (3 h): the `invariants/nema-safety` check, the expectation rows, the city
  run through `runtiny.sh`, the model card, and the status note. The card states:
  "Re-implemented from FHWA STM 2nd ed. and NEMA TS 2-2016; SUMO was read for behaviour
  only".

---

## WP-18 GEMV²-style channel enrichment

**Today.**
- Shadowing σ is a fixed value per preset in `v2xw-radio/src/prop.rs:270-352` (for
  example Abbas LOS highway 3.95 dB), spatially correlated by `ShadowProcess`
  (`:434-472`).
- Obstacle cards:
  - `obstacle/building/sommer-2011` (`obstacle.rs:1071`);
  - `obstacle/vehicle/tr37885-nlosv` (`:1559`);
  - `obstacle/terrain/knife-edge-p526` (`:1884`).
- The foliage card `obstacle/foliage/boban-mel` is referred to (`obstacle.rs:2204`,
  `terrain.rs:97`) but **does not exist**.
- OSM imports `natural=wood`, `scrub`, `grassland` and `heath`, and `landuse=forest`,
  `meadow` and `grass`, all as `LanduseClass::Park` (`osm.rs:9262-9271`). Trees and grass
  are therefore indistinguishable.
- `natural=tree_row` and `natural=tree` are not imported.
- Neither the GEMV² paper (Boban, Barros, Tonguz, IEEE TVT 63(9), 2014, doi
  10.1109/TVT.2014.2317803) nor ITU-R P.833 is cached.

**Decision.**
1. **The vegetation layer.** Add a separate `World.vegetation: Vec<VegetationArea { ring, kind: Wood|TreeRow|Tree }>`.
   - It is serde-defaulted and skipped when empty.
   - It is **excluded from `content_hash` when empty** (`v2xw-world/src/hash.rs:236`:
     its tag is written only when the layer is non-empty), so every cached world keeps
     its hash.
   - In the native format it is a new section kind (`serde_native.rs`: readers skip
     unknown kinds, so this is additive and needs no version bump).
   - It is imported only under a new OSM option `vegetation: true` (default off).
   - A tree row is buffered to a polygon of `tree_row_width_m`; a single tree to a disc
     of `tree_crown_m`.
2. **Foliage loss.** A new card `obstacle/foliage/p833` beside `vehicle_blockage`:
   - the in-vegetation path length comes from `ring_crossing` (`obstacle.rs:702`) summed
     over the candidate polygons;
   - the loss is the ITU-R P.833 excess attenuation for a terrestrial path through
     woodland, using P.833's specific attenuation and maximum attenuation at 5.9 GHz
     [verify: pin the edition, equation numbers and the figure in P18.1].
3. **Small-scale σ.** σ becomes `σ_min + (σ_max − σ_min)·clamp(ρ/ρ_ref)` per link class
   (LOS, NLOSv, NLOSb), where ρ is the density of vehicles and obstacles inside the link
   ellipse.
   - σ_min, σ_max and ρ_ref come from Boban et al. 2014 [verify: pin the table in P18.1].
   - Vehicles are counted from the actor obstacle index, buildings from
     `BuildingIndex::candidates` (`obstacle.rs:127`).
4. **Building reflection.** Optional single-bounce reflection for NLOSb: the image
   method on the nearest façades from `CornerTracer` (`obstacle.rs:470`). Its power is
   used only when it beats the Sommer through-wall value. Gated separately. It may be
   cut to a follow-up if P18.5 runs over.
5. **Off by default.** Every term is off by default and is validated before it is
   proposed as the default.

**Files and functions.**
- `v2xw-world/src/model.rs`: `World.vegetation`, `VegetationArea`, and the
  `content_hash` exclusion.
- `v2xw-world/src/osm.rs`: the vegetation import next to the landuse import
  (`:9262-9300`), and `OsmOptions.vegetation` (`:953` region).
- `v2xw-radio/src/obstacle.rs`:
  - a new `FoliageLoss` with `ID = "obstacle/foliage/p833"`;
  - `VegetationIndex::build(world)`, by analogy with `BuildingIndex` (`:75-153`);
  - fix the dangling `boban-mel` references at `:2204` and `terrain.rs:97`.
- `v2xw-radio/src/prop.rs`: a `DensitySigma` hook in the σ fed to `ShadowProcess::update`
  (`:472`).
- Engine wiring, where the obstacle cards are assembled: grep `"obstacle/"` users in
  `wiring.rs`.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `world.osm.vegetation` | `false` | bool | OSM wiki `natural=wood`, `landuse=forest`, `natural=tree_row` [verify] |
| `world.osm.tree_row_width_m` | ASSUMPTION, pinned in P18.1 | m | none known: stated as an assumption on the card |
| `radio.channel_model.foliage` | `off` (`p833`) | choice | ITU-R P.833 [verify] |
| `radio.channel_model.small_scale` | `fixed` (`density`) | choice | Boban et al. 2014 [verify] |
| `radio.channel_model.reflection` | `off` (`single-bounce`) | choice | Boban et al. 2014 [verify] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Empty road gives the class minimum σ | `prop::tests::density_sigma_is_class_min_on_an_empty_road` | density term ignored (always max): red |
| Dense arterial approaches the maximum | `prop::tests::density_sigma_approaches_max_on_a_dense_arterial`: ρ ≥ ρ_ref gives σ within 0.1 dB of σ_max | ρ counted outside the ellipse: red |
| 30 m of forest loses the P.833 value | `obstacle::tests::thirty_metres_of_wood_lose_the_p833_value`: within 0.1 dB of the hand-computed P.833 figure | path length taken as 0: red |
| Disabled is byte-identical | `tests/gemv2_off.rs`: content digest equals the no-option run; world hash unchanged with `vegetation: false` | include the empty vegetation in the hash: red |
| Vegetation import | `osm::tests::wood_and_tree_rows_import_as_vegetation` on a self-written fixture | tree_row dropped: red |

**Expectation rows** (`expectations/phy-channel.yaml`; a new family
`scenarios/sensitivity/rural-wood.yaml` on a cached city edge with woods, which P18.5
picks):
```yaml
- {setting: radio.channel_model.foliage, change: p833, metric: range90_m, expect: down,
   source: "vegetation on the path adds excess attenuation that grows with depth to a maximum (ITU-R P.833 [verify clause])"}
- {setting: radio.channel_model.small_scale, change: density, metric: prr, expect: changes,
   source: "small-scale variation grows with the density of vehicles and objects around the link (Boban, Barros, Tonguz, IEEE TVT 63(9) 2014)"}
```

**Goldens.** None (off). With `vegetation: true`, the world hash of that import changes
by design.

**Pieces.**
- **P18.1** (3 h, no cargo):
  - cache Boban 2014 (IEEE: check access; the authors' preprint if allowed) and ITU-R
    P.833 (ITU recommendations are free);
  - write `docs/design/research/gemv2-p833-constants.md` with σ_min, σ_max, ρ_ref per
    class, the P.833 equations and figure values at 5.9 GHz, and the tree-row
    assumption.

  **Sources resolved, 2026-10-27 (sources track, no code; `l.N` = this file at commit 081fcab2).**
  - *P.833 edition (l.459).* **ITU-R P.833-10 (09/2021)** is in force
    (<https://www.itu.int/rec/R-REC-P.833/en>). PDF:
    <https://www.itu.int/dms_pubrec/itu-r/rec/p/R-REC-P.833-10-202109-I!!PDF-E.pdf>.
    Page numbers below are the PDF's.
  - **Design defect: the plan names the wrong clause.** Eq. (1), Aev = Am[1 − exp(−dγ/Am)]
    (§2.1, p. 4), is for a path with **one terminal inside** woodland. For a path blocked
    by a single tree or a tree row with **both terminals outside**, §3 says "Equation (1)
    does not apply". §3.1 (≤ 1 GHz, p. 9) gives eq. (7), Aet = dγ, capped at the
    lowest-loss path around the vegetation (diffraction per P.526). **§3.2.1 (above 1 GHz,
    terrestrial, pp. 9–10)** requires the RET method: top diffraction (eq. 8, double knife
    edge per P.526), side diffraction, a ground-reflected component, and the
    radiative-energy-transfer scattered component. Its fitted parameters are Tables 4–8
    (pp. 15–19), given at 1.3, 2, 2.2, 11, 37 and 61.5 GHz. **There is no 5.9 GHz column**,
    so it must be interpolated between 2.2 and 11 GHz (an ASSUMPTION). Table 9 (p. 22)
    holds default oak-tree scatterer sizes. The γ of Fig. 2 (p. 5) is a plot only and would
    have to be digitised. **Recommendation:** for 5.9 GHz V2X, `p833` should implement
    §3.2.1. Failing that, a documented eq. (7) with a P.526 cap, labelled "P.833 §3.1 used
    above its 1 GHz range".
  - *Boban 2014 (l.463).* Author preprint arXiv:1305.0124 v3 (2014-04-22), "preprint of
    an article … IEEE TVT", doi 10.1109/TVT.2014.2317803. Its licence is the arXiv
    non-exclusive one, so the PDF is cited, not cached. **Table V, p. 8:** LOS σ_min 3.3 dB
    (Porto open space) and σ_max 5.2 dB (Porto downtown). NLOSv 3.8 dB (Porto highway) and
    5.3 dB. NLOSb σ_min 0 dB when reflections/diffractions are computed, else 4.1 dB (from
    Mangel et al.), with σ_max 6.8 dB. **Eq. (10), p. 8, is not the plan's linear clamp:**
    σ_i = σ_min + (σ_max − σ_min)/2 · (√(N_V,i/N_V,max) + √(A_S,i/A_S,max)). There is one
    vehicle-density term and one static-object-area term, with square roots. N_V,max and
    A_S,max are per km² from the Porto data, and **the paper gives no numeric values**, so
    ρ_ref stays an ASSUMPTION and becomes two references. The ellipse (§IV, p. 4) has foci
    Tx and Rx, major axis r and minor axis √(r² − d²). r comes from Table VI, p. 8: LOS
    500 m urban and 1 000 m outside, NLOSv 400 m, NLOSb 300 m. Only single-interaction
    reflections are modelled (footnote 4), which supports the `single-bounce` option.
  - *The dangling `boban-mel` references.* GEMV² eq. (4), pp. 6–7, gives foliage loss
    MEL = 0.79·f^0.61 dB per metre (f in GHz), from Goldhirsh & Vogel (its ref. [34]). That
    is **2.3 dB/m at 5.9 GHz**, which agrees with Durgin et al. 1998 at 5.85 GHz (ref.
    [32]). This is the source `boban-mel` should point to.
  - *OSM (l.491).* <https://wiki.openstreetmap.org/wiki/Tag:natural%3Dtree_row>:
    `natural=tree_row` is a **way** only. Neither a width nor `diameter_crown` is
    documented. So `tree_row_width_m` stays an ASSUMPTION, as planned.
  - **Not resolved.** Digitised P.833 Fig. 2 values. A 5.9 GHz RET parameter set (none is
    published in P.833). N_V,max / A_S,max for Porto.
- **P18.2** (3 h): the world vegetation layer, the OSM import, the hash exclusion, and
  the import and hash tests.
- **P18.3** (3 h): the `obstacle/foliage/p833` card, `VegetationIndex`, the 30 m test and
  its fault, and the fix for the dangling `boban-mel` references.
- **P18.4** (3 h): density σ in `prop.rs`, the ellipse counting, the two σ tests, and
  wiring into the link budget.
- **P18.5** (3 h):
  - the schema and settings;
  - `tests/gemv2_off.rs`;
  - the sensitivity family and expectation rows;
  - the reflection term, only if time allows (otherwise its key is refused with "not
    built");
  - the status note.

---

## WP-19 SCMS traffic through RSUs over the air

**Today.**
- `rsu-relay` access exists (`v2xw-engine/src/backend.rs:15`, `AccessKind::RsuRelay`
  at `:173`). Its air hop is a **constant** `Link` (`backend.rs:484-492` `air`, plus
  `:1244-1256`) with `ScmsParams::v2x_air_latency` and `v2x_air_bandwidth_bps`.
- Relay binding is by distance, at `RELAY_RANGE_M` = 300 m (`:157`; constants.md marks
  it ASSUMPTION).
- TCP/TLS rides `PacketPath` (`v2xw-radio/src/ip_transport.rs:35`), which has
  implementations for `PacketUu`, `ConstantPath` and `WiredLink`, but none for
  802.11p.
- The SCMS realism defects this fixes (`docs/design/scms-real/06-validation-summary.md`):
  - **M11**: relayed report uploads go out on safety channel 172;
  - **U5**: there is no WSA encoder (`v2xw-node/src/rsu.rs:51`,
    `RsuConfig::wsa_payload_bytes` = `None`).
- The deployment fact (`01-deployment-and-governance.md:267-268`, RSU LL p.4): NYC topped
  off weekly through RSUs advertising the SCMS service in a WSA, and OBUs sometimes left
  coverage before a top-off finished.
- The WSA ASN.1 (`IEEE-1609-3-WSA`, `wsa.asn` and `wee.asn`) is read in R4 §A.8 from
  the TCI_ASN1 copy. It is **not** in `third_party/asn1`.

**Decision.**
1. **The air hop as a PacketPath.** The SCH hop is a new `PacketPath`, `RsuSchPath`,
   evaluated per IP packet at offer time:
   - **contact window**: the vehicle's predicted distance at `at`, by constant-velocity
     extrapolation from the session start. The packet is lost once the predicted
     distance passes the SCH range;
   - **loss**: the PER from the existing link budget at that distance, at the SCH power
     and MCS;
   - **access**: AIFS(AC_BE) plus the mean backoff plus the busy wait from the SCH
     channel's measured CBR (the WP-13 meters, read at session start);
   - **alternating units**: a one-radio OBU sends only in the SCH interval (WP-16's
     `fits`).
2. **Why not frame by frame.** A frame-level SCH (every TCP segment a DES frame) would
   need `ip_transport`'s synchronous round model rewritten as events. That is a later
   package.
3. **WSA codec.** Cache `wsa.asn` and `wee.asn` under `third_party/asn1/ieee1609.3/`,
   with their licence in `PROVENANCE.md`. Generate bindings with `rasn-compiler` as a
   third unit in `v2xw-msg/build.rs`, or hand-write them if the compiler cannot. Use the
   encoding rules the module and IEEE 1609.3-2020 §8 state [verify: read them from the
   cached module; do not assume UPER or COER].
4. **Which RSU a vehicle uses.** When `net.rsu_ip.air: sch`, a vehicle binds to an RSU
   only after it has received that RSU's WSA advertising the SCMS service. The current
   distance-only binding (`bind_relay`, `backend.rs:983`) stays for `constant`.
5. **Everything SCMS moves.** Certificate downloads, CRL fetches and misbehaviour report
   uploads all move to the SCH path (fixes M11).

**Files and functions.**
- `third_party/asn1/ieee1609.3/` and `PROVENANCE.md`; `v2xw-msg/build.rs`.
- A new `v2xw-msg/src/wsa.rs` with `encode_wsa` and `decode_wsa`, including the
  `RoutingAdvertisement`. The size feeds `RsuConfig::wsa_payload_bytes`
  (`v2xw-node/src/rsu.rs:775`, `:1785`).
- `v2xw-radio/src/ip_transport.rs`: a new `RsuSchPath` (`impl PacketPath`) and its card
  `transport/80211p-sch`.
- `v2xw-engine/src/backend.rs`:
  - `carry` (`:494-540`), branch `Transport::RsuBackhaul`: the air leg becomes
    `RsuSchPath` when the mode is `sch`;
  - `from_scenario` (`:718`);
  - stats.
- `v2xw-net/src/stack/wave.rs:111-160`: `WsaProvider` and `WsaUser` carry the encoded
  bytes.
- `run.rs` RsuRelay sites (`:8976`, `:9013`, `:9051`, `:9300`, `:9507`): read only, to
  see where uploads are scheduled; move the report-upload airtime from 172 to the
  service channel.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `net.rsu_ip.air` | `constant` (`sch`) | choice | IEEE 1609.3-2020 (IP over the SCH) [repo R4 §A.8, secondary]; IEEE 1609.2.1 §6.3 (SCMS REST over HTTPS) [repo `ip_transport.rs:7`] |
| `net.rsu_ip.mtu_bytes` | 1500 | B | RFC 2464 §2 (IPv6 over Ethernet default MTU) [verify]; ≤ `timing::MAX_MSDU_BYTES` |
| `net.rsu_ip.service_psid` | pinned in P19.1 | PSID | IEEE 1609.12 PSID allocations [verify]; RSU LL p.4 says the SCMS service is WSA-advertised [repo] |
| `net.rsu_ip.wsa_interval_ms` | the existing `WsaProvider::interval_ms` | ms | 1609.3 RepeatRate semantics UNVERIFIED [repo R4 §A.8] |

The SCH channel itself is WP-13's `radio.channels` service channel. There is no new key
for it.

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| WSA round-trips through the codec | `wsa::tests::round_trip_with_routing_advertisement`; plus the oracle (`tools/oracle`, asn1tools) decodes our bytes, as WP-14 did | one field swapped in the encoder: red |
| Download success rises with contact time | `tests/rsu_sch.rs::download_success_rises_with_contact_time`: vehicles pass one RSU at 5, 10, 20, 30 m/s; the share of top-offs completed is monotone falling in speed | contact window ignored (infinite): flat, red |
| Falls with SCH load | `tests/rsu_sch.rs::download_success_falls_with_sch_load`: background SCH traffic gives CBR 0.1, 0.3, 0.5; completion falls monotonically | CBR read as 0: flat, red |
| CRL fetch and report upload complete over an RSU | `tests/rsu_sch.rs::crl_and_report_complete_over_the_air`: both complete, and their airtime is on the service channel, not 172 | uploads left on 172: red |
| Backhaul-only unchanged | `phase2` and `scms_scale` digests (G-SCMS) unchanged with `air: constant` | n/a |

**Expectation rows** (`expectations/live-rsu.yaml`, RSU family):
```yaml
- {setting: net.rsu_ip.air, change: sch, metric: report.rsu_sch_completed_share, expect: down,   # new RunReport leaf (P19.4)
   source: "an over-the-air top-off ends when the vehicle leaves RSU coverage (NYC CV pilot, RSU lessons learned p.4 [repo 01-deployment 267-268])"}
- {setting: net.rsu_ip.air, change: sch, metric: report.channel_busy_ns.172, expect: down,
   source: "SCMS traffic moves to the WSA-advertised service channel (IEEE 1609.3-2020 8 [repo R4 A.8]; fixes M11)"}
```

**Goldens.** None (default `constant`). With `sch`, every SCMS scenario that opts in
changes G-SCMS by design. Re-bless only those cases, and list them.

**Pieces.**
- **P19.1** (3 h):
  - cache `wsa.asn` and `wee.asn` with their licence;
  - pin the encoding rules and the SCMS PSID;
  - codec generation, or a hand-written codec, with the round-trip test and the oracle
    decode;
  - fill `wsa_payload_bytes` (fixes U5).

  **Sources resolved, 2026-10-27 (sources track, no code; `l.N` = this file at commit 081fcab2).** Full record:
  `third_party/asn1/ieee1609.3/PROVENANCE.md`. The PSID reasoning is in
  `scms-real/rule-gap-specs-2026-10-27.md` §2.1.
  - *Cache (step 3, first bullet): done.* The modules live in `third_party/asn1/ieee1609.3/`.
    That directory holds `wsa.asn`, `wee.asn`, `wsm.asn` and ISO 17419
    `CITSapplMgmtIDs`, which WSA imports. They come from TCI_ASN1 commit `63d82d87976b`
    (2017-05-21), byte-identical. **Upstream states no licence.** They are committed on the
    same basis as `../etsi/PROVENANCE.md` §3.5–3.7.
  - *Encoding (l.582): UPER.* It is read from the module headers ("mapping of UPER …
    validated with the OSS ASN.1 Studio tool", `AUTOMATIC TAGS`). It is cross-checked
    with asn1tools against the USDOT RSU Spec v4.1 Appendix D example octets. The one
    mismatch is App. D's TX-power octet, written without UPER's +128 offset. The 1609.3-2020
    §8 text stays paywalled. The modules are the 2016 edition (WSA version 3).
  - *`service_psid` (l.614): 0x23 (35)*, "WAVE security management". The source is the
    IEEE RA registry (<https://standards.ieee.org/products-programs/regauth/psid/public/>)
    and 1609.2.1 `SecurityMgmtPsid ::= Psid (35)`. It is an ASSUMPTION that US RSUs
    advertise this exact PSID: FHWA-JPO-20-804 p. 4 names no PSID. Also advertise **IPv6
    Routing 0x1020407E** (0pEF-FF-FF-FE) beside the WRA, as RSU Spec v4.1 App. D does.
  - *`wsa_interval_ms` (l.615).* RepeatRate counts **WSAs per 5 s** (RSU Spec v4.1 App. D:
    50 per 5 s, i.e. 10 Hz). Placement is the CCH (Req_587) in time slot 0 (Req_588). The
    WSA is signed per 1609.3-2016 Annex H.1 (Req_586).
  - *`mtu_bytes` (l.613).* RFC 8691 §4.1: "The default MTU for IP packets on 802.11-OCB is
    inherited from [RFC2464] and, as such, is 1500 octets". RFC 8200's 1 280 minimum also
    applies. Cite RFC 8691, the OCB-specific RFC, over bare RFC 2464. Verified.
- **P19.2** (3 h): the RSU sends the encoded WSA on air; `WsaUser` decides; the vehicle
  binds to the RSU only after a WSA in `sch` mode.
- **P19.3** (3 h): `RsuSchPath` (contact window, PER, CBR access, WP-16 interval) with
  unit tests against `ConstantPath` limits (no loss, infinite contact, so it is equal).
- **P19.4** (3 h): wiring in `backend.rs` `carry`; downloads, CRL and reports moved to
  the SCH; settings and units.
- **P19.5** (3 h): `tests/rsu_sch.rs`, the expectation rows, the faults, and
  `scms-real/06-validation-summary.md` rows M11 and U5 updated.

---

## WP-20 Traffic yields to emergency vehicles

**Today.**
- `VehicleClass::Emergency` exists (`v2xw-mobility/src/classes.rs:85`, 5 000 kg).
- Signal priority exists (`v2xw-engine/src/priority.rs`, `scenarios/emergency-priority.yaml`).
- Emergency beacons set `LAMP_EMERGENCY` (`lamps.rs:192`).
- **No civilian reacts**, and an EV obeys reds.
- `rules.rs` already chooses rules by jurisdiction preset (RTOR, filtering, mid-block
  crossing).
- The lateral model is SL2015 (`lateral/sl2015.rs`).
- WP-08's red-light-violation tag exists, but WP-08's own four tests are still red
  (adoption plan line 289). WP-20 reuses the tag, so its EV test is independent of
  those failures.
- V2X carriers:
  - BSM Part II `SpecialVehicleExtensions` is carried opaquely (`j2735/bsm.rs:93`,
    `j2735/mod.rs:213`);
  - the CAM has `VehicleRole::Emergency` (`cam.rs:184`), but no EmergencyContainer.
- The belief-only command pattern to copy is `reroute_info.rs` →
  `MobilityCommand::HazardHeard` (WP-05).

**Decision.**
1. **Who reacts, and how.** A civilian reacts when an EV with beacons on is behind it on
   the same carriageway, within `reaction_distance_m` (siren trigger) or after its unit
   verified an emergency message (V2X trigger). It reacts by jurisdiction (`rules.rs`):
   - **US**: pull to the right edge and stop (UVC §11-405, as `sumo.md` S-P5 cites
     [verify text]);
   - **`urban-de`, multi-lane**: the rescue lane, with the leftmost lane going left and
     the others right (StVO §11(2) [verify]);
   - **`urban-de`, single lane**: clear the way (StVO §38(1) [verify]).
2. **The movement.** The lateral offset goes through `LateralModel` when SL2015 is on;
   otherwise a lane change to the right-most lane plus a slow-down. The offset is
   clamped to the carriageway edge, so no wheel leaves the road.
3. **The EV at a red.** It may pass a red after slowing to `red_pass_speed_mps` and
   finding the conflict area clear, as UVC §11-106 allows [verify]. It is tagged as an
   intended violation, so `audit.rs` does not count it as a defect.
4. **The V2X variant.**
   - **US**: encode `SpecialVehicleExtensions.vehicleAlerts` (EmergencyDetails:
     `sirenUse`, `lightsUse`) [verify against J2735 ASN.1];
   - **EU**: the CAM `EmergencyContainer` (`lightBarSirenInUse`) from the cached
     `cam_ts103900` module [cached];
   - the node app turns a verified one into `MobilityCommand::EmergencyApproach {claimed position, heading, speed}`;
   - only receivers react, from their belief.

   Encoding is gated with the feature, so BSM bytes do not change by default.

**Files and functions.**
- `v2xw-mobility/src/rules.rs`: `EmergencyYield { PullRight, RescueLane, ClearWay }` per
  preset, with the source table in the module doc.
- `v2xw-mobility/src/engine.rs`, `MobilityEngine::step`:
  - pass 6 "decide" (from `:6138`): yield speed and stop;
  - the lane-change request;
  - the EV red pass in `held_by_signal` (`:4729`) and `red_elapsed_s` (`:5678`);
  - `command` (`:7748`) handles `EmergencyApproach`.
- `v2xw-mobility/src/lateral/mod.rs` and `sl2015.rs`: a target offset to the edge, with
  the carriageway clamp.
- `v2xw-mobility/src/audit.rs`: accept the tagged EV red pass.
- `v2xw-msg/src/j2735/bsm.rs`: Part II id 1 EmergencyDetails. `v2xw-msg/src/cam.rs`: the
  EmergencyContainer.
- `v2xw-node/src/apps.rs`: an emergency-vehicle receive hook.
- A new `v2xw-engine/src/ev_info.rs`, modelled on `reroute_info.rs`.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `traffic.emergency_yield.enabled` | `false` | bool | n/a |
| `traffic.emergency_yield.trigger` | `siren` (`v2x`, `both`) | choice | n/a |
| `traffic.emergency_yield.reaction_distance_m` | 25 | m | SUMO bluelight `reactiondist` default [repo `sumo.md` §3.8; S-P5 says calibrate against siren audibility] |
| `traffic.emergency_yield.v2x_reaction_distance_m` | ASSUMPTION, pinned in P20.4 | m | none known: the card says so |
| `traffic.emergency_yield.red_pass_speed_mps` | ASSUMPTION, pinned in P20.3 | m/s | UVC §11-106 states "slowing down as necessary" without a figure [verify] |
| `messages.special_vehicle_extensions` | `false` | bool | SAE J2735 `SpecialVehicleExtensions` [verify] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Corridor travel time falls with yielding on | `tests/emergency_yield.rs::corridor_time_falls_with_yielding`: an ambulance through a congested 1 km arterial, under 3 seeds; on < off in every seed, mean −15 % or more (threshold set from the first measured run, before the fault run) | yielding off in the on-run: red |
| No civilian leaves the carriageway | the auditor `off-road` count is 0 over the same runs, plus `lateral::tests::pull_over_offset_is_clamped_to_the_edge` | remove the clamp: red |
| With the V2X variant only receivers react | `tests/emergency_yield.rs::only_receivers_react`: equipped share 0.5, and every vehicle that yielded verified an EV message first | trigger reads truth: unequipped yield, red |
| EV passes a red after slowing | `engine::tests::ev_passes_red_after_slowing`: speed at the stop line ≤ `red_pass_speed_mps` + 0.5, the pass is tagged, and audit defects stay 0 | tag missing: audit defect, red |
| Rescue lane on `urban-de` | `engine::tests::rescue_lane_on_a_three_lane_road` | the US rule applied: red |
| Off is byte-identical | `phase2` and `bsm_timing` (G-SCMS) and G-TV unchanged | n/a |

**Expectation rows** (`expectations/traffic.yaml`, emergency family =
`scenarios/emergency-priority.yaml` cut to 120 s):
```yaml
- {setting: traffic.emergency_yield.enabled, change: true, metric: report.emergency_travel_time_s, expect: down,   # new RunReport leaf (P20.5)
   source: "drivers pull to the edge and stop for an approaching emergency vehicle (UVC 11-405 [verify]); SUMO bluelight device behaviour"}
```

**Goldens.** None with it off. `messages.special_vehicle_extensions: true` changes BSM
bytes for EVs; list that when the owner flips it.

**Pieces.**
- **P20.1** (3 h): the `rules.rs` yield table with sources; pass-6 lane-level yield (slow,
  move right, stop) for the siren trigger; unit tests.
- **P20.2** (3 h): the SL2015 edge offset and carriageway clamp, the rescue lane, and
  their tests.
- **P20.3** (3 h): the EV red pass, the tag, the audit, its test, and pinning
  `red_pass_speed_mps` (cite or mark it as an assumption).
- **P20.4** (3 h):
  - BSM EmergencyDetails and the CAM EmergencyContainer encoding, gated, with
    round-trip tests (the J2735 ASN.1 for EmergencyDetails cached first);
  - the node hook, `ev_info.rs`, and `EmergencyApproach`.
- **P20.5** (3 h): `tests/emergency_yield.rs`, the expectation row, the faults, and the
  status note.

---

## WP-21 Condition-triggered scenario events

**Today.**
- `TimelineItem { t, until, kind, params }` (`schema.rs:3634-3646`), with
  `TimelineKind` closed (`:3655`).
- Items are scheduled once at `t` by `seed_timeline` (`run.rs:2563-2592`) as
  `Event::Control`, and fired by `on_control` (`run.rs:2889`).
- Safety kinds pick `target: "auto"` (`safety_events.rs`).
- TTC exists in `v2xw-metrics/src/ssm.rs` (`evaluate_following` at `:600`; WP-11).

**Decision.**
1. **The clause.** `TimelineItem.when: Option<Condition>`. `Condition` is a closed,
   serde-tagged enum:
   - `ttc_below { s }`;
   - `in_polygon { points | zone }`;
   - `speed_diff_above { mps }`;
   - `probability { p }`;
   - `all [..]` and `any [..]`.
2. **Arming and firing.** An item with `when` is armed at `t` and evaluated once per
   mobility step after the mobility update (end of `on_mobility_step`, `run.rs:3350`).
   It fires at the first step where the clause holds. `max_fires` defaults to 1, with a
   `cooldown_s`.
3. **Firing reuses the existing path.** It schedules `Event::Control { item, end: false }`
   at `now`. With `until`, the end is scheduled `until − t` after firing; validation
   requires `until > t`.
4. **Binding the target.** Conditions bind actors: `ttc_below` binds `leader` and
   `follower`, `in_polygon` binds `actor`. Effects read `target: "when.leader"` and the
   like; `"auto"` is unchanged.
5. **Evaluation order.** Actors are visited in actor-id order. `probability` draws are
   keyed by `(seed, item index, step)`, so replays are byte-identical and no shared
   stream shifts.
6. **Ground truth.** The scenario director reads ground truth by design. It is not a
   detector, so I-T1 to I-T3 are unaffected. The card says so.

**Files and functions.**
- `schema.rs` near `:3634`: `when`, `Condition`, `max_fires`, `cooldown_s`.
- `validate.rs`: the target binding must match the condition; the polygon must be closed
  and inside the world box.
- `options.rs`, `guide.rs`.
- `v2xw-engine/src/timeline.rs`: `Condition::eval(&self, view: &TriggerView) -> Option<Binding>`
  as a pure function, with unit tests.
- `run.rs`:
  - `seed_timeline` skips the scheduling of `when` items;
  - a new `fn evaluate_triggers(&mut self, now)` called from `on_mobility_step`;
  - `on_control` reads the binding (`:3058` and `:3140` for the safety kinds).
- `safety_events.rs`: accept a bound target.
- A new record `timeline.fired {item, t, binding}`, under an opt-in metric group so that
  default records do not change.

**Settings.** `events[].when`, `events[].max_fires` (1, count) and `events[].cooldown_s`
(0, s). These are syntax, and need no physical citation. The design reference is
Artery's storyboard (behaviour only), and the use case is Euro NCAP-style scripted tests
[repo `safety_events.rs:13`].

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Hard brake fires when the follower's TTC drops below 2 s, under several seeds | `tests/trigger_ttc.rs::hard_brake_fires_at_ttc_two_seconds`: 5 seeds; at the firing step the bound pair's TTC ∈ (2 − closing·dt, 2] s and > 2 one step earlier | comparator `>`: red; TTC read one step stale: red |
| Reruns are byte-identical | the same file: two runs per seed, equal content digests; with `probability` in the clause | draw from the shared stream: seeds diverge after a tweak, red |
| Time-only scenarios unchanged | `phase2` and `bsm_timing` (G-SCMS) unchanged; `scenarios/edge/*` digests unchanged | n/a |
| Clause logic | `timeline::tests::all_any_truth_table`, `in_polygon_binds_the_first_actor_inside`, `probability_zero_never_one_always` | `any` evaluated as `all`: red |

**Expectation rows.** None. This is a control feature, not physics.

**Goldens.** None. Add the example scenario `scenarios/edge/ttc-triggered-brake.yaml`
with its own new pinned digest.

**Pieces.**
- **P21.1** (3 h): the schema, validation, options and guide; `Condition::eval` as a pure
  function with its unit tests and faults.
- **P21.2** (3 h): the engine hook, the binding into `on_control` and the safety kinds,
  and the `timeline.fired` record.
- **P21.3** (3 h): `tests/trigger_ttc.rs`, the example scenario and its digest, the
  faults, and the status note.

---

## WP-22 Grade-aware, power-limited acceleration

**Today.**
- No car-following model reads the grade (`sumo.md` §3.1).
- Lane geometry carries z from the DEM drape (`v2xw-world/src/dem.rs:14`), and
  `Lane::point_at(s)` (`model.rs:1104`) gives the grade by finite difference.
- `ClassSpec` (`classes.rs:32-56`) has `mass_kg` (truck 4 500 kg, the SUMO vType table),
  but **no power, CdA or rolling coefficient**.
- Road-load physics exists only in the energy crate:
  - PHEMlight `Cep::power_kw(v, a, grade)` (`v2xw-energy/src/phemlight.rs:169`), with
    `rated_kw` for two passenger cars;
  - `ev.rs` (`drag_coefficient`, `mass_kg`, SUMO-cited).
- `powertrain.rs`, which the plan names, holds only segment and fleet assignment. The
  physics to reuse is in `phemlight.rs` and `ev.rs`.
- The energy provider takes the grade from the track z (`provider.rs:459`).
- The acceleration is decided in `MobilityEngine::step` pass 6 (`engine.rs:6138-6990`):
  a sequence of `accel = accel.min(..)`.

**Decision.**
1. **The clamp.** After every limit in pass 6 and before integration:
   `a ≤ min(a_model, (η·P/(m·v) − F_roll/m − F_aero/m)) − g·sinθ`.
   - v is floored at 1 m/s to avoid the singularity.
   - It is applied **only as an upper bound**: braking is untouched.
   - On a downgrade the `− g·sinθ` term raises the bound. The model's desired speed
     still holds the speed.
2. **Two flags.** They let flat roads be byte-identical when only the grade is on:
   - `grade`: the `g·sinθ` term;
   - `power_limit`: the power term, which can bind on flat roads at speed.
3. **θ** comes from the lane polyline: `(z(s+5) − z(s−5))/10`, clamped like
   `provider.rs`'s `GRADE_CLAMP`.
4. **Class dynamics.** A new class table: `power_kw`, `cd_a_m2`, `c_rr`, `eta`. The
   passenger rows reuse the PHEMlight constants already cited. The truck row comes from
   the AASHTO Green Book's typical heavy truck, 120 kg/kW (200 lb/hp) [verify: Green
   Book edition and the speed-distance figure], and the HCM truck weight-to-power
   assumption [verify: HCM 6th/7th ed. chapter].
5. **The truck's mass.** The SUMO truck mass (4 500 kg) is far below a loaded tractor
   trailer. The test vehicle uses `Trailer` or an explicit mass, and the class table
   states which.

**Files and functions.**
- `v2xw-mobility/src/classes.rs`: `ClassDynamics` per class, with a source row each.
- `v2xw-mobility/src/engine.rs`: `fn power_grade_bound(actor, lane, s, v) -> f64`,
  called at the end of pass 6; and a grade sampler from the lane geometry.
- `v2xw-energy/src/phemlight.rs`: read-only reuse of the passenger constants, through an
  accessor, without moving code.
- `schema.rs` and validation; `v2xw-trafficval/src/calib.rs` gets a new check.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `traffic.vehicle_dynamics.grade` | `false` | bool | KraussPS `aMax − g·sin(slope)` behaviour [repo `sumo.md` §3.1] |
| `traffic.vehicle_dynamics.power_limit` | `false` | bool | S-P6 [repo] |
| class `power_kw`, `cd_a_m2`, `c_rr`, `eta` | passenger: PHEMlight rows [repo `phemlight.rs`]; truck and trailer: pinned in P22.1 | kW, m², 1, 1 | AASHTO Green Book; HCM [verify] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| A heavy truck on a long 5 % upgrade reaches a crawl speed in the AASHTO/HCM band | `engine::tests::truck_crawls_on_a_five_percent_grade`: generated 3 km lane at a constant 5 %, 120 kg/kW truck; the speed after 2 km is within the band read from the Green Book curve in P22.1 | grade sign flipped: truck accelerates, red; power term omitted: no crawl, red |
| Flat roads byte-identical | `tests/vehicle_dynamics.rs::flat_grade_only_is_byte_identical`: `grade: true` on a flat procedural world gives the same content digest as off | θ from an un-draped z offset: red |
| Downgrade does not overspeed | `engine::tests::downgrade_holds_desired_speed` | bound applied to braking too: red |
| Cars barely affected | `engine::tests::passenger_car_keeps_limit_on_five_percent`: reaches 90 % of the limit within the HCM passenger-car expectation [verify] | n/a |
| Held check | `calibration/truck-grade-crawl` (G-TV, new) | as the first row |

**Expectation rows** (`expectations/traffic.yaml`; a new family
`scenarios/sensitivity/grade-truck.yaml`):
```yaml
- {setting: traffic.vehicle_dynamics.power_limit, change: true, metric: traffic.speed_mean_mps, expect: down,
   source: "a power-limited heavy vehicle loses speed on an upgrade to a crawl speed (AASHTO Green Book truck speed-distance curves [verify figure])"}
```

**Goldens.** None (both flags off). Turning `power_limit` on moves G-TV
(`calibration/saturation-flow` start-up, `cities/*`) and the BSM acceleration fields.

**Pieces.**
- **P22.1** (3 h, mostly no cargo): pin the Green Book and HCM figures; write the class
  dynamics table with sources (`docs/design/research/grade-dynamics.md`); add the schema
  keys.

  **Sources checked, 2026-10-27 (sources track, no code; `l.N` = this file at commit
  081fcab2).** Mostly unresolved, because both primaries are paywalled.
  - *120 kg/kW (200 lb/hp) truck (l.878).* **Verified, secondary.** NCHRP Report 505,
    *Review of Truck Characteristics as Factors in Roadway Design* (2003, doi
    10.17226/23379), App. D "Field Estimates of Truck Weight-to-Power Ratios", pp. 153–159:
    "for the 2001 Green Book, a truck with a weight-to-power ratio of 120 kg/kW [200 lb/hp]
    was chosen as the basis for computation of critical length of grade". It adds that the
    data "could also support a choice in the 90- to 108-kg/kW range". The text was read
    through a search-index excerpt, because the NAP online page rendered no body text.
    Critical length is defined as the grade length giving a **15 km/h (10 mph)** reduction
    for that truck (2001 Green Book, the same secondary). Field studies give 102–108 kg/kW
    for CA/CO freeways and 126 kg/kW for PA (TRR 1881, doi 10.3141/1881-05; seen in a search summary only,
    not read). That makes a good sensitivity range for the class table.
  - **Not resolved.** The current Green Book edition, and the figure number and curve
    values for the 120 kg/kW deceleration-on-upgrade chart (paywalled; no public
    reproduction found). That means the **crawl-speed band for 5 %** in
    `truck_crawls_on_a_five_percent_grade` has no source yet. The HCM 6th/7th ed. truck
    weight-to-power assumption and chapter (l.880) are unresolved. So is the HCM passenger-car
    expectation (l.908). Until a copy is bought, the band must be labelled ASSUMPTION. The
    test should derive it from the steady-state power balance (P·η = m·g·(sin θ + c_rr)·v +
    ½ρ·C_dA·v³) at 120 kg/kW and state that it does, not cite the Green Book for a number
    nobody here has read.
- **P22.2** (3 h): the clamp and grade sampler in `engine.rs`, with the unit tests and
  faults.
- **P22.3** (3 h): `tests/vehicle_dynamics.rs`, the `calibration/truck-grade-crawl`
  check, the expectation row, and the status note.

---

## WP-23 Import actuated and NEMA programmes from `net.xml` (after WP-17)

**Today.**
- `RawTlLogic { id, program_id, logic_type, offset_s, phases: Vec<(duration, state)> }`
  (`v2xw-world/src/sumo.rs:944-957`). The parser is at `:1214-1235`. Phase `minDur`,
  `maxDur` and `name`, and `<param>`, are **not read**.
- The card says actuated and delay-based programmes "are imported as their static phase
  list" (`:3404`).
- Signal control is global (`traffic.signals`), with no per-plan kind.
- TraCI `GET_TL` (0xa2) and `TL_STATE` (0x20) are defined (`v2xw-interop/src/traci.rs:35,75`)
  but **unused**. Cross-validation compares signal counts only (`xval.rs:957`, `:1014`).
- SUMO runs from the venv `V2XW_SUMO_VENV` (1.27.1, adoption plan line 233). It is not
  on PATH. `grid3` tests print "SUMO not found: skipped" (`tests/grid3.rs:46-51`).

**Decision.**
1. **What is read.** `minDur`, `maxDur`, `name` and `<param key value>`. The import uses
   a ledger like WP-15's: applied, approximated, not modelled, so every attribute and
   param is counted.
2. **Per-plan controllers.** A new world side table,
   `World.signal_programmes: BTreeMap<SignalId, SignalProgramme { kind: Static|Actuated{min,max per phase, params}|Nema{params} }>`.
   It is hash-excluded when empty (`hash.rs:236`) and is a new native section kind
   (`serde_native.rs`, additive), as in WP-18, so OSM and procedural worlds keep their
   hashes.
   - `ActuatedControllers::new` uses a plan's programme when present, else the global
     `traffic.signals`.
   - NEMA goes through WP-17's `NemaSettings::from_sumo_params`.
   - `delay_based` becomes actuated, with a new anomaly `DelayBasedAsActuated` counted.
3. **Default.** `world.sumo.signal_programmes: as-file`. Flattening is the card's stated
   limitation, and therefore a defect fix. It moves no golden, because the only shipped
   `net.xml` (`worlds/sumo/grid3.net.xml`) has 18 static tlLogics. P23.1 re-checks that
   with grep before relying on it.
4. **A skip is not a pass.** The TraCI comparison is reported **not run**, not passed,
   when SUMO is absent ("checks that cannot fail").

**Files and functions.**
- `v2xw-world/src/sumo.rs`: `RawTlLogic` gains `phase_bounds` and `params`; the parser at
  `:1214-1235`; the build into `World.signal_programmes`; a new `SumoAnomaly` variant; the
  card text at `:3404`.
- `v2xw-world/src/model.rs`: `SignalProgramme`, and the hash exclusion.
- `v2xw-engine/src/actuated.rs:138`: per-plan programmes.
- `v2xw-interop/src/traci.rs`: `get_tl_state(id)`. `v2xw-interop/src/xval.rs`: a per-step
  state comparison and `TlAgreement { steps, equal, first_diff }`.

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `world.sumo.signal_programmes` | `as-file` (`static`) | choice | SUMO `net.xml` tlLogic attributes [repo `sumo.md` S-P4; XSD as a file format] |
| SUMO actuated params (`max-gap`, `detector-gap`, `passing-time`, …) | the file's, else SUMO's documented defaults | s | SUMO "Traffic Lights" documentation [verify each; `detector-gap` 2 s already cited at `actuated.rs:51`] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| An actuated and a NEMA tlLogic import as live controllers | `sumo::tests::actuated_tllogic_imports_with_phase_bounds` and `nema_tllogic_imports_as_rings` (self-written fixtures); engine `actuated::tests::imported_programme_gaps_out` | `minDur` ignored: red; type read as static: red |
| Delay-based falls back with a counted anomaly | `sumo::tests::delay_based_becomes_actuated_and_is_counted` | anomaly not counted: red |
| Every param is counted | `sumo::tests::every_tl_param_is_in_the_ledger` | drop a param silently: red |
| TraCI signal phases agree | `tests/grid3.rs::tl_states_agree_with_sumo` on a netconvert-generated actuated grid (`--tls.default-type actuated` [verify flag]): ≥ 0.95 of (tls, step) pairs identical over 600 s, with the measured share recorded and SUMO version pinned; reports not run without SUMO | compare against the wrong tls id: red |

**Expectation rows.** None. This is import fidelity.

**Goldens.** None expected; P23.1 confirms. Static programmes are byte-identical.

**Pieces.**
- **P23.1** (3 h): parse the bounds and params, the ledger, the anomaly, the
  `SignalProgramme` side table and its hash exclusion, and the unit tests. Grep the
  shipped worlds for non-static tlLogics.
- **P23.2** (3 h): per-plan controller binding in `actuated.rs`, the NEMA adapter call,
  and the engine test.
- **P23.3** (3 h): the TraCI `get_tl_state` comparator, the generated actuated fixture,
  the agreement test (run through the venv under `runtiny.sh` limits), the faults, and
  the status note.

---

## WP-24 VeReMi Extension replica and reference ML baseline

**Blocked in part:** WP-07 (`f2md-19`) delivered nothing. It needs an **owner decision**
(adoption plan lines 252-253). The replica's "30 % attackers on `f2md-19`" cannot run
until then. The pieces below are ordered so that everything that does not need the
`f2md-19` profile lands first.

**Today.**
- The VeReMi export is done (WP-03): `veremi` exporter with `opts.codebook` and
  `opts.slice_s`, metric group `detection-scores` with MCC, BM, MK and kappa, and
  per-message metrics.
- WP-10's checks are done.
- A fixed beacon rate exists: `messages.rate_hz` (`schema.rs:2705-2711`). The 1 Hz
  option needs no new code.
- The detector profile `f2md-veremi-1hz` exists (WP-02).
- `PyDetector` is in `crates/v2xw-py/src/plugins.rs:662`.
- A LuST sample route file is in `worlds/sumo/lust-sample.rou.xml`. WP-15 records LuST
  as MIT; P24.1 re-checks the full scenario repository's licence.
- F2MD's VeReMi noise (`veremiConf`) is described in `f2md.md` §4.2: position confidence
  U(3,5) m, heading confidence U(0,20)°·e^(−0.1v), speed ≈ N(0, 0.0016)·v. That is
  behaviour from read source; the numbers are F2MD's constants, allowed by §1 of the
  adoption plan.
- The scenario description (`f2md.md` P10): the LuST subsection ≈ 1.61 km², 07-09 h and
  14-16 h, 30 % attackers. Source: Kamel et al., ICC 2020 [repo `f2md.md:40`].

**Decision.**
1. **Network.** The replica imports the LuST `net.xml` subsection through
   `v2xw-world::sumo`, with the WP-23 programmes as in the file.
2. **Routes and timing.** Routes come from the LuST `.rou.xml` through WP-15's import,
   with `messages.rate_hz: 1`, plus an opt-in GNSS preset `veremi-ext` (never default).
3. **Metrics** come from the existing `detection-scores` group.
4. **The published-numbers table** gets every number from the ICC 2020 paper, with its
   table and page, and the note on F2MD defect D3 (GridSybil inflated).
5. **ML.** XGBoost and MLP train on our own export (feature vector = all check scores
   over the last 3 messages, F-P11) through `PyDetector`. The recipe is in
   `docs/guides/veremi-ext-baseline.md`, with seeds fixed. No F2MD Python is used: it
   has no licence.

**Files and functions.**
- New `scenarios/benchmark/veremi-ext-replica.yaml`, with two windows.
- `worlds/sumo/` (the LuST subsection, if the licence allows; else a fetch script like
  `worlds/fetch-gtfs.sh`).
- `v2xw-mobility/src/gnss`: a new preset `veremi-ext`, reached through
  `GnssSettings` (`schema.rs:1140`).
- `tools/` or `crates/v2xw-py` example: training script.
- `docs/guides/veremi-ext-baseline.md`, and `docs/design/frameworks/veremi-ext-comparison.md`
  (the table).

**Settings.**

| Key | Default | Unit | Source |
|---|---|---|---|
| `actors.vehicles.gnss.preset` | `none` (`veremi-ext`) | choice | F2MD `veremiConf` constants [repo `f2md.md` §4.2] |
| `messages.rate_hz` (exists) | unset | Hz | VeReMi Extension 1 Hz beaconing [repo `f2md.md` P10] |

**Tests.**

| Accept | Test | Injected fault |
|---|---|---|
| Message counts and attacker share match the published description within tolerance | `tests/veremi_replica.rs`: a 120 s slice gives a message rate per vehicle of 1 Hz ±5 %, and the attacker share of senders is 0.30 ± 2σ binomial (needs `f2md-19`; until then, run with a legacy family and mark it so) | attacker share drawn per message, not per vehicle: red |
| Ours and the published numbers side by side, per attack | the table generator is checked against the export: its row count equals the attack ids present | n/a |
| Baseline trains and scores reproducibly | the recipe run twice gives an identical model hash and metrics; G-ML unchanged | seed taken from the clock: red |
| GNSS preset | `gnss::tests::veremi_ext_confidences_sit_in_their_ranges` | U(3,5) drawn as U(0,5): red |

**Goldens.** None. G-ML and G-LEG must stay unchanged, because the export path is opt-in.

**Pieces.**
- **P24.1** (3 h, no cargo): the LuST repository licence; cut the subsection by bbox
  (netconvert in the venv, outside the build lock, under `runtiny.sh`); extract the ICC
  2020 numbers into the comparison table with page references.
- **P24.2** (3 h): the replica scenario and the `veremi-ext` GNSS preset with its test;
  a 120 s smoke run under `runtiny.sh`.
- **P24.3** (3 h): the ML recipe on a legacy-attack export (XGBoost and MLP, fixed
  seeds), the reproducibility check, and the guide.
- **P24.4** (after WP-07): 30 % attackers on `f2md-19`, `tests/veremi_replica.rs` in
  full.
- **P24.5** (after WP-07): both 2 h windows (city-scale; run alone, at night) and the
  final table.

---

## Sources to cache before the matching pieces

| For | Source | Access |
|---|---|---|
| WP-16 | IEEE 1609.4-2016 §6.2.5 (intervals, guard) | paywalled; constants stay UNVERIFIED until purchased |
| WP-16 | Eichler, "Performance evaluation of the IEEE 802.11p WAVE communication standard", IEEE VTC 2007-Fall | IEEE |
| WP-17 | FHWA Signal Timing Manual 2nd ed., NCHRP Report 812 (2015) | public (TRB); read 2026-10-27 (P17.1 note) |
| WP-17 | NEMA TS 2-2016 | paywalled; STM is the working text |
| WP-17 | MUTCD 2009 ch. 4D, 4E (§4E.06 pedestrian intervals) | public (FHWA) |
| WP-17 | ITE guideline on change and clearance intervals (2020) | paywalled; secondary source allowed with a flag |
| WP-17, 23 | SUMO documentation: "Traffic Lights", "NEMA" pages | public (behaviour and names only) |
| WP-18 | Boban, Barros, Tonguz, IEEE TVT 63(9) 2014, doi 10.1109/TVT.2014.2317803 | IEEE; authors' copy if permitted |
| WP-18 | ITU-R P.833 (latest edition) | public (ITU); P.833-10 (09/2021) read 2026-10-27 (P18.1 note) |
| WP-19 | IEEE 1609.3 ASN.1 `wsa.asn`, `wee.asn` (TCI_ASN1 copy, R4 §A.8) | cached 2026-10-27 in `third_party/asn1/ieee1609.3/` (no licence stated upstream; see its PROVENANCE.md) |
| WP-19 | IEEE 1609.12 PSID allocations | public: IEEE RA PSID registry page, read 2026-10-27 (P19.1 note) |
| WP-20 | Uniform Vehicle Code §11-405, §11-106 | NCUTLO text; check access |
| WP-20 | StVO §11(2), §38(1) | public (gesetze-im-internet.de) |
| WP-20 | SAE J2735 ASN.1 `SpecialVehicleExtensions`, `EmergencyDetails` | as the existing J2735 codec's source |
| WP-22 | AASHTO Green Book (truck speed-distance curves, 120 kg/kW truck) | paywalled; edition and figure to pin |
| WP-22 | HCM 6th/7th ed. (truck weight-to-power; grade PCE) | paywalled |
| WP-24 | Kamel et al., "VeReMi Extension", IEEE ICC 2020 | IEEE; arXiv copy if one exists |
| WP-24 | LuST scenario repository licence | public |
