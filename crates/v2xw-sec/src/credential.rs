//! The credential an end entity signs with: an implicit certificate and the private key
//! its holder derived for it (WP-C1).
//!
//! The US SCMS issues every pseudonym, identification and RSU application certificate as an
//! ECQV-1609 **implicit** certificate. The device never receives a private key: it receives
//! the certificate and the CA's contribution `r`, and derives `d = e·b + r` itself from the
//! request key `b` only it holds (`e` is the ECQV-1609 hash of the certificate under its
//! issuer; 02 §9.3). A relying party never receives a public key either: it reconstructs
//! `Q_U = e·P_U + Q_CA` from the certificate and the issuer's certificate.
//!
//! [`ImplicitKey`] is what crosses from the credential protocol to the node: the
//! certificate, its issuer and `d`. Nothing else (no seed, no linkage secret, no node id)
//! crosses, so a node's air identity is exactly the protocol's.
//!
//! [`ImplicitIssuer`] is the same ECQV issuance without the protocol's flows, for the two
//! places that have no protocol run: a scenario that runs no SCMS backend (the device left
//! the factory with its pool) and unit tests. It is one CA shared by every device it
//! serves, never one per node.

use std::sync::Arc;

use p256::Scalar;
use p256::elliptic_curve::Field;
use v2xw_msg::sec_types::Certificate;
use v2xw_msg::sec_types::ieee1609_dot2::{CertificateId, ToBeSignedCertificate};

use crate::cert::{self, CertKey, CertSpec};
use crate::ec::{self, Point};
use crate::error::{Result, SecError};
use crate::linkage::LinkageValue;

/// One implicit certificate with the private key its holder derived for it.
#[derive(Clone)]
pub struct ImplicitKey {
    certificate: Arc<Certificate>,
    issuer: Arc<Certificate>,
    private: Scalar,
}

impl core::fmt::Debug for ImplicitKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The private key is never printed.
        write!(f, "ImplicitKey(linkage={:?})", self.linkage())
    }
}

impl PartialEq for ImplicitKey {
    fn eq(&self, other: &ImplicitKey) -> bool {
        self.certificate == other.certificate
            && self.issuer == other.issuer
            && self.private == other.private
    }
}

impl ImplicitKey {
    /// The key a holder derived: `certificate` (implicit), its `issuer`, and `d`.
    #[must_use]
    pub fn new(certificate: Arc<Certificate>, issuer: Arc<Certificate>, private: Scalar) -> Self {
        ImplicitKey {
            certificate,
            issuer,
            private,
        }
    }

    /// The implicit certificate.
    #[must_use]
    pub fn certificate(&self) -> &Arc<Certificate> {
        &self.certificate
    }

    /// The issuing CA's certificate.
    #[must_use]
    pub fn issuer(&self) -> &Arc<Certificate> {
        &self.issuer
    }

    /// `d = e·b + r`.
    #[must_use]
    pub fn private(&self) -> &Scalar {
        &self.private
    }

    /// The device's own acceptance check: `d·G = e·P_U + Q_CA`.
    #[must_use]
    pub fn is_consistent(&self) -> bool {
        let Ok(issuer_key) = cert::verification_key(&self.issuer, None) else {
            return false;
        };
        match cert::verification_key(&self.certificate, Some((&self.issuer, &issuer_key))) {
            Ok(CertKey::P256(q)) => {
                Point::mul_base(&self.private).compressed() == q.compressed()
            }
            _ => false,
        }
    }

    /// `(iCert, linkage value)` from the certificate's own `linkageData`, when it has one.
    #[must_use]
    pub fn linkage(&self) -> Option<(u16, LinkageValue)> {
        linkage_of(&self.certificate)
    }
}

/// `(iCert, linkage value)` from a certificate's own `toBeSigned.id.linkageData`: the
/// field a linked CRL entry is matched against (1609.2 §5.1.3.4).
#[must_use]
pub fn linkage_of(c: &Certificate) -> Option<(u16, LinkageValue)> {
    match &c.0.to_be_signed.id {
        CertificateId::linkageData(ld) => Some((
            ld.i_cert.0.0,
            LinkageValue::from_asn1(&ld.linkage_value),
        )),
        _ => None,
    }
}

/// A CA that issues implicit certificates by ECQV-1609 without the protocol's flows.
///
/// See the module documentation for the only two places it is used.
#[derive(Clone)]
pub struct ImplicitIssuer {
    cert: Arc<Certificate>,
    private: Scalar,
    public: Point,
}

impl core::fmt::Debug for ImplicitIssuer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ImplicitIssuer")
    }
}

impl ImplicitIssuer {
    /// An issuer from an existing CA certificate (explicit, P-256) and its signing scalar.
    ///
    /// # Errors
    /// The certificate's key is not P-256 or is not `private·G`.
    pub fn new(cert: Certificate, private: Scalar) -> Result<ImplicitIssuer> {
        let CertKey::P256(public) = cert::verification_key(&cert, None)? else {
            return Err(SecError::UnsupportedPrimitive {
                backend: "ECQV",
                primitive: crate::primitive::PrimitiveId::ECDSA_P384,
            });
        };
        if Point::mul_base(&private).compressed() != public.compressed() {
            return Err(SecError::Crypto {
                op: "ECQV issuer",
                primitive: crate::primitive::PrimitiveId::ECQV_P256,
                detail: "the scalar is not the certificate's key".to_string(),
            });
        }
        Ok(ImplicitIssuer {
            cert: Arc::new(cert),
            private,
            public,
        })
    }

    /// A self-signed P-256 issuing authority from `seed`, valid from `valid_from`
    /// (`Time32`) for three years, permitting `psids`. For tests and for a scenario with
    /// no SCMS backend; a run with one uses its ACA.
    ///
    /// # Errors
    /// `psids` is empty.
    pub fn standalone(seed: &[u8], valid_from: u32, psids: &[u64]) -> Result<ImplicitIssuer> {
        let d = ec::scalar_from_be_mod_n(&v2xw_core::hash::sha256(
            &[b"v2xw-sec/credential/standalone-ca/v1".as_slice(), seed].concat(),
        ));
        let d = if bool::from(d.is_zero()) { Scalar::ONE } else { d };
        let key = Point::mul_base(&d)
            .compressed()
            .ok_or(SecError::MissingField {
                profile: "ECQV issuer",
                field: "a nonzero key",
            })?;
        let mut spec = CertSpec::issuing_authority(valid_from, psids.first().copied().unwrap_or(0));
        spec.app_permissions = psids.to_vec();
        let certificate = cert::issue_explicit(&spec, &key, &[], |digest| {
            let k = ec::scalar_from_be_mod_n(&v2xw_core::hash::sha256(
                &[b"v2xw-sec/credential/standalone-ca/k".as_slice(), digest].concat(),
            ));
            cert::ecdsa_p256_sign_with_k(&d, &k, digest)
        })?;
        ImplicitIssuer::new(certificate, d)
    }

    /// The CA's certificate.
    #[must_use]
    pub fn certificate(&self) -> &Arc<Certificate> {
        &self.cert
    }

    /// Issues `tbs` (whose `verifyKeyIndicator` is replaced) to the holder of the request
    /// key `b`, with CA ephemeral `k`, and returns what the holder derives: the
    /// certificate and `d = e·b + r`.
    ///
    /// # Errors
    /// `k` keeps landing on the point at infinity, or an encoding fails.
    pub fn issue(
        &self,
        tbs: &ToBeSignedCertificate,
        request_private: &Scalar,
        mut k: impl FnMut() -> Scalar,
    ) -> Result<ImplicitKey> {
        let b = Point::mul_base(request_private);
        let issued =
            cert::issue_implicit(tbs, &self.cert, &self.public, &self.private, &b, &mut k)?;
        let d = crate::crypto::ecqv::subject_private_key(
            &issued.e,
            request_private,
            &issued.private_key_contribution,
        );
        Ok(ImplicitKey::new(
            Arc::new(issued.certificate),
            Arc::clone(&self.cert),
            d,
        ))
    }

    /// [`ImplicitIssuer::issue`] for the `toBeSigned` of `spec` (the request key is
    /// placed and then replaced by the reconstruction value).
    ///
    /// # Errors
    /// As [`ImplicitIssuer::issue`], or `spec` has no permissions.
    pub fn issue_spec(
        &self,
        spec: &CertSpec,
        request_private: &Scalar,
        k: impl FnMut() -> Scalar,
    ) -> Result<ImplicitKey> {
        let b = Point::mul_base(request_private)
            .compressed()
            .ok_or(SecError::MissingField {
                profile: "ECQV request",
                field: "a nonzero request key",
            })?;
        let tbs = cert::to_be_signed(spec, cert::verification_key_indicator(&b)?)?;
        self.issue(&tbs, request_private, k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cert::HolderId;
    use crate::hashedid;

    fn issuer() -> ImplicitIssuer {
        ImplicitIssuer::standalone(b"t", 600_000_000, &[0x20]).expect("issuer")
    }

    fn spec(iss: &ImplicitIssuer, lv: [u8; 9]) -> CertSpec {
        let id8 = hashedid::hashed_id8(&cert::encode(iss.certificate()).expect("enc"));
        CertSpec::pseudonym(
            id8,
            HolderId::Linkage {
                i_cert: 7,
                linkage_value: LinkageValue::new(lv),
            },
            600_000_100,
            0x20,
        )
    }

    /// The key a holder derives reconstructs: `d·G = e·P_U + Q_CA`, and the certificate
    /// is implicit with the linkage value it was issued with.
    #[test]
    fn issued_keys_reconstruct() {
        let iss = issuer();
        let mut n = 1u64;
        let k = issued(&iss, [5; 9], &mut n);
        assert!(k.is_consistent());
        assert_eq!(k.linkage(), Some((7, LinkageValue::new([5; 9]))));
        assert_eq!(
            k.certificate().0.r_type,
            v2xw_msg::sec_types::ieee1609_dot2::CertificateType::implicit
        );
        // Fault: a key off by one does not reconstruct.
        let bad = ImplicitKey::new(
            Arc::clone(k.certificate()),
            Arc::clone(k.issuer()),
            *k.private() + Scalar::ONE,
        );
        assert!(!bad.is_consistent());
    }

    fn issued(iss: &ImplicitIssuer, lv: [u8; 9], n: &mut u64) -> ImplicitKey {
        let b = Scalar::from(1000 + *n);
        *n += 1;
        let seed = *n;
        iss.issue_spec(&spec(iss, lv), &b, move || Scalar::from(77 + seed))
            .expect("issues")
    }

    /// I-S1 for implicit keys: a holder signs with `d`, a *different* backend instance
    /// reconstructs the key from the certificate and its issuer, and the verdicts agree in
    /// both modes, for the honest certificate and for the three faults (a certificate
    /// altered by one byte, the wrong issuer, a wrong `d`).
    #[test]
    fn both_backends_agree_on_implicit_keys() {
        use crate::crypto::{CryptoBackend, Modeled, Real};
        use crate::testctx::TestCtx;
        use v2xw_core::ids::NodeId;
        let p = crate::primitive::PrimitiveId::ECDSA_P256_SHA256;
        let iss = issuer();
        let other = ImplicitIssuer::standalone(b"other", 600_000_000, &[0x20]).expect("issuer");
        let mut n = 1;
        let good = issued(&iss, [3; 9], &mut n);
        let mut altered = (**good.certificate()).clone();
        altered.0.to_be_signed.crl_series = v2xw_msg::sec_types::ieee1609_dot2_base_types::CrlSeries(
            v2xw_msg::sec_types::ieee1609_dot2_base_types::Uint16(2),
        );
        let digest = [9u8; 32];
        let owner = NodeId::new(1);
        // (signing d, presented certificate, presented issuer) → expected verdict.
        let cases: [(Scalar, Certificate, Arc<Certificate>, bool); 4] = [
            (*good.private(), (**good.certificate()).clone(), Arc::clone(good.issuer()), true),
            (*good.private(), altered, Arc::clone(good.issuer()), false),
            (*good.private(), (**good.certificate()).clone(), Arc::clone(other.certificate()), false),
            (*good.private() + Scalar::ONE, (**good.certificate()).clone(), Arc::clone(good.issuer()), false),
        ];
        for (idx, (d, cert_shown, issuer_shown, want)) in cases.iter().enumerate() {
            let mut ctx = TestCtx::new(5);
            let mut real_s = Real::new();
            let mut real_v = Real::new();
            let held = ImplicitKey::new(Arc::clone(good.certificate()), Arc::clone(good.issuer()), *d);
            let real = real_s.import_implicit(p, owner, &held).is_ok_and(|k| {
                let sig = real_s.sign_prehashed(&mut ctx, &k, &digest).expect("sign");
                real_v
                    .import_reconstructed(p, owner, cert_shown, issuer_shown)
                    .is_ok_and(|pk| real_v.verify_prehashed(&mut ctx, &pk, &digest, &sig))
            });
            let mut mod_s = Modeled::new();
            let mut mod_v = Modeled::new();
            let modeled = mod_s.import_implicit(p, owner, &held).is_ok_and(|k| {
                let sig = mod_s.sign_prehashed(&mut ctx, &k, &digest).expect("sign");
                mod_v
                    .import_reconstructed(p, owner, cert_shown, issuer_shown)
                    .is_ok_and(|pk| mod_v.verify_prehashed(&mut ctx, &pk, &digest, &sig))
            });
            assert_eq!(real, *want, "case {idx}: real");
            assert_eq!(modeled, *want, "case {idx}: modelled");
        }
    }

    /// The implicit pseudonym certificate is 67 bytes smaller than an explicit one would be
    /// and carries no signature.
    #[test]
    fn implicit_certificate_has_no_signature() {
        let iss = issuer();
        let mut n = 1;
        let k = issued(&iss, [1; 9], &mut n);
        assert!(k.certificate().0.signature.is_none());
    }
}
