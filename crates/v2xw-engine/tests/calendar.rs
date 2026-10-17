//! WP-D2: the real calendar, the `time.calendar: compressed` lint and label, the
//! deployment's CRL publication rule, and the fast-forward mode (docs/design/scms-real/00
//! §1.4 rows 43-44, §4.2 WP-D2).
//!
//! # Faults injected to prove these checks can fail
//!
//! * `validate::calendar` without its `(Real, true)` arm — an unflagged compressed
//!   calendar validates and `an_unflagged_compressed_calendar_fails_validation` fails.
//! * `manifest::calendar_warnings` returning nothing — the label test fails.
//! * `apply_crl_publication` without the `SlaOneBusinessDay` arm —
//!   `the_us_presets_publish_the_crl_on_the_business_day_sla` fails.
//! * `Phase2::next_wake` without the top-up horizon (or without the kernel's next
//!   delivery, or without the re-enrolment lead) — the fast-forward stops a minute late
//!   and `fast_forward_lands_every_backend_event_on_the_step_a_full_run_does` fails on
//!   the backend message trace.
//! * `Phase2::next_wake` offering a kernel delivery only strictly after `now` (the first
//!   version: a top-up injected at the tick's own instant waited for the minute cap) —
//!   the same test fails on the top-up's install time.
//! * `Engine::fast_forward_tick` stopping one step after the due instant — the same test
//!   fails. (Rounding the stop *down* is not a fault: the backend has nothing due yet at
//!   that stop, and the next one lands on the same step a full run uses.)

use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use serde_json::json;
use v2xw_engine::scenario::{Calendar, DilationWindow, ModelChoice};
use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_world::model::LaneKind;

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios")
}

/// `phase1-grid.yaml` cut to 4 × 8 junctions with exactly `n` vehicles departing at
/// t = 0, crawling at `speed_mps` so none reaches its destination inside the run (a
/// vehicle that arrives is retired, and a fleet that changes is not "the same events").
fn crawling_fleet(n: usize, seconds: f64, speed_mps: f64) -> Scenario {
    let mut s = Scenario::load(scenarios().join("phase1-grid.yaml")).expect("loads");
    s.time.duration_s = seconds;
    if let v2xw_world::WorldSourceSpec::Procedural { params, .. } = &mut s.world.source {
        params["cols"] = 4.into();
        params["rows"] = 8.into();
        params["speed_limit_mps"] = speed_mps.into();
    }
    s.world.buildings.enabled = false;
    let world = v2xw_engine::wiring::build_world(&s).expect("world");
    let roads = &world.roads;
    let driving: Vec<_> = roads
        .lanes()
        .iter()
        .filter(|l| l.kind == LaneKind::Driving && l.junction.is_none() && l.length_m > 40.0)
        .collect();
    let mid = |l: &v2xw_world::model::Lane| l.centreline[l.centreline.len() / 2];
    let dist = |a: &v2xw_world::model::Lane, b: &v2xw_world::model::Lane| {
        (mid(a).x - mid(b).x).hypot(mid(a).y - mid(b).y)
    };
    let trips: Vec<_> = driving
        .iter()
        .take(n)
        .enumerate()
        .map(|(i, o)| {
            let mut seen = BTreeSet::from([o.id]);
            let mut queue = VecDeque::from([o.id]);
            while let Some(l) = queue.pop_front() {
                for c in roads.successors(l) {
                    for next in [c.via, Some(c.to_lane)].into_iter().flatten() {
                        if seen.insert(next) {
                            queue.push_back(next);
                        }
                    }
                }
            }
            let dest = driving
                .iter()
                .filter(|l| {
                    l.id != o.id && seen.contains(&l.id) && !roads.successors(l.id).is_empty()
                })
                .max_by(|a, b| dist(a, o).total_cmp(&dist(b, o)).then(b.id.cmp(&a.id)))
                .expect("a reachable destination");
            json!({"id": i, "depart_s": 0.0, "origin": o.id, "destination": dest.id})
        })
        .collect();
    let set = json!({
        "schema": "v2xw/trips/1",
        "world_hash": v2xw_world::hash::content_hash_hex(&world),
        "source": {"test": "calendar"},
        "trips": trips,
    });
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("calendar-{n}.json"));
    std::fs::write(&path, set.to_string()).expect("writes the trip file");
    s.actors.vehicles.demand.kind = "mobility/demand/trips".into();
    s.actors.vehicles.demand.rate_veh_per_h = None;
    s.actors.vehicles.demand.params = json!({ "path": path });
    s.net.uu = Some(ModelChoice {
        id: "cellular/uu/fixed-latency".to_string(),
        params: json!({"preset": "4g-east-coast"}),
    });
    s
}

fn with_scms(mut s: Scenario, params: serde_json::Value) -> Scenario {
    s.actors.backend.protocol = Some(v2xw_engine::phase2::CAMP_SCMS.to_string());
    s.security.protocol = Some(ModelChoice {
        id: v2xw_engine::phase2::CAMP_SCMS.to_string(),
        params,
    });
    s
}

fn errors_at(s: &Scenario) -> Vec<String> {
    v2xw_engine::scenario::validate::validate(s)
        .iter()
        .map(|e| format!("{e}"))
        .collect()
}

/// Gap row 43's lint: a shorter i-period without `time.calendar: compressed` is refused,
/// with it it loads, and the flag on the real week is refused too.
#[test]
fn an_unflagged_compressed_calendar_fails_validation() {
    let s = with_scms(
        crawling_fleet(2, 10.0, 5.0),
        json!({"i_period_s": 60, "cert_lifetime_s": 61}),
    );
    let e = errors_at(&s);
    assert!(
        e.iter()
            .any(|m| m.contains("security.protocol.params.i_period_s") && m.contains("compressed")),
        "an unflagged 60 s i-period validated: {e:?}"
    );
    let mut flagged = s.clone();
    flagged.time.calendar = Calendar::Compressed;
    assert!(errors_at(&flagged).is_empty(), "{:?}", errors_at(&flagged));
    assert!(
        Engine::build(s, "").is_err(),
        "the engine built an unflagged compressed run"
    );

    let mut real = with_scms(crawling_fleet(2, 10.0, 5.0), json!({}));
    assert!(errors_at(&real).is_empty(), "{:?}", errors_at(&real));
    real.time.calendar = Calendar::Compressed;
    assert!(
        errors_at(&real).iter().any(|m| m.contains("time.calendar")),
        "compressed on the real week validated"
    );
}

/// Every output of a compressed run says so: the manifest's warnings and the run report;
/// a real-calendar run says `real` and carries no compressed label.
#[test]
fn a_compressed_calendar_is_labelled_in_the_manifest_and_the_report() {
    let mut c = with_scms(
        crawling_fleet(1, 2.0, 5.0),
        json!({"i_period_s": 60, "cert_lifetime_s": 61}),
    );
    c.time.calendar = Calendar::Compressed;
    let mut engine = Engine::build(c, "").expect("builds");
    assert!(
        engine
            .manifest()
            .warnings
            .iter()
            .any(|w| w.starts_with("calendar: compressed") && w.contains("i-period is 60 s")),
        "{:?}",
        engine.manifest().warnings
    );
    let r = engine.run(&mut MemoryRecorder::new()).expect("runs");
    assert_eq!(r.calendar, "compressed");

    let mut engine =
        Engine::build(with_scms(crawling_fleet(1, 2.0, 5.0), json!({})), "").expect("builds");
    assert!(
        !engine
            .manifest()
            .warnings
            .iter()
            .any(|w| w.contains("calendar: compressed")),
        "a real-calendar run is labelled compressed"
    );
    let r = engine.run(&mut MemoryRecorder::new()).expect("runs");
    assert_eq!(r.calendar, "real");
}

/// The deployment's CRL publication rule reaches the CRL Generator on the real calendar:
/// the US presets publish within one business day of a change (SM-PROV §3.4.9) at the
/// provider's slot in Eastern time; `camp-reference`, an explicit `crl_cadence_s` and a
/// compressed calendar keep the cadence.
#[test]
fn the_us_presets_publish_the_crl_on_the_business_day_sla() {
    let params = |s: Scenario| {
        let engine = Engine::build(s, "").expect("builds");
        let p = engine.phase2().expect("a backend").scms().state.params;
        (p.crl.business_day_sla, p.publish_on_cadence)
    };
    let base = || crawling_fleet(1, 1.0, 5.0);
    assert_eq!(
        params(with_scms(base(), json!({}))),
        (true, true),
        "us-production-2025"
    );
    let mut camp = with_scms(base(), json!({}));
    camp.security.scms = Some(
        serde_json::from_value(json!({"deployment": {"preset": "camp-reference"}}))
            .expect("a deployment"),
    );
    assert_eq!(
        params(camp).0,
        false,
        "camp-reference keeps the daily cadence"
    );
    assert_eq!(
        params(with_scms(base(), json!({"crl_cadence_s": 3600}))).0,
        false,
        "an explicit cadence is the scenario's choice"
    );
    let mut c = with_scms(base(), json!({"i_period_s": 60, "cert_lifetime_s": 61}));
    c.time.calendar = Calendar::Compressed;
    assert_eq!(
        params(c).0,
        false,
        "a compressed calendar has no business day"
    );
}

/// The lifecycle the equivalence runs exercise on the real weekly grid: the run starts
/// five minutes before the i-period boundary of Tuesday 2026-10-20 09:00 UTC, so every
/// vehicle's pool (i = 614 and 615) reaches its top-up horizon inside the run; the RA's
/// request shuffle is cut to 30 s and enrolments to half an hour (renewed ten minutes
/// before the end) so that the shuffle release and the successor enrolments also fall
/// inside a run the full stepping can afford. The i-period is the real week.
fn lifecycle_run(window: (f64, f64), horizon: f64, fast_forward: bool) -> (RunReport, Trace) {
    let mut s = with_scms(
        crawling_fleet(3, horizon, 0.3),
        json!({
            "cert_shuffle_window_s": 30,
            "first_batch_delay_s": 2,
            "download_poll_interval_s": 5,
            "enrolment_lifetime_s": 1800,
            "reenrol_lead_s": 600,
            "reenrol_download_delay_s": 5,
            "crl_fetch_interval_s": 120,
        }),
    );
    s.time.t0 = "2026-10-20T08:55:00Z".into();
    let w = DilationWindow {
        from_s: window.0,
        to_s: window.1,
    };
    if fast_forward {
        s.time.fast_forward = vec![w];
    } else {
        s.time.time_dilation = vec![w];
    }
    assert!(errors_at(&s).is_empty(), "{:?}", errors_at(&s));
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    let p2 = engine.phase2().expect("a backend");
    let steps: Vec<String> = p2
        .scms()
        .kernel
        .steps
        .iter()
        .map(|s| {
            format!(
                "{} {}->{} {} {} {}",
                s.t,
                s.from.index(),
                s.to.index(),
                s.step,
                s.bytes,
                s.run.0
            )
        })
        .collect();
    let mut creds = Vec::new();
    for node in 0..64u32 {
        let c = p2.creds(v2xw_core::ids::NodeId::new(node));
        if !c.is_empty() {
            creds.push(format!("{node}: {c:?}"));
        }
    }
    let certs: Vec<String> = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "sec.cert")
        .map(|(t, r)| format!("{t} {}", String::from_utf8_lossy(&r.json)))
        // A pseudonym change is the vehicle's own step, which a fast-forward does not
        // simulate (traffic and nodes frozen); every other credential event is the
        // backend's.
        .filter(|l| !l.contains("\"event\":\"change\""))
        .collect();
    (
        report,
        Trace {
            steps,
            creds,
            certs,
        },
    )
}

#[derive(Debug, PartialEq)]
struct Trace {
    steps: Vec<String>,
    creds: Vec<String>,
    certs: Vec<String>,
}

/// Gap row 44's equivalence: the backend after a fast-forward window is the backend after
/// the same window stepped in full (radio off in both, so the only difference is the
/// skipped mobility and node steps): the same backend messages at the same instants, the
/// same certificates in every vehicle's pool, the same credential events.
#[test]
fn fast_forward_lands_every_backend_event_on_the_step_a_full_run_does() {
    let (window, horizon) = ((20.0, 1_500.0), 1_520.0);
    let (full, full_trace) = lifecycle_run(window, horizon, false);
    let (ff, ff_trace) = lifecycle_run(window, horizon, true);
    let (a, b) = (&full.phase2, &ff.phase2);
    println!(
        "full: {} steps, top-ups {}/{} ({} certs), renewals {}/{}, {} backend messages; \
         fast-forward: {} steps, {} backend ticks, {} skipped",
        full.mobility_steps,
        a.topups_completed,
        a.topups_started,
        a.certs_topped_up,
        a.reenrolments_completed,
        a.reenrolments_started,
        full_trace.steps.len(),
        ff.mobility_steps,
        ff.fast_forward.backend_ticks,
        ff.fast_forward.skipped_steps,
    );
    // The window exercised what it is meant to: top-ups at the i-period boundary through
    // the shuffle, and successor enrolments.
    assert!(a.topups_completed >= 3, "no top-up in the window: {a:?}");
    assert!(
        a.reenrolments_completed > 0,
        "no re-enrolment in the window"
    );
    assert_eq!(a.backend_errors, 0, "{}", a.first_backend_error);
    // The fast-forward skipped the stepping.
    assert_eq!(ff.fast_forward.windows, 1);
    assert!(
        ff.mobility_steps * 10 < full.mobility_steps,
        "fast-forward stepped {} of {} mobility steps",
        ff.mobility_steps,
        full.mobility_steps
    );
    assert!(ff.fast_forward.skipped_steps > 13_000);
    // ... and the backend did exactly the same.
    for (name, x, y) in [
        ("topups_started", a.topups_started, b.topups_started),
        ("topups_completed", a.topups_completed, b.topups_completed),
        ("certs_topped_up", a.certs_topped_up, b.certs_topped_up),
        ("topups_refused", a.topups_refused, b.topups_refused),
        (
            "reenrolments_started",
            a.reenrolments_started,
            b.reenrolments_started,
        ),
        (
            "reenrolments_completed",
            a.reenrolments_completed,
            b.reenrolments_completed,
        ),
        (
            "crl_versions_published",
            a.crl_versions_published,
            b.crl_versions_published,
        ),
        ("backend_errors", a.backend_errors, b.backend_errors),
    ] {
        assert_eq!(x, y, "{name}: full {x}, fast-forward {y}");
    }
    assert_eq!(
        full_trace.steps.len(),
        ff_trace.steps.len(),
        "backend message count"
    );
    for (x, y) in full_trace.steps.iter().zip(&ff_trace.steps) {
        assert_eq!(x, y, "backend message trace diverges");
    }
    assert_eq!(full_trace.creds, ff_trace.creds, "certificate pools differ");
    assert_eq!(full_trace.certs, ff_trace.certs, "credential events differ");
}

/// Multi-week PKI behaviour on the real calendar in a short run: three weeks fast-
/// forwarded with every lifecycle parameter at its cited default (the CAMP 1.2.2 shuffle
/// of 1,000 requests or two days, the one-week i-period with its hour of overlap, the
/// six-year enrolment): each vehicle tops up once a week, and the pools reach i = 617.
#[test]
fn three_real_weeks_fast_forward_in_a_short_run() {
    let week = 604_800.0;
    let mut s = with_scms(crawling_fleet(2, 3.0 * week + 30.0, 0.3), json!({}));
    s.time.t0 = "2026-10-16T12:00:00Z".into();
    s.time.fast_forward = vec![DilationWindow {
        from_s: 10.0,
        to_s: 3.0 * week + 20.0,
    }];
    assert!(errors_at(&s).is_empty(), "{:?}", errors_at(&s));
    let mut engine = Engine::build(s, "").expect("builds");
    let report = engine.run(&mut MemoryRecorder::new()).expect("runs");
    let p = &report.phase2;
    let ff = &report.fast_forward;
    println!(
        "{} mobility steps, {} backend ticks, {} skipped; top-ups {}/{} ({} certs)",
        report.mobility_steps,
        ff.backend_ticks,
        ff.skipped_steps,
        p.topups_completed,
        p.topups_started,
        p.certs_topped_up
    );
    assert_eq!(report.calendar, "real");
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    assert!(
        report.mobility_steps < 1_000,
        "the traffic was stepped through the weeks"
    );
    assert!(ff.skipped_steps > 18_000_000);
    let p2 = engine.phase2().expect("a backend");
    let mut last = Vec::new();
    for node in 0..64u32 {
        let c = p2.creds(v2xw_core::ids::NodeId::new(node));
        if let Some(max) = c.iter().map(|c| c.i).max() {
            last.push(max);
        }
    }
    assert_eq!(last.len(), 2, "two vehicles hold certificates");
    // Provisioned with 614 and 615 on Friday 2026-10-16; the Tuesdays 10-20, 10-27 and
    // 11-03 each start a period with one left, so each pool is topped up three times.
    for max in &last {
        assert_eq!(
            *max, 618,
            "a pool's last i-period after three weeks: {last:?}"
        );
    }
    assert!(p.topups_completed >= 6, "{p:?}");
}

/// The scenario migration (WP-D2): the shipped compressed-calendar files declare it and
/// validate, and `credential-weeks.yaml` runs the same lifecycle on the real calendar,
/// with every compressed value removed and the weeks fast-forwarded.
#[test]
fn the_shipped_scenarios_declare_their_calendar() {
    for name in [
        "credential-lifecycle.yaml",
        "ccms-lifecycle.yaml",
        "pq-signatures.yaml",
        "threshold-revocation-2of2.yaml",
    ] {
        let s = Scenario::load(scenarios().join(name)).expect("loads");
        assert_eq!(s.time.calendar, Calendar::Compressed, "{name}");
        assert!(errors_at(&s).is_empty(), "{name}: {:?}", errors_at(&s));
    }
    let s = Scenario::load(scenarios().join("credential-weeks.yaml")).expect("loads");
    assert!(errors_at(&s).is_empty(), "{:?}", errors_at(&s));
    assert_eq!(s.time.calendar, Calendar::Real);
    assert_eq!(s.time.fast_forward.len(), 2);
    let params = &s.security.protocol.as_ref().expect("a protocol").params;
    assert!(
        params.as_object().is_none_or(serde_json::Map::is_empty),
        "a compressed value survived: {params}"
    );
}
