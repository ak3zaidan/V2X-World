//! The catalogue sweep's fast CI subset (`v2xw sensitivity --catalogue` runs all of it;
//! docs/validation/sensitivity-catalogue.md is the full result).
//!
//! - every expectation names a setting the sweep plans, at an end it plans (no run);
//! - a handful of link-budget, load and frame settings on the DSRC family move the
//!   metrics the expectation table says, and none of them is dead.

use std::path::{Path, PathBuf};

use v2xw_engine::sensitivity::sweep::{self, Families};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn families() -> (PathBuf, Families) {
    let p = root().join("scenarios/sensitivity/catalogue.yaml");
    let f = Families::load(&p).expect("families file loads");
    (p, f)
}

fn expectations() -> Vec<sweep::Expectation> {
    let dir = root().join("crates/v2xw-engine/tests/expectations");
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("expectations dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "yaml"))
        .collect();
    files.sort();
    sweep::load_expectations(&files).expect("expectations parse")
}

#[test]
fn every_expectation_names_a_planned_setting_and_end() {
    let (_, fam) = families();
    let exp = expectations();
    assert!(exp.len() >= 30, "the seed table has {} rows", exp.len());
    let (settings, _) = sweep::plan(&fam, &exp);
    let mut missing = Vec::new();
    for e in &exp {
        let Some(s) = settings.iter().find(|s| s.path == e.setting) else {
            missing.push(format!("{} (not planned)", e.setting));
            continue;
        };
        let found = s.values.iter().any(|(l, v)| match e.change.as_str() {
            Some(end @ ("low" | "high")) => l == end,
            _ => *v == e.change,
        });
        if !found {
            missing.push(format!("{} {} (no such end)", e.setting, e.change));
        }
    }
    assert!(missing.is_empty(), "expectations the sweep cannot judge: {missing:?}");
}

#[test]
fn the_plan_covers_the_catalogue() {
    let (_, fam) = families();
    let (settings, skipped) = sweep::plan(&fam, &[]);
    // Hundreds of settings: a plan that shrank would sweep less than it claims.
    assert!(settings.len() >= 150, "{} settings planned", settings.len());
    assert!(settings.iter().all(|s| !s.values.is_empty()));
    assert!(skipped.iter().all(|(_, why)| !why.is_empty()));
}

/// The fast subset: settings whose effect on the DSRC family is fixed by first principles.
const FAST: &[&str] = &[
    "radio.devices.obu.tx_power_dbm",
    "radio.phy.noise_figure_db",
    "radio.phy.data_rate_mbps",
    "messages.rate_hz",
];

#[test]
fn the_fast_subset_moves_as_expected() {
    let (path, fam) = families();
    let exp: Vec<_> = expectations()
        .into_iter()
        .filter(|e| FAST.contains(&e.setting.as_str()))
        .collect();
    assert!(exp.len() >= 6, "{} fast expectations", exp.len());
    let (mut settings, _) = sweep::plan(&fam, &exp);
    settings.retain(|s| FAST.contains(&s.path.as_str()));
    for s in &mut settings {
        s.families = vec!["dsrc".into()];
    }
    assert_eq!(settings.len(), FAST.len());
    let bases = fam.bases(&path).expect("bases load");
    let out = sweep::run(settings, vec![], &bases, &exp, 2, |_, _, _| {}).expect("runs");
    let dead: Vec<_> = out.dead().iter().map(|s| s.path.clone()).collect();
    assert!(dead.is_empty(), "dead: {dead:?}");
    let bad: Vec<String> = out
        .contradictions()
        .iter()
        .map(|v| format!("{} {} {}: {}", v.expectation.setting, v.expectation.change, v.expectation.metric, v.note))
        .collect();
    assert!(bad.is_empty(), "contradicted: {bad:#?}");
}
