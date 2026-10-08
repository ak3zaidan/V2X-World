//! `traffic_calibration` — the traffic-calibration study, written to
//! `docs/validation/traffic-calibration.md`.
//!
//! ```text
//! cargo run -p v2xw-trafficval --bin traffic_calibration -- [--quick] [--out FILE]
//! ```
//!
//! Fundamental diagrams on a city ring and a freeway corridor ring (Greenshields and
//! triangular fits, capacity against the HCM), the stop-and-go wave's speed, the headway
//! distribution, the effect of the human-driver population, the IDM's rest gap and string
//! stability against theory, the Sugiyama ring, and every figure beside SUMO's default
//! driver (Krauß, σ = 0.5). Exits 1 when a held figure is out of band.

use std::fmt::Write as _;
use std::path::PathBuf;

use v2xw_mobility::{IdmPreset, VehicleClass};
use v2xw_mobility::drivers::HumanDriverParams;
use v2xw_trafficval::fdlab::{
    self, DiagramFits, HeadwayStats, Model, RingRun, RingSpec, fit_diagram, headway_stats,
};
use v2xw_trafficval::run::{Verdict, run_check};
use v2xw_trafficval::{Config, Lab, Row, checks};

/// HCM 6th ed. (2016) Exhibit 12-6 / Eq. 12-6: base capacity of a basic freeway segment,
/// 2,400 pc/h/ln at a free-flow speed of 70 mph or more (secondary: as widely quoted).
const HCM_FREEWAY_CAPACITY: f64 = 2400.0;

struct Sweep {
    name: &'static str,
    spec: RingSpec,
    densities: Vec<f64>,
}

fn sweep(c: &Sweep) -> Result<Vec<RingRun>, String> {
    c.densities
        .iter()
        .map(|k| {
            eprintln!("  {} at {k} veh/km", c.name);
            fdlab::run_ring(&c.spec, *k)
        })
        .collect()
}

fn verdict(r: &Row) -> &'static str {
    if !r.tested() {
        "reported"
    } else if r.passes() {
        "pass"
    } else {
        "FAIL"
    }
}

fn table(out: &mut String, rows: &[Row]) {
    out.push_str("| figure | measured | band | reference | verdict |\n|---|---|---|---|---|\n");
    for r in rows {
        let band = r
            .band
            .map_or("—".to_string(), |(lo, hi)| format!("{lo:.3} – {hi:.3}"));
        let _ = writeln!(
            out,
            "| {} | {:.3} | {} | {} | {} |",
            r.metric,
            r.measured,
            band,
            r.reference,
            verdict(r)
        );
    }
    out.push('\n');
}

fn diagram_table(out: &mut String, name: &str, runs: &[RingRun]) {
    let _ = writeln!(out, "**{name}**\n");
    out.push_str("| density, veh/km | speed, km/h | flow, veh/h | speed sd, m/s | slowest, m/s | wave, km/h | smallest gap, m |\n|---|---|---|---|---|---|---|\n");
    for r in runs {
        let _ = writeln!(
            out,
            "| {:.1} | {:.1} | {:.0} | {:.3} | {:.2} | {:.1} | {:.2} |",
            r.density_veh_km,
            r.speed_mps * 3.6,
            r.flow_veh_h,
            r.speed_sd_mps,
            r.min_speed_mps,
            r.wave_kmh,
            r.min_gap_m
        );
    }
    out.push('\n');
}

fn fits_row(out: &mut String, name: &str, f: &DiagramFits) {
    let _ = writeln!(
        out,
        "| {name} | {:.0} | {:.1} / {:.0} / {:.0} / {:.3} | {:.1} / {:.1} / {:.0} / {:.1} / {:.0} / {:.3} |",
        f.measured_capacity_veh_h,
        f.gs_free_kmh,
        f.gs_jam_veh_km,
        f.gs_capacity_veh_h,
        f.gs_r2,
        f.tri_free_kmh,
        f.tri_wave_kmh,
        f.tri_jam_veh_km,
        f.tri_critical_veh_km,
        f.tri_capacity_veh_h,
        f.tri_r2
    );
}

/// The wave speeds of the congested runs that jammed.
fn waves(runs: &[RingRun]) -> Vec<f64> {
    runs.iter()
        .map(|r| r.wave_kmh)
        .filter(|w| w.is_finite())
        .collect()
}

fn median(mut v: Vec<f64>) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quick = args.iter().any(|a| a == "--quick");
    let out_path = args
        .iter()
        .position(|a| a == "--out")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from);
    let cfg = Config::new(quick);
    let (warmup, measure) = if quick { (120.0, 180.0) } else { (300.0, 300.0) };
    let seed = 0x7CA1_1B4A_0000_0001;
    let city_v0 = 13.9; // 50 km/h
    let fwy_v0 = 33.3; // 120 km/h, Treiber 2000
    let city_k = vec![10.0, 20.0, 30.0, 40.0, 45.0, 50.0, 55.0, 70.0, 90.0, 110.0, 130.0];
    let fwy_k = vec![5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 40.0, 55.0, 70.0, 90.0, 110.0];
    let ring = |model, human, length, v0| RingSpec {
        model,
        human,
        circumference_m: length,
        v0_mps: v0,
        warmup_s: warmup,
        measure_s: measure,
        seed,
    };
    let ideal = HumanDriverParams::default();
    let human = HumanDriverParams::human();
    let configs = [
        Sweep {
            name: "city, IDM city drivers, identical",
            spec: ring(Model::Idm(IdmPreset::UrbanHcm), ideal, 1000.0, city_v0),
            densities: city_k.clone(),
        },
        Sweep {
            name: "city, IDM city drivers, human",
            spec: ring(Model::Idm(IdmPreset::UrbanHcm), human, 1000.0, city_v0),
            densities: city_k.clone(),
        },
        Sweep {
            name: "city, SUMO default (Krauss)",
            spec: ring(Model::KraussSumo, ideal, 1000.0, city_v0),
            densities: city_k.clone(),
        },
        Sweep {
            name: "corridor, IDM Treiber 2000, identical",
            spec: ring(Model::Idm(IdmPreset::Treiber2000), ideal, 2000.0, fwy_v0),
            densities: fwy_k.clone(),
        },
        Sweep {
            name: "corridor, IDM Treiber 2000, human",
            spec: ring(Model::Idm(IdmPreset::Treiber2000), human, 2000.0, fwy_v0),
            densities: fwy_k.clone(),
        },
        Sweep {
            name: "corridor, SUMO default (Krauss)",
            spec: ring(Model::KraussSumo, ideal, 2000.0, fwy_v0),
            densities: fwy_k.clone(),
        },
    ];
    let mut results: Vec<(&str, f64, Vec<RingRun>)> = Vec::new();
    for c in &configs {
        match sweep(c) {
            Ok(r) => results.push((c.name, c.spec.v0_mps, r)),
            Err(e) => {
                eprintln!("{}: {e}", c.name);
                std::process::exit(2);
            }
        }
    }
    let fits: Vec<DiagramFits> = results
        .iter()
        .map(|(_, v0, r)| fit_diagram(r, *v0))
        .collect();

    // --- held figures --------------------------------------------------------------
    let mut rows: Vec<Row> = Vec::new();
    let city_id = &results[0].2;
    let city_h = &results[1].2;
    let fwy_id = &results[3].2;
    let fwy_h = &results[4].2;
    // Kesting 2010 Eq. 4.1: Q_max = (1/T)(1 − l_eff/(v0·T + l_eff)), l_eff = s0 + length.
    let qmax = |preset: IdmPreset, v0: f64| {
        let d = preset.profile(VehicleClass::Passenger);
        let leff = d.min_gap_m + VehicleClass::Passenger.spec().length_m;
        3600.0 / d.time_headway_s * (1.0 - leff / (v0 * d.time_headway_s + leff))
    };
    let q_fwy = qmax(IdmPreset::Treiber2000, fwy_v0);
    let q_city = qmax(IdmPreset::UrbanHcm, city_v0);
    rows.push(Row::held(
        "corridor capacity below Kesting 2010's Q_max, identical drivers, fraction",
        1.0 - fits[3].measured_capacity_veh_h / q_fwy,
        (0.0, 0.2),
        format!("Kesting 2010 Eq. 4.1: Q_max = {q_fwy:.0} veh/h for Treiber 2000's drivers; a perturbed ring sits at or below it"),
    ));
    rows.push(Row::held(
        "city capacity below Kesting 2010's Q_max, identical drivers, fraction",
        1.0 - fits[0].measured_capacity_veh_h / q_city,
        (0.0, 0.2),
        format!("Kesting 2010 Eq. 4.1: Q_max = {q_city:.0} veh/h for the city drivers at 50 km/h"),
    ));
    rows.push(Row::held(
        "corridor capacity, human drivers, against identical ones, ratio",
        fits[4].measured_capacity_veh_h / fits[3].measured_capacity_veh_h,
        (0.8, 1.05),
        "the population keeps the mean parameters, so it keeps capacity within its own waves' loss",
    ));
    rows.push(Row::reported(
        "corridor capacity, Treiber 2000 drivers, against the HCM's 2,400 pc/h/ln, ratio",
        fits[3].measured_capacity_veh_h / HCM_FREEWAY_CAPACITY,
        "HCM 6th ed. Exhibit 12-6 (secondary): Treiber 2000's T = 1.6 s is a cautious German-autobahn calibration, below the HCM's US base capacity by construction",
    ));
    rows.push(Row::reported(
        "corridor capacity, SUMO default, against the HCM's 2,400 pc/h/ln, ratio",
        fits[5].measured_capacity_veh_h / HCM_FREEWAY_CAPACITY,
        "Krauß with τ = 1 s: q_max ≈ 1/(τ + l_eff/v0)",
    ));
    rows.push(Row::held(
        "triangular fit beats Greenshields on the corridor (R² congested branch − R² Greenshields)",
        fits[3].tri_r2 - fits[3].gs_r2,
        (0.0, 1.0),
        "Greenshields' linear speed-density law is known to fit freeway data poorly near capacity; the triangular (Newell 1993; Daganzo 1994) is the standard (Treiber & Kesting 2013 §4.4)",
    ));
    let wave_h = median(waves(fwy_h).into_iter().chain(waves(city_h)).collect());
    rows.push(Row::held(
        "stop-and-go wave speed, human drivers (median over jammed densities), km/h",
        wave_h,
        (-25.0, -10.0),
        "upstream at 15–20 km/h (Kerner & Rehborn 1996; Treiber & Kesting 2013 §18.1: −15 ± 5 km/h)",
    ));
    rows.push(Row::held(
        "triangular congested wave speed, corridor human drivers, km/h",
        fits[4].tri_wave_kmh,
        (-25.0, -10.0),
        "same: the slope of the congested branch is the kinematic wave speed",
    ));
    let jams = |runs: &[RingRun]| runs.iter().filter(|r| r.min_speed_mps < 2.0).count() as f64;
    rows.push(Row::held(
        "densities (of 11) where human city drivers break into stop-and-go and identical ones do not",
        city_h
            .iter()
            .zip(city_id)
            .filter(|(h, i)| h.min_speed_mps < 2.0 && i.min_speed_mps >= 2.0)
            .count() as f64,
        (1.0, 11.0),
        "Treiber, Kesting & Helbing 2006 (HDM): a reaction time of the order of the time gap destabilises traffic the ODE model carries smoothly",
    ));
    rows.push(Row::reported(
        "jammed densities, city identical / human",
        jams(city_id) * 100.0 + jams(city_h),
        "encoded as identical × 100 + human",
    ));
    let min_gap = results
        .iter()
        .flat_map(|(_, _, r)| r.iter().map(|x| x.min_gap_m))
        .fold(f64::INFINITY, f64::min);
    rows.push(Row::held(
        "smallest net gap in any run, m (no collision)",
        min_gap,
        (0.0, f64::INFINITY),
        "a car-following model must be collision-free (Treiber & Kesting 2013 §11.3)",
    ));

    // Headways at the density of the most flow on the city ring.
    let at_cap = |runs: &[RingRun]| -> HeadwayStats {
        let best = runs
            .iter()
            .max_by(|a, b| a.flow_veh_h.total_cmp(&b.flow_veh_h))
            .map(|r| r.headways_s.clone())
            .unwrap_or_default();
        headway_stats(&best)
    };
    let h_id = at_cap(city_id);
    let h_h = at_cap(city_h);
    let h_k = at_cap(&results[2].2);
    let h_fid = at_cap(fwy_id);
    let h_fh = at_cap(fwy_h);
    let h_fk = at_cap(&results[5].2);
    rows.push(Row::held(
        "headway coefficient of variation near capacity, human corridor drivers",
        h_fh.cv,
        (0.15, 1.5),
        "observed freeway headways are widely spread (lognormal-like, cv well above zero; NGSIM I-80 / US-101 per-lane headways, e.g. Treiber & Kesting 2013 Fig. 3.7, secondary). Identical drivers give a cv near zero.",
    ));
    rows.push(Row::reported(
        "headway coefficient of variation near capacity, identical corridor drivers",
        h_fid.cv,
        "the ideal model: every car the same gap",
    ));

    // --- the model-level checks the open findings were about ---------------------------
    let mut lab = Lab::new(cfg.clone());
    let mut model_md = String::new();
    let mut model_fail = false;
    for id in ["model/idm-stops-at-s0", "model/string-stability", "model/sugiyama-ring"] {
        let Some(c) = checks().into_iter().find(|c| c.id == id) else {
            continue;
        };
        eprintln!("  {id}");
        let r = run_check(&mut lab, &c);
        let _ = writeln!(model_md, "#### `{id}` — {}\n", c.title);
        match &r.shipped {
            Err(e) => {
                model_fail = true;
                let _ = writeln!(model_md, "could not run: {e}\n");
            }
            Ok(o) => {
                model_md.push_str("| figure | measured | band | verdict |\n|---|---|---|---|\n");
                for (row, v) in r.verdicts() {
                    if v == Verdict::Fail {
                        model_fail = true;
                    }
                    let band = row
                        .band
                        .map_or("—".to_string(), |(lo, hi)| format!("{lo:.3} – {hi:.3}"));
                    let _ = writeln!(
                        model_md,
                        "| {} | {:.4} | {} | {} |",
                        row.metric,
                        row.measured,
                        band,
                        v.label()
                    );
                }
                for n in &o.notes {
                    let _ = writeln!(model_md, "\n{n}");
                }
                for f in &r.faults {
                    let _ = writeln!(
                        model_md,
                        "\n- fault *{}*: {}",
                        f.fault,
                        match &f.outcome {
                            Ok(o) if o.passes() => "all held figures stayed in band".to_string(),
                            Ok(o) => format!(
                                "red on {}",
                                o.rows
                                    .iter()
                                    .filter(|x| !x.passes())
                                    .map(|x| x.metric.clone())
                                    .collect::<Vec<_>>()
                                    .join("; ")
                            ),
                            Err(e) => format!("could not run: {e}"),
                        }
                    );
                }
                model_md.push('\n');
            }
        }
    }

    // --- the report ----------------------------------------------------------------
    let failed = rows.iter().any(|r| !r.passes()) || model_fail;
    let mut md = String::new();
    let cmd = if quick {
        "cargo run -p v2xw-trafficval --bin traffic_calibration -- --quick"
    } else {
        "cargo run -p v2xw-trafficval --bin traffic_calibration"
    };
    let _ = writeln!(
        md,
        "# Traffic calibration: fundamental diagrams, waves, headways and human drivers\n\n\
         Written by `{cmd}` ({} mode: {warmup:.0} s warm-up, {measure:.0} s measured per density). \
         Every run is the engine's own `NativeMobility` on a single-lane ring \
         (`v2xw_trafficval::fdlab`), loaded evenly with one car 20 % slow. Overall: **{}**.\n",
        if quick { "quick" } else { "full" },
        if failed { "FAIL" } else { "pass" }
    );
    md.push_str(
        "Driver populations: *identical* is the textbook model (every car the preset's \
         driver, reacting at once); *human* is `actors.vehicles.drivers.realism: human` \
         (`v2xw_mobility::drivers::HumanDriverParams::human`): every driver their own T, a, \
         b, s0 (mean-one lognormal, cv 0.20–0.25), a 0.8 s reaction time (Green 2000) and \
         the Human Driver Model's estimation errors (V_s = 5 %, σ_r = 0.01 s⁻¹, τ̃ = 20 s; \
         Treiber, Kesting & Helbing 2006). *SUMO default* is SUMO's default car: Krauß with \
         σ = 0.5, τ = 1 s, a = 2.6 m/s², b = 4.5 m/s², 5 m long, minGap 2.5 m (the SUMO \
         vType defaults).\n\n## Held figures\n\n",
    );
    table(&mut md, &rows);
    md.push_str("## Fundamental-diagram fits\n\nGreenshields: free speed km/h / jam density veh/km / capacity veh/h / R² of speed on density. Triangular: free speed km/h / congested wave km/h / jam density veh/km / critical density veh/km / capacity veh/h / R² of the congested branch.\n\n| configuration | highest measured flow, veh/h | Greenshields | triangular |\n|---|---|---|---|\n");
    for ((name, _, _), f) in results.iter().zip(&fits) {
        fits_row(&mut md, name, f);
    }
    md.push_str("\nAgainst SUMO: the same ring, the same densities, SUMO's default driver. ");
    let _ = writeln!(
        md,
        "On the corridor the engine's IDM carries {:.0} veh/h at its best density against Krauß's {:.0}; \
         its congested wave runs at {:.1} km/h (human drivers {:.1}) against Krauß's {:.1}. \
         Krauß's dawdling (σ) is what makes SUMO's default traffic jam at all; the engine gets its waves \
         from reaction time and misjudgement instead, which is what the HDM showed reproduces the observed \
         wave speed and capacity drop without random braking.\n",
        fits[3].measured_capacity_veh_h,
        fits[5].measured_capacity_veh_h,
        fits[3].tri_wave_kmh,
        fits[4].tri_wave_kmh,
        fits[5].tri_wave_kmh
    );
    md.push_str("## Headways near capacity\n\nTime headways at a fixed detector, at the density of the most flow.\n\n| configuration | n | mean, s | cv | share < 1 s | p15, s | median, s | p85, s |\n|---|---|---|---|---|---|---|---|\n");
    for (name, h) in [
        ("city, identical", h_id),
        ("city, human", h_h),
        ("city, SUMO default", h_k),
        ("corridor, identical", h_fid),
        ("corridor, human", h_fh),
        ("corridor, SUMO default", h_fk),
    ] {
        let _ = writeln!(
            md,
            "| {name} | {} | {:.2} | {:.3} | {:.3} | {:.2} | {:.2} | {:.2} |",
            h.n, h.mean_s, h.cv, h.below_1s, h.p15_s, h.p50_s, h.p85_s
        );
    }
    md.push_str("\nNGSIM: the I-80 and US-101 trajectory sets are not in this repository, so the headway rows are held only to the spread a heterogeneous population must show; importing NGSIM's per-lane detector headways and holding the distribution (Kolmogorov-Smirnov) is the next step.\n\n");
    md.push_str("## Per-density runs\n\n");
    for (name, _, r) in &results {
        diagram_table(&mut md, name, r);
    }
    md.push_str("## The model-level checks behind the open findings\n\nThe IDM's gap at rest (1.927 m against the old band [1.95, 2.5]) and string stability against theory at six densities were open findings; both are re-run here. The rest gap is held to an independent integration and to s0 within the continuous model's own shortfall; Sugiyama's ring now runs with the human population and its fault is the ideal driver.\n\n");
    md.push_str(&model_md);

    let path = out_path.unwrap_or_else(|| cfg.root.join("docs/validation/traffic-calibration.md"));
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Err(e) = std::fs::write(&path, md) {
        eprintln!("cannot write {}: {e}", path.display());
        std::process::exit(2);
    }
    eprintln!("wrote {}", path.display());
    if failed {
        std::process::exit(1);
    }
}
