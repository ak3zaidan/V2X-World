//! CRL issuance (CRL Generator) and distribution (RA download, air broadcast).
//!
//! Split out of `run.rs` without behaviour change (WP-B1); the state types and the shared
//! helpers stay in [`super`].

use super::*;
use crate::scms::pdu::SpduKind;
use v2xw_sec::crl::{CrlBody, CrlSpec, LinkageRevocation};

/// The SHA-256 a CRL Generator signs: every entry, in order.
#[must_use]
pub fn crl_digest(entries: &[CrlLinkageEntry]) -> [u8; 32] {
    let mut b = Vec::with_capacity(entries.len() * 64);
    for e in entries {
        b.extend_from_slice(&e.i.to_be_bytes());
        b.extend_from_slice(&e.la_id1.0.to_be_bytes());
        b.extend_from_slice(&e.la_id2.0.to_be_bytes());
        b.extend_from_slice(e.ls1_i.as_bytes());
        b.extend_from_slice(e.ls2_i.as_bytes());
        b.extend_from_slice(&e.jmax.to_be_bytes());
    }
    sha256(&b)
}

/// The size, in bytes, of a CRL with `entries` entries under this deployment's size model.
pub fn crl_bytes(sizes: &ScmsSizes, entries: u32) -> WireSize {
    sizes.crl(entries)
}

/// The series-1 entries of a linkage CRL with `iRev = i_rev`: each entry's seeds walked
/// forward from its own period to `i_rev` (`ls(i+1) = H(la_id ‖ ls(i))`), and entries
/// whose last tracked period `i + max_forward` is before `i_rev` pruned as exhausted
/// ([CAMP122] §5.2.10.2.3). `iMax` is that last period.
#[must_use]
pub fn linkage_revocations(entries: &[CrlLinkageEntry], i_rev: u32) -> Vec<LinkageRevocation> {
    entries
        .iter()
        .filter(|e| e.i <= i_rev && e.i.saturating_add(e.max_forward) >= i_rev)
        .map(|e| {
            let walk = |la: LaId, ls: LinkageSeed| {
                (e.i..i_rev).fold(ls, |s, _| linkage::linkage_seed_next(la, s))
            };
            LinkageRevocation {
                la1: e.la_id1,
                la2: e.la_id2,
                ls1: walk(e.la_id1, e.ls1_i),
                ls2: walk(e.la_id2, e.ls2_i),
                jmax: u8::try_from(e.jmax).unwrap_or(u8::MAX),
                i_max: u16::try_from(e.i.saturating_add(e.max_forward)).unwrap_or(u16::MAX),
            }
        })
        .collect()
}

/// A CRL the CRL Generator signed (the issuance log attack studies read).
#[derive(Debug, Clone)]
pub struct IssuedCrl {
    /// Simulated time of signing.
    pub at: SimTime,
    /// `crlSeries`.
    pub series: u16,
    /// A delta CRL.
    pub delta: bool,
    /// `issueDate` / `nextCrl` (Time32).
    pub issue_date: u32,
    /// `nextCrl` (Time32).
    pub next_crl: u32,
    /// Entries it carries.
    pub entries: u32,
    /// The `SecuredCrl` bytes.
    pub pdu: crate::scms::pdu::Pdu,
}

/// One SCMS component's certificate put on its provider's component CRL (SC-P8.1).
///
/// [SMP] Table 1 and IEEE 1609.2 §5.1.3: a hash CRL revokes the certificates that name
/// its `crlCraca` (by the low 3 bytes, `cracaId`) and its `crlSeries`. Each component
/// certificate names its own: an ICA-issued component series 2 under the ICA (S2X) or
/// the root (ISS); a root-issued one (MA, PG, the ISS CRL signer) series 256 under the
/// root; the ICA itself series 2 under the root.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentRevocation {
    /// The component's role (`ica`, `pca`, `ra`, `eca`, `la1`, `la2`, `ma`, `crlg`,
    /// `dcm`).
    pub role: &'static str,
    /// When the revocation was decided.
    pub at: SimTime,
    /// The `crlSeries` its certificate names.
    pub series: u16,
    /// The `crlCraca` of the list it goes on: the HashedId8 of its CRACA.
    pub craca: [u8; 8],
    /// Its `HashedId10` and expiry.
    pub entry: v2xw_sec::crl::HashRevocation,
}

/// The component of `pki` that `role` names, by its canonical role name (`pca` and the
/// engine's host label `aca` are the same component).
#[must_use]
pub fn component_authority<'a>(
    pki: &'a crate::scms::pdu::ScmsPki,
    role: &str,
) -> Option<(&'static str, &'a crate::scms::pdu::Authority)> {
    Some(match role {
        "ica" => ("ica", &pki.ica),
        "pca" | "aca" => ("pca", &pki.aca),
        "ra" => ("ra", &pki.ra),
        "eca" => ("eca", &pki.eca),
        "la1" => ("la1", &pki.la1),
        "la2" => ("la2", &pki.la2),
        "ma" => ("ma", &pki.ma),
        "crlg" => ("crlg", &pki.crlg),
        "dcm" => ("dcm", &pki.dcm),
        _ => return None,
    })
}

/// The certificate that signs `pki`'s hash CRLs of `series`, and the `crlCraca` such a
/// list carries: the first of the CRL signer and the root whose `CrlSsp` (version 1)
/// lists the series, with the CRACA its `associatedCraca` names (itself for `isCraca`,
/// its issuer for `issuerIsCraca`).
///
/// The published certificates decide it ([SMP] Table 1 rows 2 and 256 name no signer):
/// the CRL signer holds `{issuerIsCraca, [4, 3, 1, 2]}` (ISS `crlg.preprod`), so it signs
/// series 2, 3 and 4 under its issuer; every published US root holds
/// `{isCraca, [256]}`, so the root signs series 256 itself ("root-signed", Table 1). No
/// published certificate signs series 2 under the root when the CRL signer is
/// ICA-issued (S2X), so the S2X ICA has no component CRL (`None` for that pairing,
/// [`ScmsRun::revoke_component`]).
#[must_use]
pub fn hash_crl_signer(
    pki: &crate::scms::pdu::ScmsPki,
    series: u16,
) -> Option<(&crate::scms::pdu::Authority, [u8; 8])> {
    use v2xw_sec::crl::CracaType;
    [&pki.crlg, &pki.root].into_iter().find_map(|a| {
        let ssp = v2xw_sec::crl::crl_ssp(&a.cert).filter(|s| s.version.0 == 1)?;
        if !ssp.crls.iter().any(|c| c.0.0 == series) {
            return None;
        }
        let craca = match ssp.associated_craca {
            CracaType::isCraca => a.id8(),
            CracaType::issuerIsCraca => match &a.cert.0.issuer {
                v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha256AndDigest(h)
                | v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha384AndDigest(h) => {
                    let id = v2xw_sec::hashedid::id8_bytes(h);
                    [&pki.root, &pki.ica]
                        .into_iter()
                        .find(|i| i.id8() == id)?
                        .id8()
                }
                _ => return None,
            },
        };
        Some((a, craca))
    })
}

/// Why a device refuses a certificate on a hash CRL it holds
/// ([`ScmsState::device_refuses`]); the receivers' `RxReject` of the same names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashRefusal {
    /// The certificate itself is listed.
    RevokedByHash,
    /// A CA certificate above it is listed (a revoked PCA, ICA, ...).
    IssuerRevoked,
}

/// Where a CRL stands in the sequence its CRL Generator issues for one
/// `(crlCraca, crlSeries)` (R2).
///
/// IEEE 1609.2 gives every CRL of a series a counter: `crlSerial` in a hash CRL
/// ("increments by 1 every time a new full or delta CRL is issued for the indicated
/// crlCraca and crlSeries values"), and `(iRev, indexWithinI)` in a linkage CRL
/// (`indexWithinI` "is set to 0 for the first CRL issued for the indicated combination of
/// crlCraca, crlSeries, and iRev, and increments by 1 every time a new full or delta CRL
/// is issued … without changing iRev"; `Ieee1609Dot2CrlBaseTypes`). A device that holds
/// a CRL refuses another of the same series whose counter is not higher: a replay of the
/// Generator's own earlier, validly signed list would otherwise roll a revocation back.
/// `issueDate` breaks a tie, so a Generator whose `indexWithinI` has reached its Uint8
/// ceiling within one i-period is still followed. What a real device does with a stale
/// CRL is not published; refusing it is the reading of the counters' purpose (recorded
/// in docs/design/scms-real/validation/protocols.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CrlOrder {
    /// `crlSerial`, or `iRev · 256 + indexWithinI`.
    pub counter: u32,
    /// `issueDate` (Time32).
    pub issue_date: u32,
}

impl CrlOrder {
    /// The `(crlCraca, crlSeries)` a CRL belongs to and where it stands in that sequence.
    #[must_use]
    pub fn of(
        c: &v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::CrlContents,
    ) -> (([u8; 8], u16), CrlOrder) {
        use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents as T;
        let mut craca = [0u8; 8];
        craca.copy_from_slice(&c.crl_craca.0[..]);
        let counter = match &c.type_specific {
            T::fullHashCrl(h) | T::deltaHashCrl(h) => h.crl_serial.0,
            T::fullLinkedCrl(l) | T::deltaLinkedCrl(l) => {
                u32::from(l.i_rev.0.0) * 256 + u32::from(l.index_within_i.0)
            }
            _ => 0,
        };
        (
            (craca, c.crl_series.0.0),
            CrlOrder {
                counter,
                issue_date: c.issue_date.0.0,
            },
        )
    }
}

/// The device's check of a received `SecuredCrl` against the CRL list it is about to act
/// on (see the `CrlDownload` handler). Returns the CRL's series and [`CrlOrder`], which
/// the caller compares with the CRL it holds.
///
/// A hash CRL (series 2, 3, 4 or 256; SC-P8.2) carries its own entries, so `list` is not
/// read for it; its signer's issuer is the authority the signer names (the CRACA of an
/// `issuerIsCraca` signer; none is needed for the root's `isCraca` series-256 list).
///
/// # Errors
/// The CRL does not decode, the signer is not one attached certificate that chains to
/// the PKI, [`v2xw_sec::crl::verify_crl`] does not accept it, `list` is not exactly
/// what a linkage CRL signs, or a hash CRL claims series 1.
pub fn device_check_crl(
    pki: &crate::scms::pdu::ScmsPki,
    bytes: &[u8],
    list: &[CrlLinkageEntry],
) -> core::result::Result<(([u8; 8], u16), CrlOrder), crate::scms::pdu::PduError> {
    use crate::scms::pdu::PduError;
    use v2xw_msg::sec_types::ieee1609_dot2_crl_base_types::TypeSpecificCrlContents as T;
    let opened = v2xw_sec::crl::decode_crl(bytes).map_err(|e| PduError::Coer(e.to_string()))?;
    let v2xw_sec::sig1609::SpduSigner::Certificate(signer) = opened.signed.signer.clone() else {
        return Err(PduError::Signer("a CRL carries its signer's certificate"));
    };
    pki.signer_key(&signer)?;
    let hash = matches!(
        opened.contents.type_specific,
        T::fullHashCrl(_) | T::deltaHashCrl(_)
    );
    let issuer = if hash {
        match &signer.0.issuer {
            v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha256AndDigest(h)
            | v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha384AndDigest(h) => pki
                .by_id8(&v2xw_sec::hashedid::id8_bytes(h))
                .map(|a| &a.cert),
            _ => None,
        }
    } else {
        Some(&pki.crl_signer_issuer().cert)
    };
    let verdict = v2xw_sec::crl::verify_crl(
        &opened,
        &signer,
        issuer,
        v2xw_msg::scms_types::WireEdition::default(),
    );
    if !verdict.accepted() {
        return Err(if verdict.signature_ok {
            PduError::Policy("CRL signer, series, CRACA or contents rule")
        } else {
            PduError::BadSignature
        });
    }
    let c = &opened.contents;
    if hash {
        if c.crl_series.0.0 == v2xw_sec::crl::SERIES_PSEUDONYM {
            return Err(PduError::Shape("series-1 CRL is a linkage CRL"));
        }
        return Ok(CrlOrder::of(c));
    }
    let (i_rev, index_within_i, delta) = match &c.type_specific {
        T::fullLinkedCrl(t) => (t.i_rev.0.0, t.index_within_i.0, false),
        T::deltaLinkedCrl(t) => (t.i_rev.0.0, t.index_within_i.0, true),
        _ => return Err(PduError::Shape("series-1 CRL is a linkage CRL")),
    };
    let spec = CrlSpec {
        series: c.crl_series.0.0,
        craca: {
            let mut b = [0u8; 8];
            b.copy_from_slice(&c.crl_craca.0[..]);
            b
        },
        issue_date: c.issue_date.0.0,
        next_crl: c.next_crl.0.0,
        priority: c.priority_info.priority.as_ref().map(|p| p.0),
        delta,
        body: CrlBody::Linkage {
            i_rev,
            index_within_i,
            individual: linkage_revocations(list, u32::from(i_rev)),
        },
    };
    if v2xw_sec::crl::crl_contents(&spec) != *c {
        return Err(PduError::Shape("the CRL list differs from the signed CRL"));
    }
    Ok(CrlOrder::of(c))
}

impl ScmsRun {
    /// Puts `n` revocations from before the run on this provider's CRL and publishes it
    /// at the run's start: the standing list of a deployment that has been revoking for
    /// years. Each entry is one revoked device's seed pair at the current i-period, under
    /// this provider's own two LAs (a US provider runs one LA1 and one LA2, so every entry
    /// shares one `LAGroup` of the series-1 CRL, as the run's own revocations do), with
    /// random seeds from the backend's keyed stream, so it matches no device of the run
    /// (they were revoked before it). The list is signed as a real `SecuredCrl` when the
    /// PDU layer is on. With `n` 0 it is the empty list every CRACA publishes before its
    /// first revocation (R-NOCRL): the engine seeds every provider's Generator so, and the
    /// list is renewed on its `nextCrl` like any other.
    pub fn seed_standing_crl(&mut self, n: u32) {
        let st = &mut self.state;
        let i = st.params.period_at(0);
        let jmax = st.params.certs_per_period;
        let (la1, la2) = (st.la[0].la_id, st.la[1].la_id);
        let mut draws = vec![0u8; 36 * n as usize];
        st.rng
            .checkout(
                RngDomain::Backend,
                EntityRef::custom("scms/standing-crl", 0),
            )
            .fill_bytes(&mut draws);
        for b in draws.chunks_exact(36) {
            let mut ls1 = [0u8; 16];
            let mut ls2 = [0u8; 16];
            ls1.copy_from_slice(&b[4..20]);
            ls2.copy_from_slice(&b[20..36]);
            st.crlg.entries.push(CrlLinkageEntry {
                i,
                la_id1: la1,
                la_id2: la2,
                ls1_i: LinkageSeed::new(ls1),
                ls2_i: LinkageSeed::new(ls2),
                jmax,
                max_forward: linkage::DEFAULT_MAX_FORWARD_PERIODS,
            });
        }
        st.crlg.signature = st.gov.sign_as("crlg", &crl_digest(&st.crlg.entries));
        st.crlg.versions += 1;
        let _ = st.issue_secured_crls(0);
        st.crlg.standing = n;
        for store in [&mut st.crl_store, &mut st.crl_broadcast] {
            store.entries = st.crlg.entries.clone();
            store.secured = st.crlg.secured.clone();
            store.signature = st.crlg.signature;
            store.versions += 1;
        }
        // Renewed by its nextCrl like any list the Generator signs.
        if st.params.crl.reissue && st.params.crl.next_crl > Duration::ZERO {
            let issued = st.crlg.versions;
            let at = st.params.crl.next_crl.after(0);
            let crlg = st.nodes.crlg;
            let run = self.new_run();
            self.inject_at(
                at,
                crlg,
                ScmsMsg::CrlReissueTimer { issued },
                FlowId::CrlIssuance,
                run,
            );
        }
    }

    /// The provider's PDU PKI has followed a root rollover's changeover (Provider Req.
    /// §3.5.4): its CRL Generator, re-certified under the new root, signs its list again
    /// now, under the new CRACA, with the superseded CRACA's list beside it
    /// (`ScmsState::issue_secured_crls`), through the Generator's own reissue flow (so a
    /// Generator that is down signs when it is back).
    ///
    /// Every certificate issued after the changeover names the new CRACA, and a device
    /// matches a CRL to a certificate by its `cracaId` (IEEE 1609.2 §5.1.3.2), so until
    /// that CRACA has a list no device can check one of them for revocation or apply the
    /// overdue rule to it ([CAMP122] Table 67): as R-NOCRL, every CRACA's CRL exists.
    /// Before, the first list under the new root waited for the next `nextCrl` (a week
    /// later), and with the Generator down from 700 s (`multi-week-crlg-down`) never came:
    /// the run refused nothing as `crl-overdue` (RR.MWC.overdue, scms-fix4-a.yaml).
    pub fn reissue_crl_after_changeover(&mut self, at: SimTime) {
        if self.state.pki.is_none() {
            return;
        }
        let at = at.max(self.kernel.now());
        let issued = self.state.crlg.versions;
        let crlg = self.state.nodes.crlg;
        let run = self.new_run();
        self.inject_at(
            at,
            crlg,
            ScmsMsg::CrlReissueTimer { issued },
            FlowId::CrlIssuance,
            run,
        );
    }

    /// The provider revokes one of its own SCMS components at `at` (SC-P8.1): the
    /// component's certificate goes on the hash CRL of the series and CRACA it names
    /// ([`ComponentRevocation`]), signed by the certificate whose `CrlSsp` lists that
    /// series ([`hash_crl_signer`]), at the first publication at or after `at`.
    ///
    /// The decision reaches the CRL Generator as a change to the list, so it is
    /// published by the provider's publication rule as a linkage revocation is: the next
    /// business-day slot ([SM-PROV] §3.4.9, "within 1 business day of any change"), the
    /// cadence boundary, or at once. ASSUMPTION (labelled): the root-signed series-256
    /// list rides the provider's publication; a real root is offline and signs it at a
    /// ceremony whose delay is not published.
    ///
    /// # Errors
    /// The PDU layer is off, `role` names no component, or no certificate of the PKI
    /// can sign a CRL that matches the component's `(cracaId, crlSeries)`: the S2X ICA
    /// names the root on series 2, and its CRL signer's CRACA is the ICA (recorded in
    /// `pki_profile.rs`, CF-CRACA).
    pub fn revoke_component(
        &mut self,
        role: &str,
        at: SimTime,
    ) -> crate::scms::pdu::Result<ComponentRevocation> {
        use crate::scms::pdu::PduError;
        let pki = self
            .state
            .pki
            .clone()
            .ok_or(PduError::Policy("component CRLs need the PDU layer"))?;
        let (role, a) =
            component_authority(&pki, role).ok_or(PduError::Policy("no such SCMS component"))?;
        let tbs = &a.cert.0.to_be_signed;
        let series = tbs.crl_series.0.0;
        let craca3 = &tbs.craca_id.0[..];
        let (_, craca) = hash_crl_signer(&pki, series)
            .filter(|(_, c)| c[5..] == *craca3)
            .ok_or(PduError::Policy(
                "no CRL signer holds the component's cracaId and crlSeries",
            ))?;
        let entry =
            v2xw_sec::crl::hash_entry(&a.cert).map_err(|e| PduError::Coer(e.to_string()))?;
        let rev = ComponentRevocation {
            role,
            at,
            series,
            craca,
            entry,
        };
        self.state.crlg.component_pending.push(rev);
        let at = at.max(self.kernel.now());
        let crlg = self.state.nodes.crlg;
        let run = self.new_run();
        // A change to the list with no linkage entry: the Generator arms its publication
        // exactly as for a revocation's `crl-append`.
        self.inject_at(
            at,
            crlg,
            ScmsMsg::CrlAppend {
                i: 0,
                pairs: Vec::new(),
                jmax: 0,
            },
            FlowId::CrlIssuance,
            run,
        );
        Ok(rev)
    }

    /// The MA revokes `device`'s OBU identification certificates at `at` (SC-P9.1;
    /// [SMP] §6.3.3, Table 1): a non-pseudonym certificate carries no linkage data, so it
    /// is revoked by its `HashedId10` on the series-3 hash CRL of the CRACA it names (the
    /// ICA; the CRL signer `{issuerIsCraca, [4,3,1,2]}` signs it). Every identification
    /// certificate the ACA has issued the device and that has not expired goes on the
    /// list at each publication at or after `at`, so one the ACA issues while the
    /// decision is on its way joins the next one; and from `at` the RA issues the device
    /// no more.
    ///
    /// ASSUMPTION (labelled): the decision withdraws the privilege only. The device keeps
    /// its enrolment certificate and its pseudonyms; revoking the whole device is the
    /// linkage path ([`ScmsRun::revoke`] and the RA blocklist). No published source says
    /// whether a misbehaving emergency vehicle loses both.
    ///
    /// # Errors
    /// The PDU layer is off, or no certificate of the PKI signs a series-3 list under the
    /// CRACA the device's identification certificates name.
    pub fn revoke_identification(
        &mut self,
        device: NodeId,
        at: SimTime,
    ) -> crate::scms::pdu::Result<()> {
        use crate::scms::pdu::PduError;
        let pki = self
            .state
            .pki
            .clone()
            .ok_or(PduError::Policy("identification CRLs need the PDU layer"))?;
        let (_, craca) = hash_crl_signer(&pki, v2xw_sec::crl::SERIES_APPLICATION_IDENTIFICATION)
            .ok_or(PduError::Policy("no CRL signer lists series 3"))?;
        let named = self
            .state
            .apps
            .devices
            .get(&device)
            .into_iter()
            .flat_map(|d| d.installed.iter())
            .all(|c| {
                let t = &c.cred.cert.0.to_be_signed;
                t.crl_series.0.0 == v2xw_sec::crl::SERIES_APPLICATION_IDENTIFICATION
                    && t.craca_id.0[..] == craca[5..]
            });
        if !named {
            return Err(PduError::Policy(
                "the identification certificate names another cracaId or crlSeries",
            ));
        }
        self.state.apps.id_revoked.entry(device).or_insert(at);
        let at = at.max(self.kernel.now());
        let crlg = self.state.nodes.crlg;
        let run = self.new_run();
        self.inject_at(
            at,
            crlg,
            ScmsMsg::CrlAppend {
                i: 0,
                pairs: Vec::new(),
                jmax: 0,
            },
            FlowId::CrlIssuance,
            run,
        );
        Ok(())
    }
}

impl ScmsState {
    /// Signs the CRL the Generator holds and sends it to the store and the broadcast path.
    /// With the PDU layer on, the artefact is a real `SecuredCrl` and every hop is charged
    /// its encoded length; the digest signature of the modelled path is kept alongside.
    pub(super) fn publish_crl(
        &mut self,
        at: SimTime,
        out: &mut Outbox<ScmsMsg>,
        flow: FlowId,
        run: FlowRun,
    ) {
        let n = self.nodes;
        self.crlg.signature = self.gov.sign_as("crlg", &crl_digest(&self.crlg.entries));
        self.crlg.versions += 1;
        let entries = u32::try_from(self.crlg.entries.len()).unwrap_or(u32::MAX);
        let secured = self.issue_secured_crls(at);
        // The list must be renewed by its `nextCrl` even when nothing new is revoked, or
        // every device stops trusting the CRACA's certificates two weeks later
        // (`overdue_CRL_tolerance`). Armed at every publication; a timer that finds a
        // newer publication does nothing.
        if self.params.crl.reissue && self.params.crl.next_crl > Duration::ZERO {
            out.start_timer(
                self.params.crl.next_crl,
                ScmsMsg::CrlReissueTimer {
                    issued: self.crlg.versions,
                },
                flow,
                run,
            );
        }
        // The store first, then the broadcast path, so that two links with the same
        // parameters stamp `published` before `first_rsu_broadcast`.
        for (to, step) in [
            (n.crl_store, "crl-publish"),
            (n.crl_broadcast, "crl-broadcast"),
        ] {
            let size = match &secured {
                Some(p) => {
                    let full = self.emit_pdu(SpduKind::SecuredCrl, n.crlg, to, at, p);
                    // The hash CRLs of the publication travel with it, each its own
                    // `SecuredCrl` (SC-P8.2); none unless a component was revoked.
                    let hash: Vec<_> = self.crlg.hash_crls.values().cloned().collect();
                    let mut bytes = full.bytes();
                    for h in &hash {
                        bytes = bytes.saturating_add(
                            self.emit_pdu(SpduKind::SecuredCrl, n.crlg, to, at, h)
                                .bytes(),
                        );
                    }
                    if hash.is_empty() {
                        full
                    } else {
                        WireSize::real(bytes, SpduKind::SecuredCrl.name())
                    }
                }
                None => self.sizes.crl(entries),
            };
            out.send(
                to,
                ScmsMsg::CrlPublish { entries },
                step,
                size,
                Transport::BackendNet,
                flow,
                run,
            );
        }
    }

    /// The real CRLs of one publication (02 §11; 03 §10): a full series-1 linkage CRL
    /// with `iRev` the current i-period (or the latest entry's), `indexWithinI` counting
    /// the CRLs at that `iRev`, `crlCraca` the ICA, signed by the CRL signer; a delta
    /// with the entries added since the previous publication when
    /// [`crate::scms::params::CrlIssuance::deltas`] is set; and a full hash CRL for each
    /// of series 3 and 4 that has entries. Returns the full linkage CRL, which is what
    /// the store and the RSUs distribute; `None` with the layer off.
    fn issue_secured_crls(&mut self, at: SimTime) -> Option<crate::scms::pdu::Pdu> {
        let pki = self.pki.clone()?;
        let i_now = self.params.period_at(at);
        let i_rev = self
            .crlg
            .entries
            .iter()
            .map(|e| e.i)
            .max()
            .unwrap_or(0)
            .max(i_now);
        let i_rev16 = u16::try_from(i_rev).unwrap_or(u16::MAX);
        let issue_date = self.time32(at);
        let next_crl = issue_date.saturating_add(
            u32::try_from(self.params.crl.next_crl.as_nanos() / v2xw_core::time::NS_PER_S)
                .unwrap_or(u32::MAX),
        );
        // `indexWithinI` counts every full *and* delta CRL issued at this `iRev`
        // (`Ieee1609Dot2CrlBaseTypes`): the full takes the next value and a delta issued
        // with it the one after. It is a Uint8; past 255 it stays at 255 and `issueDate`
        // orders the lists ([`CrlOrder`]).
        let next_index = |crls_at_i: &mut BTreeMap<u16, u8>| {
            let n = crls_at_i.entry(i_rev16).or_insert(0);
            let k = *n;
            *n = n.saturating_add(1);
            k
        };
        let index_within_i = next_index(&mut self.crlg.crls_at_i);
        let craca = pki.craca_id8();
        let sign = |spec: &CrlSpec| -> Option<crate::scms::pdu::Pdu> {
            let contents = v2xw_sec::crl::crl_contents(spec);
            v2xw_sec::crl::check_crl(&contents, v2xw_msg::scms_types::WireEdition::default())
                .ok()?;
            let crl = v2xw_sec::crl::sign_crl(&contents, &pki.crlg.cert, &pki.crlg.key).ok()?;
            Some(crate::scms::pdu::Pdu::new(
                v2xw_sec::crl::encode_crl(&crl).ok()?,
            ))
        };
        let mut log = Vec::new();
        let individual = linkage_revocations(&self.crlg.entries, i_rev);
        let full_spec = CrlSpec {
            series: v2xw_sec::crl::SERIES_PSEUDONYM,
            craca,
            issue_date,
            next_crl,
            priority: None,
            delta: false,
            body: CrlBody::Linkage {
                i_rev: i_rev16,
                index_within_i,
                individual: individual.clone(),
            },
        };
        let full = sign(&full_spec)?;
        log.push((1, false, individual.len(), full.clone()));
        // Every CRL signer a root rollover superseded signs the same list under its own
        // CRACA (its issuer, by `issuerIsCraca`): the old hierarchy's pseudonyms name
        // that CRACA until they expire, and a device matches an entry only to a CRL of
        // the certificate's own CRACA (audit-gov §4 item 2).
        let issuer_of = |c: &v2xw_msg::sec_types::Certificate| match &c.0.issuer {
            v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha256AndDigest(h)
            | v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha384AndDigest(h) => {
                Some(v2xw_sec::hashedid::id8_bytes(h))
            }
            _ => None,
        };
        let mut retired = Vec::new();
        for signer in pki
            .retired
            .iter()
            .filter(|a| crate::scms::governance::role_of(&a.cert) == Some("crlg"))
        {
            let Some(craca) = issuer_of(&signer.cert)
                .and_then(|id| pki.retired.iter().map(|a| a.id8()).find(|a| *a == id))
            else {
                continue;
            };
            let spec = CrlSpec {
                craca,
                ..full_spec.clone()
            };
            let contents = v2xw_sec::crl::crl_contents(&spec);
            let Some(p) = v2xw_sec::crl::sign_crl(&contents, &signer.cert, &signer.key)
                .ok()
                .and_then(|c| v2xw_sec::crl::encode_crl(&c).ok())
                .map(crate::scms::pdu::Pdu::new)
            else {
                continue;
            };
            log.push((1, false, individual.len(), p.clone()));
            retired.push(p);
        }
        self.crlg.secured_retired = retired;
        let since = self.crlg.published_entries.min(self.crlg.entries.len());
        if self.params.crl.deltas && self.crlg.secured.is_some() {
            let added = linkage_revocations(&self.crlg.entries[since..], i_rev);
            let delta_index = next_index(&mut self.crlg.crls_at_i);
            let spec = CrlSpec {
                delta: true,
                body: CrlBody::Linkage {
                    i_rev: i_rev16,
                    index_within_i: delta_index,
                    individual: added.clone(),
                },
                ..full_spec.clone()
            };
            if let Some(d) = sign(&spec) {
                log.push((1, true, added.len(), d.clone()));
                self.crlg.delta = Some(d);
            }
        }
        // Component revocations decided by now join their series' list (SC-P8.1).
        let (due, later): (Vec<_>, Vec<_>) = std::mem::take(&mut self.crlg.component_pending)
            .into_iter()
            .partition(|r| r.at <= at);
        self.crlg.component_pending = later;
        for r in due {
            let list = self.crlg.hash_entries.entry(r.series).or_default();
            if !list.iter().any(|e| e.id == r.entry.id) {
                list.push(r.entry);
                self.crlg.component_revoked.push(r);
            }
        }
        // Revoked identification certificates join the series-3 list (SC-P9.1): every one
        // the ACA issued a revoked device by now that has not expired.
        let now32 = self.time32(at);
        let ids: Vec<_> = self
            .apps
            .issued
            .iter()
            .filter(|c| {
                c.kind == crate::scms::appcert::AppCertKind::ObuIdentification
                    && c.at <= at
                    && c.expiry >= now32
                    && self.apps.id_revoked.get(&c.device).is_some_and(|&t| t <= at)
            })
            .map(|c| v2xw_sec::crl::HashRevocation {
                id: c.id10,
                expiry: c.expiry,
            })
            .collect();
        if !ids.is_empty() {
            let list = self
                .crlg
                .hash_entries
                .entry(v2xw_sec::crl::SERIES_APPLICATION_IDENTIFICATION)
                .or_default();
            for e in ids {
                if !list.iter().any(|x| x.id == e.id) {
                    list.push(e);
                }
            }
        }
        // A full hash CRL for every series with entries (2, 3, 4, 256; [SMP] Table 1),
        // each signed by the certificate whose `CrlSsp` lists it, under the CRACA that
        // `CrlSsp` names ([`hash_crl_signer`]). No entries, no list.
        let hash_series: Vec<u16> = self.crlg.hash_entries.keys().copied().collect();
        for series in hash_series {
            let Some(entries) = self.crlg.hash_entries.get(&series).cloned() else {
                continue;
            };
            let Some((signer, craca)) = hash_crl_signer(&pki, series) else {
                continue;
            };
            let serial = {
                let s = self.crlg.hash_serial.entry(series).or_insert(0);
                *s += 1;
                *s
            };
            let spec = CrlSpec {
                series,
                craca,
                issue_date,
                next_crl,
                priority: None,
                delta: false,
                body: CrlBody::Hash {
                    serial,
                    entries: entries.clone(),
                },
            };
            let signed = (|| {
                let contents = v2xw_sec::crl::crl_contents(&spec);
                v2xw_sec::crl::check_crl(&contents, v2xw_msg::scms_types::WireEdition::default())
                    .ok()?;
                let crl = v2xw_sec::crl::sign_crl(&contents, &signer.cert, &signer.key).ok()?;
                Some(crate::scms::pdu::Pdu::new(
                    v2xw_sec::crl::encode_crl(&crl).ok()?,
                ))
            })();
            if let Some(p) = signed {
                log.push((series, false, entries.len(), p.clone()));
                self.crlg.hash_crls.insert(series, p);
            }
        }
        for (series, delta, n, pdu) in log {
            self.crlg.issued.push(IssuedCrl {
                at,
                series,
                delta,
                issue_date,
                next_crl,
                entries: u32::try_from(n).unwrap_or(u32::MAX),
                pdu,
            });
        }
        self.crlg.published_entries = self.crlg.entries.len();
        self.crlg.secured = Some(full.clone());
        Some(full)
    }
}

impl ScmsRun {
    /// Logs the signed CRL `crl` that `device` fetched from this provider's CRL Store
    /// **before the run** (R-NOCRL, OF-3): a device on the road holds every CRACA's
    /// current list. Marked pre-run like the device's enrolment and first batches
    /// ([`ScmsRun::preload_at`]): no step carried it, and `at` is the instant the device
    /// joins holding it, so the PKI view shows the fetch as a `preload` PDU.
    pub fn note_prerun_crl(&mut self, device: NodeId, at: SimTime, crl: &crate::scms::pdu::Pdu) {
        let keep = self.state.params.pdu.keep_bytes;
        let from = self.state.nodes.crl_store.index();
        self.state.pdus.push(crate::scms::pdu::EmittedPdu {
            kind: SpduKind::SecuredCrl,
            from,
            to: device.index(),
            at,
            bytes: keep.then(|| crl.clone()),
            len: crl.len(),
            step: None,
            preload: true,
        });
    }

    /// Has a device fetch, expand and enforce the current CRL.
    pub fn distribute_crl(&mut self, device: NodeId) -> FlowRun {
        let run = self.new_run();
        self.inject(
            device,
            ScmsMsg::CrlDownloadRequest { device },
            FlowId::CrlDistribution,
            run,
        );
        run
    }

    /// Has the roadside broadcast path push the current CRL to `device`.
    ///
    /// The device must have been attached with [`ScmsRun::attach_rsu`]; without a link the
    /// kernel refuses to deliver rather than delivering for free (invariant I-P1).
    pub fn broadcast_crl_to(&mut self, device: NodeId) -> FlowRun {
        let run = self.new_run();
        let broadcast = self.state.nodes.crl_broadcast;
        self.inject(
            broadcast,
            ScmsMsg::CrlAirBroadcast { device },
            FlowId::CrlDistribution,
            run,
        );
        run
    }

    /// RA side of 1609.2.1 §6.3.5.14 (WP-35, A16): the RA's signed
    /// `CertificateManagementInformationStatusSpdu` for `status`, logged as a PDU from the
    /// RA to `device` at `at`, as the RA answers a device's information-status poll.
    /// `None` when the PDU layer is off (no RA key to sign with).
    pub fn ra_info_status(
        &mut self,
        device: NodeId,
        at: SimTime,
        status: &crate::scms::pdu::InfoStatus,
    ) -> Option<crate::scms::pdu::Pdu> {
        let pki = self.state.pki.clone()?;
        let p =
            crate::scms::pdu::Pdu::new(crate::scms::pdu::ra_info_status_spdu(&pki, status).ok()?);
        let keep = self.state.params.pdu.keep_bytes;
        self.state.pdus.push(crate::scms::pdu::EmittedPdu {
            kind: SpduKind::InfoStatus,
            from: self.state.nodes.ra.index(),
            to: device.index(),
            at,
            bytes: keep.then(|| p.clone()),
            len: p.len(),
            step: None,
            preload: false,
        });
        Some(p)
    }

    /// The RA's own lines of an information status (WP-35): `(ctl, caCcf, ma, ra)`. The
    /// CTL line is the home CTL (the US series, its sequence, its effective date); `caCcf`
    /// is the same date, since the chain file carries the CTL; the MA line is BSM
    /// misbehaviour (PSID 0x20, [SM-MBR]) dated by the MA certificate's validity start, and
    /// `ra` the RA certificate's. The CRL lines are the caller's: the lists of the
    /// composite CRL the RA serves, every provider's. `None` with the PDU layer off.
    pub fn ra_status_lines(
        &self,
    ) -> Option<(
        crate::scms::pdu::CtlStatusLine,
        u32,
        crate::scms::pdu::MaStatusLine,
        u32,
    )> {
        let pki = self.state.pki.as_ref()?;
        let ctl = &self.state.gov.ctl;
        let start = |c: &v2xw_msg::sec_types::ieee1609_dot2::Certificate| {
            u32::try_from(v2xw_sec::ctl::validity(c).0).unwrap_or(u32::MAX)
        };
        Some((
            crate::scms::pdu::CtlStatusLine {
                series_id: v2xw_sec::ctl::US_CTL_SERIES_ID,
                sequence: u16::try_from(ctl.sequence).unwrap_or(u16::MAX),
                last_update: ctl.effective,
            },
            ctl.effective,
            crate::scms::pdu::MaStatusLine {
                psids: vec![0x20],
                updated: start(&pki.ma.cert),
            },
            start(&pki.ra.cert),
        ))
    }

    /// The single entry on the published CRL, for a test that needs to inspect it.
    pub fn crl_entry(&self) -> Option<&v2xw_sec::linkage::CrlLinkageEntry> {
        self.state.crl_store.entries.first()
    }

    /// `crl_component_entries`: the component certificates on this provider's signed
    /// component CRLs, series 2 and 256 (SC-P8.2). 0 unless a component was revoked.
    #[must_use]
    pub fn crl_component_entries(&self) -> u32 {
        let n: usize = [
            v2xw_sec::crl::SERIES_SCMS_COMPONENTS,
            v2xw_sec::crl::SERIES_ROOT_SIGNED,
        ]
        .iter()
        .filter(|s| self.state.crlg.hash_crls.contains_key(s))
        .filter_map(|s| self.state.crlg.hash_entries.get(s))
        .map(Vec::len)
        .sum();
        u32::try_from(n).unwrap_or(u32::MAX)
    }
}

impl ScmsState {
    /// Handles every message of this flow; [`ScmsState::handle`] routes here.
    #[allow(unused_variables)]
    pub(super) fn handle_crl(
        &mut self,
        d: Delivery<ScmsMsg>,
        out: &mut Outbox<ScmsMsg>,
    ) -> Result<()> {
        let n = self.nodes;
        let (flow, run, to, at, from) = (d.flow, d.run, d.to, d.at, d.from);
        match d.msg {
            ScmsMsg::CrlAppend { i, pairs, jmax } => {
                // One entry per chain pair of the device (B4), in one issuance.
                for (la1, la2) in pairs {
                    self.crlg.entries.push(CrlLinkageEntry {
                        i,
                        la_id1: la1.0,
                        la_id2: la2.0,
                        ls1_i: la1.1,
                        ls2_i: la2.1,
                        jmax,
                        max_forward: linkage::DEFAULT_MAX_FORWARD_PERIODS,
                    });
                }
                if let Some(case) = self.ma.case_by_crl_run(run) {
                    case.done = true;
                }
                let crl_committee = self.committees.role("crl");
                if crl_committee.is_none() {
                    Self::sign(out, 1);
                }
                let entries = u32::try_from(self.crlg.entries.len()).unwrap_or(u32::MAX);
                out.stage_at(
                    StageId::Issued,
                    to,
                    Some(self.sizes.crl(entries).bytes()),
                    flow,
                    run,
                );
                if self.params.crl.business_day_sla {
                    // The provider's operator publishes at the next business-day slot
                    // (Eastern time, US federal holidays observed): a change decided on a
                    // Friday evening or a holiday goes out on the next business day, and
                    // every entry appended before then rides along ([SM-PROV] §3.4.9).
                    if !self.crlg.publish_armed {
                        self.crlg.publish_armed = true;
                        let unix = self.wall.unix_seconds_at(at);
                        let (hh, mm) = self.params.crl.slot_et;
                        let slot = crate::scms::calendar::next_publication_slot(unix, hh, mm);
                        let wait = u64::try_from(slot - unix).unwrap_or(0);
                        let sub = at % v2xw_core::time::NS_PER_S;
                        out.start_timer(
                            Duration::from_nanos(
                                (wait * v2xw_core::time::NS_PER_S).saturating_sub(sub),
                            ),
                            ScmsMsg::CrlPublishTimer,
                            flow,
                            run,
                        );
                    }
                    return Ok(());
                }
                let cadence = self.params.crl_cadence;
                if self.params.publish_on_cadence && cadence > Duration::ZERO {
                    // The list is re-issued on the cadence, not per entry: the entry waits
                    // for the next boundary of the calendar that starts at the epoch
                    // [BRECHT §VI-G], and every entry appended before it rides along.
                    if !self.crlg.publish_armed {
                        self.crlg.publish_armed = true;
                        let c = cadence.as_nanos().max(1);
                        let since = at.saturating_sub(self.params.epoch);
                        let next = (since / c + 1) * c;
                        out.start_timer(
                            Duration::from_nanos(next - since),
                            ScmsMsg::CrlPublishTimer,
                            flow,
                            run,
                        );
                    }
                    return Ok(());
                }
                if let Some(role) = crl_committee {
                    // A t-of-n CRL Generator: the list is published once the committee
                    // has signed it.
                    self.committees.start(
                        role,
                        1,
                        Pending::PublishCrl { flow, run },
                        at,
                        flow,
                        run,
                        out,
                        wrap_committee,
                    );
                    return Ok(());
                }
                self.publish_crl(at, out, flow, run);
            }
            ScmsMsg::CrlReissueTimer { issued } => {
                // `nextCrl` has come with no publication since: the same entries, a new
                // issueDate and nextCrl, signed again.
                if self.crlg.versions == issued {
                    self.crlg.reissued += 1;
                    Self::sign(out, 1);
                    self.publish_crl(at, out, flow, run);
                }
            }
            ScmsMsg::CrlPublishTimer => {
                self.crlg.publish_armed = false;
                if let Some(role) = self.committees.role("crl") {
                    self.committees.start(
                        role,
                        1,
                        Pending::PublishCrl { flow, run },
                        at,
                        flow,
                        run,
                        out,
                        wrap_committee,
                    );
                    return Ok(());
                }
                Self::sign(out, 1);
                self.publish_crl(at, out, flow, run);
            }
            ScmsMsg::CrlPublish { entries } => {
                let list = self.crlg.entries.clone();
                let secured = self.crlg.secured.clone();
                let retired = self.crlg.secured_retired.clone();
                let hash = self.crlg.hash_crls.clone();
                let size = secured.as_ref().map_or_else(
                    || self.sizes.crl(entries).bytes(),
                    |p| with_hash_lists(p.len(), &hash),
                );
                let signature = self.crlg.signature;
                if to == n.crl_store {
                    self.crl_store.entries = list;
                    self.crl_store.secured = secured;
                    self.crl_store.secured_retired = retired;
                    self.crl_store.hash_crls = hash;
                    self.crl_store.signature = signature;
                    self.crl_store.versions += 1;
                    self.crl_store.published_runs.push(run);
                    out.stage_at(StageId::Published, to, Some(size), flow, run);
                } else {
                    self.crl_broadcast.entries = list;
                    self.crl_broadcast.secured = secured;
                    self.crl_broadcast.secured_retired = retired;
                    self.crl_broadcast.hash_crls = hash;
                    self.crl_broadcast.signature = signature;
                    self.crl_broadcast.versions += 1;
                    out.stage_at(StageId::FirstRsuBroadcast, to, Some(size), flow, run);
                }
            }
            ScmsMsg::CrlDownloadRequest { device } if to == device => {
                out.send(
                    n.crl_store,
                    ScmsMsg::CrlDownloadRequest { device },
                    "crl-request",
                    self.sizes.crl_request(),
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlDownloadRequest { device } => {
                let entries = u32::try_from(self.crl_store.entries.len()).unwrap_or(u32::MAX);
                let size = self.crl_store.secured.as_ref().map_or_else(
                    || self.sizes.crl(entries),
                    |p| {
                        WireSize::real(
                            with_hash_lists(p.len(), &self.crl_store.hash_crls),
                            SpduKind::SecuredCrl.name(),
                        )
                    },
                );
                out.send(
                    device,
                    ScmsMsg::CrlDownload { device, entries },
                    "crl-download",
                    size,
                    Transport::CellularUu,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlAirBroadcast { device } => {
                // The roadside unit puts the list it holds on the air. It performs no
                // cryptography: the CRL Generator already signed it, and re-signing at
                // every RSU would be both wrong and a cost the deployment does not pay.
                let entries = u32::try_from(self.crl_broadcast.entries.len()).unwrap_or(u32::MAX);
                let size = self.crl_broadcast.secured.as_ref().map_or_else(
                    || self.sizes.crl(entries),
                    |p| {
                        WireSize::real(
                            with_hash_lists(p.len(), &self.crl_broadcast.hash_crls),
                            SpduKind::SecuredCrl.name(),
                        )
                    },
                );
                out.send(
                    device,
                    ScmsMsg::CrlDownload { device, entries },
                    "crl-air-broadcast",
                    size,
                    Transport::V2xAir,
                    flow,
                    run,
                );
            }
            ScmsMsg::CrlDownload { device, entries } => {
                let (secured, hash) = if from == n.crl_broadcast {
                    (
                        self.crl_broadcast.secured.clone(),
                        self.crl_broadcast.hash_crls.clone(),
                    )
                } else {
                    (
                        self.crl_store.secured.clone(),
                        self.crl_store.hash_crls.clone(),
                    )
                };
                let size = secured.as_ref().map_or_else(
                    || self.sizes.crl(entries).bytes(),
                    |p| with_hash_lists(p.len(), &hash),
                );
                out.stage_at(StageId::Downloaded, device, Some(size), flow, run);
                // One signature verification over the list, then per entry: two SHA-256
                // per i-period walked and two AES per index searched ([ACPC §2],
                // [BRECHT §VII]).
                Self::verify(out, 1);
                // The hash CRLs that came with it (series 2, 3, 4, 256; SC-P8.2), each
                // verified on its own (1609.2.1 §6.3.5: every `SecuredCrl` of a composite
                // file is checked by itself): one that fails is refused and recorded, the
                // others install whatever the series-1 list does.
                if let Some(pki) = self.pki.clone() {
                    for p in hash.values() {
                        Self::verify(out, 1);
                        self.install_hash_crl(&pki, device, at, p);
                    }
                }
                // The device processes the list it *received*, from whichever path
                // delivered it. The two paths carry the same signed artefact, and reading
                // the store's copy on the broadcast path would make an RSU-only vehicle
                // silently enforce a CRL it never heard.
                let (list, signature) = if from == n.crl_broadcast {
                    (
                        self.crl_broadcast.entries.clone(),
                        self.crl_broadcast.signature,
                    )
                } else {
                    (self.crl_store.entries.clone(), self.crl_store.signature)
                };
                // The real path (WP-B4): the device verifies the `SecuredCrl` itself —
                // psid 256 and no other header field, the CRL signer's certificate
                // chains to its ICA and holds a `CrlSsp` for series 1 whose
                // `issuerIsCraca` names that ICA as `crlCraca`, the signature, the
                // contents rules — and acts only on the entries those signed bytes
                // carry: a list that differs from them is refused whole.
                // R2: a genuine list is still refused if it is not newer than the one
                // the device holds for that CRACA and series ([`CrlOrder`]); the same
                // list heard again is installed again, which changes nothing.
                let mut held = None;
                if let (Some(p), Some(pki)) = (secured.as_ref(), self.pki.clone()) {
                    match device_check_crl(&pki, p.bytes(), &list) {
                        Err(e) => {
                            if let Some(dev) = self.devices.get_mut(&device) {
                                dev.crls_rejected += 1;
                            }
                            self.reject_pdu(SpduKind::SecuredCrl, device, at, e);
                            return Ok(());
                        }
                        Ok((key, order)) => {
                            let digest = v2xw_core::hash::sha256(p.bytes());
                            let stale = self
                                .devices
                                .get(&device)
                                .and_then(|d| d.crl_held.get(&key))
                                .is_some_and(|&(o, h)| h != digest && order <= o);
                            if stale {
                                if let Some(dev) = self.devices.get_mut(&device) {
                                    dev.crls_stale += 1;
                                }
                                self.reject_pdu(
                                    SpduKind::SecuredCrl,
                                    device,
                                    at,
                                    crate::scms::pdu::PduError::Policy(
                                        "stale CRL: not newer than the one held for its \
                                         CRACA and series",
                                    ),
                                );
                                return Ok(());
                            }
                            held = Some((key, (order, digest)));
                        }
                    }
                }
                let Some(dev) = self.devices.get_mut(&device) else {
                    return Err(ProtoError::NoEntity { node: device });
                };
                // The device checks the CRL Generator's signature against the chain it
                // was given at bootstrap. An empty list has nothing to trust; a list that
                // does not verify is discarded whole.
                if !list.is_empty() {
                    let digest = crl_digest(&list);
                    let ok = signature.is_some_and(|(signer, sig)| {
                        dev.trust.verify_signed(signer, &digest, &sig)
                    });
                    if !ok {
                        dev.crls_rejected += 1;
                        return Ok(());
                    }
                }
                let periods: u32 = dev
                    .credentials
                    .keys()
                    .map(|&(i, _)| i)
                    .max()
                    .unwrap_or(0)
                    .saturating_add(1);
                let mut hashes = 0u32;
                let mut aes = 0u32;
                for e in &list {
                    let walked = periods.saturating_sub(e.i).min(e.max_forward);
                    hashes = hashes.saturating_add(2u32.saturating_mul(walked));
                    aes = aes
                        .saturating_add(2u32.saturating_mul(e.jmax).saturating_mul(walked.max(1)));
                }
                dev.crl = list;
                if let Some((key, v)) = held {
                    dev.crl_held.insert(key, v);
                }
                let silenced = !dev.revoked_credentials().is_empty();
                dev.silenced = silenced;
                Self::hashes(out, hashes);
                Self::aes(out, aes);
                out.stage_at(StageId::Processed, device, None, flow, run);
                out.stage_at(StageId::Enforced, device, None, flow, run);
                let _ = at;
            }
            _ => unreachable!("ScmsState::handle routes only this flow's messages here"),
        }
        Ok(())
    }

    /// `device` checks one hash `SecuredCrl` it received ([`device_check_crl`]) and, when
    /// it verifies and is not older than the list it holds for that `(crlCraca,
    /// crlSeries)`, installs its entries (SC-P8.2). A list that does not verify, or a
    /// replay of an older one, is refused and recorded.
    fn install_hash_crl(
        &mut self,
        pki: &crate::scms::pdu::ScmsPki,
        device: NodeId,
        at: SimTime,
        p: &crate::scms::pdu::Pdu,
    ) {
        let checked = device_check_crl(pki, p.bytes(), &[]).and_then(|(key, order)| {
            let c = v2xw_sec::crl::decode_crl(p.bytes())
                .map_err(|e| crate::scms::pdu::PduError::Coer(e.to_string()))?;
            let ids: Vec<[u8; 10]> = v2xw_sec::crl::hash_entries(&c.contents)
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            Ok((key, order, ids))
        });
        let (key, order, ids) = match checked {
            Ok(v) => v,
            Err(e) => {
                if let Some(dev) = self.devices.get_mut(&device) {
                    dev.crls_rejected += 1;
                }
                self.reject_pdu(SpduKind::SecuredCrl, device, at, e);
                return;
            }
        };
        let digest = v2xw_core::hash::sha256(p.bytes());
        let Some(dev) = self.devices.get_mut(&device) else {
            return;
        };
        if dev
            .crl_held
            .get(&key)
            .is_some_and(|&(o, h)| h != digest && order <= o)
        {
            dev.crls_stale += 1;
            self.reject_pdu(
                SpduKind::SecuredCrl,
                device,
                at,
                crate::scms::pdu::PduError::Policy(
                    "stale CRL: not newer than the one held for its CRACA and series",
                ),
            );
            return;
        }
        dev.crl_held.insert(key, (order, digest));
        dev.hash_held.insert(key, ids);
    }

    /// Whether `device` refuses `cert` on a hash CRL it holds (SC-P8.2; IEEE 1609.2
    /// §5.1.3), and why: `cert` itself is listed, or a CA certificate above it in this
    /// provider's PKI is; `None` when neither. A list applies to a certificate only when its `crlCraca` ends in
    /// the certificate's `cracaId` and its series is the certificate's `crlSeries`.
    #[must_use]
    pub fn device_refuses(
        &self,
        device: NodeId,
        cert: &v2xw_msg::sec_types::ieee1609_dot2::Certificate,
    ) -> Option<HashRefusal> {
        use v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier;
        let dev = self.devices.get(&device)?;
        if dev.hash_held.values().all(Vec::is_empty) {
            return None;
        }
        let listed = |c: &v2xw_msg::sec_types::ieee1609_dot2::Certificate| {
            let tbs = &c.0.to_be_signed;
            let (craca, series) = (&tbs.craca_id.0[..], tbs.crl_series.0.0);
            let Ok(id) = v2xw_sec::sig1609::cert_hashed_id10(c) else {
                return false;
            };
            dev.hash_held
                .iter()
                .any(|((k, s), ids)| *s == series && k[5..] == *craca && ids.contains(&id))
        };
        if listed(cert) {
            return Some(HashRefusal::RevokedByHash);
        }
        let pki = self.pki.as_ref()?;
        let mut c = cert.clone();
        // Root, ICA levels, component: a short chain; the bound stops a loop.
        for _ in 0..8 {
            let up = match &c.0.issuer {
                IssuerIdentifier::sha256AndDigest(h) | IssuerIdentifier::sha384AndDigest(h) => {
                    v2xw_sec::hashedid::id8_bytes(h)
                }
                _ => return None,
            };
            let issuer = pki.by_id8(&up)?;
            if listed(&issuer.cert) {
                return Some(HashRefusal::IssuerRevoked);
            }
            c = issuer.cert.clone();
        }
        None
    }

    /// The MA certificate `device` encrypts its reports to: the first of the provider's
    /// MA certificates whose `MaSsp` covers BSM misbehaviour (PSID 0x20) and that no hash
    /// CRL the device holds revokes (SC-P8.2: a revoked MA is no longer written to).
    #[must_use]
    pub fn device_ma<'a>(
        &self,
        device: NodeId,
        pki: &'a crate::scms::pdu::ScmsPki,
    ) -> Option<&'a crate::scms::pdu::Authority> {
        core::iter::once(&pki.ma)
            .chain(pki.extra.iter())
            .filter(|a| crate::scms::mbr::ma_covers(&a.cert, crate::scms::mbr::BSM_APP_PSID))
            .find(|a| self.device_refuses(device, &a.cert).is_none())
    }
}

/// `full` bytes of the series-1 `SecuredCrl` plus every hash CRL beside it: what one
/// download or publication carries (SC-P8.2). Equal to `full` with no hash list.
fn with_hash_lists(full: u32, hash: &BTreeMap<u16, crate::scms::pdu::Pdu>) -> u32 {
    hash.values().fold(full, |b, p| b.saturating_add(p.len()))
}
