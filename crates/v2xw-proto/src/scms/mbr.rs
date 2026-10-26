//! US misbehaviour reports as the CV-Pilot SCMS specifies them (WP-B4; gap row 25).
//!
//! # Format ([SM-MBR] v1.0 §1–3; [SM-MBR-ASN] v1.1 `CvpdMbdModule`; 03 §8.1)
//!
//! ```text
//! EndEntityMaInterfacePDU ::= CHOICE { eeMaApplicationSpecificMisbehaviorReport …, ... }
//! EeMaApplicationSpecificMisbehaviorReport ::= SEQUENCE {
//!     version Uint8 (1), generationTime Time32, generationLocation ThreeDLocation,
//!     psid  APPLICATION-SPECIFIC-MISBEHAVIOR-REPORT.&id({CvpdMbr}),          -- 32 (BSM)
//!     contents …&Contents({CvpdMbr}{@.psid}), ... }                         -- open type
//! Psid20Mbr ::= SEQUENCE { version Uint8 (1), type Psid20MbrType,
//!     subtype …&SubtypeArrayAndParams({…}{@.type}),                          -- open type
//!     evidence …&Evidence({…}{@.type}), ... }                                -- open type
//! subtype list ::= SEQUENCE (SIZE(1..MAX)) OF SEQUENCE { subtype INTEGER (0..255),
//!     params …&Params({…}{@.subtype}) }                                       -- open type
//! SignedBsmsWithCertificate ::= SEQUENCE {
//!     signedBsmList SEQUENCE (SIZE(1..MAX)) OF SignedBSM, signingCertificate Certificate, ... }
//! ```
//!
//! Types and subtypes for PSID 0x20 (the v1.1 information-object sets):
//!
//! | type | subtype | params |
//! |---|---|---|
//! | 1 `1VInconsistent` (≥ 2 BSMs, one sender) | 1 ConstantPosition | NULL |
//! | | 2 RandomPosition | `{ gpsDrift Uint16 (mm), ... }` |
//! | 2 `Implausible` (exactly 1 BSM) | 1 MaxSpeed | `{ threshold Uint16 (m/s), ... }` |
//! | | 2 MaxAcceleration | `{ threshold Uint16 (cm/s²), ... }` |
//! | | 3 BrakeAcceleration | NULL |
//! | 3 `ObsImplausible` (suspect + reporter BSM) | 1 Distance | `{ threshold Uint16 (m), gpsDrift Uint16 (m), ... }` |
//!
//! **Open types.** Every CLASS-bound field is an ASN.1 open type, which COER encodes as a
//! length determinant followed by the encoding of the value (X.696 §30) — the same octets
//! as an unconstrained `OCTET STRING` holding that encoding. The Rust types below carry
//! those fields as `OctetString` and fill them with the inner COER, so the bytes are the
//! module's. `tools/pdu_check/check_mbr.py` decodes them with asn1tools against a
//! module written from the published one (the published PDF is not committed: owner
//! decision, "All Rights Reserved").
//!
//! # Security processing ([SM-MBR] §3)
//!
//! COER `EndEntityMaInterfacePDU` → 1609.2 signed data (payload: unsecured data holding
//! that COER; `headerInfo` carries `psid 0x26` and `generationTime` only; signer = the
//! end-entity certificate, a pseudonym certificate in the CAMP design) → encrypted to the
//! `encryptionKey` of the MA certificate whose `MaSsp.relevantPsids` covers the
//! application PSID (one `certRecipInfo`, ephemeral key, compressed points).
//!
//! # Upload ([SM-MBR] §4)
//!
//! One HTTPS POST per report to the RA at `:8892/process-misbehavior-report`, body the
//! encrypted report, authenticated by a `Download-Req` header holding an
//! `EeRaDownloadRequest` for the file name `misbehavior-report` signed by the
//! **enrolment** certificate ([`upload_header`]). The RA therefore knows the reporter's
//! enrolment identity and cannot read the report; the MA can read the report and does
//! not learn the enrolment identity unless the RA tells it. That split is what the
//! privacy tests check.
//!
//! # Queue ([SM-MBR] §2.2; [`MbrQueue`])
//!
//! Each report gets a subtype weight (default 1.0), multiplied by the subtype's daily
//! multiplier (default 0.9) every day at 05:00 Eastern. Upload highest weight first,
//! newer first on ties; over the storage quota delete lowest weight first, older first on
//! ties; delete after the TCP-confirmed upload.

use rasn::prelude::*;
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, Ieee1609Dot2Content, Ieee1609Dot2Data, SignerIdentifier,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    Psid, ThreeDLocation, Time32, Time64, Uint8, Uint16, Uint32, Uint64,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::SecurityMgmtSsp;
use v2xw_sec::ec::Point;
use v2xw_sec::envelope::GenerationLocation;
use v2xw_sec::sig1609::{self, Signer1609, SpduSigner};

use crate::scms::pdu::{self, Drbg, PduError, Result, ScmsPki};

/// The misbehaviour-reporting PSID 0x26 ([SM-MBR] §1).
pub const MBR_PSID: u64 = 0x26;
/// The only application PSID with a defined report: 0x20, BSM ([SM-MBR] §1).
pub const BSM_APP_PSID: u64 = 0x20;
/// `EeMaApplicationSpecificMisbehaviorReport.version` and `Psid20Mbr.version`.
pub const MBR_VERSION: u8 = 1;
/// The `filename` of the `Download-Req` authenticating an upload ([SM-MBR] §4).
pub const UPLOAD_FILENAME: &str = "misbehavior-report";

/// The COER `EndEntityMaInterfacePDU` of the reference US report, bytes: a
/// `1VInconsistent`/`ConstantPosition` report carrying two signed BSMs (the engine's
/// evidence depth) and the accused's implicit pseudonym certificate. **Derived** from this
/// module's encoder, not chosen: `tests/mbr.rs` builds the report on real credentials and
/// asserts this length. The size a run charges when it has no real report to send.
pub const REFERENCE_REPORT_COER_BYTES: u32 = 524;
/// The upload of the reference report, bytes: the signed-then-encrypted report (804 B)
/// plus its enrolment-signed `Download-Req` header (315 B), as [`crate::scms::run`]'s
/// `secure_mbr` produces them ([SM-MBR] §3–4). Derived and pinned as
/// [`REFERENCE_REPORT_COER_BYTES`].
pub const REFERENCE_UPLOAD_BYTES: u32 = 1_119;

// ---------------------------------------------------------------------------------------
// Wire types (CvpdMbdModule v1.1, open types as OCTET STRING)
// ---------------------------------------------------------------------------------------

/// `EndEntityMaInterfacePDU`.
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(choice, automatic_tags)]
#[non_exhaustive]
pub enum EndEntityMaInterfacePdu {
    /// `eeMaApplicationSpecificMisbehaviorReport`.
    #[rasn(identifier = "eeMaApplicationSpecificMisbehaviorReport")]
    Report(EeMaApplicationSpecificMisbehaviorReport),
}

/// `EeMaApplicationSpecificMisbehaviorReport` (v1.1 added `generationLocation`).
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct EeMaApplicationSpecificMisbehaviorReport {
    /// `version`, 1.
    pub version: Uint8,
    /// `generationTime`.
    #[rasn(identifier = "generationTime")]
    pub generation_time: Time32,
    /// `generationLocation`.
    #[rasn(identifier = "generationLocation")]
    pub generation_location: ThreeDLocation,
    /// `psid`: the application PSID.
    pub psid: Psid,
    /// `contents`: the open type, here the COER of a [`Psid20Mbr`].
    pub contents: OctetString,
}

/// `Psid20Mbr`.
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct Psid20Mbr {
    /// `version`, 1.
    pub version: Uint8,
    /// `type` (`Psid20MbrType ::= INTEGER (0..255)`).
    #[rasn(identifier = "type")]
    pub mbr_type: Uint8,
    /// `subtype`: open type, the COER of a `SEQUENCE OF` [`SubtypeParams`].
    pub subtype: OctetString,
    /// `evidence`: open type, the COER of the type's evidence.
    pub evidence: OctetString,
}

/// `Psid20MbrSubtypeParams1VInconsistent` / `…Implausible` / `…ObsImplausible` (all
/// three have this shape and no extension marker).
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
pub struct SubtypeParams {
    /// `subtype` (`INTEGER (0..255)`).
    pub subtype: Uint8,
    /// `params`: open type (NULL encodes as zero octets).
    pub params: OctetString,
}

/// `Psid20MbrParams1VInconsistentRandomPosition`.
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct ParamsRandomPosition {
    /// `gpsDrift`, thousandths of a metre.
    #[rasn(identifier = "gpsDrift")]
    pub gps_drift: Uint16,
}

/// `Psid20MbrParamsImplausibleMaxSpeed` and `…MaxAcceleration`.
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct ParamsThreshold {
    /// `threshold` (m/s for speed, hundredths of m/s² for acceleration).
    pub threshold: Uint16,
}

/// `Psid20MbrParamsObsImplausibleDistance`.
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct ParamsObsDistance {
    /// `threshold`, metres.
    pub threshold: Uint16,
    /// `gpsDrift`, metres.
    #[rasn(identifier = "gpsDrift")]
    pub gps_drift: Uint16,
}

/// `SignedBsmsWithCertificate`.
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct SignedBsmsWithCertificate {
    /// `signedBsmList` (`SIZE (1..MAX)` of `SignedBSM`).
    #[rasn(identifier = "signedBsmList")]
    pub signed_bsm_list: SequenceOf<Ieee1609Dot2Data>,
    /// `signingCertificate`.
    #[rasn(identifier = "signingCertificate")]
    pub signing_certificate: Certificate,
}

/// `Psid20MbrEvidenceObsImplausible`.
#[derive(AsnType, Encode, Decode, Debug, Clone, PartialEq, Eq)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct EvidenceObsImplausible {
    /// `suspectBsm`.
    #[rasn(identifier = "suspectBsm")]
    pub suspect_bsm: SignedBsmsWithCertificate,
    /// `reporterBsm`.
    #[rasn(identifier = "reporterBsm")]
    pub reporter_bsm: SignedBsmsWithCertificate,
}

// ---------------------------------------------------------------------------------------
// The report as a model value
// ---------------------------------------------------------------------------------------

/// `Psid20MbrType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MbrType {
    /// 1, `psid20MbrType1VInconsistent`.
    OneVInconsistent = 1,
    /// 2, `psid20MbrTypeImplausible`.
    Implausible = 2,
    /// 3, `psid20MbrTypeObsImplausible`.
    ObsImplausible = 3,
}

impl MbrType {
    /// The type number (`Psid20MbrType`).
    #[must_use]
    pub fn number(self) -> u8 {
        self as u8
    }

    /// The module's name for the type, without the `psid20MbrType` prefix.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            MbrType::OneVInconsistent => "1VInconsistent",
            MbrType::Implausible => "Implausible",
            MbrType::ObsImplausible => "ObsImplausible",
        }
    }

    fn from_u8(v: u8) -> Option<MbrType> {
        match v {
            1 => Some(MbrType::OneVInconsistent),
            2 => Some(MbrType::Implausible),
            3 => Some(MbrType::ObsImplausible),
            _ => None,
        }
    }
}

/// One subtype with its parameters ([SM-MBR] §2.3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Subtype {
    /// 1VInconsistent / 1 ConstantPosition.
    ConstantPosition,
    /// 1VInconsistent / 2 RandomPosition; default drift 1 m = 1000 mm.
    RandomPosition {
        /// `gpsDrift` in mm.
        gps_drift_mm: u16,
    },
    /// Implausible / 1 MaxSpeed; default 90 m/s.
    MaxSpeed {
        /// m/s.
        threshold_mps: u16,
    },
    /// Implausible / 2 MaxAcceleration; default 10 m/s² = 1000 cm/s².
    MaxAcceleration {
        /// cm/s².
        threshold_cmps2: u16,
    },
    /// Implausible / 3 BrakeAcceleration.
    BrakeAcceleration,
    /// ObsImplausible / 1 Distance; default 3000 m plus 10 m drift, **off by default**
    /// because a relay makes an innocent sender look far away ([SM-MBR] §2.3.1.4.2).
    ObsDistance {
        /// m.
        threshold_m: u16,
        /// m.
        gps_drift_m: u16,
    },
}

impl Subtype {
    /// The report type this subtype belongs to.
    #[must_use]
    pub fn mbr_type(self) -> MbrType {
        match self {
            Subtype::ConstantPosition | Subtype::RandomPosition { .. } => MbrType::OneVInconsistent,
            Subtype::MaxSpeed { .. }
            | Subtype::MaxAcceleration { .. }
            | Subtype::BrakeAcceleration => MbrType::Implausible,
            Subtype::ObsDistance { .. } => MbrType::ObsImplausible,
        }
    }

    /// The subtype number inside its type.
    #[must_use]
    pub fn number(self) -> u8 {
        match self {
            Subtype::ConstantPosition | Subtype::MaxSpeed { .. } | Subtype::ObsDistance { .. } => 1,
            Subtype::RandomPosition { .. } | Subtype::MaxAcceleration { .. } => 2,
            Subtype::BrakeAcceleration => 3,
        }
    }

    /// The subtype's name, as the module's table names it (this file's header).
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Subtype::ConstantPosition => "ConstantPosition",
            Subtype::RandomPosition { .. } => "RandomPosition",
            Subtype::MaxSpeed { .. } => "MaxSpeed",
            Subtype::MaxAcceleration { .. } => "MaxAcceleration",
            Subtype::BrakeAcceleration => "BrakeAcceleration",
            Subtype::ObsDistance { .. } => "Distance",
        }
    }

    fn params(self) -> Result<Vec<u8>> {
        match self {
            Subtype::ConstantPosition | Subtype::BrakeAcceleration => Ok(Vec::new()),
            Subtype::RandomPosition { gps_drift_mm } => pdu::coer(&ParamsRandomPosition {
                gps_drift: Uint16(gps_drift_mm),
            }),
            Subtype::MaxSpeed { threshold_mps: t }
            | Subtype::MaxAcceleration { threshold_cmps2: t } => pdu::coer(&ParamsThreshold {
                threshold: Uint16(t),
            }),
            Subtype::ObsDistance {
                threshold_m,
                gps_drift_m,
            } => pdu::coer(&ParamsObsDistance {
                threshold: Uint16(threshold_m),
                gps_drift: Uint16(gps_drift_m),
            }),
        }
    }

    fn from_wire(t: MbrType, sp: &SubtypeParams) -> Result<Subtype> {
        let p = &sp.params[..];
        let empty = |s: Subtype| {
            if p.is_empty() {
                Ok(s)
            } else {
                Err(PduError::Shape("NULL subtype params carry octets"))
            }
        };
        match (t, sp.subtype.0) {
            (MbrType::OneVInconsistent, 1) => empty(Subtype::ConstantPosition),
            (MbrType::OneVInconsistent, 2) => {
                let v: ParamsRandomPosition = pdu::decode(p)?;
                Ok(Subtype::RandomPosition {
                    gps_drift_mm: v.gps_drift.0,
                })
            }
            (MbrType::Implausible, 1) => {
                let v: ParamsThreshold = pdu::decode(p)?;
                Ok(Subtype::MaxSpeed {
                    threshold_mps: v.threshold.0,
                })
            }
            (MbrType::Implausible, 2) => {
                let v: ParamsThreshold = pdu::decode(p)?;
                Ok(Subtype::MaxAcceleration {
                    threshold_cmps2: v.threshold.0,
                })
            }
            (MbrType::Implausible, 3) => empty(Subtype::BrakeAcceleration),
            (MbrType::ObsImplausible, 1) => {
                let v: ParamsObsDistance = pdu::decode(p)?;
                Ok(Subtype::ObsDistance {
                    threshold_m: v.threshold.0,
                    gps_drift_m: v.gps_drift.0,
                })
            }
            _ => Err(PduError::Shape(
                "subtype not in the CvpdPsid20Mbr object set",
            )),
        }
    }
}

/// A sender's signed BSMs and the certificate that signed them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedBsms {
    /// The `SignedBSM`s as received.
    pub bsms: Vec<Ieee1609Dot2Data>,
    /// `signingCertificate`.
    pub cert: Certificate,
}

/// The evidence of a report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evidence {
    /// Types 1 and 2: BSMs from the suspect sender.
    Suspect(SignedBsms),
    /// Type 3: one suspect BSM and one reporter BSM.
    Observed {
        /// `suspectBsm`.
        suspect: SignedBsms,
        /// `reporterBsm`.
        reporter: SignedBsms,
    },
}

impl Evidence {
    /// The suspect's evidence.
    #[must_use]
    pub fn suspect(&self) -> &SignedBsms {
        match self {
            Evidence::Suspect(s) | Evidence::Observed { suspect: s, .. } => s,
        }
    }
}

/// A PSID-0x20 misbehaviour report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisbehaviorReport {
    /// `generationTime` (Time32).
    pub generation_time: u32,
    /// `generationLocation`.
    pub location: GenerationLocation,
    /// `type`.
    pub mbr_type: MbrType,
    /// The subtypes, in order (at least one, all of `mbr_type`).
    pub subtypes: Vec<Subtype>,
    /// The evidence.
    pub evidence: Evidence,
}

fn bsms_wire(s: &SignedBsms) -> SignedBsmsWithCertificate {
    SignedBsmsWithCertificate {
        signed_bsm_list: s.bsms.clone(),
        signing_certificate: s.cert.clone(),
    }
}

fn bsms_model(w: SignedBsmsWithCertificate) -> Result<SignedBsms> {
    if w.signed_bsm_list.is_empty() {
        return Err(PduError::Shape("signedBsmList SIZE (1..MAX)"));
    }
    Ok(SignedBsms {
        bsms: w.signed_bsm_list,
        cert: w.signing_certificate,
    })
}

/// The evidence-count rules of v1.1: a 1VInconsistent report SHALL hold at least two
/// BSMs, an Implausible report exactly one, an ObsImplausible report one of each.
fn check_shape(r: &MisbehaviorReport) -> Result<()> {
    if r.subtypes.is_empty() {
        return Err(PduError::Shape("subtype list SIZE (1..MAX)"));
    }
    if r.subtypes.iter().any(|s| s.mbr_type() != r.mbr_type) {
        return Err(PduError::Shape("subtype of another report type"));
    }
    let ok = match (&r.evidence, r.mbr_type) {
        (Evidence::Suspect(s), MbrType::OneVInconsistent) => s.bsms.len() >= 2,
        (Evidence::Suspect(s), MbrType::Implausible) => s.bsms.len() == 1,
        (Evidence::Observed { suspect, reporter }, MbrType::ObsImplausible) => {
            suspect.bsms.len() == 1 && reporter.bsms.len() == 1
        }
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(PduError::Shape("evidence does not fit the report type"))
    }
}

/// The COER `EndEntityMaInterfacePDU` of a report.
///
/// # Errors
/// The report breaks a v1.1 shape rule, or does not encode.
pub fn encode_report(r: &MisbehaviorReport) -> Result<Vec<u8>> {
    check_shape(r)?;
    let subtypes = r
        .subtypes
        .iter()
        .map(|s| {
            Ok(SubtypeParams {
                subtype: Uint8(s.number()),
                params: s.params()?.into(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let evidence = match &r.evidence {
        Evidence::Suspect(s) => pdu::coer(&bsms_wire(s))?,
        Evidence::Observed { suspect, reporter } => pdu::coer(&EvidenceObsImplausible {
            suspect_bsm: bsms_wire(suspect),
            reporter_bsm: bsms_wire(reporter),
        })?,
    };
    let inner = Psid20Mbr {
        version: Uint8(MBR_VERSION),
        mbr_type: Uint8(r.mbr_type as u8),
        subtype: pdu::coer(&subtypes)?.into(),
        evidence: evidence.into(),
    };
    pdu::coer(&EndEntityMaInterfacePdu::Report(
        EeMaApplicationSpecificMisbehaviorReport {
            version: Uint8(MBR_VERSION),
            generation_time: Time32(Uint32(r.generation_time)),
            generation_location: r.location.to_asn1(),
            psid: Psid(Integer::from(BSM_APP_PSID)),
            contents: pdu::coer(&inner)?.into(),
        },
    ))
}

/// Decodes and checks a COER `EndEntityMaInterfacePDU`: versions 1, application PSID
/// 0x20, a known type and subtypes, open types that hold exactly one value each, and the
/// evidence-count rules.
///
/// # Errors
/// The first rule that fails.
pub fn decode_report(bytes: &[u8]) -> Result<MisbehaviorReport> {
    let EndEntityMaInterfacePdu::Report(top) = pdu::decode(bytes)?;
    if top.version.0 != MBR_VERSION {
        return Err(PduError::Version(
            "EeMaApplicationSpecificMisbehaviorReport",
        ));
    }
    if u64::try_from(&top.psid.0).ok() != Some(BSM_APP_PSID) {
        return Err(PduError::Shape("psid not in the CvpdMbr object set"));
    }
    let inner: Psid20Mbr = pdu::decode(&top.contents)?;
    if inner.version.0 != MBR_VERSION {
        return Err(PduError::Version("Psid20Mbr"));
    }
    let t = MbrType::from_u8(inner.mbr_type.0)
        .ok_or(PduError::Shape("type not in the CvpdPsid20Mbr object set"))?;
    let list: Vec<SubtypeParams> = pdu::decode(&inner.subtype)?;
    let subtypes = list
        .iter()
        .map(|sp| Subtype::from_wire(t, sp))
        .collect::<Result<Vec<_>>>()?;
    let evidence = match t {
        MbrType::ObsImplausible => {
            let e: EvidenceObsImplausible = pdu::decode(&inner.evidence)?;
            Evidence::Observed {
                suspect: bsms_model(e.suspect_bsm)?,
                reporter: bsms_model(e.reporter_bsm)?,
            }
        }
        _ => Evidence::Suspect(bsms_model(pdu::decode(&inner.evidence)?)?),
    };
    let r = MisbehaviorReport {
        generation_time: top.generation_time.0.0,
        location: GenerationLocation::from_asn1(&top.generation_location),
        mbr_type: t,
        subtypes,
        evidence,
    };
    check_shape(&r)?;
    Ok(r)
}

// ---------------------------------------------------------------------------------------
// Security processing
// ---------------------------------------------------------------------------------------

/// Which MA a report for `app_psid` goes to: the first certificate whose PSID-35
/// `SecurityMgmtSsp` is `ma` with `relevantPsids` containing it ([ASN-2.1] `MaSsp`).
#[must_use]
pub fn select_ma<'a>(candidates: &[&'a Certificate], app_psid: u64) -> Option<&'a Certificate> {
    candidates.iter().copied().find(|c| ma_covers(c, app_psid))
}

/// Does `cert` carry an `MaSsp` whose `relevantPsids` lists `app_psid`?
#[must_use]
pub fn ma_covers(cert: &Certificate, app_psid: u64) -> bool {
    let Some(Some(ssp)) = v2xw_sec::ctl::app_ssp(cert, pdu::SECURITY_MGMT_PSID) else {
        return false;
    };
    let Ok(SecurityMgmtSsp::ma(ma)) = pdu::decode::<SecurityMgmtSsp>(&ssp) else {
        return false;
    };
    ma.relevant_psids
        .0
        .iter()
        .any(|p| u64::try_from(&p.0).ok() == Some(app_psid))
}

/// A signed BSM as its sender puts it on the air: 1609.2 signed data, `headerInfo`
/// psid 0x20 and `generationTime` (Time64 µs), signer the full certificate, payload the
/// UPER BSM. Used to make the evidence a report carries.
///
/// # Errors
/// It does not encode.
pub fn sign_bsm(
    uper_bsm: &[u8],
    generation_time_us: u64,
    cert: &Certificate,
    key: &Signer1609,
) -> Result<Ieee1609Dot2Data> {
    sig1609::sign_spdu(
        header(BSM_APP_PSID, generation_time_us),
        uper_bsm.to_vec(),
        SpduSigner::Certificate(cert.clone()),
        key,
    )
    .map_err(|e| PduError::Coer(e.to_string()))
}

/// A signed BSM as received (COER `Ieee1609Dot2Data`), for a report's evidence.
///
/// # Errors
/// The octets are not one COER `Ieee1609Dot2Data`.
pub fn decode_signed_bsm(spdu: &[u8]) -> Result<Ieee1609Dot2Data> {
    rasn::coer::decode::<Ieee1609Dot2Data>(spdu).map_err(|e| PduError::Coer(e.to_string()))
}

fn header(psid: u64, t_us: u64) -> v2xw_msg::sec_types::ieee1609_dot2::HeaderInfo {
    let mut h = sig1609::header_psid_only(psid);
    h.generation_time = Some(Time64(Uint64(t_us)));
    h
}

/// The `SecuredMisbehaviorReport` the device POSTs: [SM-MBR] §3, signed by the device's
/// pseudonym certificate with `generationTime` only, then encrypted to the MA.
///
/// # Errors
/// A value does not encode or the MA key is unusable.
pub fn secure_report(
    report_coer: &[u8],
    generation_time_us: u64,
    signer_cert: &Certificate,
    signer_key: &Signer1609,
    ma_cert: &Certificate,
    ma_enc_key: &Point,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    let signed = sig1609::sign_spdu(
        header(MBR_PSID, generation_time_us),
        report_coer.to_vec(),
        SpduSigner::Certificate(signer_cert.clone()),
        signer_key,
    )
    .map_err(|e| PduError::Coer(e.to_string()))?;
    pdu::encrypt_to_cert(&pdu::coer(&signed)?, ma_cert, ma_enc_key, g)
}

/// The `Download-Req` header of an upload ([SM-MBR] §4): an `EeRaDownloadRequest` for
/// `misbehavior-report`, signed by the **enrolment** certificate and encrypted to the RA
/// as every authenticated RA request is.
///
/// # Errors
/// A value does not encode.
pub fn upload_header(
    generation_time: u32,
    enrolment: &crate::scms::pdu::EnrolmentCredential,
    ra: &Certificate,
    ra_enc: &Point,
    g: &mut Drbg,
) -> Result<Vec<u8>> {
    let p = pdu::ee_ra_download_request(generation_time, UPLOAD_FILENAME);
    pdu::signed_encrypted(&p, &enrolment.cert, &enrolment.key, ra, ra_enc, g)
}

/// What the MA learns from a report it opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenedReport {
    /// The report.
    pub report: MisbehaviorReport,
    /// The reporter's (pseudonym) certificate.
    pub reporter_cert: Certificate,
    /// The outer `generationTime` (Time64 µs).
    pub signed_at_us: u64,
    /// Whether every suspect BSM verifies under the evidence's `signingCertificate`,
    /// which chains to the PKI.
    pub evidence_verified: bool,
}

/// The MA's processing of a received report ([5GAA] (263) preconditions; [SM-MBR] §3):
/// decrypt with the MA key, open the signed layer (psid 0x26, `generationTime` present,
/// every other header field absent, signer one certificate that chains to the PKI,
/// signature valid), decode the report, and verify the suspect evidence.
///
/// A forged report, one encrypted to another MA, or a tampered one is an error; evidence
/// that does not verify is reported in [`OpenedReport::evidence_verified`], so a
/// pipeline can weigh it (an accuser cannot sign BSMs as the accused).
///
/// # Errors
/// The first check that fails.
pub fn ma_open_report(
    bytes: &[u8],
    ma_cert: &Certificate,
    ma_private: &p256::Scalar,
    pki: &ScmsPki,
) -> Result<OpenedReport> {
    let plain = pdu::decrypt_cert(bytes, ma_cert, ma_private)?;
    let d: Ieee1609Dot2Data = pdu::decode(&plain)?;
    if d.protocol_version.0 != pdu::PROTOCOL_VERSION {
        return Err(PduError::Version("protocolVersion"));
    }
    if let Ieee1609Dot2Content::signedData(sd) = &d.content {
        if !matches!(&sd.signer, SignerIdentifier::certificate(v) if v.0.len() == 1) {
            return Err(PduError::Signer("the end-entity certificate only"));
        }
    }
    let o = sig1609::open_signed(&d).ok_or(PduError::Shape("signed data over unsecured data"))?;
    if o.psid != MBR_PSID {
        return Err(PduError::Psid(o.psid));
    }
    let h = &o.header;
    let Some(t) = h.generation_time.as_ref() else {
        return Err(PduError::Shape("generationTime is required"));
    };
    for (absent, name) in [
        (h.expiry_time.is_none(), "expiryTime"),
        (h.generation_location.is_none(), "generationLocation"),
        (h.p2pcd_learning_request.is_none(), "p2pcdLearningRequest"),
        (h.missing_crl_identifier.is_none(), "missingCrlIdentifier"),
        (h.encryption_key.is_none(), "encryptionKey"),
    ] {
        if !absent {
            return Err(PduError::HeaderField(name));
        }
    }
    let SpduSigner::Certificate(signer) = o.signer.clone() else {
        return Err(PduError::Signer("certificate"));
    };
    verify_under(&o, &signer, pki)?;
    let report = decode_report(&o.payload)?;
    let evidence_verified = verify_evidence(report.evidence.suspect(), pki);
    Ok(OpenedReport {
        report,
        reporter_cert: signer,
        signed_at_us: t.0.0,
        evidence_verified,
    })
}

fn verify_under(o: &sig1609::OpenedSigned, cert: &Certificate, pki: &ScmsPki) -> Result<()> {
    let (hash, key) = pki.signer_key(cert)?;
    if hash != o.hash {
        return Err(PduError::BadSignature);
    }
    let input = sig1609::canonical_cert_coer(cert).map_err(|e| PduError::Coer(e.to_string()))?;
    if sig1609::verify_digest(
        hash,
        &key,
        &sig1609::digest_1609(hash, &o.tbs_coer, &input),
        &o.signature,
    ) {
        Ok(())
    } else {
        Err(PduError::BadSignature)
    }
}

/// Every BSM verifies under `signingCertificate` (signer the certificate itself or its
/// digest), carries psid 0x20, and the certificate chains to the PKI.
#[must_use]
pub fn verify_evidence(e: &SignedBsms, pki: &ScmsPki) -> bool {
    let id8 = pdu::cert_id8(&e.cert);
    e.bsms.iter().all(|b| {
        let Some(o) = sig1609::open_signed(b) else {
            return false;
        };
        let signer_ok = match &o.signer {
            SpduSigner::Certificate(c) => c == &e.cert,
            SpduSigner::Digest(d) => *d == id8,
            SpduSigner::SelfSigned => false,
        };
        signer_ok && o.psid == BSM_APP_PSID && verify_under(&o, &e.cert, pki).is_ok()
    })
}

// ---------------------------------------------------------------------------------------
// The end entity's report queue
// ---------------------------------------------------------------------------------------

/// Weighting of one subtype ([SM-MBR] §2.3.1: every subtype defaults to 1.0 and 0.9).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Weighting {
    /// `initialWeight`.
    pub initial: f64,
    /// `DailyMultiplier`, applied at 05:00 Eastern.
    pub daily_multiplier: f64,
}

impl Default for Weighting {
    fn default() -> Self {
        Weighting {
            initial: 1.0,
            daily_multiplier: 0.9,
        }
    }
}

/// The [SM-MBR] §2.3.1 `initialWeight` of each PSID-0x20 subtype and the
/// `DailyMultiplier` applied at the daily re-weighting. Every subtype of v1.0 defaults to
/// 1.0 and 0.9 (§2.3.1.2.2–§2.3.1.4). The specification gives each subtype its own
/// multiplier; this store applies one multiplier to every subtype (all are 0.9 in v1.0),
/// so a sweep moves them together.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SubtypeWeights {
    /// 1VInconsistent / RandomPosition, §2.3.1.2.2.
    pub random_position: f64,
    /// 1VInconsistent / ConstantPosition, §2.3.1.2.3.
    pub constant_position: f64,
    /// Implausible / MaxSpeed, §2.3.1.3.1.
    pub max_speed: f64,
    /// Implausible / MaxAcceleration, §2.3.1.3.2.
    pub max_acceleration: f64,
    /// Implausible / BrakeAcceleration, §2.3.1.3.4.
    pub brake_acceleration: f64,
    /// ObsImplausible / Distance, §2.3.1.4 (the check is off by default).
    pub obs_distance: f64,
    /// `DailyMultiplier`, every subtype.
    pub daily_multiplier: f64,
}

impl Default for SubtypeWeights {
    fn default() -> Self {
        SubtypeWeights {
            random_position: 1.0,
            constant_position: 1.0,
            max_speed: 1.0,
            max_acceleration: 1.0,
            brake_acceleration: 1.0,
            obs_distance: 1.0,
            daily_multiplier: 0.9,
        }
    }
}

impl SubtypeWeights {
    /// The `initialWeight` of one subtype.
    #[must_use]
    pub fn initial(&self, s: Subtype) -> f64 {
        match s {
            Subtype::RandomPosition { .. } => self.random_position,
            Subtype::ConstantPosition => self.constant_position,
            Subtype::MaxSpeed { .. } => self.max_speed,
            Subtype::MaxAcceleration { .. } => self.max_acceleration,
            Subtype::BrakeAcceleration => self.brake_acceleration,
            Subtype::ObsDistance { .. } => self.obs_distance,
        }
    }

    /// The weighting of a report listing `subtypes`. [SM-MBR] §1 lets one report carry
    /// several subtypes of one type, and §2.2 assigns "a (subtype-specific) weight"
    /// without saying which subtype's: ASSUMPTION, the largest, so evidence of several
    /// misbehaviours is never stored below any one of them. No subtype: 1.0.
    #[must_use]
    pub fn weighting(&self, subtypes: &[Subtype]) -> Weighting {
        let initial = subtypes
            .iter()
            .map(|s| self.initial(*s))
            .reduce(f64::max)
            .unwrap_or(1.0);
        Weighting {
            initial,
            daily_multiplier: self.daily_multiplier,
        }
    }
}

/// One stored report.
#[derive(Debug, Clone)]
pub struct QueuedReport<T> {
    /// Its current weight.
    pub weight: f64,
    /// Its daily multiplier.
    pub multiplier: f64,
    /// Creation order (larger is newer).
    pub seq: u64,
    /// When it was generated (Unix s).
    pub created_unix: i64,
    /// Its stored size in bytes.
    pub size: u64,
    /// The report.
    pub item: T,
}

/// The device's misbehaviour-report store ([SM-MBR] §2.2).
#[derive(Debug, Clone)]
pub struct MbrQueue<T> {
    /// The device-specific storage quota, bytes.
    pub quota: u64,
    /// Reports older than this (seconds) may be deleted unsent (03 §8.1; [CAMP122]
    /// §5.2.8.4 step 5: one week).
    pub max_age_s: i64,
    /// The Eastern hour of the daily re-weighting ([SM-MBR] §2.2: 5 am).
    pub decay_hour_et: u32,
    entries: Vec<QueuedReport<T>>,
    seq: u64,
    decayed_to: Option<i64>,
    /// Reports deleted to keep within the quota.
    pub evicted: u64,
    /// Reports deleted for age.
    pub expired: u64,
}

/// The daily re-weighting instant, 05:00 Eastern.
pub const DECAY_HOUR_ET: u32 = 5;

impl<T> MbrQueue<T> {
    /// An empty store with `quota` bytes.
    #[must_use]
    pub fn new(quota: u64) -> MbrQueue<T> {
        MbrQueue {
            quota,
            max_age_s: 7 * 86_400,
            decay_hour_et: DECAY_HOUR_ET,
            entries: Vec::new(),
            seq: 0,
            decayed_to: None,
            evicted: 0,
            expired: 0,
        }
    }

    /// Stored reports.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing is stored.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Bytes stored.
    #[must_use]
    pub fn used(&self) -> u64 {
        self.entries.iter().map(|e| e.size).sum()
    }

    /// The stored reports, in no particular order.
    pub fn iter(&self) -> impl Iterator<Item = &QueuedReport<T>> {
        self.entries.iter()
    }

    /// Applies every 05:00-Eastern multiplication between the last call and `now_unix`.
    pub fn decay_to(&mut self, now_unix: i64) {
        let from = self.decayed_to.unwrap_or(now_unix);
        if now_unix > from {
            let n =
                crate::scms::calendar::eastern_times_between(from, now_unix, self.decay_hour_et, 0);
            if n > 0 {
                for e in &mut self.entries {
                    e.weight *= e.multiplier.powi(n as i32);
                }
            }
        }
        self.decayed_to = Some(self.decayed_to.map_or(now_unix, |d| d.max(now_unix)));
    }

    /// Stores a report; then, while over quota, deletes the lowest weight (older first
    /// on ties) — possibly the new report itself. Returns the deleted reports.
    pub fn push(&mut self, item: T, size: u64, w: Weighting, now_unix: i64) -> Vec<T> {
        self.push_entry(item, size, w, now_unix)
            .into_iter()
            .map(|e| e.item)
            .collect()
    }

    /// [`Self::push`], returning the deleted reports whole (with their weights).
    ///
    /// A report generated at `now_unix` before an instant the store has already been
    /// re-weighted to (it waited before being stored, as for a signer) takes the daily
    /// multiplications it lived through: §2.2 weighs a report from its generation.
    pub fn push_entry(
        &mut self,
        item: T,
        size: u64,
        w: Weighting,
        now_unix: i64,
    ) -> Vec<QueuedReport<T>> {
        self.decay_to(now_unix);
        let mut weight = w.initial;
        if let Some(d) = self.decayed_to
            && d > now_unix
        {
            let n =
                crate::scms::calendar::eastern_times_between(now_unix, d, self.decay_hour_et, 0);
            weight *= w.daily_multiplier.powi(n as i32);
        }
        self.seq += 1;
        self.entries.push(QueuedReport {
            weight,
            multiplier: w.daily_multiplier,
            seq: self.seq,
            created_unix: now_unix,
            size,
            item,
        });
        self.evict_over_quota()
    }

    /// Puts back a report taken by [`Self::take_upload_order`] whose upload was not
    /// confirmed, with the weight, order and age it had: it is the same stored report,
    /// not a new one ([SM-MBR] §2.2 deletes a report only after its TCP-confirmed
    /// upload). Returns what the quota then deletes.
    pub fn reinsert(&mut self, e: QueuedReport<T>) -> Vec<QueuedReport<T>> {
        self.seq = self.seq.max(e.seq);
        self.entries.push(e);
        self.evict_over_quota()
    }

    /// Takes every stored report out in upload order (highest weight first, newer first
    /// on ties), after the re-weightings up to `now_unix`. The caller puts back with
    /// [`Self::reinsert`] each one whose upload it could not confirm.
    pub fn take_upload_order(&mut self, now_unix: i64) -> Vec<QueuedReport<T>> {
        self.decay_to(now_unix);
        let mut v: Vec<_> = self.entries.drain(..).collect();
        v.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(b.seq.cmp(&a.seq))
        });
        v
    }

    /// Deletes the stored report `pick` selects, its upload now confirmed ([SM-MBR] §2.2),
    /// returning it whole; `None` when none matches. The others keep their order.
    pub fn remove_where(&mut self, pick: impl Fn(&T) -> bool) -> Option<QueuedReport<T>> {
        let i = self.entries.iter().position(|e| pick(&e.item))?;
        Some(self.entries.remove(i))
    }

    /// Whether a stored report matches `pick`.
    pub fn contains_where(&self, pick: impl Fn(&T) -> bool) -> bool {
        self.entries.iter().any(|e| pick(&e.item))
    }

    /// Deletes unsent reports older than [`Self::max_age_s`], returning them whole.
    pub fn expire_entries(&mut self, now_unix: i64) -> Vec<QueuedReport<T>> {
        let max = self.max_age_s;
        let (old, keep): (Vec<_>, Vec<_>) = self
            .entries
            .drain(..)
            .partition(|e| now_unix - e.created_unix > max);
        self.entries = keep;
        self.expired += old.len() as u64;
        old
    }

    fn evict_over_quota(&mut self) -> Vec<QueuedReport<T>> {
        let mut out = Vec::new();
        while self.used() > self.quota && !self.entries.is_empty() {
            let i = self
                .entries
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.weight
                        .partial_cmp(&b.weight)
                        .unwrap_or(core::cmp::Ordering::Equal)
                        .then(a.seq.cmp(&b.seq))
                })
                .map(|(i, _)| i)
                .expect("non-empty");
            out.push(self.entries.swap_remove(i));
            self.evicted += 1;
        }
        out
    }

    /// The report to upload next: the highest weight, newer first on ties.
    #[must_use]
    pub fn next_upload(&self) -> Option<&QueuedReport<T>> {
        self.entries.iter().max_by(|a, b| {
            a.weight
                .partial_cmp(&b.weight)
                .unwrap_or(core::cmp::Ordering::Equal)
                .then(a.seq.cmp(&b.seq))
        })
    }

    /// The sequence number of the report stored last.
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.seq
    }

    /// Takes report `seq` out whole (for an upload attempt; [`Self::reinsert`] puts it
    /// back if the upload is not confirmed).
    pub fn take_entry(&mut self, seq: u64) -> Option<QueuedReport<T>> {
        let i = self.entries.iter().position(|e| e.seq == seq)?;
        Some(self.entries.swap_remove(i))
    }

    /// The upload of report `seq` was TCP-confirmed: it is deleted.
    pub fn confirm(&mut self, seq: u64) -> Option<T> {
        let i = self.entries.iter().position(|e| e.seq == seq)?;
        Some(self.entries.swap_remove(i).item)
    }

    /// Deletes unsent reports older than [`Self::max_age_s`].
    pub fn expire(&mut self, now_unix: i64) -> Vec<T> {
        self.expire_entries(now_unix)
            .into_iter()
            .map(|e| e.item)
            .collect()
    }
}
