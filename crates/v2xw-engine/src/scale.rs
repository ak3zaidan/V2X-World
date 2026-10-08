//! City scale: the `scale` section of a scenario (the city-scale track).
//!
//! # What it does
//!
//! Three things, each a seam the rest of the engine already had:
//!
//! 1. **Traffic tiers.** `scale.traffic.model` picks a mobility provider from
//!    [`TRAFFIC_MODELS`]: `mobility/tiered/meso-micro` drives the box `scale.traffic.region`
//!    microscopically and the rest of the map with the queue network
//!    (`v2xw_mobility::meso`, Eissfeldt 2003 / SUMO MESO), handing vehicles across under one
//!    actor id; `mobility/meso/queue` drives the whole map as queues.
//! 2. **The radio follows.** With `scale.radio.follows` (the default) and no focus region
//!    of the scenario's own, [`normalise`] makes the microscopic box the radio's focus
//!    region (02-architecture.md §7.3): the declared propagation tier inside it, the
//!    abstract tier (free space, no fading) outside. Everything else about the focus
//!    region — the crossing rules, the boundary-bias tags on `phy.rx` — is the existing
//!    machinery, unchanged.
//! 3. **Radios where they matter.** A vehicle in the queue network farther than
//!    `scale.radio.margin_m` from the box ([`RadioGate`]) is *dormant*: its node does not
//!    step (no generation, no signing, no medium access) and it is not a candidate
//!    receiver. Inside the margin it transmits and receives as any node does, so the box
//!    sees every frame and every joule of interference that can reach it — rule 3 of the
//!    focus region — and the margin defaults to the radio's own reach for exactly that
//!    reason. Inside the margin but outside the box a vehicle *transmits only*: it is not a
//!    candidate receiver unless `scale.radio.receive_in_margin` is on, because its
//!    receptions are not where the study is (rule 4 of the focus region already says the
//!    outside is evaluated by the cheap rule) and they are most of a city's reception
//!    work. A dormant node keeps its identity, credentials and state, and wakes when its
//!    vehicle comes within the margin; a vehicle spawned dormant gets its node then.
//!
//! # What it costs in fidelity, stated
//!
//! * Outside the margin there is no radio at all: a study of channel load across the whole
//!   city needs the margin set to cover it (and pays for it).
//! * A vehicle in the margin hears nothing (unless `receive_in_margin`), so its channel
//!   busy ratio — and the DCC state it would derive — reads an idle channel: it transmits
//!   at the rate an idle channel allows, which can only *over*state the interference the
//!   box sees.
//! * A dormant node's GNSS belief and application state stand still until it wakes; the
//!   first BSM after waking carries a fresh fix, as a radio switched on would.
//! * Inside the box the radio's PHY is raised to the declared propagation tier when that
//!   is higher than `radio.tiers.phy` (the focus region has one tier).
//!
//! # How to add another traffic tier
//!
//! Implement `v2xw_mobility::Mobility` (and `tier_of`, if it has more than one tier), add
//! its id to [`TRAFFIC_MODELS`] and a branch to [`build_mobility`]. The scenario validator
//! and the settings page read the list.

use std::collections::BTreeSet;

use v2xw_core::card::Tier;
use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_mobility::Mobility;
use v2xw_mobility::meso::tiered::{TierRegion, TieredMobility, TieredParams};
use v2xw_mobility::meso::{MesoMobility, MesoParams};
use v2xw_world::World;

use crate::scenario::Scenario;
use crate::scenario::schema::{Focus, FocusRegion, ScaleMeso};

/// The traffic models `scale.traffic.model` may name.
pub const TRAFFIC_MODELS: [&str; 2] = [
    v2xw_mobility::meso::tiered::MODEL_ID,
    v2xw_mobility::meso::MODEL_ID,
];

/// The queue network's parameters from the scenario's.
#[must_use]
pub fn meso_params(m: &ScaleMeso) -> MesoParams {
    MesoParams {
        segment_length_m: m.segment_length_m,
        tau_ff_s: m.tau_ff_s,
        tau_fj_s: m.tau_fj_s,
        tau_jf_s: m.tau_jf_s,
        tau_jj_s: m.tau_jj_s,
        jam_threshold: m.jam_threshold,
        junction_control: m.junction_control,
    }
}

/// The microscopic box in the world's frame, when the scenario has one.
#[must_use]
pub fn region(scenario: &Scenario, world: &World) -> Option<TierRegion> {
    let bbox = scenario.scale.as_ref()?.traffic.region?;
    let origin: v2xw_core::geo::GeoOrigin = world.origin.into();
    let a = origin.to_enu(bbox.min_lat_deg, bbox.min_lon_deg, 0.0);
    let b = origin.to_enu(bbox.max_lat_deg, bbox.max_lon_deg, 0.0);
    Some(TierRegion::Box {
        min_m: [a.x.min(b.x), a.y.min(b.y)],
        max_m: [a.x.max(b.x), a.y.max(b.y)],
    })
}

/// The mobility provider `scale.traffic` names, or `None` when the scenario has no
/// `scale` section (the caller then builds the ordinary microscopic engine).
#[must_use]
pub fn build_mobility(scenario: &Scenario, world: &World) -> Option<Box<dyn Mobility>> {
    let scale = scenario.scale.as_ref()?;
    let meso = meso_params(&scale.traffic.meso);
    match scale.traffic.model.as_str() {
        m if m == v2xw_mobility::meso::MODEL_ID => Some(Box::new(MesoMobility::new(meso))),
        m if m == v2xw_mobility::meso::tiered::MODEL_ID => {
            let region = region(scenario, world)?;
            Some(Box::new(TieredMobility::new(
                TieredParams { region, meso },
                crate::wiring::native_mobility(scenario),
            )))
        }
        _ => None,
    }
}

/// The scenario the engine runs: with `scale.radio.follows`, the microscopic box becomes
/// the radio's focus region at the declared propagation tier, and the abstract tier runs
/// outside it. A scenario with a focus region of its own, or no `scale`, is returned as
/// it is.
#[must_use]
pub fn normalise(mut scenario: Scenario) -> Scenario {
    let Some(scale) = scenario.scale.as_ref() else {
        return scenario;
    };
    let Some(bbox) = scale.traffic.region else {
        return scenario;
    };
    if !scale.radio.follows || scenario.radio.tiers.focus.is_some() {
        return scenario;
    }
    let tiers = &mut scenario.radio.tiers;
    let inside = tiers.propagation.max(Tier::Medium);
    tiers.focus = Some(Focus {
        region: FocusRegion::Bbox { bbox },
        tier: inside,
    });
    tiers.propagation = Tier::Abstract;
    scenario
}

/// Which nodes are dormant: their vehicle is in the queue network and farther than the
/// margin from the microscopic box.
#[derive(Debug, Clone)]
pub struct RadioGate {
    region: TierRegion,
    margin_m: f64,
    dormant: BTreeSet<NodeId>,
    /// Queued vehicles inside the margin: they transmit, and receive only when
    /// `scale.radio.receive_in_margin` says so.
    margin_only: BTreeSet<NodeId>,
    /// Whether a vehicle in the margin receives.
    receive_in_margin: bool,
    /// Node-steps a dormant node did not take, over the run.
    pub dormant_node_steps: u64,
    /// The most nodes awake at once.
    pub peak_awake: u64,
    /// The most nodes dormant at once.
    pub peak_dormant: u64,
    /// Equipped vehicles whose node is not created yet, this step (set by the engine).
    pub deferred: u64,
    /// The most of those at once.
    pub peak_deferred: u64,
}

impl RadioGate {
    /// The gate for `scenario`, if it has a microscopic box. `reach_m` is the radio's own
    /// reach, the margin's default.
    #[must_use]
    pub fn for_scenario(scenario: &Scenario, world: &World, reach_m: f64) -> Option<Self> {
        let region = region(scenario, world)?;
        let radio = scenario.scale.as_ref()?.radio;
        let margin_m = radio.margin_m.unwrap_or(reach_m);
        Some(Self {
            region,
            margin_m,
            dormant: BTreeSet::new(),
            margin_only: BTreeSet::new(),
            receive_in_margin: radio.receive_in_margin,
            dormant_node_steps: 0,
            peak_awake: 0,
            peak_dormant: 0,
            deferred: 0,
            peak_deferred: 0,
        })
    }

    /// The margin in force, metres.
    #[must_use]
    pub fn margin_m(&self) -> f64 {
        self.margin_m
    }

    /// Whether a vehicle at `pos` driven by tier `tier` has its radio off.
    #[must_use]
    pub fn would_be_dormant(&self, pos: Vec3, tier: Option<Tier>) -> bool {
        tier == Some(Tier::Abstract) && self.region.distance_outside(pos) > self.margin_m
    }

    /// Recomputes the dormant set from every node's position and its vehicle's tier.
    pub fn update(&mut self, nodes: impl Iterator<Item = (NodeId, Vec3, Option<Tier>)>) {
        self.dormant.clear();
        self.margin_only.clear();
        self.peak_deferred = self.peak_deferred.max(self.deferred);
        let mut awake = 0u64;
        for (node, pos, tier) in nodes {
            if self.would_be_dormant(pos, tier) {
                self.dormant.insert(node);
            } else {
                awake += 1;
                if tier == Some(Tier::Abstract) {
                    self.margin_only.insert(node);
                }
            }
        }
        let dormant = self.dormant.len() as u64;
        self.dormant_node_steps += dormant;
        self.peak_awake = self.peak_awake.max(awake);
        self.peak_dormant = self.peak_dormant.max(dormant);
    }

    /// Whether `node` is dormant this step.
    #[must_use]
    pub fn is_dormant(&self, node: NodeId) -> bool {
        self.dormant.contains(&node)
    }

    /// Whether `node` is a candidate receiver this step: awake, and either microscopic or
    /// in the margin with `receive_in_margin` on.
    #[must_use]
    pub fn receives(&self, node: NodeId) -> bool {
        !self.dormant.contains(&node)
            && (self.receive_in_margin || !self.margin_only.contains(&node))
    }

    /// How many nodes are dormant this step.
    #[must_use]
    pub fn dormant_count(&self) -> usize {
        self.dormant.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_queued_vehicle_past_the_margin_is_dormant_and_nothing_else_is() {
        let mut gate = RadioGate {
            region: TierRegion::Box {
                min_m: [0.0, 0.0],
                max_m: [100.0, 100.0],
            },
            margin_m: 50.0,
            dormant: BTreeSet::new(),
            margin_only: BTreeSet::new(),
            receive_in_margin: false,
            dormant_node_steps: 0,
            peak_awake: 0,
            peak_dormant: 0,
            deferred: 0,
            peak_deferred: 0,
        };
        let far = Vec3::new(200.0, 50.0, 0.0);
        let near = Vec3::new(140.0, 50.0, 0.0);
        gate.update(
            [
                (NodeId::new(1), far, Some(Tier::Abstract)),
                (NodeId::new(2), near, Some(Tier::Abstract)),
                (NodeId::new(3), far, Some(Tier::Medium)),
                (NodeId::new(4), far, None),
            ]
            .into_iter(),
        );
        assert!(gate.is_dormant(NodeId::new(1)));
        assert!(!gate.is_dormant(NodeId::new(2)), "inside the margin");
        assert!(!gate.is_dormant(NodeId::new(3)), "microscopic");
        assert!(!gate.is_dormant(NodeId::new(4)), "a single-tier provider");
        assert_eq!(gate.dormant_node_steps, 1);
        // The margin transmits and does not receive; the box does both.
        assert!(!gate.receives(NodeId::new(1)));
        assert!(
            !gate.receives(NodeId::new(2)),
            "in the margin: transmit only"
        );
        assert!(gate.receives(NodeId::new(3)));
        assert!(gate.receives(NodeId::new(4)));
    }
}
