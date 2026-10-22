//! Reproductions of the disagreements the final independent oracle check found
//! (`docs/design/scms-real/testing/oracle-final.md`, `tools/oracle/oracle.py records`).
//!
//! Written from the engine's public outputs only: the `pki.flow`, `pki.knowledge` and
//! `node.credentials` records of `scenarios/scms-oracle-repro.yaml` (the conformance run's
//! first 640 s). No engine source was read to write them. Both fail on the engine of
//! 2004bb87, so they are ignored until the engine is fixed:
//! `cargo test -p v2xw-engine --test oracle_final -- --ignored`.
//!
//! * OF-1: a device has one enrolment certificate (IEEE 1609.2.1 6.3.4.2 issues it; the DCM
//!   registers it with the RA, [CAMP122] 5.2; every Download-Req is signed by it, [CAMP122]
//!   6.3, [SM-MBR] 4, [SMP] 4.4). The engine's vehicles hold two: the one their recorded ECA
//!   exchange issued (6 years) signs their pre-drive Download-Reqs; another one, which no
//!   recorded exchange issued and whose lifetime is not 6 years, is the one the RA registered,
//!   `node.credentials` reports, and that signs every later Download-Req and report upload.
//! * OF-2: every report upload is an SPDU on the wire ([SM-MBR] 3-4: signed, encrypted to
//!   the MA, POSTed with a Download-Req). Most cellular uploads in the engine carry no bytes.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::Value;
use v2xw_engine::{Engine, MemoryRecorder, Scenario};

struct Recs {
    flow: Vec<Value>,
    /// The RA's registered enrolment certificates (HashedId8 hex), every RA entity.
    ra_enrolments: BTreeSet<String>,
    /// device -> (enrolment HashedId8, that certificate's hex), from `node.credentials`.
    enrolment: BTreeMap<String, (String, String)>,
}

fn run() -> &'static Recs {
    static RUN: std::sync::OnceLock<Recs> = std::sync::OnceLock::new();
    RUN.get_or_init(run_once)
}

fn run_once() -> Recs {
    let s = Scenario::load(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/scms-oracle-repro.yaml"),
    )
    .expect("loads");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    engine.run(&mut rec).expect("runs");
    let mut out = Recs {
        flow: Vec::new(),
        ra_enrolments: BTreeSet::new(),
        enrolment: BTreeMap::new(),
    };
    for (_, r) in rec.records() {
        match r.channel {
            "pki.flow" => out
                .flow
                .push(serde_json::from_slice(&r.json).expect("json")),
            "pki.knowledge" => {
                let v: Value = serde_json::from_slice(&r.json).expect("json");
                if v["role"] == "ra" {
                    for row in v["holds"]["enrolment_rows"]
                        .as_array()
                        .into_iter()
                        .flatten()
                    {
                        if let Some(e) = row["enrolment"].as_str() {
                            out.ra_enrolments.insert(e.to_string());
                        }
                    }
                }
            }
            "node.credentials" => {
                let v: Value = serde_json::from_slice(&r.json).expect("json");
                let (Some(dev), Some(e)) = (v["device"].as_str(), v["enrolment"].as_str()) else {
                    continue;
                };
                if let Some(hex) = v["certificates"][e].as_str() {
                    out.enrolment
                        .insert(dev.to_string(), (e.to_string(), hex.to_string()));
                }
            }
            _ => {}
        }
    }
    out
}

fn pdus(flow: &[Value]) -> impl Iterator<Item = (&Value, &Value)> {
    flow.iter().flat_map(|m| {
        m["pdus"]
            .as_array()
            .into_iter()
            .flatten()
            .map(move |p| (m, p))
    })
}

#[test]
#[ignore = "OF-1 (oracle-final.md): fails on 2004bb87, two enrolment certificates per vehicle"]
fn one_enrolment_certificate_is_issued_registered_and_signs_every_download_request() {
    let r = run();
    assert!(
        !r.enrolment.is_empty(),
        "no node.credentials enrolment recorded"
    );
    let mut issued_elsewhere = BTreeSet::new();
    let mut unregistered = BTreeSet::new();
    let mut signed_by_other = BTreeMap::<String, u64>::new();
    let mut checked = 0u64;
    for (dev, (id8, hex)) in &r.enrolment {
        if !r.ra_enrolments.contains(id8) {
            unregistered.insert(dev.clone());
        }
        // The ECA's response carries the issued certificate verbatim.
        let issued = pdus(&r.flow).any(|(_, p)| {
            p["type"] == "EcaEeCertResponseSpdu"
                && p["to"].as_str() == Some(dev)
                && p["hex"].as_str().is_some_and(|h| h.contains(hex.as_str()))
        });
        if !issued {
            issued_elsewhere.insert(dev.clone());
        }
        // A SignerSingleCert Download-Req carries the signer certificate verbatim.
        for (m, p) in pdus(&r.flow) {
            if p["type"] == "EeRaDownloadRequestSpdu" && p["from"].as_str() == Some(dev) {
                checked += 1;
                let opened = p["opened"]["hex"].as_str().unwrap_or("");
                if !opened.contains(hex.as_str()) {
                    *signed_by_other
                        .entry(m["name"].as_str().unwrap_or("?").to_string())
                        .or_insert(0) += 1;
                }
            }
        }
    }
    assert!(checked > 0, "no Download-Req captured");
    assert!(
        issued_elsewhere.is_empty() && unregistered.is_empty() && signed_by_other.is_empty(),
        "{} devices' recorded enrolment certificate is not the one their ECA exchange issued; \
         {} are not registered at their RA; Download-Reqs (of {checked}) not signed by it, by message: {signed_by_other:?}",
        issued_elsewhere.len(),
        unregistered.len()
    );
}

#[test]
#[ignore = "OF-2 (oracle-final.md): fails on 2004bb87, most cellular report uploads carry no SPDU"]
fn every_cellular_report_upload_carries_its_spdus() {
    let r = run();
    let (mut with, mut without) = (0u64, 0u64);
    for m in &r.flow {
        if m["kind"] == "msg"
            && m["name"] == "report"
            && m["transport"] == "cellular-uu"
            && m["from"].as_str().is_some_and(|f| f.starts_with("ee/"))
        {
            if m["pdus"].as_array().is_some_and(|a| !a.is_empty()) {
                with += 1;
            } else {
                without += 1;
            }
        }
    }
    assert!(with + without > 0, "no report upload in the run");
    assert_eq!(
        without,
        0,
        "{without} of {} cellular report uploads carry no SecuredMisbehaviorReport / Download-Req",
        with + without
    );
}
