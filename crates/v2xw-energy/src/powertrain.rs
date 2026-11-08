//! What drives a vehicle: its market segment, its fuel and its emission standard — and how
//! a run's fleet is drawn.
//!
//! The mobility classes (`passenger`, `truck`, `bus`, … — SUMO's vClass names, which is
//! what `gt.kinematics` carries) say how a vehicle drives. They do not say what it burns:
//! a passenger car may be a Euro 4 petrol car, a Euro 6 diesel or a battery-electric car,
//! and its emissions differ by an order of magnitude between those. The [`FleetSpec`] of
//! the scenario's `energy.fleet` section supplies that second axis, per segment, and every
//! vehicle draws its powertrain **once**, from a stream keyed by its actor id
//! ([`assign`]), so the draw is the same on every run, every thread count and every replay
//! of a recording, and does not depend on which other vehicles exist.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use v2xw_core::ids::ActorId;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};

/// The model id the fleet draw is keyed under ([`RngDomain::plugin`]).
pub const FLEET_DRAW_ID: &str = "energy/fleet/draw";

/// A vehicle market segment, in the sense HBEFA, PHEM and MOVES all classify by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Segment {
    /// Passenger car (HBEFA "PC", MOVES source type 21).
    PassengerCar,
    /// Light commercial vehicle / delivery van (HBEFA "LDV"/"LCV", MOVES 31–32).
    LightCommercial,
    /// Heavy-duty truck, with or without trailer (HBEFA "HDV", MOVES 52–62).
    HeavyTruck,
    /// Urban bus (HBEFA "Bus", MOVES 42).
    UrbanBus,
    /// Coach (HBEFA "Coach", MOVES 41).
    Coach,
    /// Motorcycle or moped (HBEFA "MC", MOVES 11).
    Motorcycle,
}

impl Segment {
    /// Every segment, in declaration order.
    pub const ALL: [Segment; 6] = [
        Segment::PassengerCar,
        Segment::LightCommercial,
        Segment::HeavyTruck,
        Segment::UrbanBus,
        Segment::Coach,
        Segment::Motorcycle,
    ];

    /// The segment a mobility class drives as, or `None` for a road user with no
    /// combustion engine or traction battery of the kind these models describe
    /// (pedestrian, bicycle, e-scooter).
    ///
    /// `emergency` is a passenger car (SUMO's vType defaults give it a passenger car's
    /// body); `trailer` is a heavy truck; `moped` is a motorcycle. An unknown class name is
    /// `None`, and the provider counts it as unmodelled rather than guessing.
    #[must_use]
    pub fn of_vehicle_class(class: &str) -> Option<Segment> {
        Some(match class {
            "passenger" | "emergency" | "car" | "taxi" | "evehicle" | "private" => {
                Segment::PassengerCar
            }
            "delivery" => Segment::LightCommercial,
            "truck" | "trailer" => Segment::HeavyTruck,
            "bus" => Segment::UrbanBus,
            "coach" => Segment::Coach,
            "motorcycle" | "moped" => Segment::Motorcycle,
            _ => return None,
        })
    }

    /// The segment's name as the scenario spells it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Segment::PassengerCar => "passenger-car",
            Segment::LightCommercial => "light-commercial",
            Segment::HeavyTruck => "heavy-truck",
            Segment::UrbanBus => "urban-bus",
            Segment::Coach => "coach",
            Segment::Motorcycle => "motorcycle",
        }
    }
}

/// What a vehicle burns or stores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Fuel {
    /// Spark-ignition, petrol.
    Gasoline,
    /// Compression-ignition, diesel.
    Diesel,
    /// Battery-electric: no tailpipe, a traction battery.
    Electric,
}

/// One vehicle's powertrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Powertrain {
    /// Its segment.
    pub segment: Segment,
    /// Its fuel.
    pub fuel: Fuel,
    /// Its Euro emission standard, `0..=6` (ignored for an electric vehicle).
    pub euro: u8,
}

impl Powertrain {
    /// True for a battery-electric vehicle.
    #[must_use]
    pub const fn is_electric(&self) -> bool {
        matches!(self.fuel, Fuel::Electric)
    }

    /// A short label: `passenger-car/gasoline/euro4`, `urban-bus/electric`.
    #[must_use]
    pub fn label(&self) -> String {
        let fuel = match self.fuel {
            Fuel::Gasoline => "gasoline",
            Fuel::Diesel => "diesel",
            Fuel::Electric => return format!("{}/electric", self.segment.as_str()),
        };
        format!("{}/{fuel}/euro{}", self.segment.as_str(), self.euro)
    }
}

/// One segment's share of each fuel, and its emission standard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SegmentMix {
    /// The share of battery-electric vehicles, `0..=1`.
    #[serde(default)]
    pub ev_share: f64,
    /// The share of diesel vehicles among the combustion-engined ones, `0..=1`.
    #[serde(default)]
    pub diesel_share: f64,
    /// The Euro emission standard of the combustion-engined ones, `0..=6`.
    #[serde(default = "SegmentMix::default_euro")]
    pub euro: u8,
}

impl SegmentMix {
    /// Euro 4: the class SUMO falls back to for an unknown or `default` emission class
    /// (`HelpersHBEFA3`'s `default` alias is `PC_G_EU4`), and the one class every model in
    /// this crate has for a passenger car, so the default fleet means the same thing under
    /// each of them.
    const fn default_euro() -> u8 {
        4
    }
}

impl Default for SegmentMix {
    fn default() -> Self {
        Self {
            ev_share: 0.0,
            diesel_share: 0.0,
            euro: Self::default_euro(),
        }
    }
}

/// The fleet: a mix per segment, keyed by [`Segment::as_str`] spelling. A segment the map
/// does not name gets [`SegmentMix::default`] — all petrol, Euro 4, no electric vehicles —
/// except heavy trucks, urban buses and coaches, whose default is diesel (HBEFA 3 has no
/// petrol bus or coach class, and its one petrol truck class is an aggregate).
///
/// A key that is not a segment name is a load error (the map's key type is [`Segment`]).
pub type FleetSpec = BTreeMap<Segment, SegmentMix>;

/// What a [`FleetSpec`] answers.
pub trait FleetExt {
    /// The mix a segment draws from.
    fn mix(&self, segment: Segment) -> SegmentMix;

    /// Checks every share is a fraction and every Euro class exists.
    ///
    /// # Errors
    /// A message naming the segment and the field.
    fn check(&self) -> Result<(), String>;
}

impl FleetExt for FleetSpec {
    fn mix(&self, segment: Segment) -> SegmentMix {
        self.get(&segment).cloned().unwrap_or_else(|| {
            let diesel = matches!(
                segment,
                Segment::HeavyTruck | Segment::UrbanBus | Segment::Coach
            );
            SegmentMix {
                diesel_share: if diesel { 1.0 } else { 0.0 },
                ..SegmentMix::default()
            }
        })
    }

    fn check(&self) -> Result<(), String> {
        for (seg, m) in self {
            for (name, v) in [("ev_share", m.ev_share), ("diesel_share", m.diesel_share)] {
                if !(0.0..=1.0).contains(&v) || !v.is_finite() {
                    return Err(format!(
                        "energy.fleet.{}.{name} must be in [0, 1], got {v}",
                        seg.as_str()
                    ));
                }
            }
            if m.euro > 6 {
                return Err(format!(
                    "energy.fleet.{}.euro must be 0..=6, got {}",
                    seg.as_str(),
                    m.euro
                ));
            }
        }
        Ok(())
    }
}

/// Draws one vehicle's powertrain.
///
/// Two uniforms from the stream `(plugin("energy/fleet/draw"), Actor(actor))`: the first
/// decides electric against combustion, the second diesel against petrol. Both are drawn
/// whatever the first decides, so changing one share never shifts the other draw.
#[must_use]
pub fn assign(seed: u64, actor: ActorId, segment: Segment, fleet: &FleetSpec) -> Powertrain {
    let mix = fleet.mix(segment);
    let mut rng = RngStream::derive(
        seed,
        RngDomain::plugin(FLEET_DRAW_ID),
        EntityRef::Actor(actor),
    );
    let u_ev = rng.f64();
    let u_diesel = rng.f64();
    let fuel = if u_ev < mix.ev_share {
        Fuel::Electric
    } else if u_diesel < mix.diesel_share {
        Fuel::Diesel
    } else {
        Fuel::Gasoline
    };
    Powertrain {
        segment,
        fuel,
        euro: mix.euro.min(6),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_draw_is_keyed_by_actor_and_honours_the_shares() {
        let mut fleet = FleetSpec::default();
        fleet.insert(
            Segment::PassengerCar,
            SegmentMix {
                ev_share: 0.25,
                diesel_share: 0.5,
                euro: 6,
            },
        );
        let draws: Vec<Powertrain> = (0..4000)
            .map(|i| assign(7, ActorId::new(i), Segment::PassengerCar, &fleet))
            .collect();
        let ev = draws.iter().filter(|p| p.is_electric()).count() as f64 / 4000.0;
        let diesel = draws.iter().filter(|p| p.fuel == Fuel::Diesel).count() as f64 / 4000.0;
        assert!((ev - 0.25).abs() < 0.03, "ev share {ev}");
        assert!((diesel - 0.375).abs() < 0.03, "diesel share {diesel}");
        // Keyed: the same actor draws the same powertrain, whatever was drawn before.
        assert_eq!(
            assign(7, ActorId::new(17), Segment::PassengerCar, &fleet),
            draws[17]
        );
    }

    #[test]
    fn heavy_segments_default_to_diesel_and_the_spec_round_trips() {
        let f = FleetSpec::default();
        assert_eq!(f.mix(Segment::UrbanBus).diesel_share, 1.0);
        assert_eq!(f.mix(Segment::PassengerCar).diesel_share, 0.0);
        let yaml = serde_json::json!({"passenger-car": {"ev_share": 0.1}});
        let back: FleetSpec = serde_json::from_value(yaml).expect("parses");
        assert_eq!(back.mix(Segment::PassengerCar).ev_share, 0.1);
        assert_eq!(back.mix(Segment::PassengerCar).euro, 4);
        assert!(back.check().is_ok());
        assert_eq!(
            Segment::of_vehicle_class("emergency"),
            Some(Segment::PassengerCar)
        );
        assert_eq!(Segment::of_vehicle_class("pedestrian"), None);
    }
}
