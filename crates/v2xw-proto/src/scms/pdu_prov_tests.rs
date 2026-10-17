//! Unit tests of the WP-B3a provisioning PDUs: the whole original-butterfly chain from the
//! LAs' pre-linkage values to the device's key, and every receiver rule refusing the PDU
//! that breaks it.

use super::*;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::AcaSsp;
use v2xw_sec::butterfly;
use v2xw_sec::linkage::{LaId, LinkageSeed};

const T: u32 = 694_310_405;
const JMAX: u32 = 20;

fn pki() -> ScmsPki {
    ScmsPki::new(7, T - 1_000).expect("PKI")
}

fn cat() -> Caterpillar {
    Caterpillar::from_seed(&[0x5Au8; butterfly::SEED_BYTES]).unwrap()
}

/// What the LAs, the RA and the ACA produce for `(i, j)`, and the PLVs the LAs computed.
struct One {
    request: Pdu,
    issued: AcaIssued,
    plv: [PreLinkageValue; 2],
}

fn one(pki: &ScmsPki, keys: &LaAcaKeys, cat: &Caterpillar, i: u32, j: u32, g: &mut Drbg) -> One {
    one_with(pki, keys, cat, i, j, g, &[])
}

fn one_with(
    pki: &ScmsPki,
    keys: &LaAcaKeys,
    cat: &Caterpillar,
    i: u32,
    j: u32,
    g: &mut Drbg,
    psids: &[u64],
) -> One {
    let seeds = [LinkageSeed::new([0x11; 16]), LinkageSeed::new([0x22; 16])];
    let ids = [1u16, 2u16];
    let plv: [PreLinkageValue; 2] = core::array::from_fn(|k| {
        let ls = linkage::linkage_seed_at(LaId(ids[k]), seeds[k], i);
        linkage::pre_linkage_value(LaId(ids[k]), ls, j)
    });
    let enc: [EncryptedIndividualPLV; 2] = core::array::from_fn(|k| {
        encrypted_individual_plv(ids[k], &plv[k], &keys.current(ids[k], T), g).unwrap()
    });
    let (b, jk) = butterfly::ra_cocoon_keys(
        &cat.signing_public(),
        &cat.encryption_public(),
        cat.ck(),
        cat.ek(),
        i,
        j,
    );
    let x = RaAcaInput {
        generation_time: T,
        i: u16::try_from(i).unwrap(),
        cocoon_signing: b,
        cocoon_encryption: jk,
        plv: enc,
        valid_from: T,
        dummy_lv: [0xEE; PLV_BYTES],
        psids: psids.to_vec(),
    };
    let request = ra_aca_cert_request_spdu(pki, &x).unwrap();
    let issued = aca_issue(pki, keys, request.bytes(), T, g).unwrap();
    One {
        request,
        issued,
        plv,
    }
}

#[test]
fn every_certificate_of_two_periods_yields_a_key_the_device_derives() {
    // Acceptance (00 §4.2 WP-B3a): the device derives every private key, and
    // d·G = e·P_U + Q_ACA holds for all 20 × n certificates (here n = 2).
    let pki = pki();
    let keys = LaAcaKeys::new(9, 7 * 86_400);
    let cat = cat();
    let mut g = Drbg::new(3, b"t");
    let q_aca = Point::mul_base(pki.aca.private.as_ref().unwrap());
    let mut lens = std::collections::BTreeSet::new();
    for i in [614u32, 615] {
        for j in 0..JMAX {
            let o = one(&pki, &keys, &cat, i, j, &mut g);
            // The RA opens the ACA's answer and files the private response as it is.
            let (rh, private) = ra_open_aca_response(&pki, o.issued.to_ra.bytes()).unwrap();
            assert_eq!(rh, o.request.hashed_id8());
            assert_eq!(private.bytes(), o.issued.private.bytes());
            let p = ee_open_aca_response(&pki, private.bytes(), &cat, i, j).unwrap();
            // The identity, recomputed here from the certificate alone.
            let VerificationKeyIndicator::reconstructionValue(pu) =
                &p.certificate.0.to_be_signed.verify_key_indicator
            else {
                panic!("implicit");
            };
            let e = cert::implicit_e(&p.certificate, &pki.aca.cert).unwrap();
            let rhs = pdu::point_of(pu).unwrap().mul(&e).add(&q_aca);
            assert_eq!(Point::mul_base(&p.private).compressed(), rhs.compressed());
            // The linkage value is the two LAs' contributions, and nobody else's.
            assert_eq!(p.lv, linkage::linkage_value(o.plv[0], o.plv[1]));
            assert_eq!(p.lv, o.issued.lv);
            let tbs = &p.certificate.0.to_be_signed;
            assert_eq!(tbs.craca_id.0[..], pki.craca_id()[..]);
            assert_eq!(tbs.crl_series.0.0, PSEUDONYM_CRL_SERIES);
            assert_eq!(u32::from(p.i), i);
            lens.insert(cert::encode(&p.certificate).unwrap().len());
        }
    }
    // Every implicit pseudonym certificate has one size (CAMP's example is 86 B in its
    // pre-1609.2.1 format, [CAMP122] Table 3; the 1609.2 one is measured here).
    assert_eq!(lens.len(), 1, "{lens:?}");
    eprintln!("implicit pseudonym certificate: {:?} bytes", lens);
}

#[test]
fn a_response_signed_by_a_different_aca_is_rejected() {
    // Mutation (00 §4.2 WP-B3a): the same-ACA rule. A second, fully legitimate ACA of the
    // same PKI re-signs the encrypted payload of the first ACA's response. The signature
    // verifies and the signer is an ACA; only the rule can refuse it.
    let mut pki = pki();
    let idx = pki
        .issue_component(
            7,
            "aca2.provider-a",
            &SecurityMgmtSsp::aca(AcaSsp::new(Uint8(2))),
            false,
            T - 1_000,
        )
        .unwrap();
    let aca2 = pki.extra[idx].clone();
    let keys = LaAcaKeys::new(9, 0);
    let cat = cat();
    let mut g = Drbg::new(4, b"t");
    let o = one(&pki, &keys, &cat, 614, 3, &mut g);
    let payload = pdu::open_signed(o.issued.private.bytes(), &pki)
        .unwrap()
        .payload;
    let forged = pdu::signed_bytes(payload.clone(), &aca2.cert, &aca2.key).unwrap();
    assert!(
        pdu::open_signed(&forged, &pki).is_ok(),
        "the forgery verifies"
    );
    assert_eq!(
        ee_open_aca_response(&pki, &forged, &cat, 614, 3).unwrap_err(),
        PduError::Signer("same-ACA rule: the outer signer did not issue the certificate")
    );
    // Control: re-signed by the issuing ACA, the same payload opens.
    let honest = pdu::signed_bytes(payload, &pki.aca.cert, &pki.aca.key).unwrap();
    assert!(ee_open_aca_response(&pki, &honest, &cat, 614, 3).is_ok());
    // Signed by a non-ACA component (the RA), it is refused before decryption.
    let ra_signed = pdu::signed_bytes(
        pdu::open_signed(o.issued.private.bytes(), &pki)
            .unwrap()
            .payload,
        &pki.ra.cert,
        &pki.ra.key,
    )
    .unwrap();
    assert_eq!(
        ee_open_aca_response(&pki, &ra_signed, &cat, 614, 3).unwrap_err(),
        PduError::Signer("AcaEeCertResponsePrivateSpdu is signed by an ACA")
    );
}

#[test]
fn only_the_right_cocoon_key_opens_a_response_and_tampering_is_caught() {
    let pki = pki();
    let keys = LaAcaKeys::new(9, 0);
    let cat = cat();
    let mut g = Drbg::new(5, b"t");
    let o = one(&pki, &keys, &cat, 614, 7, &mut g);
    // h_ij of another j addresses another rekRecipInfo.
    assert_eq!(
        ee_open_aca_response(&pki, o.issued.private.bytes(), &cat, 614, 8).unwrap_err(),
        PduError::NotForMe
    );
    // Another device's caterpillar.
    let other = Caterpillar::from_seed(&[0x33u8; butterfly::SEED_BYTES]).unwrap();
    assert_eq!(
        ee_open_aca_response(&pki, o.issued.private.bytes(), &other, 614, 7).unwrap_err(),
        PduError::NotForMe
    );
    // Any flipped byte is refused (signature, shape or decryption).
    let b = o.issued.private.bytes();
    for k in [b.len() - 70, b.len() / 2, 40] {
        let mut t = b.to_vec();
        t[k] ^= 0x01;
        assert!(
            ee_open_aca_response(&pki, &t, &cat, 614, 7).is_err(),
            "byte {k}"
        );
    }
}

#[test]
fn the_aca_refuses_requests_the_profile_does_not_allow() {
    let pki = pki();
    let keys = LaAcaKeys::new(9, 0);
    let cat = cat();
    let mut g = Drbg::new(6, b"t");
    let o = one(&pki, &keys, &cat, 614, 0, &mut g);
    let (p, _) = pdu::open_signed_cert_request(o.request.bytes(), &pki).unwrap();
    let ScmsPduContent::aca_ra(AcaRaInterfacePdu::raAcaCertRequest(r0)) = p.content else {
        panic!("shape");
    };
    let resign = |r: RaAcaCertRequest, signer: &pdu::Authority| {
        let p = pdu::pdu(ScmsPduContent::aca_ra(AcaRaInterfacePdu::raAcaCertRequest(
            r,
        )));
        pdu::signed_cert_request(&p, RequestSigner::Cert(&signer.cert, &signer.key)).unwrap()
    };
    let mut g2 = Drbg::new(6, b"u");
    // Signed by the ECA, not the RA.
    let by_eca = resign(r0.clone(), &pki.eca);
    assert_eq!(
        aca_issue(&pki, &keys, &by_eca, T, &mut g2).unwrap_err(),
        PduError::Signer("RaAcaCertRequestSpdu is signed by an RA")
    );
    // cubk set.
    let mut r = r0.clone();
    r.flags.0.set(1, true);
    assert!(matches!(
        aca_issue(&pki, &keys, &resign(r, &pki.ra), T, &mut g2),
        Err(PduError::Shape(_))
    ));
    // No certEncKey.
    let mut r = r0.clone();
    r.cert_enc_key = None;
    assert!(matches!(
        aca_issue(&pki, &keys, &resign(r, &pki.ra), T, &mut g2),
        Err(PduError::Shape(_))
    ));
    // Both PLVs from one LA.
    let mut r = r0.clone();
    let li = r.linkage_info.as_mut().unwrap();
    li.enc_plv2 = li.enc_plv1.clone();
    assert!(matches!(
        aca_issue(&pki, &keys, &resign(r, &pki.ra), T, &mut g2),
        Err(PduError::Shape(_))
    ));
    // A PLV sealed under a key the ACA does not share.
    let mut r = r0;
    let wrong = LaAcaKeys::new(10, 0);
    let plv = PreLinkageValue::from_bytes([1; PLV_BYTES]);
    r.linkage_info.as_mut().unwrap().enc_plv1 =
        encrypted_individual_plv(1, &plv, &wrong.current(1, T), &mut g2).unwrap();
    assert_eq!(
        aca_issue(&pki, &keys, &resign(r, &pki.ra), T, &mut g2).unwrap_err(),
        PduError::NotForMe
    );
}

#[test]
fn a_plv_sealed_before_a_key_rotation_still_opens_one_epoch_later() {
    let week = 7 * 86_400;
    let keys = LaAcaKeys::new(9, week);
    let mut g = Drbg::new(8, b"t");
    let plv = PreLinkageValue::from_bytes([0xA5; PLV_BYTES]);
    let t0 = week * 3_000 + 5;
    let e = encrypted_individual_plv(2, &plv, &keys.current(2, t0), &mut g).unwrap();
    assert_eq!(aca_open_plv(&e, &keys, t0).unwrap(), (2, plv));
    assert_eq!(aca_open_plv(&e, &keys, t0 + week).unwrap(), (2, plv));
    assert_eq!(
        aca_open_plv(&e, &keys, t0 + 2 * week).unwrap_err(),
        PduError::NotForMe
    );
    // LA1's key is not LA2's.
    let mut e1 = e;
    e1.la_id = AsnLaId(FixedOctetString::from(1u16.to_be_bytes()));
    assert_eq!(
        aca_open_plv(&e1, &keys, t0).unwrap_err(),
        PduError::NotForMe
    );
}

#[test]
fn batch_names_follow_the_profile() {
    assert_eq!(batch_entry_name(0x266, 0x13), "266_13");
    assert_eq!(
        pdu::batch_filename([0xAB, 0x09, 0x28, 0x1C, 0x98, 0x67, 0xDE, 0x53], 15),
        "AB09281C9867DE53_F.zip"
    );
    // One 10-byte entry: 30 + 46 + 2·6 + 10 + 22.
    assert_eq!(stored_zip_len(&[("266_13".into(), 10)]), 120);
}

/// WP-C2: a receiver validates the whole chain, so the PKI's component certificates carry
/// the published issue permissions and a pseudonym the ACA issued validates to the root
/// under the US device policy. The RA's published `{all, 0, 0}` group is tolerated and
/// reported; the pseudonym permits exactly the PSIDs the request asked for.
#[test]
fn a_pseudonym_and_every_component_validate_to_the_root_under_the_us_policy() {
    use v2xw_sec::chain::{self, ChainPolicy};
    let pki = pki();
    let anchors = std::collections::BTreeSet::from([cert::hashed_id32(&pki.root.cert).unwrap()]);
    let us = ChainPolicy::us_device(anchors, T + 10);
    let chain_of = |c: &Certificate| -> Vec<Certificate> {
        let mut v = vec![c.clone()];
        while let Some(Ok(d)) = match &v.last().unwrap().0.issuer {
            IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
                Some(Ok::<_, ()>(hashedid::id8_bytes(d)))
            }
            _ => None,
        } {
            v.push(pki.by_id8(&d).expect("issuer in the PKI").cert.clone());
        }
        v
    };
    for a in pki.all() {
        let v = chain::validate(&chain_of(&a.cert), &us)
            .unwrap_or_else(|e| panic!("{}: {e:?}", a.name));
        assert_eq!(
            v.tolerated.is_empty(),
            a.name != "ra.provider-a",
            "{}: only the RA uses the tolerated profile",
            a.name
        );
    }
    let keys = LaAcaKeys::new(9, 7 * 86_400);
    let cat = cat();
    let mut g = Drbg::new(3, b"t");
    let psids = [0x20u64, 38, 0xE000_0016];
    let o = one_with(&pki, &keys, &cat, 614, 3, &mut g, &psids);
    let (_, private) = ra_open_aca_response(&pki, o.issued.to_ra.bytes()).unwrap();
    let p = ee_open_aca_response(&pki, private.bytes(), &cat, 614, 3).unwrap();
    let c = chain_of(&p.certificate);
    assert_eq!(c.len(), 4, "pseudonym, ACA, ICA, root");
    let v = chain::validate(&c, &us).expect("the pseudonym chain validates");
    assert!(v.tolerated.is_empty());
    for psid in psids {
        assert!(chain::ee_permits_psid(&p.certificate, psid), "{psid:#x}");
    }
    assert!(!chain::ee_permits_psid(&p.certificate, 35));
}
