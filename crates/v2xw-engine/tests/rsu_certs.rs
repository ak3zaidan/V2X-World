//! The RSU application-certificate cycle in the engine (WP-B3b wiring, 00 gap row 24).
//!
//! `connected-intersections.yaml` (four SPaT/MAP units) under the SCMS on the real
//! calendar, starting at 08:00 UTC on Tuesday 2026-10-20, one hour before i-period 615
//! begins (the US week starts 2015-01-06 04:00 EST = 09:00 UTC). Two units have a wired
//! backhaul, two have none (`backhaul/none`). The run fast-forwards to 30 s before
//! 10:00 UTC, when the certificate of the week that began on 10-13 (169 h) lapses.
//!
//! * Before the run every unit is enrolled and issued this week's certificate by its
//!   provider's ACA; a connected unit also holds next week's (asked `ra-maxGenDelay`
//!   ahead). The factory certificate is gone.
//! * At 09:00 UTC each connected unit asks for week 616 over its own backhaul line and
//!   installs it: two renewals, carried on the carrier's backhaul lines.
//! * At 10:00 UTC the two units without backhaul hold nothing valid: they starve, sign no
//!   SPaT or MAP, and the dedicated counters say so, matching the SCMS's own record.
//!
//! Faults that turn it red (each injected and reverted, see the package report): the
//! factory certificate kept beside the cycle's, renewals not handed to the node, the
//! unit's own traffic not routed on its line, and starvation read from the wrong clock.

use std::path::Path;
use std::sync::OnceLock;

use serde_json::json;
use v2xw_engine::scenario::{DilationWindow, ModelChoice};
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

const S: u64 = 1_000_000_000;
/// t0 + 2 h: the lapse of the certificate for the week that began 2026-10-13.
const LAPSE_S: u64 = 7_200;
const HORIZON_S: f64 = 7_260.0;

fn scenario() -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/connected-intersections.yaml"))
        .expect("the shipped scenario loads");
    s.time.t0 = "2026-10-20T08:00:00Z".into();
    s.time.duration_s = HORIZON_S;
    s.time.fast_forward = vec![DilationWindow {
        from_s: 20.0,
        to_s: LAPSE_S as f64 - 30.0,
    }];
    s.actors.vehicles.demand.rate_veh_per_h = Some(300.0);
    s.actors.backend.protocol = Some(v2xw_engine::phase2::CAMP_SCMS.to_string());
    s.security.protocol = Some(ModelChoice {
        id: v2xw_engine::phase2::CAMP_SCMS.to_string(),
        params: json!({}),
    });
    for r in &mut s.actors.rsus[2..] {
        r.backhaul = Some("backhaul/none".into());
    }
    s
}

struct Outcome {
    report: RunReport,
    /// Per unit: connected, starvation intervals from the SCMS record (s), certificates
    /// in the node's store, and messages it withheld.
    units: Vec<(bool, Vec<(u64, u64)>, usize, u64)>,
    backhaul_messages: u64,
}

fn outcome() -> &'static Outcome {
    static RUN: OnceLock<Outcome> = OnceLock::new();
    RUN.get_or_init(|| {
        let mut engine = Engine::build(scenario(), "").expect("builds");
        let report = engine.run(&mut MemoryRecorder::new()).expect("runs");
        let p2 = engine.phase2().expect("a backend");
        let horizon = (HORIZON_S as u64) * S;
        let units = p2
            .rsu_nodes()
            .iter()
            .map(|n| {
                let connected = p2.rsu_spec_of(*n).expect("a unit").backhaul.connected;
                let gaps = p2
                    .rsu_starvation(*n, 0, horizon)
                    .into_iter()
                    .map(|(a, b)| (a / S, b / S))
                    .collect();
                let certs = engine.node_cert_count(*n);
                let withheld = engine.node_tx_uncredentialed(*n);
                (connected, gaps, certs, withheld)
            })
            .collect();
        let backhaul_messages = p2.access().report_now().carrier.backhaul_messages;
        Outcome {
            report,
            units,
            backhaul_messages,
        }
    })
}

#[test]
fn every_unit_joins_with_its_providers_certificates_and_renews_over_its_backhaul() {
    let o = outcome();
    let p = &o.report.phase2;
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    assert_eq!(p.rsus_in_cert_cycle, 4);
    // This week's for all four, next week's for the two connected ones.
    assert_eq!(p.rsu_app_certs_preloaded, 6, "{p:?}");
    // Week 616, asked at the start of week 615 (t0 + 1 h) by each connected unit.
    assert_eq!(p.rsu_app_certs_renewed, 2, "{p:?}");
    assert_eq!(p.rsu_app_certs_refused, 0);
    // The request, ack, polls and download rode the units' own backhaul lines.
    assert!(o.backhaul_messages >= 8, "{}", o.backhaul_messages);
    for (connected, gaps, certs, _) in &o.units {
        if *connected {
            assert!(gaps.is_empty(), "a connected unit never starves: {gaps:?}");
            assert_eq!(*certs, 3, "weeks 614, 615 and 616, nothing else");
        } else {
            assert_eq!(*certs, 1, "this week's only; the factory certificate is gone");
        }
    }
}

#[test]
fn a_unit_without_backhaul_starves_when_its_week_lapses_and_signs_nothing() {
    let o = outcome();
    let p = &o.report.phase2;
    for (connected, gaps, _, withheld) in &o.units {
        if *connected {
            assert_eq!(*withheld, 0, "a unit with a valid certificate withholds nothing");
        } else {
            assert_eq!(gaps, &vec![(LAPSE_S, HORIZON_S as u64)], "the SCMS record");
            assert!(*withheld > 500, "{withheld}");
        }
    }
    assert_eq!(p.rsus_starved, 2, "{p:?}");
    // Stepped from the lapse to the end: 60 s of mobility steps at 10 a second, per unit,
    // at least (a unit also steps at its own timed wakes).
    assert!(
        p.rsu_starved_steps >= 1_150,
        "{}",
        p.rsu_starved_steps
    );
    assert!(
        (115 * S..=121 * S).contains(&p.rsu_starved_ns),
        "{}",
        p.rsu_starved_ns
    );
    // SPaT at 10 Hz and MAP at 1 Hz, from two units for about 60 s, all withheld.
    assert!(
        (1_100..=1_400).contains(&p.rsu_messages_withheld),
        "{}",
        p.rsu_messages_withheld
    );
}
