//! WP-F3, scale and regression on the multi-provider topology (00-gap-analysis §4.2).
//!
//! * **I-S1 on the SCMS deployment.** The Phase 2 path (Manhattan, both providers of
//!   `us-production-2025`, roadside units, the attacker) is run with real cryptography and
//!   with the modelled backend. Every record, every frame and every count must agree
//!   (03-interfaces §6, I-S1): a security outcome that depends on the crypto mode is a
//!   modelling error. The shared digest is then pinned (`tests/regression/`).
//! * **Fleet scale** (`long_` tests, `#[ignore]`d): realistic Manhattan fleets with the
//!   modelled backend, printing the run's cost; the merge step runs them with
//!   `cargo test -p v2xw-engine --test scms_scale -- --ignored long_`. Memory is measured
//!   outside the process (`/usr/bin/time -l`), see docs/design/scms-real/05-scale-and-regression.md.

mod regression;

use std::path::{Path, PathBuf};
use std::time::Instant;

use v2xw_engine::{DigestRecorder, Engine, RunReport, Scenario};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn rooted(mut s: Scenario) -> Scenario {
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root().join(&*path).to_string_lossy().into_owned();
    }
    s
}

/// The Phase 2 path as `tests/phase2.rs` runs it (every third roadside unit), cut to
/// `duration_s`.
fn phase2(duration_s: f64) -> Scenario {
    let mut s = rooted(
        Scenario::load(root().join("scenarios").join("phase2-manhattan.yaml"))
            .expect("the shipped scenario loads"),
    );
    s.actors.rsus = s
        .actors
        .rsus
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 3 == 0)
        .map(|(_, r)| r)
        .collect();
    s.time.duration_s = duration_s;
    s
}

fn crypto(s: &mut Scenario, mode: &str) {
    s.security.crypto_mode = serde_json::from_value(serde_json::json!(mode)).expect("a mode");
}

struct Ran {
    report: RunReport,
    pinned: regression::Pinned,
    wall_s: f64,
}

fn run(s: Scenario) -> Ran {
    let errors = v2xw_engine::scenario::validate::validate(&s);
    assert!(errors.is_empty(), "the test scenario must load: {errors:?}");
    let t0 = Instant::now();
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = DigestRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    Ran {
        report,
        pinned: regression::Pinned {
            record_digest: rec.digest_hex(),
            frame_digest: rec.frame_digest_hex(),
            records: rec.written(),
        },
        wall_s: t0.elapsed().as_secs_f64(),
    }
}

/// The simulated seconds of the I-S1 pair: long enough that every vehicle enrols with its
/// provider, is provisioned, signs, and is verified across providers, and that the attacker
/// is detected and reported; short enough for the default suite in real cryptography.
const I_S1_S: f64 = 60.0;

/// **I-S1 on the new topology.** Real and modelled cryptography give the same run.
#[test]
fn real_and_modelled_crypto_give_the_same_run_on_the_two_provider_deployment() {
    let mut real = phase2(I_S1_S);
    crypto(&mut real, "real");
    let mut modelled = phase2(I_S1_S);
    crypto(&mut modelled, "modeled");
    let (real, modelled) = std::thread::scope(|s| {
        let r = s.spawn(|| run(real));
        let m = run(modelled);
        (r.join().expect("the real run"), m)
    });
    let p = &modelled.report.phase2;
    println!(
        "I-S1 {I_S1_S} s: real {:.1} s, modelled {:.1} s wall; {} records; devices {:?}; \
         verified {:?}; reports {}; receptions {}",
        real.wall_s,
        modelled.wall_s,
        modelled.pinned.records,
        p.devices_by_provider,
        p.verified_by_provider,
        p.reports_sent,
        modelled.report.receptions_ok
    );

    // Not two identical nothings: both providers certify devices, each provider's
    // certificates are verified by receivers, and the misbehaviour path has started.
    assert!(
        p.devices_by_provider.len() >= 2 && p.devices_by_provider.values().all(|n| *n > 0),
        "the run must certify devices under both providers: {:?}",
        p.devices_by_provider
    );
    assert!(
        p.verified_by_provider.len() >= 2 && p.verified_by_provider.values().all(|n| *n > 0),
        "every provider's certificates must be verified on the air: {:?}",
        p.verified_by_provider
    );
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    assert!(
        p.reports_sent > 0,
        "the attacker must be reported within the window"
    );
    assert!(modelled.report.receptions_ok > 0);
    assert_eq!(real.report.credentials_refused, 0);

    // I-S1: the same counts, the same records, the same frames.
    assert_eq!(
        real.report, modelled.report,
        "the run reports differ between modes"
    );
    assert_eq!(
        real.pinned, modelled.pinned,
        "the record or frame streams differ between modes"
    );
    regression::check("i-s1-phase2-60s", &modelled.pinned);
}

/// One fleet-scale run of the revocation-latency scenario at `rate` vehicles an hour.
fn fleet(rate: f64, duration_s: f64) -> Ran {
    let mut s = rooted(
        Scenario::load(root().join("scenarios").join("revocation-latency.yaml"))
            .expect("the shipped scenario loads"),
    );
    s.time.duration_s = duration_s;
    s.actors.vehicles.demand.rate_veh_per_h = Some(rate);
    s.experiment = None;
    run(s)
}

fn env_f64(key: &str, default: f64) -> f64 {
    std::env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// **Fleet scale, modelled cryptography.** The revocation-latency scenario's Manhattan
/// fleet at its three study rates, printing what each run costs. `V2XW_SCALE_RATE` picks
/// one rate (so a run can be measured alone under `/usr/bin/time -l`), `V2XW_SCALE_S` its
/// simulated seconds (default 90: the scenario's attack starts at 60 s).
#[test]
#[ignore = "long: fleet-scale measurement; the merge step runs `--ignored long_`"]
fn long_fleet_scale_run_cost() {
    let duration_s = env_f64("V2XW_SCALE_S", 90.0);
    let rates: Vec<f64> = match std::env::var("V2XW_SCALE_RATE") {
        Ok(v) => vec![v.parse().expect("V2XW_SCALE_RATE is a number")],
        Err(_) => vec![1500.0, 6000.0, 24000.0],
    };
    let mut last: Option<(f64, u64)> = None;
    for rate in rates {
        let r = fleet(rate, duration_s);
        let p = &r.report.phase2;
        println!(
            "SCALE rate={rate} sim_s={duration_s} wall_s={:.1} wall_per_sim_s={:.3} \
             spawned={} nodes={} frames={} attempts={} rx_ok={} devices={:?} topups={} \
             certs={} reports={} at_ma={} revoked_attackers={} revoked_honest={} \
             records={} digest={}",
            r.wall_s,
            r.wall_s / duration_s,
            r.report.actors_spawned,
            r.report.nodes_created,
            r.report.frames_transmitted,
            r.report.reception_attempts,
            r.report.receptions_ok,
            p.devices_by_provider,
            p.topups_completed,
            p.certs_topped_up,
            p.reports_sent,
            p.reports_at_ma,
            p.revoked_attackers,
            p.revoked_honest,
            r.pinned.records,
            r.pinned.record_digest
        );
        // What must hold at any fleet size.
        assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
        assert_eq!(
            p.revoked_honest, 0,
            "an honest device was revoked at {rate} veh/h"
        );
        assert!(
            p.devices_by_provider.len() >= 2,
            "both providers must certify devices at {rate} veh/h"
        );
        if let Some((prev_rate, prev_frames)) = last {
            assert!(
                r.report.frames_transmitted > prev_frames,
                "{rate} veh/h sent no more frames than {prev_rate} veh/h"
            );
        }
        last = Some((rate, r.report.frames_transmitted));
    }
}
