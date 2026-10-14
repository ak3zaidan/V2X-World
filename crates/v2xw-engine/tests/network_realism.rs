//! Network-realism settings (round 16): Doppler fading coherence changes a run and is
//! refused where no fading is drawn; the HSM signing lower edge is refused at or above
//! the profile's bound. (An explicit 5 000 ms DENM repetition duration holding on the air
//! is `run::gn_air::repetition_tests`: on gn-grid the vehicle stands for under 5 s before
//! its hazards go out, so 5 s and 15 s send the same DENMs there.)

use std::path::{Path, PathBuf};

use v2xw_engine::scenario::validate::validate;
use v2xw_engine::sensitivity::{self as sens, Spec};

fn spec() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/sensitivity/test-networking.yaml")
}

fn refused(s: &v2xw_engine::scenario::Scenario, path: &str) -> bool {
    validate(s).iter().any(|e| e.to_string().contains(path))
}

#[test]
fn the_new_settings_refuse_what_would_do_nothing() {
    let (_, base) = Spec::load(&spec()).expect("loads");
    assert!(validate(&base).is_empty());

    let mut s = base.clone();
    s.radio.channel_model.fading_coherence = Some("doppler".into());
    assert!(!refused(&s, "fading_coherence"), "{:?}", validate(&s));
    s.radio.channel_model.fading = Some("none".into());
    assert!(refused(&s, "fading_coherence"));

    let mut s = base.clone();
    s.nodes.hsm_sign_lower_us = Some(3_000.0);
    assert!(!refused(&s, "hsm_sign_lower_us"), "{:?}", validate(&s));
    s.nodes.hsm_sign_lower_us = Some(9_000.0);
    assert!(refused(&s, "hsm_sign_lower_us"));
}

#[test]
fn doppler_coherence_changes_the_run() {
    let (_, base) = Spec::load(&spec()).expect("loads");
    let b = sens::measure(base.clone()).expect("runs").values;

    let mut doppler = base.clone();
    doppler.radio.channel_model.fading_coherence = Some("doppler".into());
    let d = sens::measure(doppler).expect("runs").values;
    let moved = b.iter().filter(|(k, v)| d.get(*k) != Some(v)).count();
    assert!(moved > 0, "doppler coherence changed no metric");
}
