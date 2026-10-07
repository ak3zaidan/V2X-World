//! Group 5: three real cities.
//!
//! Manhattan (`manhattan-5min.yaml`, the D7 Midtown box, `urban-us-nyc`), Portland
//! (`portland-downtown.yaml`, `urban-us-portland`) and Berlin-Mitte (the Portland scenario
//! with Berlin's extract, box and `urban-de`, the recipe of the road-network track's
//! 2026-09-29 measurements) are imported from OpenStreetMap and their traffic run under
//! the auditor. The extracts are not in git: `worlds/fetch.sh <city>` downloads them, and
//! a missing extract makes the check report that it could not run rather than pass.

use v2xw_engine::Scenario;

use crate::invariants::{Engine, Overrides, apply, audit_rows, load, scenario_study, teleport};
use crate::study::Tamper;
use crate::{Check, Group, Lab, Outcome, Row, Variant};

/// The checks of this group.
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "cities/manhattan",
            group: Group::Cities,
            title: "Manhattan imports and its traffic runs with every safety invariant held",
            procedure: "`scenarios/manhattan-5min.yaml` at 1,500 veh/h with 150 pedestrians \
                and its jurisdiction's rules, 180 s (90 s in quick mode).",
            faults: &["one vehicle's published position jumps 30 m for one step at 30 s"],
            run: manhattan,
        },
        Check {
            id: "cities/portland",
            group: Group::Cities,
            title: "Portland imports and its traffic runs with every safety invariant held",
            procedure: "`scenarios/portland-downtown.yaml` at its 1,000 veh/h with 100 \
                pedestrians, Oregon's rules (right on red after a stop), 180 s (90 s in quick \
                mode).",
            faults: &["one vehicle's published position jumps 30 m for one step at 30 s"],
            run: portland,
        },
        Check {
            id: "cities/berlin",
            group: Group::Cities,
            title: "Berlin-Mitte imports and its traffic runs with every safety invariant held",
            procedure: "The Portland scenario with `worlds/cache/berlin.osm.xml`, the box \
                52.5140,13.3880–52.5250,13.4080 and `urban-de` (StVO), 1,500 veh/h, 100 \
                pedestrians, 180 s (90 s in quick mode).",
            faults: &["one vehicle's published position jumps 30 m for one step at 30 s"],
            run: berlin,
        },
    ]
}

fn city(lab: &mut Lab, key: &str, s: Scenario, variant: Variant) -> Result<Outcome, String> {
    let tamper: Option<Tamper> = variant.is_fault().then_some(teleport as Tamper);
    let r = scenario_study(lab, key, &s, Engine::Shipped, tamper)?;
    let (mut rows, notes) = audit_rows(key, &r.audit);
    rows.push(Row::held(
        format!("{key}: trips completed"),
        r.audit.stats.trips_completed as f64,
        (1.0, f64::INFINITY),
        "traffic must flow through the city, not only appear",
    ));
    rows.push(Row::reported(format!("{key}: network mean speed, m/s"), r.audit.stats.mean_speed_mps, "—"));
    Ok(Outcome { rows, notes })
}

fn seconds(lab: &Lab) -> f64 {
    lab.cfg.secs(180, 90) as f64
}

fn manhattan(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let s = load(
        &lab.cfg.root.join("scenarios/manhattan-5min.yaml"),
        &Overrides {
            rate_veh_per_h: Some(1_500.0),
            seconds: Some(seconds(lab)),
            pedestrians: Some(150),
        },
    )?;
    city(lab, "manhattan", s, variant)
}

fn portland(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let s = load(
        &lab.cfg.root.join("scenarios/portland-downtown.yaml"),
        &Overrides {
            rate_veh_per_h: Some(1_000.0),
            seconds: Some(seconds(lab)),
            pedestrians: Some(100),
        },
    )?;
    city(lab, "portland", s, variant)
}

/// The Berlin-Mitte scenario: Portland's with Berlin's extract, box and rules.
///
/// # Errors
/// If the Portland scenario cannot be read or the result does not parse.
pub fn berlin_scenario(lab: &Lab) -> Result<Scenario, String> {
    let path = lab.cfg.root.join("scenarios/portland-downtown.yaml");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut t = text;
    for (from, to) in [
        ("worlds/cache/portland.osm.xml", "worlds/cache/berlin.osm.xml"),
        ("min_lat_deg: 45.5120", "min_lat_deg: 52.5140"),
        ("min_lon_deg: -122.6860", "min_lon_deg: 13.3880"),
        ("max_lat_deg: 45.5250", "max_lat_deg: 52.5250"),
        ("max_lon_deg: -122.6700", "max_lon_deg: 13.4080"),
        ("highway_preset: urban-us-portland", "highway_preset: urban-de"),
    ] {
        if !t.contains(from) {
            return Err(format!("portland-downtown.yaml no longer contains `{from}`"));
        }
        t = t.replace(from, to);
    }
    Scenario::parse(&t, path.parent()).map_err(|e| e.to_string())
}

fn berlin(lab: &mut Lab, variant: Variant) -> Result<Outcome, String> {
    let mut s = berlin_scenario(lab)?;
    apply(
        &mut s,
        &Overrides {
            rate_veh_per_h: Some(1_500.0),
            seconds: Some(seconds(lab)),
            pedestrians: Some(100),
        },
    );
    city(lab, "berlin", s, variant)
}
