//! WP-B3b: RSU application certificates and OBU identification certificates.
//!
//! * an RSU's weekly application certificate: no butterfly, implicit, `crlSeries` 3, no
//!   `LinkageData`, `hours(169)`, SPaT/MAP, `cracaId` = the CRACA (ICA); the key the RSU
//!   reconstructs signs verifiably under the certificate;
//! * an emergency OBU's identification certificate: original butterfly at `(i, 0)`,
//!   `crlSeries` 3, no `LinkageData`, `firstI` in the ack;
//! * every leg is a real SPDU, charged its encoded length (I-S3), and none is refused;
//! * the Wyoming scenario: an RSU whose backhaul is down for 8 days starves (holds no
//!   valid certificate) and recovers when the backhaul returns; 6 days does not starve it;
//! * the RA refuses a second, different request for an answered period and a request
//!   beyond `ra-maxGenDelay`, and only the requester's enrolment certificate may download;
//! * the EE refuses a response not signed by the issuing ACA, and a tampered one;
//! * with `V2XW_APPCERT_OUT=<dir>` the corpus for `tools/pdu_check/check_appcerts.py`.

mod common;

use std::collections::BTreeMap;

use common::{DEVICE_A, DEVICE_B};
use v2xw_core::ids::NodeId;
use v2xw_core::time::SimTime;
use v2xw_msg::sec_types::ieee1609_dot2::{CertificateId, CertificateType};
use v2xw_proto::kernel::Delivery;
use v2xw_proto::scms::appcert::{self, AppCertKind, RequestKeys};
use v2xw_proto::scms::msg::ScmsMsg;
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::{self, Drbg, Pdu, PduError, SpduKind};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::scms::run::appcert::{AppEventKind, AppMsg};
use v2xw_proto::stage::FlowId;
use v2xw_sec::sig1609::{self, Hash1609};

const HOUR: u64 = 3_600_000_000_000;
const DAY: u64 = 24 * HOUR;
const WEEK: u64 = 7 * DAY;

fn deployment() -> ScmsRun {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    ScmsRun::new(p).expect("certificates encode")
}

/// An enrolled RSU (or OBU) at the kernel's clock.
fn enrolled(run: &mut ScmsRun, node: NodeId, rsu: bool) {
    if rsu {
        run.add_rsu(node);
    } else {
        run.add_device(node);
    }
    run.enrol(node);
    run.run().expect("enrolment runs");
    assert!(run.state.devices[&node].enrolment_credential.is_some());
}

fn installed(run: &ScmsRun, node: NodeId) -> usize {
    run.state.apps.devices[&node].installed.len()
}

/// The certified key signs, and the signature verifies under the key a receiver
/// reconstructs from the certificate alone (ECQV under the ACA).
fn signs_verifiably(run: &ScmsRun, node: NodeId) {
    let pki = run.state.pki.as_ref().unwrap();
    for c in &run.state.apps.devices[&node].installed {
        let (h, q) = pki.signer_key(&c.cred.cert).expect("reconstructs");
        assert_eq!(h, Hash1609::Sha256);
        let sig = c.cred.key.sign_1609(b"spat", b"");
        let dg = sig1609::digest_1609(h, b"spat", b"");
        assert!(
            sig1609::verify_digest(h, &q, &dg, &sig),
            "period {}",
            c.cred.period
        );
    }
}

fn step_kind(step: &str) -> Option<SpduKind> {
    Some(match step {
        "app-cert-request" => SpduKind::EeRaCertRequest,
        "app-cert-ack" => SpduKind::RaEeCertAck,
        "app-cert-aca-request" => SpduKind::RaAcaCertRequest,
        "app-cert-aca-response" => SpduKind::AcaRaCertResponse,
        "app-cert-download-request" => SpduKind::EeRaDownloadRequest,
        _ => return None,
    })
}

/// I-S3 as equality on every app-cert hop that carried an SPDU.
fn charged_their_length(run: &ScmsRun) -> usize {
    let mut matched = 0;
    for s in &run.kernel.steps {
        let kind = match s.step {
            "app-cert-download" if s.bytes != run.state.sizes.refusal().bytes() => {
                SpduKind::AcaEeCertResponsePrivate
            }
            other => match step_kind(other) {
                Some(k) => k,
                None => continue,
            },
        };
        let ok = run.state.pdus.iter().any(|p| {
            p.kind == kind && p.from == s.from.index() && p.to == s.to.index() && p.len == s.bytes
        });
        assert!(
            ok,
            "step {} ({} B) has no SPDU of that length",
            s.step, s.bytes
        );
        matched += 1;
    }
    matched
}

#[test]
fn an_rsu_application_certificate_is_implicit_series_3_without_linkage_data() {
    let mut run = deployment();
    enrolled(&mut run, DEVICE_A, true);
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_A, AppCertKind::RsuApplication, t0);
    run.run_until(t0 + 2 * HOUR).unwrap();

    let st = &run.state;
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    // The current week's certificate, and next week's at once: the RA generates up to a
    // week ahead and this RSU asks as early as it may.
    assert_eq!(installed(&run, DEVICE_A), 2);
    let pki = st.pki.as_ref().unwrap();
    // Derived here, not through `appcert::craca_id`, so a wrong CRACA cannot pass.
    let ica3 = v2xw_sec::cert::cert_id3(&pki.ica.cert).unwrap().0.to_vec();
    for (k, c) in st.apps.devices[&DEVICE_A].installed.iter().enumerate() {
        let t = &c.cred.cert.0.to_be_signed;
        assert_eq!(c.cred.cert.0.r_type, CertificateType::implicit);
        assert_eq!(t.crl_series.0.0, 3, "CRL series 3");
        assert!(matches!(t.id, CertificateId::none(())), "no LinkageData");
        assert_eq!(
            &t.craca_id.0[..],
            &ica3[..],
            "cracaId is the ICA's HashedId3"
        );
        assert_eq!(
            t.validity_period.duration,
            v2xw_msg::sec_types::ieee1609_dot2_base_types::Duration::hours(
                v2xw_msg::sec_types::ieee1609_dot2_base_types::Uint16(169)
            )
        );
        let psids: Vec<_> = t
            .app_permissions
            .as_ref()
            .unwrap()
            .0
            .iter()
            .map(|p| p.psid.0.clone())
            .collect();
        assert_eq!(psids, vec![rasn::types::Integer::from(0x82)]);
        assert_eq!(c.cred.period, k as u32);
        assert_eq!(c.until - c.from, WEEK + HOUR, "1 week + 1 h");
    }
    // Consecutive certificates overlap by exactly the hour.
    let i = &st.apps.devices[&DEVICE_A].installed;
    assert_eq!(i[0].until - i[1].from, HOUR);
    signs_verifiably(&run, DEVICE_A);
    assert_eq!(st.apps.issued.len(), 2);
    assert!(
        st.apps
            .issued
            .iter()
            .all(|x| x.kind == AppCertKind::RsuApplication)
    );

    let mut counts = BTreeMap::new();
    for p in &st.pdus {
        *counts.entry(p.kind).or_insert(0u32) += 1;
    }
    for k in [
        SpduKind::EeRaCertRequest,
        SpduKind::RaEeCertAck,
        SpduKind::RaAcaCertRequest,
        SpduKind::AcaRaCertResponse,
        SpduKind::AcaEeCertResponsePrivate,
    ] {
        assert!(
            counts.get(&k).copied().unwrap_or(0) >= 2,
            "{k:?}: {counts:?}"
        );
    }
    assert!(charged_their_length(&run) >= 10);
    // The legs ride the RSU's backhaul, and the stages are stamped.
    let backhaul = run.kernel.bytes_by_transport();
    assert!(
        backhaul
            .get(&v2xw_proto::net::Transport::RsuBackhaul)
            .copied()
            .unwrap_or(0)
            > 0
    );
    assert_eq!(
        run.kernel.stages.runs_of(FlowId::RsuApplicationCert).len(),
        1
    );
}

/// One deployment with an RSU and an emergency OBU: every SPDU verifies, and with
/// `V2XW_APPCERT_OUT=<dir>` the corpus for `tools/pdu_check/check_appcerts.py`.
#[test]
fn an_rsu_and_an_emergency_obu_side_by_side_corpus_for_the_oracle() {
    let mut run = deployment();
    enrolled(&mut run, DEVICE_A, true);
    enrolled(&mut run, DEVICE_B, false);
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_A, AppCertKind::RsuApplication, t0);
    run.start_app_certs(DEVICE_B, AppCertKind::ObuIdentification, t0);
    run.run_until(t0 + 2 * HOUR).unwrap();
    assert!(run.state.pdu_rejections.is_empty());
    assert_eq!(installed(&run, DEVICE_A) + installed(&run, DEVICE_B), 3);
    assert_eq!(run.state.apps.debug_secrets().len(), 3);
    if let Ok(dir) = std::env::var("V2XW_APPCERT_OUT") {
        write_corpus(&run, std::path::Path::new(&dir));
    }
}

#[test]
fn an_identification_certificate_uses_the_original_butterfly_and_series_3() {
    let mut run = deployment();
    enrolled(&mut run, DEVICE_B, false);
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_B, AppCertKind::ObuIdentification, t0);
    run.run_until(t0 + 2 * HOUR).unwrap();
    let st = &run.state;
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    // Only the current month: the next one is more than a week ahead.
    assert_eq!(installed(&run, DEVICE_B), 1);
    let c = &st.apps.devices[&DEVICE_B].installed[0];
    let t = &c.cred.cert.0.to_be_signed;
    assert_eq!(t.crl_series.0.0, 3);
    assert!(matches!(t.id, CertificateId::none(())));
    assert_eq!(c.until - c.from, 30 * DAY + HOUR);
    signs_verifiably(&run, DEVICE_B);
    // The ack carries firstI (a butterfly request).
    let ack = st
        .pdus
        .iter()
        .find(|p| p.kind == SpduKind::RaEeCertAck)
        .unwrap();
    let (p, _) = pdu::open_signed_pdu(
        ack.bytes.as_ref().unwrap().bytes(),
        st.pki.as_ref().unwrap(),
    )
    .unwrap();
    let v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPduContent::ee_ra(
        v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::EeRaInterfacePdu::raEeCertAck(a),
    ) = p.content
    else {
        panic!("ack")
    };
    assert!(a.first_i.is_some());
    assert!(
        run.kernel
            .bytes_by_transport()
            .contains_key(&v2xw_proto::net::Transport::CellularUu)
    );
    assert!(charged_their_length(&run) >= 5);
}

/// The Wyoming pilot: rural RSUs could not reach the SCMS and their weekly certificates
/// ran out (01 §6). The RSU asks for week k+1 at the start of week k; the backhaul goes
/// down an hour before that and stays down `days`.
fn wyoming(days: u64) -> (ScmsRun, SimTime, SimTime) {
    let mut run = deployment();
    enrolled(&mut run, DEVICE_A, true);
    let p = run.state.params;
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_A, AppCertKind::RsuApplication, t0);
    // Week 1 starts one i-period after the epoch.
    let week1 = p.period_start(1);
    let down_from = week1 - HOUR;
    let down_until = down_from + days * DAY;
    run.set_backhaul_outage(DEVICE_A, down_from, down_until);
    let horizon = week1 + 10 * DAY;
    run.run_until(horizon).unwrap();
    (run, down_until, horizon)
}

#[test]
fn wyoming_an_rsu_without_backhaul_for_8_days_starves_then_recovers() {
    let (run, down_until, horizon) = wyoming(8);
    let p = run.state.params;
    // From the first installation (a new RSU holds nothing before it).
    let first = run.state.apps.devices[&DEVICE_A].installed[0].at;
    let gaps = run.app_starvation(DEVICE_A, first, horizon);
    assert_eq!(gaps.len(), 1, "{gaps:?}");
    let (from, until) = gaps[0];
    // Week 1's certificate expires at start(2) + 1 h; nothing replaces it until the
    // backhaul is back and the hourly retry gets through.
    assert_eq!(from, p.period_start(2) + HOUR);
    assert!(
        until >= down_until && until <= down_until + HOUR + 5 * 60_000_000_000,
        "{until} vs {down_until}"
    );
    assert!(
        until - from > 20 * HOUR,
        "starving {} h",
        (until - from) / HOUR
    );
    assert!(
        run.app_credential_at(DEVICE_A, from + HOUR).is_none(),
        "cannot sign SPaT/MAP"
    );
    assert!(
        run.app_credential_at(DEVICE_A, until + HOUR).is_some(),
        "recovers"
    );
    let ev = &run.state.apps.devices[&DEVICE_A].events;
    let down = ev
        .iter()
        .filter(|e| e.kind == AppEventKind::BackhaulDown)
        .count();
    assert!(down >= 7 * 24, "an hourly retry all week: {down}");
    assert!(run.state.pdu_rejections.is_empty());
}

#[test]
fn wyoming_control_a_6_day_outage_does_not_starve_the_rsu() {
    let (run, _, horizon) = wyoming(6);
    let p = run.state.params;
    let first = run.state.apps.devices[&DEVICE_A].installed[0].at;
    assert!(first < p.period_start(1));
    assert!(run.app_starvation(DEVICE_A, first, horizon).is_empty());
}

fn second_request(run: &ScmsRun, node: NodeId, period: u32, valid_from_week: u32) -> Pdu {
    let st = &run.state;
    let pki = st.pki.as_ref().unwrap();
    let enrol = st.devices[&node].enrolment_credential.clone().unwrap();
    let mut g = Drbg::new(99, b"attacker");
    let keys = RequestKeys::fresh(AppCertKind::RsuApplication, period, &mut g).unwrap();
    let t = run.state.params.period_start(valid_from_week);
    let wall = v2xw_core::time::WallClock::new(st.params.pdu.t0_unix_s);
    let v = wall.time32(t).unwrap();
    let now = wall.time32(run.kernel.now()).unwrap();
    Pdu::new(
        appcert::ee_request_spdu(
            pki,
            &enrol,
            AppCertKind::RsuApplication,
            now,
            &keys,
            v,
            &mut g,
        )
        .unwrap(),
    )
}

fn inject_request(run: &mut ScmsRun, node: NodeId, period: u32, pdu: Pdu) {
    let at = run.kernel.now();
    let ra = run.state.nodes.ra;
    run.kernel.inject(
        at,
        Delivery {
            at,
            from: node,
            to: ra,
            msg: ScmsMsg::AppCert(Box::new(AppMsg::Request {
                device: node,
                period,
                pdu,
            })),
            flow: FlowId::RsuApplicationCert,
            run: v2xw_proto::stage::FlowRun(9_999),
        },
    );
}

#[test]
fn the_ra_refuses_stockpiling_a_second_request_and_one_beyond_max_gen_delay() {
    let mut run = deployment();
    enrolled(&mut run, DEVICE_A, true);
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_A, AppCertKind::RsuApplication, t0);
    run.run_until(t0 + HOUR).unwrap();
    assert_eq!(installed(&run, DEVICE_A), 2);
    let issued = run.state.apps.issued.len();
    // A second, different request for week 1, which is already answered.
    let dup = second_request(&run, DEVICE_A, 1, 1);
    inject_request(&mut run, DEVICE_A, 1, dup);
    // A request for week 3, more than a week ahead.
    let early = second_request(&run, DEVICE_A, 3, 3);
    inject_request(&mut run, DEVICE_A, 3, early);
    let t1 = run.kernel.now();
    run.run_until(t1 + HOUR).unwrap();
    let why: Vec<_> = run.state.apps.ra_refusals.iter().map(|r| r.3).collect();
    assert_eq!(
        why,
        vec![
            "a second request for a period already answered",
            "validity starts beyond ra-maxGenDelay"
        ]
    );
    assert_eq!(run.state.apps.issued.len(), issued, "nothing more issued");
    // The legitimate RSU's cycle is not disturbed by refusals of requests it never sent.
    assert!(!run.state.apps.devices[&DEVICE_A].stopped);
}

#[test]
fn only_the_requesters_enrolment_certificate_may_download_its_file() {
    let mut run = deployment();
    enrolled(&mut run, DEVICE_A, true);
    enrolled(&mut run, DEVICE_B, true);
    let t0 = run.kernel.now();
    run.start_app_certs(DEVICE_A, AppCertKind::RsuApplication, t0);
    run.run_until(t0 + HOUR).unwrap();
    // B asks for A's week-0 file under A's name, signed with B's enrolment certificate.
    let st = &run.state;
    let pki = st.pki.as_ref().unwrap();
    let name = st.apps.ra_store.keys().next().unwrap().clone();
    let enrol_b = st.devices[&DEVICE_B].enrolment_credential.clone().unwrap();
    let mut g = Drbg::new(7, b"thief");
    let req = appcert::ee_download_request_spdu(pki, &enrol_b, 0, &name, &mut g).unwrap();
    let at = run.kernel.now();
    let before = run.state.pdu_rejections.len();
    run.kernel.inject(
        at,
        Delivery {
            at,
            from: DEVICE_B,
            to: run.state.nodes.ra,
            msg: ScmsMsg::AppCert(Box::new(AppMsg::DownloadRequest {
                device: DEVICE_A,
                period: 0,
                pdu: Pdu::new(req),
            })),
            flow: FlowId::RsuApplicationCert,
            run: v2xw_proto::stage::FlowRun(9_998),
        },
    );
    run.run_until(at + 60_000_000_000).unwrap();
    let r = &run.state.pdu_rejections[before..];
    assert_eq!(r.len(), 1, "{r:?}");
    assert_eq!(r[0].kind, SpduKind::EeRaDownloadRequest);
}

/// The pieces of one RSU exchange, run by hand.
fn exchange() -> (
    std::sync::Arc<pdu::ScmsPki>,
    RequestKeys,
    appcert::AcaIssued,
    u32,
) {
    let mut run = deployment();
    enrolled(&mut run, DEVICE_A, true);
    let pki = run.state.pki.clone().unwrap();
    let enrol = run.state.devices[&DEVICE_A]
        .enrolment_credential
        .clone()
        .unwrap();
    let mut g = Drbg::new(5, b"hand");
    let keys = RequestKeys::fresh(AppCertKind::RsuApplication, 0, &mut g).unwrap();
    let from = 600_000_000;
    let req = appcert::ee_request_spdu(
        &pki,
        &enrol,
        AppCertKind::RsuApplication,
        from,
        &keys,
        from,
        &mut g,
    )
    .unwrap();
    let o = appcert::ra_open_request(&pki, &req).unwrap();
    let (r, u) = o.aca_keys(0).unwrap();
    let aca_req = appcert::ra_aca_cert_request_spdu(&pki, from, &o.tbs, &r, &u).unwrap();
    let issued = appcert::aca_issue(&pki, &aca_req, from, &mut g).unwrap();
    let h = Pdu::new(aca_req).hashed_id8();
    let stored = appcert::ra_open_aca_response(&pki, &issued.aca_ra_spdu, h).unwrap();
    assert_eq!(
        stored, issued.private_spdu,
        "the RA passes the response on untouched"
    );
    (pki, keys, issued, from)
}

#[test]
fn the_ee_installs_a_genuine_response_and_refuses_another_signer_or_a_tampered_one() {
    let (pki, keys, issued, from) = exchange();
    let ok = appcert::ee_open_response(
        &pki,
        &issued.private_spdu,
        &keys,
        AppCertKind::RsuApplication,
        0,
        from,
    );
    assert!(ok.is_ok(), "{ok:?}");

    // Re-signed by the RA (a component of the same PKI, not the issuing ACA).
    let o = pdu::open_signed(&issued.private_spdu, &pki).unwrap();
    let inner = pdu::decrypt_key(&o.payload, &keys.response_private()).unwrap();
    let p = pdu::open_unsecured(&inner).unwrap();
    let u = v2xw_sec::ec::Point::mul_base(&keys.response_private());
    let mut g = Drbg::new(6, b"forger");
    let forged = pdu::encrypted_signed(&p, &u, &pki.ra.cert, &pki.ra.key, &mut g).unwrap();
    let e = appcert::ee_open_response(&pki, &forged, &keys, AppCertKind::RsuApplication, 0, from)
        .unwrap_err();
    assert!(matches!(e, PduError::Signer(_)), "{e:?}");

    // One ciphertext byte flipped, near the end (inside the CCM ciphertext, under the
    // ACA signature).
    let mut t = issued.private_spdu.clone();
    let k = t.len() - 80;
    t[k] ^= 1;
    assert!(
        appcert::ee_open_response(&pki, &t, &keys, AppCertKind::RsuApplication, 0, from).is_err()
    );

    // Wrong response key.
    let mut g = Drbg::new(8, b"other");
    let other = RequestKeys::fresh(AppCertKind::RsuApplication, 0, &mut g).unwrap();
    assert!(
        appcert::ee_open_response(
            &pki,
            &issued.private_spdu,
            &other,
            AppCertKind::RsuApplication,
            0,
            from
        )
        .is_err()
    );
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Writes every app-cert SPDU, the PKI and the keys an independent verifier needs: the
/// RA's encryption key and, per installed certificate, `k_U` and the response key `u`.
fn write_corpus(run: &ScmsRun, dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    let st = &run.state;
    let pki = st.pki.as_ref().unwrap();
    let mut files = Vec::new();
    for (k, p) in st.pdus.iter().enumerate() {
        let name = format!("{k:02}_{}.oer", p.kind.name());
        std::fs::write(dir.join(&name), p.bytes.as_ref().unwrap().bytes()).unwrap();
        files.push(serde_json::json!({"file": name, "kind": p.kind.name()}));
    }
    let certs: BTreeMap<String, String> = pki
        .all()
        .iter()
        .map(|a| {
            (
                a.name.clone(),
                hex(&v2xw_sec::cert::encode(&a.cert).unwrap()),
            )
        })
        .collect();
    let keys: Vec<_> = st
        .apps
        .devices
        .values()
        .flat_map(|d| d.installed.iter())
        .map(|c| {
            serde_json::json!({
                "cert": hex(&v2xw_sec::cert::encode(&c.cred.cert).unwrap()),
                "period": c.cred.period,
            })
        })
        .collect();
    let m = serde_json::json!({
        "pdus": files,
        "certs": certs,
        "ra_enc_private": hex(&v2xw_sec::ec::scalar_to_be32(&pki.ra.enc_private.unwrap())),
        "installed": keys,
        "secrets": st.apps.debug_secrets(),
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&m).unwrap(),
    )
    .unwrap();
}

fn periods(run: &ScmsRun, node: NodeId) -> Vec<u32> {
    run.app_installed(node).iter().map(|c| c.cred.period).collect()
}

/// The engine's pre-run issuance: a unit whose backhaul was up before the run joins it
/// holding this week's certificate and next week's (requested `ra-maxGenDelay` ahead),
/// both through the real SPDUs, marked pre-run and installed at 0; the cycle then goes on
/// in the run and renews over the backhaul. A unit with no backhaul since installation
/// holds this week's only and starves an hour after the next period starts.
#[test]
fn a_preloaded_rsu_joins_with_this_and_next_week_and_renews_in_the_run() {
    let mut run = deployment();
    let p = run.state.params;
    let at = p.period_start(1) + DAY;
    run.add_rsu(DEVICE_A);
    run.add_rsu(DEVICE_B);
    run.set_backhaul_outage(DEVICE_B, 0, SimTime::MAX);
    run.preload_app_cert(DEVICE_A, AppCertKind::RsuApplication, at, true)
        .expect("pre-run flows");
    run.preload_app_cert(DEVICE_B, AppCertKind::RsuApplication, at, false)
        .expect("pre-run flows");
    assert_eq!(periods(&run, DEVICE_A), vec![1, 2]);
    assert_eq!(periods(&run, DEVICE_B), vec![1]);
    assert!(run.app_installed(DEVICE_A).iter().all(|c| c.at == 0));
    assert!(run.app_installed(DEVICE_B).iter().all(|c| c.at == 0));
    assert!(!run.state.pdus.is_empty());
    assert!(run.state.pdus.iter().all(|r| r.preload && r.step.is_none()));
    assert!(run.app_credential_at(DEVICE_A, at).is_some());
    assert!(run.app_credential_at(DEVICE_B, at).is_some());
    let horizon = p.period_start(3) - HOUR;
    run.run_until(horizon).unwrap();
    // Week 3 was asked for at the start of week 2, in the run.
    assert_eq!(periods(&run, DEVICE_A), vec![1, 2, 3]);
    let third = &run.app_installed(DEVICE_A)[2];
    assert!(third.at >= p.period_start(2) && third.at < p.period_start(2) + HOUR);
    assert!(run.app_starvation(DEVICE_A, at, horizon).is_empty());
    assert!(run.state.pdus.iter().any(|r| !r.preload));
    // B never reaches the RA again: week 1 lapses at start(2) + 1 h.
    assert_eq!(periods(&run, DEVICE_B), vec![1]);
    assert_eq!(
        run.app_starvation(DEVICE_B, at, horizon),
        vec![(p.period_start(2) + HOUR, horizon)]
    );
    assert!(run.state.pdu_rejections.is_empty());
    signs_verifiably(&run, DEVICE_A);
}
