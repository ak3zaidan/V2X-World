//! `v2xw energy` — score a finished recording's traffic for emissions, fuel and EV energy.
//!
//! The energy provider (`v2xw_energy`) reads nothing but `gt.kinematics`, so it can be run
//! over any full-profile recording after the fact — including one made before the
//! `energy:` section existed, and under a different model or fleet than the run used. The
//! simulation is not repeated; the same recording scored twice gives the same numbers.
//!
//! The section the scenario carried (from the recording's `scenario.json` attachment, when
//! it has one) is the starting point; `--spec` replaces it with a JSON or YAML `energy:`
//! section, and `--emission-model` / `--ev-model` override the models. The fleet draw is
//! keyed by the recording's master seed, so a re-score reproduces the run's own fleet.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_core::ctx::{OwnedRecord, Visibility};
use v2xw_energy::{EnergyProvider, EnergySpec, Health, Totals};
use v2xw_metrics::MetricProvider;
use v2xw_record::Reader;

use crate::error::{CliError, Result};

/// What to score and how.
#[derive(Debug, Clone, Default)]
pub struct EnergyOptions {
    /// The recording.
    pub recording: PathBuf,
    /// A JSON or YAML file holding an `energy:` section (the section's body).
    pub spec: Option<PathBuf>,
    /// Overrides the emission model.
    pub emission_model: Option<String>,
    /// Overrides the EV model.
    pub ev_model: Option<String>,
    /// Writes one CSV row per vehicle here.
    pub vehicles_csv: Option<PathBuf>,
    /// Writes the MOVES operating-mode distribution per lane here.
    pub opmode_csv: Option<PathBuf>,
}

/// One class's totals.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClassRow {
    /// The mobility class.
    pub class: String,
    /// Distance, km.
    pub km: f64,
    /// Tailpipe CO2, kg.
    pub co2_kg: f64,
    /// Fuel, L.
    pub fuel_l: f64,
    /// NOx, g.
    pub nox_g: f64,
    /// PM, g.
    pub pm_g: f64,
    /// Net EV battery energy, kWh.
    pub ev_kwh: f64,
    /// Stops.
    pub stops: u64,
}

/// One health band's verdict.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BandRow {
    /// The metric.
    pub metric: String,
    /// The class, or empty for the headline.
    pub class: String,
    /// The value graded, if any.
    pub value: Option<f64>,
    /// `ok`, `warn`, `fail` or `not-measured`.
    pub health: String,
    /// The healthy range.
    pub ok: (f64, f64),
}

/// What the scoring found.
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnergyOutcome {
    /// The recording.
    pub recording: String,
    /// The emission model used.
    pub emission_model: String,
    /// The EV model used.
    pub ev_model: String,
    /// Where the section came from: `scenario`, `--spec` or `default`.
    pub spec_from: String,
    /// The `gt.kinematics` records read.
    pub records: u64,
    /// Vehicles scored.
    pub vehicles: u64,
    /// Vehicles the emission model has no class for.
    pub unmodelled: u64,
    /// Totals by class.
    pub by_class: Vec<ClassRow>,
    /// The fleet's CO2 per km, fuel per 100 km and EV kWh per 100 km.
    pub co2_g_per_km: Option<f64>,
    /// Fuel of the combustion fleet, L/100 km.
    pub fuel_l_per_100km: Option<f64>,
    /// EV battery energy, kWh/100 km.
    pub ev_kwh_per_100km: Option<f64>,
    /// The health bands.
    pub bands: Vec<BandRow>,
}

fn ratio(n: f64, d: f64) -> Option<f64> {
    (d > 0.0).then(|| n / d)
}

fn row(class: &str, t: &Totals) -> ClassRow {
    ClassRow {
        class: class.to_string(),
        km: t.km_all,
        co2_kg: t.co2_g / 1000.0,
        fuel_l: t.fuel_l,
        nox_g: t.nox_g,
        pm_g: t.pm_g,
        ev_kwh: t.ev_net_wh() / 1000.0,
        stops: t.stops,
    }
}

fn read_spec(path: &Path) -> Result<EnergySpec> {
    let text = std::fs::read_to_string(path).map_err(|e| CliError::io("cannot read", path, e))?;
    let parsed: std::result::Result<EnergySpec, String> =
        if path.extension().is_some_and(|x| x == "json") {
            serde_json::from_str(&text).map_err(|e| e.to_string())
        } else {
            serde_yml::from_str(&text).map_err(|e| e.to_string())
        };
    parsed.map_err(|problem| CliError::BadArgument {
        flag: "spec",
        problem: format!("{}: {problem}", path.display()),
    })
}

/// Scores a recording.
///
/// # Errors
/// An unreadable recording or spec, or a section the energy crate refuses.
pub fn energy(o: &EnergyOptions) -> Result<EnergyOutcome> {
    let mut reader = Reader::open_paged(&o.recording)?;
    let manifest = reader
        .manifest()
        .get("json")
        .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok());
    let seed = manifest
        .as_ref()
        .and_then(|m| m["master_seed"].as_u64())
        .unwrap_or(0);
    // `v2xw run` attaches the resolved scenario as `scenario.yaml`, the server as
    // `scenario.json`; YAML is a superset of JSON, so one parser reads either.
    let mut from_scenario = None;
    for name in ["scenario.yaml", "scenario.json"] {
        if from_scenario.is_none() && reader.index().attachments.contains_key(name) {
            from_scenario = reader
                .attachment(name)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .and_then(|text| serde_yml::from_str::<serde_json::Value>(&text).ok())
                .and_then(|v| v.get("energy").cloned())
                .and_then(|e| serde_json::from_value::<EnergySpec>(e).ok());
        }
    }
    let (mut spec, spec_from) = match (&o.spec, from_scenario) {
        (Some(p), _) => (read_spec(p)?, "--spec"),
        (None, Some(s)) => (s, "scenario"),
        (None, None) => (EnergySpec::default(), "default"),
    };
    if let Some(m) = &o.emission_model {
        spec.emission_model.clone_from(m);
    }
    if let Some(m) = &o.ev_model {
        spec.ev_model.clone_from(m);
    }
    let mut provider =
        EnergyProvider::new(spec, seed).map_err(|problem| CliError::BadArgument {
            flag: "spec",
            problem,
        })?;
    let mut records = 0u64;
    for r in reader.records(Some("gt.kinematics"))? {
        records += 1;
        provider.on_event(&OwnedRecord {
            channel: "gt.kinematics",
            visibility: Visibility::Gt,
            json: r.json,
        });
    }
    // One flush, after the last record, gives the band-graded run-so-far metrics.
    let samples = provider.flush(u64::MAX);
    let vehicles = provider.vehicles();
    let by_class_totals: BTreeMap<String, Totals> = provider.run_by_class();
    let all = provider.run_totals();
    if let Some(path) = &o.vehicles_csv {
        let mut csv = String::from(
            "actor,class,powertrain,emission_class,modelled,km,co2_g,fuel_l,nox_g,pm_g,\
             ev_out_wh,ev_regen_wh,soc,stops\n",
        );
        for v in &vehicles {
            let t = &v.totals;
            csv.push_str(&format!(
                "{},{},{},{},{},{:.4},{:.3},{:.5},{:.4},{:.5},{:.2},{:.2},{},{}\n",
                v.actor.index(),
                v.class,
                v.powertrain.map(|p| p.label()).unwrap_or_default(),
                v.emission_class.clone().unwrap_or_default(),
                v.modelled,
                t.km_all,
                t.co2_g,
                t.fuel_l,
                t.nox_g,
                t.pm_g,
                t.ev_out_wh,
                t.ev_regen_wh,
                v.soc.map(|s| format!("{s:.4}")).unwrap_or_default(),
                t.stops,
            ));
        }
        std::fs::write(path, csv).map_err(|e| CliError::io("cannot write", path, e))?;
    }
    if let Some(path) = &o.opmode_csv {
        // MOVES's `opModeDistribution` shape: per (source type, link), each mode's share of
        // the time, with the seconds beside it so a reader can re-weight.
        let secs = provider.op_mode_seconds();
        let mut totals: BTreeMap<(u16, u32), f64> = BTreeMap::new();
        for ((src, lane, _), s) in secs {
            *totals.entry((*src, *lane)).or_default() += s;
        }
        let mut csv = String::from("sourceTypeID,linkID,opModeID,seconds,opModeFraction\n");
        for ((src, lane, mode), s) in secs {
            let total = totals.get(&(*src, *lane)).copied().unwrap_or(0.0);
            let frac = if total > 0.0 { s / total } else { 0.0 };
            csv.push_str(&format!("{src},{lane},{mode},{s:.3},{frac:.6}\n"));
        }
        std::fs::write(path, csv).map_err(|e| CliError::io("cannot write", path, e))?;
    }
    let bands = v2xw_energy::assess(&samples)
        .into_iter()
        .map(|v| BandRow {
            metric: v.band.metric.to_string(),
            class: v.band.class.unwrap_or("").to_string(),
            value: v.value,
            health: match v.health {
                Health::Ok => "ok",
                Health::Warn => "warn",
                Health::Fail => "fail",
                Health::NotMeasured => "not-measured",
            }
            .to_string(),
            ok: v.band.ok,
        })
        .collect();
    Ok(EnergyOutcome {
        recording: o.recording.display().to_string(),
        emission_model: provider.spec().emission_model.clone(),
        ev_model: provider.spec().ev_model.clone(),
        spec_from: spec_from.to_string(),
        records,
        vehicles: vehicles.len() as u64,
        unmodelled: vehicles.iter().filter(|v| !v.modelled).count() as u64,
        by_class: by_class_totals.iter().map(|(c, t)| row(c, t)).collect(),
        co2_g_per_km: ratio(all.co2_g, all.km_ice + all.km_ev),
        fuel_l_per_100km: ratio(all.fuel_l * 100.0, all.km_ice),
        ev_kwh_per_100km: ratio(all.ev_net_wh() / 10.0, all.km_ev),
        bands,
    })
}

/// The outcome as a terminal report.
#[must_use]
pub fn format(o: &EnergyOutcome) -> String {
    let opt = |v: Option<f64>, unit: &str| v.map_or("-".to_string(), |x| format!("{x:.2} {unit}"));
    let mut s = format!(
        "recording   {}\nmodels      {} (combustion), {} (electric); section from {}\n\
         scored      {} vehicles from {} gt.kinematics records, {} unmodelled\n\
         fleet       {} CO2, {} fuel, {} EV\n\n",
        o.recording,
        o.emission_model,
        o.ev_model,
        o.spec_from,
        o.vehicles,
        o.records,
        o.unmodelled,
        opt(o.co2_g_per_km, "g/km"),
        opt(o.fuel_l_per_100km, "L/100km"),
        opt(o.ev_kwh_per_100km, "kWh/100km"),
    );
    s.push_str("class          km     CO2 kg    fuel L   NOx g    PM g   EV kWh  stops\n");
    for r in &o.by_class {
        s.push_str(&format!(
            "{:<12} {:>7.2} {:>9.3} {:>9.3} {:>7.2} {:>7.3} {:>8.3} {:>6}\n",
            r.class, r.km, r.co2_kg, r.fuel_l, r.nox_g, r.pm_g, r.ev_kwh, r.stops
        ));
    }
    s.push_str("\nhealth\n");
    for b in &o.bands {
        s.push_str(&format!(
            "  {:<12} {:<22} {:<10} {} (healthy {}..{})\n",
            b.health,
            b.metric,
            b.class,
            b.value.map_or("-".to_string(), |v| format!("{v:.3}")),
            b.ok.0,
            b.ok.1
        ));
    }
    s
}
