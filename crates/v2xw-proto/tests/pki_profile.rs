//! Crypto-formats validation (docs/design/scms-real/validation/crypto-formats.md, CF-MA and
//! CF-LIFE): the component certificates the devices and the wire PDUs use (the PDU PKI
//! under the governance root and ICA) carry the published US profile of their role.
//!
//! * Lifetimes: the published certificate of the same role
//!   (`governance::ROLE_SPECS`, transcribed from the SCMS Manager chain files, 01 §2.2).
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
