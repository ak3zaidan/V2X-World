//! Mobility and safety metrics: surrogate safety measures, headway, the fundamental
//! diagram, speed and acceleration (08-measurement-and-data.md §2.5, 04-models.md §11 and
//! §2.9).
//!
//! | Metric | Formula | Unit | What it does **not** account for |
//! |---|---|---|---|
//! | `ttc_min` | `gap / (v_follower − v_leader)` while closing, per same-lane pair per step | s | **vehicle length**: the gap is between reported lane positions, so a bumper-to-bumper TTC is shorter than this by `length / Δv`; and crossing conflicts, which need trajectory intersection |
//! | `ttc_conflicts` | pairs whose TTC is below the threshold | count | the same, plus the threshold itself (1.5 s is the only *verified* number in 04-models.md §11) |
//! | `pet` | time between one actor leaving a cell and a different actor entering it | s | the conflict angle, the vehicles' extent beyond one cell, and a cell revisited by the *same* actor, which is not a conflict |
//! | `drac` | `(v_follower − v_leader)² / (2·gap)` while closing | m/s² | the follower's actual braking capability, and the road surface: it is the deceleration *required*, not one that was achieved |
//! | `headway_time` | `gap / v_follower` | s | a stopped follower, which has no time headway and contributes no sample rather than an infinite one |
//! | `headway_distance` | `pos_leader − pos_follower` along the lane | m | vehicle length, as with TTC |
//! | `flow`, `density`, `mean_speed` | Edie's generalised definitions over the window and the lane | veh/h, veh/km, m/s | the lane's own capacity and geometry; and every lane whose length the run did not declare, which is left out and counted |
//! | `speed`, `acceleration` | the sampled distributions | m/s, m/s² | the sampling interval's bias: these are time-sampled, so a vehicle stopped for a minute contributes six hundred samples at 10 Hz |
//!
//! # Edie's definitions, and why they need the lane's length
//!
//! Over a window of length `T` on a lane of length `L`, with `Δt` between samples:
//!
//! ```text
//! total time spent      T_s = Σ Δt                       (one term per sample on the lane)
//! total distance        D   = Σ v·Δt
//! density   k = T_s / (L·T)         [veh/m]  → ×1000 → veh/km
//! flow      q = D   / (L·T)         [veh/s]  → ×3600 → veh/h
//! mean speed v̄ = D / T_s = q / k    [m/s]    (the *space*-mean speed)
//! ```
//!
//! Edie (1963), "Discussion of traffic stream measurements and definitions"; the same
//! definitions in FHWA's *Traffic Flow Theory* chapter 2, which 04-models.md §2.9 cites for
//! the fundamental-diagram targets. `L` is world geometry, which this crate has no business
//! knowing, so the run declares it with [`SafetyProvider::declare_lane_length`]; a lane
//! whose length was never declared produces no aggregate and is counted into
//! [`SafetyProvider::undeclared_lanes`].
//!
//! The mean is the **space**-mean speed, which is what a fundamental diagram is drawn with.
//! The time-mean speed is 6–12 % higher on a mixed-speed signalised dataset (Wardrop 1952,
//! via 04-models.md §2.9), so the two are not interchangeable and this one says which it is.
//!
//! # Determinism
//!
//! Same-lane pairing sorts by the **integer millimetre grid index** of the lane position,
//! tie-broken by `ActorId`, so two platforms order a lane's vehicles identically even when
//! two positions differ in the last bit. The PET cells are integer grid coordinates for the
//! same reason. Every threshold comparison is made on quantised values, which is build
//! decision D10.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;
use v2xw_core::card::ModelCard;
use v2xw_core::ctx::{ChannelName, EventRecord, Visibility};
use v2xw_core::ids::ActorId;
use v2xw_core::model::Model;
use v2xw_core::time::{Duration, SimTime};

use crate::cards;
use crate::channels::{ChannelView, GtKinematicsView};
use crate::def::{Agg, Dim, DimValue, Dims, MetricDef, MetricSample, SampleValue};
use crate::provider::{Decoded, MetricProvider};
use crate::quant::Quantum;
use crate::ssm::{EncounterType, Follow, SsmActor, SsmParams, SsmTracker};
use crate::stats::{Distribution, Estimate, ratio_of_sums};

/// The opt-in metric group of the full surrogate safety measures ([`crate::ssm`]):
/// `metrics: [all, ssm]` or any `ssm_*` name. Not under a bare `all`, so a run that does not
/// name it keeps the metrics and digests it had.
pub const SSM_GROUP: &str = "ssm";

/// The body a vehicle of an undeclared class is given for the full surrogate measures,
/// length and width in metres: a passenger car. A run declares every class's own with
/// [`SafetyProvider::declare_class_dims`].
const DEFAULT_BODY_M: (f64, f64) = (4.5, 1.8);

/// The time-to-collision threshold below which a same-lane pair is counted as a conflict:
/// **1.5 s**.
///
/// The one *verified* surrogate-safety threshold in 04-models.md §11 (FHWA-HRT-08-051, "as
/// suggested in previous research"). The PET and DRAC thresholds in the same table are
/// marked UNVERIFIED, which is why this crate thresholds TTC and reports the other two as
/// distributions.
pub const TTC_THRESHOLD_S: f64 = 1.5;

/// The follower speed below which no time headway is taken, m/s: 5 mi/h, the speed under
/// which HCM 7th ed. (Ch. 4, queued state; Ch. 19 control delay) counts a vehicle as
/// stopped or queued. A queue creeping at 0.05 m/s 7 m behind its leader has a "time
/// headway" of 140 s, which is not a car-following headway at all; with every crawling
/// follower in it the mean ran to 129 s on the Midtown grid and a longer desired headway
/// for trucks *lowered* it by 60 % (docs/validation/sensitivity-expectations.md). The
/// distance headway keeps every pair.
pub const HEADWAY_MIN_SPEED_MPS: f64 = 2.235;

/// The time headway above which a follower is free-flowing rather than following, s:
/// Vogel (Accident Analysis and Prevention 35, 2003, 427-433) regards headways over 6 s
/// as not following. Without it a lone vehicle 200 m behind another at 10 m/s added a
/// 20 s "headway" and the mean (8.5 s on the Midtown grid) hardly saw the drivers' own
/// desired time gap: doubling IDM T moved it 6 %.
pub const HEADWAY_FOLLOWING_MAX_S: f64 = 6.0;

/// One vehicle's state on a lane, as this provider needs it.
#[derive(Debug, Clone, Copy)]
struct OnLane {
    actor: ActorId,
    /// The lane position's integer millimetre grid index — the sort key.
    pos_grid: i64,
    pos_m: f64,
    speed_mps: f64,
}

/// Edie's accumulators for one lane over one window.
#[derive(Debug, Default)]
struct LaneAccumulator {
    /// One sample per (actor, step) on this lane.
    samples: u64,
    /// `v·Δt` per sample, reduced by sorting so the sum is order-independent.
    distance: Distribution,
}

/// The mobility and safety metric provider.
///
/// Every metric here is **ground truth**: surrogate safety measures are computed from the
/// simulator's own state, which no node could know, so every definition is tagged
/// [`Visibility::Gt`] and an exporter must keep them out of a node-visible file
/// (08-measurement-and-data.md §1).
///
/// Windowed. A frame — all the kinematics samples sharing one `SimTime` — is processed when
/// the first record of the *next* frame arrives, or at flush, so pairing sees the whole
/// frame. That makes one documented assumption on the input: records arrive in
/// non-decreasing time order, which is the order a run produces them in.
pub struct SafetyProvider {
    card: ModelCard,
    min_samples: u64,
    /// The mobility sampling interval, the scenario's `mobility_step_ms`.
    dt: Duration,
    /// The PET cell's side length in metres.
    cell_m: f64,
    /// The longest gap counted as a post-encroachment time.
    pet_horizon: Duration,
    /// The TTC threshold, quantised once so the comparison is on the grid (D10).
    ttc_threshold_grid: i64,

    /// The declared lane lengths: world geometry the run supplies.
    lane_length_m: BTreeMap<u32, f64>,
    /// Σ of the declared lengths, m: the network the headline aggregates are over.
    network_length_m: f64,
    /// Whether per-lane samples are reported ([`SafetyProvider::with_per_lane`]).
    per_lane: bool,
    /// Lanes seen in the data whose length was never declared.
    undeclared_lanes: BTreeSet<u32>,

    /// The frame being assembled, keyed by actor so a duplicate sample cannot double-count.
    frame_t: Option<SimTime>,
    frame: BTreeMap<ActorId, GtKinematicsView>,

    ttc: Distribution,
    ttc_conflicts: u64,
    drac: Distribution,
    headway_time: Distribution,
    headway_distance: Distribution,
    pet: Distribution,
    speed: Distribution,
    acceleration: Distribution,
    lanes: BTreeMap<u32, LaneAccumulator>,
    /// The last actor to occupy each PET cell, and when.
    cell_last: BTreeMap<(i64, i64), (ActorId, SimTime)>,

    window_start: SimTime,
    rejected: u64,

    /// The full surrogate safety measures, when the `ssm` group is on.
    ssm: Option<SsmTracker>,
    /// Body length and width by class name, for the measures' rectangles.
    class_dims: BTreeMap<String, (f64, f64)>,
    ssm_counts: BTreeMap<EncounterType, u64>,
    ssm_min_ttc: Distribution,
    ssm_max_drac: Distribution,
    ssm_max_mdrac: Distribution,
    ssm_min_ppet: Distribution,
}

impl SafetyProvider {
    /// A provider whose first window starts at `t0`, sampling at 100 ms (the scenario
    /// default `mobility_step_ms`), with 1 m PET cells and a 5 s PET horizon.
    #[must_use]
    pub fn new(t0: SimTime) -> Self {
        Self {
            card: Self::build_card(),
            min_samples: crate::stats::DEFAULT_MIN_SAMPLES,
            dt: Duration::from_millis(100),
            cell_m: 1.0,
            pet_horizon: Duration::from_secs(5),
            ttc_threshold_grid: Quantum::TIME_S.grid(TTC_THRESHOLD_S),
            lane_length_m: BTreeMap::new(),
            network_length_m: 0.0,
            per_lane: true,
            undeclared_lanes: BTreeSet::new(),
            frame_t: None,
            frame: BTreeMap::new(),
            ttc: Distribution::new(),
            ttc_conflicts: 0,
            drac: Distribution::new(),
            headway_time: Distribution::new(),
            headway_distance: Distribution::new(),
            pet: Distribution::new(),
            speed: Distribution::new(),
            acceleration: Distribution::new(),
            lanes: BTreeMap::new(),
            cell_last: BTreeMap::new(),
            window_start: t0,
            rejected: 0,
            ssm: None,
            class_dims: BTreeMap::new(),
            ssm_counts: BTreeMap::new(),
            ssm_min_ttc: Distribution::new(),
            ssm_max_drac: Distribution::new(),
            ssm_max_mdrac: Distribution::new(),
            ssm_min_ppet: Distribution::new(),
        }
    }

    /// Turns the full surrogate safety measures on ([`crate::ssm`], the `ssm` group) with
    /// SUMO's SSM-device defaults as thresholds. Off by default.
    #[must_use]
    pub fn with_ssm(self, on: bool) -> Self {
        self.with_ssm_params(on.then(SsmParams::default))
    }

    /// As [`SafetyProvider::with_ssm`], with explicit parameters (`None` is off).
    #[must_use]
    pub fn with_ssm_params(mut self, params: Option<SsmParams>) -> Self {
        self.ssm = params.map(SsmTracker::new);
        self.card = Self::build_card();
        if let Some(p) = params {
            add_ssm_card(&mut self.card, &p);
        }
        self
    }

    /// Declares a class's body length and width, metres, for the full surrogate measures.
    pub fn declare_class_dims(&mut self, class: &str, length_m: f64, width_m: f64) {
        self.class_dims
            .insert(class.to_string(), (length_m, width_m));
    }

    /// Sets the insufficiency threshold.
    #[must_use]
    pub const fn with_min_samples(mut self, n: u64) -> Self {
        self.min_samples = n;
        self
    }

    /// Sets the mobility sampling interval (the card's `sample_interval_s`).
    ///
    /// Edie's definitions weight every sample by this, so a scenario running at 10 ms steps
    /// must say so or its flow and density will be ten times too large.
    #[must_use]
    pub const fn with_sample_interval(mut self, dt: Duration) -> Self {
        self.dt = dt;
        self
    }

    /// Sets the PET cell's side length (the card's `pet_cell_m`).
    #[must_use]
    pub const fn with_pet_cell(mut self, metres: f64) -> Self {
        self.cell_m = metres;
        self
    }

    /// Declares a lane's length, so Edie's aggregates can be computed on it.
    pub fn declare_lane_length(&mut self, lane: u32, metres: f64) {
        if let Some(old) = self.lane_length_m.insert(lane, metres) {
            self.network_length_m -= old;
        }
        self.network_length_m += metres;
    }

    /// Whether every lane's own aggregate is reported (`region` = the lane), besides the
    /// network's. On by default; a city run turns it off, because a Manhattan peak has
    /// fifteen hundred occupied lanes a window and three samples each would triple the
    /// run's metric stream for a breakdown nothing reads.
    #[must_use]
    pub const fn with_per_lane(mut self, on: bool) -> Self {
        self.per_lane = on;
        self
    }

    /// Lanes that appeared in the data and whose length was never declared, so they have no
    /// fundamental-diagram aggregate.
    #[must_use]
    pub const fn undeclared_lanes(&self) -> &BTreeSet<u32> {
        &self.undeclared_lanes
    }

    fn build_card() -> ModelCard {
        let mut card = cards::provider_card(
            "metric/safety/surrogates-and-flow",
            "1.0.0",
            "Surrogate safety measures (time to collision, post-encroachment time, \
             deceleration rate to avoid the crash), headway distributions, Edie's \
             fundamental-diagram aggregates, and the speed and acceleration distributions.",
        );
        card.equations = vec![
            v2xw_core::card::Equation::new(
                "ttc",
                "ttc = gap / (v_follower − v_leader) for a closing same-lane pair; gap is \
                 the difference of reported lane positions, so vehicle length is not \
                 subtracted",
            ),
            v2xw_core::card::Equation::new(
                "pet",
                "pet = t(actor B enters cell) − t(actor A left cell) for A ≠ B, over cells of \
                 pet_cell_m metres",
            ),
            v2xw_core::card::Equation::new(
                "drac",
                "drac = (v_follower − v_leader)² / (2·gap) for a closing same-lane pair",
            ),
            v2xw_core::card::Equation::new(
                "headway_time",
                "headway_time = gap / v_follower, following pairs only: v_follower >= 2.235 m/s (5 mi/h, HCM queued) and headway <= 6 s (Vogel 2003)",
            ),
            v2xw_core::card::Equation::new(
                "edie",
                "k = Σ Δt / (L·T); q = Σ v·Δt / (L·T); v̄ = q / k (the space-mean speed)",
            ),
        ];
        card.parameters = cards::statistics_params();
        card.parameters.push(cards::param(
            "ttc_threshold_s",
            "s",
            json!(TTC_THRESHOLD_S),
            json!(0.1),
            json!(10.0),
            cards::paper(
                "FHWA-HRT-08-051 (SSAM): TTC threshold 1.5 s, the one verified surrogate \
                 threshold in 04-models.md §11",
            ),
        ));
        card.parameters.push(cards::param(
            "sample_interval_s",
            "s",
            json!(0.1),
            json!(0.001),
            json!(1.0),
            cards::design(
                "03-interfaces.md §13 (time.mobility_step_ms, default 100 ms): Edie's \
                 definitions weight every sample by this interval",
            ),
        ));
        card.parameters.push(cards::param_todo(
            "pet_cell_m",
            "m",
            json!(1.0),
            json!(0.1),
            json!(10.0),
            "the cell size at which a shared position counts as the same position for PET",
            "Compare the cell-based PET distribution against a trajectory-intersection PET \
             on the same recording for cell sizes 0.5, 1 and 2 m, and pick the largest size \
             whose p50 and p95 agree with the intersection method within 5 %.",
        ));
        card.parameters.push(cards::param_todo(
            "pet_horizon_s",
            "s",
            json!(5.0),
            json!(0.1),
            json!(60.0),
            "the longest gap that still counts as a post-encroachment conflict",
            "04-models.md §11 records the PET threshold as UNVERIFIED (commonly < 5 s, not \
             confirmed). Plan: read FHWA-HRT-08-050 §3 and replace this default with the \
             sourced number.",
        ));
        card.sources = vec![
            cards::design("08-measurement-and-data.md §2.5 (traffic and safety)"),
            cards::design("04-models.md §11 (surrogate safety measures and their thresholds)"),
            cards::design("04-models.md §2.9 (fundamental-diagram validation targets)"),
            cards::paper(
                "L. C. Edie, Discussion of traffic stream measurements and definitions, \
                 Proc. 2nd Int. Symp. on the Theory of Traffic Flow, 1963",
            ),
            cards::paper(
                "FHWA-HRT-08-051, Surrogate Safety Assessment Model (SSAM): TTC, PET and DRAC",
            ),
            cards::paper(
                "J. G. Wardrop, 1952, via FHWA Traffic Flow Theory ch. 2: the time-mean \
                 versus space-mean speed difference this provider avoids by reporting the \
                 space-mean",
            ),
        ];
        card.limitations = vec![
            "TTC, DRAC and the headways are computed between reported lane positions, so \
             vehicle length is not subtracted: every gap is longer than the bumper-to-bumper \
             gap and every TTC is correspondingly optimistic."
                .to_string(),
            "Only same-lane, same-direction conflicts are found by the pairing. Crossing \
             conflicts are covered, approximately, by the cell-based PET and not by TTC."
                .to_string(),
            "PET is cell-based: it finds a shared position to within pet_cell_m and says \
             nothing about the conflict angle. The cell size is a todo-calibrate parameter."
                .to_string(),
            "The speed and acceleration distributions are time-sampled, so they are weighted \
             by how long each vehicle spent at each speed. A stopped vehicle dominates them."
                .to_string(),
            "Edie's aggregates need the lane's length, which the run declares. Undeclared \
             lanes are excluded and counted."
                .to_string(),
        ];
        card.ignores = vec![
            "Lane changes as conflicts: a vehicle moving between lanes leaves one pairing and \
             joins another, and the transition itself is not measured."
                .to_string(),
            "Vehicle dynamics: DRAC is a required deceleration, not an achievable one.".to_string(),
        ];
        card.validation.tests = vec![
            "safety::tests::ttc_and_headway_reproduce_a_hand_computed_fixture".to_string(),
            "safety::tests::edies_aggregates_reproduce_a_hand_computed_fixture".to_string(),
            "safety::tests::pet_needs_two_different_actors_in_one_cell".to_string(),
        ];
        card
    }

    fn definitions(&self) -> Vec<MetricDef> {
        let s25 = cards::design("08-measurement-and-data.md §2.5");
        let ssam = cards::paper("FHWA-HRT-08-051 (SSAM); 04-models.md §11");
        let edie =
            cards::paper("L. C. Edie 1963; FHWA Traffic Flow Theory ch. 2; 04-models.md §2.9");
        let mut defs = vec![
            MetricDef::new(
                "ttc_min",
                "s",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::TIME_S,
                "Time to collision for a closing same-lane pair: `gap / (v_follower − \
                 v_leader)`, sampled at every mobility step. Reported as a distribution; the \
                 minimum of the run is its `min`.",
            )
            .with_dims([Dim::T])
            .with_source(ssam.clone())
            .with_min_samples(self.min_samples)
            .not_accounting_for("vehicle length: the gap is between reported lane positions")
            .not_accounting_for("crossing conflicts, which need trajectory intersection")
            .not_accounting_for("a pair that is not closing, which has no time to collision"),
            MetricDef::new(
                "ttc_conflicts",
                "count",
                Agg::Count,
                Visibility::Gt,
                Quantum::COUNT,
                "Same-lane pair-steps whose time to collision is below the 1.5 s threshold \
                 (the one verified threshold in 04-models.md §11). The comparison is made on \
                 the quantised value, so a boundary case cannot flip between platforms.",
            )
            .with_dims([Dim::T])
            .with_source(ssam.clone())
            .with_min_samples(1)
            .not_accounting_for("distinct conflicts: a pair closing for ten steps counts ten times")
            .not_accounting_for("the threshold's own uncertainty"),
            MetricDef::new(
                "pet",
                "s",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::TIME_S,
                "Post-encroachment time: the interval between one actor leaving a position \
                 and a different actor arriving at it, resolved to cells of `pet_cell_m`.",
            )
            .with_dims([Dim::T])
            .with_source(ssam.clone())
            .with_min_samples(self.min_samples)
            .not_accounting_for("the conflict angle")
            .not_accounting_for("vehicle extent beyond one cell")
            .not_accounting_for("a cell revisited by the same actor, which is not a conflict"),
            MetricDef::new(
                "drac",
                "m/s²",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::SPEED,
                "The deceleration rate required to avoid the crash: `(v_follower − \
                 v_leader)² / (2·gap)` for a closing same-lane pair.",
            )
            .with_dims([Dim::T])
            .with_source(ssam)
            .with_min_samples(self.min_samples)
            .not_accounting_for("the follower's braking capability or the road surface")
            .not_accounting_for("vehicle length, as with ttc_min"),
            MetricDef::new(
                "headway_time",
                "s",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::TIME_S,
                "Time headway: the gap to the leader divided by the follower's speed.",
            )
            .with_dims([Dim::T])
            .with_source(s25.clone())
            .with_min_samples(self.min_samples)
            .not_accounting_for("a stopped follower, which contributes no sample")
            .not_accounting_for("vehicle length"),
            MetricDef::new(
                "headway_distance",
                "m",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::LENGTH_M,
                "Distance headway: the difference of lane positions between a follower and \
                 its leader.",
            )
            .with_dims([Dim::T])
            .with_source(s25.clone())
            .with_min_samples(self.min_samples)
            .not_accounting_for("vehicle length, so this is centre-to-centre, not gap")
            .not_accounting_for("vehicles on other lanes"),
            MetricDef::new(
                "flow",
                "veh/h",
                Agg::Rate,
                Visibility::Gt,
                Quantum::TRAFFIC,
                "Edie's flow over the window: `Σ v·Δt / (L·T)`, in veh/h per lane — over \
                 the whole network's motorised lanes (L their summed length; the \
                 macroscopic fundamental diagram's production), and per lane where the \
                 run reports lanes.",
            )
            .with_dims([Dim::T, Dim::Region])
            .with_source(edie.clone())
            .with_min_samples(1)
            .not_accounting_for("lanes whose length the run did not declare")
            .not_accounting_for("the lane's capacity, which is a property of the road"),
            MetricDef::new(
                "density",
                "veh/km",
                Agg::Rate,
                Visibility::Gt,
                Quantum::TRAFFIC,
                "Edie's density over the window: `Σ Δt / (L·T)`, in veh/km per lane — over \
                 the whole network's motorised lanes, and per lane where the run reports \
                 lanes.",
            )
            .with_dims([Dim::T, Dim::Region])
            .with_source(edie.clone())
            .with_min_samples(1)
            .not_accounting_for("lanes whose length the run did not declare")
            .not_accounting_for("vehicle length, so this is a count density, not an occupancy"),
            MetricDef::new(
                "mean_speed",
                "m/s",
                Agg::Mean,
                Visibility::Gt,
                Quantum::SPEED,
                "The **space**-mean speed, `q / k` — not the time-mean speed, which is 6–12 % \
                 higher on a mixed-speed signalised dataset (Wardrop 1952).",
            )
            .with_dims([Dim::T, Dim::Region])
            .with_source(edie)
            .with_min_samples(1)
            .not_accounting_for("lanes whose length the run did not declare")
            .not_accounting_for("the time-mean speed, which is a different quantity"),
            MetricDef::new(
                "speed",
                "m/s",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::SPEED,
                "The distribution of sampled actor speeds over the window.",
            )
            .with_dims([Dim::T])
            .with_source(s25.clone())
            .with_min_samples(self.min_samples)
            .not_accounting_for(
                "the sampling bias: this is time-weighted, so a stopped vehicle dominates it",
            ),
            MetricDef::new(
                "acceleration",
                "m/s²",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::SPEED,
                "The distribution of sampled actor accelerations over the window.",
            )
            .with_dims([Dim::T])
            .with_source(s25)
            .with_min_samples(self.min_samples)
            .not_accounting_for("the same time-weighting bias as `speed`")
            .not_accounting_for("jerk, which the kinematics channel does not carry"),
        ];
        if self.ssm.is_some() {
            defs.extend(ssm_definitions(self.min_samples));
        }
        defs
    }

    fn def(&self, name: &str) -> MetricDef {
        self.definitions()
            .into_iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("metric {name} is not one of this provider's definitions"))
    }

    fn on_kinematics(&mut self, v: GtKinematicsView) {
        if self.frame_t != Some(v.t) {
            self.close_frame();
            self.frame_t = Some(v.t);
        }
        // Pedestrians are the VRU provider's (`crate::vru`): walking in groups along a
        // sidewalk lane, they paired as leader and follower here and filled `headway_*`,
        // `ttc_min` and `speed` with metre gaps at walking pace.
        if v.class.as_deref() == Some("pedestrian") {
            return;
        }
        self.frame.insert(v.actor, v);
    }

    /// Processes the assembled frame: pairing, PET cells, distributions and Edie's terms.
    fn close_frame(&mut self) {
        let Some(t) = self.frame_t.take() else {
            self.frame.clear();
            return;
        };
        let frame = core::mem::take(&mut self.frame);
        let dt_s = self.dt.as_secs_f64();

        // --- per-actor quantities -------------------------------------------------------
        let mut by_lane: BTreeMap<u32, Vec<OnLane>> = BTreeMap::new();
        for (actor, v) in &frame {
            self.speed.observe(v.speed_mps);
            if let Some(a) = v.acc_mps2 {
                self.acceleration.observe(a);
            }
            // The PET cell. Integer coordinates, so the cell a position falls in is a
            // property of the declared grid rather than of the last bit of a float.
            let cx = Quantum::new(self.cell_m).grid(v.x_m);
            let cy = Quantum::new(self.cell_m).grid(v.y_m);
            match self.cell_last.insert((cx, cy), (*actor, t)) {
                Some((prev_actor, prev_t))
                    if prev_actor != *actor
                        && t >= prev_t
                        && (t - prev_t) <= self.pet_horizon.as_nanos() =>
                {
                    self.pet.observe(Duration::between(prev_t, t).as_secs_f64());
                }
                _ => {}
            }
            if let (Some(lane), Some(pos)) = (v.lane, v.lane_pos_m) {
                by_lane.entry(lane).or_default().push(OnLane {
                    actor: *actor,
                    pos_grid: Quantum::LENGTH_M.grid(pos),
                    pos_m: Quantum::LENGTH_M.quantise(pos),
                    speed_mps: v.speed_mps,
                });
            }
            if let Some(lane) = v.lane {
                if self.lane_length_m.contains_key(&lane) {
                    let acc = self.lanes.entry(lane).or_default();
                    acc.samples += 1;
                    acc.distance.observe(v.speed_mps * dt_s);
                } else {
                    self.undeclared_lanes.insert(lane);
                }
            }
        }

        // --- same-lane pairing ----------------------------------------------------------
        let mut follows: Vec<Follow> = Vec::new();
        for vehicles in by_lane.values_mut() {
            // Sorted by the integer grid index, tie-broken by actor id: a total order that
            // two platforms cannot disagree about.
            vehicles.sort_by(|a, b| a.pos_grid.cmp(&b.pos_grid).then(a.actor.cmp(&b.actor)));
            for pair in vehicles.windows(2) {
                let follower = pair[0];
                let leader = pair[1];
                let gap = leader.pos_m - follower.pos_m;
                if gap <= 0.0 {
                    // Two vehicles on the same grid point: no gap, so no surrogate measure.
                    continue;
                }
                if self.ssm.is_some() {
                    follows.push(Follow {
                        follower: follower.actor,
                        leader: leader.actor,
                        gap_m: gap,
                    });
                }
                self.headway_distance.observe(gap);
                if follower.speed_mps >= HEADWAY_MIN_SPEED_MPS
                    && gap / follower.speed_mps <= HEADWAY_FOLLOWING_MAX_S
                {
                    self.headway_time.observe(gap / follower.speed_mps);
                }
                let closing = follower.speed_mps - leader.speed_mps;
                if closing > 0.0 {
                    let ttc = gap / closing;
                    self.ttc.observe(ttc);
                    // D10: the threshold comparison is on the quantised value, never on the
                    // raw quotient.
                    if Quantum::TIME_S.grid(ttc) < self.ttc_threshold_grid {
                        self.ttc_conflicts += 1;
                    }
                    self.drac.observe(closing * closing / (2.0 * gap));
                }
            }
        }

        // --- the full surrogate measures (opt-in) ----------------------------------------
        if let Some(tracker) = self.ssm.as_mut() {
            let actors: Vec<SsmActor> = frame
                .values()
                .filter_map(|v| {
                    let heading = v.heading_rad?;
                    let (length_m, width_m) = v
                        .class
                        .as_deref()
                        .and_then(|c| self.class_dims.get(c).copied())
                        .unwrap_or(DEFAULT_BODY_M);
                    Some(SsmActor {
                        actor: v.actor,
                        x_m: v.x_m,
                        y_m: v.y_m,
                        heading_rad: heading,
                        speed_mps: v.speed_mps.max(0.0),
                        length_m,
                        width_m,
                        lane: v.lane,
                    })
                })
                .collect();
            tracker.step(t as f64 / 1e9, &actors, &follows);
            let log = tracker.take_conflicts();
            self.log_conflicts(log);
        }
    }

    fn log_conflicts(&mut self, log: Vec<crate::ssm::Conflict>) {
        for c in log {
            *self.ssm_counts.entry(c.ty).or_default() += 1;
            if let Some((_, v)) = c.min_ttc {
                self.ssm_min_ttc.observe(v);
            }
            if let Some((_, v)) = c.max_drac {
                self.ssm_max_drac.observe(v);
            }
            if let Some((_, v)) = c.max_mdrac {
                self.ssm_max_mdrac.observe(v);
            }
            if let Some((_, v)) = c.min_ppet {
                self.ssm_min_ppet.observe(v);
            }
        }
    }

    fn window_secs(&self, at: SimTime) -> Option<f64> {
        if at <= self.window_start {
            return None;
        }
        Some(Duration::between(self.window_start, at).as_secs_f64())
    }
}

impl Model for SafetyProvider {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MetricProvider for SafetyProvider {
    fn defs(&self) -> Vec<MetricDef> {
        self.definitions()
    }

    fn subscribe(&self) -> Vec<ChannelName> {
        vec![GtKinematicsView::channel_name()]
    }

    fn on_event(&mut self, ev: &EventRecord) {
        self.on_decoded(&Decoded::new(ev));
    }

    fn on_decoded(&mut self, ev: &Decoded<'_>) {
        if ev.channel() == GtKinematicsView::CHANNEL {
            ev.with(|v: Option<&GtKinematicsView>| match v {
                Some(v) => self.on_kinematics(v.clone()),
                None => self.rejected += 1,
            });
        }
    }

    fn flush(&mut self, at: SimTime) -> Vec<MetricSample> {
        // The frame in progress belongs to this window.
        self.close_frame();
        let secs = self.window_secs(at);
        let mut out = Vec::new();

        let push_distribution = |out: &mut Vec<MetricSample>, def: MetricDef, d: Distribution| {
            let min = def.min_samples;
            out.push(MetricSample::new(
                &def,
                at,
                Dims::new(),
                SampleValue::Distribution(d.summary(min)),
            ));
        };
        push_distribution(
            &mut out,
            self.def("ttc_min"),
            core::mem::replace(&mut self.ttc, Distribution::new()),
        );
        push_distribution(
            &mut out,
            self.def("pet"),
            core::mem::replace(&mut self.pet, Distribution::new()),
        );
        push_distribution(
            &mut out,
            self.def("drac"),
            core::mem::replace(&mut self.drac, Distribution::new()),
        );
        push_distribution(
            &mut out,
            self.def("headway_time"),
            core::mem::replace(&mut self.headway_time, Distribution::new()),
        );
        push_distribution(
            &mut out,
            self.def("headway_distance"),
            core::mem::replace(&mut self.headway_distance, Distribution::new()),
        );
        push_distribution(
            &mut out,
            self.def("speed"),
            core::mem::replace(&mut self.speed, Distribution::new()),
        );
        push_distribution(
            &mut out,
            self.def("acceleration"),
            core::mem::replace(&mut self.acceleration, Distribution::new()),
        );
        out.push(MetricSample::new(
            &self.def("ttc_conflicts"),
            at,
            Dims::new(),
            SampleValue::count(core::mem::take(&mut self.ttc_conflicts)),
        ));
        if let Some(tracker) = self.ssm.as_mut() {
            // Encounters idle past the extra time at the window's end belong to it.
            tracker.close_idle(at as f64 / 1e9);
            let log = tracker.take_conflicts();
            self.log_conflicts(log);
            let counts = core::mem::take(&mut self.ssm_counts);
            for ty in EncounterType::ALL {
                out.push(MetricSample::new(
                    &self.def(&ssm_count_name(ty)),
                    at,
                    Dims::new(),
                    SampleValue::count(counts.get(&ty).copied().unwrap_or(0)),
                ));
            }
            let dists = [
                ("ssm_min_ttc", core::mem::replace(&mut self.ssm_min_ttc, Distribution::new())),
                ("ssm_max_drac", core::mem::replace(&mut self.ssm_max_drac, Distribution::new())),
                (
                    "ssm_max_mdrac",
                    core::mem::replace(&mut self.ssm_max_mdrac, Distribution::new()),
                ),
                ("ssm_min_ppet", core::mem::replace(&mut self.ssm_min_ppet, Distribution::new())),
            ];
            for (name, d) in dists {
                push_distribution(&mut out, self.def(name), d);
            }
        }

        // --- Edie's aggregates, per lane -------------------------------------------------
        let flow_def = self.def("flow");
        let density_def = self.def("density");
        let speed_def = self.def("mean_speed");
        // The network's: Edie over the union of the declared lanes — Σ distance and
        // Σ time over every lane, over Σ L·T — which is the macroscopic fundamental
        // diagram's production and accumulation per lane-kilometre (Geroliminis and
        // Daganzo, Transp. Res. B 42(9), 2008). Summed in lane-id order.
        let (net_samples, net_time, net_distance) =
            self.lanes
                .values()
                .fold((0u64, 0.0f64, 0.0f64), |(n, t, d), acc| {
                    (
                        n + acc.samples,
                        t + (acc.samples as f64) * self.dt.as_secs_f64(),
                        d + acc.distance.sum(1).point().unwrap_or(0.0),
                    )
                });
        if let Some(t) = secs
            && self.network_length_m > 0.0
            && t > 0.0
        {
            let area = self.network_length_m * t;
            out.push(MetricSample::new(
                &flow_def,
                at,
                Dims::new(),
                SampleValue::Scalar(Estimate::Value {
                    point: net_distance / area * 3600.0,
                    n: net_samples,
                }),
            ));
            out.push(MetricSample::new(
                &density_def,
                at,
                Dims::new(),
                SampleValue::Scalar(Estimate::Value {
                    point: net_time / area * 1000.0,
                    n: net_samples,
                }),
            ));
            out.push(MetricSample::new(
                &speed_def,
                at,
                Dims::new(),
                SampleValue::Ratio(ratio_of_sums(net_distance, net_time, net_samples, 1)),
            ));
        }
        let lanes = core::mem::take(&mut self.lanes);
        for (lane, acc) in lanes.into_iter().filter(|_| self.per_lane) {
            let mut dims = Dims::new();
            dims.insert(Dim::Region, DimValue::index(u64::from(lane)));
            let length = self.lane_length_m.get(&lane).copied().unwrap_or(0.0);
            let time_spent = (acc.samples as f64) * self.dt.as_secs_f64();
            let distance = acc.distance.sum(1).point().unwrap_or(0.0);
            let (flow, density, mean) = match secs {
                Some(t) if length > 0.0 && t > 0.0 => {
                    let area = length * t;
                    (
                        Estimate::Value {
                            point: distance / area * 3600.0,
                            n: acc.samples,
                        },
                        Estimate::Value {
                            point: time_spent / area * 1000.0,
                            n: acc.samples,
                        },
                        // The space-mean speed is q/k = distance/time_spent. A lane with no
                        // time spent on it has no mean speed, which is what ratio_of_sums
                        // answers.
                        ratio_of_sums(distance, time_spent, acc.samples, 1),
                    )
                }
                _ => (
                    Estimate::Insufficient {
                        n: acc.samples,
                        required: 1,
                    },
                    Estimate::Insufficient {
                        n: acc.samples,
                        required: 1,
                    },
                    crate::stats::RatioEstimate::Insufficient {
                        trials: acc.samples,
                        required: 1,
                        successes: None,
                    },
                ),
            };
            out.push(MetricSample::new(
                &flow_def,
                at,
                dims.clone(),
                SampleValue::Scalar(flow),
            ));
            out.push(MetricSample::new(
                &density_def,
                at,
                dims.clone(),
                SampleValue::Scalar(density),
            ));
            out.push(MetricSample::new(
                &speed_def,
                at,
                dims,
                SampleValue::Ratio(mean),
            ));
        }

        // PET cells are kept across windows: a conflict spans the boundary as easily as not.
        // They are pruned to the horizon so the map does not grow with the run.
        let cutoff = at.saturating_sub(self.pet_horizon.as_nanos());
        self.cell_last.retain(|_, (_, t)| *t >= cutoff);
        self.window_start = at;
        out
    }

    fn rejected(&self) -> u64 {
        self.rejected
    }
}

/// The count metric of one encounter type: `ssm_conflicts_<type>`, or `ssm_collisions`.
#[must_use]
pub fn ssm_count_name(ty: EncounterType) -> String {
    if ty == EncounterType::Collision {
        "ssm_collisions".to_string()
    } else {
        format!("ssm_conflicts_{}", ty.label())
    }
}

/// The names of the `ssm` group's metrics.
#[must_use]
pub fn ssm_metric_names() -> Vec<String> {
    let mut v: Vec<String> = EncounterType::ALL.iter().map(|t| ssm_count_name(*t)).collect();
    v.extend(["ssm_min_ttc", "ssm_max_drac", "ssm_max_mdrac", "ssm_min_ppet"].map(String::from));
    v
}

fn ssm_definitions(min_samples: u64) -> Vec<MetricDef> {
    let sumo = cards::paper(
        "SUMO documentation, Simulation/Output/SSM Device (thresholds TTC 3.0 s, DRAC 3.0, \
         MDRAC 3.4, PPET 2.0 s, range 50 m, extratime 5 s); Gettman et al. 2008, \
         FHWA-HRT-08-051 (SSAM); Laureshyn et al. 2010; Fazekas et al. 2017",
    );
    let mut defs: Vec<MetricDef> = EncounterType::ALL
        .iter()
        .map(|ty| {
            let what = if *ty == EncounterType::Collision {
                "Encounters whose bodies overlapped (SUMO code 111), logged when the \
                 encounter closed."
                    .to_string()
            } else {
                format!(
                    "Conflicts of type {} (SUMO code {}): closed encounters whose minimum \
                     TTC fell below 3.0 s, maximum DRAC or MDRAC rose above 3.0 or 3.4 \
                     m/s², or minimum PPET fell below 2.0 s; the type is the one at the \
                     minimum TTC.",
                    ty.label(),
                    ty.sumo_code()
                )
            };
            MetricDef::new(
                ssm_count_name(*ty),
                "count",
                Agg::Count,
                Visibility::Gt,
                Quantum::COUNT,
                what,
            )
            .with_dims([Dim::T])
            .with_source(sumo.clone())
            .with_min_samples(1)
            .not_accounting_for("an encounter still open at the run's last window")
            .not_accounting_for("a turning vehicle's curved path: extrapolation is straight")
        })
        .collect();
    for (name, unit, q, what) in [
        (
            "ssm_min_ttc",
            "s",
            Quantum::TIME_S,
            "Each logged conflict's minimum time to collision, every encounter type: the \
             first overlap of the two bodies' rectangles extrapolated at current velocity \
             (Laureshyn et al. 2010), or gap / Δv for a same-lane pair.",
        ),
        (
            "ssm_max_drac",
            "m/s²",
            Quantum::SPEED,
            "Each logged conflict's maximum deceleration rate to avoid the crash.",
        ),
        (
            "ssm_max_mdrac",
            "m/s²",
            Quantum::SPEED,
            "Each logged conflict's maximum MDRAC: the DRAC after a 1 s perception-reaction \
             time (Fazekas et al. 2017).",
        ),
        (
            "ssm_min_ppet",
            "s",
            Quantum::TIME_S,
            "Each logged conflict's minimum predicted post-encroachment time at a crossing \
             or merging conflict area, when no collision was predicted.",
        ),
    ] {
        defs.push(
            MetricDef::new(name, unit, Agg::Distribution, Visibility::Gt, q, what)
                .with_dims([Dim::T])
                .with_source(sumo.clone())
                .with_min_samples(min_samples)
                .not_accounting_for("an encounter still open at the run's last window"),
        );
    }
    defs
}

fn add_ssm_card(card: &mut ModelCard, p: &SsmParams) {
    card.equations.push(v2xw_core::card::Equation::new(
        "ssm_ttc",
        "TTC = first t >= 0 at which the two bodies' rectangles, moved at constant velocity, \
         overlap on all four separating axes (Laureshyn et al. 2010 extended TTC); a \
         same-lane pair keeps gap / Δv",
    ));
    card.equations.push(v2xw_core::card::Equation::new(
        "ssm_drac_crossing",
        "DRAC = 2(v·T − d)/T² for the later arrival, d its distance to the conflict area and \
         T the earlier vehicle's exit time; v²/(2d) when that would stop it short",
    ));
    card.equations.push(v2xw_core::card::Equation::new(
        "ssm_mdrac",
        "MDRAC = Δv / (2(TTC − R)) (Fazekas et al. 2017); crossing: the DRAC rule with d − vR \
         and T − R",
    ));
    card.equations.push(v2xw_core::card::Equation::new(
        "ssm_ppet",
        "PPET = t_in(second) − t_out(first) over the conflict area at current speeds",
    ));
    let sumo = |what: &str| {
        cards::paper(&format!(
            "SUMO documentation, Simulation/Output/SSM Device: default {what}"
        ))
    };
    for (name, unit, v, lo, hi, what) in [
        ("ssm_ttc_threshold_s", "s", p.ttc_s, 0.1, 10.0, "TTC threshold 3.0 s"),
        ("ssm_drac_threshold_mps2", "m/s²", p.drac_mps2, 0.1, 10.0, "DRAC threshold 3.0 m/s²"),
        ("ssm_mdrac_threshold_mps2", "m/s²", p.mdrac_mps2, 0.1, 10.0, "MDRAC threshold 3.4 m/s²"),
        ("ssm_ppet_threshold_s", "s", p.ppet_s, 0.1, 10.0, "PPET threshold 2.0 s"),
        ("ssm_range_m", "m", p.range_m, 1.0, 500.0, "range 50 m"),
        ("ssm_extra_time_s", "s", p.extra_time_s, 0.0, 60.0, "extratime 5 s"),
        ("ssm_reaction_s", "s", p.reaction_s, 0.0, 5.0, "mdrac.prt 1 s"),
    ] {
        card.parameters
            .push(cards::param(name, unit, json!(v), json!(lo), json!(hi), sumo(what)));
    }
    card.sources.push(cards::paper(
        "D. Gettman, L. Pu, T. Sayed, S. Shelby, Surrogate Safety Assessment Model and \
         Validation: Final Report, FHWA-HRT-08-051, 2008 (conflict-angle classes 30°/85°)",
    ));
    card.sources.push(cards::paper(
        "A. Laureshyn, Å. Svensson, C. Hydén, Evaluation of traffic safety, based on \
         micro-level behavioural data, Accident Analysis and Prevention 42(6), 2010 \
         (extended TTC by extrapolation)",
    ));
    card.sources.push(cards::paper(
        "A. Fazekas, F. Hennecke, E. Kalló, M. Horváth, A novel surrogate indicator based on \
         constant initial acceleration and reaction time assumption, J. Advanced \
         Transportation 2017 (MDRAC)",
    ));
    card.limitations.push(
        "Full SSM: trajectories are extrapolated in a straight line at the current velocity, \
         so a turning vehicle's predicted path leaves its lane; SUMO's device extrapolates \
         along the route. Bodies are the class's declared length and width. Re-implemented \
         from the cited papers and SUMO's documentation; SUMO was read for behaviour only \
         and no code was copied."
            .to_string(),
    );
    card.validation
        .tests
        .push("ssm::tests::a_hand_computed_crossing_case_matches".to_string());
    card.validation
        .tests
        .push("safety::tests::the_ssm_following_results_equal_the_same_lane_measures".to_string());
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::ctx::OwnedRecord;

    fn kin(
        t: SimTime,
        actor: u32,
        lane: Option<u32>,
        pos: f64,
        speed: f64,
        acc: Option<f64>,
    ) -> OwnedRecord {
        let mut j = json!({
            "t": t, "actor": actor, "x_m": pos, "y_m": 0.0, "speed_mps": speed,
        });
        if let Some(l) = lane {
            j["lane"] = json!(l);
            j["lane_pos_m"] = json!(pos);
        }
        if let Some(a) = acc {
            j["acc_mps2"] = json!(a);
        }
        OwnedRecord {
            channel: "gt.kinematics",
            visibility: Visibility::Gt,
            json: serde_json::to_vec(&j).unwrap(),
        }
    }

    fn sample<'a>(samples: &'a [MetricSample], key: &str) -> &'a MetricSample {
        samples.iter().find(|s| s.key() == key).unwrap_or_else(|| {
            panic!(
                "no sample with key {key}; have {:?}",
                samples.iter().map(MetricSample::key).collect::<Vec<_>>()
            )
        })
    }

    #[test]
    fn every_definition_validates_and_the_card_is_accepted() {
        let p = SafetyProvider::new(0);
        p.validate_defs().unwrap();
        p.card().validate().unwrap();
        p.card().check_api_version().unwrap();
        // The two todo-calibrate parameters carry their plans (registry rule R1).
        let todo: Vec<&str> = p.card().todo_calibrate().map(|x| x.name.as_str()).collect();
        assert_eq!(todo, vec!["pet_cell_m", "pet_horizon_s"]);
    }

    /// The hand-computed fixture: a follower at 0 m doing 20 m/s behind a leader at 30 m
    /// doing 10 m/s. Gap 30 m, closing speed 10 m/s, so TTC = 3 s, time headway =
    /// 30/20 = 1.5 s, distance headway 30 m, DRAC = 10²/(2·30) = 1.666… m/s².
    #[test]
    fn ttc_and_headway_reproduce_a_hand_computed_fixture() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, Some(7), 0.0, 20.0, Some(0.5)));
        p.on_event(&kin(0, 2, Some(7), 30.0, 10.0, Some(-1.0)));
        let s = p.flush(1_000_000_000);
        assert_eq!(sample(&s, "ttc_min").value.point(), Some(3.0));
        assert_eq!(sample(&s, "headway_time").value.point(), Some(1.5));
        assert_eq!(sample(&s, "headway_distance").value.point(), Some(30.0));
        assert_eq!(sample(&s, "drac").value.point(), Some(1.667));
        assert_eq!(
            sample(&s, "ttc_conflicts").value,
            SampleValue::count(0),
            "3 s is above the 1.5 s threshold"
        );
        // Speeds 10 and 20: mean 15. Accelerations 0.5 and −1.0: mean −0.25.
        assert_eq!(sample(&s, "speed").value.point(), Some(15.0));
        assert_eq!(sample(&s, "acceleration").value.point(), Some(-0.25));

        // A creeping queue (0.05 m/s, 7 m gap) is queued, not following: it has a
        // distance headway and no time headway.
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, Some(7), 0.0, 0.05, Some(0.0)));
        p.on_event(&kin(0, 2, Some(7), 7.0, 0.05, Some(0.0)));
        let s = p.flush(1_000_000_000);
        assert_eq!(sample(&s, "headway_distance").value.point(), Some(7.0));
        assert_eq!(sample(&s, "headway_time").value.point(), None);

        // A free-flowing pair (200 m at 10 m/s, 20 s) is not following either.
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, Some(7), 0.0, 10.0, Some(0.0)));
        p.on_event(&kin(0, 2, Some(7), 200.0, 10.0, Some(0.0)));
        let s = p.flush(1_000_000_000);
        assert_eq!(sample(&s, "headway_time").value.point(), None);
    }

    #[test]
    fn a_pair_that_is_not_closing_has_no_ttc() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        // The leader is faster, so the gap is opening.
        p.on_event(&kin(0, 1, Some(7), 0.0, 10.0, None));
        p.on_event(&kin(0, 2, Some(7), 30.0, 20.0, None));
        let s = p.flush(1_000_000_000);
        assert!(sample(&s, "ttc_min").value.is_insufficient());
        assert!(sample(&s, "drac").value.is_insufficient());
        // …but the headways are still defined.
        assert_eq!(sample(&s, "headway_distance").value.point(), Some(30.0));
    }

    #[test]
    fn a_ttc_below_the_threshold_is_counted_and_the_comparison_is_quantised() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        // Gap 10 m, closing 10 m/s → TTC 1.0 s, below 1.5 s.
        p.on_event(&kin(0, 1, Some(1), 0.0, 20.0, None));
        p.on_event(&kin(0, 2, Some(1), 10.0, 10.0, None));
        // Gap 15 m, closing 10 m/s → TTC exactly 1.5 s, which is *not* below the threshold.
        p.on_event(&kin(0, 3, Some(2), 0.0, 20.0, None));
        p.on_event(&kin(0, 4, Some(2), 15.0, 10.0, None));
        let s = p.flush(1_000_000_000);
        assert_eq!(sample(&s, "ttc_conflicts").value, SampleValue::count(1));
    }

    #[test]
    fn a_stopped_follower_has_no_time_headway_and_no_infinity() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, Some(7), 0.0, 0.0, None));
        p.on_event(&kin(0, 2, Some(7), 30.0, 0.0, None));
        let s = p.flush(1_000_000_000);
        assert!(sample(&s, "headway_time").value.is_insufficient());
        assert_eq!(sample(&s, "headway_distance").value.point(), Some(30.0));
        for f in s.iter().flat_map(MetricSample::floats) {
            assert!(f.is_finite(), "{f}");
        }
    }

    /// Edie on a 100 m lane over a 1 s window with 100 ms samples: one vehicle present for
    /// all ten samples at 20 m/s.
    ///
    /// `T_s = 10·0.1 = 1 s`, `D = 10·20·0.1 = 20 m`, `L·T = 100 m·1 s = 100 m·s`.
    /// `k = 1/100 = 0.01 veh/m = 10 veh/km`; `q = 20/100 = 0.2 veh/s = 720 veh/h`;
    /// `v̄ = 20/1 = 20 m/s`.
    #[test]
    fn edies_aggregates_reproduce_a_hand_computed_fixture() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.declare_lane_length(7, 100.0);
        for i in 0..10u64 {
            p.on_event(&kin(
                i * 100_000_000,
                1,
                Some(7),
                (i as f64) * 2.0,
                20.0,
                None,
            ));
        }
        let s = p.flush(1_000_000_000);
        assert_eq!(sample(&s, "density|region=7").value.point(), Some(10.0));
        assert_eq!(sample(&s, "flow|region=7").value.point(), Some(720.0));
        assert_eq!(sample(&s, "mean_speed|region=7").value.point(), Some(20.0));
        // The network's headline is the same lane here: one lane is the whole network.
        assert_eq!(sample(&s, "density").value.point(), Some(10.0));
        assert_eq!(sample(&s, "flow").value.point(), Some(720.0));
        assert_eq!(sample(&s, "mean_speed").value.point(), Some(20.0));
        // And the identity q = k·v̄ holds on the reported numbers: 10 veh/km × 20 m/s =
        // 10 veh/km × 72 km/h = 720 veh/h.
        assert_eq!(10.0 * 72.0, 720.0);
    }

    /// Over a network of two 100 m lanes with one vehicle on one of them, the network's
    /// density is half the lane's, its flow half, and its space-mean speed the vehicle's;
    /// a pedestrian on the other lane is not traffic.
    #[test]
    fn the_network_aggregate_is_over_every_declared_lane_and_leaves_pedestrians_out() {
        let mut p = SafetyProvider::new(0)
            .with_min_samples(1)
            .with_per_lane(false);
        p.declare_lane_length(7, 100.0);
        p.declare_lane_length(8, 100.0);
        for i in 0..10u64 {
            p.on_event(&kin(
                i * 100_000_000,
                1,
                Some(7),
                (i as f64) * 2.0,
                20.0,
                None,
            ));
            let mut ped: serde_json::Value =
                serde_json::from_slice(&kin(i * 100_000_000, 2, Some(8), 0.0, 1.0, None).json)
                    .unwrap();
            ped["class"] = json!("pedestrian");
            p.on_event(&OwnedRecord {
                channel: "gt.kinematics",
                visibility: Visibility::Gt,
                json: serde_json::to_vec(&ped).unwrap(),
            });
        }
        let s = p.flush(1_000_000_000);
        assert_eq!(sample(&s, "density").value.point(), Some(5.0));
        assert_eq!(sample(&s, "flow").value.point(), Some(360.0));
        assert_eq!(sample(&s, "mean_speed").value.point(), Some(20.0));
        assert!(!s.iter().any(|x| x.metric == "flow" && !x.dims.is_empty()));
    }

    #[test]
    fn an_undeclared_lane_has_no_aggregate_and_is_counted() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, Some(42), 0.0, 20.0, None));
        let s = p.flush(1_000_000_000);
        assert!(!s.iter().any(|x| x.metric == "flow"));
        assert!(p.undeclared_lanes().contains(&42));
    }

    #[test]
    fn pet_needs_two_different_actors_in_one_cell() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        // Actor 1 occupies the cell at t = 0; actor 2 arrives 1.5 s later.
        p.on_event(&kin(0, 1, None, 0.25, 5.0, None));
        p.on_event(&kin(1_500_000_000, 2, None, 0.25, 5.0, None));
        let s = p.flush(3_000_000_000);
        assert_eq!(sample(&s, "pet").value.point(), Some(1.5));

        // The same actor revisiting its own cell is not a conflict.
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, None, 0.25, 5.0, None));
        p.on_event(&kin(1_500_000_000, 1, None, 0.25, 5.0, None));
        let s = p.flush(3_000_000_000);
        assert!(sample(&s, "pet").value.is_insufficient());

        // And a gap beyond the horizon is not a conflict either.
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, None, 0.25, 5.0, None));
        p.on_event(&kin(9_000_000_000, 2, None, 0.25, 5.0, None));
        let s = p.flush(10_000_000_000);
        assert!(sample(&s, "pet").value.is_insufficient());
    }

    #[test]
    fn an_empty_window_is_insufficient_and_never_nan() {
        let mut p = SafetyProvider::new(0);
        let s = p.flush(1_000_000_000);
        for key in [
            "ttc_min",
            "pet",
            "drac",
            "headway_time",
            "headway_distance",
            "speed",
            "acceleration",
        ] {
            assert!(sample(&s, key).value.is_insufficient(), "{key}");
        }
        assert_eq!(sample(&s, "ttc_conflicts").value, SampleValue::count(0));
        for f in s.iter().flat_map(MetricSample::floats) {
            assert!(f.is_finite(), "{f}");
        }
    }

    /// Permuting the actors within a frame must not move a bit: the pairing sorts by the
    /// integer lane-position grid index, and the distributions sort before they reduce.
    #[test]
    fn a_frames_results_do_not_depend_on_the_order_of_its_actors() {
        let positions = [(1u32, 0.0, 22.5), (2, 13.75, 18.0), (3, 41.125, 9.5)];
        let run = |order: Vec<usize>| {
            let mut p = SafetyProvider::new(0).with_min_samples(1);
            p.declare_lane_length(3, 200.0);
            for i in order {
                let (a, pos, v) = positions[i];
                p.on_event(&kin(0, a, Some(3), pos, v, Some(0.125)));
            }
            p.flush(1_000_000_000)
                .into_iter()
                .map(|s| {
                    (
                        s.key(),
                        s.floats().iter().map(|f| f.to_bits()).collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let forward = run(vec![0, 1, 2]);
        assert_eq!(forward, run(vec![2, 1, 0]));
        assert_eq!(forward, run(vec![1, 2, 0]));
    }

    #[test]
    fn every_safety_metric_is_tagged_ground_truth() {
        for d in SafetyProvider::new(0).defs() {
            assert_eq!(d.visibility, Visibility::Gt, "{}", d.name);
            assert!(
                !d.visibility.allowed_on_node_channel(),
                "{} must not reach a node channel",
                d.name
            );
        }
    }

    #[test]
    fn a_duplicate_sample_for_one_actor_in_one_frame_does_not_double_count() {
        let mut p = SafetyProvider::new(0).with_min_samples(1);
        p.on_event(&kin(0, 1, Some(7), 0.0, 20.0, None));
        p.on_event(&kin(0, 1, Some(7), 0.0, 20.0, None));
        let s = p.flush(1_000_000_000);
        assert_eq!(sample(&s, "speed").value.n(), 1);
    }

    fn kin_h(t: SimTime, actor: u32, lane: Option<u32>, x: f64, y: f64, h: f64, v: f64) -> OwnedRecord {
        let mut j = json!({
            "t": t, "actor": actor, "x_m": x, "y_m": y, "speed_mps": v, "heading_rad": h,
        });
        if let Some(l) = lane {
            j["lane"] = json!(l);
            j["lane_pos_m"] = json!(x);
        }
        OwnedRecord {
            channel: "gt.kinematics",
            visibility: Visibility::Gt,
            json: serde_json::to_vec(&j).unwrap(),
        }
    }

    fn dist_min(s: &MetricSample) -> Option<f64> {
        match &s.value {
            SampleValue::Distribution(crate::stats::DistributionSummary::Summary { min, .. }) => {
                Some(*min)
            }
            _ => None,
        }
    }

    /// A follower closing at 4 m/s from 20 m for 3 s, then holding its gap: with the `ssm`
    /// group on, every existing sample is byte-identical to the run without it, and the one
    /// following conflict's minimum TTC is the same-lane `ttc_min`'s minimum.
    #[test]
    fn the_ssm_following_results_equal_the_same_lane_measures() {
        let run = |ssm: bool| {
            let mut p = SafetyProvider::new(0).with_min_samples(1).with_ssm(ssm);
            for k in 0..=100u64 {
                let t = k * 100_000_000;
                let ts = k as f64 * 0.1;
                let (xf, vf) = if ts <= 3.0 { (12.0 * ts, 12.0) } else { (36.0 + 8.0 * (ts - 3.0), 8.0) };
                p.on_event(&kin_h(t, 1, Some(7), xf, 0.0, 0.0, vf));
                p.on_event(&kin_h(t, 2, Some(7), 20.0 + 8.0 * ts, 0.0, 0.0, 8.0));
            }
            p.flush(10_100_000_000)
        };
        let off = run(false);
        let on = run(true);
        for s in &off {
            let twin = on.iter().find(|o| o.key() == s.key()).expect("every sample kept");
            assert_eq!(
                serde_json::to_string(s).unwrap(),
                serde_json::to_string(twin).unwrap()
            );
        }
        assert!(off.iter().all(|s| !s.metric.starts_with("ssm_")));
        assert_eq!(
            sample(&on, "ssm_conflicts_following").value,
            SampleValue::count(1)
        );
        let ttc = dist_min(sample(&on, "ttc_min")).expect("a ttc");
        assert_eq!(dist_min(sample(&on, "ssm_min_ttc")), Some(ttc));
        assert_eq!(sample(&on, "ssm_conflicts_crossing").value, SampleValue::count(0));
    }

    /// The crossing case through the provider: two cars on different lanes, headings 0
    /// and π/2, meeting at the origin as in `ssm::tests`: one crossing conflict with the
    /// hand-computed 2.5 s minimum TTC at the first frame; the same-lane measures see
    /// nothing.
    #[test]
    fn the_provider_logs_the_crossing_conflict() {
        let mut p = SafetyProvider::new(0).with_min_samples(1).with_ssm(true);
        p.declare_class_dims("car", 4.0, 2.0);
        for k in 0..=80u64 {
            let t = k * 100_000_000;
            let ts = k as f64 * 0.1;
            let stop = ts >= 0.5;
            let (xa, ya) = if stop { (-25.0, 0.0) } else { (-30.0 + 10.0 * ts, 0.0) };
            let (xb, yb) = if stop { (0.0, -16.0) } else { (0.0, -20.0 + 8.0 * ts) };
            let mut a = kin_h(t, 1, Some(1), xa, ya, 0.0, if stop { 0.0 } else { 10.0 });
            let mut b = kin_h(t, 2, Some(2), xb, yb, core::f64::consts::FRAC_PI_2, if stop { 0.0 } else { 8.0 });
            for r in [&mut a, &mut b] {
                let mut j: serde_json::Value = serde_json::from_slice(&r.json).unwrap();
                j["class"] = json!("car");
                r.json = serde_json::to_vec(&j).unwrap();
            }
            p.on_event(&a);
            p.on_event(&b);
        }
        let s = p.flush(8_100_000_000);
        assert_eq!(sample(&s, "ssm_conflicts_crossing").value, SampleValue::count(1));
        assert_eq!(sample(&s, "ssm_conflicts_following").value, SampleValue::count(0));
        // TTC 2.5 − t falls to 2.1 at the last moving frame (0.4 s).
        assert_eq!(dist_min(sample(&s, "ssm_min_ttc")), Some(2.1));
        assert_eq!(sample(&s, "ttc_min").value.n(), 0);
    }

    #[test]
    fn the_ssm_definitions_validate_and_the_card_is_accepted() {
        let p = SafetyProvider::new(0).with_ssm(true);
        p.validate_defs().unwrap();
        p.card().validate().unwrap();
        let names: Vec<String> = p.defs().into_iter().map(|d| d.name).collect();
        for n in ssm_metric_names() {
            assert!(names.contains(&n), "{n}");
        }
    }
}
