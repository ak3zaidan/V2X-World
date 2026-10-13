//! The expectation table as a test target (`v2xw sensitivity --expect` runs the same).
//!
//! Fast (default): every interaction check builds and validates its four scenarios; the
//! judging goes red on a wrong expectation; one cheap interaction runs end to end.
//! Full (`cargo test -p v2xw-engine --test sensitivity_expectations -- --ignored`, about
//! 7 minutes on two threads): every row of `tests/expectations/*.yaml` and every check in
//! `tests/expectations/interactions/*.yaml` holds.

use std::path::{Path, PathBuf};

use v2xw_engine::sensitivity::expect::{self, Interaction, Interplay};
use v2xw_engine::sensitivity::{Direction, apply, sweep};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn families() -> (PathBuf, sweep::Families) {
    let p = root().join("scenarios/sensitivity/catalogue.yaml");
    let f = sweep::Families::load(&p).expect("families file loads");
    (p, f)
}

fn interactions() -> Vec<Interaction> {
    expect::load_interactions(&[root().join("crates/v2xw-engine/tests/expectations/interactions")])
        .expect("interactions parse")
}

#[test]
fn every_interaction_builds_four_valid_scenarios() {
    let (path, fam) = families();
    let bases = fam.bases(&path).expect("bases load");
    let all = interactions();
    assert!(all.len() >= 10, "{} interaction checks", all.len());
    let mut bad = Vec::new();
    for i in &all {
        let Some(base) = bases.get(&i.family) else {
            bad.push(format!("{}: no family {}", i.name, i.family));
            continue;
        };
        let mut b = i.base_set.clone();
        let mut a = b.clone();
        a.extend(i.factor.clone());
        let base_only = b.clone();
        b.extend(i.condition.clone());
        let mut ab = b.clone();
        ab.extend(i.factor.clone());
        for set in [base_only, a, b, ab] {
            if let Err(e) = apply(base, &set).and_then(|s| s.validate().map_err(Into::into)) {
                bad.push(format!("{}: {set:?}: {e}", i.name));
            }
        }
    }
    assert!(bad.is_empty(), "interactions the loader refuses: {bad:#?}");
}

#[test]
fn every_row_is_planned() {
    let (_, fam) = families();
    let exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    assert!(exp.len() >= 120, "{} rows", exp.len());
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    assert!(unplanned.is_empty(), "rows the sweep cannot plan: {unplanned:?}");
    // Every row's end is among the planned values.
    for e in &exp {
        let s = settings.iter().find(|s| s.path == e.setting).expect("planned");
        assert!(
            s.values.iter().any(|(l, v)| match e.change.as_str() {
                Some(end @ ("low" | "high")) => l == end,
                _ => *v == e.change,
            }),
            "{} {} not planned",
            e.setting,
            e.change
        );
    }
}

/// The judging is not a check that cannot fail: a wrong expectation goes red.
#[test]
fn interaction_judging_goes_red_when_wrong() {
    let mut i = interactions().remove(0);
    i.direction = Some(Direction::Up);
    // A rises 0.2 -> 0.5 alone, 0.4 -> 0.5 under B: B dampens it.
    let v = [Some(0.2), Some(0.5), Some(0.4), Some(0.5)];
    i.expect = Interplay::Dampens;
    assert!(expect::judge_interaction(&i, v).pass);
    i.expect = Interplay::Amplifies;
    assert!(!expect::judge_interaction(&i, v).pass);
    i.expect = Interplay::Reverses;
    assert!(!expect::judge_interaction(&i, v).pass);
    i.expect = Interplay::Holds;
    assert!(expect::judge_interaction(&i, v).pass);
    i.direction = Some(Direction::Down);
    assert!(!expect::judge_interaction(&i, v).pass);
    // Nothing moved: no effect to compare, never a pass.
    i.direction = None;
    let flat = [Some(0.2), Some(0.2), Some(0.4), Some(0.5)];
    assert!(!expect::judge_interaction(&i, flat).pass);
}

/// One cheap interaction end to end (four 2 s runs of 40 vehicles).
#[test]
fn data_rate_saves_more_airtime_on_longer_frames() {
    let (path, fam) = families();
    let bases = fam.bases(&path).expect("bases load");
    let all: Vec<Interaction> = interactions()
        .into_iter()
        .filter(|i| i.name.starts_with("data rate x frame size"))
        .collect();
    assert_eq!(all.len(), 1);
    let v = expect::run_interactions(&all, &bases, 2, None, |_, _, _| {}).expect("runs");
    assert!(v[0].pass, "{}: {:?} {}", v[0].interaction.name, v[0].values, v[0].note);
}

/// The whole table: every single-setting row and every interaction.
#[test]
#[ignore = "about 7 minutes on two threads; run with --ignored, or `v2xw sensitivity --expect`"]
fn the_full_table_holds() {
    let (path, fam) = families();
    let exp = sweep::load_expectations(&[root().join("crates/v2xw-engine/tests/expectations")])
        .expect("rows parse");
    let (settings, unplanned) = expect::expect_plan(&fam, &exp);
    let bases = fam.bases(&path).expect("bases load");
    let cache = std::env::var_os("V2XW_EXPECT_CACHE").map(PathBuf::from);
    let result = sweep::run_cached(settings, vec![], &bases, &exp, 2, cache.as_deref(), |_, _, _| {})
        .expect("runs");
    let inter = expect::run_interactions(&interactions(), &bases, 2, cache.as_deref(), |_, _, _| {})
        .expect("runs");
    let report = expect::report(&result, &inter, &unplanned);
    let failed = result.verdicts.iter().filter(|v| !v.pass).count()
        + inter.iter().filter(|v| !v.pass).count();
    assert!(failed == 0 && unplanned.is_empty(), "{report}");
}
