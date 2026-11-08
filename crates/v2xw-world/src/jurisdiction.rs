//! The US jurisdictions of the city library (`worlds/cities.yaml`): each state's default
//! urban speed limit and rules of the road, as `urban-us-<state>` highway presets.
//!
//! A city's untagged street falls back to a speed limit that is a statement about its
//! jurisdiction ([`crate::osm::HighwayPreset`]), and so is whether a car may turn right on
//! red. The two hand-built US presets (`urban-us-nyc`, `urban-us-portland`) carry their
//! own tables in `osm.rs`; every other state is a row here, and its preset is the
//! `urban-us-nyc` table with this row's urban speed in place of New York City's 25 mph.
//!
//! # How much each row is worth
//!
//! [`UsJurisdictionData::cited`] says which rows quote the state's own statute (the large
//! states and the ones whose rule was checked). The rest state the **generic US urban
//! default** — 25 mph in a business or residence district, the most common statutory
//! value (California, Arizona, Virginia, New Jersey, Washington and others) — and say so in
//! their source text rather than pretend to a citation. The rows marked "checked
//! 2026-10-07" were compared with the published statute or regulation that day; the other
//! cited rows give the section as the code is commonly cited and have not been re-read
//! against its current text. Either way the fallback only
//! applies to a street with no `maxspeed` tag: a downtown arterial in OpenStreetMap is
//! usually tagged, the tag wins, and the import report counts how often the fallback was
//! reached (`speeds_defaulted`).
//!
//! An untagged motorway falls back to 55 mph in every state: urban freeways are posted and
//! nearly always tagged, and 55 mph is the most common urban freeway posting.
//!
//! # Rules of the road
//!
//! * **Right turn on red** is permitted after a full stop, unless a sign prohibits it, in
//!   every state and the District of Columbia (Uniform Vehicle Code §11-202(c)3; the
//!   Energy Policy and Conservation Act of 1975 §362(c)(5) made it a condition of federal
//!   energy-conservation funds, and by 1980 every state had adopted it). The exception is
//!   New York City (`urban-us-nyc`, NY VTL §1111(d)(2)), not a state.
//! * **Motorcycle filtering or lane splitting** is lawful in California (CVC §21658.1),
//!   Utah (Utah Code §41-6a-704.5, 2019), Montana (MCA §61-8-328, 2021), Arizona (ARS
//!   §28-903, 2022) and Colorado (CRS §42-4-1503 as amended by SB 24-079, 2024), in each
//!   case between slow or stopped vehicles; prohibited elsewhere (UVC §11-1103(c)).
//! * **Crossing away from a crosswalk** is lawful without right of way in California
//!   (the Freedom to Walk Act, AB 2147, amending CVC §21955, in force 2023-01-01) and
//!   Virginia (Va. Code §46.2-923 as amended in 2020, HB 1350/SB 914: no stop for
//!   jaywalking alone); an offence elsewhere (UVC §11-503 and the codes modelled on it).
//!   Some cities have done the same by ordinance (Kansas City, Missouri, 2021; Denver,
//!   2023); a city-level rule is not modelled here.

/// One US jurisdiction's defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UsJurisdictionData {
    /// The suffix of its preset's label: a state's USPS code, lower case, or a city's
    /// name where the city sets its own default (`boston`).
    pub key: &'static str,
    /// The highway preset label, `urban-us-<key>`.
    pub label: &'static str,
    /// The jurisdiction's name.
    pub name: &'static str,
    /// The default speed limit on an untagged urban street, mph.
    pub urban_mph: f64,
    /// Where `urban_mph` comes from.
    pub urban_source: &'static str,
    /// Whether `urban_source` quotes the jurisdiction's own law (true) or states the
    /// generic US urban default (false).
    pub cited: bool,
    /// Whether motorcycles may filter between slow or stopped vehicles.
    pub motorcycle_filtering: bool,
    /// Whether crossing away from a crosswalk is lawful (without right of way).
    pub jaywalking_lawful: bool,
}

impl UsJurisdictionData {
    /// The urban default in m/s.
    pub fn urban_mps(&self) -> f64 {
        self.urban_mph * MPH
    }
}

/// One mile per hour in metres per second.
pub const MPH: f64 = 0.447_04;

/// The fallback for an untagged motorway in every state preset.
pub const US_MOTORWAY_55: &str = "untagged urban motorway taken as 55 mph = 24.587 m/s, the \
    most common urban freeway posting; an urban freeway is posted and nearly always carries \
    maxspeed in OpenStreetMap, so this row is rarely reached";

/// The generic US urban default, for a state whose own statute this table does not quote.
pub const GENERIC_25: &str = "generic US urban default, 25 mph = 11.176 m/s: the most common \
    statutory limit in a business or residence district (e.g. CVC §22352, ARS §28-702.04, \
    Va. Code §46.2-874, N.J.S.A. 39:4-98); NOT checked against this state's code. A street \
    posted otherwise carries maxspeed in OpenStreetMap, and the tag wins";

macro_rules! j {
    ($key:literal, $name:literal, $mph:expr, $src:expr, $cited:expr) => {
        j!($key, $name, $mph, $src, $cited, false, false)
    };
    ($key:literal, $name:literal, $mph:expr, $src:expr, $cited:expr, $filter:expr, $walk:expr) => {
        UsJurisdictionData {
            key: $key,
            label: concat!("urban-us-", $key),
            name: $name,
            urban_mph: $mph,
            urban_source: $src,
            cited: $cited,
            motorcycle_filtering: $filter,
            jaywalking_lawful: $walk,
        }
    };
}

/// Every jurisdiction with an `urban-us-<key>` preset, in label order of the states and
/// then the cities. New York and Oregon are absent: their cities use the hand-built
/// `urban-us-nyc` and `urban-us-portland` presets.
#[rustfmt::skip]
pub const US_JURISDICTIONS: &[UsJurisdictionData] = &[
    j!("ak", "Alaska", 20.0, "Alaska: 20 mph in a business district, 25 mph in a residence district (13 AAC 02.275, checked 2026-10-07); the business-district value is taken for a downtown", true),
    j!("al", "Alabama", 30.0, "Alabama: 30 mph in any urban district (Code of Ala. §32-5A-171(1))", true),
    j!("ar", "Arkansas", 30.0, "Arkansas: 30 mph in any urban district (Ark. Code §27-51-201(b)(1))", true),
    j!("az", "Arizona", 25.0, "Arizona: 25 mph in a business or residence district (ARS §28-702.04(A)(2))", true, true, false),
    j!("ca", "California", 25.0, "California: 25 mph prima facie in a business or residence district (CVC §22352(a)(2)(A))", true, true, true),
    j!("co", "Colorado", 25.0, "Colorado: 25 mph in any business district (CRS §42-4-1101(4)(b))", true, true, false),
    j!("ct", "Connecticut", 25.0, GENERIC_25, false),
    j!("dc", "District of Columbia", 20.0, "District of Columbia: 20 mph on every street without a posted limit, DDOT's Vision Zero rule in force since 2020-06-01 (18 DCMR §2200; DDOT, \"20 MPH default speed limit\", checked 2026-10-07)", true),
    j!("de", "Delaware", 25.0, "Delaware: 25 mph in any business or residence district (21 Del. C. §4169(b)(1))", true),
    j!("fl", "Florida", 30.0, "Florida: 30 mph in business or residence districts (Fla. Stat. §316.183(2))", true),
    j!("ga", "Georgia", 30.0, "Georgia: 30 mph in any urban or residential district (O.C.G.A. §40-6-181(b)(1))", true),
    j!("hi", "Hawaii", 25.0, GENERIC_25, false),
    j!("ia", "Iowa", 20.0, "Iowa: 20 mph in any business district, 25 mph in a residence district (Iowa Code §321.285(2)); the business-district value is taken for a downtown", true),
    j!("id", "Idaho", 35.0, "Idaho: 35 mph in any urban district (Idaho Code §49-654(2)(a))", true),
    j!("il", "Illinois", 30.0, "Illinois: 30 mph in an urban district (625 ILCS 5/11-601(a))", true),
    j!("in", "Indiana", 30.0, "Indiana: 30 mph in an urban district (IC 9-21-5-2(a)(1))", true),
    j!("ks", "Kansas", 30.0, "Kansas: 30 mph in any urban district (K.S.A. 8-1558(a)(1))", true),
    j!("ky", "Kentucky", 35.0, "Kentucky: 35 mph on a highway in a business district (KRS §189.390(3)(c), checked 2026-10-07)", true),
    j!("la", "Louisiana", 25.0, GENERIC_25, false),
    j!("ma", "Massachusetts", 30.0, "Massachusetts: 30 mph inside a thickly settled or business district (M.G.L. c.90 §17)", true),
    j!("md", "Maryland", 30.0, "Maryland: 30 mph on a highway in a business district (Md. Transp. Code §21-801.1(b)(1))", true),
    j!("me", "Maine", 25.0, "Maine: 25 mph in a business or residence district or a compact area (29-A M.R.S. §2074(3))", true),
    j!("mi", "Michigan", 25.0, "Michigan: 25 mph in a business district (MCL §257.627(2)(a))", true),
    j!("mn", "Minnesota", 30.0, "Minnesota: 30 mph in an urban district (Minn. Stat. §169.14 subd. 2(a)(1))", true),
    j!("mo", "Missouri", 25.0, GENERIC_25, false),
    j!("ms", "Mississippi", 25.0, GENERIC_25, false),
    j!("mt", "Montana", 25.0, GENERIC_25, false, true, false),
    j!("nc", "North Carolina", 35.0, "North Carolina: 35 mph inside municipal corporations (N.C.G.S. §20-141(b)(1))", true),
    j!("nd", "North Dakota", 25.0, "North Dakota: 25 mph in any residential or business district (N.D.C.C. §39-09-02(1)(a))", true),
    j!("ne", "Nebraska", 20.0, "Nebraska: 20 mph in any business district, 25 mph in a residential district (Neb. Rev. Stat. §60-6,186, checked 2026-10-07); the business-district value is taken for a downtown", true),
    j!("nh", "New Hampshire", 30.0, "New Hampshire: 30 mph prima facie in a business or urban residence district (RSA 265:60 II, checked 2026-10-07)", true),
    j!("nj", "New Jersey", 25.0, "New Jersey: 25 mph in a business or residence district (N.J.S.A. 39:4-98(a))", true),
    j!("nm", "New Mexico", 25.0, GENERIC_25, false),
    j!("nv", "Nevada", 25.0, GENERIC_25, false),
    j!("oh", "Ohio", 25.0, "Ohio: 25 mph in all other portions of a municipal corporation (ORC §4511.21(B)(2))", true),
    j!("ok", "Oklahoma", 25.0, GENERIC_25, false),
    j!("pa", "Pennsylvania", 25.0, "Pennsylvania: 25 mph in a residence district (75 Pa.C.S. §3362(a)(1.1)); 35 mph in other urban districts (§3362(a)(1)), and Philadelphia and Pittsburgh post their downtown streets at 25 mph", true),
    j!("ri", "Rhode Island", 25.0, "Rhode Island: 25 mph in any business district (R.I. Gen. Laws §31-14-2(1))", true),
    j!("sc", "South Carolina", 30.0, "South Carolina: 30 mph in urban districts (S.C. Code §56-5-1520, checked 2026-10-07)", true),
    j!("sd", "South Dakota", 25.0, GENERIC_25, false),
    j!("tn", "Tennessee", 25.0, GENERIC_25, false),
    j!("tx", "Texas", 30.0, "Texas: 30 mph on a street in an urban district (Transp. Code §545.352(b)(1))", true),
    j!("ut", "Utah", 25.0, "Utah: 25 mph in any urban district (Utah Code §41-6a-603(1)(a)(ii))", true, true, false),
    j!("va", "Virginia", 25.0, "Virginia: 25 mph on highways in business and residence districts (Va. Code §46.2-874)", true, false, true),
    j!("vt", "Vermont", 25.0, GENERIC_25, false),
    j!("wa", "Washington", 25.0, "Washington: 25 mph on city and town streets (RCW 46.61.400(2)(a))", true),
    j!("wi", "Wisconsin", 25.0, "Wisconsin: 25 mph in any zone within a corporate limit (Wis. Stat. §346.57(4)(e))", true),
    j!("wv", "West Virginia", 25.0, "West Virginia: 25 mph in any business or residence district (W. Va. Code §17C-6-1(b)(1))", true),
    j!("wy", "Wyoming", 30.0, "Wyoming: 30 mph in any urban district (W.S. 31-5-301(b)(i))", true),
    // Cities that set their own default under a state's enabling law.
    j!("boston", "Boston, Massachusetts", 25.0, "Boston: 25 mph citywide default on thickly settled streets, adopted 2017-01-09 under M.G.L. c.90 §17C", true),
];

/// The jurisdiction a preset label names, by `urban-us-<key>`.
pub fn by_label(label: &str) -> Option<usize> {
    US_JURISDICTIONS.iter().position(|j| j.label == label)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::osm::HighwayPreset;

    #[test]
    fn every_preset_is_spelt_the_same_by_serde_and_by_its_label() {
        for p in HighwayPreset::ALL {
            let wire = serde_json::to_string(&p).expect("serialises");
            assert_eq!(wire, format!("\"{}\"", p.label()), "{p:?}");
            assert_eq!(
                serde_json::from_str::<HighwayPreset>(&wire).expect("parses"),
                p
            );
            assert_eq!(HighwayPreset::parse(p.label()), Some(p));
        }
        for j in US_JURISDICTIONS {
            let p = HighwayPreset::parse(j.label).expect("a preset per row");
            assert_eq!(p.us_jurisdiction().map(|d| d.key), Some(j.key));
        }
    }

    #[test]
    fn every_state_and_dc_has_a_preset_or_a_hand_built_one() {
        // 48 states here, New York and Oregon by hand, and the District of Columbia.
        let states = US_JURISDICTIONS.iter().filter(|j| j.key.len() == 2).count();
        assert_eq!(states, 49);
        let mut labels: Vec<&str> = US_JURISDICTIONS.iter().map(|j| j.label).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), US_JURISDICTIONS.len(), "labels are unique");
        for j in US_JURISDICTIONS {
            assert!((15.0..=35.0).contains(&j.urban_mph), "{}", j.key);
            assert!(!j.urban_source.is_empty());
            assert_eq!(j.cited, j.urban_source != GENERIC_25, "{}", j.key);
        }
    }
}
