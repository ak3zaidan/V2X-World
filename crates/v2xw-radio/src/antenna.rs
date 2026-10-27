//! Vehicle antenna patterns: `antenna/vehicle/tr37885-option1`.
//!
//! 3GPP TR 37.885 V15.3.0 §6.1.4, Tables 6.1.4-8 and 6.1.4-9, Option 1 at 6 GHz (the
//! frequency the TR evaluates 5.9 GHz at). Where the antenna sits depends on the vehicle
//! type:
//!
//! * **Type 2** (a passenger vehicle with its antenna on the roof, 1.6 m): one rooftop
//!   panel, omnidirectional in azimuth, `A_E,H(φ) = 0`.
//! * **Types 1 and 3** (a passenger vehicle with a low antenna, 0.75 m, and a truck or bus,
//!   3 m): two panels, at the front (bearing 0°) and the rear (180°), each with
//!   `A_E,H(φ) = −min{12·(φ/φ3dB)², A_m}`, `φ3dB` = 120°, `A_m` = 20 dB.
//!
//! Every element has the vertical pattern `A_E,V(θ) = −min{12·((θ − 90°)/θ3dB)², SLA_V}`,
//! `θ3dB` = 90°, `SLA_V` = 20 dB (θ the zenith angle, 90° the horizon), the two combine
//! as `A(θ, φ) = −min{−(A_E,V + A_E,H), A_m}`, and the element's maximum directional gain
//! is 3 dBi — the scalar gain the link budget already carries, so this module returns the
//! pattern *relative* to it, 0 dB at best.
//!
//! How the two panels of a Type 1 or 3 vehicle are combined is "up to proponents
//! decision" (Table 6.1.4-9, TXRU mapping). This model takes the better panel toward the
//! other end of the link — selection, which is what a receiver with one chain per panel
//! does — and says so on the card. A truck heard from its side is therefore about 7 dB
//! down on the same truck heard from ahead or behind.

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};

use crate::types::ActorClass;

/// Where a vehicle's V2X antenna is mounted, which decides its pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AntennaMount {
    /// One rooftop panel, omnidirectional in azimuth (TR 37.885 vehicle Type 2).
    Rooftop,
    /// Two panels, front and rear (TR 37.885 vehicle Types 1 and 3).
    FrontRear,
    /// No pattern: the scalar gain in every direction (a handheld, a mast).
    Isotropic,
}

impl AntennaMount {
    /// The mount TR 37.885 gives a class: a car or van is Type 2 (rooftop), a truck or bus
    /// Type 3 (front and rear), a motorcycle a rooftop-like single antenna; a pedestrian,
    /// a bicycle and a roadside unit carry no vehicle pattern.
    #[must_use]
    pub const fn for_class(class: ActorClass) -> Self {
        match class {
            ActorClass::Car | ActorClass::Van | ActorClass::Motorcycle => AntennaMount::Rooftop,
            ActorClass::Truck => AntennaMount::FrontRear,
            ActorClass::Bicycle
            | ActorClass::Pedestrian
            | ActorClass::Rsu
            | ActorClass::BaseStation => AntennaMount::Isotropic,
        }
    }

    /// The scenario's name for it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            AntennaMount::Rooftop => "rooftop",
            AntennaMount::FrontRear => "front-rear",
            AntennaMount::Isotropic => "isotropic",
        }
    }

    /// The mount a scenario names.
    #[must_use]
    pub fn from_label(s: &str) -> Option<Self> {
        [
            AntennaMount::Rooftop,
            AntennaMount::FrontRear,
            AntennaMount::Isotropic,
        ]
        .into_iter()
        .find(|m| m.label() == s)
    }

    /// The pattern toward a direction, dB relative to the element's maximum gain.
    ///
    /// `azimuth_rad` is the direction's bearing relative to the vehicle's heading (0
    /// ahead, ±π behind); `elevation_rad` its angle above the horizon.
    #[must_use]
    pub fn relative_gain_db(self, azimuth_rad: f64, elevation_rad: f64) -> f64 {
        if self == AntennaMount::Isotropic {
            return 0.0;
        }
        let theta_deg = 90.0 - elevation_rad * DEG_PER_RAD;
        let a_v = -(12.0 * sq((theta_deg - 90.0) / THETA_3DB_DEG)).min(SLA_V_DB);
        let a_h = match self {
            AntennaMount::Rooftop | AntennaMount::Isotropic => 0.0,
            AntennaMount::FrontRear => {
                let phi = wrap_deg(azimuth_rad * DEG_PER_RAD);
                let panel = |bearing: f64| {
                    let off = wrap_deg(phi - bearing);
                    -(12.0 * sq(off / PHI_3DB_DEG)).min(A_M_DB)
                };
                panel(0.0).max(panel(180.0))
            }
        };
        -(-(a_v + a_h)).min(A_M_DB)
    }
}

/// `θ3dB`, the vertical half-power beamwidth at 6 GHz, degrees.
pub const THETA_3DB_DEG: f64 = 90.0;
/// `SLA_V`, the vertical side-lobe attenuation at 6 GHz, dB.
pub const SLA_V_DB: f64 = 20.0;
/// `φ3dB`, the horizontal half-power beamwidth of a Type 1 or 3 panel at 6 GHz, degrees.
pub const PHI_3DB_DEG: f64 = 120.0;
/// `A_m`, the maximum attenuation at 6 GHz, dB.
pub const A_M_DB: f64 = 20.0;

const DEG_PER_RAD: f64 = 180.0 / core::f64::consts::PI;

fn sq(x: f64) -> f64 {
    x * x
}

/// An angle in degrees folded into (−180, 180].
fn wrap_deg(a: f64) -> f64 {
    let mut x = a % 360.0;
    if x > 180.0 {
        x -= 360.0;
    } else if x <= -180.0 {
        x += 360.0;
    }
    x
}

/// The model's id.
pub const ANTENNA_ID: &str = "antenna/vehicle/tr37885-option1";

/// The card of the vehicle antenna patterns.
#[must_use]
pub fn card() -> ModelCard {
    let tr = Source {
        kind: SourceKind::Standard,
        reference: "3GPP TR 37.885 V15.3.0 (2019-06) §6.1.4, Tables 6.1.4-8 and 6.1.4-9 \
                    (Option 1, 6 GHz), via ATIS.3GPP.37.885.V1530"
            .to_string(),
        accessed: Some("2026-09-29".to_string()),
        note: None,
    };
    let mut card = ModelCard::new(
        ANTENNA_ID,
        Family::Propagation,
        "1.0.0",
        "Vehicle antenna patterns from TR 37.885 Option 1: a rooftop panel omnidirectional \
         in azimuth on cars and vans, front and rear panels on trucks and buses, and the \
         vertical pattern of every element, relative to the element's 3 dBi maximum.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new(
            "vertical",
            "A_E,V(θ) = −min{12·((θ − 90°)/θ3dB)², SLA_V}, θ3dB = 90°, SLA_V = 20 dB",
        ),
        Equation::new(
            "horizontal",
            "rooftop: A_E,H = 0; front-rear: A_E,H(φ) = −min{12·(φ/φ3dB)², A_m} per panel at \
             0° and 180°, φ3dB = 120°, A_m = 20 dB, the better panel taken",
        ),
        Equation::new("combined", "A(θ, φ) = −min{−(A_E,V + A_E,H), A_m}"),
    ];
    card.parameters = vec![
        Parameter::new(
            "theta_3db_deg",
            "deg",
            serde_json::json!(THETA_3DB_DEG),
            tr.clone(),
        ),
        Parameter::new("sla_v_db", "dB", serde_json::json!(SLA_V_DB), tr.clone()),
        Parameter::new(
            "phi_3db_deg",
            "deg",
            serde_json::json!(PHI_3DB_DEG),
            tr.clone(),
        ),
        Parameter::new("a_m_db", "dB", serde_json::json!(A_M_DB), tr.clone()),
    ];
    card.assumptions = vec![
        "A front-rear vehicle uses the better of its two panels toward the other end \
         (selection); TR 37.885 leaves the TXRU mapping to the proponent."
            .to_string(),
        "The class-to-type mapping: car, van and motorcycle are Type 2 (rooftop), truck \
         and bus Type 3 (front and rear)."
            .to_string(),
    ];
    card.limitations = vec![
        "No measured installed pattern: the roof's own shaping (ripple of a few dB, the \
         5GAA P-190033 Figure 10 patterns) is not modelled."
            .to_string(),
    ];
    card.sources = vec![tr.clone()];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![tr],
        tests: vec!["the_patterns_are_tr37885s".to_string()],
    };
    card.determinism = Determinism::default();
    card
}

// =========================================================================================
// `antenna/sampled` — a measured installed pattern
// =========================================================================================

/// The sampled-pattern model's id.
pub const SAMPLED_ID: &str = "antenna/sampled";

/// The most samples a sampled pattern may carry: a 1° azimuth by 1° elevation grid over
/// the whole sphere is 65,160, so this is generous for any measured table.
pub const MAX_SAMPLES: usize = 100_000;

/// A measured antenna pattern from a table of samples (`antenna/sampled`).
///
/// Re-implemented from the description in Eckhoff, Brummer and Sommer, "On the Impact of
/// Antenna Patterns on VANET Simulation", IEEE VNC 2016, §III: the pattern is sampled in
/// the azimuth plane, each angle carries a positive or negative gain, and values between
/// samples are interpolated. Here the interpolation is linear in dB and wraps round the
/// circle; an optional elevation column turns the table into rows of azimuth cuts that are
/// interpolated linearly in elevation and held at the outermost row beyond it. Measured
/// installed patterns of this kind are Kornek et al., IEEE ICC Workshops 2010. Veins'
/// `SampledAntenna1D` was read for behaviour only and no code was copied.
///
/// The gains are dB **relative to** the device's scalar `antenna_gain_dbi`, which the
/// link budget already carries, as the TR 37.885 patterns above are; a table in absolute
/// dBi is used as such with `antenna_gain_dbi: 0`. A table of zeros is therefore exactly
/// the isotropic antenna.
#[derive(Debug, Clone, PartialEq)]
pub struct SampledAntenna {
    /// Rows in ascending elevation (degrees above the horizon), each a cut of
    /// `(azimuth in [0, 360), gain dB)` in ascending azimuth. A table without an
    /// elevation column is one row at elevation 0.
    rows: Vec<(f64, Vec<(f64, f64)>)>,
}

impl SampledAntenna {
    /// A pattern from azimuth samples alone: `(azimuth °, gain dB)`, the azimuth measured
    /// from the vehicle's heading, positive counter-clockwise seen from above (toward the
    /// left), the engine's convention.
    ///
    /// # Errors
    /// If there is no sample, a value is not finite, or two samples fall on one angle.
    pub fn from_azimuth_samples(samples: &[(f64, f64)]) -> Result<Self, String> {
        Self::from_points(samples.iter().map(|&(a, g)| (0.0, a, g)).collect())
    }

    /// A pattern from a CSV table.
    ///
    /// The first line that is not blank and does not start with `#` is the header. It
    /// names the columns, in any order: `gain_db`; one of `azimuth_deg` (counter-clockwise
    /// from the heading, the engine's convention) or `azimuth_cw_deg` (clockwise, the way
    /// many measured patterns and Veins' `SampledAntenna1D` number their samples); and
    /// optionally `elevation_deg` (above the horizon, −90 to 90).
    ///
    /// # Errors
    /// A missing or unknown column, a row of the wrong width, a value that is not a
    /// finite number, an elevation outside ±90°, two samples on one angle, or more than
    /// [`MAX_SAMPLES`] rows.
    pub fn from_csv(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.trim()))
            .filter(|(_, l)| !l.is_empty() && !l.starts_with('#'));
        let (_, header) = lines.next().ok_or("the table is empty")?;
        let names: Vec<String> = header
            .split(',')
            .map(|c| c.trim().to_ascii_lowercase())
            .collect();
        let col = |n: &str| names.iter().position(|c| c == n);
        for n in &names {
            if !matches!(
                n.as_str(),
                "azimuth_deg" | "azimuth_cw_deg" | "elevation_deg" | "gain_db"
            ) {
                return Err(format!(
                    "unknown column '{n}' (the columns are azimuth_deg or azimuth_cw_deg, \
                     optionally elevation_deg, and gain_db)"
                ));
            }
        }
        let (az_col, clockwise) = match (col("azimuth_deg"), col("azimuth_cw_deg")) {
            (Some(c), None) => (c, false),
            (None, Some(c)) => (c, true),
            (Some(_), Some(_)) => {
                return Err("name azimuth_deg or azimuth_cw_deg, not both".to_string());
            }
            (None, None) => return Err("no azimuth_deg or azimuth_cw_deg column".to_string()),
        };
        let gain_col = col("gain_db").ok_or("no gain_db column")?;
        let elev_col = col("elevation_deg");
        let mut points = Vec::new();
        for (line_no, line) in lines {
            let cells: Vec<&str> = line.split(',').map(str::trim).collect();
            if cells.len() != names.len() {
                return Err(format!(
                    "line {line_no}: {} values for {} columns",
                    cells.len(),
                    names.len()
                ));
            }
            let num = |c: usize| -> Result<f64, String> {
                cells[c]
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("line {line_no}: '{}' is not a number", cells[c]))
            };
            let az = num(az_col)?;
            let az = if clockwise { -az } else { az };
            let el = match elev_col {
                Some(c) => num(c)?,
                None => 0.0,
            };
            points.push((el, az, num(gain_col)?));
            if points.len() > MAX_SAMPLES {
                return Err(format!("more than {MAX_SAMPLES} samples"));
            }
        }
        Self::from_points(points)
    }

    fn from_points(points: Vec<(f64, f64, f64)>) -> Result<Self, String> {
        if points.is_empty() {
            return Err("the table has no samples".to_string());
        }
        let mut rows: Vec<(f64, Vec<(f64, f64)>)> = Vec::new();
        for (el, az, g) in points {
            if !(el.is_finite() && az.is_finite() && g.is_finite()) {
                return Err("a sample is not a finite number".to_string());
            }
            if !(-90.0..=90.0).contains(&el) {
                return Err(format!("elevation {el}° is outside ±90°"));
            }
            let az = wrap_360(az);
            match rows.iter_mut().find(|(e, _)| *e == el) {
                Some((_, cut)) => cut.push((az, g)),
                None => rows.push((el, vec![(az, g)])),
            }
        }
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (el, cut) in &mut rows {
            cut.sort_by(|a, b| a.0.total_cmp(&b.0));
            if let Some(w) = cut.windows(2).find(|w| w[0].0 == w[1].0) {
                return Err(format!(
                    "two samples at azimuth {}° (elevation {el}°)",
                    w[0].0
                ));
            }
        }
        Ok(Self { rows })
    }

    /// How many samples the table holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.iter().map(|(_, c)| c.len()).sum()
    }

    /// Whether the table is empty (never, once built).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The pattern toward a direction, dB relative to the device's scalar gain, for an
    /// antenna mounted `rotation_rad` counter-clockwise from the vehicle's heading.
    ///
    /// `azimuth_rad` is the direction's bearing relative to the heading (0 ahead,
    /// positive to the left), `elevation_rad` its angle above the horizon. Rotating the
    /// mount by `r` moves every feature of the pattern, its nulls included, by `r`.
    #[must_use]
    pub fn relative_gain_db(&self, azimuth_rad: f64, elevation_rad: f64, rotation_rad: f64) -> f64 {
        let az = wrap_360((azimuth_rad - rotation_rad) * DEG_PER_RAD);
        if self.rows.len() == 1 {
            return cut_gain(&self.rows[0].1, az);
        }
        let el = elevation_rad * DEG_PER_RAD;
        let first = &self.rows[0];
        let last = &self.rows[self.rows.len() - 1];
        if el <= first.0 {
            return cut_gain(&first.1, az);
        }
        if el >= last.0 {
            return cut_gain(&last.1, az);
        }
        let i = self.rows.partition_point(|(e, _)| *e <= el) - 1;
        let (e0, c0) = &self.rows[i];
        let (e1, c1) = &self.rows[i + 1];
        let g0 = cut_gain(c0, az);
        let g1 = cut_gain(c1, az);
        g0 + (el - e0) / (e1 - e0) * (g1 - g0)
    }
}

/// How a sampled antenna's mount is rotated from one vehicle to the next
/// (`radio.devices.obu.sampled_pattern.rotation_jitter`): installation tolerance, or a
/// fleet whose antennas are not all aligned with the heading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RotationJitter {
    /// Every mount at the configured rotation.
    #[default]
    None,
    /// A per-vehicle rotation uniform in ±`rotation_jitter_deg`.
    Uniform,
    /// A per-vehicle rotation normal with σ = `rotation_jitter_deg`.
    Normal,
}

/// The extra mount rotation of `node`, radians: a keyed draw from the run's seed and the
/// node's id in the `antenna/sampled` plug-in domain, so it is the same at every
/// evaluation, in every thread, and independent of every other draw in the run.
#[must_use]
pub fn rotation_draw_rad(
    seed: u64,
    node: v2xw_core::ids::NodeId,
    jitter: RotationJitter,
    width_deg: f64,
) -> f64 {
    use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
    if jitter == RotationJitter::None || width_deg == 0.0 {
        return 0.0;
    }
    let mut s = RngStream::derive(seed, RngDomain::plugin(SAMPLED_ID), EntityRef::Node(node));
    let deg = match jitter {
        RotationJitter::None => 0.0,
        RotationJitter::Uniform => s.uniform(-width_deg, width_deg),
        RotationJitter::Normal => s.normal(0.0, width_deg),
    };
    deg / DEG_PER_RAD
}

/// The gain of one azimuth cut at `az` (degrees in [0, 360)), linear in dB between the
/// neighbouring samples and wrapping from the last sample to the first.
fn cut_gain(cut: &[(f64, f64)], az: f64) -> f64 {
    let n = cut.len();
    if n == 1 {
        return cut[0].1;
    }
    let j = cut.partition_point(|(a, _)| *a <= az);
    let ((a0, g0), (a1, g1)) = if j == 0 {
        // Before the first sample: between the last (one turn back) and the first.
        let (a, g) = cut[n - 1];
        ((a - 360.0, g), cut[0])
    } else if j == n {
        let (a, g) = cut[0];
        (cut[n - 1], (a + 360.0, g))
    } else {
        (cut[j - 1], cut[j])
    };
    g0 + (az - a0) / (a1 - a0) * (g1 - g0)
}

/// An angle in degrees folded into [0, 360).
fn wrap_360(a: f64) -> f64 {
    let x = a.rem_euclid(360.0);
    if x >= 360.0 { 0.0 } else { x }
}

/// The card of the sampled antenna pattern.
#[must_use]
pub fn sampled_card() -> ModelCard {
    let eckhoff = Source {
        kind: SourceKind::Paper,
        reference: "D. Eckhoff, A. Brummer, C. Sommer, \"On the Impact of Antenna Patterns \
                    on VANET Simulation\", IEEE VNC 2016, doi:10.1109/VNC.2016.7835925, §III"
            .to_string(),
        accessed: Some("2026-10-26".to_string()),
        note: Some(
            "Equidistant azimuth samples, positive or negative gain, interpolated between \
             samples; azimuth plane only. Full text cached as \
             research/papers/eckhoff2016impact.pdf."
                .to_string(),
        ),
    };
    let kornek = Source::new(
        SourceKind::Paper,
        "D. Kornek, M. Schack, E. Slottke et al., \"Effects of Antenna Characteristics and \
         Placements on a Vehicle-to-Vehicle Channel Scenario\", IEEE ICC Workshops 2010 \
         (measured installed vehicle patterns)",
    );
    let veins = Source::new(
        SourceKind::Code,
        "Veins SampledAntenna1D (behaviour only: equidistant samples from the heading, \
         linear interpolation in dB, a random rotation per antenna) and its veins_catch \
         test inputs; no code was copied",
    );
    let mut card = ModelCard::new(
        SAMPLED_ID,
        Family::Propagation,
        "1.0.0",
        "A measured installed antenna pattern: a CSV table of azimuth (optionally \
         elevation) gains relative to the device's scalar gain, interpolated linearly in \
         dB, with an optional fixed and per-vehicle random mount rotation.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new(
            "azimuth",
            "G(φ) = G_i + (φ − φ_i)/(φ_{i+1} − φ_i)·(G_{i+1} − G_i), φ = bearing − heading − \
             rotation, wrapping round 360°",
        ),
        Equation::new(
            "elevation",
            "G(θ, φ) linear in θ between the two bracketing cuts; the outermost cut beyond \
             them",
        ),
    ];
    card.parameters = vec![
        Parameter {
            name: "rotation_deg".to_string(),
            unit: "deg".to_string(),
            default: serde_json::json!(0.0),
            range: Some(vec![serde_json::json!(-360.0), serde_json::json!(360.0)]),
            source: veins.clone(),
            calibration: None,
        },
        Parameter {
            name: "rotation_jitter_deg".to_string(),
            unit: "deg".to_string(),
            default: serde_json::json!(0.0),
            range: Some(vec![serde_json::json!(0.0), serde_json::json!(180.0)]),
            source: veins.clone(),
            calibration: None,
        },
    ];
    card.assumptions = vec![
        "One table for every vehicle (car, van, motorcycle, truck, bus); roadside units and \
         VRU devices keep their scalar gain."
            .to_string(),
        "A vehicle whose heading is unknown is treated as isotropic toward that link.".to_string(),
        "Gains are relative to radio.devices.obu.antenna_gain_dbi, so a table of zeros is \
         the isotropic antenna exactly."
            .to_string(),
    ];
    card.limitations = vec![
        "No pattern is shipped: the table is the scenario's, so the realism is the \
         measurement's. Polarisation and the near field are not modelled."
            .to_string(),
    ];
    card.sources = vec![eckhoff.clone(), kornek, veins];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![eckhoff],
        tests: vec![
            "a_sampled_pattern_interpolates_and_agrees_with_the_veins_catch_numbers".to_string(),
            "a_rotated_pattern_moves_its_null_by_the_rotation".to_string(),
        ],
    };
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec!["plugin".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sampled_pattern_interpolates_and_agrees_with_the_veins_catch_numbers() {
        // Veins veins_catch, SampledAntenna1D: four equidistant samples of 1, 2, 3 and
        // 4 mW (as dBm) numbered clockwise from the heading — front, right, back, left —
        // and the gains toward front, right, back, left, and 45° to either side, which
        // is the mean in dB of the two neighbours. Read in the engine's
        // counter-clockwise convention through the azimuth_cw_deg column.
        let db = |mw: f64| 10.0 * mw.log10();
        let csv = format!(
            "# veins_catch inputs\nazimuth_cw_deg,gain_db\n0,{}\n90,{}\n180,{}\n270,{}\n",
            db(1.0),
            db(2.0),
            db(3.0),
            db(4.0)
        );
        let p = SampledAntenna::from_csv(&csv).expect("parses");
        let deg = |d: f64| d / DEG_PER_RAD;
        let lin = |az_ccw: f64| 10f64.powf(p.relative_gain_db(deg(az_ccw), 0.0, 0.0) / 10.0);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9 * b.abs().max(1.0);
        assert!(close(lin(0.0), 1.0), "front {}", lin(0.0));
        assert!(close(lin(-90.0), 2.0), "right {}", lin(-90.0));
        assert!(close(lin(180.0), 3.0), "back {}", lin(180.0));
        assert!(close(lin(90.0), 4.0), "left {}", lin(90.0));
        let mid = |a: f64, b: f64| 10f64.powf(0.5 * (db(a) + db(b)) / 10.0);
        assert!(close(lin(-45.0), mid(1.0, 2.0)), "45° right {}", lin(-45.0));
        assert!(close(lin(45.0), mid(1.0, 4.0)), "45° left {}", lin(45.0));
        // Veins' "also left" row: the antenna faces +90° (rotated a quarter turn
        // counter-clockwise in the engine's frame, Veins' (0, 1) heading being its
        // right-hand side) ... expressed here as the mount's own rotation.
        let also = 10f64.powf(p.relative_gain_db(0.0, 0.0, deg(-90.0)) / 10.0);
        assert!(close(also, 4.0), "rotated {also}");
        // A table of zeros is exactly the isotropic antenna, at every angle.
        let flat = SampledAntenna::from_csv("azimuth_deg,gain_db\n0,0\n120,0\n240,0\n").unwrap();
        for a in -400..400 {
            let g = flat.relative_gain_db(f64::from(a) * 0.017, f64::from(a) * 0.003, 0.7);
            assert!(g == 0.0 && g.is_sign_positive(), "{a}: {g}");
        }
        // Elevation cuts: linear between rows, held beyond them.
        let el = SampledAntenna::from_csv(
            "elevation_deg,azimuth_deg,gain_db\n0,0,0\n0,180,-10\n20,0,-6\n20,180,-16\n",
        )
        .unwrap();
        assert!((el.relative_gain_db(0.0, deg(10.0), 0.0) + 3.0).abs() < 1e-12);
        assert!((el.relative_gain_db(deg(90.0), deg(10.0), 0.0) + 8.0).abs() < 1e-12);
        assert!((el.relative_gain_db(0.0, deg(45.0), 0.0) + 6.0).abs() < 1e-12);
        assert!((el.relative_gain_db(0.0, deg(-30.0), 0.0)).abs() < 1e-12);
        // Bad tables are refused with a reason.
        for bad in [
            "",
            "gain_db\n0\n",
            "azimuth_deg,gain_db,colour\n0,0,red\n",
            "azimuth_deg,azimuth_cw_deg,gain_db\n0,0,0\n",
            "azimuth_deg,gain_db\n0,0\n360,1\n",
            "azimuth_deg,gain_db\n0,x\n",
            "azimuth_deg,gain_db\n0\n",
            "elevation_deg,azimuth_deg,gain_db\n95,0,0\n",
            "azimuth_deg,gain_db\n",
        ] {
            assert!(
                SampledAntenna::from_csv(bad).is_err(),
                "{bad:?} was accepted"
            );
        }
        sampled_card().validate().expect("the card validates");
    }

    #[test]
    fn a_rotated_pattern_moves_its_null_by_the_rotation() {
        // A patch-like pattern with a 40 dB null straight behind (the shape of Eckhoff
        // 2016 Figure 1d's mirror-mounted patch).
        let mut csv = String::from("azimuth_deg,gain_db\n");
        for a in (0..360).step_by(10) {
            let off = f64::from(a.min(360 - a)); // 0 ahead, 180 behind
            let g = if off == 180.0 {
                -40.0
            } else {
                -12.0 * (off / 120.0).powi(2)
            };
            csv.push_str(&format!("{a},{g}\n"));
        }
        let p = SampledAntenna::from_csv(&csv).unwrap();
        let null_of = |rot_deg: f64| {
            (0..3600)
                .map(|i| f64::from(i) * 0.1)
                .map(|a| {
                    (
                        a,
                        p.relative_gain_db(a / DEG_PER_RAD, 0.0, rot_deg / DEG_PER_RAD),
                    )
                })
                .min_by(|x, y| x.1.total_cmp(&y.1).then(x.0.total_cmp(&y.0)))
                .map(|(a, _)| a)
                .unwrap()
        };
        let base = null_of(0.0);
        assert!((base - 180.0).abs() < 1e-9, "null at {base}");
        for rot in [15.0, 90.0, -40.0, 200.0, 7.3] {
            let moved = null_of(rot);
            let want = (base + rot).rem_euclid(360.0);
            assert!(
                (moved - want).abs() < 1e-6,
                "rotation {rot}: null at {moved}, want {want}"
            );
            // And the null's depth goes with it.
            let depth = p.relative_gain_db(want / DEG_PER_RAD, 0.0, rot / DEG_PER_RAD);
            assert!((depth + 40.0).abs() < 1e-6, "{depth}");
        }
        // The per-vehicle draw is keyed: the same for one node, different across nodes,
        // zero when switched off, inside the uniform width.
        use v2xw_core::ids::NodeId;
        let a = rotation_draw_rad(7, NodeId::new(3), RotationJitter::Uniform, 30.0);
        assert_eq!(
            a,
            rotation_draw_rad(7, NodeId::new(3), RotationJitter::Uniform, 30.0)
        );
        assert_ne!(
            a,
            rotation_draw_rad(7, NodeId::new(4), RotationJitter::Uniform, 30.0)
        );
        assert_ne!(
            a,
            rotation_draw_rad(8, NodeId::new(3), RotationJitter::Uniform, 30.0)
        );
        assert!(a.abs() <= 30.0 / DEG_PER_RAD);
        assert_eq!(
            rotation_draw_rad(7, NodeId::new(3), RotationJitter::None, 30.0),
            0.0
        );
        assert_eq!(
            rotation_draw_rad(7, NodeId::new(3), RotationJitter::Normal, 0.0),
            0.0
        );
    }

    #[test]
    fn the_patterns_are_tr37885s() {
        let deg = |d: f64| d / DEG_PER_RAD;
        // A rooftop antenna is the same in every horizontal direction.
        for a in [0.0, 45.0, 90.0, 180.0, -135.0] {
            assert_eq!(AntennaMount::Rooftop.relative_gain_db(deg(a), 0.0), 0.0);
        }
        // Its vertical pattern: 12·(θ−90)²/90², so 3 dB down at 45° elevation ... and
        // SLA_V = 20 dB at most.
        let v45 = AntennaMount::Rooftop.relative_gain_db(0.0, deg(45.0));
        assert!((v45 + 3.0).abs() < 1e-9, "{v45}");
        let v90 = AntennaMount::Rooftop.relative_gain_db(0.0, deg(90.0));
        assert!((v90 + 12.0).abs() < 1e-9, "{v90}");
        // A truck: full gain ahead and behind, 12·(90/120)² = 6.75 dB down to the side,
        // 3 dB down at the half-power angle of 60° off either panel's bearing.
        let t = |a: f64| AntennaMount::FrontRear.relative_gain_db(deg(a), 0.0);
        assert_eq!(t(0.0), 0.0);
        assert_eq!(t(180.0), 0.0);
        assert!((t(90.0) + 6.75).abs() < 1e-9, "{}", t(90.0));
        assert!((t(-90.0) + 6.75).abs() < 1e-9);
        assert!((t(60.0) + 3.0).abs() < 1e-9, "{}", t(60.0));
        assert!((t(120.0) + 3.0).abs() < 1e-9, "{}", t(120.0));
        // Symmetric, and never above the maximum.
        for a in (-180..=180).step_by(15) {
            let g = t(f64::from(a));
            assert!((g - t(-f64::from(a))).abs() < 1e-9);
            assert!(g <= 0.0 && g >= -A_M_DB);
        }
        // The class mapping.
        assert_eq!(
            AntennaMount::for_class(ActorClass::Car),
            AntennaMount::Rooftop
        );
        assert_eq!(
            AntennaMount::for_class(ActorClass::Truck),
            AntennaMount::FrontRear
        );
        assert_eq!(
            AntennaMount::for_class(ActorClass::Rsu),
            AntennaMount::Isotropic
        );
        assert_eq!(AntennaMount::Isotropic.relative_gain_db(1.0, 1.0), 0.0);
        card().validate().expect("the card validates");
    }
}
