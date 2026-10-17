//! Real-world cross-check: the CTLs, certificate-chain files and elector certificates
//! SCMS Manager publishes (https://www.scmsmanager.org/publications/, read 2026-10-16;
//! 01 §2.2, §3.1; gap row 14; WP-A5).
//!
//! The files are fetched, not committed (no licence on the page; see
//! `third_party/scms-artefacts/PROVENANCE.md`), so these tests are `#[ignore]`d:
//!
//! ```text
//! python3 -I tools/scms-artefacts/fetch.py      # verifies every file's published SHA-384
//! cargo test -p v2xw-sec --test real_artefacts -- --ignored
//! ```
//!
//! The independent anchors are SCMS Manager's own published SHA-384 fingerprints of the
//! ten elector certificates (transcribed below) and the separately published root
//! certificate files of the 2025-07-17 test CTL. Our `HashedId48` of each attached
//! elector certificate must equal its published fingerprint, our `HashedId32` of each
//! root must equal both the CTL's `rootCaApprove` entry and the SHA-256 of the
//! separately published root file, and every P-384 elector signature and every P-256
//! certificate signature must verify. Flipping any covered byte must break it.

use std::path::PathBuf;

use v2xw_msg::sec_types::Certificate;
use v2xw_sec::ctl::{
    self, ChainLink, Ctl, CtlTrust, check_chain, check_roots, decode_certificate_chain,
    decode_ctl_spdu, verify_ctl,
};
use v2xw_sec::sig1609::{cert_hashed_id32, cert_hashed_id48, verify_explicit_cert};

const IGNORE: &str = "needs tools/scms-artefacts/fetch.py";

/// Published SHA-384 fingerprints of the production elector certificates
/// (scmsmanager.org/publications, "Elector certificates (SHA 384 fingerprint)").
const PROD_ELECTORS: [&str; 5] = [
    "2621351ce48cd2dde9e7a2c21a0c6831af5ff8e348887fc64b98cc7066d1dd8ade4ed9f290b343ca5bc9586e217980f4",
    "b29cea24e45d1bf80c6771a1ba2c9d602d8520fc49226a9e861093922562a07298f659218ab9f3a58fe0d1cfe1498fb6",
    "ae574cc2ea73afd2ad1461114a92ac9faa98b533251deb993f3f717b437ff8f520211c63bd4cb3af991fffc80a0c0236",
    "649f1f960d54a4af55249faf055fc3645144f8081e50490855cb52e022af7b24c9e49c5cbd64fa47d7efc29e2c4fd699",
    "8af5944670d1faad01ef67b9a49f0a528acecec6bad377401b5c24d253c563ad57e348d7af20de33a64775d3c8e723e5",
];
/// Published SHA-384 fingerprints of the test elector certificates (same page, "Test
/// elector certificates (SHA 384)").
const TEST_ELECTORS: [&str; 5] = [
    "f0a2b93f6b27ed5b6b59655f82698f10a02e373b01488539ce02eb60857b492547f282f1e830e437a77b28daf47aa53b",
    "42d4cc9dc47f8ec105cea034cf4078969f7044ccbaa078d4d3481dc639238db7d5687e2d970873ffd1327899a869e489",
    "893e3f96aa821c6319c3bd9004722996f6f03c2ce3565401b7175f8f57be11b6a81372e724211db8253375d819624b1a",
    "5cbe8d326c9ce52b83ecfca2b98ae65e09c947c2c1c8d88dc02f277cf6468f7eb491382b1c1144310aa4f60198d935bb",
    "76d5b5e5105d0d9878fbd2b674fa93d25221060a9ce20e0de1df9df0b8baef6b655e9f77a7ccf4195965d66d0b46049c",
];

fn file(rel: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/scms-artefacts/files")
        .join(rel);
    std::fs::read(&p).unwrap_or_else(|e| {
        panic!(
            "{}: {e}; run python3 -I tools/scms-artefacts/fetch.py",
            p.display()
        )
    })
}

fn hex48(s: &str) -> [u8; 48] {
    let mut o = [0u8; 48];
    for (i, b) in o.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap();
    }
    o
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn trust(fps: &[&str; 5]) -> CtlTrust {
    CtlTrust::us_bootstrap(fps.iter().map(|s| hex48(s)))
}

fn sha256(b: &[u8]) -> [u8; 32] {
    v2xw_core::hash::sha256(b)
}

/// Each published elector certificate decodes, is a self-signed P-384 elector
/// certificate, and our HashedId48 of it equals SCMS Manager's published fingerprint.
#[test]
#[ignore = "needs tools/scms-artefacts/fetch.py"]
fn published_elector_certificates_hash_to_their_fingerprints() {
    let _ = IGNORE;
    for (kind, fps) in [("", &PROD_ELECTORS), (".test", &TEST_ELECTORS)] {
        for (i, fp) in fps.iter().enumerate() {
            let b = file(&format!(
                "electors/v2xelector{}{kind}.certificate.oer",
                i + 1
            ));
            let c: Certificate = rasn::coer::decode(&b).unwrap();
            assert_eq!(
                rasn::coer::encode(&c).unwrap(),
                b,
                "elector {i}{kind} canonical"
            );
            assert_eq!(
                hex(&cert_hashed_id48(&c).unwrap()),
                *fp,
                "elector {}{kind}",
                i + 1
            );
            assert!(ctl::elector_cert_ok(&c), "elector {}{kind}", i + 1);
        }
    }
}

fn check_ctl(ctl: &Ctl, fps: &[&str; 5]) -> ctl::CtlVerdict {
    let v = verify_ctl(ctl, &trust(fps));
    for e in &v.electors {
        assert!(e.ok(), "{e:?}");
    }
    assert_eq!(v.valid_signatures, 5, "{v:?}");
    assert!(v.accepted());
    assert_eq!(ctl.series(), ctl::US_CTL_SERIES_ID);
    assert_eq!(ctl.quorum(), Some(3));
    // electorApprove is exactly the five published electors.
    let mut want: Vec<[u8; 48]> = fps.iter().map(|s| hex48(s)).collect();
    let mut got = ctl.elector_approve();
    want.sort();
    got.sort();
    assert_eq!(got, want);
    v
}

/// The production CTL of 2025-08-13: decodes, 5 of 5 elector P-384 signatures verify
/// against the published elector fingerprints, and both embedded roots are approved by
/// their HashedId32 and self-signed.
#[test]
#[ignore = "needs tools/scms-artefacts/fetch.py"]
fn production_ctl_2025_08_13_verifies() {
    let b = file("production-2025-08-13/production_ctl.oer");
    let ctl = decode_ctl_spdu(&b).unwrap();
    assert_eq!(
        ctl::encode_ctl_spdu(&ctl.raw).unwrap(),
        b,
        "re-encodes byte for byte"
    );
    check_ctl(&ctl, &PROD_ELECTORS);
    assert_eq!((ctl.sequence(), ctl.effective_date()), (1, 679_737_605));
    assert!(ctl.elector_remove().is_empty() && ctl.root_ca_remove().is_empty());
    let roots = check_roots(&ctl).unwrap();
    let names: Vec<_> = roots
        .iter()
        .map(|r| r.name.clone().unwrap_or_default())
        .collect();
    println!("production 2025-08-13 roots: {names:?}");
    assert_eq!(roots.len(), 2);
    assert_eq!(ctl.root_ca_approve().len(), 2);
    assert!(
        roots.iter().all(|r| r.approved && r.self_signed_ok),
        "{roots:?}"
    );
}

/// The test CTL of 2025-07-17: 5 of 5 test elector signatures, sequence 3, quorum 3;
/// five of the six carried roots are approved, and each approved HashedId32 is the
/// SHA-256 of the separately published root file.
#[test]
#[ignore = "needs tools/scms-artefacts/fetch.py"]
fn test_ctl_2025_07_17_verifies_and_its_roots_match_the_published_root_files() {
    let b = file("test-2025-07-17/test_ctl.oer");
    let ctl = decode_ctl_spdu(&b).unwrap();
    assert_eq!(ctl::encode_ctl_spdu(&ctl.raw).unwrap(), b);
    check_ctl(&ctl, &TEST_ELECTORS);
    assert_eq!((ctl.sequence(), ctl.effective_date()), (3, 679_737_605));
    let roots = check_roots(&ctl).unwrap();
    assert_eq!(roots.len(), 6);
    assert!(roots.iter().all(|r| r.self_signed_ok));
    let unapproved: Vec<_> = roots
        .iter()
        .filter(|r| !r.approved)
        .map(|r| r.name.clone())
        .collect();
    println!("test 2025-07-17 roots not on rootCaApprove: {unapproved:?}");
    assert_eq!(unapproved.len(), 1);
    let mut approved = ctl.root_ca_approve();
    approved.sort();
    let mut files: Vec<[u8; 32]> = [
        "AutocryptTestCTLRoot.oer",
        "cht-scms.rca.cert",
        "iss_preprod_root.oer",
        "microsecRCA.oer",
        "saesol_rca_09F453B62DE0813A.oer",
    ]
    .iter()
    .map(|n| {
        let rb = file(&format!("test-2025-07-17/roots/{n}"));
        let c: Certificate = rasn::coer::decode(&rb).unwrap();
        assert_eq!(
            cert_hashed_id32(&c).unwrap(),
            sha256(&rb),
            "{n}: our HashedId32"
        );
        assert!(verify_explicit_cert(&c, None), "{n}");
        sha256(&rb)
    })
    .collect();
    files.sort();
    assert_eq!(approved, files);
}

/// Every published CTL verifies 5 of 5, and each series installs in publication order:
/// a device provisioned with the five electors and quorum 3 accepts sequence 0, then
/// each next CTL under the trust the previous one left (`quorum` is absent in sequence
/// 0, so 3 carries over). Replaying an older CTL after a newer one is refused.
#[test]
#[ignore = "needs tools/scms-artefacts/fetch.py"]
fn published_ctl_series_install_in_order() {
    // (file, sequence, effectiveDate, quorum, roots carried, roots approved)
    type Row = (&'static str, u16, u32, Option<u32>, usize, usize);
    let prod: [Row; 2] = [
        (
            "production-2025-08/production_ctl.oer",
            0,
            503_222_405,
            None,
            1,
            1,
        ),
        (
            "production-2025-08-13/production_ctl.oer",
            1,
            679_737_605,
            Some(3),
            2,
            2,
        ),
    ];
    let test: [Row; 4] = [
        ("test-2023/test_ctl.oer", 0, 503_222_405, None, 1, 1),
        (
            "test-2024-04-19/test_ctl.oer",
            1,
            640_624_233,
            Some(3),
            6,
            6,
        ),
        (
            "test-2025-03-18/test_ctl.oer",
            2,
            669_060_983,
            Some(3),
            6,
            6,
        ),
        (
            "test-2025-07-17/test_ctl.oer",
            3,
            679_737_605,
            Some(3),
            6,
            5,
        ),
    ];
    for (rows, fps) in [(&prod[..], &PROD_ELECTORS), (&test[..], &TEST_ELECTORS)] {
        let mut t = trust(fps);
        let mut installed: Vec<Ctl> = Vec::new();
        for &(rel, seq, eff, quorum, carried, approved) in rows {
            let ctl = decode_ctl_spdu(&file(rel)).unwrap();
            assert_eq!(
                (ctl.sequence(), ctl.effective_date(), ctl.quorum()),
                (seq, eff, quorum),
                "{rel}"
            );
            assert_eq!(ctl.certificates.len(), carried, "{rel}");
            assert_eq!(ctl.root_ca_approve().len(), approved, "{rel}");
            let roots = check_roots(&ctl).unwrap();
            assert!(roots.iter().all(|r| r.self_signed_ok), "{rel}");
            let eviden = Some("https://rca.plugfest.c-its-pki.org".to_string());
            for r in &roots {
                let quirk = r.name == eviden && seq == 2 && rows.len() == 4;
                assert_eq!(r.approved_by_sha256 && !r.approved, quirk, "{rel}: {r:?}");
            }
            let either = roots
                .iter()
                .filter(|r| r.approved || r.approved_by_sha256)
                .count();
            assert_eq!(either, approved, "{rel}");
            let v = verify_ctl(&ctl, &t);
            assert_eq!(v.valid_signatures, 5, "{rel}: {v:?}");
            assert!(v.accepted(), "{rel}: {v:?}");
            t = ctl::trust_after(&ctl, &t);
            assert_eq!(t.quorum, 3);
            installed.push(ctl);
        }
        let v = verify_ctl(&installed[0], &t);
        assert!(
            !v.sequence_ok && !v.accepted(),
            "a replayed older CTL is refused"
        );
    }
}

/// Checks a chain file against `anchor`, the SCMS Manager CTL its root must be on, and
/// returns (certificates, the home CTL's series).
fn check_chain_file(rel: &str, anchor: &Ctl) -> (usize, [u8; 8]) {
    let (home, others) = decode_certificate_chain(&file(rel)).unwrap();
    // The home CTL must at least be self-consistent: a quorum of its own approved
    // electors signs it.
    let own = CtlTrust {
        series: home.series(),
        electors: home.elector_approve().into_iter().collect(),
        quorum: home.quorum().unwrap_or(ctl::US_QUORUM),
        sequence: None,
        roots: Default::default(),
    };
    let v = verify_ctl(&home, &own);
    assert!(v.accepted(), "{rel}: home CTL {v:?}");
    let checks = check_chain(anchor, &others).unwrap();
    for c in &checks {
        assert!(
            matches!(c.link, ChainLink::Root | ChainLink::Explicit { .. }) && c.anchored,
            "{rel}: {c:?}"
        );
    }
    (checks.len(), home.series())
}

/// Every certificate in both published chain files verifies up to a root approved by the
/// SCMS Manager CTL published with it. None is implicit, so there is no ECQV
/// reconstruction to check.
///
/// Finding: the production chain file's `homeCtl` is **not** the SCMS Manager CTL. It
/// is a provider-local CTL, series `ffffffffffffff00`, sequence 0, signed by five
/// electors named `elector1`…`elector5` that are not the published SCMS Manager
/// electors. It verifies only against itself, so a device must anchor on the SCMS
/// Manager CTL (`production_ctl.oer`), not on a chain file's home CTL.
#[test]
#[ignore = "needs tools/scms-artefacts/fetch.py"]
fn published_chain_files_verify_to_their_roots() {
    let prod = decode_ctl_spdu(&file("production-2025-08-13/production_ctl.oer")).unwrap();
    let test = decode_ctl_spdu(&file("test-2025-07-17/test_ctl.oer")).unwrap();
    let (n, ps) = check_chain_file("production-2025-08-13/cert-chain_0.oer", &prod);
    let (m, ts) = check_chain_file("test-2025-07-17/test_cert_chain.oer", &test);
    println!(
        "chain files: production {n} certificates (home series {}), test {m} (home series {})",
        hex(&ps),
        hex(&ts)
    );
    assert_eq!(n, 6);
    assert_eq!(ps, [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00]);
    assert_ne!(
        verify_ctl(
            &decode_certificate_chain(&file("production-2025-08-13/cert-chain_0.oer"))
                .unwrap()
                .0,
            &trust(&PROD_ELECTORS)
        )
        .valid_signatures,
        5,
        "the home CTL is not signed by the SCMS Manager electors"
    );
}

/// The CTL is accepted unchanged, and after a flip of any byte it is not: decoding
/// fails, or fewer than five electors verify, or a root stops matching rootCaApprove.
/// (Quorum 3 still accepts a CTL with one broken signature; that is correct, and is why
/// the assertion counts signatures rather than asking for rejection.)
#[test]
#[ignore = "needs tools/scms-artefacts/fetch.py"]
fn flipping_any_byte_of_the_production_ctl_breaks_it() {
    let b = file("production-2025-08-13/production_ctl.oer");
    let t = trust(&PROD_ELECTORS);
    let intact = |bytes: &[u8]| -> bool {
        let Ok(ctl) = decode_ctl_spdu(bytes) else {
            return false;
        };
        verify_ctl(&ctl, &t).valid_signatures == 5
            && check_roots(&ctl).is_ok_and(|r| r.iter().all(|r| r.approved && r.self_signed_ok))
            && ctl::encode_ctl_spdu(&ctl.raw).is_ok_and(|e| e == bytes)
    };
    assert!(intact(&b));
    let stride = std::env::var("V2XW_FLIP_STRIDE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(13);
    let mut tried = 0;
    for i in (0..b.len()).step_by(stride) {
        let mut m = b.clone();
        m[i] ^= 0x01;
        assert!(!intact(&m), "flipping byte {i} left the CTL intact");
        tried += 1;
    }
    println!(
        "flipped {tried} of {} bytes (stride {stride}); none survived",
        b.len()
    );
}

/// One flipped byte in a chain-file certificate's signature is caught.
#[test]
#[ignore = "needs tools/scms-artefacts/fetch.py"]
fn a_flipped_signature_byte_in_a_chain_file_fails() {
    let b = file("production-2025-08-13/cert-chain_0.oer");
    let (ctl, mut others) = decode_certificate_chain(&b).unwrap();
    let last = others.len() - 1;
    let mut sig = others[last].0.signature.clone().unwrap();
    if let v2xw_msg::sec_types::Signature::ecdsaNistP256Signature(s) = &mut sig {
        let mut v = s.s_sig.to_vec();
        v[5] ^= 1;
        s.s_sig = v.into();
    } else {
        panic!("expected a P-256 signature");
    }
    others[last].0.signature = Some(sig);
    let checks = check_chain(&ctl, &others).unwrap();
    assert_eq!(checks[last].link, ChainLink::BadSignature);
}
