//! The demand models the engine runs: a list of trips (with their routes, when an
//! assignment or a count sampler chose them), and a composite that runs several demand
//! models side by side — cars from an OD matrix and buses from a GTFS feed in one run.
//!
//! Both implement [`v2xw_mobility::traits::Demand`], including the planned-route and
//! progress seams of [`v2xw_mobility::demand::hooks`].
//!
//! # The daily profile
//!
//! [`TripListDemand`] honours [`v2xw_mobility::traits::Demand::set_multiplier`], so a
//! scenario timeline's `demand.multiplier` — and the long-run engine's daily profile, which
//! drives that same hook — scales a trip list: a multiplier `m` keeps each trip `⌊m⌋`
//! times plus once more with probability `m − ⌊m⌋`, the coin keyed by the trip's id, so the
//! same trips survive whatever else changes. Copies depart `copy · 1 s` later.

use std::collections::BTreeMap;

use v2xw_core::card::{Family, ModelCard, Tier, Validation, ValidationStatus};
use v2xw_core::ids::LaneId;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::{SimTime, secs_to_ns};
use v2xw_mobility::MobCtx;
use v2xw_mobility::demand::SpeedLaw;
use v2xw_mobility::demand::hooks::ActorProgress;
use v2xw_mobility::traits::Demand;
use v2xw_mobility::views::{MobilityCommand, TripRequest};
use v2xw_world::World;

use crate::trips::{Trip, sort_trips};

/// A fixed list of trips, each with an optional planned route.
#[derive(Debug, Clone)]
pub struct TripListDemand {
    model_id: String,
    trips: Vec<Trip>,
    routes: Vec<Option<Vec<LaneId>>>,
    cursor: usize,
    next_seq: u64,
    by_seq: BTreeMap<u64, usize>,
    multiplier: f64,
    speed: SpeedLaw,
    card: ModelCard,
}

impl TripListDemand {
    /// A demand model running `trips`, `routes[i]` (when `Some`) planned for `trips[i]`.
    /// `card` describes where the trips came from; its id keys the model's RNG streams.
    pub fn new(trips: Vec<Trip>, routes: Vec<Option<Vec<LaneId>>>, card: ModelCard) -> Self {
        let mut paired: Vec<(Trip, Option<Vec<LaneId>>)> = trips
            .into_iter()
            .zip(routes.into_iter().chain(std::iter::repeat(None)))
            .collect();
        paired.sort_by(|a, b| {
            a.0.depart_s
                .total_cmp(&b.0.depart_s)
                .then(a.0.id.cmp(&b.0.id))
        });
        let (trips, routes): (Vec<Trip>, Vec<Option<Vec<LaneId>>>) = paired.into_iter().unzip();
        Self {
            model_id: card.id.clone(),
            trips,
            routes,
            cursor: 0,
            next_seq: 0,
            by_seq: BTreeMap::new(),
            multiplier: 1.0,
            speed: SpeedLaw::default(),
            card,
        }
    }

    /// Draws desired speeds with `law` instead of the default.
    #[must_use]
    pub fn with_speed_law(mut self, law: SpeedLaw) -> Self {
        self.speed = law;
        self
    }

    /// The trips, in stream order.
    pub fn trips(&self) -> &[Trip] {
        &self.trips
    }

    /// How many trips carry a planned route.
    pub fn routed(&self) -> usize {
        self.routes.iter().filter(|r| r.is_some()).count()
    }
}

impl v2xw_core::model::Model for TripListDemand {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Demand for TripListDemand {
    fn spawns_in(&mut self, ctx: &mut dyn MobCtx, _from: SimTime, to: SimTime) -> Vec<TripRequest> {
        let mut out = Vec::new();
        while self.cursor < self.trips.len() {
            let i = self.cursor;
            let depart = secs_to_ns(self.trips[i].depart_s.max(0.0));
            if depart >= to {
                break;
            }
            self.cursor += 1;
            let trip = &self.trips[i];
            let whole = self.multiplier.floor().max(0.0) as u64;
            let frac = self.multiplier - self.multiplier.floor();
            let extra = frac > 0.0 && {
                let mut rng = ctx.rng(
                    RngDomain::plugin(&self.model_id),
                    EntityRef::custom(&self.model_id, trip.id),
                );
                rng.bool(frac)
            };
            let copies = whole + u64::from(extra);
            for copy in 0..copies {
                let seq = self.next_seq;
                self.next_seq += 1;
                let desired_speed_mps = {
                    let mut rng = ctx.rng(
                        RngDomain::DesiredSpeed,
                        EntityRef::custom(&self.model_id, seq),
                    );
                    self.speed.draw(trip.class, &mut rng)
                };
                self.by_seq.insert(seq, i);
                out.push(TripRequest {
                    seq,
                    t: depart + copy * v2xw_core::time::NS_PER_S,
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

    fn planned_route(&self, _world: &World, trip: &TripRequest) -> Option<Vec<LaneId>> {
        let i = *self.by_seq.get(&trip.seq)?;
        let t = &self.trips[i];
        if t.origin != trip.origin || t.destination != trip.destination {
            return None;
        }
        self.routes[i].clone()
    }
}

/// Several demand models in one run.
///
/// Each part keeps its own stream; the composite merges the trips of a window in
/// `(time, part, part's seq)` order and gives them one global sequence, remembering which
/// part each came from so planned routes and progress reach the right part with the part's
/// own numbering.
pub struct CompositeDemand {
    parts: Vec<Box<dyn Demand>>,
    origin: BTreeMap<u64, (usize, u64)>,
    next_seq: u64,
    card: ModelCard,
}

impl std::fmt::Debug for CompositeDemand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CompositeDemand")
            .field(
                "parts",
                &self
                    .parts
                    .iter()
                    .map(|p| p.card().id.clone())
                    .collect::<Vec<_>>(),
            )
            .field("next_seq", &self.next_seq)
            .finish()
    }
}

/// The composite's model id.
pub const COMPOSITE_ID: &str = "mobility/demand/composite";

impl CompositeDemand {
    /// The composite of `parts`, run in the order given.
    pub fn new(parts: Vec<Box<dyn Demand>>) -> Self {
        let mut card = ModelCard::new(
            COMPOSITE_ID,
            Family::Mobility,
            "1.0.0",
            "Several demand models in one run, their trips merged in time order under one \
             sequence.",
        );
        card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
        card.assumptions = parts
            .iter()
            .map(|p| format!("part: {}", p.card().id_at_version()))
            .collect();
        card.validation = Validation {
            status: ValidationStatus::UnitTested,
            references: Vec::new(),
            tests: vec!["models::tests::a_composite_merges_in_time_order".to_string()],
        };
        Self {
            parts,
            origin: BTreeMap::new(),
            next_seq: 0,
            card,
        }
    }

    /// The parts' cards.
    pub fn part_cards(&self) -> Vec<&ModelCard> {
        self.parts.iter().map(|p| p.card()).collect()
    }
}

impl v2xw_core::model::Model for CompositeDemand {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl Demand for CompositeDemand {
    fn spawns_in(&mut self, ctx: &mut dyn MobCtx, from: SimTime, to: SimTime) -> Vec<TripRequest> {
        let mut all: Vec<(usize, TripRequest)> = Vec::new();
        for (i, p) in self.parts.iter_mut().enumerate() {
            all.extend(p.spawns_in(ctx, from, to).into_iter().map(|t| (i, t)));
        }
        all.sort_by(|a, b| {
            a.1.t
                .cmp(&b.1.t)
                .then(a.0.cmp(&b.0))
                .then(a.1.seq.cmp(&b.1.seq))
        });
        all.into_iter()
            .map(|(part, mut trip)| {
                let seq = self.next_seq;
                self.next_seq += 1;
                self.origin.insert(seq, (part, trip.seq));
                trip.seq = seq;
                trip
            })
            .collect()
    }

    fn set_multiplier(&mut self, m: f64) -> bool {
        let mut any = false;
        for p in &mut self.parts {
            any |= p.set_multiplier(m);
        }
        any
    }

    fn resume_after_pause(&mut self, paused: v2xw_core::time::Duration) {
        for p in &mut self.parts {
            p.resume_after_pause(paused);
        }
    }

    fn planned_route(&self, world: &World, trip: &TripRequest) -> Option<Vec<LaneId>> {
        let (part, seq) = *self.origin.get(&trip.seq)?;
        let mut own = trip.clone();
        own.seq = seq;
        self.parts[part].planned_route(world, &own)
    }

    fn trip_driver(&self, trip: &TripRequest) -> Option<v2xw_mobility::views::TripDriver> {
        let (part, seq) = *self.origin.get(&trip.seq)?;
        let mut own = trip.clone();
        own.seq = seq;
        self.parts[part].trip_driver(&own)
    }

    fn wants_progress(&self) -> bool {
        self.parts.iter().any(|p| p.wants_progress())
    }

    fn observe(&mut self, t: SimTime, progress: &[ActorProgress]) -> Vec<MobilityCommand> {
        let mut out = Vec::new();
        for (i, p) in self.parts.iter_mut().enumerate() {
            if !p.wants_progress() {
                continue;
            }
            let mine: Vec<ActorProgress> = progress
                .iter()
                .filter_map(|a| {
                    let (part, seq) = *self.origin.get(&a.seq)?;
                    (part == i).then_some(ActorProgress { seq, ..*a })
                })
                .collect();
            out.extend(p.observe(t, &mine));
        }
        out
    }
}

/// A card for a trip list from `source`, citing `sources`.
pub fn trip_list_card(id: &str, purpose: &str, sources: Vec<v2xw_core::card::Source>) -> ModelCard {
    let mut card = ModelCard::new(id, Family::Mobility, "1.0.0", purpose);
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.sources = sources;
    card.determinism = v2xw_core::card::Determinism {
        uses_rng: true,
        rng_domains: vec![
            RngDomain::plugin(id).as_str().to_string(),
            RngDomain::DesiredSpeed.as_str().to_string(),
        ],
    };
    card.validation = Validation::new(ValidationStatus::UnitTested);
    card
}

/// Sorts trips and pairs them with routes by trip id, for callers that hold routes in a
/// map.
pub fn pair_routes(
    mut trips: Vec<Trip>,
    routes: &BTreeMap<u64, Vec<LaneId>>,
) -> (Vec<Trip>, Vec<Option<Vec<LaneId>>>) {
    sort_trips(&mut trips);
    let r = trips.iter().map(|t| routes.get(&t.id).cloned()).collect();
    (trips, r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::rng::RngRegistry;
    use v2xw_mobility::MobilityCtx;

    fn trip(id: u64, t: f64, lane: u32) -> Trip {
        Trip {
            id,
            depart_s: t,
            origin: LaneId::new(lane),
            destination: LaneId::new(lane),
            class: Default::default(),
            purpose: None,
            tag: None,
        }
    }

    #[test]
    fn a_composite_merges_in_time_order() {
        let w = crate::graph::tests::grid();
        let rng = RngRegistry::new(7);
        let a = TripListDemand::new(
            vec![trip(1, 0.5, 0), trip(2, 2.0, 0)],
            vec![Some(vec![LaneId::new(0)]), None],
            trip_list_card("mobility/demand/test-a", "test", Vec::new()),
        );
        let b = TripListDemand::new(
            vec![trip(9, 1.0, 1)],
            vec![Some(vec![LaneId::new(1)])],
            trip_list_card("mobility/demand/test-b", "test", Vec::new()),
        );
        let mut c = CompositeDemand::new(vec![Box::new(a), Box::new(b)]);
        let mut ctx = MobilityCtx::new(0, &w, &rng);
        let got = c.spawns_in(&mut ctx, 0, secs_to_ns(3.0));
        let order: Vec<(u64, u32)> = got
            .iter()
            .map(|t| (t.seq, t.origin.as_usize() as u32))
            .collect();
        assert_eq!(order, vec![(0, 0), (1, 1), (2, 0)]);
        // Planned routes find their way back to the right part.
        assert_eq!(c.planned_route(&w, &got[1]), Some(vec![LaneId::new(1)]));
        assert_eq!(c.planned_route(&w, &got[2]), None);
    }

    #[test]
    fn a_multiplier_thins_and_copies_by_trip() {
        let w = crate::graph::tests::grid();
        let rng = RngRegistry::new(7);
        let trips: Vec<Trip> = (0..1000).map(|i| trip(i, 0.0, 0)).collect();
        let mut d = TripListDemand::new(
            trips,
            Vec::new(),
            trip_list_card("mobility/demand/test-m", "test", Vec::new()),
        );
        d.set_multiplier(0.5);
        let mut ctx = MobilityCtx::new(0, &w, &rng);
        let n = d.spawns_in(&mut ctx, 0, secs_to_ns(1.0)).len();
        assert!((430..570).contains(&n), "{n}");
    }
}
