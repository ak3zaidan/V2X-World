//! Compare two runs: what changed in the settings, what changed in the metrics, and how
//! much of it to believe.
//!
//! A run is reduced to a [`RunDigest`] — its scenario document, seed, city and one pooled
//! value per headline metric with a short time series — so a digest can be kept after the
//! run itself is gone (the live server keeps the last few, `runs.list`), written to a file
//! and read back (`runs.digest`, `v2xw compare`), or built from a `v2xw run` output
//! directory. [`compare`] turns two digests into a [`Comparison`]; the Studio, the agent and
//! `v2xw compare` all print that one value, so the three cannot disagree.
//!
//! # How a run's windows become one number
//!
//! As `v2xw compare-signatures` and the experiment runner do: proportions pool their counts,
//! ratios of sums their two sums, scalars and distributions are sample-count weighted means
//! of each window's figure (so a p95 is the typical window's p95), and counts are summed —
//! except a count whose metric aggregates by its maximum (`agg: max`: a revocation list's
//! entries and bytes "as last published", the memory high-water mark), which is a level
//! carried from window to window and pools to its largest value. Summing it would report a
//! list of 42 entries as 42 times the number of windows. A window too thin to estimate
//! alone is pooled by the counts it carries (a proportion's successes and trials, a
//! distribution's sum and n, which join the run's mean but not its percentiles), so a run's
//! value does not depend on its metric period.
//!
//! # When a difference is "meaningful"
//!
//! The rule of `v2xw_copilot::experiment` and `compare-signatures`: the value crosses a
//! health band, or moves by at least [`RELATIVE_FLOOR`] of the baseline and by more than the
//! unit's resolution. When the two runs used different seeds the difference must also exceed
//! twice the combined standard error, estimated per run by the method of batch means over
//! its windows (Law & Kelton, *Simulation Modeling and Analysis*, 5th ed., §9.5.3: 10 to 30
//! batches; we use up to 10 because a short run has few windows). Same seed: the two runs
//! share every random draw the settings did not touch, so the difference is the settings'.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use v2xw_metrics::health::{Better, HealthBand, HealthLevel};
use v2xw_metrics::stats::{DistributionSummary, Percentile, RatioEstimate};
use v2xw_metrics::{MetricSample, SampleValue};

/// The schema id a digest carries.
pub const DIGEST_SCHEMA: &str = "v2xw/run-digest/1";
/// The schema id a comparison carries.
pub const COMPARISON_SCHEMA: &str = "v2xw/run-comparison/1";
/// A difference this large relative to the baseline is called out.
pub const RELATIVE_FLOOR: f64 = 0.10;
/// How many points a digest keeps of each series.
pub const SERIES_POINTS: usize = 400;
/// How many batches the batch-means standard error uses at most.
const BATCHES: usize = 10;

/// One metric of a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DigestMetric {
    /// The series name, as `metrics.query` names it (`pdr`, `latency.p95`).
    pub name: String,
    /// The metric family (`latency` for `latency.p95`).
    pub base: String,
    /// The unit.
    #[serde(default)]
    pub unit: String,
    /// The run's pooled value; `None` when the run never observed it.
    pub value: Option<f64>,
    /// The samples (or windows) behind it.
    #[serde(default)]
    pub n: u64,
    /// The batch-means standard error of `value`, or across replications when the digest
    /// pools several runs.
    #[serde(default)]
    pub se: Option<f64>,
    /// Replications pooled into `value` (1 for one run).
    #[serde(default = "one")]
    pub replications: u32,
    /// `(t_s, value)` per window, at most [`SERIES_POINTS`].
    #[serde(default)]
    pub series: Vec<[f64; 2]>,
}

fn one() -> u32 {
    1
}

/// One run, reduced.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunDigest {
    /// [`DIGEST_SCHEMA`].
    pub schema: String,
    /// The run's id.
    pub run_id: String,
    /// The scenario's name.
    pub label: String,
    /// The run seed.
    pub seed: u64,
    /// The city, when the scenario is on one from the library.
    #[serde(default)]
    pub city: Option<String>,
    /// The scenario digest.
    #[serde(default)]
    pub scenario_hash: String,
    /// The scenario document the run executed.
    pub scenario: Value,
    /// How far the run got, simulated seconds.
    pub t_reached_s: f64,
    /// The scenario's horizon, simulated seconds.
    pub t_end_s: f64,
    /// Where it came from: `live`, or the directory path.
    #[serde(default)]
    pub source: String,
    /// Every headline metric.
    pub metrics: Vec<DigestMetric>,
}

impl RunDigest {
    /// Whether the run reached its horizon (within one second).
    #[must_use]
    pub fn finished(&self) -> bool {
        self.t_end_s > 0.0 && self.t_reached_s + 1.0 >= self.t_end_s
    }

    /// The short header a list shows.
    #[must_use]
    pub fn header(&self) -> Value {
        json!({
            "run_id": self.run_id,
            "label": self.label,
            "seed": self.seed,
            "city": self.city,
            "scenario_hash": self.scenario_hash,
            "t_reached_s": self.t_reached_s,
            "t_end_s": self.t_end_s,
            "finished": self.finished(),
            "source": self.source,
            "replications": self.metrics.iter().map(|m| m.replications).max().unwrap_or(1),
        })
    }

    /// Pools several runs of one configuration (replications) into one digest whose
    /// values are the mean across them and whose `se` is their standard error.
    #[must_use]
    pub fn replicated(runs: &[RunDigest]) -> Option<RunDigest> {
        let first = runs.first()?.clone();
        if runs.len() == 1 {
            return Some(first);
        }
        let mut out = first;
        let names: BTreeSet<String> = runs
            .iter()
            .flat_map(|r| r.metrics.iter().map(|m| m.name.clone()))
            .collect();
        let mut metrics = Vec::new();
        for name in names {
            let found: Vec<&DigestMetric> = runs
                .iter()
                .filter_map(|r| r.metrics.iter().find(|m| m.name == name))
                .collect();
            let values: Vec<f64> = found.iter().filter_map(|m| m.value).collect();
            let mut m = found[0].clone();
            m.value = mean(&values);
            m.n = found.iter().map(|m| m.n).sum();
            m.replications = u32::try_from(values.len()).unwrap_or(u32::MAX);
            m.se = if values.len() >= 2 {
                sd(&values).map(|s| s / (values.len() as f64).sqrt())
            } else {
                None
            };
            metrics.push(m);
        }
        out.metrics = metrics;
        out.label = format!("{} ({} replications)", out.label, runs.len());
        Some(out)
    }
}

// ------------------------------------------------------------------------------------------------
// Pooling
// ------------------------------------------------------------------------------------------------

/// A run's samples of one metric, pooled as they arrive.
#[derive(Debug, Clone, Default)]
pub struct Pool {
    unit: String,
    successes: u64,
    trials: u64,
    num: f64,
    den: f64,
    w: [f64; 4],
    wn: f64,
    count: Option<u64>,
    /// The largest window value of a count whose metric aggregates by its maximum.
    peak: Option<u64>,
    /// The sum and sample count of distribution windows too thin to summarise alone: they
    /// join the run's mean (a sum over every sample divided by their count), not its
    /// percentiles, which are the typical summarised window's.
    thin_sum: f64,
    thin_n: f64,
    distribution: bool,
    n: u64,
}

impl Pool {
    /// Adds one window's value; `agg` is the metric's aggregation tag (`Agg::tag`).
    pub fn add(&mut self, unit: &str, agg: &str, value: &SampleValue) {
        if self.unit.is_empty() {
            self.unit = unit.to_string();
        }
        if value.is_insufficient() {
            // A window too thin to estimate alone still carries its counts, so that it can be
            // pooled into a run that is not (`RatioEstimate::Insufficient`,
            // `DistributionSummary::Insufficient`). Dropping it would make a run's value
            // depend on its metric period: a 1 s period has thin windows a 5 s period has not.
            match value {
                SampleValue::Ratio(RatioEstimate::Insufficient {
                    trials,
                    successes: Some(successes),
                    ..
                }) => {
                    self.successes += successes;
                    self.trials += trials;
                    self.n += trials;
                }
                SampleValue::Distribution(DistributionSummary::Insufficient {
                    n,
                    sum: Some(sum),
                    ..
                }) if sum.is_finite() => {
                    self.distribution = true;
                    self.thin_sum += sum;
                    self.thin_n += *n as f64;
                    self.n += n;
                }
                _ => {}
            }
            return;
        }
        self.n += value.n();
        match value {
            SampleValue::Ratio(RatioEstimate::Proportion {
                successes, trials, ..
            }) => {
                self.successes += successes;
                self.trials += trials;
            }
            SampleValue::Ratio(RatioEstimate::RatioOfSums {
                numerator,
                denominator,
                ..
            }) => {
                self.num += numerator;
                self.den += denominator;
            }
            SampleValue::Distribution(d @ DistributionSummary::Summary { n, .. }) => {
                self.distribution = true;
                let w = *n as f64;
                let parts = [
                    d.mean(),
                    d.quantile(Percentile::P50),
                    d.quantile(Percentile::P95),
                    d.quantile(Percentile::P99),
                ];
                for (slot, v) in self.w.iter_mut().zip(parts) {
                    *slot += v.unwrap_or(0.0) * w;
                }
                self.wn += w;
            }
            SampleValue::Count { count } if agg == "max" => {
                let p = self.peak.get_or_insert(0);
                *p = (*p).max(*count);
            }
            SampleValue::Count { count } => *self.count.get_or_insert(0) += count,
            other => {
                if let Some(p) = other.point() {
                    let w = other.n().max(1) as f64;
                    self.w[0] += p * w;
                    self.wn += w;
                }
            }
        }
    }

    /// `(suffix, value)`: the headline under `""`, and a distribution's percentiles.
    #[must_use]
    pub fn values(&self) -> Vec<(&'static str, f64)> {
        let mut out = Vec::new();
        if self.trials > 0 {
            out.push(("", self.successes as f64 / self.trials as f64));
        } else if self.den > 0.0 {
            out.push(("", self.num / self.den));
        } else if self.wn + self.thin_n > 0.0 {
            out.push(("", (self.w[0] + self.thin_sum) / (self.wn + self.thin_n)));
            if self.distribution && self.wn > 0.0 {
                out.push((".p50", self.w[1] / self.wn));
                out.push((".p95", self.w[2] / self.wn));
                out.push((".p99", self.w[3] / self.wn));
            }
        } else if let Some(c) = self.count {
            out.push(("", c as f64));
        } else if let Some(p) = self.peak {
            out.push(("", p as f64));
        }
        out
    }
}

/// Whether a sample is a headline (no dimension but time).
#[must_use]
pub fn is_headline(sample: &MetricSample) -> bool {
    sample.dims.iter().all(|(d, _)| d.to_string() == "t")
}

/// Builds a digest's metrics from samples as they arrive.
#[derive(Debug, Clone, Default)]
pub struct DigestBuilder {
    pools: BTreeMap<String, Pool>,
    series: BTreeMap<String, Vec<(f64, f64)>>,
    keep_series: bool,
    t_max_ns: u64,
}

impl DigestBuilder {
    /// A builder; `keep_series` false when the caller already keeps the series.
    #[must_use]
    pub fn new(keep_series: bool) -> Self {
        Self {
            keep_series,
            ..Self::default()
        }
    }

    /// Adds one sample; dimensioned ones are not headlines and are skipped.
    pub fn add(&mut self, sample: &MetricSample) {
        if !is_headline(sample) {
            return;
        }
        let t: u64 = sample.t;
        self.t_max_ns = self.t_max_ns.max(t);
        self.pools.entry(sample.metric.clone()).or_default().add(
            &sample.unit,
            &sample.agg,
            &sample.value,
        );
        if self.keep_series && !sample.value.is_insufficient() {
            let ts = t as f64 / 1e9;
            let mut push = |name: String, v: Option<f64>| {
                if let Some(v) = v.filter(|v| v.is_finite()) {
                    self.series.entry(name).or_default().push((ts, v));
                }
            };
            push(sample.metric.clone(), sample.value.point());
            if let SampleValue::Distribution(d) = &sample.value {
                push(
                    format!("{}.p95", sample.metric),
                    d.quantile(Percentile::P95),
                );
            }
        }
    }

    /// The latest sample instant seen, seconds.
    #[must_use]
    pub fn t_max_s(&self) -> f64 {
        self.t_max_ns as f64 / 1e9
    }

    /// The digest metrics, with `series` (when given) taking the place of the kept ones.
    #[must_use]
    pub fn metrics(&self, series: Option<&BTreeMap<String, Vec<(f64, f64)>>>) -> Vec<DigestMetric> {
        let series = series.unwrap_or(&self.series);
        let mut out = Vec::new();
        for (base, pool) in &self.pools {
            for (suffix, value) in pool.values() {
                // The table keeps a distribution's mean and p95; p50 and p99 are in the
                // charts' series only.
                if suffix == ".p50" || suffix == ".p99" {
                    continue;
                }
                let name = format!("{base}{suffix}");
                let s = series.get(&name).map_or(&[][..], Vec::as_slice);
                out.push(DigestMetric {
                    base: base.clone(),
                    unit: pool.unit.clone(),
                    value: value.is_finite().then_some(value),
                    n: pool.n,
                    se: batch_se(s),
                    replications: 1,
                    series: decimate(s, SERIES_POINTS),
                    name,
                });
            }
        }
        out
    }
}

/// The batch-means standard error of a window series' mean (Law & Kelton §9.5.3).
#[must_use]
pub fn batch_se(series: &[(f64, f64)]) -> Option<f64> {
    let n = series.len();
    if n < 8 {
        return None;
    }
    let b = BATCHES.min(n / 2);
    let size = n / b;
    let means: Vec<f64> = (0..b)
        .filter_map(|i| {
            let chunk: Vec<f64> = series[i * size..(i + 1) * size]
                .iter()
                .map(|p| p.1)
                .collect();
            mean(&chunk)
        })
        .collect();
    sd(&means).map(|s| s / (means.len() as f64).sqrt())
}

/// At most `points` bin means of a series, in time order.
#[must_use]
pub fn decimate(series: &[(f64, f64)], points: usize) -> Vec<[f64; 2]> {
    if series.len() <= points {
        return series.iter().map(|&(t, v)| [t, v]).collect();
    }
    let per = series.len().div_ceil(points);
    series
        .chunks(per)
        .map(|c| {
            let k = c.len() as f64;
            [
                c.iter().map(|p| p.0).sum::<f64>() / k,
                c.iter().map(|p| p.1).sum::<f64>() / k,
            ]
        })
        .collect()
}

/// Two-sided 95 % Student t quantiles, t(0.975, ν) for ν = 1..30 (Abramowitz and Stegun,
/// Handbook of Mathematical Functions, Table 26.10).
const T975: [f64; 30] = [
    12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
    2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
    2.052, 2.048, 2.045, 2.042,
];

/// t(0.975, ν), rounding ν down (conservative); above 30 it falls toward 1.96 as 1/ν.
fn t975(df: f64) -> f64 {
    if !df.is_finite() || df < 1.0 {
        return T975[0];
    }
    if df > 30.0 {
        return 1.96 + (T975[29] - 1.96) * 30.0 / df;
    }
    T975[(df.floor() as usize).clamp(1, 30) - 1]
}

/// How many combined standard errors a difference must clear. Between two sets of
/// replications the standard errors come from only `n − 1` degrees of freedom each, so
/// the threshold is Welch's: t(0.975) at the Welch–Satterthwaite degrees of freedom (Law,
/// Simulation Modeling and Analysis 5e, §10.2.1). The flat 2 that a single run's batch
/// means use would let a seed-only change between 5 and 5 replications pass for real
/// about 8 % of the time per metric instead of 5 %.
fn noise_multiplier(sa: f64, sb: f64, (na, nb): (u32, u32)) -> f64 {
    if na < 2 || nb < 2 {
        return 2.0;
    }
    let (va, vb) = (sa * sa, sb * sb);
    let (fa, fb) = (f64::from(na - 1), f64::from(nb - 1));
    let den = va * va / fa + vb * vb / fb;
    let df = if den > 0.0 {
        (va + vb) * (va + vb) / den
    } else {
        fa + fb
    };
    t975(df)
}

fn mean(v: &[f64]) -> Option<f64> {
    (!v.is_empty()).then(|| v.iter().sum::<f64>() / v.len() as f64)
}

fn sd(v: &[f64]) -> Option<f64> {
    if v.len() < 2 {
        return None;
    }
    let m = mean(v)?;
    let var = v.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (v.len() - 1) as f64;
    Some(var.sqrt())
}

/// The city a scenario document is on, when it names one.
#[must_use]
pub fn city_of(doc: &Value) -> Option<String> {
    v2xw_engine::Scenario::from_document(doc.clone())
        .ok()
        .and_then(|s| s.current_city().map(|c| c.id.clone()))
}

/// A scenario's name, for a label.
#[must_use]
pub fn label_of(doc: &Value) -> String {
    ["/meta/name", "/name", "/meta/id", "/id"]
        .iter()
        .find_map(|p| doc.pointer(p).and_then(Value::as_str))
        .unwrap_or("run")
        .to_string()
}

// ------------------------------------------------------------------------------------------------
// Persistence
// ------------------------------------------------------------------------------------------------

/// How many digest files the store keeps on disk; older ones are deleted on save.
pub const STORED_DIGESTS: usize = 32;

/// Where finished runs' digests are kept between server starts: `V2XW_RUN_DIGESTS_DIR`, else
/// `runs/digests` under the working directory. `off` (or an empty value) turns the store off,
/// and unit tests never use it.
#[must_use]
pub fn digest_dir() -> Option<std::path::PathBuf> {
    if cfg!(test) {
        return None;
    }
    match std::env::var("V2XW_RUN_DIGESTS_DIR") {
        Ok(v) if v.is_empty() || v == "off" => None,
        Ok(v) => Some(v.into()),
        Err(_) => Some("runs/digests".into()),
    }
}

/// Write `d` to the store and prune it to [`STORED_DIGESTS`]. Failures are logged, never
/// fatal: the in-memory history still works.
pub fn save_digest(d: &RunDigest) {
    let Some(dir) = digest_dir() else { return };
    // The store's order is its file names' numeric prefix: one past the newest stored, so
    // a new digest sorts after every earlier one without reading a clock (ADR 0004; the
    // firewall's wall-clock check). Earlier stores named by milliseconds keep their order.
    let stamp = stored_files(&dir)
        .iter()
        .filter_map(|p| {
            let name = p.file_name()?.to_str()?;
            name.split('-').next()?.parse::<u128>().ok()
        })
        .max()
        .map_or(0, |n| n + 1);
    let id: String = d
        .run_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let path = dir.join(format!("{stamp:015}-{id}.json"));
    let res = std::fs::create_dir_all(&dir)
        .and_then(|()| serde_json::to_vec(d).map_err(std::io::Error::other))
        .and_then(|bytes| std::fs::write(&path, bytes));
    if let Err(e) = res {
        tracing::warn!("run digest not saved to {}: {e}", path.display());
        return;
    }
    let mut files = stored_files(&dir);
    while files.len() > STORED_DIGESTS {
        let _ = std::fs::remove_file(files.remove(files.len() - 1));
    }
}

/// The store's files, newest first.
fn stored_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    files.reverse();
    files
}

/// Up to `cap` stored digests, newest first; unreadable or other-schema files are skipped.
#[must_use]
pub fn load_digests(cap: usize) -> std::collections::VecDeque<RunDigest> {
    let Some(dir) = digest_dir() else {
        return std::collections::VecDeque::new();
    };
    stored_files(&dir)
        .into_iter()
        .filter_map(|p| {
            let d: RunDigest = serde_json::from_slice(&std::fs::read(p).ok()?).ok()?;
            (d.schema == DIGEST_SCHEMA).then_some(d)
        })
        .take(cap)
        .collect()
}

// ------------------------------------------------------------------------------------------------
// Direction and health
// ------------------------------------------------------------------------------------------------

/// The health band that judges `name` (a headline or a `.p95` series), if any.
#[must_use]
pub fn band_for_series(name: &str) -> Option<HealthBand> {
    let (base, suffix) = split(name);
    v2xw_metrics::health::band_for(base).filter(|b| b.judged == suffix)
}

/// `(base, suffix)`: `latency.p95` → (`latency`, `.p95`).
fn split(name: &str) -> (&str, &str) {
    for s in [".p50", ".p95", ".p99"] {
        if let Some(base) = name.strip_suffix(s) {
            return (base, s);
        }
    }
    (name, "")
}

/// Which way is better, for a metric family: the shared table in `v2xw_metrics::polarity`,
/// which the Studio's `polarityOf` reads too. `None` is neutral (a speed, a flow, a count).
#[must_use]
pub fn better_of(base: &str) -> Option<Better> {
    v2xw_metrics::polarity::better_of(base)
}

fn level_name(l: HealthLevel) -> &'static str {
    match l {
        HealthLevel::Healthy => "healthy",
        HealthLevel::Caution => "caution",
        HealthLevel::Unhealthy => "unhealthy",
    }
}

/// The smallest difference a unit can show.
fn resolution(unit: &str) -> f64 {
    match unit {
        "ratio" | "1" | "" | "fraction" => 1e-4,
        "ms" | "m" | "dB" | "dBm" | "s" => 1e-3,
        _ => 1e-6,
    }
}

// ------------------------------------------------------------------------------------------------
// The comparison
// ------------------------------------------------------------------------------------------------

/// One setting that differs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SettingChange {
    /// The dotted path.
    pub path: String,
    /// The settings window's label.
    pub label: String,
    /// The settings window's section.
    pub group: String,
    /// The unit, if any.
    pub unit: String,
    /// Run A's value, `null` when absent.
    pub before: Value,
    /// Run B's value, `null` when absent.
    pub after: Value,
    /// "20 dBm → 23 dBm".
    pub text: String,
}

/// One metric of the comparison.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetricRow {
    /// Series name.
    pub name: String,
    /// Metric family.
    pub base: String,
    /// Unit.
    pub unit: String,
    /// `higher`, `lower` or `neutral`.
    pub better: String,
    /// Run A (previous).
    pub a: Option<f64>,
    /// Run B (current).
    pub b: Option<f64>,
    /// `b - a`.
    pub delta: Option<f64>,
    /// `(b - a) / |a|` × 100; `None` when `a` is 0 or either is missing.
    pub pct: Option<f64>,
    /// `better`, `worse`, `changed` (neutral metric), `same`, or `missing`.
    pub verdict: String,
    /// Health level of A, when banded.
    pub level_a: Option<String>,
    /// Health level of B, when banded.
    pub level_b: Option<String>,
    /// Whether the difference is worth reading (see the module header).
    pub meaningful: bool,
    /// Whether it clears twice the combined standard error, when both are known.
    pub beyond_noise: Option<bool>,
}

/// Two runs, compared.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Comparison {
    /// [`COMPARISON_SCHEMA`].
    pub schema: String,
    /// Run A's header (the baseline, usually the previous run).
    pub a: Value,
    /// Run B's header.
    pub b: Value,
    /// Same seed on both sides.
    pub same_seed: bool,
    /// What the seeds mean for the differences.
    pub noise_note: String,
    /// Settings that differ (seed excluded; it is in `noise_note`).
    pub settings: Vec<SettingChange>,
    /// Every metric either run has, largest meaningful change first.
    pub rows: Vec<MetricRow>,
    /// The main differences, in plain sentences.
    pub summary: Vec<String>,
}

struct FieldInfo {
    label: String,
    group: String,
    unit: String,
}

fn field_index() -> &'static BTreeMap<String, FieldInfo> {
    static INDEX: OnceLock<BTreeMap<String, FieldInfo>> = OnceLock::new();
    INDEX.get_or_init(|| {
        v2xw_engine::scenario::publish::fields()
            .into_iter()
            .filter_map(|f| {
                let path = f.get("x-path")?.as_str()?.to_string();
                let s = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_string();
                Some((
                    path,
                    FieldInfo {
                        label: s("title"),
                        group: s("x-group"),
                        unit: s("unit"),
                    },
                ))
            })
            .collect()
    })
}

fn flatten(prefix: &str, v: &Value, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(map) if !map.is_empty() => {
            for (k, child) in map {
                let p = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten(&p, child, out);
            }
        }
        Value::Array(items) if items.iter().any(Value::is_object) => {
            for (i, child) in items.iter().enumerate() {
                flatten(&format!("{prefix}.{i}"), child, out);
            }
        }
        other => {
            out.insert(prefix.to_string(), other.clone());
        }
    }
}

fn show_value(v: &Value, unit: &str) -> String {
    let s = match v {
        Value::Null => return "(not set)".to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => {
            let s = other.to_string();
            if s.len() > 60 {
                format!(
                    "{}…",
                    &s[..s.char_indices().nth(57).map_or(s.len(), |c| c.0)]
                )
            } else {
                s
            }
        }
    };
    if unit.is_empty() || !v.is_number() {
        s
    } else {
        format!("{s} {unit}")
    }
}

/// Every setting that differs between two scenario documents, labelled as the settings
/// window labels it. `seed` is left out (the comparison reports it separately).
#[must_use]
pub fn settings_diff(a: &Value, b: &Value) -> Vec<SettingChange> {
    let (mut fa, mut fb) = (BTreeMap::new(), BTreeMap::new());
    flatten("", a, &mut fa);
    flatten("", b, &mut fb);
    let keys: BTreeSet<&String> = fa.keys().chain(fb.keys()).collect();
    let index = field_index();
    let mut out = Vec::new();
    for key in keys {
        // The seed is reported on its own; `meta` (name, description, tags, base) describes
        // a run and changes nothing it computes.
        if key == "seed" || key.starts_with("meta.") {
            continue;
        }
        let (va, vb) = (
            fa.get(key).cloned().unwrap_or(Value::Null),
            fb.get(key).cloned().unwrap_or(Value::Null),
        );
        if va == vb || numbers_equal(&va, &vb) {
            continue;
        }
        let bare: String = key
            .split('.')
            .filter(|s| s.parse::<usize>().is_err())
            .collect::<Vec<_>>()
            .join(".");
        let info = index.get(key.as_str()).or_else(|| index.get(&bare));
        let label = info
            .map(|i| i.label.clone())
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| humanise(key.rsplit('.').next().unwrap_or(key)));
        let unit = info.map(|i| i.unit.clone()).unwrap_or_default();
        let text = format!("{} → {}", show_value(&va, &unit), show_value(&vb, &unit));
        out.push(SettingChange {
            path: key.clone(),
            group: info.map(|i| i.group.clone()).unwrap_or_default(),
            label,
            unit,
            before: va,
            after: vb,
            text,
        });
    }
    out
}

fn numbers_equal(a: &Value, b: &Value) -> bool {
    matches!((a.as_f64(), b.as_f64()), (Some(x), Some(y)) if x == y)
}

/// `tx_power_dbm` → "Tx power dbm".
#[must_use]
pub fn humanise(key: &str) -> String {
    let w = key.replace('_', " ");
    let mut c = w.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// A metric series in words: `verify_queue_depth.p95` → "Verify queue depth (p95)",
/// `pdr` → "PDR".
#[must_use]
pub fn metric_words(name: &str) -> String {
    let (base, suffix) = split(name);
    const ACRONYMS: &[(&str, &str)] = &[
        ("pdr", "PDR"),
        ("per", "PER"),
        ("aoi", "AoI"),
        ("cbr", "CBR"),
        ("pir", "PIR"),
        ("ttc", "TTC"),
        ("pet", "PET"),
        ("crl", "CRL"),
        ("ma", "MA"),
        ("p2pcd", "P2PCD"),
        ("nar", "NAR"),
        ("rsu", "RSU"),
        ("obu", "OBU"),
        ("sinr", "SINR"),
        ("rssi", "RSSI"),
    ];
    let words: Vec<String> = base
        .split('_')
        .enumerate()
        .map(|(i, w)| {
            ACRONYMS.iter().find(|(k, _)| *k == w).map_or_else(
                || if i == 0 { humanise(w) } else { w.to_string() },
                |(_, v)| (*v).to_string(),
            )
        })
        .collect();
    let mut out = words.join(" ");
    if let Some(q) = suffix.strip_prefix('.') {
        out.push_str(&format!(" ({q})"));
    }
    out
}

fn better_name(b: Option<Better>) -> &'static str {
    match b {
        Some(Better::Higher) => "higher",
        Some(Better::Lower) => "lower",
        None => "neutral",
    }
}

/// Compares two digests; `a` is the baseline.
#[must_use]
pub fn compare(a: &RunDigest, b: &RunDigest) -> Comparison {
    let same_seed = a.seed == b.seed;
    let replicated = a
        .metrics
        .iter()
        .chain(&b.metrics)
        .any(|m| m.replications > 1);
    let noise_note = if replicated {
        "Replications exist: a difference is marked beyond noise when it exceeds twice the \
         combined standard error across replications."
            .to_string()
    } else if same_seed {
        format!(
            "Both runs used seed {}: every random draw the settings did not touch is shared, \
             so the differences are caused by the settings change.",
            a.seed
        )
    } else {
        format!(
            "Different seeds ({} and {}): part of every difference is random noise. A \
             difference is marked beyond noise only when it exceeds twice the combined \
             batch-means standard error; rerun with the same seed to isolate the settings.",
            a.seed, b.seed
        )
    };
    let settings = settings_diff(&a.scenario, &b.scenario);

    let names: BTreeSet<&str> = a
        .metrics
        .iter()
        .chain(&b.metrics)
        .map(|m| m.name.as_str())
        .collect();
    let mut rows = Vec::new();
    for name in names {
        let ma = a.metrics.iter().find(|m| m.name == name);
        let mb = b.metrics.iter().find(|m| m.name == name);
        let any = ma.or(mb).expect("named by one side");
        let (va, vb) = (ma.and_then(|m| m.value), mb.and_then(|m| m.value));
        let better = better_of(&any.base);
        let band = band_for_series(name);
        let level = |v: Option<f64>| {
            v.and_then(|v| band.as_ref().and_then(|b| b.classify(v)))
                .map(|l| level_name(l).to_string())
        };
        let (level_a, level_b) = (level(va), level(vb));
        let delta = va.zip(vb).map(|(x, y)| y - x);
        let pct = va
            .zip(delta)
            .and_then(|(x, d)| (x.abs() > 0.0).then(|| d / x.abs() * 100.0));
        let se = ma.and_then(|m| m.se).zip(mb.and_then(|m| m.se));
        let reps = (
            ma.map_or(1, |m| m.replications),
            mb.map_or(1, |m| m.replications),
        );
        let beyond_noise = se.zip(delta).map(|((sa, sb), d)| {
            d.abs() > noise_multiplier(sa, sb, reps) * (sa * sa + sb * sb).sqrt()
        });
        let res = resolution(&any.unit);
        let big = match (va, delta) {
            (Some(x), Some(d)) => {
                d.abs() > res && (x.abs() <= res || d.abs() >= RELATIVE_FLOOR * x.abs())
            }
            _ => false,
        };
        let crossed = level_a.is_some() && level_b.is_some() && level_a != level_b;
        let noise_ok = same_seed && !replicated || beyond_noise != Some(false);
        let meaningful = (big || crossed) && noise_ok;
        let verdict = match delta {
            None => "missing",
            Some(d) if d.abs() <= res => "same",
            Some(d) => match better {
                None => "changed",
                Some(Better::Higher) if d > 0.0 => "better",
                Some(Better::Lower) if d < 0.0 => "better",
                Some(_) => "worse",
            },
        };
        rows.push(MetricRow {
            name: name.to_string(),
            base: any.base.clone(),
            unit: any.unit.clone(),
            better: better_name(better).to_string(),
            a: va,
            b: vb,
            delta,
            pct,
            verdict: verdict.to_string(),
            level_a,
            level_b,
            meaningful,
            beyond_noise,
        });
    }
    rows.sort_by(|x, y| {
        let key = |r: &MetricRow| {
            let crossed = r.level_a.is_some() && r.level_a != r.level_b;
            let size = r.pct.map_or_else(
                || {
                    if r.delta.is_some_and(|d| d != 0.0) {
                        1e9
                    } else {
                        0.0
                    }
                },
                f64::abs,
            );
            (r.meaningful, crossed, size)
        };
        let (kx, ky) = (key(x), key(y));
        ky.0.cmp(&kx.0)
            .then(ky.1.cmp(&kx.1))
            .then(ky.2.total_cmp(&kx.2))
            .then(x.name.cmp(&y.name))
    });
    let summary = summarise(a, b, &settings, &rows, same_seed);
    Comparison {
        schema: COMPARISON_SCHEMA.to_string(),
        a: a.header(),
        b: b.header(),
        same_seed,
        noise_note,
        settings,
        rows,
        summary,
    }
}

/// A value with its unit, short.
#[must_use]
pub fn fmt_value(v: Option<f64>, unit: &str) -> String {
    let Some(v) = v else {
        return "—".to_string();
    };
    let a = v.abs();
    let s = if a == 0.0 {
        "0".to_string()
    } else if a >= 1000.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else if a >= 0.01 {
        format!("{v:.3}")
    } else {
        format!("{v:.2e}")
    };
    match unit {
        "" | "1" | "ratio" | "fraction" | "count" => s,
        u => format!("{s} {u}"),
    }
}

fn fmt_pct(p: Option<f64>) -> String {
    p.map_or_else(String::new, |p| format!("{p:+.1} %"))
}

fn summarise(
    a: &RunDigest,
    b: &RunDigest,
    settings: &[SettingChange],
    rows: &[MetricRow],
    same_seed: bool,
) -> Vec<String> {
    let mut out = Vec::new();
    match settings.len() {
        0 if same_seed => out.push(
            "The two runs have identical settings and seed, so they should be identical."
                .to_string(),
        ),
        0 => out.push(format!(
            "The settings are identical; only the seed changed ({} → {}), so every \
             difference below is run-to-run noise.",
            a.seed, b.seed
        )),
        n => {
            let shown: Vec<String> = settings
                .iter()
                .take(3)
                .map(|s| format!("{} {}", s.label, s.text))
                .collect();
            let more = if n > 3 {
                format!(" and {} more", n - 3)
            } else {
                String::new()
            };
            out.push(format!(
                "{n} setting{} changed: {}{more}.",
                if n == 1 { "" } else { "s" },
                shown.join("; ")
            ));
        }
    }
    if !same_seed && !settings.is_empty() {
        out.push(format!(
            "The seed also changed ({} → {}), so small differences may be noise.",
            a.seed, b.seed
        ));
    }
    if a.t_reached_s + 1.0 < b.t_reached_s.min(a.t_end_s)
        || b.t_reached_s + 1.0 < a.t_reached_s.min(b.t_end_s)
    {
        out.push(format!(
            "The runs cover different spans ({:.0} s and {:.0} s of simulated time); a \
             metric that builds up over a run is not comparable until both finish.",
            a.t_reached_s, b.t_reached_s
        ));
    }
    let meaningful: Vec<&MetricRow> = rows.iter().filter(|r| r.meaningful).collect();
    let better = meaningful.iter().filter(|r| r.verdict == "better").count();
    let worse = meaningful.iter().filter(|r| r.verdict == "worse").count();
    let compared = rows.iter().filter(|r| r.delta.is_some()).count();
    out.push(if meaningful.is_empty() {
        format!("None of the {compared} metrics compared changed meaningfully.")
    } else {
        format!(
            "{} of {compared} metrics changed meaningfully: {better} better, {worse} worse, {} \
             neutral.",
            meaningful.len(),
            meaningful.len() - better - worse
        )
    });
    for r in meaningful.iter().take(4) {
        let dir = if r.delta.unwrap_or(0.0) > 0.0 {
            "rose"
        } else {
            "fell"
        };
        let judged = match r.verdict.as_str() {
            "better" => ", an improvement",
            "worse" => ", a deterioration",
            _ => "",
        };
        let pct = r.pct.map_or_else(String::new, |p| format!(" ({p:+.0} %)"));
        out.push(format!(
            "{} {dir} from {} to {}{pct}{judged}.",
            metric_words(&r.name),
            fmt_value(r.a, &r.unit),
            fmt_value(r.b, &r.unit),
        ));
    }
    for r in rows {
        if let (Some(la), Some(lb)) = (&r.level_a, &r.level_b)
            && la != lb
        {
            out.push(format!(
                "{} moved from {la} to {lb}.",
                metric_words(&r.name)
            ));
        }
    }
    out
}

/// The comparison as a Markdown document.
#[must_use]
pub fn markdown(c: &Comparison) -> String {
    let mut s = String::new();
    let h = |v: &Value, k: &str| {
        v.get(k).map_or(String::new(), |x| {
            x.to_string().trim_matches('"').to_string()
        })
    };
    s.push_str(&format!(
        "# Run comparison\n\n- **A (baseline):** {} — run `{}`, seed {}\n- **B:** {} — run `{}`, seed {}\n\n> {}\n\n",
        h(&c.a, "label"), h(&c.a, "run_id"), h(&c.a, "seed"),
        h(&c.b, "label"), h(&c.b, "run_id"), h(&c.b, "seed"), c.noise_note
    ));
    s.push_str("## Summary\n\n");
    for line in &c.summary {
        s.push_str(&format!("- {line}\n"));
    }
    s.push_str("\n## Settings changed\n\n");
    if c.settings.is_empty() {
        s.push_str("None.\n");
    } else {
        s.push_str("| Setting | Path | A | B |\n|---|---|---|---|\n");
        for x in &c.settings {
            s.push_str(&format!(
                "| {} | `{}` | {} | {} |\n",
                x.label,
                x.path,
                show_value(&x.before, &x.unit),
                show_value(&x.after, &x.unit)
            ));
        }
    }
    s.push_str("\n## Metrics\n\n| Metric | A | B | Change | % | Verdict | Health A → B |\n|---|---|---|---|---|---|---|\n");
    for r in &c.rows {
        let mark = if r.meaningful { "**" } else { "" };
        s.push_str(&format!(
            "| {mark}{}{mark} | {} | {} | {} | {} | {} | {} |\n",
            r.name,
            fmt_value(r.a, &r.unit),
            fmt_value(r.b, &r.unit),
            r.delta
                .map_or_else(String::new, |d| fmt_value(Some(d), &r.unit)),
            fmt_pct(r.pct),
            r.verdict,
            match (&r.level_a, &r.level_b) {
                (Some(a), Some(b)) => format!("{a} → {b}"),
                _ => String::new(),
            }
        ));
    }
    s
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// The comparison as CSV: one line per metric, then one per setting.
#[must_use]
pub fn csv(c: &Comparison) -> String {
    let num = |v: Option<f64>| v.map_or_else(String::new, |v| v.to_string());
    let mut s = String::from(
        "kind,name,unit,a,b,change,pct_change,verdict,better,health_a,health_b,meaningful,beyond_noise\n",
    );
    for r in &c.rows {
        s.push_str(&format!(
            "metric,{},{},{},{},{},{},{},{},{},{},{},{}\n",
            csv_cell(&r.name),
            csv_cell(&r.unit),
            num(r.a),
            num(r.b),
            num(r.delta),
            num(r.pct),
            r.verdict,
            r.better,
            r.level_a.as_deref().unwrap_or(""),
            r.level_b.as_deref().unwrap_or(""),
            r.meaningful,
            r.beyond_noise.map_or(String::new(), |b| b.to_string()),
        ));
    }
    for x in &c.settings {
        s.push_str(&format!(
            "setting,{},{},{},{},,,,,,,,\n",
            csv_cell(&x.path),
            csv_cell(&x.unit),
            csv_cell(&show_value(&x.before, "")),
            csv_cell(&show_value(&x.after, "")),
        ));
    }
    s
}

/// The table for a terminal: summary, settings, then the meaningful rows (all with `all`).
#[must_use]
pub fn text(c: &Comparison, all: bool) -> String {
    let mut s = String::new();
    for line in &c.summary {
        s.push_str(&format!("  {line}\n"));
    }
    s.push_str(&format!("  {}\n\n", c.noise_note));
    if !c.settings.is_empty() {
        s.push_str("Settings changed:\n");
        for x in &c.settings {
            s.push_str(&format!("  {:<40} {}\n", x.label, x.text));
        }
        s.push('\n');
    }
    s.push_str(&format!(
        "{:<34} {:>14} {:>14} {:>14} {:>9}  {}\n",
        "metric", "A", "B", "change", "%", "verdict"
    ));
    for r in c.rows.iter().filter(|r| all || r.meaningful) {
        s.push_str(&format!(
            "{:<34} {:>14} {:>14} {:>14} {:>9}  {}{}\n",
            r.name,
            fmt_value(r.a, &r.unit),
            fmt_value(r.b, &r.unit),
            r.delta
                .map_or_else(String::new, |d| fmt_value(Some(d), &r.unit)),
            fmt_pct(r.pct),
            r.verdict,
            match (&r.level_a, &r.level_b) {
                (Some(a), Some(b)) if a != b => format!(" ({a} → {b})"),
                _ => String::new(),
            }
        ));
    }
    if !all {
        let hidden = c.rows.iter().filter(|r| !r.meaningful).count();
        s.push_str(&format!(
            "  ({hidden} rows without a meaningful change hidden; --all shows them)\n"
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(seed: u64, power: f64, pdr: f64, lat: f64) -> RunDigest {
        let m = |name: &str, unit: &str, v: f64| DigestMetric {
            name: name.into(),
            base: split(name).0.into(),
            unit: unit.into(),
            value: Some(v),
            n: 100,
            se: None,
            replications: 1,
            series: vec![[1.0, v]],
        };
        RunDigest {
            schema: DIGEST_SCHEMA.into(),
            run_id: format!("r{seed}{power}"),
            label: "t".into(),
            seed,
            city: None,
            scenario_hash: String::new(),
            scenario: json!({"seed": seed, "meta": {"name": format!("r{power}")},
                             "network": {"radio": {"tx_power_dbm": power}}}),
            t_reached_s: 60.0,
            t_end_s: 60.0,
            source: "test".into(),
            metrics: vec![
                m("pdr", "ratio", pdr),
                m("cbr", "ratio", 0.3),
                m("latency", "ms", lat),
            ],
        }
    }

    #[test]
    fn same_seed_settings_change_is_attributed_and_ranked() {
        let a = digest(1, 20.0, 0.80, 10.0);
        let b = digest(1, 23.0, 0.90, 10.0);
        let c = compare(&a, &b);
        assert!(c.same_seed);
        assert_eq!(c.settings.len(), 1);
        assert!(c.settings[0].text.contains("20") && c.settings[0].text.contains("23"));
        assert_eq!(c.rows[0].name, "pdr", "the largest meaningful change leads");
        assert_eq!(c.rows[0].verdict, "better");
        assert!(c.rows[0].meaningful);
        assert!(!c.rows.iter().find(|r| r.name == "cbr").unwrap().meaningful);
        assert!(c.noise_note.contains("caused by the settings"));
        assert!(c.summary.iter().any(|l| l.contains("1 setting changed")));
    }

    #[test]
    fn worse_follows_the_health_direction() {
        let a = digest(1, 20.0, 0.9, 10.0);
        let b = digest(1, 20.0, 0.9, 30.0);
        let c = compare(&a, &b);
        let lat = c.rows.iter().find(|r| r.name == "latency").unwrap();
        assert_eq!(lat.verdict, "worse", "a latency going up is worse");
        let pdr = c.rows.iter().find(|r| r.name == "pdr").unwrap();
        assert_eq!(pdr.verdict, "same");
    }

    #[test]
    fn replications_use_the_welch_t_threshold() {
        // 5 and 5 replications, equal errors: ν = 8, t = 2.306, so a 2.1-SE difference
        // that the flat 2 would call real is noise.
        assert!((noise_multiplier(1.0, 1.0, (5, 5)) - 2.306).abs() < 1e-9);
        assert_eq!(noise_multiplier(1.0, 1.0, (1, 1)), 2.0);
        assert!(noise_multiplier(1.0, 1.0, (200, 200)) < 1.97);
        let mut a = digest(1, 20.0, 0.80, 10.0);
        let mut b = digest(2, 20.0, 0.80 + 2.1 * 0.02_f64.sqrt() * 0.1, 10.0);
        for (d, m) in [(&mut a, 0), (&mut b, 0)] {
            d.metrics[m].se = Some(0.01);
            d.metrics[m].replications = 5;
        }
        let pdr = compare(&a, &b);
        let pdr = pdr.rows.iter().find(|r| r.name == "pdr").unwrap();
        assert_eq!(pdr.beyond_noise, Some(false));
    }

    #[test]
    fn different_seeds_warn_and_noise_hides_small_changes() {
        let mut a = digest(1, 20.0, 0.80, 10.0);
        let mut b = digest(2, 20.0, 0.85, 10.0);
        a.metrics[0].se = Some(0.05);
        b.metrics[0].se = Some(0.05);
        let c = compare(&a, &b);
        assert!(!c.same_seed);
        assert!(c.noise_note.contains("noise"));
        let pdr = c.rows.iter().find(|r| r.name == "pdr").unwrap();
        assert_eq!(pdr.beyond_noise, Some(false));
        assert!(
            !pdr.meaningful,
            "inside twice the combined error: not called out"
        );
    }

    #[test]
    fn exports_carry_every_row() {
        let c = compare(&digest(1, 20.0, 0.8, 10.0), &digest(1, 23.0, 0.9, 12.0));
        assert_eq!(csv(&c).lines().count(), 1 + c.rows.len() + c.settings.len());
        assert_eq!(
            metric_words("verify_queue_depth.p95"),
            "Verify queue depth (p95)"
        );
        assert_eq!(metric_words("pdr_all_pairs"), "PDR all pairs");
        assert!(markdown(&c).contains("| **pdr** |"));
    }

    #[test]
    fn pooling_counts_proportions_and_batch_se() {
        let mut p = Pool::default();
        for (s, t) in [(8u64, 10u64), (1, 10)] {
            p.add(
                "ratio",
                "ratio",
                &SampleValue::Ratio(RatioEstimate::Proportion {
                    point: s as f64 / t as f64,
                    ci_lo: 0.0,
                    ci_hi: 1.0,
                    level: v2xw_metrics::stats::ConfidenceLevel::default(),
                    trials: t,
                    successes: s,
                }),
            );
        }
        assert_eq!(p.values(), vec![("", 0.45)]);
        let flat: Vec<(f64, f64)> = (0..20).map(|i| (f64::from(i), 1.0)).collect();
        assert_eq!(batch_se(&flat), Some(0.0));
        assert!(batch_se(&flat[..4]).is_none());
    }

    /// A count a metric aggregates by its maximum is a level carried from window to window
    /// (the CRL "as last published"), so the run's value is its largest window, not the sum
    /// of every window's copy of it; a per-window count (events processed) is still summed.
    /// Live Studio track (docs/validation/live-studio-2026-10-25.md, SV.compare-records).
    #[test]
    fn a_max_aggregated_count_pools_to_its_peak_and_a_window_count_to_its_sum() {
        use v2xw_core::ctx::Visibility;
        use v2xw_metrics::{Agg, Dim, Dims, MetricDef, Quantum};
        let crl = MetricDef::new(
            "crl_entries",
            "count",
            Agg::Max,
            Visibility::Public,
            Quantum::COUNT,
            "entries",
        )
        .with_dims([Dim::T]);
        let events = MetricDef::new(
            "events_processed",
            "count",
            Agg::Count,
            Visibility::Public,
            Quantum::COUNT,
            "events",
        )
        .with_dims([Dim::T]);
        let mut b = DigestBuilder::new(false);
        for (i, (entries, n)) in [(3_u64, 10_u64), (3, 20), (5, 30), (5, 40)]
            .into_iter()
            .enumerate()
        {
            let t = (i as u64 + 1) * 1_000_000_000;
            b.add(&MetricSample::new(
                &crl,
                t,
                Dims::new(),
                SampleValue::count(entries),
            ));
            b.add(&MetricSample::new(
                &events,
                t,
                Dims::new(),
                SampleValue::count(n),
            ));
        }
        let value = |name: &str| {
            b.metrics(None)
                .into_iter()
                .find(|m| m.name == name)
                .and_then(|m| m.value)
        };
        assert_eq!(value("crl_entries"), Some(5.0));
        assert_eq!(value("events_processed"), Some(100.0));
    }

    /// The same receptions and latencies binned at a 1 s and a 5 s metric period pool to the
    /// same run value: a window too thin to estimate alone (below `min_trials`, or below a
    /// distribution's `min_samples`) is pooled by the counts it carries, not dropped. Live
    /// Studio track, LV.period5.pdr: the run's PDR moved with the period (+0.012 %) because
    /// the 1 s run's two thin windows were left out.
    #[test]
    fn a_thin_window_is_pooled_by_its_counts_so_the_period_does_not_move_a_run() {
        use v2xw_core::ctx::Visibility;
        use v2xw_metrics::stats::{ConfidenceLevel, Distribution, Proportion};
        use v2xw_metrics::{Agg, Dim, Dims, MetricDef, Quantum};
        let pdr = MetricDef::new(
            "pdr",
            "ratio",
            Agg::Ratio {
                numerator: "rx_ok".into(),
                denominator: "rx_attempts".into(),
            },
            Visibility::Public,
            Quantum::PROBABILITY,
            "delivery",
        )
        .with_dims([Dim::T]);
        let lat = MetricDef::new(
            "e2e_latency",
            "ms",
            Agg::Mean,
            Visibility::Public,
            Quantum::TIME_MS,
            "latency",
        )
        .with_dims([Dim::T]);
        // Ten 1 s windows: (successes, trials) and that window's latencies (ms). Windows 0,
        // 2, 5 and 7 are below 10 trials and below 5 latency samples, and deliver better and
        // slower than the rest, so leaving them out moves both run values.
        let windows: [((u64, u64), &[f64]); 10] = [
            ((3, 3), &[50.0, 52.0]),
            ((10, 40), &[10.0, 11.0, 12.0, 9.0, 10.0, 11.0]),
            ((2, 2), &[60.0]),
            ((20, 50), &[8.0, 9.0, 10.0, 11.0, 12.0]),
            ((15, 30), &[10.0, 10.0, 10.0, 10.0, 10.0, 10.0, 10.0]),
            ((1, 1), &[70.0, 40.0, 41.0]),
            ((9, 45), &[12.0, 13.0, 14.0, 15.0, 16.0]),
            ((4, 4), &[55.0]),
            ((7, 35), &[9.0, 9.0, 9.0, 9.0, 9.0]),
            ((5, 25), &[11.0, 12.0, 13.0, 14.0, 15.0, 16.0]),
        ];
        let pooled = |period: usize| {
            let mut b = DigestBuilder::new(false);
            for (k, group) in windows.chunks(period).enumerate() {
                let (mut s, mut n) = (0, 0);
                let mut d = Distribution::default();
                for ((ws, wn), xs) in group {
                    s += ws;
                    n += wn;
                    d.observe_all(xs.iter().copied());
                }
                let t = ((k + 1) * period) as u64 * 1_000_000_000;
                let ratio = Proportion::from_counts(s, n).estimate(10, ConfidenceLevel::P95);
                b.add(&MetricSample::new(
                    &pdr,
                    t,
                    Dims::new(),
                    SampleValue::Ratio(ratio),
                ));
                b.add(&MetricSample::new(
                    &lat,
                    t,
                    Dims::new(),
                    SampleValue::Distribution(d.summary(5)),
                ));
            }
            let value = |name: &str| {
                b.metrics(None)
                    .into_iter()
                    .find(|m| m.name == name)
                    .and_then(|m| m.value)
                    .unwrap()
            };
            (value("pdr"), value("e2e_latency"))
        };
        let (s, n) = windows
            .iter()
            .fold((0, 0), |(s, n), ((a, b), _)| (s + a, n + b));
        let all: Vec<f64> = windows
            .iter()
            .flat_map(|(_, xs)| xs.iter().copied())
            .collect();
        let mean = all.iter().sum::<f64>() / all.len() as f64;
        for period in [1, 5] {
            let (p, l) = pooled(period);
            assert!(
                (p - s as f64 / n as f64).abs() < 1e-9,
                "period {period} s: pdr {p} against {s}/{n}"
            );
            assert!(
                (l - mean).abs() < 2e-3,
                "period {period} s: mean latency {l} against {mean}"
            );
        }
    }
}
