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
//! | Preset | Motorcycle filtering or lane splitting | Source |
//! |---|---|---|
//! | `urban-us-nyc` | prohibited | New York VTL §1252(c): no motorcycle "between lanes of traffic or between adjacent lines or rows of vehicles" |
//! | `urban-us-portland` | prohibited on city streets | ORS 814.240 as amended in 2021 (SB 574) allows it only on highways posted 50 mph or more, with traffic at 10 mph or less |
//! | `sumo-german`, `urban-de` | prohibited | StVO §5 (overtaking on the left, with sufficient lateral clearance); filtering past a queue is not permitted |
//! | `urban-us` | prohibited | Uniform Vehicle Code §11-1103(c), the model for most states |
//! | none, or unknown | prohibited | the New York rule |
//!
//! Where filtering is lawful (California, CVC §21658.1; Utah, between stopped vehicles on
//! roads posted 45 mph or less), a preset would set [`Filtering::BetweenStopped`]; none of
//! the presets this build knows does, so the engine has no filtering manoeuvre and a
//! motorcycle keeps its place in a queue.
//!
//! | Preset | Crossing away from a crosswalk or against the signal | Source |
//! |---|---|---|
//! | `urban-us-nyc` | lawful, without right of way | NYC Council Int. 346-A of 2024 (law without the mayor's signature, autumn 2024): a pedestrian may cross at any point, outside a crosswalk or against the signal, without a summons, and must yield to traffic that has the right of way |
//! | the others | an offence, without right of way | UVC §11-503 and the state codes modelled on it; ORS 814.040; StVO §25(3) |
//!
//! Pedestrians are observed to cross mid-block and against the signal either way; the rule
//! decides whether the run counts those crossings as offences, never whether they happen.
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
    /// Whether a motorcycle may pass between lanes of stopped or slow vehicles.
    pub motorcycle_filtering: Filtering,
    /// Whether a pedestrian crossing away from a crosswalk or against the signal commits
    /// an offence (the run counts those crossings either way).
    pub jaywalking_lawful: bool,
}

/// Whether, and when, a motorcycle may ride between lanes of vehicles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Filtering {
    /// Never.
    Prohibited,
    /// Only between stopped vehicles, at a limited speed.
    BetweenStopped,
}

impl TrafficRules {
    /// New York City's rules, which are also the default.
    pub const NEW_YORK_CITY: TrafficRules = TrafficRules {
        right_turn_on_red: false,
        motorcycle_filtering: Filtering::Prohibited,
        jaywalking_lawful: true,
    };

    /// The rules of the jurisdiction a highway preset names, by its label; the default
    /// (New York City's) for `None` or a label this table does not know.
    pub fn of_highway_preset(label: Option<&str>) -> TrafficRules {
        match label {
            Some("urban-us") => TrafficRules {
                right_turn_on_red: true,
                motorcycle_filtering: Filtering::Prohibited,
                jaywalking_lawful: false,
            },
            Some("urban-us-portland") => TrafficRules {
                right_turn_on_red: true,
                motorcycle_filtering: Filtering::Prohibited,
                jaywalking_lawful: false,
            },
            Some("sumo-german") | Some("urban-de") => TrafficRules {
                right_turn_on_red: false,
                motorcycle_filtering: Filtering::Prohibited,
                jaywalking_lawful: false,
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

    #[test]
    fn no_known_jurisdiction_lets_a_motorcycle_filter_and_new_york_lets_a_pedestrian_cross_anywhere()
    {
        for preset in [
            None,
            Some("urban-us-nyc"),
            Some("urban-us-portland"),
            Some("urban-de"),
            Some("sumo-german"),
            Some("urban-us"),
        ] {
            assert_eq!(
                TrafficRules::of_highway_preset(preset).motorcycle_filtering,
                Filtering::Prohibited,
                "{preset:?}"
            );
        }
        assert!(TrafficRules::of_highway_preset(Some("urban-us-nyc")).jaywalking_lawful);
        assert!(!TrafficRules::of_highway_preset(Some("urban-us-portland")).jaywalking_lawful);
        assert!(!TrafficRules::of_highway_preset(Some("urban-de")).jaywalking_lawful);
    }
}
