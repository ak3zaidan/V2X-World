//! WP-A5 emitted vectors: deterministic CRLs, a CTL, a CCF and a composite CRL built by
//! `v2xw_sec::{crl, ctl}`, committed under `tests/vectors/rust/wp-a5/` so the A0 oracle
//! can check them independently (`tools/scms-artefacts/xcheck_crl.py`, which decodes
//! with asn1tools and verifies with its own ECDSA, hashing and linkage code).
//!
//! This test pins the bytes: the code must still produce exactly the committed files
//! (RFC 6979 signing makes them a pure function of the keys). `V2XW_BLESS=1` rewrites
//! them after an intended change; the oracle must then be re-run.

use std::path::PathBuf;

use rasn::types::{FixedOctetString, Integer};
use v2xw_msg::scms_types::WireEdition;
use v2xw_msg::sec_types::Certificate;
use v2xw_msg::sec_types::ieee1609_dot2::{
    CertificateBase, CertificateId, CertificateType, IssuerIdentifier, LinkageData,
    ToBeSignedCertificate, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    CrlSeries, Duration, HashedId3, HashedId8, IValue, Psid, PsidSsp, SequenceOfPsidSsp,
    ServiceSpecificPermissions, Time32, Uint8, Uint16, Uint32, ValidityPeriod,
};
use v2xw_sec::crl::{self, CracaType, CrlBody, CrlSpec, CrlSsp, LinkageRevocation};
use v2xw_sec::ctl;
use v2xw_sec::linkage::{DeviceLinkageContext, LaId, LinkageSeed, linkage_seed_at};
use v2xw_sec::sig1609::{Signer1609, cert_hashed_id8, cert_hashed_id32, coer};

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/vectors/rust/wp-a5")
}

/// A CRL signer certificate issued by `root` (explicit, P-256), PSID 256 with `ssp`.
fn issued(
    name: &str,
    root: &Certificate,
    root_key: &Signer1609,
    key: &Signer1609,
    ssp: &CrlSsp,
    (series, craca3): (u16, [u8; 3]),
) -> Certificate {
    let tbs = ToBeSignedCertificate::new(
        CertificateId::name(v2xw_msg::sec_types::ieee1609_dot2_base_types::Hostname(
            name.into(),
        )),
        HashedId3(FixedOctetString::from(craca3)),
        CrlSeries(Uint16(series)),
        ValidityPeriod::new(Time32(Uint32(600_000_000)), Duration::years(Uint16(10))),
        None,
        None,
        Some(SequenceOfPsidSsp(vec![PsidSsp::new(
            Psid(Integer::from(256u32)),
            Some(ServiceSpecificPermissions::opaque(ssp.to_coer().into())),
        )])),
        None,
        None,
        None,
        None,
        VerificationKeyIndicator::verificationKey(key.public_key()),
        None,
        None,
        None,
        None,
    );
    let sig = root_key.sign_1609(
        &coer(&tbs).unwrap(),
        &v2xw_sec::sig1609::canonical_cert_coer(root).unwrap(),
    );
    let id = cert_hashed_id8(root).unwrap();
    Certificate(CertificateBase::new(
        Uint8(3),
        CertificateType::explicit,
        IssuerIdentifier::sha256AndDigest(HashedId8(FixedOctetString::from(id))),
        tbs,
        Some(sig),
    ))
}

/// A pseudonym certificate (explicit here; revocation reads only its linkage data).
fn pseudonym(
    aca: &Certificate,
    aca_key: &Signer1609,
    craca: [u8; 8],
    i: u16,
    lv: [u8; 9],
) -> Certificate {
    let k = Signer1609::p256(&[0x77; 32]).unwrap();
    let tbs = ToBeSignedCertificate::new(
        CertificateId::linkageData(LinkageData::new(
            IValue(Uint16(i)),
            v2xw_msg::sec_types::ieee1609_dot2_base_types::LinkageValue(FixedOctetString::from(lv)),
            None,
        )),
        HashedId3(FixedOctetString::from([craca[5], craca[6], craca[7]])),
        CrlSeries(Uint16(1)),
        ValidityPeriod::new(Time32(Uint32(700_000_000)), Duration::hours(Uint16(168))),
        None,
        None,
        Some(SequenceOfPsidSsp(vec![PsidSsp::new(
            Psid(Integer::from(32u32)),
            None,
        )])),
        None,
        None,
        None,
        None,
        VerificationKeyIndicator::verificationKey(k.public_key()),
        None,
        None,
        None,
        None,
    );
    let sig = aca_key.sign_1609(
        &coer(&tbs).unwrap(),
        &v2xw_sec::sig1609::canonical_cert_coer(aca).unwrap(),
    );
    Certificate(CertificateBase::new(
        Uint8(3),
        CertificateType::explicit,
        IssuerIdentifier::sha256AndDigest(HashedId8(FixedOctetString::from(
            cert_hashed_id8(aca).unwrap(),
        ))),
        tbs,
        Some(sig),
    ))
}

fn artefacts() -> Vec<(&'static str, Vec<u8>)> {
    let root_key = Signer1609::p256(&[0x10; 32]).unwrap();
    let root_ssp = CrlSsp::new(CracaType::isCraca, &[256]);
    let root = ctl::self_signed_certificate(
        "rootca.wp-a5.sim",
        600_000_000,
        30,
        vec![PsidSsp::new(
            Psid(Integer::from(256u32)),
            Some(ServiceSpecificPermissions::opaque(
                root_ssp.to_coer().into(),
            )),
        )],
        None,
        &root_key,
    )
    .unwrap();
    let crlg_key = Signer1609::p256(&[0x20; 32]).unwrap();
    let crlg = issued(
        "crlg.wp-a5.sim",
        &root,
        &root_key,
        &crlg_key,
        &CrlSsp::new(CracaType::isCraca, &[1, 2, 3, 4]),
        (256, [0; 3]),
    );
    let craca = cert_hashed_id8(&crlg).unwrap();

    let dev = DeviceLinkageContext::new(
        LaId(0x0102),
        LaId(0x0304),
        LinkageSeed::new([0xA1; 16]),
        LinkageSeed::new([0xB2; 16]),
    );
    let other = DeviceLinkageContext::new(
        LaId(0x0102),
        LaId(0x0304),
        LinkageSeed::new([0xC3; 16]),
        LinkageSeed::new([0xD4; 16]),
    );
    let rev = |d: &DeviceLinkageContext, i_rev: u32, i_max: u16| LinkageRevocation {
        la1: d.la_id1,
        la2: d.la_id2,
        ls1: linkage_seed_at(d.la_id1, d.ls1_0, i_rev),
        ls2: linkage_seed_at(d.la_id2, d.ls2_0, i_rev),
        jmax: 20,
        i_max,
    };
    let series1 = crl::crl_contents(&CrlSpec {
        series: 1,
        craca,
        issue_date: 700_000_000,
        next_crl: 700_604_800,
        priority: None,
        delta: false,
        body: CrlBody::Linkage {
            i_rev: 600,
            index_within_i: 0,
            individual: vec![rev(&dev, 600, 760), rev(&other, 600, 601)],
        },
    });
    crl::check_crl(&series1, WireEdition::Ieee2022).unwrap();
    let crl1 = crl::sign_crl(&series1, &crlg, &crlg_key).unwrap();

    let aca_key = Signer1609::p256(&[0x30; 32]).unwrap();
    let aca = issued(
        "aca.wp-a5.sim",
        &root,
        &root_key,
        &aca_key,
        &CrlSsp::new(CracaType::isCraca, &[]),
        (2, [craca[5], craca[6], craca[7]]),
    );
    let revoked = pseudonym(
        &aca,
        &aca_key,
        craca,
        603,
        *dev.linkage_value_for(603, 7).as_bytes(),
    );
    let past_imax = pseudonym(
        &aca,
        &aca_key,
        craca,
        603,
        *other.linkage_value_for(603, 7).as_bytes(),
    );

    let series2 = crl::crl_contents(&CrlSpec {
        series: 2,
        craca,
        issue_date: 700_000_000,
        next_crl: 700_604_800,
        priority: Some(2),
        delta: true,
        body: CrlBody::Hash {
            serial: 41,
            entries: vec![crl::hash_entry(&aca).unwrap()],
        },
    });
    let crl2 = crl::sign_crl(&series2, &crlg, &crlg_key).unwrap();

    let electors: Vec<ctl::Elector> = (1..=5u8)
        .map(|i| {
            ctl::elector(
                &format!("v2xelector{i}.wp-a5.sim"),
                503_136_005,
                9 + 3 * u16::from(i),
                &[i; 48],
            )
            .unwrap()
        })
        .collect();
    let ids: Vec<_> = electors.iter().map(ctl::Elector::hashed_id48).collect();
    let tbs = ctl::full_ieee_tbs_ctl(
        ctl::US_CTL_SERIES_ID,
        4,
        700_000_000,
        &ids,
        &[cert_hashed_id32(&root).unwrap()],
        3,
    );
    let m = ctl::build_ctl(
        &tbs,
        vec![root.clone()],
        &[&electors[0], &electors[2], &electors[4]],
    )
    .unwrap();

    vec![
        ("root.cert.oer", coer(&root).unwrap()),
        ("crlg.cert.oer", coer(&crlg).unwrap()),
        ("aca.cert.oer", coer(&aca).unwrap()),
        ("pseudonym_revoked.cert.oer", coer(&revoked).unwrap()),
        ("pseudonym_past_imax.cert.oer", coer(&past_imax).unwrap()),
        ("crl_series1_linkage.oer", crl::encode_crl(&crl1).unwrap()),
        (
            "crl_series2_hash_delta.oer",
            crl::encode_crl(&crl2).unwrap(),
        ),
        ("ctl_3of5.oer", ctl::encode_ctl_spdu(&m).unwrap()),
        (
            "ccf.oer",
            ctl::encode_certificate_chain(&m, vec![root.clone(), crlg.clone(), aca.clone()])
                .unwrap(),
        ),
        (
            "composite_crl.oer",
            ctl::encode_composite_crl(vec![crl1, crl2], &m).unwrap(),
        ),
    ]
}

/// The code reproduces the committed vectors byte for byte, and they verify in Rust.
#[test]
fn emitted_vectors_are_reproduced_and_verify() {
    let bless = std::env::var("V2XW_BLESS").is_ok_and(|v| v == "1");
    let arts = artefacts();
    for (name, bytes) in &arts {
        let p = dir().join(name);
        if bless {
            std::fs::create_dir_all(dir()).unwrap();
            std::fs::write(&p, bytes).unwrap();
        }
        let committed = std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(
            &committed, bytes,
            "{name} differs from the committed vector"
        );
    }
    let get = |n: &str| arts.iter().find(|(k, _)| *k == n).unwrap().1.clone();
    let cert = |n: &str| -> Certificate { rasn::coer::decode(&get(n)).unwrap() };
    let (crlg, root) = (cert("crlg.cert.oer"), cert("root.cert.oer"));
    let c1 = crl::decode_crl(&get("crl_series1_linkage.oer")).unwrap();
    assert!(crl::verify_crl(&c1, &crlg, Some(&root), WireEdition::Ieee2022).accepted());
    assert!(crl::revokes(
        &c1.contents,
        &cert("pseudonym_revoked.cert.oer")
    ));
    assert!(!crl::revokes(
        &c1.contents,
        &cert("pseudonym_past_imax.cert.oer")
    ));
    let c2 = crl::decode_crl(&get("crl_series2_hash_delta.oer")).unwrap();
    assert!(crl::verify_crl(&c2, &crlg, Some(&root), WireEdition::Ieee2022).accepted());
    let aca = cert("aca.cert.oer");
    assert!(crl::revokes(&c2.contents, &aca));
    assert!(
        !crl::revokes(&c2.contents, &crlg),
        "series 256 certificate, series 2 CRL"
    );
    assert!(
        !crl::revokes(&c1.contents, &aca),
        "a hash-revoked certificate is not on the linkage CRL"
    );
    let (crls, home) = ctl::decode_composite_crl(&get("composite_crl.oer")).unwrap();
    assert_eq!(crls.len(), 2);
    let ids = home.elector_approve();
    let v = ctl::verify_ctl(&home, &ctl::CtlTrust::us_bootstrap(ids));
    assert!(v.accepted() && v.valid_signatures == 3, "{v:?}");
}
