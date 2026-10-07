//! `v2xw compare-signatures` — one scenario, one seed, several signature schemes, side by
//! side, with every difference against ECDSA P-256 measured and the large ones called out.
//!
//! ```text
//! v2xw compare-signatures scenarios/compare-signature-schemes.yaml \
//!     --schemes ecdsa-p256,ml-dsa-44,falcon-512,hybrid-mldsa44-ecdsa-p256
//! ```
//!
//! It is the experiment runner with a sweep over `security.signature`: the scenario's own
//! `experiment` block is replaced by one axis, one seed and one replication, and the
//! signing-overhead metric group (`v2xw_metrics::signing`) is switched on in every run. The
//! runner derives the run seed without the cell index, so every scheme sees the same
//! traffic, the same channel draws and the same credential calendar; only the signature
//! changes. A sweep that stops part-way resumes where it stopped (the runner's journal).
//!
//! # What it writes, in `--out`
//!
//! | File | What |
//! |---|---|
//! | `comparison.json` | the schemes' registry facts and every row, value, difference and health level (schema `v2xw/signature-comparison/1`) — what the Studio's comparison view loads |
//! | `comparison.csv` | the same, long form: one line per (row, scheme) |
//! | `comparison.md` | a table to paste into a paper or an issue, differences in brackets, called-out ones in bold |
//! | `runs/…` | each run's metrics, report and manifest, as `v2xw experiment run` writes them |
//!
//! # How a run's windows are pooled into one number
//!
//! Proportions pool their counts and ratios of sums their two sums, exactly as the
//! experiment runner does. A distribution's mean and percentiles are the sample-count
//! weighted mean of each window's — so a p95 here is the typical window's p95, not the p95
//! of every message in the run, and the caveats say so.
//!
//! # What is called out
//!
//! The rule of `v2xw_copilot::experiment`: a difference is highlighted when it moves the
//! value across a health band, or by at least 10 % of the baseline and by more than the
//! row's resolution. One realisation, so no confidence interval is claimed.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use v2xw_experiment::plan::ExperimentPlan;
use v2xw_experiment::runner::{RUNS_DIR, RunnerOptions, run_experiment};
use v2xw_metrics::health::{Better, HealthBand};
use v2xw_metrics::stats::{DistributionSummary, Estimate, RatioEstimate};
use v2xw_metrics::{MetricSample, SampleValue};

use crate::error::{CliError, Result};

/// The schema id `comparison.json` carries.
pub const COMPARISON_SCHEMA: &str = "v2xw/signature-comparison/1";

/// The baseline every difference is measured against.
pub const BASELINE: &str = "ecdsa-p256";

/// The schemes compared when none are named.
pub const DEFAULT_SCHEMES: [&str; 4] =
    ["ecdsa-p256", "ml-dsa-44", "falcon-512", "hybrid-mldsa44-ecdsa-p256"];

/// The relative change below which a difference is not called out.
pub const RELATIVE_FLOOR: f64 = 0.10;

/// How `v2xw compare-signatures` was asked to run.
#[derive(Debug, Clone)]
pub struct CompareOptions {
    /// The scenario to run under each scheme.
    pub scenario: PathBuf,
    /// The scheme ids; the baseline is added first when missing.
    pub schemes: Vec<String>,
    /// Where the outputs go. `None` means `runs/<meta.name>-signatures`.
    pub out: Option<PathBuf>,
    /// Override `time.duration_s` for every run.
    pub duration_s: Option<f64>,
    /// The manifest timestamp every run uses.
    pub build_utc: Option<String>,
    /// Write a recording per run.
    pub record: bool,
}

/// One scheme's registry and hardware facts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SchemeColumn {
    /// The scheme id.
    pub id: String,
    /// Its plain name.
    pub label: String,
    /// `classical`, `hybrid` or `post-quantum`.
    pub mode: String,
    /// Signature octets in the envelope.
    pub sig_octets: u64,
    /// Verification-key octets in a certificate.
    pub key_octets: u64,
    /// Octets a certificate grows by over P-256's.
    pub cert_extra_octets: u64,
    /// NIST post-quantum security category; 0 for classical.
    pub quantum_level: u8,
    /// Where the default OBU signs.
    pub sign_engine: String,
    /// Where it verifies.
    pub verify_engine: String,
    /// The run that measured it.
    pub run_id: String,
}

/// One compared quantity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// The metric series (`sig_sign_latency.p95`, `cbr`).
    pub key: String,
    /// A plain label.
    pub label: String,
    /// The section of the table.
    pub group: String,
    /// The unit.
    pub unit: String,
    /// Which way is better, when known.
    pub better: Option<Better>,
    /// One value per scheme, in column order; `None` when the run measured nothing.
    pub values: Vec<Option<f64>>,
    /// `value − baseline` per scheme.
    pub delta: Vec<Option<f64>>,
    /// `(value − baseline) / |baseline|` per scheme.
    pub rel: Vec<Option<f64>>,
    /// The health level per scheme, when the row has a band.
    pub level: Vec<Option<String>>,
    /// Whether the difference against the baseline is called out.
    pub highlight: Vec<bool>,
}

/// The whole comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// [`COMPARISON_SCHEMA`].
    pub schema: String,
    /// The scenario's name.
    pub scenario: String,
    /// The derived run seed every scheme ran under.
    pub seed_hex: String,
    /// Simulated seconds per run.
    pub duration_s: f64,
    /// The baseline scheme.
    pub baseline: String,
    /// The schemes, in column order.
    pub schemes: Vec<SchemeColumn>,
    /// The rows.
    pub rows: Vec<Row>,
    /// What a reader should know before quoting a number.
    pub caveats: Vec<String>,
    /// The files written.
    #[serde(default)]
    pub files: Vec<String>,
}

/// How one row's value is read from a run's samples.
#[derive(Clone, Copy)]
enum Stat {
    /// The pooled point: proportion, ratio of sums, weighted scalar, or a count's total.
    Point,
    /// A distribution's weighted median.
    P50,
    /// A distribution's weighted 95th percentile.
    P95,
}

/// The rows, in table order: (group, metric, stat, label).
const ROWS: &[(&str, &str, Stat, &str)] = &[
    ("Bytes on the air", "sig_signature_bytes", Stat::Point, "Signature per message"),
    ("Bytes on the air", "sig_signature_rate", Stat::Point, "Signature bytes per second (all senders)"),
    ("Bytes on the air", "sig_signer_id_bytes", Stat::Point, "Signer identifier per message"),
    ("Bytes on the air", "sig_cert_bytes", Stat::Point, "Attached certificate per message (averaged)"),
    ("Bytes on the air", "sig_cert_airtime_share", Stat::Point, "Certificates' share of the air"),
    ("Bytes on the air", "sig_signer_id_airtime_share", Stat::Point, "Signer identifiers' share of the air"),
    ("Bytes on the air", "sig_security_airtime_share", Stat::Point, "Security share of the air (with certificates)"),
    ("Bytes on the air", "sig_security_airtime_share_nocert", Stat::Point, "Security share of the air (without certificates)"),
    ("Device cost", "sig_sign_latency", Stat::P50, "Sign latency p50"),
    ("Device cost", "sig_sign_latency", Stat::P95, "Sign latency p95"),
    ("Device cost", "sig_sign_wait", Stat::P95, "Wait for the signer p95"),
    ("Device cost", "sig_verify_latency", Stat::P50, "Verify latency p50"),
    ("Device cost", "sig_verify_latency", Stat::P95, "Verify latency p95"),
    ("Device cost", "sig_signer_util", Stat::Point, "Signer utilisation (mean node)"),
    ("Device cost", "sig_signer_util_max", Stat::Point, "Signer utilisation (busiest node)"),
    ("Device cost", "sig_verifier_util", Stat::Point, "Verifier utilisation (mean node)"),
    ("Device cost", "sig_verifier_util_max", Stat::Point, "Verifier utilisation (busiest node)"),
    ("Device cost", "sig_verify_capacity", Stat::Point, "Verifications per second one engine can do"),
    ("Device cost", "sig_neighbour_capacity", Stat::Point, "Neighbours a device can keep up with"),
    ("Device cost", "sig_verify_headroom", Stat::Point, "Verification headroom (capacity / load)"),
    ("Device cost", "sig_verify_dropped", Stat::Point, "Verifications dropped"),
    ("Device cost", "sig_verify_late", Stat::Point, "Verifications later than 100 ms"),
    ("Fragmentation", "sig_fragments_per_msg", Stat::Point, "Frames per message"),
    ("Fragmentation", "sig_fragmented_share", Stat::Point, "Messages fragmented"),
    ("Fragmentation", "sig_reassembly_loss", Stat::Point, "Fragmented messages lost in reassembly"),
    ("Latency and channel", "sig_e2e_added", Stat::P50, "Latency added by security p50"),
    ("Latency and channel", "sig_e2e_added", Stat::P95, "Latency added by security p95"),
    ("Latency and channel", "sig_e2e_added_share", Stat::Point, "Security's share of end-to-end latency"),
    ("Latency and channel", "e2e_latency", Stat::P95, "End-to-end latency p95 (all flows)"),
    ("Latency and channel", "cbr", Stat::Point, "Channel busy ratio (measured)"),
    ("Latency and channel", "sig_cbr_extra", Stat::Point, "Channel busy ratio from security bytes (estimate)"),
    ("Latency and channel", "pdr", Stat::Point, "Packet delivery ratio within 100 m"),
    ("Backend", "sig_backend_bytes", Stat::Point, "Credential bytes to and from devices per second"),
    ("Backend", "sig_ca_sign_rate", Stat::Point, "CA signatures per second"),
    ("Backend", "sig_ca_busy", Stat::Point, "CA service time per second"),
];

/// Runs the comparison and writes its three files.
///
/// # Errors
/// [`CliError::BadArgument`] for an unknown scheme or one the scenario's hardware cannot
/// price, and whatever the runner or a run reports.
pub fn run(options: &CompareOptions) -> Result<Comparison> {
    let scenario = v2xw_engine::Scenario::load(&options.scenario)?;
    run_scenario(scenario, options)
}

/// [`run`] over a scenario already in memory — what the Studio's button sends, the
/// scenario the engine is serving. `options.scenario` is not read.
///
/// # Errors
/// As [`run`].
pub fn run_scenario(
    mut scenario: v2xw_engine::Scenario,
    options: &CompareOptions,
) -> Result<Comparison> {
    let mut schemes: Vec<String> = if options.schemes.is_empty() {
        DEFAULT_SCHEMES.iter().map(|s| (*s).to_string()).collect()
    } else {
        options.schemes.clone()
    };
    schemes.dedup();
    if !schemes.iter().any(|s| s == BASELINE) {
        schemes.insert(0, BASELINE.to_string());
    }
    let profile = v2xw_node::profiles::get(&scenario.nodes.default_obu);
    for id in &schemes {
        let Some(s) = v2xw_sec::scheme::signature(id) else {
            return Err(CliError::BadArgument {
                flag: "schemes",
                problem: format!(
                    "'{id}' is not a registered signature scheme; known: {}",
                    v2xw_sec::scheme::SIGNATURE_IDS.join(", ")
                ),
            });
        };
        if let Some(p) = profile
            && (p.op_cost(s.sign_op()).is_none() || p.op_cost(s.verify_op()).is_none())
        {
            return Err(CliError::BadArgument {
                flag: "schemes",
                problem: format!(
                    "the scenario's OBU '{}' publishes no cost for {} or {}; pick a profile \
                     that prices it (obu/generic-automotive-soc-no-hsm prices every \
                     registered scheme) or drop the scheme",
                    scenario.nodes.default_obu,
                    s.sign_op(),
                    s.verify_op()
                ),
            });
        }
    }
    if let Some(d) = options.duration_s {
        scenario.time.duration_s = d;
    }
    if scenario.metrics.is_empty() {
        scenario.metrics = vec!["all".to_string()];
    }
    if !scenario.metrics.iter().any(|m| m == v2xw_metrics::signing::GROUP) {
        scenario.metrics.push(v2xw_metrics::signing::GROUP.to_string());
    }
    let mut sweep = BTreeMap::new();
    sweep.insert(
        "security.signature".to_string(),
        schemes.iter().map(|s| serde_json::Value::String(s.clone())).collect(),
    );
    scenario.experiment = Some(v2xw_engine::scenario::schema::Experiment {
        sweep,
        seeds: vec![scenario.seed],
        replications: 1,
    });

    let out = options.out.clone().unwrap_or_else(|| {
        PathBuf::from("runs").join(format!("{}-signatures", scenario.meta.name))
    });
    let executor = crate::experiment::CliExecutor::new(
        options
            .build_utc
            .clone()
            .unwrap_or_else(|| crate::wall::now_iso8601_utc().to_string()),
        options.record,
    );
    let runner = RunnerOptions::new(&out);
    run_experiment(&scenario, &executor, &runner)?;
    let plan = ExperimentPlan::expand(&scenario)?;

    let mut columns = Vec::new();
    let mut samples = Vec::new();
    for run in &plan.runs {
        let id = run
            .cell
            .values
            .get("security.signature")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(BASELINE)
            .to_string();
        let mut materialised = scenario.clone();
        materialised.security.signature.clone_from(&id);
        let facts = v2xw_engine::wiring::signing_facts(&materialised);
        let s = v2xw_sec::scheme::signature(&id).expect("validated above");
        let (_, cert_extra) = s.extra_octets();
        columns.push(SchemeColumn {
            id: id.clone(),
            label: s.label.to_string(),
            mode: s.mode.as_str().to_string(),
            sig_octets: u64::from(s.sig_octets()),
            key_octets: u64::from(s.key_octets()),
            cert_extra_octets: u64::from(cert_extra),
            quantum_level: s.quantum_level(),
            sign_engine: facts.sign_engine,
            verify_engine: facts.verify_engine,
            run_id: run.run_id.clone(),
        });
        let path = out.join(RUNS_DIR).join(&run.run_id).join("metrics.json");
        samples.push(v2xw_experiment::reduce::read_samples(&path)?);
    }

    let mut comparison = build(
        &scenario.meta.name,
        plan.runs.first().map(|r| r.seed_hex()).unwrap_or_default(),
        scenario.time.duration_s,
        columns,
        &samples,
    );
    comparison.files = write(&comparison, &out)?;
    Ok(comparison)
}

/// The health band and better-direction of a metric series, from the catalogue.
fn band_of(metric: &str, series: &str) -> (Option<HealthBand>, Option<Better>) {
    let band = v2xw_metrics::signing::SigningProvider::new(0, v2xw_metrics::signing::SchemeFacts::default())
        .defs()
        .into_iter()
        .find(|d| d.name == metric)
        .and_then(|d| d.health)
        .or_else(|| v2xw_metrics::health::band_for(metric));
    let better = band.as_ref().map(|b| b.better).or(match metric {
        "pdr" | "sig_verify_capacity" | "sig_neighbour_capacity" | "sig_verify_headroom" => {
            Some(Better::Higher)
        }
        _ => Some(Better::Lower),
    });
    let judged = band.filter(|b| format!("{metric}{}", b.judged) == series);
    (judged, better)
}

use v2xw_metrics::MetricProvider as _;

/// Assembles the rows from each scheme's samples. Pure, so a test can drive it.
#[must_use]
pub fn build(
    scenario: &str,
    seed_hex: String,
    duration_s: f64,
    schemes: Vec<SchemeColumn>,
    samples: &[Vec<MetricSample>],
) -> Comparison {
    let base_ix = schemes.iter().position(|s| s.id == BASELINE).unwrap_or(0);
    let mut rows = Vec::new();
    for (group, metric, stat, label) in ROWS {
        let suffix = match stat {
            Stat::Point => "",
            Stat::P50 => ".p50",
            Stat::P95 => ".p95",
        };
        let key = format!("{metric}{suffix}");
        let mut unit = String::new();
        let values: Vec<Option<f64>> = samples
            .iter()
            .map(|run| {
                let mine: Vec<&MetricSample> = run
                    .iter()
                    .filter(|s| s.metric == *metric && dims_match(metric, s))
                    .collect();
                if let Some(s) = mine.first() {
                    unit.clone_from(&s.unit);
                }
                pool(&mine, *stat)
            })
            .collect();
        if values.iter().all(Option::is_none) {
            continue;
        }
        let (band, better) = band_of(metric, &key);
        let base = values.get(base_ix).copied().flatten();
        let delta: Vec<Option<f64>> = values.iter().map(|v| Some((*v)? - base?)).collect();
        let rel: Vec<Option<f64>> = values
            .iter()
            .map(|v| {
                let b = base?;
                let v = (*v)?;
                (b != 0.0).then(|| (v - b) / b.abs())
            })
            .collect();
        let level: Vec<Option<String>> = values
            .iter()
            .map(|v| {
                let l = band.as_ref()?.classify((*v)?)?;
                Some(format!("{l:?}").to_lowercase())
            })
            .collect();
        let floor = resolution(&unit);
        let highlight = values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                if i == base_ix {
                    return false;
                }
                let (Some(v), Some(b)) = (*v, base) else { return false };
                let crossed = level[i].is_some() && level[i] != level[base_ix];
                let big = (v - b).abs() > floor
                    && (b == 0.0 || (v - b).abs() / b.abs() >= RELATIVE_FLOOR);
                crossed || big
            })
            .collect();
        rows.push(Row {
            key,
            label: (*label).to_string(),
            group: (*group).to_string(),
            unit,
            better,
            values,
            delta,
            rel,
            level,
            highlight,
        });
    }
    Comparison {
        schema: COMPARISON_SCHEMA.to_string(),
        scenario: scenario.to_string(),
        seed_hex,
        duration_s,
        baseline: BASELINE.to_string(),
        schemes,
        rows,
        caveats: vec![
            "One realisation per scheme under the same seed: differences are the scheme's \
             effect on that traffic, with no confidence interval."
                .to_string(),
            "Percentiles are the sample-weighted mean of each metric window's percentile, \
             not the percentile of every message in the run."
                .to_string(),
            "Sizes and costs are the scheme registry's and the hardware profile's (cited in \
             v2xw_sec::scheme and profiles/hardware); post-quantum arithmetic is modelled, \
             not executed."
                .to_string(),
            "sig_cbr_extra is an estimate from the security octet share; the measured CBR row \
             against the ECDSA P-256 column is the observed difference."
                .to_string(),
        ],
        files: Vec::new(),
    }
}

/// Which of a metric's dimension bins a row reads: delivery within 100 m, the radius the
/// 3GPP reliability requirement is read at; every bin otherwise.
fn dims_match(metric: &str, s: &MetricSample) -> bool {
    match metric {
        "pdr" => s.dims.iter().any(|(d, v)| d.to_string() == "radius" && v.to_string() == "100m"),
        _ => true,
    }
}

/// The smallest difference worth calling out, by unit.
fn resolution(unit: &str) -> f64 {
    match unit {
        "ratio" | "×" => 0.01,
        "ms" | "ms/s" => 0.1,
        "B/msg" | "B/s" | "B" | "count" | "1/s" => 1.0,
        _ => 1e-9,
    }
}

/// Pools one run's windows of one metric.
fn pool(samples: &[&MetricSample], stat: Stat) -> Option<f64> {
    let (mut s, mut t) = (0u64, 0u64);
    let (mut num, mut den) = (0.0f64, 0.0f64);
    let (mut wsum, mut wn) = (0.0f64, 0.0f64);
    let mut count: Option<u64> = None;
    for sample in samples {
        match &sample.value {
            SampleValue::Ratio(RatioEstimate::Proportion { successes, trials, .. }) => {
                s += successes;
                t += trials;
            }
            SampleValue::Ratio(RatioEstimate::RatioOfSums { numerator, denominator, .. }) => {
                num += numerator;
                den += denominator;
            }
            SampleValue::Scalar(Estimate::Value { point, n }) => {
                let w = (*n).max(1) as f64;
                wsum += point * w;
                wn += w;
            }
            SampleValue::Distribution(DistributionSummary::Summary { n, mean, p50, p95, .. }) => {
                let v = match stat {
                    Stat::P50 => *p50,
                    Stat::P95 => *p95,
                    Stat::Point => *mean,
                };
                wsum += v * (*n as f64);
                wn += *n as f64;
            }
            SampleValue::Count { count: c } => *count.get_or_insert(0) += c,
            _ => {}
        }
    }
    if t > 0 {
        Some(s as f64 / t as f64)
    } else if den > 0.0 {
        Some(num / den)
    } else if wn > 0.0 {
        Some(wsum / wn)
    } else {
        count.map(|c| c as f64)
    }
}

/// Formats a value for the Markdown table.
fn show(v: Option<f64>) -> String {
    match v {
        None => "—".to_string(),
        Some(v) if v.abs() >= 1000.0 => format!("{v:.0}"),
        Some(v) if v.abs() >= 10.0 => format!("{v:.1}"),
        Some(v) => format!("{v:.3}"),
    }
}

/// The Markdown rendering.
#[must_use]
pub fn markdown(c: &Comparison) -> String {
    let mut md = format!(
        "# Signature schemes compared: {}\n\nSame scenario, seed {} and {} s of simulated \
         time under every scheme; differences against **{}** in brackets, called-out ones in \
         bold.\n\n",
        c.scenario, c.seed_hex, c.duration_s, c.baseline
    );
    md.push_str("| Scheme | Mode | Signature B | Key B | Certificate +B | Signs on | Verifies on |\n|---|---|---|---|---|---|---|\n");
    for s in &c.schemes {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            s.id, s.mode, s.sig_octets, s.key_octets, s.cert_extra_octets, s.sign_engine, s.verify_engine
        ));
    }
    let mut group = "";
    for r in &c.rows {
        if r.group != group {
            group = &r.group;
            md.push_str(&format!("\n## {group}\n\n| Quantity | Unit |"));
            for s in &c.schemes {
                md.push_str(&format!(" {} |", s.id));
            }
            md.push_str("\n|---|---|");
            md.push_str(&"---|".repeat(c.schemes.len()));
            md.push('\n');
        }
        md.push_str(&format!("| {} | {} |", r.label, r.unit));
        for i in 0..c.schemes.len() {
            let mut cell = show(r.values[i]);
            if let Some(rel) = r.rel[i]
                && c.schemes[i].id != c.baseline
            {
                cell.push_str(&format!(" ({:+.0} %)", rel * 100.0));
            }
            if r.highlight[i] {
                cell = format!("**{cell}**");
            }
            md.push_str(&format!(" {cell} |"));
        }
        md.push('\n');
    }
    md.push_str("\n## Caveats\n\n");
    for x in &c.caveats {
        md.push_str(&format!("- {x}\n"));
    }
    md
}

/// The CSV rendering, long form.
#[must_use]
pub fn csv(c: &Comparison) -> String {
    let mut out = String::from("group,key,label,unit,scheme,mode,value,delta,rel,level,highlight\n");
    let q = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let num = |v: Option<f64>| v.map(|v| format!("{v}")).unwrap_or_default();
    for r in &c.rows {
        for (i, s) in c.schemes.iter().enumerate() {
            out.push_str(&format!(
                "{},{},{},{},{},{},{},{},{},{},{}\n",
                q(&r.group),
                r.key,
                q(&r.label),
                q(&r.unit),
                s.id,
                s.mode,
                num(r.values[i]),
                num(r.delta[i]),
                num(r.rel[i]),
                r.level[i].clone().unwrap_or_default(),
                r.highlight[i]
            ));
        }
    }
    out
}

fn write(c: &Comparison, out: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(out).map_err(|source| CliError::Io {
        what: "cannot create",
        path: out.display().to_string(),
        source,
    })?;
    let mut files = Vec::new();
    let json = serde_json::to_string_pretty(c).unwrap_or_default();
    for (name, text) in [
        ("comparison.json", json),
        ("comparison.csv", csv(c)),
        ("comparison.md", markdown(c)),
    ] {
        let path = out.join(name);
        std::fs::write(&path, text).map_err(|source| CliError::Io {
            what: "cannot write",
            path: path.display().to_string(),
            source,
        })?;
        files.push(path.display().to_string());
    }
    Ok(files)
}

/// The one-screen summary the command prints.
#[must_use]
pub fn format(c: &Comparison) -> String {
    let mut s = markdown(c);
    s.push_str("\nWritten:\n");
    for f in &c.files {
        s.push_str(&format!("  {f}\n"));
    }
    s
}

/// The comparison the Studio started, if any: `POST /agent/compare-signatures` starts one
/// on a thread, `GET /agent/compare-signatures` reads where it is.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CompareJob {
    /// Whether a comparison is running.
    pub running: bool,
    /// The schemes it compares.
    pub schemes: Vec<String>,
    /// How many runs have finished, read from the runs directory.
    pub done: usize,
    /// Where it writes.
    pub out: Option<String>,
    /// Why it failed.
    pub error: Option<String>,
    /// The finished comparison.
    pub result: Option<Comparison>,
}

static JOB: std::sync::Mutex<Option<CompareJob>> = std::sync::Mutex::new(None);

/// What the Studio's button asks for.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompareRequest {
    /// A scenario path the server can read, relative to where it was started.
    #[serde(default)]
    pub scenario: Option<String>,
    /// The scenario document itself, as the engine serves it.
    #[serde(default)]
    pub document: Option<serde_json::Value>,
    /// The scheme ids; the default four when empty.
    #[serde(default)]
    pub schemes: Vec<String>,
    /// Simulated seconds per run.
    #[serde(default)]
    pub duration_s: Option<f64>,
}

/// The shipped scenario a request with no scenario compares.
pub const SHIPPED: &str = "scenarios/compare-signature-schemes.yaml";

/// Starts a comparison on a thread, refusing while one runs.
///
/// # Errors
/// A message when one is already running or the scenario cannot be read.
pub fn start(req: CompareRequest) -> core::result::Result<CompareJob, String> {
    let scenario = match (&req.document, &req.scenario) {
        (Some(doc), _) => serde_json::from_value::<v2xw_engine::Scenario>(doc.clone())
            .map_err(|e| format!("the scenario the engine serves did not parse: {e}"))?,
        (None, Some(path)) => v2xw_engine::Scenario::load(path).map_err(|e| e.to_string())?,
        (None, None) => v2xw_engine::Scenario::load(SHIPPED).map_err(|e| e.to_string())?,
    };
    let mut guard = JOB
        .lock()
        .map_err(|_| "the comparison's lock is poisoned".to_string())?;
    if guard.as_ref().is_some_and(|j| j.running) {
        return Err("a comparison is already running".to_string());
    }
    let mut schemes: Vec<String> = if req.schemes.is_empty() {
        DEFAULT_SCHEMES.iter().map(|s| (*s).to_string()).collect()
    } else {
        req.schemes.clone()
    };
    if !schemes.iter().any(|s| s == BASELINE) {
        schemes.insert(0, BASELINE.to_string());
    }
    // One directory per scheme list and duration, so the runner's journal resumes a repeat
    // of the same request and never refuses a different one.
    let tag = v2xw_core::hash::sha256_hex(
        format!("{}|{:?}|{:?}", scenario.meta.name, schemes, req.duration_s).as_bytes(),
    );
    let out = PathBuf::from("runs").join(format!("signatures-{}", &tag[..12]));
    let job = CompareJob {
        running: true,
        schemes: schemes.clone(),
        done: 0,
        out: Some(out.display().to_string()),
        error: None,
        result: None,
    };
    *guard = Some(job.clone());
    drop(guard);
    let options = CompareOptions {
        scenario: PathBuf::new(),
        schemes,
        out: Some(out),
        duration_s: req.duration_s,
        build_utc: None,
        record: false,
    };
    std::thread::spawn(move || {
        let outcome = run_scenario(scenario, &options);
        if let Ok(mut g) = JOB.lock()
            && let Some(j) = g.as_mut()
        {
            j.running = false;
            match outcome {
                Ok(c) => {
                    j.done = c.schemes.len();
                    j.result = Some(c);
                }
                Err(e) => j.error = Some(e.to_string()),
            }
        }
    });
    Ok(job)
}

/// Where the Studio's comparison is.
#[must_use]
pub fn job() -> Option<CompareJob> {
    let mut j = JOB.lock().ok()?.clone()?;
    if j.running
        && let Some(out) = &j.out
    {
        j.done = std::fs::read_dir(Path::new(out).join(RUNS_DIR))
            .map(|d| {
                d.filter_map(core::result::Result::ok)
                    .filter(|e| e.path().join("metrics.json").exists())
                    .count()
            })
            .unwrap_or(0);
    }
    Some(j)
}
