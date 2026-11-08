//! Attack name to family and to the codes of the datasets it is compared with (schema.md §9).
//!
//! The VeReMi codes come from [`crate::dataset::veremi::Codebook`], whose tables this module
//! does not copy: a behaviour that does not match F2MD's exports as
//! [`crate::dataset::veremi::UNNAMED_CODE`]. The NextGen and DARE names are the closest
//! class §9 lists for the attack, or null where §9 lists none.

use crate::dataset::veremi::Codebook;

/// One attack's crosswalk row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Crosswalk {
    /// The engine's short name, e.g. `ConstPos`.
    pub attack: String,
    /// The §9 family, e.g. `position`.
    pub family: &'static str,
    /// VeReMi Extension (F2MD) code; 0 for benign, 1024 for unnamed.
    pub veremi_ext_code: i64,
    /// VeReMi 1.0 code; 0 for benign, 1024 for unnamed.
    pub veremi1_code: i64,
    /// The closest VeReMi NextGen class.
    pub nextgen_name: Option<&'static str>,
    /// The closest DARE class.
    pub dare_name: Option<&'static str>,
}

/// `(attack, family, nextgen, dare)`, from schema.md §9.
const TABLE: &[(&str, &str, Option<&str>, Option<&str>)] = &[
    ("ConstPos", "position", None, Some("position malfunction")),
    (
        "ConstPosOffset",
        "position",
        Some("constantPositionOffset"),
        Some("position malfunction"),
    ),
    (
        "RandomPos",
        "position",
        Some("randomPositionOffset"),
        Some("position malfunction"),
    ),
    ("Teleport", "position", None, Some("position malfunction")),
    (
        "SineWavePos",
        "position",
        None,
        Some("position malfunction"),
    ),
    ("SlowDrift", "position", None, Some("position malfunction")),
    (
        "AlongRoadOffset",
        "position",
        None,
        Some("position malfunction"),
    ),
    (
        "ConstSpeedOffset",
        "speed",
        Some("constantSpeedOffset"),
        None,
    ),
    ("RandomSpeed", "speed", Some("randomSpeedOffset"), None),
    ("StopAndGo", "speed", Some("suddenStop"), None),
    ("ReversedHeading", "heading", Some("reversedHeading"), None),
    ("HeadingOffset", "heading", None, None),
    ("PosSpeedInconsistent", "inconsistent_state", None, None),
    ("PosHeadingInconsistent", "inconsistent_state", None, None),
    (
        "EventualStop",
        "inconsistent_state",
        Some("suddenStop"),
        Some("sudden stop"),
    ),
    (
        "DelayedMessages",
        "timing",
        Some("timeDelayAttack"),
        Some("delayed"),
    ),
    ("OutOfOrder", "timing", None, None),
    (
        "DataReplay",
        "timing",
        Some("dataReplay"),
        Some("data replay"),
    ),
    (
        "Sybil",
        "identity",
        Some("trafficCongestionSybil"),
        Some("Sybil"),
    ),
    (
        "GhostVehicles",
        "identity",
        Some("trafficCongestionSybil"),
        Some("traffic congestion"),
    ),
    ("InvalidSignature", "credentials", None, None),
    ("ExpiredCert", "credentials", None, None),
    ("NotYetValid", "credentials", None, None),
    ("WrongRegionCert", "credentials", None, None),
    ("DoS", "flooding", Some("dosAttack"), Some("DoS")),
    ("DoSRandom", "flooding", Some("dosAttack"), Some("DoS")),
    ("Disruptive", "flooding", None, Some("disruptive")),
    ("OversizedFlood", "flooding", None, None),
    ("FakeHazard", "application", None, None),
    ("VruImpersonation", "application", None, None),
    ("VruPositionSpoof", "application", None, None),
    ("FakeCpm", "application", None, None),
    ("SelectiveDrop", "suppression_relay", None, None),
    ("RelayReplay", "suppression_relay", Some("dataReplay"), None),
    ("ConstantJamming", "jamming", None, None),
    ("ReactiveJamming", "jamming", None, None),
    ("RandomDutyJamming", "jamming", None, None),
    ("FalseCrl", "compromised_rsu", None, None),
    ("SuppressForwardedReports", "compromised_rsu", None, None),
    ("PoisonForwardedReports", "compromised_rsu", None, None),
    ("FalseSpat", "compromised_rsu", None, None),
    ("FalseMap", "compromised_rsu", None, None),
    ("FalseCtl", "compromised_rsu", None, None),
    // The F2MD / VeReMi NextGen behaviours (`threat/attacker/f2md/<Kind>`, schema.md §9).
    (
        "FeignedBraking",
        "acceleration",
        Some("feignedBraking"),
        None,
    ),
    (
        "AccelerationMultiplication",
        "acceleration",
        Some("accelerationMultiplication"),
        None,
    ),
    (
        "PositionMirroring",
        "position",
        Some("positionMirroring"),
        Some("position malfunction"),
    ),
    ("ZeroSpeedReport", "speed", Some("zeroSpeedReport"), None),
    (
        "UniformRandomPos",
        "position",
        None,
        Some("position malfunction"),
    ),
    ("ConstSpeed", "speed", None, None),
    (
        "RandomSpeedOffset",
        "speed",
        Some("randomSpeedOffset"),
        None,
    ),
    (
        "NeighbourReplay",
        "timing",
        Some("dataReplay"),
        Some("data replay"),
    ),
    (
        "DoSDisruptive",
        "flooding",
        Some("dosAttack"),
        Some("disruptive"),
    ),
    (
        "DataReplaySybil",
        "identity",
        Some("dataReplay"),
        Some("Sybil"),
    ),
    (
        "DoSRandomSybil",
        "identity",
        Some("dosAttack"),
        Some("Sybil"),
    ),
    (
        "DoSDisruptiveSybil",
        "identity",
        Some("dosAttack"),
        Some("Sybil"),
    ),
    ("ReportPoisoning", "against_ma", None, None),
    ("BenignClassMimic", "against_ma", None, None),
    ("PrivacyObserver", "privacy", None, None),
];

/// The short name of an attacker id: `threat/attacker/legacy/ConstPos` is `ConstPos`.
#[must_use]
pub fn short_name(attacker: &str) -> &str {
    attacker.rsplit('/').next().unwrap_or(attacker)
}

/// The crosswalk row of an attacker id or short name. An attack §9 does not list gets
/// family `other` and the codebooks' answer (normally 1024).
#[must_use]
pub fn of(attacker: &str) -> Crosswalk {
    let name = short_name(attacker);
    let row = TABLE.iter().find(|(a, ..)| a.eq_ignore_ascii_case(name));
    Crosswalk {
        attack: name.to_string(),
        family: row.map_or("other", |r| r.1),
        veremi_ext_code: Codebook::VeremiExt.code(name),
        veremi1_code: Codebook::Veremi1.code(name),
        nextgen_name: row.and_then(|r| r.2),
        dare_name: row.and_then(|r| r.3),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn const_pos_maps_to_the_f2md_codes_and_its_family() {
        let c = of("threat/attacker/legacy/ConstPos");
        assert_eq!(c.attack, "ConstPos");
        assert_eq!(c.family, "position");
        assert_eq!(c.veremi_ext_code, 1);
        assert_eq!(c.veremi1_code, 1);
        let r = of("RandomPos");
        assert_eq!((r.veremi_ext_code, r.veremi1_code), (4, 8));
        assert_eq!(r.nextgen_name, Some("randomPositionOffset"));
    }

    /// The F2MD behaviours have rows: their family, F2MD's code where F2MD has the
    /// behaviour, and the NextGen class.
    #[test]
    fn the_f2md_behaviours_have_their_rows() {
        let c = of("threat/attacker/f2md/FeignedBraking");
        assert_eq!(c.family, "acceleration");
        assert_eq!(c.nextgen_name, Some("feignedBraking"));
        assert_eq!(c.veremi_ext_code, crate::dataset::veremi::UNNAMED_CODE);
        let c = of("threat/attacker/f2md/UniformRandomPos");
        assert_eq!(
            (c.family, c.veremi_ext_code, c.veremi1_code),
            ("position", 3, 4)
        );
        let c = of("threat/attacker/f2md/NeighbourReplay");
        assert_eq!((c.family, c.veremi_ext_code), ("timing", 11));
        assert_eq!(c.dare_name, Some("data replay"));
        let c = of("threat/attacker/f2md/DoSDisruptiveSybil");
        assert_eq!((c.family, c.veremi_ext_code), ("identity", 19));
        for k in [
            "AccelerationMultiplication",
            "PositionMirroring",
            "ZeroSpeedReport",
            "ConstSpeed",
            "RandomSpeedOffset",
            "DoSDisruptive",
            "DataReplaySybil",
            "DoSRandomSybil",
        ] {
            assert_ne!(of(k).family, "other", "{k} has no crosswalk row");
        }
    }

    #[test]
    fn an_unlisted_attack_is_other_and_unnamed() {
        let c = of("threat/attacker/new/SomethingElse");
        assert_eq!(c.family, "other");
        assert_eq!(c.veremi_ext_code, crate::dataset::veremi::UNNAMED_CODE);
        assert_eq!(c.nextgen_name, None);
    }
}
