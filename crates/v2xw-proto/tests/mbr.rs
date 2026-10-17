//! WP-B4: the US misbehaviour report ([SM-MBR] v1.0, [SM-MBR-ASN] v1.1) end to end on real
//! credentials: a provisioned device reports another device's real signed BSMs, signs the
//! report with its own implicit pseudonym certificate under PSID 0x26, encrypts it to
//! the MA chosen by `relevantPsids`, and the MA opens and checks it.
//!
//! * with `V2XW_PDU_OUT=<dir>` the corpus test writes the reports and the keys an
//!   independent verifier needs, for `tools/pdu_check/check_mbr.py`.

mod common;

use std::collections::BTreeMap;

use common::{DEVICE_A, DEVICE_B, deployment, provisioned};
use v2xw_core::belief::{FixQuality, PositionEstimate};
use v2xw_core::geo::GeoOrigin;
use v2xw_core::geom::{Dims, Vec3};
use v2xw_msg::j2735::bsm::{BsmInput, build_bsm, encode_bsm};
use v2xw_msg::sec_types::ieee1609_dot2::{Certificate, Ieee1609Dot2Data};
use v2xw_proto::scms::calendar;
use v2xw_proto::scms::mbr::{
    self, Evidence, MbrQueue, MbrType, MisbehaviorReport, SignedBsms, Subtype, Weighting,
};
use v2xw_proto::scms::pdu::{Drbg, PduError, ScmsPki};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_sec::envelope::GenerationLocation;
use v2xw_sec::sig1609::Signer1609;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// A device's pseudonym certificate `(0, j)` and its derived signing key.
fn pseudonym(run: &ScmsRun, dev: v2xw_core::ids::NodeId, j: u32) -> (Certificate, Signer1609) {
    let c = run.state.devices[&dev].credentials[&(0, j)]
        .implicit
        .as_ref()
        .expect("real implicit credential");
    let key = Signer1609::p256(&v2xw_sec::ec::scalar_to_be32(&c.private)).expect("key");
    (c.certificate.clone(), key)
}

/// A real UPER BSM at `speed` m/s east, from the J2735 builder.
fn uper_bsm(cnt: u8, x: f64, speed: f64) -> Vec<u8> {
    let origin = GeoOrigin::new(40.7440, -73.9900, 0.0);
    let mut belief = PositionEstimate::no_fix(0);
    belief.pos = Vec3::new(x, 0.0, 0.0);
    belief.vel = Vec3::new(speed, 0.0, 0.0);
    belief.heading_rad = 0.0;
    belief.semi_major_m = 1.8;
    belief.semi_minor_m = 1.1;
    belief.fix = FixQuality::ThreeD;
    let input = BsmInput::new(cnt, [0xb0, 0, 0, cnt], belief, origin, Dims::CAR, 1_000);
    encode_bsm(&build_bsm(&input).expect("builds"))
        .expect("encodes")
        .bytes
}

fn signed_bsms(n: u8, speed: f64, cert: &Certificate, key: &Signer1609) -> Vec<Ieee1609Dot2Data> {
    (0..n)
        .map(|k| {
            mbr::sign_bsm(
                &uper_bsm(k, f64::from(k) * 10.0, speed),
                600_000_000_000 + u64::from(k) * 100_000,
                cert,
                key,
            )
            .expect("signs")
        })
        .collect()
}

const LOC: GenerationLocation = GenerationLocation {
    lat_tenth_microdeg: 407_440_000,
    lon_tenth_microdeg: -739_900_000,
    elevation: 4_096,
};

struct World {
    run: ScmsRun,
    pki: std::sync::Arc<ScmsPki>,
}

fn world() -> World {
    let mut run = deployment(2);
    provisioned(&mut run, DEVICE_A, 0, 1, 2);
    provisioned(&mut run, DEVICE_B, 0, 1, 2);
    let pki = run.state.pki.clone().expect("PDU layer on");
    World { run, pki }
}

fn speeding_report(w: &World) -> MisbehaviorReport {
    let (b_cert, b_key) = pseudonym(&w.run, DEVICE_B, 0);
    MisbehaviorReport {
        generation_time: 600_000,
        location: LOC,
        mbr_type: MbrType::Implausible,
        subtypes: vec![Subtype::MaxSpeed { threshold_mps: 90 }],
        evidence: Evidence::Suspect(SignedBsms {
            bsms: signed_bsms(1, 120.0, &b_cert, &b_key),
            cert: b_cert,
        }),
    }
}

fn secure(w: &World, r: &MisbehaviorReport, g: &mut Drbg) -> Vec<u8> {
    let (a_cert, a_key) = pseudonym(&w.run, DEVICE_A, 0);
    let candidates = [&w.pki.ra.cert, &w.pki.ma.cert];
    let ma = mbr::select_ma(&candidates, mbr::BSM_APP_PSID).expect("an MA covers 0x20");
    assert_eq!(ma, &w.pki.ma.cert, "the RA certificate has no MaSsp");
    mbr::secure_report(
        &mbr::encode_report(r).expect("encodes"),
        600_000_000_000,
        &a_cert,
        &a_key,
        ma,
        &w.pki.ma.enc_public().expect("MA encryption key"),
        g,
    )
    .expect("secures")
}

fn ma_open(w: &World, bytes: &[u8]) -> Result<mbr::OpenedReport, PduError> {
    mbr::ma_open_report(
        bytes,
        &w.pki.ma.cert,
        w.pki.ma.enc_private.as_ref().unwrap(),
        &w.pki,
    )
}

#[test]
fn the_open_type_layout_is_x696() {
    // Hand-derived from X.696 §30 (open type = length ‖ value), §16 (extensible SEQUENCE
    // preamble), §10 (Uint8 / Uint16 fixed width): the subtype list of a MaxSpeed-90
    // report is `01 01` (quantity 1) ‖ `01` (subtype) ‖ `03` ‖ `00 00 5a` (params:
    // preamble, threshold 90).
    let w = world();
    let bytes = mbr::encode_report(&speeding_report(&w)).unwrap();
    let EndEntity::Report(top) = rasn::coer::decode::<EndEntity>(&bytes).unwrap() else {
        panic!("the only root alternative");
    };
    let inner: mbr::Psid20Mbr = rasn::coer::decode(&top.contents).unwrap();
    assert_eq!(inner.subtype.to_vec(), vec![1, 1, 1, 3, 0, 0, 0x5a]);
    // CHOICE tag [0] → 0x80; preamble 00; version 01; Time32 600000 = 00 09 27 c0.
    assert_eq!(&bytes[..7], &[0x80, 0x00, 0x01, 0x00, 0x09, 0x27, 0xc0]);
    // `psid` 0x20 after the 10-byte ThreeDLocation: length 1, value 0x20.
    assert_eq!(&bytes[17..19], &[0x01, 0x20]);

    // Two subtypes, one with NULL params (zero octets, so open-type length 00):
    // quantity `01 02` ‖ RandomPosition `02 03 00 03 e8` (1000 mm) ‖ ConstantPosition `01 00`.
    let (b_cert, b_key) = pseudonym(&w.run, DEVICE_B, 0);
    let r = MisbehaviorReport {
        mbr_type: MbrType::OneVInconsistent,
        subtypes: vec![
            Subtype::RandomPosition {
                gps_drift_mm: 1_000,
            },
            Subtype::ConstantPosition,
        ],
        evidence: Evidence::Suspect(SignedBsms {
            bsms: signed_bsms(2, 20.0, &b_cert, &b_key),
            cert: b_cert,
        }),
        ..speeding_report(&w)
    };
    let bytes = mbr::encode_report(&r).unwrap();
    let EndEntity::Report(top) = rasn::coer::decode::<EndEntity>(&bytes).unwrap() else {
        panic!("the only root alternative");
    };
    let inner: mbr::Psid20Mbr = rasn::coer::decode(&top.contents).unwrap();
    assert_eq!(
        inner.subtype.to_vec(),
        vec![1, 2, 2, 3, 0, 0x03, 0xe8, 1, 0]
    );
    assert_eq!(mbr::decode_report(&bytes).unwrap(), r);
}
use mbr::EndEntityMaInterfacePdu as EndEntity;

#[test]
fn a_device_reports_and_the_ma_opens_it() {
    let w = world();
    let r = speeding_report(&w);
    let mut g = Drbg::new(7, b"mbr-test");
    let bytes = secure(&w, &r, &mut g);
    let o = ma_open(&w, &bytes).expect("the MA opens the report");
    assert_eq!(o.report, r);
    assert!(
        o.evidence_verified,
        "B's BSM verifies under B's certificate"
    );
    assert_eq!(o.signed_at_us, 600_000_000_000);
    assert_eq!(o.reporter_cert, pseudonym(&w.run, DEVICE_A, 0).0);
    // The reporter's identity on the report is a pseudonym, never the enrolment
    // certificate the RA authenticates the upload with.
    let enrol = w.run.state.devices[&DEVICE_A]
        .enrolment_credential
        .as_ref()
        .unwrap();
    assert_ne!(o.reporter_cert, enrol.cert);
}

#[test]
fn the_ma_refuses_tampered_misaddressed_and_wrong_psid_reports() {
    let w = world();
    let r = speeding_report(&w);
    let mut g = Drbg::new(8, b"mbr-test");
    let mut bytes = secure(&w, &r, &mut g);
    let n = bytes.len();
    bytes[n - 20] ^= 1;
    assert!(matches!(ma_open(&w, &bytes), Err(PduError::Decrypt(_))));

    // Encrypted to the RA instead of the MA: not for the MA.
    let (a_cert, a_key) = pseudonym(&w.run, DEVICE_A, 0);
    let to_ra = mbr::secure_report(
        &mbr::encode_report(&r).unwrap(),
        1,
        &a_cert,
        &a_key,
        &w.pki.ra.cert,
        &w.pki.ra.enc_public().unwrap(),
        &mut g,
    )
    .unwrap();
    assert_eq!(ma_open(&w, &to_ra).unwrap_err(), PduError::NotForMe);

    // A signed BSM (psid 0x20) dressed as a report: refused on the PSID.
    let bsm = mbr::sign_bsm(&mbr::encode_report(&r).unwrap(), 1, &a_cert, &a_key).unwrap();
    let wrapped = v2xw_proto::scms::pdu::encrypt_to_cert(
        &rasn::coer::encode(&bsm).unwrap(),
        &w.pki.ma.cert,
        &w.pki.ma.enc_public().unwrap(),
        &mut g,
    )
    .unwrap();
    assert_eq!(ma_open(&w, &wrapped).unwrap_err(), PduError::Psid(0x20));
}

#[test]
fn evidence_the_accuser_signed_does_not_verify() {
    // A framing attack: A builds "B's" BSMs with its own key but attaches B's certificate.
    let w = world();
    let (b_cert, _) = pseudonym(&w.run, DEVICE_B, 0);
    let (_, a_key) = pseudonym(&w.run, DEVICE_A, 1);
    let mut r = speeding_report(&w);
    r.evidence = Evidence::Suspect(SignedBsms {
        bsms: signed_bsms(1, 120.0, &b_cert, &a_key),
        cert: b_cert,
    });
    let mut g = Drbg::new(9, b"mbr-test");
    let o = ma_open(&w, &secure(&w, &r, &mut g)).expect("the report itself is valid");
    assert!(!o.evidence_verified, "forged evidence must be flagged");
}

#[test]
fn the_evidence_count_rules_hold() {
    let w = world();
    let (b_cert, b_key) = pseudonym(&w.run, DEVICE_B, 0);
    let mut r = speeding_report(&w);
    r.evidence = Evidence::Suspect(SignedBsms {
        bsms: signed_bsms(2, 120.0, &b_cert, &b_key),
        cert: b_cert.clone(),
    });
    assert!(
        mbr::encode_report(&r).is_err(),
        "Implausible holds exactly one BSM"
    );
    r.mbr_type = MbrType::OneVInconsistent;
    r.subtypes = vec![Subtype::RandomPosition {
        gps_drift_mm: 1_000,
    }];
    let ok = mbr::encode_report(&r).expect("two BSMs for 1VInconsistent");
    assert_eq!(mbr::decode_report(&ok).unwrap(), r);
    r.subtypes.push(Subtype::MaxSpeed { threshold_mps: 90 });
    assert!(mbr::encode_report(&r).is_err(), "subtypes of one type only");
}

#[test]
fn the_queue_decays_at_five_eastern_and_orders_uploads() {
    let mon = calendar::eastern_to_unix(2026, 10, 19, 4, 0);
    let mut q: MbrQueue<&str> = MbrQueue::new(300);
    let w = Weighting::default();
    q.push("old", 100, w, mon);
    // 04:59 the same morning: no decay yet.
    q.push("same-day", 100, w, mon + 59 * 60);
    assert!(q.iter().all(|e| (e.weight - 1.0).abs() < 1e-12));
    // 05:00 passed once by 06:00: both stored reports are now 0.9.
    let fresh = Weighting {
        initial: 0.95,
        daily_multiplier: 0.9,
    };
    q.push("fresh", 100, fresh, mon + 2 * 3600);
    let weights: BTreeMap<&str, f64> = q.iter().map(|e| (e.item, e.weight)).collect();
    assert!((weights["old"] - 0.9).abs() < 1e-12);
    assert!((weights["fresh"] - 0.95).abs() < 1e-12);
    // Highest weight first.
    assert_eq!(q.next_upload().unwrap().item, "fresh");
    let seq = q.next_upload().unwrap().seq;
    q.confirm(seq);
    // Equal weights: newer first.
    assert_eq!(q.next_upload().unwrap().item, "same-day");
    // Over quota: the lowest weight goes, older first on ties.
    let evicted = q.push("x", 100, w, mon + 3 * 3600);
    assert!(evicted.is_empty());
    let evicted = q.push("y", 100, w, mon + 3 * 3600);
    assert_eq!(evicted, vec!["old"]);
    // Three days later: 0.9³ for the survivors of the first day.
    q.decay_to(mon + 3 * 86_400 + 2 * 3600);
    let sd = q.iter().find(|e| e.item == "same-day").unwrap().weight;
    assert!((sd - 0.9f64.powi(4)).abs() < 1e-12, "{sd}");
    assert_eq!(q.expire(mon + 9 * 86_400).len(), 3);
}

/// Writes the reports and the keys an independent checker needs.
#[test]
fn corpus_for_the_oracle() {
    let Ok(dir) = std::env::var("V2XW_PDU_OUT") else {
        return;
    };
    let dir = std::path::Path::new(&dir).join("mbr");
    std::fs::create_dir_all(&dir).unwrap();
    let w = world();
    let (b_cert, b_key) = pseudonym(&w.run, DEVICE_B, 0);
    let (a_cert, a_key) = pseudonym(&w.run, DEVICE_A, 0);
    let mut g = Drbg::new(10, b"mbr-corpus");
    let reports = vec![
        ("implausible", speeding_report(&w), true),
        (
            "inconsistent",
            MisbehaviorReport {
                generation_time: 600_001,
                location: LOC,
                mbr_type: MbrType::OneVInconsistent,
                subtypes: vec![
                    Subtype::RandomPosition {
                        gps_drift_mm: 1_000,
                    },
                    Subtype::ConstantPosition,
                ],
                evidence: Evidence::Suspect(SignedBsms {
                    bsms: signed_bsms(3, 20.0, &b_cert, &b_key),
                    cert: b_cert.clone(),
                }),
            },
            true,
        ),
        (
            "observed",
            MisbehaviorReport {
                generation_time: 600_002,
                location: LOC,
                mbr_type: MbrType::ObsImplausible,
                subtypes: vec![Subtype::ObsDistance {
                    threshold_m: 3_000,
                    gps_drift_m: 10,
                }],
                evidence: Evidence::Observed {
                    suspect: SignedBsms {
                        bsms: signed_bsms(1, 20.0, &b_cert, &b_key),
                        cert: b_cert.clone(),
                    },
                    reporter: SignedBsms {
                        bsms: signed_bsms(1, 15.0, &a_cert, &a_key),
                        cert: a_cert.clone(),
                    },
                },
            },
            true,
        ),
        (
            "forged-evidence",
            MisbehaviorReport {
                evidence: Evidence::Suspect(SignedBsms {
                    bsms: signed_bsms(1, 120.0, &b_cert, &a_key),
                    cert: b_cert.clone(),
                }),
                ..speeding_report(&w)
            },
            false,
        ),
    ];
    let mut files = Vec::new();
    for (k, (name, r, evidence_ok)) in reports.iter().enumerate() {
        let bytes = secure(&w, r, &mut g);
        let f = format!("{k:02}_{name}.oer");
        std::fs::write(dir.join(&f), &bytes).unwrap();
        files.push(serde_json::json!({
            "file": f,
            "type": r.mbr_type as u8,
            "subtypes": r.subtypes.iter().map(|s| s.number()).collect::<Vec<_>>(),
            "generation_time": r.generation_time,
            "evidence_ok": evidence_ok,
            "reporter_cert": hex(&v2xw_sec::cert::encode(&a_cert).unwrap()),
        }));
    }
    let certs: BTreeMap<String, String> = w
        .pki
        .all()
        .iter()
        .map(|a| {
            (
                a.name.clone(),
                hex(&v2xw_sec::cert::encode(&a.cert).unwrap()),
            )
        })
        .collect();
    let m = serde_json::json!({
        "reports": files,
        "certs": certs,
        "ma_cert": "ma.provider-a",
        "ma_enc_private": hex(&v2xw_sec::ec::scalar_to_be32(w.pki.ma.enc_private.as_ref().unwrap())),
        "aca_cert": "aca.provider-a",
    });
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&m).unwrap(),
    )
    .unwrap();
}
