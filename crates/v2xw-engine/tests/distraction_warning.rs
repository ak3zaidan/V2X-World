//! Driver distraction (WP-39, `actors.vehicles.drivers.distraction`) through the whole
//! engine: the expectation row (more low-TTC conflicts) and the V2X benefit (a forward
//! collision or brake-light warning from the vehicle's own unit brings a driver's eyes
//! back, so FCW and EEBL on give fewer conflicts than off).

use std::path::{Path, PathBuf};

use v2xw_engine::Scenario;
use v2xw_engine::scenario::schema::DriverRealism;
use v2xw_engine::sensitivity::{expect, sweep};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The `traffic` family: the signalised Midtown-shaped grid, 1,500 veh/h for 90 s, every
/// vehicle equipped and every application on (`phase1-grid.yaml`).
fn traffic() -> Scenario {
    let path = root().join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    fam.bases(&path)
        .expect("bases load")
        .remove("traffic")
        .expect("the traffic family")
}

#[test]
fn the_distraction_row_holds() {
    let path = root().join("scenarios/sensitivity/catalogue.yaml");
    let fam = sweep::Families::load(&path).expect("families file loads");
    let mut exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    exp.retain(|e| e.setting == "actors.vehicles.drivers.distraction");
    assert_eq!(exp.len(), 1, "the distraction row");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "{unplanned:?}");
    let bases = fam.bases(&path).expect("bases load");
    let result =
        sweep::run_cached(settings, vec![], &bases, &exp, 1, None, |_, _, _| {}).expect("runs");
    for v in &result.verdicts {
        eprintln!(
            "{} {}: {:?} -> {:?} ({})",
            v.expectation.setting, v.expectation.metric, v.base, v.value, v.note
        );
    }
    assert_eq!(result.verdicts.len(), 1);
    assert!(result.verdicts.iter().all(|v| v.pass));
}

#[test]
fn a_warning_brings_a_distracted_drivers_eyes_back() {
    // The Midtown grid, every vehicle equipped, human drivers who glance away. FCW and
    // EEBL on against off: the warnings are the only difference, and they reach the
    // driving only through the driver's eyes.
    let run = |warnings: bool| {
        let mut s = traffic();
        s.actors.vehicles.drivers.distraction = Some(true);
        s.actors.vehicles.drivers.realism = DriverRealism::Human;
        if !warnings {
            s.apps.enabled.retain(|a| a != "fcw" && a != "eebl");
        }
        sweep::fingerprint(s).expect("runs")
    };
    let on = run(true);
    let off = run(false);
    let conflicts = |f: &sweep::Fingerprint| f.get("metric.ttc_conflicts").copied().unwrap_or(0.0);
    let fcw: f64 = on
        .iter()
        .filter(|(k, _)| k.contains("app_warnings") && (k.contains("fcw") || k.contains("eebl")))
        .map(|(_, v)| *v)
        .sum();
    eprintln!(
        "TTC < 1.5 s conflicts: warnings on {}, off {}; FCW+EEBL warnings {fcw}",
        conflicts(&on),
        conflicts(&off)
    );
    for (k, v) in off.iter().filter(|(k, _)| {
        ["ttc", "drac", "pet", "app_warnings"]
            .iter()
            .any(|m| k.contains(m))
    }) {
        eprintln!("  off: {k} = {v}; on: {:?}", on.get(k));
    }
    assert!(fcw > 0.0, "no FCW or EEBL warning was issued");
    assert!(conflicts(&off) > 0.0, "no conflict to cut");
    assert!(
        conflicts(&on) < conflicts(&off),
        "warnings on {} against off {}",
        conflicts(&on),
        conflicts(&off)
    );
}
