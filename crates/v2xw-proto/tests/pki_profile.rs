//! Crypto-formats validation (CF-MA and
//! CF-LIFE): the component certificates the devices and the wire PDUs use (the PDU PKI
//! under the governance root and ICA) carry the published US profile of their role.
//!
//! * Lifetimes: the published certificate of the same role
//!   (`governance::ROLE_SPECS`, transcribed from the SCMS Manager chain files).
//!   Before this check the device-facing RA, ACA, MA, LAs, DCM and CRLG were all valid
//!   for 3 years, a value no published certificate has.
//! * The MA is a root-issued system certificate on CRL series 256 ([SMP] Table 1), as both
//!   published US MA certificates are (ISS pre-production and SAESOL test chains): its
//!   CRACA is the root, whose `CrlSsp` is `isCraca` for series 256.
//! * Every other component is ICA-issued on series 2 with the ICA as CRACA. The CRL
//!   signer stays ICA-issued; that choice and its impact are recorded as finding CF-CRACA.

use v2xw_msg::sec_types::ieee1609_dot2_base_types::Uint16;
use v2xw_msg::sec_types::{Ieee1609Duration, IssuerIdentifier};
use v2xw_proto::scms::governance::{Lifetime, ROLE_SPECS};
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::Authority;
use v2xw_proto::scms::run::ScmsRun;

fn issuer8(a: &Authority) -> [u8; 8] {
    match &a.cert.0.issuer {
        IssuerIdentifier::sha256AndDigest(d) => {
            let mut o = [0u8; 8];
            o.copy_from_slice(&d.0[..]);
            o
        }
        other => panic!("{}: issuer {other:?}", a.name),
    }
}

#[test]
fn the_device_facing_components_carry_their_published_profile() {
    let run = ScmsRun::new(ScmsParams::default().quick()).expect("certificates encode");
    let pki = run.state.pki.as_ref().expect("the PDU PKI");
    let (root8, ica8) = (pki.root.id8(), pki.ica.id8());
    for (a, role) in [
        (&pki.eca, "eca"),
        (&pki.ra, "ra"),
        (&pki.aca, "pca"),
        (&pki.ma, "ma"),
        (&pki.la1, "la1"),
        (&pki.la2, "la2"),
        (&pki.dcm, "dcm"),
        (&pki.crlg, "crlg"),
    ] {
        let spec = ROLE_SPECS
            .iter()
            .find(|s| s.role == role)
            .expect("a role spec");
        let t = &a.cert.0.to_be_signed;
        let want = match spec.lifetime {
            Lifetime::Years(y) => Ieee1609Duration::years(Uint16(y)),
            Lifetime::SixtyHours(h) => Ieee1609Duration::sixtyHours(Uint16(h)),
        };
        assert_eq!(
            t.validity_period.duration, want,
            "{}: the lifetime of the published {} ({})",
            a.name, role, spec.source
        );
        if role == "crlg" {
            continue; // CF-CRACA: ICA-issued by choice, see the module documentation.
        }
        let (issuer, series) = if role == "ma" {
            (root8, 256)
        } else {
            (ica8, 2)
        };
        assert_eq!(issuer8(a), issuer, "{}: issuer ([SMP] Table 1)", a.name);
        assert_eq!(
            t.crl_series.0.0, series,
            "{}: CRL series ([SMP] Table 1)",
            a.name
        );
        assert_eq!(
            t.craca_id.0[..],
            issuer[5..],
            "{}: the CRACA is the issuer of the series' CRL signer",
            a.name
        );
    }
    assert!(
        pki.ma.cert.0.to_be_signed.encryption_key.is_some(),
        "the MA certificate carries the key reports are encrypted to"
    );
}

mod per_provider {
    use super::issuer8;
    use v2xw_msg::scms_types::WireEdition;
    use v2xw_msg::sec_types::Certificate;
    use v2xw_proto::scms::deployment::{Deployment, Preset};
    use v2xw_proto::scms::governance::CertProfile;
    use v2xw_proto::scms::params::ScmsParams;
    use v2xw_proto::scms::pdu::{self, Authority, Drbg, ScmsPki};
    use v2xw_proto::scms::run::ScmsRun;
    use v2xw_sec::crl::{self, CrlBody, CrlSpec};
    use v2xw_sec::sig1609::{Signer1609, cert_hashed_id8};

    fn production() -> ScmsRun {
        ScmsRun::new_deployed(
            ScmsParams::default().quick(),
            0,
            Deployment::preset(Preset::UsProduction2025),
        )
        .expect("the production deployment builds")
    }

    fn id8(c: &Certificate) -> [u8; 8] {
        cert_hashed_id8(c).expect("encodes")
    }

    /// A hash CRL on `series` signed by `signer` (issued by `issuer`, the CRACA by its
    /// `issuerIsCraca`) listing `target`, opened as a device would; and whether it is
    /// accepted and revokes `target`.
    fn crl_revokes(
        signer: (&Certificate, &Signer1609),
        issuer: &Certificate,
        series: u16,
        target: &Certificate,
    ) -> (bool, bool) {
        let start = target.0.to_be_signed.validity_period.start.0.0;
        let spec = CrlSpec {
            series,
            craca: id8(issuer),
            issue_date: start + 60,
            next_crl: start + 60 + 7 * 86_400,
            priority: None,
            delta: false,
            body: CrlBody::Hash {
                serial: 1,
                entries: vec![crl::hash_entry(target).expect("a hash entry")],
            },
        };
        let contents = crl::crl_contents(&spec);
        let signed = crl::sign_crl(&contents, signer.0, signer.1).expect("signs");
        let opened = crl::decode_crl(&crl::encode_crl(&signed).expect("encodes")).expect("decodes");
        let accepted =
            crl::verify_crl(&opened, signer.0, Some(issuer), WireEdition::Ieee2022).accepted();
        (accepted, crl::revokes(&opened.contents, target))
    }

    /// The issuer of the CRL signer, found by its digest (not through the PKI's own
    /// `crl_signer_issuer`, so a wrong CRACA cannot pass).
    fn crlg_issuer(pki: &ScmsPki) -> &Authority {
        let up = issuer8(&pki.crlg);
        [&pki.root, &pki.ica]
            .into_iter()
            .find(|a| a.id8() == up)
            .expect("the CRL signer chains to the root or the ICA")
    }

    /// Every provider certifies in its own published form.
    /// provider-a (ISS): a 70 y root, the CRL signer root-issued on series 256 (ISS's
    /// `crlg.preprod`, [SMP] Table 1), the root as CRACA of every certificate below it,
    /// identifiedRegion {124, 484, 840} on the ICA-issued components, the RA's group in
    /// `certRequestPermissions`. provider-b (SAESOL production): a 30 y root, the ICA
    /// as CRACA, no region, the RA's group in `certIssuePermissions`. In both, a CRL the
    /// provider's CRL signer signs revokes each of its components, in the device-facing
    /// PKI and in the governance hierarchy. Faults: the CRACA named by the issuer under
    /// the ISS form (the documented deviation, `Form::sub_ca` / `craca_for` returning
    /// the issuer), or one profile for every provider.
    #[test]
    fn each_provider_certifies_in_its_own_published_form_and_its_crls_reach_every_component() {
        let run = production();
        let home = run.state.pki.as_ref().expect("the PDU PKI");
        let (label, peer) = &run.state.peer_pki[0];
        assert_eq!(label, "provider-b");
        assert_eq!(home.profile, CertProfile::Iss);
        assert_eq!(peer.profile, CertProfile::S2xProduction);
        // Who issues the CRL signer, and on which series.
        assert_eq!(
            issuer8(&home.crlg),
            home.root.id8(),
            "ISS: root-issued CRL signer"
        );
        assert_eq!(home.crlg.cert.0.to_be_signed.crl_series.0.0, 256);
        assert_eq!(
            issuer8(&peer.crlg),
            peer.ica.id8(),
            "S2X: ICA-issued CRL signer"
        );
        assert_eq!(peer.crlg.cert.0.to_be_signed.crl_series.0.0, 2);
        // The root lifetimes of the two published production roots.
        let years = |c: &Certificate| match &c.0.to_be_signed.validity_period.duration {
            v2xw_msg::sec_types::Ieee1609Duration::years(y) => y.0,
            d => panic!("{d:?}"),
        };
        assert_eq!(years(&home.root.cert), 70, "v2xrootca.ghsiss.com");
        assert_eq!(years(&peer.root.cert), 30, "rootca.prod.s2x.io");
        assert_eq!(years(&home.ica.cert), 30, "ica.preprod.v2x.isscms.com");
        assert_eq!(years(&peer.ica.cert), 25, "ica.prod.s2x.io");
        // Regions and the RA's permission form.
        for a in [&home.ica, &home.eca, &home.aca, &home.ra, &home.la1] {
            assert!(
                a.cert.0.to_be_signed.region.is_some(),
                "{}: ISS region",
                a.name
            );
        }
        assert!(
            home.ma.cert.0.to_be_signed.region.is_none(),
            "root-issued: none"
        );
        for a in [&peer.ica, &peer.eca, &peer.aca, &peer.ra] {
            assert!(
                a.cert.0.to_be_signed.region.is_none(),
                "{}: no region",
                a.name
            );
        }
        let ra = |p: &ScmsPki| {
            let t = &p.ra.cert.0.to_be_signed;
            (
                t.cert_issue_permissions.is_some(),
                t.cert_request_permissions.is_some(),
            )
        };
        assert_eq!(ra(home), (false, true), "ISS RA: certRequestPermissions");
        assert_eq!(ra(peer), (true, false), "S2X RA: certIssuePermissions");
        // A CRL each provider's CRL signer signs reaches every component it must.
        for pki in [home, peer] {
            let craca = crlg_issuer(pki);
            for a in [&pki.eca, &pki.ra, &pki.aca, &pki.la1, &pki.la2, &pki.dcm] {
                let (ok, revoked) =
                    crl_revokes((&pki.crlg.cert, &pki.crlg.key), &craca.cert, 2, &a.cert);
                assert!(ok, "{}: the series-2 CRL verifies", a.name);
                assert!(
                    revoked,
                    "{}: a series-2 CRL of its CRACA revokes it",
                    a.name
                );
            }
            assert_eq!(pki.craca_id8(), craca.id8(), "{}", pki.crlg.name);
        }
        // The ICA itself: ISS's root-issued CRL signer can revoke it; SAESOL's cannot
        // (its CRACA is the ICA): recorded, not hidden.
        let (_, ica_revoked) = crl_revokes(
            (&home.crlg.cert, &home.crlg.key),
            &home.root.cert,
            2,
            &home.ica.cert,
        );
        assert!(ica_revoked, "ISS: a series-2 CRL revokes the ICA");
        // The governance hierarchy of each provider: its CRL signer's CRL reaches its
        // ICA-issued components (it could not, before: CF-CRACA).
        let gov = &run.state.gov;
        let signer = (
            gov.cert("crlg").expect("crlg"),
            gov.signing_key("crlg").expect("key"),
        );
        let root = gov.cert("root").expect("root");
        assert_eq!(
            match &signer.0.0.issuer {
                v2xw_msg::sec_types::IssuerIdentifier::sha256AndDigest(d) => d.0[..].to_vec(),
                other => panic!("{other:?}"),
            },
            id8(root).to_vec(),
            "ISS: the governance CRL signer is root-issued"
        );
        for role in ["eca", "pca", "ra", "la1", "la2", "ica"] {
            let target = gov.cert(role).expect("certified");
            let (ok, revoked) = crl_revokes(signer, root, 2, target);
            assert!(
                ok && revoked,
                "governance {role}: a root-CRACA CRL revokes it"
            );
        }
        let b_signer = (
            gov.peer_cert("provider-b", "crlg").expect("crlg"),
            gov.peer_signing_key("provider-b", "crlg").expect("key"),
        );
        let b_ica = gov.peer_cert("provider-b", "ica").expect("ica");
        for role in ["eca", "pca", "ra", "la1", "la2"] {
            let target = gov.peer_cert("provider-b", role).expect("certified");
            let (ok, revoked) = crl_revokes(b_signer, b_ica, 2, target);
            assert!(
                ok && revoked,
                "provider-b governance {role}: an ICA-CRACA CRL revokes it"
            );
        }
    }

    /// crypto #29 (CF-ENR-CRACA): the ECA names the provider's CRACA in the enrolment
    /// certificate it issues ("may have different values, set by the ECA"), so a series-4
    /// hash CRL of the provider's CRL signer (`CrlSsp` 4, 3, 1, 2) revokes it. Fault: the
    /// request's `000000` kept, under which no CRL could ever match.
    #[test]
    fn a_series_4_crl_revokes_an_enrolment_certificate() {
        let run = production();
        for pki in [
            run.state.pki.as_ref().expect("home").as_ref(),
            run.state.peer_pki[0].1.as_ref(),
        ] {
            let mut g = Drbg::new(9, b"enrolment-test");
            let k = g.scalar();
            let public = v2xw_sec::ec::Point::mul_base(&k);
            let start = pki.eca.cert.0.to_be_signed.validity_period.start.0.0 + 86_400;
            let req = v2xw_proto::scms::enrolment::EnrolmentRequest {
                name: "obe-1.example",
                regions: &[840, 124, 484],
                valid_from: start,
                lifetime_s: 6 * 31_556_952,
            };
            let tbs = v2xw_proto::scms::enrolment::us_enrolment_tbs(&req, &public)
                .expect("the US enrolment profile");
            assert_eq!(&tbs.craca_id.0[..], &[0u8; 3], "the request carries 000000");
            let (cert, _) = pdu::issue_enrolment(pki, &tbs, &public, &mut g).expect("issues");
            let t = &cert.0.to_be_signed;
            assert_eq!(t.crl_series.0.0, 4);
            let craca = crlg_issuer(pki);
            assert_eq!(
                &t.craca_id.0[..],
                &craca.id8()[5..],
                "{}: the CRACA",
                pki.eca.name
            );
            let (ok, revoked) = crl_revokes((&pki.crlg.cert, &pki.crlg.key), &craca.cert, 4, &cert);
            assert!(ok, "the series-4 CRL verifies");
            assert!(
                revoked,
                "{}: a series-4 CRL revokes the enrolment cert",
                pki.eca.name
            );
        }
    }
}

/// ISS's LAs carry `LaSsp` version 1 with `laId` 4 and 5, decoded from SCMS
/// Manager's 2025-07-17 test chain file (`88 00 01 00 04`, `88 00 01 00 05`). Under the US
/// production deployment provider-a is ISS: both engine PKIs (the device-facing PDU PKI
/// and the governance hierarchy of the chain files) give its LAs exactly those bytes,
/// its own LA states use those ids, and its linkage CRL's `LAGroup` names them.
/// provider-b (SAESOL) keeps version 2 with 1 and 2. The published bytes are checked
/// against the decoded test chain itself whenever the artefact cache is present.
mod iss_la_ssp {
    use v2xw_msg::sec_types::Certificate;
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::ServiceSpecificPermissions;
    use v2xw_proto::scms::deployment::{Deployment, Preset};
    use v2xw_proto::scms::governance::{CertProfile, la_ids, ssp_bytes};
    use v2xw_proto::scms::params::ScmsParams;
    use v2xw_proto::scms::run::ScmsRun;
    use v2xw_sec::linkage::LaId;

    /// The decoded ISS LA SSPs (PSID 35): choice `la` (0x88), the SEQUENCE preamble,
    /// version 1, `laId` 4 and 5.
    const ISS_LA_SSPS: [[u8; 5]; 2] = [[0x88, 0, 1, 0, 4], [0x88, 0, 1, 0, 5]];

    fn ssp35(c: &Certificate) -> Vec<u8> {
        let perms =
            c.0.to_be_signed
                .app_permissions
                .as_ref()
                .expect("appPermissions");
        let p = perms
            .0
            .iter()
            .find(|p| p.psid.0 == 35.into())
            .expect("a security-management permission");
        match p.ssp.as_ref().expect("an SSP") {
            ServiceSpecificPermissions::opaque(o) => o.to_vec(),
            other => panic!("{other:?}"),
        }
    }

    fn production() -> ScmsRun {
        ScmsRun::new_deployed(
            ScmsParams::default().quick(),
            0,
            Deployment::preset(Preset::UsProduction2025),
        )
        .expect("the production deployment builds")
    }

    #[test]
    fn the_iss_las_carry_the_published_la_ssp_and_name_their_ids_on_the_crl() {
        assert_eq!(la_ids(CertProfile::Iss), [4, 5]);
        assert_eq!(ssp_bytes(CertProfile::Iss, "la1"), ISS_LA_SSPS[0]);
        assert_eq!(ssp_bytes(CertProfile::Iss, "la2"), ISS_LA_SSPS[1]);
        let mut run = production();
        let pki = run.state.pki.clone().expect("the PDU PKI");
        assert_eq!(pki.profile, CertProfile::Iss);
        // The device-facing LA certificates and the governance ones (chain files).
        assert_eq!(ssp35(&pki.la1.cert), ISS_LA_SSPS[0], "PDU PKI la1");
        assert_eq!(ssp35(&pki.la2.cert), ISS_LA_SSPS[1], "PDU PKI la2");
        for (k, role) in ["la1", "la2"].into_iter().enumerate() {
            let c = run.state.gov.cert(role).expect("certified");
            assert_eq!(ssp35(c), ISS_LA_SSPS[k], "governance {role}");
        }
        // The provider's LAs use the same ids, and its linkage CRL names them.
        assert_eq!(
            (run.state.la[0].la_id, run.state.la[1].la_id),
            (LaId(4), LaId(5))
        );
        run.seed_standing_crl(1);
        let secured = run.state.crlg.secured.clone().expect("a signed CRL");
        let opened = v2xw_sec::crl::decode_crl(secured.bytes()).expect("decodes");
        let entries = v2xw_sec::crl::linkage_entries(&opened.contents);
        assert_eq!(entries.len(), 1);
        assert_eq!(
            (entries[0].la_id1, entries[0].la_id2),
            (LaId(4), LaId(5)),
            "LAGroup"
        );
        // provider-b (SAESOL production) keeps the 2022 module's version 2, ids 1 and 2.
        let (_, peer) = &run.state.peer_pki[0];
        assert_eq!(peer.profile, CertProfile::S2xProduction);
        assert_eq!(ssp35(&peer.la1.cert), [0x88, 0, 2, 0, 1]);
        assert_eq!(ssp35(&peer.la2.cert), [0x88, 0, 2, 0, 2]);
        let b = ScmsRun::new_peer(&run, 1).expect("provider-b's run");
        assert_eq!(
            (b.state.la[0].la_id, b.state.la[1].la_id),
            (LaId(1), LaId(2))
        );
    }

    /// The oracle: the decoded ISS LA certificates of SCMS Manager's 2025-07-17 test
    /// chain file carry exactly the SSP bytes the engine gives provider-a's LAs.
    #[test]
    fn the_published_iss_la_certificates_carry_these_bytes() {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../third_party/scms-artefacts/files");
        let forced = std::env::var("V2XW_SCMS_ARTEFACTS").is_ok_and(|v| v == "1");
        if !forced && !dir.is_dir() {
            eprintln!(
                "SKIPPED the_published_iss_la_certificates_carry_these_bytes: no \
                 third_party/scms-artefacts/files (python3 -I tools/scms-artefacts/fetch.py)"
            );
            return;
        }
        let bytes = std::fs::read(dir.join("test-2025-07-17/test_cert_chain.oer"))
            .expect("the 2025-07-17 test chain file");
        let (_, others) = v2xw_sec::ctl::decode_certificate_chain(&bytes).expect("decodes");
        let mut las: Vec<Vec<u8>> = others
            .iter()
            .filter_map(|c| {
                let perms = c.0.to_be_signed.app_permissions.as_ref()?;
                perms.0.iter().find(|p| p.psid.0 == 35.into())?;
                Some(ssp35(c))
            })
            .filter(|s| s.first() == Some(&0x88))
            .collect();
        las.sort();
        assert_eq!(
            las,
            ISS_LA_SSPS.map(|s| s.to_vec()),
            "the published ISS LAs"
        );
        let run = production();
        let pki = run.state.pki.as_ref().expect("the PDU PKI");
        assert_eq!([ssp35(&pki.la1.cert), ssp35(&pki.la2.cert)].to_vec(), las);
    }
}
