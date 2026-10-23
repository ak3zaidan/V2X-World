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

**WP-06 SUMO-format outputs** (S, 2 days). Source: S-P7.
- Scope: FCD XML, tripinfo, edgeData/laneData, summary, and E1/E2 virtual detectors at named
  positions, read from the recording like the existing exporters.
- Files: `crates/v2xw-engine/src/export.rs` (new exporter ids).
- Accept: outputs validate against SUMO's XSDs, referenced from a local SUMO install (not
  vendored); tripinfo totals equal the engine's own trip records; edgeData flow equals the
  recording's counts; SUMO's own tools read the files where SUMO is installed. Fault: a
  dropped required attribute fails validation.

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

**WP-12 CPM rate and content under load, two DENM triggers** (S ×2, 4 days). Sources: O-P3b, O-P9.
- Scope: `T_GenCpm` clamped to the DCC interval; object ordering by the TS 103 324 utility
  when a CPM must be cut; `ObjectPerceptionQualityThreshold`; DENM traffic jam ahead / end of
  queue and adverse weather / traction loss triggers from the C2C-CC triggering-condition
  documents (cached first).
- Files: `crates/v2xw-node/src/cpm.rs`, `crates/v2xw-node/src/events.rs`.
- Accept: at a CBR that forces a 300 ms DCC interval the CPM rate follows it; truncation keeps
  the highest-utility objects; free flow raises no jam DENM and a queue meeting the documented
  condition does; low friction with wheel slip raises the traction-loss DENM. Fault per item.

### Wave 5: channels, sensors, imported calibrations

**WP-13 Multi-channel, part 1: channel map, second radio, own-channel leakage** (M, 7 days). Source: O-P1 (a, b, d).
- Scope: `radio.channels` maps message sets to channels from the region's band plan (US:
  J2945/1 and the 2020 plan; EU: EN 302 663 / TS 103 141); an optional second radio per
  device; leakage between our own adjacent channels through the existing ACLR/ACS.
- Files: `crates/v2xw-radio/src/phy.rs`, `crates/v2xw-radio/src/mac.rs` (keyed by channel),
  scenario schema.
- Accept: a single-channel configuration is byte-identical to today; moving SPaT/MAP/TIM/WSA
  off the safety channel lowers its CBR by the moved share; the adjacent-channel noise rise
  equals the ACIR computed from `regulation.rs`. Fault: leakage disabled fails the noise test.

**WP-14 Configurable sensors, RSU sensors, outline occlusion** (M, 7 days). Source: O-P3a.
- Scope: a sensor suite per vehicle class with mount point and azimuth (front and rear radars,
  cameras); RSU-mounted sensors producing infrastructure CPMs; occlusion tested against the
  target's outline corners with occluders as oriented rectangles instead of discs.
- Files: `crates/v2xw-engine/src/perception.rs`, scenario schema.
- Accept: the default suite reproduces today's detections (or the golden change is listed); a
  car fully behind a truck's outline is not detected and a partly visible one is; a rear radar
  detects an overtaking car; an RSU camera at a junction produces CPMs a vehicle decodes.

**WP-15 Complete `.rou.xml` import** (S-M, 3 days). Source: S-P8.
- Scope: every vType attribute, `vTypeDistribution`, Bernoulli `probability` flows with their
  variance kept, and the `departSpeed`/`departLane`/`departPos` keywords; unknown params
  counted.
- Files: `crates/v2xw-interop/src/routes.rs`, `crates/v2xw-mobility` model library.
- Accept: a vType's `accel`, `decel`, `tau`, `sigma` and `minGap` reach the actor's model; the
  count variance of a Bernoulli flow is within tolerance of n·p·(1−p); a LuST or MoST sample
  (licence checked) imports with zero silently dropped attributes.

### Wave 6: US signal control, channel switching, channel realism

**WP-16 Multi-channel, part 2: IEEE 1609.4 alternating access in the MAC** (M, 5 days). Source: O-P1 (c). After WP-13.
- Scope: move the `wave.rs` schedule into the MAC as a gate on transmit; per-channel EDCA
  queues; no transmission in the 4 ms guard; a frame that would cross the interval end waits.
- Files: `crates/v2xw-radio/src/mac.rs`, `crates/v2xw-net/src/stack/wave.rs`.
- Accept: no frame straddles a guard in a 10-minute dense run; continuous mode is unchanged;
  SCH frames queue across the CCH interval. Fault: removing the end-of-interval check yields a
  straddling frame.

**WP-17 NEMA dual-ring 8-phase controller** (L, 2-3 weeks). Source: S-P3.
- Scope: two rings, barriers, concurrent phases, lead/lag and protected/permissive and
  flashing-yellow-arrow lefts, min/max and pedestrian recall, coordination with force-offs and
  permissive windows; from the FHWA Signal Timing Manual 2nd ed. (NCHRP 812) and NEMA TS 2-2016;
  SUMO's parameter names for import parity.
- Files: `crates/v2xw-engine/src/actuated.rs` (generalised to rings), signal-plan types.
- Accept: two concurrent through phases gap out independently; ring pointers meet at each
  barrier; the FHWA manual's 8-phase example timing reproduces; SPaT `minEndTime`/`maxEndTime`
  stay consistent with the controller state; existing single-ring scenarios are unchanged.

**WP-18 GEMV²-style channel enrichment** (M, 5-7 days). Source: O-P4.
- Scope: small-scale σ growing with vehicle and obstacle density inside the link ellipse,
  bounded by Boban et al. 2014's per-class ranges; foliage loss through OSM tree polygons
  (ITU-R P.833); optional single-bounce building reflection for NLOSb. A modifier card, off by
  default until validated.
- Files: `crates/v2xw-radio/src/obstacle.rs`, shadowing in `crates/v2xw-radio`,
  `crates/v2xw-world/src/osm.rs` (tree polygons; sequence after WP-05).
- Accept: an empty road gives the class minimum σ and a dense arterial approaches the maximum;
  a link through 30 m of forest loses the P.833 value; disabled, outputs are byte-identical.

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

**WP-20 Traffic yields to emergency vehicles** (M, 5 days). Source: S-P5.
- Scope: within a reaction distance civilians pull to the kerb or slow (US move-over, UVC
  §11-405; rescue lane for `urban-de`); an emergency vehicle may pass red after slowing;
  optional variant where only drivers who received an EVA message react.
- Files: `crates/v2xw-mobility/src/rules.rs`, `crates/v2xw-mobility/src/lateral/`.
- Accept: ambulance corridor travel time falls with yielding on; no civilian leaves the
  carriageway; with the V2X variant only receivers react. Fault: yielding off fails the
  travel-time check.

**WP-21 Condition-triggered scenario events** (M, 5 days). Source: O-P7.
- Scope: a `when:` clause on timeline items: TTC below x, an actor inside a polygon, a speed
  difference, a probability, AND/OR; existing effects reused.
- Files: `crates/v2xw-engine/src/scenario/schema.rs` and the timeline evaluator.
- Accept: a hard brake fires when the follower's TTC drops below 2 s, under several seeds;
  reruns are byte-identical; time-only scenarios are unchanged.

### Wave 8: grades, imported controllers, the literature benchmark

**WP-22 Grade-aware, power-limited acceleration** (M, 5 days). Source: S-P6.
- Scope: `a_max(v, θ) = min(a_model, (P/(m·v) − F_roll − F_aero)/m) − g·sin θ` for every
  car-following model, from class power and mass and `dem.rs` grade; behind a flag first.
- Files: `crates/v2xw-mobility/src/engine.rs`, `crates/v2xw-energy/src/powertrain.rs` (reuse),
  `crates/v2xw-world/src/dem.rs` (reuse).
- Accept: a heavy truck on a long 5 % upgrade reaches a crawl speed within the band of the
  AASHTO/HCM truck speed-distance curves; flat-road runs are byte-identical.

**WP-23 Import actuated and NEMA programmes from `net.xml`** (S, 2 days). Source: S-P4. After WP-17.
- Files: `crates/v2xw-world/src/sumo.rs`.
- Accept: a SUMO actuated and a NEMA `tlLogic` import as live controllers; delay-based falls
  back to actuated with a counted anomaly; the TraCI cross-validation signal phases agree.

**WP-24 VeReMi Extension replica and reference ML baseline** (M, 7 days). Sources: F-P10, F-P11. After WP-03, 07, 10.
- Scope: the LuST subsection (licence checked), 07-09 h and 14-16 h, 30 % attackers on
  `f2md-19`, a 1 Hz beacon option and an opt-in `veremi-ext` GNSS preset; per-message metrics
  published next to the VeReMi Extension numbers, noting F2MD defect D3 (GridSybil
  inflated); an XGBoost/MLP baseline through `PyDetector` on our own export, recipe in
  `docs/guides`.
- Accept: the replica's message counts and attacker share match the published scenario
  description within tolerance; the comparison table lists ours and the published numbers per
  attack; the baseline trains and scores reproducibly from a fixed seed.

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
