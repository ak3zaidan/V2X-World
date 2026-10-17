//! The IEEE 1609.2.1 certificate trust list (CTL) as the US SCMS deploys it: a
//! `MultiSignedCtl` of type `fullIeeeCtl` whose `FullIeeeTbsCtl` lists the approved
//! elector certificates (`HashedId48`) and root CA certificates (`HashedId32`), signed by
//! the SCMS Manager electors, any 3 of 5, with ECDSA P-384 / SHA-384 (01 §3.1; 03 §10;
//! gap rows 14 and 31). Also the two 1609.2.1 containers that carry a CTL to devices:
//! the certificate chain file (`CertificateChain`, the CCF, 1609.2.1 §8.4) and the
//! composite CRL (`CompositeCrl`).
//!
//! # Wire form (1609.2.1-2022 `Ieee1609Dot2Dot1CertManagement`, `…Protocol`)
//!
//! ```text
//! MultiSignedCtlSpdu = Ieee1609Dot2Data{ v3, unsecuredData( COER ScmsPdu{ v2,
//!     cert: multiSignedCtl MultiSignedCtl{
//!         type 1 (fullIeeeCtl),
//!         tbsCtl   open type  = FullIeeeTbsCtl,
//!         unsigned open type  = SequenceOfCertificate   (the root certificates),
//!         signatures SEQUENCE OF CtlSignatureSpdu } }) }
//! CtlSignatureSpdu = Ieee1609Dot2Data{ signedData, hashId sha384, psid 35,
//!     payload unsecuredData( ScmsPdu{ cert: tbsCtlSignature ToBeSignedCtlSignature{
//!         ctlSeriesId, ctlType, sequenceNumber, tbsCtlHash = SHA-384(COER tbsCtl) } }),
//!     signer certificate (the elector's self-signed explicit P-384 certificate) }
//! ```
//!
//! The two open types are length-prefixed COER (X.696 §30); the hash in `tbsCtlHash` is
//! over the `FullIeeeTbsCtl` bytes *inside* the length prefix. That reading, the
//! canonical-hash rules of [`crate::sig1609`], and every signature rule below are
//! confirmed by the real SCMS Manager production CTL (2025-08-13) and test CTL
//! (2025-07-17) in `tests/real_artefacts.rs`, and by the A0 oracle (`oracle.py real`).
//!
//! # Acceptance rule ([`verify_ctl`])
//!
//! A device holds a [`CtlTrust`]: the CtlSeriesId, the elector `HashedId48`s it trusts,
//! the quorum, and the last sequence number. Initially that is the five elector
//! certificates it was provisioned with and quorum 3 (Profile §2.1.15; Elector Tech Spec
//! §2.2). A new CTL is accepted when, for at least `quorum` **distinct** trusted
//! electors, the elector certificate is self-signed P-384, carries PSID 35 with the
//! elector SSP, is valid at `effectiveDate`, the `CtlSignatureSpdu` signature verifies,
//! and its `ToBeSignedCtlSignature` names this CTL (series, type, sequence) with
//! `tbsCtlHash` equal to SHA-384 of the transmitted `tbsCtl`. The accepted CTL then sets
//! the next trust: its `electorApprove`, its `quorum` (1609.2.1: "the number of the
//! electors required to sign the *next* CTL"; absent → unchanged), its sequence number,
//! and its `rootCaApprove` as the trust anchors.

use std::collections::BTreeSet;

use rasn::types::{Any, Integer};
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, CertificateBase, CertificateId, CertificateType, Ieee1609Dot2Content,
    Ieee1609Dot2Data, IssuerIdentifier, SequenceOfCertificate, ToBeSignedCertificate,
    VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    CrlSeries, Duration, HashedId3, HashedId32, HashedId48, Hostname, Opaque, Psid, PsidSsp,
    SequenceOfPsidSsp, ServiceSpecificPermissions, Time32, Uint8, Uint16, Uint32, ValidityPeriod,
};
use v2xw_msg::sec_types::ieee1609_dot2_crl::SecuredCrl;
use v2xw_msg::sec_types::ieee1609_dot2_dot1_cert_management::{
    CertManagementPdu, CertificateChain, CompositeCrl, CtlElectorEntry, CtlRootCaEntry,
    CtlSequenceNumber, CtlSeriesId, FullIeeeTbsCtl, Ieee1609dot2dot1MsctlType, MultiSignedCtl,
    ToBeSignedCtlSignature,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::{
    CtlSignatureSpdu, ElectorSsp, MultiSignedCtlSpdu, ScmsPdu, ScmsPduContent, SecurityMgmtSsp,
};

use crate::sig1609::{
    self, Hash1609, OpenedSigned, SigError, Signer1609, SpduSigner, cert_hashed_id32,
    cert_hashed_id48, coer, open_signed, verify_explicit_cert, verify_opened,
};

/// The US CtlSeriesId, `03 48 00 00 00 00 00 00` (SCMS Manager Profile §6.1; 01 §3.1).
pub const US_CTL_SERIES_ID: [u8; 8] = [0x03, 0x48, 0, 0, 0, 0, 0, 0];
/// `fullIeeeCtl`, the only `Ieee1609dot2dot1MsctlType` 1609.2.1-2022 defines.
pub const FULL_IEEE_CTL: u8 = 1;
/// SecurityMgmtPsid (1609.2.1 `Ieee1609Dot2Dot1Protocol`).
pub const SECURITY_MGMT_PSID: u64 = 35;
/// Electors in the US system (Elector Tech Spec §2.2; Provider Req. §1.2.2).
pub const US_ELECTORS: usize = 5;
/// Signatures a US CTL needs (Elector Tech Spec §2.4: "any 3 of the 5").
pub const US_QUORUM: u32 = 3;
/// The 1609.2 year, in seconds (1609.2 `Duration`: a year is 31 556 952 s).
pub const YEAR_S: u64 = 31_556_952;

/// Why a CTL, chain file or composite CRL could not be read or built.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CtlError {
    /// Decoding failed at the named layer.
    #[error("decode {layer}: {detail}")]
    Decode {
        /// Which structure.
        layer: &'static str,
        /// The decoder's message.
        detail: String,
    },
    /// The structure decoded but is not what this layer requires.
    #[error("not a {0}")]
    Shape(&'static str),
    /// A signature or hashing step failed.
    #[error(transparent)]
    Sig(#[from] SigError),
}

fn decode<T: rasn::Decode>(layer: &'static str, b: &[u8]) -> Result<T, CtlError> {
    rasn::coer::decode(b).map_err(|e| CtlError::Decode {
        layer,
        detail: e.to_string(),
    })
}

/// The duration of a 1609.2 `Duration`, in seconds (sub-second units round down).
pub fn duration_s(d: &Duration) -> u64 {
    match d {
        Duration::microseconds(v) => u64::from(v.0) / 1_000_000,
        Duration::milliseconds(v) => u64::from(v.0) / 1000,
        Duration::seconds(v) => u64::from(v.0),
        Duration::minutes(v) => u64::from(v.0) * 60,
        Duration::hours(v) => u64::from(v.0) * 3600,
        Duration::sixtyHours(v) => u64::from(v.0) * 216_000,
        Duration::years(v) => u64::from(v.0) * YEAR_S,
    }
}

/// `[start, start + duration)` of a certificate, in Time32 seconds.
pub fn validity(cert: &Certificate) -> (u64, u64) {
    let v = &cert.0.to_be_signed.validity_period;
    let start = u64::from(v.start.0.0);
    (start, start + duration_s(&v.duration))
}

/// The certificate's `id.name`, if it has one.
pub fn cert_name(cert: &Certificate) -> Option<&str> {
    match &cert.0.to_be_signed.id {
        CertificateId::name(Hostname(n)) => Some(n.as_str()),
        _ => None,
    }
}

/// The SSP a certificate carries for `psid`, as opaque bytes. `None` when the PSID is
/// absent; `Some(None)` when present without an SSP.
pub fn app_ssp(cert: &Certificate, psid: u64) -> Option<Option<Vec<u8>>> {
    let perms = cert.0.to_be_signed.app_permissions.as_ref()?;
    perms
        .0
        .iter()
        .find(|p| u64::try_from(&p.psid.0).ok() == Some(psid))
        .map(|p| match &p.ssp {
            Some(ServiceSpecificPermissions::opaque(o)) => Some(o.to_vec()),
            _ => None,
        })
}

/// The `SecurityMgmtSsp` a certificate holds for PSID 35, decoded.
pub fn security_mgmt_ssp(cert: &Certificate) -> Option<SecurityMgmtSsp> {
    let ssp = app_ssp(cert, SECURITY_MGMT_PSID)??;
    rasn::coer::decode(&ssp).ok()
}

/// A decoded `ToBeSignedCtlSignature`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtlSignatureFields {
    /// `ctlSeriesId`.
    pub series: [u8; 8],
    /// `ctlType`.
    pub ctl_type: u8,
    /// `sequenceNumber`.
    pub sequence: u16,
    /// `tbsCtlHash`.
    pub tbs_ctl_hash: [u8; 48],
}

/// One elector signature, opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtlSignature {
    /// The signed SPDU, taken apart.
    pub opened: OpenedSigned,
    /// Its `ToBeSignedCtlSignature`; `None` if the payload is not one.
    pub fields: Option<CtlSignatureFields>,
}

/// A decoded `MultiSignedCtl` of type `fullIeeeCtl`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ctl {
    /// The CTL contents.
    pub tbs: FullIeeeTbsCtl,
    /// The `tbsCtl` bytes as transmitted (inside the open-type length): what
    /// `tbsCtlHash` covers.
    pub tbs_coer: Vec<u8>,
    /// The `unsigned` certificates (the root CA certificates the CTL approves).
    pub certificates: Vec<Certificate>,
    /// The elector signatures.
    pub signatures: Vec<CtlSignature>,
    /// The `MultiSignedCtl` as decoded, for re-wrapping in a CCF or composite CRL.
    pub raw: MultiSignedCtl,
}

impl Ctl {
    /// `ctlSeriesId`.
    pub fn series(&self) -> [u8; 8] {
        let mut b = [0u8; 8];
        b.copy_from_slice(&self.tbs.ctl_series_id.0[..]);
        b
    }

    /// `sequenceNumber`.
    pub fn sequence(&self) -> u16 {
        self.tbs.sequence_number.0
    }

    /// `effectiveDate` (Time32).
    pub fn effective_date(&self) -> u32 {
        self.tbs.effective_date.0.0
    }

    /// `quorum`, if present and in range. A CTL may omit it (it is an extension
    /// addition; the 2023 SCMS Manager test CTL does), and then the next CTL's quorum is
    /// this one's (1609.2.1 `FullIeeeTbsCtl` note).
    pub fn quorum(&self) -> Option<u32> {
        self.tbs.quorum.as_ref().and_then(|q| u32::try_from(q).ok())
    }

    /// `electorApprove`.
    pub fn elector_approve(&self) -> Vec<[u8; 48]> {
        self.tbs.elector_approve.iter().map(|e| arr(&e.0.0[..])).collect()
    }

    /// `rootCaApprove`.
    pub fn root_ca_approve(&self) -> Vec<[u8; 32]> {
        self.tbs.root_ca_approve.iter().map(|e| arr(&e.0.0[..])).collect()
    }

    /// `electorRemove`.
    pub fn elector_remove(&self) -> Vec<[u8; 48]> {
        self.tbs.elector_remove.iter().map(|e| arr(&e.0.0[..])).collect()
    }

    /// `rootCaRemove`.
    pub fn root_ca_remove(&self) -> Vec<[u8; 32]> {
        self.tbs.root_ca_remove.iter().map(|e| arr(&e.0.0[..])).collect()
    }

    /// The elector certificates attached to the signatures.
    pub fn elector_certificates(&self) -> Vec<&Certificate> {
        self.signatures
            .iter()
            .filter_map(|s| match &s.opened.signer {
                SpduSigner::Certificate(c) => Some(c),
                _ => None,
            })
            .collect()
    }
}

fn arr<const N: usize>(b: &[u8]) -> [u8; N] {
    let mut o = [0u8; N];
    o.copy_from_slice(b);
    o
}

fn scms_pdu_of(data: &Ieee1609Dot2Data) -> Result<ScmsPdu, CtlError> {
    let Ieee1609Dot2Content::unsecuredData(Opaque(b)) = &data.content else {
        return Err(CtlError::Shape("unsecuredData SPDU"));
    };
    decode("ScmsPdu", b)
}

fn cert_pdu(pdu: ScmsPdu) -> Result<CertManagementPdu, CtlError> {
    if pdu.version.0 != 2 {
        return Err(CtlError::Shape("ScmsPdu version 2"));
    }
    match pdu.content {
        ScmsPduContent::cert(c) => Ok(c),
        _ => Err(CtlError::Shape("ScmsPdu cert content")),
    }
}

fn unsecured_cert_pdu(bytes: &[u8]) -> Result<CertManagementPdu, CtlError> {
    let data: Ieee1609Dot2Data = decode("Ieee1609Dot2Data", bytes)?;
    cert_pdu(scms_pdu_of(&data)?)
}

/// Opens a `MultiSignedCtl`: decodes both open types and every elector signature.
pub fn open_multi_signed_ctl(m: &MultiSignedCtl) -> Result<Ctl, CtlError> {
    if m.r_type.0 != FULL_IEEE_CTL {
        return Err(CtlError::Shape("fullIeeeCtl MultiSignedCtl"));
    }
    let tbs_coer = m.tbs_ctl.as_bytes().to_vec();
    let tbs: FullIeeeTbsCtl = decode("FullIeeeTbsCtl", &tbs_coer)?;
    let SequenceOfCertificate(certificates) = decode("SequenceOfCertificate", m.unsigned.as_bytes())?;
    let mut signatures = Vec::with_capacity(m.signatures.len());
    for s in &m.signatures {
        let data = Ieee1609Dot2Data::new(s.protocol_version.clone(), s.content.clone());
        let opened = open_signed(&data).ok_or(CtlError::Shape("signed CtlSignatureSpdu"))?;
        let fields = rasn::coer::decode::<ScmsPdu>(&opened.payload)
            .ok()
            .and_then(|p| cert_pdu(p).ok())
            .and_then(|c| match c {
                CertManagementPdu::tbsCtlSignature(t) => Some(CtlSignatureFields {
                    series: arr(&t.ctl_series_id.0[..]),
                    ctl_type: t.ctl_type.0,
                    sequence: t.sequence_number.0,
                    tbs_ctl_hash: arr(&t.tbs_ctl_hash.0[..]),
                }),
                _ => None,
            });
        signatures.push(CtlSignature { opened, fields });
    }
    Ok(Ctl {
        tbs,
        tbs_coer,
        certificates,
        signatures,
        raw: m.clone(),
    })
}

/// Decodes a `MultiSignedCtlSpdu` file (`production_ctl.oer`, the test CTL `.oer`).
pub fn decode_ctl_spdu(bytes: &[u8]) -> Result<Ctl, CtlError> {
    match unsecured_cert_pdu(bytes)? {
        CertManagementPdu::multiSignedCtl(m) => open_multi_signed_ctl(&m),
        _ => Err(CtlError::Shape("multiSignedCtl")),
    }
}

/// Decodes a certificate chain file (`CertificateChainSpdu`, the CCF): the home CTL and
/// the other CA/MA certificates.
pub fn decode_certificate_chain(bytes: &[u8]) -> Result<(Ctl, Vec<Certificate>), CtlError> {
    match unsecured_cert_pdu(bytes)? {
        CertManagementPdu::certificateChain(c) => {
            let ctl = match cert_pdu(scms_pdu_of(&spdu_data(&c.home_ctl))?)? {
                CertManagementPdu::multiSignedCtl(m) => open_multi_signed_ctl(&m)?,
                _ => return Err(CtlError::Shape("multiSignedCtl home CTL")),
            };
            Ok((ctl, c.others))
        }
        _ => Err(CtlError::Shape("certificateChain")),
    }
}

/// Decodes a `CompositeCrlSpdu`: the CRLs and the home CTL.
pub fn decode_composite_crl(bytes: &[u8]) -> Result<(Vec<SecuredCrl>, Ctl), CtlError> {
    match unsecured_cert_pdu(bytes)? {
        CertManagementPdu::compositeCrl(c) => {
            let ctl = match cert_pdu(scms_pdu_of(&spdu_data(&c.home_ctl))?)? {
                CertManagementPdu::multiSignedCtl(m) => open_multi_signed_ctl(&m)?,
                _ => return Err(CtlError::Shape("multiSignedCtl home CTL")),
            };
            Ok((c.crl, ctl))
        }
        _ => Err(CtlError::Shape("compositeCrl")),
    }
}

fn spdu_data(s: &MultiSignedCtlSpdu) -> Ieee1609Dot2Data {
    Ieee1609Dot2Data::new(s.protocol_version.clone(), s.content.clone())
}

fn unsecured(pdu: CertManagementPdu) -> Result<Ieee1609Dot2Data, CtlError> {
    let p = ScmsPdu::new(Uint8(2), ScmsPduContent::cert(pdu));
    Ok(Ieee1609Dot2Data::new(
        Uint8(3),
        Ieee1609Dot2Content::unsecuredData(Opaque(coer(&p)?.into())),
    ))
}

fn ctl_spdu(m: &MultiSignedCtl) -> Result<MultiSignedCtlSpdu, CtlError> {
    let d = unsecured(CertManagementPdu::multiSignedCtl(m.clone()))?;
    Ok(MultiSignedCtlSpdu::new(d.protocol_version, d.content))
}

/// Encodes a `MultiSignedCtlSpdu`.
pub fn encode_ctl_spdu(m: &MultiSignedCtl) -> Result<Vec<u8>, CtlError> {
    Ok(coer(&unsecured(CertManagementPdu::multiSignedCtl(m.clone()))?)?)
}

/// Encodes a certificate chain file (`CertificateChainSpdu`).
pub fn encode_certificate_chain(
    home_ctl: &MultiSignedCtl,
    others: Vec<Certificate>,
) -> Result<Vec<u8>, CtlError> {
    let c = CertificateChain::new(ctl_spdu(home_ctl)?, others);
    Ok(coer(&unsecured(CertManagementPdu::certificateChain(c))?)?)
}

/// Encodes a `CompositeCrlSpdu`: every series' CRLs plus the home CTL, as the RA hands
/// them to an EE on every connection (03 §10; gap row 30).
pub fn encode_composite_crl(
    crls: Vec<SecuredCrl>,
    home_ctl: &MultiSignedCtl,
) -> Result<Vec<u8>, CtlError> {
    let c = CompositeCrl::new(crls, ctl_spdu(home_ctl)?);
    Ok(coer(&unsecured(CertManagementPdu::compositeCrl(c))?)?)
}

// ---------------------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------------------

/// What a device trusts before it installs a CTL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtlTrust {
    /// The CtlSeriesId it follows.
    pub series: [u8; 8],
    /// `HashedId48` of every elector it trusts.
    pub electors: BTreeSet<[u8; 48]>,
    /// Signatures the next CTL needs.
    pub quorum: u32,
    /// The installed CTL's sequence number; `None` before the first install.
    pub sequence: Option<u16>,
    /// `HashedId32` of the approved roots (the trust anchors).
    pub roots: BTreeSet<[u8; 32]>,
}

impl CtlTrust {
    /// A US device as provisioned (Profile §2.1.15): the elector certificates' hashes,
    /// quorum 3, CtlSeriesId `0348000000000000`, no CTL installed yet.
    pub fn us_bootstrap(electors: impl IntoIterator<Item = [u8; 48]>) -> CtlTrust {
        CtlTrust {
            series: US_CTL_SERIES_ID,
            electors: electors.into_iter().collect(),
            quorum: US_QUORUM,
            sequence: None,
            roots: BTreeSet::new(),
        }
    }
}

/// The outcome for one elector signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElectorCheck {
    /// The elector's `HashedId48`, when its certificate is available.
    pub elector: Option<[u8; 48]>,
    /// The elector's `id.name`.
    pub name: Option<String>,
    /// Its `HashedId48` is in the trust set.
    pub trusted: bool,
    /// Its certificate is explicit, P-384, self-signed with SHA-384, and the
    /// self-signature verifies.
    pub cert_ok: bool,
    /// It carries PSID 35 with the `elector` SecurityMgmtSsp.
    pub ssp_ok: bool,
    /// Its validity covers the CTL's `effectiveDate`.
    pub valid_at_effective_date: bool,
    /// The `CtlSignatureSpdu` is psid 35, sha384, and its signature verifies.
    pub signature_ok: bool,
    /// `ToBeSignedCtlSignature` names this CTL and `tbsCtlHash = SHA-384(tbsCtl)`.
    pub binds_this_ctl: bool,
}

impl ElectorCheck {
    /// Every check passed.
    pub fn ok(&self) -> bool {
        self.trusted
            && self.cert_ok
            && self.ssp_ok
            && self.valid_at_effective_date
            && self.signature_ok
            && self.binds_this_ctl
    }
}

/// The verdict on a CTL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CtlVerdict {
    /// One entry per signature, in order.
    pub electors: Vec<ElectorCheck>,
    /// Distinct trusted electors whose signature passed every check.
    pub valid_signatures: u32,
    /// The quorum applied (the prior trust's).
    pub quorum: u32,
    /// The series matches the trust's.
    pub series_ok: bool,
    /// The type fields are `fullIeeeCtl` in both places.
    pub type_ok: bool,
    /// The sequence number advances past the installed one.
    pub sequence_ok: bool,
}

impl CtlVerdict {
    /// The CTL may be installed.
    pub fn accepted(&self) -> bool {
        self.series_ok && self.type_ok && self.sequence_ok && self.valid_signatures >= self.quorum
    }
}

/// Is this an elector certificate per 1609.2.1 §7.7.3.7 as the US uses it: explicit,
/// self-signed with SHA-384, a P-384 verification key, and the self-signature verifies?
pub fn elector_cert_ok(cert: &Certificate) -> bool {
    cert.0.r_type == CertificateType::explicit
        && matches!(
            cert.0.issuer,
            IssuerIdentifier::R_self(v2xw_msg::sec_types::ieee1609_dot2_base_types::HashAlgorithm::sha384)
        )
        && matches!(sig1609::explicit_key(cert), Some((Hash1609::Sha384, _)))
        && verify_explicit_cert(cert, None)
}

/// Verifies a CTL against what the device trusts (see the module documentation).
pub fn verify_ctl(ctl: &Ctl, trust: &CtlTrust) -> CtlVerdict {
    let tbs_hash = Hash1609::Sha384.digest(&ctl.tbs_coer);
    let eff = u64::from(ctl.effective_date());
    let mut counted: BTreeSet<[u8; 48]> = BTreeSet::new();
    let mut electors = Vec::with_capacity(ctl.signatures.len());
    for s in &ctl.signatures {
        let cert = match &s.opened.signer {
            SpduSigner::Certificate(c) => Some(c),
            _ => None,
        };
        let id48 = cert.and_then(|c| cert_hashed_id48(c).ok());
        let (start, end) = cert.map(validity).unwrap_or((1, 0));
        let check = ElectorCheck {
            elector: id48,
            name: cert.and_then(cert_name).map(str::to_owned),
            trusted: id48.is_some_and(|h| trust.electors.contains(&h)),
            cert_ok: cert.is_some_and(elector_cert_ok),
            ssp_ok: cert.is_some_and(|c| {
                matches!(security_mgmt_ssp(c), Some(SecurityMgmtSsp::elector(e)) if e.version.0 == 2)
            }),
            valid_at_effective_date: start <= eff && eff < end,
            signature_ok: s.opened.psid == SECURITY_MGMT_PSID
                && s.opened.hash == Hash1609::Sha384
                && cert.is_some_and(|c| verify_opened(&s.opened, c)),
            binds_this_ctl: s.fields.as_ref().is_some_and(|f| {
                f.series == ctl.series()
                    && f.ctl_type == FULL_IEEE_CTL
                    && f.sequence == ctl.sequence()
                    && f.tbs_ctl_hash[..] == tbs_hash[..]
            }),
        };
        if check.ok() {
            counted.insert(id48.expect("ok implies a certificate"));
        }
        electors.push(check);
    }
    CtlVerdict {
        electors,
        valid_signatures: counted.len() as u32,
        quorum: trust.quorum,
        series_ok: ctl.series() == trust.series,
        type_ok: ctl.raw.r_type.0 == FULL_IEEE_CTL && ctl.tbs.r_type.0 == FULL_IEEE_CTL,
        sequence_ok: trust.sequence.is_none_or(|prev| ctl.sequence() > prev),
    }
}

/// The trust a device holds after installing `ctl` (which [`verify_ctl`] accepted).
pub fn trust_after(ctl: &Ctl, prior: &CtlTrust) -> CtlTrust {
    CtlTrust {
        series: ctl.series(),
        electors: ctl.elector_approve().into_iter().collect(),
        quorum: ctl.quorum().unwrap_or(prior.quorum),
        sequence: Some(ctl.sequence()),
        roots: ctl.root_ca_approve().into_iter().collect(),
    }
}

/// One root certificate carried in a CTL's `unsigned` material.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootCheck {
    /// Its `id.name`.
    pub name: Option<String>,
    /// Its `HashedId32`.
    pub hashed_id32: [u8; 32],
    /// That hash is in `rootCaApprove`.
    pub approved: bool,
    /// SHA-256 of its canonical COER is in `rootCaApprove`. This differs from
    /// `approved` only for a root identified by SHA-384: the 1609.2.1 note says its entry
    /// is the low-order 32 bytes of the SHA-384 hash, but the SCMS Manager test CTLs of
    /// 2025-03-18 approves the Eviden P-384 root by its SHA-256 instead (the 2024-04-19 one
    /// used the low-order SHA-384 bytes, and the 2025-07-17 one does not approve it)
    /// (`tests/real_artefacts.rs`). Reported, never used by [`trust_after`].
    pub approved_by_sha256: bool,
    /// Its self-signature verifies.
    pub self_signed_ok: bool,
}

/// Checks every root certificate in the CTL against `rootCaApprove`.
pub fn check_roots(ctl: &Ctl) -> Result<Vec<RootCheck>, CtlError> {
    let approved: BTreeSet<[u8; 32]> = ctl.root_ca_approve().into_iter().collect();
    ctl.certificates
        .iter()
        .map(|c| {
            let h = cert_hashed_id32(c)?;
            Ok(RootCheck {
                name: cert_name(c).map(str::to_owned),
                hashed_id32: h,
                approved: approved.contains(&h),
                approved_by_sha256: approved
                    .contains(&arr::<32>(&Hash1609::Sha256.digest(&sig1609::canonical_cert_coer(c)?))),
                self_signed_ok: verify_explicit_cert(c, None),
            })
        })
        .collect()
}

/// How one certificate of a chain file relates to its issuer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainLink {
    /// A self-signed root whose signature verifies.
    Root,
    /// An explicit certificate whose issuer is in the file (or among the CTL roots) and
    /// whose signature verifies under it.
    Explicit {
        /// The issuer's `id.name`.
        issuer: Option<String>,
    },
    /// An implicit certificate: its key is reconstructed, not verified (ECQV, WP-A3).
    Implicit,
    /// The issuer is not in the file.
    IssuerMissing,
    /// The signature does not verify.
    BadSignature,
}

/// The verdict on one certificate of a chain file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainCheck {
    /// Its `id.name`.
    pub name: Option<String>,
    /// How it links to its issuer.
    pub link: ChainLink,
    /// Following issuers reaches a root whose `HashedId32` is on the CTL.
    pub anchored: bool,
}

/// Checks every certificate in a chain file: each links to its issuer by `HashedId8`,
/// its signature verifies, and its issuers lead to a root approved by `ctl`.
pub fn check_chain(ctl: &Ctl, others: &[Certificate]) -> Result<Vec<ChainCheck>, CtlError> {
    let pool: Vec<&Certificate> = others.iter().chain(ctl.certificates.iter()).collect();
    let mut by_id8 = std::collections::BTreeMap::new();
    for c in &pool {
        by_id8.insert(sig1609::cert_hashed_id8(c)?, *c);
    }
    let approved: BTreeSet<[u8; 32]> = ctl.root_ca_approve().into_iter().collect();
    let issuer_of = |c: &Certificate| -> Option<&Certificate> {
        match &c.0.issuer {
            IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
                by_id8.get(&arr::<8>(&d.0[..])).copied()
            }
            _ => None,
        }
    };
    let mut out = Vec::with_capacity(others.len());
    for c in others {
        let link = if matches!(c.0.issuer, IssuerIdentifier::R_self(_)) {
            if verify_explicit_cert(c, None) {
                ChainLink::Root
            } else {
                ChainLink::BadSignature
            }
        } else {
            match issuer_of(c) {
                None => ChainLink::IssuerMissing,
                Some(_) if c.0.r_type == CertificateType::implicit => ChainLink::Implicit,
                Some(i) if verify_explicit_cert(c, Some(i)) => ChainLink::Explicit {
                    issuer: cert_name(i).map(str::to_owned),
                },
                Some(_) => ChainLink::BadSignature,
            }
        };
        // Walk to the root (bounded: a chain is at most a handful deep).
        let mut cur = c;
        let mut anchored = false;
        for _ in 0..8 {
            if matches!(cur.0.issuer, IssuerIdentifier::R_self(_)) {
                anchored = cert_hashed_id32(cur).is_ok_and(|h| approved.contains(&h));
                break;
            }
            match issuer_of(cur) {
                Some(i) => cur = i,
                None => break,
            }
        }
        out.push(ChainCheck {
            name: cert_name(c).map(str::to_owned),
            link,
            anchored,
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------------------
// Building (the simulator's own electors and CTLs)
// ---------------------------------------------------------------------------------------

/// An elector: its P-384 key and its self-signed certificate.
#[derive(Debug, Clone)]
pub struct Elector {
    /// The signing key.
    pub key: Signer1609,
    /// The explicit, self-signed certificate.
    pub cert: Certificate,
}

impl Elector {
    /// `HashedId48` of the certificate.
    pub fn hashed_id48(&self) -> [u8; 48] {
        cert_hashed_id48(&self.cert).expect("an elector certificate is SHA-384 identified")
    }
}

fn opaque_ssp(ssp: &SecurityMgmtSsp) -> Result<ServiceSpecificPermissions, CtlError> {
    Ok(ServiceSpecificPermissions::opaque(coer(ssp)?.into()))
}

/// A self-signed explicit certificate: `issuer self(h)`, the key's curve, `cracaId
/// 000000`, `crlSeries 0`, no region, the given app permissions, signed over
/// `H(H(tbs) ‖ H(""))`. The form of the published elector and root certificates.
pub fn self_signed_certificate(
    name: &str,
    start: u32,
    years: u16,
    app_permissions: Vec<PsidSsp>,
    cert_issue_permissions: Option<
        v2xw_msg::sec_types::ieee1609_dot2::SequenceOfPsidGroupPermissions,
    >,
    key: &Signer1609,
) -> Result<Certificate, CtlError> {
    let tbs = ToBeSignedCertificate::new(
        CertificateId::name(Hostname(name.into())),
        HashedId3(rasn::types::FixedOctetString::from([0u8; 3])),
        CrlSeries(Uint16(0)),
        ValidityPeriod::new(Time32(Uint32(start)), Duration::years(Uint16(years))),
        None,
        None,
        Some(SequenceOfPsidSsp(app_permissions)),
        cert_issue_permissions,
        None,
        None,
        None,
        VerificationKeyIndicator::verificationKey(key.public_key()),
        None,
        None,
        None,
        None,
    );
    let sig = key.sign_1609(&coer(&tbs)?, b"");
    Ok(Certificate(CertificateBase::new(
        Uint8(3),
        CertificateType::explicit,
        IssuerIdentifier::R_self(key.hash().to_asn1()),
        tbs,
        Some(sig),
    )))
}

/// An elector certificate like SCMS Manager's: P-384, self-signed with SHA-384, PSID 35
/// with SSP `elector {version 2}` (`80 00 02`, as on the published test electors).
pub fn elector(name: &str, start: u32, years: u16, d: &[u8; 48]) -> Result<Elector, CtlError> {
    let key = Signer1609::p384(d).ok_or(CtlError::Shape("P-384 scalar in [1, n)"))?;
    let ssp = opaque_ssp(&SecurityMgmtSsp::elector(ElectorSsp::new(Uint8(2))))?;
    let perms = vec![PsidSsp::new(
        Psid(Integer::from(SECURITY_MGMT_PSID)),
        Some(ssp),
    )];
    let cert = self_signed_certificate(name, start, years, perms, None, &key)?;
    Ok(Elector { key, cert })
}

/// The contents of a `fullIeeeCtl`.
pub fn full_ieee_tbs_ctl(
    series: [u8; 8],
    sequence: u16,
    effective_date: u32,
    electors: &[[u8; 48]],
    roots: &[[u8; 32]],
    quorum: u32,
) -> FullIeeeTbsCtl {
    FullIeeeTbsCtl::new(
        Ieee1609dot2dot1MsctlType(FULL_IEEE_CTL),
        CtlSeriesId(rasn::types::FixedOctetString::from(series)),
        CtlSequenceNumber(sequence),
        Time32(Uint32(effective_date)),
        electors
            .iter()
            .map(|h| CtlElectorEntry(HashedId48(rasn::types::FixedOctetString::from(*h))))
            .collect(),
        Vec::new(),
        roots
            .iter()
            .map(|h| CtlRootCaEntry(HashedId32(rasn::types::FixedOctetString::from(*h))))
            .collect(),
        Vec::new(),
        Some(Integer::from(quorum)),
    )
}

/// One elector's `CtlSignatureSpdu` over `tbs_coer`.
pub fn sign_ctl(
    tbs: &FullIeeeTbsCtl,
    tbs_coer: &[u8],
    elector: &Elector,
) -> Result<CtlSignatureSpdu, CtlError> {
    let fields = ToBeSignedCtlSignature::new(
        tbs.ctl_series_id.clone(),
        Ieee1609dot2dot1MsctlType(FULL_IEEE_CTL),
        tbs.sequence_number.clone(),
        HashedId48(rasn::types::FixedOctetString::from(arr::<48>(
            &Hash1609::Sha384.digest(tbs_coer),
        ))),
    );
    let payload = coer(&ScmsPdu::new(
        Uint8(2),
        ScmsPduContent::cert(CertManagementPdu::tbsCtlSignature(fields)),
    ))?;
    let d = sig1609::sign_spdu(
        sig1609::header_psid_only(SECURITY_MGMT_PSID),
        payload,
        SpduSigner::Certificate(elector.cert.clone()),
        &elector.key,
    )?;
    Ok(CtlSignatureSpdu::new(d.protocol_version, d.content))
}

/// A `MultiSignedCtl` over `tbs`, carrying `roots` as unsigned material, signed by each
/// of `signers` in order.
pub fn build_ctl(
    tbs: &FullIeeeTbsCtl,
    roots: Vec<Certificate>,
    signers: &[&Elector],
) -> Result<MultiSignedCtl, CtlError> {
    let tbs_coer = coer(tbs)?;
    let unsigned = coer(&SequenceOfCertificate(roots))?;
    let signatures = signers
        .iter()
        .map(|e| sign_ctl(tbs, &tbs_coer, e))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MultiSignedCtl::new(
        Ieee1609dot2dot1MsctlType(FULL_IEEE_CTL),
        Any::new(tbs_coer),
        Any::new(unsigned),
        signatures,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn five() -> Vec<Elector> {
        (1..=5u8)
            .map(|i| {
                let years = 9 + 3 * u16::from(i); // 12/15/18/21/24 y, as the test electors
                elector(&format!("v2xelector{i}.sim"), 503_136_005, years, &[i; 48]).unwrap()
            })
            .collect()
    }

    fn root() -> Certificate {
        let k = Signer1609::p256(&[0x42; 32]).unwrap();
        self_signed_certificate(
            "rootca.sim",
            600_000_000,
            30,
            vec![PsidSsp::new(Psid(Integer::from(256u32)), None)],
            None,
            &k,
        )
        .unwrap()
    }

    fn ctl_signed_by(n: usize) -> (Vec<Elector>, Ctl) {
        let el = five();
        let ids: Vec<_> = el.iter().map(Elector::hashed_id48).collect();
        let r = root();
        let tbs = full_ieee_tbs_ctl(
            US_CTL_SERIES_ID,
            1,
            679_737_605,
            &ids,
            &[cert_hashed_id32(&r).unwrap()],
            US_QUORUM,
        );
        let signers: Vec<&Elector> = el.iter().take(n).collect();
        let m = build_ctl(&tbs, vec![r], &signers).unwrap();
        let bytes = encode_ctl_spdu(&m).unwrap();
        (el, decode_ctl_spdu(&bytes).unwrap())
    }

    /// Gap row 31: a CTL signed by 2 of 5 is rejected, by 3 of 5 accepted.
    #[test]
    fn two_of_five_is_rejected_three_of_five_accepted() {
        for (n, want) in [(2, false), (3, true), (5, true)] {
            let (el, ctl) = ctl_signed_by(n);
            let trust = CtlTrust::us_bootstrap(el.iter().map(Elector::hashed_id48));
            let v = verify_ctl(&ctl, &trust);
            assert_eq!(v.valid_signatures, n as u32, "{v:?}");
            assert_eq!(v.accepted(), want, "{n} of 5");
        }
    }

    /// The same elector signing three times counts once.
    #[test]
    fn one_elector_three_times_is_one_signature() {
        let el = five();
        let ids: Vec<_> = el.iter().map(Elector::hashed_id48).collect();
        let tbs = full_ieee_tbs_ctl(US_CTL_SERIES_ID, 1, 679_737_605, &ids, &[], 3);
        let m = build_ctl(&tbs, vec![], &[&el[0], &el[0], &el[0]]).unwrap();
        let ctl = decode_ctl_spdu(&encode_ctl_spdu(&m).unwrap()).unwrap();
        let v = verify_ctl(&ctl, &CtlTrust::us_bootstrap(ids));
        assert_eq!(v.valid_signatures, 1);
        assert!(!v.accepted());
    }

    /// An elector outside the trust set does not count; a changed tbsCtl breaks the
    /// binding; a stale sequence number is refused; roots are matched by HashedId32.
    #[test]
    fn untrusted_elector_tampered_tbs_and_stale_sequence_fail() {
        let (el, ctl) = ctl_signed_by(3);
        let mut trust = CtlTrust::us_bootstrap(el.iter().skip(1).map(Elector::hashed_id48));
        assert_eq!(verify_ctl(&ctl, &trust).valid_signatures, 2);
        trust = CtlTrust::us_bootstrap(el.iter().map(Elector::hashed_id48));
        let mut t = ctl.clone();
        t.tbs_coer[20] ^= 1;
        assert_eq!(verify_ctl(&t, &trust).valid_signatures, 0);
        trust.sequence = Some(1);
        assert!(!verify_ctl(&ctl, &trust).sequence_ok);
        let roots = check_roots(&ctl).unwrap();
        assert!(roots.iter().all(|r| r.approved && r.self_signed_ok), "{roots:?}");
        let next = trust_after(&ctl, &trust);
        assert_eq!((next.quorum, next.sequence, next.roots.len()), (3, Some(1), 1));
    }

    /// A P-384 self-signed certificate holding the *root* SSP, not the elector SSP, does
    /// not count as an elector signature even when it is in the trust set.
    #[test]
    fn a_signer_without_the_elector_ssp_does_not_count() {
        let mut el = five();
        let key = Signer1609::p384(&[0x33; 48]).unwrap();
        let ssp = opaque_ssp(&SecurityMgmtSsp::root(
            v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::RootCaSsp::new(Uint8(2)),
        ))
        .unwrap();
        let perms = vec![PsidSsp::new(Psid(Integer::from(35u32)), Some(ssp))];
        let cert = self_signed_certificate("notanelector", 503_136_005, 12, perms, None, &key).unwrap();
        el[0] = Elector { key, cert };
        let ids: Vec<_> = el.iter().map(Elector::hashed_id48).collect();
        let tbs = full_ieee_tbs_ctl(US_CTL_SERIES_ID, 1, 679_737_605, &ids, &[], 3);
        let m = build_ctl(&tbs, vec![], &[&el[0], &el[1], &el[2]]).unwrap();
        let ctl = decode_ctl_spdu(&encode_ctl_spdu(&m).unwrap()).unwrap();
        let v = verify_ctl(&ctl, &CtlTrust::us_bootstrap(ids));
        assert!(!v.electors[0].ssp_ok && v.electors[0].signature_ok);
        assert_eq!(v.valid_signatures, 2);
        assert!(!v.accepted());
    }

    /// The elector certificate is the published form: 80 00 02 SSP, self(sha384), P-384.
    #[test]
    fn elector_certificate_is_the_published_form() {
        let e = &five()[0];
        assert!(elector_cert_ok(&e.cert));
        assert_eq!(app_ssp(&e.cert, 35), Some(Some(vec![0x80, 0x00, 0x02])));
        assert_eq!(validity(&e.cert).1 - validity(&e.cert).0, 12 * YEAR_S);
    }

    /// A CCF and a composite CRL carry the CTL and round-trip.
    #[test]
    fn chain_file_and_composite_crl_round_trip() {
        let (_, ctl) = ctl_signed_by(3);
        let ccf = encode_certificate_chain(&ctl.raw, vec![root()]).unwrap();
        let (c2, others) = decode_certificate_chain(&ccf).unwrap();
        assert_eq!(c2, ctl);
        let chk = check_chain(&c2, &others).unwrap();
        assert!(matches!(chk[0].link, ChainLink::Root) && chk[0].anchored);
        let comp = encode_composite_crl(vec![], &ctl.raw).unwrap();
        let (crls, c3) = decode_composite_crl(&comp).unwrap();
        assert!(crls.is_empty());
        assert_eq!(c3, ctl);
    }
}
