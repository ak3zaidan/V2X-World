//! The [`crate::MessageCodec`] seam for the hand-written J2735 infrastructure messages.
//!
//! [`J2735InfraCodec`] claims [`MsgType::Spat`] and [`MsgType::Map`] and routes them to
//! [`crate::j2735::spat`] and [`crate::j2735::map`]. It is a separate codec from
//! [`crate::j2735::J2735BsmCodec`] for one reason, and it is the reason this module has a
//! long card: **the evidence behind its bytes is weaker**, and a registry entry is where a
//! run's manifest goes to find that out. Merging the two would average an oracle-validated
//! encoder together with an unvalidated one behind a single status.
//!
//! Like the BSM codec, this one carries messages as [`Message::HandEncoded`] bytes rather
//! than as a structured [`Message`] variant: putting a MAP's lane geometry into the
//! engine-wide message enum would make every consumer of that enum depend on J2735's field
//! layout. [`crate::j2735::spat::encode_spat`] and [`crate::j2735::map::encode_map`] are
//! the structured path.

use v2xw_core::card::{
    Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation, ValidationStatus,
};
use v2xw_core::model::Model;

use crate::codec::{Encoded, Message, MessageCodec, MsgType};
use crate::error::CodecError;
use crate::j2735::{map, spat};

/// Model id of the hand-written J2735 SPaT and MAP codec.
pub const J2735_INFRA_CODEC_ID: &str = "codec/uper/j2735-spat-map";

/// ETSI `MessageId` of a SPATEM, `spatem(4)` (ETSI TS 102 894-2).
pub const SPATEM_MESSAGE_ID: u8 = 4;
/// ETSI `MessageId` of a MAPEM, `mapem(5)` (ETSI TS 102 894-2).
pub const MAPEM_MESSAGE_ID: u8 = 5;
/// The `protocolVersion` a SPATEM or MAPEM header carries.
///
/// **Recalled, UNVERIFIED**: 2, the Release 2 value the CAM and DENM of this crate carry
/// from their own ASN.1. TS 103 301 is not in this repository to confirm the SPATEM and
/// MAPEM value; the field is one octet either way, so only its content is uncertain.
pub const INFRA_PROTOCOL_VERSION: u8 = 2;
/// The octets of the ETSI `ItsPduHeader` at the front of a SPATEM or MAPEM: 8 + 8 + 32
/// bits, octet-aligned.
pub const ITS_PDU_HEADER_B: usize = 6;

/// An ETSI SPATEM or MAPEM (TS 103 301): the `ItsPduHeader` — `protocolVersion`,
/// `messageId`, `stationId` — followed by the SPAT or MAP of ISO TS 19091's DSRC module.
///
/// `body` is the bare J2735 PDU ([`spat::encode_spat`] or [`map::encode_map`], not the
/// `MessageFrame`). The header is exactly 48 bits, so the SPATEM's UPER encoding is the two
/// encodings back to back; `SPATEM ::= SEQUENCE { header, spat }` has no preamble bits of
/// its own. That ISO TS 19091's SPAT and MAP are J2735's for the fields this crate fills is
/// **recalled, not re-read**, and is on this codec's card with the protocol version.
///
/// # Errors
/// The header encoder's error, which a station id in range cannot produce.
pub fn its_wrap(
    ty: MsgType,
    message_id: u8,
    station_id: u32,
    body: &[u8],
) -> Result<Vec<u8>, CodecError> {
    use crate::asn1::cdd::{ItsPduHeader, MessageId, OrdinalNumber1B, StationId};
    let header = ItsPduHeader::new(
        OrdinalNumber1B(INFRA_PROTOCOL_VERSION),
        MessageId(message_id),
        StationId(station_id),
    );
    let mut out = crate::codec::uper_encode(ty, &header)?;
    debug_assert_eq!(out.len(), ITS_PDU_HEADER_B);
    out.extend_from_slice(body);
    Ok(out)
}

/// The `(messageId, stationId, body)` of a SPATEM or MAPEM, or `None` when the bytes are
/// too short to hold the header.
pub fn its_unwrap(bytes: &[u8]) -> Option<(u8, u32, &[u8])> {
    if bytes.len() < ITS_PDU_HEADER_B {
        return None;
    }
    let station = u32::from_be_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
    Some((bytes[1], station, &bytes[ITS_PDU_HEADER_B..]))
}

/// The hand-written J2735 SPaT and MAP codec.
///
/// Stateless; one instance serves every RSU, and building the card is its only cost.
#[derive(Debug, Clone)]
pub struct J2735InfraCodec {
    card: ModelCard,
}

impl Default for J2735InfraCodec {
    fn default() -> Self {
        Self::new()
    }
}

impl J2735InfraCodec {
    /// The two types this codec implements.
    pub const TYPES: [MsgType; 2] = [MsgType::Spat, MsgType::Map];

    /// Builds the codec and its card.
    pub fn new() -> Self {
        Self { card: card() }
    }
}

impl Model for J2735InfraCodec {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MessageCodec for J2735InfraCodec {
    fn message_types(&self) -> &[MsgType] {
        &Self::TYPES
    }

    /// Validates already-encoded bytes and hands them back.
    ///
    /// The same contract as [`crate::j2735::J2735BsmCodec::encode`]: the bytes are decoded,
    /// which enforces every constraint and every refusal, then re-encoded and compared. A
    /// difference means the payload is not in the canonical form this codec produces, and
    /// is refused rather than passed on — a receiver that re-encoded to sign or hash would
    /// otherwise compute a different digest from the sender's.
    ///
    /// For a MAP, "canonical" includes each node's `NodeOffsetPointXY` alternative, which
    /// [`map::XyOffset`] keeps as part of the value precisely so that this comparison can
    /// succeed for an encoder that chose a wider alternative than strictly necessary.
    fn encode(&self, msg: &Message) -> Result<Encoded, CodecError> {
        let (ty, bytes) = match msg {
            Message::HandEncoded { ty, bytes } if Self::TYPES.contains(ty) => (*ty, bytes),
            other => {
                return Err(CodecError::Unsupported {
                    codec: J2735_INFRA_CODEC_ID.to_string(),
                    ty: other.msg_type(),
                });
            }
        };
        let re = match ty {
            MsgType::Spat => spat::encode_spat(&spat::decode_spat(bytes)?)?,
            MsgType::Map => map::encode_map(&map::decode_map(bytes)?)?,
            // Unreachable: the match above admits only the two types in `TYPES`. If a
            // third is ever added there without a branch here, this refuses it by name
            // rather than encoding it as one of the other two.
            other => {
                return Err(CodecError::Unsupported {
                    codec: J2735_INFRA_CODEC_ID.to_string(),
                    ty: other,
                });
            }
        };
        if re.bytes != *bytes {
            return Err(CodecError::Encode {
                ty,
                detail: format!(
                    "the payload decodes but is not in canonical UPER form: {} octets in, \
                     {} octets out",
                    bytes.len(),
                    re.bytes.len()
                ),
            });
        }
        Ok(re)
    }

    /// Decodes and returns the bytes as [`Message::HandEncoded`].
    ///
    /// What the caller gains over keeping its own bytes is the guarantee that they decode:
    /// [`spat::decode_spat`] and [`map::decode_map`] are the structured readers.
    fn decode(&self, bytes: &[u8], t: MsgType) -> Result<Message, CodecError> {
        match t {
            MsgType::Spat => {
                spat::decode_spat(bytes)?;
            }
            MsgType::Map => {
                map::decode_map(bytes)?;
            }
            other => {
                return Err(CodecError::Unsupported {
                    codec: J2735_INFRA_CODEC_ID.to_string(),
                    ty: other,
                });
            }
        }
        Ok(Message::HandEncoded {
            ty: t,
            bytes: bytes.to_vec(),
        })
    }
}

/// The card: what is real, what is refused, and what is not yet proven.
fn card() -> ModelCard {
    let mut card = ModelCard::new(
        J2735_INFRA_CODEC_ID,
        Family::Codec,
        "1.0.0",
        "Hand-written ASN.1 UPER codec for the SAE J2735 SPAT and MapData messages, over \
         the subset an intersection application fills. Real wire bytes, measured sizes, \
         cross-validated byte for byte against pycrate compiled from the public SAE J2735 \
         2016 and 2020 modules (the 2024-09 modules are not public).",
    );
    // A codec has no cheap variant: the abstract tier still needs an exact size.
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];

    // The byte-exactness statement, generated from the crate's one evidence table so the
    // card and the table cannot drift apart.
    card.limitations = crate::evidence::card_statement(J2735_INFRA_CODEC_ID);

    card.limitations.push(
        "Real fields — SPAT: timeStamp; per IntersectionState id (region and id), revision, \
         status (16-bit IntersectionStatusObject), moy, timeStamp, and 1..255 MovementState, \
         each with signalGroup and 1..16 MovementEvent carrying eventState \
         (MovementPhaseState) and an optional TimeChangeDetails (startTime, minEndTime, \
         maxEndTime, likelyTime, confidence, nextTime)."
            .to_string(),
    );
    card.limitations.push(
        "Real fields — MapData: timeStamp, msgIssueRevision, and 1..32 IntersectionGeometry, \
         each with id, revision, refPoint (Position3D lat/long/elevation), laneWidth and \
         1..255 GenericLane carrying laneID, ingressApproach, egressApproach, \
         laneAttributes (directionalUse, sharedWith, laneType: vehicle only), maneuvers, an \
         explicit NodeListXY node set of 2..63 NodeXY over all six node-XY alternatives and \
         node-LatLon, and 1..16 Connection (connectingLane with its maneuver, \
         remoteIntersection, signalGroup, userClass, connectionID)."
            .to_string(),
    );
    card.limitations.push(
        "Unimplemented and REFUSED rather than skipped — SPAT: name, regional, \
         IntersectionState.name/enabledLanes/maneuverAssistList/regional, \
         MovementState.movementName/maneuverAssistList/regional, \
         MovementEvent.speeds/regional. Refusing is not caution, it is correctness: PER \
         carries no tags, so a decoder that stepped over an unmodelled element would \
         misread every element after it."
            .to_string(),
    );
    card.limitations.push(
        "Unimplemented and REFUSED — MapData: layerType, layerID, roadSegments, \
         dataParameters, restrictionList, regional, IntersectionGeometry.name/speedLimits/\
         preemptPriorityData/regional, GenericLane.name/overlays/regional, \
         NodeListXY.computed (a ComputedLane), NodeXY.attributes, the seven \
         LaneTypeAttributes alternatives other than vehicle, and \
         NodeOffsetPointXY.regional."
            .to_string(),
    );
    card.limitations.push(
        "Because layerType and layerID are refused, a MAP from this codec is a conformant \
         MapData *encoding* but not a CTI 4501-conformant *message*: that profile expects \
         both fields. Nothing in the simulator reads them, and encoding an enumeration whose \
         width has not been verified would risk shifting every field after it."
            .to_string(),
    );
    card.limitations.push(
        "Every SEQUENCE and CHOICE extension marker in the MAP structure is a named \
         constant in v2xw_msg::j2735::map::assumptions. They were written from recall; the \
         oracle run of 2026-10-06 (112 MAP vectors byte-identical with pycrate) confirms \
         every one against the 2016 and 2020 modules."
            .to_string(),
    );
    card.limitations.push(
        "MovementPhaseState is a 4-bit non-extensible ENUMERATED index, which the oracle \
         confirms (115 SPaT vectors byte-identical). The size-model derivation in \
         codec/size-model/j2735 counted 5 bits for the same field and is one bit per \
         movement event too large."
            .to_string(),
    );
    card.limitations.push(
        "Edition: the codec writes J2735 2024-09, whose TimeMark is (0..36111) with 36111 \
         meaning unknown; the 2016 and 2020 modules the oracle compiled bound it at 36001. \
         Both need 16 bits, so every value both editions admit was checked; the 2024-only \
         values 36002..36111 were not, and no 2024 change outside the fields this codec \
         fills could be seen."
            .to_string(),
    );

    card.assumptions = vec![
        "The message set is SAE J2735 2024-09. The constraint values, bit-string sizes and \
         CHOICE alternative counts were written from recall and then checked by the \
         pycrate oracle against the public 2016 (J2735_201603DA) and 2020 modules, read on \
         2026-10-06 from the USDOT JPO ODE asn1_codec repository's history; the modules \
         stay outside this repository (build decision D3)."
            .to_string(),
        "Latitude, Longitude, Elevation and MsgCount reuse the constants of \
         v2xw_msg::j2735::bsm, which the 235-vector pycrate oracle validated, so those four \
         field widths are as well evidenced here as they are in a BSM."
            .to_string(),
        "MinuteOfTheYear and DSecond are derived from the scenario wall clock and SimTime, \
         never from a system clock, and ignore leap seconds — the same assumption \
         v2xw_msg::j2735::bsm::sec_mark already makes."
            .to_string(),
        "A node offset keeps the node-XY alternative it was encoded in, so a decode \
         followed by an encode is byte-exact even when a narrower alternative would have \
         held the value."
            .to_string(),
    ];

    card.ignores = vec![
        "Regional extensions entirely, in every structure of both messages. They are the \
         reason J2735 cannot be code-generated at all (build decision D2)."
            .to_string(),
        "SRM, SSM and PSM, which remain in the validated size model \
         codec/size-model/j2735 with placeholder bytes."
            .to_string(),
        "The IEEE 1609.2 security envelope, which v2xw-sec puts around these payloads.".to_string(),
    ];

    // The unverified structural choices, as registry parameters, so they appear on the
    // todo-calibrate report (rule R1) instead of living only in prose. Each carries the
    // plan that would settle it.
    const ORACLE_PLAN: &str = "Settled for the 2016 and 2020 editions by the pycrate oracle \
                               run of 2026-10-06 (tests/j2735_infra_oracle.rs with \
                               V2XW_J2735_ORACLE_EDITION set); re-run it against the \
                               2024-09 modules, which are not public, to settle it for that \
                               edition.";
    let unverified = [
        (
            "spat_movement_phase_state_index_bits",
            "bit",
            serde_json::json!(spat::MOVEMENT_PHASE_STATE_BITS),
            "MovementPhaseState is encoded as a non-extensible 10-value ENUMERATED (X.691 \
             clause 14.3); the size-model derivation counted 5 bits, which is what an \
             extensible one costs",
        ),
        (
            "map_position3d_has_extension_marker",
            "-",
            serde_json::json!(map::assumptions::EXT_POSITION_3D),
            "whether Position3D carries `...`, which adds one bit before its optional-field \
             bit-map",
        ),
        (
            "map_lane_attributes_has_extension_marker",
            "-",
            serde_json::json!(map::assumptions::EXT_LANE_ATTRIBUTES),
            "whether LaneAttributes carries `...`; modelled as not, by analogy with \
             IntersectionReferenceID, whose 1-bit preamble the size-model derivation records",
        ),
        (
            "map_node_xy_has_extension_marker",
            "-",
            serde_json::json!(map::assumptions::EXT_NODE_XY),
            "whether NodeXY carries `...`",
        ),
        (
            "map_node_list_xy_has_extension_marker",
            "-",
            serde_json::json!(map::assumptions::EXT_NODE_LIST_XY),
            "whether the NodeListXY CHOICE carries `...`, which adds one bit before its \
             alternative index",
        ),
        (
            "map_lane_type_attributes_has_extension_marker",
            "-",
            serde_json::json!(map::assumptions::EXT_LANE_TYPE_ATTRIBUTES),
            "whether the LaneTypeAttributes CHOICE carries `...`",
        ),
        (
            "map_connection_has_extension_marker",
            "-",
            serde_json::json!(map::assumptions::EXT_CONNECTION),
            "whether Connection carries `...`",
        ),
        (
            "map_lane_width_max_cm",
            "cm",
            serde_json::json!(map::LANE_WIDTH_MAX),
            "the upper bound of LaneWidth, which fixes the field's width at 15 bits; the \
             size-model derivation counted 16",
        ),
    ];
    for (name, unit, default, what) in unverified {
        let mut parameter = Parameter::new(name, unit, default, Source::todo_calibrate(what));
        parameter.calibration = Some(ORACLE_PLAN.to_string());
        card.parameters.push(parameter);
    }

    card.sources = vec![
        Source::new(
            SourceKind::Standard,
            "SAE J2735 2024-09 — SPAT and MapData (written from recall; checked against the \
             2016 and 2020 modules, which are never redistributed, per build decision D3)",
        ),
        Source::new(
            SourceKind::Standard,
            "SAE J2735 2016-03 (J2735_201603DA.ASN) and 2020 module set, read from the history \
             of github.com/usdot-jpo-ode/asn1_codec (commits cb83a39a and 31ab0738)",
        ),
        Source::new(
            SourceKind::Standard,
            "SAE J2735 2024-09 — MessageFrame, DSRCmsgID mapData(18) and \
             signalPhaseAndTimingMessage(19)",
        ),
        Source::new(
            SourceKind::Standard,
            "ITU-T X.691 (02/2021) — Packed Encoding Rules, UNALIGNED variant: clauses 11.2, \
             11.5, 11.9, 14, 16, 19, 23",
        ),
        Source::new(
            SourceKind::Code,
            "this repository: crates/v2xw-msg/src/size_model.rs — the derivation table \
             written while the J2735 modules were on disk, which corroborates the SPAT \
             preamble widths and list determinants used here",
        ),
        Source::new(
            SourceKind::Code,
            "this repository: docs/design/12-build-decisions.md D2 (encoder strategy), D3 \
             (the J2735 modules are never committed), D9 (writer-side quantisation)",
        ),
    ];

    card.validation = Validation {
        // `LiteratureChecked`, the BSM codec's rung: an independent implementation of the
        // standard's own ASN.1 agreed with every byte.
        status: ValidationStatus::LiteratureChecked,
        references: vec![Source::new(
            SourceKind::Code,
            "Oracle-validated 2026-10-06: pycrate 0.8.1 compiled from the SAE J2735 2016 and \
             2020 modules agreed byte for byte with 115 of 115 SPaT and 112 of 112 MAP \
             vectors, decoded each to the same fields, encoded vectors of its own that this \
             codec decoded to the same fields, and produced 10 messages using elements this \
             codec refuses, each of which it refused by name (tests/j2735_infra_oracle.rs).",
        )],
        tests: vec![
            "j2735::spat::tests::a_minimal_spat_is_eleven_octets".to_string(),
            "j2735::spat::tests::round_trip_is_exact".to_string(),
            "j2735::spat::tests::an_unmodelled_element_is_refused_rather_than_skipped".to_string(),
            "j2735::map::tests::a_minimal_map_is_twenty_nine_octets".to_string(),
            "j2735::map::tests::round_trip_is_exact".to_string(),
            "j2735::map::tests::a_node_offset_keeps_its_alternative_so_the_bytes_are_stable"
                .to_string(),
            "j2735::map::tests::the_structural_assumptions_are_the_ones_the_module_documents"
                .to_string(),
            "j2735_infra_oracle::rust_spat_encodings_match_pycrate_byte_for_byte".to_string(),
            "j2735_infra_oracle::rust_map_encodings_match_pycrate_byte_for_byte".to_string(),
            "j2735_infra_oracle::pycrate_encodings_decode_field_for_field".to_string(),
            "j2735_infra_oracle::refused_elements_are_refused_rather_than_misread".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codec::SizeSource;
    use crate::j2735::spat::{
        IntersectionReferenceId, IntersectionState, IntersectionStatus, MovementEvent,
        MovementPhaseState, MovementState, Spat,
    };

    fn spat_bytes() -> Vec<u8> {
        spat::encode_spat(&Spat::one(IntersectionState {
            id: IntersectionReferenceId::new(1),
            revision: 0,
            status: IntersectionStatus::NONE,
            moy: None,
            time_stamp: None,
            states: vec![MovementState::current(
                1,
                MovementEvent::phase(MovementPhaseState::StopAndRemain),
            )],
        }))
        .expect("encodes")
        .bytes
    }

    fn map_bytes() -> Vec<u8> {
        let lane = map::GenericLane {
            lane_id: 1,
            ingress_approach: Some(1),
            egress_approach: None,
            attributes: map::LaneAttributes::vehicle(map::LaneDirection::INGRESS),
            maneuvers: Some(map::AllowedManeuvers::STRAIGHT),
            nodes: vec![
                map::NodeXy::offset(0, 0).expect("fits"),
                map::NodeXy::offset(150, 2_000).expect("fits"),
            ],
            connects_to: vec![map::Connection::signalised(2, 1)],
        };
        map::encode_map(&map::MapData {
            time_stamp: None,
            msg_issue_revision: 1,
            intersections: vec![map::IntersectionGeometry {
                id: IntersectionReferenceId::new(1),
                revision: 1,
                ref_point: map::Position3D {
                    lat: 407_440_000,
                    lon: -739_900_000,
                    elevation: Some(100),
                },
                lane_width_cm: Some(map::lane_width_cm(3.5)),
                lanes: vec![lane],
            }],
        })
        .expect("encodes")
        .bytes
    }

    #[test]
    fn the_card_validates_and_names_the_family() {
        let codec = J2735InfraCodec::new();
        codec.card().validate().expect("card validates");
        codec.card().check_api_version().expect("api version");
        assert_eq!(codec.family(), Family::Codec);
        assert_eq!(codec.id(), J2735_INFRA_CODEC_ID);
    }

    /// The card is where a run's manifest learns that these bytes are unvalidated. A
    /// silent deletion of that sentence is a defect in its own right — it is the one thing
    /// that must never happen to this crate's claims.
    #[test]
    fn the_card_says_what_the_oracle_validated_and_against_which_edition() {
        let card = J2735InfraCodec::new().card().clone();
        assert_eq!(card.validation.status, ValidationStatus::LiteratureChecked);
        let references = card
            .validation
            .references
            .iter()
            .map(|s| s.reference.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(references.contains("Oracle-validated"), "{references}");
        assert!(references.contains("2016") && references.contains("2020"), "{references}");

        let text = card.limitations.join("\n");
        for needle in [
            "oracle-validated",
            "assumptions",
            "MovementPhaseState",
            "REFUSED",
            "2024-09",
        ] {
            assert!(text.contains(needle), "the card should mention {needle}");
        }
        assert!(
            card.purpose.contains("2024-09 modules are not public"),
            "the one-line purpose names the editions it was checked against: {}",
            card.purpose
        );
        // The structural choices stay on the todo-calibrate report until the 2024-09
        // modules, which are not public, have been compared too.
        let todo: Vec<&str> = card.todo_calibrate().map(|p| p.name.as_str()).collect();
        assert!(
            todo.iter().any(|n| n.contains("extension_marker")),
            "{todo:?}"
        );
        assert!(
            todo.iter()
                .any(|n| *n == "spat_movement_phase_state_index_bits"),
            "{todo:?}"
        );
    }

    #[test]
    fn it_claims_exactly_spat_and_map() {
        let codec = J2735InfraCodec::new();
        assert!(codec.supports(MsgType::Spat));
        assert!(codec.supports(MsgType::Map));
        for ty in MsgType::ALL {
            if ty != MsgType::Spat && ty != MsgType::Map {
                assert!(!codec.supports(ty), "{ty} should not be claimed");
            }
        }
    }

    #[test]
    fn the_seam_validates_bytes_and_returns_them_unchanged() {
        let codec = J2735InfraCodec::new();
        for (ty, bytes) in [(MsgType::Spat, spat_bytes()), (MsgType::Map, map_bytes())] {
            let encoded = codec
                .encode(&Message::HandEncoded {
                    ty,
                    bytes: bytes.clone(),
                })
                .unwrap_or_else(|e| panic!("{ty} should be valid: {e}"));
            assert_eq!(encoded.bytes, bytes, "{ty}");
            assert_eq!(encoded.size_source, SizeSource::Uper, "{ty}");
            assert!(encoded.is_real(), "{ty}");

            let Message::HandEncoded {
                ty: back_ty,
                bytes: back,
            } = codec.decode(&bytes, ty).expect("decodes")
            else {
                unreachable!("this codec returns HandEncoded")
            };
            assert_eq!(back_ty, ty);
            assert_eq!(back, bytes);
        }
    }

    #[test]
    fn the_seam_refuses_bytes_that_are_not_the_message_it_was_told() {
        let codec = J2735InfraCodec::new();
        // A MAP's bytes are not a SPaT, and the seam must not pretend otherwise.
        assert!(codec.decode(&map_bytes(), MsgType::Spat).is_err());
        assert!(codec.decode(&spat_bytes(), MsgType::Map).is_err());
        assert!(codec.decode(&[0xa5; 16], MsgType::Spat).is_err());
        let err = codec
            .encode(&Message::HandEncoded {
                ty: MsgType::Bsm,
                bytes: spat_bytes(),
            })
            .expect_err("not ours");
        assert!(matches!(err, CodecError::Unsupported { .. }), "{err}");
    }

    #[test]
    fn a_codec_is_usable_as_a_trait_object() {
        let codec: Box<dyn MessageCodec> = Box::new(J2735InfraCodec::new());
        assert_eq!(codec.message_types(), &[MsgType::Spat, MsgType::Map]);
    }
}
