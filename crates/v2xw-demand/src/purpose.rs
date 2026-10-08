//! `mobility/demand/purposes` — trip purposes, tours and departure-time profiles.
//!
//! A day's travel is generated as **tours**: a home-based tour leaves home at a time drawn
//! from its purpose's departure profile, stays at the activity for a drawn duration, and
//! comes back; a non-home-based or commercial trip is a single leg. Purposes follow the
//! classification of trip-based travel models — home-based work, school, shopping and
//! other, non-home-based, and commercial vehicles [NCHRP Report 716, *Travel Demand
//! Forecasting: Parameters and Techniques*, Transportation Research Board 2012, ch. 4] —
//! and SUMO's `activitygen` builds the same kind of day from the same ingredients
//! [SUMO ACTIVITYGEN documentation].
//!
//! Homes and activities are lanes drawn by the zone weights of [`crate::od::zones`]
//! (building floor area); the activity is drawn with a distance decay
//! `w_j · exp(−d_ij / d₀)` from the home, `d₀` per purpose.
//!
//! **The numbers.** The purpose shares, the departure-time profiles and the activity
//! durations shipped here are uncalibrated shapes — morning and evening work peaks, a
//! school peak, a midday-and-afternoon shopping profile — and the card marks every one
//! `todo-calibrate` with its plan: fit to the 2017 NHTS trip file for the study region
//! [FHWA, *2017 National Household Travel Survey*]. A study that has its own profiles
//! passes them (`profiles`), as hourly shares or as any [`TimeOfDayCurve`].
//!
//! # The daily profile, and multi-day runs
//!
//! Trips are generated **one day at a time**, the day's tours keyed by `(seed, day,
//! purpose, tour)`, as the run reaches that day — so a week-long run costs one day's
//! memory, day 3 is the same whatever happened on day 2, and a run restored from a
//! checkpoint on day 3 regenerates exactly day 3's trips. The long-run engine's daily
//! profile plugs in two ways: through the [`v2xw_mobility::traits::Demand::set_multiplier`]
//! hook (thinning by trip id, as [`crate::models::TripListDemand`] does), and by handing a
//! [`TimeOfDayCurve`] to [`DepartureProfile::from_curve`] to replace a purpose's profile.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::ids::LaneId;
use v2xw_core::math;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_core::time::{NS_PER_S, SimTime, ns_to_secs, secs_to_ns};
use v2xw_mobility::demand::SpeedLaw;
use v2xw_mobility::traits::Demand;
use v2xw_mobility::views::TripRequest;
use v2xw_mobility::{MobCtx, VehicleClass};
use v2xw_world::{ClassMask, World};

use crate::error::{DemandError, Result};
use crate::graph::LaneGraph;
use crate::od::zones::{MIN_ENDPOINT_LANE_M, ZoneWeight, mid, weights};
use crate::trips::Trip;

/// The model id.
pub const MODEL_ID: &str = "mobility/demand/purposes";

/// Seconds in a day.
pub const DAY_S: f64 = 86_400.0;

/// Why a trip is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Purpose {
    /// Home-based work.
    HomeBasedWork,
    /// Home-based school.
    HomeBasedSchool,
    /// Home-based shopping.
    HomeBasedShop,
    /// Home-based other (personal business, social, recreation).
    HomeBasedOther,
    /// Non-home-based.
    NonHomeBased,
    /// Commercial vehicles (deliveries, service).
    Commercial,
}

impl Purpose {
    /// Every purpose.
    pub const ALL: [Purpose; 6] = [
        Purpose::HomeBasedWork,
        Purpose::HomeBasedSchool,
        Purpose::HomeBasedShop,
        Purpose::HomeBasedOther,
        Purpose::NonHomeBased,
        Purpose::Commercial,
    ];

    /// A stable label.
    pub const fn label(self) -> &'static str {
        match self {
            Purpose::HomeBasedWork => "home-based-work",
            Purpose::HomeBasedSchool => "home-based-school",
            Purpose::HomeBasedShop => "home-based-shop",
            Purpose::HomeBasedOther => "home-based-other",
            Purpose::NonHomeBased => "non-home-based",
            Purpose::Commercial => "commercial",
        }
    }

    /// True for a purpose whose trips come in out-and-back tours.
    pub const fn is_tour(self) -> bool {
        !matches!(self, Purpose::NonHomeBased | Purpose::Commercial)
    }

    const fn index(self) -> u64 {
        self as u64
    }
}

/// A time-of-day weighting: what the long-run engine's daily profile, or any study's own
/// curve, implements to drive departures here.
pub trait TimeOfDayCurve {
    /// The relative weight of departures at `s_of_day` (seconds after midnight). Need not
    /// be normalised.
    fn weight_at(&self, s_of_day: f64) -> f64;
}

impl<F: Fn(f64) -> f64> TimeOfDayCurve for F {
    fn weight_at(&self, s_of_day: f64) -> f64 {
        self(s_of_day)
    }
}

/// A departure-time distribution over the day.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum DepartureProfile {
    /// Every instant equally likely.
    Uniform,
    /// Piecewise-constant weights in bins of `bin_s` from midnight (24 hourly shares is
    /// `bin_s: 3600`).
    Binned {
        /// Bin width, seconds.
        bin_s: f64,
        /// Relative weights, one per bin.
        weights: Vec<f64>,
    },
    /// A mixture of normal peaks on the time of day, `(weight, mean_s, sd_s)` each.
    Peaks {
        /// The peaks.
        peaks: Vec<(f64, f64, f64)>,
    },
}

impl TimeOfDayCurve for DepartureProfile {
    fn weight_at(&self, s: f64) -> f64 {
        let s = s.rem_euclid(DAY_S);
        match self {
            DepartureProfile::Uniform => 1.0,
            DepartureProfile::Binned { bin_s, weights } => {
                if weights.is_empty() || !(*bin_s > 0.0) {
                    return 1.0;
                }
                let i = ((s / bin_s).floor() as usize) % weights.len();
                weights[i].max(0.0)
            }
            DepartureProfile::Peaks { peaks } => {
                math::sum_ordered(peaks.iter().map(|(w, m, sd)| {
                    // The peak wrapped once either side of midnight.
                    let sd = sd.max(1.0);
                    [-DAY_S, 0.0, DAY_S]
                        .iter()
                        .map(|k| {
                            let z = (s - m - k) / sd;
                            w * math::exp(-0.5 * z * z) / sd
                        })
                        .sum::<f64>()
                }))
            }
        }
    }
}

/// The grid on which profiles are sampled, seconds.
const GRID_S: f64 = 60.0;

impl DepartureProfile {
    /// A profile that follows `curve`, sampled every `bin_s` seconds over the day: how the
    /// long-run engine's daily profile becomes a departure profile.
    pub fn from_curve(curve: &dyn TimeOfDayCurve, bin_s: f64) -> Self {
        let bin_s = if bin_s > 0.0 { bin_s } else { 900.0 };
        let n = (DAY_S / bin_s).ceil() as usize;
        DepartureProfile::Binned {
            bin_s,
            weights: (0..n)
                .map(|i| curve.weight_at((i as f64 + 0.5) * bin_s).max(0.0))
                .collect(),
        }
    }

    /// Draws an instant in `[a_s, b_s)` (seconds of the day; `b_s` may exceed a day) with
    /// density proportional to the profile, by inverse CDF on a one-minute grid and
    /// uniformly within the minute. Two draws from `rng`.
    pub fn sample_between(&self, rng: &mut RngStream, a_s: f64, b_s: f64) -> f64 {
        if !(b_s > a_s) {
            return a_s;
        }
        if *self == DepartureProfile::Uniform {
            let t = rng.uniform(a_s, b_s);
            let _ = rng.f64();
            return t;
        }
        let n = ((b_s - a_s) / GRID_S).ceil().max(1.0) as usize;
        let w: Vec<f64> = (0..n)
            .map(|i| {
                let lo = a_s + i as f64 * GRID_S;
                let hi = (lo + GRID_S).min(b_s);
                self.weight_at((lo + hi) / 2.0) * (hi - lo)
            })
            .collect();
        let total = math::sum_ordered(w.iter().copied());
        let u = rng.f64();
        let v = rng.f64();
        if !(total > 0.0) {
            return a_s + u * (b_s - a_s);
        }
        let target = u * total;
        let mut acc = 0.0;
        for (i, wi) in w.iter().enumerate() {
            acc += wi;
            if target < acc || i == n - 1 {
                let lo = a_s + i as f64 * GRID_S;
                let hi = (lo + GRID_S).min(b_s);
                return lo + v * (hi - lo);
            }
        }
        b_s
    }
}

/// One purpose's day: its share of the daily trips, when tours leave, how long the
/// activity lasts, how far people go and in what.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PurposeSpec {
    /// Share of the day's trips (a tour's two legs count as two).
    pub share: f64,
    /// When the outbound leg (or the single trip) departs.
    pub departure: DepartureProfile,
    /// Activity duration, mean seconds (tours only).
    pub duration_mean_s: f64,
    /// Activity duration, standard deviation seconds.
    pub duration_sd_s: f64,
    /// The distance-decay length `d₀`, metres.
    pub decay_m: f64,
    /// The vehicle class.
    pub class: VehicleClass,
}

fn h(x: f64) -> f64 {
    x * 3600.0
}

/// The shipped, uncalibrated purpose set (see the module docs).
pub fn default_purposes() -> BTreeMap<Purpose, PurposeSpec> {
    let peaks = |p: &[(f64, f64, f64)]| DepartureProfile::Peaks {
        peaks: p.iter().map(|(w, m, sd)| (*w, h(*m), h(*sd))).collect(),
    };
    let spec = |share, dep, dm, dsd, decay, class| PurposeSpec {
        share,
        departure: dep,
        duration_mean_s: h(dm),
        duration_sd_s: h(dsd),
        decay_m: decay,
        class,
    };
    BTreeMap::from([
        (
            Purpose::HomeBasedWork,
            spec(
                0.20,
                peaks(&[(1.0, 7.75, 0.9)]),
                8.5,
                1.0,
                4000.0,
                VehicleClass::Passenger,
            ),
        ),
        (
            Purpose::HomeBasedSchool,
            spec(
                0.08,
                peaks(&[(1.0, 7.5, 0.4)]),
                7.0,
                0.5,
                1500.0,
                VehicleClass::Passenger,
            ),
        ),
        (
            Purpose::HomeBasedShop,
            spec(
                0.17,
                peaks(&[(0.4, 11.0, 2.0), (0.6, 16.5, 2.0)]),
                0.75,
                0.35,
                2000.0,
                VehicleClass::Passenger,
            ),
        ),
        (
            Purpose::HomeBasedOther,
            spec(
                0.25,
                peaks(&[(0.3, 10.0, 2.5), (0.7, 18.5, 2.5)]),
                2.0,
                1.0,
                3000.0,
                VehicleClass::Passenger,
            ),
        ),
        (
            Purpose::NonHomeBased,
            spec(
                0.25,
                peaks(&[(1.0, 12.5, 3.0)]),
                0.0,
                0.0,
                2500.0,
                VehicleClass::Passenger,
            ),
        ),
        (
            Purpose::Commercial,
            spec(
                0.05,
                peaks(&[(1.0, 10.5, 3.0)]),
                0.0,
                0.0,
                5000.0,
                VehicleClass::Delivery,
            ),
        ),
    ])
}

/// The purpose model's parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct PurposeParams {
    /// Vehicle trips per day over the whole world.
    pub daily_trips: f64,
    /// The purposes; omitted ones are not generated. `None` is [`default_purposes`].
    pub purposes: Option<BTreeMap<Purpose, PurposeSpec>>,
    /// The time of day the run starts, seconds after midnight.
    pub run_start_s_of_day: f64,
    /// Lane weights.
    pub weight: ZoneWeight,
    /// The seed of the tour draws.
    pub seed: u64,
}

impl Default for PurposeParams {
    fn default() -> Self {
        Self {
            daily_trips: 20_000.0,
            purposes: None,
            run_start_s_of_day: 0.0,
            weight: ZoneWeight::FloorArea,
            seed: 0x7_0025E,
        }
    }
}

/// The purpose model: generates any day's trips.
#[derive(Debug, Clone)]
pub struct PurposeModel {
    params: PurposeParams,
    purposes: BTreeMap<Purpose, PurposeSpec>,
    lanes: Vec<LaneId>,
    weights: Vec<f64>,
    centres: Vec<(f64, f64)>,
    card: ModelCard,
}

impl PurposeModel {
    /// The model over `world`.
    ///
    /// # Errors
    /// [`DemandError::Invalid`] when the world has no lane a car can start on, or a share
    /// is negative.
    pub fn new(world: &World, params: PurposeParams) -> Result<Self> {
        let purposes = params.purposes.clone().unwrap_or_else(default_purposes);
        if purposes.values().any(|p| !(p.share >= 0.0)) {
            return Err(DemandError::Invalid(
                "a purpose share is negative".to_string(),
            ));
        }
        let g = LaneGraph::new(world, ClassMask::CAR);
        let lanes: Vec<LaneId> = g
            .endpoints(MIN_ENDPOINT_LANE_M)
            .into_iter()
            .map(|n| g.lane(n))
            .collect();
        if lanes.len() < 2 {
            return Err(DemandError::Invalid(
                "the world has fewer than two lanes a car can start a trip on".to_string(),
            ));
        }
        let weights = weights(world, &lanes, params.weight);
        let centres = lanes
            .iter()
            .map(|l| {
                let m = mid(world, *l);
                (m.x, m.y)
            })
            .collect();
        let card = card(&params, &purposes);
        Ok(Self {
            params,
            purposes,
            lanes,
            weights,
            centres,
            card,
        })
    }

    /// The card.
    pub fn card(&self) -> &ModelCard {
        &self.card
    }

    /// Every trip of day `day` (0 = the day the run starts), departure times in seconds
    /// from that day's midnight, ids unique across days, sorted by departure.
    pub fn trips_for_day(&self, day: u64) -> Vec<Trip> {
        let p = &self.params;
        let mut out: Vec<(f64, u64, u64, LaneId, LaneId, VehicleClass, Purpose)> = Vec::new();
        for (purpose, spec) in &self.purposes {
            let legs = p.daily_trips * spec.share;
            let units = if purpose.is_tour() { legs / 2.0 } else { legs };
            let key = (day << 24) | (purpose.index() << 20);
            let mut rng = RngStream::derive(
                p.seed,
                RngDomain::plugin(MODEL_ID),
                EntityRef::custom(MODEL_ID, key),
            );
            let n = units.max(0.0).floor() as u64 + u64::from(rng.bool(units - units.floor()));
            for k in 0..n {
                let mut r = RngStream::derive(
                    p.seed,
                    RngDomain::plugin(MODEL_ID),
                    EntityRef::custom(MODEL_ID, key | (k + 1)),
                );
                let home = r.choose_index(&self.weights);
                let (hx, hy) = self.centres[home];
                let w: Vec<f64> = self
                    .weights
                    .iter()
                    .zip(&self.centres)
                    .enumerate()
                    .map(|(i, (w, (x, y)))| {
                        if i == home {
                            0.0
                        } else {
                            let d = math::hypot(x - hx, y - hy);
                            w * math::exp(-d / spec.decay_m.max(1.0))
                        }
                    })
                    .collect();
                let act = if w.iter().any(|x| *x > 0.0) {
                    r.choose_index(&w)
                } else {
                    (home + 1) % self.lanes.len()
                };
                let out_t = spec.departure.sample_between(&mut r, 0.0, DAY_S);
                let unit = (key | (k + 1)) << 1;
                out.push((
                    out_t,
                    unit,
                    0,
                    self.lanes[home],
                    self.lanes[act],
                    spec.class,
                    *purpose,
                ));
                if purpose.is_tour() {
                    let dur = r
                        .normal(spec.duration_mean_s, spec.duration_sd_s)
                        .max(600.0);
                    out.push((
                        out_t + dur,
                        unit,
                        1,
                        self.lanes[act],
                        self.lanes[home],
                        spec.class,
                        *purpose,
                    ));
                }
            }
        }
        out.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        out.into_iter()
            .map(|(t, unit, leg, o, d, class, purpose)| Trip {
                id: unit | leg,
                depart_s: math::quantize(t, 3),
                origin: o,
                destination: d,
                class,
                purpose: Some(purpose),
                tag: None,
            })
            .collect()
    }
}

/// The purpose model as a running demand: each day's trips are generated when the run
/// reaches that day.
#[derive(Debug, Clone)]
pub struct PurposeDemand {
    model: PurposeModel,
    queue: Vec<Trip>,
    cursor: usize,
    next_day: u64,
    next_seq: u64,
    multiplier: f64,
    speed: SpeedLaw,
}

impl PurposeDemand {
    /// The demand.
    pub fn new(model: PurposeModel) -> Self {
        Self {
            model,
            queue: Vec::new(),
            cursor: 0,
            next_day: 0,
            next_seq: 0,
            multiplier: 1.0,
            speed: SpeedLaw::default(),
        }
    }

    /// Generates days until the queue reaches `to_s` of run time.
    fn fill(&mut self, to_s: f64) {
        let start = self.model.params.run_start_s_of_day;
        while (self.next_day as f64) * DAY_S - start <= to_s {
            let day = self.next_day;
            self.next_day += 1;
            let offset = day as f64 * DAY_S - start;
            let mut trips: Vec<Trip> = self
                .model
                .trips_for_day(day)
                .into_iter()
                .filter_map(|mut t| {
                    t.depart_s += offset;
                    (t.depart_s >= 0.0).then_some(t)
                })
                .collect();
            // Drop the consumed prefix and merge the new day in.
            self.queue.drain(..self.cursor);
            self.cursor = 0;
            self.queue.append(&mut trips);
            crate::trips::sort_trips(&mut self.queue);
        }
    }
}

impl v2xw_core::model::Model for PurposeDemand {
    fn card(&self) -> &ModelCard {
        &self.model.card
    }
}

impl Demand for PurposeDemand {
    fn spawns_in(&mut self, ctx: &mut dyn MobCtx, _from: SimTime, to: SimTime) -> Vec<TripRequest> {
        self.fill(ns_to_secs(to));
        let mut out = Vec::new();
        while self.cursor < self.queue.len() {
            let trip = &self.queue[self.cursor];
            let depart = secs_to_ns(trip.depart_s);
            if depart >= to {
                break;
            }
            self.cursor += 1;
            let keep = {
                let whole = self.multiplier.floor();
                let frac = self.multiplier - whole;
                let mut rng = ctx.rng(
                    RngDomain::plugin(MODEL_ID),
                    EntityRef::custom(MODEL_ID, trip.id),
                );
                whole as u64 + u64::from(frac > 0.0 && rng.bool(frac))
            };
            for copy in 0..keep {
                let seq = self.next_seq;
                self.next_seq += 1;
                let desired_speed_mps = {
                    let mut rng =
                        ctx.rng(RngDomain::DesiredSpeed, EntityRef::custom(MODEL_ID, seq));
                    self.speed.draw(trip.class, &mut rng)
                };
                out.push(TripRequest {
                    seq,
                    t: depart + copy * NS_PER_S,
                    origin: trip.origin,
                    origin_s_m: 0.0,
                    destination: trip.destination,
                    class: trip.class,
                    desired_speed_mps,
                });
            }
        }
        out
    }

    fn set_multiplier(&mut self, m: f64) -> bool {
        if m.is_finite() && m >= 0.0 {
            self.multiplier = m;
        }
        true
    }
}

fn card(p: &PurposeParams, purposes: &BTreeMap<Purpose, PurposeSpec>) -> ModelCard {
    let nchrp = Source::new(
        SourceKind::Paper,
        "Cambridge Systematics et al., NCHRP Report 716: Travel Demand Forecasting: \
         Parameters and Techniques, Transportation Research Board 2012, ch. 4 (trip purposes, \
         time-of-day distributions)",
    );
    let ite = Source::new(
        SourceKind::Standard,
        "Institute of Transportation Engineers, Trip Generation Manual, 11th ed., 2021 \
         (trips per gross floor area)",
    );
    let sumo = Source::new(
        SourceKind::Code,
        "SUMO ACTIVITYGEN (sumo.dlr.de/docs/Demand/Activity-based_Demand_Generation.html)",
    );
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Mobility,
        "1.0.0",
        "Tour-based daily demand by trip purpose: departure-time profiles, activity \
         durations and distance-decayed destinations, generated one day at a time.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new("destination", "P(j | home i) ~ w_j exp(-d_ij / d0)"),
        Equation::new("return", "t_back = t_out + max(600 s, N(mean, sd))"),
    ];
    let plan = "fit to the 2017 NHTS trip file (FHWA) for the study's metropolitan area: \
                purpose shares, hourly departure shares and activity durations by purpose";
    let mut shares = Parameter::new(
        "purposes",
        "-",
        serde_json::to_value(purposes).unwrap_or_default(),
        Source::todo_calibrate("purpose shares, departure profiles, durations, decay"),
    );
    shares.calibration = Some(plan.to_string());
    card.parameters = vec![
        Parameter::new(
            "daily_trips",
            "veh trips/day",
            serde_json::json!(p.daily_trips),
            Source::new(SourceKind::Code, "the scenario's stated total"),
        ),
        shares,
    ];
    card.assumptions = vec![
        "Homes and activities are drawn from the same lane weights (building floor area): \
         the world does not say which buildings are residential."
            .to_string(),
        "Each tour is one vehicle; no ride sharing, no mode choice.".to_string(),
    ];
    card.sources = vec![nchrp, ite, sumo];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![
            RngDomain::plugin(MODEL_ID).as_str().to_string(),
            RngDomain::DesiredSpeed.as_str().to_string(),
        ],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "purpose::tests::a_day_follows_its_profiles".to_string(),
            "purpose::tests::days_are_independent".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_profile_samples_its_peak() {
        let p = DepartureProfile::Peaks {
            peaks: vec![(1.0, h(8.0), h(0.5))],
        };
        let mut rng = RngStream::derive(1, RngDomain::plugin(MODEL_ID), EntityRef::Global);
        let xs: Vec<f64> = (0..2000)
            .map(|_| p.sample_between(&mut rng, 0.0, DAY_S))
            .collect();
        let mean = xs.iter().sum::<f64>() / xs.len() as f64;
        assert!((mean - h(8.0)).abs() < 300.0, "{}", mean / 3600.0);
        // A curve handed over by another engine becomes a binned profile.
        let flat = DepartureProfile::from_curve(&|_s: f64| 2.0, 3600.0);
        assert!(
            matches!(flat, DepartureProfile::Binned { ref weights, .. } if weights.len() == 24)
        );
    }

    #[test]
    fn a_day_follows_its_profiles() {
        let w = crate::graph::tests::grid();
        let m = PurposeModel::new(
            &w,
            PurposeParams {
                daily_trips: 4000.0,
                ..PurposeParams::default()
            },
        )
        .expect("model");
        let trips = m.trips_for_day(0);
        assert!((3800..4200).contains(&trips.len()), "{}", trips.len());
        let work_out: Vec<f64> = trips
            .iter()
            .filter(|t| t.purpose == Some(Purpose::HomeBasedWork) && t.id & 1 == 0)
            .map(|t| t.depart_s)
            .collect();
        let mean = work_out.iter().sum::<f64>() / work_out.len() as f64;
        assert!((h(7.0)..h(8.5)).contains(&mean), "{}", mean / 3600.0);
        let ids: std::collections::BTreeSet<u64> = trips.iter().map(|t| t.id).collect();
        assert_eq!(ids.len(), trips.len(), "ids are unique");
        m.card().validate().expect("card");
    }

    #[test]
    fn days_are_independent() {
        let w = crate::graph::tests::grid();
        let m = PurposeModel::new(
            &w,
            PurposeParams {
                daily_trips: 500.0,
                ..PurposeParams::default()
            },
        )
        .expect("model");
        let d2a = m.trips_for_day(2);
        let _ = m.trips_for_day(1);
        let d2b = m.trips_for_day(2);
        assert_eq!(d2a, d2b);
        assert_ne!(m.trips_for_day(1), d2a);
    }
}
