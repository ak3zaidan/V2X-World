//! The rules of the road that belong to a jurisdiction rather than to a junction.
//!
//! A world's `highway=*` class-default preset (`v2xw_world::osm::HighwayPreset`) already
//! names the jurisdiction its speed limits come from; the same name chooses the driving
//! rules here, so a Manhattan world never gets another city's rules by accident.
//!
//! | Preset | Right turn on red | Source |
//! |---|---|---|
//! | `urban-us-nyc` | prohibited unless signed | New York VTL §1111(d)(2): no turn on red in a city of one million or more unless a sign permits it (**secondary**, as NYC DOT states the rule) |
//! | `urban-us-portland` | permitted after a full stop | ORS 811.360(1)(a): a right turn against a steady red after stopping, unless a sign prohibits it |
//! | `sumo-german`, `urban-de` | prohibited unless signed | StVO §37(2): only with the green-arrow sign (Grünpfeil) |
//! | `urban-us` | permitted after a full stop | Uniform Vehicle Code §11-202(c)3 |
//! | none, or unknown | prohibited | the conservative default, and the legacy engine's behaviour |
//!
//! `urban-us` is the generic rule for a US city outside New York; the world crate has no
//! preset of that name, so it is only reachable from code. A procedural world (no preset)
//! keeps the New York rule.

use serde::Serialize;

/// The jurisdiction's rules the mobility engine applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct TrafficRules {
    /// Whether a right turn may be made against a steady red after a full stop
    /// ([`crate::intersection::signal_fixed_time::SignalPlanParams::right_turn_on_red`]).
    pub right_turn_on_red: bool,
}

impl TrafficRules {
    /// New York City's rules, which are also the default.
    pub const NEW_YORK_CITY: TrafficRules = TrafficRules {
        right_turn_on_red: false,
    };

    /// The rules of the jurisdiction a highway preset names, by its label; the default
    /// (New York City's) for `None` or a label this table does not know.
    pub fn of_highway_preset(label: Option<&str>) -> TrafficRules {
        match label {
            Some("urban-us") => TrafficRules {
                right_turn_on_red: true,
            },
            Some("urban-us-portland") => TrafficRules {
                right_turn_on_red: true,
            },
            Some("sumo-german") | Some("urban-de") => TrafficRules {
                right_turn_on_red: false,
            },
            _ => TrafficRules::NEW_YORK_CITY,
        }
    }

    /// `params` with these rules in force.
    pub fn apply(self, params: crate::EngineParams) -> crate::EngineParams {
        crate::EngineParams {
            right_turn_on_red: self.right_turn_on_red,
            ..params
        }
    }
}

impl Default for TrafficRules {
    fn default() -> Self {
        TrafficRules::NEW_YORK_CITY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_york_prohibits_a_right_turn_on_red_and_a_generic_us_city_permits_it() {
        assert!(!TrafficRules::of_highway_preset(Some("urban-us-nyc")).right_turn_on_red);
        assert!(!TrafficRules::of_highway_preset(None).right_turn_on_red);
        assert!(!TrafficRules::of_highway_preset(Some("sumo-german")).right_turn_on_red);
        assert!(!TrafficRules::of_highway_preset(Some("urban-de")).right_turn_on_red);
        assert!(TrafficRules::of_highway_preset(Some("urban-us-portland")).right_turn_on_red);
        assert!(TrafficRules::of_highway_preset(Some("urban-us")).right_turn_on_red);
        let p = TrafficRules::of_highway_preset(Some("urban-us")).apply(Default::default());
        assert!(p.right_turn_on_red);
    }
}
