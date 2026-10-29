//! One-at-a-time sensitivity checks: does a parameter move the metric theory says it moves?
//!
//! A [`Spec`] names a base scenario and a list of [`Row`]s. Each row changes a few scenario
//! fields (`set`), runs the base and the changed scenario **on the same seed**, and checks
//! the named metric against an expected direction and, where theory gives one, a band for
//! the ratio changed / base. Same seed means common random numbers: the two runs draw the
//! same vehicles, the same phases and the same fading streams wherever the change does not
//! reach, so a small effect is not lost in run-to-run noise.
//!
//! The metrics are measured by a [`SensitivityRecorder`] while the run goes, from the
//! records the engine emits (`phy.prr`, `node.tx`, `node.rx`, `mac.cbr`) and from the
//! [`RunReport`]. It keeps running sums and histograms, never the records, so a large fleet
//! does not hold its `phy.rx` stream in memory. [`Measured::get`] lists the names.
//!
//! `v2xw sensitivity <spec.yaml>` runs a spec file; `crates/v2xw-engine/tests/` call
//! [`check_row`] directly, one test per row. `v2xw sensitivity --catalogue` runs
//! [`sweep`], every published setting at once.
//!
//! # Spec file
//!
//! ```yaml
//! base: ../scenarios/sensitivity/dsrc-grid.yaml   # relative to this file
//! set: { time.duration_s: 3.0 }                   # applied to every run
//! rows:
//!   - name: lower transmit power
//!     set: { radio.devices.obu.tx_power_dbm: 5.0 }
//!     metric: range_m
//!     expect: down          # up | down | same | changes
//!     band: [0.2, 0.8]      # optional: changed / base must fall in it
//!     min_change: 0.05      # optional: relative change required (default 0.01)
//!     base_set: { }         # optional: fields applied to both runs of this row
//!     source: Friis / log-distance law
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use v2xw_core::ctx::OwnedRecord;
use v2xw_core::time::SimTime;
use v2xw_metrics::channels::{MacCbrView, NodeRxView, NodeTxView, PhyPrrView, RxFate, decode};

use crate::ctx::RunRecorder;
use crate::error::{EngineError, Result, ScenarioError};
use crate::run::{Engine, RunReport};
use crate::scenario::Scenario;

pub mod expect;
pub mod sweep;

/// The direction a row expects its metric to move.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// The changed run's value is larger.
    Up,
    /// The changed run's value is smaller.
    Down,
    /// The value stays within `min_change` of the base.
    Same,
    /// The value moves by at least `min_change`, either way: for a parameter whose
    /// direction theory does not fix (a non-monotone trade-off).
    Changes,
}

/// One perturbation and the metric it should move.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Row {
    /// A short label.
    pub name: String,
    /// The scenario fields the changed run sets, by dotted path.
    #[serde(default)]
    pub set: BTreeMap<String, Value>,
    /// Fields applied to both the base and the changed run of this row (a heavier load, a
    /// RAT), by dotted path.
    #[serde(default)]
    pub base_set: BTreeMap<String, Value>,
    /// The metric, a [`Measured::get`] name.
    pub metric: String,
    /// Which way it should move.
    pub expect: Direction,
    /// The band `changed / base` must fall in, where theory gives a magnitude.
    #[serde(default)]
    pub band: Option<[f64; 2]>,
    /// The relative change required for `up`/`down` (and allowed for `same`). Default 0.01.
    #[serde(default)]
    pub min_change: Option<f64>,
    /// Where the expectation comes from.
    #[serde(default)]
    pub source: Option<String>,
}

/// A spec file: a base scenario and its rows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    /// The base scenario, relative to the spec file.
    pub base: String,
    /// Fields applied to every run.
    #[serde(default)]
    pub set: BTreeMap<String, Value>,
    /// The rows.
    pub rows: Vec<Row>,
}

impl Spec {
    /// Reads a spec file and loads its base scenario (unvalidated: the rows' fields are
    /// validated with it, run by run).
    ///
    /// # Errors
    /// The file cannot be read or parsed, or the base scenario cannot be loaded.
    pub fn load(path: impl AsRef<Path>) -> Result<(Spec, Scenario)> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| EngineError::Io {
            path: path.display().to_string(),
            source,
        })?;
        let spec: Spec = serde_yml::from_str(&text).map_err(|e| {
            EngineError::Scenario(ScenarioError::Parse {
                path: path.display().to_string(),
                message: e.to_string(),
            })
        })?;
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let base_path: PathBuf = dir.join(&spec.base);
        let mut base = Scenario::load_unvalidated(&base_path)?;
        absolutise_world(&mut base, &base_path);
        let base = apply(&base, &spec.set)?;
        Ok((spec, base))
    }
}

/// Makes a relative OSM path, and a relative DEM path (`world.terrain.dem`), absolute:
/// against the working directory when it exists there (the shipped scenarios name
/// `worlds/cache/...` from the repository root), else against the scenario's own directory
/// or the first parent of it that holds the file.
fn absolutise_world(s: &mut Scenario, scenario_path: &Path) {
    let resolve = |path: &mut String| {
        if !Path::new(path.as_str()).is_relative() || Path::new(path.as_str()).exists() {
            return;
        }
        let Some(dir) = scenario_path.parent() else {
            return;
        };
        let mut d = dir.to_path_buf();
        // Walk up to the directory that holds it (scenarios/sub/x.yaml -> repo root).
        for _ in 0..4 {
            if d.join(path.as_str()).exists() {
                *path = d.join(path.as_str()).to_string_lossy().into_owned();
                return;
            }
            if !d.pop() {
                break;
            }
        }
        // A base named through `..` (`../real-world/x.yaml` from scenarios/sensitivity)
        // pops back onto its own `..` lexically and never reaches the repository root in
        // four steps; the same walk from the resolved directory does. Tried only after the
        // lexical walk, so every path that resolved before resolves as before.
        let Ok(mut d) = std::fs::canonicalize(dir) else {
            return;
        };
        for _ in 0..4 {
            if d.join(path.as_str()).exists() {
                *path = d.join(path.as_str()).to_string_lossy().into_owned();
                return;
            }
            if !d.pop() {
                break;
            }
        }
    };
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source {
        resolve(path);
    }
    if let Some(dem) = s.world.terrain.dem.as_mut() {
        resolve(dem);
    }
}

/// `s` with each dotted path in `set` replaced, re-read through the schema. Intermediate
/// objects that are absent or null are created.
///
/// # Errors
/// A path that does not parse into the schema.
pub fn apply(s: &Scenario, set: &BTreeMap<String, Value>) -> Result<Scenario> {
    if set.is_empty() {
        return Ok(s.clone());
    }
    let mut doc = serde_json::to_value(s)
        .map_err(|e| EngineError::Scenario(ScenarioError::conflict("<scenario>", e.to_string())))?;
    for (path, value) in set {
        let mut at = &mut doc;
        let keys: Vec<&str> = path.split('.').collect();
        for (i, k) in keys.iter().enumerate() {
            if !at.is_object() {
                *at = Value::Object(serde_json::Map::new());
            }
            let obj = at.as_object_mut().expect("made an object");
            if i + 1 == keys.len() {
                obj.insert((*k).to_string(), value.clone());
                break;
            }
            at = obj.entry((*k).to_string()).or_insert(Value::Null);
        }
    }
    Scenario::from_document(doc)
}

/// What a sensitivity run measured.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Measured {
    /// The named scalar metrics.
    pub values: BTreeMap<String, f64>,
    /// The packet reception ratio census, per 20 m bin: (receivers in range, decoded).
    pub prr_bins: Vec<(u64, u64)>,
}

/// The fewest receivers a PRR bin needs before [`Measured::get`]'s range metrics use it.
pub const PRR_MIN_CENSUS: u64 = 20;

impl Measured {
    /// The metric `name`.
    ///
    /// The fixed names are the keys of [`Measured::values`]: `frames_tx`, `attempts`,
    /// `rx_ok`, `pdr` (decoded / candidate attempts), `prr` (census, 0-1000 m), `mac_drops`,
    /// `access_ms` (channel-access delay, `t - t_handoff` of `node.tx`), `e2e_ms_mean`,
    /// `e2e_ms_p95`, `ipg_ms_mean`, `ipg_ms_p95`, `aoi_ms`, `cbr_mean`, `cbr_max`,
    /// `queue_depth_mean`, `tx_rate_hz` (frames per transmitting node per second),
    /// `airtime_us` and `psdu_bytes` (per frame), `aifs_us` and `backoff_us` (per frame),
    /// `tx_phase_peak` (the largest share of frames on air in one 5 ms slot of the 100 ms
    /// cycle: 0.05 when transmissions are spread evenly),
    /// `stage_<name>_ms` (the mean of each latency stage of a delivered message: queue,
    /// sign, handoff, access, air, rx, verify_wait, verify, deliver), `collision_rate` (collision,
    /// hidden-terminal, resource-collision and in-band-emission losses per attempt),
    /// `loss_<cause>` (per attempt), `sl_*` (sidelink report), `gn_*` (GeoNetworking).
    ///
    /// Computed names: `prr_<lo>_<hi>` (census PRR between `lo` and `hi` metres, on the
    /// 20 m grid), `range_m` (the lower edge of the first 20 m bin, with at least
    /// [`PRR_MIN_CENSUS`] receivers, whose PRR is below 0.5 and stays below it in the next
    /// such bin) and `range90_m` (below 0.9).
    #[must_use]
    pub fn get(&self, name: &str) -> Option<f64> {
        if let Some(v) = self.values.get(name) {
            return Some(*v);
        }
        if let Some(rest) = name.strip_prefix("prr_") {
            let (lo, hi) = rest.split_once('_')?;
            let lo: f64 = lo.parse().ok()?;
            let hi: f64 = hi.parse().ok()?;
            let (mut n, mut ok) = (0u64, 0u64);
            for (i, (c, d)) in self.prr_bins.iter().enumerate() {
                let edge = 20.0 * i as f64;
                if edge >= lo && edge < hi {
                    n += c;
                    ok += d;
                }
            }
            return (n > 0).then(|| ok as f64 / n as f64);
        }
        let threshold = match name {
            "range_m" => 0.5,
            "range90_m" => 0.9,
            _ => return None,
        };
        // The bins with a census, as (index, below the threshold). The edge is the first
        // bin below the threshold whose next counted bin is below it too: one bin's dip
        // (a handful of near-field half-duplex or in-band-emission losses in the 0-20 m bin
        // of an NR run) is not where the link budget runs out.
        let counted: Vec<(usize, bool)> = self
            .prr_bins
            .iter()
            .enumerate()
            .filter(|(_, (c, _))| *c >= PRR_MIN_CENSUS)
            .map(|(i, (c, d))| (i, (*d as f64) < threshold * *c as f64))
            .collect();
        for (k, (i, below)) in counted.iter().enumerate() {
            if *below && counted.get(k + 1).is_none_or(|(_, next)| *next) {
                return Some(20.0 * *i as f64);
            }
        }
        Some(counted.last().map_or(0.0, |(i, _)| 20.0 * (*i + 1) as f64))
    }
}

/// Per (receiver, transmitter) pair: the freshest delivery so far and the age integral.
#[derive(Debug, Default, Clone, Copy)]
struct PairAge {
    last_gen: u64,
    last_del: u64,
    area_ns2: f64,
    span_ns: f64,
}

/// A recorder that measures [`Measured`] on the fly and keeps no record.
#[derive(Debug, Default)]
pub struct SensitivityRecorder {
    prr: Vec<(u64, u64)>,
    tx_frames: u64,
    tx_nodes: BTreeMap<u32, u64>,
    access_ns: u128,
    access_n: u64,
    airtime_us: u64,
    airtime_n: u64,
    /// Sum of each frame's transmit power, dBm, and the frames it covers: the power a
    /// J2945/1 or ETSI power control leaves the radio at.
    power_dbm_sum: f64,
    power_n: u64,
    e2e_us: BTreeMap<u64, u64>,
    e2e_sum_us: u128,
    delivered: u64,
    lost: u64,
    ipg_ms: BTreeMap<u64, u64>,
    ipg_sum_ns: u128,
    pairs: BTreeMap<(u32, u32), PairAge>,
    cbr_sum: f64,
    cbr_n: u64,
    cbr_max: f64,
    depth_sum: u64,
    records: u64,
    aifs_ns: u128,
    backoff_ns: u128,
    phase_bins: [u64; 20],
    stage_ns: [u128; STAGES.len()],
    stage_n: [u64; STAGES.len()],
}

/// The latency stages of a delivered message, as `node.rx` stamps them: name, and the
/// indices of the two stamps (in [`stamps`]) it lies between.
const STAGES: [(&str, usize, usize); 9] = [
    ("stage_queue_ms", 0, 1),
    ("stage_sign_ms", 1, 2),
    ("stage_handoff_ms", 2, 3),
    ("stage_access_ms", 3, 4),
    ("stage_air_ms", 4, 5),
    ("stage_rx_ms", 5, 6),
    ("stage_verify_wait_ms", 6, 7),
    ("stage_verify_ms", 7, 8),
    ("stage_deliver_ms", 8, 9),
];

/// A delivered message's stamps in journey order.
fn stamps(v: &NodeRxView) -> [Option<SimTime>; 10] {
    [
        v.t_generated,
        v.t_sign_start,
        v.t_signed,
        v.t_handoff,
        v.t_tx_start,
        v.t_arrival,
        v.t_rx_done,
        v.t_verify_start,
        v.t_verify_done,
        v.t_delivered,
    ]
}

/// A [`SimTime`] in nanoseconds (it is one already; named for the reader).
const fn ns(t: SimTime) -> u64 {
    t
}

/// The `q`-quantile of a histogram (nearest rank).
fn quantile(h: &BTreeMap<u64, u64>, q: f64) -> Option<u64> {
    let n: u64 = h.values().sum();
    if n == 0 {
        return None;
    }
    let rank = ((q * n as f64).ceil() as u64).max(1);
    let mut seen = 0;
    for (k, c) in h {
        seen += c;
        if seen >= rank {
            return Some(*k);
        }
    }
    h.keys().next_back().copied()
}

impl RunRecorder for SensitivityRecorder {
    fn write(&mut self, _at: SimTime, r: &OwnedRecord) {
        self.records += 1;
        match r.channel {
            "phy.prr" => {
                if let Ok(v) = decode::<PhyPrrView>(r) {
                    for [bin, within, ok] in v.bins {
                        let i = bin as usize;
                        if self.prr.len() <= i {
                            self.prr.resize(i + 1, (0, 0));
                        }
                        self.prr[i].0 += u64::from(within);
                        self.prr[i].1 += u64::from(ok);
                    }
                }
            }
            "node.tx" => {
                if let Ok(v) = decode::<NodeTxView>(r) {
                    self.tx_frames += 1;
                    *self.tx_nodes.entry(v.node.index()).or_default() += 1;
                    if let Some(h) = v.t_handoff.or(v.t_signed) {
                        self.access_ns += u128::from(ns(v.t).saturating_sub(ns(h)));
                        self.access_n += 1;
                    }
                    self.phase_bins[((ns(v.t) % 100_000_000) / 5_000_000) as usize] += 1;
                    self.aifs_ns += u128::from(v.mac_aifs_ns.unwrap_or(0));
                    self.backoff_ns += u128::from(v.mac_backoff_ns.unwrap_or(0));
                    if let Some(a) = v.airtime_us {
                        self.airtime_us += a;
                        self.airtime_n += 1;
                    }
                    if let Some(p) = v.power_dbm {
                        self.power_dbm_sum += p;
                        self.power_n += 1;
                    }
                }
            }
            "node.rx" => {
                if let Ok(v) = decode::<NodeRxView>(r) {
                    match v.outcome {
                        RxFate::Delivered => {}
                        RxFate::Lost => {
                            self.lost += 1;
                            return;
                        }
                        RxFate::InFlight => return,
                    }
                    self.delivered += 1;
                    let st = stamps(&v);
                    for (i, (_, a, b)) in STAGES.iter().enumerate() {
                        if let (Some(a), Some(b)) = (st[*a], st[*b]) {
                            self.stage_ns[i] += u128::from(b.saturating_sub(a));
                            self.stage_n[i] += 1;
                        }
                    }
                    let (Some(g), Some(d)) = (v.t_generated, v.t_delivered) else {
                        return;
                    };
                    let (g, d) = (ns(g), ns(d));
                    let lat_us = d.saturating_sub(g) / 1_000;
                    *self.e2e_us.entry(lat_us).or_default() += 1;
                    self.e2e_sum_us += u128::from(lat_us);
                    let Some(tx) = v.tx else { return };
                    let p = self.pairs.entry((v.rx.index(), tx.index())).or_default();
                    if p.last_del > 0 && d >= p.last_del && g > p.last_gen {
                        let gap = (d - p.last_del) as f64;
                        let age0 = (p.last_del - p.last_gen) as f64;
                        p.area_ns2 += age0 * gap + gap * gap / 2.0;
                        p.span_ns += gap;
                        let gap_ns = d - p.last_del;
                        *self.ipg_ms.entry(gap_ns / 1_000_000).or_default() += 1;
                        self.ipg_sum_ns += u128::from(gap_ns);
                    }
                    if p.last_del == 0 || (d >= p.last_del && g > p.last_gen) {
                        p.last_gen = g;
                        p.last_del = d;
                    }
                }
            }
            "mac.cbr" => {
                if let Ok(v) = decode::<MacCbrView>(r) {
                    self.cbr_sum += v.cbr;
                    self.cbr_n += 1;
                    self.cbr_max = self.cbr_max.max(v.cbr);
                    self.depth_sum += v.queue_depth.unwrap_or(0);
                }
            }
            _ => {}
        }
    }

    fn records_written(&self) -> Option<u64> {
        Some(self.records)
    }
}

impl SensitivityRecorder {
    /// The measurements, joined with the run report, for a run of `duration_s`.
    #[must_use]
    pub fn finish(&self, report: &RunReport, duration_s: f64) -> Measured {
        let mut m = BTreeMap::new();
        let mut put = |k: &str, v: f64| {
            m.insert(k.to_string(), v);
        };
        let attempts = report.reception_attempts.max(1) as f64;
        put("frames_tx", report.frames_transmitted as f64);
        put("attempts", report.reception_attempts as f64);
        put("rx_ok", report.receptions_ok as f64);
        put("pdr", report.receptions_ok as f64 / attempts);
        put("mac_drops", report.mac_drops as f64);
        let mut collisions = 0;
        for (cause, n) in &report.rx_losses {
            put(&format!("loss_{cause}"), *n as f64 / attempts);
            if matches!(
                cause.as_str(),
                "collision" | "hidden-terminal" | "resource-collision" | "in-band-emission"
            ) {
                collisions += n;
            }
        }
        put("collision_rate", collisions as f64 / attempts);
        let (n, ok) = self
            .prr
            .iter()
            .fold((0, 0), |(a, b), (c, d)| (a + c, b + d));
        if n > 0 {
            put("prr", ok as f64 / n as f64);
        }
        if self.access_n > 0 {
            put(
                "access_ms",
                self.access_ns as f64 / self.access_n as f64 / 1e6,
            );
        }
        if self.tx_frames > 0 {
            put("aifs_us", self.aifs_ns as f64 / self.tx_frames as f64 / 1e3);
            put(
                "backoff_us",
                self.backoff_ns as f64 / self.tx_frames as f64 / 1e3,
            );
        }
        if self.tx_frames > 0 {
            let peak = self.phase_bins.iter().copied().max().unwrap_or(0);
            put("tx_phase_peak", peak as f64 / self.tx_frames as f64);
        }
        for (i, (name, _, _)) in STAGES.iter().enumerate() {
            if self.stage_n[i] > 0 {
                put(name, self.stage_ns[i] as f64 / self.stage_n[i] as f64 / 1e6);
            }
        }
        if self.airtime_n > 0 {
            put("airtime_us", self.airtime_us as f64 / self.airtime_n as f64);
        }
        if self.power_n > 0 {
            put(
                "tx_power_dbm_mean",
                self.power_dbm_sum / self.power_n as f64,
            );
        }
        if report.air_bytes.frames > 0 {
            put(
                "psdu_bytes",
                report.air_bytes.psdu as f64 / report.air_bytes.frames as f64,
            );
        }
        if !self.tx_nodes.is_empty() && duration_s > 0.0 {
            put(
                "tx_rate_hz",
                self.tx_frames as f64 / self.tx_nodes.len() as f64 / duration_s,
            );
        }
        let delivered: u64 = self.e2e_us.values().sum();
        if delivered > 0 {
            put(
                "e2e_ms_mean",
                self.e2e_sum_us as f64 / delivered as f64 / 1e3,
            );
            if let Some(p) = quantile(&self.e2e_us, 0.95) {
                put("e2e_ms_p95", p as f64 / 1e3);
            }
        }
        if self.delivered + self.lost > 0 {
            put(
                "delivery_ratio",
                self.delivered as f64 / (self.delivered + self.lost) as f64,
            );
        }
        let gaps: u64 = self.ipg_ms.values().sum();
        if gaps > 0 {
            put("ipg_ms_mean", self.ipg_sum_ns as f64 / gaps as f64 / 1e6);
            if let Some(p) = quantile(&self.ipg_ms, 0.95) {
                put("ipg_ms_p95", p as f64);
            }
        }
        let (area, span) = self
            .pairs
            .values()
            .fold((0.0, 0.0), |(a, s), p| (a + p.area_ns2, s + p.span_ns));
        if span > 0.0 {
            put("aoi_ms", area / span / 1e6);
        }
        if self.cbr_n > 0 {
            put("cbr_mean", self.cbr_sum / self.cbr_n as f64);
            put("cbr_max", self.cbr_max);
            put(
                "queue_depth_mean",
                self.depth_sum as f64 / self.cbr_n as f64,
            );
        }
        if let Some(sl) = &report.sidelink {
            put("sl_cbr", sl.mean_cbr_at_grant_pm as f64 / 1000.0);
            put("sl_grants", sl.grants as f64);
            put("sl_overlapping", sl.overlapping_transmissions as f64);
            put("sl_cc_dropped", sl.cc_dropped as f64);
            put("sl_retransmissions", sl.retransmissions as f64);
            let reselections: u64 = sl.selections.values().sum();
            put("sl_selections", reselections as f64);
        }
        if !report.geonet.is_empty() {
            let w = &report.geonet;
            let k = w.len() as f64;
            put(
                "gn_reach",
                w.iter().map(|g| g.reach_permille as f64).sum::<f64>() / k / 1000.0,
            );
            put(
                "gn_transmissions",
                w.iter().map(|g| g.transmissions as f64).sum::<f64>() / k,
            );
            put(
                "gn_forwarded",
                w.iter().map(|g| g.forwarded as f64).sum::<f64>() / k,
            );
            put(
                "gn_max_hops",
                w.iter().map(|g| f64::from(g.max_hops)).fold(0.0, f64::max),
            );
        }
        if let Some(g) = &report.gn_air {
            put("gn_forwards_sent", g.forwards_sent as f64);
            put("gn_forward_airtime_us", g.forward_airtime_us as f64);
        }
        put("gn_forwarded_bytes", report.air_bytes.gn_forwarded as f64);
        // Every numeric report field as `report.<path>` (the catalogue fingerprint's
        // names), so a spec row can name a layer's octets (`report.air_bytes.network`) or
        // one message type's air time (`report.air_by_type.cam.airtime_us`).
        if let Ok(v) = serde_json::to_value(report) {
            sweep::flatten("report", &v, &mut m);
        }
        Measured {
            values: m,
            prr_bins: self.prr.clone(),
        }
    }
}

/// Runs `s` and measures it. The scenario is validated first, and its metric list is set
/// to `cbr` when empty, because the engine reports `mac.cbr` only when a provider runs.
///
/// A spec row measures every name the catalogue's fingerprint does: the radio harness's
/// names above, `report.*`, the `metric.*` sample means and the ground-truth `traffic.*` /
/// `vru.*` state ([`sweep::FingerprintRecorder`]). A spec row naming `traffic.speed_mean_mps`
/// or `metric.verify_rate` used to measure nothing and fail as "not measured" whatever the
/// engine did (platform validation 2026-10-24, rows ST.traffic and ST.security).
///
/// # Errors
/// The scenario is refused, or the engine fails to build or run.
pub fn measure(mut s: Scenario) -> Result<Measured> {
    if s.metrics.is_empty() {
        s.metrics = vec!["cbr".to_string()];
    }
    s.validate()?;
    let duration = s.time.duration_s;
    let mut engine = Engine::build(s, "")?;
    let mut rec = sweep::FingerprintRecorder::default();
    let report = engine.run(&mut rec)?;
    let prr_bins = rec.prr_bins();
    let mut values = rec.finish(&report, duration);
    // The fingerprint leaves out the recording's bookkeeping; a spec row may still name it.
    if let Ok(v) = serde_json::to_value(&report) {
        let mut all = BTreeMap::new();
        sweep::flatten("report", &v, &mut all);
        for (k, x) in all {
            values.entry(k).or_insert(x);
        }
    }
    Ok(Measured { values, prr_bins })
}

/// One row's outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowResult {
    /// The row.
    pub row: Row,
    /// The base run's value, if it measured one.
    pub base: Option<f64>,
    /// The changed run's value, if it measured one.
    pub value: Option<f64>,
    /// Whether the row held.
    pub pass: bool,
    /// Why it failed, or empty.
    pub note: String,
}

impl RowResult {
    /// `value / base`, when both exist and the base is not zero.
    #[must_use]
    pub fn ratio(&self) -> Option<f64> {
        match (self.base, self.value) {
            (Some(b), Some(v)) if b != 0.0 => Some(v / b),
            _ => None,
        }
    }
}

/// Judges `base` against `value` for `row`.
#[must_use]
pub fn judge(row: &Row, base: Option<f64>, value: Option<f64>) -> RowResult {
    let mut r = RowResult {
        row: row.clone(),
        base,
        value,
        pass: false,
        note: String::new(),
    };
    let (Some(b), Some(v)) = (base, value) else {
        r.note = format!("`{}` was not measured", row.metric);
        return r;
    };
    let tol = row.min_change.unwrap_or(0.01);
    let scale = b.abs().max(1e-12);
    let rel = (v - b) / scale;
    let moved = match row.expect {
        Direction::Up => v > b && rel >= tol,
        Direction::Down => v < b && -rel >= tol,
        Direction::Same => rel.abs() <= tol,
        Direction::Changes => rel.abs() >= tol,
    };
    if !moved {
        r.note = format!(
            "expected {:?}, changed by {:+.1} %",
            row.expect,
            rel * 100.0
        );
        return r;
    }
    if let (Some([lo, hi]), Some(q)) = (row.band, r.ratio())
        && !(lo..=hi).contains(&q)
    {
        r.note = format!("ratio {q:.3} outside [{lo}, {hi}]");
        return r;
    }
    r.pass = true;
    r
}

/// A cache of base runs, keyed by the base fields' JSON, so rows sharing a base run it
/// once.
#[derive(Debug, Default)]
pub struct BaseCache {
    runs: BTreeMap<String, Measured>,
}

impl BaseCache {
    /// The base run of `row` over `base`, run on first use.
    ///
    /// # Errors
    /// As [`measure`].
    pub fn base(&mut self, base: &Scenario, row: &Row) -> Result<Measured> {
        let key = serde_json::to_string(&row.base_set).unwrap_or_default();
        if let Some(m) = self.runs.get(&key) {
            return Ok(m.clone());
        }
        let m = measure(apply(base, &row.base_set)?)?;
        self.runs.insert(key, m.clone());
        Ok(m)
    }
}

/// Runs one row against `base` (the base run through `cache`).
///
/// # Errors
/// Either scenario is refused or fails to run.
pub fn check_row(base: &Scenario, row: &Row, cache: &mut BaseCache) -> Result<RowResult> {
    let b = cache.base(base, row)?;
    let mut both = row.base_set.clone();
    both.extend(row.set.clone());
    let v = measure(apply(base, &both)?)?;
    Ok(judge(row, b.get(&row.metric), v.get(&row.metric)))
}

/// Runs every row of `spec`, calling `progress` after each. A row whose scenario is
/// refused or fails to run fails, with the error as its note.
///
/// # Errors
/// None at present; the `Result` keeps room for a spec-level refusal.
pub fn run_spec(
    spec: &Spec,
    base: &Scenario,
    mut progress: impl FnMut(&RowResult),
) -> Result<Vec<RowResult>> {
    let mut cache = BaseCache::default();
    let mut out = Vec::with_capacity(spec.rows.len());
    for row in &spec.rows {
        // A refused or failed run fails its row and the rest still run.
        let r = check_row(base, row, &mut cache).unwrap_or_else(|e| RowResult {
            row: row.clone(),
            base: None,
            value: None,
            pass: false,
            note: e.to_string(),
        });
        progress(&r);
        out.push(r);
    }
    Ok(out)
}

fn change_text(row: &Row) -> String {
    row.set
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn num(v: Option<f64>) -> String {
    match v {
        None => "-".to_string(),
        Some(x) if x != 0.0 && (x.abs() >= 1e5 || x.abs() < 1e-3) => format!("{x:.3e}"),
        Some(x) => format!("{x:.4}"),
    }
}

/// A compact plain-text table, one line per row.
#[must_use]
pub fn table(results: &[RowResult]) -> String {
    let mut s = format!(
        "{:<4} {:<34} {:<16} {:<5} {:>11} {:>11} {:>7}\n",
        "ok", "row", "metric", "exp", "base", "changed", "ratio"
    );
    for r in results {
        s.push_str(&format!(
            "{:<4} {:<34} {:<16} {:<5} {:>11} {:>11} {:>7} {}\n",
            if r.pass { "PASS" } else { "FAIL" },
            r.row.name.chars().take(34).collect::<String>(),
            r.row.metric,
            format!("{:?}", r.row.expect).to_lowercase(),
            num(r.base),
            num(r.value),
            r.ratio().map_or("-".into(), |q| format!("{q:.3}")),
            r.note
        ));
    }
    let failed = results.iter().filter(|r| !r.pass).count();
    s.push_str(&format!("{} rows, {failed} failed\n", results.len()));
    s
}

/// The same table in Markdown: parameter change, metric, expected, measured, pass.
#[must_use]
pub fn markdown(results: &[RowResult]) -> String {
    let mut s = String::from(
        "| Row | Change | Metric | Expected | Base | Changed | Ratio | Pass |\n|---|---|---|---|---|---|---|---|\n",
    );
    for r in results {
        let mut exp = format!("{:?}", r.row.expect).to_lowercase();
        if let Some([lo, hi]) = r.row.band {
            exp.push_str(&format!(", ratio in [{lo}, {hi}]"));
        }
        s.push_str(&format!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} | {} |\n",
            r.row.name,
            change_text(&r.row),
            r.row.metric,
            exp,
            num(r.base),
            num(r.value),
            r.ratio().map_or("-".into(), |q| format!("{q:.3}")),
            if r.pass { "yes" } else { "**no**" }
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(expect: Direction, band: Option<[f64; 2]>) -> Row {
        Row {
            name: "r".into(),
            set: BTreeMap::new(),
            base_set: BTreeMap::new(),
            metric: "m".into(),
            expect,
            band,
            min_change: None,
            source: None,
        }
    }

    #[test]
    fn the_judge_can_fail_each_way() {
        assert!(judge(&row(Direction::Up, None), Some(1.0), Some(2.0)).pass);
        assert!(!judge(&row(Direction::Up, None), Some(1.0), Some(0.5)).pass);
        assert!(!judge(&row(Direction::Up, None), Some(1.0), Some(1.001)).pass);
        assert!(judge(&row(Direction::Down, None), Some(1.0), Some(0.5)).pass);
        assert!(!judge(&row(Direction::Down, None), Some(1.0), Some(1.5)).pass);
        assert!(judge(&row(Direction::Same, None), Some(1.0), Some(1.005)).pass);
        assert!(!judge(&row(Direction::Same, None), Some(1.0), Some(1.5)).pass);
        assert!(!judge(&row(Direction::Up, Some([1.8, 2.2])), Some(1.0), Some(3.0)).pass);
        assert!(judge(&row(Direction::Up, Some([1.8, 2.2])), Some(1.0), Some(2.0)).pass);
        assert!(!judge(&row(Direction::Up, None), None, Some(2.0)).pass);
        assert!(judge(&row(Direction::Changes, None), Some(1.0), Some(0.5)).pass);
        assert!(!judge(&row(Direction::Changes, None), Some(1.0), Some(1.0)).pass);
    }

    #[test]
    fn the_range_metrics_read_the_census() {
        let m = Measured {
            values: BTreeMap::new(),
            prr_bins: vec![(100, 99), (100, 95), (100, 60), (100, 40), (5, 5)],
        };
        assert_eq!(m.get("range_m"), Some(60.0));
        assert_eq!(m.get("range90_m"), Some(40.0));
        assert_eq!(m.get("prr_0_40"), Some(194.0 / 200.0));
        // A dip in one bin is not the edge.
        let dip = Measured {
            values: BTreeMap::new(),
            prr_bins: vec![(100, 80), (100, 99), (100, 95), (100, 60), (100, 40)],
        };
        assert_eq!(dip.get("range90_m"), Some(60.0));
        assert_eq!(m.get("nope"), None);
    }
}
