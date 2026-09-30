//! Which PSID / ITS-AID and which BTP port each message is sent under — checked against the
//! registries, not recalled.
//!
//! # Sources, and how they were read
//!
//! * **PSID / ITS-AID.** The IEEE Registration Authority's public PSID list
//!   (<https://standards.ieee.org/products-programs/regauth/psid/public/>), which is the
//!   registry IEEE 1609.12 points at and ETSI TS 102 965 shares for ITS-AIDs. The page
//!   itself refuses scripted access, so the values were read on 2026-09-30 from Wireshark's
//!   transcription of it (`epan/dissectors/asn1/ieee1609dot2/IEEE1609dot12.asn`, whose first
//!   line cites that URL). Every value below is on that list with the owner named there.
//! * **BTP ports.** ETSI TS 103 248's well-known ports, read on 2026-09-30 from Wireshark's
//!   ITS dissector (`epan/dissectors/asn1/its/packet-its-template.c`, `ITS_WKP_*`).
//!
//! | Message | Stack | PSID / ITS-AID | Registry name | BTP port |
//! |---|---|---|---|---|
//! | BSM | US | 32 (`0x20`) | vehicle-to-vehicle-safety-and-awareness (SAE J2735) | — |
//! | PSM | US | 39 (`0x27`) | vulnerable-road-users-safety-applications | — |
//! | SPaT | US | 130 (`0x82`) | intersection-safety-and-awareness (SAE J2735) | — |
//! | MAP | US | 130 (`0x82`) | the same entry; see the note below | — |
//! | SRM | US | 2113686 (`0x204096`) | traffic-signal-request | — |
//! | SSM | US | 2113685 (`0x204095`) | traffic-signal-priority-status | — |
//! | CAM | EU | 36 | ca-basic-services (EN 302 637-2) | 2001 |
//! | DENM | EU | 37 | den-basic-services (EN 302 637-3) | 2002 |
//! | MAPEM | EU | 138 | road-and-lane-topology-service (ISO 19091 / TS 103 301) | 2003 |
//! | SPATEM | EU | 137 | traffic-light-manoeuver-service (ISO 19091 / TS 103 301) | 2004 |
//! | SREM | EU | 140 | traffic-light-control-requests-service | 2007 |
//! | SSEM | EU | 637 | traffic-light-control-status-service | 2008 |
//! | CPM | EU | 639 | collective-perception-service (TS 103 324) | 2009 |
//! | VAM | EU | 638 | vru-awareness-basic-service (TS 103 300-3) | 2018 |
//! | misbehaviour report | both | 38 | misbehavior-reporting-for-common-applications (CAMP) | — |
//! | CRL | both | 256 | certificate-revocation-list-application | 2015 |
//!
//! **MAP.** The registry also lists `map-distribution` (2113687, `0x204097`); US
//! connected-intersection deployments send MAP beside SPaT under the intersection entry
//! (130), which is what this build does. [`MAP_DISTRIBUTION_PSID`] names the alternative so
//! a study can switch to it.
//!
//! **What was wrong before this module.** Every message a node signed carried PSID 0x20 in
//! its 1609.2 header — a SPaT, a CAM and a DENM alike — and the SRM and SSM travelled under
//! 0x82 on WSMP. The first misstates what the message is to every receiver that dispatches
//! by PSID (1609.3 hands a WSM to the application registered for its PSID); the second
//! sent a signal request under the SPaT's identifier.

use crate::codec::MsgType;

/// SAE J2735 BSM: `psid-vehicle-to-vehicle-safety-and-awarenesss` (32).
pub const PSID_BSM: u64 = 32;
/// J2735 PSM: `psid-vulnerable-road-users-safety-applications` (39).
pub const PSID_VRU: u64 = 39;
/// J2735 SPaT and MAP: `psid-intersection-safety-and-awareness` (130).
pub const PSID_INTERSECTION: u64 = 130;
/// The registry's `psid-map-distribution` (2113687), an alternative for MAP.
pub const MAP_DISTRIBUTION_PSID: u64 = 2_113_687;
/// J2735 SRM: `psid-traffic-signal-request` (2113686).
pub const PSID_SIGNAL_REQUEST: u64 = 2_113_686;
/// J2735 SSM: `psid-traffic-signal-priority-status` (2113685).
pub const PSID_SIGNAL_STATUS: u64 = 2_113_685;
/// ETSI CAM: `psid-ca-basic-services` (36).
pub const ITS_AID_CAM: u64 = 36;
/// ETSI DENM: `psid-den-basic-services` (37).
pub const ITS_AID_DENM: u64 = 37;
/// ETSI SPATEM: `psid-traffic-light-manoeuver-service` (137).
pub const ITS_AID_SPATEM: u64 = 137;
/// ETSI MAPEM: `psid-road-and-lane-topology-service` (138).
pub const ITS_AID_MAPEM: u64 = 138;
/// ETSI SREM: `psid-traffic-light-control-requests-service` (140).
pub const ITS_AID_SREM: u64 = 140;
/// ETSI SSEM: `psid-traffic-light-control-status-service` (637).
pub const ITS_AID_SSEM: u64 = 637;
/// ETSI VAM: `psid-vru-awareness-basic-service` (638).
pub const ITS_AID_VAM: u64 = 638;
/// ETSI CPM: `psid-collective-perception-service` (639).
pub const ITS_AID_CPM: u64 = 639;
/// Misbehaviour reporting: `psid-misbehavior-reporting-for-common-applications` (38).
pub const PSID_MISBEHAVIOUR_REPORT: u64 = 38;
/// CRLs: `psid-certificate-revocation-list-application` (256).
pub const PSID_CRL: u64 = 256;
/// WSA: `psid-wave-service-advertisement` (135).
pub const PSID_WSA: u64 = 135;

/// The PSID / ITS-AID `msg` is signed and addressed under, on the European stack (`etsi`)
/// or the US one.
pub const fn psid(msg: MsgType, etsi: bool) -> u64 {
    match (msg, etsi) {
        (MsgType::Bsm, _) => PSID_BSM,
        (MsgType::Cam, _) => ITS_AID_CAM,
        (MsgType::Denm, _) => ITS_AID_DENM,
        (MsgType::Psm, _) => PSID_VRU,
        (MsgType::Vam, _) => ITS_AID_VAM,
        (MsgType::Cpm, _) => ITS_AID_CPM,
        (MsgType::Spat, false) | (MsgType::Map, false) => PSID_INTERSECTION,
        (MsgType::Spat, true) => ITS_AID_SPATEM,
        (MsgType::Map, true) => ITS_AID_MAPEM,
        (MsgType::Srm, false) => PSID_SIGNAL_REQUEST,
        (MsgType::Srm, true) => ITS_AID_SREM,
        (MsgType::Ssm, false) => PSID_SIGNAL_STATUS,
        (MsgType::Ssm, true) => ITS_AID_SSEM,
        (MsgType::Wsa, _) => PSID_WSA,
        (MsgType::Crl, _) => PSID_CRL,
        (MsgType::Mbr, _) => PSID_MISBEHAVIOUR_REPORT,
    }
}

/// The PSIDs a station's pseudonym or application certificate must permit: every message
/// in `sends`, on its stack, deduplicated and ascending (1609.2 §6.4.28: a receiver checks
/// the SPDU's PSID against the signer's `appPermissions`).
pub fn permissions(sends: &[MsgType], etsi: bool) -> Vec<u64> {
    let mut v: Vec<u64> = sends.iter().map(|m| psid(*m, etsi)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// ETSI TS 103 248 BTP destination port of an ETSI message, or `None` for a message the
/// European stack does not carry over BTP here.
pub const fn btp_port(msg: MsgType) -> Option<u16> {
    match msg {
        MsgType::Cam => Some(2001),
        MsgType::Denm => Some(2002),
        MsgType::Map => Some(2003),
        MsgType::Spat => Some(2004),
        MsgType::Srm => Some(2007),
        MsgType::Ssm => Some(2008),
        MsgType::Cpm => Some(2009),
        MsgType::Crl => Some(2015),
        MsgType::Vam => Some(2018),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registry values, pinned: a change here is a change to what the air says, and it
    /// must come with a registry citation.
    #[test]
    fn every_message_has_its_registered_identifier() {
        assert_eq!(psid(MsgType::Bsm, false), 0x20);
        assert_eq!(psid(MsgType::Spat, false), 0x82);
        assert_eq!(psid(MsgType::Map, false), 0x82);
        assert_eq!(psid(MsgType::Srm, false), 0x20_4096);
        assert_eq!(psid(MsgType::Ssm, false), 0x20_4095);
        assert_eq!(psid(MsgType::Psm, false), 0x27);
        assert_eq!(psid(MsgType::Cam, true), 36);
        assert_eq!(psid(MsgType::Denm, true), 37);
        assert_eq!(psid(MsgType::Spat, true), 137);
        assert_eq!(psid(MsgType::Map, true), 138);
        assert_eq!(psid(MsgType::Srm, true), 140);
        assert_eq!(psid(MsgType::Ssm, true), 637);
        assert_eq!(psid(MsgType::Vam, true), 638);
        assert_eq!(psid(MsgType::Cpm, true), 639);
        assert_eq!(btp_port(MsgType::Cpm), Some(2009));
        assert_eq!(btp_port(MsgType::Vam), Some(2018));
        assert_eq!(
            permissions(&[MsgType::Cam, MsgType::Denm, MsgType::Cam], true),
            vec![36, 37]
        );
    }
}
