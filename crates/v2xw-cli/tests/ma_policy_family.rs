//! The MA policy family offline and online.
//!
//! 2. **Online and offline agree before the first revocation.** For each of the eight
//!    presets, the run's first `ma.decision` is about the pseudonym of the first decision
//!    `ma_replay::replay_policy` takes over that run's recorded `ma.received` rows, stamped
//!    at the backend step that processed the replayed decision's tipping report. Fault: the replay dates evidence by its arrival rather than its observation.
//!    The fleet has no roadside unit, whose reports the online MA trusts as
//!    infrastructure and an `ma.received` row does not mark.
//! 5. **Sweep report.** A sweep over three policies × two seeds writes six run rows and
//!    three summaries; on an honest-only fleet no attacker is revoked and every revoked
//!    device is counted as honest. Fault: the honest join keys on the pseudonym rather
//!    than the device.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::report_records::MaReceived;
use v2xw_engine::scenario::Experiment;
use v2xw_engine::{Engine, RunRecorder, Scenario};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives two levels below the workspace root")
        .to_path_buf()
}

/// `credential-lifecycle` with the MA chosen by `ma_policy` and the MA's view recorded.
fn base(duration_s: f64) -> Value {
    let s = Scenario::load(repo_root().join("scenarios/credential-lifecycle.yaml"))
        .expect("the scenario loads");
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["time"]["duration_s"] = json!(duration_s);
    d["detection"]["ma"] = Value::Null;
    d["experiment"] = Value::Null;
    d["security"]["scms"] = json!({"report_records": true, "pki_view": false,
                                   "ma_policy": {"preset": "cv-pilot-manual"}});
    d
}

#[derive(Default)]
struct Out {
    received: Vec<MaReceived>,
    decisions: Vec<(SimTime, String)>,
}

impl RunRecorder for Out {
    fn write(&mut self, at: SimTime, r: &OwnedRecord) {
        match r.channel {
            "ma.received" => self
                .received
                .push(serde_json::from_slice(&r.json).expect("ma.received parses")),
            "ma.decision" => {
                let v: Value = serde_json::from_slice(&r.json).expect("ma.decision parses");
                if v["decision"] == "revoke" {
                    let s = v["subject"].as_str().unwrap_or_default().to_string();
                    self.decisions.push((at, s));
                }
            }
            _ => {}
        }
    }
}

/// Test 2 over `presets`: how many decided anything, and what each saw.
fn first_decisions_agree(presets: &[&str]) -> (usize, Vec<String>) {
    let mut decided = 0;
    let mut seen = Vec::new();
    for preset in presets {
        let mut d = base(200.0);
        let step_ns = d["time"]["mobility_step_ms"].as_u64().unwrap_or(100) * 1_000_000;
        d["security"]["scms"]["ma_policy"]["preset"] = json!(preset);
        // No roadside unit: the online MA trusts a unit's reports as infrastructure, and
        // the `ma.received` row does not say a reporter is one, so a replay cannot.
        d["actors"]["rsus"] = json!([]);
        let s = Scenario::from_document(d).expect("deserialises");
        let errors = v2xw_engine::scenario::validate(&s);
        assert!(errors.is_empty(), "{preset}: {errors:?}");
        let mut engine = Engine::build(s.clone(), "").expect("builds");
        let mut out = Out::default();
        engine.run(&mut out).expect("runs");
        // The same policy, resolved as the replay resolves it.
        let mut one = s.clone();
        let mut sweep = BTreeMap::new();
        sweep.insert(
            v2xw_engine::ma_policy::SWEEP_AXIS.to_string(),
            vec![json!(preset)],
        );
        one.experiment = Some(Experiment {
            sweep,
            seeds: Vec::new(),
            replications: 1,
        });
        let (_, policy) = v2xw_engine::ma_policy::family_of(&one)
            .expect("resolves")
            .remove(0);
        let offline = v2xw_cli::ma_replay::replay_policy(&policy, &out.received);
        let online = out.decisions.first().cloned();
        let replayed = offline.first().map(|x| (x.t, x.subject.clone()));
        seen.push(format!("{preset}: online {online:?} offline {replayed:?}"));
        // The same pseudonym; and the run stamps `ma.decision` at the backend step that
        // processed the tipping report, the first step boundary at or after its arrival.
        match (&online, &replayed) {
            (Some((on, a)), Some((off, b))) => {
                assert_eq!(a, b, "{preset}: the first decision's subject differs");
                assert!(
                    *on >= *off && on - off < step_ns,
                    "{preset}: decided at {off} offline, stamped {on} online"
                );
            }
            (None, None) => {}
            _ => {
                panic!("{preset}: one side decided and the other did not: {online:?} {replayed:?}")
            }
        }
        decided += usize::from(online.is_some());
    }
    println!("{seen:#?}");
    (decided, seen)
}

/// Test 2, the first four presets (today's MA family and the legacy gate), so the eight
/// runs take two test threads.
#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn online_and_offline_agree_until_the_first_decision_corroborating() {
    let (decided, seen) = first_decisions_agree(&v2xw_engine::ma_policy::PRESET_IDS[..4]);
    assert!(decided >= 2, "too few presets decided anything: {seen:#?}");
}

/// Test 2, the other four (the replay family and `custom`).
#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn online_and_offline_agree_until_the_first_decision_replay_family() {
    let (decided, seen) = first_decisions_agree(&v2xw_engine::ma_policy::PRESET_IDS[4..]);
    assert!(decided >= 2, "too few presets decided anything: {seen:#?}");
}

fn out_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir()
        .join("v2xw-ma-policy-family")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the test's own directory");
    dir
}

/// Test 5.
#[test]
#[ignore = "slow: whole runs; CI runs it in the slow-test job (tools/ci/slow-tests.sh)"]
fn the_sweep_reports_each_policy_and_seed() {
    let dir = out_dir("sweep");
    let mut d = base(120.0);
    // Honest only. Benign GNSS outliers (one per 5 s of driving, 30 m) make honest
    // vehicles' position checks fire, so a single-report policy has someone to revoke:
    // 120 s of the default receiver brought no report to the MA.
    d["threats"]["attackers"] = json!([]);
    d["actors"]["vehicles"]["demand"]["rate_veh_per_h"] = json!(2000.0);
    d["actors"]["vehicles"]["gnss"] = json!({"outlier_rate_hz": 0.2, "outlier_magnitude_m": 30.0});
    d["security"]["scms"]["ma_policies"] = json!({"single-report": {"preset": "pseudonym-k",
        "threshold": {"min_distinct_reporters": 1}}});
    let mut s = Scenario::from_document(d).expect("deserialises");
    let mut sweep = BTreeMap::new();
    sweep.insert(
        v2xw_engine::ma_policy::SWEEP_AXIS.to_string(),
        vec![
            json!("cv-pilot-manual"),
            json!("corroborated-auto"),
            json!("single-report"),
        ],
    );
    s.experiment = Some(Experiment {
        sweep,
        seeds: vec![1, 2],
        replications: 1,
    });
    // The point the sweep runs would otherwise not record the MA's view.
    s.security.scms.as_mut().expect("scms").report_records = None;
    let path = dir.join("family.yaml");
    std::fs::write(&path, s.to_yaml().expect("serialises")).expect("writes");
    let out = dir.join("out");
    let options = v2xw_cli::experiment::ExperimentOptions {
        scenario: path,
        out: Some(out.clone()),
        build_utc: Some("2026-10-29T00:00:00Z".to_string()),
        // One at a time: this suite runs on the same 8 GB machine everything else does.
        concurrency: 1,
        level: v2xw_metrics::ConfidenceLevel::P95,
        keyframe_ms: 1000,
        // The report reads each run's recording.
        record: true,
        node_only: false,
        verify: false,
        format: None,
        resume: false,
    };
    let done = v2xw_cli::experiment::run(&options).expect("the sweep runs");
    assert!(
        done.outcome
            .files
            .iter()
            .any(|f| f == "ma-policy-family.csv"),
        "{:?}",
        done.outcome.files
    );
    let csv = std::fs::read_to_string(out.join("ma-policy-family.csv")).expect("the report");
    println!("{csv}");
    let header: Vec<&str> = csv.lines().next().expect("a header").split(',').collect();
    let col = |name: &str| header.iter().position(|h| *h == name).expect(name);
    let rows: Vec<Vec<&str>> = csv
        .lines()
        .skip(1)
        .map(|l| l.split(',').collect())
        .collect();
    let kind = |k: &str| rows.iter().filter(|r| r[1] == k).count();
    assert_eq!(
        (kind("run"), kind("mean"), kind("ci95")),
        (6, 3, 3),
        "{csv}"
    );
    let num = |r: &Vec<&str>, c: &str| r[col(c)].parse::<f64>().unwrap_or(0.0);
    let runs: Vec<&Vec<&str>> = rows.iter().filter(|r| r[1] == "run").collect();
    assert!(
        runs.iter().all(|r| num(r, "attackers_revoked") == 0.0),
        "{csv}"
    );
    let single: f64 = runs
        .iter()
        .filter(|r| r[0] == "single-report")
        .map(|r| num(r, "honest_revoked"))
        .sum();
    assert!(
        single > 0.0,
        "a single-report policy revoked no honest device: {csv}"
    );
    let md = std::fs::read_to_string(out.join("ma-policy-family.md")).expect("the summary");
    assert!(md.contains("report_records: true"), "{md}");
    // Six recordings of about 480 MB each.
    let _ = std::fs::remove_dir_all(&dir);
}
