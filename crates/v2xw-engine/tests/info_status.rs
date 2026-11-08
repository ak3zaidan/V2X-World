//! The CRL poll through the RA's information status
//! (`security.scms.crl_poll: info-status`, IEEE 1609.2.1 §6.3.5.14).
//!
//! `credential-lifecycle.yaml` as shipped: 300 s on the compressed calendar, a CRL every
//! 30 s, a cellular poll every 30 s, false-position attackers from 60 s, so the MA revokes
//! and the lists change while the vehicles poll. Same seed, both modes:
//!
//! * a quiet poll is one exchange under both (412 for the composite, or for the status);
//! * under `info-status` a vehicle downloads the composite only after a status that names a
//!   newer list (or, its last download lost, a 412 while it still holds an older list than
//!   the status it kept), so no more downloads than signed statuses;
//! * the vehicles fetch about as many composites either way: what changed is when they
//!   learn of it, not what they fetch;
//! * the downlink carries the signed statuses on top.
//!
//! (The first, unverified form ran the conformance scenario's first 420 s, where no list
//! changes: 0 downloads in either mode, so it could not tell the two apart.)

use std::path::Path;

use v2xw_engine::{Engine, NullRecorder, RunReport, Scenario};

fn run(poll: &str) -> RunReport {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/credential-lifecycle.yaml")).expect("loads");
    s.security
        .scms
        .get_or_insert_with(Default::default)
        .crl_poll = Some(poll.to_string());
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    engine.run(&mut NullRecorder::new()).expect("runs")
}

/// fault: the composite fetched on every status poll, current or not
/// (`info_status_exchange` ignoring `changed`) -> red.
#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn an_info_status_poll_fetches_the_composite_only_after_a_change() {
    let cond = run("conditional");
    let info = run("info-status");
    let (c, i) = (&cond.phase2, &info.phase2);
    println!(
        "conditional: 412 {} downloads {} DL {} UL {}; info-status: polls {} sent {} 412 {} \
         downloads {} DL {} UL {}; cellular vehicles {}",
        c.crl_polls_not_modified,
        c.crl_downloads,
        c.access.uu_dl_bytes,
        c.access.uu_ul_bytes,
        i.info_status_polls,
        i.info_status_sent,
        i.crl_polls_not_modified,
        i.crl_downloads,
        i.access.uu_dl_bytes,
        i.access.uu_ul_bytes,
        i.access.cellular_vehicles
    );
    assert_eq!(c.info_status_polls, 0, "conditional asks for no status");
    assert_eq!(c.info_status_sent, 0);
    assert!(c.crl_downloads > 0, "the lists change during the run");
    assert!(
        c.crl_polls_not_modified > 0,
        "and some polls find them current"
    );
    assert!(
        i.info_status_polls > 2 * i.access.cellular_vehicles,
        "too few polls to judge"
    );
    assert!(
        i.crl_polls_not_modified > 0,
        "a vehicle holding the RA's status is answered 412"
    );
    assert!(i.info_status_sent > 0, "a changed status is sent signed");
    assert!(
        i.info_status_sent + i.crl_polls_not_modified <= i.info_status_polls,
        "{} sent + {} 412 for {} polls: one answer a poll",
        i.info_status_sent,
        i.crl_polls_not_modified,
        i.info_status_polls
    );
    assert!(
        i.crl_downloads <= i.info_status_sent,
        "{} downloads for {} signed statuses: a vehicle fetches the composite only after a \
         status naming a newer list",
        i.crl_downloads,
        i.info_status_sent
    );
    let ratio = i.crl_downloads as f64 / c.crl_downloads as f64;
    assert!(
        (0.8..=1.25).contains(&ratio),
        "{} vs {} downloads: the same lists are fetched either way",
        i.crl_downloads,
        c.crl_downloads
    );
    assert!(
        i.access.uu_dl_bytes > c.access.uu_dl_bytes,
        "the signed statuses are on the downlink"
    );
}
