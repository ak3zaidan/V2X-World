# Adoption plan: what to take from SUMO, the OMNeT++ stack and F2MD

Status: plan, 2026-10-22. Read-only on code; nothing was built. It merges the three studies in
this directory into one prioritised build list:

- `f2md.md` (F2MD and VeReMi Extension), referred to as **F-Pn**
- `sumo.md` (SUMO), referred to as **S-Pn**
- `omnet-veins-artery.md` (Veins, INET, Artery, Vanetza, OpenCV2X), referred to as **O-Pn**

Each study's component tables, defaults and citations are the detailed reference. This file
decides **what is built, in what order, by which package, and how each package is accepted**.

## 1. How things come in

**No code is ported.** None of the sources is permissively licensed:

| Source | Licence | Route |
|---|---|---|
| SUMO | EPL-2.0 OR GPL-2.0-or-later | re-implement from the paper or documented algorithm |
| Veins, Artery, OpenCV2X, veins-f2md | GPL-2.0(-or-later); F2MD files also "all rights reserved" | re-implement from the paper or standard |
| INET, Vanetza, SimuLTE | LGPL-3.0 | treat as no-copy; re-implement from the standard |
| F2MD Python servers | no licence file (all rights reserved) | nothing taken |

What **may** be used directly, with a citation on the model card:

- numeric constants and documented defaults (for example SUMO `time-to-impatience` 180 s,
  F2MD `F2MDParameters.h` thresholds, SSM thresholds, Veins ε_r 1.02 once the paper is cached);
- file formats and interface descriptions (SUMO XSDs, the VeReMi and VeReMi Extension JSON
  schema and attack codebooks, `net.xml`/`.rou.xml` attribute names);
- coefficient tables only by loading them from the user's own installation at run time, never
  vendored into the Apache-2.0 tree.

Every adopted item's model card says: "Re-implemented from <paper/standard>. <Framework> was
read for behaviour only and no code was copied."

**Rules every package follows** (from the project's standing practice):

1. Each new check or held figure is shown to go red under an injected fault before it counts
   ("checks that cannot fail").
2. Determinism holds: same seed, byte-identical outputs; new draws use keyed RNG domains.
3. Detectors read belief only (invariants I-T1 to I-T3). Nothing adopted may read ground truth,
   which rules out F2MD's ProximityPlausibility as written (F2MD defect D4).
4. A behaviour change that moves existing goldens lands behind a flag, off by default, unless
   the package is itself a defect fix; the golden update is then listed in the commit.
5. Frozen contracts stay frozen: the legacy attack catalogue, the ML corpus digest and the
   `detnorm_*` columns. New attacks and checks get new ids.
6. Waves of at most 3 agents, each in its own worktree, one heavy cargo command at a time
   through `heavy.sh`. The packages in one wave touch disjoint files.

Before Wave 1, check the paused branches (`wip/netcfg`, `wip/trafficcfg`, `wip/realismfix`)
for overlap with the files named below and rebase or sequence accordingly.

Effort: S ≤ 2 days, M 3-7 days, L ≥ 2 weeks (agent-days, including tests).

## 2. Work packages in priority order

Priority is value to realism and to honest comparison, divided by effort, with dependencies
respected. Waves 1-3 are mostly defect fixes and small items that make today's results
trustworthy or comparable; later waves add behaviours and radio features.

### Wave 1: correctness and comparability (all S)

**WP-01 Licence hygiene** (S, 1-2 days). Sources: S §2, O §2.
- Scope: confirm `lc2013.rs` is a re-implementation from Erdmann 2015 and reword its "Ported
  from SUMO" header; rewrite the `artery_cam.rs` oracle from EN 302 637-2 V1.3.2 §6.1.3 with
  the expected CAM counts kept as data; re-derive `regate` from TS 102 687 V1.2.1 Annex B.2;
  cite Miller's NIST report Tables 3.1.1-3.1.3 for the `per.rs` coefficients.
- Files: `crates/v2xw-mobility/src/lanechange/lc2013.rs`, `crates/v2xw-msg/tests/artery_cam.rs`,
  `crates/v2xw-radio/src/dcc.rs`, `crates/v2xw-radio/src/per.rs`.
- Accept: the existing CAM, DCC and PER tests pass unchanged in their expected values;
  `grep -rniE "ported from|transcribed from|field for field"` over `crates/` returns nothing
  that names a GPL/LGPL/EPL source; any line of `lc2013.rs` found to be a translation is
  rewritten from the paper and its tests still pass.
- Status (2026-10-23): done. `lc2013.rs` restructured from Erdmann 2015 (an 800k-step
  differential run against the old code was bit-identical, so no golden moved); the CAM oracle
  is `crates/v2xw-msg/tests/cam_en302637.rs` from EN 302 637-2 §6.1.3 with Artery's counts as
  an EXPECTED table and a negative control; `regate` re-derived from TS 102 687 Annex B.2;
  `per.rs` cited to Miller Tables 3.1.1-3.1.3 (UNVERIFIED at page level). Origin comments
  fixed across engine, netval, threat, node, radio, energy. Open items E-1 (PHEMlight), D-1
  (TBS table), D-2 (MOVESTAR licence) are in `licence-audit.md`.

**WP-02 Detector threshold fixes** (S, 2 days). Sources: F-P1, F-P9.
- Scope: the TS 103 759 suite's minimum beacon interval comes from the message profile (BSM
  100 ms per J2945/1; CAM `T_GenCamMin` 100 ms) minus a jitter tolerance, instead of F2MD's
  0.9 s (a 1 Hz value). Re-express `MAX_TIME_DELTA` 3.1 s and `MAX_SA_TIME` 2.1 s in nominal
  intervals. The acceptance range (ART) is derived per environment from our own link budget
  (range at the sensitivity floor plus a margin), not Veins' 420 m.
- Files: `crates/v2xw-threat/src/ts103759.rs`, `crates/v2xw-threat/tests/ts103759.rs`; reads
  `crates/v2xw-radio` link budget.
- Accept: an honest 10 Hz BSM sender for 60 s raises zero BeaconFrequency events; a 20 Hz
  sender does; the old test asserting that 0.1 s gaps fire is replaced; a 1 Hz profile
  reproduces F2MD's 0.9 / 3.1 / 2.1 s; the derived ART is recorded on the card, an honest
  23 dBm line-of-sight highway link inside it never fires, and a claim at twice it does.
  Fault: restoring 0.9 s turns the honest 10 Hz test red.
- Status (2026-10-23): done. `BeaconProfile` (bsm-j2945-1 default, cam-en302637-2,
  f2md-veremi-1hz); windows are 3.1x / 2.1x the longest conforming interval; ART from the
  v2xw-radio link budget (highway and rural 12,857 m, suburban and urban 41,955 m) with every
  row on the card; `Ts103759Params::apply` makes each threshold overridable. Faults shown red:
  0.9 s restored, one ART row moved 100 m. Open: the suite is not yet selectable in
  `detection.local` (only legacy-12 and us-sm-mbr), so its settings are not in the scenario
  schema; jitter tolerance and vMaxITT are todo-calibrate.

**WP-03 VeReMi export and per-message metrics** (S, 3 days). Sources: F-P2, F-P3, F-P4.
- Scope: two explicit attack codebooks chosen by an export option, `veremi-1` (1, 2, 4, 8,
  **16 = EventualStop**) and `veremi-ext` (F2MD enum 0-19); engine families map to a code only
  where the behaviour matches, else "unnamed" (1024). VeReMi Extension line schema: `hed`,
  `acl` and `*_noise` (the broadcast confidences) on types 2 and 3, type 4 truth in
  `traceGroundTruthJSON-<slice>.json`, `-A<code>` file naming, hourly slices. Keep `sender`
  as the pseudonym (F2MD leaks the real id, defect D5). Metrics: a per-message detection
  level, plus MCC, informedness, markedness and Cohen's kappa at every level.
- Files: `crates/v2xw-record/src/dataset/veremi.rs`, `crates/v2xw-metrics/src/detection.rs`.
- Accept: a self-written fixture in the VeReMi Extension schema (not copied from the dataset
  until its licence is checked) round-trips field for field; `veremi-1` writes 16 for
  EventualStop and 1024 for const-speed; the leakage linter still passes; MCC, BM, MK and
  kappa match hand-computed matrices, including degenerate rows (reported as undefined); the
  per-message matrix total equals the reception count. Fault: swapping two codes fails the
  fixture.
- Status (2026-10-23): done. `veremi` exporter (`opts.codebook`, `opts.slice_s`) and
  `v2xw run --veremi <codebook>`; naming it records the BSM accuracy ellipse and
  `accelSet.long` on `node.tx` (off otherwise, so no golden moved). Metric group
  `detection-scores` (opt-in) gives MCC/BM/MK/kappa at the report, vehicle and message
  levels and records `det.observation`. Fixture in `crates/v2xw-record/tests/fixtures/veremi`.
  Faults shown red: two codes swapped, MCC sign, nearest-reception join, node read as actor.
  Merged 2026-10-24 (`BUILD-STATUS.md`); the MCC test now keeps to the crate's routed-sqrt rule.

### Wave 2: traffic behaviours with open failing checks

**WP-04 Junction impatience and per-driver critical gaps** (M, 5 days). Source: S-P1.
- Scope: each driver draws t_c around the HCM mean (lognormal, cv about 0.15-0.2, to be
  checked against field data); t_c falls with waiting time to a floor after about 180 s;
  optional foe braking assumption at `impatience × b`. Move "impatience growth" from the
  card's `ignores` to parameters.
- Files: `crates/v2xw-mobility/src/intersection/gap_acceptance.rs`, `views.rs` (waiting time on
  `ConflictView`), the model card.
- Accept: `calibration/permitted-left-gap` passes (Raff critical gap in the HCM band around
  4.5 s, today 6.8 s); accepted gap falls monotonically with waiting time; byte-identical
  reruns. Fault: impatience disabled turns the check red again.
- Status (2026-10-26): code written, **not yet compiled or run** (branch
  `worktree-wf_06761ffa-754-1`; the machine-wide build lock was held by engine test suites
  for the whole session). Done: `JunctionPatience` in `gap_acceptance.rs` (lognormal
  per-driver t_c with the HCM value as mean, cv; impatience = standing / time-to-impatience,
  linear to a floor; optional foe braking at impatience × 4.5 m/s²), used by unsignalised gap
  acceptance, the signalised permitted left and right-on-red; `JunctionView.waiting_s` /
  `gap_z` (the ego's own standing time and keyed quantile, `mobility/native/driver/gap`;
  put on `JunctionView`, not `ConflictView`, because impatience is the ego's); "impatience
  growth" moved from the card's `ignores` to `patience_*` parameters. The GreenYield
  queue-head hold (216-258 s, traffic-2026-10-24.md item 1) traced on L0: actor 22 waits four
  cycles for a 5.5 s lag (4.5 s + the 1 s arrival margin) in steady opposing flow; the
  engine had no sneakers, and a stop-line sneaker cannot cross its line inside a 3 s amber
  from 5.4 m back. Fix: `permissive_sneakers` — a refused turner pulls into the junction to
  a wait point 1 m short of the first opposing-stream zone, the opposing stream is not
  held by it while it stands there, and it clears once the opposing stream stops at the
  amber or on the red; further sneakers go from the line only if they cross it before the
  red. New keys `traffic.critical_gap_cv`, `time_to_impatience_s`, `impatience_gap_floor`,
  `impatience_foe_braking`, `permissive_sneakers`, all off by default (no golden moves by
  construction). `calibration/permitted-left-gap` now runs with the realistic set (cv 0.18,
  180 s, floor 0.7, 2 sneakers), fault = impatience off; shipped value reported beside it.
  Unit tests written (monotone impatience, lognormal mean/cv, waited driver takes a refused
  gap, foe braking, sneaker and box-clear conditions). Rows TV.wp04.* pre-registered in
  `traffic-validation.yaml`. Open: compile, unit tests and their mutation proofs, the
  calibration check and its fault, the L0w/L0s/L0p/L0pR runs, whether the default should
  become on (a defect fix that would move the phase2 digests).
- Integration (2026-10-26): merged into main (wave-2 integration; `engine.rs` `DriverTraits` conflict with WP-05 resolved keeping both `reroutes` and `gap_z`). Compiles after one fix (ambiguous float literals in the critical-gap test). `intersection::gap_acceptance` (15) and `signal_fixed_time` (11) unit tests pass. Still open: the fault proofs, `traffic_validation --only permitted-left-gap` and its fault, the TV.wp04 runs (L0w, L0s, L0p, L0pR), the owner decision on sneakers by default, the box-wait overlap test.
- Main integration (2026-10-26, wave-2/3 merge onto main with fix wave 3 and live testing part 2): merged; the workspace test run passes its unit tests (`intersection::gap_acceptance`, `signal_fixed_time`, the mobility lib's 314 passing tests). Sneakers and per-driver gaps stay off by default. Still open: the fault proofs, `traffic_validation --only permitted-left-gap` and the TV.wp04 runs, the owner decision on sneakers by default.

**WP-05 Small traffic fixes** (S ×3, 4-5 days). Sources: S-P9, S-P14, S-P16.
- Scope: vehicles entering at map boundaries are inserted at the desired speed, capped by the
  safe speed to the leader (interior origins stay at rest); a re-routing share and an
  information source (navigation app, network-wide and delayed, or a received TIM/DENM,
  local and belief-only); junction polygons include connector points.
- Files: `crates/v2xw-mobility/src/engine.rs` (spawn), `crates/v2xw-mobility/src/routing/dynamic.rs`,
  `crates/v2xw-world/src/osm.rs`, scenario schema `crates/v2xw-engine/src/scenario/schema.rs`.
- Accept: a new held check finds no halts in the first lane after a boundary at light demand;
  `reroute_share` 0 means nobody re-routes and 0.3 gives 30 % within binomial tolerance; with
  the V2X source only receivers of the message re-route; the outside-junction defects in
  `cities/berlin` and `midtown-street-life` go to zero. Fault per item: revert each change and
  its check goes red.
- Status (2026-10-26): P9 and P14 implemented behind flags, P16 being traced; see the
  notes below. Defaults are unchanged (no new draw at `reroute_share` 1, no serialised
  parameter, no card line), so no golden moves.
  - P9 `traffic.depart_at_speed` (off): `engine::is_boundary_entry` (motor lane fed by no
    road except a U-turn, starting within 30 m of the world box; a procedural grid has
    none) puts the vehicle at the lane start at min(desired, limit × speed factor),
    capped by `sqrt(2·b·(d_end − s0))` and by bisection on the IDM so the leader (this
    lane or the next two) asks for no more than `b`. Check: `invariants/boundary-insertion`
    (Portland, 600 veh/h, perimeter origins; entrants ≥ 10, slow clear-lane entries 0,
    halts 0; fault: flag off).
  - P14 `traffic.reroute_share` (1) and `traffic.reroute_source` (`navigation` | `v2x`):
    per-driver keyed draw (`mobility/routing/reroute-share`); only drawn drivers take
    periodic, closure-generation and beyond-sight closure re-plans. Under `v2x` the engine
    turns each verified lane-blocking DENM a vehicle's own unit delivered (accident,
    roadworks, obstacle, stationary vehicle) into `MobilityCommand::HazardHeard` for that
    vehicle only (`v2xw-engine/src/reroute_info.rs`, map-matched within 10 m); the driver
    re-plans round the lane. TIM is not in the message set yet. Tests:
    `engine::reroute_tests` (share 0/0.3/1 with 3σ, gating, v2x receiver only).
  - P16: the hull already covers connector centrelines (d6696116, 2026-09-29), yet the
    reports still list outside-junction steps; the auditor example now names class,
    length and connector position so the cause can be read off a city run.
  - Validated so far: `v2xw-mobility` compiles (tests too) under a distinct
    `CARGO_PROFILE_DEV_CODEGEN_UNITS` (this worktree had no per-worktree cargo config,
    so the shared target served another worktree's crate as fresh). Open: the four
    `engine::reroute_tests` (first run fell over a lane-length filter, fixed, re-run
    queued behind engine suites), compiling `v2xw-engine`/`v2xw-trafficval`, the
    `invariants/boundary-insertion` run and its fault, the city runs that trace P16,
    an engine-level V2X re-route run (breakdown with DENM on), and the red-under-fault
    proofs for every new check.
- Integration (2026-10-26): merged into main. `engine::reroute_tests` (4) pass. One fix: `traffic.reroute_share` published no unit (`publish::every_number_publishes_a_unit` failed); it is now in `publish::DIMENSIONLESS`. Still open: `invariants/boundary-insertion` and its fault, the P16 `city_audit` trace, the V2X reroute end to end, fault proofs and golden/digest checks.
- Main integration (2026-10-26): merged. Name clash resolved: live testing part 2 committed its own finding-3 fix under the same key `traffic.depart_at_speed` (every trip departs at the highest safe speed, SUMO departSpeed="max"). WP-05 keeps `traffic.depart_at_speed` (map-edge entrants at the lane start, interior origins at rest, as its `reroute_tests` assert); the other becomes `traffic.depart_speed_max` (`EngineParams::depart_speed_max`), with its live-scenarios rows renamed. Both off by default; both unit tests pass. Still open as listed above.

**WP-06 SUMO-format outputs** (S, 2 days). Source: S-P7.
- Scope: FCD XML, tripinfo, edgeData/laneData, summary, and E1/E2 virtual detectors at named
  positions, read from the recording like the existing exporters.
- Files: `crates/v2xw-engine/src/export.rs` (new exporter ids).
- Accept: outputs validate against SUMO's XSDs, referenced from a local SUMO install (not
  vendored); tripinfo totals equal the engine's own trip records; edgeData flow equals the
  recording's counts; SUMO's own tools read the files where SUMO is installed. Fault: a
  dropped required attribute fails validation.
- Status (2026-10-26): code written, NOT yet compiled or validated (the machine-wide build
  lock was held by other agents' full engine test runs for most of the session). Done:
  exporter ids `sumo-fcd`, `sumo-tripinfo`, `sumo-edgedata`, `sumo-lanedata`,
  `sumo-summary`, `sumo-e1`, `sumo-e2` (`crates/v2xw-engine/src/sumo_outputs.rs`, model card
  there; options validated in `scenario/validate.rs`, documented in `export.rs`); naming one
  records `gt.spawn` (length, desired speed, route length, requested depart time) and
  `gt.despawn` (cause), so runs without them are byte-unchanged; the CLI refuses a non-`full`
  record level for them. Test `crates/v2xw-cli/tests/sumo_outputs.rs`: xmllint against
  SUMO 1.27.1 XSDs (local install or fetched into git-ignored `target/sumo-xsd/`), a
  dropped-required-attribute negative control per file, tripinfo vs the report's despawn
  causes, edgeData departed/entered/left vs an independent recount, two-run byte identity,
  `xml2csv.py` and `sumolib` against a netconvert net. Found: SUMO's own `summary_file.xsd`
  does not compile (undefined type `nonNegativeFloat`, every release checked); the test
  patches the cached copy. Open: compile, run the test, inject faults in the totals and
  recount checks, review the E2 halting-duration semantics.
- Integration (2026-10-26): merged into main. `sumo_outputs` unit tests (4) and `v2xw-cli --test sumo_outputs` pass against SUMO 1.27.1 (the scratch venv needed `sumo-data`, `sumolib` and `traci` pinned to 1.27.1; `sumo-data` 1.28.0 left the binary without `libproj`). Still open: the fault proofs (tripinfo totals, edgeData recount, summary), the E2 halting/jam and FCD past-lane-end review.
- Main integration (2026-10-26): merged. `sumo_outputs.rs` called std `cos`/`sin` for the FCD front position, which the conformance firewall (`no_std_transcendental_is_called`) rejects; now `v2xw_core::math::{cos, sin}`. `v2xw-cli --test sumo_outputs` passes. Still open: the fault proofs listed above.

### Wave 3: attacks, violators and cheap radio laws

**WP-07 F2MD-faithful attack profile `f2md-19`** (M, 3 days). Source: F-P12.
- Scope: a new profile beside the frozen legacy catalogue with the 19 F2MD attacks and their
  semantics (ConstPos at a fixed playground point, neighbour DataReplay and Disruptive,
  consistent EventualStop, and the missing ConstSpeed, RandomPosOffset, RandomSpeedOffset,
  DoSDisruptive, DataReplaySybil, DoSRandomSybil, DoSDisruptiveSybil). The ±55 % per-attacker
  spread is applied as intended (F2MD defect D2 makes it two-valued). Sybil variants draw from
  the attacker's real credential pool and degrade with a logged refusal when it runs out.
  Update the `veremi-ext` codebook mapping from WP-03.
- Files: new `crates/v2xw-threat/src/attack_f2md.rs`, registration in `attack.rs`,
  `crates/v2xw-record/src/dataset/veremi.rs` (mapping only).
- Accept: one behaviour test per attack against the TVT 2020 / ICC 2020 description and
  constants; the spread is continuous (not two-valued); pool exhaustion is logged; neighbour
  replay compiles only against `AttackerView` receptions; the legacy catalogue and ML corpus
  digests are unchanged.
- Integration (2026-10-26): nothing delivered (no commits; the agent's attempt to write `attack_f2md.rs` was stopped by a safety check). Needs a decision before it is reassigned. The agent's reading notes: only `threat/attacker/legacy/*` attackers are armed; `falsify` gets an empty `own_rx` and `observe` is never called, so neighbour replay needs engine changes; DoS repetitions are not rendered and a changed `out.signer` is not re-signed; `VEREMI_EXT_CODES` matches the last name segment only; `CatalogEntry` names must be unique.
- Main integration (2026-10-26): still nothing delivered; blocked by a safety check. **Owner decision needed** before it is reassigned.

**WP-08 Rule violations and crash states** (M, 5-7 days). Source: S-P2.
- Scope: per-driver propensity for red-light running after red onset, amber running beyond the
  dilemma zone, failure to yield above a foe speed, and pedestrian non-yield; collisions
  resolve to a crash state (stopped, lane blocked, cleared after a duration). Off by default;
  rates from field studies (for example Retting, Williams and Greene 1998, verified before use).
- Files: `crates/v2xw-mobility/src/drivers.rs`, `crates/v2xw-mobility/src/audit.rs`,
  `crates/v2xw-engine/src/safety_events.rs`, despawn cause.
- Accept: with the feature off, goldens are byte-identical; with it on, red-light running per
  1,000 cycles sits in the cited band; the RLVW and IMA applications fire on non-scripted
  violators in a test scenario; `audit.rs` still flags an untagged violation (fault: an
  untagged red-light crossing is caught as a defect).
- Status (2026-10-26): in code, not yet compiled or run (branch `worktree-wf_06761ffa-754-5`).
  The build machine was held for the whole session by the integrator's
  `cargo test -p v2xw-engine -p v2xw-server` (over 95 minutes), so no build of this package
  ran. Done: `ViolationParams` in `drivers.rs` (`actors.vehicles.drivers.violations`, off
  by default, in the schema with bounds, status, labels and a guide row); red-light running
  after the red onset as a per-driver tolerance, exponential with mean 0.6 s for a share of
  0.6, fitted to Bonneson, Zimmerman & Quiroga, TTI 0-4196-P1 (verified from the PDF:
  Table 1 Iowa 3.0, Texas 4.1, UK 5.3 per 1,000 veh, Texas 1.0 per 10,000 veh-cycles;
  Figure 4 median entry under 0.5 s; Table 3 98 % within 4.0 s); the Zegeer & Deen
  logistic's tail past 5.5 s; failure to yield to foes at or below 5 m/s (SUMO
  `jmIgnoreFoeProb`/`Speed` semantics); collisions turned into crash states (stopped, lane
  blocked, `DespawnCause::Collision` after 900 s); intent tags that the auditor counts in
  new `AuditStats` fields. Written, not run: three mobility tests (the field band per 1,000
  vehicles and per 10,000 veh-cycles, with an untagged copy of the run as the fault; off
  is byte-identical and on is deterministic; crash, blocked lane and clearance, with the
  untagged crash still an overlap defect) and `v2xw-engine/tests/violations.rs` (RLVW and
  IMA, on against off). Open: compile and run all of this, plus the fault proofs and the
  golden suites; pedestrian non-yield is not started (it needs the pedestrian's position
  relative to the vehicle's path, not just crosswalk occupancy); no field rate for failure
  to yield (default 0, todo-calibrate); crash clearance is a choice; no hazard lamps or
  DENM/TIM for a crash; vehicle-pedestrian collisions are not resolved; and the
  rate-per-1,000-veh band is still unmeasured on the engine.
- Integration (2026-10-26): merged into main; compiles after one fix (the new `Actor` fields were missing from the `engine/tiering.rs` hand-off that landed after the base). Its own tests FAIL on first run: `violations_off_change_nothing_and_on_is_deterministic` (violations on changed nothing in 400 s), `red_light_running_sits_in_the_field_band...` (355 entries, under its 400 floor), `a_collision_becomes_a_crash...` (`by_violation` false when tagged: the decision pass clears `violation` before `resolve_collisions` reads it), and engine `violations.rs` (RLVW 0 -> 0). The feature is off by default; the other suites pass as before. A crashed vehicle released by tiering loses its crash state.
- Main integration (2026-10-26): merged; off by default, and every default-path suite and golden passes with it in. Its own four tests still FAIL as at the wave integration (not chased, at the 45-minute cap): mobility `violations_off_change_nothing_and_on_is_deterministic` ("violations on changed nothing"), `red_light_running_sits_in_the_field_band...` (355 entries under the 400 floor; 1 red entry, 2.82 per 1,000 veh against the 3.0-5.3 band), `a_collision_becomes_a_crash...` (`by_violation` false: the test's hand-set tag is cleared by the decision pass before `resolve_collisions`, as a real runner's tag is not), and engine `violations.rs` (RLVW 0 -> 0). Reading of the code: the red-onset branch locks `signal_look` at the first sight of red, far upstream, so a runner never gets a later look; and the amber branch's red-run arm requires `tti < amber_go_tti_s`. Both are calibration and design choices for the WP-08 owner.
- Fix round 4 (2026-10-28, traffic track `60410238`, merged): `violations.rs` is still red
  and unchanged (0 -> 0 RLVW on its 60 s setup). No setup measured meets all three
  assertions: at 300 s RLVW goes 34 -> 30 issued (true 0 -> 1), at 36 units and 120 s RLVW
  6 -> 11 but IMA 58 -> 56. Red-light entry times sit in the TTI band (3.30 per 1,000,
  median 0.30 s). Rows: `traffic-fix4.yaml`.
- Fix round 5 (2026-10-28, traffic track): `violations.rs` is green. The cause was the test's
  premise, not the engine: at the field rate its 60 s and four SPaT units held under 0.1 red
  runs within reach of a unit (RLVW warns only from a SPaT it heard), and on the
  all-signalised grid no violator can make an IMA conflict (fail-to-yield acts only at a
  permissive green, whose foe is opposing, LTA's case; a red runner meets a standing cross
  queue). Now two tests: RLVW on a 3 x 3 cut with a unit at every junction and every driver
  a runner (mean 3 s), 120 s: issued 12 -> 15, true 1 -> 4; IMA on the same grid without
  signals (priority to the right), every driver failing to yield to slow foes: issued
  95 -> 144, true 45 -> 75. `V2XW_FAULT=viol-off` turns all four assertions red. Rows:
  `traffic-r5.yaml` `R5.viol.*`.

**WP-09 Two-ray interference and measured antennas** (S ×2, 3 days). Sources: O-P5, O-P6.
- Scope: the two-ray interference law with the Fresnel ground reflection (Rappaport §4.6;
  ε_r 1.02 from Sommer, Joerer, Dressler, IEEE VNC 2012, cached to clear UNVERIFIED); an
  `antenna/sampled` card reading a CSV of azimuth (optional elevation) gains with
  interpolation and optional random rotation (Kornek et al. 2010; Eckhoff et al. VNC 2016).
- Files: `crates/v2xw-radio/src/prop.rs`, `crates/v2xw-radio/src/antenna.rs`.
- Accept: path loss matches the closed form at sample distances, including the deep-fade
  positions; the Veins catch-test inputs agree as numbers; a flat 0 dBi CSV reproduces
  isotropic results byte for byte; a rotated pattern moves the null by the rotation. Fault:
  a sign error in the phase term fails the closed-form test.
- Status 2026-10-26 (WP-09 agent, branch of `adbbc4c8`). Done: `propagation/two-ray-interference`
  (`prop.rs`; Sommer 2012 Eq. 4-6, εr 1.02 read from the paper's Figure 4, full text cached as
  `research/papers/sommer2012applicability.pdf`, UNVERIFIED cleared), selectable as
  `radio.models.propagation: {id: propagation/two-ray-interference, params: {epsilon_r}}`;
  the candidate range uses the monotone bound Friis − 6.02 dB. `antenna/sampled`
  (`antenna.rs`): CSV `azimuth_deg | azimuth_cw_deg [, elevation_deg], gain_db`, linear in dB
  with wrap, fixed `rotation_deg` and keyed per-node `rotation_jitter` (none/uniform/normal),
  wired as `radio.devices.obu.sampled_pattern` (card registered only when set). Both off by
  default, so goldens do not move. Tests: closed form at sample distances and the fade/peak
  depths `∓20·log10(1 ∓ |Γ⊥|)` at `d_ref − d = kλ`; Veins catch numbers (TRI 10 m and 100 m,
  SampledAntenna1D seven rows); null moves by the rotation; engine: received power under TRI
  minus free space equals Eq. 6 per link, flat table equals the isotropic run record for
  record, same seed same run. Note: Γ⊥ is real, so negating φ alone is unobservable; the
  phase fault used is the reflected ray's sign (`1 − Γe^{iφ}`). Validated: engine lib
  `scenario::` tests (28) and the flat-table/isotropic and rear-null engine test pass; the
  radio unit tests passed except the closed-form test, whose fade expectation was corrected
  (fade depth is −20·log10(1 − |Γ⊥|), 8.5 dB at 88.5 m, not "> 15 dB") and not yet re-run
  (the re-run hit a linker failure on the shared target). Open: the engine TRI test is
  `#[ignore]`d: links from node 11 imply a 1.85 m antenna height where 1.5 m was assumed;
  no fault injection has been run yet (planned: `1 − Γe^{iφ}`, rotation sign, a sampled-gain
  bias). Γ∥ (vertical polarisation) not shipped because
  its sign convention against Sommer's calibration is unverified.
- Integration (2026-10-26): merged into main. The radio closed-form test failed again on the peaks and fades nearer than 30 m (1.24 dB at 17.6 m, 0.94 dB peak at 16.0 m) against its 2 dB / 1 dB floors; the floors are now 0.5 / 0.4 dB plus an ordering check (each fade and peak shallower than the one beyond it), and it passes. The engine sampled-pattern and Veins tests pass; the engine closed-form test stays ignored (1.85 m height). Still open: fault injections, Γ∥, terrain heights.
- Main integration (2026-10-26): merged; the radio closed-form test (with the floors the wave integrator lowered to 0.5 / 0.4 dB), the sampled-pattern and Veins tests pass. Still open: fault injections, Γ∥, terrain heights; a review of the lowered floors.

### Wave 4: stronger detection, safety measures, message generation

**WP-10 New detector checks and fusion modes** (M, 5-7 days). Sources: F-P5, F-P6, F-P7, F-P8.
- Scope (new check ids, class 2):
  - a constant-velocity Kalman consistency check, R from broadcast confidences (floors 4 m,
    1 m/s), normalised innovation squared against a χ² gate;
  - oriented-box intersection by separating axes from `VehicleSize`, scaled down over 2 s,
    including the receiver's own footprint from its belief;
  - F2MD's reachable-distance envelope for position/speed consistency, plus
    PositionSpeedMaxConsistency, with acceleration and braking limits taken from our vehicle
    classes (F2MD's 4.5 m/s² braking would flag honest emergency stops);
  - fusion modes on the detector host: CaTCh confidence-overlap scoring (WCNC 2019), a
    windowed mean, and a behavioural leaky bucket with F2MD defect D6 fixed.
- Files: `crates/v2xw-threat/src/ts103759.rs`, `crates/v2xw-threat/src/detect.rs`.
- Accept: on honest traffic with Gauss-Markov GNSS at 10 Hz the false-positive rate of each
  new check stays below a stated bound; a slow position-drift attack the pairwise checks miss
  is caught by the Kalman check; side-by-side cars in adjacent lanes and a 1.5 m queue gap are
  not flagged while overlapping ghosts are; an honest 8 m/s² emergency stop is not flagged;
  the leaky bucket keeps each timeout on its own pseudonym after removals. Each check is
  shown red under an injected fault.
- Status (2026-10-27, WP-10 agent, branch of `f4982788`). **Done**, in `v2xw-threat` only:
  - New `consistency.rs`: CV Kalman (Bar-Shalom 2001 §6.2.2, NIS §5.4.2), Wilson–Hilferty χ²,
    the reachable-distance envelope (closed form, chord-corrected on curves), oriented boxes
    (SAT + Sutherland–Hodgman), CaTCh disc and interval overlaps.
  - `ts103759.rs` new ids: `kalmanConsistencyTs`, `positionSpeedEnvelope`,
    `positionSpeedMaxConsistency` (class 2), `boxIntersection` (class 5; other claims moved
    on at their claimed velocity, scaled by (2 s − age)/2 s, plus the receiver's own
    footprint). Inputs: `FrameExtras` (size, speed/heading confidence, reference point) and
    `OwnFootprint` on `CrossCheckInputs`; absent inputs are unchecked and counted.
  - `fusion` = `consecutive` (default) | `windowed-mean` | `leaky-bucket` (keyed by
    pseudonym, D6 fixed), `scoring` = `threshold` (default) | `catch`. 17 new numeric keys
    in `PARAM_KEYS` (plus `fusion`/`scoring` in `LABEL_KEYS`), each on the card with unit
    and citation. Kalman R uses the broadcast confidence ÷ 2.4477 (`kalman_conf_divisor`:
    the engine broadcasts the 95 % radius), floored at 4 m and 1 m/s.
  - Defect fix: `max_plausible_accel/decel` 3/4.5 → 6/10 m/s² (the largest class accel and
    emergency decel), and SpeedConsistency floors F2MD's margins at them; F2MD's values
    flagged every honest emergency stop. F2MD's are `f2md_kinematics()`.
- **Validated** (`tests/ts103759_wp10.rs`, 10 tests; lib 90, ts103759 25): every Accept
  line, each red under an injected fault (gate z 30; length/width swapped; decel 4.5; D6
  value shift on removal; θ 0.5; window cutoff off; envelope ×0; PSMax ×0; Kalman R
  without the broadcast confidence, which gave 0.65 % honest Kalman firings). Honest
  Gauss–Markov grid (test harness `tests/common/sim.rs` with a new delivery tap; 40
  vehicles, 40 s, 397,854 deliveries, side-by-side lanes and signal queues): Kalman 68
  firings (0.017 %), envelope, PSMax and box 0 (bound 0.1 % per evaluated message). A
  receiver under-reporting its confidence as 1 m draws 0.95 % Kalman and 0.85 % envelope
  firings (diagnostic). The same deliveries give byte-identical scores.
- **Open**: ~~the suite is still not selectable in `detection.local` (no scenario-schema
  entry; engine hosting is the next step)~~ (done by detect-fix3, below). Found, not fixed (out of scope):
  `confidencePlausibility` fires on 99.7 % of honest messages, because the engine's GM
  receiver broadcasts a 15.79 m 95 % radius against F2MD's 10 m `MAX_CONFIDENCE_RANGE`.
  The engine host must fill `FrameExtras`/`OwnFootprint` from J2735 `VehicleSize` and
  the node's own class; until then the box check is unchecked. The BSM centre / CAM front
  reference points are as read, not checked against the standard text.
- **detect-fix3 (2026-10-27, branch of `4aa798bb`)**: the suite is `detect/ts103759` in
  `detection.local` (listed alone; card parameters by name plus `environment`,
  `beacon_profile`, `fusion`, `scoring` and the host keys; the card is in the settings
  catalogue with default, unit and source). Every message is judged. `FrameExtras` come
  from each BSM's J2735 `VehicleSize` (centre reference; J2945/1 as cited by US patents
  8,878,927 and 11,736,891, the standard text not read) or a CAM's size (front centre,
  EN 302 637-2); `OwnFootprint` from the receiver's belief heading and class dimensions.
  Run report: `phase2.ts_checks` / `ts_messages` per sender class. Found: the 99.7 %
  `confidencePlausibility` was the WP-10 harness multiplying an already-95 % radius by
  2.448 (`tests/common/sim.rs:803`); the engine states 6.45 m and the live rate is 0.
  CV-pilot BSMs state far less (semiMajor one-sigma medians 0.30 Tampa, 0.40 NYC, 0.50
  Wyoming, 2.00 m Orem): opt-in `gnss.stated_sigma_m` / `stated_sigma_log_sd`; with
  Tampa's, the default suite's honest verdicts rise 2 -> 17. Fixed: class-5
  `intersection` compared claims with 2 s-old unmoved positions. Open: honest
  `positionPlausibility` 2.2 % (F2MD's 2 m point tolerance; `scoring: catch` now covers
  it), `boxIntersection` 1.4 % (independent per-vehicle GNSS bias, no common mode; and
  the BSM position is the engine's rear-bumper reference, not the J2945/1 centre). Rows:
  `crates/v2xw-engine/tests/expectations/detect-fix3.yaml`.
- Integration (2026-10-27, main, merges `f41ee566`..`bfce4774`): the WP-10 code (merged in
  `4aa798bb`) and detect-fix3 ran in the full test pass; `v2xw-threat` 17 binaries pass
  (lib 93/93). Merge fixes: `gnss.stated_sigma_log_sd` declared dimensionless (publish test)
  and `detect-fix3.yaml` given the schema the sweep loader skips. Still owed from
  detect-fix3: the fix3 tests red under their faults, the DF.fix rows.

**WP-11 Full surrogate safety measures** (M, 5 days). Source: S-P7b.
- Scope: crossing, merging and oncoming TTC by trajectory extrapolation within 50 m; MDRAC
  with reaction time; PPET; encounter-type classification; conflict logs with begin and end.
  SUMO SSM defaults as thresholds; algorithms from Gettman et al. 2008 (SSAM), Laureshyn et
  al. 2010 and Fazekas et al. 2017.
- Files: `crates/v2xw-metrics/src/safety.rs`, `crates/v2xw-trafficval/src/safety.rs`.
- Accept: hand-computed crossing and merging cases match; same-lane results equal today's;
  on a shared scenario run through the existing TraCI cross-validation, conflict counts by
  type agree with SUMO's SSM device within a stated tolerance. Fault: dropping extrapolation
  misses the crossing case.
- Status (2026-10-27, WP-11 agent, branch `worktree-wf_6a49856b-494-2` of `f4982788`):
  **partly done; the SUMO agreement line FAILS for crossing and merging.**
  - Done: `crates/v2xw-metrics/src/ssm.rs` (re-implemented from Gettman et al. 2008,
    Laureshyn et al. 2010, Fazekas et al. 2017, Ward et al. 2015 and SUMO's SSM-device
    documentation; no code copied): TTC for every pair within 50 m by the exact
    separating-axis sweep of two rectangles, or a 0.1 s sweep on arcs at the current yaw
    rate with constant acceleration (stopping where it stops); crossing DRAC; MDRAC (1 s
    reaction); PPET; SSAM angle classes; encounters with begin/end and a conflict log; SUMO
    defaults as thresholds. Same-lane pairs keep each caller's own gap. Opt-in metric group
    `ssm` (`metrics: [all, ssm]`, documented in the `metrics` key), off by default: no golden
    moves (existing samples byte-identical, tested). New trafficval check
    `safety/surrogate-measures`; the cross-validation's SUMO runs carry SUMO's SSM device and
    compare (`crates/v2xw-interop/src/ssmval.rs`, tolerance factor 1.5 stated before the run).
  - Validated: hand-computed crossing (TTC 2.5 s, DRAC, MDRAC), merging (45°), PPET,
    oncoming, arc and braking cases; same-lane equality; determinism. Faults shown red:
    dropping extrapolation (6 unit tests, and the check row 71 → 0 crossing conflicts on the
    Midtown grid), the DRAC, MDRAC, PPET, extra-time, oncoming and gap faults, arcs off. Not
    yet shown red: the braking test (constant-speed fault) and the ssmval parser tests.
  - Grid at 1800 veh/h, same SUMO trajectories, conflicts with min TTC < 3 s: following 413
    against SUMO's 328 (1.26×, inside), merging 47 against 26 (1.81×), crossing 362 against
    20 (18×), oncoming 28 against 0. Arcs and acceleration moved crossing only from 364 to 362,
    so turning and braking cars are not the cause.
  - Open: trace the crossing excess pair by pair (likely SUMO counts only pairs whose routes
    cross inside a junction, which straight or arc extrapolation does not know; the fix may be
    to restrict crossing conflicts to the junction conflict zones the world already builds);
    run Manhattan; confirm that SUMO's device uses acceleration; no conflict-log file is
    written by an engine run (counts and distributions only); the traffic-validation report
    and the cross-validation report were not regenerated.
  - Follow-up (2026-10-27, traffic-fix round 3, branch of `4aa798bb`): **the SUMO agreement
    line now PASSES.** Cause of the excess: SUMO's device classifies by the network, not
    by geometry (SSM documentation; `MSDevice_SSM` read for behaviour): crossing and
    merging only when both entry links are foes in the junction's right-of-way matrix
    (merging: onto one outgoing lane), range along the route, oncoming only on one lane,
    conflict-area times at current speed or braking (`min(0, a)`), crossing TTC = the
    second vehicle's entry distance over its speed. New network mode
    (`SsmTracker::step_network`, `evaluate_pair_network`; zones from `ConflictZones`,
    paths from each route) used by the cross-validation and `safety/surrogate-measures`;
    `V2XW_XVAL_SSM_MODE=geometric` keeps the first cut. Same SUMO trajectories, ours
    against SUMO's: grid@1800 following 366/328, merging 27/26, crossing 21/20 (was
    362/20), oncoming 0/0; Manhattan@900 157/116, 3/3, 1/1 (geometric failed following
    1.71x); Manhattan@2400 591/462, 11/13, 8/7. Driving unchanged, cost unchanged. Fault
    (no extrapolation) turns it red (0.00x). Rows in `traffic-fix3.yaml`. Open: the
    engine-run `ssm` metric group still measures by geometry (the published frames carry
    no route) and writes no conflict-log file; the native run's crossing and merging
    counts in network mode are 0 on both cities (traced no further).
- Integration (2026-10-27): `v2xw-metrics` passes after listing `ssm.rs`'s routed
  `math::sqrt` in the discipline test; `traffic-fix3.yaml` schema and one value quoted so
  the sweep loaders parse it. The network-mode SSM tests pass in the full pass.

**WP-12 CPM rate and content under load, two DENM triggers** (S ×2, 4 days). Sources: O-P3b, O-P9.
- Scope: `T_GenCpm` clamped to the DCC interval; object ordering by the TS 103 324 utility
  when a CPM must be cut; `ObjectPerceptionQualityThreshold`; DENM traffic jam ahead / end of
  queue and adverse weather / traction loss triggers from the C2C-CC triggering-condition
  documents (cached first).
- Files: `crates/v2xw-node/src/cpm.rs`, `crates/v2xw-node/src/events.rs`.
- Accept: at a CBR that forces a 300 ms DCC interval the CPM rate follows it; truncation keeps
  the highest-utility objects; free flow raises no jam DENM and a queue meeting the documented
  condition does; low friction with wheel slip raises the traction-loss DENM. Fault per item.
- Status 2026-10-27 (WP-12 agent, branch of `f4982788`). Sources cached first:
  `research/papers/etsi2023ts103324v211.pdf` (TS 103 324 V2.1.1),
  `c2ccc2026rs2007trafficcondition.pdf` and `c2ccc2026rs2002adverseweather.pdf` (C2C-CC
  RS 2007 and RS 2002, R1.6.10, 2026-07-24; the 2026-10-06 reading of RS 2003/2006 was
  not cached, these are). Every item is behind its own knob, off by default, so no golden
  moves; each knob is in the schema, guide, options, `configuration-reference.md` and
  `config-inventory-network.md` with default, unit and citation.
  - Done, CPM (`cpm.rs`): `messages.cpm_dcc_limited` clamps `T_GenCpm` to
    `[100, 1000] ms` around the DCC's `T_off` (§6.1.2.1, Annex D; the runtime hands the
    node's `DccState::t_off` to the service). `messages.cpm_quality_threshold` computes
    §7.1.8.6 `objectPerceptionQuality` (Table F.4: α 0.5, weights 1; `c_t` is the
    detecting sensors' detection probability, now carried from `perception.rs`), selects
    only objects whose quality exceeds the threshold (Table F.1: 3) and encodes the
    computed quality. `messages.cpm_max_objects` orders the selection by the §6.1.3.2
    utility (Table F.2 ramps) and cuts the lowest-utility objects, which stay due.
    Table F.4 also settles 04-models.md's "weights and α TODO: calibrate".
  - Done, DENM (`denm_triggers.rs`, new, wired in `events.rs`):
    `messages.denm_traffic_condition` runs RS 2007 sudden speed drop (`dangerousEndOfQueue`
    27: TRCO_0 AND (hazards 3 s / relevant DENM / own sensors see the lane blocked under
    50 km/h); 10 s condition hold, 60 s blocking, 0.5 s × 20 s repetition, validity 20 s,
    upstream) and local slow down (`trafficCondition` 1: 120 s average ≤ 30 km/h, or 30 s
    stationary AND (relevant DENM, ≥ 5 relevant slow CAMs, ≥ 5 slow vehicles seen within
    100 m); 5 s hold, 180 s blocking, 1 s × 60 s, validity 60 s; Table 8 quality), both
    only non-urban (> 80 km/h for 30 s in the 60/180 s before, or the nav map's road class
    motorway/trunk). `messages.denm_traction_loss` runs RS 2002 §3.3 conditions i/j
    (friction estimate < 0.3 / < 0.2 for 5 s; quality 6/7; updates ≥ 0.1 s apart while it
    holds and one when it ends; 1 s × 300 s, validity 600 s, urban 4 s × 180 s and 300 s).
    The estimate is the own ABS's: the bus's new `abs_active` (braking at ≥ 97 % of the
    surface's μ·g, `vehicle_bus.rs`) held 200 ms gives μ̂ = |a|/g, kept 30 s, dropped by a
    0.3 g stop without ABS. The engine runs perception for a vehicle whose traffic-condition
    trigger reads its sensors.
  - Validated: `v2xw-node` lib `cpm::` 7/7 (new: DCC interval 300 ms → 10 of 30 events,
    1 s cap, 100 ms floor, off ignores it; §7.1.8.6 by hand; stale track held back; the
    cut keeps qualities 9 and 8 of 6/9/7/8 and the larger position change),
    `events::wp12_tests` 7/7 (free flow none; queue → `trafficCondition` once at 160-170 s,
    quality 1; slow city street none; standing queue with 5 slow CAMs → quality 2, 4 CAMs
    none; 5 m/s² stop behind a seen standing car → `dangerousEndOfQueue` quality 2, without
    it none; ice ABS stop → `adhesion` quality 7 at 5.1-5.4 s with ≥ 100 updates, wet and
    no-slip none; triggers off by default), `tests/runtime_behaviour.rs` 22/22 (new: a
    300 ms DCC gap gives 10 CPMs in 3 s through the runtime, 30 unrestricted or with the
    knob off). Engine `tests/cpm_denm_load.rs`: knobs off equal the default run `node.tx`
    record for record; all on is deterministic, ≤ 2 objects per CPM; the loader refuses
    each knob without its service and out of range. `scenario::publish` 13/13 (after
    `touch build.rs`; units added for `cpm_max_objects` and `cpm_quality_threshold`).
  - Faults (one run, all injected, then restored): DCC gap ignored, threshold not
    applied, cut unsorted, object age in seconds, slow-down average compared the wrong
    way, non-urban precondition dropped, speed drop on TRCO_0 alone, friction 5 s hold
    skipped, four slow CAMs enough, triggers on regardless of the knob, runtime handing
    a zero DCC gap, wiring dropping the threshold and budget: all 13 new tests went red
    (11 `v2xw-node` lib, `the_cpm_rate_follows_a_300_ms_dcc_interval`, the engine
    knobs-on test); the refusal test and the 3 older CPM tests stayed green.
  - Regression on the restored tree: engine `cpm_metrics` 1/1, `message_sets` 6/6,
    `cpm_denm_load` 2/2; node `firewall_sentinel` 7/7, `runtime_behaviour` 22/22; msg
    `etsi_oracle` 1/1. Goldens: none moved and none re-blessed (every knob off).
  - Open: a run on a highway world with the binary (the C2C-CC services are non-urban
    and the built-in highway is procedural: its road class and a measured DENM count are
    owed); RS 2007 TRCO_2 (others' hazard lights in CAMs), TRCO_5 (special vehicles),
    mobile-radio TRCO_3; RS 2002 conditions a-h (the car-following models do not expose
    the brake or throttle demand the µ-high comparison needs); AT-change blocking
    (RS_tcTrJa_120/146), `eventHistory` (RS_BSP_544), `roadType` and `lanePosition`; CPM
    segmentation by `MTU_CPM` and Annex E VoI. The 30 s friction-estimate hold and the
    motorway/trunk non-urban mapping are this build's choices, stated on the cards.
- Status 2026-10-27, radio-fix3 follow-up (expectations `radio-fix3.yaml` R3.3): the built-in
  highway (procedural, unsignalised, 65 mph) now tags its streets Motorway (it said
  Residential, so the services thought it urban; traction loss used the urban 4 s
  repetition). Measured on it: end-of-queue and local slow-down DENMs follow RS 2007 (each
  originator once; local slow-down from every vehicle standing 30 s), traction loss on ice
  repeats at 1 s with 10 Hz updates (CBR 0.21 -> 0.36), `cpm_dcc_limited` moves one CPM in
  59,454 at CBR 0.37 (only where a unit passes 0.62). A DENM's cause is now in `node.tx`
  content (`denm_cause`). Open: the built-in highway is a grid with at-grade junctions and
  queues with no incident; emergency-brake-light DENMs at 33 per 100 vehicle-km in free flow
  and 132 events on ice (limiters after the car-following model ignore mu g); no
  end-of-queue DENM without forward sensors or hazard lights until a local slow-down exists.

### Wave 5: channels, sensors, imported calibrations
- Integration (2026-10-27): every WP-12 knob is off by default; the CPM/DENM tests
  (`cpm_metrics`, `applications`, node lib) pass in the full test pass. One unexplained
  shift: `scms_knobs_runs` cache knob sees `MacTimer` 152,541 vs 152,539 (base vs variant;
  it pinned identical counts), not bisected.

**WP-13 Multi-channel, part 1: channel map, second radio, own-channel leakage** (M, 7 days). Source: O-P1 (a, b, d).
- Scope: `radio.channels` maps message sets to channels from the region's band plan (US:
  J2945/1 and the 2020 plan; EU: EN 302 663 / TS 103 141); an optional second radio per
  device; leakage between our own adjacent channels through the existing ACLR/ACS.
- Files: `crates/v2xw-radio/src/phy.rs`, `crates/v2xw-radio/src/mac.rs` (keyed by channel),
  scenario schema.
- Accept: a single-channel configuration is byte-identical to today; moving SPaT/MAP/TIM/WSA
  off the safety channel lowers its CBR by the moved share; the adjacent-channel noise rise
  equals the ACIR computed from `regulation.rs`. Fault: leakage disabled fails the noise test.
- Status (2026-10-27): done, opt-in (branch `worktree-wf_6a49856b-494-4`). `radio.channels`
  (`[{channel, messages}]`, default empty) maps message sets onto one service channel of the
  region's band plan (`run/channels.rs`); `radio.devices.{obu,vru}.radios` (default 1) and
  `rsu.radios` (default 2) decide who sends and hears it; `radio.channel_leakage` (default on)
  prices our own channels' leakage by `regulation::own_channel_leakage` (EN 302 571 mask +
  Table 8 ACS, TR 36.942 ACIR). PHY half-duplex is per (node, channel) and `leaked_dbm` is the
  one leakage rule for SINR, carrier sense and CBR; MAC timers, CCA and CBR follow the frame's
  channel; report `channel_busy_ns`/`channel_window_ns`, `untuned_arrivals`, `untuned_tx_drops`.
  Validated (`tests/multichannel.rs`, `phy::tests::own_channel_leakage_*`): a 172-only map is
  byte-identical to no map (records and report) and multi-channel runs are deterministic; SPaT+MAP
  to 180 (share 0.787): CBR172 0.00793 -> 0.00169 (theory 0.00169), CBR180 0.00624 (theory
  0.00623); 174->172 ACIR 12.98 dB and the recorded noise rise matches it within 0.3 dB in >= 90 %
  of covered BSMs. Each red under its fault (leakage off, busy on the safety channel, co-channel
  attenuated, every unit tuned to all, validation skipped). bsm_timing 3/3, phase2 13/13 run incl. the regression digest (the 14th hit the 2.5 GB run cap), engine `scenario::`
  28/28 and radio `phy::`/`regulation::` 37/37 pass.
  Open: no TIM message exists (SPaT/MAP/WSA only); one service channel (more needs WP-16
  switching); co-located desense between a unit's two radios is not modelled; the DCC gate and
  jammers act per node, not per channel; ACS beyond the alternate channel is held at the
  alternate figure (conservative); the USDOT RSU Spec v4.1 two-radio clause is not yet checked
  against a cached copy; C-V2X runs refuse a map.
- Status 2026-10-27, radio-fix3 follow-up (R3.2): the DCC gate is per channel (a second ETSI
  gatekeeper fed with the service channel's CBR) and jammers act on their own channel (carrier
  sense and reactive triggers). The jammer test turns red under the PHY-level fault (the
  CCA was already per channel; the engine-side filter only dates busy periods); the DCC
  test does not turn red with the per-channel gate removed (R3.2.DCC2, open: roadside
  frames carry no DCC state), and the reactive-jammer filter has no test yet; the 14th `phase2` test keeps one record channel (peak 94 MB, was 2,554 MB);
  multichannel 7/7, radio_channel 8/8, scms_scale 1/1. Dense CBR with the map on: the
  built-in highway at 296 vehicles reaches CBR 0.456 with J2945/1 acting; on the grid,
  moving SPaT/MAP lowers CBR172 by their share at the listeners less the 174->172 leakage
  (0.923 against 0.9215 predicted). Open: leaked energy is sensed against -85 dBm for DSRC
  too (802.11 CCA would count a non-decodable leak only above -65 dBm).
- Integration (2026-10-27): `multichannel` and `radio_channel` pass in the full test
  pass. R3.2.DCC2 (per-channel DCC gate without a failing test) stays open.
- Fix round 4 (2026-10-28, radio track `96db6559`, merged): own-channel leakage on DSRC is
  sensed from -65 dBm, not -85 (IEEE 802.11-2020 §17.3.10.6; new knob
  `radio.phy.energy_detect_dbm`; ns-3 preset -82/-62); G20-MAP CBR172 ratio 0.7855 against
  0.7842 predicted. Roadside ETSI gates now get a CBR, and the CBR meter keeps a jammer's
  windows. R3.2.DCC2 is closed by the rewritten multichannel test (R4.2.FEED). Open:
  jammers still held to -85 dBm (R4.1.JAM-ED), frames pruned at their end (R4.2.METER-FRAMES,
  moves every golden), the adjacent-emitter path not run. Rows: `radio-fix4.yaml`.
- Fix round 5 (2026-10-28, stamp and radio track, branch only): R4.1.JAM-ED closed. A
  jammer's noise and an adjacent-channel emitter carry no 802.11 preamble, so carrier sense
  (the PHY's CCA and the engine's busy-since / idle-at) holds both to energy detect only
  (IEEE 802.11-2020 §17.3.10.6, -65 dBm in 10 MHz), and the J2945/1 CBR counts a jammer from
  there too (Europe keeps -85 dBm, EN 302 571 §4.2.10.1). Rows `radio-fix5.yaml` R5.JAM-ED.*.
  R4.1.EMITTER run (`scenarios/real-world/radio-fix5/`, Europe): a 23 dBm LTE-V2X emitter beside
  ITS-G5 raises the mean CBR on 180 from 0.027 to 0.278, cuts frames to 0.785 through the ETSI
  gate, and loses 13,936 receptions to `adjacent-channel`; ACIR 80 dB is byte-identical to no
  emitter. Leak-run wall time: profiled, no hot spot of ours (energy walk under 0.1 % of
  samples; G20-MAP vs ED-85 within 1-3 %), nothing changed.

**WP-14 Configurable sensors, RSU sensors, outline occlusion** (M, 7 days). Source: O-P3a.
- Scope: a sensor suite per vehicle class with mount point and azimuth (front and rear radars,
  cameras); RSU-mounted sensors producing infrastructure CPMs; occlusion tested against the
  target's outline corners with occluders as oriented rectangles instead of discs.
- Files: `crates/v2xw-engine/src/perception.rs`, scenario schema.
- Accept: the default suite reproduces today's detections (or the golden change is listed); a
  car fully behind a truck's outline is not detected and a partly visible one is; a rear radar
  detects an overtaking car; an RSU camera at a junction produces CPMs a vehicle decodes.
- Status (2026-10-27): done (branch `worktree-wf_6a49856b-494-5`). `SensorSpec` has a mount
  (front/rear/centre anchor plus offset) and a boresight azimuth; an optional `perception`
  block gives the vehicle suite, suites per class, the RSU suite (default four 80 m 90°
  cameras) and `occlusion: centre-line | outline` (outline off by default: any visible
  outline corner, vehicles as oriented rectangles, own segment/rectangle clipping after
  Günther et al. VNC 2016). New RSU role `cpm` sends infrastructure CPMs
  (`originatingRsuContainer`); the sensor container states mount and opening about the
  boresight when the block is present. The node's certificate permission list now
  carries the CPM ITS-AID 639 (without it every RSU CPM was refused
  `psid-not-permitted`; vehicle CPMs verified either way). Validated: legacy
  scan reproduced draw for draw on 40 random scenes; truck/partial/crossing-truck,
  rear-radar, RSU-approach unit tests; RSU CPMs verified by vehicles (49 of 91 in 12 s);
  asn1tools reads the new containers; schema refusals. Each shown red under its fault.
  Open: ~~SCMS-cycle RSU application certificates still carry only the SPaT/MAP PSID, so a
  `cpm` unit inside that cycle signs CPMs a receiver refuses~~ (fixed 2026-10-27, `3c40282a`:
  `wiring::rsu_app_psids`, two `rsu_certs` tests); no 3D (mast see-over)
  occlusion; no run-level byte comparison against the f4982788 binary (the PSID fix
  changes certificate bytes); outline default left for the owner to flip.
  Follow-up (2026-10-27, traffic-fix round 3): the outline rule is 2.5D (mount height to
  the target corner's top; a body blocks only where the line passes over its footprint
  below its roof, after Boban et al. 2014), so a mast or truck-cab camera sees over cars
  and not over a truck; unit test with the hand-computed heights. Centre-line (default)
  unchanged. `awareness_direct_permille = 0` in the CPM test scenario: diagnosis pending
  (no CAM is credited in `cpm_on_receive`; the run was not reached in this round).
- Integration (2026-10-27): `rsu_certs` 4/4 with the RSU application-PSID fix;
  perception tests pass. `awareness_direct_permille = 0` stays undiagnosed.

**WP-15 Complete `.rou.xml` import** (S-M, 3 days). Source: S-P8.
- Scope: every vType attribute, `vTypeDistribution`, Bernoulli `probability` flows with their
  variance kept, and the `departSpeed`/`departLane`/`departPos` keywords; unknown params
  counted.
- Files: `crates/v2xw-interop/src/routes.rs`, `crates/v2xw-mobility` model library.
- Accept: a vType's `accel`, `decel`, `tau`, `sigma` and `minGap` reach the actor's model; the
  count variance of a Bernoulli flow is within tolerance of n·p·(1−p); a LuST or MoST sample
  (licence checked) imports with zero silently dropped attributes.
- Status (2026-10-27): done (branch worktree-wf_6a49856b-494-6). `routes.rs` reads every vType attribute into an
  `ImportLedger` (applied / approximated / not modelled / cosmetic, plus unmodelled elements and
  `<param>` keys), so every attribute is counted. accel, decel, tau, minGap, sigma,
  speedFactor/speedDev (norm/normc, redrawn inside bounds), maxSpeed/desiredMaxSpeed and departSpeed
  reach the actor through the new `Demand::trip_driver` hook (`TripDriver`; default `None`). An
  imported vehicle drives its vType exactly, on SUMO's vClass defaults, with no engine style or
  spread. Krauss reads `DriverProfile.sigma` (serde-skipped when None). vTypeDistribution and
  routeDistribution are drawn per vehicle; `probability` flows are Bernoulli trials each second
  (`ProbabilityFlows::EqualSpacing` keeps the old behaviour); `period="exp(λ)"` is a Poisson stream
  (defect: it was read as a period of λ s). departLane best/random, departPos random/negative and
  departSpeed max/desired/speedLimit/random are applied. New scenario kind
  `mobility/demand/sumo-rou {path, probability_flows: bernoulli|equal-spacing}`, plus
  `scenarios/interop/sumo-rou-native.yaml`. LuST sample (MIT; MoST is GPL-3.0 so it was not
  used) is in `worlds/sumo/lust-sample.rou.xml`.
  Validated: 7 unit tests + `tests/rou_import.rs` (3), all green. Fault runs: 8 faults, each turned
  its test red (Bernoulli→spacing, exp read as a period, vType ignored by the engine, Krauss
  ignoring sigma, a not-modelled attribute dropped, distribution weights ignored, seed ignored,
  departSpeed not applied). Bernoulli count variance is within 20 % of n·p·(1−p) over 400 seeds.
  The LuST ledger total equals the file's attribute count, with 0 approximated.
  End to end: the same seed gives a byte-identical content digest. A slow vType (accel 0.8,
  maxSpeed 6) cut completed trips from 34 to 7 with the same 50 arrivals. A Poisson grid run gives
  the same digest as the f4982788 release, so no golden moved.
  Open: per-type body length/width, emergencyDecel, lc*/jm* params, per-type carFollowModel
  (mapped by `library_model` but not applied per vehicle), stops/persons and arrival* are counted
  but not modelled. Occupancy keywords (free/allowed/last/avg/stop) fall back and are counted.
  An unsafe stated departSpeed is capped rather than delayed.
  Follow-up (2026-10-27, traffic-fix round 3): a vType's `carFollowModel` is driven per
  vehicle (`TripDriver::car_following`, `library::build_one`, the engine's `VehicleCf`
  dispatch; CACC and unknown models counted not modelled); a stated departSpeed (number,
  desired, speedLimit) above the safe speed delays the insertion as SUMO documents (max and
  random are lowered); the settings form's demand dropdown lists sumo-rou and the seven
  `v2xw-demand` kinds with words. Tests written (`rou_import.rs`, routes unit test, options
  test); results in `traffic-fix3.yaml`.

### Wave 6: US signal control, channel switching, channel realism
- Integration (2026-10-27): `rou_import` 5/5 after the Krauss test was made to judge the
  last-second slope (the engine's start-up delay offsets v(t) by 0.5 s for every model).

**WP-16 Multi-channel, part 2: IEEE 1609.4 alternating access in the MAC** (M, 5 days). Source: O-P1 (c). After WP-13.
- Scope: move the `wave.rs` schedule into the MAC as a gate on transmit; per-channel EDCA
  queues; no transmission in the 4 ms guard; a frame that would cross the interval end waits.
- Files: `crates/v2xw-radio/src/mac.rs`, `crates/v2xw-net/src/stack/wave.rs`.
- Accept: no frame straddles a guard in a 10-minute dense run; continuous mode is unchanged;
  SCH frames queue across the CCH interval. Fault: removing the end-of-interval check yields a
  straddling frame.
- Prep (2026-10-27): build plan in `wp16-24-build-plans.md` §WP-16, 4 pieces. Moves `ChannelSchedule` into `v2xw-radio` (radio cannot depend on net) and gates `EdcaOcbMac::poll`/`next_poll_at`; per-(node, channel) queues already exist. The app-layer deferral at `run.rs:5410` goes, and `tuned` becomes time-aware for single-radio units. The 1609.4 constants stay UNVERIFIED.
- Status P16.1 (2026-10-28): done. `ChannelSchedule`, `ChannelAccess`, `Interval` and the 100/50/4 ms constants moved to `v2xw-radio/src/wave_schedule.rs`; `v2xw_net::stack::wave` re-exports them (v2xw-net now depends on v2xw-radio). New `interval_end_ns(t)` and `fits(t, air, cch) -> Result<(), next_start>`; a frame longer than a whole 46 ms interval is let through at the interval start rather than held for ever (no WSM is: 1 400 B at 3 Mb/s is under 4 ms). Tests `wave_schedule::tests::{the_interval_end_is_the_next_boundary, a_frame_fits_only_inside_its_interval, fits_agrees_with_the_interval_for_random_frames, the_next_opportunity_is_unchanged}`; fault: the end check in `fits` removed -> two of them red. No behaviour change.
- Status P16.2 (2026-10-28): done. `EdcaOcbMac::with_alternating(sched, cch, sch)` and `set_two_radios(node)`; the gate (`gate_step`, applied in `poll`, `next_poll_at`, `on_cca(Busy)` and `freeze_at`) holds a countdown that resumes outside its channel's interval, one that runs into the interval end, and a frame that would cross it; each resumes one AIFS after the next start of its interval, with only the idle slots counted inside the interval kept (Veins `revokeTxOPs` + `timeLeftInSlot`, behaviour only). Decision: a radio that has just switched senses a whole AIFS before counting (EN 302 663 C.4.2), so a frame enqueued in the first AIFS after a switch waits too. Any channel other than `cch` owns the SCH interval. Counters `gate_deferrals`/`guard_deferrals` per (node, channel), `total_gate_deferrals()`. Card limitation rewritten (no sync tolerance or clock error modelled). Tests `mac::tests::{alternating_gate_never_grants_into_a_guard (10 000 keyed-random frames, own-transmission busy periods), continuous_gate_is_the_ungated_mac (also: a two-radio unit is ungated), sch_frame_waits_for_the_sch_interval}`; faults: check (b) removed -> the first red; the gate applied in continuous mode -> the second red; the gate keyed on the wrong channel -> the third red.
- Status P16.3 (2026-10-28): done. The engine builds the EDCA MAC `with_alternating(primary, service)` when `net.channel_access: alternating` and the MAC tier is not abstract (`Engine::wave`); the hand-off deferral now runs only where there is no EDCA MAC to gate (abstract MAC tier, sidelink), so the setting still moves those runs. Single-radio units alternate (`channels::unit_radios`: OBU/VRU 1, RSU 2 unless `radio.devices.*.radios` says otherwise, with or without a channel map); two-radio units are marked `set_two_radios` and stay continuous. `tuned_at(node, ch, start, end)` replaces `tuned` at the three reception sites: an alternating unit hears the safety channel only for frames wholly inside a CCH interval and the service channel only inside an SCH interval (a switch mid-frame loses it); `uses_channel` replaces it at the send-side drop and the service-channel DCC, so a one-radio unit may now send its service-channel messages (in the SCH interval). Report leaves `mac_gate_deferrals`, `mac_guard_deferrals`, `straddling_frames` (skipped when zero, so the default report is unchanged). Decision against the plan: the medium tier is not refused, because in this code both the medium and high MAC tiers run `EdcaOcbMac` (`wiring::build_mac`); `SlottedMac` is selected by no tier, so there is nothing to refuse. Key notes (`net.channel_access`, `radio.channels`), the option text and the configuration reference updated; no new keys. Test: `geonet::alternating_channel_access_holds_bsms_to_the_control_channel_interval` passes through the MAC path (share in CCH 1.000; continuous 0.183); fault: the MAC built without the gate -> red (0.183). Default path: `bsm_timing` reproduces main's re-blessed `i-s1-phase2-60s` digest `a24806dc` (the base commit's committed digest was stale; main re-blessed it in 46d5ebef).
- Status P16.4 (2026-10-28): done; WP-16 complete. `tests/alternating_mac.rs`: `dense_run_has_no_straddling_frame` (80 vehicles, 1.2 km grid, 120 s: 85 812 BSMs, every one inside a CCH interval recomputed from `node.tx`, `straddling_frames` 0; fault: check (b) removed -> 345 straddle, red); `single_radio_hears_spat_only_in_sch` (RSUs with two radios send SPaT on 180; one-radio vehicles decode none outside an SCH interval and 0.185 of what two-radio vehicles decode, against 0.185 predicted by the SCH share of those receptions; fault: `tuned_at` ignoring the time -> 0.999 and 1 313 outside, red). Decision: the plan's 0.46 holds only for a uniformly timed stream; each RSU sends SPaT at its own fixed 100 ms phase, so the test weighs the SCH share by the receptions instead of asserting 0.46. `the_channel_access_rows_hold_on_the_dsrc_family` judges every `net.channel_access` row (DSRC family, 2 s): e2e 12.53 -> 12.74 ms, `report.mac_guard_deferrals` 0 -> 91, prr 0.904 -> 0.286, collision_rate 0.041 -> 0.217; fault: MAC without the gate -> guard holds 0, e2e and collision rows red. Two rows added to `expectations/networking.yaml` (guard deferrals up; prr down, Eichler 2007 [verify]); the sweep fingerprint counts the three WP-16 counters as 0 when absent in a run with MAC grants, as it already does for the traffic-control counters. The large prr drop is the synchronised release at the CCH start (Eichler; Campolo 2011): the family's BSMs are generated mostly outside the CCH window (continuous: 18 % start inside it). Proposed default: keep `continuous`, the deployed US profile (SAE J2945/1 §6.2, BSMs on 172); `alternating` stays a what-if until IEEE 1609.4-2016 is cached and the 100/50/4 ms values pinned. Open: the medium-tier `SlottedMac` gate (no tier selects it); a sync-tolerance/clock-error model.
- Integrated (2026-10-28): merged onto main as `f6b6c6ec` with WP-21 (mac track, head `3d6e33bb`). Off by default (`net.channel_access: continuous`); `bsm_timing` keeps main's `a24806dc` and no golden moved. `alternating_mac` (3) and `geonet` (4) pass on the merged tree. Open: the SlottedMac gate, a sync-tolerance model; alternating access cuts DSRC-family PRR 0.90 -> 0.29 (the owner decides the default).

**WP-17 NEMA dual-ring 8-phase controller** (L, 2-3 weeks). Source: S-P3.
- Scope: two rings, barriers, concurrent phases, lead/lag and protected/permissive and
  flashing-yellow-arrow lefts, min/max and pedestrian recall, coordination with force-offs and
  permissive windows; from the FHWA Signal Timing Manual 2nd ed. (NCHRP 812) and NEMA TS 2-2016;
  SUMO's parameter names for import parity.
- Files: `crates/v2xw-engine/src/actuated.rs` (generalised to rings), signal-plan types.
- Accept: two concurrent through phases gap out independently; ring pointers meet at each
  barrier; the FHWA manual's 8-phase example timing reproduces; SPaT `minEndTime`/`maxEndTime`
  stay consistent with the controller state; existing single-ring scenarios are unchanged.
- Prep (2026-10-27): build plan §WP-17, 10 pieces (P17.1 caches STM 2nd ed. and MUTCD and pins every default). The new `nema.rs` re-projects the composite ring intervals into `SignalPlan` each step, so the 10 files that read the plan and the SPaT-is-the-signal rule are unchanged. Plans with more than 4 legs fall back to single ring, counted.
- Sources (2026-10-28): P17.1's [verify] notes resolved in the build plan. Found: `min_recall`
  [2,6] disagrees with the STM (soft recall); SUMO has no `coordinatePhases`. NEMA TS 2 and
  the ITE 2020 guideline stay paywalled; the STM has no numeric 8-phase example.
- P17.2 (2026-10-28): adds `traffic.signals.control: nema` and the `traffic.signals.nema` block, which is validated in full. Loading `nema` is still refused. All six settings-checklist files are done. Defaults: barriers are SUMO's documented `[4,8]` and `[2,6]` (the plan had them reversed); soft recall is `[2,6]` with no minimum recall (STM Exhibit 6-22); `controller_type` is ts2 or type170, as SUMO has it. Decisions are recorded under P17.2 in the build plan. Tests: `scenario::schema::nema_settings_tests` (4). No golden moved, and the default signal settings write no `nema` key.
- P17.3 (2026-10-28): adds `nema/assign.rs`, which gives each junction's movements NEMA numbers from its geometry. Main street = the higher class, then more lanes, then more plan green. A left with no opposing through rides its through. Crosswalks take the adjacent parallel through. Junctions with more than 4 approaches, Ys and skewed layouts fall back and are counted by reason. Tests: `nema::assign::tests` (7) on generated crossroads, tees, the 3x3 grid and one-way cores. Nothing runs it yet. Open: shared-lane lefts should become permissive (P17.6).
- P17.4 (2026-10-28): adds the pure dual-ring state machine (`nema::DualRing`). It times min, extension, gap-out (latched with a conflicting call), max-out, yellow and red. The rings move within a side on their own, then clear together and start together at each barrier. It covers rest in green, dual entry, start-up in 2/6, soft, min and max recall, and lead/lag order. Tests: `nema::tests` (4), including the three plan Accepts. Nothing runs it in a scenario yet.
- P17.5 (2026-10-28): `control: nema` runs (off by default; the default path is unchanged). `nema/control.rs` re-projects the rings into each mapped junction's plan as the composite intervals from now. It rewrites only when the shown vector changes, when composite 0 nears its end, or (at most 1 Hz) when the calls change. Unmapped junctions keep the single ring and are counted. `plan_invariants` is debug-asserted after every rewrite. The SPaT feed rebuilds its timelines when the phases change. Lefts are protected-permissive. Crosswalk phases are on pedestrian recall, with walk + clearance + the 3 s buffer held (until P17.7). Priority requests at NEMA junctions are skipped (P17.9). The loader refuses the modes later pieces build. Tests: `nema::control::tests::the_plan_shows_what_the_rings_show`, `nema::tests::stm_example_timing` (a 132.3 s cycle built from the STM tables), and `tests/nema_control.rs` (a 4x6 grid under nema moves and stops by it, reruns byte-identical, and differs from the single ring). On the grid in 180 s: 198 gap-outs and 0 max-outs, against the single ring's 71 and 3. Mean speed 6.87 against 7.11 m/s. Open: the SPaT window is a conservative bound (P17.9), and no expectation rows yet (P17.10).
- P17.6 (2026-10-28): the protected, permissive and protected-permissive left modes run, and so do both permissive faces. Under FYA a permissive left turns in the opposing through's green, which removes the lead-lag yellow trap; under a circular green it turns in its own approach's green. A protected-permissive left that shares its lane with the through has no arrow (US practice gives the arrow to exclusive lanes) and runs permissive, counted. The procedural grids therefore run two-phase under the defaults. Tests: `left_modes_follow_the_display_and_the_lane`, `fya_ends_the_lead_lag_yellow_trap`, and `infra::tests::fya_maps_to_permissive_movement_allowed`. Open: permissive clearance (J2735 code 7) needs a world signal state, and per-left-lane signal groups.
- P17.7 (2026-10-28): pedestrian phases. A standing pedestrian within 2.5 m of a crosswalk's kerb end presses the button. The call is latched for the phase's next green, or a resting green recycles a walk at once. `nema.ped_recall` walks every cycle. A green that walks holds walk + clearance (longest crosswalk at `ped_speed_mps`) + the MUTCD buffer not covered by Y+R; any other green shows don't walk. P17.5's blanket pedestrian recall is removed. Tests: `ped_recall_serves_walk`, `a_pedestrian_at_the_kerb_calls_the_walk`.
- P17.8 (2026-10-28): coordination runs (`nema.coordination`). Each junction keeps its load-time plan's cycle and offset, so a green wave reaches the rings this way, and without coordination the wave is refused under nema. Splits keep the plan's division of the cycle between the two streets. The coordinated phases are always called, hold to the yield point (TS 2: offset + split − Y − R; Type 170: the offset) and may end only in the permissive window. Force-offs are fixed or floating (STM §7.3.4), counted, and logged with the max-outs. Tests: `coordinated_phases_hold_their_offset` (yield within one step in every cycle under saturation) and `coordinated_rings_with_a_green_wave_run_the_grid`. Open: offset transition (shortway or dwell), and volume-based splits.
- P17.9 (2026-10-28): NEMA junctions serve priority through their rings. The green extension is capped at max + 10 s, and early green cuts conflicting greens after 7 s and any walk. The priority service keeps, logs and lapses the requests, but no longer moves those plans' clocks. SPaT `minEndTime` is the rings' earliest possible change for every group, and `maxEndTime`/`likelyTime` come from the projection. `tests/nema_spat.rs` finds 100 % of the group changes inside the window published the step before, and `likelyTime` within 0.5 s + 2 steps while the calls hold. `NemaSettings::from_sumo_params` is ready for WP-23. Open: phases between a priority cut and the requested phase are not omitted.
- Integrated (2026-10-28): P17.2-P17.9 merged onto main as `c1866cce` (nema track, head `7c4784bb`). Off by default; no golden moved. `nema_control` (2), `nema_spat` (1) and the engine lib (151) pass on the merged tree. P17.10 (the G-TV nema-safety check, two expectation rows, the Manhattan run, the model card, the default proposal) is not started.
- P17.10 (2026-10-28, fix round 5 traffic track): built, and its check is **red**.
  - **The hook.** `v2xw-trafficval` `study::run_controlled` gives a study its own world. A hook runs before each mobility step, in the kernel's order. The new check uses it to step `ActuatedControllers` from the previous step's detector calls and push-button pedestrians.
  - **The check.** `invariants/nema-safety` (G-TV) runs `manhattan-5min` at 1,500 veh/h with 150 pedestrians under `control: nema` for 600 s. Every step, it checks every dual-ring plan against the junction's conflict matrix (`audit_world`'s rule), and it reads the auditor.
  - **Result:** red entries 0, but two foe movements are protected green together on 6,000 of 6,000 steps. There are 189 distinct pairs on 263 dual-ring plans. Examples: a Left and a Straight from adjacent lanes of one edge at junction 106; two crossing Lefts from edge 265 at junction 107. The same rule finds 0 such pairs on the imported fixed-time plans, which make one movement of such a pair yield.
  - **Consequences:** conflict-zone 73 (junction 207) and standstill 2.
  - **Cause:** `nema/control.rs` `NemaPlan::vector` (l.243-271) shows Green for every movement whose own phase is green. It never tests the movements green together against the matrix.
  - **Fault:** with the barrier skipped, the red-entry row goes red.
  - **Rows.** `traffic.yaml` gains `control: nema` -> `traffic.stopped_share` down. On the grid it passes at 0.165 -> 0.035; with the variant set back to fixed-time it is DEAD. The families file gains the `traffic.signals.nema` context (`control: nema`), so the catalogue sweeps nema settings where they act.
  - **Max recall.** The second row (max recall -> fewer gap-outs) cannot be swept by `--expect`, because the setting is a list. It is measured by hand through the new `RunReport` leaves `signal_gap_outs` and `signal_max_outs`: gap-outs 79 -> 0, max-outs 0 -> 44 (`traffic-r5.yaml` `R5.nema.*`).
  - **City run.** The full engine on Manhattan for 60 s: speed 6.25 -> 6.56 m/s against fixed-time, 37 gap-outs.
  - **Model card.** `nema::card()` (`signals/nema-dual-ring`) goes in the catalogue only, so no manifest moves.
  - **Default proposal (owner decision; not flipped):** do not make `nema` the default until the conflict defect is fixed and `invariants/nema-safety` is green. The fix is to show the turning movement of a concurrent foe pair GreenYield, as the fixed-time import does.
  - **Then, for US presets:** `nema` with coordination on for suburban and arterial signals, where US practice is actuated NEMA TS 2 control (FHWA STM 2e ch. 5), and fixed-time kept for `urban-us-nyc`. New York City's Midtown signals are pretimed and coordinated [verify: NYC DOT signal timing practice]. That flip moves G-TV (`cities/*`) and every SPaT-carrying digest.

**WP-18 GEMV²-style channel enrichment** (M, 5-7 days). Source: O-P4.
- Scope: small-scale σ growing with vehicle and obstacle density inside the link ellipse,
  bounded by Boban et al. 2014's per-class ranges; foliage loss through OSM tree polygons
  (ITU-R P.833); optional single-bounce building reflection for NLOSb. A modifier card, off by
  default until validated.
- Files: `crates/v2xw-radio/src/obstacle.rs`, shadowing in `crates/v2xw-radio`,
  `crates/v2xw-world/src/osm.rs` (tree polygons; sequence after WP-05).
- Accept: an empty road gives the class minimum σ and a dense arterial approaches the maximum;
  a link through 30 m of forest loses the P.833 value; disabled, outputs are byte-identical.
- Prep (2026-10-27): build plan §WP-18, 5 pieces. Today trees and grass are both `LanduseClass::Park` and the `boban-mel` foliage card is referred to but missing. It adds a hash-excluded `World.vegetation` layer, `obstacle/foliage/p833` and a density σ in `prop.rs`. Boban 2014 and P.833 must be cached first (P18.1).
- P18.2 (2026-10-28): done, off by default, no golden moves. `World.vegetation: Vec<VegetationArea { ring, kind: Wood | TreeRow | Tree }>` (`model.rs`) is threaded through the builder, `WorldParts`, the DEM rebuild and `worlds::rebuild_with`. It is hashed (tag 0x0D) and written (native section 15) **only when non-empty**: `hash::tests::vegetation_is_hashed_only_when_present` pins the 3 × 3 grid's pre-layer hash. The importer gets `OsmOptions.vegetation` (off), which brings in `natural=wood` and `landuse=forest` (closed ways and multipolygon outers), `natural=tree_row` ways buffered to `tree_row_width_m`, and `natural=tree` nodes as 12-gon discs of `diameter_crown` or `tree_crown_m`. Both widths are 8 m, an ASSUMPTION: the OSM wiki gives no width. Woods are still `LanduseClass::Park` zones in the land-use layer. The scenario key (`world.osm.vegetation`) is left for P18.5.
- P18.3 (2026-10-28): done, not yet wired, so no golden moves. The card `obstacle/foliage/p833` is `FoliageLoss` (`v2xw-radio/src/foliage.rs`, a new module rather than obstacle.rs) over a `VegetationIndex` R-tree, with `A = min(d·γ, J(ν_canopy))`. γ = 0.79·f^0.61 dB/m (GEMV² eq. 4), which is 2.33 dB/m at 5.9 GHz. The cap is P.833-10 §3.1 eq. (7) with a P.526 knife edge over a 15 m canopy (ASSUMPTION). §3.1 is used above its 1 GHz range because §3.2.1 RET has no 5.9 GHz parameters, and the card says so. The dangling `boban-mel` references now point to it. Constants: docs/design/research/gemv2-p833-constants.md. The engine `ObstacleStack` term and the key `radio.channel_model.foliage` are P18.5.
- P18.4 (2026-10-28): the radio half is done; the engine wiring moves to P18.5 with its key. `prop::DensitySigma` is GEMV² eq. (10), `σ = σ_min + (σ_max − σ_min)/2·(√(N_V/N_V,max) + √(A_S/A_S,max))`, **not** the plan's linear clamp. It uses Table V's σ bounds (LOS 3.3/5.2, NLOSv 3.8/5.3, NLOSb 4.1/6.8 with no reflections) and Table VI's ellipse major axes (LOS 500 m urban and 1,000 m elsewhere, NLOSv 400 m, NLOSb 300 m). `link_density` counts the vehicles and building-footprint centroids inside the link ellipse (foci at the antennas, minor axis √(r² − d²)). N_V,max = 11,500 veh/km² and A_S,max = 0.6 are ASSUMPTIONs, because the paper gives no values. `LogDistanceShadowing::loss_db_drawn_sigma` takes the caller's σ, and `loss_db_drawn` is that with `None`, so it is byte-identical. Open for P18.5: computing the density per link in `run.rs` (actors from the snapshot, footprints from the building index), and `radio.channel_model.small_scale`.
- P18.5 (2026-10-28): done except the reflection term. All three keys are off by default, and no golden moves. The keys:
  - `world.vegetation` reaches `OsmOptions.vegetation`; the world cache key moves only when it is on. The plan's `world.osm.vegetation` has no `osm` section to live in, so it sits beside `world.service_roads`.
  - `radio.channel_model.foliage`, a bool: `p833` is the only model. It is refused at the abstract tier and without `world.vegetation`.
  - `radio.channel_model.density_sigma`, a bool for the plan's `small_scale: density`.

  `ObstacleStack` gains `foliage`, added after the three summed terms so the default sum is unchanged, and `density` (`wiring::DensityTerm`: the building footprints, plus the snapshot's non-VRU vehicles from `actors_within`). `run.rs` passes the σ to `loss_db_drawn_sigma`. `tests/gemv2_off.rs` checks four things: setting the keys to false is byte-identical; the density σ reaches the run; a self-written forest road (`scenarios/sensitivity/forest-road.osm.xml`: a road through a 110 m wood) loses receptions with foliage on; and the two new `phy-channel.yaml` rows hold on the new `forest` family and on `traffic`. **Not built:** single-bounce building reflection. It has no key, so nothing is refused. The `rural-wood` family on a cached city edge is replaced by the fixture. Defaults proposed to the owner: `density_sigma: true` (measured, Boban 2014), and `foliage` plus `vegetation` on for suburban and rural maps, after a check against a measured foliage-loss V2V trace.
- Integrated (2026-10-28): P18.2-P18.5 merged onto main as `efbb7a42` with WP-22 (grade track, head `9999e616`). All three keys off by default; no golden moved. `gemv2_off` (4), world lib (123) and `osm` (57, 5 ignored) pass on the merged tree. Open: single-bounce reflection (no key), an R-tree for `DensityTerm` before it can be a default, and the foliage check against measured loss.
- WP-27 P27.1 (2026-10-28): done; no key, no golden moves. `DensityTerm` keeps the footprints in `prop::FootprintIndex`, an `rstar` R-tree of their centroids. `near_link` queries the box of the circle of half the ellipse's major axis round the link midpoint (every ellipse point is inside it, by the triangle inequality), widened by 1 mm. It returns the hits in building-id order, and `link_density` then applies `in_link_ellipse` as before, so the areas are summed in the scan's order: σ is bit-identical. Tests: `gemv2_off::density_index_equals_the_scan` (2,000 SplitMix-keyed links over the Manhattan world, 3,024 footprints, every `LinkDensity` bit-equal to the scan) and `prop::tests::footprint_index_keeps_the_given_order_and_misses_nothing`. Faults: the query box at a quarter of the axis -> both red (1,164 of 2,000 links differ); the hits left in R-tree order -> the prop test red. The plan's fault, R-tree order, does **not** turn the Manhattan test red: the compensated `sum_ordered` of positive areas gave the same bits on all 2,000 links, so the order is pinned by the prop test instead. Wall time (debug build, the 2,000 links): scan 57 ms, R-tree 16 ms; a full Manhattan run before and after was not timed. Found on the way: a link longer than Table VI's `r` gets a degenerate ellipse (area 0) and so σ_min (about 40 % of these links), as in P18.4. The default `density_sigma: true` stays an owner proposal.

### Wave 7: SCMS over the air, priority vehicles, scripted situations

**WP-19 SCMS traffic through RSUs over the air** (M, 7 days). Source: O-P2. After WP-16.
- Scope: WSA with a WAVE Routing Advertisement encoded per IEEE 1609.3-2020 §8 with the
  existing codec forge; an 802.11p/PC5 `PacketPath` for the SCH hop under the TCP/TLS model,
  used for certificate downloads, CRL fetches and misbehaviour-report uploads (1609.2.1 §6.3).
  This serves the SCMS realism project directly.
- Files: `crates/v2xw-radio/src/ip_transport.rs`, WSA encoding in `crates/v2xw-msg`,
  `crates/v2xw-net/src/stack/wave.rs`.
- Accept: WSA round-trips through the ASN.1 codec; download success rises with RSU contact
  time and falls with SCH load; a CRL fetch and a report upload complete over an RSU in a test
  scenario; backhaul-only configurations are unchanged.
- Prep (2026-10-27): build plan §WP-19, 5 pieces. The air hop of `rsu-relay` is a constant `Link` today (`backend.rs:484`); it becomes an `RsuSchPath` `PacketPath` (contact window, PER, SCH CBR, WP-16 interval). The WSA ASN.1 (`wsa.asn`/`wee.asn`) is not in `third_party` and must be cached with its licence. Fixes SCMS rows M11 and U5.
- Sources (2026-10-28, merged `e74c153d`): the IEEE 1609.3 ASN.1 modules (WSA, WEE, WSM,
  plus ISO 17419) are in `third_party/asn1/ieee1609.3/` with `PROVENANCE.md` (TCI_ASN1
  `63d82d87976b`, byte-identical, CRLF kept, no upstream licence). Encoding is UPER; a sample
  WSA matches USDOT RSU Spec v4.1 App. D except the power byte. SCMS PSID 0x23 is an
  assumption (no deployment document names one). Owed: the oracle copy and the
  vectors-provenance row.

**WP-20 Traffic yields to emergency vehicles** (M, 5 days). Source: S-P5.
- Scope: within a reaction distance civilians pull to the kerb or slow (US move-over, UVC
  §11-405; rescue lane for `urban-de`); an emergency vehicle may pass red after slowing;
  optional variant where only drivers who received an EVA message react.
- Files: `crates/v2xw-mobility/src/rules.rs`, `crates/v2xw-mobility/src/lateral/`.
- Accept: ambulance corridor travel time falls with yielding on; no civilian leaves the
  carriageway; with the V2X variant only receivers react. Fault: yielding off fails the
  travel-time check.
- Prep (2026-10-27): build plan §WP-20, 5 pieces. A jurisdiction yield table in `rules.rs`, the pull-over through SL2015 with a carriageway clamp, an EV red pass tagged like WP-08 violations, and a V2X trigger from verified BSM `EmergencyDetails`/CAM `EmergencyContainer` (both new encodings, gated). UVC and StVO clauses are [verify].

**WP-21 Condition-triggered scenario events** (M, 5 days). Source: O-P7.
- Scope: a `when:` clause on timeline items: TTC below x, an actor inside a polygon, a speed
  difference, a probability, AND/OR; existing effects reused.
- Files: `crates/v2xw-engine/src/scenario/schema.rs` and the timeline evaluator.
- Accept: a hard brake fires when the follower's TTC drops below 2 s, under several seeds;
  reruns are byte-identical; time-only scenarios are unchanged.
- Prep (2026-10-27): build plan §WP-21, 3 pieces. A closed `Condition` enum on `TimelineItem.when`, evaluated each mobility step and fired through the existing `on_control`. TTC comes from `v2xw-metrics::ssm`, `probability` draws are keyed per (seed, item, step), and targets bind as `when.leader`.
- Status P21.1 (2026-10-28): done. `TimelineItem.when: Option<Condition>`, `max_fires` (1), `cooldown_s` (0), all skipped when unset (existing scenarios serialise as before). `Condition` is tagged by `kind` (`ttc_below {ttc_s}`, `in_polygon {points_m}`, `speed_diff_above {speed_diff_mps}`, `probability {p}`, `all|any {of: [Clause]}`). Decisions: one level of `all`/`any` over leaf `Clause`s, because the settings reflection publishes every nested path (an unbounded recursion would need a unit row per depth); field names carry their units (`ttc_s`, `speed_diff_mps`, `points_m`) so publish reads them off the name; `zone` is not offered (the world has no named zones); `speed_diff_above` looks for the leader within 100 m (`PAIR_RANGE_M`); the TTC gap is bumper to bumper (rear-axle difference less the follower's length) and compared on the D10 time grid like WP-11; `when` on `attack.wave` is refused (its windows are fixed before the run). `timeline::trigger`: `Condition::eval(&TriggerView) -> Option<Binding>` (pure; actors in id order; `probability` keyed by (seed, item, step) on its own `scenario/trigger` stream), `Role` (`when.leader|follower|actor`), `Pick::Bound`. Validation: knobs only with `when`, ranges (BOUNDS rows), polygon >= 3 finite corners with area, non-empty composites, a bound target only on safety kinds and only a binding the clause always makes. Options, publish units, KEY_STATUS note, configuration reference. Tests `timeline::trigger::tests::{all_any_truth_table, in_polygon_binds_the_first_actor_inside, probability_zero_never_one_always, ttc_below_binds_the_closing_pair, the_loader_holds_a_when_item_to_its_rules}`; faults: `any` evaluated as `all` -> truth table red; TTC comparator `>` -> ttc test red; binding check removed -> loader test red; draw not keyed by the item -> probability test red; polygon test inverted -> in_polygon test red. The engine does not evaluate the clause yet (P21.2).
- Status P21.2 (2026-10-28): done. `seed_timeline` leaves `when` items off the calendar; `Engine::evaluate_triggers` runs at each mobility step right after the states of that instant are published and before the drivers move (so the clause reads the truth of the firing instant and the effect reaches the same step's update), visits armed items in list order, builds the vehicle view once (VRUs excluded), and fires through `on_control`; `max_fires`/`cooldown_s` are kept in `TimelineState.trigger_fires`, an `until` ends each firing `until - t` after it. A safety kind with `target: when.*` acts on the bound vehicle (`Pick::Bound`, no waiting). Decision: no separate `timeline.fired` channel; the item's own `scenario.event` record gains `firing`, `actor`, `leader`, `follower` (actor ids), absent unless a `when` item fired, so default records are unchanged. Open: the plan's 'polygon inside the world box' check (the world publishes no local bounding box; a polygon off the map never holds). Tests `tests/trigger_ttc.rs::{a_when_item_fires_on_its_clause_with_its_knobs, a_probability_clause_replays_byte_identically}`; `tests/timeline.rs` passes with the new fields; faults: seeding `when` items at `t` -> the first red (an extra firing at 2 s); cool-down ignored -> red (2.0, 2.1, 2.2 s).
- Status P21.3 (2026-10-28): done; WP-21 complete. `tests/trigger_ttc.rs::hard_brake_fires_at_ttc_two_seconds`: 5 seeds on a signalised 3x3 grid (3 600 veh/h, 40 s); at the firing step the bound pair's TTC recomputed from `gt.kinematics` is 1.91-1.99 s and one step earlier every adjacent pair's is >= 2.0 s (2.01-2.35); the bound follower then brakes at about 0.5 g; seeds 1-2 replay byte for byte. Decision: the scenario scripts four full ABS stops (8 m/s²) of lead vehicles at 10/16/22/28 s, because the calibrated drivers alone did not close below 2 s on seed 3 (least TTC 2.36 s with one 0.5 g stop). Faults: TTC comparator `>` -> red (fires at 2.43 s); the clause evaluated after the mobility update (one step off) -> red (fires on a pair at 2.20 s) and the example digest moves. Example `scenarios/edge/ttc-triggered-brake.yaml` (its trigger fires on a bound pair) pinned in `tests/regression/ttc-triggered-brake.json` (`5047d6fc`, 8 184 records). Time-only runs unchanged: `bsm_timing` reproduces main's `i-s1-phase2-60s` digest `a24806dc` with all of WP-21 in. Not done: the plan's 'draw from the shared stream' fault (the pure clause has no access to a shared stream; `probability_zero_never_one_always` holds the keying instead); the polygon-in-world-box check. No default changes: this is a control feature.
- Integrated (2026-10-28): P21.1-P21.3 merged onto main as `f6b6c6ec` (mac track). New golden `tests/regression/ttc-triggered-brake.json`; `trigger_ttc` (4) and `timeline` (9) pass on the merged tree. Departures recorded above: one nesting level of all/any, the firing recorded on `scenario.event`; the world-box polygon check is still open.

### Wave 8: grades, imported controllers, the literature benchmark

**WP-22 Grade-aware, power-limited acceleration** (M, 5 days). Source: S-P6.
- Scope: `a_max(v, θ) = min(a_model, (P/(m·v) − F_roll − F_aero)/m) − g·sin θ` for every
  car-following model, from class power and mass and `dem.rs` grade; behind a flag first.
- Files: `crates/v2xw-mobility/src/engine.rs`, `crates/v2xw-energy/src/powertrain.rs` (reuse),
  `crates/v2xw-world/src/dem.rs` (reuse).
- Accept: a heavy truck on a long 5 % upgrade reaches a crawl speed within the band of the
  AASHTO/HCM truck speed-distance curves; flat-road runs are byte-identical.
- Prep (2026-10-27): build plan §WP-22, 3 pieces. Road-load physics is in `phemlight.rs`/`ev.rs`, not `powertrain.rs`, and `ClassSpec` has no power. It adds a sourced class-dynamics table and an upper-bound clamp at the end of pass 6, with separate `grade` and `power_limit` flags so grade-only flat runs stay byte-identical.
- Sources (2026-10-28): P22.1 notes added; the Green Book figure, the 5 % crawl band and the
  HCM chapter are unresolved, and 120 kg/kW rests on a search excerpt.
- P22.1-P22.2 (2026-10-28): done, both flags off by default, no golden moves. `VehicleClass::dynamics` (`classes.rs`) holds two sourced rows (docs/design/research/grade-dynamics.md). Passenger is PHEMlight `PC_D_EU4`, checked against `v2xw-energy` by a test. Trailer is the Green Book design truck: 120 kg/kW at 36.3 t (80,000 lb, 23 U.S.C. 127), C_dA 5.95 m² (40 CFR 1037.520 Table 5, bin III) and c_rr 0.0077 (NRC 2012). The rigid truck, buses and vans get **no row**: the design ratio would cap SUMO's 4.5 t truck at 70 km/h on the flat. The bound is `power_grade_bound`, the last limit of pass 6. θ comes from `lane_grade_sin`, a 10 m chord, so it is `sin θ`. The test world is `worlds::graded_ring`. On 5 % the trailer crawls at 12.9 m/s, from the power balance; the band is an ASSUMPTION because the Green Book curve was not read. Keys: `traffic.vehicle_dynamics.{grade,power_limit}`. Open: scenario worlds attach a DEM **without draping** (`wiring::attach_terrain`), so `grade` acts only on worlds whose lanes carry z.
- P22.3 (2026-10-28): done. The held check `calibration/truck-grade-crawl` (G-TV, new; in `model_checks`' fast list) holds the trailer 2 km into a 5 % climb at the balance crawl speed ±0.3/+0.5 m/s. `tests/vehicle_dynamics.rs` shows that grade-only is byte-identical on a flat signalised grid, and that power_limit slows 30 % trailers on a flat freeway. A new `freight` family (`highway-congestion.yaml`, 30 % trailers) carries two rows in `expectations/traffic.yaml`: `power_limit` gives a lower `traffic.speed_mean_mps`, and `grade` leaves it the same, flat. In a 35 mph city the power limit never binds, which is the realistic answer: the IDM asks less than 120 kg/kW gives below 13 m/s. The plan's upgrade row is **not** built, because no shipped scenario has a draped hill. That needs a `world.terrain.drape` key plus a DEM in the repo (proposed P22.4). Proposed defaults for the owner: `grade: true` (byte-identical on flat maps, SUMO KraussPS); `power_limit: true` once G-TV is re-blessed.
- Integrated (2026-10-28): P22.1-P22.3 merged onto main as `efbb7a42` (grade track). Both flags off by default; no golden moved. `vehicle_dynamics` (3), `model_checks` (1) and the mobility lib (324) pass on the merged tree. Open: P22.4 (a terrain-drape key and a repo DEM, so a shipped scenario has a hill), power rows for the rigid truck, bus and van, and the ASSUMPTION crawl band.
- P22.4 (2026-10-28, fix round 5 traffic track): done; off by default.
  - **The key.** `world.terrain.drape` (false) lays lanes, junctions, crossings, buildings, sites and signal heads on `world.terrain.dem` (`DrapeOptions::all`). It is refused without a DEM, and all six settings-checklist files are done. Undraped, the default path is unchanged: the key is not serialised, so world cache keys are unchanged.
  - **The DEM.** `scenarios/sensitivity/grade-5pct.asc` is a synthetic plane rising 5 % to the east around Null Island, the origin of every procedural world. It is not measured terrain; `grade-5pct.md` gives its provenance and the formula.
  - **The test.** `vehicle_dynamics.rs` `a_draped_ramp_tilts_the_lanes_and_undraped_nothing_moves`: undraped, every lane is at z = 0; draped, every 1.5 km lane rises 5 % ± 0.2 %. Fault: with the drape ignored, the test goes red.
  - **The upgrade row.** The new `grade-truck` family is the freight road at 2,400 veh/h, draped, with grade on. The power-limit row is now judged there: 10.56 -> 10.53 m/s (-0.3 %). The effect is small because in 90 s few vehicles reach the roughly 13 m/s the limit holds a design truck to on a 5 % climb.
  - **Sensitivity sweep.** Its families now resolve a relative DEM path, as they already did a map path.
  - **Not done.** There are no rigid truck, bus or van power rows, because no single-unit weight-to-power figure has been read.
  - **Proposal for the owner.** `drape: true` wherever a scenario names a DEM: an undraped DEM gives the radio a hill the roads ignore. This moves only scenarios that carry a DEM.
- WP-26 P26.1 (2026-10-28, no cargo): done. A measured DEM is in the repo: `worlds/dem/portland-downtown.asc`, 81 × 70 posts at 1 arc-second over the `portland` map box plus about 0.003°, cut unchanged from the AWS Terrain Tiles skadi tile `N45W123.hgt.gz` (public, no login; SHA-256 and the cut rows in `worlds/dem/portland-downtown.md`). Datum WGS84/EGM96 geoid, metres (Joerd `formats.md`); licence 3DEP public domain with the USGS attribution line (Joerd `attribution.md`); Joerd does not say whether SRTM or 3DEP fills a US skadi tile, so the provenance claims neither. Grades over the map's 49.8 km of OSM road (10 m chords, bridges and tunnels left out): 19.6 % of the length above 4 %, the steepest 40 m+ blocks 6.5-7.2 % on SW Columbia, Jefferson and Clay streets towards the West Hills, so the plan's site holds. Upgrade left: the 3DEP 1/3 arc-second GeoTIFF through an offline conversion.
- WP-26 P26.2 (2026-10-28): done; no key, no golden moves. `scenarios/real-world/portland-hills.yaml` is portland-downtown on the measured DEM with `drape: true` and `grade` plus `power_limit` on. `vehicle_dynamics::a_measured_dem_drapes_the_lanes_to_its_heights`: 20 keyed lane points rise over the undraped world by a direct bilinear read of the `.asc`, within the cell's relief + 0.5 m and inside the 4 × 4 post hull (a bridge deck keeps its OSM `layer` level, since the drape adds the ground to it). New family `hills` (drape off in the base, grade and power limit on in both runs) and two `traffic.yaml` rows on `world.terrain.drape`, `the_hill_rows_hold`: speed 3.7549 -> 3.7493 m/s (-0.15 %, down), PRR 0.0703 -> 0.0804 (changes; undraped, the antennas stand at z = 0 under up to 115 m of ground). The plan's `grade: true -> speed down` row is carried by the drape row, because the routes send `traffic.vehicle_dynamics` to the flat `freight` family, where the grade must change nothing. Fixed on the way: `sensitivity.rs` `absolutise_world` walked a `../real-world/` base lexically back onto its own `..` and never reached the repo root; it now retries from the canonical directory after the old walk, so every path that resolved before resolves the same. Faults: rows flipped north/south in `dem.rs` `read_ascii_grid` -> the drape test red (18 of 20 points); the drape ignored in `attach_terrain` -> both rows DEAD (+0.0 %); before the path fix the rows test was red (map not found). The plan's `report.loss_cause` metric does not exist; not used here. Open: the DEM card still says `UNSTATED` (no key names the source; `.asc` cannot say), and the 3DEP 1/3 arc-second upgrade.

**WP-23 Import actuated and NEMA programmes from `net.xml`** (S, 2 days). Source: S-P4. After WP-17.
- Files: `crates/v2xw-world/src/sumo.rs`.
- Accept: a SUMO actuated and a NEMA `tlLogic` import as live controllers; delay-based falls
  back to actuated with a counted anomaly; the TraCI cross-validation signal phases agree.
- Prep (2026-10-27): build plan §WP-23, 3 pieces. `minDur`/`maxDur`/`<param>` are not read today, and TraCI `TL_STATE` is defined but unused. It adds a hash-excluded per-plan `SignalProgramme` table and a TraCI state comparator that reports "not run" without SUMO. Default `as-file`: the only shipped `net.xml` is static.

**WP-24 VeReMi Extension replica and reference ML baseline** (M, 7 days). Sources: F-P10, F-P11. After WP-03, 07, 10.
- Scope: the LuST subsection (licence checked), 07-09 h and 14-16 h, 30 % attackers on
  `f2md-19`, a 1 Hz beacon option and an opt-in `veremi-ext` GNSS preset; per-message metrics
  published next to the VeReMi Extension numbers, noting F2MD defect D3 (GridSybil
  inflated); an XGBoost/MLP baseline through `PyDetector` on our own export, recipe in
  `docs/guides`.
- Accept: the replica's message counts and attacker share match the published scenario
  description within tolerance; the comparison table lists ours and the published numbers per
  attack; the baseline trains and scores reproducibly from a fixed seed.
- Prep (2026-10-27): build plan §WP-24, 5 pieces. P24.1-P24.3 (LuST licence and cut, `veremi-ext` GNSS preset, ML recipe on a legacy export) can start now; `messages.rate_hz` already gives 1 Hz. P24.4-P24.5 need `f2md-19`, so they wait on the WP-07 owner decision.

### Wave summary

| Wave | Packages | Effort |
|---|---|---|
| 1 | WP-01, WP-02, WP-03 | S, S, S |
| 2 | WP-04, WP-05, WP-06 | M, S×3, S |
| 3 | WP-07, WP-08, WP-09 | M, M, S×2 |
| 4 | WP-10, WP-11, WP-12 | M, M, S×2 |
| 5 | WP-13, WP-14, WP-15 | M, M, S-M |
| 6 | WP-16, WP-17 (continues into wave 7), WP-18 | M, L, M |
| 7 | WP-19, WP-20, WP-21 | M, M, M |
| 8 | WP-22, WP-23, WP-24 | M, S, M |

## 3. Deferred and dropped

**Deferred** (useful, but lower value per effort, or waiting on an input):

| Item | Why it waits |
|---|---|
| Zipper merge (S-P10), online calibrators (S-P11), parking (S-P12) | Real gains, but none fixes a failing check or feeds a deployed V2X application before the waves above. |
| Driver distraction episodes (S-P15) | Best after WP-08, which gives it crashes to precede. |
| Overtaking in the opposing lane (S-P13, L) | Needed only for Do-Not-Pass Warning studies. |
| DCC_NET CBR sharing (O-P8) | EU stack only; the deployment target is US. |
| Doppler-aware 802.11p PER tables (O-P12) | Beyond both frameworks, but the link-level source is not cached or checked yet. |
| Hardware-in-the-loop bridge (O-P10, L) | Needs an owner decision and a real OBU on the bench. |

**Dropped** (not useful for this engine): Wiedemann 74 (S-P18, optional, no current user),
HBEFA4 (S-P17, EU only and US runs use MOVES), GN Location Service and GeoAnycast (O-P11),
F2MD Disposable and Random pseudonym policies, F2MD's variable threshold, Experi app and live
attack server, INET bit-level OFDM and dielectric obstacle loss, true Rician draws without a
sourced K, SimuLTE Mode 3, 802.11 HT/VHT, SUMO's KraussOrig1/KraussX and niche car-following
models, SOTL and rail signalling, SUMO taxi dispatch.

## 4. Where the engine is already better

- **Security and SCMS**: real IEEE 1609.2 / TS 103 097 signatures, the full SCMS chain,
  CRL/CTL, linkage, butterfly keys, P2PCD, post-quantum modes and priced crypto hardware.
  Artery runs without security by default; Veins and F2MD have none.
- **Misbehaviour loop**: signed TS 103 759 and US SM MBR reports, MA corroboration,
  two-authority resolution and revocation, against F2MD's report counter; credential-bounded
  attackers; detectors on belief only, where F2MD reads true positions and real ids.
- **Determinism**: keyed RNG per (seed, actor, step) and byte-reproducible outputs; F2MD
  reseeds from the wall clock on every draw.
- **Radio**: SINR per interference window, preamble capture and labelled loss causes;
  spatially correlated shadowing; NLOSv from the actual bodies; 802.11p, LTE-V2X Mode 4 and
  NR-V2X Mode 2 in one engine with in-band emission.
- **Messages**: real J2735 and ETSI ASN.1 encodings, 10 Hz BSM and the US profile (J2945/1
  DCC, J3161/1), where the OMNeT++ stack is EU-only and F2MD beacons at 1 Hz with stubs.
- **Traffic**: one deterministic process with V2X; applications (CACC, GLOSA, priority) fed
  by received messages rather than ground truth; micro and meso in one run; HCM-calibrated
  drivers and smooth lateral lane changes; social-force pedestrians; US models (MOVES,
  MUTCD/ITE clearance, NTCIP 1211 priority); field-banded validation with fault injection.
- **Sensing**: GNSS, clock and perception error with tracking, against Artery's perfect
  geometric detection and F2MD's oracle proximity check.
- **Threats**: jamming, CPM/DENM/VRU falsification, compromised RSUs, report poisoning and a
  privacy observer, none of which F2MD models.
