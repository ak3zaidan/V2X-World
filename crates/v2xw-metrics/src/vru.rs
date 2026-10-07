//! Pedestrian safety and delay: vehicle-pedestrian conflicts and near misses, the time to
//! collision between them, how long pedestrians stand waiting, and how much of their time
//! is spent crossing away from any lane (mid-block).
//!
//! | Metric | Formula | Unit | What it does **not** account for |
//! |---|---|---|---|
//! | `ped_conflicts` | vehicle-pedestrian approaches whose time to collision falls below 3 s, each counted once | count | conflicts with cyclists; an approach that dips below the threshold, recovers and dips again counts twice |
//! | `ped_near_misses` | of those, the ones that fall below 1.5 s | count | the same |
//! | `ped_ttc` | the time to collision of every such pair-step below 10 s | s | the pedestrian's own evasive manoeuvre, which a straight-line extrapolation cannot know |
//! | `ped_wait` | the length of each standing episode (speed below 0.2 m/s) that ended in the window | s | why the pedestrian stood: a kerb, a group, a crowd |
//! | `ped_midblock_share` | pedestrian samples off every lane (crossing mid-block) over all pedestrian samples | 1 | a crossing at an unmarked junction leg mapped without a crossing way, which is on a lane |
//!
//! # The time to collision
//!
//! For a motor vehicle moving faster than 0.5 m/s and a pedestrian walking *across* its
//! heading (at least 0.3 m/s perpendicular to it), with the pedestrian inside the vehicle's
//! swept width (half the vehicle's, half a pedestrian's and a quarter metre) and within 60 m
//! ahead of its front bumper: `ttc = (distance ahead − half a body) / (v_vehicle −
//! v_pedestrian along the heading)`. This is the constant-velocity extrapolation of the
//! surrogate safety literature (FHWA-HRT-08-051), applied to the two bodies' paths. The
//! thresholds are the traffic auditor's: 1.5 s, the one verified surrogate threshold of
//! 04-models.md §11, for a near miss; 3 s, the conventional upper bound of an encounter
//! worth recording, for a conflict (a choice, stated). The record's reference point is at
//! the vehicle's rear bumper, so its front is one body length (its class's) ahead.
//!
//! Requiring the pedestrian to walk across the vehicle's heading keeps out a pedestrian
//! standing at a corner that a turning vehicle's heading happens to sweep over, and one
//! walking along a sidewalk beside a lane: neither is on a collision course.

use std::collections::BTreeMap;

use serde_json::json;
use v2xw_core::card::ModelCard;
use v2xw_core::ctx::{ChannelName, EventRecord, Visibility};
use v2xw_core::ids::ActorId;
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_core::time::{Duration, SimTime};

use crate::cards;
use crate::channels::{ChannelView, GtKinematicsView};
use crate::def::{Agg, Dim, Dims, MetricDef, MetricSample, SampleValue};
use crate::provider::{Decoded, MetricProvider};
use crate::quant::Quantum;
use crate::stats::{Distribution, ratio_of_sums};

/// Below this time to collision a vehicle-pedestrian approach is a conflict, seconds.
pub const PED_CONFLICT_TTC_S: f64 = 3.0;

/// Below this it is a near miss, seconds (04-models.md §11, FHWA-HRT-08-051).
pub const PED_NEAR_MISS_TTC_S: f64 = 1.5;

/// The longest time to collision sampled into `ped_ttc`, seconds.
const TTC_SAMPLE_MAX_S: f64 = 10.0;

/// Below this speed a pedestrian is standing, m/s.
const STANDING_MPS: f64 = 0.2;

/// The class names the kinematics channel carries for the two road users this provider
/// pairs.
const PEDESTRIAN: &str = "pedestrian";
const BICYCLE: &str = "bicycle";

/// The pedestrian safety and delay provider.
pub struct VruSafetyProvider {
    card: ModelCard,
    min_samples: u64,
    frame_t: Option<SimTime>,
    frame: BTreeMap<ActorId, GtKinematicsView>,
    /// The vehicle-pedestrian pairs inside the conflict threshold at the last frame, and
    /// whether each had been a near miss.
    open: BTreeMap<(ActorId, ActorId), bool>,
    /// When each pedestrian started standing.
    standing_since: BTreeMap<ActorId, SimTime>,
    /// The last time each pedestrian was seen, so one who leaves the run ends its episode.
    last_seen: BTreeMap<ActorId, SimTime>,
    conflicts: u64,
    near_misses: u64,
    ttc: Distribution,
    wait: Distribution,
    ped_samples: u64,
    offlane_samples: u64,
    rejected: u64,
}

impl Default for VruSafetyProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl VruSafetyProvider {
    /// A provider with the default insufficiency threshold.
    #[must_use]
    pub fn new() -> Self {
        Self {
            card: Self::build_card(),
            min_samples: crate::stats::DEFAULT_MIN_SAMPLES,
            frame_t: None,
            frame: BTreeMap::new(),
            open: BTreeMap::new(),
            standing_since: BTreeMap::new(),
            last_seen: BTreeMap::new(),
            conflicts: 0,
            near_misses: 0,
            ttc: Distribution::new(),
            wait: Distribution::new(),
            ped_samples: 0,
            offlane_samples: 0,
            rejected: 0,
        }
    }

    fn build_card() -> ModelCard {
        let mut card = cards::provider_card(
            "metric/safety/pedestrians",
            "1.0.0",
            "Vehicle-pedestrian conflicts and near misses by time to collision, the \
             distribution of that time, pedestrians' standing waits, and the share of their \
             time spent crossing mid-block.",
        );
        card.equations = vec![v2xw_core::card::Equation::new(
            "ttc",
            "ttc = (d_ahead − l_ped/2) / (v_vehicle − v_ped·ê), for a pedestrian inside the \
             vehicle's swept width, walking across its heading",
        )];
        card.parameters = cards::statistics_params();
        card.parameters.push(cards::param(
            "near_miss_ttc_s",
            "s",
            json!(PED_NEAR_MISS_TTC_S),
            json!(0.1),
            json!(10.0),
            cards::paper(
                "FHWA-HRT-08-051 (SSAM): TTC threshold 1.5 s, the one verified surrogate \
                 threshold in 04-models.md §11",
            ),
        ));
        card.parameters.push(cards::param(
            "conflict_ttc_s",
            "s",
            json!(PED_CONFLICT_TTC_S),
            json!(0.1),
            json!(10.0),
            cards::design(
                "a choice: the conventional upper bound of a vehicle-pedestrian encounter \
                 worth recording in traffic-conflict studies",
            ),
        ));
        card.sources = vec![
            cards::paper("FHWA-HRT-08-051, Surrogate Safety Assessment Model (SSAM)"),
            cards::design("04-models.md §11 (surrogate safety measures and their thresholds)"),
        ];
        card.limitations = vec![
            "Straight-line extrapolation: a turning vehicle's time to collision is computed \
             along its present heading."
                .to_string(),
            "Pedestrian-cyclist and cyclist-vehicle conflicts are not counted.".to_string(),
        ];
        card.validation.tests = vec![
            "vru::tests::a_vehicle_closing_on_a_crossing_pedestrian_is_a_near_miss_once"
                .to_string(),
            "vru::tests::a_pedestrian_walking_beside_the_lane_is_no_conflict".to_string(),
            "vru::tests::a_standing_episode_is_a_wait".to_string(),
        ];
        card
    }

    fn definitions(&self) -> Vec<MetricDef> {
        let ssam = cards::paper("FHWA-HRT-08-051 (SSAM); 04-models.md §11");
        vec![
            MetricDef::new(
                "ped_conflicts",
                "count",
                Agg::Count,
                Visibility::Gt,
                Quantum::COUNT,
                "Vehicle-pedestrian approaches whose time to collision fell below 3 s, each \
                 approach counted once.",
            )
            .with_dims([Dim::T])
            .with_source(ssam.clone())
            .with_min_samples(1)
            .not_accounting_for("cyclists")
            .not_accounting_for("the pedestrian's own evasive action"),
            MetricDef::new(
                "ped_near_misses",
                "count",
                Agg::Count,
                Visibility::Gt,
                Quantum::COUNT,
                "Of the vehicle-pedestrian conflicts, those whose time to collision fell \
                 below 1.5 s.",
            )
            .with_dims([Dim::T])
            .with_source(ssam.clone())
            .with_min_samples(1)
            .not_accounting_for("cyclists"),
            MetricDef::new(
                "ped_ttc",
                "s",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::TIME_S,
                "The time to collision of every vehicle-pedestrian pair-step below 10 s: \
                 a pedestrian walking across a moving vehicle's heading inside its swept \
                 width.",
            )
            .with_dims([Dim::T])
            .with_source(ssam)
            .with_min_samples(self.min_samples)
            .not_accounting_for("a turning vehicle's curved path"),
            MetricDef::new(
                "ped_wait",
                "s",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::TIME_S,
                "Pedestrian delay: the length of each standing episode (below 0.2 m/s) that \
                 ended in the window — at a kerb waiting for walk or a gap, mostly.",
            )
            .with_dims([Dim::T])
            .with_source(cards::design("08-measurement-and-data.md §2.5 (traffic and safety)"))
            .with_min_samples(self.min_samples)
            .not_accounting_for("why the pedestrian stood"),
            MetricDef::new(
                "ped_midblock_share",
                "1",
                Agg::Mean,
                Visibility::Gt,
                Quantum::RATIO,
                "The share of pedestrian samples off every lane: time spent crossing \
                 mid-block, away from any crosswalk.",
            )
            .with_dims([Dim::T])
            .with_source(cards::design("08-measurement-and-data.md §2.5 (traffic and safety)"))
            .with_min_samples(1)
            .not_accounting_for(
                "a crossing at an unmarked junction leg mapped without a crossing way, which \
                 is on a lane",
            ),
        ]
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
        self.frame.insert(v.actor, v);
    }

    fn close_frame(&mut self) {
        let Some(t) = self.frame_t.take() else {
            self.frame.clear();
            return;
        };
        let frame = core::mem::take(&mut self.frame);
        let is_ped = |v: &GtKinematicsView| v.class.as_deref() == Some(PEDESTRIAN);
        let peds: Vec<&GtKinematicsView> = frame.values().filter(|v| is_ped(v)).collect();
        // --- waits ---------------------------------------------------------------------
        for p in &peds {
            self.ped_samples += 1;
            if p.lane.is_none() {
                self.offlane_samples += 1;
            }
            self.last_seen.insert(p.actor, t);
            if p.speed_mps < STANDING_MPS {
                self.standing_since.entry(p.actor).or_insert(t);
            } else if let Some(since) = self.standing_since.remove(&p.actor) {
                self.wait.observe(Duration::between(since, t).as_secs_f64());
            }
        }
        // A pedestrian who left the run (finished its walk) ends its episode where it was
        // last seen.
        let gone: Vec<ActorId> = self
            .last_seen
            .iter()
            .filter(|(_, seen)| **seen < t)
            .map(|(a, _)| *a)
            .collect();
        for a in gone {
            let seen = self.last_seen.remove(&a).unwrap_or(t);
            if let Some(since) = self.standing_since.remove(&a) {
                self.wait.observe(Duration::between(since, seen).as_secs_f64());
            }
        }
        // --- conflicts -----------------------------------------------------------------
        let mut now: BTreeMap<(ActorId, ActorId), bool> = BTreeMap::new();
        if !peds.is_empty() {
            for veh in frame.values() {
                if is_ped(veh) || veh.class.as_deref() == Some(BICYCLE) {
                    continue;
                }
                let Some(h) = veh.heading_rad else { continue };
                if veh.speed_mps <= 0.5 {
                    continue;
                }
                let (sn, cs) = math::sin_cos(h);
                let (len, wid) = body_of(veh.class.as_deref());
                let front = (veh.x_m + cs * len, veh.y_m + sn * len);
                for p in &peds {
                    let (dx, dy) = (p.x_m - front.0, p.y_m - front.1);
                    let ahead = dx * cs + dy * sn;
                    if ahead <= 0.0 || ahead > 60.0 {
                        continue;
                    }
                    let side = -dx * sn + dy * cs;
                    if side.abs() > 0.5 * wid + 0.24 + 0.25 {
                        continue;
                    }
                    let (pv_along, pv_across) = match p.heading_rad {
                        Some(ph) => {
                            let (psn, pcs) = math::sin_cos(ph);
                            let (vx, vy) = (p.speed_mps * pcs, p.speed_mps * psn);
                            (vx * cs + vy * sn, -vx * sn + vy * cs)
                        }
                        None => (0.0, 0.0),
                    };
                    if pv_across.abs() < 0.3 {
                        continue;
                    }
                    let closing = veh.speed_mps - pv_along;
                    if closing <= 0.1 {
                        continue;
                    }
                    let ttc = (ahead - 0.1).max(0.0) / closing;
                    if ttc < TTC_SAMPLE_MAX_S {
                        self.ttc.observe(ttc);
                    }
                    // D10: compare on the grid.
                    let g = Quantum::TIME_S.grid(ttc);
                    if g >= Quantum::TIME_S.grid(PED_CONFLICT_TTC_S) {
                        continue;
                    }
                    let near = g < Quantum::TIME_S.grid(PED_NEAR_MISS_TTC_S);
                    let key = (veh.actor, p.actor);
                    let was = self.open.get(&key).copied();
                    if was.is_none() {
                        self.conflicts += 1;
                    }
                    if near && was != Some(true) {
                        self.near_misses += 1;
                    }
                    now.insert(key, near || was == Some(true));
                }
            }
        }
        self.open = now;
    }
}

/// A class's body length and width, metres, from its name (SUMO's vType defaults, as the
/// mobility crate's class table; a car's for a class this provider does not know).
fn body_of(class: Option<&str>) -> (f64, f64) {
    match class {
        Some("emergency" | "delivery") => (6.5, 2.16),
        Some("truck") => (7.1, 2.4),
        Some("trailer") => (16.5, 2.55),
        Some("bus") => (12.0, 2.5),
        Some("coach") => (14.0, 2.6),
        Some("motorcycle") => (2.2, 0.9),
        Some("moped") => (2.1, 0.8),
        Some("scooter") => (1.2, 0.5),
        _ => (5.0, 1.8),
    }
}

impl Model for VruSafetyProvider {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MetricProvider for VruSafetyProvider {
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
        self.close_frame();
        let mut out = Vec::new();
        out.push(MetricSample::new(
            &self.def("ped_conflicts"),
            at,
            Dims::new(),
            SampleValue::count(core::mem::take(&mut self.conflicts)),
        ));
        out.push(MetricSample::new(
            &self.def("ped_near_misses"),
            at,
            Dims::new(),
            SampleValue::count(core::mem::take(&mut self.near_misses)),
        ));
        let min = self.min_samples;
        out.push(MetricSample::new(
            &self.def("ped_ttc"),
            at,
            Dims::new(),
            SampleValue::Distribution(
                core::mem::replace(&mut self.ttc, Distribution::new()).summary(min),
            ),
        ));
        out.push(MetricSample::new(
            &self.def("ped_wait"),
            at,
            Dims::new(),
            SampleValue::Distribution(
                core::mem::replace(&mut self.wait, Distribution::new()).summary(min),
            ),
        ));
        let (num, den) = (
            core::mem::take(&mut self.offlane_samples),
            core::mem::take(&mut self.ped_samples),
        );
        out.push(MetricSample::new(
            &self.def("ped_midblock_share"),
            at,
            Dims::new(),
            SampleValue::Ratio(ratio_of_sums(num as f64, den as f64, den, 1)),
        ));
        out
    }

    fn rejected(&self) -> u64 {
        self.rejected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(t_ms: u64, actor: u32, class: &str, x: f64, y: f64, v: f64, h: f64) -> GtKinematicsView {
        let mut s: GtKinematicsView = serde_json::from_value(json!({
            "t": t_ms * 1_000_000,
            "actor": actor,
            "x_m": x,
            "y_m": y,
            "speed_mps": v,
            "heading_rad": h,
            "class": class,
        }))
        .expect("a kinematics view");
        s.lane = Some(1);
        s
    }

    fn count(out: &[MetricSample], name: &str) -> u64 {
        out.iter()
            .find(|s| s.metric == name)
            .and_then(|s| match s.value {
                SampleValue::Count { count } => Some(count),
                _ => None,
            })
            .expect("a count")
    }

    /// A car at 10 m/s heading east, a pedestrian crossing north 20 m ahead of it: the
    /// time to collision falls through 3 s and then 1.5 s — one conflict, one near miss,
    /// however many steps it lasts.
    #[test]
    fn a_vehicle_closing_on_a_crossing_pedestrian_is_a_near_miss_once() {
        let mut p = VruSafetyProvider::new();
        for k in 0..12u64 {
            let t = k * 100;
            let car_x = k as f64 * 1.0;
            p.on_kinematics(sample(t, 1, "passenger", car_x, 0.0, 10.0, 0.0));
            p.on_kinematics(sample(t, 2, "pedestrian", 24.0, -1.0 + 0.1 * k as f64, 1.4, core::f64::consts::FRAC_PI_2));
        }
        let out = p.flush(1_300_000_000);
        assert_eq!(count(&out, "ped_conflicts"), 1);
        assert_eq!(count(&out, "ped_near_misses"), 1);
    }

    /// The same car with the pedestrian walking east beside it: not across its heading,
    /// so no conflict, though it is inside the swept width.
    #[test]
    fn a_pedestrian_walking_beside_the_lane_is_no_conflict() {
        let mut p = VruSafetyProvider::new();
        for k in 0..12u64 {
            let t = k * 100;
            p.on_kinematics(sample(t, 1, "passenger", k as f64, 0.0, 10.0, 0.0));
            p.on_kinematics(sample(t, 2, "pedestrian", 20.0 + 0.14 * k as f64, 0.5, 1.4, 0.0));
        }
        let out = p.flush(1_300_000_000);
        assert_eq!(count(&out, "ped_conflicts"), 0);
    }

    /// A pedestrian who stands for 2 s and walks on has waited 2 s.
    #[test]
    fn a_standing_episode_is_a_wait() {
        let mut p = VruSafetyProvider::new();
        for k in 0..=30u64 {
            let v = if (5..25).contains(&k) { 0.0 } else { 1.3 };
            p.on_kinematics(sample(k * 100, 7, "pedestrian", 0.0, 0.0, v, 0.0));
        }
        p.close_frame();
        assert_eq!(p.wait.len(), 1);
        match p.wait.summary(1) {
            crate::stats::DistributionSummary::Summary { max, .. } => {
                assert!((max - 2.0).abs() < 1e-9, "{max}");
            }
            other => panic!("{other:?}"),
        }
    }
}
