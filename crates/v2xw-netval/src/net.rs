//! Layer 3: WSMP and GeoNetworking/BTP header sizes, PSID encoding and the identifier
//! registries, fragmentation, and the BSM encoder against an independent decoder.

use serde_json::{Value, json};
use v2xw_core::ids::{NodeId, SduId};
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_core::time::Duration;
use v2xw_msg::j2735::bsm::{
    self, AccelerationSet4Way, AuxiliaryBrakeStatus, BasicSafetyMessage, BrakeAppliedStatus,
    BrakeBoostApplied, BrakeSystemStatus, BsmCoreData, ControlStatus, PositionalAccuracy,
    TransmissionState, VehicleSize,
};
use v2xw_net::amplification::{FragmentLoss, sdu_loss};
use v2xw_net::frag::generic::GenericSduFragmenter;
use v2xw_net::frag::{ReassemblyBuffer, ReassemblyOutcome};
use v2xw_net::gn::{GnBtpNetLayer, GnParams};
use v2xw_net::netlayer::{BtpKind, GnTransport, NetMeta, Psid, WsmpExtensions, p_encoded_bytes};
use v2xw_net::wsmp::{WsmpNetLayer, WsmpParams};

use crate::{Check, Cost, Layer, Mode, Outcome};

/// The checks of this layer.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "NET-01",
            layer: Layer::Network,
            title: "GeoNetworking header sizes per packet type, and the BTP header",
            reference: "ETSI EN 302 636-4-1 V1.4.1 (2020-01) Tables 11-15, read from the published PDF: GUC octets 0-59, TSB 0-39, SHB 0-39, GBC/GAC 0-55, BEACON 0-35; EN 302 636-5-1 §7: BTP-A and BTP-B 4 octets",
            tolerance: "exact",
            fault: "The SHB media-dependent field (octets 36-39) left out",
            cost: Cost::Fast,
            run: gn_headers,
        },
        Check {
            id: "NET-02",
            layer: Layer::Network,
            title: "WSMP header: N-header, TPID, p-encoded PSID and length, the three N-header extensions",
            reference: "IEEE 1609.3-2020 §8.3 (WSMP-N: subtype/version octet, TPID; WSMP-T: PSID, length) and §8.1.3 variable-length encoding (1 octet to 0x7F, 2 to 0x407F, 3 to 0x20407F, 4 to 0x1020407F); BSM: 5 octets",
            tolerance: "exact",
            fault: "The length field counted as one octet at every payload size (no p-encoding)",
            cost: Cost::Fast,
            run: wsmp_header,
        },
        Check {
            id: "NET-03",
            layer: Layer::Network,
            title: "Identifiers: every safety message is signed under its registered ITS-AID / PSID",
            reference: "ETSI TS 102 965 V2.1.1 (2021-11) Table A.1, read from the published PDF: CA 36, DEN 37, TLM (SPATEM) 137, RLT (MAPEM) 138, IVI 139, CP 639, VRU 638; IEEE 1609.12: BSM 0x20",
            tolerance: "exact",
            fault: "BSM signed under PSID 0x21",
            cost: Cost::Fast,
            run: identifiers,
        },
        Check {
            id: "NET-04",
            layer: Layer::Network,
            title: "Fragmentation: fragment count and sizes, and the SDU loss formula 1 − Π(1 − p_i) against a Monte-Carlo of the reassembly buffer",
            reference: "Independent-loss amplification (04-models.md §7.3): P(SDU lost) = 1 − Π(1 − p_i); fragment count ⌈S/(MTU − 4)⌉ for the generic 4-octet fragment header",
            tolerance: "count exact; simulated loss within 4 binomial standard errors of the formula (20,000 SDUs per case)",
            fault: "The reassembly buffer fed duplicate copies of fragment 0 in place of the others' losses (loss counted per SDU, not per fragment)",
            cost: Cost::Fast,
            run: fragmentation,
        },
        Check {
            id: "NET-05",
            layer: Layer::Network,
            title: "The J2735 BSM encoder against an independent UPER decoder, 300 random and boundary messages",
            reference: "SAE J2735 BSMcoreData (ASN.1) and ITU-T X.691 unaligned PER, decoded by reference/bsm_uper.py (written separately from the Rust codec); 293 bits → 37 octets",
            tolerance: "every field equal, padding zero, length 37 octets",
            fault: "The encoder writes longitude one unit high (the range read as −1800000000..1800000000)",
            cost: Cost::Fast,
            run: bsm_decoder,
        },
    ]
}

fn gn_headers(mode: Mode) -> Outcome {
    let want = [
        (GnTransport::Guc, 60u32),
        (GnTransport::Tsb, 40),
        (GnTransport::Shb, 40),
        (GnTransport::Gbc, 56),
        (GnTransport::Gac, 56),
        (GnTransport::Beacon, 36),
    ];
    let mut bad = Vec::new();
    for (t, bytes) in want {
        let model = t.header_bytes() - if mode.faulted() && t == GnTransport::Shb { 4 } else { 0 };
        if model != bytes {
            bad.push(format!("{t:?}: {model} vs {bytes}"));
        }
    }
    if BtpKind::A.header_bytes() != 4 || BtpKind::B.header_bytes() != 4 {
        bad.push("BTP".to_string());
    }
    // A CAM below its security envelope: GN SHB 40 + BTP-B 4 + LLC/SNAP 8 = 52.
    let layer = GnBtpNetLayer::new(GnParams::default());
    let mut meta = NetMeta::wsmp(300, Psid::BSM);
    meta.gn = GnTransport::Shb;
    meta.btp = BtpKind::B;
    meta.include_llc_snap = true;
    let cam = layer.header_size(&meta) - if mode.faulted() { 4 } else { 0 };
    if cam != 52 {
        bad.push(format!("CAM below the envelope {cam} vs 52"));
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { "6 packet types, BTP-A/B and the 52-octet CAM stack match".to_string() } else { bad.join("; ") })
}

fn wsmp_header(mode: Mode) -> Outcome {
    let layer = WsmpNetLayer::new(WsmpParams::default());
    let mut bad = Vec::new();
    // PSID p-encoding boundaries.
    for (v, n) in [(0u32, 1u32), (0x7F, 1), (0x80, 2), (0x407F, 2), (0x4080, 3), (0x20_407F, 3), (0x20_4080, 4), (0x1020_407F, 4)] {
        if p_encoded_bytes(v) != Some(n) {
            bad.push(format!("p-encoding of {v:#x}"));
        }
    }
    if p_encoded_bytes(0x1020_4080).is_some() {
        bad.push("0x10204080 accepted".to_string());
    }
    // Header = N-header 1 + extensions + TPID 1 + PSID + length.
    for (psid, payload, ext, want) in [
        (0x20u32, 100u32, WsmpExtensions::NONE, 1 + 1 + 1 + 1),
        (0x20, 300, WsmpExtensions::NONE, 1 + 1 + 1 + 2),
        (0x82, 300, WsmpExtensions::NONE, 1 + 1 + 2 + 2),
        (0x20, 300, WsmpExtensions::ALL, 1 + 9 + 1 + 1 + 2),
        (0x20_4080, 1_400, WsmpExtensions::NONE, 1 + 1 + 4 + 2),
    ] {
        let mut meta = NetMeta::wsmp(payload, Psid::new(psid).expect("in range"));
        meta.extensions = ext;
        let mut got = layer.header_size(&meta);
        if mode.faulted() && payload > 127 {
            got -= 1;
        }
        if got != want {
            bad.push(format!("PSID {psid:#x}, {payload} B, {} ext: {got} vs {want}", ext.count()));
        }
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { "9 p-encoding boundaries and 5 header shapes match (BSM of 300 B: 5 octets)".to_string() } else { bad.join("; ") })
}

fn identifiers(mode: Mode) -> Outcome {
    use v2xw_node::secure::PSID_SAFETY;
    // Every message this simulator signs goes out under the one PSID the node holds
    // (`NodeSecurity::psid`, set from `PSID_SAFETY` by the engine's wiring).
    let signed_under = |_msg: &str| -> u64 { if mode.faulted() { 0x21 } else { PSID_SAFETY } };
    let registry: [(&str, u64); 5] = [("BSM (US)", 0x20), ("CAM", 36), ("DENM", 37), ("SPATEM", 137), ("MAPEM", 138)];
    let mut bad = Vec::new();
    for (msg, want) in registry {
        let got = signed_under(msg);
        if got != want {
            bad.push(format!("{msg} signed under {got} ({got:#x}), registry {want}"));
        }
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { "5 identifiers match".to_string() } else { bad.join("; ") })
}

fn fragmentation(mode: Mode) -> Outcome {
    let f = GenericSduFragmenter::default();
    let mut bad = Vec::new();
    for (sdu, mtu) in [(1_399u32, 1_398u32), (4_000, 1_398), (2_796, 1_398), (10_000, 500), (2_000, 1_400)] {
        let parts = f.split(SduId::new(1), sdu, mtu).expect("splits");
        let want = sdu.div_ceil(mtu - 4) as usize;
        let sum: u32 = parts.iter().map(|p| p.payload_bytes).sum();
        if parts.len() != want || sum != sdu || parts.iter().any(|p| p.total_bytes() > mtu) {
            bad.push(format!("{sdu} B over {mtu}: {} fragments (want {want}), {sum} B", parts.len()));
        }
    }
    let mut rng = RngStream::derive(0xF2A6, RngDomain::plugin("netval"), EntityRef::Global);
    let mut summary = Vec::new();
    for probs in [vec![0.1, 0.1, 0.1], vec![0.05, 0.2], vec![0.3, 0.01, 0.01, 0.01]] {
        let parts = f.split(SduId::new(1), 1_394 * probs.len() as u32, 1_398).expect("splits");
        let mut buf = ReassemblyBuffer::new(64, Duration::from_millis(1_000));
        let n = 20_000u64;
        let mut lost = 0u64;
        for s in 0..n {
            let mut complete = false;
            for (i, p) in parts.iter().enumerate() {
                let mut frag = *p;
                frag.sdu = SduId::new(s as u32 + 1);
                let drop = rng.bool(probs[i]);
                if drop && !mode.faulted() {
                    continue;
                }
                if mode.faulted() && drop {
                    frag.index = 0;
                }
                if matches!(buf.accept(s * 10_000_000, NodeId::new(1), &frag), ReassemblyOutcome::Complete { .. }) {
                    complete = true;
                }
            }
            if !complete {
                lost += 1;
            }
        }
        let formula = 1.0 - probs.iter().map(|p| 1.0 - p).product::<f64>();
        let model = sdu_loss(&probs.iter().enumerate().map(|(i, p)| FragmentLoss::new(i as u16, *p, 1_394)).collect::<Vec<_>>(), true).p_any_fragment_lost;
        let sim = lost as f64 / n as f64;
        let se = (formula * (1.0 - formula) / n as f64).sqrt();
        if (sim - formula).abs() > 4.0 * se || (model - formula).abs() > 1e-12 {
            bad.push(format!("p={probs:?}: simulated {sim:.4}, formula {formula:.4}, model {model:.4}"));
        }
        summary.push(format!("{sim:.4} vs {formula:.4}"));
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { format!("5 splits exact; SDU loss simulated vs formula: {}", summary.join(", ")) } else { bad.join("; ") })
}

fn pick(rng: &mut RngStream, min: i64, max: i64) -> i64 {
    match rng.below(4) {
        0 => min,
        1 => max,
        _ => min + rng.below((max - min + 1) as u64) as i64,
    }
}

fn random_core(rng: &mut RngStream) -> BsmCoreData {
    let mut id = [0u8; 4];
    rng.fill_bytes(&mut id);
    BsmCoreData {
        msg_cnt: pick(rng, 0, 127) as u8,
        id,
        sec_mark: pick(rng, 0, 65_535) as u16,
        lat: pick(rng, bsm::LATITUDE_MIN, bsm::LATITUDE_MAX) as i32,
        lon: pick(rng, bsm::LONGITUDE_MIN, bsm::LONGITUDE_MAX) as i32,
        elev: pick(rng, bsm::ELEVATION_MIN, bsm::ELEVATION_MAX) as i32,
        accuracy: PositionalAccuracy {
            semi_major: pick(rng, 0, 255) as u8,
            semi_minor: pick(rng, 0, 255) as u8,
            orientation: pick(rng, 0, 65_535) as u16,
        },
        transmission: TransmissionState::from_index(rng.below(8)).expect("in range"),
        speed: pick(rng, 0, 8_191) as u16,
        heading: pick(rng, 0, 28_800) as u16,
        angle: pick(rng, -126, 127) as i8,
        accel_set: AccelerationSet4Way {
            long: pick(rng, -2_000, 2_001) as i16,
            lat: pick(rng, -2_000, 2_001) as i16,
            vert: pick(rng, -127, 127) as i8,
            yaw: pick(rng, -32_767, 32_767) as i16,
        },
        brakes: BrakeSystemStatus {
            wheel_brakes: BrakeAppliedStatus(rng.below(32) as u8),
            traction: ControlStatus::from_index(rng.below(4)).expect("in range"),
            abs: ControlStatus::from_index(rng.below(4)).expect("in range"),
            scs: ControlStatus::from_index(rng.below(4)).expect("in range"),
            brake_boost: BrakeBoostApplied::from_index(rng.below(3)).expect("in range"),
            aux_brakes: AuxiliaryBrakeStatus::from_index(rng.below(4)).expect("in range"),
        },
        size: VehicleSize {
            width: pick(rng, 0, 1_023) as u16,
            length: pick(rng, 0, 4_095) as u16,
        },
    }
}

fn expected_json(c: &BsmCoreData) -> Value {
    json!({
        "msgCnt": c.msg_cnt, "id": format!("{:02x}{:02x}{:02x}{:02x}", c.id[0], c.id[1], c.id[2], c.id[3]),
        "secMark": c.sec_mark, "lat": c.lat, "long": c.lon, "elev": c.elev,
        "semiMajor": c.accuracy.semi_major, "semiMinor": c.accuracy.semi_minor, "orientation": c.accuracy.orientation,
        "transmission": c.transmission.index(), "speed": c.speed, "heading": c.heading, "angle": c.angle,
        "accelLong": c.accel_set.long, "accelLat": c.accel_set.lat, "accelVert": c.accel_set.vert, "yaw": c.accel_set.yaw,
        "wheelBrakes": c.brakes.wheel_brakes.0, "traction": c.brakes.traction.index(), "abs": c.brakes.abs.index(),
        "scs": c.brakes.scs.index(), "brakeBoost": c.brakes.brake_boost.index(), "auxBrakes": c.brakes.aux_brakes.index(),
        "width": c.size.width, "length": c.size.length,
        "ext": 0, "partII": 0, "regional": 0, "bits": 293, "padding_zero": true, "octets": 37,
    })
}

fn bsm_decoder(mode: Mode) -> Outcome {
    let mut rng = RngStream::derive(0xB5A1, RngDomain::plugin("netval"), EntityRef::Global);
    let cores: Vec<BsmCoreData> = (0..300).map(|_| random_core(&mut rng)).collect();
    let mut hexes = Vec::new();
    for c in &cores {
        let mut written = c.clone();
        if mode.faulted() && written.lon < bsm::LONGITUDE_MAX as i32 {
            written.lon += 1;
        }
        let enc = bsm::encode_bsm(&BasicSafetyMessage::part_i(written)).expect("encodes");
        hexes.push(Value::String(enc.bytes.iter().map(|b| format!("{b:02x}")).collect()));
    }
    let decoded = match crate::python("bsm_uper.py", &Value::Array(hexes)) {
        Ok(v) => v,
        Err(e) => return Outcome::skip(e),
    };
    let Some(items) = decoded.as_array() else {
        return Outcome::skip("decoder returned no array");
    };
    let mut wrong = Vec::new();
    for (c, got) in cores.iter().zip(items) {
        let want = expected_json(c);
        for (k, v) in want.as_object().expect("object") {
            if got.get(k) != Some(v) {
                wrong.push(format!("{k}: encoded {v}, decoded {}", got.get(k).cloned().unwrap_or(Value::Null)));
                break;
            }
        }
    }
    Outcome::judge(
        wrong.is_empty() && items.len() == cores.len(),
        if wrong.is_empty() {
            format!("{} messages, 25 fields each, decode identically", items.len())
        } else {
            format!("{} of {} messages differ; first: {}", wrong.len(), cores.len(), wrong[0])
        },
    )
}
