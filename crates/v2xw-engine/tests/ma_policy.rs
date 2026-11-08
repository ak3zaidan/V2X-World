//! `security.scms.ma_policy` in the run.
//!
//! 1. **Default identity.** `ma_policy: {preset: cv-pilot-manual}` in place of
//!    `detection.ma` runs today's MA: the same record digest and run report on
//!    `credential-lifecycle` and `revocation-latency`. Fault: the preset's
//!    `event_min_reporters` is 3.
//! 3. **Per-provider policy.** provider-b on `pseudonym-k` with k = 1 and provider-a on the
//!    default: provider-b's MA decides on fewer than three reporters (one suffices),
//!    provider-a's never. Fault: the provider's policy falls back to the deployment's.
//! 6. **Refusals.** `detection.ma` with `ma_policy`; a provider naming no policy;
//!    `min_distinct_reporters: 0`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::report_records::MaReceived;
use v2xw_engine::{DigestRecorder, Engine, RunRecorder, Scenario};

fn shipped(name: &str) -> Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut s = Scenario::load(root.join("scenarios").join(name)).expect("the scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    let mut d = serde_json::to_value(&s).expect("serialises");
    if let Some(e) = d.get_mut("experiment") {
        *e = Value::Null;
    }
    d
}

fn scenario(d: Value) -> Scenario {
    let s = Scenario::from_document(d).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    s
}

/// The record digest and the run report.
fn digest(d: Value) -> (String, String) {
    let mut engine = Engine::build(scenario(d), "").expect("builds");
    let mut rec = DigestRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    assert!(
        report.phase2.reports_at_ma > 0,
        "no report reached the MA: the check could not see a decision rule"
    );
    (
        rec.digest_hex(),
        serde_json::to_string(&report).expect("serialises"),
    )
}

/// The shipped scenario with `detection.ma` replaced by `ma_policy`.
fn with_policy(mut d: Value, policy: Value) -> Value {
    d["detection"]["ma"] = Value::Null;
    if d["security"]["scms"].is_null() {
        d["security"]["scms"] = json!({});
    }
    d["security"]["scms"]["ma_policy"] = policy;
    d
}

fn assert_identity(mut d: Value) {
    if d["security"]["scms"].is_null() {
        d["security"]["scms"] = json!({});
    }
    let today = digest(d.clone());
    let policy = digest(with_policy(d, json!({"preset": "cv-pilot-manual"})));
    assert_eq!(today.0, policy.0, "the record digest moved");
    assert_eq!(today.1, policy.1, "the run report moved");
}

/// Test 1 on `credential-lifecycle` (compressed calendar, 5 % ConstPos liars), 200 s.
#[test]
fn default_preset_is_todays_ma_on_credential_lifecycle() {
    let mut d = shipped("credential-lifecycle.yaml");
    d["time"]["duration_s"] = json!(200.0);
    assert_identity(d);
}

/// Test 1 on `revocation-latency` (Manhattan, automatic MA), shortened to 300 s at
/// 1,500 veh/h for the debug build (150 s brought no report to the MA; 3,000 veh/h took
/// half an hour).
#[test]
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
fn default_preset_is_todays_ma_on_revocation_latency() {
    let mut d = shipped("revocation-latency.yaml");
    d["time"]["duration_s"] = json!(300.0);
    d["actors"]["vehicles"]["demand"]["rate_veh_per_h"] = json!(1500.0);
    d["threats"]["attackers"][0]["schedule"]["from_s"] = json!(30.0);
    assert_identity(d);
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

/// Test 3: per deciding MA (provider index), the fewest distinct reporters any decided
/// subject had when it was decided.
#[test]
#[allow(
    clippy::type_complexity,
    reason = "a one-off tuple read where it is built; an alias would hide its fields"
)]
fn a_providers_policy_decides_for_its_own_certificates_only() {
    let mut d = shipped("credential-lifecycle.yaml");
    d["time"]["duration_s"] = json!(200.0);
    d["detection"]["ma"] = Value::Null;
    d["security"]["scms"] = json!({
        "report_records": true,
        "pki_view": false,
        "ma_policies": {"single-report": {"preset": "pseudonym-k",
                                          "threshold": {"min_distinct_reporters": 1}}},
        "deployment": {"preset": "us-production-2025",
                       "providers": [{"name": "provider-b", "ma_policy": "single-report"}]}
    });
    let mut engine = Engine::build(scenario(d), "").expect("builds");
    let mut out = Out::default();
    engine.run(&mut out).expect("runs");
    let names: Vec<String> = engine
        .phase2()
        .expect("an SCMS run")
        .scms_runs()
        .first()
        .expect("a run")
        .state
        .deployment
        .providers
        .iter()
        .map(|p| p.name.clone())
        .collect();
    let b = names
        .iter()
        .position(|n| n == "provider-b")
        .expect("provider-b runs");
    // Per subject: its deciding MA and its reporters in arrival order.
    type About<'a> = (u32, Vec<(SimTime, &'a str)>);
    let mut about: BTreeMap<&str, About<'_>> = BTreeMap::new();
    for r in out.received.iter().filter(|r| r.set_aside.is_none()) {
        let e = about
            .entry(r.subject_cert.as_str())
            .or_insert((r.ma, Vec::new()));
        e.1.push((r.t, r.reporter_cert.as_str()));
    }
    let mut fewest: BTreeMap<u32, usize> = BTreeMap::new();
    for (t, subject) in &out.decisions {
        let Some((ma, rows)) = about.get(subject.as_str()) else {
            continue;
        };
        let k: BTreeSet<&str> = rows
            .iter()
            .filter(|(at, _)| at <= t)
            .map(|(_, r)| *r)
            .collect();
        let f = fewest.entry(*ma).or_insert(usize::MAX);
        *f = (*f).min(k.len());
    }
    println!("providers {names:?} fewest reporters per deciding MA {fewest:?}");
    // The decision record is written when the case reaches its decision stage, a few
    // backend round trips after the tipping report, so a report or two more may have
    // arrived by then; the default rule can never decide on fewer than three.
    assert!(
        fewest.get(&(b as u32)).is_some_and(|k| *k < 3),
        "provider-b's MA ({b}) must decide on fewer than three reporters: {fewest:?}"
    );
    for (ma, k) in &fewest {
        if *ma != b as u32 {
            assert!(
                *k >= 3,
                "provider {ma} runs the default MA, yet decided on {k}"
            );
        }
    }
}

/// Test 6: the loader refuses the conflicts, each at its path.
#[test]
fn conflicts_are_refused() {
    let base = shipped("credential-lifecycle.yaml");
    let refused = |scms: Value, keep_detection_ma: bool| {
        let mut d = base.clone();
        if !keep_detection_ma {
            d["detection"]["ma"] = Value::Null;
        }
        d["security"]["scms"] = scms;
        let s = Scenario::from_document(d).expect("deserialises");
        v2xw_engine::scenario::validate(&s)
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
    };
    let cases = [
        (
            json!({"ma_policy": {"preset": "corroborated-auto"}}),
            true,
            "security.scms.ma_policy",
            "detection.ma both choose",
        ),
        (
            json!({"deployment": {"preset": "us-production-2025",
                "providers": [{"name": "provider-b", "ma_policy": "strict-k5"}]}}),
            false,
            "security.scms.deployment.providers.provider-b.ma_policy",
            "names neither a preset nor a policy",
        ),
        (
            json!({"ma_policy": {"threshold": {"min_distinct_reporters": 0}}}),
            false,
            "security.scms.ma_policy.threshold.min_distinct_reporters",
            "outside the allowed range",
        ),
    ];
    for (scms, keep, path, why) in cases {
        let got = refused(scms.clone(), keep);
        assert!(
            got.iter().any(|m| m.starts_with(path) && m.contains(why)),
            "{scms} must be refused at {path} ({why}); got {got:#?}"
        );
    }
    assert!(
        refused(json!({"ma_policy": {"preset": "catch"}}), false).is_empty(),
        "a preset alone loads"
    );
}
