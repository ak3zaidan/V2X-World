//! The SCMS flows wired end to end into the running engine (WP-B/C leftovers of waves
//! 3-5; docs/design/scms-real/00 §4.3).
//!
//! # Faults injected to prove these checks can fail
//!
//! * `LifecycleParams::from_scenario` without the `calendar_epoch_unix_s` line — the run
//!   provisions i = 0 and `real_calendar_puts_the_real_i_on_the_air` fails on `iCert`.
//! * `ScmsParams::period_start_time32_of` returning `time32` of the saturated start —
//!   the validity start is the run's start, not the Tuesday, and the same test fails.
//! * `from_scenario` without the `pdu.t0_unix_s` line — the run's PDUs count from the
//!   library default and the same test fails (iCert 573).
//! * `ScmsRun::new_at` building `ScmsPki::new` (its own P-384 root) again —
//!   `the_pdu_pki_is_anchored_at_the_governance_root` fails on the root.
//! * `Phase2::real_mbr` taking the evidence BSMs of another signer (the first in the
//!   evidence map) — the MA's evidence check fails and
//!   `detectors_file_real_us_reports_that_the_ma_opens_and_verifies` fails.
//! * `Phase2::advance` without its `governance_due` call — the rollover is never
//!   decided and `a_scheduled_root_rollover_is_decided_and_waits_for_its_sla` fails.
//! * The report store drained in insertion order (lowest `seq` first) — the lib test
//!   `phase2::report_store_tests` fails (the decayed older report comes out first).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_core::ctx::OwnedRecord;
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_engine::{Engine, MemoryRecorder, RunRecorder, Scenario};
use v2xw_msg::sec_types::ieee1609_dot2::CertificateId;
use v2xw_sec::envelope::{Envelope, ParsedSigner, SecurityEnvelopeInfo};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

/// A recorder that keeps the frame taps (the SPDU each node actually signed).
struct Tapping {
    inner: MemoryRecorder,
    taps: BTreeMap<u64, (NodeId, Vec<u8>)>,
}

impl RunRecorder for Tapping {
    fn write(&mut self, at: SimTime, record: &OwnedRecord) {
        self.inner.write(at, record);
    }
    fn write_wire_frame(&mut self, frame: &v2xw_record::wire::Frame) {
        self.inner.write_wire_frame(frame);
    }
    fn tap_frame(&mut self, _at: SimTime, node: NodeId, msg: u64, spdu: &[u8]) {
        self.taps.insert(msg, (node, spdu.to_vec()));
    }
}

fn load(name: &str, duration_s: f64) -> Scenario {
    let mut s = Scenario::load(root().join("scenarios").join(name)).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root().join(&*path).to_string_lossy().into_owned();
    }
    s.time.duration_s = duration_s;
    s.threats.attackers.clear();
    s
}

/// The certificates on the air in a run: `(iCert, validity start)` of every pseudonym.
fn pseudonyms_on_air(s: Scenario) -> Vec<(u16, u32)> {
    let mut engine = Engine::build(s, "").expect("builds");
    let wall = engine.wall_clock();
    let mut rec = Tapping {
        inner: MemoryRecorder::new(),
        taps: BTreeMap::new(),
    };
    engine.run(&mut rec).expect("runs");
    let env = Envelope::ieee1609(wall);
    let mut out = Vec::new();
    for (_, spdu) in rec.taps.values() {
        let parsed = env.parse(spdu).expect("parses");
        if let ParsedSigner::Certificate { certificate, .. } = &parsed.signer
            && let CertificateId::linkageData(ld) = &certificate.0.to_be_signed.id
        {
            out.push((
                ld.i_cert.0.0,
                certificate.0.to_be_signed.validity_period.start.0.0,
            ));
        }
    }
    out
}

/// Joint (6): the run's `Time32`s count from the scenario's `time.t0`, and on the real
/// weekly grid the i-period counts from the US epoch.
///
/// Independent derivation (not the engine's code): the US i-period epoch is 2015-01-06
/// 04:00 EST = 09:00 UTC = Unix 1 420 534 800 (03 §1 fact 4; [SMP] §2.1.17 Table 2).
/// 2026-10-16 12:00 UTC is Unix 1 792 152 000; (1 792 152 000 − 1 420 534 800) / 604 800
/// = 614.44, so i = 614, which began Tuesday 2026-10-13 09:00 UTC = Unix 1 791 882 000.
/// Time32 counts TAI seconds from 2004-01-01 (Unix 1 072 915 200) and five leap seconds
/// were inserted since (2005-12-31, 2008-12-31, 2012-06-30, 2015-06-30, 2016-12-31), so
/// its validity starts at 1 791 882 000 − 1 072 915 200 + 5 = 718 966 805.
#[test]
fn real_calendar_puts_the_real_i_on_the_air() {
    let mut s = load("phase2-manhattan.yaml", 3.0);
    s.time.t0 = "2026-10-16T12:00:00Z".into();
    s.security.crypto_mode = serde_json::from_value(serde_json::json!("real")).expect("mode");
    let seen = pseudonyms_on_air(s);
    assert!(!seen.is_empty(), "no pseudonym certificate on the air");
    for (i, start) in &seen {
        assert!(
            *i == 614 || *i == 615,
            "iCert {i}: the run does not count from the 2015 epoch"
        );
        assert_eq!(
            *start,
            718_966_805 + u32::from(*i - 614) * 604_800,
            "validity start of i = {i}"
        );
    }
    assert!(
        seen.iter().any(|(i, _)| *i == 614),
        "nobody signs in i = 614"
    );
}

/// Joint (5): the PDU layer's Root CA and ICA are the governance entities' own, so the
/// pseudonyms a run signs with chain to the root the genesis CTL approves.
///
/// Checked from the published artefacts, not from the PKI struct's own claims: the root
/// approved by the CTL is the PKI's root (its HashedId32), the ICA's issuer is that
/// root's HashedId8, the ACA's and the CRL signer's issuer is the ICA's, and the ACA, RA
/// and CRL signer validate up to a CTL root with the full 1609.2 chain rules.
#[test]
fn the_pdu_pki_is_anchored_at_the_governance_root() {
    let s = load("phase2-manhattan.yaml", 1.0);
    let engine = Engine::build(s, "").expect("builds");
    let scms = engine.phase2().expect("phase 2").scms();
    let pki = scms.state.pki.as_ref().expect("the PDU layer is on");
    let gov = &scms.state.gov;
    let enc = |c: &v2xw_msg::sec_types::ieee1609_dot2::Certificate| {
        v2xw_sec::cert::encode(c).expect("encodes")
    };
    assert_eq!(enc(&pki.root.cert), enc(gov.cert("root").expect("root")));
    assert_eq!(enc(&pki.ica.cert), enc(gov.cert("ica").expect("ica")));
    // The CTL approves exactly that root.
    let root32 = v2xw_sec::cert::hashed_id32(&pki.root.cert).expect("id32");
    assert!(
        gov.ctl.roots.contains(&root32),
        "the PDU layer's root is not on the CTL"
    );
    let id8 = |c| v2xw_sec::hashedid::id8_bytes(&v2xw_sec::cert::cert_id8(c).expect("id8"));
    let issuer = |c: &v2xw_msg::sec_types::ieee1609_dot2::Certificate| match &c.0.issuer {
        v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha256AndDigest(d)
        | v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha384AndDigest(d) => d.0.to_vec(),
        other => panic!("issuer {other:?}"),
    };
    assert_eq!(issuer(&pki.ica.cert), id8(&pki.root.cert).to_vec());
    assert_eq!(issuer(&pki.aca.cert), id8(&pki.ica.cert).to_vec());
    assert_eq!(issuer(&pki.crlg.cert), id8(&pki.ica.cert).to_vec());
    // The full 1609.2 chain rules, anchored at the CTL's roots, at the run's start.
    let policy = v2xw_sec::chain::ChainPolicy {
        anchors: Some(gov.ctl.roots.iter().copied().collect()),
        now: Some(engine.wall_clock().time32(0).expect("time32")),
    };
    for (name, c) in [("aca", &pki.aca), ("ra", &pki.ra), ("crlg", &pki.crlg)] {
        let chain = [c.cert.clone(), pki.ica.cert.clone(), pki.root.cert.clone()];
        if let Err(e) = v2xw_sec::chain::validate(&chain, &policy) {
            panic!("{name} does not validate up to the CTL root: {e:?}");
        }
    }
}

/// Joint (1): a vehicle's detector files a real US misbehaviour report (SM-MBR v1.1)
/// whose evidence is the accused's own signed BSMs, the RA accepts its enrolment-signed
/// upload, the MA decrypts it and verifies the evidence, and the revocation goes through
/// the MA's decision mode (`request_revocation`; automatic here, `ma_review_s: 0`).
#[test]
fn detectors_file_real_us_reports_that_the_ma_opens_and_verifies() {
    let mut s =
        Scenario::load(root().join("scenarios").join("phase2-manhattan.yaml")).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root().join(&*path).to_string_lossy().into_owned();
    }
    s.time.duration_s = 120.0;
    // The RA's report shuffle closes after 5 s, so the late vehicle reports reach the MA
    // inside the run (the shipped 60 s is itself a compression of one day).
    if let Some(p) = s.security.protocol.as_mut() {
        p.params["report_shuffle_window_s"] = serde_json::json!(5.0);
    }
    s.security.crypto_mode = serde_json::from_value(serde_json::json!("real")).expect("mode");
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    let p = &report.phase2;
    println!(
        "{:?} cell {} relay {} rsu {} real {} unmapped {} unsecured {} sent {} at_ma {} cases {}",
        p.reports_mbr_unsecured_by,
        p.reports_uploaded_cellular,
        p.reports_uploaded_relay,
        p.reports_from_rsus,
        p.reports_mbr_real,
        p.reports_mbr_unmapped,
        p.reports_mbr_unsecured,
        p.reports_sent,
        p.reports_at_ma,
        p.cases_opened
    );
    assert!(p.reports_mbr_real > 0, "no real US report was filed");
    let st = &engine.phase2().expect("phase 2").scms().state;
    assert!(
        st.pdu_rejections.is_empty(),
        "the backend refused: {:?}",
        st.pdu_rejections
    );
    assert!(!st.ma.opened_reports.is_empty(), "the MA opened no report");
    for o in &st.ma.opened_reports {
        assert!(
            o.evidence_verified,
            "evidence does not verify: {:?}",
            o.report.subtypes
        );
        assert_eq!(
            o.report.mbr_type,
            v2xw_proto::scms::mbr::MbrType::OneVInconsistent
        );
    }
    assert!(p.cases_opened > 0, "the MA opened no case");
}

/// Joint (4): an EAC root rollover scheduled in the run is decided at its time, and its
/// CTL waits for the SLA publication deadline.
///
/// Independent derivation: `time.t0` 2026-10-16T12:00:00Z is a Friday; a root addition
/// decided at t = 1 s must be published within 3 business days (Provider Req. v1.1
/// §3.4.11): Monday, Tuesday, Wednesday, so at 1 s + 5 days. The genesis CTL stays the
/// published one for the whole (short) run.
#[test]
fn a_scheduled_root_rollover_is_decided_and_waits_for_its_sla() {
    let mut s = load("phase2-manhattan.yaml", 3.0);
    s.time.t0 = "2026-10-16T12:00:00Z".into();
    if let Some(p) = s.security.protocol.as_mut() {
        p.params["root_rollover_at_s"] = serde_json::json!(1.0);
    }
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    assert_eq!(
        report.phase2.gov_decisions, 1,
        "the rollover was not decided"
    );
    assert_eq!(report.phase2.ctls_published, 0);
    let gov = &engine.phase2().expect("phase 2").scms().state.gov;
    assert_eq!(gov.counters.roots_added, 1);
    assert_eq!(gov.ctl.sequence, 0, "published before its deadline");
    let pending: Vec<(u64, u32)> = gov.pending.iter().map(|(t, c)| (*t, c.sequence)).collect();
    assert_eq!(pending, vec![((5 * 86_400 + 1) * 1_000_000_000, 1)]);
}
