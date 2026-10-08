//! `metrics/energy` — the metric provider that integrates the selected models over every
//! vehicle's ground-truth trajectory.
//!
//! It reads `gt.kinematics` and nothing else, so it computes the same numbers from a live
//! run, from a replayed recording and from a recording made before this crate existed —
//! and a recording can be re-scored under a different emission model or fleet without
//! re-running the simulation, which no coupled emission model in SUMO or Veins allows.
//!
//! # What it integrates
//!
//! Between two consecutive records of one actor (at most [`MAX_GAP_S`] apart) it builds
//! the interval's [`DriveState`] — mean speed, interval acceleration, grade from the
//! height change, heading change — and charges it to the actor:
//!
//! * a combustion vehicle: the emission model's rates × the interval's length;
//! * an electric vehicle: the EV model's battery power × the length, through its
//!   [`Battery`], which tracks state of charge, regeneration and depletion;
//! * a vehicle the selected emission model has no class for: nothing, and it is counted in
//!   `energy_unmodelled` — a number is never invented for it.
//!
//! The interval is charged to the lane the vehicle was on at its start. Pedestrians,
//! bicycles and e-scooters are not vehicles here and are skipped.
//!
//! # The metrics
//!
//! | Metric | Unit | Window | Dims |
//! |---|---|---|---|
//! | `co2_mass`, `fuel_mass`, `nox_mass`, `pm_mass`, `co_mass`, `hc_mass` | g | per flush | total, and by `class` |
//! | `ev_battery_energy` (net), `ev_regen_energy` | Wh | per flush | total, and by `class` |
//! | `vehicle_distance` | km | per flush | total, and by `class` |
//! | `vehicle_stops` | count | per flush | total, and by `class` |
//! | `link_co2_mass`, `link_nox_mass`, `link_pm_mass`, `link_fuel_mass`, `link_ev_energy`, `link_distance` | g, Wh, km | per flush | `region` = lane id |
//! | `co2_per_km` | g/km | run so far | total, and by `class` |
//! | `fuel_per_100km` | L/100 km | run so far | total, and by `class` |
//! | `nox_per_km` | g/km | run so far | total, and by `class` |
//! | `ev_energy_per_100km` | kWh/100 km | run so far | total, and by `class` |
//! | `stops_per_km` | 1/km | run so far | total, and by `class` |
//! | `vehicle_co2_per_km` | g/km | run so far | distribution over combustion vehicles with ≥ 0.5 km |
//! | `ev_soc_min` | ratio | run so far | lowest state of charge any EV reached |
//! | `ev_depleted` | count | run so far | EVs that needed energy their battery did not have |
//! | `energy_unmodelled` | count | run so far | vehicles the emission model has no class for |
//!
//! A network total is the headline sample (no dimensions) of a per-flush metric; a link
//! total is the sum of its `link_*` samples; a vehicle's totals are [`EnergyProvider::vehicles`].
//! The per-lane samples use `region` = lane id, the convention the safety provider's Edie
//! aggregates (`flow`, `density`, `mean_speed`) already use.
//!
//! # Determinism
//!
//! Records arrive in the run's canonical order, every accumulation happens in that order,
//! every map is a `BTreeMap` iterated in key order, the fleet draw is a keyed stream per
//! actor ([`crate::powertrain::assign`]) and every transcendental is `v2xw_core::math`.

use std::collections::BTreeMap;

use v2xw_core::card::{Family, ModelCard, SourceKind, Tier, Validation, ValidationStatus};
use v2xw_core::ctx::{ChannelName, EventRecord, Visibility};
use v2xw_core::ids::ActorId;
use v2xw_core::model::Model;
use v2xw_core::time::SimTime;
use v2xw_metrics::channels::{ChannelView, GtKinematicsView};
use v2xw_metrics::def::{Agg, Dim, DimValue, Dims, MetricDef, MetricSample, SampleValue};
use v2xw_metrics::provider::{Decoded, MetricProvider};
use v2xw_metrics::quant::Quantum;
use v2xw_metrics::stats::{Distribution, Estimate, ratio_of_sums};

use crate::cite;
use crate::ev::Battery;
use crate::moves::BRAKING_RUN_DECEL_MPS2;
use crate::powertrain::{Fuel, Powertrain, Segment, assign};
use crate::registry::{self, BoxedEmission, BoxedEv};
use crate::spec::EnergySpec;
use crate::state::DriveState;
use crate::traits::EmissionClass;

/// The provider's id.
pub const PROVIDER_ID: &str = "metrics/energy";

/// Two records of one actor further apart than this, s, are two trips: the gap is not
/// integrated.
pub const MAX_GAP_S: f64 = 5.0;
/// A stopped vehicle's next stop counts only after it has exceeded this speed, m/s.
pub const REARM_SPEED_MPS: f64 = 1.0;
/// A grade is only re-estimated over at least this distance, m.
pub const GRADE_MIN_RUN_M: f64 = 1.0;
/// The largest grade believed, rise over run; a larger height change per metre is a
/// recording artefact (a lane change across a kerb), not a road.
pub const GRADE_CLAMP: f64 = 0.3;
/// A vehicle enters the per-vehicle distribution after this distance, km.
pub const VEHICLE_MIN_KM: f64 = 0.5;
/// Petrol density, g/L.
pub const GASOLINE_G_PER_L: f64 = 742.0;
/// Diesel density, g/L.
pub const DIESEL_G_PER_L: f64 = 836.0;

/// The mobility classes a per-class metric is broken down by.
pub const CLASS_LABELS: [&str; 9] = [
    "passenger",
    "emergency",
    "delivery",
    "truck",
    "trailer",
    "bus",
    "coach",
    "motorcycle",
    "moped",
];

/// Sums over a set of intervals.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Totals {
    /// CO₂, g.
    pub co2_g: f64,
    /// Fuel, g.
    pub fuel_g: f64,
    /// Fuel, L.
    pub fuel_l: f64,
    /// NOₓ, g.
    pub nox_g: f64,
    /// PM, g.
    pub pm_g: f64,
    /// CO, g.
    pub co_g: f64,
    /// HC, g.
    pub hc_g: f64,
    /// Battery energy drawn, Wh.
    pub ev_out_wh: f64,
    /// Battery energy regenerated, Wh.
    pub ev_regen_wh: f64,
    /// Distance by combustion vehicles the model computed, km.
    pub km_ice: f64,
    /// Distance by electric vehicles, km.
    pub km_ev: f64,
    /// Distance by every vehicle, modelled or not, km.
    pub km_all: f64,
    /// Stops.
    pub stops: u64,
    /// Integrated intervals.
    pub intervals: u64,
}

impl Totals {
    fn add(&mut self, o: &Totals) {
        self.co2_g += o.co2_g;
        self.fuel_g += o.fuel_g;
        self.fuel_l += o.fuel_l;
        self.nox_g += o.nox_g;
        self.pm_g += o.pm_g;
        self.co_g += o.co_g;
        self.hc_g += o.hc_g;
        self.ev_out_wh += o.ev_out_wh;
        self.ev_regen_wh += o.ev_regen_wh;
        self.km_ice += o.km_ice;
        self.km_ev += o.km_ev;
        self.km_all += o.km_all;
        self.stops += o.stops;
        self.intervals += o.intervals;
    }

    /// Net battery energy, Wh.
    #[must_use]
    pub fn ev_net_wh(&self) -> f64 {
        self.ev_out_wh - self.ev_regen_wh
    }
}

/// How a tracked actor is computed.
#[derive(Debug, Clone)]
enum Kind {
    /// A combustion vehicle the emission model resolved.
    Ice {
        /// Which of the provider's emission models computes it.
        model: usize,
        /// The class that model resolved it to.
        class: EmissionClass,
        /// Its fuel.
        fuel: Fuel,
    },
    /// A battery-electric vehicle.
    Ev { battery: Battery },
    /// A vehicle the emission model has no class for.
    Unmodelled,
    /// Not a motor vehicle (pedestrian, bicycle, e-scooter) or an unknown class.
    Ignored,
}

#[derive(Debug, Clone, Copy)]
struct Last {
    t: SimTime,
    v: f64,
    z: Option<f64>,
    heading: Option<f64>,
    lane: Option<u32>,
}

#[derive(Debug, Clone)]
struct Track {
    class: String,
    powertrain: Option<Powertrain>,
    kind: Kind,
    last: Option<Last>,
    grade: f64,
    decel_run_s: f64,
    halted: bool,
    soc_min: f64,
    totals: Totals,
    /// This flush window's share of `totals`.
    window: Totals,
    /// This flush window's share, by the lane each interval started on.
    window_links: BTreeMap<u32, Totals>,
}

/// One vehicle's run so far.
#[derive(Debug, Clone, PartialEq)]
pub struct VehicleEnergy {
    /// The actor.
    pub actor: ActorId,
    /// Its mobility class.
    pub class: String,
    /// Its powertrain, `None` for a road user that has none.
    pub powertrain: Option<Powertrain>,
    /// The emission class the model computed it as, if a combustion vehicle it resolved.
    pub emission_class: Option<String>,
    /// False when the emission model has no class for it.
    pub modelled: bool,
    /// Its totals.
    pub totals: Totals,
    /// Its battery's state of charge now, for an EV.
    pub soc: Option<f64>,
}

/// The energy metric provider.
pub struct EnergyProvider {
    card: ModelCard,
    seed: u64,
    spec: EnergySpec,
    /// The emission models: the section's default first, then each per-segment one.
    emission: Vec<BoxedEmission>,
    /// Which of `emission` a segment uses, when not the default.
    by_segment: BTreeMap<Segment, usize>,
    ev: BoxedEv,
    tracks: BTreeMap<ActorId, Track>,
    /// Vehicles not seen for longer than [`MAX_GAP_S`], folded away at a flush in actor
    /// order so the live set stays the size of the traffic, not of the run.
    retired: Vec<VehicleEnergy>,
    /// The retired vehicles' totals by class, accumulated in retirement (actor) order.
    retired_class: BTreeMap<String, Totals>,
    /// The retired combustion vehicles' CO2 intensities, for the per-vehicle distribution.
    retired_co2_per_km: Distribution,
    /// The lowest state of charge a retired EV reached.
    retired_soc_min: Option<f64>,
    /// Retired EVs that were depleted.
    retired_depleted: u64,
    /// Retired vehicles the model had no class for.
    retired_unmodelled: u64,
    /// MOVES operating-mode seconds by (source type, lane, mode).
    op_modes: BTreeMap<(u16, u32, u8), f64>,
    rejected: u64,
}

impl core::fmt::Debug for EnergyProvider {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("EnergyProvider")
            .field("emission", &self.emission[0].card().id)
            .field("ev", &self.ev.card().id)
            .field("tracks", &self.tracks.len())
            .finish_non_exhaustive()
    }
}

impl EnergyProvider {
    /// The provider a scenario's `energy:` section describes, for a run with master seed
    /// `seed`.
    ///
    /// # Errors
    /// What [`EnergySpec::validate`] rejects.
    pub fn new(spec: EnergySpec, seed: u64) -> Result<Self, String> {
        spec.validate()?;
        let mut emission = vec![registry::emission_model(&spec.emission_model)?];
        let mut by_segment = BTreeMap::new();
        for (seg, name) in &spec.emission_model_by_segment {
            by_segment.insert(*seg, emission.len());
            emission.push(registry::emission_model(name)?);
        }
        let ev = registry::ev_model(&spec.ev_model, &spec.ev.params)?;
        let ids: Vec<String> = emission.iter().map(|m| m.card().id.clone()).collect();
        Ok(Self {
            card: card(&spec, &ids.join(", "), ev.card().id.as_str()),
            seed,
            spec,
            emission,
            by_segment,
            ev,
            tracks: BTreeMap::new(),
            retired: Vec::new(),
            retired_class: BTreeMap::new(),
            retired_co2_per_km: Distribution::new(),
            retired_soc_min: None,
            retired_depleted: 0,
            retired_unmodelled: 0,
            op_modes: BTreeMap::new(),
            rejected: 0,
        })
    }

    /// The section it was built from.
    #[must_use]
    pub fn spec(&self) -> &EnergySpec {
        &self.spec
    }

    /// Every vehicle seen so far, in actor order.
    #[must_use]
    pub fn vehicles(&self) -> Vec<VehicleEnergy> {
        let mut v: Vec<VehicleEnergy> = self.retired.clone();
        v.extend(
            self.tracks
                .iter()
                .filter(|(_, t)| !matches!(t.kind, Kind::Ignored))
                .map(|(a, t)| summary(*a, t)),
        );
        v.sort_by_key(|e| e.actor);
        v
    }

    /// The run's totals so far by class: retired vehicles, then live ones in actor order.
    #[must_use]
    pub fn run_by_class(&self) -> BTreeMap<String, Totals> {
        let mut m = self.retired_class.clone();
        for t in self.tracks.values() {
            if !matches!(t.kind, Kind::Ignored) {
                m.entry(t.class.clone()).or_default().add(&t.totals);
            }
        }
        m
    }

    /// The run's totals so far, over every class.
    #[must_use]
    pub fn run_totals(&self) -> Totals {
        let mut t = Totals::default();
        for c in self.run_by_class().values() {
            t.add(c);
        }
        t
    }

    /// Folds away every vehicle last seen before `at - MAX_GAP_S`, in actor order.
    fn retire(&mut self, at: SimTime) {
        let horizon = (MAX_GAP_S * 1e9) as u64;
        let gone: Vec<ActorId> = self
            .tracks
            .iter()
            .filter(|(_, t)| t.last.is_some_and(|l| l.t.saturating_add(horizon) < at))
            .map(|(a, _)| *a)
            .collect();
        for a in gone {
            let Some(t) = self.tracks.remove(&a) else {
                continue;
            };
            if matches!(t.kind, Kind::Ignored) {
                continue;
            }
            self.retired_class
                .entry(t.class.clone())
                .or_default()
                .add(&t.totals);
            match &t.kind {
                Kind::Ice { .. } if t.totals.km_ice >= VEHICLE_MIN_KM => {
                    self.retired_co2_per_km
                        .observe(t.totals.co2_g / t.totals.km_ice);
                }
                Kind::Ev { battery } => {
                    self.retired_soc_min =
                        Some(self.retired_soc_min.map_or(t.soc_min, |m| m.min(t.soc_min)));
                    self.retired_depleted += u64::from(battery.depleted());
                }
                Kind::Unmodelled => self.retired_unmodelled += 1,
                _ => {}
            }
            self.retired.push(summary(a, &t));
        }
    }

    fn new_track(&self, actor: ActorId, class: &str) -> Track {
        let segment = Segment::of_vehicle_class(class);
        let powertrain = segment.map(|s| assign(self.seed, actor, s, &self.spec.fleet));
        let kind = match powertrain {
            None => Kind::Ignored,
            Some(p) if p.is_electric() => Kind::Ev {
                battery: Battery::new(self.ev.capacity_wh(), self.spec.ev.initial_soc),
            },
            Some(p) => {
                let model = self.by_segment.get(&p.segment).copied().unwrap_or(0);
                match self.emission[model].resolve(&p) {
                    Some(class) => Kind::Ice {
                        model,
                        class,
                        fuel: p.fuel,
                    },
                    None => Kind::Unmodelled,
                }
            }
        };
        Track {
            class: class.to_string(),
            powertrain,
            kind,
            last: None,
            grade: 0.0,
            decel_run_s: 0.0,
            halted: true,
            soc_min: self.spec.ev.initial_soc,
            totals: Totals::default(),
            window: Totals::default(),
            window_links: BTreeMap::new(),
        }
    }

    fn on_kinematics(&mut self, k: &GtKinematicsView) {
        let class = k.class.as_deref().unwrap_or("unknown");
        if !self.tracks.contains_key(&k.actor) {
            let t = self.new_track(k.actor, class);
            self.tracks.insert(k.actor, t);
        }
        let stop_speed = self.spec.stop_speed_mps;
        let Some(track) = self.tracks.get_mut(&k.actor) else {
            return;
        };
        let now = Last {
            t: k.t,
            v: k.speed_mps.max(0.0),
            z: k.z_m,
            heading: k.heading_rad,
            lane: k.lane,
        };
        if matches!(track.kind, Kind::Ignored) {
            // Kept only so `retire` can drop it once it is gone.
            track.last = Some(now);
            return;
        }
        let Some(last) = track.last.replace(now) else {
            track.halted = now.v < REARM_SPEED_MPS;
            return;
        };
        if k.t <= last.t {
            return;
        }
        let dt = (k.t - last.t) as f64 / 1e9;
        if dt > MAX_GAP_S {
            track.decel_run_s = 0.0;
            track.halted = now.v < REARM_SPEED_MPS;
            return;
        }
        let mut s = DriveState::between(last.v, now.v, dt);
        if let (Some(z0), Some(z1)) = (last.z, now.z)
            && s.distance_m >= GRADE_MIN_RUN_M
        {
            track.grade = ((z1 - z0) / s.distance_m).clamp(-GRADE_CLAMP, GRADE_CLAMP);
        }
        s.grade = track.grade;
        if let (Some(h0), Some(h1)) = (last.heading, now.heading) {
            s.heading_change_rad = h1 - h0;
        }
        track.decel_run_s = if s.a_mps2 < -BRAKING_RUN_DECEL_MPS2 {
            track.decel_run_s + dt
        } else {
            0.0
        };
        s.decel_run_s = track.decel_run_s;

        let km = s.distance_m / 1000.0;
        let mut d = Totals {
            km_all: km,
            intervals: 1,
            ..Totals::default()
        };
        match &mut track.kind {
            Kind::Ice { model, class, fuel } => {
                let r = self.emission[*model].rates(class, &s).times(dt);
                d.co2_g = r.co2_g_s;
                d.fuel_g = r.fuel_g_s;
                d.fuel_l = r.fuel_g_s
                    / match fuel {
                        Fuel::Diesel => DIESEL_G_PER_L,
                        _ => GASOLINE_G_PER_L,
                    };
                d.nox_g = r.nox_g_s;
                d.pm_g = r.pm_g_s;
                d.co_g = r.co_g_s;
                d.hc_g = r.hc_g_s;
                d.km_ice = km;
            }
            Kind::Ev { battery } => {
                let (out0, regen0) = (battery.out_wh, battery.regen_wh);
                battery.apply(self.ev.battery_power_w(&s), dt);
                d.ev_out_wh = battery.out_wh - out0;
                d.ev_regen_wh = battery.regen_wh - regen0;
                d.km_ev = km;
                track.soc_min = track.soc_min.min(battery.soc());
            }
            Kind::Unmodelled | Kind::Ignored => {}
        }
        if now.v < stop_speed && !track.halted {
            d.stops = 1;
            track.halted = true;
        } else if now.v > REARM_SPEED_MPS {
            track.halted = false;
        }
        track.totals.add(&d);
        track.window.add(&d);
        if let Some(lane) = last.lane {
            track.window_links.entry(lane).or_default().add(&d);
        }
        // The MOVES operating-mode time, for a project-level MOVES run of this traffic.
        let source = match track.powertrain.map(|p| p.segment) {
            Some(Segment::PassengerCar) => Some((21u16, &crate::moves::PASSENGER_CAR)),
            Some(Segment::LightCommercial) => Some((31u16, &crate::moves::PASSENGER_TRUCK)),
            _ => None,
        };
        if let (Some((source_type, rl)), Some(lane)) = (source, last.lane) {
            let mode = crate::moves::op_mode(rl, &s);
            *self.op_modes.entry((source_type, lane, mode)).or_default() += dt;
        }
    }

    /// Seconds spent in each MOVES operating mode, by `(source type, lane, mode)`: 21 is a
    /// passenger car, 31 a passenger truck (light commercial vehicles). Divided by its
    /// `(source type, lane)` total, this is the `opModeDistribution` table a MOVES
    /// project-level run takes as its activity input, so MOVES's own current rates can
    /// be applied to the traffic this engine simulated.
    #[must_use]
    pub fn op_mode_seconds(&self) -> &BTreeMap<(u16, u32, u8), f64> {
        &self.op_modes
    }

    fn definitions(&self) -> Vec<MetricDef> {
        defs()
    }

    fn def_named(name: &str) -> MetricDef {
        defs()
            .into_iter()
            .find(|d| d.name == name)
            .unwrap_or_else(|| panic!("energy metric {name} is not defined"))
    }
}

fn summary(actor: ActorId, t: &Track) -> VehicleEnergy {
    VehicleEnergy {
        actor,
        class: t.class.clone(),
        powertrain: t.powertrain,
        emission_class: match &t.kind {
            Kind::Ice { class, .. } => Some(class.name.clone()),
            _ => None,
        },
        modelled: !matches!(t.kind, Kind::Unmodelled),
        totals: t.totals,
        soc: match &t.kind {
            Kind::Ev { battery } => Some(battery.soc()),
            _ => None,
        },
    }
}

fn class_dims(class: &str) -> Dims {
    let mut d = Dims::new();
    d.insert(Dim::Class, DimValue::label(class));
    d
}

fn lane_dims(lane: u32) -> Dims {
    let mut d = Dims::new();
    d.insert(Dim::Region, DimValue::index(u64::from(lane)));
    d
}

fn sum_value(x: f64, n: u64) -> SampleValue {
    if n == 0 {
        SampleValue::Scalar(Estimate::Insufficient { n: 0, required: 1 })
    } else {
        SampleValue::Scalar(Estimate::Value { point: x, n })
    }
}

/// Every metric definition this provider publishes.
#[must_use]
pub fn defs() -> Vec<MetricDef> {
    let gt = Visibility::Gt;
    let q = Quantum::new(1e-3);
    let note_model = "what the selected model ignores (its card's `ignores`), e.g. cold \
                      starts, evaporative and non-exhaust emissions";
    let class_sum = |name: &str, unit: &str, what: &str| {
        MetricDef::new(name, unit, Agg::Sum, gt, q, what)
            .with_dims([Dim::Class])
            .with_breakdown(Dim::Class, CLASS_LABELS)
            .with_min_samples(1)
            .with_range(0.0, f64::INFINITY)
            .with_source(cite::krajzewicz_2015())
            .not_accounting_for(note_model)
            .not_accounting_for("vehicles the emission model has no class for (energy_unmodelled)")
    };
    let link_sum = |name: &str, unit: &str, what: &str| {
        MetricDef::new(name, unit, Agg::Sum, gt, q, what)
            .with_dims([Dim::Region])
            .with_min_samples(1)
            .with_range(0.0, f64::INFINITY)
            .not_accounting_for(note_model)
            .not_accounting_for(
                "the junction-internal share of a movement, charged to the lane the interval \
                 started on",
            )
    };
    let class_ratio = |name: &str, unit: &str, num: &str, den: &str, what: &str| {
        MetricDef::new(name, unit, Agg::ratio(num, den), gt, q, what)
            .with_dims([Dim::Class])
            .with_breakdown(Dim::Class, CLASS_LABELS)
            .with_min_samples(1)
            .with_range(0.0, f64::INFINITY)
            .not_accounting_for(note_model)
    };
    let mut v = vec![
        class_sum(
            "co2_mass",
            "g",
            "Tailpipe CO2 emitted in the window: the emission model's CO2 rate integrated over \
             every interval of every combustion vehicle it resolved.",
        ),
        class_sum("fuel_mass", "g", "Fuel burned in the window, by mass."),
        class_sum("nox_mass", "g", "NOx emitted in the window (as NO2 mass)."),
        class_sum(
            "pm_mass",
            "g",
            "Exhaust particulate matter emitted in the window.",
        ),
        class_sum("co_mass", "g", "CO emitted in the window."),
        class_sum("hc_mass", "g", "Hydrocarbons emitted in the window."),
        class_sum(
            "ev_battery_energy",
            "Wh",
            "Net traction-battery energy of electric vehicles in the window: energy drawn \
             minus energy regenerated.",
        )
        .with_range(f64::NEG_INFINITY, f64::INFINITY)
        .with_source(cite::kurczveil_2014()),
        class_sum(
            "ev_regen_energy",
            "Wh",
            "Energy electric vehicles returned to their batteries by regenerative braking in \
             the window.",
        )
        .with_source(cite::kurczveil_2014()),
        class_sum(
            "vehicle_distance",
            "km",
            "Distance driven in the window by every motor vehicle, modelled or not.",
        ),
        MetricDef::new(
            "vehicle_stops",
            "count",
            Agg::Count,
            gt,
            Quantum::COUNT,
            "Stops in the window: a vehicle that had exceeded 1 m/s falling below \
             energy.stop_speed_mps (SUMO's 0.1 m/s halting threshold by default).",
        )
        .with_dims([Dim::Class])
        .with_breakdown(Dim::Class, CLASS_LABELS)
        .with_range(0.0, f64::INFINITY)
        .with_source(cite::src(
            SourceKind::Code,
            "Eclipse SUMO documentation, Simulation/Output: a vehicle is halting below 0.1 m/s",
            "",
        ))
        .not_accounting_for("slowdowns that do not reach the stop speed"),
        link_sum(
            "link_co2_mass",
            "g",
            "Tailpipe CO2 emitted on each lane in the window.",
        ),
        link_sum(
            "link_fuel_mass",
            "g",
            "Fuel burned on each lane in the window.",
        ),
        link_sum(
            "link_nox_mass",
            "g",
            "NOx emitted on each lane in the window.",
        ),
        link_sum(
            "link_pm_mass",
            "g",
            "Exhaust PM emitted on each lane in the window.",
        ),
        link_sum(
            "link_ev_energy",
            "Wh",
            "Net electric-vehicle battery energy used on each lane in the window.",
        )
        .with_range(f64::NEG_INFINITY, f64::INFINITY),
        link_sum(
            "link_distance",
            "km",
            "Vehicle distance driven on each lane in the window.",
        ),
        class_ratio(
            "co2_per_km",
            "g/km",
            "tailpipe CO2 (g), run so far",
            "distance of modelled vehicles (km), electric ones included",
            "Fleet tailpipe CO2 intensity over the run so far. Electric vehicles count with \
             zero tailpipe CO2, so the fleet's EV share lowers it.",
        )
        .not_accounting_for("upstream (well-to-tank, electricity generation) emissions"),
        class_ratio(
            "fuel_per_100km",
            "L/100km",
            "fuel volume (L) at 742 g/L petrol, 836 g/L diesel",
            "distance of modelled combustion vehicles (100 km)",
            "Fuel consumption of the combustion fleet over the run so far.",
        )
        .with_source(cite::fuel_density()),
        class_ratio(
            "nox_per_km",
            "g/km",
            "NOx (g), run so far",
            "distance of modelled combustion vehicles (km)",
            "NOx intensity of the combustion fleet over the run so far.",
        ),
        class_ratio(
            "ev_energy_per_100km",
            "kWh/100km",
            "net battery energy (kWh), run so far",
            "distance of electric vehicles (100 km)",
            "Battery-side energy consumption of the electric fleet over the run so far, \
             regeneration netted out.",
        )
        .with_range(f64::NEG_INFINITY, f64::INFINITY)
        .not_accounting_for("charger losses between the grid and the battery"),
        class_ratio(
            "stops_per_km",
            "1/km",
            "stops, run so far",
            "distance of every motor vehicle (km)",
            "Stops per vehicle-kilometre over the run so far: the quantity GLOSA and signal \
             coordination act on.",
        ),
        MetricDef::new(
            "vehicle_co2_per_km",
            "g/km",
            Agg::Distribution,
            gt,
            q,
            "Per-vehicle tailpipe CO2 intensity over the run so far, across combustion \
             vehicles that have driven at least 0.5 km.",
        )
        .with_min_samples(1)
        .with_range(0.0, f64::INFINITY)
        .not_accounting_for("vehicles under 0.5 km, whose idling dominates their ratio")
        .not_accounting_for(note_model),
        MetricDef::new(
            "ev_soc_min",
            "ratio",
            Agg::Max,
            gt,
            Quantum::RATIO,
            "The lowest state of charge any electric vehicle's battery has reached in the run.",
        )
        .with_min_samples(1)
        .with_range(0.0, 1.0)
        .not_accounting_for("charging, which this engine does not model yet"),
        MetricDef::new(
            "ev_depleted",
            "count",
            Agg::Count,
            gt,
            Quantum::COUNT,
            "Electric vehicles that needed energy their battery no longer had.",
        )
        .with_range(0.0, f64::INFINITY)
        .not_accounting_for(
            "the driving response to an empty battery: the vehicle keeps driving as the \
             mobility model moves it",
        ),
        MetricDef::new(
            "energy_unmodelled",
            "count",
            Agg::Count,
            gt,
            Quantum::COUNT,
            "Motor vehicles the selected emission model has no class for, which are \
             therefore excluded from every emission total rather than given a guessed one.",
        )
        .with_range(0.0, f64::INFINITY)
        .not_accounting_for("their emissions, by construction"),
    ];
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}

impl Model for EnergyProvider {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MetricProvider for EnergyProvider {
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
            let view = ev.with(|v: Option<&GtKinematicsView>| v.cloned());
            match view {
                Some(v) => self.on_kinematics(&v),
                None => self.rejected += 1,
            }
        }
    }

    fn flush(&mut self, at: SimTime) -> Vec<MetricSample> {
        let mut out = Vec::new();
        // Gather the window from every live vehicle, in actor order, so a sum never
        // depends on how the actors' records happened to interleave.
        let mut window: BTreeMap<String, Totals> = BTreeMap::new();
        let mut links: BTreeMap<u32, Totals> = BTreeMap::new();
        for t in self.tracks.values_mut() {
            if t.window.intervals > 0 {
                window.entry(t.class.clone()).or_default().add(&t.window);
            }
            for (lane, l) in core::mem::take(&mut t.window_links) {
                links.entry(lane).or_default().add(&l);
            }
            t.window = Totals::default();
        }
        // --- per window, by class and in total ----------------------------------------
        let mut total = Totals::default();
        for t in window.values() {
            total.add(t);
        }
        let per_window: [(&str, fn(&Totals) -> f64); 9] = [
            ("co2_mass", |t| t.co2_g),
            ("fuel_mass", |t| t.fuel_g),
            ("nox_mass", |t| t.nox_g),
            ("pm_mass", |t| t.pm_g),
            ("co_mass", |t| t.co_g),
            ("hc_mass", |t| t.hc_g),
            ("ev_battery_energy", Totals::ev_net_wh),
            ("ev_regen_energy", |t| t.ev_regen_wh),
            ("vehicle_distance", |t| t.km_all),
        ];
        for (name, get) in per_window {
            let def = Self::def_named(name);
            out.push(MetricSample::new(
                &def,
                at,
                Dims::new(),
                sum_value(get(&total), total.intervals),
            ));
            for (class, t) in &window {
                out.push(MetricSample::new(
                    &def,
                    at,
                    class_dims(class),
                    sum_value(get(t), t.intervals),
                ));
            }
        }
        let stops = Self::def_named("vehicle_stops");
        out.push(MetricSample::new(
            &stops,
            at,
            Dims::new(),
            SampleValue::count(total.stops),
        ));
        for (class, t) in &window {
            out.push(MetricSample::new(
                &stops,
                at,
                class_dims(class),
                SampleValue::count(t.stops),
            ));
        }
        // --- per window, by lane ---------------------------------------------------------
        let per_link: [(&str, fn(&Totals) -> f64); 6] = [
            ("link_co2_mass", |t| t.co2_g),
            ("link_fuel_mass", |t| t.fuel_g),
            ("link_nox_mass", |t| t.nox_g),
            ("link_pm_mass", |t| t.pm_g),
            ("link_ev_energy", Totals::ev_net_wh),
            ("link_distance", |t| t.km_all),
        ];
        for (name, get) in per_link {
            let def = Self::def_named(name);
            for (lane, t) in &links {
                out.push(MetricSample::new(
                    &def,
                    at,
                    lane_dims(*lane),
                    sum_value(get(t), t.intervals),
                ));
            }
        }
        // --- run so far, by class and in total -------------------------------------------
        let run_class = self.run_by_class();
        let mut run = Totals::default();
        for t in run_class.values() {
            run.add(t);
        }
        let ratios: [(&str, fn(&Totals) -> (f64, f64)); 5] = [
            ("co2_per_km", |t| (t.co2_g, t.km_ice + t.km_ev)),
            ("fuel_per_100km", |t| (t.fuel_l, t.km_ice / 100.0)),
            ("nox_per_km", |t| (t.nox_g, t.km_ice)),
            ("ev_energy_per_100km", |t| {
                (t.ev_net_wh() / 1000.0, t.km_ev / 100.0)
            }),
            ("stops_per_km", |t| (t.stops as f64, t.km_all)),
        ];
        for (name, get) in ratios {
            let def = Self::def_named(name);
            let (num, den) = get(&run);
            out.push(MetricSample::new(
                &def,
                at,
                Dims::new(),
                SampleValue::Ratio(ratio_of_sums(num, den, run.intervals, 1)),
            ));
            for (class, t) in &run_class {
                let (num, den) = get(t);
                out.push(MetricSample::new(
                    &def,
                    at,
                    class_dims(class),
                    SampleValue::Ratio(ratio_of_sums(num, den, t.intervals, 1)),
                ));
            }
        }
        // --- run so far, across vehicles -------------------------------------------------
        let mut per_vehicle = Distribution::new();
        per_vehicle.merge(&self.retired_co2_per_km);
        let mut soc_min: Option<f64> = self.retired_soc_min;
        let mut depleted = self.retired_depleted;
        let mut unmodelled = self.retired_unmodelled;
        for t in self.tracks.values() {
            match &t.kind {
                Kind::Ice { .. } if t.totals.km_ice >= VEHICLE_MIN_KM => {
                    per_vehicle.observe(t.totals.co2_g / t.totals.km_ice);
                }
                Kind::Ev { battery } => {
                    soc_min = Some(soc_min.map_or(t.soc_min, |m: f64| m.min(t.soc_min)));
                    if battery.depleted() {
                        depleted += 1;
                    }
                }
                Kind::Unmodelled => unmodelled += 1,
                _ => {}
            }
        }
        let def = Self::def_named("vehicle_co2_per_km");
        out.push(MetricSample::new(
            &def,
            at,
            Dims::new(),
            SampleValue::Distribution(per_vehicle.summary(def.min_samples)),
        ));
        let def = Self::def_named("ev_soc_min");
        out.push(MetricSample::new(
            &def,
            at,
            Dims::new(),
            SampleValue::Scalar(match soc_min {
                Some(m) => Estimate::Value { point: m, n: 1 },
                None => Estimate::Insufficient { n: 0, required: 1 },
            }),
        ));
        out.push(MetricSample::new(
            &Self::def_named("ev_depleted"),
            at,
            Dims::new(),
            SampleValue::count(depleted),
        ));
        out.push(MetricSample::new(
            &Self::def_named("energy_unmodelled"),
            at,
            Dims::new(),
            SampleValue::count(unmodelled),
        ));
        self.retire(at);
        out
    }

    fn rejected(&self) -> u64 {
        self.rejected
    }
}

/// The provider's card, naming the two models it was built with.
#[must_use]
pub fn card(spec: &EnergySpec, emission_id: &str, ev_id: &str) -> ModelCard {
    let mut card = ModelCard::new(
        PROVIDER_ID,
        Family::Metric,
        "1.0.0",
        "Integrates the selected emission and electric-vehicle energy models over every \
         motor vehicle's ground-truth trajectory (gt.kinematics) and reports emissions, fuel, \
         battery energy, state of charge and stops per window, per class, per lane and per \
         vehicle.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![
        cite::eq(
            "interval",
            "v = (v0 + v1)/2, a = (v1 - v0)/dt, s = v dt, grade = dz/s (s >= 1 m, |grade| <= 0.3)",
            "between consecutive records of one actor at most 5 s apart",
        ),
        cite::eq(
            "integration",
            "mass += rate(class, state) dt;  battery -= P_bat(state) dt",
            "",
        ),
        cite::eq(
            "stop",
            "a stop is counted when v < stop_speed after v > 1 m/s",
            "",
        ),
    ];
    card.parameters = vec![
        cite::param(
            "stop_speed_mps",
            "m/s",
            spec.stop_speed_mps,
            cite::src(
                SourceKind::Code,
                "Eclipse SUMO documentation, Simulation/Output: halting below 0.1 m/s",
                "",
            ),
        ),
        cite::todo(
            "rearm_speed_mps",
            "m/s",
            REARM_SPEED_MPS,
            "hysteresis so creeping in a queue is one stop, not several",
            "compare stop counts against a published stop-count definition (HCM queue \
             accumulation) on a signalised-corridor run",
        ),
        cite::todo(
            "max_gap_s",
            "s",
            MAX_GAP_S,
            "a longer gap between two records of one actor is two trips",
            "set to a multiple of the recording's gt.kinematics period; the engine writes \
             one per mobility step (0.1 s by default)",
        ),
        cite::todo(
            "grade_clamp",
            "-",
            GRADE_CLAMP,
            "grades steeper than this are recording artefacts",
            "the steepest public streets are about 0.35; check against the world's DEM \
             slope distribution",
        ),
        cite::todo(
            "ev.initial_soc",
            "-",
            spec.ev.initial_soc,
            "state of charge every EV starts with",
            "set from the study's charging assumption",
        ),
        cite::param(
            "gasoline_g_per_l",
            "g/L",
            GASOLINE_G_PER_L,
            cite::fuel_density(),
        ),
        cite::param(
            "diesel_g_per_l",
            "g/L",
            DIESEL_G_PER_L,
            cite::fuel_density(),
        ),
    ];
    card.assumptions = vec![
        format!("emission model {emission_id}; EV model {ev_id}"),
        "every vehicle's powertrain is drawn once from the fleet section, keyed by actor id"
            .to_string(),
    ];
    card.limitations = vec![
        "post-hoc: the energy state does not feed back into driving (an empty EV keeps \
         moving), as in SUMO's battery device without its stop-on-empty option"
            .to_string(),
    ];
    card.ignores = vec![
        "charging".to_string(),
        "upstream emissions of fuel production and electricity generation".to_string(),
    ];
    card.sources = vec![
        cite::krajzewicz_2015(),
        cite::kurczveil_2014(),
        cite::epa_moves_opmodes(),
    ];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: Vec::new(),
        tests: vec![
            "provider::tests::a_recorded_stop_and_go_is_integrated_and_reported".to_string(),
            "tests/glosa.rs (v2xw-engine) compares GLOSA on and off".to_string(),
        ],
    };
    card.determinism.uses_rng = true;
    card.determinism.rng_domains = vec![crate::powertrain::FLEET_DRAW_ID.to_string()];
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::ctx::OwnedRecord;

    fn rec(t_s: f64, actor: u32, speed: f64, x: f64, class: &str, lane: u32) -> OwnedRecord {
        let j = serde_json::json!({
            "t": (t_s * 1e9) as u64, "actor": actor, "x_m": x, "y_m": 0.0, "z_m": 0.0,
            "speed_mps": speed, "heading_rad": 0.0, "lane": lane, "class": class,
        });
        OwnedRecord {
            channel: "gt.kinematics",
            visibility: Visibility::Gt,
            json: serde_json::to_vec(&j).unwrap(),
        }
    }

    fn drive(p: &mut EnergyProvider, actor: u32, class: &str, profile: &[f64]) {
        let mut x = 0.0;
        for (i, w) in profile.windows(2).enumerate() {
            p.on_event(&rec(i as f64, actor, w[0], x, class, 7));
            x += 0.5 * (w[0] + w[1]);
        }
        let n = profile.len() - 1;
        p.on_event(&rec(n as f64, actor, profile[n], x, class, 7));
    }

    fn sample<'a>(s: &'a [MetricSample], key: &str) -> &'a MetricSample {
        s.iter().find(|x| x.key() == key).unwrap_or_else(|| {
            panic!(
                "no {key}: {:?}",
                s.iter().map(MetricSample::key).collect::<Vec<_>>()
            )
        })
    }

    #[test]
    fn every_definition_validates_and_the_card_is_accepted() {
        let p = EnergyProvider::new(EnergySpec::default(), 1).unwrap();
        p.validate_defs().unwrap();
        p.card().validate().unwrap();
        let mut names: Vec<String> = defs().into_iter().map(|d| d.name).collect();
        let n = names.len();
        names.dedup();
        assert_eq!(n, names.len());
    }

    #[test]
    fn a_recorded_stop_and_go_is_integrated_and_reported() {
        let mut p = EnergyProvider::new(EnergySpec::default(), 1).unwrap();
        // 0 -> 14 m/s in 7 s, cruise 20 s, stop in 7 s, idle 5 s, go again.
        let mut prof: Vec<f64> = (0..=7).map(|i| 2.0 * f64::from(i)).collect();
        prof.extend(std::iter::repeat_n(14.0, 20));
        prof.extend((0..=7).rev().map(|i| 2.0 * f64::from(i)));
        prof.extend(std::iter::repeat_n(0.0, 5));
        prof.extend((1..=7).map(|i| 2.0 * f64::from(i)));
        drive(&mut p, 1, "passenger", &prof);
        drive(&mut p, 2, "pedestrian", &[1.0, 1.0, 1.0]);
        let out = p.flush(60_000_000_000);
        let co2 = sample(&out, "co2_mass").value.point().unwrap();
        let co2_car = sample(&out, "co2_mass|class=passenger")
            .value
            .point()
            .unwrap();
        assert!(
            co2 > 0.0 && (co2 - co2_car).abs() < 1e-9,
            "only the car emits"
        );
        let km = sample(&out, "vehicle_distance").value.point().unwrap();
        let want: f64 = prof.windows(2).map(|w| 0.5 * (w[0] + w[1])).sum::<f64>() / 1000.0;
        assert!((km - want).abs() < 1e-6, "{km} km, want {want}");
        let g_km = sample(&out, "co2_per_km").value.point().unwrap();
        assert!((100.0..400.0).contains(&g_km), "{g_km} g/km");
        assert_eq!(sample(&out, "vehicle_stops").value.point(), Some(1.0));
        assert!(
            sample(&out, "link_co2_mass|region=7")
                .value
                .point()
                .unwrap()
                > 0.0
        );
        assert_eq!(sample(&out, "energy_unmodelled").value.point(), Some(0.0));
        assert!(sample(&out, "ev_soc_min").value.is_insufficient(), "no EVs");
        // The window drains; the run-so-far ratio does not.
        let again = p.flush(61_000_000_000);
        assert!(sample(&again, "co2_mass").value.is_insufficient());
        assert_eq!(sample(&again, "co2_per_km").value.point(), Some(g_km));
        assert_eq!(p.vehicles().len(), 1, "the pedestrian is not a vehicle");
        // The operating-mode time adds up to the car's driving time, idle included.
        let secs: f64 = p.op_mode_seconds().values().sum();
        assert!((secs - (prof.len() - 1) as f64).abs() < 1e-9, "{secs}");
        assert!(
            p.op_mode_seconds().contains_key(&(21, 7, 1)),
            "idle was seen"
        );
    }

    #[test]
    fn electric_vehicles_draw_on_their_battery_and_regenerate() {
        let mut spec = EnergySpec::default();
        spec.fleet.insert(
            Segment::PassengerCar,
            crate::powertrain::SegmentMix {
                ev_share: 1.0,
                ..Default::default()
            },
        );
        let mut p = EnergyProvider::new(spec, 1).unwrap();
        let mut prof: Vec<f64> = (0..=7).map(|i| 2.0 * f64::from(i)).collect();
        prof.extend((0..=6).rev().map(|i| 2.0 * f64::from(i)));
        drive(&mut p, 1, "passenger", &prof);
        let out = p.flush(30_000_000_000);
        assert_eq!(sample(&out, "co2_mass").value.point(), Some(0.0));
        assert!(sample(&out, "ev_regen_energy").value.point().unwrap() > 0.0);
        let soc = sample(&out, "ev_soc_min").value.point().unwrap();
        assert!(soc < 0.8 && soc > 0.79, "{soc}");
        let v = &p.vehicles()[0];
        assert!(
            v.soc.unwrap() > soc - 1e-12,
            "regeneration lifted it from its minimum"
        );
    }

    #[test]
    fn a_model_without_the_class_counts_the_vehicle_as_unmodelled() {
        let spec = EnergySpec {
            emission_model: "phemlight".to_string(),
            ..EnergySpec::default()
        };
        let mut p = EnergyProvider::new(spec, 1).unwrap();
        drive(&mut p, 1, "bus", &[5.0, 6.0, 7.0]);
        drive(&mut p, 2, "passenger", &[5.0, 6.0, 7.0]);
        let out = p.flush(10_000_000_000);
        assert_eq!(sample(&out, "energy_unmodelled").value.point(), Some(1.0));
        assert!(sample(&out, "co2_mass|class=bus").value.point() == Some(0.0));
        // ... and a per-segment model fills the gap: PHEMlight for cars, HBEFA 3 for the bus.
        let spec = EnergySpec {
            emission_model: "hbefa3".to_string(),
            emission_model_by_segment: [(Segment::PassengerCar, "phemlight".to_string())].into(),
            ..EnergySpec::default()
        };
        let mut p = EnergyProvider::new(spec, 1).unwrap();
        drive(&mut p, 1, "bus", &[5.0, 6.0, 7.0]);
        drive(&mut p, 2, "passenger", &[5.0, 6.0, 7.0]);
        let out = p.flush(10_000_000_000);
        assert_eq!(sample(&out, "energy_unmodelled").value.point(), Some(0.0));
        let classes: Vec<Option<String>> =
            p.vehicles().into_iter().map(|v| v.emission_class).collect();
        assert_eq!(
            classes,
            [Some("Bus".to_string()), Some("PC_G_EU4".to_string())]
        );
        assert!(p.card().assumptions[0].contains("phemlight"));
    }

    #[test]
    fn the_result_does_not_depend_on_how_actors_interleave() {
        let run = |interleave: bool| {
            let mut p = EnergyProvider::new(EnergySpec::default(), 9).unwrap();
            let a: Vec<f64> = (0..20).map(|i| f64::from(i % 7)).collect();
            let b: Vec<f64> = (0..20).map(|i| f64::from((i * 3) % 11)).collect();
            if interleave {
                let mut xa = 0.0;
                let mut xb = 0.0;
                for i in 0..20 {
                    p.on_event(&rec(i as f64, 1, a[i], xa, "passenger", 1));
                    p.on_event(&rec(i as f64, 2, b[i], xb, "truck", 2));
                    if i + 1 < 20 {
                        xa += 0.5 * (a[i] + a[i + 1]);
                        xb += 0.5 * (b[i] + b[i + 1]);
                    }
                }
            } else {
                drive(&mut p, 1, "passenger", &a);
                drive(&mut p, 2, "truck", &b);
            }
            p.vehicles()
        };
        assert_eq!(run(true), run(false));
    }
}
