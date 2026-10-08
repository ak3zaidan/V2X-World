//! `demand/loader/micro` — the engine itself as the network loader.
//!
//! Each iteration runs [`v2xw_mobility::NativeMobility`] — car following, lane changes,
//! signals, gap acceptance, insertion queues — with every trip on its chosen route (the
//! planned-route seam of [`v2xw_mobility::demand::hooks`]), and measures each traversal of
//! each road from the vehicles' lanes at every step. This is what SUMO's `duaIterate`
//! does with `sumo` and its edge-data dumps [SUMO `duaIterate.py` documentation]; it is the
//! most faithful loader and by far the slowest, so it is the one to finish an assignment
//! with after the queue loader has brought it close.
//!
//! A traversal is measured per road (edge), not per lane, because a lane change moves a
//! vehicle sideways without ending its trip along the road; junction connectors, on which
//! nobody changes lane, are measured per lane. The measured time is credited to the lane
//! the vehicle entered the road on.

use std::collections::BTreeMap;

use v2xw_core::card::{
    Determinism, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::ids::{ActorId, LaneId};
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, ns_to_secs, secs_to_ns};
use v2xw_mobility::views::DespawnCause;
use v2xw_mobility::{EngineParams, Mobility, MobilityCtx, NativeMobility};
use v2xw_world::World;

use super::{Loading, NetworkLoader, Window};
use crate::costs::TravelTimes;
use crate::error::{DemandError, Result};
use crate::graph::{LaneGraph, Node};
use crate::models::{TripListDemand, trip_list_card};
use crate::trips::Trip;

/// The model id.
pub const MODEL_ID: &str = "demand/loader/micro";

/// The engine's default step, ms.
pub fn default_step_ms() -> u64 {
    100
}

/// The microscopic loader.
#[derive(Debug, Clone)]
pub struct Micro {
    step_ms: u64,
    seed: u64,
    card: ModelCard,
}

impl Micro {
    /// The loader running the engine at `step_ms` with master seed `seed`.
    pub fn new(step_ms: u64, seed: u64) -> Self {
        let step_ms = step_ms.max(10);
        let mut card = ModelCard::new(
            MODEL_ID,
            Family::Mobility,
            "1.0.0",
            "Microscopic network loading: the native engine drives every chosen route and \
             each road traversal is timed from the vehicles' lanes.",
        );
        card.tier = vec![Tier::Medium, Tier::High];
        card.parameters = vec![Parameter::new(
            "step_ms",
            "ms",
            serde_json::json!(step_ms),
            Source::new(
                SourceKind::Code,
                "the engine's mobility step (EngineParams::step)",
            ),
        )];
        card.sources = vec![Source::new(
            SourceKind::Code,
            "SUMO duaIterate.py: one microscopic simulation per iteration, edge travel times \
             from its edgeData dumps (sumo.dlr.de/docs/Demand/Dynamic_User_Assignment.html)",
        )];
        card.determinism = Determinism {
            uses_rng: true,
            rng_domains: vec!["the engine's own".to_string()],
        };
        card.validation = Validation {
            status: ValidationStatus::UnitTested,
            references: Vec::new(),
            tests: vec!["assign::tests::the_micro_loader_runs_the_routes".to_string()],
        };
        Self {
            step_ms,
            seed,
            card,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct OnRoad {
    trip: usize,
    node: Node,
    key: (bool, u32),
    since_s: f64,
}

impl NetworkLoader for Micro {
    fn card(&self) -> &ModelCard {
        &self.card
    }

    fn load(
        &mut self,
        world: &World,
        graph: &LaneGraph,
        trips: &[Trip],
        routes: &[Vec<Node>],
        window: Window,
    ) -> Result<Loading> {
        let mut times = TravelTimes::new(graph, window.begin_s, window.end_s, window.bin_s);
        let mut experienced = vec![f64::NAN; trips.len()];
        // The demand model sorts by (depart, id); its seq k is the k-th trip in that order.
        let mut order: Vec<usize> = (0..trips.len()).collect();
        order.sort_by(|a, b| {
            trips[*a]
                .depart_s
                .total_cmp(&trips[*b].depart_s)
                .then(trips[*a].id.cmp(&trips[*b].id))
        });
        let shifted: Vec<Trip> = order
            .iter()
            .map(|i| Trip {
                depart_s: trips[*i].depart_s - window.begin_s,
                ..trips[*i].clone()
            })
            .collect();
        let planned: Vec<Option<Vec<LaneId>>> = order
            .iter()
            .map(|i| Some(graph.lanes_of(&routes[*i])))
            .collect();
        let demand = TripListDemand::new(
            shifted,
            planned,
            trip_list_card(
                MODEL_ID,
                "the assignment's trips on their chosen routes",
                Vec::new(),
            ),
        );
        let rng = RngRegistry::new(self.seed);
        let params = EngineParams {
            step: Duration::from_millis(self.step_ms),
            ..EngineParams::default()
        };
        let mut engine = NativeMobility::new(params);
        {
            let mut ctx = MobilityCtx::new(0, world, &rng);
            engine
                .init(&mut ctx, Box::new(demand))
                .map_err(|e| DemandError::Invalid(format!("the micro loader's engine: {e}")))?;
        }
        let key_of = |lane: LaneId| -> (bool, u32) {
            let l = world.lane(lane);
            if l.kind == v2xw_world::LaneKind::Internal {
                (true, lane.0)
            } else {
                (false, l.edge.0)
            }
        };
        let mut on: BTreeMap<ActorId, OnRoad> = BTreeMap::new();
        let mut seq_trip: BTreeMap<u64, usize> = BTreeMap::new();
        for (k, i) in order.iter().enumerate() {
            seq_trip.insert(k as u64, *i);
        }
        let end = secs_to_ns((window.end_s - window.begin_s).max(0.0));
        let mut t = 0u64;
        let mut arrived = 0usize;
        let total = trips.len();
        let mut finished = 0usize;
        while t < end && finished < total {
            let mut ctx = MobilityCtx::new(t, world, &rng);
            let update = engine.step(&mut ctx, params.step);
            let now_s = ns_to_secs(update.t) + window.begin_s;
            for s in &update.spawned {
                if let Some(trip) = seq_trip.get(&s.seq).copied() {
                    let lane = s.route.lanes.first().copied().unwrap_or(trips[trip].origin);
                    on.insert(
                        s.actor,
                        OnRoad {
                            trip,
                            node: graph.node(lane).unwrap_or(0),
                            key: key_of(lane),
                            since_s: ns_to_secs(s.t) + window.begin_s,
                        },
                    );
                }
            }
            for a in engine.audit_actors(world, update.t) {
                let Some(o) = on.get_mut(&a.actor) else {
                    continue;
                };
                let key = key_of(a.lane);
                if key != o.key {
                    times.record(o.node, o.since_s, now_s - o.since_s);
                    o.key = key;
                    o.since_s = now_s;
                    if let Some(n) = graph.node(a.lane) {
                        o.node = n;
                    }
                }
            }
            for (actor, cause) in &update.despawned {
                if let Some(o) = on.remove(actor) {
                    times.record(o.node, o.since_s, now_s - o.since_s);
                    finished += 1;
                    if *cause == DespawnCause::TripComplete {
                        experienced[o.trip] = now_s - trips[o.trip].depart_s;
                        arrived += 1;
                    }
                }
            }
            t = update.t;
        }
        Ok(Loading {
            times,
            experienced_s: experienced,
            arrived,
        })
    }
}
