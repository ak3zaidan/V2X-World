//! The WSA codec against `asn1tools` (WP-19 P19.1).
//!
//! `tests/fixtures/wsa_asn1tools.txt` holds the UPER octets asn1tools 0.169 writes for
//! five WSAs, compiling the cached IEEE 1609.3-2016 modules
//! (`third_party/asn1/ieee1609.3/`) with the encoding rule they state (UPER). Each value
//! is built independently in `tests/oracle/wsa_oracle.py` and again here. Two directions:
//!
//! | Direction | Catches |
//! |---|---|
//! | Rust encodes, octets compared with asn1tools' | a misread constraint, a wrong width, a field out of place |
//! | Rust decodes asn1tools' octets, value compared | a decoder wrong the same way as the encoder |
//!
//! The crate's own round trips cannot catch either: an encoder and a decoder that share an
//! assumption round-trip perfectly. To have asn1tools decode octets the Rust codec wrote,
//! print them with `--nocapture` into a file and run the script's `check` mode on it.

use v2xw_msg::wsa::{
    ChannelInfo, ChannelInfoExt, EdcaRecord, HeaderExt, Location, PSID_IPV6_ROUTING, RoutingExt,
    ServiceInfo, ServiceInfoExt, Wsa, decode_wsa, encode_wsa, scms_service_wsa,
};

const VECTORS: &str = include_str!("fixtures/wsa_asn1tools.txt");

fn edca(aci: u8, aifsn: u8, ecw_max: u8, ecw_min: u8) -> EdcaRecord {
    EdcaRecord {
        aci,
        aifsn,
        ecw_max,
        ecw_min,
        ..Default::default()
    }
}

fn scms() -> Wsa {
    scms_service_wsa(
        50,
        Location {
            lat: 407_527_000,
            lon: -739_868_000,
        },
        120,
        174,
        20,
    )
}

fn full() -> Wsa {
    let mut w = scms();
    w.sa_id = 3;
    w.content_count = 9;
    w.header
        .as_mut()
        .expect("header")
        .push(HeaderExt::AdvertiserId("RSU-42".into()));
    w.services.as_mut().expect("services")[0].extensions = Some(vec![
        ServiceInfoExt::ProviderServiceContext(b"scms".to_vec()),
        ServiceInfoExt::Ipv6Address([0x20; 16]),
        ServiceInfoExt::ServicePort(443),
        ServiceInfoExt::ProviderMac([1, 2, 3, 4, 5, 6]),
        ServiceInfoExt::RcpiThreshold(110),
        ServiceInfoExt::WsaCountThreshold(2),
        ServiceInfoExt::WsaCountThresholdInterval(10),
    ]);
    w.channels.as_mut().expect("channels")[0].extensions = Some(vec![
        ChannelInfoExt::Edca([
            edca(0, 6, 10, 4),
            edca(1, 9, 10, 4),
            edca(2, 3, 4, 3),
            edca(3, 2, 3, 2),
        ]),
        ChannelInfoExt::ChannelAccess(1),
    ]);
    w.routing.as_mut().expect("routing").extensions = vec![
        RoutingExt::SecondaryDns([0x20; 16]),
        RoutingExt::GatewayMac([6, 5, 4, 3, 2, 1]),
        RoutingExt::Other(99, vec![0xab, 0xcd]),
    ];
    w
}

fn psids() -> Wsa {
    Wsa {
        sa_id: 15,
        content_count: 15,
        services: Some(
            [0x20, 0x7F, 0x80, 0x4080, 0x8002, PSID_IPV6_ROUTING]
                .into_iter()
                .enumerate()
                .map(|(i, psid)| ServiceInfo {
                    psid,
                    channel_index: i as u8,
                    extensions: None,
                })
                .collect(),
        ),
        ..Wsa::default()
    }
}

fn edges() -> Wsa {
    Wsa {
        sa_id: 1,
        content_count: 2,
        header: Some(vec![HeaderExt::Location2d(Location {
            lat: -900_000_000,
            lon: 1_800_000_001,
        })]),
        channels: Some(vec![ChannelInfo {
            operating_class: 255,
            channel: 184,
            power_dbm: -128,
            adaptable: true,
            data_rate: 127,
            extensions: None,
        }]),
        ..Wsa::default()
    }
}

fn expected(name: &str) -> Wsa {
    match name {
        "minimal" => Wsa::default(),
        "scms-service" => scms(),
        "full" => full(),
        "psids" => psids(),
        "edges" => edges(),
        other => panic!("the fixture holds a vector this test does not build: {other}"),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).expect("hex"))
        .collect()
}

#[test]
fn the_wsa_codec_writes_and_reads_what_asn1tools_does() {
    let mut seen = 0;
    for line in VECTORS
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let (name, oracle) = line.split_once(' ').expect("name hex");
        let value = expected(name);
        let ours = encode_wsa(&value).expect("encodes");
        println!("{name} {}", hex(&ours));
        assert_eq!(
            hex(&ours),
            oracle,
            "{name}: Rust's octets differ from asn1tools'"
        );
        assert_eq!(
            decode_wsa(&unhex(oracle)).expect("decodes asn1tools' octets"),
            value,
            "{name}"
        );
        seen += 1;
    }
    assert_eq!(seen, 5, "every vector of the fixture was checked");
}

/// The RSU's SCMS-service WSA is the size the RSU signs and sends
/// (`RsuConfig::wsa_payload_bytes`): 84 octets, whatever the location and power.
#[test]
fn the_scms_service_wsa_is_84_octets() {
    assert_eq!(v2xw_msg::wsa::scms_service_wsa_bytes(), 84);
    assert_eq!(encode_wsa(&scms()).expect("encodes").len(), 84);
}
