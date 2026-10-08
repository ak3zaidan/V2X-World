//! Health bands: is a run's energy result physically believable?
//!
//! A band is not a target. It is the range a correct model on a sensible fleet lands in,
//! from published figures, so that a result outside it points at a defect — a unit slip, a
//! wrong class, a fleet section that says something it did not mean, a trajectory with
//! teleports in it — before anyone draws a conclusion from it. Each band carries its
//! rationale and source; [`assess`] grades a run's latest samples against them:
//!
//! * [`Health::Ok`] — inside the band;
//! * [`Health::Warn`] — outside the band but inside the wider plausibility range: possible
//!   (an extreme fleet, a congested grid) but worth a look;
//! * [`Health::Fail`] — outside the plausibility range: almost certainly a defect;
//! * [`Health::NotMeasured`] — no sample, or an insufficient one. Not measured is not
//!   healthy, and the report says so rather than passing it.

use v2xw_core::card::{Source, SourceKind};
use v2xw_metrics::def::{Dim, DimValue, MetricSample};

use crate::cite;

/// A graded result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Health {
    /// Inside the band.
    Ok,
    /// Outside the band, inside the plausibility range.
    Warn,
    /// Outside the plausibility range.
    Fail,
    /// No usable sample.
    NotMeasured,
}

/// One band.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// The metric it grades.
    pub metric: &'static str,
    /// The class it applies to, or `None` for the headline (all classes).
    pub class: Option<&'static str>,
    /// The healthy range, inclusive.
    pub ok: (f64, f64),
    /// The plausible range, inclusive; outside it is a failure.
    pub plausible: (f64, f64),
    /// Why these numbers.
    pub rationale: &'static str,
    /// Where they come from.
    pub source: Source,
}

/// One band's verdict on a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The band.
    pub band: Band,
    /// The value it graded, if there was one.
    pub value: Option<f64>,
    /// The grade.
    pub health: Health,
}

/// Every band this crate ships.
#[must_use]
pub fn bands() -> Vec<Band> {
    let eea = cite::src(
        SourceKind::Dataset,
        "European Environment Agency (2011) Monitoring CO2 emissions from new passenger cars \
         in the EU: summary of data for 2010 - new-car fleet average 140.3 g/km (NEDC type \
         approval)",
        "the type-approval figure under-states real driving; urban stop-and-go is well above it",
    );
    let epa_leaf = cite::src(
        SourceKind::Dataset,
        "US DOE/EPA fueleconomy.gov, 2013 Nissan Leaf: 29 kWh/100 mi combined (18 kWh/100 km, \
         wall-to-wheels)",
        "battery-side consumption is lower by the charger's losses",
    );
    let euro = cite::src(
        SourceKind::Standard,
        "Directive 98/69/EC (Euro 4: diesel car NOx 0.25 g/km, petrol 0.08 g/km) and \
         Regulation (EC) No 715/2007 Annex I (Euro 5 diesel 0.18 g/km, Euro 6 diesel 0.08 g/km)",
        "type-approval limits; real-world urban NOx of diesel cars is a multiple of them",
    );
    vec![
        Band {
            metric: "co2_per_km",
            class: Some("passenger"),
            ok: (90.0, 350.0),
            plausible: (40.0, 700.0),
            rationale: "a petrol or diesel car's tailpipe CO2 from a frugal cruise (about \
                        100 g/km) to dense urban stop-and-go (idling at 0.6-1 g/s dominates \
                        below 15 km/h); an EV share lowers the fleet figure towards the \
                        lower edge. Under 40 or over 700 g/km is a unit or trajectory defect",
            source: eea.clone(),
        },
        Band {
            metric: "fuel_per_100km",
            class: Some("passenger"),
            ok: (4.0, 15.0),
            plausible: (2.0, 30.0),
            rationale: "the CO2 band through the carbon content of petrol (EPA: 8,887 g CO2 per \
                        gallon, about 23.5 g/km per L/100 km)",
            source: cite::epa_co2_per_gallon(),
        },
        Band {
            metric: "ev_energy_per_100km",
            class: Some("passenger"),
            ok: (9.0, 25.0),
            plausible: (3.0, 45.0),
            rationale: "a compact EV's label figure is 18 kWh/100 km at the wall; at the \
                        battery, urban driving with regeneration runs from about half that \
                        to its label value plus HVAC",
            source: epa_leaf,
        },
        Band {
            metric: "nox_per_km",
            class: Some("passenger"),
            ok: (0.0, 1.0),
            plausible: (0.0, 3.0),
            rationale: "zero to a few times the diesel type-approval limits; a petrol fleet \
                        sits near zero",
            source: euro,
        },
        Band {
            metric: "ev_soc_min",
            class: None,
            ok: (0.05, 1.0),
            plausible: (0.0, 1.0),
            rationale: "below 5 % an EV would be looking for a charger rather than driving the \
                        scenario's trips",
            source: cite::src(
                SourceKind::Paper,
                "Kurczveil T. et al. (2014), battery device state of charge",
                "",
            ),
        },
        Band {
            metric: "energy_unmodelled",
            class: None,
            ok: (0.0, 0.0),
            plausible: (0.0, f64::INFINITY),
            rationale: "every motor vehicle the run moved had a class in the selected model; \
                        otherwise totals exclude some vehicles (pick energy/emission/hbefa3 for \
                        the widest coverage)",
            source: cite::krajzewicz_2015(),
        },
    ]
}

fn grade(b: &Band, v: f64) -> Health {
    let inside = |(lo, hi): (f64, f64)| v >= lo && v <= hi;
    if inside(b.ok) {
        Health::Ok
    } else if inside(b.plausible) {
        Health::Warn
    } else {
        Health::Fail
    }
}

/// Grades the latest sample of every banded metric among `samples`.
#[must_use]
pub fn assess(samples: &[MetricSample]) -> Vec<Verdict> {
    bands()
        .into_iter()
        .map(|band| {
            let latest = samples
                .iter()
                .filter(|s| s.metric == band.metric)
                .filter(|s| match band.class {
                    Some(c) => s.dims.get(&Dim::Class) == Some(&DimValue::label(c)),
                    None => s.dims.is_empty(),
                })
                .max_by_key(|s| s.t);
            let value = latest.and_then(|s| s.value.point());
            let health = value.map_or(Health::NotMeasured, |v| grade(&band, v));
            Verdict {
                band,
                value,
                health,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_metrics::def::{Dims, SampleValue};
    use v2xw_metrics::stats::ratio_of_sums;

    fn sample(metric: &str, class: Option<&str>, v: f64, t: u64) -> MetricSample {
        let def = crate::provider::defs()
            .into_iter()
            .find(|d| d.name == metric)
            .unwrap();
        let mut dims = Dims::new();
        if let Some(c) = class {
            dims.insert(Dim::Class, DimValue::label(c));
        }
        MetricSample::new(
            &def,
            t,
            dims,
            SampleValue::Ratio(ratio_of_sums(v, 1.0, 5, 1)),
        )
    }

    #[test]
    fn every_band_is_ordered_and_the_grades_fall_where_they_should() {
        for b in bands() {
            assert!(
                b.ok.0 <= b.ok.1 && b.plausible.0 <= b.ok.0 && b.ok.1 <= b.plausible.1,
                "{b:?}"
            );
        }
        let s = vec![
            sample("co2_per_km", Some("passenger"), 900.0, 1),
            sample("co2_per_km", Some("passenger"), 180.0, 2),
            sample("fuel_per_100km", Some("passenger"), 18.0, 2),
            sample("nox_per_km", Some("passenger"), 5.0, 2),
        ];
        let v = assess(&s);
        let of = |m: &str| v.iter().find(|x| x.band.metric == m).unwrap().health;
        assert_eq!(of("co2_per_km"), Health::Ok, "the latest sample is graded");
        assert_eq!(of("fuel_per_100km"), Health::Warn);
        assert_eq!(of("nox_per_km"), Health::Fail);
        assert_eq!(of("ev_energy_per_100km"), Health::NotMeasured);
    }
}
