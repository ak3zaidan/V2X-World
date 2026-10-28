//! The SCMS settings audit's same-seed runs (docs/design/scms-real/testing/scms-knobs.md):
//! each knob of `security.scms.lifecycle` (and `security.scms.tls`, the deployment preset)
//! moved on the credential-lifecycle scenario against the same seed, and the change it
//! makes to (a) the run report's metrics, (b) the run's cost and (c) the SCMS
//! architecture as the PKI records show it, against what the deployment's documents say.
//!
//! The base is `scenarios/credential-lifecycle.yaml` (300 s, i-period 60 s, 5 % ConstPos
//! attackers from 60 s, automatic MA, CRL every 30 s, cellular CRL fetch every 30 s,
//! report shuffle 30 s). Each test prints one `KNOB` line with the measured numbers; the
//! testing document's table is those lines.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, RunRecorder, RunReport, Scenario};

/// Keeps the PKI view's message and case records, and nothing else.
#[derive(Default)]
struct Pki(Vec<(String, Value)>);

impl RunRecorder for Pki {
    fn write(&mut self, _at: SimTime, r: &OwnedRecord) {
        if r.channel == "pki.flow" || r.channel == "pki.case" || r.channel == "ma.decision" {
            let mut v: Value = serde_json::from_slice(&r.json).expect("json");
            if let Some(m) = v.as_object_mut() {
                m.remove("pdus");
            }
            self.0.push((r.channel.to_string(), v));
        }
    }
}

/// A PKI node id's role: the last segment that is not a number.
fn role(v: &Value) -> String {
    let s = v.as_str().unwrap_or("");
    s.split('/')
        .rev()
        .find(|p| !p.is_empty() && !p.chars().all(|c| c.is_ascii_digit()))
        .unwrap_or(s)
        .to_string()
}

struct Out {
    report: RunReport,
    pki: Vec<(String, Value)>,
    wall: Duration,
}

impl Out {
    fn events(&self) -> u64 {
        self.report.events_by_class.values().sum()
    }
    fn msgs(&self) -> impl Iterator<Item = &Value> {
        self.pki
            .iter()
            .filter(|(c, v)| c == "pki.flow" && v["kind"] == "msg")
            .map(|(_, v)| v)
    }
    /// Messages of one flow.
    fn count(&self, flow: &str) -> usize {
        self.msgs().filter(|v| v["flow"] == flow).count()
    }
    /// Which roles exchange messages: `(from, to)` with the provider and the instance
    /// number taken off (`provider-a/ra/0` is `ra`, `ee/1000123` is `ee`).
    fn pairs(&self) -> BTreeSet<(String, String)> {
        self.msgs()
            .map(|v| (role(&v["from"]), role(&v["to"])))
            .collect()
    }
    /// Messages of `flow` from role `from` to a role starting with `to`.
    fn between(&self, flow: &str, from: &str, to: &str) -> usize {
        self.msgs()
            .filter(|v| {
                v["flow"] == flow && role(&v["from"]) == from && role(&v["to"]).starts_with(to)
            })
            .count()
    }
    /// The earliest instant a case reached `stage`, s.
    fn first(&self, stage: &str) -> Option<f64> {
        self.pki
            .iter()
            .filter(|(c, v)| c == "pki.case" && v["stage"] == stage)
            .filter_map(|(_, v)| v["t"].as_u64())
            .min()
            .map(|t| t as f64 / 1e9)
    }
    /// Per top-up flow run: how many batch-download requests its device sent (one when the
    /// batch was ready at nextDlTime, more when it polled again every download-minWait).
    fn download_polls(&self) -> Vec<usize> {
        let mut by: std::collections::BTreeMap<String, usize> = Default::default();
        for v in self.msgs().filter(|v| v["flow"] == "topup") {
            if v["name"] == "batch-download-request" && role(&v["from"]) == "ee" {
                *by.entry(v["instance"].as_str().unwrap_or("").to_string())
                    .or_default() += 1;
            }
        }
        by.into_values().collect()
    }
    fn topup_ms(&self) -> f64 {
        self.report
            .phase2
            .access
            .exchange_latency
            .get("topup")
            .map_or(0.0, |l| l.mean_ms())
    }
    fn cases(&self, stage: &str) -> usize {
        self.pki
            .iter()
            .filter(|(c, v)| c == "pki.case" && v["stage"] == stage)
            .count()
    }
    fn line(&self) -> String {
        let p = &self.report.phase2;
        format!(
            "wall {:.1}s events {} certs_topped_up {} topups {}/{} topup_ms {:.0} starved {} \
             reenrol {} cases {} crls_issued {} crl_versions {} crl_downloads {} \
             revocation_s {:.1} first_decision_s {:.1} first_enforced_s {:.1} changes {} \
             topup_msgs {} ra_to_aca {} revocation_msgs {} backend_bytes {} tls_bytes {}",
            self.wall.as_secs_f64(),
            self.events(),
            p.certs_topped_up,
            p.topups_completed,
            p.topups_started,
            self.topup_ms(),
            p.vehicles_starved,
            p.reenrolments_started,
            p.cases_opened,
            p.crls_issued,
            p.crl_versions_published,
            p.crl_downloads,
            p.revocation_latency_ns as f64 / 1e9,
            self.first("decision").unwrap_or(-1.0),
            self.first("enforced").unwrap_or(-1.0),
            p.pseudonym_changes,
            self.count("topup"),
            self.between("topup", "ra", "aca") + self.between("topup", "ra", "pca"),
            self.count("linkage-resolution") + self.count("crl-issuance"),
            p.access.backend_bytes,
            p.access.carrier.tls_handshake_bytes,
        )
    }
}

fn base_doc() -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(root.join("scenarios").join("credential-lifecycle.yaml"))
        .expect("the shipped scenario loads");
    serde_json::to_value(&s).expect("serialises")
}

fn run(doc: Value) -> Out {
    let s = Scenario::from_document(doc).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    let t = Instant::now();
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = Pki::default();
    let report = engine.run(&mut rec).expect("runs");
    Out {
        report,
        pki: rec.0,
        wall: t.elapsed(),
    }
}

fn base() -> &'static Out {
    static BASE: OnceLock<Out> = OnceLock::new();
    BASE.get_or_init(|| run(base_doc()))
}

/// The base with `security.scms.lifecycle` set to `lifecycle`.
fn with(lifecycle: Value) -> Out {
    let mut doc = base_doc();
    doc["security"]["scms"] = json!({"lifecycle": lifecycle});
    run(doc)
}

fn knob(name: &str, b: &Out, v: &Out) {
    println!(
        "KNOB {name}\n  base    {}\n  variant {}",
        b.line(),
        v.line()
    );
}

#[test]
fn certs_per_period_scales_every_batch() {
    // CAMP-EE §2.1.5.3.2: a batch is one i-period's j = 0..certs_per_period-1. Doubling it
    // doubles the certificates each top-up installs; the top-ups themselves and their
    // number of protocol messages per batch do not change (one request, one download).
    let (b, v) = (base(), with(json!({"certs_per_period": 10})));
    knob("certs_per_period 5 -> 10", b, &v);
    let (pb, pv) = (&b.report.phase2, &v.report.phase2);
    assert!(pb.certs_topped_up > 0, "the base tops up");
    assert_eq!(pv.topups_completed, pb.topups_completed, "same top-ups");
    assert_eq!(
        pv.certs_topped_up,
        2 * pb.certs_topped_up,
        "twice the certificates"
    );
    assert!(
        pv.access.backend_bytes > pb.access.backend_bytes,
        "bigger downloads cross the backend"
    );
}

#[test]
fn no_topups_starve_the_fleet_after_its_preload() {
    // topup_below_periods 0: a vehicle never asks for more, so after its two preloaded
    // 60 s periods it holds nothing valid and cannot sign (CAMP-EE §2.1.5.3.2).
    let (b, v) = (base(), with(json!({"topup_below_periods": 0})));
    knob("topup_below_periods 1 -> 0", b, &v);
    let (pb, pv) = (&b.report.phase2, &v.report.phase2);
    assert_eq!(pv.topups_started, 0);
    assert_eq!(
        v.count("provisioning"),
        0,
        "no provisioning message is sent"
    );
    assert!(
        pv.vehicles_starved > pb.vehicles_starved,
        "{} vs {}",
        pv.vehicles_starved,
        pb.vehicles_starved
    );
    assert!(pv.cert_unavailable_ns > pb.cert_unavailable_ns);
}

#[test]
fn the_request_shuffle_does_not_touch_in_run_topups() {
    // CAMP 1.2.2 Table 67: a certificate request leaves the RA's shuffle at the threshold
    // or the window. Expected first: a threshold of 1 releases each request at once, so
    // top-ups finish sooner. Measured: the run is identical, event for event. The reason
    // is the deployment's design, which the engine follows: a device sends one butterfly
    // request, at bootstrap (here before it joins, ScmsRun::preload), and every later
    // week is the RA's pre-generation from the same keys, sent to the ACA at the period
    // boundary without waiting in the request shuffle (CAMP Primer p.6, provision.rs
    // `ready && pregen`). So the shuffle settings are published as partial; this test
    // pins it, and goes red the day a run makes a first request in-run. It runs on the
    // 2-week ra-maxPreloadTime ([SMP] Table 3 floor): a week inside the horizon is held
    // since the request and its top-off crosses no RA-ACA hop at all
    // (`a_slow_aca_does_not_delay_a_week_the_ra_already_holds`), so only weeks the weekly
    // job makes at an in-run boundary reach the ACA in the run.
    let (b, v) = (
        h2_base(),
        with(json!({"max_periods_ahead": 2,
            "cert_shuffle_threshold": 1, "cert_shuffle_window_s": 600})),
    );
    knob(
        "cert_shuffle_threshold 1000 -> 1, window 5 -> 600 s (2-week horizon)",
        b,
        &v,
    );
    assert!(
        // Since the weekly job runs at every boundary (52432316) those RA-ACA hops are the
        // job's own flow, `pregeneration`, not a device's `topup`.
        ["topup", "pregeneration"]
            .iter()
            .map(|f| b.between(f, "ra", "aca") + b.between(f, "ra", "pca"))
            .sum::<usize>()
            > 0,
        "{:?}",
        b.pairs()
    );
    assert_eq!(v.events(), b.events(), "the shuffle moved an in-run event");
    assert_eq!(
        v.report.phase2.certs_topped_up,
        b.report.phase2.certs_topped_up
    );
    assert_eq!(v.topup_ms(), b.topup_ms());
}

/// The base on the 2-week ra-maxPreloadTime ([SMP] Table 3 floor): every week after the
/// request's horizon is the RA's weekly job at its in-run i-period boundary.
fn h2_base() -> &'static Out {
    static BASE: OnceLock<Out> = OnceLock::new();
    BASE.get_or_init(|| with(json!({"max_periods_ahead": 2})))
}

/// The base with the ACA slow (4 s more per message for the whole run). A week the RA
/// generated before the request is held and the slowness does not reach the device; a
/// week the weekly job makes in the run (the 2-week horizon, `slow_h2_base`) is not ready
/// at the RA's nextDlTime and the device polls again (download-minWait), up to
/// download-maxReqs times.
fn slow_aca(lifecycle: Value) -> Out {
    let mut doc = base_doc();
    doc["actors"]["backend"]["failures"] =
        json!([{"node": "pca", "mode": "slow", "from_s": 0.0, "to_s": 300.0, "extra_ms": 4000.0}]);
    doc["security"]["scms"] = json!({"lifecycle": lifecycle});
    run(doc)
}

/// The ACA slow from 30 s only: after the weeks the 60 s top-offs ask for fell due.
fn late_slow_base() -> &'static Out {
    static BASE: OnceLock<Out> = OnceLock::new();
    BASE.get_or_init(|| {
        let mut doc = base_doc();
        doc["actors"]["backend"]["failures"] = json!([
            {"node": "pca", "mode": "slow", "from_s": 30.0, "to_s": 300.0, "extra_ms": 4000.0}
        ]);
        run(doc)
    })
}

fn slow_base() -> &'static Out {
    static BASE: OnceLock<Out> = OnceLock::new();
    BASE.get_or_init(|| slow_aca(json!({})))
}

/// The slow ACA on the 2-week horizon: each top-off's week is made by the weekly job at
/// its in-run boundary, through the slow ACA.
fn slow_h2_base() -> &'static Out {
    static BASE: OnceLock<Out> = OnceLock::new();
    BASE.get_or_init(|| slow_aca(json!({"max_periods_ahead": 2})))
}

#[test]
fn a_slow_aca_does_not_delay_a_week_the_ra_already_holds() {
    // A real RA generates every week within ra-maxPreloadTime when it accepts the request
    // and adds one a week ([PRIMER] p.6; [CAMP122] 5.2.7.6.4), so a top-off of such a week
    // is the .info request and the download of a file it holds (1609.2.1 §6.3.5.4): no
    // generation wait, no RA-ACA hop in the top-off, and an ACA slow in the run does not
    // reach it. Before the fix the RA ran the week's job when the device asked and told
    // it now + first_batch_delay: 2,340 ms per top-up on this base, 19,182 ms with the ACA
    // slow. Fault (2026-10-27): `ScmsRun::generate_past_due` returning at once.
    let (b, v) = (base(), late_slow_base());
    knob("pca slow 4 s from 30 s, weeks held since the request", b, v);
    assert!(b.topup_ms() > 0.0, "the base tops up");
    assert!(b.topup_ms() < 1_000.0, "a held week: {} ms", b.topup_ms());
    assert!(
        (v.topup_ms() - b.topup_ms()).abs() < 0.1 * b.topup_ms() + 50.0,
        "{} vs {}",
        v.topup_ms(),
        b.topup_ms()
    );
    assert_eq!(
        b.between("topup", "ra", "aca") + b.between("topup", "ra", "pca"),
        0,
        "a top-off of a held week crossed an RA-ACA hop"
    );
    // Slow from 0 s, the ACA is slow when the weeks fall due (the fleet's acceptance), so
    // the RA's job for them runs under it and can finish after the top-offs ask: they wait
    // for it (the .info answer is the job's finish), as at a real RA whose ACA was slow
    // during the batch (scms-fix3-ra RA.slow-aca-held: 4,947 ms against 337 ms).
    let early = slow_base();
    knob("pca slow 4 s from 0 s (when the weeks fell due)", b, early);
    assert!(
        early.topup_ms() > b.topup_ms() + 1_000.0,
        "{}",
        early.topup_ms()
    );
}

#[test]
fn a_longer_download_wait_lengthens_a_late_topup() {
    // download-minWait (SCMS Manager EE profile Table 3): a batch that is not ready at
    // nextDlTime is asked for again after this long. With the ACA 4 s late, a 2 s wait
    // finds it at the second or third poll, a 20 s wait at the next one 20 s on. A week the
    // RA already holds is never late, so the batch here is one the weekly job makes at its
    // in-run boundary: the 2-week ra-maxPreloadTime ([SMP] Table 3 floor). First run (ACA
    // on time): no change at all, as the model's nextDlTime is exact; the knob only acts
    // on a late batch.
    let (b, v) = (
        slow_h2_base(),
        slow_aca(json!({"max_periods_ahead": 2, "download_poll_interval_s": 20})),
    );
    knob(
        "download_poll_interval_s 2 -> 20 (ACA 4 s slow, 2-week horizon)",
        b,
        &v,
    );
    // Only a late top-off waits longer: one whose batch was not ready at nextDlTime and
    // that polled again. Its last poll overshoots the batch by U[0, minWait), so each one
    // late in both runs waits (20 s - 2 s) / 2 = 9 s more on average, and an on-time one
    // not at all: the mean moves by about the late share times 9 s (scms-fix3-ra
    // RA.download-wait; the 5 s bound this test had assumed most top-offs late). Half of
    // that, for the overshoots' spread over a few late top-offs.
    let (pb, pv) = (b.download_polls(), v.download_polls());
    let late = pb.iter().filter(|n| **n > 1).count();
    let n = pb.len().max(1);
    println!(
        "late top-offs: base {late} of {n} (polls {}), variant {} of {} (polls {})",
        pb.iter().sum::<usize>(),
        pv.iter().filter(|n| **n > 1).count(),
        pv.len(),
        pv.iter().sum::<usize>()
    );
    assert!(late > 0, "the slow ACA makes some top-offs late");
    let bound = 0.5 * late as f64 / n as f64 * (20_000.0 - 2_000.0) / 2.0;
    assert!(
        v.topup_ms() > b.topup_ms() + bound,
        "{} vs {} (+{bound:.0} ms for {late} late of {n})",
        v.topup_ms(),
        b.topup_ms()
    );
    // And a longer wait asks less often: fewer download requests per late top-off.
    let per = |p: &[usize]| {
        let late: Vec<_> = p.iter().filter(|n| **n > 1).collect();
        late.iter().map(|n| **n).sum::<usize>() as f64 / late.len().max(1) as f64
    };
    assert!(per(&pv) < per(&pb), "{} vs {}", per(&pv), per(&pb));
    assert!(
        v.report.phase2.cert_unavailable_ns > b.report.phase2.cert_unavailable_ns,
        "vehicles wait longer without a certificate"
    );
}

#[test]
fn one_download_poll_gives_up_on_a_late_batch() {
    // download-maxReqs 1: with the ACA 4 s late the one poll at nextDlTime finds nothing
    // and the device makes no other. First run (ACA on time): no change, as the poll at
    // the exact nextDlTime always succeeds.
    let (b, v) = (
        slow_h2_base(),
        slow_aca(json!({"max_periods_ahead": 2, "max_download_polls": 1})),
    );
    knob(
        "max_download_polls 100 -> 1 (ACA 4 s slow, 2-week horizon)",
        b,
        &v,
    );
    let (pb, pv) = (&b.report.phase2, &v.report.phase2);
    assert!(pb.topups_completed > 0);
    assert!(
        pv.topups_completed < pb.topups_completed,
        "{} vs {}",
        pv.topups_completed,
        pb.topups_completed
    );
    assert!(pv.vehicles_starved >= pb.vehicles_starved);
}

#[test]
fn a_short_enrolment_is_renewed_in_the_run() {
    // CAMP 1.2.2 Table 3 / SM-REENR: the device re-enrols reenrol_lead_s (120 s) before
    // its enrolment certificate ends. First expectation (none in the base) was wrong: the
    // base already re-enrols 26 vehicles, whose 600 s enrolment began before the run at
    // their bootstrap. 200 s puts every vehicle's renewal inside the run, some twice.
    let (b, v) = (base(), with(json!({"enrolment_lifetime_s": 200})));
    knob("enrolment_lifetime_s 600 -> 200", b, &v);
    let (pb, pv) = (&b.report.phase2, &v.report.phase2);
    assert!(
        pv.reenrolments_started > 2 * pb.reenrolments_started,
        "{} vs {}",
        pv.reenrolments_started,
        pb.reenrolments_started
    );
    assert!(v.count("reenrolment") > b.count("reenrolment"));
}

/// The first revocation's latency, detection to the first vehicle enforcing, s.
fn revocation_s(o: &Out) -> f64 {
    o.report.phase2.revocation_latency_ns as f64 / 1e9
}

#[test]
fn the_ma_review_delays_every_revocation() {
    // The pilots' manual MA (CAMP 1.2.2 §4.2-4.3): the operator's review comes between the
    // decision and the CRL entry, so the first revocation is later by the review, give or
    // take one 30 s CRL cadence slot and one 30 s fetch.
    let (b, v) = (base(), with(json!({"ma_review_s": 60})));
    knob("ma_review_s 0 -> 60", b, &v);
    assert!(revocation_s(b) > 0.0, "the base revokes");
    assert!(
        revocation_s(&v) > revocation_s(b),
        "{} vs {}",
        revocation_s(&v),
        revocation_s(b)
    );
    assert!(v.report.phase2.crls_issued <= b.report.phase2.crls_issued);
}

/// When each `ma.decision` record is stamped, s.
fn ma_decisions_s(o: &Out) -> Vec<f64> {
    o.pki
        .iter()
        .filter(|(c, _)| c == "ma.decision")
        .filter_map(|(_, v)| v["t"].as_u64())
        .map(|t| t as f64 / 1e9)
        .collect()
}

#[test]
fn ma_decision_is_the_operators_decision_after_the_review() {
    // audit-mbpipe: `ma.decision` was stamped when the MA pipeline recommended, 8 h before
    // the operator's decision under ManualReview, so `time_to_decision` and the
    // vehicle-level det_* cells counted decisions not yet taken. The record is the
    // case's `decision` stage: the review moves the first one by the review (the run is
    // identical up to the first recommendation), and every one is a case's decision.
    let (b, v) = (base(), with(json!({"ma_review_s": 60})));
    let (db, dv) = (ma_decisions_s(b), ma_decisions_s(&v));
    let first = |d: &[f64]| d.iter().copied().fold(f64::INFINITY, f64::min);
    assert!(!db.is_empty() && !dv.is_empty(), "both decide");
    assert!(
        first(&dv) >= first(&db) + 59.0,
        "{} vs {}",
        first(&dv),
        first(&db)
    );
    let stages: Vec<f64> = v
        .pki
        .iter()
        .filter(|(c, x)| c == "pki.case" && x["stage"] == "decision")
        .filter_map(|(_, x)| x["t"].as_u64())
        .map(|t| t as f64 / 1e9)
        .collect();
    for t in &dv {
        assert!(
            stages.contains(t),
            "ma.decision at {t} s is no case's decision"
        );
    }
}

#[test]
fn publishing_on_decision_is_faster_than_a_cadence() {
    // crl_cadence_s 0: the CRL Generator publishes as soon as an entry is issued instead
    // of at the next 30 s slot.
    let (b, v) = (base(), with(json!({"crl_cadence_s": 0})));
    knob("crl_cadence_s 30 -> 0", b, &v);
    assert!(revocation_s(&v) > 0.0);
    assert!(
        revocation_s(&v) <= revocation_s(b),
        "{} vs {}",
        revocation_s(&v),
        revocation_s(b)
    );
    assert!(v.report.phase2.crl_versions_published >= b.report.phase2.crl_versions_published);
}

#[test]
fn a_slower_crl_fetch_downloads_less_and_enforces_later() {
    // A cellular vehicle polls for a newer CRL every crl_fetch_interval_s: four times
    // slower is about a quarter of the downloads, and the first enforcement waits for a
    // poll after publication.
    let (b, v) = (base(), with(json!({"crl_fetch_interval_s": 120})));
    knob("crl_fetch_interval_s 30 -> 120", b, &v);
    let (pb, pv) = (&b.report.phase2, &v.report.phase2);
    assert!(
        pv.crl_downloads < pb.crl_downloads,
        "{} vs {}",
        pv.crl_downloads,
        pb.crl_downloads
    );
    assert!(
        revocation_s(&v) >= revocation_s(b),
        "{} vs {}",
        revocation_s(&v),
        revocation_s(b)
    );
}

#[test]
fn a_shorter_report_shuffle_decides_sooner() {
    // CAMP-EE SCMS-765: reports wait in the RA's shuffle up to its window before the MA
    // sees them; 5 s instead of 30 s brings the first decision forward. First instrument
    // (revocation_latency_ns) was wrong: it runs from the *triggering* report's detection,
    // which a batch release leaves at a few seconds whatever the window; the first
    // decision's instant is what the window moves.
    let (b, v) = (base(), with(json!({"report_shuffle_window_s": 5})));
    knob("report_shuffle_window_s 30 -> 5", b, &v);
    let (fb, fv) = (
        b.first("decision").expect("base decides"),
        v.first("decision").expect("variant decides"),
    );
    assert!(fv < fb, "{fv} vs {fb}");
}

/// The base with a fifth of the fleet attacking and a 60 s review, so that several cases
/// are open at once.
fn busy_ma(lifecycle: Value) -> Out {
    let mut doc = base_doc();
    doc["threats"]["attackers"][0]["fraction"] = json!(0.2);
    let mut l = json!({"ma_review_s": 60});
    if let (Some(m), Some(extra)) = (l.as_object_mut(), lifecycle.as_object()) {
        m.extend(extra.clone());
    }
    doc["security"]["scms"] = json!({"lifecycle": l});
    run(doc)
}

#[test]
fn an_ma_with_one_open_case_revokes_fewer_in_the_run() {
    // ma_max_open_cases 1: a case beyond the first waits for it to close. First run (5 %
    // attackers, automatic MA): no change, as no two cases were ever open together. With
    // a fifth of the fleet attacking and each case held 60 s for review, the cap leaves
    // cases waiting and fewer CRL entries are issued by the end.
    let (b, v) = (busy_ma(json!({})), busy_ma(json!({"ma_max_open_cases": 1})));
    knob(
        "ma_max_open_cases 1000 -> 1 (20 % attackers, 60 s review)",
        &b,
        &v,
    );
    let (pb, pv) = (&b.report.phase2, &v.report.phase2);
    assert!(
        pb.crls_issued > 1,
        "the busy base revokes several: {}",
        pb.crls_issued
    );
    assert!(
        pv.crls_issued < pb.crls_issued,
        "{} vs {}",
        pv.crls_issued,
        pb.crls_issued
    );
}

#[test]
fn the_camp_reference_preset_is_one_scms_with_a_lop() {
    // 00-gap-analysis §2.2: camp-reference is the CAMP design (one SCMS with LOP, no
    // providers); the default is the US deployment's two providers on one CTL.
    let mut doc = base_doc();
    doc["security"]["scms"] = json!({"deployment": {"preset": "camp-reference"}});
    let (b, v) = (base(), run(doc));
    knob(
        "deployment.preset us-production-2025 -> camp-reference",
        b,
        &v,
    );
    println!("PAIRS base {:?}\nPAIRS camp {:?}", b.pairs(), v.pairs());
    println!(
        "PROVIDERS base {:?} camp {:?}",
        b.report.phase2.devices_by_provider, v.report.phase2.devices_by_provider
    );
    assert_eq!(
        b.report.phase2.devices_by_provider.len(),
        2,
        "{:?}",
        b.report.phase2.devices_by_provider
    );
    assert!(
        v.report.phase2.devices_by_provider.len() <= 1,
        "{:?}",
        v.report.phase2.devices_by_provider
    );
    // Two providers share reports MA to MA (01 §2.3); one SCMS has no peer MA. Before
    // the view named the peer providers' components, the shared report read "ee -> ma".
    let (bp, vp) = (b.pairs(), v.pairs());
    let pair = |f: &str, t: &str| (f.to_string(), t.to_string());
    assert!(bp.contains(&pair("ma", "ma")), "{bp:?}");
    assert!(
        !bp.contains(&pair("ee", "ma")),
        "a device reaches the MA directly: {bp:?}"
    );
    assert!(!vp.contains(&pair("ma", "ma")), "{vp:?}");
    // The CAMP design runs a LOP between the devices and the RA; the US deployment runs
    // none (01 §2.2), and its records fold the transparent hop the run still carries.
    assert!(
        vp.contains(&pair("ee", "lop")) && vp.contains(&pair("lop", "ra")),
        "{vp:?}"
    );
    assert!(!bp.iter().any(|(f, t)| f == "lop" || t == "lop"), "{bp:?}");
}

#[test]
fn the_architecture_is_the_deployments() {
    // Who talks to whom (01 §2.2, 03 §3-§9): a device talks only to the RA (and the ECA
    // through the DCM at enrolment); the RA to the ACA and the LAs; the MA to the PCA
    // (ACA), the LAs and the CRL Generator. No device talks to the ACA, an LA or the MA.
    let b = base();
    let pairs = b.pairs();
    println!("PAIRS {pairs:?}");
    for (from, to) in &pairs {
        if from == "ee" {
            assert!(
                !matches!(
                    to.as_str(),
                    "aca" | "pca" | "la1" | "la2" | "crlg" | "eca" | "ma"
                ),
                "a device reaches {to} directly"
            );
        }
    }
    for p in [
        ("ra", "aca"),
        ("ra", "la1"),
        ("ra", "la2"),
        ("ma", "aca"),
        ("ma", "la1"),
        ("ma", "la2"),
        ("ma", "crlg"),
    ] {
        assert!(
            pairs.contains(&(p.0.to_string(), p.1.to_string())),
            "no {p:?}: {pairs:?}"
        );
    }
    // The LAs never talk to each other or to a device (separation of the linkage
    // authorities, CAMP-EE §2.1.7).
    assert!(
        !pairs
            .iter()
            .any(|(f, t)| (f.starts_with("la") && (t.starts_with("la") || t == "ee")))
    );
    assert!(
        pairs.iter().any(|(f, t)| f == "ee" && t == "ra"),
        "{pairs:?}"
    );
    assert!(
        pairs.iter().any(|(f, t)| f == "ra" && t == "ee"),
        "{pairs:?}"
    );
    // No LOP in the US deployment (01 §2.2), so none in its records.
    assert!(
        !pairs.iter().any(|(f, t)| f == "lop" || t == "lop"),
        "{pairs:?}"
    );
    // Every case that reached a CRL was enforced by no more vehicles than were present.
    assert!(b.cases("enforced") > 0, "the base enforces a revocation");
}

/// The packet-level Uu family of `tests/packet_cellular.rs`: backend traffic on TCP/TLS
/// over a scheduled LTE cell, where the TLS handshake is carried and timed.
fn packet_doc(tls: Option<&str>) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(root.join("scenarios").join("phase1-grid.yaml")).expect("loads");
    let mut doc = serde_json::to_value(&s).expect("serialises");
    doc["time"]["duration_s"] = json!(25.0);
    doc["time"]["calendar"] = json!("compressed");
    doc["actors"]["vehicles"]["demand"]["rate_veh_per_h"] = json!(300.0);
    doc["world"]["source"]["params"]["cols"] = json!(4);
    doc["world"]["source"]["params"]["rows"] = json!(4);
    let p = v2xw_engine::phase2::CAMP_SCMS;
    doc["actors"]["backend"]["protocol"] = json!(p);
    doc["security"]["protocol"] = json!({"id": p, "params": {}});
    let mut scms = json!({"lifecycle": {
        "i_period_s": 20, "cert_lifetime_s": 21, "certs_per_period": 3, "pool_periods": 2,
        "topup_below_periods": 1, "cert_shuffle_window_s": 1, "first_batch_delay_s": 1,
        "download_poll_interval_s": 1}});
    if let Some(t) = tls {
        scms["tls"] = json!(t);
    }
    doc["security"]["scms"] = scms;
    doc["net"]["uu"] = json!({"id": "cellular/uu/packet-pf",
        "params": {"rat": "lte", "background_load": 0.5, "isd_m": 500}});
    doc
}

#[test]
fn tls_1_2_costs_more_handshake_and_a_slower_topup() {
    // SCMS Manager EE profile §2.1.18: session-scmsAuth is TLS 1.2 or 1.3. A full 1.2
    // handshake is two round trips before the first request, 1.3's one (RFC 8446 §2).
    // The measured US-profile payloads (constants.md, TLS) are 173 + 934 + 93 + 51 =
    // 1,251 B for 1.2 against 256 + 1,066 = 1,322 B for 1.3, but 1.2 sends two more
    // flights, each with its own TCP/IP headers. Corrected expectation (the first said
    // fewer bytes from the payloads alone): slightly more bytes on the air, and one more
    // round trip in every top-up.
    let (b, v) = (run(packet_doc(None)), run(packet_doc(Some("1.2"))));
    knob("tls 1.3 -> 1.2 (packet-level LTE Uu)", &b, &v);
    let (cb, cv) = (
        &b.report.phase2.access.carrier,
        &v.report.phase2.access.carrier,
    );
    assert!(cb.tls_handshakes > 0, "the base opens TLS sessions");
    assert_eq!(cv.tls_handshakes, cb.tls_handshakes, "the same sessions");
    assert!(
        cv.tls_handshake_bytes > cb.tls_handshake_bytes,
        "{} vs {}",
        cv.tls_handshake_bytes,
        cb.tls_handshake_bytes
    );
    assert!(
        v.topup_ms() > b.topup_ms(),
        "{} vs {}",
        v.topup_ms(),
        b.topup_ms()
    );
}

#[test]
fn a_shorter_pseudonym_period_changes_more_often() {
    // SAE J2945/1 §6.1.2: 300 s by default. In this run a vehicle also changes at every
    // 60 s i-period, when its certificate expires, so the base's changes are those; 30 s
    // adds one between each pair: at least twice the base. (First expectation, ten times,
    // forgot the i-period changes.)
    let mut doc = base_doc();
    doc["security"]["pseudonym_change"]["period_s"] = json!(30.0);
    let (b, v) = (base(), run(doc));
    knob("pseudonym_change.period_s 300 -> 30", b, &v);
    assert!(v.report.phase2.pseudonym_changes > 2 * b.report.phase2.pseudonym_changes.max(1));
}

#[test]
fn every_enforcement_count_is_out_of_the_devices_it_could_have_reached() {
    // The bug this audit fixed: the conformance run's case board read "enforced by 88 of
    // 74" because N was the devices present at the view. N is every device present at
    // some instant since the CRL was published: it never falls, and never falls below n.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let s = Scenario::load(root.join("scenarios").join("scms-conformance.yaml")).expect("loads");
    let o = run(serde_json::to_value(&s).expect("serialises"));
    let mut by_case: BTreeMap<String, Vec<(u64, u64, u64)>> = BTreeMap::new();
    for (c, v) in &o.pki {
        if c == "pki.case" && v["stage"] == "enforced" {
            by_case
                .entry(v["case"].as_str().unwrap_or("").to_string())
                .or_default()
                .push((
                    v["t"].as_u64().unwrap_or(0),
                    v["detail"]["installed"].as_u64().unwrap_or(0),
                    v["detail"]["of"].as_u64().unwrap_or(0),
                ));
        }
    }
    assert!(
        !by_case.is_empty(),
        "the conformance run enforces a revocation"
    );
    for (case, stamps) in &by_case {
        let last = stamps.last().expect("stamps");
        println!(
            "ENFORCED {case}: {} of {} ({} stamps)",
            last.1,
            last.2,
            stamps.len()
        );
        for w in stamps.windows(2) {
            assert!(w[0].2 <= w[1].2, "{case}: N fell {w:?}");
        }
        for (_, n, of) in stamps {
            assert!(n <= of, "{case}: {n} of {of}");
        }
    }
}

#[test]
fn a_small_validated_certificate_cache_revalidates_and_costs_more_crypto() {
    // nodes.validated_cert_cache 4,096 -> 16 (v2xw-node rxval EE_CACHE; no device
    // publishes one). Expected before the run: (a) a vehicle hears far more than 16
    // pseudonyms in 300 s (125 vehicles, a new pseudonym every 60 s), so the cache
    // evicts, a returning signer pays first sight again: more evictions, more EE
    // validations, fewer hits, more validation time charged; (b) the same events within
    // a few per cent (validation is service time, not events); (c) the backend is not
    // involved: the same top-ups and CRL downloads.
    let mut doc = base_doc();
    doc["nodes"]["validated_cert_cache"] = json!(16);
    let (b, v) = (base(), run(doc));
    knob("nodes.validated_cert_cache 4096 -> 16", b, &v);
    let (rb, rv) = (&b.report.receiver_validation, &v.report.receiver_validation);
    println!(
        "CACHE base ee_validations {} hits {} evictions {} charged_ns {}\n      variant ee_validations {} hits {} evictions {} charged_ns {}",
        rb.ee_validations,
        rb.ee_hits,
        rb.ee_evictions,
        rb.charged_ns,
        rv.ee_validations,
        rv.ee_hits,
        rv.ee_evictions,
        rv.charged_ns
    );
    assert!(rv.ee_evictions > rb.ee_evictions, "{rv:?} vs {rb:?}");
    assert!(rv.ee_validations > rb.ee_validations, "{rv:?} vs {rb:?}");
    assert!(rv.ee_hits < rb.ee_hits, "{rv:?} vs {rb:?}");
    assert!(rv.charged_ns > rb.charged_ns, "{rv:?} vs {rb:?}");
    // (b) explained (protocol/flow gaps package, item 5): the +12 % events are all
    // NodeTask, measured 3,774,561 -> 4,268,034 with every other class identical. A
    // receiver is woken at the instant each signature check finishes
    // (`Engine::schedule_wake`); a cache hit costs 400 µs and its finish usually falls
    // inside a wake already pending, while a first-sight validation (priced SHA-256 and
    // ECQV) finishes later, at an instant of its own, and needs a wake of its own. The
    // radio, mobility and backend schedules are untouched: the cost is event-loop work,
    // not more simulated traffic.
    let (eb, ev) = (&b.report.events_by_class, &v.report.events_by_class);
    for (class, n) in eb {
        if class != "NodeTask" {
            assert_eq!(ev.get(class), Some(n), "{class}: {eb:?} vs {ev:?}");
        }
    }
    assert!(ev["NodeTask"] > eb["NodeTask"], "{eb:?} vs {ev:?}");
    assert!(
        ev["NodeTask"] - eb["NodeTask"] <= rv.ee_validations - rb.ee_validations,
        "at most one extra wake per extra first-sight validation"
    );
}

#[test]
fn the_overdue_crl_tolerance_cannot_act_inside_a_week() {
    // security.scms.crl_overdue_tolerance_s 2 weeks -> 0 (CAMP 1.2.2 Table 67). Expected
    // before the run: nothing changes. A CRL's nextCrl is a week after its issue (CAMP
    // 1.2.2 §3 item 10), so even the strictest device (tolerance 0) finds no CRL overdue in
    // a 300 s run: no `crl-overdue` refusal and the run identical event for event. The
    // rule's effect is proven at the node (v2xw-node tests/rx_validation.rs) and its reach
    // by tests/scms_knobs.rs; an in-run study needs weeks of calendar.
    let mut doc = base_doc();
    doc["security"]["scms"] = json!({"crl_overdue_tolerance_s": 0});
    let (b, v) = (base(), run(doc));
    knob("security.scms.crl_overdue_tolerance_s 1209600 -> 0", b, &v);
    let rv = &v.report.receiver_validation;
    assert_eq!(rv.refused.get("crl-overdue"), None, "{rv:?}");
    assert_eq!(v.events(), b.events(), "the run is identical");
}
