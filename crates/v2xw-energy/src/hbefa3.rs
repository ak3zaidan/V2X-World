//! `energy/emission/hbefa3` — the polynomial fit of HBEFA 3.1 that Krajzewicz et al. (2015)
//! published and SUMO ships as its `HBEFA3` emission model.
//!
//! # The model
//!
//! For each of 45 vehicle classes and six quantities (CO₂, CO, HC, fuel, NOₓ, PM), a
//! cubic in speed with two acceleration-power terms, fitted to the HBEFA 3.1 emission
//! factors:
//!
//! ```text
//! a' = a + g·sin θ                       (acceleration plus the grade's share of gravity)
//! E  = max(0, c0 + c1·a'·v + c2·a'²·v + c3·v + c4·v² + c5·v³) / 3.6     mg/s
//! ```
//!
//! with `v` in m/s and `a` in m/s². The `/3.6` and the units are SUMO's
//! (`HelpersHBEFA3::compute`, `scale = 3.6`, output mg/s); this crate reports g/s.
//!
//! # What differs from SUMO, and why
//!
//! * **No coasting cut-off.** SUMO returns zero when the deceleration is below a
//!   "coasting deceleration" from a linear fit whose own comment calls its constants magic
//!   numbers (`PollutantsInterface::Helper::getCoastingDecel`). This crate keeps the
//!   polynomial's own clamp at zero, which already returns zero under hard braking, and
//!   states the difference on the card: a mild deceleration emits here and is cut off in
//!   SUMO, so gently decelerating traffic reads slightly higher here.
//! * **Grade from the vehicle's own height change**, not a lane attribute: the provider
//!   differentiates the recorded `z` over the distance travelled.
//!
//! The coefficient table is data, transcribed by script from SUMO's published table; the
//! code is this crate's own.

use v2xw_core::card::{Family, ModelCard, SourceKind, Tier, Validation, ValidationStatus};
use v2xw_core::model::Model;

use crate::cite;
use crate::powertrain::{Fuel, Powertrain, Segment};
use crate::state::{DriveState, GRAVITY_MPS2, Rates};
use crate::traits::{EmissionClass, EmissionModel};

/// The model's id.
pub const MODEL_ID: &str = "energy/emission/hbefa3";
/// The model's version.
pub const MODEL_VERSION: &str = "1.0.0";

/// Quantity rows of a class's coefficient block, in SUMO's `EmissionType` order.
const CO2: usize = 0;
const CO: usize = 1;
const HC: usize = 2;
const FUEL: usize = 3;
const NOX: usize = 4;
const PM: usize = 5;

/// One class's coefficients.
#[derive(Debug)]
pub struct HbefaRow {
    /// The class's HBEFA name, as SUMO spells it.
    pub class: &'static str,
    /// A heavy-duty class (SUMO's `HEAVY_BIT`).
    pub heavy: bool,
    /// `c[quantity][k]`, quantities in the order CO₂, CO, HC, fuel, NOₓ, PM.
    pub c: [[f64; 6]; 6],
}

/// The HBEFA 3 emission model.
#[derive(Debug, Clone)]
pub struct Hbefa3 {
    card: ModelCard,
}

impl Default for Hbefa3 {
    fn default() -> Self {
        Self::new()
    }
}

impl Hbefa3 {
    /// The model with its card.
    #[must_use]
    pub fn new() -> Self {
        Self { card: card() }
    }

    /// Every class this model knows, in SUMO's table order.
    pub fn classes() -> impl Iterator<Item = &'static str> {
        TABLE.iter().map(|r| r.class)
    }

    /// The class by its HBEFA name.
    #[must_use]
    pub fn class_named(name: &str) -> Option<EmissionClass> {
        TABLE
            .iter()
            .position(|r| r.class == name)
            .map(|index| EmissionClass {
                index,
                name: name.to_string(),
            })
    }
}

impl Model for Hbefa3 {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl EmissionModel for Hbefa3 {
    fn resolve(&self, p: &Powertrain) -> Option<EmissionClass> {
        let fuel = match p.fuel {
            Fuel::Gasoline => "G",
            Fuel::Diesel => "D",
            Fuel::Electric => return None,
        };
        let euro = p.euro.min(6);
        let name = match p.segment {
            Segment::PassengerCar => format!("PC_{fuel}_EU{euro}"),
            Segment::LightCommercial => format!("LDV_{fuel}_EU{euro}"),
            Segment::HeavyTruck => match p.fuel {
                Fuel::Diesel => format!("HDV_D_EU{euro}"),
                // HBEFA 3 has one aggregate petrol truck class.
                _ => "HDV_G".to_string(),
            },
            Segment::UrbanBus => "Bus".to_string(),
            Segment::Coach => "Coach".to_string(),
            // SUMO's fit of HBEFA 3 has no two-wheeler classes.
            Segment::Motorcycle => return None,
        };
        Self::class_named(&name)
    }

    fn rates(&self, class: &EmissionClass, s: &DriveState) -> Rates {
        let Some(row) = TABLE.get(class.index) else {
            return Rates::default();
        };
        let v = s.v_mps.max(0.0);
        let a2 = s.a_mps2 + GRAVITY_MPS2 * s.sin_incline();
        let eval = |q: usize| -> f64 {
            let f = &row.c[q];
            let mg_s = (f[0] + f[1] * a2 * v + f[2] * a2 * a2 * v + f[3] * v + f[4] * v * v
                + f[5] * v * v * v)
                / 3.6;
            mg_s.max(0.0) / 1000.0
        };
        Rates {
            co2_g_s: eval(CO2),
            fuel_g_s: eval(FUEL),
            nox_g_s: eval(NOX),
            pm_g_s: eval(PM),
            co_g_s: eval(CO),
            hc_g_s: eval(HC),
        }
    }
}

/// The model's card.
#[must_use]
pub fn card() -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Emission,
        MODEL_VERSION,
        "Tailpipe CO2, CO, HC, NOx, PM and fuel of a combustion-engined vehicle from its \
         speed, acceleration and the road grade, by the polynomial fit of the HBEFA 3.1 \
         emission factors (Krajzewicz et al. 2015) - the model SUMO ships as HBEFA3. 45 \
         classes: passenger cars and light commercial vehicles, petrol and diesel, Euro 0 \
         to 6; heavy trucks, urban buses and coaches.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium];
    card.equations = vec![
        cite::eq(
            "effective acceleration",
            "a' = a + g sin(atan(grade))",
            "g = 9.80665 m/s2; a is the interval acceleration (crate::state)",
        ),
        cite::eq(
            "emission rate",
            "E = max(0, c0 + c1 a' v + c2 a'^2 v + c3 v + c4 v^2 + c5 v^3) / 3.6  [mg/s]",
            "v in m/s; one coefficient row per class and quantity; the fuel row is fuel mass",
        ),
    ];
    let mut coefficients = cite::param("coefficients", "mg/s per term", 0.0, cite::krajzewicz_2015());
    coefficients.default = serde_json::json!("45 classes x 6 quantities x 6 terms (table)");
    card.parameters = vec![coefficients];
    card.assumptions = vec![
        "a vehicle's class is its HBEFA class; the fleet section of the scenario chooses \
         segment, fuel and Euro standard per vehicle"
            .to_string(),
        "hot running emissions: the engine and after-treatment are at operating temperature"
            .to_string(),
    ];
    card.limitations = vec![
        "a polynomial fit: outside the speeds and accelerations of the HBEFA driving \
         situations it was fitted to it extrapolates"
            .to_string(),
        "no coasting fuel cut-off beyond the clamp at zero; SUMO's coasting rule is not \
         reproduced (see the module documentation)"
            .to_string(),
        "no two-wheeler classes; a motorcycle is unmodelled under this model".to_string(),
    ];
    card.ignores = vec![
        "cold-start excess emissions".to_string(),
        "evaporative emissions".to_string(),
        "non-exhaust PM (brake, tyre and road wear)".to_string(),
        "ambient temperature and air-conditioning load".to_string(),
        "deterioration with mileage".to_string(),
    ];
    card.sources = vec![
        cite::krajzewicz_2015(),
        cite::sumo_hbefa3_code(),
        cite::src(
            SourceKind::Dataset,
            "INFRAS (2010) HBEFA - Handbook Emission Factors for Road Transport, version 3.1",
            "the emission factors the polynomials were fitted to; not redistributed here",
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

/// The coefficient table, in SUMO's order (`HelpersHBEFA3::myFunctionParameter`).
pub static TABLE: &[HbefaRow] = &[
    HbefaRow {
        class: "LDV",
        heavy: false,
        c: [
            [7192.0, 1022.0, 0.0, -161.0, 23.33, 0.0],
            [961.4, 69.4, 0.0, -114.3, 3.851, 0.0],
            [59.65, 1.963, 0.0, -5.305, 0.159, 0.0],
            [2294.0, 323.8, 0.0, -52.78, 7.43, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [5.695, 0.2347, 0.0, -0.8032, 0.05115, -0.0007435],
        ],
    },
    HbefaRow {
        class: "LDV_G_EU0",
        heavy: false,
        c: [
            [10450.0, 1105.0, 0.0, -430.5, 29.24, 0.0],
            [956.6, 219.3, 0.0, -41.98, 0.0, 0.13],
            [80.96, 4.731, 0.0, -4.85, 0.1828, 0.0],
            [3332.0, 352.5, 0.0, -137.3, 9.328, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.243, 0.2422, 0.0, -0.1785, 0.008536, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_G_EU1",
        heavy: false,
        c: [
            [4906.0, 1144.0, 0.0, 295.4, 0.0, 0.3967],
            [2176.0, 243.2, 0.0, -303.2, 11.33, 0.0],
            [26.97, 3.78, 0.0, -3.277, 0.1403, 0.0],
            [1565.0, 365.0, 0.0, 94.21, 0.0, 0.1266],
            [19.35, 3.279, 0.0, 0.8384, 0.0, 0.001006],
            [0.915, 0.18, 0.2317, -0.1635, 0.006222, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_G_EU2",
        heavy: false,
        c: [
            [5122.0, 1131.0, 0.0, 277.6, 0.0, 0.391],
            [1296.0, 123.5, 0.0, -174.8, 6.212, 0.0],
            [10.94, 1.322, 0.0, -1.316, 0.05277, 0.0],
            [1634.0, 360.9, 0.0, 88.54, 0.0, 0.1247],
            [6.574, 1.161, 0.0, 0.3115, 0.0, 0.0003318],
            [1.988, 0.3357, 0.0, -0.2804, 0.01234, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_G_EU3",
        heavy: false,
        c: [
            [5363.0, 1115.0, 0.0, 264.0, 0.0, 0.394],
            [809.6, 64.08, 0.0, -108.0, 3.632, 0.0],
            [3.502, 0.3612, 0.0, -0.4307, 0.01612, 0.0],
            [1710.0, 355.8, 0.0, 84.18, 0.0, 0.1257],
            [0.5866, 0.2926, 0.0, 0.1955, 0.0, 0.0],
            [0.7376, 0.09247, 0.0, -0.0981, 0.004036, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_G_EU4",
        heavy: false,
        c: [
            [5056.0, 1015.0, 0.0, 286.5, 0.0, 0.3172],
            [567.4, 43.43, 0.0, -76.69, 2.618, 0.0],
            [2.052, 0.1911, 0.0, -0.2652, 0.009888, 0.0],
            [1613.0, 323.8, 0.0, 91.39, 0.0, 0.1012],
            [1.1, 0.1892, 0.0, 0.08809, 0.0, 0.0],
            [0.5892, 0.06059, 0.0, -0.07563, 0.002986, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_G_EU5",
        heavy: false,
        c: [
            [4635.0, 985.8, 0.0, 296.2, 0.0, 0.3007],
            [639.8, 43.92, 0.0, -85.21, 2.829, 0.0],
            [2.248, 0.1968, 0.0, -0.2896, 0.01057, 0.0],
            [1479.0, 314.4, 0.0, 94.47, 0.0, 0.0959],
            [1.101, 0.1824, 0.0, 0.08517, 0.0, 0.0],
            [0.6097, 0.06993, 0.0, -0.08108, 0.003213, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_G_EU6",
        heavy: false,
        c: [
            [4423.0, 927.9, 0.0, 279.0, 0.0, 0.2844],
            [645.1, 41.88, 0.0, -85.32, 2.798, 0.0],
            [2.311, 0.1872, 0.0, -0.2952, 0.01056, 0.0],
            [1411.0, 296.0, 0.0, 88.98, 0.0, 0.09072],
            [1.109, 0.1806, 0.0, 0.08319, 0.0, 0.0],
            [0.6029, 0.06641, 0.0, -0.08017, 0.003149, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_G_East",
        heavy: false,
        c: [
            [8365.0, 832.4, 0.0, -368.6, 22.94, 0.0],
            [2266.0, 172.8, 0.0, -235.2, 8.393, 0.0],
            [2226.0, 46.86, 0.0, -212.9, 5.895, 0.0],
            [2668.0, 265.5, 0.0, -117.6, 7.317, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_D_EU0",
        heavy: false,
        c: [
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [7.189, 2.276, 0.0, 2.542, 0.0, 0.0],
            [4.995, 0.2038, 0.0, 0.4551, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [53.84, 11.77, 0.0, -3.702, 0.3365, 0.0],
            [17.82, 0.9409, 0.0, -1.8, 0.1364, -0.001849],
        ],
    },
    HbefaRow {
        class: "LDV_D_EU1",
        heavy: false,
        c: [
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [4.958, 1.428, 0.0, 1.624, 0.0, 0.0],
            [2.849, 0.1092, 0.0, 0.2372, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [45.13, 10.42, 0.0, -2.941, 0.2852, 0.0],
            [15.3, 0.6367, 0.0, -2.007, 0.1254, -0.001671],
        ],
    },
    HbefaRow {
        class: "LDV_D_EU2",
        heavy: false,
        c: [
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.437, 0.4449, 0.0, 0.4966, 0.0, 0.0],
            [1.262, 0.04612, 0.0, 0.1011, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [42.83, 9.371, 0.0, -2.973, 0.2623, 0.0],
            [6.58, 0.2888, 0.0, -0.7254, 0.04933, -0.0006649],
        ],
    },
    HbefaRow {
        class: "LDV_D_EU3",
        heavy: false,
        c: [
            [3779.0, 902.0, 0.0, 288.6, 0.0, 0.3084],
            [1.94, 0.02085, 0.0, -0.03984, 0.0, 0.0],
            [0.4687, 0.02017, 0.0, -0.02515, 0.00116, 0.0],
            [1189.0, 283.7, 0.0, 90.77, 0.0, 0.09701],
            [33.02, 6.747, 1.706, -1.94, 0.1688, 0.0],
            [1.553, 0.2034, 0.0, -0.09003, 0.006345, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_D_EU4",
        heavy: false,
        c: [
            [3682.0, 915.4, 0.0, 331.0, 0.0, 0.306],
            [1.724, 0.03099, 0.0, -0.02995, 0.0, 0.0],
            [0.03577, 0.01844, 0.0, 0.02506, 0.0, 0.0],
            [1158.0, 288.0, 0.0, 104.1, 0.0, 0.09624],
            [40.68, 6.272, 0.0, -3.613, 0.2148, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_D_EU5",
        heavy: false,
        c: [
            [3505.0, 918.8, 0.0, 319.7, 0.0, 0.2978],
            [1.135, 0.01769, 0.0, -0.01882, 0.0, 0.0],
            [0.01997, 0.01916, 0.0, 0.02654, 0.0, 0.0],
            [1102.0, 289.0, 0.0, 100.6, 0.0, 0.09367],
            [38.6, 6.079, 0.0, -3.414, 0.2026, 0.0],
            [0.1111, 0.007346, 0.005486, -0.009925, 0.0004092, 0.0],
        ],
    },
    HbefaRow {
        class: "LDV_D_EU6",
        heavy: false,
        c: [
            [3374.0, 880.2, 0.0, 308.9, 0.0, 0.2798],
            [1.651, 0.03157, 0.0, -0.02797, 0.0, 0.0],
            [0.03311, 0.01971, 0.0, 0.02401, 0.0, 0.0],
            [1061.0, 276.9, 0.0, 97.17, 0.0, 0.088],
            [13.57, 2.119, 0.0, -1.211, 0.07068, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "PC",
        heavy: false,
        c: [
            [9034.0, 925.8, 0.0, -394.3, 25.71, 0.0],
            [428.7, 37.11, 0.0, -40.02, 1.494, 0.0],
            [48.97, 1.325, 0.0, -3.261, 0.1002, 0.0],
            [2937.0, 301.0, 0.0, -128.6, 8.373, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [1.021, 0.1731, 0.0, -0.03389, 0.0, 0.0001301],
        ],
    },
    HbefaRow {
        class: "PC_Alternative",
        heavy: false,
        c: [
            [7932.0, 785.0, 0.0, -393.0, 23.68, 0.0],
            [505.6, 20.69, 0.0, -62.72, 1.831, 0.0],
            [3.917, 0.2646, 0.0, -0.4245, 0.01417, 0.0],
            [3040.0, 300.9, 0.0, -150.6, 9.077, 0.0],
            [7.5, 0.563, 0.0, -0.6094, 0.02294, 0.0],
            [0.6383, 0.05188, 0.0, -0.08341, 0.002963, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_EU0",
        heavy: false,
        c: [
            [11720.0, 1160.0, 0.0, -580.5, 34.98, 0.0],
            [738.6, 83.85, 0.0, -52.74, 2.41, 0.0],
            [26.96, 2.829, 0.0, 2.118, 0.0, 0.0],
            [3737.0, 369.9, 0.0, -185.2, 11.16, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [2.159, 0.1897, 0.0, -0.2832, 0.00997, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_EU1",
        heavy: false,
        c: [
            [10930.0, 1081.0, 0.0, -541.4, 32.63, 0.0],
            [291.2, 24.14, 0.0, -32.65, 1.125, 0.0],
            [14.58, 1.01, 0.0, -1.172, 0.03809, 0.0],
            [3486.0, 345.0, 0.0, -172.7, 10.41, 0.0],
            [30.63, 2.031, 0.0, -3.595, 0.2137, -0.002299],
            [1.371, 0.09943, 0.0, -0.1742, 0.005969, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_EU2",
        heavy: false,
        c: [
            [10390.0, 1028.0, 0.0, -514.9, 31.03, 0.0],
            [620.1, 35.08, 0.0, -78.49, 2.429, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [3315.0, 328.0, 0.0, -164.2, 9.897, 0.0],
            [23.05, 1.089, 0.0, -2.081, 0.06925, 0.0],
            [2.371, 0.18, 0.0, -0.3078, 0.01044, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_EU3",
        heavy: false,
        c: [
            [9858.0, 975.6, 0.0, -488.4, 29.43, 0.0],
            [597.5, 32.52, 0.0, -75.27, 2.305, 0.0],
            [2.303, 0.25, 0.0, -0.2555, 0.00992, 0.0],
            [3144.0, 311.2, 0.0, -155.8, 9.388, 0.0],
            [7.204, 0.3327, 0.0, -0.6641, 0.0215, 0.0],
            [0.714, 0.05158, 0.0, -0.08903, 0.00308, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_EU4",
        heavy: false,
        c: [
            [9449.0, 938.4, 0.0, -467.1, 28.26, 0.0],
            [593.2, 19.32, 0.0, -73.25, 2.086, 0.0],
            [2.923, 0.1113, 0.0, -0.3476, 0.01032, 0.0],
            [3014.0, 299.3, 0.0, -149.0, 9.014, 0.0],
            [4.336, 0.4428, 0.0, -0.3204, 0.01371, 0.0],
            [0.2375, 0.0245, 0.0, -0.03251, 0.001325, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_EU5",
        heavy: false,
        c: [
            [8823.0, 886.0, 0.0, -432.9, 26.53, 0.0],
            [499.5, 16.93, 0.0, -61.37, 1.737, 0.0],
            [2.554, 0.1011, 0.0, -0.3009, 0.008911, 0.0],
            [2814.0, 282.6, 0.0, -138.1, 8.462, 0.0],
            [4.02, 0.4289, 0.0, -0.286, 0.01257, 0.0],
            [0.2786, 0.0241, 0.0, -0.03718, 0.001396, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_EU6",
        heavy: false,
        c: [
            [8273.0, 845.1, 0.0, -401.1, 25.08, 0.0],
            [405.3, 13.7, 0.0, -49.38, 1.389, 0.0],
            [2.016, 0.08505, 0.0, -0.2338, 0.006998, 0.0],
            [2639.0, 269.6, 0.0, -127.9, 8.001, 0.0],
            [3.444, 0.4035, 0.0, -0.2313, 0.01086, 0.0],
            [0.3107, 0.02274, 0.0, -0.04042, 0.001421, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_G_East",
        heavy: false,
        c: [
            [9396.0, 929.9, 0.0, -465.5, 28.05, 0.0],
            [3034.0, 176.7, 0.0, -332.4, 10.88, 0.0],
            [1363.0, 16.62, 0.0, -133.4, 3.537, 0.0],
            [2997.0, 296.6, 0.0, -148.5, 8.948, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [2.394, 0.2126, 0.0, -0.3147, 0.01108, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_D_EU0",
        heavy: false,
        c: [
            [8212.0, 888.8, 0.0, -237.3, 19.77, 0.0],
            [84.56, 3.162, 0.0, -6.251, 0.1955, 0.0],
            [6.153, 0.1644, 0.0, 0.1236, 0.0, 0.0],
            [2583.0, 279.6, 0.0, -74.65, 6.22, 0.0],
            [28.84, 5.756, 0.0, -1.228, 0.1234, 0.0],
            [18.43, 0.7548, 0.0, -1.834, 0.06254, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_D_EU1",
        heavy: false,
        c: [
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [2.811, 1.004, 0.0, 0.9606, 0.0, 0.0],
            [1.989, 0.06299, 0.0, 0.05963, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [43.38, 5.386, 0.0, -3.436, 0.1704, 0.0],
            [2.569, 0.4934, 0.0, 0.1002, 0.0, 0.0003083],
        ],
    },
    HbefaRow {
        class: "PC_D_EU2",
        heavy: false,
        c: [
            [4485.0, 760.9, 0.0, 160.0, 0.0, 0.2488],
            [6.66, 0.08232, 0.0, 0.01522, 0.0, 0.0],
            [1.145, 0.01858, 0.0, 0.04206, -0.0009262, 0.0],
            [1411.0, 239.3, 0.0, 50.32, 0.0, 0.07827],
            [58.93, 5.709, 0.0, -5.174, 0.2186, 0.0],
            [1.496, 0.2026, 0.0, 0.0777, 0.0, 0.0001067],
        ],
    },
    HbefaRow {
        class: "PC_D_EU3",
        heavy: false,
        c: [
            [4234.0, 691.1, 0.0, 149.4, 0.0, 0.2161],
            [4.436, 0.04963, 0.0, -0.1117, 0.0, 0.0],
            [0.3601, 0.04356, 0.0, 0.02848, 0.0, 0.0],
            [1332.0, 217.4, 0.0, 47.0, 0.0, 0.06797],
            [65.19, 7.25, 0.0, -6.12, 0.2648, 0.0],
            [1.773, 0.175, 0.0, -0.1295, 0.006047, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_D_EU4",
        heavy: false,
        c: [
            [6824.0, 742.3, 0.0, -195.3, 16.46, 0.0],
            [2.105, 0.02898, 0.0, -0.03593, 0.0, 0.0],
            [0.1208, 0.01649, 0.0, 0.02261, 0.0, 0.0],
            [2147.0, 233.5, 0.0, -61.45, 5.178, 0.0],
            [47.45, 4.011, 0.0, -4.061, 0.1619, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_D_EU5",
        heavy: false,
        c: [
            [4125.0, 686.1, 0.0, 153.1, 0.0, 0.2125],
            [1.319, 0.02428, 0.0, -0.01683, 0.0, 0.0],
            [0.1285, 0.0168, 0.0, 0.02269, 0.0, 0.0],
            [1298.0, 215.8, 0.0, 48.15, 0.0, 0.06684],
            [46.01, 4.064, 0.0, -3.872, 0.1567, 0.0],
            [0.6729, 0.02248, 0.0, -0.0764, 0.002153, 0.0],
        ],
    },
    HbefaRow {
        class: "PC_D_EU6",
        heavy: false,
        c: [
            [3946.0, 661.8, 0.0, 149.8, 0.0, 0.2041],
            [2.096, 0.02906, 0.0, -0.03578, 0.0, 0.0],
            [0.1368, 0.01692, 0.0, 0.02037, 0.0, 0.0],
            [1241.0, 208.2, 0.0, 47.1, 0.0, 0.06419],
            [16.31, 1.39, 0.0, -1.391, 0.05512, 0.0],
            [0.553, 0.01902, 0.0, -0.06184, 0.001758, 0.0],
        ],
    },
    HbefaRow {
        class: "Bus",
        heavy: true,
        c: [
            [19030.0, 6475.0, 0.0, 2073.0, 0.0, 0.0],
            [72.61, 7.482, 0.0, 0.6348, 0.0, 0.0],
            [17.46, 0.8473, 0.0, 0.254, 0.0, 0.0],
            [6016.0, 2049.0, 0.0, 656.6, 0.0, 0.0],
            [218.7, 46.17, 0.0, 11.27, 0.0, 0.0],
            [7.222, 0.8024, 0.0, 0.1201, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "Coach",
        heavy: true,
        c: [
            [16670.0, 7763.0, 0.0, 2610.0, 0.0, 0.0],
            [70.85, 7.73, 0.0, 1.306, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [5244.0, 2442.0, 0.0, 820.9, 0.0, 0.0],
            [226.7, 60.86, 0.0, 16.97, 0.0, 0.0],
            [8.297, 0.8085, 0.0, 0.1417, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV",
        heavy: true,
        c: [
            [26570.0, 7076.0, 0.0, 1753.0, 0.0, 0.0],
            [54.72, 7.864, 0.0, 1.669, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [8358.0, 2226.0, 0.0, 551.4, 0.0, 0.0],
            [305.6, 55.28, 0.0, 9.505, 0.0, 0.0],
            [7.952, 0.854, 0.0, 0.1195, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_G",
        heavy: true,
        c: [
            [7628.0, 3462.0, 0.0, 1058.0, 0.0, 0.9676],
            [464.0, 22.34, 0.0, -81.01, 5.955, -0.1182],
            [220.4, 3.65, 0.0, -13.27, 0.4346, 0.0],
            [2434.0, 1104.0, 0.0, 337.4, 0.0, 0.3087],
            [53.53, 33.43, 0.0, 11.47, 0.0, 0.007806],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_EU0",
        heavy: true,
        c: [
            [32510.0, 7256.0, 0.0, 1631.0, 0.0, 0.0],
            [52.89, 12.75, 0.0, 4.547, 0.0, 0.0],
            [27.82, 1.453, 0.0, 0.2468, 0.0, 0.0],
            [10230.0, 2283.0, 0.0, 513.1, 0.0, 0.0],
            [428.6, 104.2, 0.0, 24.18, 0.0, 0.0],
            [14.15, 2.335, 0.0, 0.6566, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_EU1",
        heavy: true,
        c: [
            [27490.0, 6533.0, 0.0, 1538.0, 0.0, 0.0],
            [58.3, 8.423, 0.0, 1.86, 0.0, 0.0],
            [19.78, 1.929, 0.0, 0.2819, 0.0, 0.0],
            [8647.0, 2055.0, 0.0, 483.7, 0.0, 0.0],
            [299.3, 67.56, 0.0, 14.89, 0.0, 0.0],
            [11.9, 1.744, 0.0, 0.3954, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_EU2",
        heavy: true,
        c: [
            [25370.0, 6723.0, 0.0, 1689.0, 0.0, 0.0],
            [41.4, 5.325, 0.0, 1.366, 0.0, 0.0],
            [13.69, 1.19, 0.0, 0.1655, 0.0, 0.0],
            [7980.0, 2115.0, 0.0, 531.2, 0.0, 0.0],
            [298.4, 67.21, 0.0, 15.36, 0.0, 0.0],
            [4.584, 0.789, 0.0, 0.2434, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_EU3",
        heavy: true,
        c: [
            [25980.0, 6712.0, 0.0, 1728.0, 0.0, 0.0],
            [51.47, 4.238, 0.0, 1.078, 0.0, 0.0],
            [12.25, 0.9033, 0.0, 0.1305, 0.0, 0.0],
            [8174.0, 2111.0, 0.0, 543.5, 0.0, 0.0],
            [241.1, 51.57, 0.0, 11.42, 0.0, 0.0],
            [5.436, 0.5336, 0.0, 0.1604, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_EU4",
        heavy: true,
        c: [
            [24290.0, 7180.0, 0.0, 1835.0, 0.0, 0.0],
            [50.68, 6.655, 0.0, 0.8349, 0.0, 0.0],
            [1.119, 0.1747, 0.0, 0.03217, 0.0, 0.0],
            [7639.0, 2259.0, 0.0, 577.1, 0.0, 0.0],
            [202.2, 42.34, 0.0, 8.858, 0.0, 0.0],
            [1.267, 0.131, 0.0, -0.006405, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_EU5",
        heavy: true,
        c: [
            [23960.0, 7231.0, 0.0, 1868.0, 0.0, 0.0],
            [51.51, 6.785, 0.0, 0.8496, 0.0, 0.0],
            [1.124, 0.175, 0.0, 0.03241, 0.0, 0.0],
            [7536.0, 2274.0, 0.0, 587.6, 0.0, 0.0],
            [164.8, 33.84, 0.0, 7.036, 0.0, 0.0],
            [1.305, 0.1324, 0.0, -0.007715, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_EU6",
        heavy: true,
        c: [
            [23530.0, 7227.0, 0.0, 1889.0, 0.0, 0.0],
            [30.78, 4.512, 0.0, 0.6383, 0.0, 0.0],
            [0.7469, 0.136, 0.0, 0.03002, 0.0, 0.0],
            [7400.0, 2273.0, 0.0, 594.2, 0.0, 0.0],
            [47.47, 11.68, 0.0, 2.737, 0.0, 0.0],
            [0.1279, 0.01668, 0.0, 0.00329, 0.0, 0.0],
        ],
    },
    HbefaRow {
        class: "HDV_D_East",
        heavy: true,
        c: [
            [20170.0, 6019.0, 0.0, 1723.0, 0.0, 0.0],
            [51.97, 16.95, 0.0, 6.523, 0.0, 0.0],
            [78.82, 2.36, 0.0, -4.421, 0.1487, 0.0],
            [6344.0, 1893.0, 0.0, 541.9, 0.0, 0.0],
            [299.7, 91.14, 0.0, 25.15, 0.0, 0.0],
            [13.59, 3.709, 0.0, 1.39, 0.0, 0.0],
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_is_complete_and_the_card_validates() {
        assert_eq!(TABLE.len(), 45);
        assert!(card().validate().is_ok());
        assert!(Hbefa3::class_named("PC_G_EU4").is_some());
        assert_eq!(TABLE.iter().filter(|r| r.heavy).count(), 12);
    }

    #[test]
    fn a_euro4_petrol_car_at_50_kmh_emits_what_the_fit_says() {
        // Hand evaluation of the PC_G_EU4 row at v = 13.889 m/s, a = 0:
        // CO2 = (9449 - 467.1 v + 28.26 v^2) / 3.6 mg/s.
        let m = Hbefa3::new();
        let c = Hbefa3::class_named("PC_G_EU4").unwrap();
        let v = 50.0 / 3.6;
        let r = m.rates(&c, &DriveState::between(v, v, 1.0));
        let want = (9449.0 - 467.1 * v + 28.26 * v * v) / 3.6 / 1000.0;
        assert!((r.co2_g_s - want).abs() < 1e-12);
        // ... about 168 g/km, a plausible steady 50 km/h figure for a Euro 4 petrol car.
        let g_per_km = r.co2_g_s / v * 1000.0;
        assert!((150.0..190.0).contains(&g_per_km), "{g_per_km} g/km");
        assert!(r.is_physical());
    }

    #[test]
    fn hard_braking_emits_nothing_and_climbing_emits_more() {
        let m = Hbefa3::new();
        let c = Hbefa3::class_named("PC_D_EU6").unwrap();
        let brake = m.rates(&c, &DriveState::between(15.0, 10.0, 1.0));
        assert_eq!(brake.co2_g_s, 0.0);
        let flat = m.rates(&c, &DriveState::between(15.0, 15.0, 1.0));
        let hill = m.rates(&c, &DriveState::between(15.0, 15.0, 1.0).with_grade(0.05));
        assert!(hill.co2_g_s > flat.co2_g_s * 1.3, "{hill:?} vs {flat:?}");
    }
}
