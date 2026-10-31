//! An MA decision rule written in Python (docs/design/scms-real/08-settings-ux-and-gaps.md
//! §7, accept test 7; piece B4).
//!
//! 7. **Python.** A Python port of the `pseudonym-k` preset (k = 3 distinct reporters per
//!    pseudonym, every report counted, no trust gate) takes the same revocation decisions,
//!    at the same instants, as the Rust preset on `credential-lifecycle`, both through
//!    `Engine::attach_ma_policy` and through `v2xw.run(..., ma_policy=obj)`. Fault: the
//!    adapter drops the last row of each batch.
//!
//! The cheap checks beside it: what the adapter refuses, the batch's columns, and a rule
//! that raises.

use std::path::{Path, PathBuf};

use pyo3::prelude::*;
use pyo3::types::PyDict;
use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_engine::ma_hook::{MaHookCall, MaHookRow, MaPolicyHook};
use v2xw_engine::report_records::MaReceived;
use v2xw_engine::{Engine, RunRecorder, Scenario};

const BUILD_UTC: &str = "2026-10-30T00:00:00Z";

/// The port, as a researcher would write it in a module file.
const PORT: &str = r#"
CARD = {
    "id": "threat/ma/python-pseudonym-k",
    "family": "ma-pipeline",
    "version": "0.1.0",
    "api_version": "1.0.0",
    "tier": ["medium"],
    "purpose": "The pseudonym-k preset in Python: revoke a pseudonym once k distinct reporters accused it.",
    "equations": [],
    "parameters": [],
    "determinism": {"uses_rng": False},
}

class PseudonymK:
    card = CARD

    def __init__(self, k=3):
        self.k = k
        self.reporters = {}
        self.revoked = set()
        self.calls = 0
        self.rows = 0

    def decide(self, ctx, batch):
        self.calls += 1
        self.rows += len(batch)
        subjects = batch.column("subject_cert")
        reporters = batch.column("reporter_cert")
        out = []
        for i in range(len(batch)):
            key = (ctx.ma, subjects[i])
            if key in self.revoked:
                continue
            seen = self.reporters.setdefault(key, set())
            seen.add(reporters[i])
            if len(seen) >= self.k:
                self.revoked.add(key)
                out.append((subjects[i], True, i))
        return out

class Raises:
    card = CARD

    def decide(self, ctx, batch):
        raise ValueError("no rule here")

class NoDecide:
    card = CARD

class WrongFamily:
    card = dict(CARD, family="detector")

    def decide(self, ctx, batch):
        return []

class Shape:
    card = CARD

    def decide(self, ctx, batch):
        rows = batch.rows()
        return [(rows[-1]["subject_cert"], True)] if rows else []
"#;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// The port's module, written to a file and imported, so its source can be pinned.
fn port<'py>(py: Python<'py>) -> Bound<'py, PyModule> {
    let dir = std::env::temp_dir().join(format!("v2xw-ma-port-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("ma_port.py"), PORT).unwrap();
    let sys = py.import("sys").unwrap();
    sys.getattr("path")
        .unwrap()
        .call_method1("insert", (0, dir.display().to_string()))
        .unwrap();
    py.import("ma_port").unwrap()
}

fn object<'py>(py: Python<'py>, class: &str) -> Bound<'py, PyAny> {
    port(py).getattr(class).unwrap().call0().unwrap()
}

fn module(py: Python<'_>) -> Bound<'_, PyModule> {
    _v2xw::build_module(py).expect("the native module builds")
}

/// `credential-lifecycle` (compressed calendar, automatic MA, 5 % ConstPos liars), 200 s,
/// with no `detection.ma`.
fn shipped() -> Value {
    let mut s = Scenario::load(root().join("scenarios").join("credential-lifecycle.yaml"))
        .expect("the scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root().join(&*path).to_string_lossy().into_owned();
    }
    let mut d = serde_json::to_value(&s).expect("serialises");
    d["experiment"] = Value::Null;
    d["time"]["duration_s"] = json!(200.0);
    d["detection"]["ma"] = Value::Null;
    if d["security"]["scms"].is_null() {
        d["security"]["scms"] = json!({});
    }
    d
}

fn scenario(d: Value) -> Scenario {
    let s = Scenario::from_document(d).expect("deserialises");
    let errors = v2xw_engine::scenario::validate(&s);
    assert!(errors.is_empty(), "{errors:?}");
    s
}

#[derive(Default)]
struct Decisions(Vec<(SimTime, String)>);

impl RunRecorder for Decisions {
    fn write(&mut self, at: SimTime, r: &OwnedRecord) {
        if &*r.channel == "ma.decision" {
            let v: Value = serde_json::from_slice(&r.json).expect("ma.decision parses");
            if v["decision"] == "revoke" {
                self.0
                    .push((at, v["subject"].as_str().unwrap_or_default().to_string()));
            }
        }
    }
}

/// The counters a decision rule moves.
fn counters(r: &v2xw_engine::RunReport) -> [u64; 4] {
    let p = &r.phase2;
    [
        p.reports_at_ma,
        p.ma_revoke_decisions,
        p.decisions_linked,
        p.cases_opened,
    ]
}

/// Test 7.
#[test]
fn a_python_pseudonym_k_decides_as_the_rust_preset() {
    pyo3::prepare_freethreaded_python();
    // The Rust preset.
    let mut d = shipped();
    d["security"]["scms"]["ma_policy"] = json!({"preset": "pseudonym-k"});
    let mut engine = Engine::build(scenario(d), "").expect("builds");
    let mut rust = Decisions::default();
    let rust_report = engine.run(&mut rust).expect("runs");
    assert!(
        !rust.0.is_empty(),
        "the Rust preset revoked nobody: the comparison could not see a decision"
    );

    Python::with_gil(|py| {
        // The port, through the engine's seam.
        let obj = object(py, "PseudonymK");
        let hook = _v2xw::ma_policy::PyMaPolicy::attach(obj.clone()).expect("attaches");
        let sha = hook.source_sha256().to_string();
        assert_eq!(sha, v2xw_core::hash::sha256_hex(PORT.as_bytes()));
        let mut engine = Engine::build(scenario(shipped()), "").expect("builds");
        let status = engine.attach_ma_policy(Box::new(hook)).expect("hosts it");
        let pin = engine
            .manifest()
            .plugins
            .iter()
            .find(|p| p.id == "ma_policy:python:ma_port.PseudonymK")
            .cloned()
            .expect("the manifest pins the rule");
        assert_eq!(
            (pin.version.as_str(), pin.content_hash.as_str()),
            ("0.1.0", sha.as_str())
        );
        let mut python = Decisions::default();
        let python_report = engine.run(&mut python).expect("runs");
        assert_eq!(status.error(), None, "the rule raised");
        let calls: u64 = obj.getattr("calls").unwrap().extract().unwrap();
        assert!(
            calls > 0 && calls == status.calls(),
            "{calls} vs {}",
            status.calls()
        );
        // Every report the MA took as evidence reached the rule, once.
        let rows: u64 = obj.getattr("rows").unwrap().extract().unwrap();
        let p = &python_report.phase2;
        let evidence =
            p.reports_at_ma - p.reports_evidence_unverifiable - p.reports_evidence_time_implausible;
        println!(
            "rust {} decisions, python {} ({calls} calls, {rows} rows of {evidence} evidence), \
             counters {:?}",
            rust.0.len(),
            python.0.len(),
            counters(&python_report)
        );
        assert_eq!(
            rows, evidence,
            "the rule did not see every report the MA took"
        );
        assert_eq!(rust.0, python.0, "the decisions differ");
        assert_eq!(counters(&rust_report), counters(&python_report));

        // The same through `v2xw.run(..., ma_policy=obj)`, read back from its recording.
        let dir = std::env::temp_dir().join(format!("v2xw-ma-run-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("run.mcap");
        let m = module(py);
        let json_mod = py.import("json").unwrap();
        let doc = json_mod
            .call_method1("loads", (shipped().to_string(),))
            .unwrap();
        let sc = m
            .getattr("Scenario")
            .unwrap()
            .call_method1("from_dict", (doc,))
            .unwrap();
        let kwargs = PyDict::new(py);
        kwargs.set_item("build_utc", BUILD_UTC).unwrap();
        kwargs.set_item("metrics", false).unwrap();
        kwargs
            .set_item("recording", path.display().to_string())
            .unwrap();
        kwargs
            .set_item("ma_policy", object(py, "PseudonymK"))
            .unwrap();
        let run = m
            .call_method("run", (&sc,), Some(&kwargs))
            .unwrap_or_else(|e| {
                e.print(py);
                panic!("v2xw.run must complete");
            });
        let plugins: Vec<String> = run
            .getattr("manifest")
            .unwrap()
            .get_item("plugins")
            .unwrap()
            .try_iter()
            .unwrap()
            .map(|p| p.unwrap().get_item("id").unwrap().extract().unwrap())
            .collect();
        assert!(
            plugins
                .iter()
                .any(|p| p == "ma_policy:python:ma_port.PseudonymK"),
            "{plugins:?}"
        );
        let rec = m
            .getattr("Recording")
            .unwrap()
            .call_method1("open", (path.display().to_string(),))
            .unwrap();
        let rows = rec.call_method1("records", ("ma.decision",)).unwrap();
        let mut recorded = Vec::new();
        for row in rows.try_iter().unwrap() {
            let row = row.unwrap();
            let r = row.get_item("record").unwrap();
            if r.get_item("decision").unwrap().extract::<String>().unwrap() == "revoke" {
                recorded.push((
                    row.get_item("sim_time_ns")
                        .unwrap()
                        .extract::<u64>()
                        .unwrap(),
                    r.get_item("subject").unwrap().extract::<String>().unwrap(),
                ));
            }
        }
        assert_eq!(rust.0, recorded, "v2xw.run's decisions differ");
        let _ = std::fs::remove_dir_all(&dir);
    });
}

/// What the adapter and the seam refuse, each by name.
#[test]
fn the_adapter_and_the_seam_refuse_what_cannot_decide() {
    pyo3::prepare_freethreaded_python();
    Python::with_gil(|py| {
        let refused = |class: &str| {
            _v2xw::ma_policy::PyMaPolicy::attach(object(py, class))
                .map(|_| ())
                .unwrap_err()
                .to_string()
        };
        assert!(refused("NoDecide").contains("no callable `decide"));
        assert!(refused("WrongFamily").contains("declares family detector"));
        // A rule typed into a prompt has no source to pin.
        let globals = PyDict::new(py);
        let code = std::ffi::CString::new(PORT.replace("class Shape", "class Typed")).unwrap();
        py.run(&code, Some(&globals), None).unwrap();
        let typed = globals.get_item("Typed").unwrap().unwrap().call0().unwrap();
        let e = _v2xw::ma_policy::PyMaPolicy::attach(typed)
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(e.contains("cannot be read"), "{e}");

        // A run with no SCMS has no MA to decide for.
        let mut engine = Engine::build(Scenario::minimal(), "").expect("builds");
        let hook = _v2xw::ma_policy::PyMaPolicy::attach(object(py, "PseudonymK")).unwrap();
        let e = engine
            .attach_ma_policy(Box::new(hook))
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(e.contains("no SCMS deployment"), "{e}");
        // A scenario that already chooses the rule.
        let mut d = shipped();
        d["security"]["scms"]["ma_policy"] = json!({"preset": "catch"});
        let mut engine = Engine::build(scenario(d), "").expect("builds");
        let hook = _v2xw::ma_policy::PyMaPolicy::attach(object(py, "PseudonymK")).unwrap();
        let e = engine
            .attach_ma_policy(Box::new(hook))
            .map(|_| ())
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("security.scms.ma_policy") && e.contains("both choose"),
            "{e}"
        );
    });
}

fn row(i: u32, subject: &str) -> MaHookRow {
    MaHookRow {
        view: MaReceived {
            t: 1_000 + u64::from(i),
            report_id: format!("r-{i}"),
            ma: 0,
            reporter_cert: format!("rep-{i}"),
            subject_cert: subject.to_string(),
            subtypes: vec!["constantPositionFrozen".to_string()],
            generation_time: 900,
            evidence_start: 800,
            evidence_end: 850,
            excess: 1.5,
            pos_accuracy_m: 2.0,
            secmark_phase_ms: 7,
            issuer: "provider-0".to_string(),
            la_link: "link".to_string(),
            set_aside: None,
        },
        observed: 900 + u64::from(i),
        reporter_infrastructure: i == 1,
    }
}

/// Every row of a call reaches the rule, in order, with every column; a raising rule's
/// traceback comes back as the error.
#[test]
fn the_batch_carries_every_row_and_a_raise_is_an_error() {
    pyo3::prepare_freethreaded_python();
    let rows = [row(0, "a"), row(1, "b"), row(2, "c")];
    let call = MaHookCall {
        t: 5_000,
        ma: 0,
        provider: "provider-a",
        rows: &rows,
    };
    Python::with_gil(|py| {
        let mut shape = _v2xw::ma_policy::PyMaPolicy::attach(object(py, "Shape")).unwrap();
        let v = shape.decide(&call).expect("decides");
        assert_eq!(v.len(), 1);
        assert_eq!(
            (v[0].subject.as_str(), v[0].revoke, v[0].row),
            ("c", true, None)
        );

        let batch = _v2xw::ma_policy::MaBatch::new(rows.to_vec());
        let b = batch.arrow();
        assert_eq!(b.num_rows(), 3);
        let names: Vec<String> = b
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().clone())
            .collect();
        assert_eq!(names, _v2xw::ma_policy::COLUMNS.to_vec());
        let obj = Py::new(py, batch).unwrap().into_bound(py);
        let infra: Vec<bool> = obj
            .call_method1("column", ("reporter_infrastructure",))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(infra, vec![false, true, false]);
        let subtypes: Vec<Vec<String>> = obj
            .call_method1("column", ("subtypes",))
            .unwrap()
            .extract()
            .unwrap();
        assert_eq!(subtypes[2], vec!["constantPositionFrozen".to_string()]);
        assert!(obj.call_method1("column", ("truth",)).is_err());

        let mut raises = _v2xw::ma_policy::PyMaPolicy::attach(object(py, "Raises")).unwrap();
        let e = raises.decide(&call).unwrap_err();
        assert!(e.contains("no rule here") && e.contains("decide"), "{e}");
    });
}
