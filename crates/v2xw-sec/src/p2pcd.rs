//! Peer-to-peer certificate distribution (IEEE 1609.2 clause 8): the wire formats and the
//! recommended parameters. The protocol state (who requests, who answers, when) is the
//! node's (`v2xw-node::p2pcd`); this module is what goes on the air.
//!
//! # What is on the air
//!
//! | Piece | Where | Source |
//! |---|---|---|
//! | Out-of-band learning request | `HeaderInfo.p2pcdLearningRequest`, one `HashedId3` of an unknown CA certificate, in the requester's next signed SPDU | 1609.2-2022 §6.3.9; 1609.2a-2017 §8.2.4.1.1 |
//! | Out-of-band learning response | an unsecured `Ieee1609dot2Peer2PeerPDU { version 1, content caCerts }` broadcast by the P2PCD Entity; `caCerts[0]` is the requested certificate, each one is issued by the next, the last one is issued by a root | `Ieee1609Dot2Peer2Peer.asn` (1609.2-2022 module, minor version 3), its `@param content` note |
//! | Inline request | `HeaderInfo.inlineP2pcdRequest`, a `SequenceOfHashedId3` naming unknown CA *or* end-entity certificates | 1609.2a-2017 §8.2.4.1.2 |
//! | Inline response | `HeaderInfo.requestedCertificate`, one known CA certificate, or the responder's own certificate as its `SignerIdentifier` | 1609.2a-2017 §8.2.4.2.3 e) |
//!
//! The two request fields are mutually exclusive ([`crate::envelope::P2PCD_EXCLUSIVE_RULE`]).
//!
//! # The response PDU's encoding
//!
//! `Ieee1609dot2Peer2PeerPDU ::= SEQUENCE { version Uint8(1), content CHOICE { caCerts
//! CaCertP2pPDU, ... } }` with `CaCertP2pPDU ::= SEQUENCE OF Certificate`. In COER (ITU-T
//! X.696) the outer SEQUENCE has no optional field and no extension marker, so it has no
//! preamble; `version` is one octet; the CHOICE's root alternative `caCerts` is its tag
//! octet `0x80` (context class, number 0) followed by the bare value; the SEQUENCE OF is a
//! length-prefixed quantity followed by the certificates. Encoded here through the
//! generated `SequenceOfCertificate`, so the certificates are the bindings' COER; the
//! whole PDU is checked against asn1tools compiling the published module
//! (`tests/vectors/p2pcd_response.json`).
//!
//! # The PSID it is sent under
//!
//! The 1609.2a-2017 module comment says the PDU is "associated with a two-byte PSID to be
//! assigned" and "when broadcast over WSMP, to be encoded with COER". The IEEE RA's public
//! PSID list carries `peer-to-peer-distribution-of-security-management-information` = 136
//! (owner CAMP), whose P-encoding is two octets (`0x80 0x08`, 1609.12 §4.1.3). This build
//! uses 136 ([`P2PCD_PSID`]); the registry name is the match, not a sentence in the
//! standard, so the binding is labelled an inference.
//!
//! # Recommended parameters
//!
//! 1609.2a-2017 Annex C.2.1.3.1 gives one worked set, which [`P2pcdParams::RECOMMENDED`]
//! carries: `p2pcd_maxResponseBackoff = p2pcd_responseActiveTimeout =
//! p2pcd_requestActiveTimeout = p2pcd_observedRequestTimeout = 0.25 s`,
//! `p2pcd_currentlyUsedTriggerCertificateTime = 1 minute`,
//! `p2pcd_responseCountThreshold = 3`, for which the standard estimates "about 12
//! messages per second" per unknown certificate.

use v2xw_core::time::Duration;
use v2xw_msg::codec::MsgType;
use v2xw_msg::sec_types::{Certificate, SequenceOfCertificate, coer};

use crate::error::{Result, SecError};

/// The PSID an out-of-band learning response is broadcast under: 136,
/// `peer-to-peer-distribution-of-security-management-information` on the IEEE RA list
/// (read from Wireshark's transcription `IEEE1609dot12.asn`, sha256
/// `a8ad7385d6a28902e120e7883ebe132e1bdfb8e7462e92ce036c3d2f8457e559`, 2026-10-17). See
/// the module documentation for why the binding is an inference.
pub const P2PCD_PSID: u64 = 136;

/// `Ieee1609dot2Peer2PeerPDU.version` in this edition.
pub const P2P_PDU_VERSION: u8 = 1;

/// The COER tag octet of the `caCerts` alternative (context-specific, number 0).
const CA_CERTS_TAG: u8 = 0x80;

/// The P2PCD flavour an SDEE uses (`p2pcd_flavor`, 1609.2a-2017 §8.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum P2pcdFlavor {
    /// `p2pcdLearningRequest` in the header and separate response PDUs; CA certificates
    /// only. The original IEEE 1609.2-2016 form, and the one a US 1609.2 stack runs.
    OutOfBand,
    /// `inlineP2pcdRequest` and `requestedCertificate` in signed SPDUs; end-entity
    /// certificates too. The form TS 103 097 §7.1.1 makes the ETSI CAM profile's.
    Inline,
    /// No P2PCD (the CRL SDEE's profile, 1609.2a-2017 §7.4.4.2: "Full cert chain is
    /// attached").
    None,
}

impl P2pcdFlavor {
    /// The kebab-case spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            P2pcdFlavor::OutOfBand => "out-of-band",
            P2pcdFlavor::Inline => "inline",
            P2pcdFlavor::None => "none",
        }
    }
}

/// The SSME's P2PCD configuration parameters (1609.2a-2017 §8.2.3), plus the two limits
/// the standard leaves to the implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct P2pcdParams {
    /// `p2pcd_flavor`.
    pub flavor: P2pcdFlavor,
    /// `p2pcd_requestActiveTimeout`: no second request for the same certificate sooner.
    pub request_active_timeout: Duration,
    /// `p2pcd_observedRequestTimeout`: no request for a certificate someone else asked
    /// for this recently.
    pub observed_request_timeout: Duration,
    /// `p2pcd_maxResponseBackoff`: the responder's back-off is uniform on `[0, this]`.
    pub max_response_backoff: Duration,
    /// `p2pcd_responseActiveTimeout`: no second response process for the same certificate
    /// sooner.
    pub response_active_timeout: Duration,
    /// `p2pcd_currentlyUsedTriggerCertificateTime`: a node answers only for certificates
    /// in the chain of one it signed with this recently (out-of-band only).
    pub currently_used_trigger_certificate_time: Duration,
    /// `p2pcd_responseCountThreshold`: a response is sent only while fewer than this many
    /// have been observed.
    pub response_count_threshold: u32,
    /// The most response processes running at once. "Implementation-specific" (§8.2.4.2.2
    /// a)1)ii), PICS A.2.3.3); the value is an ASSUMPTION.
    pub max_active_responses: usize,
    /// The most entries one `inlineP2pcdRequest` carries. "The criteria for selection and
    /// the number of entries selected may be implementation-specific" (§8.2.4.1.2 d)1));
    /// the value is an ASSUMPTION.
    pub max_inline_entries: usize,
}

impl P2pcdParams {
    /// Annex C.2.1.3.1's worked values, out-of-band.
    pub const RECOMMENDED: P2pcdParams = P2pcdParams {
        flavor: P2pcdFlavor::OutOfBand,
        request_active_timeout: Duration::from_millis(250),
        observed_request_timeout: Duration::from_millis(250),
        max_response_backoff: Duration::from_millis(250),
        response_active_timeout: Duration::from_millis(250),
        currently_used_trigger_certificate_time: Duration::from_secs(60),
        response_count_threshold: 3,
        max_active_responses: 8,
        max_inline_entries: 8,
    };

    /// The recommended values with `flavor`.
    pub const fn recommended(flavor: P2pcdFlavor) -> P2pcdParams {
        let mut p = P2pcdParams::RECOMMENDED;
        p.flavor = flavor;
        p
    }
}

impl Default for P2pcdParams {
    fn default() -> Self {
        P2pcdParams::RECOMMENDED
    }
}

/// COER of `Ieee1609dot2Peer2PeerPDU { version 1, content caCerts certs }`.
///
/// # Errors
/// The certificates do not encode, or `certs` is empty (a response names the requested
/// certificate first, so an empty one answers nothing).
pub fn encode_learning_response(certs: &[Certificate]) -> Result<Vec<u8>> {
    if certs.is_empty() {
        return Err(SecError::MissingField {
            profile: "ieee1609dot2-peer2peer",
            field: "content.caCerts[0] (the requested certificate)",
        });
    }
    let seq = coer::encode(MsgType::Crl, &SequenceOfCertificate(certs.to_vec()))?;
    let mut out = Vec::with_capacity(2 + seq.len());
    out.push(P2P_PDU_VERSION);
    out.push(CA_CERTS_TAG);
    out.extend_from_slice(&seq);
    Ok(out)
}

/// Decodes a learning response into its `caCerts`, refusing anything that is not the
/// canonical COER of a version-1 `caCerts` PDU with at least one certificate.
///
/// # Errors
/// A wrong version, an unknown alternative, a malformed or non-canonical encoding, or an
/// empty `caCerts`.
pub fn decode_learning_response(bytes: &[u8]) -> Result<Vec<Certificate>> {
    let malformed = |field| SecError::MissingField {
        profile: "ieee1609dot2-peer2peer",
        field,
    };
    match bytes {
        [P2P_PDU_VERSION, CA_CERTS_TAG, rest @ ..] => {
            let seq: SequenceOfCertificate = coer::decode(MsgType::Crl, rest)?;
            // Strict: the bytes must be exactly the canonical encoding, so a trailing
            // octet or a non-minimal length cannot ride along.
            let again = coer::encode(MsgType::Crl, &seq)?;
            if again != rest {
                return Err(malformed(
                    "caCerts (not canonical COER, or trailing octets)",
                ));
            }
            if seq.0.is_empty() {
                return Err(malformed("content.caCerts[0]"));
            }
            Ok(seq.0)
        }
        [v, ..] if *v != P2P_PDU_VERSION => Err(malformed("version (must be 1)")),
        _ => Err(malformed("content (caCerts is the only known alternative)")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_response_is_refused_both_ways() {
        assert!(encode_learning_response(&[]).is_err());
        assert!(decode_learning_response(&[1, 0x80, 1, 0]).is_err());
        assert!(decode_learning_response(&[2, 0x80, 1, 0]).is_err());
        assert!(decode_learning_response(&[1, 0x81]).is_err());
    }

    #[test]
    fn the_recommended_values_are_annex_c() {
        let p = P2pcdParams::RECOMMENDED;
        assert_eq!(p.max_response_backoff, Duration::from_millis(250));
        assert_eq!(p.request_active_timeout, Duration::from_millis(250));
        assert_eq!(p.observed_request_timeout, Duration::from_millis(250));
        assert_eq!(p.response_active_timeout, Duration::from_millis(250));
        assert_eq!(
            p.currently_used_trigger_certificate_time,
            Duration::from_secs(60)
        );
        assert_eq!(p.response_count_threshold, 3);
    }
}
