//! The SCMS's governance and trust-management entities, and what a device does with
//! their products (WP-B5; docs/design/scms-real/00 gap rows 14, 31 and 32; 01 §1–§3).
//!
//! The US SCMS has entities that never touch a pseudonym request: the SCMS Manager and its
//! Ecosystem Audit Committee (EAC), which decide; the five electors, which sign the
//! certificate trust list (CTL); each provider's offline Root CA; the Intermediate CA
//! (ICA); and the Policy Generator (PG). A device cannot verify a single pseudonym
//! certificate, a CRL or a policy without their artefacts, and a run that left them out
//! would be trusting everything for free.
//!
//! # What is real (bit-exact IEEE 1609.2-2022 / 1609.2.1-2022)
//!
//! * **Electors.** Five explicit, self-signed (SHA-384) ECDSA P-384 certificates with
//!   PSID 35 and SSP `elector{version 2}` = `80 00 02`, valid 12/15/18/21/24 years from
//!   Time32 503 136 005, exactly as SCMS Manager's published production and test elector
//!   certificates (01 §3.1; `third_party/scms-artefacts/PROVENANCE.md`).
//! * **The CTL.** A `MultiSignedCtlSpdu` (`fullIeeeCtl`, CtlSeriesId `0348000000000000`)
//!   built and verified by [`v2xw_sec::ctl`] (WP-A5): `electorApprove` by `HashedId48`,
//!   `rootCaApprove` by `HashedId32`, the roots in `unsigned`, one `CtlSignatureSpdu` per
//!   elector. Every elector responds to every ballot (SCMS Manager "waits for the first 3
//!   but still requires all 5 to respond", Elector Tech Spec §2.2, §2.4). A device accepts
//!   a CTL with at least `quorum` distinct elector signatures from the electors **its
//!   previously installed CTL** approved — so 2 of 5 is refused and 3 of 5 accepted.
//! * **Component certificates.** Explicit, ECDSA P-256, issued by `sha256AndDigest`, in
//!   the shape of the published US chain files (production SAESOL `*.prod.s2x.io`, test
//!   ISS/SAESOL/Autocrypt): which issuer, `crlSeries` (2 for ICA-issued, 256 for
//!   root-issued; Profile Table 1), `cracaId` = the issuer's `HashedId3`, the
//!   `SecurityMgmtSsp` per component (`81 00 02` root, `83 00 02` ICA, …), the
//!   `certIssuePermissions` groups, the RA's `certRequestPermissions`, encryption keys on
//!   the RA and MA only (production). Lifetimes are the published ones (see
//!   [`ROLE_SPECS`]). A device validates each with the full 1609.2 chain rules of
//!   [`v2xw_sec::chain::validate`], so a certificate whose SSP or permissions its issuer
//!   could not grant is refused, not just one with a bad signature.
//! * **Certificate chain files.** The GCCF (PG) and LCCF (RA) are the 1609.2.1
//!   `CertificateChainSpdu` (`homeCtl` + `others`), the US profile's choice (1609.2.1
//!   §8.4; 00 gap row 32). A device installs the `homeCtl` if it is newer.
//! * **Policy files.** GPF (signed by the PG) and LPF (signed by the RA) are real
//!   IEEE 1609.2 `signedData` SPDUs, PSID 35, header psid only, signer `certificate`.
//!   **UNCONFIRMED:** the payload layout. CAMP 1.2.2 defines `SignedLocalPolicyFile` in an
//!   ASN.1 module this build could not obtain, so the payload is this engine's fixed
//!   layout ([`Policy::payload`]); the envelope and signature are real.
//!
//! # Lifecycle operations (gap row 31)
//!
//! An EAC decision ([`Governance::add_root`], [`Governance::remove_root`],
//! [`Governance::replace_elector`], [`Governance::rollover_root`]) produces a new CTL
//! with the next sequence number, signed by the electors, and scheduled for publication
//! at the SLA deadline: an addition within 3 business days (Provider Req. v1.1 §3.4.11),
//! a removal within 48 h (Elector Policy v1.0 §2.2.4) — [`ctl_publication_deadline`].
//! [`Governance::publish_due`] releases it. A removed root or elector is not put on a CRL;
//! it simply drops out of the next CTL (ISS CP §6.5.1), and a device that installs that
//! CTL discards every authority certificate anchored at the removed root. An elector
//! change must be installed in sequence: a device that missed it cannot verify a later
//! CTL signed by the new elector, which [`Governance::ctl_history`] lets it catch up on.
//! [`Governance::test_ballot`] is the yearly elector exercise (Elector Policy §1.1).
//!
//! Every decision, publication and refusal is recorded ([`Governance::log`],
//! [`DeviceTrust::last_rejection`]) so an attack scenario can see what was trusted when.
//!
//! # What it does not do
//!
//! Business days skip weekends only; US federal holidays are not modelled here
//! (UNCONFIRMED which calendar SCMS Manager uses). The provider's own CRL of revoked
//! components (series 2/256) is the CRL work package's.

use std::collections::{BTreeMap, BTreeSet};

use p256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use p256::ecdsa::{Signature as EcdsaSig, VerifyingKey};
use rasn::types::{FixedBitString, FixedOctetString, Integer};
use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngDomain, RngRegistry};
use v2xw_core::time::{NS_PER_S, SimTime, WallClock};
use v2xw_msg::sec_types::ieee1609_dot2::{
    Certificate, CertificateBase, CertificateId, CertificateType, EndEntityType, Ieee1609Dot2Data,
    IssuerIdentifier, PsidGroupPermissions, SequenceOfPsidGroupPermissions, SubjectPermissions,
    ToBeSignedCertificate, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    AnonymousSequenceOfOctetString, BasePublicEncryptionKey, CrlSeries, Duration as Duration1609,
    EccP256CurvePoint, HashedId3, HashedId8, Hostname, Psid, PsidSsp, PsidSspRange,
    PublicEncryptionKey, SequenceOfOctetString, SequenceOfPsid, SequenceOfPsidSsp,
    SequenceOfPsidSspRange, ServiceSpecificPermissions, SspRange, SymmAlgorithm, Time32, Uint8,
    Uint16, Uint32, ValidityPeriod,
};
use v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::{
    AcaSsp, CrlSignerSsp, DcSsp, DcmSsp, EcaSsp, EeSsp, IcaSsp, LaSsp, LopSsp, MaSsp, PgSsp, RaSsp,
    RootCaSsp, SecurityMgmtSsp,
};
use v2xw_sec::chain::{self, ChainErrorKind, ChainPolicy};
use v2xw_sec::crl::{CracaType, CrlSsp};
use v2xw_sec::ctl::{self, Ctl, CtlTrust, Elector, US_CTL_SERIES_ID};
use v2xw_sec::sig1609::{
    self, Signer1609, SpduSigner, canonical_cert_coer, cert_hashed_id8, cert_hashed_id32,
    cert_hashed_id48, coer,
};

use crate::scms::params::{ScmsNodes, ScmsParams};

/// PSID 35, `SecurityMgmtPsid` (1609.2.1).
pub const SECURITY_MGMT_PSID: u64 = 35;
/// PSID 0x20, the BSM, which the MA's `MaSsp.relevantPsids` covers (published MA
/// certificate `ma.v2x-scms.saesoltech.io`: SSP `8a 00 02 01 01 01 20`).
pub const BSM_PSID: u64 = 0x20;
/// One 1609.2 year (`Duration.years`): 31 556 952 s (365.2425 days), as [`ctl::YEAR_S`].
pub const YEAR_S: u64 = ctl::YEAR_S;
/// The published elector certificates' start, Time32 503 136 005 (2019-12-11).
pub const ELECTOR_START_TIME32: u32 = 503_136_005;
/// The published electors' lifetimes, years: staggered so they expire one at a time.
pub const ELECTOR_YEARS: [u16; 5] = [12, 15, 18, 21, 24];
/// The US i-period epoch, 2015-01-06 04:00 EST = 09:00 UTC (03 §1 fact 4).
pub const US_IPERIOD_EPOCH_UNIX_S: i64 = 1_420_534_800;
/// The default wall-clock instant of scenario `t0`: 2026-01-06 09:00 UTC, the start of US
/// i-period 574 (exactly 574 weeks after [`US_IPERIOD_EPOCH_UNIX_S`]). A scenario with a
/// real `time.t0` passes its own [`WallClock`] in [`GovernanceConfig`].
pub const DEFAULT_T0_UNIX_S: i64 = US_IPERIOD_EPOCH_UNIX_S + 574 * 7 * 86_400;

/// The node ids the governance entities are known by. None of them is hosted on the
/// request path; the SCMS Manager and the Policy Generator are hosted so a policy change
/// is a message with a latency like any other.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceNodes {
    /// The SCMS Manager (its EAC decides CTL changes).
    pub manager: NodeId,
    /// The Policy Generator: signs the global policy and chain files.
    pub pg: NodeId,
    /// The home provider's Root CA (offline).
    pub root: NodeId,
    /// The Intermediate CA (offline between certifications).
    pub ica: NodeId,
    /// The electors currently approved, in slot order (offline).
    pub electors: Vec<NodeId>,
}

impl GovernanceNodes {
    /// The reference ids, clear of [`ScmsNodes`]' 1–11. Elector slot `k` is node `24 + k`.
    #[must_use]
    pub fn new(electors: u32) -> GovernanceNodes {
        GovernanceNodes {
            manager: NodeId::new(20),
            pg: NodeId::new(21),
            root: NodeId::new(22),
            ica: NodeId::new(23),
            electors: (0..electors.max(1)).map(elector_node).collect(),
        }
    }
}

fn elector_node(slot: u32) -> NodeId {
    NodeId::new(24 + slot)
}

/// Why a device refused a trust artefact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum TrustError {
    /// A trust list without a quorum of valid elector signatures.
    CtlQuorum {
        /// Distinct trusted electors whose signature verified.
        valid: u32,
        /// The quorum the device's installed trust requires.
        needed: u32,
    },
    /// A CTL of another series or type, or one that does not decode.
    CtlMalformed,
    /// A certificate or file whose issuer the device does not trust.
    UnknownIssuer,
    /// A signature that does not verify.
    BadSignature,
    /// A file no newer than the one installed.
    Stale,
    /// A certificate outside its validity window.
    Expired,
    /// A certificate its issuer could not grant (permissions, SSP, chain length,
    /// validity or region not contained): 1609.2 §5.1.2.4.
    NotPermitted,
    /// A file signed by a certificate whose SSP is not the role that may sign it.
    WrongSigner,
    /// Bytes that do not decode as the artefact they claim to be.
    Malformed,
}

impl core::fmt::Display for TrustError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            TrustError::CtlQuorum { valid, needed } => write!(
                f,
                "trust list carries {valid} valid elector signature(s), quorum is {needed}"
            ),
            TrustError::CtlMalformed => f.write_str("trust list of the wrong series or type"),
            TrustError::UnknownIssuer => f.write_str("issuer not in the device's trust store"),
            TrustError::BadSignature => f.write_str("signature does not verify"),
            TrustError::Stale => f.write_str("file is not newer than the installed one"),
            TrustError::Expired => f.write_str("certificate outside its validity window"),
            TrustError::NotPermitted => f.write_str("issuer could not grant this certificate"),
            TrustError::WrongSigner => f.write_str("signer's SSP does not allow this file"),
            TrustError::Malformed => f.write_str("does not decode"),
        }
    }
}

// ---------------------------------------------------------------------------------------
// The component certificate profile
// ---------------------------------------------------------------------------------------

/// Who issues a component certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssuedBy {
    /// Self-signed (the Root CA).
    SelfSigned,
    /// The Root CA (`crlSeries` 256 unless the ICA, Profile Table 1).
    Root,
    /// The Intermediate CA (`crlSeries` 2).
    Ica,
}

/// A 1609.2 `Duration` of whole years or sixty-hour units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifetime {
    /// `years(n)`.
    Years(u16),
    /// `sixtyHours(n)`.
    SixtyHours(u16),
}

impl Lifetime {
    fn to_asn1(self) -> Duration1609 {
        match self {
            Lifetime::Years(y) => Duration1609::years(Uint16(y)),
            Lifetime::SixtyHours(h) => Duration1609::sixtyHours(Uint16(h)),
        }
    }

    /// Seconds.
    #[must_use]
    pub fn seconds(self) -> u64 {
        match self {
            Lifetime::Years(y) => u64::from(y) * YEAR_S,
            Lifetime::SixtyHours(h) => u64::from(h) * 216_000,
        }
    }
}

/// One component's certificate profile, transcribed from a published US certificate.
#[derive(Debug, Clone, Copy)]
pub struct RoleSpec {
    /// The engine's role name.
    pub role: &'static str,
    /// The host label in `id.name` (Provider Req. v1.1 §3.3.1.1 names).
    pub host: &'static str,
    /// The issuer.
    pub issuer: IssuedBy,
    /// Validity.
    pub lifetime: Lifetime,
    /// Whether the certificate carries an `encryptionKey`.
    pub encryption: bool,
    /// The published certificate the profile is transcribed from.
    pub source: &'static str,
}

/// Every component's profile. Lifetimes are the US production chain's (SAESOL
/// `*.prod.s2x.io`, 2025-08-13) where it has the component, else the test chain's
/// (2025-07-17) named in `source`; DCM, LOP and DC have no published certificate and take
/// the RA's 6 years (**assumption**). The ISS test LAs carry their ICA's full lifetime (30 y
/// under a 30 y ICA); here that is the production ICA's 25 y, so validity stays inside the
/// issuer's (1609.2 §5.1.2.4).
pub const ROLE_SPECS: [RoleSpec; 13] = [
    RoleSpec {
        role: "root",
        host: "rootca",
        issuer: IssuedBy::SelfSigned,
        lifetime: Lifetime::Years(30),
        encryption: false,
        source: "rootca.prod.s2x.io",
    },
    RoleSpec {
        role: "ica",
        host: "ica",
        issuer: IssuedBy::Root,
        lifetime: Lifetime::Years(25),
        encryption: false,
        source: "ica.prod.s2x.io",
    },
    RoleSpec {
        role: "pg",
        host: "pg",
        issuer: IssuedBy::Root,
        lifetime: Lifetime::Years(12),
        encryption: false,
        source: "pg.v2x-scms.saesoltech.io (test)",
    },
    RoleSpec {
        role: "ma",
        host: "ma",
        issuer: IssuedBy::Root,
        lifetime: Lifetime::Years(12),
        encryption: true,
        source: "ma.v2x-scms.saesoltech.io (test)",
    },
    RoleSpec {
        role: "crlg",
        host: "crlg",
        issuer: IssuedBy::Root,
        lifetime: Lifetime::SixtyHours(1473),
        encryption: false,
        source: "crlg.preprod.v2x.isscms.com (test)",
    },
    RoleSpec {
        role: "eca",
        host: "eca",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(15),
        encryption: false,
        source: "eca.prod.s2x.io",
    },
    RoleSpec {
        role: "pca",
        host: "aca",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(4),
        encryption: false,
        source: "aca.prod.s2x.io",
    },
    RoleSpec {
        role: "ra",
        host: "ra",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(6),
        encryption: true,
        source: "ra.prod.s2x.io",
    },
    RoleSpec {
        role: "la1",
        host: "la1",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(25),
        encryption: false,
        source: "la1.preprod.v2x.isscms.com (test): the ICA's full lifetime",
    },
    RoleSpec {
        role: "la2",
        host: "la2",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(25),
        encryption: false,
        source: "la2.preprod.v2x.isscms.com (test): the ICA's full lifetime",
    },
    RoleSpec {
        role: "lop",
        host: "lop",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(6),
        encryption: false,
        source: "none published (assumption: RA lifetime)",
    },
    RoleSpec {
        role: "dcm",
        host: "dcm",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(6),
        encryption: false,
        source: "none published (assumption: RA lifetime)",
    },
    RoleSpec {
        role: "crl-store",
        host: "dc",
        issuer: IssuedBy::Ica,
        lifetime: Lifetime::Years(6),
        encryption: false,
        source: "none published (assumption: RA lifetime)",
    },
];

/// The roles the Intermediate CA certifies, in issuance order.
pub const ICA_ISSUED: [&str; 8] = ["eca", "pca", "ra", "la1", "la2", "lop", "dcm", "crl-store"];

/// The roles the Root CA certifies directly, besides the ICA (ISS CP §2.7.1; Profile
/// Table 1).
pub const ROOT_ISSUED: [&str; 3] = ["pg", "ma", "crlg"];

fn spec_of(role: &str) -> &'static RoleSpec {
    ROLE_SPECS
        .iter()
        .find(|s| s.role == role)
        .expect("every role has a spec")
}

/// The `SecurityMgmtSsp` a role's certificate carries for PSID 35 (1609.2.1-2022
/// `Ieee1609Dot2Dot1Protocol`; version 2 throughout, as the 2022 module fixes).
#[must_use]
pub fn security_mgmt_ssp(role: &str) -> SecurityMgmtSsp {
    let v = Uint8(2);
    match role {
        "elector" => SecurityMgmtSsp::elector(
            v2xw_msg::sec_types::ieee1609_dot2_dot1_protocol::ElectorSsp::new(v),
        ),
        "root" => SecurityMgmtSsp::root(RootCaSsp::new(v)),
        "pg" => SecurityMgmtSsp::pg(PgSsp::new(v)),
        "ica" => SecurityMgmtSsp::ica(IcaSsp::new(v)),
        "eca" => SecurityMgmtSsp::eca(EcaSsp::new(v)),
        "pca" => SecurityMgmtSsp::aca(AcaSsp::new(v)),
        "crlg" => SecurityMgmtSsp::crl(CrlSignerSsp::new(v)),
        "dcm" => SecurityMgmtSsp::dcm(DcmSsp::new(v)),
        "la1" => SecurityMgmtSsp::la(LaSsp::new(v, Uint16(1))),
        "la2" => SecurityMgmtSsp::la(LaSsp::new(v, Uint16(2))),
        "lop" => SecurityMgmtSsp::lop(LopSsp::new(v)),
        "ma" => SecurityMgmtSsp::ma(MaSsp::new(
            v,
            SequenceOfPsid(vec![Psid(Integer::from(BSM_PSID))]),
        )),
        "ra" => SecurityMgmtSsp::ra(RaSsp::new(v)),
        "crl-store" => SecurityMgmtSsp::dc(DcSsp::new(v)),
        _ => SecurityMgmtSsp::ee(EeSsp::new(v)),
    }
}

/// The engine role a certificate's PSID-35 SSP names; `None` without one.
#[must_use]
pub fn role_of(cert: &Certificate) -> Option<&'static str> {
    Some(match ctl::security_mgmt_ssp(cert)? {
        SecurityMgmtSsp::elector(_) => "elector",
        SecurityMgmtSsp::root(_) => "root",
        SecurityMgmtSsp::pg(_) => "pg",
        SecurityMgmtSsp::ica(_) => "ica",
        SecurityMgmtSsp::eca(_) => "eca",
        SecurityMgmtSsp::aca(_) => "pca",
        SecurityMgmtSsp::crl(_) => "crlg",
        SecurityMgmtSsp::dcm(_) => "dcm",
        SecurityMgmtSsp::la(l) if l.la_id.0 == 1 => "la1",
        SecurityMgmtSsp::la(_) => "la2",
        SecurityMgmtSsp::lop(_) => "lop",
        SecurityMgmtSsp::ma(_) => "ma",
        SecurityMgmtSsp::ra(_) => "ra",
        SecurityMgmtSsp::ee(_) => "ee",
        SecurityMgmtSsp::dc(_) => "crl-store",
        _ => return None,
    })
}

fn psid(p: u64) -> Psid {
    Psid(Integer::from(p))
}

fn opaque(bytes: Vec<u8>) -> ServiceSpecificPermissions {
    ServiceSpecificPermissions::opaque(bytes.into())
}

fn ee_type(app: bool, enrol: bool) -> EndEntityType {
    let mut b = FixedBitString::<8>::ZERO;
    b.set(0, app);
    b.set(1, enrol);
    EndEntityType(b)
}

fn group(
    subject: SubjectPermissions,
    min_chain: i32,
    range: i32,
    app: bool,
    enrol: bool,
) -> PsidGroupPermissions {
    PsidGroupPermissions::new(
        subject,
        Integer::from(min_chain),
        Integer::from(range),
        ee_type(app, enrol),
    )
}

fn explicit_psids(items: Vec<(u64, Option<SspRange>)>) -> SubjectPermissions {
    SubjectPermissions::explicit(SequenceOfPsidSspRange(
        items
            .into_iter()
            .map(|(p, r)| PsidSspRange::new(psid(p), r))
            .collect(),
    ))
}

/// A role's `appPermissions`.
fn app_permissions(role: &str) -> Vec<PsidSsp> {
    let ssp = coer(&security_mgmt_ssp(role)).expect("an SSP encodes");
    let mut v = vec![PsidSsp::new(psid(SECURITY_MGMT_PSID), Some(opaque(ssp)))];
    match role {
        // Published roots: `256: 00 01 00 01 01 01 00` (CrlSsp isCraca, series 256).
        "root" => v.push(PsidSsp::new(
            psid(v2xw_sec::crl::CRL_PSID),
            Some(opaque(CrlSsp::new(CracaType::isCraca, &[256]).to_coer())),
        )),
        // Published CRL signer (ISS): issuerIsCraca, series 4, 3, 1, 2.
        "crlg" => v.push(PsidSsp::new(
            psid(v2xw_sec::crl::CRL_PSID),
            Some(opaque(
                CrlSsp::new(CracaType::issuerIsCraca, &[4, 3, 1, 2]).to_coer(),
            )),
        )),
        _ => {}
    }
    v
}

/// A role's `certIssuePermissions`, as on the published certificates.
fn cert_issue_permissions(role: &str) -> Option<SequenceOfPsidGroupPermissions> {
    let all = || SubjectPermissions::all(());
    let groups = match role {
        "root" => vec![
            group(all(), 3, -1, true, true),
            group(
                explicit_psids(vec![(SECURITY_MGMT_PSID, None)]),
                1,
                -1,
                true,
                true,
            ),
            group(explicit_psids(vec![(38, None)]), 1, -1, true, true),
            group(
                explicit_psids(vec![(v2xw_sec::crl::CRL_PSID, Some(SspRange::all(())))]),
                1,
                -1,
                true,
                true,
            ),
        ],
        "ica" => vec![
            group(all(), 2, 0, true, true),
            group(
                explicit_psids(vec![
                    (SECURITY_MGMT_PSID, Some(SspRange::all(()))),
                    (v2xw_sec::crl::CRL_PSID, Some(SspRange::all(()))),
                ]),
                1,
                -1,
                true,
                true,
            ),
        ],
        // The ECA may issue enrolment certificates (eeType enrol, any PSID) and PSID 35
        // with the EE SSP `8c 00 02`.
        "eca" => {
            let ee = coer(&security_mgmt_ssp("ee")).expect("an SSP encodes");
            vec![
                group(
                    explicit_psids(vec![(
                        SECURITY_MGMT_PSID,
                        Some(SspRange::opaque(SequenceOfOctetString(vec![
                            AnonymousSequenceOfOctetString(ee.into()),
                        ]))),
                    )]),
                    1,
                    0,
                    true,
                    false,
                ),
                group(all(), 1, 0, false, true),
            ]
        }
        "pca" => vec![group(all(), 1, 0, true, false)],
        _ => return None,
    };
    Some(SequenceOfPsidGroupPermissions(groups))
}

/// The RA's `certRequestPermissions`: it may request any application certificate
/// (published RA certificates, 1609.2.1-2022 form).
fn cert_request_permissions(role: &str) -> Option<SequenceOfPsidGroupPermissions> {
    (role == "ra").then(|| {
        SequenceOfPsidGroupPermissions(vec![group(SubjectPermissions::all(()), 0, 0, true, false)])
    })
}

// ---------------------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------------------

/// The RNG scope authority keys are drawn from: the node's id and key generation under
/// this module's own custom scope, so an authority key never consumes a draw from the
/// stream the same node's protocol role uses.
fn key_scope(stream: &str, node: NodeId, generation: u32) -> EntityRef {
    EntityRef::custom(
        stream,
        u64::from(node.index()) | (u64::from(generation) << 32),
    )
}

fn draw<const N: usize>(rng: &RngRegistry, stream: &str, node: NodeId, generation: u32) -> [u8; N] {
    let mut g = rng.checkout(RngDomain::Crypto, key_scope(stream, node, generation));
    let mut b = [0u8; N];
    g.fill_bytes(&mut b);
    b
}

fn p256_key(rng: &RngRegistry, stream: &str, node: NodeId, generation: u32) -> Signer1609 {
    // A draw of zero or of at least the group order is not a key; the next draw is, with
    // overwhelming probability, and the stream is deterministic either way.
    for attempt in 0..16 {
        let b: [u8; 32] = draw(rng, stream, node, generation.wrapping_add(attempt << 16));
        if let Some(k) = Signer1609::p256(&b) {
            return k;
        }
    }
    unreachable!("16 consecutive out-of-range P-256 scalars")
}

fn elector_key(rng: &RngRegistry, slot: u32) -> [u8; 48] {
    for attempt in 0..16 {
        let b: [u8; 48] = draw(
            rng,
            "protocol/scms/governance/elector-keys",
            elector_node(slot),
            attempt,
        );
        if Signer1609::p384(&b).is_some() {
            return b;
        }
    }
    unreachable!("16 consecutive out-of-range P-384 scalars")
}

fn compressed_p256(k: &Signer1609) -> Vec<u8> {
    match k {
        Signer1609::P256(s) => s.verifying_key().to_encoded_point(true).as_bytes().to_vec(),
        Signer1609::P384(_) => unreachable!("component keys are P-256"),
    }
}

fn encryption_key_field(public_compressed: &[u8]) -> PublicEncryptionKey {
    let x: Vec<u8> = public_compressed[1..].to_vec();
    let point = if public_compressed[0] == 2 {
        EccP256CurvePoint::compressed_y_0(x.into())
    } else {
        EccP256CurvePoint::compressed_y_1(x.into())
    };
    PublicEncryptionKey::new(
        SymmAlgorithm::aes128Ccm,
        BasePublicEncryptionKey::eciesNistP256(point),
    )
}

fn hashed_id3(cert: &Certificate) -> [u8; 3] {
    let id8 = cert_hashed_id8(cert).expect("a certificate encodes");
    [id8[5], id8[6], id8[7]]
}

/// Issues an explicit certificate over `tbs`: self-signed when `issuer` is `None`, else
/// `sha256AndDigest(HashedId8(issuer))` signed over `H(H(tbs) ‖ H(issuer))` (1609.2
/// §5.3.1, 02 §5).
fn issue(
    tbs: ToBeSignedCertificate,
    issuer: Option<(&Certificate, &Signer1609)>,
    self_key: &Signer1609,
) -> Certificate {
    let tbs_coer = coer(&tbs).expect("a ToBeSignedCertificate encodes");
    let (issuer_id, sig) = match issuer {
        None => (
            IssuerIdentifier::R_self(self_key.hash().to_asn1()),
            self_key.sign_1609(&tbs_coer, b""),
        ),
        Some((ic, ik)) => {
            let d = HashedId8(FixedOctetString::from(
                cert_hashed_id8(ic).expect("issuer encodes"),
            ));
            let id = match ik {
                Signer1609::P256(_) => IssuerIdentifier::sha256AndDigest(d),
                Signer1609::P384(_) => IssuerIdentifier::sha384AndDigest(d),
            };
            let input = canonical_cert_coer(ic).expect("issuer encodes");
            (id, ik.sign_1609(&tbs_coer, &input))
        }
    };
    Certificate(CertificateBase::new(
        Uint8(3),
        CertificateType::explicit,
        issuer_id,
        tbs,
        Some(sig),
    ))
}

/// One authority: its certificate and keys.
#[derive(Clone)]
struct Authority {
    node: NodeId,
    generation: u32,
    cert: Certificate,
    key: Signer1609,
    /// The ECIES private scalar for the certificate's `encryptionKey`.
    enc_scalar: Option<[u8; 32]>,
}

/// One elector slot, approved or not.
#[derive(Clone)]
struct ElectorSlot {
    node: NodeId,
    elector: Elector,
    approved: bool,
}

/// One provider root (the home provider's current and past roots, and added roots).
#[derive(Clone)]
struct RootSlot {
    label: String,
    authority: Authority,
    approved: bool,
}

// ---------------------------------------------------------------------------------------
// Artefacts
// ---------------------------------------------------------------------------------------

/// An IEEE 1609.2.1 `MultiSignedCtlSpdu` with the fields an inspector reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedCtl {
    /// `sequenceNumber`; a device installs only a newer one.
    pub sequence: u32,
    /// `quorum`: the signatures the *next* CTL needs.
    pub quorum: u32,
    /// `rootCaApprove`: `HashedId32` of each approved root.
    pub roots: Vec<[u8; 32]>,
    /// `electorApprove`: `HashedId48` of each approved elector.
    pub electors: Vec<[u8; 48]>,
    /// `effectiveDate`, Time32.
    pub effective: u32,
    /// The elector slots that signed it, in signature order.
    pub signers: Vec<u32>,
    /// The COER `MultiSignedCtlSpdu`.
    pub spdu: Vec<u8>,
}

/// What a policy file fixes. The fields are the lifecycle numbers this build's flows
/// read; every one of them is also on the plug-in's model card.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Policy {
    /// The i-period, seconds.
    pub i_period_s: u64,
    /// A pseudonym certificate's lifetime, seconds.
    pub cert_lifetime_s: u64,
    /// Certificates valid at once in one i-period.
    pub certs_per_period: u32,
    /// How many i-periods ahead the RA provisions at most.
    pub max_periods_ahead: u32,
    /// The RA's request-shuffle window, seconds.
    pub shuffle_window_s: u64,
    /// The CRL cadence, seconds.
    pub crl_cadence_s: u64,
    /// An enrolment certificate's lifetime, seconds.
    pub enrolment_lifetime_s: u64,
    /// How long before its enrolment certificate expires a device asks for a successor,
    /// seconds.
    pub reenrol_lead_s: u64,
}

/// The policy payload's magic: names the engine layout (the CAMP ASN.1 is UNCONFIRMED).
const POLICY_MAGIC: &[u8; 8] = b"V2XWPOL1";

impl Policy {
    /// The policy a deployment's parameters describe.
    #[must_use]
    pub fn of(p: &ScmsParams) -> Policy {
        Policy {
            i_period_s: p.i_period.as_nanos() / NS_PER_S,
            cert_lifetime_s: p.cert_lifetime.as_nanos() / NS_PER_S,
            certs_per_period: p.certs_per_period,
            max_periods_ahead: p.max_periods_ahead,
            shuffle_window_s: p.shuffle_window.as_nanos() / NS_PER_S,
            crl_cadence_s: p.crl_cadence.as_nanos() / NS_PER_S,
            enrolment_lifetime_s: p.enrolment_lifetime.as_nanos() / NS_PER_S,
            reenrol_lead_s: p.reenrol_lead.as_nanos() / NS_PER_S,
        }
    }

    fn fields(&self) -> [u64; 8] {
        [
            self.i_period_s,
            self.cert_lifetime_s,
            u64::from(self.certs_per_period),
            u64::from(self.max_periods_ahead),
            self.shuffle_window_s,
            self.crl_cadence_s,
            self.enrolment_lifetime_s,
            self.reenrol_lead_s,
        ]
    }

    /// The signed payload: magic, scope (0 global, 1 local), version (u32 BE), then the
    /// eight fields as u64 BE. **Engine layout** (the CAMP format is UNCONFIRMED).
    #[must_use]
    pub fn payload(&self, scope: Scope, version: u32) -> Vec<u8> {
        let mut b = POLICY_MAGIC.to_vec();
        b.push(u8::from(scope == Scope::Local));
        b.extend_from_slice(&version.to_be_bytes());
        for v in self.fields() {
            b.extend_from_slice(&v.to_be_bytes());
        }
        b
    }

    /// Reads [`Policy::payload`] back.
    #[must_use]
    pub fn from_payload(b: &[u8]) -> Option<(Scope, u32, Policy)> {
        if b.len() != 8 + 1 + 4 + 64 || &b[..8] != POLICY_MAGIC {
            return None;
        }
        let scope = match b[8] {
            0 => Scope::Global,
            1 => Scope::Local,
            _ => return None,
        };
        let version = u32::from_be_bytes(b[9..13].try_into().ok()?);
        let f = |k: usize| u64::from_be_bytes(b[13 + 8 * k..21 + 8 * k].try_into().expect("8"));
        let p = Policy {
            i_period_s: f(0),
            cert_lifetime_s: f(1),
            certs_per_period: u32::try_from(f(2)).ok()?,
            max_periods_ahead: u32::try_from(f(3)).ok()?,
            shuffle_window_s: f(4),
            crl_cadence_s: f(5),
            enrolment_lifetime_s: f(6),
            reenrol_lead_s: f(7),
        };
        Some((scope, version, p))
    }

    /// How many fields a policy file encodes, for its size.
    pub const FIELDS: u32 = 8;
}

/// Global or local.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// Signed by the Policy Generator for the whole system.
    Global,
    /// Served by the Registration Authority to its devices.
    Local,
}

/// A signed policy file (GPF or LPF): an IEEE 1609.2 `signedData` SPDU.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyFile {
    /// Global or local.
    pub scope: Scope,
    /// Its version; a device installs only a newer one.
    pub version: u32,
    /// What it says (a device reads it from [`PolicyFile::spdu`], never from here).
    pub policy: Policy,
    /// The signer's certificate `HashedId8`.
    pub signer: [u8; 8],
    /// The COER `Ieee1609Dot2Data`.
    pub spdu: Vec<u8>,
}

/// A certificate chain file (GCCF or LCCF): the 1609.2.1 `CertificateChainSpdu`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainFile {
    /// Global or local.
    pub scope: Scope,
    /// Its version (the file carries none; the RA's `If-None-Match` names it).
    pub version: u32,
    /// The certificates in `others`, roots first.
    pub certs: Vec<Certificate>,
    /// The `homeCtl`'s sequence number.
    pub ctl_sequence: u32,
    /// The COER `CertificateChainSpdu`.
    pub spdu: Vec<u8>,
}

/// What the governance entities have done, for an inspector.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct GovernanceCounters {
    /// Certificates the Root CAs signed (their own included).
    pub root_certs_issued: u32,
    /// Certificates the Intermediate CA signed.
    pub ica_certs_issued: u32,
    /// CTL signatures the electors produced.
    pub elector_endorsements: u32,
    /// Decisions the SCMS Manager (EAC) took.
    pub manager_decisions: u32,
    /// Files the Policy Generator signed.
    pub pg_files_signed: u32,
    /// Local files the Registration Authority signed.
    pub ra_files_signed: u32,
    /// CTLs published.
    pub ctls_published: u32,
    /// Roots added to and removed from the CTL.
    pub roots_added: u32,
    /// Roots removed from the CTL.
    pub roots_removed: u32,
    /// Elector replacements.
    pub elector_changes: u32,
    /// Test ballots run.
    pub test_ballots: u32,
}

/// What an EAC decision changes in the CTL.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CtlChangeKind {
    /// The genesis CTL.
    Genesis,
    /// A root added (a new provider, or a rollover's new root).
    AddRoot,
    /// A root removed.
    RemoveRoot,
    /// An elector removed and a new one added.
    ReplaceElector,
}

/// One governance record.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GovernanceEvent {
    /// When it was decided.
    pub decided_at: SimTime,
    /// When the CTL is (or was) published; `None` for events without one.
    pub published_at: Option<SimTime>,
    /// What happened.
    pub kind: &'static str,
    /// The CTL sequence number concerned, if any.
    pub ctl_sequence: Option<u32>,
    /// Elector slots that signed, if a ballot.
    pub signers: Vec<u32>,
    /// A free-form detail (labels, roles).
    pub detail: String,
}

/// The SLA deadline by which a CTL change must be published (01 §3.1): additions within
/// 3 business days (Provider Req. v1.1 §3.4.11; the Elector Policy allows 5), removals
/// within 48 h (Elector Policy v1.0 §2.2.4). Business days skip Saturday and Sunday in UTC.
#[must_use]
pub fn ctl_publication_deadline(
    kind: CtlChangeKind,
    decided_at: SimTime,
    clock: WallClock,
) -> SimTime {
    const DAY: u64 = 86_400 * NS_PER_S;
    match kind {
        CtlChangeKind::Genesis => decided_at,
        CtlChangeKind::RemoveRoot => decided_at + 2 * DAY,
        CtlChangeKind::AddRoot | CtlChangeKind::ReplaceElector => {
            let mut t = decided_at;
            let mut left = 3;
            while left > 0 {
                t += DAY;
                let days = clock.unix_seconds_at(t).div_euclid(86_400);
                let weekday = (days + 4).rem_euclid(7); // 0 = Sunday
                if (1..=5).contains(&weekday) {
                    left -= 1;
                }
            }
            t
        }
    }
}

/// Deployment choices for the governance entities.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GovernanceConfig {
    /// Maps scenario time to Time32.
    pub clock: WallClock,
    /// The home provider's neutral label (owner decision: neutral provider names).
    pub provider: String,
}

impl Default for GovernanceConfig {
    fn default() -> Self {
        GovernanceConfig {
            clock: WallClock::new(DEFAULT_T0_UNIX_S),
            provider: "provider-a".into(),
        }
    }
}

/// The bootstrap trust a device is provisioned with (Profile §2.1.15): the elector
/// certificates, the quorum, and its clock.
#[derive(Debug, Clone, Default)]
pub struct TrustAnchors {
    /// The elector certificates.
    pub electors: Vec<Certificate>,
    /// Signatures the first CTL needs.
    pub quorum: u32,
    /// The device's clock (scenario time to Time32).
    pub clock: WallClock,
}

impl TrustAnchors {
    /// How many elector certificates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.electors.len()
    }

    /// No anchors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.electors.is_empty()
    }
}

// ---------------------------------------------------------------------------------------
// The governance entities
// ---------------------------------------------------------------------------------------

/// The governance entities, their keys and their artefacts.
pub struct Governance {
    /// Where they are.
    pub nodes: GovernanceNodes,
    /// Deployment choices.
    pub config: GovernanceConfig,
    electors: Vec<ElectorSlot>,
    roots: Vec<RootSlot>,
    authorities: BTreeMap<&'static str, Authority>,
    /// Superseded component certificates that have not expired (key changeover keeps
    /// them available, Provider Req. §3.5.4).
    pub retired: Vec<Certificate>,
    quorum: u32,
    /// The current (published) trust list.
    pub ctl: SignedCtl,
    /// Every published CTL, in sequence order.
    pub ctl_history: Vec<SignedCtl>,
    /// Decided CTLs awaiting their publication time.
    pub pending: Vec<(SimTime, SignedCtl)>,
    /// The current Global Policy File.
    pub gpf: PolicyFile,
    /// The current Global Certificate Chain File.
    pub gccf: ChainFile,
    /// The Registration Authority's current Local Policy File.
    pub lpf: PolicyFile,
    /// The Registration Authority's current Local Certificate Chain File.
    pub lccf: ChainFile,
    /// What they have done.
    pub counters: GovernanceCounters,
    /// Every decision and publication.
    pub log: Vec<GovernanceEvent>,
}

impl core::fmt::Debug for Governance {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Governance")
            .field("nodes", &self.nodes)
            .field("authorities", &self.authorities.len())
            .field("ctl_sequence", &self.ctl.sequence)
            .field("gpf_version", &self.gpf.version)
            .field("lpf_version", &self.lpf.version)
            .finish_non_exhaustive()
    }
}

fn role_node(role: &str, nodes: &GovernanceNodes, online: &ScmsNodes) -> NodeId {
    match role {
        "root" => nodes.root,
        "ica" => nodes.ica,
        "pg" => nodes.pg,
        "ma" => online.ma,
        "crlg" => online.crlg,
        "eca" => online.eca,
        "pca" => online.pca,
        "ra" => online.ra,
        "la1" => online.la1,
        "la2" => online.la2,
        "lop" => online.lop,
        "dcm" => online.dcm,
        _ => online.crl_store,
    }
}

impl Governance {
    /// The setup ceremony with the default [`GovernanceConfig`].
    #[must_use]
    pub fn ceremony(
        p: &ScmsParams,
        online: &ScmsNodes,
        rng: &RngRegistry,
        t0: SimTime,
    ) -> Governance {
        Governance::ceremony_with(p, online, rng, t0, GovernanceConfig::default())
    }

    /// The setup ceremony: the five electors, the home provider's Root CA, its ICA and
    /// every component certificate, the genesis CTL (sequence 0) signed by every elector,
    /// and the first global and local policy and chain files.
    ///
    /// It happens at `t0` and charges nothing to the run's clock: the Root CA and the
    /// electors are offline and their ceremonies are not part of any flow a scenario
    /// measures.
    #[must_use]
    pub fn ceremony_with(
        p: &ScmsParams,
        online: &ScmsNodes,
        rng: &RngRegistry,
        t0: SimTime,
        config: GovernanceConfig,
    ) -> Governance {
        let nodes = GovernanceNodes::new(p.electors);
        let now32 = config.clock.time32(t0).unwrap_or(0);
        let start = ELECTOR_START_TIME32.min(now32);
        let electors = nodes
            .electors
            .iter()
            .enumerate()
            .map(|(k, &node)| {
                let slot = u32::try_from(k).unwrap_or(u32::MAX);
                let years = ELECTOR_YEARS[k % ELECTOR_YEARS.len()];
                ElectorSlot {
                    node,
                    elector: ctl::elector(
                        &format!("v2xelector{}.manager.sim", k + 1),
                        start,
                        years,
                        &elector_key(rng, slot),
                    )
                    .expect("an elector certificate builds"),
                    approved: true,
                }
            })
            .collect();
        let placeholder_ctl = SignedCtl {
            sequence: 0,
            quorum: p.elector_quorum,
            roots: Vec::new(),
            electors: Vec::new(),
            effective: now32,
            signers: Vec::new(),
            spdu: Vec::new(),
        };
        let policy = Policy::of(p);
        let pf = |scope| PolicyFile {
            scope,
            version: 0,
            policy,
            signer: [0; 8],
            spdu: Vec::new(),
        };
        let cf = |scope| ChainFile {
            scope,
            version: 0,
            certs: Vec::new(),
            ctl_sequence: 0,
            spdu: Vec::new(),
        };
        let mut gov = Governance {
            nodes,
            config,
            electors,
            roots: Vec::new(),
            authorities: BTreeMap::new(),
            retired: Vec::new(),
            quorum: p.elector_quorum,
            ctl: placeholder_ctl,
            ctl_history: Vec::new(),
            pending: Vec::new(),
            gpf: pf(Scope::Global),
            gccf: cf(Scope::Global),
            lpf: pf(Scope::Local),
            lccf: cf(Scope::Local),
            counters: GovernanceCounters::default(),
            log: Vec::new(),
        };
        let label = gov.config.provider.clone();
        let root = gov.make_authority("root", rng, gov.nodes.root, 0, None, &label, now32);
        gov.roots.push(RootSlot {
            label: label.clone(),
            authority: root,
            approved: true,
        });
        gov.certify_hierarchy(rng, online, 0, now32);
        let ctl = gov.build_ctl(0, now32, None, None, &gov.approved_signers());
        gov.ctl = ctl.clone();
        gov.ctl_history.push(ctl);
        gov.counters.ctls_published += 1;
        gov.log.push(GovernanceEvent {
            decided_at: t0,
            published_at: Some(t0),
            kind: "ctl-genesis",
            ctl_sequence: Some(0),
            signers: gov.ctl.signers.clone(),
            detail: label,
        });
        gov.sign_global(policy);
        gov.sign_local();
        gov
    }

    #[allow(clippy::too_many_arguments)]
    fn make_authority(
        &mut self,
        role: &'static str,
        rng: &RngRegistry,
        node: NodeId,
        generation: u32,
        issuer: Option<&Authority>,
        label: &str,
        start: u32,
    ) -> Authority {
        let spec = spec_of(role);
        let key = p256_key(rng, "protocol/scms/governance/keys", node, generation);
        let (enc_scalar, enc_field) = if spec.encryption {
            let s = p256_key(rng, "protocol/scms/governance/enc-keys", node, generation);
            let pubc = compressed_p256(&s);
            let scalar = match &s {
                Signer1609::P256(k) => {
                    let mut b = [0u8; 32];
                    b.copy_from_slice(&k.to_bytes());
                    b
                }
                Signer1609::P384(_) => unreachable!(),
            };
            (Some(scalar), Some(encryption_key_field(&pubc)))
        } else {
            (None, None)
        };
        let (craca, series) = match issuer {
            None => ([0u8; 3], 0u16),
            Some(i) => (
                hashed_id3(&i.cert),
                if spec.issuer == IssuedBy::Root && role != "ica" {
                    v2xw_sec::crl::SERIES_ROOT_SIGNED
                } else {
                    v2xw_sec::crl::SERIES_SCMS_COMPONENTS
                },
            ),
        };
        let name = format!("{}.{}.sim", spec.host, label);
        let tbs = ToBeSignedCertificate::new(
            CertificateId::name(Hostname(name)),
            HashedId3(FixedOctetString::from(craca)),
            CrlSeries(Uint16(series)),
            ValidityPeriod::new(Time32(Uint32(start)), spec.lifetime.to_asn1()),
            None,
            None,
            Some(SequenceOfPsidSsp(app_permissions(role))),
            cert_issue_permissions(role),
            cert_request_permissions(role),
            None,
            enc_field,
            VerificationKeyIndicator::verificationKey(key.public_key()),
            None,
            None,
            None,
            None,
        );
        let cert = issue(tbs, issuer.map(|i| (&i.cert, &i.key)), &key);
        match spec.issuer {
            IssuedBy::SelfSigned | IssuedBy::Root => self.counters.root_certs_issued += 1,
            IssuedBy::Ica => self.counters.ica_certs_issued += 1,
        }
        Authority {
            node,
            generation,
            cert,
            key,
            enc_scalar,
        }
    }

    /// Certifies the ICA and the root-issued components under the current home root, and
    /// the ICA-issued components under that ICA. `generation` keys them afresh.
    fn certify_hierarchy(
        &mut self,
        rng: &RngRegistry,
        online: &ScmsNodes,
        generation: u32,
        now32: u32,
    ) {
        let label = self.config.provider.clone();
        let root = self.home_root().authority.clone();
        let ica = self.make_authority(
            "ica",
            rng,
            self.nodes.ica,
            generation,
            Some(&root),
            &label,
            now32,
        );
        if let Some(old) = self.authorities.insert("ica", ica.clone()) {
            self.retired.push(old.cert);
        }
        for role in ROOT_ISSUED {
            let node = role_node(role, &self.nodes, online);
            let a = self.make_authority(role, rng, node, generation, Some(&root), &label, now32);
            if let Some(old) = self.authorities.insert(role, a) {
                self.retired.push(old.cert);
            }
        }
        self.certify_ica_issued(rng, online, generation, now32);
    }

    fn certify_ica_issued(
        &mut self,
        rng: &RngRegistry,
        online: &ScmsNodes,
        generation: u32,
        now32: u32,
    ) {
        let label = self.config.provider.clone();
        let ica = self.authorities["ica"].clone();
        for role in ICA_ISSUED {
            let node = role_node(role, &self.nodes, online);
            let a = self.make_authority(role, rng, node, generation, Some(&ica), &label, now32);
            if let Some(old) = self.authorities.insert(role, a) {
                self.retired.push(old.cert);
            }
        }
    }

    fn home_root(&self) -> &RootSlot {
        self.roots
            .iter()
            .rev()
            .find(|r| r.label == self.config.provider && r.approved)
            .expect("the home provider has an approved root")
    }

    fn approved_signers(&self) -> Vec<u32> {
        self.electors
            .iter()
            .enumerate()
            .filter(|(_, e)| e.approved)
            .map(|(k, _)| u32::try_from(k).unwrap_or(u32::MAX))
            .collect()
    }

    /// Builds and signs a CTL listing the approved electors and roots.
    fn build_ctl(
        &self,
        sequence: u32,
        effective: u32,
        removed_root: Option<[u8; 32]>,
        removed_elector: Option<[u8; 48]>,
        signers: &[u32],
    ) -> SignedCtl {
        let electors: Vec<[u8; 48]> = self
            .electors
            .iter()
            .filter(|e| e.approved)
            .map(|e| e.elector.hashed_id48())
            .collect();
        let approved_roots: Vec<&RootSlot> = self.roots.iter().filter(|r| r.approved).collect();
        let roots: Vec<[u8; 32]> = approved_roots
            .iter()
            .map(|r| cert_hashed_id32(&r.authority.cert).expect("root encodes"))
            .collect();
        let mut tbs = ctl::full_ieee_tbs_ctl(
            US_CTL_SERIES_ID,
            u16::try_from(sequence).unwrap_or(u16::MAX),
            effective,
            &electors,
            &roots,
            self.quorum,
        );
        if let Some(r) = removed_root {
            tbs.root_ca_remove = vec![
                v2xw_msg::sec_types::ieee1609_dot2_dot1_cert_management::CtlRootCaEntry(
                    v2xw_msg::sec_types::ieee1609_dot2_base_types::HashedId32(
                        FixedOctetString::from(r),
                    ),
                ),
            ];
        }
        if let Some(e) = removed_elector {
            tbs.elector_remove = vec![
                v2xw_msg::sec_types::ieee1609_dot2_dot1_cert_management::CtlElectorEntry(
                    v2xw_msg::sec_types::ieee1609_dot2_base_types::HashedId48(
                        FixedOctetString::from(e),
                    ),
                ),
            ];
        }
        let signing: Vec<&Elector> = signers
            .iter()
            .filter_map(|&k| self.electors.get(k as usize).map(|s| &s.elector))
            .collect();
        let root_certs = approved_roots
            .iter()
            .map(|r| r.authority.cert.clone())
            .collect();
        let m = ctl::build_ctl(&tbs, root_certs, &signing).expect("a CTL builds");
        SignedCtl {
            sequence,
            quorum: self.quorum,
            roots,
            electors,
            effective,
            signers: signers.to_vec(),
            spdu: ctl::encode_ctl_spdu(&m).expect("a CTL encodes"),
        }
    }

    fn signer(&self, role: &str) -> &Authority {
        self.authorities.get(role).expect("role certified at setup")
    }

    /// Every certificate a chain file carries: approved roots first, then the ICA, then
    /// every component in role order.
    fn chain_certs(&self) -> Vec<Certificate> {
        let mut out: Vec<Certificate> = self
            .roots
            .iter()
            .filter(|r| r.approved && r.label == self.config.provider)
            .map(|r| r.authority.cert.clone())
            .collect();
        out.push(self.authorities["ica"].cert.clone());
        for (role, a) in &self.authorities {
            if *role != "ica" {
                out.push(a.cert.clone());
            }
        }
        out
    }

    fn chain_file(&self, scope: Scope, version: u32) -> ChainFile {
        let home = ctl::decode_ctl_spdu(&self.ctl.spdu).expect("our CTL decodes");
        let certs = self.chain_certs();
        ChainFile {
            scope,
            version,
            ctl_sequence: self.ctl.sequence,
            spdu: ctl::encode_certificate_chain(&home.raw, certs.clone())
                .expect("a chain file encodes"),
            certs,
        }
    }

    fn policy_file(&self, scope: Scope, version: u32, policy: Policy, role: &str) -> PolicyFile {
        let a = self.signer(role);
        let d = sig1609::sign_spdu(
            sig1609::header_psid_only(SECURITY_MGMT_PSID),
            policy.payload(scope, version),
            SpduSigner::Certificate(a.cert.clone()),
            &a.key,
        )
        .expect("a policy SPDU signs");
        PolicyFile {
            scope,
            version,
            policy,
            signer: cert_hashed_id8(&a.cert).expect("encodes"),
            spdu: coer(&d).expect("encodes"),
        }
    }

    /// The Policy Generator signs a new Global Policy File and Global Certificate Chain
    /// File. Returns the new GPF version.
    pub fn sign_global(&mut self, policy: Policy) -> u32 {
        let version = self.gpf.version + 1;
        self.gpf = self.policy_file(Scope::Global, version, policy, "pg");
        self.gccf = self.chain_file(Scope::Global, self.gccf.version + 1);
        self.counters.pg_files_signed += 2;
        version
    }

    /// The Registration Authority re-issues its Local Policy File and Local Certificate
    /// Chain File from the current global ones. Returns the new LPF version.
    pub fn sign_local(&mut self) -> u32 {
        let version = self.lpf.version + 1;
        self.lpf = self.policy_file(Scope::Local, version, self.gpf.policy, "ra");
        self.lccf = self.chain_file(Scope::Local, self.lccf.version + 1);
        self.counters.ra_files_signed += 2;
        version
    }

    /// The SCMS Manager's policy decision: the Policy Generator signs a new global policy,
    /// and the RA a new local one. Returns the new LPF version.
    pub fn decide_policy(&mut self, policy: Policy) -> u32 {
        self.counters.manager_decisions += 1;
        self.sign_global(policy);
        self.sign_local()
    }

    /// The bootstrap anchors a device receives from the DCM: the currently approved
    /// elector certificates and the quorum.
    #[must_use]
    pub fn anchors(&self) -> TrustAnchors {
        TrustAnchors {
            electors: self
                .electors
                .iter()
                .filter(|e| e.approved)
                .map(|e| e.elector.cert.clone())
                .collect(),
            quorum: self.quorum,
            clock: self.config.clock,
        }
    }

    /// The current certificate of `role`.
    #[must_use]
    pub fn cert(&self, role: &str) -> Option<&Certificate> {
        if role == "root" {
            return Some(&self.home_root().authority.cert);
        }
        self.authorities.get(role).map(|a| &a.cert)
    }

    /// The `HashedId8` of `role`'s current certificate.
    #[must_use]
    pub fn cert_digest(&self, role: &str) -> Option<[u8; 8]> {
        self.cert(role).and_then(|c| cert_hashed_id8(c).ok())
    }

    /// The signing key of `role` (for flows that sign real SPDUs as that authority).
    #[must_use]
    pub fn signing_key(&self, role: &str) -> Option<&Signer1609> {
        if role == "root" {
            return Some(&self.home_root().authority.key);
        }
        self.authorities.get(role).map(|a| &a.key)
    }

    /// The ECIES private scalar behind `role`'s certificate `encryptionKey` (RA, MA).
    #[must_use]
    pub fn encryption_scalar(&self, role: &str) -> Option<[u8; 32]> {
        self.authorities.get(role).and_then(|a| a.enc_scalar)
    }

    /// The node holding `role`'s key and that key's generation.
    #[must_use]
    pub fn key_holder(&self, role: &str) -> Option<(NodeId, u32)> {
        self.authorities.get(role).map(|a| (a.node, a.generation))
    }

    /// Signs `hash` (ECDSA P-256 over the prehash) as the entity certified for `role`, for
    /// a flow that signs an artefact the device will check (the CRL Generator's list).
    #[must_use]
    pub fn sign_as(&self, role: &str, hash: &[u8; 32]) -> Option<([u8; 8], [u8; 64])> {
        let a = self.authorities.get(role)?;
        let Signer1609::P256(k) = &a.key else {
            return None;
        };
        let sig: EcdsaSig = k.sign_prehash(hash).ok()?;
        let mut out = [0u8; 64];
        out.copy_from_slice(&sig.to_bytes());
        Some((cert_hashed_id8(&a.cert).ok()?, out))
    }

    /// The current CTL re-signed by only the elector slots `electors`, for the negative
    /// tests a device must pass.
    #[must_use]
    pub fn ctl_endorsed_by(&self, electors: &[u32]) -> SignedCtl {
        self.build_ctl(self.ctl.sequence, self.ctl.effective, None, None, electors)
    }

    /// A CTL with the next sequence number, the current approvals, `quorum` for the CTL
    /// after it, signed by `electors`: what an attacker holding those elector keys could
    /// publish.
    #[must_use]
    pub fn forge_next_ctl(&self, quorum: u32, electors: &[u32]) -> SignedCtl {
        let mut g = self.shadow();
        g.quorum = quorum;
        g.build_ctl(
            self.ctl.sequence + 1,
            self.ctl.effective,
            None,
            None,
            electors,
        )
    }

    fn shadow(&self) -> Governance {
        Governance {
            nodes: self.nodes.clone(),
            config: self.config.clone(),
            electors: self.electors.clone(),
            roots: self.roots.clone(),
            authorities: self.authorities.clone(),
            retired: Vec::new(),
            quorum: self.quorum,
            ctl: self.ctl.clone(),
            ctl_history: Vec::new(),
            pending: Vec::new(),
            gpf: self.gpf.clone(),
            gccf: self.gccf.clone(),
            lpf: self.lpf.clone(),
            lccf: self.lccf.clone(),
            counters: self.counters,
            log: Vec::new(),
        }
    }

    fn next_sequence(&self) -> u32 {
        self.pending
            .iter()
            .map(|(_, c)| c.sequence)
            .max()
            .unwrap_or(self.ctl.sequence)
            + 1
    }

    fn decide(
        &mut self,
        kind: CtlChangeKind,
        decided_at: SimTime,
        signers: &[u32],
        removed_root: Option<[u8; 32]>,
        removed_elector: Option<[u8; 48]>,
        detail: String,
    ) -> SimTime {
        self.counters.manager_decisions += 1;
        let at = ctl_publication_deadline(kind, decided_at, self.config.clock);
        let effective = self.config.clock.time32(at).unwrap_or(0);
        let seq = self.next_sequence();
        let ctl = self.build_ctl(seq, effective, removed_root, removed_elector, signers);
        self.counters.elector_endorsements += u32::try_from(signers.len()).unwrap_or(0);
        self.log.push(GovernanceEvent {
            decided_at,
            published_at: Some(at),
            kind: match kind {
                CtlChangeKind::Genesis => "ctl-genesis",
                CtlChangeKind::AddRoot => "eac-add-root",
                CtlChangeKind::RemoveRoot => "eac-remove-root",
                CtlChangeKind::ReplaceElector => "eac-replace-elector",
            },
            ctl_sequence: Some(seq),
            signers: signers.to_vec(),
            detail,
        });
        self.pending.push((at, ctl));
        at
    }

    /// EAC vote: add a new provider's root, generated now (a fresh self-signed root under
    /// `label`). The CTL that approves it is published at the 3-business-day deadline.
    /// Returns the root's `HashedId32` and the publication time.
    pub fn add_root(
        &mut self,
        rng: &RngRegistry,
        label: &str,
        decided_at: SimTime,
    ) -> ([u8; 32], SimTime) {
        let now32 = self.config.clock.time32(decided_at).unwrap_or(0);
        let generation = u32::try_from(self.roots.len()).unwrap_or(u32::MAX);
        let node = NodeId::new(60 + generation);
        let root = self.make_authority("root", rng, node, generation, None, label, now32);
        let id = cert_hashed_id32(&root.cert).expect("encodes");
        self.roots.push(RootSlot {
            label: label.to_string(),
            authority: root,
            approved: true,
        });
        self.counters.roots_added += 1;
        let signers = self.approved_signers();
        let at = self.decide(
            CtlChangeKind::AddRoot,
            decided_at,
            &signers,
            None,
            None,
            label.to_string(),
        );
        (id, at)
    }

    /// EAC vote: remove the root with `HashedId32` `root`. It drops out of the next CTL
    /// (it is never put on a CRL, ISS CP §6.5.1), published within 48 h.
    pub fn remove_root(&mut self, root: [u8; 32], decided_at: SimTime) -> Option<SimTime> {
        let slot = self
            .roots
            .iter_mut()
            .find(|r| r.approved && cert_hashed_id32(&r.authority.cert).ok() == Some(root))?;
        slot.approved = false;
        let label = slot.label.clone();
        self.counters.roots_removed += 1;
        let signers = self.approved_signers();
        Some(self.decide(
            CtlChangeKind::RemoveRoot,
            decided_at,
            &signers,
            Some(root),
            None,
            label,
        ))
    }

    /// Home-provider root rollover (key changeover, Provider Req. §3.5.4): a new root key
    /// and certificate, the ICA and every component re-certified under it with fresh
    /// keys, and an EAC vote adding the new root. The old root stays approved until
    /// [`Governance::remove_root`]. Returns `(old root, new root, publication time)`.
    pub fn rollover_root(
        &mut self,
        rng: &RngRegistry,
        online: &ScmsNodes,
        decided_at: SimTime,
    ) -> ([u8; 32], [u8; 32], SimTime) {
        let now32 = self.config.clock.time32(decided_at).unwrap_or(0);
        let old = cert_hashed_id32(&self.home_root().authority.cert).expect("encodes");
        let generation = 1 + self
            .roots
            .iter()
            .filter(|r| r.label == self.config.provider)
            .map(|r| r.authority.generation)
            .max()
            .unwrap_or(0);
        let label = self.config.provider.clone();
        let root = self.make_authority(
            "root",
            rng,
            self.nodes.root,
            generation,
            None,
            &label,
            now32,
        );
        let new = cert_hashed_id32(&root.cert).expect("encodes");
        self.roots.push(RootSlot {
            label: label.clone(),
            authority: root,
            approved: true,
        });
        self.certify_hierarchy(rng, online, generation, now32);
        self.counters.roots_added += 1;
        let signers = self.approved_signers();
        let at = self.decide(
            CtlChangeKind::AddRoot,
            decided_at,
            &signers,
            None,
            None,
            format!("{label} rollover"),
        );
        (old, new, at)
    }

    /// ICA re-key: a new ICA certificate under the current root and every ICA-issued
    /// component re-certified (no CTL change: the CTL names roots only).
    pub fn reissue_ica(&mut self, rng: &RngRegistry, online: &ScmsNodes, decided_at: SimTime) {
        let now32 = self.config.clock.time32(decided_at).unwrap_or(0);
        let generation = self.authorities["ica"].generation + 1;
        let label = self.config.provider.clone();
        let root = self.home_root().authority.clone();
        let ica = self.make_authority(
            "ica",
            rng,
            self.nodes.ica,
            generation,
            Some(&root),
            &label,
            now32,
        );
        if let Some(old) = self.authorities.insert("ica", ica) {
            self.retired.push(old.cert);
        }
        self.certify_ica_issued(rng, online, generation, now32);
        self.log.push(GovernanceEvent {
            decided_at,
            published_at: None,
            kind: "ica-reissue",
            ctl_sequence: None,
            signers: Vec::new(),
            detail: format!("generation {generation}"),
        });
    }

    /// EAC vote: remove elector slot `slot` and add a fresh elector in a new slot. The CTL
    /// is signed by the electors approved *before* the change, the removed one excluded
    /// (it may be compromised), and lists the removed elector in `electorRemove`.
    /// Returns the new slot and the publication time.
    pub fn replace_elector(
        &mut self,
        rng: &RngRegistry,
        slot: u32,
        decided_at: SimTime,
    ) -> Option<(u32, SimTime)> {
        let old = self.electors.get(slot as usize)?;
        if !old.approved {
            return None;
        }
        let removed = old.elector.hashed_id48();
        let signers: Vec<u32> = self
            .approved_signers()
            .into_iter()
            .filter(|&k| k != slot)
            .collect();
        let new_slot = u32::try_from(self.electors.len()).unwrap_or(u32::MAX);
        let now32 = self.config.clock.time32(decided_at).unwrap_or(0);
        let years = ELECTOR_YEARS[ELECTOR_YEARS.len() - 1];
        let e = ctl::elector(
            &format!("v2xelector{}.manager.sim", new_slot + 1),
            now32,
            years,
            &elector_key(rng, new_slot),
        )
        .ok()?;
        self.electors[slot as usize].approved = false;
        self.electors.push(ElectorSlot {
            node: elector_node(new_slot),
            elector: e,
            approved: true,
        });
        self.nodes.electors = self
            .electors
            .iter()
            .filter(|s| s.approved)
            .map(|s| s.node)
            .collect();
        self.counters.elector_changes += 1;
        let at = self.decide(
            CtlChangeKind::ReplaceElector,
            decided_at,
            &signers,
            None,
            Some(removed),
            format!("slot {slot} -> slot {new_slot}"),
        );
        Some((new_slot, at))
    }

    /// The yearly test ballot (Elector Policy §1.1): every approved elector signs the
    /// current CTL's contents to prove its key and HSM still work. Returns the slots that
    /// produced a valid signature.
    pub fn test_ballot(&mut self, at: SimTime) -> Vec<u32> {
        let signers = self.approved_signers();
        let c = self.ctl_endorsed_by(&signers);
        let decoded = ctl::decode_ctl_spdu(&c.spdu).expect("our CTL decodes");
        let trust = CtlTrust {
            series: US_CTL_SERIES_ID,
            electors: c.electors.iter().copied().collect(),
            quorum: self.quorum,
            sequence: None,
            roots: BTreeSet::new(),
        };
        let v = ctl::verify_ctl(&decoded, &trust);
        let ok: Vec<u32> = signers
            .iter()
            .zip(v.electors.iter())
            .filter(|(_, e)| e.ok())
            .map(|(&k, _)| k)
            .collect();
        self.counters.test_ballots += 1;
        self.counters.elector_endorsements += u32::try_from(signers.len()).unwrap_or(0);
        self.log.push(GovernanceEvent {
            decided_at: at,
            published_at: None,
            kind: "test-ballot",
            ctl_sequence: Some(self.ctl.sequence),
            signers: ok.clone(),
            detail: format!("{} of {} responded", ok.len(), signers.len()),
        });
        ok
    }

    /// Publishes every decided CTL whose deadline is at or before `now`, in sequence
    /// order, and re-issues the chain files around it. Returns how many were published.
    pub fn publish_due(&mut self, now: SimTime) -> u32 {
        self.pending.sort_by_key(|(_, c)| c.sequence);
        let mut n = 0;
        while self.pending.first().is_some_and(|(at, _)| *at <= now) {
            let (_, c) = self.pending.remove(0);
            self.ctl = c.clone();
            self.ctl_history.push(c);
            self.counters.ctls_published += 1;
            n += 1;
        }
        if n > 0 {
            let policy = self.gpf.policy;
            self.sign_global(policy);
            self.sign_local();
        }
        n
    }

    /// The published CTLs after `sequence`, for a device catching up.
    #[must_use]
    pub fn ctls_after(&self, sequence: Option<u32>) -> Vec<SignedCtl> {
        self.ctl_history
            .iter()
            .filter(|c| sequence.is_none_or(|s| c.sequence > s))
            .cloned()
            .collect()
    }
}

// ---------------------------------------------------------------------------------------
// The device
// ---------------------------------------------------------------------------------------

/// A device's trust store: its anchors, the trust list, the chain and the policy it holds.
#[derive(Debug, Clone, Default)]
pub struct DeviceTrust {
    /// The bootstrap elector certificates and quorum (Profile §2.1.15).
    pub anchors: TrustAnchors,
    /// The CTL trust in force (electors, quorum, sequence, approved roots).
    pub ctl_trust: Option<CtlTrust>,
    /// The sequence number of the trust list installed.
    pub ctl_sequence: Option<u32>,
    /// Approved root certificates, by `HashedId32` (from the CTL's `unsigned` material).
    pub roots: BTreeMap<[u8; 32], Certificate>,
    /// Authority certificates whose chain verified, by `HashedId8`.
    pub chain: BTreeMap<[u8; 8], Certificate>,
    /// The root each chain certificate is anchored at.
    pub anchored_at: BTreeMap<[u8; 8], [u8; 32]>,
    /// The installed Local Certificate Chain File's version.
    pub lccf_version: Option<u32>,
    /// The installed Local Policy File.
    pub lpf: Option<PolicyFile>,
    /// Artefacts refused.
    pub rejected: u32,
    /// Why the last one was refused.
    pub last_rejection: Option<TrustError>,
    /// Signature verifications the device performed on trust artefacts.
    pub verifications: u32,
    /// Chain certificates discarded because their root left the CTL.
    pub purged: u32,
    /// Chain-rule findings tolerated as the published-RA conflict
    /// ([`request_length_conflict`]).
    pub tolerated: u32,
}

fn chain_error(kind: &ChainErrorKind) -> TrustError {
    match kind {
        ChainErrorKind::Expired => TrustError::Expired,
        ChainErrorKind::UntrustedRoot
        | ChainErrorKind::NotRootedInSelfSigned
        | ChainErrorKind::IssuerMismatch => TrustError::UnknownIssuer,
        ChainErrorKind::BadSignature
        | ChainErrorKind::BadRootSignature
        | ChainErrorKind::BadReconstruction => TrustError::BadSignature,
        ChainErrorKind::Malformed | ChainErrorKind::Empty | ChainErrorKind::TooLong => {
            TrustError::Malformed
        }
        _ => TrustError::NotPermitted,
    }
}

/// Every published US RA certificate (Autocrypt, ISS and SAESOL test; SAESOL production)
/// carries a request group `{all, minChainLength 0, chainLengthRange 0, eeType app}`
/// (production puts it in `certIssuePermissions`) under an ICA whose `all` group has
/// `minChainLength 2`, and [`chain::validate`] refuses it twice (`minChainLength below 1`;
/// the request group's chain length). Deployed devices evidently accept these RAs, so the
/// device tolerates exactly those two findings on a certificate that issues nothing and
/// counts them in [`DeviceTrust::tolerated`]. **Conflict reported** (WP-A3 rule vs the
/// deployment; 02 §5 states the rule for `certIssuePermissions` only).
fn request_length_conflict(cert: &Certificate, e: &chain::ChainError) -> bool {
    let t = &cert.0.to_be_signed;
    e.index == 0
        && e.kind == ChainErrorKind::ChainLength
        && t.cert_issue_permissions.is_none()
        && t.cert_request_permissions.is_some()
        && (e.detail == "minChainLength below 1" || e.detail.starts_with("request "))
}

fn issuer_id8(c: &Certificate) -> Option<[u8; 8]> {
    match &c.0.issuer {
        IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&d.0[..]);
            Some(b)
        }
        _ => None,
    }
}

impl DeviceTrust {
    fn reject(&mut self, e: TrustError) -> TrustError {
        self.rejected += 1;
        self.last_rejection = Some(e);
        e
    }

    fn bootstrap_trust(&self) -> CtlTrust {
        CtlTrust {
            series: US_CTL_SERIES_ID,
            electors: self
                .anchors
                .electors
                .iter()
                .filter_map(|c| cert_hashed_id48(c).ok())
                .collect(),
            quorum: self.anchors.quorum,
            sequence: None,
            roots: BTreeSet::new(),
        }
    }

    /// Installs a trust list that a quorum of the electors the device trusts signed.
    ///
    /// # Errors
    /// [`TrustError::CtlQuorum`] below quorum; [`TrustError::Stale`] for a list that is
    /// not newer than the installed one; [`TrustError::CtlMalformed`] for another series.
    pub fn install_ctl(&mut self, ctl: &SignedCtl) -> Result<u32, TrustError> {
        if self.ctl_sequence.is_some_and(|s| ctl.sequence <= s) {
            return Err(self.reject(TrustError::Stale));
        }
        let Ok(decoded) = ctl::decode_ctl_spdu(&ctl.spdu) else {
            return Err(self.reject(TrustError::CtlMalformed));
        };
        self.install_decoded_ctl(&decoded)
    }

    fn install_decoded_ctl(&mut self, decoded: &Ctl) -> Result<u32, TrustError> {
        let trust = self
            .ctl_trust
            .clone()
            .unwrap_or_else(|| self.bootstrap_trust());
        self.verifications += u32::try_from(decoded.signatures.len()).unwrap_or(u32::MAX);
        let v = ctl::verify_ctl(decoded, &trust);
        if !v.series_ok || !v.type_ok {
            return Err(self.reject(TrustError::CtlMalformed));
        }
        if !v.sequence_ok {
            return Err(self.reject(TrustError::Stale));
        }
        if !v.accepted() {
            return Err(self.reject(TrustError::CtlQuorum {
                valid: v.valid_signatures,
                needed: v.quorum,
            }));
        }
        let next = ctl::trust_after(decoded, &trust);
        let mut roots = BTreeMap::new();
        for c in &decoded.certificates {
            self.verifications += 1;
            if let Ok(h) = cert_hashed_id32(c)
                && next.roots.contains(&h)
                && sig1609::verify_explicit_cert(c, None)
            {
                roots.insert(h, c.clone());
            }
        }
        // Keep a still-approved root the device already holds even if this CTL omits its
        // certificate from `unsigned`.
        for (h, c) in &self.roots {
            if next.roots.contains(h) {
                roots.entry(*h).or_insert_with(|| c.clone());
            }
        }
        self.roots = roots;
        let before = self.chain.len();
        let keep: BTreeSet<[u8; 8]> = self
            .anchored_at
            .iter()
            .filter(|(_, r)| next.roots.contains(*r))
            .map(|(d, _)| *d)
            .collect();
        self.chain.retain(|d, _| keep.contains(d));
        self.anchored_at.retain(|d, _| keep.contains(d));
        self.purged += u32::try_from(before - self.chain.len()).unwrap_or(u32::MAX);
        self.ctl_sequence = Some(u32::from(decoded.sequence()));
        self.ctl_trust = Some(next);
        Ok(v.valid_signatures)
    }

    /// Installs a certificate chain file: its `homeCtl` if newer, then every certificate,
    /// each validated by the full 1609.2 chain rules up to an approved root and current at
    /// `now`. All or nothing. Returns the verifications it took.
    ///
    /// # Errors
    /// [`TrustError::UnknownIssuer`], [`TrustError::BadSignature`], [`TrustError::Stale`],
    /// [`TrustError::Expired`], [`TrustError::NotPermitted`], [`TrustError::Malformed`],
    /// or the `homeCtl`'s refusal.
    pub fn install_chain(&mut self, file: &ChainFile, now: SimTime) -> Result<u32, TrustError> {
        if self.lccf_version.is_some_and(|v| file.version <= v) {
            return Err(self.reject(TrustError::Stale));
        }
        let Ok((home, others)) = ctl::decode_certificate_chain(&file.spdu) else {
            return Err(self.reject(TrustError::Malformed));
        };
        if self
            .ctl_sequence
            .is_none_or(|s| u32::from(home.sequence()) > s)
        {
            self.install_decoded_ctl(&home)?;
        }
        let Ok(now32) = self.anchors.clock.time32(now) else {
            return Err(self.reject(TrustError::Expired));
        };
        let mut pool: BTreeMap<[u8; 8], Certificate> = self.chain.clone();
        for c in self.roots.values().chain(others.iter()) {
            if let Ok(d) = cert_hashed_id8(c) {
                pool.insert(d, c.clone());
            }
        }
        let policy = ChainPolicy {
            anchors: Some(self.roots.keys().copied().collect()),
            now: Some(now32),
        };
        let mut accepted = Vec::with_capacity(others.len());
        let mut checks = 0u32;
        for c in &others {
            let mut path = vec![c.clone()];
            while let Some(i) = issuer_id8(path.last().expect("non-empty")) {
                let Some(next) = pool.get(&i) else {
                    return Err(self.reject(TrustError::UnknownIssuer));
                };
                if path.len() >= chain::MAX_CHAIN_LENGTH {
                    return Err(self.reject(TrustError::Malformed));
                }
                path.push(next.clone());
            }
            checks += u32::try_from(path.len()).unwrap_or(u32::MAX);
            if let Err(errs) = chain::validate(&path, &policy) {
                let (tolerated, real): (Vec<_>, Vec<_>) = errs
                    .iter()
                    .partition(|e| request_length_conflict(&path[0], e));
                if let Some(e) = real.first() {
                    return Err(self.reject(chain_error(&e.kind)));
                }
                self.tolerated += u32::try_from(tolerated.len()).unwrap_or(u32::MAX);
            }
            let root = cert_hashed_id32(path.last().expect("non-empty"))
                .map_err(|_| TrustError::Malformed)?;
            accepted.push((
                cert_hashed_id8(c).map_err(|_| TrustError::Malformed)?,
                root,
                c.clone(),
            ));
        }
        self.verifications += checks;
        for (d, root, c) in accepted {
            self.chain.insert(d, c);
            self.anchored_at.insert(d, root);
        }
        self.lccf_version = Some(file.version);
        Ok(checks)
    }

    /// Installs a policy file: a 1609.2 SPDU signed under PSID 35 by a certificate in the
    /// device's chain whose SSP is the PG's (global) or the RA's (local); the policy is
    /// read from the signed payload.
    ///
    /// # Errors
    /// [`TrustError::UnknownIssuer`], [`TrustError::BadSignature`], [`TrustError::Stale`],
    /// [`TrustError::WrongSigner`], [`TrustError::Malformed`].
    pub fn install_policy(&mut self, file: &PolicyFile) -> Result<(), TrustError> {
        if self.lpf.as_ref().is_some_and(|l| file.version <= l.version) {
            return Err(self.reject(TrustError::Stale));
        }
        let Some(opened) = rasn::coer::decode::<Ieee1609Dot2Data>(&file.spdu)
            .ok()
            .and_then(|d| sig1609::open_signed(&d))
        else {
            return Err(self.reject(TrustError::Malformed));
        };
        let signer = match &opened.signer {
            SpduSigner::Certificate(c) => cert_hashed_id8(c).ok(),
            SpduSigner::Digest(d) => Some(*d),
            SpduSigner::SelfSigned => None,
        };
        let Some(cert) = signer.and_then(|d| self.chain.get(&d)).cloned() else {
            return Err(self.reject(TrustError::UnknownIssuer));
        };
        let want = match file.scope {
            Scope::Global => "pg",
            Scope::Local => "ra",
        };
        if opened.psid != SECURITY_MGMT_PSID || role_of(&cert) != Some(want) {
            return Err(self.reject(TrustError::WrongSigner));
        }
        self.verifications += 1;
        if !sig1609::verify_opened(&opened, &cert) {
            return Err(self.reject(TrustError::BadSignature));
        }
        // What the device follows is what was signed, not the file's labels.
        match Policy::from_payload(&opened.payload) {
            Some((scope, version, policy))
                if scope == file.scope && version == file.version && policy == file.policy =>
            {
                self.lpf = Some(file.clone());
                Ok(())
            }
            Some(_) => Err(self.reject(TrustError::BadSignature)),
            None => Err(self.reject(TrustError::Malformed)),
        }
    }

    /// Whether a P-256 signature over `hash` by the authority whose certificate
    /// `HashedId8` is `signer` verifies against the device's chain: how a device checks a
    /// CRL.
    pub fn verify_signed(&mut self, signer: [u8; 8], hash: &[u8; 32], sig: &[u8; 64]) -> bool {
        self.verifications += 1;
        let Some(cert) = self.chain.get(&signer) else {
            return false;
        };
        let Some((sig1609::Hash1609::Sha256, key)) = sig1609::explicit_key(cert) else {
            return false;
        };
        let (Ok(k), Ok(s)) = (
            VerifyingKey::from_sec1_bytes(&key),
            EcdsaSig::from_slice(sig),
        ) else {
            return false;
        };
        k.verify_prehash(hash, &s).is_ok()
    }

    /// The policy the device follows, if it has one.
    #[must_use]
    pub fn policy(&self) -> Option<Policy> {
        self.lpf.as_ref().map(|l| l.policy)
    }

    /// Whether the device holds a verified certificate whose SSP is `role`'s.
    #[must_use]
    pub fn trusts_role(&self, role: &str) -> bool {
        if role == "root" {
            return self.roots.values().any(|c| role_of(c) == Some("root"));
        }
        self.chain.values().any(|c| role_of(c) == Some(role))
    }
}

#[cfg(test)]
mod tests;
