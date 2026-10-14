//! Networking-track wiring checks (docs/validation/test-networking.md): the on-air DENM
//! honours `net.geonet.repetition_duration_ms`, the per-type air ledger is filled, and
//! keep-alive forwarding on the air is refused.

use std::path::{Path, PathBuf};

use v2xw_engine::sensitivity::{self as sens, Spec};

fn spec() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/sensitivity/test-networking.yaml")
}

#[test]
fn on_air_keep_alive_is_refused() {
    let (_, mut s) = Spec::load(&spec()).expect("loads");
    let g = s.net.geonet.as_mut().expect("gn-grid has net.geonet");
    g.keep_alive_forwarding = true;
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(
        errors
            .iter()
            .any(|e| e.to_string().contains("keep_alive_forwarding")),
        "{errors:?}"
    );
}

#[test]
fn on_air_denm_follows_the_repetition_duration() {
    let (_, base) = Spec::load(&spec()).expect("loads");
    let mut short = base.clone();
    short
        .net
        .geonet
        .as_mut()
        .expect("geonet")
        .repetition_duration_ms = 500;
    let air = |s| {
        sens::measure(s)
            .expect("runs")
            .get("report.air_by_type.denm.airtime_us")
            .expect("DENMs went on the air")
    };
    let (b, v) = (air(base), air(short));
    assert!(
        v < b,
        "a 0.5 s repetition sends fewer DENM copies: {b} -> {v}"
    );
}
