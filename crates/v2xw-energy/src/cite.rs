//! The citations every card in this crate shares, and two small builders.
//!
//! One place, so a reference is spelled the same on every card that uses it and a
//! correction is made once.

use v2xw_core::card::{Equation, Parameter, Source, SourceKind};

/// The date the online sources below were read for this build.
pub const ACCESSED: &str = "2026-10-07";

/// A citation with an access date and a note.
pub fn src(kind: SourceKind, reference: &str, note: &str) -> Source {
    Source {
        kind,
        reference: reference.to_string(),
        accessed: Some(ACCESSED.to_string()),
        note: (!note.is_empty()).then(|| note.to_string()),
    }
}

/// A parameter with a cited numeric default.
pub fn param(name: &str, unit: &str, default: f64, source: Source) -> Parameter {
    Parameter::new(name, unit, serde_json::json!(default), source)
}

/// A parameter whose default is not yet cited, with its calibration plan (rule R1).
pub fn todo(name: &str, unit: &str, default: f64, what: &str, plan: &str) -> Parameter {
    let mut p = Parameter::new(
        name,
        unit,
        serde_json::json!(default),
        Source::todo_calibrate(what),
    );
    p.calibration = Some(plan.to_string());
    p
}

/// An equation with notes.
pub fn eq(name: &str, text: &str, notes: &str) -> Equation {
    Equation {
        name: name.to_string(),
        latex_or_text: text.to_string(),
        notes: (!notes.is_empty()).then(|| notes.to_string()),
    }
}

/// Krajzewicz et al. 2015 — the polynomial fit of HBEFA 3.1 that SUMO ships as `HBEFA3`.
pub fn krajzewicz_2015() -> Source {
    src(
        SourceKind::Paper,
        "Krajzewicz D., Behrisch M., Wagner P., Luz R., Krumnow M. (2015) Second Generation \
         of Pollutant Emission Models for SUMO. In: Behrisch M., Weber M. (eds) Modeling \
         Mobility with Open Data, Lecture Notes in Mobility, Springer",
        "the HBEFA 3.1 polynomial fits and their per-class coefficients",
    )
}

/// SUMO's `HelpersHBEFA3.cpp`, where the published fit coefficients are distributed.
pub fn sumo_hbefa3_code() -> Source {
    src(
        SourceKind::Code,
        "Eclipse SUMO, src/utils/emissions/HelpersHBEFA3.cpp (myFunctionParameter[45][6][6]), \
         https://github.com/eclipse-sumo/sumo, EPL-2.0 OR GPL-2.0-or-later",
        "coefficient table transcribed by script, 45 classes x 6 pollutants x 6 \
         coefficients; numeric data only, no code reused",
    )
}

/// Hausberger et al. — PHEM and the PHEMlight simplification of it.
pub fn hausberger_phemlight() -> Source {
    src(
        SourceKind::Paper,
        "Hausberger S., Rexeis M., Zallinger M., Luz R. (2009) Emission Factors from the \
         Model PHEM for the HBEFA Version 3. Report Nr. I-20/2009 Haus-Em 33/08/679, \
         Institute for Internal Combustion Engines and Thermodynamics, TU Graz",
        "PHEM's power-based engine-map method, of which PHEMlight is the normalised-power \
         simplification",
    )
}

/// SUMO's PHEMlight data files and their reader, where the published PHEMlight vehicles
/// are distributed.
pub fn sumo_phemlight_data() -> Source {
    src(
        SourceKind::Dataset,
        "Eclipse SUMO, data/emissions/PHEMlight/PC_G_EU4.{PHEMLight.veh,csv,_FC.csv} and \
         PC_D_EU4.*, with src/foreign/PHEMlight/cpp/CEP.cpp and Constants.cpp, \
         https://github.com/eclipse-sumo/sumo (TU Graz PHEMlight data, EPL-2.0)",
        "vehicle, road-load, gear, drag, fuel and pollutant tables transcribed; the CEP \
         algorithm re-implemented from its published description",
    )
}

/// The MOVES operating-mode method.
pub fn epa_moves_opmodes() -> Source {
    src(
        SourceKind::Standard,
        "US EPA, MOVES (Motor Vehicle Emission Simulator) technical documentation: \
         running-exhaust operating modes binned by speed and scaled tractive power (STP, \
         kW/tonne) from the source type's road-load coefficients A, B, C, mass m and \
         fixed mass factor f",
        "operating-mode definitions 0, 1, 11-16, 21-30, 33-40 and the scaled tractive power",
    )
}

/// MOVESTAR — the open-source distillation of MOVES rates used for the default tables.
pub fn movestar() -> Source {
    src(
        SourceKind::Paper,
        "Wang Z., Wu G., Scora G. (2020) MOVESTAR: An Open-Source Vehicle Fuel and Emission \
         Model based on USEPA MOVES. arXiv:2008.04986; tables from \
         https://github.com/ziranw/MOVESTAR-Fuel-and-Emission-Model (MOVESTAR_Python_v1.4/ \
         EmsRate_1.csv, EmsRate_2.csv, VehicleSrcCoeff.csv)",
        "per-operating-mode rates for a light-duty vehicle and a light-duty truck, derived \
         from MOVES (US federal work, public domain); the repository carries no licence file",
    )
}

/// Kurczveil et al. 2014 — the energy model of SUMO's battery device.
pub fn kurczveil_2014() -> Source {
    src(
        SourceKind::Paper,
        "Kurczveil T., López P.A., Schnieder E. (2014) Implementation of an Energy Model and \
         a Charging Infrastructure in SUMO. In: Behrisch M., Krajzewicz D., Weber M. (eds) \
         Simulation of Urban Mobility (SUMO 2013), LNCS 8594, Springer",
        "energy balance over one step: kinetic, potential, rotational, air, rolling and \
         curve losses, constant consumers, propulsion and recuperation efficiencies",
    )
}

/// SUMO's `HelpersEnergy`, where the Kurczveil model's defaults are distributed.
pub fn sumo_energy_code() -> Source {
    src(
        SourceKind::Code,
        "Eclipse SUMO, src/utils/emissions/HelpersEnergy.{h,cpp} (myDefault* constants) and \
         https://sumo.dlr.de/docs/Models/Electric.html",
        "default vehicle: 1830 kg, 2.6 m2, cd 0.35, rotating mass 40 kg, roll 0.01, radial \
         0.1, 100 W constant, 0.98 propulsion, 0.96 recuperation, 35 kWh",
    )
}

/// Fiori, Ahn and Rakha 2016 — VT-CPEM.
pub fn fiori_2016() -> Source {
    src(
        SourceKind::Paper,
        "Fiori C., Ahn K., Rakha H.A. (2016) Power-based electric vehicle energy consumption \
         model: Model development and validation. Applied Energy 168:257-268. \
         doi:10.1016/j.apenergy.2016.01.097",
        "VT-CPEM: wheel power from the Rakha vehicle-dynamics resistance terms, driveline \
         and motor efficiencies, regenerative-braking efficiency exp(-alpha/|a|); \
         parameterised for a 2011-2013 Nissan Leaf",
    )
}

/// Rakha et al. 2001 — the resistance terms VT-CPEM and VT-CPFM use.
pub fn rakha_2001() -> Source {
    src(
        SourceKind::Paper,
        "Rakha H., Lucic I., Demarchi S.H., Setti J.R., Van Aerde M. (2001) Vehicle Dynamics \
         Model for Predicting Maximum Truck Acceleration Levels. Journal of Transportation \
         Engineering 127(5)",
        "rolling resistance R = 9.8066 m Cr/1000 (c1 v + c2), v in km/h; Cr = 1.75 \
         (average pavement), c1 = 0.0328, c2 = 4.575 (radial tyres)",
    )
}

/// The EPA dynamometer schedules the reference-cycle tests drive.
pub fn epa_cycles() -> Source {
    src(
        SourceKind::Dataset,
        "US EPA dynamometer drive schedules: UDDS (uddscol.txt, 1369 s, 7.45 mi) and HWFET \
         (hwycol.txt, 765 s, 10.26 mi), https://www.epa.gov/vehicle-and-fuel-emissions-testing/\
         dynamometer-drive-schedules",
        "the reference cycles the per-km results are checked on (tests/data)",
    )
}

/// The EPA carbon content of fuels.
pub fn epa_co2_per_gallon() -> Source {
    src(
        SourceKind::Standard,
        "US EPA (2018) Greenhouse Gas Emissions from a Typical Passenger Vehicle. \
         EPA-420-F-18-008: 8,887 g CO2 per gallon of gasoline, 10,180 g per gallon of diesel",
        "",
    )
}

/// The fuel densities SUMO's PHEMlight and HBEFA helpers convert mass to volume with.
pub fn fuel_density() -> Source {
    src(
        SourceKind::Standard,
        "EN 228:2012 (petrol density 720-775 kg/m3 at 15 C) and EN 590:2013 (diesel \
         820-845 kg/m3); point values 742 g/L and 836 g/L as SUMO's HelpersPHEMlight and \
         HelpersHBEFA3 use them",
        "",
    )
}
