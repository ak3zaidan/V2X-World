//! WP-B2 acceptance: enrolment, registration and re-enrolment with the real 1609.2.1
//! messages.
//!
//! * the bootstrap's enrolment certificate is the US profile (name, region 840/124/484,
//!   series 4, wildcard request permissions), requested under the canonical key, and the
//!   DCM registers it with the RA, which serves only registered certificates;
//! * the ECA refuses a second bootstrap of a name it already issued (a cloned identity);
//! * calendar: the defaults are a 6-year lifetime and an 18-month lead (inside 12–24
//!   months); a successor requested at the lead starts exactly when the current
//!   certificate ends — the device signs with the old certificate until the last
//!   nanosecond and with the successor from then on, with no gap and no overlap;
//! * a second successor request before the download supersedes the first, and the
//!   successor issued for the first is blocklisted;
//! * with `V2XW_PDU_OUT=<dir>` the successor test writes every SPDU and the keys a
//!   verifier needs, for `tools/pdu_check/check_pdus.py` (the oracle's primitives).

mod common;

use std::collections::BTreeMap;

use common::{DEVICE_A, DEVICE_B, provisioned};
use v2xw_core::time::Duration;
use v2xw_proto::scms::enrolment::{self, EnrolAuth, RA_URL};
use v2xw_proto::scms::msg::Refusal;
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::{self, PduError, SpduKind};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::StageId;
use v2xw_msg::sec_types::ieee1609_dot2::CertificateType;
use v2xw_sec::ctl;

const DAY_S: u64 = 86_400;
const NS: u64 = 1_000_000_000;

fn run_with_bytes() -> ScmsRun {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    let mut run = ScmsRun::new(p).expect("certificates encode");
    run.add_device(DEVICE_A);
    run.add_device(DEVICE_B);
    run
}

fn last_pdu(run: &ScmsRun, kind: SpduKind) -> pdu::Pdu {
    run.state
        .pdus
        .iter()
        .rev()
        .find(|p| p.kind == kind)
        .and_then(|p| p.bytes.clone())
        .unwrap_or_else(|| panic!("no {kind:?}"))
}

#[test]
fn the_bootstrap_issues_the_us_enrolment_certificate_and_registers_it_with_the_ra() {
    let mut run = run_with_bytes();
    run.enrol(DEVICE_A);
    run.run().expect("enrolment runs");
    let st = &run.state;
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    let d = &st.devices[&DEVICE_A];
    let cred = d.enrolment_credential.as_ref().expect("a real credential");
    let t = &cred.cert.0.to_be_signed;
    assert_eq!(cred.cert.0.r_type, CertificateType::implicit);
    let name = enrolment::tbs_name(t).expect("id.name").to_string();
    assert_eq!(d.name.as_deref(), Some(name.as_str()));
    assert_eq!(t.crl_series.0.0, 4);
    assert_eq!(
        t.cert_request_permissions,
        Some(enrolment::wildcard_request_permissions())
    );
    assert!(t.app_permissions.is_none() && t.cert_issue_permissions.is_none());
    let (s, e) = ctl::validity(&cred.cert);
    assert_eq!(e - s, st.params.enrolment_lifetime.as_nanos() / NS);
    // The ECA's own check of the profile accepts what it issued (region included).
    let canonical = &st.eca.canonical;
    let req = last_pdu(&run, SpduKind::EeEcaCertRequest);
    let (r, auth) =
        enrolment::open_ee_eca_cert_request(req.bytes(), &|id| canonical.get(id).copied())
            .expect("canonical-signed");
    assert!(matches!(auth, EnrolAuth::Canonical(_)), "direct authorisation");
    assert_eq!(r.tbs_cert.region, t.region);
    assert_eq!(st.eca.names.get(&name), Some(&DEVICE_A));
    // Registered with the RA by the DCM, and the device knows where the RA is.
    assert!(st.ra.is_registered(DEVICE_A, Some(pdu::cert_id8(&cred.cert))));
    assert!(!st.ra.is_registered(DEVICE_B, None));
    assert_eq!(d.ra_url.as_deref(), Some(RA_URL));
    for role in ["root", "ra", "ma", "eca"] {
        assert!(d.trust.trusts_role(role), "{role}");
    }
}

#[test]
fn the_ra_serves_only_registered_enrolment_certificates() {
    let mut run = run_with_bytes();
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    // The EE operator never registered it (or the registration was lost).
    run.state.ra.registered.remove(&DEVICE_A);
    let t = run.topup(DEVICE_A, 1, 2);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(t, StageId::Installed).is_none());
    assert_eq!(run.topup_refusal_of(DEVICE_A), Some(Refusal::UnknownEnrolment));
    assert_eq!(run.state.ra.unregistered_refused, 1);
    // Nor will it forward a successor request signed by that certificate.
    let r = run.reenrol_at(DEVICE_A, run.kernel.now());
    run.run().expect("runs");
    assert!(run.kernel.stages.at(r, StageId::Installed).is_none());
    assert_eq!(run.state.ra.unregistered_refused, 2);
    assert_eq!(run.state.ra.successor_requests, 0);
}

#[test]
fn the_eca_refuses_a_second_bootstrap_of_a_name_it_already_issued() {
    let mut run = run_with_bytes();
    run.enrol(DEVICE_A);
    run.run().expect("enrolment runs");
    let first = run.state.devices[&DEVICE_A]
        .enrolment_credential
        .clone()
        .expect("enrolled");
    // The same device identity bootstrapped again (a clone, or a replayed factory step).
    run.enrol(DEVICE_A);
    run.run().expect("runs");
    let st = &run.state;
    assert_eq!(st.eca.profile_refused, 1);
    assert_eq!(st.eca.issued, 1);
    assert!(
        st.pdu_rejections.iter().any(|r| r.kind == SpduKind::EeEcaCertRequest
            && matches!(r.reason, PduError::Policy(w) if w.starts_with("id.name is unique"))),
        "{:?}",
        st.pdu_rejections
    );
    let d = &st.devices[&DEVICE_A];
    assert_eq!(d.refused, Some(Refusal::ProfileViolation));
    assert_eq!(
        d.enrolment_credential.as_ref().map(|c| pdu::cert_id8(&c.cert)),
        Some(pdu::cert_id8(&first.cert)),
        "the first certificate stays"
    );
}

/// The i-period that starts after `t`.
fn period_after(p: &ScmsParams, t: u64) -> u32 {
    let mut i = u32::try_from(t / p.i_period.as_nanos()).unwrap();
    while p.period_start(i) <= t {
        i += 1;
    }
    i
}

/// The enrolment certificate that signed the RA's last `EeRaCertRequestSpdu`.
fn last_request_signer(run: &ScmsRun) -> [u8; 8] {
    let pki = run.state.pki.as_ref().unwrap();
    let b = last_pdu(run, SpduKind::EeRaCertRequest);
    let plain = pdu::decrypt_cert(b.bytes(), &pki.ra.cert, &pki.ra.enc_private.unwrap()).unwrap();
    match pdu::open_signed_cert_request(&plain, pki).unwrap().1 {
        pdu::RequestSignerFound::Cert(c) => pdu::cert_id8(&c),
        pdu::RequestSignerFound::SelfSigned => panic!("self-signed"),
    }
}

#[test]
fn a_successor_requested_at_the_lead_takes_over_exactly_at_expiry() {
    // Calendar: 6 years (CAMP 1.2.2) and 18 months (SM-REENR's 12–24).
    let p = ScmsParams::default();
    assert_eq!(p.enrolment_lifetime.as_nanos(), 6 * 365 * DAY_S * NS);
    let lead_days = p.reenrol_lead.as_nanos() / NS / DAY_S;
    assert!((365..=730).contains(&lead_days), "{lead_days} days");
    assert_eq!(lead_days, 548);

    let mut run = run_with_bytes();
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let current = run.state.devices[&DEVICE_A].enrolment.expect("enrolled");
    let old = run.state.devices[&DEVICE_A]
        .enrolment_credential
        .clone()
        .unwrap();
    let expiry = current.valid_until;
    let due = expiry - run.state.params.reenrol_lead.as_nanos();
    let r = run.reenrol_at(DEVICE_A, due);
    run.run().expect("runs");
    assert_eq!(
        run.kernel.stages.stages(r),
        vec![
            StageId::Requested,
            StageId::Acknowledged,
            StageId::Certified,
            StageId::Installed
        ]
    );
    // Stamped when the device finishes the step (key generation and two signatures).
    let requested = run.kernel.stages.at(r, StageId::Requested).unwrap();
    assert!((due..due + NS).contains(&requested), "requested at the lead");
    let downloaded = run.kernel.stages.at(r, StageId::Installed).unwrap();
    assert!(
        downloaded >= due + run.state.params.reenrol_download_delay.as_nanos(),
        "two-phase: the download waits for nextDlTime"
    );
    let st = &run.state;
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    let d = &st.devices[&DEVICE_A];
    let next = d.successor.expect("held");
    assert_eq!((next.generation, next.valid_from), (1, expiry));
    assert_eq!(next.valid_until - next.valid_from, st.params.enrolment_lifetime.as_nanos());
    assert_eq!(d.enrolment, Some(current), "the current certificate is still in force");
    // The real certificates agree: the successor starts at the current one's last second.
    let new = d.successor_credential.clone().expect("real successor");
    assert_eq!(ctl::validity(&new.cert).0, ctl::validity(&old.cert).1);
    assert_eq!(enrolment::tbs_name(&new.cert.0.to_be_signed), d.name.as_deref());
    // The RA linked old and new.
    assert!(st.ra.is_registered(DEVICE_A, Some(pdu::cert_id8(&old.cert))));
    assert!(st.ra.is_registered(DEVICE_A, Some(pdu::cert_id8(&new.cert))));
    if let Ok(dir) = std::env::var("V2XW_PDU_OUT") {
        write_corpus(&run, std::path::Path::new(&dir));
    }

    // A second before expiry the device signs with the old certificate; at expiry with the
    // successor. Both requests are accepted: no gap. (A request signed a nanosecond before
    // expiry reaches the RA after it and is refused as expired: the RA checks validity on
    // receipt.)
    let jmax = 2;
    let before = run.topup_at(DEVICE_A, expiry - NS, period_after(&run.state.params, expiry), jmax);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(before, StageId::Acknowledged).is_some());
    assert_eq!(last_request_signer(&run), pdu::cert_id8(&old.cert));
    let at = run.topup_at(DEVICE_A, expiry, period_after(&run.state.params, expiry) + 1, jmax);
    run.run().expect("runs");
    assert!(run.kernel.stages.at(at, StageId::Acknowledged).is_some());
    assert_eq!(last_request_signer(&run), pdu::cert_id8(&new.cert));
    let d = &run.state.devices[&DEVICE_A];
    assert_eq!(d.enrolment.map(|e| e.generation), Some(1));
    assert!(d.successor.is_none());
    assert_eq!(run.state.ra.refused_enrolment, 0);
}

#[test]
fn a_second_successor_request_before_download_supersedes_the_first_and_blocklists_it() {
    let mut run = run_with_bytes();
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    let now = run.kernel.now();
    run.reenrol_at(DEVICE_A, now);
    let second = run.reenrol_at(DEVICE_A, Duration::from_secs(1).after(now));
    run.run().expect("runs");
    let st = &run.state;
    assert_eq!(st.ra.successor_requests, 2);
    assert_eq!(st.ra.successors_superseded, 1);
    assert_eq!(st.ra.successors_blocklisted.len(), 1, "the first was already issued");
    assert_eq!(st.eca.successors, 2);
    assert!(run.kernel.stages.at(second, StageId::Installed).is_some());
    let d = &st.devices[&DEVICE_A];
    let held = pdu::cert_id8(&d.successor_credential.as_ref().unwrap().cert);
    assert!(!st.ra.successors_blocklisted.contains(&held));
    let dead = *st.ra.successors_blocklisted.iter().next().unwrap();
    assert!(!st.ra.is_registered(DEVICE_A, Some(dead)));
    assert!(st.ra.is_registered(DEVICE_A, Some(held)));
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// `NN_<kind>.oer` per SPDU plus `manifest.json`, in the layout `check_pdus.py` reads.
fn write_corpus(run: &ScmsRun, dir: &std::path::Path) {
    std::fs::create_dir_all(dir).unwrap();
    let pki = run.state.pki.as_ref().unwrap();
    let mut files = Vec::new();
    for (k, p) in run.state.pdus.iter().enumerate() {
        let name = format!("{k:02}_{}.oer", p.kind.name());
        std::fs::write(dir.join(&name), p.bytes.as_ref().unwrap().bytes()).unwrap();
        files.push(
            serde_json::json!({"file": name, "kind": p.kind.name(), "from": p.from, "to": p.to}),
        );
    }
    let certs: BTreeMap<String, String> = pki
        .all()
        .iter()
        .map(|a| (a.name.clone(), hex(&v2xw_sec::cert::encode(&a.cert).unwrap())))
        .collect();
    let m = serde_json::json!({
        "pdus": files,
        "certs": certs,
        "ra_cert": "ra.provider-a",
        "ra_enc_private": hex(&v2xw_sec::ec::scalar_to_be32(&pki.ra.enc_private.unwrap())),
        "canonical": run.state.eca.canonical.iter()
            .map(|(id, q)| (id.clone(), hex(&q.compressed().unwrap())))
            .collect::<BTreeMap<_, _>>(),
    });
    std::fs::write(dir.join("manifest.json"), serde_json::to_string_pretty(&m).unwrap()).unwrap();
}
