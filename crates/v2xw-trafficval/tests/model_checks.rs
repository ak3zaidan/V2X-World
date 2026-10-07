//! The suite's cheap checks as a test: every model-level check that runs in seconds must
//! pass as shipped and be proved able to fail by its fault. The simulation-backed checks
//! are measurements run by `traffic_validation`, not part of `cargo test`.

use v2xw_trafficval::run::{Verdict, run_check};
use v2xw_trafficval::{Config, Lab, checks};

const FAST: [&str; 6] = [
    "model/idm-equation",
    "model/idm-stops-at-s0",
    "model/string-stability",
    "model/mobil-criterion",
    "model/hcm-critical-gaps",
    "model/social-force-parameters",
];

#[test]
fn the_model_level_checks_pass_and_each_can_fail() {
    let mut lab = Lab::new(Config::new(true));
    let mut problems = Vec::new();
    for c in checks().iter().filter(|c| FAST.contains(&c.id)) {
        let r = run_check(&mut lab, c);
        if let Err(e) = &r.shipped {
            problems.push(format!("{}: could not run: {e}", r.id));
            continue;
        }
        for (row, v) in r.verdicts() {
            match v {
                Verdict::Fail => problems.push(format!("{}: {} = {} outside {:?}", r.id, row.metric, row.measured, row.band)),
                Verdict::NotProved if row.metric.contains("differ") || row.metric.contains("|engine") => {
                    problems.push(format!("{}: {} never went red under a fault", r.id, row.metric))
                }
                _ => {}
            }
        }
    }
    assert!(problems.is_empty(), "{problems:#?}");
    assert_eq!(FAST.len(), checks().iter().filter(|c| FAST.contains(&c.id)).count());
}
