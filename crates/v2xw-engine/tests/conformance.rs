//! WP-F1: the SCMS conformance run (docs/design/scms-real/00 §4.2).
//!
//! One run of `scenarios/scms-conformance.yaml`: the default deployment (two providers on
//! one CTL), the real US calendar with fast-forward, every lifecycle number cited, real
//! cryptography. The test checks that the run walks every step the package names, from
//! the engine's own report and the providers' records:
//!
//! 1. factory enrolment: every vehicle and roadside unit is enrolled by its own
//!    provider's ECA through real `EeEcaCertRequestSpdu` / `EcaEeCertResponseSpdu`;
//! 2. provisioning and a week rollover: pseudonym batches for i-period 614, and top-ups
//!    for the periods after the Tuesday 2026-10-20 09:00 UTC rollover;
//! 3. RSU application-certificate renewal over the backhaul;
//! 4. a misbehaviour report (a real SM-MBR), an MA case, a published CRL, and the revoked
//!    liars' frames refused by vehicles of both providers;
//! 5. a CTL change: the home root rollover's CTL published by the electors;
//!
//! and that the capture is complete: every SPDU each provider's run put on a link is in
//! the `pki.flow` recording exactly once, with its bytes.
//!
//! With `V2XW_CONFORMANCE_OUT=<dir>` it writes the capture for the independent oracle:
//!
//! ```text
//! V2XW_CONFORMANCE_OUT=<dir> cargo test -p v2xw-engine --test conformance
//! <oracle venv>/bin/python -I tools/conformance/check_run.py <dir>
//! ```
//!
//! * `pdus.jsonl`: every SPDU of every `pki.flow` record, with its bytes (the capture);
//! * `certs.json`: every certificate the recording carried (`pki.state`,
//!   `node.credentials`), HashedId8 → COER hex;
//! * `crls.json`: every CRL head the recording carried, with its bytes;
//! * `gov.json`: per provider, every CTL (history, current, pending), chain and policy
//!   file SPDU, and the elector quorum;
//! * `air.jsonl`: a sample of the 1609.2 SPDUs vehicles and units put on the air (one
//!   per node per 5 s);
//! * `keys.json`: the private keys a recipient opens its PDUs with (RA and MA
//!   encryption keys, the LA–ACA keys of every epoch a PLV was sealed in, each device's
//!   caterpillar, each application certificate's response key). **Oracle use only.**
//! * `truth.json`: the engine's own view (device → provider, liars, revoked devices,
//!   per-run PDU counts), which the oracle compares with what it derived on its own.
//!
//! The oracle is `tools/conformance/check_run.py`; the faults injected to prove that it
//! and this test can go red are listed in `docs/design/scms-real/06-conformance-run.md`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_engine::phase2::SCMS_DEVICE_BASE;
use v2xw_engine::{Engine, RunRecorder, RunReport, Scenario};
use v2xw_proto::scms::pdu::SpduKind;
use v2xw_proto::scms::pki_view::{PduRef, PkiFlow};

const S: u64 = 1_000_000_000;

fn scenario() -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Scenario::load(root.join("scenarios/scms-conformance.yaml")).expect("the scenario loads")
}

/// Keeps what the oracle needs from the PKI view's records as they arrive (every SPDU
/// of `pki.flow`, the certificates and CRL heads of `pki.state` and `node.credentials`)
/// and a sample of the air taps; drops everything else, so a five-day run stays small.
#[derive(Default)]
struct Capture {
    pdus: Vec<PduRef>,
    certs: BTreeMap<String, Value>,
    crls: BTreeMap<String, Value>,
    states: u64,
    air: Vec<(u64, u32, u64, Vec<u8>)>,
    last_tap: BTreeMap<u32, u64>,
}

impl RunRecorder for Capture {
    fn write(&mut self, _at: SimTime, record: &OwnedRecord) {
        match record.channel {
            "pki.flow" => {
                let f: PkiFlow = serde_json::from_slice(&record.json).expect("pki.flow");
                self.pdus.extend(f.pdus);
            }
            "pki.state" | "node.credentials" => {
                let mut v: Value = serde_json::from_slice(&record.json).expect("a record is JSON");
                if let Some(Value::Object(m)) = v.get_mut("certificates").map(Value::take) {
                    self.certs.extend(m);
                }
                if record.channel == "pki.state" {
                    self.states += 1;
                    for head in v["crls"].as_array().into_iter().flatten() {
                        if let Some(id8) = head["id8"].as_str()
                            && head["hex"].is_string()
                        {
                            self.crls.insert(id8.to_string(), head.clone());
                        }
                    }
                }
            }
            _ => {}
        }
    }
    fn tap_frame(&mut self, at: SimTime, node: NodeId, msg: u64, spdu: &[u8]) {
        // One frame per node per 5 s.
        let due = self
            .last_tap
            .get(&node.index())
            .is_none_or(|t| at >= t + 5 * S);
        if due {
            self.last_tap.insert(node.index(), at);
            self.air.push((at, node.index(), msg, spdu.to_vec()));
        }
    }
}

struct Outcome {
    report: RunReport,
    capture: Capture,
    /// Per provider run: its label, the SPDUs it emitted (kind names), its refusals.
    /// Per provider: its label, the kind of every SPDU emitted on a leg the deployment
    /// runs, its rejections, and the kinds emitted on a folded LOP-RA leg (none deployed).
    runs: Vec<(String, Vec<&'static str>, usize, Vec<&'static str>)>,
    /// The flow records' SPDUs.
    pdus: Vec<PduRef>,
    /// Device node → provider index.
    providers: BTreeMap<u32, usize>,
    /// Liar nodes.
    liars: BTreeSet<u32>,
    /// Per provider: CTL sequence numbers the electors published, roots added.
    ctls: Vec<(Vec<u32>, u32)>,
    /// Per provider: (device, period) of every RSU application certificate installed.
    app_certs: Vec<Vec<(u32, u32)>>,
    /// Per provider: the highest i-period each vehicle holds a pseudonym for.
    max_i: Vec<BTreeMap<u32, u32>>,
    /// Liars the MA decided to revoke.
    revoked: BTreeSet<u32>,
    /// The capture written for the oracle, if asked.
    out: Option<PathBuf>,
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A P-256 scalar as 32 bytes of hex (a macro: this crate does not depend on `p256`).
macro_rules! scalar {
    ($x:expr) => {
        hex(&v2xw_sec::ec::scalar_to_be32($x))
    };
}

fn outcome() -> &'static Outcome {
    static RUN: OnceLock<Outcome> = OnceLock::new();
    RUN.get_or_init(|| {
        let s = scenario();
        let mut engine = Engine::build(s, "").expect("builds");
        let mut capture = Capture::default();
        let report = engine.run(&mut capture).expect("runs");
        let p2 = engine.phase2().expect("an SCMS run");
        let liars: BTreeSet<u32> = report
            .phase2
            .attacker_outcomes
            .iter()
            .map(|a| a.node)
            .collect();
        let runs = p2
            .scms_runs()
            .iter()
            .map(|r| {
                // Without a deployed LOP the transparent LOP-RA hop is not a message of
                // the deployment; its SPDUs are twins of the device leg's.
                let (lop, ra) = (r.state.nodes.lop.index(), r.state.nodes.ra.index());
                let folded = |p: &v2xw_proto::scms::pdu::EmittedPdu| {
                    !r.state.deployment.options.lop
                        && ((p.from == lop && p.to == ra) || (p.from == ra && p.to == lop))
                };
                (
                    v2xw_proto::scms::pki_view::provider_label(r),
                    r.state.pdus.iter().filter(|p| !folded(p)).map(|p| p.kind.name()).collect(),
                    r.state.pdu_rejections.len(),
                    r.state.pdus.iter().filter(|p| folded(p)).map(|p| p.kind.name()).collect(),
                )
            })
            .collect();
        let pdus = std::mem::take(&mut capture.pdus);
        let mut providers = BTreeMap::new();
        for (k, r) in p2.scms_runs().iter().enumerate() {
            for n in r.state.devices.keys() {
                providers.insert(n.index(), k);
            }
        }
        let ctls = p2
            .scms_runs()
            .iter()
            .map(|r| {
                let g = &r.state.gov;
                (
                    g.ctl_history
                        .iter()
                        .chain(std::iter::once(&g.ctl))
                        .map(|c| c.sequence)
                        .collect(),
                    g.counters.roots_added,
                )
            })
            .collect();
        let app_certs = p2
            .scms_runs()
            .iter()
            .map(|r| {
                r.state
                    .apps
                    .devices
                    .iter()
                    .flat_map(|(n, d)| d.installed.iter().map(|c| (n.index(), c.cred.period)))
                    .collect()
            })
            .collect();
        let max_i = p2
            .scms_runs()
            .iter()
            .map(|r| {
                r.state
                    .devices
                    .iter()
                    .filter_map(|(n, d)| {
                        d.credentials
                            .keys()
                            .map(|(i, _)| *i)
                            .max()
                            .map(|i| (n.index(), i))
                    })
                    .collect()
            })
            .collect();
        let revoked = report
            .phase2
            .attacker_outcomes
            .iter()
            .filter(|a| a.decided_ns.is_some())
            .map(|a| a.node)
            .collect();
        let out = std::env::var_os("V2XW_CONFORMANCE_OUT").map(PathBuf::from);
        let o = Outcome {
            report,
            capture,
            runs,
            pdus,
            providers,
            liars,
            ctls,
            app_certs,
            max_i,
            revoked,
            out,
        };
        if let Some(dir) = &o.out {
            write_capture(&o, &engine, dir);
        }
        o
    })
}

fn write_capture(o: &Outcome, engine: &Engine, dir: &Path) {
    std::fs::create_dir_all(dir).expect("creates the capture directory");
    let p2 = engine.phase2().expect("an SCMS run");
    let lines =
        |rows: Vec<Value>| -> String { rows.iter().map(|r| format!("{r}\n")).collect::<String>() };
    std::fs::write(
        dir.join("pdus.jsonl"),
        lines(o.pdus.iter().map(|p| json!(p)).collect()),
    )
    .expect("writes pdus.jsonl");
    let write = |name: &str, v: &Value| {
        std::fs::write(
            dir.join(name),
            serde_json::to_string_pretty(v).expect("json"),
        )
        .expect("writes the capture");
    };
    write("certs.json", &json!(o.capture.certs));
    write(
        "crls.json",
        &json!(o.capture.crls.values().collect::<Vec<_>>()),
    );
    std::fs::write(
        dir.join("air.jsonl"),
        lines(
            o.capture
                .air
                .iter()
                .map(|(t, n, m, b)| json!({"t": t, "node": n, "msg": m, "hex": hex(b)}))
                .collect(),
        ),
    )
    .expect("writes air.jsonl");
    let mut gov = Vec::new();
    let mut keys = Vec::new();
    for r in p2.scms_runs() {
        let st = &r.state;
        let label = v2xw_proto::scms::pki_view::provider_label(r);
        let g = &st.gov;
        gov.push(json!({
            "provider": label,
            "ctls": g.ctl_history.iter().chain(std::iter::once(&g.ctl))
                .map(|c| json!({"sequence": c.sequence, "hex": hex(&c.spdu)}))
                .collect::<Vec<_>>(),
            "pending": g.pending.iter()
                .map(|(t, c)| json!({"at": t, "sequence": c.sequence, "hex": hex(&c.spdu)}))
                .collect::<Vec<_>>(),
            "gccf": hex(&g.gccf.spdu),
            "lccf": hex(&g.lccf.spdu),
            "gpf": hex(&g.gpf.spdu),
            "lpf": hex(&g.lpf.spdu),
            "quorum": g.anchors().quorum,
            "electors": g.anchors().electors.iter()
                .map(|c| hex(&v2xw_sec::cert::encode(c).expect("encodes")))
                .collect::<Vec<_>>(),
            "log": g.log.iter()
                .map(|e| json!({"kind": e.kind, "decided_at": e.decided_at,
                    "published_at": e.published_at, "ctl_sequence": e.ctl_sequence}))
                .collect::<Vec<_>>(),
        }));
        let pki = st.pki.as_deref().expect("the PDU layer is on");
        let la = st.la_aca.expect("LA–ACA keys");
        let mut epochs = BTreeSet::new();
        for p in &st.pdus {
            if p.kind == SpduKind::EncryptedIndividualPlv {
                let e = la.epoch(st.time32(p.at));
                epochs.insert(e);
                if let Some(prev) = e.checked_sub(1) {
                    epochs.insert(prev);
                }
            }
        }
        let la_keys: Vec<Value> = epochs
            .iter()
            .flat_map(|e| {
                [1u16, 2].map(|id| json!({"la": id, "epoch": e, "key": hex(&la.key(id, *e))}))
            })
            .collect();
        let devices: BTreeMap<String, Value> = st
            .devices
            .iter()
            .filter_map(|(n, d)| {
                let c = d.caterpillar.as_ref()?;
                Some((
                    n.index().to_string(),
                    json!({"a": scalar!(c.a()), "p": scalar!(c.p()), "ck": hex(c.ck()),
                           "ek": hex(c.ek())}),
                ))
            })
            .collect();
        let enc = |a: &v2xw_proto::scms::pdu::Authority| {
            json!({
                "cert": hex(&v2xw_sec::cert::encode(&a.cert).expect("encodes")),
                "enc_private": a.enc_private.as_ref().map(|k| scalar!(k)),
            })
        };
        keys.push(json!({
            "provider": label,
            "ra": enc(&pki.ra),
            "ma": enc(&pki.ma),
            "pki": pki.all().iter()
                .map(|a| (a.name.clone(), hex(&v2xw_sec::cert::encode(&a.cert).expect("encodes"))))
                .collect::<BTreeMap<_, _>>(),
            "la_aca": la_keys,
            "canonical": st.eca.canonical.iter()
                .map(|(id, q)| (id.clone(), hex(&q.compressed().expect("a point"))))
                .collect::<BTreeMap<_, _>>(),
            "devices": devices,
            "apps": st.apps.debug_secrets(),
        }));
    }
    write("gov.json", &json!(gov));
    write("keys.json", &json!(keys));
    let truth = json!({
        "t0": "2026-10-20T08:50:00Z",
        "providers": o.providers,
        "liars": o.liars.iter().map(|n| n + SCMS_DEVICE_BASE).collect::<Vec<_>>(),
        "revoked": o.revoked.iter().map(|n| n + SCMS_DEVICE_BASE).collect::<Vec<_>>(),
        // When the MA decided on each liar, as Time32 on the run's clock.
        "decided": o.report.phase2.attacker_outcomes.iter()
            .filter_map(|a| a.decided_ns.map(|t| (a.node + SCMS_DEVICE_BASE, p2.scms().state.time32(t))))
            .collect::<Vec<_>>(),
        "rsus": p2.rsu_nodes().iter().map(|n| n.index() + SCMS_DEVICE_BASE).collect::<Vec<_>>(),
        "runs": o.runs.iter().map(|(l, kinds, rej, _)| {
            let mut by: BTreeMap<&str, u64> = BTreeMap::new();
            for k in kinds {
                *by.entry(k).or_default() += 1;
            }
            json!({"provider": l, "pdus": kinds.len(), "by_kind": by, "rejections": rej})
        }).collect::<Vec<_>>(),
    });
    write("truth.json", &truth);
    println!("conformance capture written to {}", dir.display());
}

fn get(m: &BTreeMap<String, u64>, k: &str) -> u64 {
    m.get(k).copied().unwrap_or(0)
}

#[test]
fn the_run_walks_every_lifecycle_step() {
    let o = outcome();
    let p = &o.report.phase2;
    println!(
        "devices {:?}; reports real {} at MA {}; cases {}; CRLs {}; revoked {:?}; \
         top-ups {}/{}; RSU renewed {}; CTLs {}; ff skipped {}; liars {:?}",
        p.devices_by_provider,
        p.reports_mbr_real,
        p.reports_at_ma,
        p.cases_opened,
        p.crls_issued,
        p.revoked_by_provider,
        p.topups_completed,
        p.topups_started,
        p.rsu_app_certs_renewed,
        p.ctls_published,
        o.report.fast_forward.skipped_steps,
        o.liars,
    );
    assert_eq!(o.report.calendar, "real");
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    assert_eq!(o.report.fast_forward.windows, 2);
    // 1. Factory enrolment, by both providers.
    for k in 0..2 {
        let (label, kinds, _, _) = &o.runs[k];
        let n = |name: &str| kinds.iter().filter(|x| **x == name).count();
        let devices = o.providers.values().filter(|v| **v == k).count();
        assert!(devices > 0, "{label} has no device");
        assert!(
            n("EeEcaCertRequestSpdu") >= devices && n("EcaEeCertResponseSpdu") >= devices,
            "{label}: {devices} devices, {} enrolment requests",
            n("EeEcaCertRequestSpdu")
        );
    }
    // 2. Provisioning for i = 614 and top-ups past the rollover.
    assert!(p.topups_completed > 0, "no top-up completed");
    assert!(
        o.max_i.iter().flat_map(|m| m.values()).any(|i| *i >= 616),
        "no vehicle holds a pseudonym past the rollover's top-up"
    );
    // 3. RSU application-certificate renewal: week 616, asked at the 615 rollover.
    assert!(p.rsu_app_certs_renewed >= 2, "{}", p.rsu_app_certs_renewed);
    assert!(
        o.app_certs
            .iter()
            .flatten()
            .any(|(_, period)| *period == 616)
    );
    // 4. Report, case, CRL, enforcement by both providers' receivers.
    assert!(p.reports_mbr_real > 0, "no real SM-MBR");
    assert!(p.cases_opened > 0 && p.crls_issued > 0, "{p:?}");
    assert_eq!(p.revoked_honest, 0, "an honest vehicle was revoked");
    for receiver in ["provider-a", "provider-b"] {
        let refused: u64 = p
            .revoked_by_provider
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{receiver}<-")))
            .map(|(_, v)| *v)
            .sum();
        assert!(
            refused > 0,
            "no {receiver} receiver refused a revoked sender: {:?}",
            p.revoked_by_provider
        );
    }
    // 5. The CTL change.
    assert!(p.ctls_published >= 1, "no CTL was published");
    assert!(o.ctls[0].0.iter().any(|s| *s >= 1) && o.ctls[0].1 >= 1);
    for (k, (seqs, _)) in o.ctls.iter().enumerate() {
        assert!(
            seqs.iter().any(|s| *s >= 1),
            "provider {k} holds no new CTL"
        );
    }
    let _ = get;
}

#[test]
fn every_spdu_the_runs_emitted_is_captured_once_with_its_bytes() {
    let o = outcome();
    let ids: BTreeSet<&str> = o.pdus.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(ids.len(), o.pdus.len(), "an SPDU was recorded twice");
    for (label, kinds, _, folded) in &o.runs {
        // Each SPDU on a folded leg has its twin on the device leg; one without would
        // be missing from the records.
        for k in folded.iter().copied().collect::<BTreeSet<&str>>() {
            let on = |v: &[&str]| v.iter().filter(|x| **x == k).count();
            assert!(
                on(folded) <= on(kinds),
                "{label}: {} {k} on the folded LOP-RA leg, {} on the device leg",
                on(folded),
                on(kinds)
            );
        }
        let mut emitted: BTreeMap<&str, usize> = BTreeMap::new();
        for k in kinds {
            *emitted.entry(k).or_default() += 1;
        }
        let mut captured: BTreeMap<&str, usize> = BTreeMap::new();
        for p in o
            .pdus
            .iter()
            .filter(|p| p.id.starts_with(&format!("{label}/")))
        {
            assert!(p.hex.is_some(), "{} has no bytes", p.id);
            *captured.entry(p.kind.as_str()).or_default() += 1;
        }
        assert_eq!(emitted, captured, "{label}: emitted vs captured");
    }
    assert!(!o.capture.air.is_empty(), "no frame was tapped");
}
