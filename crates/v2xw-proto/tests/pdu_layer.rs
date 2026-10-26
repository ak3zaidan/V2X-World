//! WP-B1: the enrolment and provisioning exchanges are real IEEE 1609.2.1 SPDUs.
//!
//! * every SPDU the flows emit is the expected type, and none is refused;
//! * invariant I-S3: the bytes each hop is charged are exactly the SPDU's encoded length;
//! * a request signed with an enrolment certificate the deployment's ECA did not issue is
//!   refused by the RA and recorded, and the device gets no ack;
//! * with `V2XW_PDU_OUT=<dir>` the test writes every SPDU and the keys a verifier needs,
//!   for `tools/pdu_check/check_pdus.py` (the oracle's primitives) to decode and verify.

mod common;

use std::collections::BTreeMap;

use common::{DEVICE_A, provisioned};
use v2xw_proto::scms::enrolment;
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::{self, PduError, SpduKind};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::{FlowId, StageId};

fn run_with_bytes() -> ScmsRun {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    let mut run = ScmsRun::new(p).expect("certificates encode");
    run.add_device(DEVICE_A);
    run
}

/// The step label each SPDU kind travels under, device side and proxied.
fn kind_of(step: &str) -> Option<SpduKind> {
    Some(match step {
        "enrol-request" => SpduKind::EeEcaCertRequest,
        "enrol-response"
        | "enrol-response-proxied"
        | "successor-enrol-certified"
        | "successor-enrol-response" => SpduKind::EcaEeCertResponse,
        "successor-enrol-request" => SpduKind::EeRaSuccessorEnrollmentCertRequest,
        "successor-enrol-forward" => SpduKind::EeEcaCertRequest,
        "successor-enrol-ack" => SpduKind::RaEeEnrollmentCertAck,
        "successor-enrol-download-request" => SpduKind::EeRaDownloadRequest,
        "provisioning-request" | "provisioning-request-proxied" => SpduKind::EeRaCertRequest,
        "provisioning-ack" | "provisioning-ack-proxied" => SpduKind::RaEeCertAck,
        "batch-download-request" | "batch-download-request-proxied" => {
            SpduKind::EeRaDownloadRequest
        }
        _ => return None,
    })
}

#[test]
fn enrolment_and_provisioning_are_real_spdus_charged_their_encoded_length() {
    let mut run = run_with_bytes();
    provisioned(&mut run, DEVICE_A, 0, 1, 3);
    let st = &run.state;
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    let mut counts = BTreeMap::new();
    for p in &st.pdus {
        *counts.entry(p.kind).or_insert(0u32) += 1;
    }
    assert_eq!(
        counts,
        BTreeMap::from([
            // Device → DCM, then DCM → ECA: the DCM forwards the device's own request
            // (CAMP122 bootstrap; audit-gov §4 item 4, the second hop was not recorded).
            (SpduKind::EeEcaCertRequest, 2),
            // ECA → DCM, then DCM → device (WP-B2: the DCM relays and registers).
            (SpduKind::EcaEeCertResponse, 2),
            (SpduKind::EeRaCertRequest, 2),
            (SpduKind::RaEeCertAck, 2),
            // WP-B3a: per certificate, two sealed PLVs, one RA→ACA request, one ACA→RA
            // response and the private response filed for the device; one download
            // request, carried over two hops.
            (SpduKind::EncryptedIndividualPlv, 6),
            (SpduKind::RaAcaCertRequest, 3),
            (SpduKind::AcaRaCertResponse, 3),
            // Three files, on the RA-LOP leg and again on the LOP-device leg, as the ack.
            (SpduKind::AcaEeCertResponsePrivate, 6),
            (SpduKind::EeRaDownloadRequest, 2),
        ])
    );
    let dev = &st.devices[&DEVICE_A];
    assert!(dev.enrolment_credential.is_some());
    assert_eq!(dev.credentials.len(), 3, "the batch still installs");

    // I-S3 as equality: every hop that carried an SPDU was charged its length, matched
    // hop by hop on (kind, from, to, time).
    let mut matched = 0;
    for step in &run.kernel.steps {
        let Some(kind) = kind_of(step.step) else {
            continue;
        };
        let p = st
            .pdus
            .iter()
            .find(|p| p.kind == kind && p.from == step.from.index() && p.to == step.to.index())
            .unwrap_or_else(|| panic!("no SPDU recorded for step {}", step.step));
        assert_eq!(step.bytes, p.len, "{}", step.step);
        assert_eq!(p.bytes.as_ref().map(pdu::Pdu::len), Some(p.len));
        matched += 1;
    }
    assert_eq!(matched, 9);

    // The messages that carry several SPDUs are charged exactly their sum (a stored zip
    // for the batch file).
    let sum = |k: SpduKind| -> u32 { st.pdus.iter().filter(|p| p.kind == k).map(|p| p.len).sum() };
    let charged = |name: &str| -> u32 {
        run.kernel
            .steps
            .iter()
            .filter(|s| s.step == name)
            .map(|s| s.bytes)
            .sum()
    };
    assert_eq!(charged("cert-request"), sum(SpduKind::RaAcaCertRequest));
    assert_eq!(charged("cert-response"), sum(SpduKind::AcaRaCertResponse));
    assert_eq!(
        charged("pre-linkage-response"),
        sum(SpduKind::EncryptedIndividualPlv)
    );
    let entries: Vec<(String, u32)> = st
        .pdus
        .iter()
        // The RA's copies: the batch it serves (the device leg carries the same files).
        .filter(|p| p.kind == SpduKind::AcaEeCertResponsePrivate && p.from == st.nodes.ra.index())
        .enumerate()
        .map(|(j, p)| {
            (
                v2xw_proto::scms::pdu_prov::batch_entry_name(0, j as u32),
                p.len,
            )
        })
        .collect();
    assert_eq!(
        charged("batch-download"),
        v2xw_proto::scms::pdu_prov::stored_zip_len(&entries)
    );
    // The RA's folder: one `<requestHash>_<i>.zip` holding `0_0`, `0_1`, `0_2`.
    let files = st.ra.batch_files(DEVICE_A);
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].0.len(), 16 + 6, "{}", files[0].0);
    assert!(files[0].0.ends_with("_0.zip"));
    assert_eq!(files[0].1, ["0_0", "0_1", "0_2"]);
    // Every installed credential is the real implicit one, and usable.
    for ((i, j), c) in &dev.credentials {
        assert!(c.implicit.is_some(), "({i},{j}) is implicit");
        assert!(dev.credential_is_usable(*i, *j));
    }

    // Each SPDU opens with the receiver's rules.
    let pki = st.pki.as_ref().expect("layer on");
    for p in &st.pdus {
        let b = p.bytes.as_ref().unwrap().bytes();
        match p.kind {
            SpduKind::EeEcaCertRequest => {
                // Signed with the canonical key the operator registered (WP-B2).
                let canonical = &st.eca.canonical;
                let (_, auth) =
                    enrolment::open_ee_eca_cert_request(b, &|id| canonical.get(id).copied())
                        .expect("canonical-signed request");
                assert!(matches!(auth, enrolment::EnrolAuth::Canonical(_)));
            }
            SpduKind::EeRaSuccessorEnrollmentCertRequest => {
                enrolment::ra_open_successor_request(pki, b).expect("to the RA");
            }
            SpduKind::RaEeEnrollmentCertAck => {
                pdu::open_signed_pdu(b, pki).expect("signed");
            }
            SpduKind::EcaEeCertResponse | SpduKind::RaEeCertAck => {
                pdu::open_signed_pdu(b, pki).expect("signed");
            }
            SpduKind::EeRaCertRequest => {
                let plain = pdu::decrypt_cert(b, &pki.ra.cert, &pki.ra.enc_private.unwrap())
                    .expect("to the RA");
                pdu::open_signed_cert_request(&plain, pki).expect("enrolment-signed");
            }
            SpduKind::EeRaDownloadRequest => {
                let plain = pdu::decrypt_cert(b, &pki.ra.cert, &pki.ra.enc_private.unwrap())
                    .expect("to the RA");
                pdu::open_signed_pdu(&plain, pki).expect("enrolment-signed");
            }
            SpduKind::RaAcaCertRequest => {
                pdu::open_signed_cert_request(b, pki).expect("RA-signed");
            }
            SpduKind::AcaRaCertResponse => {
                v2xw_proto::scms::pdu_prov::ra_open_aca_response(pki, b).expect("ACA-signed");
            }
            SpduKind::AcaEeCertResponsePrivate => {
                pdu::open_signed(b, pki).expect("ACA-signed");
            }
            SpduKind::EncryptedIndividualPlv | SpduKind::RaEeCertInfo => {}
            // WP-B4 kinds (not emitted by this flow; tests/crl_issuance.rs, tests/mbr.rs).
            SpduKind::SecuredCrl => {
                v2xw_sec::crl::decode_crl(b).expect("a SecuredCrl");
            }
            SpduKind::SecuredMisbehaviorReport => {
                v2xw_proto::scms::mbr::ma_open_report(
                    b,
                    &pki.ma.cert,
                    pki.ma.enc_private.as_ref().unwrap(),
                    pki,
                )
                .expect("to the MA");
            }
        }
    }
    if let Ok(dir) = std::env::var("V2XW_PDU_OUT") {
        write_corpus(&run, std::path::Path::new(&dir));
    }
}

#[test]
fn a_request_signed_by_an_enrolment_certificate_from_another_pki_is_refused() {
    let mut run = run_with_bytes();
    run.enrol(DEVICE_A);
    run.run().expect("enrolment runs");
    // The attacker: a credential issued by a different ECA (another deployment's keys).
    let rogue = pdu::ScmsPki::new(99, 600_000_000).unwrap();
    let mut g = pdu::Drbg::new(5, b"rogue");
    let k_u = g.scalar();
    let r_u = v2xw_sec::ec::Point::mul_base(&k_u);
    let real_tbs = run.state.devices[&DEVICE_A]
        .enrolment_credential
        .as_ref()
        .unwrap()
        .cert
        .0
        .to_be_signed
        .clone();
    let (cert, r) = pdu::issue_enrolment(&rogue, &real_tbs, &r_u, &mut g).unwrap();
    let forged = pdu::reconstruct_enrolment(&rogue, &cert, &k_u, &r).unwrap();
    run.state
        .devices
        .get_mut(&DEVICE_A)
        .unwrap()
        .enrolment_credential = Some(forged);
    let prov = run.provision(DEVICE_A, 0, 1, 2);
    run.run().expect("runs");
    let rej = &run.state.pdu_rejections;
    assert_eq!(rej.len(), 1, "{rej:?}");
    assert_eq!(rej[0].kind, SpduKind::EeRaCertRequest);
    assert_eq!(rej[0].reason, PduError::UntrustedSigner);
    assert!(run.state.devices[&DEVICE_A].credentials.is_empty());
    let stages = run.kernel.stages.stages(prov);
    assert!(!stages.contains(&StageId::Acknowledged), "{stages:?}");
    assert!(
        !run.state
            .pdus
            .iter()
            .any(|p| p.kind == SpduKind::RaEeCertAck),
        "no ack for a refused request"
    );
    let _ = FlowId::Provisioning;
}

#[test]
fn the_layer_off_falls_back_to_the_modelled_sizes() {
    let mut p = ScmsParams::default().quick();
    p.pdu.enabled = false;
    let mut run = ScmsRun::new(p).expect("certificates encode");
    run.add_device(DEVICE_A);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    assert!(run.state.pki.is_none() && run.state.pdus.is_empty());
    let req = run
        .kernel
        .steps
        .iter()
        .find(|s| s.step == "provisioning-request")
        .unwrap();
    assert_eq!(req.bytes, run.state.sizes.provisioning_request().bytes());
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// Writes `NN_<kind>.oer` per SPDU and `manifest.json` with every PKI certificate and the
/// RA's encryption private key, for the independent check.
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
        .map(|a| {
            (
                a.name.clone(),
                hex(&v2xw_sec::cert::encode(&a.cert).unwrap()),
            )
        })
        .collect();
    // WP-B3a: what the device and the LA↔ACA link hold, so the oracle can open every
    // provisioning PDU itself: the caterpillar, each LA's key for the epoch it sealed in,
    // and the (i, j) each filed private response is for.
    let st = &run.state;
    let cat = st.devices[&DEVICE_A].caterpillar.as_ref().unwrap();
    let keys = st.la_aca.unwrap();
    let t_plv = st
        .pdus
        .iter()
        .find(|p| p.kind == SpduKind::EncryptedIndividualPlv)
        .map(|p| st.time32(p.at))
        .unwrap();
    let ij: Vec<[u32; 2]> = st
        .ra
        .batch_files(DEVICE_A)
        .iter()
        .flat_map(|(_, entries)| entries.clone())
        .map(|e| {
            let (i, j) = e.split_once('_').unwrap();
            [
                u32::from_str_radix(i, 16).unwrap(),
                u32::from_str_radix(j, 16).unwrap(),
            ]
        })
        .collect();
    let m = serde_json::json!({
        "pdus": files,
        "certs": certs,
        "ra_cert": "ra.provider-a",
        "ra_enc_private": hex(&v2xw_sec::ec::scalar_to_be32(&pki.ra.enc_private.unwrap())),
        "canonical": run.state.eca.canonical.iter()
            .map(|(id, q)| (id.clone(), hex(&q.compressed().unwrap())))
            .collect::<BTreeMap<_, _>>(),
        "device": {
            "a": hex(&v2xw_sec::ec::scalar_to_be32(cat.a())),
            "p": hex(&v2xw_sec::ec::scalar_to_be32(cat.p())),
            "ck": hex(cat.ck()),
            "ek": hex(cat.ek()),
        },
        "la_aca_keys": {
            "1": hex(&keys.current(1, t_plv)),
            "2": hex(&keys.current(2, t_plv)),
        },
        "private_ij": ij,
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&m).unwrap(),
    )
    .unwrap();
}

/// The PSIDs of the `appPermissions` on each of `device`'s installed implicit certificates.
fn issued_psids(run: &ScmsRun, device: v2xw_core::ids::NodeId) -> Vec<Vec<String>> {
    run.state.devices[&device]
        .credentials
        .values()
        .map(|c| {
            let cert = &c.implicit.as_ref().expect("implicit").certificate;
            let perms = cert.0.to_be_signed.app_permissions.as_ref().expect("perms");
            perms.0.iter().map(|p| p.psid.0.to_string()).collect()
        })
        .collect()
}

/// A device with its own permission profile (a J2945/9 personal device: PSM, PSID 0x27,
/// and the misbehaviour-report PSID 0x26) asks for exactly those PSIDs in its
/// `EeRaCertRequest`, the RA forwards them to the ACA, and every pseudonym certificate
/// it installs lists them; a vehicle in the same run keeps the fleet's profile.
///
/// Fault injected: `ScmsState::pseudonym_psids_of` returning the fleet list for every
/// device (the defect of docs/benchmark/scms-real-benchmark.md §9.1) — the handset's
/// certificates list 32 and 38 and this test fails.
#[test]
fn a_device_profile_reaches_the_aca_and_every_certificate_it_installs() {
    use common::DEVICE_B;
    use v2xw_proto::scms::params::PsidList;
    let mut run = run_with_bytes();
    run.set_device_psids(DEVICE_B, PsidList::of(&[39, 38]));
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    provisioned(&mut run, DEVICE_B, 0, 1, 2);
    let vehicle = issued_psids(&run, DEVICE_A);
    let handset = issued_psids(&run, DEVICE_B);
    assert_eq!(vehicle.len(), 2, "{vehicle:?}");
    assert_eq!(handset.len(), 2, "{handset:?}");
    for p in &vehicle {
        assert_eq!(p, &["32", "38"], "the fleet profile");
    }
    for p in &handset {
        assert_eq!(p, &["38", "39"], "the handset's own profile");
    }
}
