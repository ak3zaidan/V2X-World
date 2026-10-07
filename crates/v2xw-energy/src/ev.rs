//! Battery-electric vehicles: two traction-energy models and the battery they draw on.
//!
//! * [`Kurczveil`] — `energy/ev/kurczveil`, the energy balance of SUMO's battery device
//!   (Kurczveil, López & Schnieder 2014), with SUMO's default vehicle.
//! * [`VtCpem`] — `energy/ev/vt-cpem`, the Virginia Tech Comprehensive Power-based EV
//!   Energy model (Fiori, Ahn & Rakha 2016), parameterised for a Nissan Leaf, whose
//!   distinguishing feature is a regenerative-braking efficiency that depends on how hard
//!   the vehicle brakes.
//! * [`Battery`] — state of charge, energy out, energy regenerated, depletion.
//!
//! Both models take their parameters from a [`ParamSet`] the scenario can override by the
//! card's parameter names (`energy.ev.params`), so a bus, a van or a different car is a
//! scenario edit, not a code change; an unknown name is a validation error, never ignored.

use std::collections::BTreeMap;

use v2xw_core::card::{Family, ModelCard, Parameter, SourceKind, Tier, Validation, ValidationStatus};
use v2xw_core::model::Model;

use crate::cite;
use crate::state::{DriveState, GRAVITY_MPS2};
use crate::traits::EvEnergyModel;

/// Parameter values by name, starting from a card's defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamSet(BTreeMap<String, f64>);

impl ParamSet {
    /// The card's numeric defaults, overridden by `overrides`.
    ///
    /// # Errors
    /// A message naming the first override the card does not declare, or a non-finite
    /// value.
    pub fn resolve(card: &ModelCard, overrides: &BTreeMap<String, f64>) -> Result<Self, String> {
        let mut m: BTreeMap<String, f64> = card
            .parameters
            .iter()
            .filter_map(|p: &Parameter| p.default.as_f64().map(|v| (p.name.clone(), v)))
            .collect();
        for (k, v) in overrides {
            if !m.contains_key(k) {
                return Err(format!(
                    "{} has no parameter {k:?}; it has {:?}",
                    card.id,
                    m.keys().collect::<Vec<_>>()
                ));
            }
            if !v.is_finite() {
                return Err(format!("{}: parameter {k} must be finite, got {v}", card.id));
            }
            m.insert(k.clone(), *v);
        }
        Ok(Self(m))
    }

    /// One value; a name the card declares is always present.
    #[must_use]
    pub fn get(&self, name: &str) -> f64 {
        self.0.get(name).copied().unwrap_or(0.0)
    }
}

/// Wraps a heading change into `[-π, π]`.
fn wrap(rad: f64) -> f64 {
    let two_pi = 2.0 * core::f64::consts::PI;
    let mut x = rad % two_pi;
    if x > core::f64::consts::PI {
        x -= two_pi;
    } else if x < -core::f64::consts::PI {
        x += two_pi;
    }
    x
}

// --- Kurczveil 2014 ----------------------------------------------------------------------

/// `energy/ev/kurczveil`'s id.
pub const KURCZVEIL_ID: &str = "energy/ev/kurczveil";

/// The energy balance of SUMO's battery device.
#[derive(Debug, Clone)]
pub struct Kurczveil {
    card: ModelCard,
    p: ParamSet,
}

impl Kurczveil {
    /// The model with its card's defaults overridden by `overrides`.
    ///
    /// # Errors
    /// An override the card does not declare.
    pub fn new(overrides: &BTreeMap<String, f64>) -> Result<Self, String> {
        let card = kurczveil_card();
        let p = ParamSet::resolve(&card, overrides)?;
        Ok(Self { card, p })
    }
}

impl Model for Kurczveil {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl EvEnergyModel for Kurczveil {
    fn battery_power_w(&self, s: &DriveState) -> f64 {
        if s.dt_s <= 0.0 {
            return self.p.get("constant_power_w");
        }
        let p = &self.p;
        let m = p.get("mass_kg");
        let d = s.distance_m;
        let v = s.v_mps;
        // Energy over the interval, J.
        let kinetic = 0.5 * (m + p.get("rotating_mass_kg")) * (s.v1_mps * s.v1_mps - s.v0_mps * s.v0_mps);
        let potential = m * GRAVITY_MPS2 * s.sin_incline() * d;
        let air = 0.5 * p.get("air_density") * p.get("front_area_m2") * p.get("drag_coefficient") * v * v * d;
        let roll = p.get("roll_coefficient") * GRAVITY_MPS2 * m * d;
        let dtheta = wrap(s.heading_change_rad).abs();
        let curve = if dtheta > 0.0 && d > 0.0 {
            let r = (d / dtheta).clamp(1e-4, 1e4);
            p.get("radial_coefficient") * m * v * v / r * s.dt_s
        } else {
            0.0
        };
        let mut e = kinetic + potential + air + roll + curve;
        if e > 0.0 {
            e /= p.get("propulsion_efficiency");
        } else {
            e *= p.get("recuperation_efficiency");
            let alpha = p.get("recuperation_decel_alpha");
            if s.a_mps2 != 0.0 && alpha > 0.0 {
                e *= v2xw_core::math::exp(-alpha / s.a_mps2.abs());
            }
        }
        e / s.dt_s + p.get("constant_power_w")
    }

    fn capacity_wh(&self) -> f64 {
        self.p.get("capacity_wh")
    }
}

/// `energy/ev/kurczveil`'s card.
#[must_use]
pub fn kurczveil_card() -> ModelCard {
    let mut card = ModelCard::new(
        KURCZVEIL_ID,
        Family::Energy,
        "1.0.0",
        "Traction-battery energy of a battery-electric vehicle by the energy balance of \
         SUMO's battery device (Kurczveil et al. 2014): kinetic, potential and rotational \
         energy change plus air, rolling and curve losses, divided by the propulsion \
         efficiency when positive and multiplied by the recuperation efficiency when \
         negative, plus constant consumers. SUMO's default vehicle.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium];
    card.equations = vec![
        cite::eq(
            "energy balance",
            "dE = 1/2 (m + m_rot)(v1^2 - v0^2) + m g sin(theta) s + 1/2 rho A cd v^2 s + c_r m g s \
             + c_rad m v^2 / r dt",
            "s distance, r = s/|dheading| clamped to [1e-4, 1e4] m, v the interval's mean speed",
        ),
        cite::eq(
            "efficiencies",
            "dE > 0: E_bat = dE / eta_prop;  dE <= 0: E_bat = dE eta_recup exp(-alpha/|a|)",
            "alpha = 0 by default (SUMO's recuperationEfficiencyByDeceleration); alpha > 0 \
             applies Fiori et al. 2016's deceleration-dependent regeneration",
        ),
        cite::eq("battery power", "P = E_bat / dt + P_const", "W"),
    ];
    let sumo = cite::sumo_energy_code();
    card.parameters = vec![
        cite::param("mass_kg", "kg", 1830.0, sumo.clone()),
        cite::param("front_area_m2", "m2", 2.6, sumo.clone()),
        cite::param("drag_coefficient", "-", 0.35, sumo.clone()),
        cite::param("rotating_mass_kg", "kg", 40.0, sumo.clone()),
        cite::param("roll_coefficient", "-", 0.01, sumo.clone()),
        cite::param("radial_coefficient", "-", 0.1, sumo.clone()),
        cite::param("constant_power_w", "W", 100.0, sumo.clone()),
        cite::param("propulsion_efficiency", "-", 0.98, sumo.clone()),
        cite::param("recuperation_efficiency", "-", 0.96, sumo.clone()),
        cite::param("recuperation_decel_alpha", "m/s2", 0.0, sumo.clone()),
        cite::param(
            "air_density",
            "kg/m3",
            1.2041,
            cite::src(
                SourceKind::Code,
                "Eclipse SUMO, src/utils/emissions/HelpersEnergy.cpp",
                "dry air at 20 C, as the code comment states",
            ),
        ),
        cite::param("capacity_wh", "Wh", 35000.0, sumo.clone()),
    ];
    for p in &mut card.parameters {
        let range = match p.name.as_str() {
            "propulsion_efficiency" | "recuperation_efficiency" => Some((0.0, 1.0)),
            "mass_kg" | "capacity_wh" => Some((1.0, 1e6)),
            _ => None,
        };
        if let Some((lo, hi)) = range {
            p.range = Some(vec![serde_json::json!(lo), serde_json::json!(hi)]);
        }
    }
    card.assumptions = vec![
        "the interval's kinetic-energy change is exact (crate::state); SUMO reconstructs the \
         previous speed as v - a dt, which is the same thing for its Euler update"
            .to_string(),
    ];
    card.limitations = vec![
        "constant efficiencies: no motor efficiency map, no dependence on torque or speed"
            .to_string(),
        "SUMO's default vehicle is a generic car; set the parameters for a specific vehicle"
            .to_string(),
    ];
    card.ignores = vec![
        "battery internal resistance and temperature".to_string(),
        "HVAC load beyond the constant consumers".to_string(),
        "charging (no charging stations in this engine yet)".to_string(),
    ];
    card.sources = vec![cite::kurczveil_2014(), sumo];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![cite::epa_cycles()],
        tests: vec![
            "tests/models.rs::both_ev_models_land_in_the_cited_band_on_udds_and_hwfet".to_string(),
            "ev::tests::a_closed_loop_returns_only_what_the_losses_did_not_take".to_string(),
        ],
    };
    card
}

// --- VT-CPEM (Fiori 2016) ----------------------------------------------------------------

/// `energy/ev/vt-cpem`'s id.
pub const VT_CPEM_ID: &str = "energy/ev/vt-cpem";

/// The Virginia Tech Comprehensive Power-based EV Energy model.
#[derive(Debug, Clone)]
pub struct VtCpem {
    card: ModelCard,
    p: ParamSet,
}

impl VtCpem {
    /// The model with its card's defaults overridden by `overrides`.
    ///
    /// # Errors
    /// An override the card does not declare.
    pub fn new(overrides: &BTreeMap<String, f64>) -> Result<Self, String> {
        let card = vt_cpem_card();
        let p = ParamSet::resolve(&card, overrides)?;
        Ok(Self { card, p })
    }

    /// Power at the wheels, W.
    #[must_use]
    pub fn wheel_power_w(&self, s: &DriveState) -> f64 {
        let p = &self.p;
        let m = p.get("mass_kg");
        let v = s.v_mps;
        let v_kmh = v * 3.6;
        let rolling = m * GRAVITY_MPS2 * s.cos_incline() * p.get("rolling_cr") / 1000.0
            * (p.get("rolling_c1") * v_kmh + p.get("rolling_c2"));
        let rolling = if v > 0.0 { rolling } else { 0.0 };
        let air = 0.5 * p.get("air_density") * p.get("frontal_area_m2") * p.get("drag_coefficient") * v * v;
        let force = m * s.a_mps2 + rolling + air + m * GRAVITY_MPS2 * s.sin_incline();
        force * v
    }
}

impl Model for VtCpem {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl EvEnergyModel for VtCpem {
    fn battery_power_w(&self, s: &DriveState) -> f64 {
        let p = &self.p;
        let wheels = self.wheel_power_w(s);
        let drive = p.get("driveline_efficiency") * p.get("motor_efficiency");
        let motor = if wheels >= 0.0 {
            wheels / drive
        } else {
            // Regeneration recovers less the gentler the braking (Fiori et al. 2016, eq. for
            // eta_rb); a negative wheel power with no deceleration (a constant-speed descent)
            // regenerates at the driveline and motor efficiency alone.
            let eta_rb = if s.a_mps2 < 0.0 {
                v2xw_core::math::exp(-p.get("regen_alpha") / s.a_mps2.abs())
            } else {
                1.0
            };
            wheels * drive * eta_rb
        };
        let eta_bat = p.get("battery_efficiency");
        let battery = if motor >= 0.0 { motor / eta_bat } else { motor * eta_bat };
        battery + p.get("auxiliary_power_w")
    }

    fn capacity_wh(&self) -> f64 {
        self.p.get("capacity_wh")
    }
}

/// `energy/ev/vt-cpem`'s card.
#[must_use]
pub fn vt_cpem_card() -> ModelCard {
    let mut card = ModelCard::new(
        VT_CPEM_ID,
        Family::Energy,
        "1.0.0",
        "Traction-battery power of a battery-electric vehicle by VT-CPEM (Fiori, Ahn & Rakha \
         2016): wheel power from inertia, rolling, aerodynamic and grade resistance (Rakha's \
         resistance terms), through driveline, motor and battery efficiencies, with a \
         regenerative-braking efficiency exp(-alpha/|a|) that recovers more the harder the \
         vehicle brakes. Parameterised for a 2011-2013 Nissan Leaf.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium];
    card.equations = vec![
        cite::eq(
            "wheel power",
            "P_w = (m a + m g cos(theta) Cr/1000 (c1 v_kmh + c2) + 1/2 rho A_f C_D v^2 + m g sin(theta)) v",
            "rolling term zero at standstill",
        ),
        cite::eq(
            "motor power",
            "P_w >= 0: P_m = P_w / (eta_D eta_EM);  P_w < 0: P_m = P_w eta_D eta_EM eta_rb,  \
             eta_rb = exp(-alpha/|a|) for a < 0, 1 otherwise",
            "",
        ),
        cite::eq(
            "battery power",
            "P_b = P_m / eta_bat (P_m >= 0) or P_m eta_bat (P_m < 0), + P_aux",
            "",
        ),
    ];
    let fiori = cite::src(
        SourceKind::Paper,
        "Fiori C., Ahn K., Rakha H.A. (2016) Applied Energy 168:257-268, Nissan Leaf \
         parameters",
        "value as this build recalls the paper's parameter table; not re-read from the \
         paper in this build - check it before citing results",
    );
    let rakha = cite::rakha_2001();
    card.parameters = vec![
        cite::param("mass_kg", "kg", 1521.0, fiori.clone()),
        cite::param("drag_coefficient", "-", 0.28, fiori.clone()),
        cite::param("frontal_area_m2", "m2", 2.3316, fiori.clone()),
        cite::param("rolling_cr", "-", 1.75, rakha.clone()),
        cite::param("rolling_c1", "h/km", 0.0328, rakha.clone()),
        cite::param("rolling_c2", "-", 4.575, rakha),
        cite::param(
            "air_density",
            "kg/m3",
            1.225,
            cite::src(SourceKind::Standard, "ISO 2533:1975 Standard Atmosphere, sea level", ""),
        ),
        cite::param("driveline_efficiency", "-", 0.92, fiori.clone()),
        cite::param("motor_efficiency", "-", 0.91, fiori.clone()),
        cite::todo(
            "battery_efficiency",
            "-",
            0.90,
            "battery round-trip share applied to VT-CPEM's motor power",
            "confirm against Fiori et al. 2016's parameter table; if the paper applies no \
             separate battery efficiency, set 1.0",
        ),
        cite::param("auxiliary_power_w", "W", 700.0, fiori.clone()),
        cite::param("regen_alpha", "m/s2", 0.0411, fiori.clone()),
        cite::param(
            "capacity_wh",
            "Wh",
            24000.0,
            cite::src(
                SourceKind::Datasheet,
                "Nissan Leaf (2011-2015, ZE0/AZE0) 24 kWh lithium-ion traction battery, \
                 manufacturer specification",
                "nominal capacity",
            ),
        ),
    ];
    for p in &mut card.parameters {
        let range = match p.name.as_str() {
            "driveline_efficiency" | "motor_efficiency" | "battery_efficiency" => Some((0.01, 1.0)),
            "mass_kg" | "capacity_wh" => Some((1.0, 1e6)),
            _ => None,
        };
        if let Some((lo, hi)) = range {
            p.range = Some(vec![serde_json::json!(lo), serde_json::json!(hi)]);
        }
    }
    card.assumptions = vec![
        "a negative wheel power without deceleration (a constant-speed descent) regenerates \
         at the driveline and motor efficiency with eta_rb = 1 - this build's reading where \
         the paper's eta_rb is stated for braking"
            .to_string(),
    ];
    card.limitations = vec![
        "constant efficiencies apart from the regeneration term".to_string(),
        "parameterised for one vehicle; a bus or van needs its own parameters".to_string(),
    ];
    card.ignores = vec![
        "battery temperature and ageing".to_string(),
        "HVAC load beyond the constant auxiliary power".to_string(),
    ];
    card.sources = vec![cite::fiori_2016(), cite::rakha_2001()];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![
            cite::epa_cycles(),
            cite::src(
                SourceKind::Dataset,
                "US EPA / DOE fueleconomy.gov, 2013 Nissan Leaf: 29 kWh/100 mi combined \
                 (wall-to-wheels, label value)",
                "the band the cycle test checks against, allowing for charger losses and \
                 the label's adjustment",
            ),
        ],
        tests: vec![
            "tests/models.rs::both_ev_models_land_in_the_cited_band_on_udds_and_hwfet".to_string(),
            "ev::tests::harder_braking_regenerates_a_larger_share".to_string(),
        ],
    };
    card
}

// --- Battery -----------------------------------------------------------------------------

/// A traction battery's state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Battery {
    /// Usable capacity, Wh.
    pub capacity_wh: f64,
    /// Energy stored now, Wh.
    pub stored_wh: f64,
    /// Energy drawn, Wh (positive power integrated).
    pub out_wh: f64,
    /// Energy regenerated, Wh (negative power integrated, as a positive number).
    pub regen_wh: f64,
    /// Energy the vehicle needed after the battery was empty, Wh.
    pub shortfall_wh: f64,
}

impl Battery {
    /// A battery of `capacity_wh` at state of charge `soc`.
    #[must_use]
    pub fn new(capacity_wh: f64, soc: f64) -> Self {
        let cap = capacity_wh.max(1.0);
        Self {
            capacity_wh: cap,
            stored_wh: cap * soc.clamp(0.0, 1.0),
            out_wh: 0.0,
            regen_wh: 0.0,
            shortfall_wh: 0.0,
        }
    }

    /// Draws `power_w` for `dt_s` seconds (negative charges). A full battery takes no more
    /// charge; an empty one records the shortfall.
    pub fn apply(&mut self, power_w: f64, dt_s: f64) {
        let e = power_w * dt_s / 3600.0;
        if e >= 0.0 {
            self.out_wh += e;
            let take = e.min(self.stored_wh);
            self.stored_wh -= take;
            self.shortfall_wh += e - take;
        } else {
            let room = self.capacity_wh - self.stored_wh;
            let put = (-e).min(room);
            self.regen_wh += put;
            self.stored_wh += put;
        }
    }

    /// State of charge, `0..=1`.
    #[must_use]
    pub fn soc(&self) -> f64 {
        self.stored_wh / self.capacity_wh
    }

    /// True once the vehicle has needed energy the battery did not have.
    #[must_use]
    pub fn depleted(&self) -> bool {
        self.shortfall_wh > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn none() -> BTreeMap<String, f64> {
        BTreeMap::new()
    }

    #[test]
    fn both_cards_validate_and_overrides_are_checked() {
        assert!(kurczveil_card().validate().is_ok());
        assert!(vt_cpem_card().validate().is_ok());
        let mut bad = none();
        bad.insert("mass".to_string(), 1.0);
        assert!(Kurczveil::new(&bad).is_err());
        let mut ok = none();
        ok.insert("mass_kg".to_string(), 12000.0);
        let bus = Kurczveil::new(&ok).unwrap();
        let car = Kurczveil::new(&none()).unwrap();
        let s = DriveState::between(5.0, 7.0, 1.0);
        assert!(bus.battery_power_w(&s) > 5.0 * car.battery_power_w(&s));
    }

    #[test]
    fn a_closed_loop_returns_only_what_the_losses_did_not_take() {
        // Accelerate 0 -> 15 m/s and brake back to 0: the battery ends lower, and the
        // regenerated energy is less than the kinetic energy put in.
        let m = Kurczveil::new(&none()).unwrap();
        let mut b = Battery::new(m.capacity_wh(), 0.5);
        for i in 0..15 {
            let s = DriveState::between(f64::from(i), f64::from(i + 1), 1.0);
            b.apply(m.battery_power_w(&s), 1.0);
        }
        let after_accel = b.stored_wh;
        for i in (0..15).rev() {
            let s = DriveState::between(f64::from(i + 1), f64::from(i), 1.0);
            b.apply(m.battery_power_w(&s), 1.0);
        }
        assert!(b.regen_wh > 0.0);
        assert!(b.stored_wh > after_accel, "braking regenerated");
        assert!(b.stored_wh < 0.5 * m.capacity_wh(), "losses: {b:?}");
        let kinetic_wh = 0.5 * 1870.0 * 225.0 / 3600.0;
        assert!(b.regen_wh < kinetic_wh);
    }

    #[test]
    fn harder_braking_regenerates_a_larger_share() {
        let m = VtCpem::new(&none()).unwrap();
        // The same 10 -> 0 m/s stop, gently (over 10 s) and firmly (over 3 s): the
        // deceleration-dependent eta_rb recovers a larger share of the firm one's wheel
        // energy, so its net regeneration per joule of braking is higher.
        let share = |dt: f64| {
            let s = DriveState::between(10.0, 0.0, dt);
            let wheels = m.wheel_power_w(&s) * dt;
            let battery = (m.battery_power_w(&s) - 700.0) * dt;
            battery / wheels
        };
        assert!(share(3.0) > share(10.0), "{} vs {}", share(3.0), share(10.0));
        assert!(share(3.0) < 1.0);
    }

    #[test]
    fn the_battery_saturates_and_records_shortfall() {
        let mut b = Battery::new(10.0, 1.0);
        b.apply(-3600.0, 1.0);
        assert_eq!(b.stored_wh, 10.0);
        b.apply(3600.0 * 11.0, 1.0);
        assert_eq!(b.stored_wh, 0.0);
        assert!(b.depleted());
        assert!((b.shortfall_wh - 1.0).abs() < 1e-12);
    }

    #[test]
    fn wrap_keeps_the_short_way_round() {
        assert!((wrap(2.0 * core::f64::consts::PI - 0.1) + 0.1).abs() < 1e-12);
    }
}
