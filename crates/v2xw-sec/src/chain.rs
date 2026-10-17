//! IEEE 1609.2 certificate chain validation (WP-A3; docs/design/scms-real/02 §12).
//!
//! A chain is `[EE, issuer of EE, …, root]`: each certificate's issuer is the next one,
//! and the last is a self-signed explicit root that must be on the trust list (a CTL names
//! roots by HashedId32, 02 §4). [`validate`] applies, and reports by name, every rule:
//!
//! | Rule | Source | [`ChainErrorKind`] |
//! |---|---|---|
//! | at most 8 certificates | 1609.2 PICS "Maximum Full Certificate Chain Length 8" | `TooLong` |
//! | well-formed: version 3, explicit ⇔ `verificationKey` ⇔ signature, implicit ⇔ `reconstructionValue` ⇔ no signature, at least one permission field, one `all` group at most, each PSID once, `minChainLength ≥ 1` | 02 §5 | `Malformed` |
//! | chain ends at a self-signed explicit root whose signature verifies | §5.1.2.1, §5.1.2.3 | `NotRootedInSelfSigned`, `BadRootSignature` |
//! | the root is a trust anchor (HashedId32 on the CTL) | §5.1.2.4a; 01 §3.1 | `UntrustedRoot` |
//! | the issuer field names the next certificate's HashedId8, with the hash of its key | `IssuerIdentifier` note | `IssuerMismatch` |
//! | explicit: ECDSA over `H(H(tbs) ‖ H(issuer))` verifies; implicit: ECQV-1609 reconstruction succeeds | §5.1.2.3, 02 §6 | `BadSignature`, `BadReconstruction` |
//! | every `appPermissions`, `certIssuePermissions` and `certRequestPermissions` PSID is covered by an issuer `certIssuePermissions` group (explicit PSID first, else the `all` group) | §5.1.2.4 | `PermissionNotCovered` |
//! | that group's `eeType` allows the use (app for appPermissions, enrol for certRequestPermissions, ⊇ the subordinate's eeType for certIssuePermissions) | §5.1.2.4 | `EeTypeNotAllowed` |
//! | SSP consistency: omitted, opaque, bitmap, and SSP-range rules | `PsidSsp`, `PsidSspRange`, `SspRange`, `BitmapSspRange` notes | `SspNotCovered` |
//! | `mcr_i ≤ mcr_s + 1` and `mcr_i + clr_i ≥ mcr_s + clr_s + 1` (EE: 0, 0; `clr = −1` unbounded) | §5.1.2.4; 02 §12 | `ChainLength` |
//! | validity period inside the issuer's | §5.1.2.4 | `ValidityNotContained` |
//! | region inside the issuer's effective region (absent ⇒ inherited; absent at the root ⇒ worldwide) | §5.1.2.4; S1 note | `RegionNotContained`, `RegionUnsupported` |
//! | optionally: every certificate current at `now` | §5.1.2.4a ("strongly recommended") | `Expired` |
//!
//! The same-ACA rule for butterfly responses is [`same_aca_response`]; the SPDU-level
//! PSID check is [`ee_permits_psid`].
//!
//! # Where the text is UNCONFIRMED (02 §12)
//!
//! The "contained in" wording for validity and region was elided in the copy of §5.1.2.4
//! the research had; the rules here are standard practice. Geometric regions (circular,
//! rectangular, polygonal) are contained only when identical to the issuer's; any other
//! geometric pairing is reported as `RegionUnsupported` and fails closed.
//!
//! # Cross-check
//!
//! `tests/chain_xcheck.rs` runs this validator over 500 random chains the independent
//! oracle (tools/oracle) built and judged, and requires the same verdict on every one.

use std::collections::BTreeSet;

use v2xw_msg::sec_types::ieee1609_dot2::{
    SequenceOfPsidGroupPermissions, SubjectPermissions, VerificationKeyIndicator,
};
use v2xw_msg::sec_types::ieee1609_dot2_base_types::{
    GeographicRegion, IdentifiedRegion, PsidSspRange, ServiceSpecificPermissions, SspRange,
};
use v2xw_msg::sec_types::{
    Certificate, CertificateType, IssuerIdentifier, PsidGroupPermissions, ToBeSignedCertificate,
};

use crate::cert::{self, CertHash, CertKey};

/// The maximum certificate chain length (1609.2 PICS, 02 §12).
pub const MAX_CHAIN_LENGTH: usize = 8;

/// What a chain rule failure is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChainErrorKind {
    /// No certificates at all.
    Empty,
    /// More than [`MAX_CHAIN_LENGTH`] certificates.
    TooLong,
    /// A certificate is not well formed (02 §5).
    Malformed,
    /// The last certificate is not a self-signed explicit certificate.
    NotRootedInSelfSigned,
    /// The root's self-signature does not verify.
    BadRootSignature,
    /// The root is not on the trust list.
    UntrustedRoot,
    /// A certificate's issuer field does not name the next certificate.
    IssuerMismatch,
    /// An explicit certificate's signature does not verify under its issuer.
    BadSignature,
    /// An implicit certificate's key cannot be reconstructed.
    BadReconstruction,
    /// A permission is not covered by any issuer `certIssuePermissions` group.
    PermissionNotCovered,
    /// The covering group's `eeType` does not allow the use.
    EeTypeNotAllowed,
    /// An SSP (or SSP range) is not consistent with the issuer's range.
    SspNotCovered,
    /// The chain-length inequalities fail, or `minChainLength` is 0.
    ChainLength,
    /// The validity period is not inside the issuer's.
    ValidityNotContained,
    /// The region is not inside the issuer's effective region.
    RegionNotContained,
    /// A geometric region pairing this validator does not decide (fails closed).
    RegionUnsupported,
    /// A certificate is not valid at the policy's `now`.
    Expired,
}

/// One rule failure: which certificate (0 = the end entity) and what.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ChainError {
    /// Index in the chain, 0 being the end-entity certificate.
    pub index: usize,
    /// The rule.
    pub kind: ChainErrorKind,
    /// A human-readable detail.
    pub detail: String,
}

/// What the validator checks beyond the always-on rules.
#[derive(Debug, Clone, Default)]
pub struct ChainPolicy {
    /// The trust list: HashedId32 of every accepted root. `None` skips the anchor rule
    /// (used only by tests that check the other rules in isolation).
    pub anchors: Option<BTreeSet<[u8; 32]>>,
    /// When set, every certificate must be valid at this Time32 (TAI seconds since
    /// 2004-01-01).
    pub now: Option<u32>,
}

impl ChainPolicy {
    /// A policy trusting exactly the given roots.
    pub fn trusting<'a>(roots: impl IntoIterator<Item = &'a Certificate>) -> crate::Result<Self> {
        let mut anchors = BTreeSet::new();
        for r in roots {
            anchors.insert(cert::hashed_id32(r)?);
        }
        Ok(ChainPolicy {
            anchors: Some(anchors),
            now: None,
        })
    }
}

/// A chain that passed: the resolved verification key of every certificate, index-aligned
/// with the input (so `keys[0]` verifies the SPDU).
#[derive(Debug, Clone)]
pub struct ValidatedChain {
    /// The verification key of each certificate.
    pub keys: Vec<CertKey>,
}

impl ValidatedChain {
    /// The end-entity key: the one an SPDU signed under `chain[0]` verifies with.
    pub fn ee_key(&self) -> &CertKey {
        &self.keys[0]
    }
}

fn err(index: usize, kind: ChainErrorKind, detail: impl Into<String>) -> ChainError {
    ChainError {
        index,
        kind,
        detail: detail.into(),
    }
}

fn psid_u64(p: &v2xw_msg::sec_types::Psid) -> u64 {
    u64::try_from(&p.0).unwrap_or(u64::MAX)
}

fn int(i: &rasn::types::Integer) -> i64 {
    i64::try_from(i).unwrap_or(i64::MAX)
}

fn groups(t: &ToBeSignedCertificate) -> &[PsidGroupPermissions] {
    t.cert_issue_permissions
        .as_ref()
        .map(|g: &SequenceOfPsidGroupPermissions| g.0.as_slice())
        .unwrap_or(&[])
}

fn request_groups(t: &ToBeSignedCertificate) -> &[PsidGroupPermissions] {
    t.cert_request_permissions
        .as_ref()
        .map(|g| g.0.as_slice())
        .unwrap_or(&[])
}

/// `eeType` bits: app = bit 0, enrol = bit 1 (MSB first).
fn ee_bits(g: &PsidGroupPermissions) -> (bool, bool) {
    (g.ee_type.0[0], g.ee_type.0[1])
}

fn is_all(g: &PsidGroupPermissions) -> bool {
    matches!(g.subject_permissions, SubjectPermissions::all(()))
}

fn explicit_ranges(g: &PsidGroupPermissions) -> &[PsidSspRange] {
    match &g.subject_permissions {
        SubjectPermissions::explicit(s) => s.0.as_slice(),
        _ => &[],
    }
}

/// The issuer group covering `psid`, and the explicit range entry if the hit is explicit:
/// an explicit entry for the PSID wins; otherwise the `all` group covers PSIDs not listed
/// elsewhere (02 §5).
fn covering(
    issuer: &ToBeSignedCertificate,
    psid: u64,
) -> Option<(&PsidGroupPermissions, Option<&PsidSspRange>)> {
    for g in groups(issuer) {
        for r in explicit_ranges(g) {
            if psid_u64(&r.psid) == psid {
                return Some((g, Some(r)));
            }
        }
    }
    groups(issuer).iter().find(|g| is_all(g)).map(|g| (g, None))
}

/// The SSP rule for an `appPermissions` entry against an explicit issuer range entry.
fn ssp_consistent(ssp: Option<&ServiceSpecificPermissions>, range: &PsidSspRange) -> bool {
    let Some(range) = &range.ssp_range else {
        return true; // an omitted sspRange means all
    };
    match range {
        SspRange::all(()) => true,
        SspRange::opaque(list) => {
            let want: &[u8] = match ssp {
                None => &[],
                Some(ServiceSpecificPermissions::opaque(o)) => o,
                Some(_) => return false,
            };
            list.0.iter().any(|o| o.0.as_ref() == want)
        }
        SspRange::bitmapSspRange(br) => {
            let Some(ServiceSpecificPermissions::bitmapSsp(b)) = ssp else {
                return false;
            };
            let b = &b.0;
            if b.len() > br.ssp_value.len() || br.ssp_value.len() != br.ssp_bitmask.len() {
                return false;
            }
            br.ssp_bitmask.iter().enumerate().all(|(i, &mask)| {
                let have = b.get(i).copied().unwrap_or(0);
                // A shorter SSP must still reach the last set mask bit.
                (mask == 0 || i < b.len()) && (have & mask) == (br.ssp_value[i] & mask)
            })
        }
        _ => false,
    }
}

/// The SSP-range rule for a subordinate `certIssuePermissions` / `certRequestPermissions`
/// entry against an explicit issuer range entry.
fn range_consistent(sub: &PsidSspRange, iss: &PsidSspRange) -> bool {
    let iss_range = match &iss.ssp_range {
        None | Some(SspRange::all(())) => return true,
        Some(r) => r,
    };
    match (&sub.ssp_range, iss_range) {
        (Some(SspRange::opaque(s)), SspRange::opaque(i)) => {
            s.0.iter().all(|o| i.0.iter().any(|p| p.0 == o.0))
        }
        (Some(SspRange::bitmapSspRange(s)), SspRange::bitmapSspRange(i)) => {
            if s.ssp_bitmask.len() != i.ssp_bitmask.len() || s.ssp_value.len() != i.ssp_value.len()
            {
                return false;
            }
            i.ssp_bitmask.iter().enumerate().all(|(n, &m)| {
                (s.ssp_bitmask[n] & m) == m && (s.ssp_value[n] & m) == (i.ssp_value[n] & m)
            })
        }
        _ => false,
    }
}

/// `mcr_i ≤ mcr_s + 1` and `mcr_i + clr_i ≥ mcr_s + clr_s + 1`, `clr = −1` unbounded.
fn chain_length_ok(iss: (i64, i64), sub: (i64, i64)) -> bool {
    let (mi, ci) = iss;
    let (ms, cs) = sub;
    if mi > ms + 1 {
        return false;
    }
    if ci == -1 {
        return true;
    }
    cs != -1 && mi + ci >= ms + cs + 1
}

fn group_len(g: &PsidGroupPermissions) -> (i64, i64) {
    (int(&g.min_chain_length), int(&g.chain_length_range))
}

fn validity(t: &ToBeSignedCertificate) -> (u64, u64) {
    let start = u64::from(t.validity_period.start.0.0);
    (
        start,
        start.saturating_add(crate::envelope::duration_seconds(
            &t.validity_period.duration,
        )),
    )
}

fn well_formed(index: usize, c: &Certificate, errs: &mut Vec<ChainError>) {
    let b = &c.0;
    let t = &b.to_be_signed;
    let m = |d: &str| err(index, ChainErrorKind::Malformed, d);
    if b.version.0 != cert::CERT_VERSION {
        errs.push(m("version is not 3"));
    }
    let vki_explicit = matches!(
        t.verify_key_indicator,
        VerificationKeyIndicator::verificationKey(_)
    );
    let vki_implicit = matches!(
        t.verify_key_indicator,
        VerificationKeyIndicator::reconstructionValue(_)
    );
    match b.r_type {
        CertificateType::explicit if !(vki_explicit && b.signature.is_some()) => errs.push(m(
            "explicit certificate needs verificationKey and a signature",
        )),
        CertificateType::implicit if !(vki_implicit && b.signature.is_none()) => errs.push(m(
            "implicit certificate needs reconstructionValue and no signature",
        )),
        CertificateType::explicit | CertificateType::implicit => {}
        _ => errs.push(m("unknown certificate type")),
    }
    if t.app_permissions.is_none()
        && t.cert_issue_permissions.is_none()
        && t.cert_request_permissions.is_none()
    {
        errs.push(m(
            "none of appPermissions, certIssuePermissions, certRequestPermissions",
        ));
    }
    if let Some(app) = &t.app_permissions {
        let mut seen = BTreeSet::new();
        for p in &app.0 {
            if !seen.insert(psid_u64(&p.psid)) {
                errs.push(m("a PSID appears twice in appPermissions"));
            }
        }
    }
    for gs in [groups(t), request_groups(t)] {
        if gs.iter().filter(|g| is_all(g)).count() > 1 {
            errs.push(m("more than one `all` permissions group"));
        }
        for g in gs {
            if int(&g.min_chain_length) < 1 {
                errs.push(err(
                    index,
                    ChainErrorKind::ChainLength,
                    "minChainLength below 1",
                ));
            }
            if int(&g.chain_length_range) < -1 {
                errs.push(m("chainLengthRange below -1"));
            }
        }
    }
}

fn check_permissions(
    index: usize,
    sub: &ToBeSignedCertificate,
    iss: &ToBeSignedCertificate,
    errs: &mut Vec<ChainError>,
) {
    use ChainErrorKind::*;
    // The chain-length inequalities apply to the subordinate's certIssuePermissions
    // groups; a subordinate with none is an end entity (mcr = clr = 0) and is checked
    // against the groups covering its appPermissions / certRequestPermissions (02 §12).
    let is_ee = groups(sub).is_empty();
    if let Some(app) = &sub.app_permissions {
        for p in &app.0 {
            let psid = psid_u64(&p.psid);
            match covering(iss, psid) {
                None => errs.push(err(index, PermissionNotCovered, format!("app psid {psid}"))),
                Some((g, range)) => {
                    if !ee_bits(g).0 {
                        errs.push(err(index, EeTypeNotAllowed, format!("app psid {psid}")));
                    }
                    if let Some(r) = range
                        && !ssp_consistent(p.ssp.as_ref(), r)
                    {
                        errs.push(err(index, SspNotCovered, format!("app psid {psid}")));
                    }
                    if is_ee && !chain_length_ok(group_len(g), (0, 0)) {
                        errs.push(err(index, ChainLength, format!("app psid {psid}")));
                    }
                }
            }
        }
    }
    // certIssuePermissions and certRequestPermissions groups.
    for (request, gs) in [(false, groups(sub)), (true, request_groups(sub))] {
        for g in gs {
            let what = if request { "request" } else { "issue" };
            let sub_len = if request { (0, 0) } else { group_len(g) };
            let check_group =
                |ig: &PsidGroupPermissions, label: &str, errs: &mut Vec<ChainError>| {
                    let (app, enrol) = ee_bits(ig);
                    let (sapp, senrol) = ee_bits(g);
                    let allowed = if request {
                        enrol
                    } else {
                        (!sapp || app) && (!senrol || enrol)
                    };
                    if !allowed {
                        errs.push(err(index, EeTypeNotAllowed, format!("{what} {label}")));
                    }
                    if (!request || is_ee) && !chain_length_ok(group_len(ig), sub_len) {
                        errs.push(err(index, ChainLength, format!("{what} {label}")));
                    }
                };
            if is_all(g) {
                match groups(iss).iter().find(|x| is_all(x)) {
                    None => errs.push(err(
                        index,
                        PermissionNotCovered,
                        format!("{what} `all` needs an issuer `all` group"),
                    )),
                    Some(ig) => check_group(ig, "all", errs),
                }
                continue;
            }
            for r in explicit_ranges(g) {
                let psid = psid_u64(&r.psid);
                match covering(iss, psid) {
                    None => errs.push(err(
                        index,
                        PermissionNotCovered,
                        format!("{what} psid {psid}"),
                    )),
                    Some((ig, irange)) => {
                        if let Some(ir) = irange
                            && !range_consistent(r, ir)
                        {
                            errs.push(err(index, SspNotCovered, format!("{what} psid {psid}")));
                        }
                        check_group(ig, &format!("psid {psid}"), errs);
                    }
                }
            }
        }
    }
}

/// Whether identified region `s` is inside identified region `i`.
fn identified_covers(i: &IdentifiedRegion, s: &IdentifiedRegion) -> bool {
    use IdentifiedRegion as R;
    let country = |r: &R| match r {
        R::countryOnly(c) => Some(c.0.0),
        R::countryAndRegions(c) => Some(c.country_only.0.0),
        R::countryAndSubregions(c) => Some(c.country_only.0.0),
        _ => None,
    };
    if country(i).is_none() || country(i) != country(s) {
        return false;
    }
    match (i, s) {
        (R::countryOnly(_), _) => true,
        (R::countryAndRegions(ir), R::countryAndRegions(sr)) => {
            sr.regions.0.iter().all(|r| ir.regions.0.contains(r))
        }
        (R::countryAndRegions(ir), R::countryAndSubregions(ss)) => ss
            .region_and_subregions
            .0
            .iter()
            .all(|rs| ir.regions.0.contains(&rs.region)),
        (R::countryAndSubregions(is), R::countryAndSubregions(ss)) => {
            ss.region_and_subregions.0.iter().all(|rs| {
                is.region_and_subregions.0.iter().any(|irs| {
                    irs.region == rs.region
                        && rs.subregions.0.iter().all(|x| irs.subregions.0.contains(x))
                })
            })
        }
        _ => false,
    }
}

/// `Some(true/false)` when containment is decided, `None` when unsupported.
fn region_contained(sub: &GeographicRegion, iss: &GeographicRegion) -> Option<bool> {
    if sub == iss {
        return Some(true);
    }
    match (sub, iss) {
        (GeographicRegion::identifiedRegion(s), GeographicRegion::identifiedRegion(i)) => Some(
            s.0.iter()
                .all(|sr| i.0.iter().any(|ir| identified_covers(ir, sr))),
        ),
        _ => None,
    }
}

/// Validates `chain` (`[EE, …, root]`) under `policy`. On success, the resolved key of
/// every certificate; on failure, every rule that failed.
pub fn validate(
    chain: &[Certificate],
    policy: &ChainPolicy,
) -> core::result::Result<ValidatedChain, Vec<ChainError>> {
    use ChainErrorKind::*;
    let mut errs = Vec::new();
    if chain.is_empty() {
        return Err(vec![err(0, Empty, "no certificates")]);
    }
    if chain.len() > MAX_CHAIN_LENGTH {
        errs.push(err(
            chain.len() - 1,
            TooLong,
            format!("{} certificates, at most {MAX_CHAIN_LENGTH}", chain.len()),
        ));
    }
    for (n, c) in chain.iter().enumerate() {
        well_formed(n, c, &mut errs);
    }

    // The root.
    let last = chain.len() - 1;
    let root = &chain[last];
    let mut keys: Vec<Option<CertKey>> = vec![None; chain.len()];
    if !matches!(root.0.issuer, IssuerIdentifier::R_self(_))
        || root.0.r_type != CertificateType::explicit
    {
        errs.push(err(
            last,
            NotRootedInSelfSigned,
            "the last certificate is not self-signed explicit",
        ));
    } else {
        match cert::verify_explicit_cert(root, None) {
            Ok(true) => keys[last] = cert::verification_key(root, None).ok(),
            _ => errs.push(err(
                last,
                BadRootSignature,
                "root self-signature does not verify",
            )),
        }
        if let Some(anchors) = &policy.anchors {
            match cert::hashed_id32(root) {
                Ok(h) if anchors.contains(&h) => {}
                _ => errs.push(err(
                    last,
                    UntrustedRoot,
                    "root HashedId32 is not on the trust list",
                )),
            }
        }
    }

    // Links, from the root down, so each issuer key is resolved before its subordinate.
    let refs: Vec<&Certificate> = chain.iter().collect();
    for n in (0..last).rev() {
        let (sub, iss) = (&chain[n], &chain[n + 1]);
        let named = match &sub.0.issuer {
            IssuerIdentifier::sha256AndDigest(d) => Some((d, CertHash::Sha256)),
            IssuerIdentifier::sha384AndDigest(d) => Some((d, CertHash::Sha384)),
            _ => None,
        };
        let id_ok = match named {
            Some((d, _)) => cert::cert_id8(iss).map(|h| &h == d).unwrap_or(false),
            None => false,
        };
        if !id_ok {
            errs.push(err(
                n,
                IssuerMismatch,
                "issuer HashedId8 does not name the next certificate",
            ));
        }
        if let Some(ik) = keys[n + 1].clone()
            && id_ok
        {
            if named.map(|x| x.1) != Some(ik.hash()) {
                errs.push(err(
                    n,
                    IssuerMismatch,
                    "issuer hash choice does not match the issuer key",
                ));
            } else {
                match sub.0.r_type {
                    CertificateType::explicit => {
                        match cert::verify_explicit_cert(sub, Some((iss, &ik))) {
                            Ok(true) => keys[n] = cert::verification_key(sub, None).ok(),
                            _ => errs.push(err(n, BadSignature, "signature does not verify")),
                        }
                    }
                    _ => match cert::verification_key(sub, Some((iss, &ik))) {
                        Ok(k) => keys[n] = Some(k),
                        Err(e) => errs.push(err(n, BadReconstruction, e.to_string())),
                    },
                }
            }
        }
        pair_rules(n, &refs, &mut errs);
    }

    if let Some(now) = policy.now {
        for (n, c) in chain.iter().enumerate() {
            let (s, e) = validity(&c.0.to_be_signed);
            if u64::from(now) < s || u64::from(now) >= e {
                errs.push(err(n, Expired, "not valid at the policy time"));
            }
        }
    }

    if errs.is_empty() {
        Ok(ValidatedChain {
            keys: keys
                .into_iter()
                .map(|k| k.expect("every key resolved"))
                .collect(),
        })
    } else {
        errs.sort_by(|a, b| (a.index, a.kind).cmp(&(b.index, b.kind)));
        Err(errs)
    }
}

/// The issuer-consistency rules for the link `chain[n]` → `chain[n + 1]` that need no
/// cryptography: permissions, SSPs, chain length, validity and region containment (the
/// region inherited from the nearest ancestor in `chain` that has one).
fn pair_rules(n: usize, chain: &[&Certificate], errs: &mut Vec<ChainError>) {
    use ChainErrorKind::*;
    let (st, it) = (&chain[n].0.to_be_signed, &chain[n + 1].0.to_be_signed);
    check_permissions(n, st, it, errs);
    let ((s0, s1), (i0, i1)) = (validity(st), validity(it));
    if s0 < i0 || s1 > i1 {
        errs.push(err(
            n,
            ValidityNotContained,
            "validity period outside the issuer's",
        ));
    }
    if let Some(sr) = &st.region {
        // The issuer's effective region: its own, else the nearest ancestor's, else
        // worldwide.
        if let Some(ir) = chain[n + 1..]
            .iter()
            .find_map(|c| c.0.to_be_signed.region.as_ref())
        {
            match region_contained(sr, ir) {
                Some(true) => {}
                Some(false) => errs.push(err(n, RegionNotContained, "region outside the issuer's")),
                None => errs.push(err(n, RegionUnsupported, "geometric region containment")),
            }
        }
    }
}

/// The rules of [`validate`] that need no cryptography, over a chain segment
/// `[EE, …, X]` whose top `X` is either a trust anchor or a certificate already
/// validated: well-formedness, the length bound, issuer HashedId8 linkage, permissions,
/// SSPs, chain length, validity and region containment.
///
/// This is what [`crate::envelope::Envelope`]'s verification plan applies before it
/// schedules the signature and reconstruction work: the cryptographic links are the
/// plan's operations, and the consistency rules decide whether the plan is worth
/// running. Region inheritance looks only as far up as the segment reaches.
pub fn check_links(chain: &[&Certificate]) -> core::result::Result<(), Vec<ChainError>> {
    use ChainErrorKind::*;
    let mut errs = Vec::new();
    if chain.is_empty() {
        return Err(vec![err(0, Empty, "no certificates")]);
    }
    if chain.len() > MAX_CHAIN_LENGTH {
        errs.push(err(
            chain.len() - 1,
            TooLong,
            "longer than the 1609.2 bound",
        ));
    }
    for (n, c) in chain.iter().enumerate() {
        well_formed(n, c, &mut errs);
    }
    for n in 0..chain.len() - 1 {
        let id_ok = match &chain[n].0.issuer {
            IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
                cert::cert_id8(chain[n + 1]).is_ok_and(|h| &h == d)
            }
            _ => false,
        };
        if !id_ok {
            errs.push(err(
                n,
                IssuerMismatch,
                "issuer HashedId8 does not name the next certificate",
            ));
        }
        pair_rules(n, chain, &mut errs);
    }
    if errs.is_empty() { Ok(()) } else { Err(errs) }
}

/// The SPDU-level permission check (02 §12 rule 3): the end-entity certificate lists
/// `psid` in its `appPermissions`.
pub fn ee_permits_psid(ee: &Certificate, psid: u64) -> bool {
    ee.0.to_be_signed
        .app_permissions
        .as_ref()
        .is_some_and(|a| a.0.iter().any(|p| psid_u64(&p.psid) == psid))
}

/// The same-ACA response rule (1609.2.1 `AcaEeCertResponsePrivateSpdu`; 02 §12): the
/// response is valid only if signed by the certificate that issued the enclosed one, i.e.
/// the enclosed certificate's issuer field names the response signer's HashedId8.
pub fn same_aca_response(response_signer: &Certificate, issued: &Certificate) -> bool {
    let named = match &issued.0.issuer {
        IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => d,
        _ => return false,
    };
    cert::cert_id8(response_signer).is_ok_and(|h| &h == named)
}
