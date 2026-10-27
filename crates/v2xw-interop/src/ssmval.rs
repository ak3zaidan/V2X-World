//! Surrogate safety measures against SUMO's SSM device (adoption plan WP-11).
//!
//! The cross-validation's SUMO runs carry SUMO's own SSM device on every vehicle, at its
//! documented defaults for the measures this engine also computes (TTC 3.0 s, DRAC 3.0
//! m/s², PPET 2.0 s, MDRAC 3.4 m/s², range 50 m, extra time 5 s, MDRAC reaction time 1 s;
//! SUMO documentation, "Simulation/Output/SSM Device"). PET is left out on both sides: the
//! engine's PET is the cell and conflict-zone measure of `v2xw-metrics` and
//! `v2xw-trafficval`, not a per-encounter one.
//!
//! The same SUMO trajectories are read by this engine's [`v2xw_metrics::ssm`] measures in
//! the cross-validation [`crate::xval::Observer`], so the comparison isolates the measure
//! from the driving: **one set of trajectories, two SSM implementations**. The native run's
//! conflicts are reported beside them, which compares the driving.
//!
//! The comparison is conflicts by encounter type whose minimum TTC is below 3.0 s, with the
//! type the one at the minimum TTC, counted over encounters that began inside the
//! measurement window. The tolerance is stated before the runs: for every type SUMO counts
//! at least [`MIN_COUNT`] of, this engine's count on the same trajectories is within a
//! factor [`TOLERANCE`] of SUMO's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use v2xw_metrics::ssm::{Conflict, EncounterType, SSM_TTC_S};

/// The ratio band the per-type counts must fall in: `1/TOLERANCE ..= TOLERANCE`.
///
/// Stated before the first run. SUMO extrapolates along each vehicle's route and finds
/// crossing points from the junction's lane geometry; this engine extrapolates straight
/// at the current heading (Laureshyn et al. 2010). A factor of 1.5 is the band inside which
/// the two methods would be counting the same conflicts with a different boundary.
pub const TOLERANCE: f64 = 1.5;
/// A type with fewer SUMO conflicts than this is reported and not held.
pub const MIN_COUNT: usize = 20;

/// The SUMO options that equip every vehicle with the SSM device, writing to `file`.
pub fn sumo_args(file: &Path) -> Vec<String> {
    [
        "--device.ssm.probability",
        "1",
        "--device.ssm.deterministic",
        "true",
        "--device.ssm.measures",
        "TTC DRAC PPET MDRAC",
        "--device.ssm.thresholds",
        "3.0 3.0 2.0 3.4",
        "--device.ssm.range",
        "50",
        "--device.ssm.extratime",
        "5",
        "--device.ssm.mdrac.prt",
        "1",
        "--device.ssm.file",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .chain(std::iter::once(file.display().to_string()))
    .collect()
}

/// One `<conflict>` of SUMO's SSM output.
#[derive(Debug, Clone, PartialEq)]
pub struct SumoConflict {
    /// `begin`, s.
    pub begin_s: f64,
    /// `end`, s.
    pub end_s: f64,
    /// The ego vehicle's id.
    pub ego: String,
    /// The foe's id.
    pub foe: String,
    /// The minimum TTC's value and encounter-type code.
    pub min_ttc: Option<(f64, u32)>,
    /// The maximum DRAC.
    pub max_drac: Option<f64>,
    /// The minimum PPET.
    pub min_ppet: Option<f64>,
    /// The maximum MDRAC.
    pub max_mdrac: Option<f64>,
}

fn attr(e: &quick_xml::events::BytesStart<'_>, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.into_inner() == key)
        .map(|a| a.value.to_string())
}

fn num(e: &quick_xml::events::BytesStart<'_>, key: &str) -> Option<f64> {
    attr(e, key).and_then(|v| v.parse::<f64>().ok()).filter(|v| v.is_finite())
}

/// Parses SUMO's SSM output.
///
/// # Errors
/// Malformed XML.
pub fn parse_sumo_ssm(xml: &str) -> Result<Vec<SumoConflict>, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = Vec::new();
    let mut open: Option<SumoConflict> = None;
    loop {
        let ev = reader.read_event().map_err(|e| format!("ssm xml: {e}"))?;
        match ev {
            quick_xml::events::Event::Eof => break,
            quick_xml::events::Event::Start(ref e) | quick_xml::events::Event::Empty(ref e) => {
                let empty = matches!(ev, quick_xml::events::Event::Empty(_));
                match e.name().into_inner() {
                    "conflict" => {
                        let c = SumoConflict {
                            begin_s: num(e, "begin").unwrap_or(0.0),
                            end_s: num(e, "end").unwrap_or(0.0),
                            ego: attr(e, "ego").unwrap_or_default(),
                            foe: attr(e, "foe").unwrap_or_default(),
                            min_ttc: None,
                            max_drac: None,
                            min_ppet: None,
                            max_mdrac: None,
                        };
                        if empty {
                            out.push(c);
                        } else {
                            open = Some(c);
                        }
                    }
                    "minTTC" => {
                        if let Some(c) = open.as_mut() {
                            let ty = attr(e, "type").and_then(|v| v.parse::<u32>().ok());
                            c.min_ttc = num(e, "value").zip(ty);
                        }
                    }
                    "maxDRAC" => {
                        if let Some(c) = open.as_mut() {
                            c.max_drac = num(e, "value");
                        }
                    }
                    "minPPET" => {
                        if let Some(c) = open.as_mut() {
                            c.min_ppet = num(e, "value");
                        }
                    }
                    "maxMDRAC" => {
                        if let Some(c) = open.as_mut() {
                            c.max_mdrac = num(e, "value");
                        }
                    }
                    _ => {}
                }
            }
            quick_xml::events::Event::End(ref e) if e.name().into_inner() == "conflict" => {
                if let Some(c) = open.take() {
                    out.push(c);
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Conflicts with a minimum TTC under 3.0 s, by type, plus every logged conflict.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TypeCounts {
    /// By type.
    pub by_type: BTreeMap<EncounterType, usize>,
    /// Every logged conflict (any measure past its threshold).
    pub logged: usize,
    /// SUMO conflicts logged twice (once by each vehicle's device) and counted once.
    pub duplicates: usize,
    /// Logged TTC conflicts whose type code has no main type (SUMO code 4, adjacent lanes).
    pub untyped: usize,
}

impl TypeCounts {
    /// The count of `ty`.
    #[must_use]
    pub fn of(&self, ty: EncounterType) -> usize {
        self.by_type.get(&ty).copied().unwrap_or(0)
    }
}

fn in_window(begin: f64, window: (f64, f64)) -> bool {
    begin >= window.0 && begin < window.1
}

/// This engine's conflict log, counted.
#[must_use]
pub fn count_engine(log: &[Conflict], window: (f64, f64)) -> TypeCounts {
    let mut out = TypeCounts::default();
    for c in log.iter().filter(|c| in_window(c.begin_s, window)) {
        out.logged += 1;
        if c.ttc_below(SSM_TTC_S) {
            *out.by_type.entry(c.ty).or_default() += 1;
        }
    }
    out
}

/// SUMO's conflicts, counted once per pair and begin (each equipped vehicle logs an
/// encounter as ego, so a pair of equipped vehicles can appear twice).
#[must_use]
pub fn count_sumo(log: &[SumoConflict], window: (f64, f64)) -> TypeCounts {
    let mut out = TypeCounts::default();
    let mut seen: BTreeSet<(String, String, i64)> = BTreeSet::new();
    for c in log.iter().filter(|c| in_window(c.begin_s, window)) {
        let (a, b) = if c.ego <= c.foe {
            (c.ego.clone(), c.foe.clone())
        } else {
            (c.foe.clone(), c.ego.clone())
        };
        if !seen.insert((a, b, (c.begin_s * 10.0).round() as i64)) {
            out.duplicates += 1;
            continue;
        }
        out.logged += 1;
        if let Some((v, code)) = c.min_ttc
            && v < SSM_TTC_S
        {
            match EncounterType::from_sumo_code(code) {
                Some(ty) => *out.by_type.entry(ty).or_default() += 1,
                None => out.untyped += 1,
            }
        }
    }
    out
}

/// Whether `ours` is within the tolerance of `sumo` for every type SUMO counts at least
/// [`MIN_COUNT`] of; with the failing types.
#[must_use]
pub fn agreement(ours: &TypeCounts, sumo: &TypeCounts) -> (bool, Vec<String>) {
    let mut bad = Vec::new();
    for ty in EncounterType::ALL {
        let s = sumo.of(ty);
        if s < MIN_COUNT {
            continue;
        }
        let r = ours.of(ty) as f64 / s as f64;
        if !(1.0 / TOLERANCE..=TOLERANCE).contains(&r) {
            bad.push(format!("{} {:.2}x", ty.label(), r));
        }
    }
    (bad.is_empty(), bad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::ids::ActorId;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<SSMLog>
    <conflict begin="200.00" end="203.40" ego="t1" foe="t2">
        <conflictPoint>
        </conflictPoint>
        <minTTC time="201.00" position="1,2" type="9" value="1.20" speed="5"/>
        <maxDRAC time="201.00" position="1,2" type="9" value="2.10" speed="5"/>
        <minPPET time="NA" position="NA" type="NA" value="NA" speed="NA"/>
        <maxMDRAC time="201.00" position="1,2" type="9" value="3.90" speed="5"/>
    </conflict>
    <conflict begin="200.00" end="203.40" ego="t2" foe="t1">
        <minTTC time="201.00" position="1,2" type="9" value="1.20" speed="5"/>
    </conflict>
    <conflict begin="250.00" end="252.00" ego="t3" foe="t4">
        <minTTC time="251.00" position="1,2" type="2" value="2.50" speed="5"/>
    </conflict>
    <conflict begin="260.00" end="262.00" ego="t3" foe="t5">
        <minTTC time="NA" position="NA" type="NA" value="NA" speed="NA"/>
        <minPPET time="261.00" position="1,2" type="9" value="1.10" speed="5"/>
    </conflict>
    <conflict begin="10.00" end="12.00" ego="t6" foe="t7">
        <minTTC time="11.00" position="1,2" type="1" value="0.50" speed="5"/>
    </conflict>
</SSMLog>"#;

    #[test]
    fn sumos_ssm_output_is_read_and_counted_once_per_pair() {
        let log = parse_sumo_ssm(XML).unwrap();
        assert_eq!(log.len(), 5);
        assert_eq!(log[0].min_ttc, Some((1.2, 9)));
        assert_eq!(log[0].max_drac, Some(2.1));
        assert_eq!(log[0].min_ppet, None);
        assert_eq!(log[0].max_mdrac, Some(3.9));
        assert_eq!(log[3].min_ttc, None);
        assert_eq!(log[3].min_ppet, Some(1.1));
        let n = count_sumo(&log, (180.0, 900.0));
        assert_eq!(n.duplicates, 1);
        assert_eq!(n.logged, 3, "the warm-up conflict at 10 s is outside the window");
        assert_eq!(n.of(EncounterType::Crossing), 1);
        assert_eq!(n.of(EncounterType::Following), 1);
        assert_eq!(n.of(EncounterType::Merging), 0);
    }

    #[test]
    fn the_engine_log_is_counted_by_type_at_the_min_ttc() {
        let c = |ty, ttc: Option<f64>, begin| Conflict {
            a: ActorId::new(1),
            b: ActorId::new(2),
            begin_s: begin,
            end_s: begin + 1.0,
            ty,
            min_ttc: ttc.map(|v| (begin, v)),
            max_drac: None,
            max_mdrac: None,
            min_ppet: None,
        };
        let log = vec![
            c(EncounterType::Crossing, Some(1.0), 200.0),
            c(EncounterType::Crossing, Some(3.5), 200.0),
            c(EncounterType::Following, Some(2.9), 300.0),
            c(EncounterType::Following, Some(1.0), 100.0),
        ];
        let n = count_engine(&log, (180.0, 900.0));
        assert_eq!(n.logged, 3);
        assert_eq!(n.of(EncounterType::Crossing), 1);
        assert_eq!(n.of(EncounterType::Following), 1);
    }

    #[test]
    fn agreement_holds_inside_the_band_and_fails_outside() {
        let mut s = TypeCounts::default();
        s.by_type.insert(EncounterType::Following, 100);
        s.by_type.insert(EncounterType::Crossing, 5);
        let mut o = TypeCounts::default();
        o.by_type.insert(EncounterType::Following, 140);
        o.by_type.insert(EncounterType::Crossing, 50);
        assert!(agreement(&o, &s).0, "crossing is below MIN_COUNT, following 1.4x");
        o.by_type.insert(EncounterType::Following, 160);
        assert!(!agreement(&o, &s).0);
        o.by_type.insert(EncounterType::Following, 60);
        assert!(!agreement(&o, &s).0);
    }
}
