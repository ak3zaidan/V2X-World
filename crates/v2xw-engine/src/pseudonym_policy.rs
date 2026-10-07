//! Pseudonym-change strategies as they are specified or deployed, enforced from the engine,
//! and the passive eavesdropper's coverage.
//!
//! The node's certificate store rotates by a simple rule (`v2xw_node::stores::
//! RotationPolicy`: an age, a distance, both or either). That is the whole of SAE J2945/1's
//! `CERTCHG` (five minutes) and of the NYC pilot's two kilometres, and those two stay
//! where they are. The strategies here need what the store does not keep — a stage, a
//! random draw per change, where the vehicle is — so the engine decides and **asks the
//! store to change** by handing it a policy that is due now; the store then changes every
//! identifier together (certificate, BSM `TemporaryID`, which is the certificate digest's
//! first four octets, and the link-layer address derived from it), exactly as it does for a
//! scheduled change.
//!
//! | `security.pseudonym_change.strategy` | Rule | Source |
//! |---|---|---|
//! | `time` | at `period_s` of age (default 300 s) | SAE J2945/1 `CERTCHG`, USDOT SCMS primer |
//! | `distance` | after `distance_m` (default 2 km) | NYC CV pilot |
//! | `c2c-cc` | first change after a random [800 m, 1 500 m]; the second at least 800 m on and a further random [120 s, 360 s]; the third after a random [10 km, 20 km]; then every random [25 km, 35 km] | C2C-CC Basic System Profile R1.5.1 (2020-07-31), RS_BSP_521–524 |
//! | `mix-zone` | change inside a mix zone at a signalised intersection once the pseudonym is `mix_zone_min_age_s` old (default 60 s); frames sent inside a zone are encrypted under the zone key and unreadable to an eavesdropper | Freudiger, Raya, Félegyházi, Papadimitratos, Hubaux, *Mix-zones for location privacy in vehicular networks*, WiN-ITS 2007 (CMIX) |
//! | `silent` | no scheduled change (the control arm) | — |
//!
//! `silent_period_s: [min, max]` adds a random silent period after every change, under
//! any strategy: the vehicle sends no safety message for a uniform draw from the range
//! (Huang, Matsuura, Yamane, Sezaki, *Enhancing wireless location privacy using silent
//! period*, IEEE WCNC 2005; the PRESERVE project's 3–13 s). A silent vehicle is invisible
//! to its neighbours' safety applications too, which is the price the literature names.
//!
//! RS_BSP_520 (no messages while stationary after a cold start, and a change on first
//! moving) has nothing to act on: vehicles here enter the network already driving.
//!
//! # The eavesdropper's coverage
//!
//! `threats.eavesdropper` places passive sniffers at a fraction of the signalised
//! intersections — the deployment an adversary with roadside receivers would choose, and
//! the one 07-threats-and-detection.md §6 places the observer at — each hearing every
//! safety frame sent within `range_m` of it (a disc: the sniffer is a receiver on a pole
//! with no transmitter, and its range is a parameter, not a link budget). Without the key
//! the observer is global, the worst case it always was.

use std::collections::BTreeMap;

use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngDomain, RngRegistry};
use v2xw_core::time::{SimTime, secs_to_ns};

use crate::error::{EngineError, Result};
use crate::scenario::Scenario;

/// The model id the policy's random draws are keyed under.
pub const MODEL_ID: &str = "security/pseudonym-policy";

/// The eavesdropper model a scenario names in `threats.eavesdropper.id`.
pub const EAVESDROPPER_ID: &str = "threat/observer/passive-privacy";

/// RS_BSP_521: the first change after a random distance in [800 m, 1 500 m].
pub const C2C_FIRST_M: (f64, f64) = (800.0, 1_500.0);
/// RS_BSP_522: at least 800 m on ...
pub const C2C_SECOND_MIN_M: f64 = 800.0;
/// ... and then a further random [120 s, 360 s].
pub const C2C_SECOND_EXTRA_S: (f64, f64) = (120.0, 360.0);
/// RS_BSP_523: the third change after a random [10 km, 20 km].
pub const C2C_THIRD_M: (f64, f64) = (10_000.0, 20_000.0);
/// RS_BSP_524: every further change after a random [25 km, 35 km].
pub const C2C_LATER_M: (f64, f64) = (25_000.0, 35_000.0);

/// The mix-zone radius around a signalised intersection's centre, metres: the junction box
/// and its approaches' stop lines. CMIX's zone is the intersection the RSU keys; its size
/// is not fixed by the paper, so it is a parameter.
pub const DEFAULT_MIX_ZONE_RADIUS_M: f64 = 30.0;

/// The age a pseudonym must reach before a mix zone changes it, seconds: long enough that
/// a vehicle crossing two adjacent intersections does not spend a pseudonym per block,
/// short against J2945/1's five minutes. A choice, not a citation; CMIX states none.
pub const DEFAULT_MIX_ZONE_MIN_AGE_S: f64 = 60.0;

/// The default sniffer range, metres: a roadside receiver's reliable 5.9 GHz range in an
/// urban street, well inside the few hundred metres DSRC reaches line-of-sight.
pub const DEFAULT_SNIFFER_RANGE_M: f64 = 200.0;

/// What the engine does beyond the store's own rule.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Rule {
    /// The store's own rule decides (`time`, `distance`, `silent`).
    Store,
    /// C2C-CC BSP RS_BSP_521–524.
    C2cCc,
    /// Change inside a mix zone once the pseudonym is `min_age` old.
    MixZone { min_age: SimTime },
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct NodeRule {
    /// Changes this vehicle has made.
    changes: u32,
    /// When the current pseudonym became active.
    since: SimTime,
    /// The odometer at that instant, metres.
    odometer_at: f64,
    /// The distance the current stage waits for, metres.
    target_m: f64,
    /// RS_BSP_522's extra time, seconds, and when its 800 m were reached.
    extra_s: f64,
    reached_at: Option<SimTime>,
    /// No safety message before this instant.
    silent_until: SimTime,
    /// A change has been asked of the store and not yet made.
    requested: bool,
    /// Whether it was inside a mix zone at the last step.
    in_zone: bool,
}

/// The run's pseudonym-change policy and eavesdropper coverage.
#[derive(Debug, Clone)]
pub struct PseudonymPolicy {
    rule: Rule,
    silent: Option<(f64, f64)>,
    zones: Vec<Vec3>,
    zone_radius_m: f64,
    sniffers: Option<(Vec<Vec3>, f64)>,
    nodes: BTreeMap<NodeId, NodeRule>,
    /// Safety frames a silent period withheld.
    pub silenced_frames: u64,
    /// Changes the engine asked the store for.
    pub requested_changes: u64,
}

/// The signalised intersections' centres, in signal-plan order.
fn signalised(world: &v2xw_world::World) -> Vec<Vec3> {
    let junctions = world.roads.junctions();
    let mut out: Vec<Vec3> = world
        .signals
        .iter()
        .filter_map(|s| junctions.get(s.junction.index() as usize))
        .map(|j| j.position)
        .collect();
    out.dedup_by(|a, b| (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6);
    out
}

/// Every `1/f`-th of `sites`, evenly: site `i` is kept when `⌊(i+1)f⌋ > ⌊i·f⌋`. Evenly
/// spread and free of any draw, so the same scenario always has the same sniffers.
fn every_fraction(sites: &[Vec3], fraction: f64) -> Vec<Vec3> {
    let f = fraction.clamp(0.0, 1.0);
    sites
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            let i = *i as f64;
            ((i + 1.0) * f).floor() > (i * f).floor()
        })
        .map(|(_, p)| *p)
        .collect()
}

fn conflict(field: &str, message: impl Into<String>) -> EngineError {
    EngineError::Scenario(crate::ScenarioError::conflict(field, message.into()))
}

impl PseudonymPolicy {
    /// The policy a scenario states.
    ///
    /// # Errors
    /// [`EngineError::Scenario`] for a silent-period range that is not `[min, max]` with
    /// `0 ≤ min ≤ max`, a negative mix-zone radius, or an eavesdropper this build does not
    /// ship.
    pub fn from_scenario(scenario: &Scenario, world: &v2xw_world::World) -> Result<Self> {
        let spec = &scenario.security.pseudonym_change;
        let rule = match spec.strategy.as_str() {
            "c2c-cc" => Rule::C2cCc,
            "mix-zone" => Rule::MixZone {
                min_age: secs_to_ns(
                    spec.mix_zone_min_age_s
                        .unwrap_or(DEFAULT_MIX_ZONE_MIN_AGE_S)
                        .max(0.0),
                ),
            },
            _ => Rule::Store,
        };
        let silent = match spec.silent_period_s.as_deref() {
            None => None,
            Some([lo, hi]) if lo.is_finite() && hi.is_finite() && *lo >= 0.0 && lo <= hi => {
                Some((*lo, *hi))
            }
            Some(other) => {
                return Err(conflict(
                    "security.pseudonym_change.silent_period_s",
                    format!("must be [min, max] seconds with 0 ≤ min ≤ max, got {other:?}"),
                ));
            }
        };
        let zone_radius_m = spec.mix_zone_radius_m.unwrap_or(DEFAULT_MIX_ZONE_RADIUS_M);
        if !(zone_radius_m.is_finite() && zone_radius_m > 0.0) {
            return Err(conflict(
                "security.pseudonym_change.mix_zone_radius_m",
                format!("must be a positive distance, got {zone_radius_m}"),
            ));
        }
        let zones = if matches!(rule, Rule::MixZone { .. }) {
            signalised(world)
        } else {
            Vec::new()
        };
        let sniffers = match &scenario.threats.eavesdropper {
            None => None,
            Some(choice) => {
                if choice.id != EAVESDROPPER_ID {
                    return Err(conflict(
                        "threats.eavesdropper.id",
                        format!(
                            "'{}' is not an eavesdropper this build ships; one: {EAVESDROPPER_ID}",
                            choice.id
                        ),
                    ));
                }
                let fraction = choice
                    .params
                    .get("sniffer_fraction")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(1.0);
                let range = choice
                    .params
                    .get("range_m")
                    .and_then(serde_json::Value::as_f64)
                    .unwrap_or(DEFAULT_SNIFFER_RANGE_M);
                if !(0.0..=1.0).contains(&fraction) || !(range.is_finite() && range > 0.0) {
                    return Err(conflict(
                        "threats.eavesdropper.params",
                        format!(
                            "sniffer_fraction must be in [0, 1] and range_m positive; got \
                             {fraction} and {range}"
                        ),
                    ));
                }
                let mut sites = signalised(world);
                if sites.is_empty() {
                    sites = world.roads.junctions().iter().map(|j| j.position).collect();
                }
                Some((every_fraction(&sites, fraction), range))
            }
        };
        Ok(Self {
            rule,
            silent,
            zones,
            zone_radius_m,
            sniffers,
            nodes: BTreeMap::new(),
            silenced_frames: 0,
            requested_changes: 0,
        })
    }

    /// Whether anything here acts at all: a strategy the store cannot express, or a
    /// silent period.
    #[must_use]
    pub fn acts(&self) -> bool {
        self.rule != Rule::Store || self.silent.is_some()
    }

    /// The sniffer sites, when the eavesdropper has limited coverage.
    #[must_use]
    pub fn sniffer_sites(&self) -> Option<&[Vec3]> {
        self.sniffers.as_ref().map(|(s, _)| s.as_slice())
    }

    fn draw(rng: &RngRegistry, node: NodeId, (lo, hi): (f64, f64)) -> f64 {
        rng.checkout(RngDomain::plugin(MODEL_ID), EntityRef::Node(node))
            .uniform(lo, hi)
    }

    /// The distance the next stage of RS_BSP_521–524 waits for, after `changes` changes.
    fn c2c_target(rng: &RngRegistry, node: NodeId, changes: u32) -> (f64, f64) {
        match changes {
            0 => (Self::draw(rng, node, C2C_FIRST_M), 0.0),
            1 => (C2C_SECOND_MIN_M, Self::draw(rng, node, C2C_SECOND_EXTRA_S)),
            2 => (Self::draw(rng, node, C2C_THIRD_M), 0.0),
            _ => (Self::draw(rng, node, C2C_LATER_M), 0.0),
        }
    }

    /// A vehicle joined the network with its first pseudonym active.
    pub fn on_spawn(&mut self, rng: &RngRegistry, node: NodeId, now: SimTime, odometer_m: f64) {
        let mut r = NodeRule {
            since: now,
            odometer_at: odometer_m,
            ..NodeRule::default()
        };
        if self.rule == Rule::C2cCc {
            (r.target_m, r.extra_s) = Self::c2c_target(rng, node, 0);
        }
        self.nodes.insert(node, r);
    }

    /// A vehicle left.
    pub fn on_retire(&mut self, node: NodeId) {
        self.nodes.remove(&node);
    }

    /// Before the vehicle's step: whether the engine asks its store for a change now.
    pub fn change_due(&mut self, node: NodeId, now: SimTime, odometer_m: f64, pos: Vec3) -> bool {
        let rule = self.rule;
        let zone_r2 = self.zone_radius_m * self.zone_radius_m;
        let in_zone = matches!(rule, Rule::MixZone { .. })
            && self.zones.iter().any(|z| {
                let (dx, dy) = (pos.x - z.x, pos.y - z.y);
                dx * dx + dy * dy <= zone_r2
            });
        let Some(r) = self.nodes.get_mut(&node) else {
            return false;
        };
        r.in_zone = in_zone;
        if r.requested {
            return false;
        }
        let travelled = odometer_m - r.odometer_at;
        let due = match rule {
            Rule::Store => false,
            Rule::C2cCc => {
                if r.changes == 1 {
                    if r.reached_at.is_none() && travelled >= r.target_m {
                        r.reached_at = Some(now);
                    }
                    r.reached_at
                        .is_some_and(|at| now >= at.saturating_add(secs_to_ns(r.extra_s)))
                } else {
                    travelled >= r.target_m
                }
            }
            // Inside a zone with a pseudonym old enough to give up.
            Rule::MixZone { min_age } => in_zone && now.saturating_sub(r.since) >= min_age,
        };
        if due {
            r.requested = true;
            self.requested_changes += 1;
        }
        due
    }

    /// The vehicle's store changed pseudonym (for any reason: scheduled, asked, expired).
    pub fn on_changed(&mut self, rng: &RngRegistry, node: NodeId, now: SimTime, odometer_m: f64) {
        let silent = self.silent;
        let c2c = self.rule == Rule::C2cCc;
        let Some(r) = self.nodes.get_mut(&node) else {
            return;
        };
        r.changes = r.changes.saturating_add(1);
        r.since = now;
        r.odometer_at = odometer_m;
        r.requested = false;
        r.reached_at = None;
        if c2c {
            (r.target_m, r.extra_s) = Self::c2c_target(rng, node, r.changes);
        }
        if let Some(range) = silent {
            let s = Self::draw(rng, node, range);
            r.silent_until = now.saturating_add(secs_to_ns(s));
        }
    }

    /// Whether the vehicle is inside a silent period: it sends no safety message.
    #[must_use]
    pub fn is_silent(&self, node: NodeId, now: SimTime) -> bool {
        self.nodes.get(&node).is_some_and(|r| now < r.silent_until)
    }

    /// Whether the eavesdropper can read a safety frame sent from `pos`: inside a sniffer's
    /// range, and not inside a mix zone (whose frames are encrypted under the zone key).
    #[must_use]
    pub fn eavesdropper_reads(&self, pos: Vec3) -> bool {
        if !self.zones.is_empty() {
            let r2 = self.zone_radius_m * self.zone_radius_m;
            if self.zones.iter().any(|z| {
                let (dx, dy) = (pos.x - z.x, pos.y - z.y);
                dx * dx + dy * dy <= r2
            }) {
                return false;
            }
        }
        match &self.sniffers {
            None => true,
            Some((sites, range)) => {
                let r2 = range * range;
                sites.iter().any(|s| {
                    let (dx, dy) = (pos.x - s.x, pos.y - s.y);
                    dx * dx + dy * dy <= r2
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(rule: Rule, silent: Option<(f64, f64)>) -> PseudonymPolicy {
        PseudonymPolicy {
            rule,
            silent,
            zones: vec![Vec3::new(0.0, 0.0, 0.0)],
            zone_radius_m: DEFAULT_MIX_ZONE_RADIUS_M,
            sniffers: None,
            nodes: BTreeMap::new(),
            silenced_frames: 0,
            requested_changes: 0,
        }
    }

    /// RS_BSP_521–524 in order: 800–1,500 m; then 800 m plus 120–360 s; then 10–20 km.
    #[test]
    fn the_c2c_cc_stages_follow_the_basic_system_profile() {
        let rng = RngRegistry::new(7);
        let n = NodeId::new(3);
        let mut p = policy(Rule::C2cCc, None);
        p.on_spawn(&rng, n, 0, 0.0);
        let far = Vec3::new(5_000.0, 0.0, 0.0);
        assert!(
            !p.change_due(n, secs_to_ns(10.0), 799.0, far),
            "never before 800 m"
        );
        let first = (800..=1_500)
            .find(|m| p.change_due(n, secs_to_ns(60.0), f64::from(*m), far))
            .expect("due somewhere in [800 m, 1500 m]");
        assert!((800..=1_500).contains(&first));
        p.on_changed(&rng, n, secs_to_ns(60.0), f64::from(first));
        // Second: 800 m on, and then a further 120–360 s.
        let at = f64::from(first) + 800.0;
        assert!(!p.change_due(n, secs_to_ns(100.0), at, far));
        assert!(!p.change_due(n, secs_to_ns(100.0 + 119.0), at, far));
        assert!(p.change_due(n, secs_to_ns(100.0 + 361.0), at, far));
        p.on_changed(&rng, n, secs_to_ns(461.0), at);
        // Third: nowhere near before 10 km.
        assert!(!p.change_due(n, secs_to_ns(600.0), at + 9_999.0, far));
        assert!(p.change_due(n, secs_to_ns(600.0), at + 20_001.0, far));
    }

    #[test]
    fn a_silent_period_follows_every_change_and_ends() {
        let rng = RngRegistry::new(7);
        let n = NodeId::new(1);
        let mut p = policy(Rule::Store, Some((3.0, 13.0)));
        p.on_spawn(&rng, n, 0, 0.0);
        assert!(!p.is_silent(n, secs_to_ns(1.0)));
        p.on_changed(&rng, n, secs_to_ns(10.0), 0.0);
        assert!(p.is_silent(n, secs_to_ns(12.9)), "at least 3 s of silence");
        assert!(!p.is_silent(n, secs_to_ns(23.1)), "at most 13 s");
    }

    #[test]
    fn a_mix_zone_changes_only_inside_and_hides_what_is_sent_there() {
        let rng = RngRegistry::new(7);
        let n = NodeId::new(2);
        let mut p = policy(
            Rule::MixZone {
                min_age: secs_to_ns(60.0),
            },
            None,
        );
        p.on_spawn(&rng, n, 0, 0.0);
        let inside = Vec3::new(10.0, 0.0, 0.0);
        let outside = Vec3::new(100.0, 0.0, 0.0);
        assert!(!p.change_due(n, secs_to_ns(30.0), 0.0, inside), "too young");
        assert!(
            !p.change_due(n, secs_to_ns(90.0), 0.0, outside),
            "outside a zone"
        );
        assert!(p.change_due(n, secs_to_ns(91.0), 0.0, inside));
        assert!(
            !p.eavesdropper_reads(inside),
            "encrypted under the zone key"
        );
        assert!(p.eavesdropper_reads(outside));
    }

    #[test]
    fn a_fraction_of_sites_is_spread_evenly_and_exactly() {
        let sites: Vec<Vec3> = (0..20).map(|i| Vec3::new(f64::from(i), 0.0, 0.0)).collect();
        assert_eq!(every_fraction(&sites, 0.25).len(), 5);
        assert_eq!(every_fraction(&sites, 1.0).len(), 20);
        assert!(every_fraction(&sites, 0.0).is_empty());
        let quarter = every_fraction(&sites, 0.25);
        assert_eq!(quarter[0].x, 3.0);
        assert_eq!(quarter[1].x, 7.0);
    }
}
