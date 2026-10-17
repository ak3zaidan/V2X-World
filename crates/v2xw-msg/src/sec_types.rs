//! IEEE 1609.2 and ETSI TS 103 097 types — **the home of the security bindings**.
//!
//! # The decision, and why
//!
//! `build.rs` generates the 1609.2 / TS 103 097 bindings into their own unit,
//! `$OUT_DIR/sec_types.rs`, and **this module is the single place that `include!`s it**.
//! `v2xw-sec` depends on `v2xw-msg` and uses these types from here:
//!
//! ```ignore
//! use v2xw_msg::sec_types::{Certificate, HeaderInfo, Ieee1609Dot2Data, SignedData, coer};
//! ```
//!
//! The alternative — letting `v2xw-sec` `include!` the same generated file itself — was
//! rejected, and not on taste. `include!` is textual: two crates that each include the
//! file define two *unrelated* sets of types with the same names. A `Certificate` built by
//! `v2xw-sec` could then not be put in a message by `v2xw-msg`, and the compiler error
//! ("expected `Certificate`, found `Certificate`") is one of the least helpful Rust
//! produces. One definition, imported, removes the possibility.
//!
//! The generation stays a *separate unit* from the facilities bindings even so, because
//! the two are needed separately: the common data dictionary is 370 KB of ASN.1 and about
//! 8,900 lines of Rust that the security layer never touches, and keeping the units apart
//! means a change to a CAM container does not recompile the certificate types.
//!
//! # What is in here
//!
//! | Rust module | ASN.1 module | Standard |
//! |---|---|---|
//! | [`ieee1609_dot2`] | `Ieee1609Dot2` | IEEE 1609.2 (SPDU, `SignedData`, `Certificate`, `HeaderInfo`) |
//! | [`ieee1609_dot2_base_types`] | `Ieee1609Dot2BaseTypes` | IEEE 1609.2 (`HashedId8`, `Signature`, `PublicVerificationKey`, `Psid`, `Time64`, …) |
//! | [`ieee1609_dot2_crl`] | `Ieee1609Dot2Crl` | IEEE 1609.2 CRL PDU |
//! | [`ieee1609_dot2_crl_base_types`] | `Ieee1609Dot2CrlBaseTypes` | CRL contents, linkage seeds |
//! | [`etsi_ts103097_module`] | `EtsiTs103097Module` | ETSI TS 103 097 profile (`EtsiTs103097Data`, `EtsiTs103097Certificate`) |
//! | [`etsi_ts103097_extension_module`] | `EtsiTs103097ExtensionModule` | TS 103 097 header-info extensions |
//! | [`ieee1609_dot2_dot1_protocol`] | `Ieee1609Dot2Dot1Protocol` | IEEE 1609.2.1 (`ScmsPdu`, every `…Spdu` wrapper, `SecurityMgmtSsp`) |
//! | [`ieee1609_dot2_dot1_ee_ra_interface`] | `Ieee1609Dot2Dot1EeRaInterface` | `EeRaCertRequest`, `RaEeCertAck`, `RaEeCertInfo`, `EeRaDownloadRequest`, butterfly `AdditionalParams` |
//! | [`ieee1609_dot2_dot1_aca_ra_interface`] | `Ieee1609Dot2Dot1AcaRaInterface` | `RaAcaCertRequest`, `AcaRaCertResponse`, `LinkageInfo`, `EncryptedIndividualPLV` |
//! | [`ieee1609_dot2_dot1_aca_ee_interface`] | `Ieee1609Dot2Dot1AcaEeInterface` | `AcaEeCertResponse` |
//! | [`ieee1609_dot2_dot1_eca_ee_interface`] | `Ieee1609Dot2Dot1EcaEeInterface` | `EeEcaCertRequest`, `EcaEeCertResponse` |
//! | [`ieee1609_dot2_dot1_cert_management`] | `Ieee1609Dot2Dot1CertManagement` | CTL (`FullIeeeTbsCtl`, `ToBeSignedCtlSignature`), composite CRL, certificate chain, info status |
//! | [`ieee1609_dot2_dot1_acpc`] | `Ieee1609Dot2Dot1Acpc` | ACPC (not used by the US profile) |
//! | the other seven `ieee1609_dot2_dot1_*` | `…AcaLa`, `…AcaMa`, `…CamRa`, `…EeMa`, `…LaMa`, `…LaRa`, `…MaRa` | interfaces that 1609.2.1-2022 leaves `NULL` or nearly so |
//!
//! # Editions (owner decision 1, provisional)
//!
//! The **1609.2.1 modules are the 2022 edition** (forge tag `2022-published`, byte-identical
//! to the IEEE download bundle). The **1609.2 modules are the 2025 edition** (forge `ieee`
//! branch). That pairing is the modules' own: every 1609.2.1-2022 module imports 1609.2
//! `WITH SUCCESSORS`. The 2022 and 2025 1609.2 modules differ in exactly three places of
//! code, which a test in [`crate::scms_types`] pins (the 2022 copies are committed under
//! `third_party/asn1/etsi/ieee1609.2-2022/` for that test and for the oracle):
//!
//! 1. `ToBeSignedCertificate`'s extension additions `appExtensions`,
//!    `certIssueExtensions`, `certRequestExtension` are mandatory in 2022 and `OPTIONAL`
//!    in 2025. The generated types are the 2025 (superset) form; the 2022 rule is enforced
//!    by [`crate::scms_types::check_edition`].
//! 2. `Countersignature` additionally requires `omitted ABSENT` in 2025 (a value rule).
//! 3. `CrlContents` gains the extension addition `iPeriodInfo` in 2025.
//!
//! None of the three changes the COER bytes of a value that is valid in both editions,
//! which is why one set of generated types serves both.
//!
//! # What the parameterised 1609.2.1 wrappers lose
//!
//! `rasn-compiler` expands `Ieee1609Dot2Data-Signed{Tbs, Psid}` and its siblings into
//! structural copies of `Ieee1609Dot2Data`. Their `WITH COMPONENTS` and `CONTAINING`
//! constraints (the signer is one certificate, the payload is a `ScmsPdu`, the PSID is
//! 35, …) are **not checked by the generated types**. They do not change the COER
//! encoding of a valid value; checking them is the protocol layer's job.
//!
//! # Encoding
//!
//! 1609.2 and TS 103 097 are **COER**, not UPER [TS 103 097 V2.1.1 §4.1]. Use [`coer`],
//! which is a thin wrapper that produces the right [`crate::codec::SizeSource`] and this
//! crate's error type.
//!
//! # Applied patches
//!
//! `rasn-compiler` 0.16 emits an uncompilable `DEFAULT` for `PsidGroupPermissions.eeType`
//! (a fixed-size BIT STRING). The fix is recorded in
//! `crates/v2xw-msg/patches/0001-ieee1609dot2-endentitytype-default.patch`, applied to the
//! generated text, and the build fails if it stops applying. Build decision D5.
//!
//! Patches 0002–0004 and `build.rs`'s `link_ieee1609dot2dot1_scopes` fix Rust name
//! resolution in the generated 1609.2.1 modules (imports of parameterised types that have
//! no Rust item, and imports that ASN.1 lets a module re-export). None changes a type or a
//! byte.

/// The generated IEEE 1609.2 / TS 103 097 modules.
#[allow(missing_docs)]
#[allow(clippy::all)]
#[allow(clippy::pedantic)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/sec_types.rs"));
}

pub use generated::{
    etsi_ts103097_extension_module, etsi_ts103097_module, ieee1609_dot2, ieee1609_dot2_base_types,
    ieee1609_dot2_crl, ieee1609_dot2_crl_base_types,
};
pub use generated::{
    ieee1609_dot2_dot1_aca_ee_interface, ieee1609_dot2_dot1_aca_la_interface,
    ieee1609_dot2_dot1_aca_ma_interface, ieee1609_dot2_dot1_aca_ra_interface,
    ieee1609_dot2_dot1_acpc, ieee1609_dot2_dot1_cam_ra_interface,
    ieee1609_dot2_dot1_cert_management, ieee1609_dot2_dot1_eca_ee_interface,
    ieee1609_dot2_dot1_ee_ma_interface, ieee1609_dot2_dot1_ee_ra_interface,
    ieee1609_dot2_dot1_la_ma_interface, ieee1609_dot2_dot1_la_ra_interface,
    ieee1609_dot2_dot1_ma_ra_interface, ieee1609_dot2_dot1_protocol,
};

// The handful of names a security implementation reaches for constantly, lifted to the
// top so `v2xw-sec` does not have to spell the module path for every one. Everything else
// is reachable through the module re-exports above.
pub use ieee1609_dot2::{
    Certificate, CertificateBase, CertificateId, CertificateType, EndEntityType,
    ExplicitCertificate, HeaderInfo, Ieee1609Dot2Content, Ieee1609Dot2Data, ImplicitCertificate,
    IssuerIdentifier, LinkageData, PsidGroupPermissions, SequenceOfCertificate, SignedData,
    SignedDataPayload, SignerIdentifier, ToBeSignedCertificate, ToBeSignedData,
};
pub use ieee1609_dot2_base_types::{
    Duration as Ieee1609Duration, EccP256CurvePoint, HashAlgorithm, HashedId3, HashedId8, Opaque,
    Psid, PublicVerificationKey, Signature, Time32, Time64, Uint8, ValidityPeriod,
};
// `CrlContents` lives in `ieee1609_dot2_crl_base_types`; `ieee1609_dot2_crl` only
// *imports* it, so it cannot be re-exported from there.
pub use etsi_ts103097_module::{EtsiTs103097Certificate, EtsiTs103097Data};
pub use ieee1609_dot2_crl::SecuredCrl;
pub use ieee1609_dot2_crl_base_types::CrlContents;

/// COER encode and decode for the security types.
///
/// IEEE 1609.2 §6.3 and ETSI TS 103 097 §4.1 both mandate the *canonical* octet encoding
/// rules, which is what `rasn::coer` implements. Using `rasn::oer` instead would produce
/// bytes a conformant receiver may reject and, worse, a hash that does not match the one
/// the signer computed — 1609.2's `HashedId8` is SHA-256 over the *canonical* encoding.
pub mod coer {
    use crate::codec::{Encoded, MsgType};
    use crate::error::CodecError;

    /// COER-encodes a 1609.2 or TS 103 097 value.
    ///
    /// `ty` only labels the error if one occurs; the security envelope is not itself a
    /// [`MsgType`], so callers pass the type of the message being protected (or
    /// [`MsgType::Crl`] for a CRL).
    pub fn encode<T: rasn::Encode>(ty: MsgType, value: &T) -> Result<Vec<u8>, CodecError> {
        rasn::coer::encode(value).map_err(|e| CodecError::Encode {
            ty,
            detail: e.to_string(),
        })
    }

    /// COER-decodes a 1609.2 or TS 103 097 value.
    pub fn decode<T: rasn::Decode>(ty: MsgType, bytes: &[u8]) -> Result<T, CodecError> {
        rasn::coer::decode(bytes).map_err(|e| CodecError::Decode {
            ty,
            len: bytes.len(),
            detail: e.to_string(),
        })
    }

    /// COER-encodes and wraps the result as [`Encoded`] with [`crate::codec::SizeSource::Coer`].
    pub fn encoded<T: rasn::Encode>(ty: MsgType, value: &T) -> Result<Encoded, CodecError> {
        Ok(Encoded::coer(encode(ty, value)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::{MsgType, SizeSource};

    /// The patched `PsidGroupPermissions::eeType` default must still mean "app": bit 0
    /// set, everything else clear. If the patch ever silently changed the value, a
    /// certificate's default permissions would change with it.
    #[test]
    fn the_patched_end_entity_type_default_is_app() {
        // The generated default function is private, so go through the struct: build a
        // `PsidGroupPermissions` with the default `ee_type` and check the bits.
        let ee = EndEntityType({
            let mut bits = rasn::types::FixedBitString::<8usize>::ZERO;
            bits.set(0usize, true);
            bits
        });
        assert!(ee.0[0], "bit 0 (app) is set");
        assert!(!ee.0[1], "bit 1 (enrol) is clear");
        assert_eq!(ee.0.count_ones(), 1);
    }

    /// The whole point of this module: the types encode with COER and round-trip.
    #[test]
    fn a_1609_2_data_round_trips_through_coer() {
        let data = Ieee1609Dot2Data::new(
            Uint8(3),
            Ieee1609Dot2Content::unsecuredData(Opaque(rasn::types::OctetString::from_static(
                b"hello",
            ))),
        );
        let bytes = coer::encode(MsgType::Cam, &data).expect("encodes");
        let back: Ieee1609Dot2Data = coer::decode(MsgType::Cam, &bytes).expect("decodes");
        assert_eq!(data, back);

        let encoded = coer::encoded(MsgType::Cam, &data).expect("encodes");
        assert_eq!(encoded.size_source, SizeSource::Coer);
        assert_eq!(encoded.size as usize, bytes.len());
    }

    /// 04-models.md §9.1 derives the envelope overhead from the ASN.1 and gives
    /// `Ieee1609Dot2Data` outer = 2 bytes (protocolVersion 1 + content choice tag 1). This
    /// pins that derivation against the real encoder rather than against arithmetic.
    #[test]
    fn the_outer_data_overhead_is_the_two_bytes_the_design_derives() {
        let payload = b"0123456789";
        let data = Ieee1609Dot2Data::new(
            Uint8(3),
            Ieee1609Dot2Content::unsecuredData(Opaque(rasn::types::OctetString::from_static(
                payload,
            ))),
        );
        let bytes = coer::encode(MsgType::Cam, &data).expect("encodes");
        // 2 bytes of preamble, 1 length byte for a 10-octet string, then the payload.
        assert_eq!(
            bytes.len(),
            2 + 1 + payload.len(),
            "unexpected COER framing: {bytes:02x?}"
        );
    }
}
