//! Every model on the US EPA reference cycles, against published figures.
//!
//! The UDDS (urban, 1369 s, 7.45 mi, 17 stops) and HWFET (highway, 765 s, 10.26 mi) are
//! the public speed traces every US fuel-economy figure is measured on, so they are the
//! one input every model here and every published number share. Each test drives a model
//! over them second by second and checks the per-distance result against a band whose
//! source is named beside it. The bands are wide on purpose: they catch a unit slip, a
//! wrong coefficient row or a sign error — the defects a model can have — not a 5 %
//! disagreement between two published models of different cars.

use std::collections::BTreeMap;
use std::path::Path;

use v2xw_energy::powertrain::{Fuel, Powertrain, Segment};
use v2xw_energy::registry;
use v2xw_energy::state::DriveState;

/// A cycle's speeds, m/s, one per second.
fn cycle(name: &str) -> Vec<f64> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name);
    let text = std::fs::read_to_string(&path).expect("cycle file");
    text.lines()
        .skip(2)
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let _t = it.next()?;
            it.next()?.parse::<f64>().ok()
        })
        .map(|mph| mph * 0.44704)
        .collect()
}

/// The 1 Hz intervals of a cycle, with the MOVES deceleration run tracked.
fn intervals(v: &[f64]) -> Vec<DriveState> {
    let mut run = 0.0;
    v.windows(2)
        .map(|w| {
            let s = DriveState::between(w[0], w[1], 1.0);
            run = if s.a_mps2 < -0.44704 { run + 1.0 } else { 0.0 };
            s.with_decel_run(run)
        })
        .collect()
}

struct Result {
    km: f64,
    co2_g: f64,
    fuel_g: f64,
}

fn drive(model: &str, p: Powertrain, cycle_name: &str) -> Result {
    let m = registry::emission_model(model).unwrap();
    let class = m.resolve(&p).unwrap_or_else(|| panic!("{model} has no class for {p:?}"));
    let mut r = Result {
        km: 0.0,
        co2_g: 0.0,
        fuel_g: 0.0,
    };
    for s in intervals(&cycle(cycle_name)) {
        let x = m.rates(&class, &s);
        assert!(x.is_physical(), "{model}: {x:?} at {s:?}");
        r.km += s.distance_m / 1000.0;
        r.co2_g += x.co2_g_s * s.dt_s;
        r.fuel_g += x.fuel_g_s * s.dt_s;
    }
    r
}

fn petrol_car() -> Powertrain {
    Powertrain {
        segment: Segment::PassengerCar,
        fuel: Fuel::Gasoline,
        euro: 4,
    }
}

#[test]
fn the_cycles_are_the_published_ones() {
    // UDDS: 7.45 mi in 1369 s; HWFET: 10.26 mi in 765 s (EPA dynamometer schedules).
    let km = |v: &[f64]| intervals(v).iter().map(|s| s.distance_m).sum::<f64>() / 1000.0;
    let udds = cycle("epa_udds.txt");
    let hwfet = cycle("epa_hwfet.txt");
    assert_eq!(udds.len(), 1370);
    assert_eq!(hwfet.len(), 766);
    assert!((km(&udds) - 7.45 * 1.609344).abs() < 0.05, "{}", km(&udds));
    assert!((km(&hwfet) - 10.26 * 1.609344).abs() < 0.05, "{}", km(&hwfet));
}

/// A petrol passenger car on the urban and highway cycles, under every combustion model.
///
/// The band: 5.5-11 L/100 km on the UDDS and 4-8 L/100 km on the HWFET. US EPA labels put
/// petrol compacts of the Euro 4 era at 24-30 mpg city and 33-40 mpg highway (adjusted);
/// the unadjusted dynamometer figures the cycles measure are about a quarter better, which
/// is 6.3-8.8 and 4.6-5.7 L/100 km; the bands widen that by the spread between a European
/// Euro 4 car (HBEFA, PHEMlight) and a US fleet-average light-duty vehicle (MOVES).
///
/// One exception, kept visible rather than hidden in a wider band: HBEFA 3's urban figure.
/// The fit's intercept is what it charges at a standstill, and for `PC_G_EU4` that is
/// 3014 / 3.6 = 837 mg/s of fuel (about 4.1 L/h) and 2.6 g/s of CO2 — some four times a
/// warm petrol car's measured idle (PHEMlight's measured idle row for the same class is
/// 0.22 g/s, about 1 L/h). The UDDS stands still for a fifth of its 1369 s, so HBEFA 3
/// reads 10-14 L/100 km there. SUMO reports the same, because it is the same polynomial.
/// The model card states it; this test pins it, so a change to it is a decision.
#[test]
fn every_combustion_model_lands_in_the_cited_band_on_udds_and_hwfet() {
    let mut seen = BTreeMap::new();
    let mut out_of_band = Vec::new();
    for model in registry::EMISSION_MODELS {
        for cyc in ["epa_udds.txt", "epa_hwfet.txt"] {
            let (lo, hi) = match (model, cyc) {
                ("energy/emission/hbefa3", "epa_udds.txt") => (10.0, 14.0),
                (_, "epa_udds.txt") => (5.5, 11.0),
                _ => (4.0, 8.0),
            };
            let r = drive(model, petrol_car(), cyc);
            let l_100km = r.fuel_g / 742.0 / r.km * 100.0;
            let g_km = r.co2_g / r.km;
            eprintln!("{model:32} {cyc:14} {l_100km:6.2} L/100km {g_km:6.1} gCO2/km");
            if !(lo..=hi).contains(&l_100km) {
                out_of_band.push(format!("{model} on {cyc}: {l_100km} L/100 km outside [{lo}, {hi}]"));
            }
            // Carbon balance: petrol releases 3.0-3.2 g CO2 per g burned (EPA 8,887 g per
            // US gallon at about 2.80 kg per gallon).
            let ratio = r.co2_g / r.fuel_g;
            if !(2.9..3.3).contains(&ratio) {
                out_of_band.push(format!("{model} on {cyc}: {ratio} g CO2 per g fuel"));
            }
            seen.insert((model, cyc), l_100km);
        }
        // Every model has the urban cycle costlier per km than the highway one.
        assert!(seen[&(model, "epa_udds.txt")] > seen[&(model, "epa_hwfet.txt")]);
    }
    assert!(out_of_band.is_empty(), "{out_of_band:#?}");
}

/// HBEFA 3 and PHEMlight describe the same Euro 4 car from the same underlying
/// measurements by two different methods (a fitted polynomial and a power-based curve
/// look-up), so a transcription error in either shows as a gross disagreement.
///
/// Measured in this build: the diesel cars agree within 5 % on the highway and 15 % in
/// town; the petrol car's polynomial reads 28 % above PHEMlight on the highway (7.3 against
/// 5.7 L/100 km — at a steady 80 km/h the polynomial gives 7.0 by hand, PHEMlight 5.3) and
/// 38 % above in town, where its idle intercept adds to it (see the test above). The
/// bounds pin those known disagreements so they cannot drift unnoticed: within 35 % on
/// the highway, [1.1, 1.6] in town.
#[test]
fn hbefa3_and_phemlight_agree_on_the_same_car() {
    let mut wrong = Vec::new();
    for (cyc, lo, hi) in [("epa_hwfet.txt", 0.75, 1.35), ("epa_udds.txt", 1.1, 1.6)] {
        for fuel in [Fuel::Gasoline, Fuel::Diesel] {
            let p = Powertrain {
                fuel,
                ..petrol_car()
            };
            let h = drive("hbefa3", p, cyc);
            let ph = drive("phemlight", p, cyc);
            let rel = (h.fuel_g / h.km) / (ph.fuel_g / ph.km);
            eprintln!("{cyc} {fuel:?}: HBEFA3 / PHEMlight fuel per km = {rel:.3}");
            if !(lo..=hi).contains(&rel) {
                wrong.push(format!("{cyc} {fuel:?}: ratio {rel} outside [{lo}, {hi}]"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

/// Both EV models on both cycles, battery side.
///
/// The band: 8-20 kWh/100 km urban and 10-22 highway. The 2013 Nissan Leaf's label is
/// 29 kWh/100 mi (18 kWh/100 km) at the wall, adjusted; the unadjusted cycle figure is
/// about 30 % lower and the battery sees the charger's 10-15 % less again, so a Leaf-class
/// car lands near 11-13 kWh/100 km at the battery on the UDDS. SUMO's default car is
/// heavier and less aerodynamic (1830 kg, cd·A = 0.91 m² against the Leaf's 0.65), so it
/// sits higher, especially on the highway.
#[test]
fn both_ev_models_land_in_the_cited_band_on_udds_and_hwfet() {
    for model in registry::EV_MODELS {
        let m = registry::ev_model(model, &BTreeMap::new()).unwrap();
        for (cyc, lo, hi) in [("epa_udds.txt", 8.0, 20.0), ("epa_hwfet.txt", 10.0, 22.0)] {
            let mut wh = 0.0;
            let mut regen = 0.0;
            let mut km = 0.0;
            for s in intervals(&cycle(cyc)) {
                let p = m.battery_power_w(&s);
                assert!(p.is_finite());
                wh += p * s.dt_s / 3600.0;
                if p < 0.0 {
                    regen -= p * s.dt_s / 3600.0;
                }
                km += s.distance_m / 1000.0;
            }
            let kwh_100km = wh / 1000.0 / km * 100.0;
            let regen_share = regen / (wh + regen);
            eprintln!(
                "{model:22} {cyc:14} {kwh_100km:6.2} kWh/100km, regenerated {:.0} % of drawn",
                regen_share * 100.0
            );
            assert!(
                (lo..=hi).contains(&kwh_100km),
                "{model} on {cyc}: {kwh_100km} kWh/100 km outside [{lo}, {hi}]"
            );
        }
    }
}

/// The V2X effect the energy layer exists to measure, on a controlled trajectory: a car
/// that approaches a red light, stops and waits, against one that is told the switching
/// time (GLOSA) and arrives at the green at a steady, lower speed. Same start, same end,
/// same distance and the same arrival time at the far side.
///
/// Published evaluations put the saving of GLOSA-style advice at a few per cent to over
/// twenty per cent depending on penetration, signal timing and how much stopping there
/// was to remove: Tielert et al. (2012, ITST) up to 20 % for battery-electric vehicles,
/// Treiber and Kesting (2014, arXiv:1408.5498) about 15 % fewer stops and 4 % on fuel and
/// the other criteria across a whole signal cycle, Düring (2011, DLR) 21-94 % for the
/// single approach-and-start manoeuvre. A single stop removed is the favourable end of
/// that range, so every model must show a saving, and one of at least 10 %.
#[test]
fn removing_one_signal_stop_saves_fuel_and_energy_under_every_model() {
    // 50 km/h; red for the first 30 s at 300 m. Stop-and-go: cruise, brake at 2 m/s² to a
    // stop, wait, accelerate at 1.5 m/s² back to 50 km/h. GLOSA: slow early at 1 m/s² to
    // the speed that arrives as the light turns, then accelerate back.
    let v0 = 50.0 / 3.6;
    let mut stop_go = Vec::new();
    let mut x: f64 = 0.0;
    let mut v = v0;
    let mut t = 0.0;
    // cruise until the braking point (v²/(2·2) before 300 m)
    while x < 300.0 - v0 * v0 / 4.0 {
        stop_go.push(v);
        x += v;
        t += 1.0;
    }
    while v > 0.0 {
        stop_go.push(v);
        v = (v - 2.0).max(0.0);
        t += 1.0;
    }
    while t < 30.0 {
        stop_go.push(0.0);
        t += 1.0;
    }
    while v < v0 {
        stop_go.push(v);
        v = (v + 1.5).min(v0);
    }
    stop_go.extend(std::iter::repeat_n(v0, 20));
    // GLOSA: the same 300 m reached at t = 30 s at a steady 10 m/s after an early ease.
    let mut glosa = vec![v0, v0];
    let mut v = v0;
    while v > 10.0 {
        v = (v - 1.0).max(10.0);
        glosa.push(v);
    }
    while glosa.len() < 30 {
        glosa.push(10.0);
    }
    while v < v0 {
        v = (v + 1.5).min(v0);
        glosa.push(v);
    }
    // pad both to the same distance at v0
    let dist = |p: &[f64]| intervals(p).iter().map(|s| s.distance_m).sum::<f64>();
    while dist(&glosa) < dist(&stop_go) {
        glosa.push(v0);
    }
    while dist(&stop_go) < dist(&glosa) - v0 {
        stop_go.push(v0);
    }
    let per_km = |model: &str, prof: &[f64]| {
        let m = registry::emission_model(model).unwrap();
        let c = m.resolve(&petrol_car()).unwrap();
        let ss = intervals(prof);
        let fuel: f64 = ss.iter().map(|s| m.rates(&c, s).fuel_g_s * s.dt_s).sum();
        fuel / (dist(prof) / 1000.0)
    };
    for model in registry::EMISSION_MODELS {
        let saving = 1.0 - per_km(model, &glosa) / per_km(model, &stop_go);
        eprintln!("{model:32} GLOSA saves {:.1} % fuel per km", saving * 100.0);
        assert!(saving > 0.10, "{model}: {saving}");
    }
    for model in registry::EV_MODELS {
        let m = registry::ev_model(model, &BTreeMap::new()).unwrap();
        let e = |prof: &[f64]| {
            intervals(prof)
                .iter()
                .map(|s| m.battery_power_w(s) * s.dt_s)
                .sum::<f64>()
                / dist(prof)
        };
        let saving = 1.0 - e(&glosa) / e(&stop_go);
        eprintln!("{model:32} GLOSA saves {:.1} % battery energy per km", saving * 100.0);
        assert!(saving > 0.0, "{model}: {saving}");
    }
}
