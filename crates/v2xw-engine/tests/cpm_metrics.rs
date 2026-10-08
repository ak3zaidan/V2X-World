//! Collective perception's figures: with CPMs, an equipped vehicle knows about road users
//! that send nothing themselves (unequipped cars, pedestrians without a device), so the
//! awareness with CPMs exceeds the awareness from awareness messages alone; and every
//! object is reported to a receiver at least once per second it is reported at all.

use std::path::Path;

use v2xw_engine::{Engine, MemoryRecorder, Scenario};

fn scenario(sets: &[&str]) -> Scenario {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/connected-intersections.yaml");
    let mut s = Scenario::load(path).expect("the shipped scenario loads");
    s.time.duration_s = 12.0;
    s.metrics = vec![];
    s.events.clear();
    s.net.layer = "gn-btp".to_string();
    s.security.envelope = "etsi103097".to_string();
    s.messages.sets = sets.iter().map(|m| (*m).to_string()).collect();
    s.messages.codec_tier = "uper".to_string();
    for rsu in &mut s.actors.rsus {
        rsu.roles.clear();
    }
    s.actors.vehicles.equipped_fraction = 0.4;
    s.actors.vru.pedestrians = 40;
    s.actors.vru.device_fraction = 0.0;
    s
}

#[test]
fn cpms_raise_object_awareness_beyond_what_awareness_messages_give() {
    let mut e = Engine::build(scenario(&["cam"]), "").expect("builds");
    let rep = e.run(&mut MemoryRecorder::new()).expect("runs");
    assert!(rep.cpm.is_none(), "no CPM, no CPM figures");

    let mut e = Engine::build(scenario(&["cam", "cpm"]), "").expect("builds");
    let rep = e.run(&mut MemoryRecorder::new()).expect("runs");
    let c = rep.cpm.expect("CPM figures");
    eprintln!("{c:?}");
    assert!(c.cpms_sent > 10 && c.senders > 1, "{c:?}");
    assert!(c.objects_sent > 0, "CPMs carry perceived objects: {c:?}");
    assert!(
        c.size_p95_bytes >= c.size_p50_bytes && c.size_p50_bytes > 0,
        "{c:?}"
    );
    // At most 10 Hz per station (T_GenCpmMin = 100 ms), and at least once a second.
    assert!(
        c.generation_mhz <= 10_000 && c.generation_mhz >= 500,
        "{c:?}"
    );
    assert!(c.awareness_pairs > 100, "{c:?}");
    assert!(
        c.awareness_with_cpm_permille > c.awareness_direct_permille,
        "CPMs report road users that send nothing: {c:?}"
    );
    assert!(c.redundancy_milli >= 1_000, "{c:?}");
}
