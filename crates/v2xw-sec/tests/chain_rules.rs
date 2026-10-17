//! One negative test per 02 §12 chain rule (WP-A3 acceptance), each starting from a
//! chain that validates and changing exactly one thing, plus the positive cases that
//! keep each rule from being over-strict.
//!
//! Base chain: root (explicit, self-signed) → ICA (implicit) → ACA (implicit) → EE
//! (implicit pseudonym), the shape the CAMP profile uses (01 §4: every profiled
//! certificate implicit, roots explicit). Rules the oracle cross-check
//! (`chain_xcheck.rs`) cannot reach — chain length > 8, bitmap SSPs, sub-region
//! containment, certRequestPermissions, duplicate PSIDs, P-384 roots — are tested only
//! here; their expected results are read off the 1609.2-2022 ASN.1 notes cited per test.

use std::collections::BTreeSet;

use p256::Scalar;
use rasn::types::{FixedBitString, Integer, OctetString};
use v2xw_msg::sec_types::ieee1609_dot2::{
    CertificateId, SequenceOfPsidGroupPermissions, SubjectPermissions, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    AnonymousSequenceOfOctetString, BitmapSsp, BitmapSspRange, CircularRegion, CountryAndRegions,
    CrlSeries, EccP384CurvePoint, GeographicRegion, Hostname, IdentifiedRegion, Latitude,
    Longitude, NinetyDegreeInt, OneEightyDegreeInt, PsidSsp, PsidSspRange, PublicVerificationKey,
    SequenceOfIdentifiedRegion, SequenceOfOctetString, SequenceOfPsidSsp, SequenceOfPsidSspRange,
    SequenceOfUint8, ServiceSpecificPermissions, SspRange, TwoDLocation, Uint8, Uint16, Uint32,
    UnCountryId,
};
use v2xw_msg::sec_types::{
    Certificate, EndEntityType, HashAlgorithm, HashedId3, Ieee1609Duration, IssuerIdentifier, Psid,
    PsidGroupPermissions, Signature, Time32, ToBeSignedCertificate, ValidityPeriod,
};
use v2xw_sec::cert::{self, CertHash, CertKey};
use v2xw_sec::chain::{self, ChainErrorKind as K, ChainPolicy};
use v2xw_sec::ec::Point;

const START: u32 = 679_737_605;
const YEAR: u32 = 31_556_952;

// ------------------------------------------------------------------------------------
// builders

fn ee_type(app: bool, enrol: bool) -> EndEntityType {
    let mut b = FixedBitString::<8>::ZERO;
    b.set(0, app);
    b.set(1, enrol);
    EndEntityType(b)
}

fn group(
    psids: Option<Vec<PsidSspRange>>,
    min: i64,
    range: i64,
    et: EndEntityType,
) -> PsidGroupPermissions {
    PsidGroupPermissions::new(
        match psids {
            None => SubjectPermissions::all(()),
            Some(v) => SubjectPermissions::explicit(SequenceOfPsidSspRange(v)),
        },
        Integer::from(min),
        Integer::from(range),
        et,
    )
}

fn all(min: i64, range: i64) -> PsidGroupPermissions {
    group(None, min, range, ee_type(true, false))
}

fn rng(psid: u64, r: Option<SspRange>) -> PsidSspRange {
    PsidSspRange::new(Psid(Integer::from(psid)), r)
}

fn opaque_range(items: &[&[u8]]) -> SspRange {
    SspRange::opaque(SequenceOfOctetString(
        items
            .iter()
            .map(|b| AnonymousSequenceOfOctetString(OctetString::from(b.to_vec())))
            .collect(),
    ))
}

fn app(psid: u64, ssp: Option<ServiceSpecificPermissions>) -> PsidSsp {
    PsidSsp::new(Psid(Integer::from(psid)), ssp)
}

fn countries(cs: &[u16]) -> GeographicRegion {
    GeographicRegion::identifiedRegion(SequenceOfIdentifiedRegion(
        cs.iter()
            .map(|c| IdentifiedRegion::countryOnly(UnCountryId(Uint16(*c))))
            .collect(),
    ))
}

fn country_regions(c: u16, rs: &[u8]) -> GeographicRegion {
    GeographicRegion::identifiedRegion(SequenceOfIdentifiedRegion(vec![
        IdentifiedRegion::countryAndRegions(CountryAndRegions::new(
            UnCountryId(Uint16(c)),
            SequenceOfUint8(rs.iter().map(|r| Uint8(*r)).collect()),
        )),
    ]))
}

/// A certificate's content; the key indicator is filled in at issuance.
#[derive(Clone)]
struct Spec {
    name: &'static str,
    start: u32,
    hours: u16,
    app: Option<Vec<PsidSsp>>,
    issue: Option<Vec<PsidGroupPermissions>>,
    request: Option<Vec<PsidGroupPermissions>>,
    region: Option<GeographicRegion>,
}

impl Spec {
    fn ca(name: &'static str, g: PsidGroupPermissions) -> Spec {
        Spec {
            name,
            start: START,
            hours: 60_000,
            app: None,
            issue: Some(vec![g]),
            request: None,
            region: None,
        }
    }
    fn ee() -> Spec {
        Spec {
            name: "ee",
            start: START + 3600,
            hours: 168,
            app: Some(vec![app(32, None)]),
            issue: None,
            request: None,
            region: None,
        }
    }
    fn tbs(&self, vki: VerificationKeyIndicator) -> ToBeSignedCertificate {
        ToBeSignedCertificate::new(
            CertificateId::name(Hostname(self.name.to_string().into())),
            HashedId3(rasn::types::FixedOctetString::from([1, 2, 3])),
            CrlSeries(Uint16(2)),
            ValidityPeriod::new(
                Time32(Uint32(self.start)),
                if self.hours == u16::MAX {
                    Ieee1609Duration::years(Uint16(40))
                } else {
                    Ieee1609Duration::hours(Uint16(self.hours))
                },
            ),
            self.region.clone(),
            None,
            self.app.clone().map(SequenceOfPsidSsp),
            self.issue.clone().map(SequenceOfPsidGroupPermissions),
            self.request.clone().map(SequenceOfPsidGroupPermissions),
            None,
            None,
            vki,
            None,
            None,
            None,
            None,
        )
    }
}

#[derive(Clone)]
struct Node {
    cert: Certificate,
    d: Scalar,
    key: CertKey,
}

fn seq() -> impl FnMut() -> Scalar {
    let mut n = 1000u64;
    move || {
        n += 7;
        Scalar::from(n)
    }
}

fn root(spec: Spec, seed: u64) -> Node {
    let d = Scalar::from(seed);
    let q = Point::mul_base(&d);
    let mut s = spec;
    s.hours = u16::MAX; // 40 years
    let tbs = s.tbs(cert::verification_key_indicator(&q.compressed().unwrap()).unwrap());
    let c = cert::issue_explicit_cert(tbs, None, CertHash::Sha256, |dg| {
        cert::ecdsa_p256_sign_with_k(&d, &Scalar::from(seed + 1), &dg.try_into().unwrap())
    })
    .unwrap();
    Node {
        cert: c,
        d,
        key: CertKey::P256(q),
    }
}

fn explicit(parent: &Node, spec: Spec, seed: u64) -> Node {
    let d = Scalar::from(seed);
    let q = Point::mul_base(&d);
    let tbs = spec.tbs(cert::verification_key_indicator(&q.compressed().unwrap()).unwrap());
    let pd = parent.d;
    let c = cert::issue_explicit_cert(
        tbs,
        Some((&parent.cert, &parent.key)),
        CertHash::Sha256,
        |dg| cert::ecdsa_p256_sign_with_k(&pd, &Scalar::from(seed + 1), &dg.try_into().unwrap()),
    )
    .unwrap();
    Node {
        cert: c,
        d,
        key: CertKey::P256(q),
    }
}

fn implicit(parent: &Node, spec: Spec, seed: u64) -> Node {
    let ku = Scalar::from(seed);
    let CertKey::P256(q_ca) = &parent.key else {
        panic!("ECQV needs a P-256 issuer")
    };
    // The template's key indicator is replaced by P_U.
    let tbs = spec
        .tbs(cert::verification_key_indicator(&Point::GENERATOR.compressed().unwrap()).unwrap());
    let out = cert::issue_implicit(
        &tbs,
        &parent.cert,
        q_ca,
        &parent.d,
        &Point::mul_base(&ku),
        seq(),
    )
    .unwrap();
    let d = v2xw_sec::crypto::ecqv::subject_private_key(&out.e, &ku, &out.private_key_contribution);
    let key = cert::verification_key(&out.certificate, Some((&parent.cert, &parent.key))).unwrap();
    assert_eq!(key, CertKey::P256(Point::mul_base(&d)));
    Node {
        cert: out.certificate,
        d,
        key,
    }
}

struct Specs {
    root: Spec,
    ica: Spec,
    aca: Spec,
    ee: Spec,
}

fn base_specs() -> Specs {
    Specs {
        root: Spec::ca("root", all(1, -1)),
        ica: Spec::ca("ica", all(1, 1)),
        aca: Spec::ca("aca", all(1, 0)),
        ee: Spec::ee(),
    }
}

fn build(s: &Specs) -> (Vec<Certificate>, ChainPolicy) {
    let r = root(s.root.clone(), 11);
    let i = implicit(&r, s.ica.clone(), 21);
    let a = implicit(&i, s.aca.clone(), 31);
    let e = implicit(&a, s.ee.clone(), 41);
    let policy = ChainPolicy::trusting([&r.cert]).unwrap();
    (vec![e.cert, a.cert, i.cert, r.cert], policy)
}

fn kinds(chain: &[Certificate], policy: &ChainPolicy) -> BTreeSet<K> {
    match chain::validate(chain, policy) {
        Ok(_) => BTreeSet::new(),
        Err(es) => es.into_iter().map(|e| e.kind).collect(),
    }
}

/// Exactly `want` fails (and nothing else).
fn only(chain: &[Certificate], policy: &ChainPolicy, want: K) {
    let got = kinds(chain, policy);
    assert_eq!(got, BTreeSet::from([want]), "expected only {want:?}");
}

fn valid(chain: &[Certificate], policy: &ChainPolicy) {
    let r = chain::validate(chain, policy);
    assert!(r.is_ok(), "expected valid, got {:?}", r.err());
}

fn with(f: impl FnOnce(&mut Specs)) -> (Vec<Certificate>, ChainPolicy) {
    let mut s = base_specs();
    f(&mut s);
    build(&s)
}

// ------------------------------------------------------------------------------------

#[test]
fn the_base_chain_is_valid_and_its_key_verifies_an_spdu() {
    let (c, p) = build(&base_specs());
    let v = chain::validate(&c, &p).expect("valid");
    // A signature under the EE private key verifies under the resolved key.
    let s = base_specs();
    let r = root(s.root.clone(), 11);
    let i = implicit(&r, s.ica.clone(), 21);
    let a = implicit(&i, s.aca.clone(), 31);
    let e = implicit(&a, s.ee.clone(), 41);
    let digest = [9u8; 32];
    let sig = cert::ecdsa_p256_sign_with_k(&e.d, &Scalar::from(5u64), &digest).unwrap();
    assert!(cert::ecdsa_verify_digest(v.ee_key(), &digest, &sig));
    assert!(chain::ee_permits_psid(&c[0], 32));
    assert!(
        !chain::ee_permits_psid(&c[0], 35),
        "SPDU PSID not in appPermissions"
    );
}

#[test]
fn rule_chain_length_at_most_8() {
    let mk = |n: usize| {
        let r = root(Spec::ca("root", all(1, -1)), 11);
        let mut nodes = vec![r];
        for k in 0..n {
            let parent = nodes.last().unwrap().clone();
            nodes.push(implicit(
                &parent,
                Spec::ca("ica", all(1, -1)),
                100 + k as u64,
            ));
        }
        let e = implicit(nodes.last().unwrap(), Spec::ee(), 41);
        let p = ChainPolicy::trusting([&nodes[0].cert]).unwrap();
        let mut c: Vec<Certificate> = nodes.into_iter().rev().map(|x| x.cert).collect();
        c.insert(0, e.cert);
        (c, p)
    };
    let (c8, p8) = mk(6);
    assert_eq!(c8.len(), 8);
    valid(&c8, &p8);
    let (c9, p9) = mk(7);
    assert_eq!(c9.len(), 9);
    only(&c9, &p9, K::TooLong);
}

#[test]
fn rule_well_formed() {
    // A PSID twice in appPermissions.
    let (c, p) = with(|s| s.ee.app = Some(vec![app(32, None), app(32, None)]));
    only(&c, &p, K::Malformed);
    // Two `all` groups.
    let (c, p) = with(|s| s.ica.issue = Some(vec![all(1, 1), all(1, 1)]));
    only(&c, &p, K::Malformed);
    // No permissions at all.
    let (c, p) = with(|s| s.ee.app = None);
    assert!(kinds(&c, &p).contains(&K::Malformed));
    // An implicit certificate with a signature, and version 2.
    let (mut c, p) = build(&base_specs());
    c[0].0.signature = Some(cert::zero_signature());
    only(&c, &p, K::Malformed);
    let (mut c, p) = build(&base_specs());
    c[0].0.version = Uint8(2);
    assert!(kinds(&c, &p).contains(&K::Malformed));
}

#[test]
fn rule_rooted_in_a_verifying_self_signed_root() {
    let (c, p) = build(&base_specs());
    only(&c[..3], &p, K::NotRootedInSelfSigned);
    let (mut c, p) = build(&base_specs());
    let root = c.last_mut().unwrap();
    if let Some(Signature::ecdsaNistP256Signature(s)) = &mut root.0.signature {
        let mut b = s.s_sig.to_vec();
        b[3] ^= 1;
        s.s_sig = OctetString::from(b);
    }
    let p2 = ChainPolicy::trusting([c.last().unwrap()]).unwrap();
    let got = kinds(&c, &p2);
    assert!(got.contains(&K::BadRootSignature), "{got:?}");
    drop(p);
}

#[test]
fn rule_root_on_the_trust_list() {
    let (c, _) = build(&base_specs());
    let other = root(Spec::ca("other-root", all(1, -1)), 12);
    let p = ChainPolicy::trusting([&other.cert]).unwrap();
    only(&c, &p, K::UntrustedRoot);
}

#[test]
fn rule_issuer_identifier_names_the_issuer() {
    // Swap the ICA and ACA: every link's issuer field now names the wrong certificate.
    let (mut c, p) = build(&base_specs());
    c.swap(1, 2);
    assert!(kinds(&c, &p).contains(&K::IssuerMismatch));
    // sha384AndDigest with the right HashedId8 but a P-256 issuer key.
    let (mut c, p) = build(&base_specs());
    if let IssuerIdentifier::sha256AndDigest(d) = c[0].0.issuer.clone() {
        c[0].0.issuer = IssuerIdentifier::sha384AndDigest(d);
    }
    // The EE's own HashedId8 now uses SHA-384, which nothing above names; only the
    // EE's link is wrong.
    only(&c, &p, K::IssuerMismatch);
}

#[test]
fn rule_explicit_signature_verifies() {
    let s = base_specs();
    let r = root(s.root.clone(), 11);
    let i = explicit(&r, s.ica.clone(), 21);
    let a = explicit(&i, s.aca.clone(), 31);
    let e = explicit(&a, s.ee.clone(), 41);
    let p = ChainPolicy::trusting([&r.cert]).unwrap();
    valid(
        &[
            e.cert.clone(),
            a.cert.clone(),
            i.cert.clone(),
            r.cert.clone(),
        ],
        &p,
    );
    // The EE signed by the ICA's key but naming the ACA.
    let wrong = Node {
        cert: a.cert.clone(),
        d: i.d,
        key: a.key.clone(),
    };
    let forged = explicit(&wrong, s.ee.clone(), 41);
    only(
        &[forged.cert, a.cert.clone(), i.cert.clone(), r.cert.clone()],
        &p,
        K::BadSignature,
    );
    // Tampered tbs after signing.
    let mut t = e.cert.clone();
    t.0.to_be_signed.crl_series = CrlSeries(Uint16(3));
    only(&[t, a.cert, i.cert, r.cert], &p, K::BadSignature);
}

#[test]
fn rule_implicit_key_reconstructs_and_tampering_moves_it() {
    // A tampered implicit EE still "validates" as a chain (there is no signature to
    // fail), but its reconstructed key no longer verifies what the subject signs: the
    // ECQV failure mode (02 §6). The SPDU check is where it is caught.
    let s = base_specs();
    let r = root(s.root.clone(), 11);
    let i = implicit(&r, s.ica.clone(), 21);
    let a = implicit(&i, s.aca.clone(), 31);
    let e = implicit(&a, s.ee.clone(), 41);
    let p = ChainPolicy::trusting([&r.cert]).unwrap();
    let digest = [7u8; 32];
    let sig = cert::ecdsa_p256_sign_with_k(&e.d, &Scalar::from(3u64), &digest).unwrap();
    let mut t = e.cert.clone();
    t.0.to_be_signed.crl_series = CrlSeries(Uint16(3));
    let v = chain::validate(&[t, a.cert.clone(), i.cert.clone(), r.cert.clone()], &p).unwrap();
    assert!(!cert::ecdsa_verify_digest(v.ee_key(), &digest, &sig));
    // An implicit certificate under a P-384 issuer cannot be reconstructed.
    let p384 = p384_root();
    let mut c = e.cert.clone();
    c.0.issuer = IssuerIdentifier::sha384AndDigest(cert::cert_id8(&p384.cert).unwrap());
    let pp = ChainPolicy::trusting([&p384.cert]).unwrap();
    let got = kinds(&[c, p384.cert.clone()], &pp);
    assert!(got.contains(&K::BadReconstruction), "{got:?}");
}

fn p384_root() -> Node {
    let key = v2xw_sec::p384::P384SigningKey::from_be48(&[0x42; 48]).unwrap();
    let sec1 = key.public_sec1(true);
    let x = OctetString::from(sec1[1..].to_vec());
    let pt = if sec1[0] == 2 {
        EccP384CurvePoint::compressed_y_0(x)
    } else {
        EccP384CurvePoint::compressed_y_1(x)
    };
    let mut s = Spec::ca("root384", all(1, -1));
    s.hours = u16::MAX;
    let tbs = s.tbs(VerificationKeyIndicator::verificationKey(
        PublicVerificationKey::ecdsaNistP384(pt),
    ));
    let c = cert::issue_explicit_cert(tbs, None, CertHash::Sha384, |dg| {
        let sg = key.sign_digest(&dg.try_into().unwrap());
        use v2xw_msg::sec_types::ieee1609_dot2_base_types::EcdsaP384Signature;
        Ok(Signature::ecdsaNistP384Signature(EcdsaP384Signature::new(
            EccP384CurvePoint::x_only(OctetString::from(sg.r.to_vec())),
            OctetString::from(sg.s.to_vec()),
        )))
    })
    .unwrap();
    assert_eq!(c.0.issuer, IssuerIdentifier::R_self(HashAlgorithm::sha384));
    Node {
        cert: c,
        d: Scalar::ONE,
        key: CertKey::P384(sec1),
    }
}

#[test]
fn a_p384_root_signs_an_explicit_ca_with_sha384() {
    let r = p384_root();
    assert!(cert::verify_explicit_cert(&r.cert, None).unwrap());
    assert_eq!(cert::whole_cert_hash(&r.cert).unwrap().len(), 48);
    // The P-384 root issues an explicit P-256 ICA (issuer = sha384AndDigest).
    let key = v2xw_sec::p384::P384SigningKey::from_be48(&[0x42; 48]).unwrap();
    let d = Scalar::from(77u64);
    let q = Point::mul_base(&d);
    let tbs = Spec::ca("ica", all(1, 0))
        .tbs(cert::verification_key_indicator(&q.compressed().unwrap()).unwrap());
    let ica = cert::issue_explicit_cert(tbs, Some((&r.cert, &r.key)), CertHash::Sha256, |dg| {
        let sg = key.sign_digest(&dg.try_into().unwrap());
        use v2xw_msg::sec_types::ieee1609_dot2_base_types::EcdsaP384Signature;
        Ok(Signature::ecdsaNistP384Signature(EcdsaP384Signature::new(
            EccP384CurvePoint::x_only(OctetString::from(sg.r.to_vec())),
            OctetString::from(sg.s.to_vec()),
        )))
    })
    .unwrap();
    assert!(matches!(ica.0.issuer, IssuerIdentifier::sha384AndDigest(_)));
    let ican = Node {
        cert: ica.clone(),
        d,
        key: CertKey::P256(q),
    };
    let ee = explicit(&ican, Spec::ee(), 41);
    let p = ChainPolicy::trusting([&r.cert]).unwrap();
    valid(&[ee.cert.clone(), ica.clone(), r.cert.clone()], &p);
    let mut bad = ica.clone();
    bad.0.to_be_signed.crl_series = CrlSeries(Uint16(9));
    assert!(!cert::verify_explicit_cert(&bad, Some((&r.cert, &r.key))).unwrap());
}

#[test]
fn rule_permissions_covered() {
    // EE PSID outside the ACA's explicit list.
    let (c, p) = with(|s| {
        s.aca.issue = Some(vec![group(
            Some(vec![rng(35, None)]),
            1,
            0,
            ee_type(true, false),
        )]);
    });
    only(&c, &p, K::PermissionNotCovered);
    // Positive: listed explicitly.
    let (c, p) = with(|s| {
        s.aca.issue = Some(vec![group(
            Some(vec![rng(32, None)]),
            1,
            0,
            ee_type(true, false),
        )]);
    });
    valid(&c, &p);
    // A subordinate `all` group needs an issuer `all` group.
    let (c, p) = with(|s| {
        s.ica.issue = Some(vec![group(
            Some(vec![rng(32, None)]),
            1,
            1,
            ee_type(true, false),
        )]);
    });
    only(&c, &p, K::PermissionNotCovered);
}

#[test]
fn rule_ee_type() {
    // The ACA may issue only enrolment certificates; the EE uses appPermissions.
    let (c, p) = with(|s| s.aca.issue = Some(vec![group(None, 1, 0, ee_type(false, true))]));
    // The ACA's own group {enrol} is also not inside the ICA's {app}.
    let got = kinds(&c, &p);
    assert_eq!(got, BTreeSet::from([K::EeTypeNotAllowed]));
    // A subordinate group's eeType must be inside the issuer group's: {app, enrol} under
    // {app} fails even though the EE below only uses app.
    let (c, p) = with(|s| s.aca.issue = Some(vec![group(None, 1, 0, ee_type(true, true))]));
    let errs = chain::validate(&c, &p).unwrap_err();
    assert_eq!(errs.len(), 1, "{errs:?}");
    assert_eq!((errs[0].index, errs[0].kind), (1, K::EeTypeNotAllowed));
    // certRequestPermissions (an enrolment certificate) needs an issuer with enrol.
    let (c, p) = with(|s| {
        s.ee.app = None;
        s.ee.request = Some(vec![all(1, 0)]);
    });
    only(&c, &p, K::EeTypeNotAllowed);
    let (c, p) = with(|s| {
        s.root.issue = Some(vec![group(None, 1, -1, ee_type(true, true))]);
        s.ica.issue = Some(vec![group(None, 1, 1, ee_type(true, true))]);
        s.aca.issue = Some(vec![group(None, 1, 0, ee_type(true, true))]);
        s.ee.app = None;
        s.ee.request = Some(vec![all(1, 0)]);
    });
    valid(&c, &p);
}

#[test]
fn rule_ssp_consistency() {
    let aca_with = |r: Option<SspRange>| {
        move |s: &mut Specs| {
            s.aca.issue = Some(vec![group(
                Some(vec![rng(32, r)]),
                1,
                0,
                ee_type(true, false),
            )]);
        }
    };
    let opaque = |b: &[u8]| {
        Some(ServiceSpecificPermissions::opaque(OctetString::from(
            b.to_vec(),
        )))
    };
    // Opaque SSP must equal an entry of an opaque range.
    let (c, p) = with(|s| {
        aca_with(Some(opaque_range(&[b"\x01", b"\x02"])))(s);
        s.ee.app = Some(vec![app(32, opaque(b"\x02"))]);
    });
    valid(&c, &p);
    let (c, p) = with(|s| {
        aca_with(Some(opaque_range(&[b"\x01", b"\x02"])))(s);
        s.ee.app = Some(vec![app(32, opaque(b"\x03"))]);
    });
    only(&c, &p, K::SspNotCovered);
    // Omitted SSP needs a zero-length entry (PsidSsp note).
    let (c, p) = with(|s| aca_with(Some(opaque_range(&[b"\x01"])))(s));
    only(&c, &p, K::SspNotCovered);
    let (c, p) = with(|s| aca_with(Some(opaque_range(&[b"\x01", b""])))(s));
    valid(&c, &p);
    // Bitmap SSP against a bitmap range (BitmapSspRange note): bits under the mask must
    // equal the range value; others are free.
    let bm = |v: &[u8], m: &[u8]| {
        Some(SspRange::bitmapSspRange(BitmapSspRange::new(
            OctetString::from(v.to_vec()),
            OctetString::from(m.to_vec()),
        )))
    };
    let bssp = |b: &[u8]| {
        Some(ServiceSpecificPermissions::bitmapSsp(BitmapSsp(
            OctetString::from(b.to_vec()),
        )))
    };
    let (c, p) = with(|s| {
        aca_with(bm(&[0x80, 0x00], &[0xC0, 0x00]))(s);
        s.ee.app = Some(vec![app(32, bssp(&[0xBF, 0xFF]))]);
    });
    valid(&c, &p);
    let (c, p) = with(|s| {
        aca_with(bm(&[0x80, 0x00], &[0xC0, 0x00]))(s);
        s.ee.app = Some(vec![app(32, bssp(&[0xC0, 0x00]))]);
    });
    only(&c, &p, K::SspNotCovered);
    // Longer than the range: invalid; shorter but reaching the last mask bit: valid.
    let (c, p) = with(|s| {
        aca_with(bm(&[0x80], &[0x80]))(s);
        s.ee.app = Some(vec![app(32, bssp(&[0x80, 0x00]))]);
    });
    only(&c, &p, K::SspNotCovered);
    let (c, p) = with(|s| {
        aca_with(bm(&[0x80, 0x00], &[0x80, 0x00]))(s);
        s.ee.app = Some(vec![app(32, bssp(&[0x80]))]);
    });
    valid(&c, &p);
    // Too short to reach the last mask bit, even though the missing bit's required value
    // is 0 (BitmapSsp note (b)).
    let (c, p) = with(|s| {
        aca_with(bm(&[0x80, 0x00], &[0x80, 0x01]))(s);
        s.ee.app = Some(vec![app(32, bssp(&[0x80]))]);
    });
    only(&c, &p, K::SspNotCovered);
    // A subordinate CA's opaque range must be a subset of the issuer's.
    let (c, p) = with(|s| {
        s.ica.issue = Some(vec![group(
            Some(vec![rng(32, Some(opaque_range(&[b"\x01", b""])))]),
            1,
            1,
            ee_type(true, false),
        )]);
        s.aca.issue = Some(vec![group(
            Some(vec![rng(32, Some(opaque_range(&[b"\x02"])))]),
            1,
            0,
            ee_type(true, false),
        )]);
        s.ee.app = Some(vec![app(32, opaque(b"\x02"))]);
    });
    only(&c, &p, K::SspNotCovered);
    // Bitmap range inside a bitmap range: issuer mask bits must be in the subordinate mask
    // with the same value.
    let (c, p) = with(|s| {
        s.ica.issue = Some(vec![group(
            Some(vec![rng(32, bm(&[0x80], &[0x80]))]),
            1,
            1,
            ee_type(true, false),
        )]);
        s.aca.issue = Some(vec![group(
            Some(vec![rng(32, bm(&[0x80], &[0xC0]))]),
            1,
            0,
            ee_type(true, false),
        )]);
        s.ee.app = Some(vec![app(32, bssp(&[0x80]))]);
    });
    valid(&c, &p);
    let (c, p) = with(|s| {
        s.ica.issue = Some(vec![group(
            Some(vec![rng(32, bm(&[0x80], &[0x80]))]),
            1,
            1,
            ee_type(true, false),
        )]);
        s.aca.issue = Some(vec![group(
            Some(vec![rng(32, bm(&[0x00], &[0x40]))]),
            1,
            0,
            ee_type(true, false),
        )]);
        s.ee.app = Some(vec![app(32, bssp(&[0x80]))]);
    });
    only(&c, &p, K::SspNotCovered);
}

#[test]
fn rule_chain_length_inequalities() {
    // mcr_i <= mcr_s + 1: the ACA demands two more levels, the EE is the last.
    let (c, p) = with(|s| s.aca.issue = Some(vec![all(2, 0)]));
    assert!(kinds(&c, &p).contains(&K::ChainLength));
    // mcr_i + clr_i >= mcr_s + clr_s + 1: the ICA allows one level below it only.
    let (c, p) = with(|s| s.ica.issue = Some(vec![all(1, 0)]));
    only(&c, &p, K::ChainLength);
    // minChainLength 0 makes the certificate invalid.
    let (c, p) = with(|s| s.aca.issue = Some(vec![all(0, 1)]));
    assert!(kinds(&c, &p).contains(&K::ChainLength));
    // At the root over the ICA's (1, 1): an exact fit (2, 1) is fine; (3, 0) fails the
    // first inequality (3 > 1 + 1) and (2, 0) the second (2 + 0 < 1 + 1 + 1).
    let (c, p) = with(|s| s.root.issue = Some(vec![all(2, 1)]));
    valid(&c, &p);
    let (c, p) = with(|s| s.root.issue = Some(vec![all(3, 0)]));
    only(&c, &p, K::ChainLength);
    let (c, p) = with(|s| s.root.issue = Some(vec![all(2, 0)]));
    only(&c, &p, K::ChainLength);
}

#[test]
fn rule_validity_inside_the_issuer() {
    let (c, p) = with(|s| s.ee.start = START - 10);
    only(&c, &p, K::ValidityNotContained);
    let (c, p) = with(|s| {
        s.aca.hours = 100;
        s.ee.start = START + 3600 * 90;
        s.ee.hours = 168;
    });
    only(&c, &p, K::ValidityNotContained);
    // Currency, when the policy asks for it.
    let (c, mut p) = build(&base_specs());
    p.now = Some(START + 3600 * 2);
    valid(&c, &p);
    p.now = Some(START + 3600 * 200);
    only(&c, &p, K::Expired);
}

#[test]
fn rule_region_inside_the_effective_issuer_region() {
    let (c, p) = with(|s| {
        s.root.region = Some(countries(&[840, 124]));
        s.ee.region = Some(countries(&[840]));
    });
    valid(&c, &p);
    // Absent at the ICA and ACA: inherited from the root, so the EE is checked against it.
    let (c, p) = with(|s| {
        s.root.region = Some(countries(&[840]));
        s.ee.region = Some(countries(&[124]));
    });
    only(&c, &p, K::RegionNotContained);
    // Absent everywhere above: worldwide.
    let (c, p) = with(|s| s.ee.region = Some(countries(&[124])));
    valid(&c, &p);
    // countryAndRegions inside countryOnly, and not the other way round.
    let (c, p) = with(|s| {
        s.aca.region = Some(countries(&[840]));
        s.ee.region = Some(country_regions(840, &[6, 12]));
    });
    valid(&c, &p);
    let (c, p) = with(|s| {
        s.aca.region = Some(country_regions(840, &[6, 12]));
        s.ee.region = Some(countries(&[840]));
    });
    only(&c, &p, K::RegionNotContained);
    let (c, p) = with(|s| {
        s.aca.region = Some(country_regions(840, &[6, 12]));
        s.ee.region = Some(country_regions(840, &[6, 13]));
    });
    only(&c, &p, K::RegionNotContained);
    // A circular region under an identified one is not decided: fails closed.
    let (c, p) = with(|s| {
        s.aca.region = Some(countries(&[840]));
        s.ee.region = Some(GeographicRegion::circularRegion(CircularRegion::new(
            TwoDLocation::new(
                Latitude(NinetyDegreeInt(407_000_000)),
                Longitude(OneEightyDegreeInt(-740_000_000)),
            ),
            Uint16(1000),
        )));
    });
    only(&c, &p, K::RegionUnsupported);
}

#[test]
fn the_same_aca_response_rule() {
    let s = base_specs();
    let r = root(s.root.clone(), 11);
    let i = implicit(&r, s.ica.clone(), 21);
    let a = implicit(&i, s.aca.clone(), 31);
    let a2 = implicit(&i, Spec::ca("aca2", all(1, 0)), 32);
    let e = implicit(&a, s.ee.clone(), 41);
    assert!(chain::same_aca_response(&a.cert, &e.cert));
    assert!(!chain::same_aca_response(&a2.cert, &e.cert));
}
