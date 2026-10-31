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
    /// Per unit node: the application certificates its `node.credentials` rows carried
    /// (id8 -> COER hex) and how many rows it had.
    rsu_creds: std::collections::BTreeMap<u64, (std::collections::BTreeMap<String, String>, usize)>,
    /// Every unit row: node, `active`, the grid's (id8, state) cells, `history`.
    rsu_active: Vec<(u64, Option<String>, Vec<(String, String)>, Vec<String>)>,
}

fn outcome() -> &'static Outcome {
    static RUN: OnceLock<Outcome> = OnceLock::new();
    RUN.get_or_init(|| {
        let mut engine = Engine::build(scenario(), "").expect("builds");
        let mut rec = MemoryRecorder::new();
        let report = engine.run(&mut rec).expect("runs");
        let mut rsu_creds = std::collections::BTreeMap::new();
        let mut rsu_active = Vec::new();
        for (_, r) in rec.records() {
            if r.channel != "node.credentials" {
                continue;
            }
            let v: serde_json::Value = serde_json::from_slice(&r.json).expect("json");
            if !v["device"].as_str().is_some_and(|d| d.starts_with("rsu/")) {
                continue;
            }
            let cells = v["cells"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|c| {
                    (
                        c["id8"].as_str().unwrap_or("").to_string(),
                        c["state"].as_str().unwrap_or("").to_string(),
                    )
                })
                .collect();
            let history = v["history"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|h| h.as_str().map(str::to_string))
                .collect();
            rsu_active.push((
                v["node"].as_u64().expect("a node"),
                v["active"].as_str().map(str::to_string),
                cells,
                history,
            ));
            let e = rsu_creds
                .entry(v["node"].as_u64().expect("a node"))
                .or_insert_with(|| (std::collections::BTreeMap::new(), 0));
            e.1 += 1;
            for (k, h) in v["certificates"].as_object().into_iter().flatten() {
                e.0.insert(k.clone(), h.as_str().unwrap_or("").to_string());
            }
        }
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
            rsu_creds,
            rsu_active,
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
            assert_eq!(
                *certs, 1,
                "this week's only; the factory certificate is gone"
            );
        }
    }
}

/// Audit-prov PV.rsu-cert (SC-D13): every unit's application certificates are on the
/// record, with their bytes, in `node.credentials` rows of device `rsu/<node>`: three for
/// a connected unit (weeks 614, 615, 616), one for an unconnected one. Before, no record
/// carried a unit's certificate. Fault: the `rsu_credentials` emission in the engine's
/// backend tick removed (no unit row at all).
#[test]
fn every_units_application_certificates_are_on_the_record() {
    let o = outcome();
    let p2 = &o.report.phase2;
    assert_eq!(
        o.rsu_creds.len(),
        4,
        "a row for each unit: {:?}",
        o.rsu_creds.keys()
    );
    let held: usize = o.rsu_creds.values().map(|(c, _)| c.len()).sum();
    assert_eq!(
        held as u64,
        p2.rsu_app_certs_preloaded + p2.rsu_app_certs_renewed,
        "every certificate the units were issued, once"
    );
    // The `node.credentials` contract a vehicle's rows keep: an `active` certificate is
    // marked `active` in the grid and is in the history.
    for (node, active, cells, history) in &o.rsu_active {
        let Some(a) = active else { continue };
        assert!(
            cells.iter().any(|(id, st)| id == a && st == "active"),
            "unit {node}: the grid marks its active certificate"
        );
        assert!(history.contains(a), "unit {node}: the history holds it");
    }
    assert!(
        o.rsu_active.iter().any(|(_, a, _, _)| a.is_some()),
        "some unit signs with a certificate of this week"
    );
    for (certs, rows) in o.rsu_creds.values() {
        assert!(*rows >= 1);
        for h in certs.values() {
            assert!(h.len() > 100, "the certificate's COER, hex");
        }
    }
}

#[test]
fn a_unit_without_backhaul_starves_when_its_week_lapses_and_signs_nothing() {
    let o = outcome();
    let p = &o.report.phase2;
    for (connected, gaps, _, withheld) in &o.units {
        if *connected {
            assert_eq!(
                *withheld, 0,
                "a unit with a valid certificate withholds nothing"
            );
        } else {
            assert_eq!(gaps, &vec![(LAPSE_S, HORIZON_S as u64)], "the SCMS record");
            assert!(*withheld > 500, "{withheld}");
        }
    }
    assert_eq!(p.rsus_starved, 2, "{p:?}");
    // Stepped from the lapse to the end: 60 s of mobility steps at 10 a second, per unit,
    // at least (a unit also steps at its own timed wakes).
    assert!(p.rsu_starved_steps >= 1_150, "{}", p.rsu_starved_steps);
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

/// Runs one of the RA-track scenarios (`scenarios/scms-fix3/`) to its end.
fn run_fix3(name: &str) -> RunReport {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let s = Scenario::load(root.join("scenarios/scms-fix3").join(name)).expect("loads");
    let mut engine = Engine::build(s, "").expect("builds");
    engine
        .run(&mut v2xw_engine::NullRecorder::new())
        .expect("runs")
}

/// A roadside unit's application certificate permits what its applications sign (USDOT
/// RSU Specification v4.1 p. 15: certificates to support the unit's transmissions, by
/// PSID; IEEE 1609.2 §5.2: a receiver refuses an SPDU whose PSID the signer's
/// `appPermissions` do not list). The four connected-intersections units run SPaT/MAP and
/// answer signal requests with SSM (PSID 0x204095, `psid-traffic-signal-priority-status`).
/// Their SCMS certificate used to carry SPaT/MAP (0x82) only, so every receiver refused
/// every SSM: 299 `psid-not-permitted` on the f4982788 release (scms-fix3-ra.yaml CB).
/// Fault (2026-10-27): `wiring::rsu_app_psids` returning an empty list puts the 299 back.
#[test]
fn a_roadside_units_scms_certificate_permits_the_ssm_it_answers_with() {
    let r = run_fix3("ci-scms.yaml");
    let p = &r.phase2;
    println!(
        "rsus in cycle {}, preloaded {}, refused {:?}, checked {}",
        p.rsus_in_cert_cycle,
        p.rsu_app_certs_preloaded,
        r.receiver_validation.refused,
        r.receiver_validation.checked
    );
    assert_eq!(
        p.rsus_in_cert_cycle, 4,
        "every unit signs with its SCMS certificate"
    );
    assert!(p.rsu_app_certs_preloaded >= 4);
    assert_eq!(
        r.receiver_validation
            .refused
            .get("psid-not-permitted")
            .copied()
            .unwrap_or(0),
        0,
        "{:?}",
        r.receiver_validation.refused
    );
}

/// WP-14's roadside CPMs under the US SCMS: the unit's application certificate lists the
/// CPM's ITS-AID (639, `psid-collective-perception-service`, ETSI TS 103 324) once the
/// `cpm` role is on, so the vehicles around it verify and decode the infrastructure CPMs.
/// Fault (2026-10-27): with SPaT/MAP alone (`rsu_app_psids` empty) no roadside CPM
/// verifies.
#[test]
fn under_the_us_scms_a_roadside_cpm_signed_with_its_application_certificate_verifies() {
    let r = run_fix3("ci-scms-cpm.yaml");
    let c = r.cpm.clone().expect("CPM figures");
    println!("{c:?} refused {:?}", r.receiver_validation.refused);
    assert_eq!(r.phase2.rsus_in_cert_cycle, 4);
    assert!(c.rsu_cpms_sent > 10, "the units send CPMs: {c:?}");
    assert!(
        c.rsu_cpms_verified > 10 && c.rsu_objects_decoded > 10,
        "vehicles verify and decode them: {c:?}"
    );
}
