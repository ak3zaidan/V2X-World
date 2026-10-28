//! The IEEE 1609.3 WAVE Service Advertisement (WSA), really encoded.
//!
//! Hand-written UPER over [`crate::j2735::uper`], from the modules cached in
//! `third_party/asn1/ieee1609.3/` (`wsa.asn`, `wee.asn` and ISO 17419
//! `CITSapplMgmtIDs`, TCI_ASN1 `63d82d87976b`; see its `PROVENANCE.md`). The modules are
//! the IEEE 1609.3-2016 edition (WSA protocol version 3), the one the USDOT RSU
//! Specification v4.1 (2017) requires (Req_586), and they state their own encoding rule:
//! every module is `AUTOMATIC TAGS` and says its "mapping of UPER … was validated with the
//! OSS ASN.1 Studio tool". So the WSA is **UPER** (ITU-T X.691 unaligned), not COER.
//!
//! `rasn-compiler` is not used: the WEE module's `Extension {EXT-TYPE : ExtensionTypes}`
//! is a parameterised type over an information object class, the idiom the build script
//! records it cannot compile (SAE J2735's `RegionalExtension`, build decision D5). Each
//! extension's value is an open type (X.691 clause 11.2), and this codec writes the inner
//! value of every extension the module's object sets list; an extension id outside them
//! is kept as its octets ([`HeaderExt::Other`] and its siblings), because every object set
//! is extensible (`...`).
//!
//! The bytes are checked against `asn1tools` 0.169 compiling the same modules
//! (`tests/wsa_oracle.rs`, vectors from `tests/oracle/wsa_oracle.py`).
//!
//! What this module does not do: the WSMP header and the IEEE 1609.2 signature around
//! the WSA (1609.3-2016 Annex H.1, RSU Spec Req_586) belong to the network and security
//! layers.

use crate::j2735::uper::{
    BitReader, BitWriter, Field, UperError, read_choice_index, read_constrained_int,
    read_constrained_length, read_fixed_bit_string, read_fixed_octet_string, read_length,
    read_open_type, read_preamble, write_choice_index, write_constrained_int,
    write_constrained_length, write_fixed_octet_string, write_length, write_open_type,
    write_preamble,
};

/// The WSA protocol version these modules define (`c-rsvAdvPrtVersionNo ::= 3`).
pub const WSA_VERSION: u8 = 3;

/// "WAVE security management", the SCMS's PSID: IEEE RA PSID registry, and IEEE
/// 1609.2.1 `SecurityMgmtPsid ::= Psid (35)`. That a US RSU advertises the SCMS service
/// under exactly this PSID is an ASSUMPTION: FHWA-JPO-20-804 (RSU lessons learned) p. 4
/// says the service is WSA-advertised and names no PSID.
pub const PSID_SECURITY_MANAGEMENT: u32 = 0x23;

/// "IPv6 Routing", p-encoded `0xEF-FF-FF-FE` in the IEEE RA registry: the service an RSU
/// lists beside its WAVE Routing Advertisement (USDOT RSU Spec v4.1 App. D).
pub const PSID_IPV6_ROUTING: u32 = 0x1020_407E;

// The `RefExt` values of `wee.asn`.
const EXT_2D_LOCATION: u8 = 5;
const EXT_3D_LOCATION: u8 = 6;
const EXT_ADVERTISER_ID: u8 = 7;
const EXT_PROVIDER_SERV_CONTEXT: u8 = 8;
const EXT_IPV6_ADDRESS: u8 = 9;
const EXT_SERVICE_PORT: u8 = 10;
const EXT_PROVIDER_MAC: u8 = 11;
const EXT_EDCA_PARAMETER_SET: u8 = 12;
const EXT_SECONDARY_DNS: u8 = 13;
const EXT_GATEWAY_MAC: u8 = 14;
const EXT_REPEAT_RATE: u8 = 17;
const EXT_RCPI_THRESHOLD: u8 = 19;
const EXT_WSA_COUNT_THRESHOLD: u8 = 20;
const EXT_CHANNEL_ACCESS: u8 = 21;
const EXT_WSA_COUNT_THRES_INT: u8 = 22;

/// A WAVE Service Advertisement (`SrvAdvMsg`, `messageID` `saMessage`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Wsa {
    /// `SrvAdvID`, 0..=15: which advertisement this is, of the provider's.
    pub sa_id: u8,
    /// `SrvAdvContentCount`, 0..=15: changes when the content changes.
    pub content_count: u8,
    /// `SrvAdvMsgHeaderExts`.
    pub header: Option<Vec<HeaderExt>>,
    /// `ServiceInfos`.
    pub services: Option<Vec<ServiceInfo>>,
    /// `ChannelInfos`.
    pub channels: Option<Vec<ChannelInfo>>,
    /// `RoutingAdvertisement`: the WAVE Routing Advertisement (WRA) for IP services.
    pub routing: Option<RoutingAdvertisement>,
}

/// A WSA header extension (`SrvAdvMsgHeaderExtTypes`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderExt {
    /// `RepeatRate`: WSAs per 5 s (USDOT RSU Spec v4.1 App. D: 50, i.e. 10 Hz).
    RepeatRate(u8),
    /// `TwoDLocation`, in 0.1 µdeg.
    Location2d(Location),
    /// `ThreeDLocation`, in 0.1 µdeg and 0.1 m.
    Location3d(Location, i32),
    /// `AdvertiserIdentifier`, 1..=32 octets of UTF-8.
    AdvertiserId(String),
    /// An extension id the object set does not list, kept as its open-type octets.
    Other(u8, Vec<u8>),
}

/// A position, as `wee.asn` defines it: `Latitude` (a fill bit and 0.1 µdeg in
/// -900000000..=900000001) and `Longitude` (0.1 µdeg in -1799999999..=1800000001).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    /// 0.1 µdeg; 900000001 is "unavailable".
    pub lat: i32,
    /// 0.1 µdeg; 1800000001 is "unavailable".
    pub lon: i32,
}

/// One advertised service (`ServiceInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceInfo {
    /// The PSID (`VarLengthNumber`, the p-encoded PSID).
    pub psid: u32,
    /// `ChannelIndex`, 0..=31: 1-based into [`Wsa::channels`], 0 when none is used.
    pub channel_index: u8,
    /// `ChannelOptions.extensions`. (`mandApp` and `serviceProviderPort` are "in WSA set
    /// to '0'b": never present here.)
    pub extensions: Option<Vec<ServiceInfoExt>>,
}

/// A service-info extension (`ServiceInfoExtTypes`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceInfoExt {
    /// `ProviderServiceContext`: 0..=31 octets (the fill bits are written zero).
    ProviderServiceContext(Vec<u8>),
    /// `IPv6Address` of the service.
    Ipv6Address([u8; 16]),
    /// `ServicePort`.
    ServicePort(u16),
    /// `ProviderMacAddress`.
    ProviderMac([u8; 6]),
    /// `RcpiThreshold` (IEEE 802.11 RCPI units).
    RcpiThreshold(u8),
    /// `WsaCountThreshold`.
    WsaCountThreshold(u8),
    /// `WsaCountThresholdInterval` (100 ms units; 0 is not used).
    WsaCountThresholdInterval(u8),
    /// An extension id the object set does not list.
    Other(u8, Vec<u8>),
}

/// One channel the services are offered on (`ChannelInfo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChannelInfo {
    /// `OperatingClass80211` (17 is the US 5.9 GHz 10 MHz class).
    pub operating_class: u8,
    /// `ChannelNumber80211`.
    pub channel: u8,
    /// `TXpower80211`, dBm.
    pub power_dbm: i8,
    /// `WsaChInfoDataRate.adaptable`.
    pub adaptable: bool,
    /// `WsaChInfoDataRate.dataRate`, 0..=127, in 500 kb/s (12 = 6 Mb/s).
    pub data_rate: u8,
    /// `ChInfoOptions.extensions` (`option1`..`option7` are "not used": never present).
    pub extensions: Option<Vec<ChannelInfoExt>>,
}

/// A channel-info extension (`ChannelInfoExtTypes`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelInfoExt {
    /// `EdcaParameterSet`: AC_BE, AC_BK, AC_VI, AC_VO.
    Edca([EdcaRecord; 4]),
    /// `ChannelAccess80211`: 0 continuous, 1 alternating SCH, 2 alternating CCH.
    ChannelAccess(u8),
    /// An extension id the object set does not list.
    Other(u8, Vec<u8>),
}

/// One `EdcaParameterRecord`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EdcaRecord {
    /// `res`, 0..=1.
    pub res: u8,
    /// `aci`, 0..=3.
    pub aci: u8,
    /// `acm`, 0..=1.
    pub acm: u8,
    /// `aifsn`, 0..=15.
    pub aifsn: u8,
    /// `ecwMax`, 0..=15 (CWmax = 2^ecwMax − 1).
    pub ecw_max: u8,
    /// `ecwMin`, 0..=15.
    pub ecw_min: u8,
    /// `txopLimit`.
    pub txop_limit: u16,
}

/// The WAVE Routing Advertisement (`RoutingAdvertisement`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutingAdvertisement {
    /// `RouterLifetime`, s.
    pub lifetime_s: u16,
    /// `IpV6Prefix`.
    pub prefix: [u8; 16],
    /// `IpV6PrefixLength`.
    pub prefix_len: u8,
    /// `defaultGateway`.
    pub default_gateway: [u8; 16],
    /// `primaryDns`.
    pub primary_dns: [u8; 16],
    /// `RoutAdvertExts` (mandatory, possibly empty).
    pub extensions: Vec<RoutingExt>,
}

/// A routing-advertisement extension (`RoutAdvertExtTypes`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingExt {
    /// `SecondaryDns`.
    SecondaryDns([u8; 16]),
    /// `GatewayMacAddress`.
    GatewayMac([u8; 6]),
    /// An extension id the object set does not list.
    Other(u8, Vec<u8>),
}

const fn f(path: &'static str, ty: &'static str) -> Field {
    Field::new(path, ty)
}

fn int(w: &mut BitWriter, field: Field, v: i64, min: i64, max: i64) -> Result<(), UperError> {
    write_constrained_int(w, field, v, min, max)
}

fn rint(r: &mut BitReader<'_>, field: Field, min: i64, max: i64) -> Result<i64, UperError> {
    read_constrained_int(r, field, min, max)
}

/// Encodes a WSA (`SrvAdvMsg`) to UPER octets.
///
/// # Errors
/// A field outside its ASN.1 constraint, or a list too long for the length forms this
/// engine writes.
pub fn encode_wsa(wsa: &Wsa) -> Result<Vec<u8>, UperError> {
    let mut w = BitWriter::with_capacity(96);
    // SrvAdvPrtVersion: messageID saMessage (0), rsvAdvPrtVersion 3.
    int(
        &mut w,
        f("wsa.version.messageID", "SrvAdvMessageType"),
        0,
        0,
        1,
    )?;
    int(
        &mut w,
        f("wsa.version.rsvAdvPrtVersion", "RsvAdvPrtVersion"),
        i64::from(WSA_VERSION),
        0,
        7,
    )?;
    // SrvAdvBody: four OPTIONAL fields, not extensible.
    write_preamble(
        &mut w,
        false,
        &[
            wsa.header.is_some(),
            wsa.services.is_some(),
            wsa.channels.is_some(),
            wsa.routing.is_some(),
        ],
    );
    int(
        &mut w,
        f("wsa.changeCount.saID", "SrvAdvID"),
        i64::from(wsa.sa_id),
        0,
        15,
    )?;
    int(
        &mut w,
        f("wsa.changeCount.contentCount", "SrvAdvContentCount"),
        i64::from(wsa.content_count),
        0,
        15,
    )?;
    if let Some(exts) = &wsa.header {
        write_length(&mut w, "SrvAdvMsgHeaderExts", exts.len())?;
        for e in exts {
            let (id, inner) = header_ext(e)?;
            write_extension(&mut w, "SrvAdvMsgHeaderExt", id, &inner)?;
        }
    }
    if let Some(services) = &wsa.services {
        write_length(&mut w, "ServiceInfos", services.len())?;
        for s in services {
            write_psid(&mut w, s.psid)?;
            int(
                &mut w,
                f("wsa.serviceInfo.channelIndex", "ChannelIndex"),
                i64::from(s.channel_index),
                0,
                31,
            )?;
            // ChannelOptions: mandApp, serviceProviderPort (never), extensions.
            write_preamble(&mut w, false, &[false, false, s.extensions.is_some()]);
            if let Some(exts) = &s.extensions {
                write_length(&mut w, "ServiceInfoExts", exts.len())?;
                for e in exts {
                    let (id, inner) = service_ext(e)?;
                    write_extension(&mut w, "ServiceInfoExt", id, &inner)?;
                }
            }
        }
    }
    if let Some(channels) = &wsa.channels {
        write_length(&mut w, "ChannelInfos", channels.len())?;
        for c in channels {
            int(
                &mut w,
                f("wsa.channelInfo.operatingClass", "OperatingClass80211"),
                i64::from(c.operating_class),
                0,
                255,
            )?;
            int(
                &mut w,
                f("wsa.channelInfo.channelNumber", "ChannelNumber80211"),
                i64::from(c.channel),
                0,
                255,
            )?;
            int(
                &mut w,
                f("wsa.channelInfo.powerLevel", "TXpower80211"),
                i64::from(c.power_dbm),
                -128,
                127,
            )?;
            // adaptable: BIT STRING (SIZE(1)).
            w.write_bits(u64::from(c.adaptable), 1);
            int(
                &mut w,
                f("wsa.channelInfo.dataRate", "WsaChInfoDataRate"),
                i64::from(c.data_rate),
                0,
                127,
            )?;
            // ChInfoOptions: option1..option7 (never), extensions.
            let mut present = [false; 8];
            present[7] = c.extensions.is_some();
            write_preamble(&mut w, false, &present);
            if let Some(exts) = &c.extensions {
                write_length(&mut w, "ChannelInfoExts", exts.len())?;
                for e in exts {
                    let (id, inner) = channel_ext(e)?;
                    write_extension(&mut w, "ChannelInfoExt", id, &inner)?;
                }
            }
        }
    }
    if let Some(ra) = &wsa.routing {
        int(
            &mut w,
            f("wsa.routing.lifetime", "RouterLifetime"),
            i64::from(ra.lifetime_s),
            0,
            65_535,
        )?;
        write_fixed_octet_string(&mut w, &ra.prefix);
        int(
            &mut w,
            f("wsa.routing.ipPrefixLength", "IpV6PrefixLength"),
            i64::from(ra.prefix_len),
            0,
            255,
        )?;
        write_fixed_octet_string(&mut w, &ra.default_gateway);
        write_fixed_octet_string(&mut w, &ra.primary_dns);
        write_length(&mut w, "RoutAdvertExts", ra.extensions.len())?;
        for e in &ra.extensions {
            let (id, inner) = routing_ext(e);
            write_extension(&mut w, "RoutAdvertExt", id, &inner)?;
        }
    }
    Ok(w.into_bytes())
}

/// Decodes UPER octets as a WSA (`SrvAdvMsg`).
///
/// # Errors
/// The bytes are not the UPER encoding of a version-3 WSA: truncated, out of range,
/// trailing data, or a `messageID` other than `saMessage`.
pub fn decode_wsa(bytes: &[u8]) -> Result<Wsa, UperError> {
    let mut r = BitReader::new(bytes);
    let message_id = rint(
        &mut r,
        f("wsa.version.messageID", "SrvAdvMessageType"),
        0,
        1,
    )?;
    if message_id != 0 {
        return Err(UperError::Malformed {
            construct: "SrvAdvPrtVersion.messageID",
            detail: "1 is the ISO 24102-5 SAR message; a WSA is saMessage (0)",
        });
    }
    let version = rint(
        &mut r,
        f("wsa.version.rsvAdvPrtVersion", "RsvAdvPrtVersion"),
        0,
        7,
    )?;
    if version != i64::from(WSA_VERSION) {
        return Err(UperError::Unsupported {
            construct: "SrvAdvPrtVersion.rsvAdvPrtVersion",
            detail: "these modules define WSA protocol version 3 (IEEE 1609.3-2016)",
        });
    }
    let p = read_preamble(&mut r, "SrvAdvBody", false, 4)?;
    let sa_id = rint(&mut r, f("wsa.changeCount.saID", "SrvAdvID"), 0, 15)? as u8;
    let content_count = rint(
        &mut r,
        f("wsa.changeCount.contentCount", "SrvAdvContentCount"),
        0,
        15,
    )? as u8;
    let header = if p.has(0) {
        let n = read_length(&mut r, "SrvAdvMsgHeaderExts")?;
        let mut v = Vec::with_capacity(n.min(16));
        for _ in 0..n {
            let (id, inner) = read_extension(&mut r, "SrvAdvMsgHeaderExt")?;
            v.push(parse_header_ext(id, inner)?);
        }
        Some(v)
    } else {
        None
    };
    let services = if p.has(1) {
        let n = read_length(&mut r, "ServiceInfos")?;
        let mut v = Vec::with_capacity(n.min(32));
        for _ in 0..n {
            let psid = read_psid(&mut r)?;
            let channel_index = rint(
                &mut r,
                f("wsa.serviceInfo.channelIndex", "ChannelIndex"),
                0,
                31,
            )? as u8;
            let o = read_preamble(&mut r, "ChannelOptions", false, 3)?;
            // mandApp and serviceProviderPort are NULL: present or not, they hold no bits.
            let extensions = if o.has(2) {
                let n = read_length(&mut r, "ServiceInfoExts")?;
                let mut e = Vec::with_capacity(n.min(16));
                for _ in 0..n {
                    let (id, inner) = read_extension(&mut r, "ServiceInfoExt")?;
                    e.push(parse_service_ext(id, inner)?);
                }
                Some(e)
            } else {
                None
            };
            v.push(ServiceInfo {
                psid,
                channel_index,
                extensions,
            });
        }
        Some(v)
    } else {
        None
    };
    let channels = if p.has(2) {
        let n = read_length(&mut r, "ChannelInfos")?;
        let mut v = Vec::with_capacity(n.min(32));
        for _ in 0..n {
            let operating_class = rint(
                &mut r,
                f("wsa.channelInfo.operatingClass", "OperatingClass80211"),
                0,
                255,
            )? as u8;
            let channel = rint(
                &mut r,
                f("wsa.channelInfo.channelNumber", "ChannelNumber80211"),
                0,
                255,
            )? as u8;
            let power_dbm = rint(
                &mut r,
                f("wsa.channelInfo.powerLevel", "TXpower80211"),
                -128,
                127,
            )? as i8;
            let adaptable = read_fixed_bit_string(&mut r, 1)? == 1;
            let data_rate = rint(
                &mut r,
                f("wsa.channelInfo.dataRate", "WsaChInfoDataRate"),
                0,
                127,
            )? as u8;
            let o = read_preamble(&mut r, "ChInfoOptions", false, 8)?;
            let extensions = if o.has(7) {
                let n = read_length(&mut r, "ChannelInfoExts")?;
                let mut e = Vec::with_capacity(n.min(16));
                for _ in 0..n {
                    let (id, inner) = read_extension(&mut r, "ChannelInfoExt")?;
                    e.push(parse_channel_ext(id, inner)?);
                }
                Some(e)
            } else {
                None
            };
            v.push(ChannelInfo {
                operating_class,
                channel,
                power_dbm,
                adaptable,
                data_rate,
                extensions,
            });
        }
        Some(v)
    } else {
        None
    };
    let routing = if p.has(3) {
        let lifetime_s = rint(
            &mut r,
            f("wsa.routing.lifetime", "RouterLifetime"),
            0,
            65_535,
        )? as u16;
        let prefix = octets16(&mut r)?;
        let prefix_len = rint(
            &mut r,
            f("wsa.routing.ipPrefixLength", "IpV6PrefixLength"),
            0,
            255,
        )? as u8;
        let default_gateway = octets16(&mut r)?;
        let primary_dns = octets16(&mut r)?;
        let n = read_length(&mut r, "RoutAdvertExts")?;
        let mut extensions = Vec::with_capacity(n.min(16));
        for _ in 0..n {
            let (id, inner) = read_extension(&mut r, "RoutAdvertExt")?;
            extensions.push(parse_routing_ext(id, inner)?);
        }
        Some(RoutingAdvertisement {
            lifetime_s,
            prefix,
            prefix_len,
            default_gateway,
            primary_dns,
            extensions,
        })
    } else {
        None
    };
    r.finish()?;
    Ok(Wsa {
        sa_id,
        content_count,
        header,
        services,
        channels,
        routing,
    })
}

/// `Extension`: `extensionId` (`RefExt`, 0..=255) and the value as an open type.
fn write_extension(
    w: &mut BitWriter,
    construct: &'static str,
    id: u8,
    inner: &[u8],
) -> Result<(), UperError> {
    int(
        w,
        f("wsa.extension.extensionId", "RefExt"),
        i64::from(id),
        0,
        255,
    )?;
    write_open_type(w, construct, inner)
}

fn read_extension(
    r: &mut BitReader<'_>,
    construct: &'static str,
) -> Result<(u8, Vec<u8>), UperError> {
    let id = rint(r, f("wsa.extension.extensionId", "RefExt"), 0, 255)? as u8;
    Ok((id, read_open_type(r, construct)?))
}

/// An inner value encoded as if outermost (X.691 clause 11.2.1), to its padded octets.
fn inner(
    build: impl FnOnce(&mut BitWriter) -> Result<(), UperError>,
) -> Result<Vec<u8>, UperError> {
    let mut w = BitWriter::with_capacity(16);
    build(&mut w)?;
    Ok(w.into_bytes())
}

/// Reads an open type's inner value, which must be exactly one encoding of its type.
fn parse<T>(
    bytes: &[u8],
    read: impl FnOnce(&mut BitReader<'_>) -> Result<T, UperError>,
) -> Result<T, UperError> {
    let mut r = BitReader::new(bytes);
    let v = read(&mut r)?;
    r.finish()?;
    Ok(v)
}

fn u8_inner(path: &'static str, ty: &'static str, v: u8) -> Result<Vec<u8>, UperError> {
    inner(|w| int(w, f(path, ty), i64::from(v), 0, 255))
}

fn u8_parse(bytes: &[u8], path: &'static str, ty: &'static str) -> Result<u8, UperError> {
    parse(bytes, |r| Ok(rint(r, f(path, ty), 0, 255)? as u8))
}

fn write_location(w: &mut BitWriter, loc: Location) -> Result<(), UperError> {
    // Latitude: the fill bit (MSB of the latitude, "set to '0'"), then the value.
    w.write_bits(0, 1);
    int(
        w,
        f("wsa.location.lat", "Latitude"),
        i64::from(loc.lat),
        -900_000_000,
        900_000_001,
    )?;
    int(
        w,
        f("wsa.location.lon", "Longitude"),
        i64::from(loc.lon),
        -1_799_999_999,
        1_800_000_001,
    )
}

fn read_location(r: &mut BitReader<'_>) -> Result<Location, UperError> {
    let _fill = read_fixed_bit_string(r, 1)?;
    let lat = rint(
        r,
        f("wsa.location.lat", "Latitude"),
        -900_000_000,
        900_000_001,
    )? as i32;
    let lon = rint(
        r,
        f("wsa.location.lon", "Longitude"),
        -1_799_999_999,
        1_800_000_001,
    )? as i32;
    Ok(Location { lat, lon })
}

fn header_ext(e: &HeaderExt) -> Result<(u8, Vec<u8>), UperError> {
    Ok(match e {
        HeaderExt::RepeatRate(n) => (
            EXT_REPEAT_RATE,
            u8_inner("wsa.header.repeatRate", "RepeatRate", *n)?,
        ),
        HeaderExt::Location2d(loc) => (EXT_2D_LOCATION, inner(|w| write_location(w, *loc))?),
        HeaderExt::Location3d(loc, elevation) => (
            EXT_3D_LOCATION,
            inner(|w| {
                write_location(w, *loc)?;
                int(
                    w,
                    f("wsa.header.elevation", "Elevation"),
                    i64::from(*elevation),
                    -4_096,
                    61_439,
                )
            })?,
        ),
        HeaderExt::AdvertiserId(s) => {
            let n = s.len();
            if !(1..=32).contains(&n) {
                return Err(UperError::OutOfRange {
                    field: "wsa.header.advertiserID",
                    asn1_type: "AdvertiserIdentifier",
                    value: n as i64,
                    min: 1,
                    max: 32,
                });
            }
            // UTF8String is not a known-multiplier character string, so its SIZE
            // constraint is not PER-visible (X.691 clause 30): an unconstrained length
            // in octets, then the octets.
            (
                EXT_ADVERTISER_ID,
                inner(|w| {
                    write_length(w, "AdvertiserIdentifier", n)?;
                    w.write_octets(s.as_bytes());
                    Ok(())
                })?,
            )
        }
        HeaderExt::Other(id, bytes) => (*id, bytes.clone()),
    })
}

fn parse_header_ext(id: u8, bytes: Vec<u8>) -> Result<HeaderExt, UperError> {
    Ok(match id {
        EXT_REPEAT_RATE => {
            HeaderExt::RepeatRate(u8_parse(&bytes, "wsa.header.repeatRate", "RepeatRate")?)
        }
        EXT_2D_LOCATION => HeaderExt::Location2d(parse(&bytes, read_location)?),
        EXT_3D_LOCATION => {
            let (loc, elevation) = parse(&bytes, |r| {
                let loc = read_location(r)?;
                let e = rint(r, f("wsa.header.elevation", "Elevation"), -4_096, 61_439)? as i32;
                Ok((loc, e))
            })?;
            HeaderExt::Location3d(loc, elevation)
        }
        EXT_ADVERTISER_ID => {
            let text = parse(&bytes, |r| {
                let n = read_length(r, "AdvertiserIdentifier")?;
                r.read_octets(n)
            })?;
            let s = String::from_utf8(text).map_err(|_| UperError::Malformed {
                construct: "AdvertiserIdentifier",
                detail: "a UTF8String whose octets are not UTF-8",
            })?;
            HeaderExt::AdvertiserId(s)
        }
        _ => HeaderExt::Other(id, bytes),
    })
}

fn service_ext(e: &ServiceInfoExt) -> Result<(u8, Vec<u8>), UperError> {
    Ok(match e {
        ServiceInfoExt::ProviderServiceContext(psc) => (
            EXT_PROVIDER_SERV_CONTEXT,
            inner(|w| {
                // fillBit BIT STRING (SIZE(3)), "all bits set to zero".
                w.write_bits(0, 3);
                write_constrained_length(
                    w,
                    f("wsa.serviceInfo.psc", "ProviderServiceContext"),
                    psc.len(),
                    0,
                    31,
                )?;
                w.write_octets(psc);
                Ok(())
            })?,
        ),
        ServiceInfoExt::Ipv6Address(a) => (
            EXT_IPV6_ADDRESS,
            inner(|w| {
                write_fixed_octet_string(w, a);
                Ok(())
            })?,
        ),
        ServiceInfoExt::ServicePort(p) => (
            EXT_SERVICE_PORT,
            inner(|w| {
                int(
                    w,
                    f("wsa.serviceInfo.port", "ServicePort"),
                    i64::from(*p),
                    0,
                    65_535,
                )
            })?,
        ),
        ServiceInfoExt::ProviderMac(m) => (
            EXT_PROVIDER_MAC,
            inner(|w| {
                write_fixed_octet_string(w, m);
                Ok(())
            })?,
        ),
        ServiceInfoExt::RcpiThreshold(v) => (
            EXT_RCPI_THRESHOLD,
            u8_inner("wsa.serviceInfo.rcpi", "RcpiThreshold", *v)?,
        ),
        ServiceInfoExt::WsaCountThreshold(v) => (
            EXT_WSA_COUNT_THRESHOLD,
            u8_inner("wsa.serviceInfo.wsaCount", "WsaCountThreshold", *v)?,
        ),
        ServiceInfoExt::WsaCountThresholdInterval(v) => (
            EXT_WSA_COUNT_THRES_INT,
            u8_inner(
                "wsa.serviceInfo.wsaCountInterval",
                "WsaCountThresholdInterval",
                *v,
            )?,
        ),
        ServiceInfoExt::Other(id, bytes) => (*id, bytes.clone()),
    })
}

fn parse_service_ext(id: u8, bytes: Vec<u8>) -> Result<ServiceInfoExt, UperError> {
    Ok(match id {
        EXT_PROVIDER_SERV_CONTEXT => ServiceInfoExt::ProviderServiceContext(parse(&bytes, |r| {
            let _fill = read_fixed_bit_string(r, 3)?;
            let n = read_constrained_length(
                r,
                f("wsa.serviceInfo.psc", "ProviderServiceContext"),
                0,
                31,
            )?;
            r.read_octets(n)
        })?),
        EXT_IPV6_ADDRESS => ServiceInfoExt::Ipv6Address(parse(&bytes, octets16)?),
        EXT_SERVICE_PORT => ServiceInfoExt::ServicePort(parse(&bytes, |r| {
            Ok(rint(r, f("wsa.serviceInfo.port", "ServicePort"), 0, 65_535)? as u16)
        })?),
        EXT_PROVIDER_MAC => ServiceInfoExt::ProviderMac(parse(&bytes, octets6)?),
        EXT_RCPI_THRESHOLD => ServiceInfoExt::RcpiThreshold(u8_parse(
            &bytes,
            "wsa.serviceInfo.rcpi",
            "RcpiThreshold",
        )?),
        EXT_WSA_COUNT_THRESHOLD => ServiceInfoExt::WsaCountThreshold(u8_parse(
            &bytes,
            "wsa.serviceInfo.wsaCount",
            "WsaCountThreshold",
        )?),
        EXT_WSA_COUNT_THRES_INT => ServiceInfoExt::WsaCountThresholdInterval(u8_parse(
            &bytes,
            "wsa.serviceInfo.wsaCountInterval",
            "WsaCountThresholdInterval",
        )?),
        _ => ServiceInfoExt::Other(id, bytes),
    })
}

fn channel_ext(e: &ChannelInfoExt) -> Result<(u8, Vec<u8>), UperError> {
    Ok(match e {
        ChannelInfoExt::Edca(records) => (
            EXT_EDCA_PARAMETER_SET,
            inner(|w| {
                for rec in records {
                    let p = "wsa.channelInfo.edca";
                    int(w, f(p, "EdcaParameterRecord.res"), i64::from(rec.res), 0, 1)?;
                    int(w, f(p, "EdcaParameterRecord.aci"), i64::from(rec.aci), 0, 3)?;
                    int(w, f(p, "EdcaParameterRecord.acm"), i64::from(rec.acm), 0, 1)?;
                    int(
                        w,
                        f(p, "EdcaParameterRecord.aifsn"),
                        i64::from(rec.aifsn),
                        0,
                        15,
                    )?;
                    int(
                        w,
                        f(p, "EdcaParameterRecord.ecwMax"),
                        i64::from(rec.ecw_max),
                        0,
                        15,
                    )?;
                    int(
                        w,
                        f(p, "EdcaParameterRecord.ecwMin"),
                        i64::from(rec.ecw_min),
                        0,
                        15,
                    )?;
                    int(
                        w,
                        f(p, "EdcaParameterRecord.txopLimit"),
                        i64::from(rec.txop_limit),
                        0,
                        65_535,
                    )?;
                }
                Ok(())
            })?,
        ),
        ChannelInfoExt::ChannelAccess(v) => (
            EXT_CHANNEL_ACCESS,
            u8_inner("wsa.channelInfo.channelAccess", "ChannelAccess80211", *v)?,
        ),
        ChannelInfoExt::Other(id, bytes) => (*id, bytes.clone()),
    })
}

fn parse_channel_ext(id: u8, bytes: Vec<u8>) -> Result<ChannelInfoExt, UperError> {
    Ok(match id {
        EXT_EDCA_PARAMETER_SET => ChannelInfoExt::Edca(parse(&bytes, |r| {
            let mut out = [EdcaRecord::default(); 4];
            let p = "wsa.channelInfo.edca";
            for rec in &mut out {
                rec.res = rint(r, f(p, "EdcaParameterRecord.res"), 0, 1)? as u8;
                rec.aci = rint(r, f(p, "EdcaParameterRecord.aci"), 0, 3)? as u8;
                rec.acm = rint(r, f(p, "EdcaParameterRecord.acm"), 0, 1)? as u8;
                rec.aifsn = rint(r, f(p, "EdcaParameterRecord.aifsn"), 0, 15)? as u8;
                rec.ecw_max = rint(r, f(p, "EdcaParameterRecord.ecwMax"), 0, 15)? as u8;
                rec.ecw_min = rint(r, f(p, "EdcaParameterRecord.ecwMin"), 0, 15)? as u8;
                rec.txop_limit = rint(r, f(p, "EdcaParameterRecord.txopLimit"), 0, 65_535)? as u16;
            }
            Ok(out)
        })?),
        EXT_CHANNEL_ACCESS => ChannelInfoExt::ChannelAccess(u8_parse(
            &bytes,
            "wsa.channelInfo.channelAccess",
            "ChannelAccess80211",
        )?),
        _ => ChannelInfoExt::Other(id, bytes),
    })
}

fn routing_ext(e: &RoutingExt) -> (u8, Vec<u8>) {
    match e {
        // Fixed-size octet strings with no other bits are their own padded encoding.
        RoutingExt::SecondaryDns(a) => (EXT_SECONDARY_DNS, a.to_vec()),
        RoutingExt::GatewayMac(m) => (EXT_GATEWAY_MAC, m.to_vec()),
        RoutingExt::Other(id, bytes) => (*id, bytes.clone()),
    }
}

fn parse_routing_ext(id: u8, bytes: Vec<u8>) -> Result<RoutingExt, UperError> {
    Ok(match id {
        EXT_SECONDARY_DNS => RoutingExt::SecondaryDns(parse(&bytes, octets16)?),
        EXT_GATEWAY_MAC => RoutingExt::GatewayMac(parse(&bytes, octets6)?),
        _ => RoutingExt::Other(id, bytes),
    })
}

fn octets16(r: &mut BitReader<'_>) -> Result<[u8; 16], UperError> {
    let v = read_fixed_octet_string(r, 16)?;
    let mut a = [0u8; 16];
    a.copy_from_slice(&v);
    Ok(a)
}

fn octets6(r: &mut BitReader<'_>) -> Result<[u8; 6], UperError> {
    let v = read_fixed_octet_string(r, 6)?;
    let mut a = [0u8; 6];
    a.copy_from_slice(&v);
    Ok(a)
}

// `VarLengthNumber` (ISO 17419 `CITSapplMgmtIDs`): a non-extensible CHOICE of `content`
// (0..=127) and `extension`, nested three deep, the innermost `Ext3` an extensible
// INTEGER (2113664..=270549119, ...). Each level is one choice bit, so the encoding is
// the p-encoded PSID of IEEE 1609.12: 1, 2, 3 or 4 octets.
const PSID_LEVELS: [(i64, i64); 3] = [(0, 127), (128, 16_511), (16_512, 2_113_663)];
const PSID_EXT3: (i64, i64) = (2_113_664, 270_549_119);

fn write_psid(w: &mut BitWriter, psid: u32) -> Result<(), UperError> {
    let v = i64::from(psid);
    let field = f("wsa.serviceInfo.serviceID", "VarLengthNumber");
    for (min, max) in PSID_LEVELS {
        if v <= max {
            write_choice_index(w, field, false, 0, 2)?;
            return int(w, field, v, min, max);
        }
        write_choice_index(w, field, false, 1, 2)?;
    }
    if v > PSID_EXT3.1 {
        return Err(UperError::Unsupported {
            construct: "VarLengthNumber.Ext3",
            detail: "a PSID above 270549119 is an extension addition of Ext3 (five or \
                     more p-encoded octets); IEEE 1609.12 allocates none",
        });
    }
    // Ext3's extension bit: the value is in the root.
    w.write_bit(false);
    int(w, field, v, PSID_EXT3.0, PSID_EXT3.1)
}

fn read_psid(r: &mut BitReader<'_>) -> Result<u32, UperError> {
    let field = f("wsa.serviceInfo.serviceID", "VarLengthNumber");
    for (min, max) in PSID_LEVELS {
        if read_choice_index(r, field, "VarLengthNumber", false, 2)? == 0 {
            return Ok(rint(r, field, min, max)? as u32);
        }
    }
    if r.read_bit()? {
        return Err(UperError::Unsupported {
            construct: "VarLengthNumber.Ext3",
            detail: "the PSID is an extension addition of Ext3 (five or more octets)",
        });
    }
    Ok(rint(r, field, PSID_EXT3.0, PSID_EXT3.1)? as u32)
}

/// The WSA a US RSU sends to advertise the SCMS service over IP on a service channel:
/// the shape of the USDOT RSU Specification v4.1 Appendix D example (pp. 118-120) and
/// Req_570-573.
///
/// - header: `RepeatRate` (WSAs per 5 s; App. D: 50) and the RSU's `ThreeDLocation`;
/// - services: WAVE security management (PSID 0x23, an ASSUMPTION, see
///   [`PSID_SECURITY_MANAGEMENT`]) and IPv6 Routing, both on channel index 1;
/// - one channel: operating class 17, the service channel, its power, 6 Mb/s fixed;
/// - the WAVE Routing Advertisement: the RSU's /64 prefix, gateway and DNS.
#[must_use]
pub fn scms_service_wsa(
    repeat_rate_per_5s: u8,
    location: Location,
    elevation_dm: i32,
    service_channel: u8,
    power_dbm: i8,
) -> Wsa {
    Wsa {
        sa_id: 0,
        content_count: 0,
        header: Some(vec![
            HeaderExt::RepeatRate(repeat_rate_per_5s),
            HeaderExt::Location3d(location, elevation_dm),
        ]),
        services: Some(vec![
            ServiceInfo {
                psid: PSID_SECURITY_MANAGEMENT,
                channel_index: 1,
                extensions: None,
            },
            ServiceInfo {
                psid: PSID_IPV6_ROUTING,
                channel_index: 1,
                extensions: None,
            },
        ]),
        channels: Some(vec![ChannelInfo {
            operating_class: 17,
            channel: service_channel,
            power_dbm,
            adaptable: false,
            data_rate: 12,
            extensions: None,
        }]),
        routing: Some(RoutingAdvertisement {
            lifetime_s: 1_800,
            // A documentation prefix (RFC 3849, 2001:db8::/32): the RSU's real one is
            // the operator's and changes nothing in the size.
            prefix: [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
            prefix_len: 64,
            default_gateway: [0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1],
            primary_dns: [
                0x20, 0x01, 0x0d, 0xb8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x53,
            ],
            extensions: Vec::new(),
        }),
    }
}

/// The encoded length of [`scms_service_wsa`] for a service channel at 20 dBm, octets:
/// the WSA payload an RSU signs and sends (`RsuConfig::wsa_payload_bytes`). Every field
/// is fixed-width except the PSIDs, so the length does not depend on the values.
#[must_use]
pub fn scms_service_wsa_bytes() -> u32 {
    let wsa = scms_service_wsa(50, Location { lat: 0, lon: 0 }, 0, 174, 20);
    encode_wsa(&wsa).map_or(0, |b| b.len() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full() -> Wsa {
        let mut w = scms_service_wsa(
            50,
            Location {
                lat: 407_527_000,
                lon: -739_868_000,
            },
            120,
            174,
            20,
        );
        w.sa_id = 3;
        w.content_count = 9;
        w.header
            .as_mut()
            .unwrap()
            .push(HeaderExt::AdvertiserId("RSU-42".into()));
        w.services.as_mut().unwrap()[0].extensions = Some(vec![
            ServiceInfoExt::ProviderServiceContext(b"scms".to_vec()),
            ServiceInfoExt::Ipv6Address([0x20; 16]),
            ServiceInfoExt::ServicePort(443),
            ServiceInfoExt::ProviderMac([1, 2, 3, 4, 5, 6]),
            ServiceInfoExt::RcpiThreshold(110),
            ServiceInfoExt::WsaCountThreshold(2),
            ServiceInfoExt::WsaCountThresholdInterval(10),
        ]);
        w.channels.as_mut().unwrap()[0].extensions = Some(vec![
            ChannelInfoExt::Edca([
                EdcaRecord {
                    aci: 0,
                    aifsn: 6,
                    ecw_max: 10,
                    ecw_min: 4,
                    ..Default::default()
                },
                EdcaRecord {
                    aci: 1,
                    aifsn: 9,
                    ecw_max: 10,
                    ecw_min: 4,
                    ..Default::default()
                },
                EdcaRecord {
                    aci: 2,
                    aifsn: 3,
                    ecw_max: 4,
                    ecw_min: 3,
                    ..Default::default()
                },
                EdcaRecord {
                    aci: 3,
                    aifsn: 2,
                    ecw_max: 3,
                    ecw_min: 2,
                    ..Default::default()
                },
            ]),
            ChannelInfoExt::ChannelAccess(1),
        ]);
        w.routing.as_mut().unwrap().extensions = vec![
            RoutingExt::SecondaryDns([0x20; 16]),
            RoutingExt::GatewayMac([6, 5, 4, 3, 2, 1]),
            RoutingExt::Other(99, vec![0xab, 0xcd]),
        ];
        w
    }

    #[test]
    fn round_trip_with_routing_advertisement() {
        for wsa in [
            Wsa::default(),
            scms_service_wsa(50, Location { lat: 1, lon: -1 }, 0, 172, -5),
            full(),
        ] {
            let bytes = encode_wsa(&wsa).expect("encodes");
            assert_eq!(decode_wsa(&bytes).expect("decodes"), wsa, "{bytes:02x?}");
        }
    }

    #[test]
    fn the_psid_is_p_encoded_in_one_to_four_octets() {
        // IEEE 1609.12's p-encoding is what VarLengthNumber's choice bits spell.
        for (psid, hex) in [
            (0x23_u32, "23"),
            (0x7f, "7f"),
            (0x80, "8000"),
            (0x4080, "c00000"),
            (PSID_IPV6_ROUTING, "effffffe"),
        ] {
            let mut w = BitWriter::new();
            write_psid(&mut w, psid).expect("encodes");
            let bytes = w.into_bytes();
            let got: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(got, hex, "PSID {psid:#x}");
            assert_eq!(
                read_psid(&mut BitReader::new(&bytes)).expect("decodes"),
                psid
            );
        }
    }

    #[test]
    fn a_minimal_wsa_is_two_octets_and_bad_bytes_are_refused() {
        assert_eq!(encode_wsa(&Wsa::default()).unwrap(), vec![0x30, 0x00]);
        // messageID 1 is the ISO SAR message, not a WSA.
        assert!(decode_wsa(&[0xb0, 0x00]).is_err());
        // Version 2.
        assert!(decode_wsa(&[0x20, 0x00]).is_err());
        // Truncated and trailing.
        assert!(decode_wsa(&[0x30]).is_err());
        assert!(decode_wsa(&[0x30, 0x00, 0x00]).is_err());
        let mut bad = Wsa::default();
        bad.sa_id = 16;
        assert!(encode_wsa(&bad).is_err());
    }
}
