//! IEEE 1609.2 certificate revocation lists as the US SCMS issues them (02 §11; 03 §3.4,
//! §10; gap rows 29, 36, 37).
//!
//! # Wire form (1609.2-2022 `Ieee1609Dot2Crl`, `Ieee1609Dot2CrlBaseTypes`)
//!
//! ```text
//! SecuredCrl = Ieee1609Dot2Data{ signedData{ hashId sha256,
//!     tbsData{ payload.data = unsecuredData( COER CrlContents ),
//!              headerInfo{ psid 256, every other field ABSENT } },
//!     signer certificate (the CRL signer), signature } }
//! CrlContents = { version 1, crlSeries, crlCraca HashedId8, issueDate, nextCrl,
//!                 priorityInfo{priority OPT}, typeSpecific CHOICE {
//!     fullHashCrl | deltaHashCrl     ToBeSignedHashIdCrl{ crlSerial, entries{id HashedId10, expiry} },
//!     fullLinkedCrl | deltaLinkedCrl ToBeSignedLinkageValueCrl{ iRev, indexWithinI,
//!         individual SEQUENCE OF JMaxGroup{ jmax, LAGroup{ la1Id, la2Id,
//!             IMaxGroup{ iMax, IndividualRevocation{ linkageSeed1, linkageSeed2 } } } },
//!         ..., iPeriodInfo (1609.2-2025 only) } } }
//! ```
//!
//! # US series (SCMS Manager Profile Table 1; 03 §10)
//!
//! | Series | Revokes | By |
//! |---|---|---|
//! | 1 | OBU pseudonym certificates | linkage seeds |
//! | 2 | ICA, ACA, ECA, RA, LA | certificate hash |
//! | 3 | RSU application and OBU identification certificates | certificate hash |
//! | 4 | enrolment certificates ("may not need to be used") | certificate hash |
//! | 256 | root-signed: CRL signer, PG, MA | certificate hash |
//!
//! # The signer rule (`Ieee1609Dot2CrlSsp`)
//!
//! The signer's certificate holds PSID 256 with `CrlSsp{version 1, associatedCraca,
//! crls}`. `crlSeries` must be one of `crls`. With `isCraca` the CRL's `crlCraca` must be
//! the `HashedId8` of the signer's own certificate; with `issuerIsCraca` it must be that
//! of the certificate that issued the signer. [`verify_crl`] applies both, and the
//! signature rule of [`crate::sig1609`].
//!
//! # The device rule ([`revokes`])
//!
//! A certificate is matched only when its `cracaId` is the low 3 bytes of `crlCraca`
//! and its `crlSeries` is the CRL's. A hash CRL matches its `HashedId10`. A linkage CRL
//! advances each entry's seeds `iCert − iRev` steps, stops at `iMax`, and compares the
//! certificate's linkage value with `lv(iCert, j)` for `j = 0 … jmax−1` (02 §11.2).

use std::collections::BTreeMap;

use v2xw_msg::scms_types::{WireEdition, check_crl_contents};
use v2xw_msg::sec_types::ieee1609_dot2::{Certificate, CertificateId, IssuerIdentifier};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    CrlSeries, HashedId8, HashedId10, IValue, LaId as AsnLaId, Time32, Uint8, Uint16, Uint32,
};
use v2xw_msg::sec_types::ieee1609_dot2_crl::SecuredCrl;
use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::{
    CrlContents, CrlPriorityInfo, HashBasedRevocationInfo, IMaxGroup, IPeriodInfo,
    IndividualRevocation, JMaxGroup, LAGroup, SequenceOfHashBasedRevocationInfo,
    SequenceOfIMaxGroup, SequenceOfIPeriodInfo, SequenceOfIndividualRevocation,
    SequenceOfJMaxGroup, SequenceOfLAGroup, ToBeSignedHashIdCrl, ToBeSignedLinkageValueCrl,
    TypeSpecificCrlContents,
};

use crate::linkage::{CrlLinkageEntry, LaId, LinkageSeed, LinkageValue};
use crate::sig1609::{
    self, OpenedSigned, SigError, Signer1609, SpduSigner, cert_hashed_id8, cert_hashed_id10, coer,
};

/// `CrlPsid` (1609.2 `Ieee1609Dot2Crl`).
pub const CRL_PSID: u64 = 256;
/// Series 1: OBU pseudonym certificates, revoked by linkage seeds.
pub const SERIES_PSEUDONYM: u16 = 1;
/// Series 2: ICA, ACA, ECA, RA and LA certificates, by hash.
pub const SERIES_SCMS_COMPONENTS: u16 = 2;
/// Series 3: RSU application and OBU identification certificates, by hash.
pub const SERIES_APPLICATION_IDENTIFICATION: u16 = 3;
/// Series 4: enrolment certificates, by hash.
pub const SERIES_ENROLMENT: u16 = 4;
/// Series 256: root-signed CRL signer, PG and MA certificates, by hash.
pub const SERIES_ROOT_SIGNED: u16 = 256;

pub use ssp::{CracaType, CrlSsp};

/// The hand-written `Ieee1609Dot2CrlSsp` types (the derive needs the rasn prelude in scope).
mod ssp {
    use rasn::prelude::*;
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::{CrlSeries, Uint8, Uint16};

    /// `CracaType` (1609.2 `Ieee1609Dot2CrlSsp`).
    #[derive(AsnType, Encode, Decode, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    #[rasn(enumerated)]
    #[allow(non_camel_case_types)]
    pub enum CracaType {
        /// The signer's own certificate is the CRACA.
        isCraca = 0,
        /// The signer's issuer is the CRACA.
        issuerIsCraca = 1,
    }

    /// `CrlSsp` (1609.2 `Ieee1609Dot2CrlSsp`, not in the generated bindings because no
    /// 1609.2 module imports it): `SEQUENCE { version Uint8(1), associatedCraca CracaType,
    /// crls PermissibleCrls, ... }`. Its encoding is cross-checked against asn1tools by
    /// `tools/scms-artefacts/xcheck_crl.py`; the published test roots carry
    /// `00 01 00 01 01 01 00` (isCraca, series 256).
    #[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq, Hash)]
    #[rasn(automatic_tags)]
    #[non_exhaustive]
    pub struct CrlSsp {
        /// `version`, 1.
        pub version: Uint8,
        /// `associatedCraca`.
        #[rasn(identifier = "associatedCraca")]
        pub associated_craca: CracaType,
        /// `crls`: the series this signer may issue.
        pub crls: Vec<CrlSeries>,
    }

    impl CrlSsp {
        /// A version-1 SSP.
        pub fn new(associated_craca: CracaType, crls: &[u16]) -> CrlSsp {
            CrlSsp {
                version: Uint8(1),
                associated_craca,
                crls: crls.iter().map(|s| CrlSeries(Uint16(*s))).collect(),
            }
        }

        /// The COER bytes, as they go in a certificate's opaque SSP.
        pub fn to_coer(&self) -> Vec<u8> {
            rasn::coer::encode(self).expect("CrlSsp encodes")
        }
    }
}

/// Why a CRL could not be built, read or accepted.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CrlError {
    /// COER decoding failed.
    #[error("decode {layer}: {detail}")]
    Decode {
        /// Which structure.
        layer: &'static str,
        /// The decoder's message.
        detail: String,
    },
    /// The contents break a 1609.2 or US-profile rule.
    #[error("invalid CRL: {0}")]
    Invalid(String),
    /// Signing or hashing failed.
    #[error(transparent)]
    Sig(#[from] SigError),
}

/// One revoked pseudonym device: what the two LAs released for it (03 §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkageRevocation {
    /// LA1's identifier.
    pub la1: LaId,
    /// LA2's identifier.
    pub la2: LaId,
    /// LA1's seed at `iRev`.
    pub ls1: LinkageSeed,
    /// LA2's seed at `iRev`.
    pub ls2: LinkageSeed,
    /// Certificates per i-period (a `Uint8`: at most 255, 03 §3.4).
    pub jmax: u8,
    /// The last i-period worth tracking (the device's last certificate).
    pub i_max: u16,
}

/// One revoked certificate in a hash CRL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashRevocation {
    /// `HashedId10` of the revoked certificate.
    pub id: [u8; 10],
    /// Its expiry (Time32): the entry may be dropped after it.
    pub expiry: u32,
}

/// The type-specific part of a CRL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrlBody {
    /// `fullHashCrl` / `deltaHashCrl`.
    Hash {
        /// `crlSerial`.
        serial: u32,
        /// The entries.
        entries: Vec<HashRevocation>,
    },
    /// `fullLinkedCrl` / `deltaLinkedCrl`.
    Linkage {
        /// `iRev`: the i-period the seeds are for.
        i_rev: u16,
        /// `indexWithinI`: 0 for the first CRL at this `iRev`, +1 per CRL after.
        index_within_i: u8,
        /// The individually revoked devices.
        individual: Vec<LinkageRevocation>,
    },
}

/// Everything a CRL generator decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrlSpec {
    /// `crlSeries`.
    pub series: u16,
    /// `crlCraca`: `HashedId8` of the CRACA certificate.
    pub craca: [u8; 8],
    /// `issueDate` (Time32).
    pub issue_date: u32,
    /// `nextCrl` (Time32), after `issue_date`.
    pub next_crl: u32,
    /// `priorityInfo.priority`.
    pub priority: Option<u8>,
    /// A delta CRL (changes since the last full one) rather than a full one.
    pub delta: bool,
    /// The entries.
    pub body: CrlBody,
}

fn la(id: LaId) -> AsnLaId {
    AsnLaId(rasn::types::FixedOctetString::from(id.0.to_be_bytes()))
}

/// Builds `CrlContents`. Linkage entries are grouped by `jmax`, then `(la1Id, la2Id)`,
/// then `iMax`, each level sorted, so the bytes are a function of the entry set.
pub fn crl_contents(spec: &CrlSpec) -> CrlContents {
    let type_specific = match &spec.body {
        CrlBody::Hash { serial, entries } => {
            let t = ToBeSignedHashIdCrl::new(
                Uint32(*serial),
                SequenceOfHashBasedRevocationInfo(
                    entries
                        .iter()
                        .map(|e| {
                            HashBasedRevocationInfo::new(
                                HashedId10(rasn::types::FixedOctetString::from(e.id)),
                                Time32(Uint32(e.expiry)),
                            )
                        })
                        .collect(),
                ),
            );
            if spec.delta {
                TypeSpecificCrlContents::deltaHashCrl(t)
            } else {
                TypeSpecificCrlContents::fullHashCrl(t)
            }
        }
        CrlBody::Linkage {
            i_rev,
            index_within_i,
            individual,
        } => {
            type Groups =
                BTreeMap<u8, BTreeMap<(u16, u16), BTreeMap<u16, Vec<([u8; 16], [u8; 16])>>>>;
            let mut g: Groups = BTreeMap::new();
            for r in individual {
                g.entry(r.jmax)
                    .or_default()
                    .entry((r.la1.0, r.la2.0))
                    .or_default()
                    .entry(r.i_max)
                    .or_default()
                    .push((*r.ls1.as_bytes(), *r.ls2.as_bytes()));
            }
            let jgroups: Vec<JMaxGroup> = g
                .into_iter()
                .map(|(jmax, las)| {
                    JMaxGroup::new(
                        Uint8(jmax),
                        SequenceOfLAGroup(
                            las.into_iter()
                                .map(|((a, b), imax)| {
                                    LAGroup::new(
                                        la(LaId(a)),
                                        la(LaId(b)),
                                        SequenceOfIMaxGroup(
                                            imax.into_iter()
                                                .map(|(i_max, mut seeds)| {
                                                    seeds.sort();
                                                    IMaxGroup::new(
                                                        Uint16(i_max),
                                                        SequenceOfIndividualRevocation(
                                                            seeds
                                                                .into_iter()
                                                                .map(|(s1, s2)| {
                                                                    IndividualRevocation::new(
                                                                        LinkageSeed::new(s1)
                                                                            .to_asn1(),
                                                                        LinkageSeed::new(s2)
                                                                            .to_asn1(),
                                                                    )
                                                                })
                                                                .collect(),
                                                        ),
                                                        None,
                                                    )
                                                })
                                                .collect(),
                                        ),
                                    )
                                })
                                .collect(),
                        ),
                    )
                })
                .collect();
            let t = ToBeSignedLinkageValueCrl::new(
                IValue(Uint16(*i_rev)),
                Uint8(*index_within_i),
                Some(SequenceOfJMaxGroup(jgroups)),
                None,
                None,
                None,
            );
            if spec.delta {
                TypeSpecificCrlContents::deltaLinkedCrl(t)
            } else {
                TypeSpecificCrlContents::fullLinkedCrl(t)
            }
        }
    };
    CrlContents::new(
        Uint8(1),
        CrlSeries(Uint16(spec.series)),
        HashedId8(rasn::types::FixedOctetString::from(spec.craca)),
        Time32(Uint32(spec.issue_date)),
        Time32(Uint32(spec.next_crl)),
        CrlPriorityInfo::new(spec.priority.map(Uint8)),
        type_specific,
    )
}

/// Adds the 1609.2-2025 `iPeriodInfo` to a linkage CRL (an error in the 2022 edition,
/// the default wire edition, and for a hash CRL).
pub fn with_i_period_info(
    mut c: CrlContents,
    info: Vec<IPeriodInfo>,
    edition: WireEdition,
) -> Result<CrlContents, CrlError> {
    match &mut c.type_specific {
        TypeSpecificCrlContents::fullLinkedCrl(l) | TypeSpecificCrlContents::deltaLinkedCrl(l) => {
            l.i_period_info = Some(SequenceOfIPeriodInfo(info));
        }
        _ => return Err(CrlError::Invalid("iPeriodInfo on a hash CRL".into())),
    }
    check_crl(&c, edition)?;
    Ok(c)
}

/// The 1609.2 and US-profile rules on `CrlContents` alone: version 1, `nextCrl` after
/// `issueDate`, series 1 is a linkage CRL and every other US series a hash CRL, `jmax`
/// non-zero, `iMax ≥ iRev`, and the edition's `iPeriodInfo` rule.
pub fn check_crl(c: &CrlContents, edition: WireEdition) -> Result<(), CrlError> {
    let bad = |s: &str| Err(CrlError::Invalid(s.into()));
    if c.version.0 != 1 {
        return bad("version is not 1");
    }
    if c.next_crl.0.0 <= c.issue_date.0.0 {
        return bad("nextCrl is not after issueDate");
    }
    let series = c.crl_series.0.0;
    let linked = match &c.type_specific {
        TypeSpecificCrlContents::fullLinkedCrl(l) | TypeSpecificCrlContents::deltaLinkedCrl(l) => {
            Some(l)
        }
        TypeSpecificCrlContents::fullHashCrl(_) | TypeSpecificCrlContents::deltaHashCrl(_) => None,
        _ => return bad("the WithAlg linkage CRL forms are not in the US profile"),
    };
    match (series, linked.is_some()) {
        (SERIES_PSEUDONYM, false) => return bad("series 1 must be a linkage CRL"),
        (
            SERIES_SCMS_COMPONENTS
            | SERIES_APPLICATION_IDENTIFICATION
            | SERIES_ENROLMENT
            | SERIES_ROOT_SIGNED,
            true,
        ) => return bad("series 2/3/4/256 are hash CRLs"),
        _ => {}
    }
    if let Some(l) = linked {
        for jg in l.individual.iter().flat_map(|s| s.0.iter()) {
            if jg.jmax.0 == 0 {
                return bad("jmax is 0");
            }
            for ig in jg.contents.0.iter().flat_map(|g| g.contents.0.iter()) {
                if ig.i_max.0 < l.i_rev.0.0 {
                    return bad("iMax is before iRev");
                }
            }
        }
    }
    check_crl_contents(c, edition).map_err(|e| CrlError::Invalid(e.to_string()))
}

/// Signs `contents` as a `SecuredCrl`: psid 256, every other header field absent, the
/// signer's certificate attached.
pub fn sign_crl(
    contents: &CrlContents,
    signer_cert: &Certificate,
    key: &Signer1609,
) -> Result<SecuredCrl, CrlError> {
    let d = sig1609::sign_spdu(
        sig1609::header_psid_only(CRL_PSID),
        coer(contents)?,
        SpduSigner::Certificate(signer_cert.clone()),
        key,
    )?;
    Ok(SecuredCrl(d))
}

/// A `SecuredCrl` taken apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedCrl {
    /// The signed envelope.
    pub signed: OpenedSigned,
    /// The contents, decoded from the signed payload.
    pub contents: CrlContents,
}

/// Opens a `SecuredCrl`.
pub fn open_crl(crl: &SecuredCrl) -> Result<OpenedCrl, CrlError> {
    let signed = sig1609::open_signed(&crl.0)
        .ok_or_else(|| CrlError::Invalid("not a signed SPDU with an unsecured payload".into()))?;
    let contents = rasn::coer::decode(&signed.payload).map_err(|e| CrlError::Decode {
        layer: "CrlContents",
        detail: e.to_string(),
    })?;
    Ok(OpenedCrl { signed, contents })
}

/// Decodes and opens the COER of a `SecuredCrl`.
pub fn decode_crl(bytes: &[u8]) -> Result<OpenedCrl, CrlError> {
    let crl: SecuredCrl = rasn::coer::decode(bytes).map_err(|e| CrlError::Decode {
        layer: "SecuredCrl",
        detail: e.to_string(),
    })?;
    open_crl(&crl)
}

/// The COER of a `SecuredCrl`.
pub fn encode_crl(crl: &SecuredCrl) -> Result<Vec<u8>, CrlError> {
    Ok(coer(crl)?)
}

/// The signer's `CrlSsp`, if its certificate grants PSID 256 with one.
pub fn crl_ssp(cert: &Certificate) -> Option<CrlSsp> {
    let ssp = crate::ctl::app_ssp(cert, CRL_PSID)??;
    rasn::coer::decode(&ssp).ok()
}

/// The verdict on a received CRL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrlVerdict {
    /// psid 256 and every other header field absent.
    pub header_ok: bool,
    /// The signature verifies under the signer's certificate.
    pub signature_ok: bool,
    /// The signer holds a version-1 `CrlSsp` listing this series.
    pub series_permitted: bool,
    /// `crlCraca` names the certificate the SSP's `associatedCraca` says it must.
    pub craca_ok: bool,
    /// The contents satisfy [`check_crl`].
    pub contents_ok: bool,
}

impl CrlVerdict {
    /// The device may act on the CRL.
    pub fn accepted(&self) -> bool {
        self.header_ok
            && self.signature_ok
            && self.series_permitted
            && self.craca_ok
            && self.contents_ok
    }
}

/// Verifies a CRL. `signer_cert` is the attached certificate or the one a digest
/// resolved to; `signer_issuer` is the certificate that issued it, needed only when its
/// SSP says `issuerIsCraca`. Chain validation of the signer to a CTL root is the chain
/// validator's job (WP-A3) and is not repeated here.
pub fn verify_crl(
    crl: &OpenedCrl,
    signer_cert: &Certificate,
    signer_issuer: Option<&Certificate>,
    edition: WireEdition,
) -> CrlVerdict {
    let h = &crl.signed.header;
    let header_ok = crl.signed.psid == CRL_PSID
        && h.generation_time.is_none()
        && h.expiry_time.is_none()
        && h.generation_location.is_none()
        && h.p2pcd_learning_request.is_none()
        && h.missing_crl_identifier.is_none()
        && h.encryption_key.is_none();
    let attached_matches = match &crl.signed.signer {
        SpduSigner::Certificate(c) => c == signer_cert,
        SpduSigner::Digest(d) => cert_hashed_id8(signer_cert).is_ok_and(|x| &x == d),
        SpduSigner::SelfSigned => false,
    };
    let signature_ok = attached_matches && sig1609::verify_opened(&crl.signed, signer_cert);
    let ssp = crl_ssp(signer_cert).filter(|s| s.version.0 == 1);
    let series = crl.contents.crl_series.0.0;
    let series_permitted = ssp
        .as_ref()
        .is_some_and(|s| s.crls.iter().any(|c| c.0.0 == series));
    let craca = &crl.contents.crl_craca.0[..];
    let craca_ok = match ssp.as_ref().map(|s| s.associated_craca) {
        Some(CracaType::isCraca) => cert_hashed_id8(signer_cert).is_ok_and(|x| x[..] == *craca),
        Some(CracaType::issuerIsCraca) => signer_issuer.is_some_and(|i| {
            let Ok(id) = cert_hashed_id8(i) else {
                return false;
            };
            let names_it = matches!(&signer_cert.0.issuer,
                IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d)
                    if d.0[..] == id[..]);
            names_it && id[..] == *craca
        }),
        None => false,
    };
    CrlVerdict {
        header_ok,
        signature_ok,
        series_permitted,
        craca_ok,
        contents_ok: check_crl(&crl.contents, edition).is_ok(),
    }
}

/// Is `cert` revoked by `contents` (see the module documentation)? Only the CRL's own
/// series and CRACA are considered; the caller checks the CRL with [`verify_crl`] first.
pub fn revokes(contents: &CrlContents, cert: &Certificate) -> bool {
    let tbs = &cert.0.to_be_signed;
    if tbs.crl_series.0.0 != contents.crl_series.0.0
        || tbs.craca_id.0[..] != contents.crl_craca.0[5..]
    {
        return false;
    }
    match &contents.type_specific {
        TypeSpecificCrlContents::fullHashCrl(t) | TypeSpecificCrlContents::deltaHashCrl(t) => {
            let Ok(id) = cert_hashed_id10(cert) else {
                return false;
            };
            t.entries.0.iter().any(|e| e.id.0[..] == id[..])
        }
        TypeSpecificCrlContents::fullLinkedCrl(l) | TypeSpecificCrlContents::deltaLinkedCrl(l) => {
            let CertificateId::linkageData(ld) = &tbs.id else {
                return false;
            };
            let mut lv = [0u8; 9];
            lv.copy_from_slice(&ld.linkage_value.0[..]);
            let lv = LinkageValue::new(lv);
            let i_cert = u32::from(ld.i_cert.0.0);
            let i_rev = u32::from(l.i_rev.0.0);
            for jg in l.individual.iter().flat_map(|s| s.0.iter()) {
                for lg in &jg.contents.0 {
                    let la1 = LaId(u16::from_be_bytes([lg.la1_id.0[0], lg.la1_id.0[1]]));
                    let la2 = LaId(u16::from_be_bytes([lg.la2_id.0[0], lg.la2_id.0[1]]));
                    for ig in &lg.contents.0 {
                        // The entry walks from iRev to iMax and no further: past iMax the
                        // device holds no certificates, and the bound also caps the hash
                        // work an attacker-chosen iCert can cause (linkage.rs).
                        let Some(forward) = u32::from(ig.i_max.0).checked_sub(i_rev) else {
                            continue;
                        };
                        for r in &ig.contents.0 {
                            let mut s1 = [0u8; 16];
                            let mut s2 = [0u8; 16];
                            s1.copy_from_slice(&r.linkage_seed1.0[..]);
                            s2.copy_from_slice(&r.linkage_seed2.0[..]);
                            let e = CrlLinkageEntry {
                                i: i_rev,
                                la_id1: la1,
                                la_id2: la2,
                                ls1_i: LinkageSeed::new(s1),
                                ls2_i: LinkageSeed::new(s2),
                                jmax: u32::from(jg.jmax.0),
                                max_forward: forward,
                            };
                            if e.matches_any_index(i_cert, lv) {
                                return true;
                            }
                        }
                    }
                }
            }
            false
        }
        _ => false,
    }
}

/// The `HashRevocation` entry for `cert`, expiring when it does.
pub fn hash_entry(cert: &Certificate) -> Result<HashRevocation, CrlError> {
    let (_, end) = crate::ctl::validity(cert);
    Ok(HashRevocation {
        id: cert_hashed_id10(cert)?,
        expiry: u32::try_from(end).unwrap_or(u32::MAX),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctl::{self_signed_certificate, validity};
    use crate::linkage::{DeviceLinkageContext, linkage_seed_at};
    use v2xw_msg::sec_types::ieee1609_dot2::LinkageData;
    use v2xw_msg::sec_types::ieee1609_dot2::{
        CertificateBase, CertificateType, ToBeSignedCertificate, VerificationKeyIndicator,
    };
    use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
        Duration, HashedId3, Psid, PsidSsp, SequenceOfPsidSsp, ServiceSpecificPermissions,
        ValidityPeriod,
    };

    fn crl_signer(ssp: CrlSsp) -> (Certificate, Signer1609) {
        let k = Signer1609::p256(&[0x11; 32]).unwrap();
        let perms = vec![PsidSsp::new(
            Psid(rasn::types::Integer::from(256u32)),
            Some(ServiceSpecificPermissions::opaque(ssp.to_coer().into())),
        )];
        (
            self_signed_certificate("crlg.sim", 600_000_000, 10, perms, None, &k).unwrap(),
            k,
        )
    }

    /// A pseudonym-like certificate with linkage data (the fields revocation reads).
    fn pseudonym(craca: [u8; 8], i: u16, lv: LinkageValue) -> Certificate {
        let k = Signer1609::p256(&[0x22; 32]).unwrap();
        let tbs = ToBeSignedCertificate::new(
            CertificateId::linkageData(LinkageData::new(IValue(Uint16(i)), lv.to_asn1(), None)),
            HashedId3(rasn::types::FixedOctetString::from([
                craca[5], craca[6], craca[7],
            ])),
            CrlSeries(Uint16(1)),
            ValidityPeriod::new(Time32(Uint32(700_000_000)), Duration::hours(Uint16(168))),
            None,
            None,
            Some(SequenceOfPsidSsp(vec![PsidSsp::new(
                Psid(rasn::types::Integer::from(32u32)),
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
        Certificate(CertificateBase::new(
            Uint8(3),
            CertificateType::explicit,
            IssuerIdentifier::sha256AndDigest(HashedId8(rasn::types::FixedOctetString::from(
                [9u8; 8],
            ))),
            tbs,
            None,
        ))
    }

    fn device() -> DeviceLinkageContext {
        DeviceLinkageContext::new(
            LaId(0x1234),
            LaId(0x5678),
            LinkageSeed::new([1; 16]),
            LinkageSeed::new([2; 16]),
        )
    }

    fn linkage_crl(craca: [u8; 8], i_rev: u16, i_max: u16) -> CrlSpec {
        let d = device();
        CrlSpec {
            series: 1,
            craca,
            issue_date: 700_000_000,
            next_crl: 700_086_400,
            priority: None,
            delta: false,
            body: CrlBody::Linkage {
                i_rev,
                index_within_i: 0,
                individual: vec![LinkageRevocation {
                    la1: d.la_id1,
                    la2: d.la_id2,
                    ls1: linkage_seed_at(d.la_id1, d.ls1_0, u32::from(i_rev)),
                    ls2: linkage_seed_at(d.la_id2, d.ls2_0, u32::from(i_rev)),
                    jmax: 20,
                    i_max,
                }],
            },
        }
    }

    /// A signed linkage CRL round-trips, verifies under an isCraca signer, and revokes
    /// the device's certificates from iRev to iMax, every j < jmax, and nothing else.
    #[test]
    fn linkage_crl_signs_verifies_and_revokes_the_right_certificates() {
        let (signer, key) = crl_signer(CrlSsp::new(CracaType::isCraca, &[1]));
        let craca = cert_hashed_id8(&signer).unwrap();
        let c = crl_contents(&linkage_crl(craca, 10, 12));
        let bytes = encode_crl(&sign_crl(&c, &signer, &key).unwrap()).unwrap();
        let o = decode_crl(&bytes).unwrap();
        assert_eq!(o.contents, c);
        let v = verify_crl(&o, &signer, None, WireEdition::Ieee2022);
        assert!(v.accepted(), "{v:?}");
        let d = device();
        for (i, j, want) in [
            (10, 0, true),
            (11, 19, true),
            (12, 5, true),
            (9, 0, false),
            (13, 0, false),
            (11, 20, false),
        ] {
            let cert = pseudonym(craca, i, d.linkage_value_for(u32::from(i), j));
            assert_eq!(revokes(&o.contents, &cert), want, "i {i} j {j}");
        }
        // Another CRACA or series does not match.
        let mut other = craca;
        other[7] ^= 1;
        assert!(!revokes(
            &o.contents,
            &pseudonym(other, 10, d.linkage_value_for(10, 0))
        ));
    }

    /// Hash CRLs for 2/3/4/256 revoke by HashedId10 only within their series and CRACA.
    #[test]
    fn hash_crl_revokes_by_hashed_id10() {
        let (signer, key) = crl_signer(CrlSsp::new(CracaType::isCraca, &[2, 3, 4, 256]));
        let craca = cert_hashed_id8(&signer).unwrap();
        let mut victim = pseudonym(craca, 0, LinkageValue::new([7; 9]));
        victim.0.to_be_signed.crl_series = CrlSeries(Uint16(3));
        victim.0.to_be_signed.id = CertificateId::none(());
        let entry = hash_entry(&victim).unwrap();
        assert_eq!(u64::from(entry.expiry), validity(&victim).1);
        for (series, delta) in [(2, false), (3, true), (4, false), (256, true)] {
            let spec = CrlSpec {
                series,
                craca,
                issue_date: 700_000_000,
                next_crl: 700_086_400,
                priority: Some(1),
                delta,
                body: CrlBody::Hash {
                    serial: 7,
                    entries: vec![entry],
                },
            };
            let c = crl_contents(&spec);
            let o =
                decode_crl(&encode_crl(&sign_crl(&c, &signer, &key).unwrap()).unwrap()).unwrap();
            assert!(verify_crl(&o, &signer, None, WireEdition::Ieee2022).accepted());
            assert_eq!(
                revokes(&o.contents, &victim),
                series == 3,
                "series {series}"
            );
        }
    }

    /// The CrlSsp rules: series not listed, the wrong CRACA, issuerIsCraca without or
    /// with the wrong issuer, and a non-256 header all refuse the CRL.
    #[test]
    fn crl_ssp_and_header_rules() {
        let (signer, key) = crl_signer(CrlSsp::new(CracaType::isCraca, &[2]));
        let craca = cert_hashed_id8(&signer).unwrap();
        let c = crl_contents(&linkage_crl(craca, 10, 12));
        let o = open_crl(&sign_crl(&c, &signer, &key).unwrap()).unwrap();
        let v = verify_crl(&o, &signer, None, WireEdition::Ieee2022);
        assert!(v.signature_ok && v.craca_ok && !v.series_permitted && !v.accepted());

        let (signer, key) = crl_signer(CrlSsp::new(CracaType::isCraca, &[1]));
        let c = crl_contents(&linkage_crl([0xAB; 8], 10, 12));
        let o = open_crl(&sign_crl(&c, &signer, &key).unwrap()).unwrap();
        let v = verify_crl(&o, &signer, None, WireEdition::Ieee2022);
        assert!(v.series_permitted && !v.craca_ok);

        // issuerIsCraca: the CRACA is the signer's issuer.
        let ik = Signer1609::p256(&[0x55; 32]).unwrap();
        let issuer = self_signed_certificate("ma.sim", 600_000_000, 10, vec![], None, &ik).unwrap();
        let (mut signer, key) = crl_signer(CrlSsp::new(CracaType::issuerIsCraca, &[1]));
        let iid = cert_hashed_id8(&issuer).unwrap();
        signer.0.issuer =
            IssuerIdentifier::sha256AndDigest(HashedId8(rasn::types::FixedOctetString::from(iid)));
        let c = crl_contents(&linkage_crl(iid, 10, 12));
        let o = open_crl(&sign_crl(&c, &signer, &key).unwrap()).unwrap();
        assert!(verify_crl(&o, &signer, Some(&issuer), WireEdition::Ieee2022).accepted());
        assert!(!verify_crl(&o, &signer, None, WireEdition::Ieee2022).craca_ok);
        assert!(!verify_crl(&o, &signer, Some(&signer.clone()), WireEdition::Ieee2022).craca_ok);
        // A CRL naming some other certificate as CRACA is refused even when that
        // certificate is offered as the issuer: the signer's issuer field must name it.
        let ok2 = Signer1609::p256(&[0x56; 32]).unwrap();
        let other =
            self_signed_certificate("other.sim", 600_000_000, 10, vec![], None, &ok2).unwrap();
        let c2 = crl_contents(&linkage_crl(cert_hashed_id8(&other).unwrap(), 10, 12));
        let o2 = open_crl(&sign_crl(&c2, &signer, &key).unwrap()).unwrap();
        assert!(!verify_crl(&o2, &signer, Some(&other), WireEdition::Ieee2022).craca_ok);

        // A header with a generation time is not a SecuredCrl.
        let mut h = sig1609::header_psid_only(CRL_PSID);
        h.generation_time = Some(v2xw_msg::sec_types::ieee1609_dot2_base_types::Time64(
            v2xw_msg::sec_types::ieee1609_dot2_base_types::Uint64(1),
        ));
        let d = sig1609::sign_spdu(
            h,
            coer(&c).unwrap(),
            SpduSigner::Certificate(signer.clone()),
            &key,
        )
        .unwrap();
        let o = open_crl(&SecuredCrl(d)).unwrap();
        assert!(!verify_crl(&o, &signer, Some(&issuer), WireEdition::Ieee2022).header_ok);
        // Nor is one under another PSID.
        let d = sig1609::sign_spdu(
            sig1609::header_psid_only(35),
            coer(&c).unwrap(),
            SpduSigner::Certificate(signer.clone()),
            &key,
        )
        .unwrap();
        let o = open_crl(&SecuredCrl(d)).unwrap();
        let v = verify_crl(&o, &signer, Some(&issuer), WireEdition::Ieee2022);
        assert!(v.signature_ok && !v.header_ok);
    }

    /// Gap row 29's mutation: an entry added after signing (outside the signed bytes as
    /// the signer produced them) makes the CRL fail on the device.
    #[test]
    fn an_entry_added_after_signing_fails() {
        let (signer, key) = crl_signer(CrlSsp::new(CracaType::isCraca, &[1]));
        let craca = cert_hashed_id8(&signer).unwrap();
        let mut crl = sign_crl(&crl_contents(&linkage_crl(craca, 10, 12)), &signer, &key).unwrap();
        let mut forged = linkage_crl(craca, 10, 12);
        if let CrlBody::Linkage { individual, .. } = &mut forged.body {
            let mut extra = individual[0];
            extra.ls1 = LinkageSeed::new([0xEE; 16]);
            individual.push(extra);
        }
        let v2xw_msg::sec_types::ieee1609_dot2::Ieee1609Dot2Content::signedData(sd) =
            &mut crl.0.content
        else {
            panic!()
        };
        sd.tbs_data.payload.data = Some(v2xw_msg::sec_types::ieee1609_dot2::Ieee1609Dot2Data::new(
            Uint8(3),
            v2xw_msg::sec_types::ieee1609_dot2::Ieee1609Dot2Content::unsecuredData(
                v2xw_msg::sec_types::ieee1609_dot2_base_types::Opaque(
                    coer(&crl_contents(&forged)).unwrap().into(),
                ),
            ),
        ));
        let o = decode_crl(&encode_crl(&crl).unwrap()).unwrap();
        assert!(!verify_crl(&o, &signer, None, WireEdition::Ieee2022).signature_ok);
    }

    /// Contents rules: series/type pairing, nextCrl > issueDate, iPeriodInfo only in 2025.
    #[test]
    fn contents_rules_and_editions() {
        let mut spec = linkage_crl([1; 8], 10, 12);
        assert!(check_crl(&crl_contents(&spec), WireEdition::Ieee2022).is_ok());
        spec.series = 2;
        assert!(check_crl(&crl_contents(&spec), WireEdition::Ieee2022).is_err());
        spec.series = 1;
        let hash_series_1 = CrlSpec {
            body: CrlBody::Hash {
                serial: 1,
                entries: vec![],
            },
            ..spec.clone()
        };
        assert!(check_crl(&crl_contents(&hash_series_1), WireEdition::Ieee2022).is_err());
        spec.next_crl = spec.issue_date;
        assert!(check_crl(&crl_contents(&spec), WireEdition::Ieee2022).is_err());
        let spec = linkage_crl([1; 8], 10, 9);
        assert!(check_crl(&crl_contents(&spec), WireEdition::Ieee2022).is_err());
        let info = vec![IPeriodInfo::new(
            Time32(Uint32(700_100_000)),
            Duration::hours(Uint16(168)),
            IValue(Uint16(20)),
        )];
        let c = crl_contents(&linkage_crl([1; 8], 10, 12));
        assert!(with_i_period_info(c.clone(), info.clone(), WireEdition::Ieee2022).is_err());
        assert!(with_i_period_info(c, info, WireEdition::Ieee2025).is_ok());
    }

    /// The published root SSP for PSID 256 (`00 01 00 01 01 01 00`, 2025 test CTL) is
    /// CrlSsp{1, isCraca, [256]}.
    #[test]
    fn crl_ssp_encoding_matches_the_published_root_ssp() {
        let s = CrlSsp::new(CracaType::isCraca, &[256]);
        assert_eq!(s.to_coer(), [0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x00]);
        let d: CrlSsp =
            rasn::coer::decode(&[0x00, 0x01, 0x01, 0x01, 0x02, 0x00, 0x01, 0x00, 0x02]).unwrap();
        assert_eq!(d, CrlSsp::new(CracaType::issuerIsCraca, &[1, 2]));
    }

    /// Size: one series-1 entry is 33 bytes (two 16-byte seeds and the extensible
    /// `IndividualRevocation` preamble) plus the grouping; 10,000 devices
    /// revoked in one (jmax, LA pair, iMax) group cost ≈33 B each, and with every device
    /// in its own iMax group about 37 B each; Brecht et al. give ≈40 B per entry and
    /// ≈400 KB per 10,000 (gap row 29).
    #[test]
    fn linkage_crl_size_is_brechts() {
        let d = device();
        let mk = |n: u32, spread: bool| {
            let individual = (0..n)
                .map(|k| {
                    let mut s1 = [0u8; 16];
                    s1[..4].copy_from_slice(&k.to_be_bytes());
                    LinkageRevocation {
                        la1: d.la_id1,
                        la2: d.la_id2,
                        ls1: LinkageSeed::new(s1),
                        ls2: LinkageSeed::new(s1),
                        jmax: 20,
                        i_max: if spread { 10 + (k % 60000) as u16 } else { 200 },
                    }
                })
                .collect();
            let spec = CrlSpec {
                series: 1,
                craca: [1; 8],
                issue_date: 1,
                next_crl: 2,
                priority: None,
                delta: false,
                body: CrlBody::Linkage {
                    i_rev: 10,
                    index_within_i: 0,
                    individual,
                },
            };
            coer(&crl_contents(&spec)).unwrap().len() as f64 / f64::from(n)
        };
        let packed = mk(10_000, false);
        let spread = mk(10_000, true);
        assert!((33.0..34.0).contains(&packed), "{packed}");
        assert!((36.0..41.0).contains(&spread), "{spread}");
    }
}
