//! The [`crate::MessageCodec`] seam for the IEEE 1609.3 WAVE Service Advertisement
//! (SC-M4, WP-19 leftover).
//!
//! [`Ieee1609WsaCodec`] claims [`MsgType::Wsa`] and routes it to [`crate::wsa`]'s
//! hand-written UPER codec, which `tests/wsa_oracle.rs` holds to asn1tools 0.169 byte for
//! byte on the cached IEEE 1609.3-2016 modules. Like the J2735 codecs, it carries the
//! message as [`Message::HandEncoded`] bytes: a roadside unit builds its own WSA
//! ([`crate::wsa::scms_service_wsa`], `v2xw-node` `RsuRuntime::wsa_payload`), and this
//! codec is the registry's statement of what those bytes are, and the check that a
//! payload is a canonical WSA before it is put on the air or after it is heard.

use v2xw_core::card::{Family, ModelCard, Source, SourceKind, Tier, Validation, ValidationStatus};
use v2xw_core::model::Model;

use crate::codec::{Encoded, Message, MessageCodec, MsgType};
use crate::error::CodecError;
use crate::j2735::uper::UperError;
use crate::wsa::{decode_wsa, encode_wsa};

/// Model id of the hand-written IEEE 1609.3 WSA codec.
pub const IEEE1609_WSA_CODEC_ID: &str = "codec/uper/ieee1609-3-wsa";

/// The UPER codec for the IEEE 1609.3-2016 `SrvAdvMsg`.
#[derive(Debug, Clone)]
pub struct Ieee1609WsaCodec {
    card: ModelCard,
}

impl Default for Ieee1609WsaCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl Ieee1609WsaCodec {
    /// The one type this codec implements.
    pub const TYPES: [MsgType; 1] = [MsgType::Wsa];

    /// Builds the codec and its card.
    pub fn new() -> Self {
        Self { card: card() }
    }
}

impl Model for Ieee1609WsaCodec {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

fn on_encode(e: UperError) -> CodecError {
    match e {
        UperError::OutOfRange {
            field,
            asn1_type,
            value,
            min,
            max,
        } => CodecError::OutOfRange {
            field,
            asn1_type,
            value,
            min,
            max,
        },
        other => CodecError::Encode {
            ty: MsgType::Wsa,
            detail: other.to_string(),
        },
    }
}

fn on_decode(len: usize) -> impl Fn(UperError) -> CodecError {
    move |e| match e {
        UperError::Unsupported { construct, detail } => CodecError::UnsupportedConstruct {
            ty: MsgType::Wsa,
            construct,
            detail,
        },
        other => CodecError::Decode {
            ty: MsgType::Wsa,
            len,
            detail: other.to_string(),
        },
    }
}

impl MessageCodec for Ieee1609WsaCodec {
    fn message_types(&self) -> &[MsgType] {
        &Self::TYPES
    }

    /// Validates already-encoded WSA bytes and hands them back: decoded (every constraint
    /// enforced), re-encoded and compared, as the J2735 codecs do. A payload that decodes
    /// but is not the canonical UPER this codec writes is refused, since a receiver that
    /// re-encoded it would hash different octets from the sender's.
    fn encode(&self, msg: &Message) -> Result<Encoded, CodecError> {
        let bytes = match msg {
            Message::HandEncoded {
                ty: MsgType::Wsa,
                bytes,
            } => bytes,
            other => {
                return Err(CodecError::Unsupported {
                    codec: IEEE1609_WSA_CODEC_ID.to_string(),
                    ty: other.msg_type(),
                });
            }
        };
        let wsa = decode_wsa(bytes).map_err(on_decode(bytes.len()))?;
        let re = encode_wsa(&wsa).map_err(on_encode)?;
        if re != *bytes {
            return Err(CodecError::Encode {
                ty: MsgType::Wsa,
                detail: format!(
                    "the payload decodes but is not in canonical UPER form: {} octets in, \
                     {} octets out",
                    bytes.len(),
                    re.len()
                ),
            });
        }
        Ok(Encoded::uper(re))
    }

    /// Decodes and returns the bytes as [`Message::HandEncoded`]; [`decode_wsa`] is the
    /// structured reader.
    fn decode(&self, bytes: &[u8], t: MsgType) -> Result<Message, CodecError> {
        if t != MsgType::Wsa {
            return Err(CodecError::Unsupported {
                codec: IEEE1609_WSA_CODEC_ID.to_string(),
                ty: t,
            });
        }
        decode_wsa(bytes).map_err(on_decode(bytes.len()))?;
        Ok(Message::HandEncoded {
            ty: t,
            bytes: bytes.to_vec(),
        })
    }
}

fn card() -> ModelCard {
    let mut card = ModelCard::new(
        IEEE1609_WSA_CODEC_ID,
        Family::Codec,
        "1.0.0",
        "Hand-written ASN.1 UPER codec for the IEEE 1609.3-2016 WAVE Service Advertisement \
         (SrvAdvMsg, WSA version 3) with every header, service-info, channel-info and \
         routing-advertisement extension its object sets list. Real wire bytes, checked \
         byte for byte against asn1tools 0.169 compiling the cached modules.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.limitations = crate::evidence::card_statement(IEEE1609_WSA_CODEC_ID);
    card.limitations.push(
        "The WSMP header and the IEEE 1609.2 signature around the WSA (1609.3-2016 Annex H.1, \
         USDOT RSU Spec v4.1 Req_586) are not this codec's: the network and security layers \
         add them."
            .to_string(),
    );
    card.limitations.push(
        "An extension id outside the modules' object sets is kept as its octets, since \
         every object set is extensible; its contents are not interpreted."
            .to_string(),
    );
    card.assumptions = vec![
        "The modules are the 2016 edition (TCI_ASN1 63d82d87976b), the one USDOT RSU Spec \
         v4.1 Req_586 cites; whether IEEE 1609.3-2020 changed them is unverified (paywalled)."
            .to_string(),
    ];
    card.ignores = vec![
        "The WAVE Short Message Protocol and IPv6 framing the advertisement rides in."
            .to_string(),
    ];
    card.sources = vec![
        Source::new(
            SourceKind::Standard,
            "IEEE 1609.3-2016 ASN.1 modules IEEE-1609-3-WSA, IEEE-1609-3-WEE and ISO 17419 \
             CITSapplMgmtIDs, third_party/asn1/ieee1609.3/ (PROVENANCE.md)",
        ),
        Source::new(
            SourceKind::Standard,
            "USDOT RSU Specification v4.1 (2017) Appendix D: the sample WSA octets \
             (RepeatRate 50 per 5 s), Req_586-588",
        ),
        Source::new(
            SourceKind::Standard,
            "ITU-T X.691 (02/2021) — Packed Encoding Rules, UNALIGNED variant",
        ),
    ];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![Source::new(
            SourceKind::Code,
            "Oracle-validated 2026-10-28: asn1tools 0.169 compiled from the cached modules \
             agreed byte for byte on 5 vectors in both directions (tests/wsa_oracle.rs, \
             vectors from tests/oracle/wsa_oracle.py).",
        )],
        tests: vec![
            "wsa::tests::round_trip_with_routing_advertisement".to_string(),
            "wsa::tests::a_minimal_wsa_is_two_octets_and_bad_bytes_are_refused".to_string(),
            "wsa_oracle::the_wsa_codec_writes_and_reads_what_asn1tools_does".to_string(),
            "wsa_codec::tests::the_codec_passes_a_canonical_wsa_and_refuses_the_rest"
                .to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    /// fault: `encode` hands the payload back without decoding it -> red (the malformed
    /// and the padded payloads pass).
    #[test]
    fn the_codec_passes_a_canonical_wsa_and_refuses_the_rest() {
        let codec: Box<dyn MessageCodec> = Box::new(Ieee1609WsaCodec::new());
        assert!(codec.supports(MsgType::Wsa));
        assert!(!codec.supports(MsgType::Bsm));
        let wsa = crate::wsa::scms_service_wsa(
            50,
            crate::wsa::Location {
                lat: 407_000_000,
                lon: -740_000_000,
            },
            100,
            174,
            20,
        );
        let bytes = encode_wsa(&wsa).expect("encodes");
        let out = codec
            .encode(&Message::HandEncoded {
                ty: MsgType::Wsa,
                bytes: bytes.clone(),
            })
            .expect("a canonical WSA passes");
        assert_eq!(out.bytes, bytes);
        assert_eq!(out.size as usize, bytes.len());
        assert!(out.is_real());
        match codec.decode(&bytes, MsgType::Wsa).expect("decodes") {
            Message::HandEncoded { ty, bytes: b } => {
                assert_eq!(ty, MsgType::Wsa);
                assert_eq!(b, bytes);
            }
            other => panic!("decoded to {other:?}"),
        }
        // Trailing octets past X.691's padding: decodes to the same WSA, not canonical.
        let mut padded = bytes.clone();
        padded.push(0);
        assert!(
            codec
                .encode(&Message::HandEncoded {
                    ty: MsgType::Wsa,
                    bytes: padded,
                })
                .is_err(),
            "a non-canonical payload is refused"
        );
        assert!(
            codec
                .encode(&Message::HandEncoded {
                    ty: MsgType::Wsa,
                    bytes: vec![0xFF],
                })
                .is_err(),
            "a payload that is not a WSA is refused"
        );
        assert!(codec.decode(&[0xFF], MsgType::Wsa).is_err());
        assert!(codec.decode(&bytes, MsgType::Bsm).is_err());
        assert!(
            codec
                .encode(&Message::HandEncoded {
                    ty: MsgType::Bsm,
                    bytes,
                })
                .is_err()
        );
        codec.card().validate().expect("the card validates");
    }
}
