//! `energy/emission/phemlight` — PHEMlight, TU Graz's normalised-power simplification of
//! PHEM (Hausberger et al. 2009), with the two passenger-car vehicles SUMO distributes.
//!
//! # The model
//!
//! A power-based model: the engine power the vehicle needs is computed from its road-load
//! and inertia, and emissions and fuel are read off measured curves of emission rate
//! against **normalised** power.
//!
//! ```text
//! P = [ (m+L)·g·(f0 + f1·v + f4·v⁴)·v                      rolling resistance
//!     + ½·ρ·cd·A·v³                                         air resistance
//!     + (m·k_rot(v) + m_rot + L)·a·v                        inertia (with rotating masses)
//!     + (m+L)·g·grade·v ] / 1000 / η_drivetrain + P_aux     grade, transmission, auxiliaries
//!
//! fuel  = FC( P / P_rated ) · P_rated              g/h    (FC curve: g/h per kW rated)
//! X     = E_X( P / P_drive )                       g/h    (P_drive = P(19.444 m/s, 0.45 m/s²))
//! CO₂   = (fuel·f_C,fuel − CO·0.429 − HC·0.866) / 0.273    (carbon balance)
//! ```
//!
//! At a standstill (`|v| ≤ 0.5 m/s`) the idle row of each curve applies; when the vehicle
//! decelerates harder than it would coast with the engine dragging in gear
//! ([`Cep::decel_coast`]), fuel is cut off and every rate is zero — PHEMlight's own rule,
//! which SUMO applies too (`HelpersPHEMlight::compute`).
//!
//! # What is reproduced, and from where
//!
//! The algorithm follows the behaviour of SUMO's PHEMlight reader (`CEP.cpp`, EPL-2.0,
//! read for behaviour only; licence audit item E-1 asks for a re-derivation from the TU
//! Graz PHEMlight documentation): `CalcPower`,
//! `GetEmission` (linear interpolation, clamped at both ends of the curve),
//! `GetCO2Emission` and `GetDecelCoast`, with the constants of `Constants.cpp` (g = 9.81,
//! ρ = 1.182, normalising speed 19.444 m/s and acceleration 0.45 m/s², drivetrain
//! efficiency 0.9 for every class but city buses). The vehicle and curve data are the
//! `PC_G_EU4` and `PC_D_EU4` files of SUMO's `data/emissions/PHEMlight`, transcribed by
//! script. One deliberate difference: the grade enters as a percentage, which is what the
//! CEP's `gradient · 0.01` expects; SUMO passes its slope in degrees there.
//!
//! # Classes
//!
//! Two vehicles are published openly: a Euro 4 petrol and a Euro 4 diesel passenger car.
//! A passenger car of any other Euro class resolves to the Euro 4 vehicle of its fuel —
//! the class name it resolved to (`PC_G_EU4`) is what the provider reports, so the
//! substitution is visible — and every other segment is unmodelled under this model. More
//! vehicles are a data addition: a [`Cep`] literal from the vehicle's three PHEMlight files.

use v2xw_core::card::{Family, ModelCard, SourceKind, Tier, Validation, ValidationStatus};
use v2xw_core::model::Model;

use crate::cite;
use crate::powertrain::{Fuel, Powertrain, Segment};
use crate::state::{DriveState, Rates};
use crate::traits::{EmissionClass, EmissionModel};

/// The model's id.
pub const MODEL_ID: &str = "energy/emission/phemlight";
/// The model's version.
pub const MODEL_VERSION: &str = "1.0.0";

/// PHEMlight's gravity constant, m/s² (`Constants::GRAVITY_CONST`).
const G: f64 = 9.81;
/// PHEMlight's air density, kg/m³ (`Constants::AIR_DENSITY_CONST`).
const RHO: f64 = 1.182;
/// The speed the driving power is normalised at, m/s (`NORMALIZING_SPEED`).
const NORM_SPEED: f64 = 19.444;
/// The acceleration the driving power is normalised at, m/s² (`NORMALIZING_ACCELARATION`).
const NORM_ACC: f64 = 0.45;
/// Below this speed the idle rows apply, m/s (`ZERO_SPEED_ACCURACY`).
const ZERO_SPEED: f64 = 0.5;
/// Below this speed the coasting deceleration is scaled linearly, m/s (`SPEED_DCEL_MIN`).
const SPEED_DCEL_MIN: f64 = 10.0 / 3.6;
/// Drivetrain efficiency of every class but city buses (`DRIVE_TRAIN_EFFICIENCY_All`).
const ETA_DRIVETRAIN: f64 = 0.9;

/// Carbon mass fractions of the carbon balance (`CEP::GetCO2Emission`).
const F_C_CO: f64 = 0.429;
const F_C_HC: f64 = 0.866;
const F_C_CO2: f64 = 0.273;

/// One PHEMlight vehicle ("CEP"): its road-load, drivetrain and measured curves.
#[derive(Debug)]
pub struct Cep {
    /// The class name, as SUMO's file is named.
    pub name: &'static str,
    /// Its fuel.
    pub fuel: Fuel,
    /// Vehicle mass, kg.
    pub mass_kg: f64,
    /// Vehicle loading, kg.
    pub loading_kg: f64,
    /// Drag coefficient.
    pub cd: f64,
    /// Cross-sectional area, m².
    pub area_m2: f64,
    /// Wheels' equivalent rotational mass, kg.
    pub mass_rot_kg: f64,
    /// Auxiliary power, normalised by rated power.
    pub aux_norm: f64,
    /// Rated engine power, kW.
    pub rated_kw: f64,
    /// Rated engine speed, rpm.
    pub rated_rpm: f64,
    /// Idling engine speed, rpm.
    pub idle_rpm: f64,
    /// Rolling-resistance coefficients f0, f1 (s/m), f2, f3, f4 (s⁴/m⁴).
    pub f: [f64; 5],
    /// Axle ratio.
    pub axle_ratio: f64,
    /// Effective wheel diameter, m.
    pub wheel_diameter_m: f64,
    /// Speed (km/h), gear ratio, rotational mass factor.
    pub speed_rot: &'static [[f64; 3]],
    /// Normalised engine speed, normalised drag power.
    pub drag: &'static [[f64; 2]],
    /// Fuel: power / rated power, g/h per kW rated.
    pub fc: &'static [[f64; 2]],
    /// Idle fuel, g/h per kW rated.
    pub idle_fc: f64,
    /// Pollutants: power / driving power, NOx, HC, CO, PM (g/h).
    pub pollutants: &'static [[f64; 5]],
    /// Idle NOx, HC, CO, PM, g/h.
    pub idle_pollutants: [f64; 4],
}

/// Linear interpolation of `y` over the ascending `x` column, clamped at both ends — the
/// combined effect of `FindLowerUpperInPattern` and `Interpolate`.
fn interp(x: f64, xs: impl Fn(usize) -> f64, ys: impl Fn(usize) -> f64, n: usize) -> f64 {
    if n == 0 {
        return 0.0;
    }
    if x <= xs(0) {
        return ys(0);
    }
    if x >= xs(n - 1) {
        return ys(n - 1);
    }
    let mut lo = 0;
    let mut hi = n - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if xs(mid) <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let (x0, x1) = (xs(lo), xs(hi));
    if x1 == x0 {
        return ys(lo);
    }
    ys(lo) + (x - x0) / (x1 - x0) * (ys(hi) - ys(lo))
}

impl Cep {
    /// The rotational mass factor at `v` m/s.
    fn rot_factor(&self, v: f64) -> f64 {
        let t = self.speed_rot;
        interp(v, |i| t[i][0] / 3.6, |i| t[i][2], t.len())
    }

    /// The gear ratio at `v` m/s.
    fn gear(&self, v: f64) -> f64 {
        let t = self.speed_rot;
        interp(v, |i| t[i][0] / 3.6, |i| t[i][1], t.len())
    }

    /// Engine power demand, kW (`CEP::CalcPower`); `grade_pct` in percent.
    #[must_use]
    pub fn power_kw(&self, v: f64, a: f64, grade_pct: f64) -> f64 {
        let m = self.mass_kg + self.loading_kg;
        let [f0, f1, _, _, f4] = self.f;
        let mut p = m * G * (f0 + f1 * v + f4 * v * v * v * v) * v;
        p += self.area_m2 * self.cd * RHO / 2.0 * v * v * v;
        p += (self.mass_kg * self.rot_factor(v) + self.mass_rot_kg + self.loading_kg) * a * v;
        p += m * G * grade_pct * 0.01 * v;
        p / 1000.0 / ETA_DRIVETRAIN + self.aux_norm * self.rated_kw
    }

    /// The power the pollutant curves are normalised by, kW: the driving power at the
    /// normalising speed and acceleration (light vehicles).
    fn driving_kw(&self) -> f64 {
        self.power_kw(NORM_SPEED, NORM_ACC, 0.0)
    }

    /// The deceleration at which the vehicle coasts in gear with the engine dragging, m/s²
    /// (`CEP::GetDecelCoast`): any harder deceleration needs no fuel.
    #[must_use]
    pub fn decel_coast(&self, v: f64, grade_pct: f64) -> f64 {
        if v < SPEED_DCEL_MIN {
            return v / SPEED_DCEL_MIN * self.decel_coast(SPEED_DCEL_MIN, grade_pct);
        }
        let m = self.mass_kg + self.loading_kg;
        let i_tot = self.gear(v) * self.axle_ratio;
        let n = 30.0 * v * i_tot / ((self.wheel_diameter_m / 2.0) * core::f64::consts::PI);
        let n_norm = (n - self.idle_rpm) / (self.rated_rpm - self.idle_rpm);
        let d = self.drag;
        let drag_norm = interp(n_norm, |i| d[i][0], |i| d[i][1], d.len());
        let f_mot = if v >= 10e-2 {
            (-drag_norm * self.rated_kw * 1000.0 / v) / 0.9
        } else {
            0.0
        };
        let [f0, f1, f2, f3, f4] = self.f;
        // CEP.cpp writes the higher terms as pow(f_k·v, k); f2 and f3 are zero in both
        // published vehicles, and f4 is reproduced as written.
        let f_roll = (f0
            + f1 * v
            + (f2 * v) * (f2 * v)
            + (f3 * v) * (f3 * v) * (f3 * v)
            + (f4 * v) * (f4 * v) * (f4 * v) * (f4 * v))
            * m
            * G;
        let f_air = self.cd * self.area_m2 * 1.2 * 0.5 * v * v;
        let f_grad = m * G * grade_pct / 100.0;
        -(f_mot + f_roll + f_air + f_grad) / (m * self.rot_factor(v))
    }

    /// Fuel, g/h, at engine power `p` kW and speed `v`.
    fn fuel_g_h(&self, p: f64, v: f64) -> f64 {
        if v.abs() <= ZERO_SPEED {
            return self.idle_fc * self.rated_kw;
        }
        let t = self.fc;
        let r = self.rated_kw;
        interp(p, |i| t[i][0] * r, |i| t[i][1] * r, t.len())
    }

    /// NOx, HC, CO, PM, g/h, at engine power `p` kW and speed `v`.
    fn pollutants_g_h(&self, p: f64, v: f64) -> [f64; 4] {
        if v.abs() <= ZERO_SPEED {
            return self.idle_pollutants;
        }
        let t = self.pollutants;
        let pd = self.driving_kw();
        let mut out = [0.0; 4];
        for (k, o) in out.iter_mut().enumerate() {
            *o = interp(p, |i| t[i][0] * pd, |i| t[i][k + 1], t.len());
        }
        out
    }

    /// The fuel's carbon mass fraction (`CEP::GetCO2Emission`).
    fn carbon_fraction(&self) -> f64 {
        match self.fuel {
            Fuel::Diesel => 0.863,
            _ => 0.865,
        }
    }

    /// Every rate, g/s, for one interval.
    #[must_use]
    pub fn rates(&self, s: &DriveState) -> Rates {
        let v = s.v_mps.max(0.0);
        let grade_pct = s.grade * 100.0;
        if v > ZERO_SPEED && s.a_mps2 < self.decel_coast(v, grade_pct) {
            return Rates::default();
        }
        let p = self.power_kw(v, s.a_mps2, grade_pct);
        let fuel = self.fuel_g_h(p, v).max(0.0);
        let [nox, hc, co, pm] = self.pollutants_g_h(p, v).map(|x| x.max(0.0));
        let co2 = ((fuel * self.carbon_fraction() - co * F_C_CO - hc * F_C_HC) / F_C_CO2).max(0.0);
        Rates {
            co2_g_s: co2 / 3600.0,
            fuel_g_s: fuel / 3600.0,
            nox_g_s: nox / 3600.0,
            pm_g_s: pm / 3600.0,
            co_g_s: co / 3600.0,
            hc_g_s: hc / 3600.0,
        }
    }
}

/// The PHEMlight emission model over the published vehicles.
#[derive(Debug, Clone)]
pub struct PhemLight {
    card: ModelCard,
}

impl Default for PhemLight {
    fn default() -> Self {
        Self::new()
    }
}

impl PhemLight {
    /// The model with its card.
    #[must_use]
    pub fn new() -> Self {
        Self { card: card() }
    }

    /// The published vehicles.
    #[must_use]
    pub fn vehicles() -> &'static [Cep] {
        VEHICLES
    }

    /// PHEMlight's road-load constants, read-only, for the mobility crate's class
    /// road-load table (`v2xw_mobility::classes::ClassDynamics`): gravity (m/s²), air
    /// density (kg/m³) and the drivetrain efficiency of every class but city buses.
    #[must_use]
    pub const fn road_load_constants() -> (f64, f64, f64) {
        (G, RHO, ETA_DRIVETRAIN)
    }
}

impl Model for PhemLight {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl EmissionModel for PhemLight {
    fn resolve(&self, p: &Powertrain) -> Option<EmissionClass> {
        let name = match (p.segment, p.fuel) {
            (Segment::PassengerCar, Fuel::Gasoline) => "PC_G_EU4",
            (Segment::PassengerCar, Fuel::Diesel) => "PC_D_EU4",
            _ => return None,
        };
        VEHICLES
            .iter()
            .position(|c| c.name == name)
            .map(|index| EmissionClass {
                index,
                name: name.to_string(),
            })
    }

    fn rates(&self, class: &EmissionClass, s: &DriveState) -> Rates {
        VEHICLES
            .get(class.index)
            .map_or_else(Rates::default, |c| c.rates(s))
    }
}

/// The model's card.
#[must_use]
pub fn card() -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Emission,
        MODEL_VERSION,
        "Tailpipe CO2, CO, HC, NOx, PM and fuel of a passenger car from the engine power its \
         road-load and inertia demand, read off measured emission curves against normalised \
         power (PHEMlight, TU Graz). Fuel is cut off when the car decelerates harder than it \
         would coast in gear. The two openly published vehicles: a Euro 4 petrol and a Euro 4 \
         diesel passenger car.",
    );
    card.tier = vec![Tier::Medium, Tier::High];
    card.equations = vec![
        cite::eq(
            "engine power",
            "P = [ (m+L) g (f0 + f1 v + f4 v^4) v + rho/2 cd A v^3 + (m k_rot(v) + m_rot + L) a v \
             + (m+L) g (grade%/100) v ] / 1000 / eta + P_aux",
            "kW; k_rot(v) interpolated from the vehicle's speed/gear table",
        ),
        cite::eq(
            "fuel",
            "FC = FC_curve(P / P_rated) * P_rated  [g/h]",
            "idle row at |v| <= 0.5 m/s",
        ),
        cite::eq(
            "pollutant",
            "E_X = E_X_curve(P / P_drive)  [g/h],  P_drive = P(19.444 m/s, 0.45 m/s2, 0)",
            "light-vehicle normalisation; idle row at |v| <= 0.5 m/s",
        ),
        cite::eq(
            "CO2 by carbon balance",
            "CO2 = (FC f_C - CO 0.429 - HC 0.866) / 0.273",
            "f_C = 0.865 petrol, 0.863 diesel",
        ),
        cite::eq(
            "coasting cut-off",
            "a < a_coast(v, grade) => every rate 0",
            "a_coast from engine drag in gear, rolling, air and grade resistance (GetDecelCoast)",
        ),
    ];
    let data = cite::sumo_phemlight_data();
    card.parameters = vec![
        cite::param("gravity", "m/s2", G, data.clone()),
        cite::param("air_density", "kg/m3", RHO, data.clone()),
        cite::param("normalising_speed", "m/s", NORM_SPEED, data.clone()),
        cite::param("normalising_acceleration", "m/s2", NORM_ACC, data.clone()),
        cite::param("drivetrain_efficiency", "-", ETA_DRIVETRAIN, data.clone()),
        cite::param("zero_speed", "m/s", ZERO_SPEED, data.clone()),
    ];
    card.assumptions = vec![
        "hot running emissions".to_string(),
        "a Euro 0-3, 5 or 6 passenger car is computed with the Euro 4 vehicle of its fuel; \
         the resolved class name is reported"
            .to_string(),
    ];
    card.limitations = vec![
        "two published vehicles only; light commercial, heavy-duty, bus, coach and \
         motorcycle are unmodelled under this model (use energy/emission/hbefa3)"
            .to_string(),
    ];
    card.ignores = vec![
        "cold start".to_string(),
        "the PN and NO columns of the curves".to_string(),
        "non-exhaust PM".to_string(),
        "deterioration and NOx corrections (Deterioration.det, NOxCor.tno of PHEMlight5)"
            .to_string(),
    ];
    card.sources = vec![
        cite::hausberger_phemlight(),
        data,
        cite::src(
            SourceKind::Paper,
            "Krajzewicz D. et al. (2015) Second Generation of Pollutant Emission Models for \
             SUMO",
            "describes PHEMlight's integration into SUMO and its relation to HBEFA 3",
        ),
    ];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![cite::epa_cycles()],
        tests: vec![
            "tests/models.rs::every_combustion_model_lands_in_the_cited_band_on_udds_and_hwfet"
                .to_string(),
            "tests/models.rs::hbefa3_and_phemlight_agree_on_the_same_car".to_string(),
        ],
    };
    card
}

/// The published vehicles.
static VEHICLES: &[Cep] = &[
    Cep {
        name: "PC_G_EU4",
        fuel: Fuel::Gasoline,
        mass_kg: 1235.0,
        loading_kg: 50.0,
        cd: 0.3113,
        area_m2: 2.11756,
        mass_rot_kg: 40.0,
        aux_norm: 0.0,
        rated_kw: 72.0,
        rated_rpm: 5565.0,
        idle_rpm: 798.0,
        f: [0.009, 5e-5, 0.0, 0.0, 1.6e-9],
        axle_ratio: 4.0826,
        wheel_diameter_m: 0.6064,
        speed_rot: G_SPEED_ROT,
        drag: G_DRAG,
        fc: G_FC,
        idle_fc: G_IDLE_FC,
        pollutants: G_POLLUTANTS,
        idle_pollutants: G_IDLE_POLLUTANTS,
    },
    Cep {
        name: "PC_D_EU4",
        fuel: Fuel::Diesel,
        mass_kg: 1500.0,
        loading_kg: 50.0,
        cd: 0.3113,
        area_m2: 2.16,
        mass_rot_kg: 41.3424,
        aux_norm: 0.0,
        rated_kw: 93.0,
        rated_rpm: 4073.0,
        idle_rpm: 816.0,
        f: [0.009, 5e-5, 0.0, 0.0, 1.6e-9],
        axle_ratio: 3.7284,
        wheel_diameter_m: 0.6264,
        speed_rot: D_SPEED_ROT,
        drag: D_DRAG,
        fc: D_FC,
        idle_fc: D_IDLE_FC,
        pollutants: D_POLLUTANTS,
        idle_pollutants: D_IDLE_POLLUTANTS,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_card_validates_and_the_tables_are_sorted() {
        assert!(card().validate().is_ok());
        for c in VEHICLES {
            for w in c.fc.windows(2) {
                assert!(w[0][0] < w[1][0], "{}: FC not ascending", c.name);
            }
            for w in c.pollutants.windows(2) {
                assert!(w[0][0] < w[1][0], "{}: pollutants not ascending", c.name);
            }
            for w in c.speed_rot.windows(2) {
                assert!(w[0][0] < w[1][0], "{}: speed table not ascending", c.name);
            }
        }
    }

    #[test]
    fn a_petrol_car_at_50_kmh_needs_about_3_kw_and_burns_about_6_litres() {
        // Hand calculation: rolling 1285 * 9.81 * (0.009 + 5e-5 * 13.889) * 13.889 = 1.70 kW,
        // air 2.11756 * 0.3113 * 1.182 / 2 * 13.889^3 = 1.05 kW; (1.70 + 1.05) / 0.9 = 3.05 kW.
        let c = &VEHICLES[0];
        let v = 50.0 / 3.6;
        let p = c.power_kw(v, 0.0, 0.0);
        assert!((p - 3.05).abs() < 0.02, "{p} kW");
        let r = c.rates(&DriveState::between(v, v, 1.0));
        let l_per_100km = r.fuel_g_s / v * 1000.0 * 100.0 / 742.0;
        assert!((5.5..6.6).contains(&l_per_100km), "{l_per_100km} L/100 km");
        // The carbon balance puts CO2 near 3.1-3.2 g per g of fuel.
        let ratio = r.co2_g_s / r.fuel_g_s;
        assert!((3.0..3.2).contains(&ratio), "{ratio}");
    }

    #[test]
    fn braking_cuts_the_fuel_and_standing_still_idles() {
        let c = &VEHICLES[1];
        assert_eq!(
            c.rates(&DriveState::between(15.0, 12.0, 1.0)),
            Rates::default()
        );
        let idle = c.rates(&DriveState::between(0.0, 0.0, 1.0));
        assert!((idle.fuel_g_s - D_IDLE_FC * 93.0 / 3600.0).abs() < 1e-12);
        assert!(idle.co2_g_s > 0.0);
    }
}

// --- Data: TU Graz PHEMlight EU4 sample vehicles, values as SUMO distributes them ------
/// `PC_G_EU4.csv` idle row: NOx, HC, CO, PM, g/h.
const G_IDLE_POLLUTANTS: [f64; 4] = [0.2311568, 0.06580322, 1.57672, 0.0005531011];
/// `PC_G_EU4.csv`: normalised drive power, then NOx, HC, CO, PM in g/h.
const G_POLLUTANTS: &[[f64; 5]] = &[
    [-1.5, 0.0, 0.0, 0.0, 0.0],
    [-1.25, 0.0, 0.0, 0.0, 0.0],
    [-1.0, 0.1968537, 0.06040405, 5.746554, 0.03304507],
    [-0.75, 0.1930701, 0.07494506, 2.899907, 0.007002674],
    [-0.5, 0.5213834, 0.1307903, 5.15753, 0.01752782],
    [-0.25, 0.8236781, 0.1857997, 8.30414, 0.02386856],
    [0.0, 1.078666, 0.1541283, 8.331351, 0.02075658],
    [0.25, 1.767643, 0.1910141, 9.857034, 0.01829969],
    [0.5, 2.494093, 0.2603717, 12.36965, 0.03831597],
    [0.75, 3.85773, 0.4649452, 25.95141, 0.1305431],
    [1.0, 6.102762, 0.7786441, 46.99697, 0.2287794],
    [1.25, 10.26341, 1.321729, 101.1859, 0.3713556],
    [1.5, 13.74077, 1.955556, 169.1437, 0.5279462],
    [1.75, 14.35727, 2.592299, 221.4526, 0.8213891],
    [2.0, 16.96906, 3.493875, 328.9751, 1.21975],
    [2.25, 19.37725, 3.967094, 467.4081, 2.652949],
    [2.5, 22.78092, 7.104819, 1068.183, 3.925668],
    [2.75, 25.44913, 8.668326, 1614.103, 3.719576],
    [3.0, 24.04244, 13.18394, 2765.932, 3.426118],
    [3.25, 26.04598, 14.2826, 2996.426, 3.711627],
    [3.5, 28.04951, 15.38126, 3226.921, 3.997137],
    [3.75, 30.05305, 16.47993, 3457.415, 4.282647],
    [4.0, 32.05659, 17.57859, 3687.909, 4.568157],
    [4.25, 34.06012, 18.67725, 3918.404, 4.853666],
    [4.5, 36.06366, 19.77591, 4148.898, 5.139176],
    [4.75, 38.0672, 20.87457, 4379.392, 5.424685],
    [5.0, 40.07073, 21.97323, 4609.886, 5.710195],
    [5.25, 42.07427, 23.07189, 4840.38, 5.995705],
    [5.5, 44.0778, 24.17056, 5070.875, 6.281214],
    [5.75, 46.08134, 25.26922, 5301.369, 6.566724],
    [6.0, 48.08488, 26.36788, 5531.863, 6.852233],
];
/// `PC_G_EU4_FC.csv` idle fuel, g/h per kW rated.
const G_IDLE_FC: f64 = 10.82388;
/// `PC_G_EU4_FC.csv`: power normalised by rated power, fuel in g/h per kW rated.
const G_FC: &[[f64; 2]] = &[
    [-0.2, 4.366878],
    [-0.1, 15.2459],
    [0.0, 24.06894],
    [0.1, 40.95275],
    [0.2, 63.58941],
    [0.3, 87.5217],
    [0.4, 109.7244],
    [0.5, 136.4364],
    [0.6, 160.0468],
    [0.7, 190.6633],
    [0.8000001, 210.6401],
    [0.9000001, 236.9701],
    [1.0, 263.3002],
];
/// `PC_G_EU4.PHEMLight.veh`: speed km/h, gear ratio, rotational mass factor.
const G_SPEED_ROT: &[[f64; 3]] = &[
    [0.0, 3.6298, 2.008095],
    [12.31168, 3.6298, 2.008095],
    [29.96561, 2.0523, 1.316293],
    [44.09933, 1.3801, 1.145397],
    [69.54142, 1.0477, 1.077003],
    [103.4718, 0.8423, 1.053688],
    [250.0, 0.8423, 1.053688],
];
/// `PC_G_EU4.PHEMLight.veh`: normalised engine speed, normalised drag power.
const G_DRAG: &[[f64; 2]] = &[
    [0.0, -0.012],
    [0.071, -0.017],
    [0.128, -0.022],
    [0.186, -0.026],
    [0.244, -0.03],
    [0.302, -0.034],
    [0.419, -0.043],
    [0.535, -0.052],
    [0.651, -0.06],
    [0.768, -0.069],
    [0.884, -0.078],
    [1.0, -0.086],
    [1.117, -0.095],
    [1.233, -0.103],
    [1.349, -0.112],
];
/// `PC_D_EU4.csv` idle row: NOx, HC, CO, PM, g/h.
const D_IDLE_POLLUTANTS: [f64; 4] = [5.95534, 0.1406479, 0.2413937, 0.1616896];
/// `PC_D_EU4.csv`: normalised drive power, then NOx, HC, CO, PM in g/h.
const D_POLLUTANTS: &[[f64; 5]] = &[
    [-1.5, 0.0, 0.0, 0.0, 0.0],
    [-1.25, 0.0, 0.0, 0.0, 0.0],
    [-1.0, 1.8106, 0.1140485, 0.3899072, 0.1578594],
    [-0.75, 3.51163, 0.2084584, 0.8743084, 0.3038903],
    [-0.5, 6.389799, 0.3229104, 1.062626, 0.438556],
    [-0.25, 9.399384, 0.4154986, 0.844896, 0.5310611],
    [0.0, 12.03175, 0.4185574, 0.7594641, 0.5656357],
    [0.25, 19.22764, 0.5726808, 1.368716, 0.8483028],
    [0.5, 30.78983, 0.6917911, 1.977875, 1.225236],
    [0.75, 44.01824, 0.7626554, 2.612527, 1.687364],
    [1.0, 66.45253, 0.8137084, 2.968721, 2.221274],
    [1.25, 81.7254, 0.8343465, 3.192058, 2.407159],
    [1.5, 111.2948, 0.9483307, 3.222644, 2.798034],
    [1.75, 130.983, 0.9917992, 3.103828, 2.876693],
    [2.0, 155.5134, 0.9762785, 3.366468, 2.696024],
    [2.25, 210.9117, 0.9075134, 4.5983105, 2.965473],
    [2.5, 220.5246, 1.006408, 5.830153, 3.234922],
    [2.75, 258.9931, 1.087851, 5.325302, 3.504371],
    [3.0, 279.6105, 1.318822, 5.325302, 3.77382],
    [3.25, 281.7131, 1.479041, 5.325302, 5.258993],
    [3.5, 300.451, 1.598447, 5.325302, 6.99257],
    [3.75, 321.9117, 1.712622, 5.325302, 7.49204],
    [4.0, 343.3725, 1.826797, 5.325302, 7.991509],
    [4.25, 364.8333, 1.940972, 5.325302, 8.490978],
    [4.5, 386.2941, 2.055147, 5.325302, 8.990448],
    [4.75, 407.7549, 2.169322, 5.325302, 9.489918],
    [5.0, 429.2157, 2.283496, 5.325302, 9.989388],
    [5.25, 450.6765, 2.397671, 5.325302, 10.48886],
    [5.5, 472.1372, 2.511846, 5.325302, 10.98833],
    [5.75, 493.598, 2.626021, 5.325302, 11.4878],
    [6.0, 515.0588, 2.740196, 5.325302, 11.98727],
];
/// `PC_D_EU4_FC.csv` idle fuel, g/h per kW rated.
const D_IDLE_FC: f64 = 6.225238;
/// `PC_D_EU4_FC.csv`: power normalised by rated power, fuel in g/h per kW rated.
const D_FC: &[[f64; 2]] = &[
    [-0.2, 3.632487],
    [-0.1, 11.32746],
    [0.0, 18.48723],
    [0.1, 33.5591],
    [0.2, 52.99504],
    [0.3, 72.9073],
    [0.4, 93.49616],
    [0.5, 106.7572],
    [0.6, 120.398],
    [0.7, 145.4391],
    [0.8000001, 157.9092],
    [0.9000001, 177.6479],
    [1.0, 197.3865],
];
/// `PC_D_EU4.PHEMLight.veh`: speed km/h, gear ratio, rotational mass factor.
const D_SPEED_ROT: &[[f64; 3]] = &[
    [0.0, 3.7079, 1.67],
    [10.53695, 3.7079, 1.67],
    [28.99677, 2.0237, 1.2],
    [43.68127, 1.2784, 1.08],
    [69.56696, 0.9359, 1.05],
    [94.3671, 0.7414, 1.03],
    [121.0647, 0.6162, 1.02],
    [250.0, 0.6162, 1.02],
];
/// `PC_D_EU4.PHEMLight.veh`: normalised engine speed, normalised drag power.
const D_DRAG: &[[f64; 2]] = &[
    [0.0, -0.015],
    [0.002, -0.015],
    [0.05, -0.017],
    [0.1, -0.02],
    [0.2, -0.028],
    [0.3, -0.039],
    [0.4, -0.05],
    [0.5, -0.063],
    [0.6, -0.078],
    [0.7, -0.098],
    [0.8, -0.121],
    [0.9, -0.145],
    [1.0, -0.169],
    [1.1, -0.198],
    [1.2, -0.232],
];
