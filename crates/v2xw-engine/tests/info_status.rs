//! WP-35 (A16): the CRL poll through the RA's information status
//! (`security.scms.crl_poll: info-status`, IEEE 1609.2.1 §6.3.5.14).
//!
//! The conformance scenario's first window, with a 60 s poll so every vehicle polls about
//! seven times, and its SCMS Manager root rollover at 120 s: the home CTL changes, so the
//! RA's status changes once and every vehicle holds an older list after it. Same seed,
//! both modes:
//!
//! * the vehicles download the composite CRL exactly as often (only what is newer);
//! * each vehicle receives the signed status about once (after the change), and every
//!   other poll is a 412, one per poll at most;
//! * the downlink carries the signed statuses on top.

use std::path::Path;

use v2xw_engine::{Engine, NullRecorder, RunReport, Scenario};

fn run(poll: &str) -> RunReport {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/scms-conformance.yaml")).expect("loads");
    s.time.duration_s = 420.0;
    s.time.fast_forward.clear();
    if let Some(p) = s.security.protocol.as_mut() {
        p.params["crl_fetch_interval_s"] = serde_json::json!(60.0);
    }
    s.security
        .scms
        .get_or_insert_with(Default::default)
        .crl_poll = Some(poll.to_string());
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::build(s, "").expect("builds");
    engine.run(&mut NullRecorder::new()).expect("runs")
}

/// fault: the status the vehicle received not kept (every poll answered 200) -> red.
/// fault: the composite exchange made on every status poll, current or not -> red.
#[test]
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
    assert!(
        i.info_status_polls > 2 * i.access.cellular_vehicles,
        "too few polls to judge"
    );
    assert!(
        c.crl_downloads > 0,
        "the CTL change at 120 s makes every copy old"
    );
    assert_eq!(
        i.crl_downloads, c.crl_downloads,
        "the composite is fetched exactly when the vehicle holds an older list"
    );
    assert!(
        i.info_status_sent > 0 && i.info_status_sent <= 2 * i.access.cellular_vehicles,
        "{} signed statuses for {} vehicles: the status changed once",
        i.info_status_sent,
        i.access.cellular_vehicles
    );
    assert!(
        i.crl_polls_not_modified <= i.info_status_polls,
        "{} 412s for {} polls: a current vehicle makes one exchange",
        i.crl_polls_not_modified,
        i.info_status_polls
    );
    assert!(
        i.access.uu_dl_bytes > c.access.uu_dl_bytes,
        "the signed statuses are on the downlink"
    );
}
