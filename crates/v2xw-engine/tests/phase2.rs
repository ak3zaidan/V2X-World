//! The Phase 2 path: a report becomes a vehicle that cannot sign.
//!
//! 10-roadmap.md Phase 2 acceptance item 2 is "every stage timestamp in 05-protocols §8 is
//! emitted and the latency decomposition renders". These tests assert on the path and on
//! the decomposition, and — because the whole point of this file is a check that could
//! pass while doing nothing — each assertion is paired with the same scenario with one
//! piece removed.
//!
//! # The scenario the tests run
//!
//! `phase2-manhattan.yaml` places a roadside unit at every signalised intersection of 5th
//! and 6th Avenues (55 units) with building obstruction on. Every unit is a receiver of
//! every frame in range, and a debug build spends most of a run's time in the reception
//! phase, so the tests thin the deployment to every third unit on each avenue (the path is
//! the same one; only the number of masts differs) and shorten nothing else.
//! [`the_shipped_scenario_builds_with_its_whole_deployment`] builds the shipped file as it
//! is, so the thinning cannot hide a scenario that does not load.

mod regression;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::json;
use v2xw_engine::scenario::ModelChoice;
use v2xw_engine::{DigestRecorder, Engine, RunReport, Scenario};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

/// The shipped scenario, rooted.
fn shipped() -> Scenario {
    rooted(
        Scenario::load(scenarios().join("phase2-manhattan.yaml"))
            .expect("the shipped scenario loads"),
    )
}

/// The shipped scenario with every third roadside unit kept (see the module docs), run for
/// 360 s.
///
/// The horizon holds the whole path whichever realisation the node-keyed draws give. The
/// MA decides on three corroborated events behind a 60 s report shuffle, and the attacker is
/// the first vehicle onto an empty map, so the decision time depends on which neighbours
/// carry a modem: 112, 136 and 242 s on seeds `0xC0FFEE_0001`, `_0002` and the scenario's
/// own since the roadside renumbering re-keyed those draws (p1gaps, BUILD-STATUS fix round
/// 3). After it the path needs the PCA and LA lookups, the Generator's 60 s cadence and one
/// 30 s fetch interval: a decision at 242 s is installed by about 334 s, past the shipped
/// 300 s (scms-fix4-a.yaml F4A.P2.*).
fn phase2() -> Scenario {
    let mut s = shipped();
    s.time.duration_s = 360.0;
    s.actors.rsus = s
        .actors
        .rsus
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 3 == 0)
        .map(|(_, r)| r)
        .collect();
    s
}

/// Makes a scenario's world source path absolute.
///
/// A scenario's paths are relative to the repository root, because that is where a user
/// runs `v2xw` from; a test's working directory is its crate. Rewriting the path here
/// rather than changing the process's directory keeps the tests runnable in parallel,
/// which `cargo test` does by default.
fn rooted(mut scenario: Scenario) -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut scenario.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    scenario
}

fn run(scenario: Scenario) -> (RunReport, regression::Pinned) {
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = DigestRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    let pinned = regression::Pinned {
        record_digest: recorder.digest_hex(),
        frame_digest: recorder.frame_digest_hex(),
        records: recorder.written(),
    };
    (report, pinned)
}

/// A recorder that keeps one channel's records and drops the rest as they come.
///
/// `MemoryRecorder` holds every `phy.rx` of a Midtown run, which took the distribution
/// test past the 2.5 GB run cap (adoption plan WP-13 status); the test reads one channel.
#[derive(Default)]
struct ChannelRecorder {
    channel: &'static str,
    kept: Vec<v2xw_core::ctx::OwnedRecord>,
    seen: u64,
}

impl v2xw_engine::RunRecorder for ChannelRecorder {
    fn write(&mut self, _at: v2xw_core::time::SimTime, record: &v2xw_core::ctx::OwnedRecord) {
        self.seen += 1;
        if record.channel == self.channel {
            self.kept.push(record.clone());
        }
    }

    fn records_written(&self) -> Option<u64> {
        Some(self.seen)
    }
}

/// [`run`] keeping only `channel`'s records.
fn run_channel(scenario: Scenario, channel: &'static str) -> (RunReport, ChannelRecorder) {
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = ChannelRecorder {
        channel,
        ..ChannelRecorder::default()
    };
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

/// The path run, once for every test in this binary that reads it.
fn path_run() -> &'static (RunReport, regression::Pinned) {
    static RUN: OnceLock<(RunReport, regression::Pinned)> = OnceLock::new();
    RUN.get_or_init(|| run(phase2()))
}

/// Sets one lifecycle parameter over the shipped ones.
fn lifecycle(s: &mut Scenario, key: &str, value: f64) {
    let choice = s.security.protocol.get_or_insert_with(|| ModelChoice {
        id: v2xw_engine::phase2::CAMP_SCMS.to_string(),
        params: json!({}),
    });
    if !choice.params.is_object() {
        choice.params = json!({});
    }
    choice.params[key] = json!(value);
}

/// The shipped file loads and builds with its whole deployment.
#[test]
fn the_shipped_scenario_builds_with_its_whole_deployment() {
    let s = shipped();
    assert!(v2xw_engine::scenario::validate::validate(&s).is_empty());
    assert_eq!(s.actors.rsus.len(), 55);
    assert!(s.world.buildings.enabled, "building obstruction must be on");
    let engine = Engine::build(s, "").expect("builds");
    let p = engine.phase2().expect("the Phase 2 path is declared");
    assert_eq!(p.rsu_nodes().len(), 55);
}

/// The whole path, in one run.
#[test]
fn a_report_becomes_a_vehicle_that_cannot_sign() {
    let (report, _) = path_run();
    let p = &report.phase2;
    println!("{}", serde_json::to_string_pretty(p).unwrap_or_default());

    assert_eq!(
        p.backend_errors, 0,
        "the backend refused: {}",
        p.first_backend_error
    );
    assert!(p.rsus > 1, "the roadside units were not created");
    assert_eq!(p.attackers, 1, "the attacker was not armed");
    assert!(
        p.falsified_claims > 0,
        "the attacker never falsified anything"
    );
    assert!(
        p.messages_checked > 0,
        "the detector suite never saw a message"
    );
    assert!(p.verdicts_fired > 0, "no detector fired");
    assert!(p.reports_sent > 0, "no report was filed");
    assert!(
        p.reports_uploaded_cellular > 0,
        "no report crossed a cellular uplink, which is how this fleet reaches the backend"
    );
    assert!(
        p.reports_received > 0,
        "no report reached the privacy proxy"
    );
    assert!(
        p.reports_at_ma > 0,
        "no report came through the RA's shuffle to the authority"
    );
    assert!(p.ma_revoke_decisions > 0, "the authority never decided");
    assert!(p.cases_opened > 0, "the backend never opened a case");
    assert!(p.crls_issued >= 1, "the CRL Generator did not issue");
    assert!(
        p.revoked_attackers >= 1,
        "the attacker was not the device revoked"
    );
    assert_eq!(
        p.revoked_honest, 0,
        "an honest device was revoked beside the attacker"
    );
    assert!(
        p.crl_versions_published >= 1,
        "the CRL Store never published"
    );
    assert!(
        p.crl_broadcasts > 0 || p.crl_downloads > 0,
        "the list was published and neither path distributed it"
    );
    assert!(p.crls_installed > 0, "no vehicle installed the entry");
    assert!(
        p.revoked_receptions > 0,
        "the entry was installed and no reception was ever refused by it"
    );
    assert!(p.revocation_latency_ns > 0);
}

/// CV.attack-onset (audit-sim-metrics §3 item 5): the run report's onset is the first
/// action that changed bytes on the air, as `time_to_detect`'s, not the 07-threats label
/// rule's first falsified claim, which can come hours later; and `onset_to_detection_ns`
/// runs to the earliest report at or after it, so it never saturates to 0. Faults that
/// turn it red: `onset_ns` read from `first_falsified`, or the latency taken from the
/// first report whatever its time.
#[test]
fn the_run_report_onset_is_the_first_change_on_the_air() {
    let (report, _) = path_run();
    let p = &report.phase2;
    let mut want = Vec::new();
    for o in &p.attacker_outcomes {
        println!(
            "attacker {} onset {:?} falsified {:?} reported {:?} detected {:?} decided {:?}",
            o.node,
            o.onset_ns,
            o.first_falsified_ns,
            o.first_reported_ns,
            o.first_detected_ns,
            o.decided_ns
        );
        if let (Some(on), Some(f)) = (o.onset_ns, o.first_falsified_ns) {
            assert!(
                on <= f,
                "attacker {}: onset {on} after its first lie {f}",
                o.node
            );
        }
        if let (Some(on), Some(d)) = (o.onset_ns, o.first_detected_ns) {
            assert!(d >= on, "attacker {}: detected before its onset", o.node);
            if o.decided_ns.is_some() {
                want.push(d - on);
            }
        }
    }
    let mut got = p.onset_to_detection_ns.clone();
    got.sort_unstable();
    want.sort_unstable();
    assert!(!got.is_empty(), "no decided attacker was measured");
    assert_eq!(got, want, "onset_to_detection_ns is not detected - onset");
}

/// The onset test above cannot fail under its own fault: a ConstPos attacker's first change
/// on the air is also its first falsified claim, so reading the onset from the 07-threats
/// label gives the same instants (06-validation-summary §9 item 11). A SlowDrift attacker
/// starts with a centimetre of drift, which changes the bytes on the air long before the
/// label rule calls the claim falsified, so here the two onsets differ and
/// `onset_to_detection_ns` must run from the first change. Faults that turn it red:
/// `AttackerOutcome::onset_ns` (and the latency) read from `first_falsified`, or the
/// latency taken from the first report whatever its time.
#[test]
fn a_slow_drift_onset_is_its_first_centimetre_not_its_first_falsified_claim() {
    let mut s = rooted(
        Scenario::load(scenarios().join("live-atk/share-constpos-l-10.yaml"))
            .expect("the live-atk scenario loads"),
    );
    s.time.duration_s = 100.0;
    for a in &mut s.threats.attackers {
        a.id = "threat/attacker/legacy/SlowDrift".to_string();
        a.schedule = Some(v2xw_engine::scenario::schema::DilationWindow {
            from_s: 10.0,
            to_s: 100.0,
        });
    }
    let (report, _) = run(s);
    let p = &report.phase2;
    let mut want = Vec::new();
    let mut ahead = 0;
    for o in &p.attacker_outcomes {
        println!(
            "{} {}: onset {:?} falsified {:?} detected {:?} decided {:?}",
            o.attack, o.node, o.onset_ns, o.first_falsified_ns, o.first_detected_ns, o.decided_ns
        );
        assert!(o.attack.ends_with("SlowDrift"), "{}", o.attack);
        if let (Some(on), Some(f)) = (o.onset_ns, o.first_falsified_ns) {
            assert!(
                on <= f,
                "attacker {}: onset {on} after its first lie {f}",
                o.node
            );
            if on < f {
                ahead += 1;
            }
        }
        if let (Some(on), Some(d)) = (o.onset_ns, o.first_detected_ns) {
            assert!(d >= on, "attacker {}: detected before its onset", o.node);
            if o.decided_ns.is_some() {
                want.push(d - on);
            }
        }
    }
    assert!(
        ahead > 0,
        "no SlowDrift attacker changed its bytes before its first falsified claim: the \
         onset is read from the label rule"
    );
    let mut got = p.onset_to_detection_ns.clone();
    got.sort_unstable();
    want.sort_unstable();
    assert!(
        !got.is_empty(),
        "no decided SlowDrift attacker was measured"
    );
    assert_eq!(
        got, want,
        "onset_to_detection_ns is not detected - first change"
    );
}

/// Found by the SCMS effects round (docs/design/scms-real/testing/effects.md, rows
/// base-concurrent-cases and att-spread): the backend opened one case per provider at a
/// time, so under the pilots' manual review the n-th attacker decided in one shuffle
/// release waited n review periods (in the real-calendar run the second decision came 8 h
/// after the first, and with 20 % attackers three of ten decisions never opened a case).
/// The MA's case table holds cases side by side (gap row 27, `MaPolicy::max_open_cases`).
/// Fault that turns it red: the `while !case_open()` gate restored in `start_cases_of`.
#[test]
fn every_decision_opens_its_case_at_once_under_manual_review() {
    let mut s = phase2();
    s.threats.attackers[0].count = Some(4);
    // A review longer than the run: no case finishes, so a serial MA opens one.
    lifecycle(&mut s, "ma_review_s", 3_600.0);
    let (report, _) = run(s);
    let p = &report.phase2;
    assert_eq!(
        p.backend_errors, 0,
        "the backend refused: {}",
        p.first_backend_error
    );
    assert!(
        p.ma_revoke_decisions >= 2,
        "the run needs two decisions to say anything: {}",
        p.ma_revoke_decisions
    );
    assert_eq!(
        p.cases_opened, p.ma_revoke_decisions,
        "a decision waited for another case to finish"
    );
    assert_eq!(p.revoked_honest, 0);
}

/// The decomposition is the revocation's own: every stage of 05-protocols §8 the path
/// passes through is present and in order, the RA's report shuffle is on it, and the CRL
/// was published on the Generator's cadence.
#[test]
fn the_revocation_latency_is_decomposed_by_stage() {
    let scenario = phase2();
    let horizon_ns = (scenario.time.duration_s * 1e9).round() as u64;
    let (report, _) = path_run();
    let p = &report.phase2;
    let stages: Vec<(&str, u64)> = p
        .revocation_stages
        .iter()
        .map(|(s, t)| (s.as_str(), *t))
        .collect();
    println!("{stages:?}");
    let at = |name: &str| stages.iter().find(|(s, _)| *s == name).map(|(_, t)| *t);
    for name in [
        "detect",
        "report_sent",
        "shuffled",
        "report_received",
        "decision",
        "resolved",
        "blocklisted",
        "issued",
        "published",
        "enforced",
    ] {
        assert!(at(name).is_some(), "stage {name} missing from {stages:?}");
    }
    for w in stages.windows(2) {
        assert!(w[0].1 <= w[1].1, "stages out of order: {stages:?}");
    }
    // The report waited in the RA's shuffle: the window is a minute in this scenario.
    let shuffled = at("shuffled").unwrap() - at("report_sent").unwrap();
    assert!(shuffled > 0, "the report skipped the RA's shuffle");
    assert!(
        shuffled <= 61_000_000_000,
        "the shuffle took longer than its window"
    );
    // The entry waited for the Generator's 60 s cadence boundary.
    assert!(at("published").unwrap() >= at("issued").unwrap());
    assert!(p.revocation_latency_ns < horizon_ns);
    assert_eq!(p.crl_past_horizon, 0);
}

/// With no attacker, nothing is revoked, at the legacy suite's default thresholds and the
/// scenario's authority (`threat/ma/corroborated`). The suite still fires on honest
/// traffic — that is printed, not hidden — and the authority's decision rule is what
/// holds.
#[test]
fn with_no_attacker_nothing_is_revoked() {
    let mut scenario = phase2();
    scenario.threats.attackers.clear();
    let (report, _) = run(scenario);
    let p = &report.phase2;
    let rate = p.verdicts_fired as f64 / p.messages_checked.max(1) as f64;
    println!(
        "legacy-12 on honest traffic: {} verdicts over {} messages ({:.2} %), {} reports, \
         {} at the authority, {} revoke decisions",
        p.verdicts_fired,
        p.messages_checked,
        100.0 * rate,
        p.reports_sent,
        p.reports_at_ma,
        p.ma_revoke_decisions
    );
    let mut by: Vec<(&String, &u64)> = p.verdicts_by_detector.iter().collect();
    by.sort_by(|a, b| b.1.cmp(a.1));
    for (name, n) in by {
        println!(
            "  {name:30} {n:7} ({:.2} % of messages)",
            100.0 * *n as f64 / p.messages_checked.max(1) as f64
        );
    }
    assert_eq!(p.attackers, 0);
    assert_eq!(p.falsified_claims, 0);
    assert!(
        p.messages_checked > 0,
        "the detectors must still be running"
    );
    assert_eq!(
        p.ma_revoke_decisions, 0,
        "the authority decided to revoke a device in a run with no attacker in it"
    );
    assert_eq!(
        p.crls_issued, 0,
        "a device was revoked in a run with no attacker in it"
    );
    assert_eq!(p.revoked_honest, 0);
}

/// Without the `crl` role, and with the cellular poll pushed past the horizon, the list is
/// published and reaches no vehicle on the road: the two distribution paths are the only
/// ways into a vehicle that was driving when it was published. A vehicle switched on after
/// the publication holds it from the fetch on its RA connection (R-JOINCRL), and those
/// installations are counted apart. Roadside units still install it over their own
/// backhaul (WP-B3b cycle) and refuse the revoked sender themselves, and each unit's
/// install of the revoked device's entry is a `proto.revocation` row `rsu_installed`
/// (audit-mbpipe AU.DIST.rsu; fault, the first form: no record showed a unit install).
#[test]
fn without_either_distribution_path_the_revocation_never_reaches_a_vehicle() {
    let mut scenario = phase2();
    for rsu in &mut scenario.actors.rsus {
        rsu.roles.retain(|r| r != "crl");
    }
    lifecycle(&mut scenario, "crl_fetch_interval_s", 1.0e6);
    let (report, recorder) = run_channel(scenario, "proto.revocation");
    assert!(recorder.seen > 0, "the run wrote no records");
    let rsu_rows: Vec<serde_json::Value> = recorder
        .kept
        .iter()
        .map(|r| serde_json::from_slice::<serde_json::Value>(&r.json).expect("json"))
        .filter(|v| v["stage"] == "rsu_installed")
        .collect();
    let units: std::collections::BTreeSet<u64> = rsu_rows
        .iter()
        .map(|v| v["node"].as_u64().expect("a unit row names its unit"))
        .collect();
    assert!(
        !units.is_empty(),
        "no record shows a unit installing the list"
    );
    assert!(
        units.len() as u64 <= report.phase2.rsu_crls_installed,
        "a row for a unit the engine never counted as installing"
    );
    assert!(
        rsu_rows
            .iter()
            .all(|v| v["entries"].as_u64().is_some_and(|e| e > 0)),
        "each row carries the installed list's version"
    );
    let p = &report.phase2;
    // The first half is unchanged.
    assert!(p.reports_received > 0);
    assert!(p.crls_issued >= 1, "the authority must still issue");
    // And the second half is gone.
    assert_eq!(
        p.crl_broadcasts, 0,
        "a unit with no `crl` role broadcast one"
    );
    assert_eq!(p.crl_downloads, 0, "a vehicle polled past its interval");
    assert_eq!(
        p.crls_installed, p.crls_installed_at_join,
        "a vehicle on the road installed the list with neither distribution path"
    );
    assert!(
        p.crls_installed_at_join > 0,
        "no vehicle switched on after the publication held the list (R-JOINCRL)"
    );
    assert_eq!(
        p.revoked_receptions,
        p.rsu_revoked_receptions + p.revoked_receptions_at_join,
        "a vehicle refused a revoked sender with no list installed"
    );
    assert!(
        p.rsu_crls_installed > 0,
        "no unit installed the list over its backhaul"
    );
    assert!(
        p.rsu_revoked_receptions > 0,
        "no unit refused the revoked sender after its backhaul install"
    );
}

/// A fleet with no modem reaches the backend only through a unit that relays reports; with
/// the `report-forward` role removed, no vehicle's report reaches the authority and they
/// wait in the vehicles' outboxes. (The units' own reports, filed as trusted
/// infrastructure over their backhaul, still arrive, and are counted apart.)
#[test]
fn without_a_modem_or_a_relay_the_authority_hears_nothing() {
    let mut scenario = phase2();
    scenario.net.uu = None;
    for rsu in &mut scenario.actors.rsus {
        rsu.roles.retain(|r| r != "report-forward");
    }
    let (report, _) = run(scenario);
    let p = &report.phase2;
    assert!(
        p.reports_sent > 0,
        "the detectors still have to fire, or this proves nothing"
    );
    assert_eq!(p.reports_uploaded_cellular, 0);
    assert_eq!(p.reports_uploaded_relay, 0);
    // The units still file their own reports over their backhaul: what reaches the
    // authority is exactly those, and not one vehicle's.
    assert_eq!(
        p.reports_received, p.reports_from_rsus,
        "a vehicle's report reached the backend over nothing"
    );
    assert!(
        p.reports_unsent > 0,
        "the vehicles' reports must be held, not dropped"
    );
}

/// The Phase 2 run is deterministic.
#[test]
fn the_phase_2_run_is_deterministic() {
    let (first_report, first) = path_run();
    let (second_report, second) = run(phase2());
    assert_eq!(first, &second, "the content digests differ");
    assert_eq!(first_report, &second_report, "the run reports differ");
    assert!(
        first_report.phase2.crls_issued > 0,
        "nothing happened to be deterministic about"
    );
}

/// **Regression (WP-F3).** The path run on the default topology (`us-production-2025`, two
/// providers on one CTL) reproduces its committed digest: any change to what the run
/// records, on any channel, is seen here and has to be re-blessed deliberately.
#[test]
fn the_phase_2_path_reproduces_its_regression_digest() {
    let (report, pinned) = path_run();
    assert!(report.phase2.crls_issued > 0, "the path must reach a CRL");
    assert!(pinned.records > 0);
    regression::check("phase2-path", pinned);
}

/// **Honest revocation on the new topology (WP-F3).** The same Phase 2 path on the earlier
/// single-SCMS `camp-reference` topology and on the default two-provider one: in both the
/// attacker, and only the attacker, is revoked, the list reaches the fleet and refuses its
/// frames, and the vehicles' air traffic is the same. The topology moves backend latency
/// and which provider certifies whom, not what the honest fleet sends.
#[test]
fn the_honest_revocation_path_holds_on_the_old_and_the_new_topology() {
    let (new, _) = path_run();
    let mut s = phase2();
    s.security.scms = Some(
        serde_json::from_value(json!({"deployment": {"preset": "camp-reference"}}))
            .expect("a deployment"),
    );
    let (old, _) = run(s);
    let (n, o) = (&new.phase2, &old.phase2);
    println!(
        "camp-reference: devices {:?} revoked {}+{} latency {:.1} s frames {}\n\
         us-production : devices {:?} revoked {}+{} latency {:.1} s frames {}",
        o.devices_by_provider,
        o.revoked_attackers,
        o.revoked_honest,
        o.revocation_latency_ns as f64 / 1e9,
        old.frames_transmitted,
        n.devices_by_provider,
        n.revoked_attackers,
        n.revoked_honest,
        n.revocation_latency_ns as f64 / 1e9,
        new.frames_transmitted,
    );
    assert_eq!(o.devices_by_provider.len(), 1, "camp-reference is one SCMS");
    assert!(
        n.devices_by_provider.len() >= 2,
        "the default is two providers"
    );
    for (name, p) in [("camp-reference", o), ("us-production-2025", n)] {
        assert_eq!(p.backend_errors, 0, "{name}: {}", p.first_backend_error);
        assert!(
            p.revoked_attackers >= 1,
            "{name}: the attacker was not revoked"
        );
        assert_eq!(p.revoked_honest, 0, "{name}: an honest device was revoked");
        assert!(
            p.crls_installed > 0,
            "{name}: no vehicle installed the entry"
        );
        assert!(
            p.revoked_receptions > 0,
            "{name}: the entry never refused a frame"
        );
    }
    assert_eq!(
        old.frames_transmitted, new.frames_transmitted,
        "the topology changed what the fleet transmits"
    );
}

/// A role belongs to the unit that declares it, not to the scenario.
#[test]
fn a_role_belongs_to_the_unit_that_declares_it_and_not_to_the_scenario() {
    let mut scenario = phase2();
    scenario.actors.rsus.truncate(1);
    let profile = scenario.actors.rsus[0].profile.clone();
    scenario.actors.rsus.push(v2xw_engine::scenario::Rsu {
        site: None,
        position_m: Some([1400.0, 1000.0, 0.0]),
        roles: vec!["report-forward".to_string()],
        profile,
        backhaul: None,
        backhaul_params: None,
    });
    let engine = Engine::build(scenario, "").expect("builds");
    let p = engine
        .phase2()
        .expect("a scenario with roadside units declares the Phase 2 path");
    let nodes = p.rsu_nodes().to_vec();
    assert_eq!(nodes.len(), 2, "both masts must have been created as nodes");

    assert_eq!(
        p.rsus_with_role("crl"),
        vec![nodes[0]],
        "only the unit that declares `crl` distributes a revocation list"
    );
    assert_eq!(
        p.rsus_with_role("report-forward"),
        nodes,
        "both units declare `report-forward`"
    );
    assert!(p.rsu_has_role(nodes[1], "report-forward"));
    assert!(
        !p.rsu_has_role(nodes[1], "crl"),
        "the second unit does not declare `crl` and must not be treated as if it did"
    );
    assert!(!p.rsu_has_role(v2xw_core::ids::NodeId::new(9_999), "crl"));
    assert!(p.rsu_spec_of(v2xw_core::ids::NodeId::new(9_999)).is_none());
}

/// The owner's default scenario runs the credential system: the SCMS backend, roadside
/// units, cellular access and the detection path, so the Backend view has something to
/// show. Built, not run: the run is the page's.
#[test]
fn the_default_scenario_runs_the_credential_system_with_roadside_units() {
    let s = rooted(
        Scenario::load(scenarios().join("manhattan-5min.yaml")).expect("the scenario loads"),
    );
    assert!(v2xw_engine::scenario::validate::validate(&s).is_empty());
    assert_eq!(
        s.security.protocol.as_ref().map(|p| p.id.as_str()),
        Some(v2xw_engine::phase2::CAMP_SCMS)
    );
    assert!(
        s.net.uu.is_some(),
        "the fleet reaches the backend over cellular"
    );
    let engine = Engine::build(s, "").expect("builds");
    let p = engine
        .phase2()
        .expect("the credential system runs in the default scenario");
    assert_eq!(p.rsu_nodes().len(), 8);
    assert_eq!(p.rsus_with_role("crl").len(), 8);
    assert!(p.detection_on());
}

/// The strategy study loads and builds under every strategy it sweeps, with and without a
/// silent period, and a strategy or a silent period the engine cannot act on is refused
/// rather than run as something else.
#[test]
fn the_strategy_study_builds_under_every_strategy_and_refuses_what_it_cannot_do() {
    let base = rooted(
        Scenario::load(scenarios().join("pseudonym-strategies.yaml")).expect("the study loads"),
    );
    for strategy in ["time", "distance", "c2c-cc", "mix-zone", "silent"] {
        for silent in [None, Some(vec![3.0, 13.0])] {
            let mut s = base.clone();
            s.security.pseudonym_change.strategy = strategy.to_string();
            s.security.pseudonym_change.silent_period_s = silent.clone();
            let errors = v2xw_engine::scenario::validate::validate(&s);
            assert!(errors.is_empty(), "{strategy} {silent:?}: {errors:?}");
            Engine::build(s, "").unwrap_or_else(|e| panic!("{strategy} {silent:?}: {e}"));
        }
    }
    let mut unknown = base.clone();
    unknown.security.pseudonym_change.strategy = "adaptive".to_string();
    assert!(!v2xw_engine::scenario::validate::validate(&unknown).is_empty());
    let mut backwards = base.clone();
    backwards.security.pseudonym_change.silent_period_s = Some(vec![13.0, 3.0]);
    assert!(!v2xw_engine::scenario::validate::validate(&backwards).is_empty());
    assert!(Engine::build(backwards, "").is_err());
}

#[test]
#[ignore = "diagnostic"]
fn diag_print_report() {
    let mut s = if std::env::var("FULL").is_ok() {
        shipped()
    } else {
        phase2()
    };
    if std::env::var("NO_ATTACKER").is_ok() {
        s.threats.attackers.clear();
    }
    let (report, _) = run(s);
    println!("{}", serde_json::to_string_pretty(&report.phase2).unwrap());
}
