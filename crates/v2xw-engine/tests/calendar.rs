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
//! * `Phase2::topups_due` returning its vehicles in a `RandomState` hash order (a
//!   `HashMap` iteration in the backend path) —
//!   `the_lifecycle_run_is_byte_identical_at_one_and_four_threads` fails (WP-DET).
//! * `NativeMobility::resume_after_pause` a no-op, or `Engine::resume_after_fast_forward`
//!   not called — the vehicles standing when the window began are teleported by the
//!   gridlock resolution on the first step after it, and
//!   `the_traffic_resumes_after_a_fast_forward_as_if_no_time_had_passed` fails (R-FFBURST).
//!
//! Inject faults with `tools/mutate/mutate.py`: a revert that restores the file with an
//! older mtime leaves cargo's faulted build in place, and the "restored" tree then fails
//! as if the test were intermittent (WP-DET found this behind the D2 report of a top-up
//! 100 ms late in 2 of 5 runs).

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
    // Origins with no reachable destination (a lane into a dead end of the grid) are
    // skipped, so a larger fleet still gets `n` trips; the first origins, and so every
    // smaller fleet, are as before.
    let trips: Vec<_> = driving
        .iter()
        .filter_map(|o| {
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
                .max_by(|a, b| dist(a, o).total_cmp(&dist(b, o)).then(b.id.cmp(&a.id)))?;
            Some((o.id, dest.id))
        })
        .take(n)
        .enumerate()
        .map(|(i, (o, d))| json!({"id": i, "depart_s": 0.0, "origin": o, "destination": d}))
        .collect();
    assert_eq!(trips.len(), n, "{n} origins with a reachable destination");
    let set = json!({
        "schema": "v2xw/trips/1",
        "world_hash": v2xw_world::hash::content_hash_hex(&world),
        "source": {"test": "calendar"},
        "trips": trips,
    });
    // Named by content: `CARGO_TARGET_TMPDIR` is shared by every test of this binary (two
    // of them want two vehicles on worlds with different speed limits, so different world
    // hashes) and by every worktree building into the same target directory, so a name
    // keyed on `n` alone lets one writer replace the file another run is about to read.
    let body = set.to_string();
    let name = format!(
        "calendar-{n}-{}.json",
        &v2xw_core::hash::sha256_hex(body.as_bytes())[..16]
    );
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    if std::fs::read(&path).ok().as_deref() != Some(body.as_bytes()) {
        // One temporary per writer (tests of this binary run on several threads).
        static WRITER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let k = WRITER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let tmp = path.with_extension(format!("{}-{k}.tmp", std::process::id()));
        std::fs::write(&tmp, &body).expect("writes the trip file");
        std::fs::rename(&tmp, &path).expect("publishes the trip file");
    }
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
    let records = rec
        .records()
        .iter()
        .map(|(t, r)| (*t, r.channel, v2xw_core::hash::sha256(&r.json)))
        .collect();
    (
        report,
        Trace {
            steps,
            creds,
            certs,
            records,
        },
    )
}

#[derive(Debug, PartialEq)]
struct Trace {
    steps: Vec<String>,
    creds: Vec<String>,
    certs: Vec<String>,
    /// Every record of the run, `(t, channel, SHA-256 of its JSON)`, in emission order.
    records: Vec<(u64, &'static str, [u8; 32])>,
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

/// The engine's contract is byte-identical runs: the lifecycle run, stepped in full and
/// fast-forwarded, gives the same record stream (every record's instant, channel and
/// bytes, in emission order) and the same report whether the parallel phases run on one
/// thread or four, and when it is repeated in the same process (a `HashMap` would seed
/// its hasher differently each time; a process-wide cache would be warm the second time).
#[test]
fn the_lifecycle_run_is_byte_identical_at_one_and_four_threads() {
    let (window, horizon) = ((20.0, 1_500.0), 1_520.0);
    let run = |threads: usize, fast_forward: bool| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .expect("a thread pool")
            .install(|| {
                let (report, trace) = lifecycle_run(window, horizon, fast_forward);
                (format!("{report:?}"), trace)
            })
    };
    let same = |what: &str, a: &(String, Trace), b: &(String, Trace)| {
        if let Some(k) = (0..a.1.records.len().max(b.1.records.len()))
            .find(|&k| a.1.records.get(k) != b.1.records.get(k))
        {
            panic!(
                "{what}: record {k} differs: {:?} vs {:?}",
                a.1.records.get(k),
                b.1.records.get(k)
            );
        }
        assert_eq!(a.1, b.1, "{what}: traces differ");
        assert_eq!(a.0, b.0, "{what}: reports differ");
    };
    for fast_forward in [true, false] {
        let one = run(1, fast_forward);
        let four = run(4, fast_forward);
        assert!(
            one.1.records.len() > 1_000,
            "{} records",
            one.1.records.len()
        );
        same(
            &format!("fast_forward {fast_forward}, 1 vs 4 threads"),
            &one,
            &four,
        );
        if fast_forward {
            let again = run(4, fast_forward);
            same("fast-forward, repeated in one process", &four, &again);
        }
        let digest = v2xw_core::hash::sha256_hex(format!("{:?}", one.1.records).as_bytes());
        println!(
            "fast_forward {fast_forward}: {} records, digest {}",
            one.1.records.len(),
            &digest[..16]
        );
    }
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

/// R-FFBURST (docs/design/scms-real/testing/final-run.md §4 item 2): after a fast-forward
/// window the traffic continues exactly where it froze, as if no time had passed on the
/// road. The run with the window [20, 980] s, from 980 s on, is the run with no window from
/// 20 s on, state for state: every vehicle where it was, at its speed, none teleported by
/// the gridlock resolution and none removed. 960 s is 16 cycles of the grid's 60 s fixed-
/// time plan, so the lamps resume in the phase they froze in and the two runs can be
/// compared step for step.
///
/// Before the fix the first step after the window read it as traffic time: a vehicle
/// standing at a red lamp when the window began had "stood" 960 s, past the 300 s gridlock
/// timeout, and was teleported along its route or removed — each teleport a position jump
/// that every honest receiver around it reported (the full-scale benchmark: 1,833 → 1,281
/// nodes at the resume and 4,876 honest reports in the next 10 s).
///
/// Faults that turn it red: `NativeMobility::resume_after_pause` a no-op (vehicles
/// teleported, positions diverge); `Engine::resume_after_fast_forward` not called; its
/// re-dating of the frozen state left out (the first state after the window is filed at
/// the instant the window began).
#[test]
fn the_traffic_resumes_after_a_fast_forward_as_if_no_time_had_passed() {
    use std::collections::BTreeMap;
    use v2xw_metrics::channels::{GtKinematicsView, decode};
    const LIVE: f64 = 30.0;
    const FULL: f64 = 90.0;
    const PAUSE: f64 = 960.0;
    let shift: u64 = 960_000_000_000;
    let run = |window: Option<(f64, f64)>| {
        let horizon = window.map_or(FULL, |(_, to)| to + LIVE);
        let mut s = with_scms(crawling_fleet(8, horizon, 8.0), json!({}));
        s.time.t0 = "2026-10-20T07:30:00Z".into();
        if let Some((from_s, to_s)) = window {
            s.time.fast_forward = vec![DilationWindow { from_s, to_s }];
        }
        assert!(errors_at(&s).is_empty(), "{:?}", errors_at(&s));
        let mut engine = Engine::build(s, "").expect("builds");
        let mut rec = MemoryRecorder::new();
        let report = engine.run(&mut rec).expect("runs");
        let mut states: BTreeMap<(u64, v2xw_core::ids::ActorId), (f64, f64, f64)> = BTreeMap::new();
        let mut jammed: Vec<(u64, u64)> = Vec::new();
        for (at, r) in rec.records() {
            match r.channel {
                "gt.kinematics" => {
                    let k: GtKinematicsView = decode(r).expect("decodes");
                    states.insert((k.t, k.actor), (k.x_m, k.y_m, k.speed_mps));
                }
                "traffic.control" => {
                    let v: serde_json::Value =
                        serde_json::from_slice(&r.json).expect("a traffic.control record");
                    for key in ["jammed", "teleported", "removed"] {
                        jammed.push((*at, v[key].as_u64().unwrap_or(0)));
                    }
                }
                _ => {}
            }
        }
        (report, states, jammed)
    };
    let (_, full, full_jams) = run(None);
    // The window begins on the first whole second from 10 s on at which a vehicle stands
    // (at a red lamp): the state the defect needs.
    let from_ns = (10..=(FULL - LIVE) as u64)
        .map(|s| s * 1_000_000_000)
        .find(|t| full.iter().any(|((t2, _), (_, _, v))| t2 == t && *v < 0.1))
        .expect("a vehicle stands at some whole second of the first minute");
    let from_s = from_ns as f64 * 1e-9;
    let (ff, resumed, ff_jams) = run(Some((from_s, from_s + PAUSE)));
    // The gridlock counters, over the same live span of each run.
    let live_end = from_ns + (LIVE * 1e9) as u64;
    let peak = |j: &[(u64, u64)], until: u64| {
        j.iter()
            .filter(|(t, _)| *t <= until)
            .map(|(_, v)| *v)
            .max()
            .unwrap_or(0)
    };
    let full_jammed = peak(&full_jams, live_end);
    let ff_jammed = peak(&ff_jams, u64::MAX);
    let standing = full
        .iter()
        .filter(|((t, _), (_, _, v))| *t == from_ns && *v < 0.1)
        .count();
    let at_from = full.keys().filter(|(t, _)| *t == from_ns).count();
    println!(
        "{at_from} vehicles at {from_s} s, {standing} standing; paused {} ns; jammed full {full_jammed}, \
         fast-forward {ff_jammed}",
        ff.fast_forward.traffic_paused_ns
    );
    assert!(
        at_from >= 6,
        "{at_from} vehicles on the road when the window began"
    );
    assert!(
        standing > 0,
        "no vehicle was standing when the window began"
    );
    assert_eq!(ff.fast_forward.windows, 1);
    assert_eq!(ff.fast_forward.traffic_paused_ns, shift);
    assert_eq!(
        ff_jammed, full_jammed,
        "the gridlock resolution acted after the window"
    );
    let after: Vec<_> = resumed
        .iter()
        .filter(|((t, _), _)| *t >= from_ns + shift)
        .collect();
    let expected: Vec<_> = full
        .iter()
        .filter(|((t, _), _)| *t >= from_ns && *t <= live_end)
        .collect();
    assert_eq!(
        after.len(),
        expected.len(),
        "states after the window: {} against {} in the run without one",
        after.len(),
        expected.len()
    );
    for (((t, a), (x, y, v)), ((t2, a2), (x2, y2, v2))) in expected.iter().zip(&after) {
        assert_eq!((t + shift, a), (*t2, a2), "the state stream diverges");
        let d = (x - x2).hypot(y - y2);
        assert!(
            d < 1e-6 && (v - v2).abs() < 1e-6,
            "{a:?} at {t} ns: ({x:.3}, {y:.3}) at {v:.3} m/s without the window, \
             ({x2:.3}, {y2:.3}) at {v2:.3} m/s after it ({d:.3} m apart)"
        );
    }
}

/// R-FFWIN (testing/full-bench.md §4): a metric window that reaches a fast-forward is
/// reported where the fast-forward begins, with its own rates; the frozen span is a window
/// of its own, reported where it ends; and the live window after it runs to the next period
/// boundary. Period 10 s, fast-forward [25, 45] s, 60 s: samples at 10, 20, 25, 45, 50
/// and 60 s, `mac.cbr` spans of 10, 10, 5, 20, 5 and 10 s, and the live windows' BSM
/// verification rate is the same per second whatever their length.
///
/// Before the fix the 20-25 s window was flushed only at 50 s, over 30 s of which 20 were
/// frozen: the benchmark's 230-240 s window was reported at 43,170 s with `verify_rate`
/// 115.7/s against 4.85e5/s live. Faults that turn it red: `Engine::next_metric_flush`
/// without the fast-forward start (no sample at 25 s), returning the boundary after the
/// window's end (no sample at 45 s), or `emit_mac_reports` spanning the period.
#[test]
fn a_metric_window_that_reaches_a_fast_forward_is_reported_at_its_own_end() {
    use v2xw_metrics::MetricSample;
    let mut s = with_scms(crawling_fleet(3, 60.0, 0.3), json!({}));
    s.time.t0 = "2026-10-20T08:55:00Z".into();
    s.time.metric_period_s = Some(10.0);
    s.time.fast_forward = vec![DilationWindow {
        from_s: 25.0,
        to_s: 45.0,
    }];
    assert!(errors_at(&s).is_empty(), "{:?}", errors_at(&s));
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    let samples: Vec<MetricSample> = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "metric.sample")
        .map(|(_, r)| serde_json::from_slice(&r.json).expect("a metric sample decodes"))
        .collect();
    let s_ = |t: u64| t * 1_000_000_000;
    let instants: BTreeSet<u64> = samples.iter().map(|m| m.t).collect();
    assert_eq!(
        instants,
        [10, 20, 25, 45, 50, 60].into_iter().map(s_).collect(),
        "metric windows close at"
    );
    let mut spans: Vec<(u64, u64)> = rec
        .records()
        .iter()
        .filter(|(_, r)| r.channel == "mac.cbr")
        .filter_map(|(t, r)| {
            let v: serde_json::Value = serde_json::from_slice(&r.json).ok()?;
            Some((*t, v["span_ns"].as_u64()?))
        })
        .collect();
    spans.sort_unstable();
    spans.dedup();
    assert_eq!(
        spans,
        [(10, 10), (20, 10), (25, 5), (45, 20), (50, 5), (60, 10)]
            .into_iter()
            .map(|(t, d)| (s_(t), s_(d)))
            .collect::<Vec<_>>(),
        "mac.cbr windows (end, span)"
    );
    let rate = |t: u64| {
        samples
            .iter()
            .filter(|m| m.t == s_(t) && m.metric == "verify_rate")
            .filter_map(|m| m.value.point())
            .sum::<f64>()
    };
    let (full, cut, after) = (rate(20), rate(25), rate(50));
    println!("verify_rate: 10-20 s {full:.1}/s, 20-25 s {cut:.1}/s, 45-50 s {after:.1}/s");
    assert!(full > 0.0, "no verification in a live window");
    for (what, r) in [("20-25 s", cut), ("45-50 s", after)] {
        assert!(
            (r / full - 1.0).abs() < 0.35,
            "the {what} window's rate {r:.1}/s is not the live rate {full:.1}/s"
        );
    }
}
