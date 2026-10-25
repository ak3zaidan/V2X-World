//! The 1609.2.1 PDUs of pseudonym provisioning behind the RA (WP-B3a).
//!
//! The original-butterfly path of the US profile, from the Linkage Authorities' pre-linkage
//! values to the key the device signs with (02 §9, §10.3; 03 §3.2, §3.3, §9):
//!
//! | step | PDU | protection | here |
//! |---|---|---|---|
//! | LA → ACA (carried by the RA) | `EncryptedIndividualPLV` | AES-128-CCM under the key that LA shares with the ACA, one `symmRecipInfo` naming its HashedId8 | [`encrypted_individual_plv`], [`aca_open_plv`] |
//! | RA → ACA, one per certificate | `RaAcaCertRequestSpdu` | SignedCertRequest, `SignerSingleCert` = the RA | [`ra_aca_cert_request_spdu`] |
//! | ACA → EE (through the RA) | `AcaEeCertResponsePrivateSpdu` | EncryptedSigned: to the cocoon encryption key `J_ij`, then signed by **the ACA that issued the certificate** | [`aca_issue`] |
//! | ACA → RA | `AcaRaCertResponseSpdu` | Signed, ACA | [`aca_issue`] |
//! | EE | — | verify, decrypt with `h_ij`, same-ACA rule, `d = e·b_ij + r`, `d·G = e·P_U + Q_ACA` | [`ee_open_aca_response`] |
//!
//! What each party can read is the point. The RA builds the request from the cocoon keys
//! `B_ij`, `J_ij` and two ciphertexts it cannot open; it keeps `j` (the ACA never sees it)
//! and the request hash that routes the answer back. The ACA sees `B_ij`, `J_ij`, `iCert`
//! and the decrypted pre-linkage values, hence the linkage value, and nothing that names
//! the device. The device learns its key only by decrypting with `h_ij`.
//!
//! The LA↔ACA keys: 1609.2.1 leaves how the key is shared out of scope, and CAMP makes its
//! rotation a local option (`shared_key_update_interval`, [CAMP122] Table 68). Here each LA
//! has one key per rotation epoch, drawn from the deployment seed ([`LaAcaKeys`]); the ACA
//! finds the key by the `symmRecipInfo` HashedId8 among the current and the previous epoch,
//! so a PLV sealed just before a rotation still opens.

use p256::Scalar;
use rasn::types::{FixedBitString, FixedOctetString, OctetString};
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, CertificateId, CertificateType, Ieee1609Dot2Content, Ieee1609Dot2Data,
    IssuerIdentifier, RecipientInfo, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    CrlSeries, Duration as Ieee1609Duration, HashedId3, IValue, LaId as AsnLaId,
    LinkageValue as AsnLinkageValue, Opaque, Psid, PsidSsp, SequenceOfPsidSsp,
    ServiceSpecificPermissions, Time32, Uint8, Uint16, Uint32, ValidityPeriod,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ee_interface::{
    AcaEeCertResponse, AcaEeInterfacePdu,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_aca_ra_interface::{
    AcaRaCertResponse, AcaRaInterfacePdu, AcaResponse, EncryptedIndividualPLV,
    EncryptedIndividualPLVEncPlv, LinkageInfo, RaAcaCertRequest, RaAcaCertRequestFlags,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_ee_ra_interface::EeRaInterfacePdu;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::{ScmsPduContent, SecurityMgmtSsp};
use v2xw_sec::butterfly::Caterpillar;
use v2xw_sec::cert;
use v2xw_sec::ec::{self, Point};
use v2xw_sec::hashedid;
use v2xw_sec::linkage::{self, LinkageValue, PLV_BYTES, PreLinkageValue};

use super::pdu::{
    self, BSM_PSID, Drbg, Pdu, PduError, RequestSigner, RequestSignerFound, Result,
    SECURITY_MGMT_PSID, ScmsPki,
};

/// The structure version every 1609.2.1 PDU here carries.
const STRUCT_VERSION: u8 = 2;

/// The pseudonym certificate's validity: 10,140 minutes, one week plus the hour of overlap
/// ([SMP] Table 2; 03 §6.1).
pub const PSEUDONYM_LIFETIME_MIN: u16 = 10_140;

/// The CRL series of a linkage-revoked pseudonym certificate (03 §3.4: series 1 is the
/// linkage-based CRL).
pub const PSEUDONYM_CRL_SERIES: u16 = 1;

// ---------------------------------------------------------------------------------------
// LA ↔ ACA keys
// ---------------------------------------------------------------------------------------

/// The AES-128 keys each Linkage Authority shares with the ACA, one per rotation epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaAcaKeys {
    seed: u64,
    /// Rotation interval in seconds (`shared_key_update_interval`); 0 never rotates.
    pub rotation_s: u32,
}

impl LaAcaKeys {
    /// The keys of a deployment seeded with `seed`, rotating every `rotation_s` seconds of
    /// `Time32` (0: one key for ever).
    #[must_use]
    pub const fn new(seed: u64, rotation_s: u32) -> LaAcaKeys {
        LaAcaKeys { seed, rotation_s }
    }

    /// The rotation epoch a `Time32` falls in.
    #[must_use]
    pub const fn epoch(&self, t32: u32) -> u32 {
        if self.rotation_s == 0 {
            0
        } else {
            t32 / self.rotation_s
        }
    }

    /// LA `la_id`'s key for `epoch`.
    #[must_use]
    pub fn key(&self, la_id: u16, epoch: u32) -> [u8; 16] {
        let mut label = b"la-aca:".to_vec();
        label.extend_from_slice(&la_id.to_be_bytes());
        label.extend_from_slice(&epoch.to_be_bytes());
        Drbg::new(self.seed, &label).key16()
    }

    /// The key LA `la_id` seals with at `t32`.
    #[must_use]
    pub fn current(&self, la_id: u16, t32: u32) -> [u8; 16] {
        self.key(la_id, self.epoch(t32))
    }

    /// The ACA's lookup: the key of LA `la_id` whose HashedId8 is `rid`, in the epoch of
    /// `t32` or the one before.
    #[must_use]
    pub fn find(&self, la_id: u16, rid: [u8; 8], t32: u32) -> Option<[u8; 16]> {
        let e = self.epoch(t32);
        [Some(e), e.checked_sub(1)]
            .into_iter()
            .flatten()
            .map(|e| self.key(la_id, e))
            .find(|k| pdu::symm_recipient_id(k).is_ok_and(|r| r == rid))
    }
}

// ---------------------------------------------------------------------------------------
// LA → ACA: EncryptedIndividualPLV
// ---------------------------------------------------------------------------------------

/// LA side: `EncryptedIndividualPLV {version 2, laId, encPlv}`; `encPlv` is
/// `SymmEncryptedSingleRecipient{PreLinkageValue}` under `shared`. The plaintext is the
/// 9 bytes encapsulated as `Ieee1609Dot2Data-Unsecured`: 1609.2 encrypts an
/// `Ieee1609Dot2Data` ("encapsulating Tbe in an Ieee1609Dot2Data of type unsecured if Tbe
/// is not already an Ieee1609Dot2Data", [ASN-2.1] Protocol `Ieee1609Dot2Data-Encrypted`;
/// 02 §8.3). WP-F1: the conformance oracle found the bare 9 bytes encrypted before.
///
/// # Errors
/// Encoding fails.
pub fn encrypted_individual_plv(
    la_id: u16,
    plv: &PreLinkageValue,
    shared: &[u8; 16],
    g: &mut Drbg,
) -> Result<EncryptedIndividualPLV> {
    let inner = pdu::coer(&Ieee1609Dot2Data::new(
        Uint8(pdu::PROTOCOL_VERSION),
        Ieee1609Dot2Content::unsecuredData(Opaque(plv.as_bytes().to_vec().into())),
    ))?;
    let bytes = pdu::symm_encrypt_single(&inner, shared, g)?;
    let d: Ieee1609Dot2Data = pdu::decode(&bytes)?;
    Ok(EncryptedIndividualPLV::new(
        Uint8(STRUCT_VERSION),
        AsnLaId(FixedOctetString::from(la_id.to_be_bytes())),
        EncryptedIndividualPLVEncPlv::new(d.protocol_version, d.content),
    ))
}

/// The COER of an [`EncryptedIndividualPLV`].
///
/// # Errors
/// Encoding fails.
pub fn encode_plv(e: &EncryptedIndividualPLV) -> Result<Vec<u8>> {
    pdu::coer(e)
}

/// ACA side: the 9-byte pre-linkage value inside `e`, decrypted with the key of its LA
/// that `keys` finds by the `symmRecipInfo` HashedId8.
///
/// # Errors
/// Wrong version, no key with that HashedId8, or a CCM failure.
pub fn aca_open_plv(
    e: &EncryptedIndividualPLV,
    keys: &LaAcaKeys,
    t32: u32,
) -> Result<(u16, PreLinkageValue)> {
    if e.version.0 != STRUCT_VERSION {
        return Err(PduError::Version("EncryptedIndividualPLV.version"));
    }
    let la_id = u16::from_be_bytes([e.la_id.0[0], e.la_id.0[1]]);
    let Ieee1609Dot2Content::encryptedData(enc) = &e.enc_plv.content else {
        return Err(PduError::Shape("encPlv is encryptedData"));
    };
    let [RecipientInfo::symmRecipInfo(s)] = &enc.recipients.0[..] else {
        return Err(PduError::Shape("encPlv has exactly one symmRecipInfo"));
    };
    let rid = hashedid::id8_bytes(&s.recipient_id);
    let key = keys.find(la_id, rid, t32).ok_or(PduError::NotForMe)?;
    let data = Ieee1609Dot2Data::new(
        e.enc_plv.protocol_version.clone(),
        e.enc_plv.content.clone(),
    );
    let plain = pdu::decrypt_symm(&pdu::coer(&data)?, &key)?;
    let Ieee1609Dot2Content::unsecuredData(Opaque(plain)) =
        pdu::decode::<Ieee1609Dot2Data>(&plain)?.content
    else {
        return Err(PduError::Shape("a sealed PLV decrypts to unsecuredData"));
    };
    let b: [u8; PLV_BYTES] = plain[..]
        .try_into()
        .map_err(|_| PduError::Shape("a PreLinkageValue is 9 bytes"))?;
    Ok((la_id, PreLinkageValue::from_bytes(b)))
}

// ---------------------------------------------------------------------------------------
// RA → ACA: RaAcaCertRequestSpdu
// ---------------------------------------------------------------------------------------

/// What the RA puts in one `RaAcaCertRequest`.
#[derive(Debug, Clone)]
pub struct RaAcaInput {
    /// `generationTime`.
    pub generation_time: u32,
    /// `iCert`: the certificate's i-period.
    pub i: u16,
    /// The cocoon signing key `B_ij` (`tbsCert.verifyKeyIndicator`).
    pub cocoon_signing: Point,
    /// The cocoon encryption key `J_ij` (`certEncKey`: selects the private response).
    pub cocoon_encryption: Point,
    /// LA1's and LA2's encrypted pre-linkage values.
    pub plv: [EncryptedIndividualPLV; 2],
    /// Validity start (`Time32`).
    pub valid_from: u32,
    /// The 9 random bytes the RA puts where the linkage value goes, for the ACA to
    /// replace ([ASN-2.1] note on `EeRaCertRequest.tbsCert`).
    pub dummy_lv: [u8; PLV_BYTES],
    /// The `appPermissions` PSIDs the EE requested (the RA forwards its `tbsCert`);
    /// empty means BSM only, the B3a profile.
    pub psids: Vec<u64>,
}

/// The pseudonym `tbsCert` the RA forwards (03 §3.3): `linkageData {iCert, dummy LV}`,
/// `cracaId 000000`, `crlSeries 0` (the ACA replaces both), one week plus an hour from
/// `valid_from`, and the requested PSIDs (BSM when none). Its `verifyKeyIndicator` is set
/// by [`aca_ra_request`].
fn pseudonym_tbs(
    x: &RaAcaInput,
) -> Result<v2xw_msg::sec_types::ieee1609_dot2::ToBeSignedCertificate> {
    Ok(
        v2xw_msg::sec_types::ieee1609_dot2::ToBeSignedCertificate::new(
            CertificateId::linkageData(v2xw_msg::sec_types::ieee1609_dot2::LinkageData::new(
                IValue(Uint16(x.i)),
                AsnLinkageValue(FixedOctetString::from(x.dummy_lv)),
                None,
            )),
            HashedId3(FixedOctetString::from([0u8; 3])),
            CrlSeries(Uint16(0)),
            ValidityPeriod::new(
                Time32(Uint32(x.valid_from)),
                Ieee1609Duration::minutes(Uint16(PSEUDONYM_LIFETIME_MIN)),
            ),
            None,
            None,
            Some(SequenceOfPsidSsp(if x.psids.is_empty() {
                vec![PsidSsp::new(
                    Psid(rasn::types::Integer::from(BSM_PSID)),
                    None,
                )]
            } else {
                x.psids
                    .iter()
                    .map(|p| PsidSsp::new(Psid(rasn::types::Integer::from(*p)), None))
                    .collect()
            })),
            None,
            None,
            None,
            None,
            VerificationKeyIndicator::verificationKey(pdu::sig1609_key_of(&x.cocoon_signing)?),
            None,
            None,
            None,
            None,
        ),
    )
}

/// THE `aca-ra.raAcaCertRequest` builder, for every certificate the ACA issues: a
/// pseudonym certificate (`linkage` present, WP-B3a), an RSU application or an OBU
/// identification certificate (`linkage` absent, WP-B3b).
///
/// The rules are the ASN.1 comments of `Ieee1609Dot2Dot1AcaRaInterface` (1609.2.1-2022,
/// `tools/oracle/asn1/ieee1609.2.1-2022`): `type implicit` (the US profile's certificates
/// are ECQV); `flags` all clear (neither `butterflyExplicit`, which needs `explicit`, nor
/// `cubk`, refused in the US profile); "if type is implicit, the request is valid only if
/// tbsCert.verifyKeyIndicator is a reconstructionValue", so `key` (the cocoon signing key
/// `B_ij`, or the EE's request key `R_U`) goes there as the compressed point;
/// `certEncKey` present with `cubk` clear selects `AcaEeCertResponsePrivateSpdu`, so
/// `cert_enc_key` (`J_ij`, or the response key `U`) is the key the response is encrypted
/// to; `linkageInfo`, when present, carries the two LAs' encrypted pre-linkage values and
/// then `tbsCert.id` is `LinkageData` with a dummy value. The requested `cracaId` and
/// `crlSeries` are what the ACA replaces.
///
/// Before WP-P2 the pseudonym path put `B_ij` in a `verificationKey`, which the module
/// comment makes an invalid implicit request; one builder makes the two paths agree.
///
/// # Errors
/// A key is the point at infinity or does not encode.
pub fn aca_ra_request(
    generation_time: u32,
    requested: &v2xw_msg::sec_types::ieee1609_dot2::ToBeSignedCertificate,
    key: &Point,
    cert_enc_key: &Point,
    linkage: Option<LinkageInfo>,
) -> Result<v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPdu> {
    let mut tbs = requested.clone();
    let r = cert::compressed_key(key).map_err(|_| PduError::Key("R at infinity"))?;
    tbs.verify_key_indicator =
        cert::reconstruction_key_indicator(&r).map_err(|_| PduError::Key("R"))?;
    Ok(pdu::pdu(ScmsPduContent::aca_ra(
        AcaRaInterfacePdu::raAcaCertRequest(RaAcaCertRequest::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(generation_time)),
            CertificateType::implicit,
            RaAcaCertRequestFlags(FixedBitString::default()),
            linkage,
            Some(pdu::public_encryption_key(cert_enc_key)?),
            tbs,
        )),
    )))
}

/// `aca-ra.raAcaCertRequest` for one pseudonym (03 §3.3): [`aca_ra_request`] over
/// [`RaAcaInput`]'s `tbsCert` with `linkageInfo`, `certEncKey = J_ij` and
/// `reconstructionValue = B_ij`.
///
/// # Errors
/// A key does not encode.
pub fn ra_aca_cert_request(
    x: &RaAcaInput,
) -> Result<v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ScmsPdu> {
    aca_ra_request(
        x.generation_time,
        &pseudonym_tbs(x)?,
        &x.cocoon_signing,
        &x.cocoon_encryption,
        Some(LinkageInfo::new(x.plv[0].clone(), x.plv[1].clone())),
    )
}

/// `RaAcaCertRequestSpdu`: [`ra_aca_cert_request`] signed by the RA certificate.
///
/// # Errors
/// Encoding fails.
pub fn ra_aca_cert_request_spdu(pki: &ScmsPki, x: &RaAcaInput) -> Result<Pdu> {
    let p = ra_aca_cert_request(x)?;
    Ok(Pdu::new(pdu::signed_cert_request(
        &p,
        RequestSigner::Cert(&pki.ra.cert, &pki.ra.key),
    )?))
}

// ---------------------------------------------------------------------------------------
// ACA: issuance and the private response
// ---------------------------------------------------------------------------------------

/// What the ACA produced for one request.
#[derive(Debug, Clone)]
pub struct AcaIssued {
    /// `AcaEeCertResponsePrivateSpdu`: what the RA files as `i_j` for the device.
    pub private: Pdu,
    /// `AcaRaCertResponseSpdu`, carrying `private` and the request's HashedId8.
    pub to_ra: Pdu,
    /// The implicit pseudonym certificate (the ACA's own record of what it issued).
    pub certificate: Certificate,
    /// The linkage value it inserted.
    pub lv: LinkageValue,
    /// `iCert`.
    pub i: u16,
    /// The HashedId8 of the `RaAcaCertRequestSpdu` (`AcaRaCertResponse.requestHash`).
    pub request_hash: [u8; 8],
}

fn ssp_of(c: &Certificate) -> Option<SecurityMgmtSsp> {
    let perms = c.0.to_be_signed.app_permissions.as_ref()?;
    perms.0.iter().find_map(|p| {
        if p.psid.0 != rasn::types::Integer::from(SECURITY_MGMT_PSID) {
            return None;
        }
        match p.ssp.as_ref()? {
            ServiceSpecificPermissions::opaque(o) => pdu::decode(o).ok(),
            _ => None,
        }
    })
}

/// Whether `c` carries the 1609.2.1 ACA SSP under PSID 35.
#[must_use]
pub fn is_aca(c: &Certificate) -> bool {
    matches!(ssp_of(c), Some(SecurityMgmtSsp::aca(_)))
}

/// Whether `c` carries the RA SSP under PSID 35.
#[must_use]
pub fn is_ra(c: &Certificate) -> bool {
    matches!(ssp_of(c), Some(SecurityMgmtSsp::ra(_)))
}

/// What [`aca_issue_request`] issued, for any certificate type.
#[derive(Debug, Clone)]
pub struct AcaIssue {
    /// `AcaEeCertResponsePrivateSpdu`.
    pub private: Pdu,
    /// `AcaRaCertResponseSpdu`, carrying `private` and the request's HashedId8.
    pub to_ra: Pdu,
    /// The implicit certificate (the ACA's own record of what it issued).
    pub certificate: Certificate,
    /// The linkage value inserted (a pseudonym certificate only).
    pub lv: Option<LinkageValue>,
    /// `iCert` (a pseudonym certificate only).
    pub i: Option<u16>,
    /// The HashedId8 of the `RaAcaCertRequestSpdu` (`AcaRaCertResponse.requestHash`).
    pub request_hash: [u8; 8],
}

/// THE ACA: opens one `RaAcaCertRequestSpdu` of any kind and issues.
///
/// Checks, in the order a receiver applies them: the SignedCertRequest verifies under one
/// certificate that chains to the PKI and carries the RA SSP; version 2, `type implicit`,
/// `flags` clear (the US profile's original butterfly; `cubk` is refused as UNCONFIRMED,
/// 02 §9.4), `certEncKey` present (the private response), no issue or request
/// permissions, `appPermissions` present, and `tbsCert.verifyKeyIndicator` a
/// `reconstructionValue` (1609.2.1-2022 module comment).
///
/// With `linkageInfo` (a pseudonym): `tbsCert.id` is `linkageData`, `cracaId 000000`,
/// `crlSeries 0`; both PLVs decrypt under `keys` and come from two LAs; `lv = plv1 ⊕ plv2`
/// is inserted with `crlSeries 1`. Without it (an RSU application or OBU identification
/// certificate, WP-B3b): `tbsCert.id` is not `linkageData`, and `crlSeries` is 3, revoked
/// by hash. Either way `cracaId` = [`ScmsPki::craca_id`] (the ICA, the CRACA of the US CRL
/// signer); ECQV-1609 issuance with the reconstruction value as `R_U` under this ACA;
/// `AcaEeCertResponse {certificate, privateKeyInfo = r}` encrypted to `certEncKey` and
/// signed by this ACA (the same ACA that issued it), inside an ACA-signed
/// `AcaRaCertResponse` whose `requestHash` is the HashedId8 of the request as received.
///
/// # Errors
/// Any check above fails, as the [`PduError`] the run records; [`PduError::NotForMe`]
/// for a PLV sealed under a key this ACA does not share (or no keys at all).
pub fn aca_issue_request(
    pki: &ScmsPki,
    keys: Option<&LaAcaKeys>,
    request: &[u8],
    now: u32,
    g: &mut Drbg,
) -> Result<AcaIssue> {
    let (p, signer) = pdu::open_signed_cert_request(request, pki)?;
    let RequestSignerFound::Cert(signer) = signer else {
        return Err(PduError::Signer("RaAcaCertRequestSpdu is SignerSingleCert"));
    };
    if !is_ra(&signer) {
        return Err(PduError::Signer("RaAcaCertRequestSpdu is signed by an RA"));
    }
    let ScmsPduContent::aca_ra(AcaRaInterfacePdu::raAcaCertRequest(r)) = p.content else {
        return Err(PduError::Shape("expected aca-ra.raAcaCertRequest"));
    };
    if r.version.0 != STRUCT_VERSION {
        return Err(PduError::Version("RaAcaCertRequest.version"));
    }
    if r.r_type != CertificateType::implicit {
        return Err(PduError::Shape("the US certificates are implicit"));
    }
    // flags BIT STRING {butterflyExplicit(0), cubk(1)} (SIZE(8)).
    if r.flags.0.any() {
        return Err(PduError::Shape(
            "flags: butterflyExplicit and cubk are clear in the US profile (cubk UNCONFIRMED)",
        ));
    }
    let enc_key = match &r.cert_enc_key {
        Some(k) => {
            if k.supported_symm_alg
                != v2xw_msg::sec_types::ieee1609_dot2_base_types::SymmAlgorithm::aes128Ccm
            {
                return Err(PduError::Shape("certEncKey supportedSymmAlg aes128Ccm"));
            }
            match &k.public_key {
                v2xw_msg::sec_types::ieee1609_dot2_base_types::BasePublicEncryptionKey::eciesNistP256(
                    pt,
                ) => pdu::point_of(pt)?,
                _ => return Err(PduError::Key("certEncKey is eciesNistP256")),
            }
        }
        None => {
            return Err(PduError::Shape(
                "certEncKey absent: the US response is the private one",
            ));
        }
    };
    let tbs = &r.tbs_cert;
    if tbs.cert_issue_permissions.is_some() || tbs.cert_request_permissions.is_some() {
        return Err(PduError::Shape(
            "an end-entity certificate carries no issue or request permissions",
        ));
    }
    if tbs.app_permissions.is_none() {
        return Err(PduError::Shape("appPermissions present"));
    }
    let VerificationKeyIndicator::reconstructionValue(rv) = &tbs.verify_key_indicator else {
        return Err(PduError::Shape(
            "an implicit request carries a reconstructionValue",
        ));
    };
    let request_public = pdu::point_of(rv)?;
    let mut t = tbs.clone();
    let (lv, i) = match &r.linkage_info {
        Some(info) => {
            let CertificateId::linkageData(ld) = &tbs.id else {
                return Err(PduError::Shape("tbsCert.id is linkageData"));
            };
            if tbs.craca_id.0[..] != [0u8; 3] || tbs.crl_series.0.0 != 0 {
                return Err(PduError::Shape("tbsCert cracaId 000000 and crlSeries 0"));
            }
            let keys = keys.ok_or(PduError::NotForMe)?;
            let (la1, plv1) = aca_open_plv(&info.enc_plv1, keys, now)?;
            let (la2, plv2) = aca_open_plv(&info.enc_plv2, keys, now)?;
            if la1 == la2 {
                return Err(PduError::Shape("the two PLVs come from two different LAs"));
            }
            let lv = linkage::linkage_value(plv1, plv2);
            let i = ld.i_cert.0.0;
            t.id =
                CertificateId::linkageData(v2xw_msg::sec_types::ieee1609_dot2::LinkageData::new(
                    IValue(Uint16(i)),
                    lv.to_asn1(),
                    None,
                ));
            t.crl_series = CrlSeries(Uint16(PSEUDONYM_CRL_SERIES));
            (Some(lv), Some(i))
        }
        None => {
            if matches!(tbs.id, CertificateId::linkageData(_)) {
                return Err(PduError::Shape("no LinkageData without linkageInfo"));
            }
            t.crl_series = CrlSeries(Uint16(v2xw_sec::crl::SERIES_APPLICATION_IDENTIFICATION));
            (None, None)
        }
    };
    t.craca_id = HashedId3(FixedOctetString::from(pki.craca_id()));
    let d_aca = pki
        .aca
        .private
        .as_ref()
        .ok_or(PduError::Key("ACA scalar"))?;
    let issued = cert::issue_implicit(
        &t,
        &pki.aca.cert,
        &Point::mul_base(d_aca),
        d_aca,
        &request_public,
        || g.scalar(),
    )
    .map_err(|_| PduError::Key("ECQV issuance"))?;
    let resp = pdu::pdu(ScmsPduContent::aca_ee(
        AcaEeInterfacePdu::acaEeCertResponse(AcaEeCertResponse::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(now)),
            issued.certificate.clone(),
            Some(OctetString::from(
                ec::scalar_to_be32(&issued.private_key_contribution).to_vec(),
            )),
        )),
    ));
    let private = Pdu::new(pdu::encrypted_signed(
        &resp,
        &enc_key,
        &pki.aca.cert,
        &pki.aca.key,
        g,
    )?);
    let request_hash = hashedid::id8_bytes(&hashedid::hashed_id8(request));
    let to_ra = pdu::pdu(ScmsPduContent::aca_ra(
        AcaRaInterfacePdu::acaRaCertResponse(AcaRaCertResponse::new(
            Uint8(STRUCT_VERSION),
            Time32(Uint32(now)),
            pdu::id8(request_hash),
            AcaResponse::private(pdu::decode(private.bytes())?),
        )),
    ));
    let to_ra = Pdu::new(pdu::signed(&to_ra, &pki.aca.cert, &pki.aca.key)?);
    Ok(AcaIssue {
        private,
        to_ra,
        certificate: issued.certificate,
        lv,
        i,
        request_hash,
    })
}

/// The ACA for a pseudonym request ([`aca_issue_request`] with the LA keys): a request
/// without `linkageInfo` is refused here.
///
/// # Errors
/// As [`aca_issue_request`], and `linkageInfo` absent.
pub fn aca_issue(
    pki: &ScmsPki,
    keys: &LaAcaKeys,
    request: &[u8],
    now: u32,
    g: &mut Drbg,
) -> Result<AcaIssued> {
    let x = aca_issue_request(pki, Some(keys), request, now, g)?;
    let (Some(lv), Some(i)) = (x.lv, x.i) else {
        return Err(PduError::Shape("linkageInfo absent"));
    };
    Ok(AcaIssued {
        private: x.private,
        to_ra: x.to_ra,
        certificate: x.certificate,
        lv,
        i,
        request_hash: x.request_hash,
    })
}

/// RA side: opens an `AcaRaCertResponseSpdu` (ACA-signed, chains to the PKI) and returns
/// `(requestHash, the AcaEeCertResponsePrivateSpdu bytes)`. The RA cannot read the inner
/// response; it only files it.
///
/// # Errors
/// The signature, the signer's SSP or the shape is wrong.
pub fn ra_open_aca_response(pki: &ScmsPki, bytes: &[u8]) -> Result<([u8; 8], Pdu)> {
    let (p, signer) = pdu::open_signed_pdu(bytes, pki)?;
    if !is_aca(&signer) {
        return Err(PduError::Signer(
            "AcaRaCertResponseSpdu is signed by an ACA",
        ));
    }
    let ScmsPduContent::aca_ra(AcaRaInterfacePdu::acaRaCertResponse(r)) = p.content else {
        return Err(PduError::Shape("expected aca-ra.acaRaCertResponse"));
    };
    let AcaResponse::private(inner) = r.aca_response else {
        return Err(PduError::Shape(
            "acaResponse is private (original butterfly)",
        ));
    };
    Ok((
        hashedid::id8_bytes(&r.request_hash),
        Pdu::new(pdu::coer(&inner)?),
    ))
}

// ---------------------------------------------------------------------------------------
// EE: open the response and derive the key
// ---------------------------------------------------------------------------------------

/// A pseudonym credential as the device derived it from its download.
#[derive(Clone)]
pub struct OpenedPseudonym {
    /// The implicit certificate.
    pub certificate: Certificate,
    /// The issuing ACA's certificate (the outer signer).
    pub aca: Certificate,
    /// `iCert`.
    pub i: u16,
    /// The linkage value in its `linkageData`.
    pub lv: LinkageValue,
    /// `e`, the ECQV-1609 certificate hash.
    pub e: Scalar,
    /// `r`, the `privateKeyInfo`.
    pub r: Scalar,
    /// The signing private key `d = e·b_ij + r`.
    pub private: Scalar,
    /// `Q_U = e·P_U + Q_ACA`, the key a verifier reconstructs.
    pub public: Point,
}

impl core::fmt::Debug for OpenedPseudonym {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "OpenedPseudonym(i={}, lv={:?})",
            self.i,
            self.lv.as_bytes()
        )
    }
}

/// The device: opens one `AcaEeCertResponsePrivateSpdu` filed as `i_j`.
///
/// The outer signature verifies under a certificate that chains to the PKI and carries the
/// ACA SSP; the payload decrypts with `h_ij = p + f₂(ek, (i, j))`; the inner PDU is
/// `aca-ee.acaEeCertResponse` version 2 with an implicit certificate whose issuer is
/// **the outer signer** (the 1609.2.1 same-ACA rule), whose `linkageData.iCert` is `i`, and
/// whose `privateKeyInfo` is present; and `d = e·b_ij + r` satisfies
/// `d·G = e·P_U + Q_ACA`. Anything else is refused.
///
/// # Errors
/// The first check that fails.
pub fn ee_open_aca_response(
    pki: &ScmsPki,
    bytes: &[u8],
    cat: &Caterpillar,
    i: u32,
    j: u32,
) -> Result<OpenedPseudonym> {
    let o = pdu::open_signed(bytes, pki)?;
    if !is_aca(&o.signer) {
        return Err(PduError::Signer(
            "AcaEeCertResponsePrivateSpdu is signed by an ACA",
        ));
    }
    let h = cat.encryption_private(i, j);
    let inner = pdu::decrypt_key(&o.payload, &h)?;
    let Ieee1609Dot2Content::unsecuredData(op) = pdu::outer(&inner)?.content else {
        return Err(PduError::Shape(
            "the encrypted response holds unsecuredData",
        ));
    };
    let p = pdu::scms_pdu(&op.0)?;
    let ScmsPduContent::aca_ee(AcaEeInterfacePdu::acaEeCertResponse(r)) = p.content else {
        return Err(PduError::Shape("expected aca-ee.acaEeCertResponse"));
    };
    if r.version.0 != STRUCT_VERSION {
        return Err(PduError::Version("AcaEeCertResponse.version"));
    }
    let c = r.certificate;
    // Same-ACA rule: the certificate inside was issued by the ACA that signed outside.
    let signer_id8 = pdu::cert_id8(&o.signer);
    match &c.0.issuer {
        IssuerIdentifier::sha256AndDigest(d) if hashedid::id8_bytes(d) == signer_id8 => {}
        _ => {
            return Err(PduError::Signer(
                "same-ACA rule: the outer signer did not issue the certificate",
            ));
        }
    }
    if c.0.r_type != CertificateType::implicit {
        return Err(PduError::Shape("the pseudonym certificate is implicit"));
    }
    let CertificateId::linkageData(ld) = &c.0.to_be_signed.id else {
        return Err(PduError::Shape("the pseudonym certificate has linkageData"));
    };
    if u32::from(ld.i_cert.0.0) != i {
        return Err(PduError::Shape("iCert is the i of the file name"));
    }
    let VerificationKeyIndicator::reconstructionValue(pu) = &c.0.to_be_signed.verify_key_indicator
    else {
        return Err(PduError::Shape(
            "implicit certificate without reconstructionValue",
        ));
    };
    let p_u = pdu::point_of(pu)?;
    let rb: [u8; 32] = r
        .private_key_info
        .as_ref()
        .ok_or(PduError::Shape("privateKeyInfo present"))?[..]
        .try_into()
        .map_err(|_| PduError::Shape("privateKeyInfo is 32 bytes"))?;
    let rr = ec::scalar_from_be32(&rb).map_err(|_| PduError::Key("privateKeyInfo"))?;
    let e = cert::implicit_e(&c, &o.signer).map_err(|_| PduError::Shape("ECQV e"))?;
    let (_, q_sec1) = v2xw_sec::sig1609::explicit_key(&o.signer).ok_or(PduError::Key("ACA key"))?;
    let q_aca = Point::from_sec1(&q_sec1).map_err(|_| PduError::Key("ACA key"))?;
    let d = cat.signing_private_implicit(i, j, &e, &rr);
    let q_u = p_u.mul(&e).add(&q_aca);
    if Point::mul_base(&d).compressed() != q_u.compressed() || q_u.is_identity() {
        return Err(PduError::Key("d·G ≠ e·P_U + Q_ACA"));
    }
    let lv = LinkageValue::from_asn1(&ld.linkage_value);
    Ok(OpenedPseudonym {
        certificate: c.clone(),
        aca: o.signer,
        i: ld.i_cert.0.0,
        lv,
        e,
        r: rr,
        private: d,
        public: q_u,
    })
}

/// The device's `EeRaDownloadRequestSpdu` for `filename`: `eeRaDownloadRequest
/// {generationTime, filename}` signed by the enrolment certificate, encrypted to the RA
/// (03 §3.2; the CAMP `Download-Req` header carries the same, [CAMP122] §6.3).
///
/// # Errors
/// Encoding fails or the RA has no encryption key.
pub fn ee_download_request_spdu(
    pki: &ScmsPki,
    enrolment: &Certificate,
    key: &v2xw_sec::sig1609::Signer1609,
    t32: u32,
    filename: &str,
    g: &mut Drbg,
) -> Result<Pdu> {
    let ra_key = pki
        .ra
        .enc_public()
        .ok_or(PduError::Key("RA encryption key"))?;
    let p = pdu::ee_ra_download_request(t32, filename);
    Ok(Pdu::new(pdu::signed_encrypted(
        &p,
        enrolment,
        key,
        &pki.ra.cert,
        &ra_key,
        g,
    )?))
}

/// What the RA reads from a download request.
#[derive(Debug, Clone)]
pub struct OpenedDownload {
    /// The requested file name.
    pub filename: String,
    /// `generationTime` (`Time32`).
    pub generation_time: u32,
    /// The enrolment certificate that signed it.
    pub enrolment: Certificate,
}

/// The RA: decrypts a download request with its encryption key, verifies it under an
/// ECA-issued enrolment certificate, and checks `generationTime` against `now` within
/// `tolerance_s` (the client-signature TTL, 5 s, [CAMP122] SCMS-1203).
///
/// # Errors
/// Decryption, signature, issuer, shape or freshness.
pub fn ra_open_download_request(
    pki: &ScmsPki,
    bytes: &[u8],
    now: u32,
    tolerance_s: u32,
) -> Result<OpenedDownload> {
    let plain = pki.decrypt_as(&pki.ra, bytes)?;
    let (p, signer) = pdu::open_signed_pdu(&plain, pki)?;
    let issued_by_eca = matches!(&signer.0.issuer,
        IssuerIdentifier::sha256AndDigest(h) if pki.is_role(&pki.eca, &hashedid::id8_bytes(h)));
    if !issued_by_eca {
        return Err(PduError::UntrustedSigner);
    }
    let ScmsPduContent::ee_ra(EeRaInterfacePdu::eeRaDownloadRequest(r)) = p.content else {
        return Err(PduError::Shape("expected ee-ra.eeRaDownloadRequest"));
    };
    let t = r.generation_time.0.0;
    if t.abs_diff(now) > tolerance_s {
        return Err(PduError::Shape(
            "download request outside the client-signature tolerance",
        ));
    }
    Ok(OpenedDownload {
        filename: r.filename.to_string(),
        generation_time: t,
        enrolment: signer,
    })
}

/// The `.info` file of a provisioning request: `<16 upper-case hex of requestHash>.info`
/// ([CAMP122] §6.3, `/download/info`, filename `[0-9A-F]{16}.info`; 03 §4.2).
#[must_use]
pub fn info_filename(request_hash8: [u8; 8]) -> String {
    let mut s = String::with_capacity(21);
    for b in request_hash8 {
        s.push_str(&format!("{b:02X}"));
    }
    s.push_str(".info");
    s
}

/// What a `Download-Req` file name asks for, under the CAMP grammar ([CAMP122] §6.3,
/// Table 79): `[0-9A-F]{16}_[0-9A-F]{1,8}.zip` is the batch of one i-period,
/// `[0-9A-F]{16}.info` the next-download time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DownloadFile {
    /// `<hash>_<i>.zip`.
    Batch {
        /// The low 8 bytes of the request hash.
        request_hash8: [u8; 8],
        /// The i-period.
        i: u32,
    },
    /// `<hash>.info`.
    Info {
        /// The low 8 bytes of the request hash.
        request_hash8: [u8; 8],
    },
}

fn upper_hex(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
}

fn hash8_of(s: &str) -> Option<[u8; 8]> {
    if s.len() != 16 || !upper_hex(s) {
        return None;
    }
    let mut h = [0u8; 8];
    for (k, b) in h.iter_mut().enumerate() {
        *b = u8::from_str_radix(&s[2 * k..2 * k + 2], 16).ok()?;
    }
    Some(h)
}

/// Parses a `Download-Req` file name; `None` for anything outside the grammar (lower-case
/// hex, a 17-digit hash, a 9-digit i, another extension), which the RA refuses.
#[must_use]
pub fn parse_download_filename(name: &str) -> Option<DownloadFile> {
    if let Some(stem) = name.strip_suffix(".info") {
        return hash8_of(stem).map(|request_hash8| DownloadFile::Info { request_hash8 });
    }
    let stem = name.strip_suffix(".zip")?;
    let (h, i) = stem.split_once('_')?;
    if i.len() > 8 || !upper_hex(i) {
        return None;
    }
    Some(DownloadFile::Batch {
        request_hash8: hash8_of(h)?,
        i: u32::from_str_radix(i, 16).ok()?,
    })
}

/// The body of a `.info` download: a single COER `Time32`, the time of the next batch
/// ([CAMP122] §6.3, Table 79 "single Time32 (next batch time)"). `Time32` is `Uint32`, a
/// fixed-size 0..2^32−1 integer, so its OER is exactly four octets, big-endian
/// (ITU-T X.696 §10.3.4).
///
/// # Errors
/// Encoding fails.
pub fn info_body(next_dl_time: u32) -> Result<Vec<u8>> {
    rasn::coer::encode(&Time32(Uint32(next_dl_time))).map_err(|e| PduError::Coer(e.to_string()))
}

/// Reads a `.info` body back.
///
/// # Errors
/// Not one COER `Time32`.
pub fn open_info_body(bytes: &[u8]) -> Result<u32> {
    let t: Time32 = rasn::coer::decode(bytes).map_err(|e| PduError::Coer(e.to_string()))?;
    if bytes.len() != 4 {
        return Err(PduError::Shape(".info is one Time32"));
    }
    Ok(t.0.0)
}

/// The name of file `i_j` inside a batch zip: `X_Y`, X = i and Y = j in upper-case hex
/// (03 §4.2, [CAMP122] §6.3).
#[must_use]
pub fn batch_entry_name(i: u32, j: u32) -> String {
    format!("{i:X}_{j:X}")
}

/// The size of a stored (uncompressed: the entries are ciphertext) zip archive holding
/// `entries` as `(name, length)`: per entry a 30-byte local file header and a 46-byte
/// central-directory header, each followed by the name, plus the 22-byte end-of-central-
/// directory record (PKWARE APPNOTE 6.3.x §4.3.7, §4.3.12, §4.3.16).
#[must_use]
pub fn stored_zip_len(entries: &[(String, u32)]) -> u32 {
    let per: u32 = entries
        .iter()
        .map(|(n, l)| 30 + 46 + 2 * u32::try_from(n.len()).unwrap_or(u32::MAX) + l)
        .sum();
    per + 22
}

#[cfg(test)]
#[path = "pdu_prov_tests.rs"]
mod tests;
