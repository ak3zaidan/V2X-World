//! `energy/emission/moves-opmode` — US EPA MOVES running-exhaust operating modes, with the
//! per-mode rates of MOVESTAR (Wang, Wu & Scora 2020).
//!
//! # The method
//!
//! MOVES does not evaluate a formula per second. It classifies each second of driving
//! into one of 23 **operating modes** by speed and **scaled tractive power** (STP, the
//! vehicle-specific power of Jiménez-Palacios generalised by a source type's road-load
//! coefficients), and multiplies the time spent in each mode by a rate per mode:
//!
//! ```text
//! STP = (A·v + B·v² + C·v³ + m·v·(a + g·sin θ)) / f          kW/tonne, v in m/s
//!
//! mode 0   braking   a ≤ −2 mph/s, or a < −1 mph/s for three consecutive seconds
//! mode 1   idle      −1 ≤ v < 1 mph
//! 11–16    1 ≤ v < 25 mph     STP < 0, 0–3, 3–6, 6–9, 9–12, ≥ 12
//! 21–30    25 ≤ v < 50 mph    STP < 0, 0–3, 3–6, 6–9, 9–12, 12–18 (27), 18–24, 24–30, ≥ 30
//! 33–40    v ≥ 50 mph         STP < 6, 6–12 (35), 12–18 (37), 18–24, 24–30, ≥ 30
//! ```
//!
//! # The rates
//!
//! MOVES's own rate tables are public (US federal work) but live in a multi-gigabyte
//! database; MOVESTAR distilled one light-duty vehicle and one light-duty truck table out
//! of it, per mode: CO, HC, NOₓ, elemental and organic PM2.5, energy and CO₂ in g/h (energy
//! in kJ/h). Those two tables are the defaults here, transcribed from the MOVESTAR
//! repository. Fuel follows MOVESTAR's carbon balance, `fuel = CO₂ · (12/44) · (13.78/12)`.
//!
//! The rates are fleet averages of the MOVES run MOVESTAR's authors made, not a Euro or
//! Tier class, so the fleet section's Euro standard and fuel do not change them; only the
//! segment chooses the table. Segments MOVESTAR has no table for (heavy-duty, bus, coach,
//! motorcycle) are unmodelled under this model.
//!
//! # One correction to MOVESTAR's Python port
//!
//! MOVESTAR's Python v1.4 looks the road-load row up by `VehicleType == veh_type`, which
//! selects the **motorcycle** row (`VehicleType` 1) for its light-duty vehicle. Its MATLAB
//! v1.1 — the reference implementation of the paper — uses the passenger-car row (A =
//! 0.156461, B = 0.002002, C = 0.000493, m = f = 1.4788) for a light-duty vehicle and the
//! passenger-truck row for a light-duty truck, and that is what this module does.
//!
//! # Operating-mode distribution
//!
//! [`op_mode`] is public: the share of time each link spends in each mode is the input a
//! MOVES project-level run takes, so a run of this engine can feed MOVES itself — with
//! MOVES's own, current rates for any calendar year and county — rather than only this
//! module's two default tables.

use v2xw_core::card::{Family, ModelCard, Tier, Validation, ValidationStatus};
use v2xw_core::model::Model;

use crate::cite;
use crate::powertrain::{Powertrain, Segment};
use crate::state::{DriveState, GRAVITY_MPS2, Rates};
use crate::traits::{EmissionClass, EmissionModel};

/// The model's id.
pub const MODEL_ID: &str = "energy/emission/moves-opmode";
/// The model's version.
pub const MODEL_VERSION: &str = "1.0.0";

/// Metres per second in one mile per hour.
const MPH: f64 = 0.44704;

/// A MOVES source type's road-load coefficients.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadLoad {
    /// Rolling term A, kW·s/m.
    pub a: f64,
    /// Rotating term B, kW·s²/m².
    pub b: f64,
    /// Drag term C, kW·s³/m³.
    pub c: f64,
    /// Source mass m, tonnes.
    pub mass_t: f64,
    /// Fixed mass factor f, tonnes.
    pub f_t: f64,
}

/// MOVES source type 21, passenger car.
pub const PASSENGER_CAR: RoadLoad = RoadLoad {
    a: 0.156461,
    b: 0.002002,
    c: 0.000493,
    mass_t: 1.4788,
    f_t: 1.4788,
};

/// MOVES source type 31, passenger truck.
pub const PASSENGER_TRUCK: RoadLoad = RoadLoad {
    a: 0.22112,
    b: 0.002838,
    c: 0.000698,
    mass_t: 1.86686,
    f_t: 1.86686,
};

/// Scaled tractive power, kW/tonne.
#[must_use]
pub fn stp(rl: &RoadLoad, s: &DriveState) -> f64 {
    let v = s.v_mps;
    (rl.a * v
        + rl.b * v * v
        + rl.c * v * v * v
        + rl.mass_t * v * (s.a_mps2 + GRAVITY_MPS2 * s.sin_incline()))
        / rl.f_t
}

/// Every running operating mode, in table order.
pub const OP_MODES: [u8; 23] = [
    0, 1, 11, 12, 13, 14, 15, 16, 21, 22, 23, 24, 25, 27, 28, 29, 30, 33, 35, 37, 38, 39, 40,
];

/// The MOVES operating mode of one interval.
#[must_use]
pub fn op_mode(rl: &RoadLoad, s: &DriveState) -> u8 {
    let a_mphs = s.a_mps2 / MPH;
    if a_mphs <= -2.0 || s.decel_run_s >= 3.0 {
        return 0;
    }
    let v_mph = s.v_mps / MPH;
    if v_mph < 1.0 {
        return 1;
    }
    let p = stp(rl, s);
    if v_mph < 25.0 {
        let edges = [0.0, 3.0, 6.0, 9.0, 12.0];
        11 + edges.iter().take_while(|e| p >= **e).count() as u8
    } else if v_mph < 50.0 {
        let modes = [21, 22, 23, 24, 25, 27, 28, 29, 30];
        let edges = [0.0, 3.0, 6.0, 9.0, 12.0, 18.0, 24.0, 30.0];
        modes[edges.iter().take_while(|e| p >= **e).count()]
    } else {
        let modes = [33, 35, 37, 38, 39, 40];
        let edges = [6.0, 12.0, 18.0, 24.0, 30.0];
        modes[edges.iter().take_while(|e| p >= **e).count()]
    }
}

/// The deceleration that MOVES's three-second braking rule counts, m/s² (1 mph/s).
pub const BRAKING_RUN_DECEL_MPS2: f64 = MPH;

/// One source type's road load and rate table.
#[derive(Debug)]
pub struct MovesTable {
    /// The class name reported for it.
    pub name: &'static str,
    /// Its road load.
    pub road_load: RoadLoad,
    /// Per mode, in [`OP_MODES`] order: CO, HC, NOₓ, PM2.5 elemental, PM2.5 organic
    /// (g/h), energy (kJ/h), CO₂ (g/h).
    pub rates: [[f64; 7]; 23],
}

impl MovesTable {
    /// The rates of one interval, g/s.
    #[must_use]
    pub fn rates(&self, s: &DriveState) -> Rates {
        let mode = op_mode(&self.road_load, s);
        let i = OP_MODES.iter().position(|m| *m == mode).unwrap_or(1);
        let r = &self.rates[i];
        let co2 = r[6] / 3600.0;
        Rates {
            co2_g_s: co2,
            fuel_g_s: co2 * 12.0 / 44.0 * 13.78 / 12.0,
            nox_g_s: r[2] / 3600.0,
            pm_g_s: (r[3] + r[4]) / 3600.0,
            co_g_s: r[0] / 3600.0,
            hc_g_s: r[1] / 3600.0,
        }
    }
}

/// The MOVES operating-mode emission model.
#[derive(Debug, Clone)]
pub struct MovesOpMode {
    card: ModelCard,
}

impl Default for MovesOpMode {
    fn default() -> Self {
        Self::new()
    }
}

impl MovesOpMode {
    /// The model with its card.
    #[must_use]
    pub fn new() -> Self {
        Self { card: card() }
    }

    /// The default tables.
    #[must_use]
    pub fn tables() -> &'static [MovesTable] {
        TABLES
    }
}

impl Model for MovesOpMode {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl EmissionModel for MovesOpMode {
    fn resolve(&self, p: &Powertrain) -> Option<EmissionClass> {
        if p.is_electric() {
            return None;
        }
        let index = match p.segment {
            Segment::PassengerCar => 0,
            Segment::LightCommercial => 1,
            _ => return None,
        };
        Some(EmissionClass {
            index,
            name: TABLES[index].name.to_string(),
        })
    }

    fn rates(&self, class: &EmissionClass, s: &DriveState) -> Rates {
        TABLES
            .get(class.index)
            .map_or_else(Rates::default, |t| t.rates(s))
    }
}

/// The model's card.
#[must_use]
pub fn card() -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Emission,
        MODEL_VERSION,
        "Tailpipe CO2, CO, HC, NOx, PM2.5 and fuel of a light-duty vehicle by the US EPA \
         MOVES method: each interval is binned into one of 23 running operating modes by \
         speed and scaled tractive power, and charged that mode's rate. Default rates: \
         MOVESTAR's light-duty vehicle and light-duty truck tables, distilled from MOVES.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium];
    card.equations = vec![
        cite::eq(
            "scaled tractive power",
            "STP = (A v + B v^2 + C v^3 + m v (a + g sin theta)) / f  [kW/tonne]",
            "v in m/s; A, B, C, m, f of the MOVES source type",
        ),
        cite::eq(
            "operating mode",
            "0 braking (a <= -2 mph/s, or a < -1 mph/s for 3 s); 1 idle (v < 1 mph); \
             11-16, 21-30, 33-40 by speed band and STP bin",
            "",
        ),
        cite::eq(
            "rate",
            "E = rate[mode] / 3600  [g/s];  fuel = CO2 (12/44)(13.78/12)",
            "PM2.5 = elemental + organic",
        ),
    ];
    let moves = cite::epa_moves_opmodes();
    card.parameters = vec![
        cite::param("passenger_car.A", "kW s/m", PASSENGER_CAR.a, moves.clone()),
        cite::param(
            "passenger_car.B",
            "kW s2/m2",
            PASSENGER_CAR.b,
            moves.clone(),
        ),
        cite::param(
            "passenger_car.C",
            "kW s3/m3",
            PASSENGER_CAR.c,
            moves.clone(),
        ),
        cite::param("passenger_car.m", "t", PASSENGER_CAR.mass_t, moves.clone()),
        cite::param("passenger_car.f", "t", PASSENGER_CAR.f_t, moves.clone()),
        cite::param(
            "passenger_truck.A",
            "kW s/m",
            PASSENGER_TRUCK.a,
            moves.clone(),
        ),
        cite::param(
            "passenger_truck.B",
            "kW s2/m2",
            PASSENGER_TRUCK.b,
            moves.clone(),
        ),
        cite::param(
            "passenger_truck.C",
            "kW s3/m3",
            PASSENGER_TRUCK.c,
            moves.clone(),
        ),
        cite::param(
            "passenger_truck.m",
            "t",
            PASSENGER_TRUCK.mass_t,
            moves.clone(),
        ),
        cite::param("passenger_truck.f", "t", PASSENGER_TRUCK.f_t, moves.clone()),
    ];
    let mut rates = cite::param("rates", "g/h per mode", 0.0, cite::movestar());
    rates.default = serde_json::json!("2 tables x 23 modes x 7 quantities (MOVESTAR)");
    card.parameters.push(rates);
    card.assumptions = vec![
        "the braking rule's three consecutive seconds are read as three seconds of \
         continuous deceleration beyond 1 mph/s, which is the same thing at the 1 Hz MOVES \
         assumes and well-defined at the engine's finer step"
            .to_string(),
        "the rates are MOVESTAR's fleet averages; the fleet section's Euro class and fuel do \
         not change them"
            .to_string(),
    ];
    card.limitations = vec![
        "light-duty only with the default tables; heavy-duty, bus, coach and motorcycle are \
         unmodelled"
            .to_string(),
        "the operating-mode rates are of one MOVES run (one calendar year, one fleet age \
         distribution); a study for a given year and county should run MOVES on the \
         operating-mode distribution instead"
            .to_string(),
    ];
    card.ignores = vec![
        "start and extended-idle emissions (MOVES start and hotelling processes)".to_string(),
        "evaporative, brake- and tyre-wear processes".to_string(),
        "temperature, humidity and fuel-formulation adjustments".to_string(),
    ];
    card.sources = vec![moves, cite::movestar()];
    card.validation = Validation {
        status: ValidationStatus::LiteratureChecked,
        references: vec![cite::epa_cycles()],
        tests: vec![
            "tests/models.rs::every_combustion_model_lands_in_the_cited_band_on_udds_and_hwfet"
                .to_string(),
            "moves::tests::the_operating_modes_follow_the_epa_bins".to_string(),
        ],
    };
    card
}

/// The default tables: MOVESTAR's `EmsRate_1.csv` (light-duty vehicle, with the passenger
/// car's road load) and `EmsRate_2.csv` (light-duty truck, with the passenger truck's).
static TABLES: &[MovesTable] = &[
    MovesTable {
        name: "MOVES_LDV",
        road_load: PASSENGER_CAR,
        rates: [
            [
                1.97892,
                0.0953798,
                0.0695436,
                0.00522486,
                0.0239643,
                48371.4,
                3441.528367,
            ],
            [
                0.341669,
                0.0231352,
                0.0294708,
                0.00450976,
                0.0206844,
                44749.1,
                3183.808967,
            ],
            [
                6.80345,
                0.0650524,
                0.103114,
                0.00447462,
                0.0205233,
                70367.0,
                5006.471316,
            ],
            [
                11.1072,
                0.0498479,
                0.157418,
                0.00470869,
                0.0215969,
                97164.0,
                6913.024272,
            ],
            [
                10.2406,
                0.0942146,
                0.368511,
                0.0066702,
                0.0305935,
                135036.0,
                9607.541328,
            ],
            [
                14.6935,
                0.128156,
                0.650607,
                0.00661505,
                0.0303405,
                170626.0,
                12139.69865,
            ],
            [
                21.3068,
                0.178659,
                1.15309,
                0.00636392,
                0.0291887,
                203467.0,
                14476.27012,
            ],
            [
                35.9513, 0.285382, 2.40478, 0.0165839, 0.0760634, 245748.0, 17484.4787,
            ],
            [
                8.86745,
                0.0973768,
                0.203915,
                0.00854285,
                0.0391826,
                95730.3,
                6811.019384,
            ],
            [
                11.7489,
                0.0894292,
                0.331209,
                0.0113997,
                0.0522858,
                108968.0,
                7752.855264,
            ],
            [
                15.1095,
                0.0964139,
                0.500717,
                0.00823071,
                0.0377509,
                132716.0,
                9442.477968,
            ],
            [
                22.0873,
                0.183979,
                0.843921,
                0.00906222,
                0.0415647,
                170266.0,
                12114.08537,
            ],
            [
                25.0671,
                0.18329,
                1.18354,
                0.0112721,
                0.0517003,
                227221.0,
                16166.31971,
            ],
            [
                37.6451,
                0.289654,
                1.86362,
                0.0180833,
                0.0829407,
                299543.0,
                21311.88536,
            ],
            [
                127.616,
                1.95524,
                6.85885,
                0.0395121,
                0.181226,
                403761.0,
                28726.78763,
            ],
            [
                270.27,
                3.47139,
                12.0423,
                0.189873,
                0.870871,
                553155.0,
                39355.87194,
            ],
            [
                949.244,
                5.73108,
                15.8433,
                0.281271,
                1.29007,
                694644.0,
                49422.53131,
            ],
            [
                6.65783,
                0.0935418,
                0.434612,
                0.0117726,
                0.053996,
                136387.0,
                9703.662276,
            ],
            [
                11.3684,
                0.129899,
                1.19894,
                0.0174424,
                0.0800013,
                218699.0,
                15559.99645,
            ],
            [
                16.7341,
                0.166455,
                1.67539,
                0.0133771,
                0.0613553,
                284892.0,
                20269.49602,
            ],
            [
                116.968,
                1.32031,
                5.8239,
                0.0327562,
                0.150239,
                371483.0,
                26430.27248,
            ],
            [
                123.421,
                1.91714,
                8.66952,
                0.0689814,
                0.31639,
                494808.0,
                35204.59958,
            ],
            [
                362.729,
                2.50657,
                10.9191,
                0.0796885,
                0.365499,
                630762.0,
                44877.45478,
            ],
        ],
    },
    MovesTable {
        name: "MOVES_LDT",
        road_load: PASSENGER_TRUCK,
        rates: [
            [
                2.36609,
                0.0391707,
                0.0538503,
                0.00131955,
                0.0180855,
                73883.5,
                5256.663258,
            ],
            [
                4.05557,
                0.000418121,
                0.0089791,
                0.00108472,
                0.014867,
                56979.3,
                4053.963236,
            ],
            [
                6.52187,
                0.0228921,
                0.146868,
                0.00155411,
                0.0213005,
                92164.5,
                6557.319846,
            ],
            [
                2.82379,
                0.0208503,
                0.155233,
                0.00156658,
                0.0214713,
                115308.0,
                8203.933584,
            ],
            [
                9.76815,
                0.052262,
                0.363034,
                0.00211212,
                0.0289484,
                165833.0,
                11798.68628,
            ],
            [
                14.2137,
                0.0725323,
                0.657844,
                0.00271017,
                0.0371452,
                210063.0,
                14945.56232,
            ],
            [
                20.8813,
                0.103686,
                1.18797,
                0.00612354,
                0.0839285,
                253242.0,
                18017.66182,
            ],
            [
                35.9857,
                0.171948,
                2.5348,
                0.017956,
                0.246103,
                316272.0,
                22502.12026,
            ],
            [
                5.8165,
                0.0387213,
                0.254133,
                0.00212064,
                0.0290653,
                123291.0,
                8771.908068,
            ],
            [
                9.33417,
                0.0462365,
                0.357951,
                0.00208597,
                0.02859,
                132348.0,
                9416.295504,
            ],
            [
                13.1798,
                0.0650499,
                0.508789,
                0.00271128,
                0.0371605,
                163386.0,
                11624.58713,
            ],
            [
                25.9039,
                0.115221,
                0.930889,
                0.00293965,
                0.0402905,
                211608.0,
                15055.48598,
            ],
            [
                18.5265, 0.124367, 1.52098, 0.00453134, 0.062106, 271723.0, 19332.548,
            ],
            [
                34.766,
                0.218173,
                2.14235,
                0.00870251,
                0.119276,
                374028.0,
                26611.34414,
            ],
            [
                200.661,
                3.25428,
                8.21223,
                0.0536606,
                0.735465,
                499605.0,
                35545.89654,
            ],
            [
                216.005,
                4.04662,
                11.1418,
                0.0769951,
                1.05529,
                684960.0,
                48733.53408,
            ],
            [
                969.878,
                7.11962,
                12.8433,
                0.245917,
                3.37051,
                807005.0,
                57416.79174,
            ],
            [
                10.9199,
                0.0703704,
                0.41958,
                0.00586679,
                0.0804096,
                178086.0,
                12670.46273,
            ],
            [
                23.846,
                0.0921036,
                1.40529,
                0.00900861,
                0.123471,
                272535.0,
                19390.32018,
            ],
            [
                28.9476,
                0.117916,
                2.28006,
                0.00918422,
                0.125878,
                360261.0,
                25631.84963,
            ],
            [
                68.1954, 1.15714, 7.50412, 0.0132642, 0.181797, 468102.0, 33304.5211,
            ],
            [
                167.612,
                2.11771,
                13.3438,
                0.0320607,
                0.43942,
                613382.0,
                43640.90254,
            ],
            [
                407.603,
                2.7341,
                14.3353,
                0.0359667,
                0.492955,
                811659.0,
                57747.91453,
            ],
        ],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    fn at(v_mph: f64, a_mphs: f64) -> DriveState {
        let v = v_mph * MPH;
        let a = a_mphs * MPH;
        // An interval of 0.1 s centred on v with acceleration a.
        DriveState::between(v - 0.05 * a, v + 0.05 * a, 0.1)
    }

    #[test]
    fn the_operating_modes_follow_the_epa_bins() {
        let rl = PASSENGER_CAR;
        assert_eq!(op_mode(&rl, &at(30.0, -2.5)), 0, "hard braking");
        assert_eq!(
            op_mode(&rl, &at(30.0, -1.5).with_decel_run(3.0)),
            0,
            "three seconds"
        );
        assert_eq!(
            op_mode(&rl, &at(30.0, -1.5).with_decel_run(1.0)),
            21,
            "not yet"
        );
        assert_eq!(op_mode(&rl, &at(0.5, 0.0)), 1, "idle");
        assert_eq!(op_mode(&rl, &at(10.0, 0.0)), 12, "slow cruise, STP in 0-3");
        assert_eq!(op_mode(&rl, &at(10.0, -0.5)), 11, "slow, coasting");
        assert_eq!(op_mode(&rl, &at(10.0, 3.0)), 14, "slow, accelerating");
        assert_eq!(op_mode(&rl, &at(30.0, 0.0)), 22, "30 mph cruise, STP 0-3");
        assert_eq!(op_mode(&rl, &at(60.0, 0.0)), 35, "60 mph cruise, STP 6-12");
        // STP at 30 mph cruise, by hand: v = 13.41 m/s;
        // (0.156461 v + 0.002002 v^2 + 0.000493 v^3) / 1.4788 = 2.467 kW/t.
        let p = stp(&rl, &at(30.0, 0.0));
        assert!((p - 2.467).abs() < 0.005, "{p}");
    }

    #[test]
    fn the_card_validates_and_rates_are_physical() {
        assert!(card().validate().is_ok());
        let m = MovesOpMode::new();
        for t in TABLES {
            let r = t.rates(&at(30.0, 0.0));
            assert!(r.is_physical());
        }
        let c = m
            .resolve(&Powertrain {
                segment: Segment::PassengerCar,
                fuel: crate::powertrain::Fuel::Gasoline,
                euro: 4,
            })
            .unwrap();
        assert_eq!(c.name, "MOVES_LDV");
    }
}
