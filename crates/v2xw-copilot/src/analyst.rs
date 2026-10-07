//! The run analyst: bottlenecks and inefficiencies in every layer, from a run's own
//! metrics, with the numbers as evidence. Deterministic, and needs no model.
//!
//! The analyst is a pure function, [`analyse`], from a [`RunEvidence`] (what
//! [`crate::gather`] read from the server: the run's metrics, whole-run and binned, their
//! breakdowns, the backend's entity view, the scenario) to an [`Analysis`]: a list of
//! [`Finding`]s, each saying which layer, how bad, what was seen, where (nodes, junction
//! regions, distance bins, times, entities), the numbers behind it, the threshold it was
//! judged against and where that threshold comes from, and links to the metric and the
//! dashboard chart that show it.
//!
//! # What it judges against
//!
//! Every threshold is a field of [`Thresholds`], with its default and its source in the
//! field's documentation and in [`Thresholds::citations`], which the report prints. Where a
//! standard gives the number, the number is the standard's. Where the literature gives a
//! convention rather than a requirement (a time-to-collision below 1.5 s counted as a
//! conflict), it says so. Where there is no source, the default is a stated engineering
//! choice and the report says that too; nothing here claims precision it does not have.
//!
//! # What it cannot see
//!
//! Only what the run measured. A layer whose metrics the scenario did not compute is
//! reported as *not measured*, never as healthy. A metric whose bins are below their
//! minimum sample count arrives as `null` and is treated as missing, not as zero.
//!
//! # Determinism
//!
//! No clock, no randomness, `BTreeMap` order everywhere, and plain arithmetic (no
//! transcendental). The same evidence gives byte-identical findings, which is what lets
//! the model's overview be checked against them (see [`crate::agent`]).

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The layer a finding is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    /// Vehicles: speed, delay, conflicts, headways.
    Traffic,
    /// The radio channel: load against the congestion-control target, collisions,
    /// half-duplex losses.
    Channel,
    /// Medium access and the node's queues: access delay, TX and verification queues,
    /// drops.
    Access,
    /// Delivery: reception against distance, awareness, age of information.
    Delivery,
    /// End-to-end latency and the stage that dominates it.
    Latency,
    /// Credentials on the vehicle: pool, top-ups, verification, revocation.
    Security,
    /// The credential backend's entities and their queues.
    Backend,
    /// Linkability and pseudonym change.
    Privacy,
    /// The run itself: did it finish, how fast did it go.
    Simulation,
}

impl Layer {
    /// Every layer, in report order.
    pub const ALL: [Layer; 9] = [
        Layer::Traffic,
        Layer::Channel,
        Layer::Access,
        Layer::Delivery,
        Layer::Latency,
        Layer::Security,
        Layer::Backend,
        Layer::Privacy,
        Layer::Simulation,
    ];

    /// The heading a report uses.
    #[must_use]
    pub fn title(self) -> &'static str {
        match self {
            Layer::Traffic => "Traffic",
            Layer::Channel => "Channel",
            Layer::Access => "Access and queues",
            Layer::Delivery => "Delivery",
            Layer::Latency => "Latency",
            Layer::Security => "Security",
            Layer::Backend => "Credential backend",
            Layer::Privacy => "Privacy",
            Layer::Simulation => "Simulation",
        }
    }
}

/// How much a finding matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Context: what was measured, or not.
    Info,
    /// Worth knowing; not a bottleneck.
    Notice,
    /// A bottleneck or inefficiency that shapes the results.
    Warning,
    /// A requirement is missed or a resource is exhausted.
    Critical,
}

/// One number behind a finding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    /// What the number is.
    pub label: String,
    /// The value.
    pub value: f64,
    /// Its unit.
    pub unit: String,
    /// The metric or record it was read from (`cbr`, `pdr[dist_bin=120-140]`,
    /// `backend:ra.queue`).
    pub source: String,
}

/// Where a finding is.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// Node ids, worst first.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<String>,
    /// Regions (junction areas, as the metric's `region` dimension names them).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<String>,
    /// Backend entities.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities: Vec<String>,
    /// Distance bins, metres.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub distance_bins: Vec<String>,
    /// The time window, simulated seconds `[from, to]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_s: Option<[f64; 2]>,
}

impl Location {
    fn is_empty(&self) -> bool {
        self.nodes.is_empty()
            && self.regions.is_empty()
            && self.entities.is_empty()
            && self.distance_bins.is_empty()
            && self.window_s.is_none()
    }
}

/// A link from a finding to what shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Link {
    /// The metric name.
    pub metric: String,
    /// The dashboard chart that draws it: the metric's base name, which is the id the
    /// metrics panel opens a chart by (`openMetricChart` in the Studio).
    pub chart: String,
    /// The JSON-RPC query that reproduces the evidence.
    pub query: Value,
}

/// One bottleneck, inefficiency or fact about a run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    /// A stable id, `layer.what`.
    pub id: String,
    /// The layer.
    pub layer: Layer,
    /// How much it matters.
    pub severity: Severity,
    /// One line.
    pub title: String,
    /// What was seen and what it means, in two or three sentences.
    pub detail: String,
    /// The numbers.
    pub evidence: Vec<Evidence>,
    /// Where.
    #[serde(default, skip_serializing_if = "Location::is_empty")]
    pub location: Location,
    /// The threshold it was judged against, and its source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// The metrics and charts behind it.
    pub links: Vec<Link>,
}

/// One group of a metric broken down by a dimension, pooled over the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Group {
    /// The dimension's value (`120-140`, `collision`, `7`).
    pub key: String,
    /// The pooled value; `None` when the group had too few samples.
    pub value: Option<f64>,
    /// The sample count.
    pub n: u64,
}

/// What the run said about one metric.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct MetricEvidence {
    /// Its unit.
    pub unit: String,
    /// Its dimensions.
    #[serde(default)]
    pub dims: Vec<String>,
    /// The whole-run value; `None` when it had too few samples.
    pub value: Option<f64>,
    /// The binned series, `(t_s, value)`.
    #[serde(default)]
    pub series: Vec<(f64, Option<f64>)>,
    /// Breakdowns, by dimension name.
    #[serde(default)]
    pub groups: BTreeMap<String, Vec<Group>>,
}

/// The facts about the scenario the analyst judges against.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScenarioFacts {
    /// The scenario's name.
    pub name: String,
    /// `radio.rat`: `dsrc-80211p`, `lte-v2x-pc5`, `nr-v2x-pc5`.
    pub rat: String,
    /// `radio.region` when set (`us`, `eu`, …).
    pub region: String,
    /// The duration, simulated seconds.
    pub duration_s: f64,
    /// The demand, vehicles per hour, when it is a rate.
    pub demand_veh_per_h: Option<f64>,
    /// The posted speed, m/s, when the world states one.
    pub speed_limit_mps: Option<f64>,
    /// Whether a credential system runs (`security.protocol` set).
    pub security_protocol: Option<String>,
    /// The pseudonym change period, seconds, when the strategy is time-based.
    pub pseudonym_period_s: Option<f64>,
    /// The metric list the scenario asked for.
    pub metrics: Vec<String>,
}

impl ScenarioFacts {
    /// The facts, read out of a scenario document.
    #[must_use]
    pub fn from_document(doc: &Value) -> Self {
        let s = |p: &str| doc.pointer(p).and_then(Value::as_str).unwrap_or("").to_string();
        let f = |p: &str| doc.pointer(p).and_then(Value::as_f64);
        ScenarioFacts {
            name: s("/meta/name"),
            rat: s("/radio/rat"),
            region: s("/radio/region"),
            duration_s: f("/time/duration_s").unwrap_or(0.0),
            demand_veh_per_h: f("/actors/vehicles/demand/rate_veh_per_h"),
            speed_limit_mps: f("/world/source/params/speed_limit_mps"),
            // `security.protocol` is a model reference: `{id: protocol/scms/camp, params}`,
            // or the bare id.
            security_protocol: doc
                .pointer("/security/protocol/id")
                .or_else(|| doc.pointer("/security/protocol"))
                .and_then(Value::as_str)
                .map(str::to_string),
            pseudonym_period_s: f("/security/pseudonym_change/period_s"),
            metrics: doc
                .get("metrics")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        }
    }
}

/// Everything the analyst reads. Built by [`crate::gather::gather`] from a live server, or
/// by hand in a test.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RunEvidence {
    /// The run id.
    pub run_id: String,
    /// The scenario's content hash.
    pub scenario_hash: String,
    /// The scenario.
    pub scenario: ScenarioFacts,
    /// How far the run got, simulated seconds.
    pub t_reached_s: f64,
    /// Where it was meant to end.
    pub t_end_s: f64,
    /// The metrics, by series name (`cbr`, `mac_access_delay.p95`, `pdr[100m]`).
    pub metrics: BTreeMap<String, MetricEvidence>,
    /// The credential backend's entity view (`inspect.entity {entity: "backend"}`).
    #[serde(default)]
    pub backend: Option<Value>,
    /// The engine's own counters (`run.status.engine`).
    #[serde(default)]
    pub engine: Option<Value>,
}

impl RunEvidence {
    fn value(&self, name: &str) -> Option<f64> {
        self.metrics.get(name).and_then(|m| m.value)
    }

    fn has(&self, name: &str) -> bool {
        self.metrics.contains_key(name)
    }

    fn unit(&self, name: &str) -> String {
        self.metrics.get(name).map(|m| m.unit.clone()).unwrap_or_default()
    }

    fn groups(&self, name: &str, dim: &str) -> &[Group] {
        self.metrics
            .get(name)
            .and_then(|m| m.groups.get(dim))
            .map_or(&[], Vec::as_slice)
    }

    /// The bin of the series with the largest value, and that value.
    fn peak(&self, name: &str) -> Option<(f64, f64)> {
        let m = self.metrics.get(name)?;
        let mut best: Option<(f64, f64)> = None;
        for (t, v) in &m.series {
            if let Some(v) = v {
                if best.is_none_or(|(_, b)| *v > b) {
                    best = Some((*t, *v));
                }
            }
        }
        best
    }

    /// The series' bin width, seconds, from its first two points.
    fn bin_s(&self, name: &str) -> f64 {
        self.metrics
            .get(name)
            .and_then(|m| (m.series.len() >= 2).then(|| m.series[1].0 - m.series[0].0))
            .filter(|b| *b > 0.0)
            .unwrap_or(1.0)
    }
}

/// The thresholds a run is judged against. Each default, with its source.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thresholds {
    /// The end-to-end latency budget for basic safety messages, 100 ms: 3GPP TS 22.185
    /// §5.1 ("the maximum latency shall be 100 ms") and the 100 ms of SAE J2945/1.
    pub latency_budget_ms: f64,
    /// The channel busy ratio the ETSI adaptive DCC steers to, 0.68 (ETSI TS 102 687
    /// V1.2.1 §5.4, `CBR_target`; the radio crate's `AdaptiveParams::cbr_target`).
    pub cbr_target_etsi: f64,
    /// The channel busy percentage at which SAE J2945/1's power control reaches its
    /// floor, 0.80 (`vMaxCBP`, the radio crate's J2945/1 parameters).
    pub cbr_max_j2945: f64,
    /// The CBR above which the LTE-V2X / NR-V2X sidelink's tightest channel-occupancy
    /// limits apply, 0.65: the upper boundary of the third CBR range in ETSI TS 103 574's
    /// congestion-control table. A parameter, because operators configure the ranges.
    pub cbr_congested_sidelink: f64,
    /// The packet reception ratio at which a communication range is read off the PRR
    /// curve, 0.9: the reliability 3GPP TR 37.885's evaluations report range at.
    pub prr_range_level: f64,
    /// Near range, metres: within it a healthy link decodes almost everything, so a PRR
    /// below [`Thresholds::near_range_prr`] there is interference, not distance. 50 m, an
    /// engineering choice (no standard defines it), stated as such.
    pub near_range_m: f64,
    /// The PRR expected within [`Thresholds::near_range_m`], 0.95. An engineering choice.
    pub near_range_prr: f64,
    /// The share of losses one cause must reach to be named the dominant loss cause, 0.3.
    /// An engineering choice.
    pub loss_cause_share: f64,
    /// Access delay counts as a bottleneck above this fraction of the latency budget,
    /// 0.2 (20 ms of 100 ms). An engineering choice.
    pub access_budget_fraction: f64,
    /// A queue whose mean depth exceeds this is saturating, 1.0 frame: a queue that is
    /// rarely empty. An engineering choice.
    pub queue_depth_saturated: f64,
    /// A share of received messages left unverified above which verification is a
    /// bottleneck, 0.05. An engineering choice.
    pub unverified_ratio: f64,
    /// Age of information counts as stale above this many generation intervals, 3
    /// (300 ms for a 10 Hz BSM). An engineering choice; the literature reports AoI
    /// relative to the generation interval.
    pub aoi_intervals: f64,
    /// The BSM / CAM generation interval used for that, 100 ms (SAE J2945/1's 10 Hz).
    pub generation_interval_ms: f64,
    /// The neighbour awareness ratio below which awareness is a problem, 0.9. An
    /// engineering choice matching the PRR range level.
    pub awareness_floor: f64,
    /// A time-to-collision below this is a conflict, 1.5 s: the common threshold in
    /// surrogate-safety studies (Hayward 1972; FHWA SSAM uses 1.5 s as its default).
    pub ttc_conflict_s: f64,
    /// A deceleration rate to avoid a crash above this is critical, 3.35 m/s² (Archer
    /// 2005, the threshold most DRAC studies use).
    pub drac_critical: f64,
    /// Mean speed below this fraction of the posted speed means heavy delay, 0.5
    /// (travel-time loss of a half). An engineering choice; HCM level of service F for
    /// urban streets begins at a travel speed of 30 % of the base free-flow speed.
    pub speed_ratio_congested: f64,
    /// A backend entity whose server utilisation exceeds this is saturated, 0.8 (queueing
    /// delay grows without bound as utilisation approaches 1; 0.8 is the usual planning
    /// ceiling). An engineering choice.
    pub entity_utilisation: f64,
    /// Linkability above which pseudonym change is not protecting the vehicles, 0.5. An
    /// engineering choice.
    pub linkability: f64,
    /// Simulated seconds per wall second below which the run is called slow, 1.0
    /// (slower than real time).
    pub realtime: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds {
            latency_budget_ms: 100.0,
            cbr_target_etsi: 0.68,
            cbr_max_j2945: 0.80,
            cbr_congested_sidelink: 0.65,
            prr_range_level: 0.9,
            near_range_m: 50.0,
            near_range_prr: 0.95,
            loss_cause_share: 0.3,
            access_budget_fraction: 0.2,
            queue_depth_saturated: 1.0,
            unverified_ratio: 0.05,
            aoi_intervals: 3.0,
            generation_interval_ms: 100.0,
            awareness_floor: 0.9,
            ttc_conflict_s: 1.5,
            drac_critical: 3.35,
            speed_ratio_congested: 0.5,
            entity_utilisation: 0.8,
            linkability: 0.5,
            realtime: 1.0,
        }
    }
}

impl Thresholds {
    /// The congestion target for a radio technology and region, with its source.
    #[must_use]
    pub fn cbr_target(&self, rat: &str, region: &str) -> (f64, &'static str) {
        if rat.contains("lte") || rat.contains("nr") {
            (
                self.cbr_congested_sidelink,
                "ETSI TS 103 574 CBR range boundary 0.65 (sidelink congestion control)",
            )
        } else if region.eq_ignore_ascii_case("eu") || region.eq_ignore_ascii_case("etsi") {
            (self.cbr_target_etsi, "ETSI TS 102 687 adaptive DCC target CBR 0.68")
        } else {
            (
                self.cbr_max_j2945,
                "SAE J2945/1 vMaxCBP 80 % (power control at its floor)",
            )
        }
    }

    /// Every threshold with its source, for the report's appendix.
    #[must_use]
    pub fn citations(&self) -> Vec<(String, String)> {
        vec![
            (format!("latency budget {} ms", fmt(self.latency_budget_ms)),
             "3GPP TS 22.185 §5.1; SAE J2945/1".into()),
            (format!("CBR target {} (ETSI)", fmt(self.cbr_target_etsi)),
             "ETSI TS 102 687 V1.2.1 §5.4".into()),
            (format!("CBP ceiling {} (J2945/1)", fmt(self.cbr_max_j2945)),
             "SAE J2945/1 vMaxCBP".into()),
            (format!("sidelink CBR boundary {}", fmt(self.cbr_congested_sidelink)),
             "ETSI TS 103 574 (operator-configured; parameter)".into()),
            (format!("range read at PRR {}", fmt(self.prr_range_level)),
             "3GPP TR 37.885 evaluation methodology".into()),
            (format!("near-range PRR {} within {} m", fmt(self.near_range_prr), fmt(self.near_range_m)),
             "engineering choice (no standard)".into()),
            (format!("TTC conflict below {} s", fmt(self.ttc_conflict_s)),
             "Hayward 1972; FHWA SSAM default".into()),
            (format!("DRAC critical above {} m/s²", fmt(self.drac_critical)),
             "Archer 2005".into()),
            (format!("mean speed below {} of the posted speed", fmt(self.speed_ratio_congested)),
             "engineering choice; HCM urban-street LOS F starts at 0.3".into()),
            (format!("entity utilisation above {}", fmt(self.entity_utilisation)),
             "engineering choice (queueing planning ceiling)".into()),
            (format!("AoI above {} × {} ms", fmt(self.aoi_intervals), fmt(self.generation_interval_ms)),
             "engineering choice; 10 Hz from SAE J2945/1".into()),
        ]
    }
}

/// The analyst's answer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Analysis {
    /// The run.
    pub run_id: String,
    /// The scenario's content hash.
    pub scenario_hash: String,
    /// The findings, most severe first, then by layer, then by id.
    pub findings: Vec<Finding>,
    /// The headline numbers, by metric, for a comparison table.
    pub headline: BTreeMap<String, f64>,
    /// Layers the run did not measure.
    pub not_measured: Vec<Layer>,
    /// The thresholds used, with their sources.
    pub thresholds: Vec<(String, String)>,
}

impl Analysis {
    /// The findings in one layer.
    #[must_use]
    pub fn in_layer(&self, layer: Layer) -> Vec<&Finding> {
        self.findings.iter().filter(|f| f.layer == layer).collect()
    }

    /// A finding by id.
    #[must_use]
    pub fn finding(&self, id: &str) -> Option<&Finding> {
        self.findings.iter().find(|f| f.id == id)
    }

    /// A compact form for a model's context: ids, titles, severities and evidence, with
    /// the details cut, so a long run does not fill the conversation.
    #[must_use]
    pub fn compact(&self) -> Value {
        json!({
            "run_id": self.run_id,
            "scenario_hash": self.scenario_hash,
            "headline": self.headline,
            "not_measured": self.not_measured,
            "findings": self.findings.iter().map(|f| json!({
                "id": f.id,
                "layer": f.layer,
                "severity": f.severity,
                "title": f.title,
                "evidence": f.evidence.iter().map(|e| json!({
                    "label": e.label, "value": fmt(e.value), "unit": e.unit, "source": e.source
                })).collect::<Vec<_>>(),
                "where": f.location,
                "reference": f.reference,
            })).collect::<Vec<_>>(),
        })
    }
}

/// The metrics whose whole-run value the analyst wants, and [`crate::gather`] fetches.
pub const HEADLINE_METRICS: [&str; 52] = [
    "pdr",
    "pdr[100m]",
    "pdr[300m]",
    "per",
    "delivery_ratio",
    "cbr",
    "cbr.p95",
    "channel_load",
    "collision_rate",
    "half_duplex_rate",
    "mac_access_delay",
    "mac_access_delay.p95",
    "mac_queue_depth",
    "mac_queue_depth.p95",
    "mac_drops",
    "verify_queue_depth",
    "verify_queue_depth.p95",
    "verify_wait",
    "verify_wait.p95",
    "unverified_ratio",
    "verify_rate",
    "latency",
    "latency.p95",
    "e2e_latency",
    "e2e_latency.p95",
    "aoi",
    "aoi.p95",
    "aoi_peak",
    "nar",
    "awareness",
    "mean_speed",
    "speed",
    "speed.p50",
    "density",
    "flow",
    "ttc_min",
    "ttc_conflicts",
    "pet",
    "drac",
    "drac.p95",
    "headway_time",
    "headway_time.p50",
    "cert_pool_valid",
    "false_accusations",
    "linkability",
    "pseudonym_change_rate",
    "wall_clock_per_sim_second",
    "memory_high_water_mark",
    "goodput",
    "offered_load",
    "security_overhead",
    "time_to_detect",
];

/// The breakdowns the analyst wants: `(metric, dimension)`.
pub const BREAKDOWNS: [(&str, &str); 10] = [
    ("pdr", "dist_bin"),
    ("pdr_by_cause", "cause"),
    ("latency_stage", "stage"),
    ("latency_stage_share", "stage"),
    ("revocation_latency_stage", "stage"),
    ("mac_access_delay", "node"),
    ("mac_queue_depth", "node"),
    ("verify_queue_depth", "node"),
    ("mean_speed", "region"),
    ("density", "region"),
];

/// The series the analyst wants binned, to say *when*.
pub const SERIES_METRICS: [&str; 6] = [
    "cbr",
    "pdr",
    "mac_queue_depth",
    "e2e_latency.p95",
    "mean_speed",
    "verify_queue_depth",
];

/// Formats a number the one way the analyst, its report and the overview check all use:
/// at most three decimals, trailing zeros dropped.
#[must_use]
pub fn fmt(v: f64) -> String {
    if !v.is_finite() {
        return "n/a".to_string();
    }
    let a = v.abs();
    let s = if a >= 100.0 {
        format!("{v:.0}")
    } else if a >= 10.0 {
        format!("{v:.1}")
    } else if a >= 0.01 || a == 0.0 {
        format!("{v:.3}")
    } else {
        format!("{v:.5}")
    };
    if s.contains('.') {
        let t = s.trim_end_matches('0').trim_end_matches('.');
        if t == "-0" { "0".to_string() } else { t.to_string() }
    } else {
        s
    }
}

fn base(metric: &str) -> String {
    metric
        .split(['.', '['])
        .next()
        .unwrap_or(metric)
        .to_string()
}

fn link(metric: &str) -> Link {
    Link {
        metric: metric.to_string(),
        chart: base(metric),
        query: json!({"method": "metrics.query", "params": {"metrics": [metric]}}),
    }
}

fn grouped_link(metric: &str, dim: &str) -> Link {
    Link {
        metric: metric.to_string(),
        chart: base(metric),
        query: json!({"method": "metrics.query",
                      "params": {"metrics": [metric], "group_by": [dim]}}),
    }
}

fn ev(label: &str, value: f64, unit: &str, source: &str) -> Evidence {
    Evidence {
        label: label.to_string(),
        value,
        unit: unit.to_string(),
        source: source.to_string(),
    }
}

/// The worst `k` groups of a breakdown by value (largest first, or smallest first).
fn worst(groups: &[Group], k: usize, largest: bool) -> Vec<&Group> {
    let mut v: Vec<&Group> = groups.iter().filter(|g| g.value.is_some()).collect();
    v.sort_by(|a, b| {
        let (x, y) = (a.value.unwrap_or(0.0), b.value.unwrap_or(0.0));
        let o = if largest { y.total_cmp(&x) } else { x.total_cmp(&y) };
        o.then_with(|| a.key.cmp(&b.key))
    });
    v.truncate(k);
    v
}

/// The window around a series' peak, one bin wide.
fn peak_window(e: &RunEvidence, metric: &str) -> Option<[f64; 2]> {
    let (t, _) = e.peak(metric)?;
    Some([t, t + e.bin_s(metric)])
}

/// The lower bound of a distance-bin key (`120-140` → 120, `1000+` → 1000).
fn bin_lo(key: &str) -> Option<f64> {
    key.split(['-', '+']).next().and_then(|s| s.trim().parse::<f64>().ok())
}

/// Analyses a run. Pure and deterministic.
#[must_use]
pub fn analyse(e: &RunEvidence, th: &Thresholds) -> Analysis {
    let mut findings = Vec::new();
    simulation(e, th, &mut findings);
    traffic(e, th, &mut findings);
    channel(e, th, &mut findings);
    access(e, th, &mut findings);
    delivery(e, th, &mut findings);
    latency(e, th, &mut findings);
    security(e, th, &mut findings);
    backend(e, th, &mut findings);
    privacy(e, th, &mut findings);

    let not_measured = not_measured(e);
    for layer in &not_measured {
        findings.push(Finding {
            id: format!("{}.not-measured", serde_json::to_value(layer).ok()
                .and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()),
            layer: *layer,
            severity: Severity::Info,
            title: format!("{} was not measured in this run", layer.title()),
            detail: not_measured_reason(e, *layer),
            evidence: Vec::new(),
            location: Location::default(),
            reference: None,
            links: Vec::new(),
        });
    }

    findings.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.layer.cmp(&b.layer))
            .then_with(|| a.id.cmp(&b.id))
    });
    let headline = e
        .metrics
        .iter()
        .filter(|(k, _)| HEADLINE_METRICS.contains(&k.as_str()))
        .filter_map(|(k, m)| m.value.map(|v| (k.clone(), v)))
        .collect();
    Analysis {
        run_id: e.run_id.clone(),
        scenario_hash: e.scenario_hash.clone(),
        findings,
        headline,
        not_measured,
        thresholds: th.citations(),
    }
}

fn not_measured(e: &RunEvidence) -> Vec<Layer> {
    let any = |names: &[&str]| names.iter().any(|n| e.has(n));
    let mut out = Vec::new();
    if !any(&["mean_speed", "speed", "ttc_min", "ttc_conflicts", "density", "flow"]) {
        out.push(Layer::Traffic);
    }
    if !any(&["cbr", "channel_load", "collision_rate"]) {
        out.push(Layer::Channel);
    }
    if !any(&["mac_access_delay", "mac_queue_depth", "verify_queue_depth"]) {
        out.push(Layer::Access);
    }
    if !any(&["pdr", "delivery_ratio", "nar", "aoi"]) {
        out.push(Layer::Delivery);
    }
    if !any(&["latency", "e2e_latency", "latency_stage"]) {
        out.push(Layer::Latency);
    }
    if e.scenario.security_protocol.is_none()
        && !any(&["cert_pool_valid", "false_accusations", "revocation_latency_stage"])
    {
        out.push(Layer::Security);
        out.push(Layer::Backend);
    } else if e.backend.is_none() {
        out.push(Layer::Backend);
    }
    if !any(&["linkability", "pseudonym_change_rate"]) {
        out.push(Layer::Privacy);
    }
    out
}

fn not_measured_reason(e: &RunEvidence, layer: Layer) -> String {
    let asked = if e.scenario.metrics.is_empty() {
        "no metric list".to_string()
    } else {
        format!("metrics: [{}]", e.scenario.metrics.join(", "))
    };
    match layer {
        Layer::Security | Layer::Backend if e.scenario.security_protocol.is_none() => {
            "The scenario sets no security.protocol, so no credential system ran: there is \
             no pool, top-up, revocation or backend entity to judge. Set security.protocol \
             (for example scms-us or ccms-etsi) to run one."
                .to_string()
        }
        Layer::Backend => "The backend's entity view could not be read for this run.".to_string(),
        _ => format!(
            "The run computed none of this layer's metrics (the scenario asks for {asked}). \
             Not measured is not healthy: set `metrics: [all]` to measure it."
        ),
    }
}

fn simulation(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    if e.t_end_s > 0.0 && e.t_reached_s + 0.5 < e.t_end_s {
        out.push(Finding {
            id: "simulation.incomplete".into(),
            layer: Layer::Simulation,
            severity: Severity::Warning,
            title: "The run did not reach its end".into(),
            detail: "Every figure below covers only the part of the run that was simulated; \
                     a bottleneck that builds up later is not in it."
                .into(),
            evidence: vec![
                ev("simulated", e.t_reached_s, "s", "run.status.t_ns"),
                ev("planned", e.t_end_s, "s", "run.status.t_end_ns"),
            ],
            location: Location::default(),
            reference: None,
            links: Vec::new(),
        });
    }
    if let Some(w) = e.value("wall_clock_per_sim_second") {
        if w > 0.0 && 1.0 / w < th.realtime {
            out.push(Finding {
                id: "simulation.slower-than-real-time".into(),
                layer: Layer::Simulation,
                severity: Severity::Notice,
                title: "The engine ran slower than real time".into(),
                detail: "Each simulated second took more than a wall second. This is a \
                         property of the machine and the scenario's fidelity, not of the \
                         system simulated; it is excluded from every digest. Lower tiers \
                         or a smaller area run faster."
                    .into(),
                evidence: vec![ev("wall seconds per simulated second", w, "s/s",
                                  "wall_clock_per_sim_second")],
                location: Location::default(),
                reference: Some(format!("real time = {}", fmt(th.realtime))),
                links: vec![link("wall_clock_per_sim_second")],
            });
        }
    }
}

fn traffic(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    if let (Some(v), Some(limit)) = (e.value("mean_speed"), e.scenario.speed_limit_mps) {
        if limit > 0.0 {
            let ratio = v / limit;
            if ratio < th.speed_ratio_congested {
                let regions = worst(e.groups("mean_speed", "region"), 3, false);
                out.push(Finding {
                    id: "traffic.delay".into(),
                    layer: Layer::Traffic,
                    severity: if ratio < 0.3 { Severity::Critical } else { Severity::Warning },
                    title: "Vehicles moved at a fraction of the posted speed".into(),
                    detail: format!(
                        "Mean speed was {} of the posted speed, a travel-time loss of {}: \
                         time spent queued at junctions and in slow traffic. {}",
                        fmt(ratio),
                        fmt(1.0 - ratio),
                        if regions.is_empty() {
                            "The run has no per-region breakdown to say where.".to_string()
                        } else {
                            format!("Slowest regions: {}.", regions.iter()
                                .map(|g| format!("{} ({} m/s)", g.key, fmt(g.value.unwrap_or(0.0))))
                                .collect::<Vec<_>>().join(", "))
                        }
                    ),
                    evidence: vec![
                        ev("mean speed", v, "m/s", "mean_speed"),
                        ev("posted speed", limit, "m/s", "scenario world speed_limit_mps"),
                        ev("speed ratio", ratio, "ratio", "mean_speed / speed limit"),
                    ],
                    location: Location {
                        regions: regions.iter().map(|g| g.key.clone()).collect(),
                        window_s: None,
                        ..Location::default()
                    },
                    reference: Some(format!(
                        "below {} of the posted speed (HCM urban-street LOS F begins at 0.3)",
                        fmt(th.speed_ratio_congested)
                    )),
                    links: vec![link("mean_speed"), grouped_link("mean_speed", "region")],
                });
            }
        }
    }
    let conflicts = e.value("ttc_conflicts").unwrap_or(0.0);
    let ttc_min = e.value("ttc_min");
    if conflicts > 0.0 || ttc_min.is_some_and(|t| t < th.ttc_conflict_s) {
        let mut evidence = vec![ev("conflicts (TTC below threshold)", conflicts, "count",
                                   "ttc_conflicts")];
        if let Some(t) = ttc_min {
            evidence.push(ev("minimum time to collision", t, "s", "ttc_min"));
        }
        if let Some(d) = e.value("drac.p95").or_else(|| e.value("drac")) {
            evidence.push(ev("deceleration to avoid a crash (p95)", d, "m/s²", "drac"));
        }
        let severe = e.value("drac.p95").is_some_and(|d| d > th.drac_critical)
            || ttc_min.is_some_and(|t| t < 0.5 * th.ttc_conflict_s);
        out.push(Finding {
            id: "traffic.conflicts".into(),
            layer: Layer::Traffic,
            severity: if severe { Severity::Warning } else { Severity::Notice },
            title: "Traffic conflicts occurred".into(),
            detail: "Pairs of vehicles came within the time-to-collision threshold of each \
                     other. These are the events V2X safety applications exist to warn about; \
                     their count is the denominator a warning-application study needs."
                .into(),
            evidence,
            location: Location { window_s: peak_window(e, "ttc_conflicts"), ..Location::default() },
            reference: Some(format!(
                "TTC below {} s (Hayward 1972; FHWA SSAM default); DRAC above {} m/s² is \
                 critical (Archer 2005)",
                fmt(th.ttc_conflict_s),
                fmt(th.drac_critical)
            )),
            links: vec![link("ttc_conflicts"), link("ttc_min"), link("drac")],
        });
    }
}

fn channel(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    let (target, source) = th.cbr_target(&e.scenario.rat, &e.scenario.region);
    if let Some(cbr) = e.value("cbr") {
        let peak = e.peak("cbr");
        let p95 = e.value("cbr.p95");
        let over_mean = cbr > target;
        let over_peak = peak.is_some_and(|(_, v)| v > target) || p95.is_some_and(|v| v > target);
        if over_mean || over_peak {
            let mut evidence = vec![ev("mean channel busy ratio", cbr, "ratio", "cbr")];
            if let Some(p) = p95 {
                evidence.push(ev("p95 channel busy ratio", p, "ratio", "cbr.p95"));
            }
            if let Some((_, v)) = peak {
                evidence.push(ev("busiest bin", v, "ratio", "cbr (binned)"));
            }
            evidence.push(ev("congestion-control target", target, "ratio", source));
            out.push(Finding {
                id: "channel.congested".into(),
                layer: Layer::Channel,
                severity: if over_mean { Severity::Critical } else { Severity::Warning },
                title: if over_mean {
                    "The channel ran above its congestion-control target".into()
                } else {
                    "The channel exceeded its congestion-control target at its busiest".into()
                },
                detail: format!(
                    "Above the target the congestion control cuts message rate or power \
                     ({}), so awareness range and update rate fall for every vehicle in the \
                     area; collisions rise with load regardless.",
                    if e.scenario.rat.contains("lte") || e.scenario.rat.contains("nr") {
                        "SAE J3161/1 rate control on the sidelink"
                    } else {
                        "SAE J2945/1 or ETSI DCC on 802.11p"
                    }
                ),
                evidence,
                location: Location { window_s: peak_window(e, "cbr"), ..Location::default() },
                reference: Some(source.to_string()),
                links: vec![link("cbr")],
            });
        }
    }
    // Loss causes: which one dominates.
    let causes = e.groups("pdr_by_cause", "cause");
    if let Some(top) = worst(causes, 1, true).first() {
        let share = top.value.unwrap_or(0.0);
        if share >= th.loss_cause_share {
            let k = top.key.as_str();
            let (id, meaning) = if k.contains("collision") || k.contains("hidden")
                || k.contains("in-band") || k.contains("adjacent")
            {
                (
                    "channel.collisions",
                    "Frames were lost to overlapping transmissions (collisions, hidden \
                     terminals, resource collisions on the sidelink, in-band or adjacent-\
                     channel leakage): the channel is shared too densely for the access \
                     scheme to keep transmitters apart.",
                )
            } else if k.contains("half") {
                (
                    "channel.half-duplex",
                    "Frames were lost because the receiver was transmitting itself: a radio \
                     cannot hear while it sends. Sidelink resource selection that puts \
                     neighbours in the same subframe makes this worse.",
                )
            } else if k.contains("jam") {
                (
                    "channel.jamming",
                    "Frames were lost to a jammer: a threat in the scenario, not load.",
                )
            } else if k.contains("sensitivity") || k.contains("range") || k.contains("fading")
                || k.contains("preamble")
            {
                (
                    "channel.weak-signal",
                    "Frames were lost because the signal was too weak at the receiver: \
                     distance, fading and buildings, not load.",
                )
            } else {
                ("channel.dominant-loss", "One loss cause accounts for most lost frames.")
            };
            out.push(Finding {
                id: id.into(),
                layer: Layer::Channel,
                severity: if share >= 0.5 { Severity::Warning } else { Severity::Notice },
                title: format!("Most losses were `{}`", top.key),
                detail: meaning.to_string(),
                evidence: causes
                    .iter()
                    .filter_map(|g| g.value.map(|v| ev(&format!("share of losses: {}", g.key),
                                                       v, "ratio",
                                                       &format!("pdr_by_cause[{}]", g.key))))
                    .collect(),
                location: Location::default(),
                reference: Some(format!(
                    "a cause with {} or more of losses is named dominant (engineering choice)",
                    fmt(th.loss_cause_share)
                )),
                links: vec![grouped_link("pdr_by_cause", "cause")],
            });
        }
    }
    if let Some(hd) = e.value("half_duplex_rate") {
        if hd > 0.05 && !out.iter().any(|f| f.id == "channel.half-duplex") {
            out.push(Finding {
                id: "channel.half-duplex-rate".into(),
                layer: Layer::Channel,
                severity: Severity::Notice,
                title: "A notable share of receptions was missed while transmitting".into(),
                detail: "Half-duplex misses are structural for any radio; on the sidelink \
                         they depend on how often neighbours pick the same subframe."
                    .into(),
                evidence: vec![ev("half-duplex rate", hd, "ratio", "half_duplex_rate")],
                location: Location::default(),
                reference: Some("above 0.05 (engineering choice)".into()),
                links: vec![link("half_duplex_rate")],
            });
        }
    }
}

fn access(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    let budget = th.latency_budget_ms * th.access_budget_fraction;
    if let Some(d) = e.value("mac_access_delay.p95").or_else(|| e.value("mac_access_delay")) {
        if d > budget {
            let nodes = worst(e.groups("mac_access_delay", "node"), 5, true);
            out.push(Finding {
                id: "access.delay".into(),
                layer: Layer::Access,
                severity: if d > th.latency_budget_ms * 0.5 { Severity::Critical } else { Severity::Warning },
                title: "Medium access took a large part of the latency budget".into(),
                detail: format!(
                    "Frames waited {} ms (p95) for the channel before going out. On 802.11p \
                     that is backoff behind a busy medium; on the sidelink, the resource \
                     reservation interval. {}",
                    fmt(d),
                    if nodes.is_empty() { String::new() } else {
                        format!("Worst nodes: {}.", nodes.iter()
                            .map(|g| format!("{} ({} ms)", g.key, fmt(g.value.unwrap_or(0.0))))
                            .collect::<Vec<_>>().join(", "))
                    }
                ),
                evidence: vec![
                    ev("access delay (p95)", d, "ms", "mac_access_delay.p95"),
                    ev("share of the budget allowed", budget, "ms", "thresholds"),
                ],
                location: Location { nodes: nodes.iter().map(|g| g.key.clone()).collect(),
                                     ..Location::default() },
                reference: Some(format!(
                    "{} of the {} ms latency budget (3GPP TS 22.185 §5.1); the fraction is \
                     an engineering choice",
                    fmt(th.access_budget_fraction),
                    fmt(th.latency_budget_ms)
                )),
                links: vec![link("mac_access_delay"), grouped_link("mac_access_delay", "node")],
            });
        }
    }
    let drops = e.value("mac_drops").unwrap_or(0.0);
    let depth = e.value("mac_queue_depth");
    if drops > 0.0 || depth.is_some_and(|d| d > th.queue_depth_saturated) {
        let nodes = worst(e.groups("mac_queue_depth", "node"), 5, true);
        let mut evidence = vec![ev("frames dropped at the MAC", drops, "count", "mac_drops")];
        if let Some(d) = depth {
            evidence.push(ev("mean TX queue depth", d, "frames", "mac_queue_depth"));
        }
        out.push(Finding {
            id: "access.tx-queue".into(),
            layer: Layer::Access,
            severity: if drops > 0.0 { Severity::Warning } else { Severity::Notice },
            title: "Transmit queues filled up".into(),
            detail: "Messages were generated faster than the channel let them out, so they \
                     queued and, past the queue's limit or lifetime, were dropped before ever \
                     reaching the air. A dropped BSM is a 100 ms gap in every neighbour's view."
                .into(),
            evidence,
            location: Location {
                nodes: nodes.iter().map(|g| g.key.clone()).collect(),
                window_s: peak_window(e, "mac_queue_depth"),
                ..Location::default()
            },
            reference: Some(format!(
                "mean depth above {} frame or any drop (engineering choice)",
                fmt(th.queue_depth_saturated)
            )),
            links: vec![link("mac_queue_depth"), link("mac_drops")],
        });
    }
    let unverified = e.value("unverified_ratio");
    let vdepth = e.value("verify_queue_depth.p95").or_else(|| e.value("verify_queue_depth"));
    let vwait = e.value("verify_wait.p95").or_else(|| e.value("verify_wait"));
    let saturated_queue = vdepth.is_some_and(|d| d > th.queue_depth_saturated * 10.0);
    if unverified.is_some_and(|u| u > th.unverified_ratio) || saturated_queue {
        let nodes = worst(e.groups("verify_queue_depth", "node"), 5, true);
        let mut evidence = Vec::new();
        if let Some(u) = unverified {
            evidence.push(ev("messages left unverified", u, "ratio", "unverified_ratio"));
        }
        if let Some(d) = vdepth {
            evidence.push(ev("verification queue depth (p95)", d, "messages", "verify_queue_depth"));
        }
        if let Some(w) = vwait {
            evidence.push(ev("verification wait (p95)", w, "ms", "verify_wait"));
        }
        out.push(Finding {
            id: "access.verification".into(),
            layer: Layer::Access,
            severity: Severity::Warning,
            title: "Signature verification could not keep up".into(),
            detail: "More signed messages arrived than the security processor could verify, \
                     so they queued or were passed up unverified. This is HSM throughput, \
                     and it grows with the number of neighbours, not with the channel."
                .into(),
            evidence,
            location: Location { nodes: nodes.iter().map(|g| g.key.clone()).collect(),
                                 window_s: peak_window(e, "verify_queue_depth"),
                                 ..Location::default() },
            reference: Some(format!(
                "unverified share above {} (engineering choice)",
                fmt(th.unverified_ratio)
            )),
            links: vec![link("unverified_ratio"), link("verify_queue_depth"), link("verify_wait")],
        });
    }
}

fn delivery(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    let mut bins: Vec<(f64, &Group)> = e
        .groups("pdr", "dist_bin")
        .iter()
        .filter(|g| g.value.is_some())
        .filter_map(|g| bin_lo(&g.key).map(|lo| (lo, g)))
        .collect();
    bins.sort_by(|a, b| a.0.total_cmp(&b.0));
    if !bins.is_empty() {
        // The range at the reliability level: the first bin below it.
        let below = bins.iter().find(|(_, g)| g.value.unwrap_or(1.0) < th.prr_range_level);
        let near: Vec<&(f64, &Group)> = bins.iter().filter(|(lo, _)| *lo < th.near_range_m).collect();
        let near_low = near.iter().find(|(_, g)| g.value.unwrap_or(1.0) < th.near_range_prr);
        if let Some((lo, g)) = near_low {
            out.push(Finding {
                id: "delivery.near-range-loss".into(),
                layer: Layer::Delivery,
                severity: Severity::Critical,
                title: "Frames were lost even between close neighbours".into(),
                detail: format!(
                    "At {} m a healthy link decodes nearly every frame, so a reception ratio \
                     of {} there is interference or half-duplex loss, not distance. Read it \
                     with the loss causes above.",
                    fmt(*lo),
                    fmt(g.value.unwrap_or(0.0))
                ),
                evidence: vec![ev(&format!("PRR at {} m", g.key), g.value.unwrap_or(0.0),
                                  "ratio", &format!("pdr[dist_bin={}]", g.key))],
                location: Location { distance_bins: vec![g.key.clone()], ..Location::default() },
                reference: Some(format!(
                    "PRR {} within {} m (engineering choice)",
                    fmt(th.near_range_prr),
                    fmt(th.near_range_m)
                )),
                links: vec![grouped_link("pdr", "dist_bin")],
            });
        }
        if let Some((lo, g)) = below {
            out.push(Finding {
                id: "delivery.range".into(),
                layer: Layer::Delivery,
                severity: if *lo < 100.0 { Severity::Warning } else { Severity::Notice },
                title: format!(
                    "Reception fell below {} from {} m",
                    fmt(th.prr_range_level),
                    fmt(*lo)
                ),
                detail: format!(
                    "The packet reception ratio against distance (3GPP TR 36.885 §A.2.1.4, 20 m \
                     bins) stays above {} out to {} m on {}; beyond that receivers miss more \
                     than one frame in ten.",
                    fmt(th.prr_range_level),
                    fmt(*lo),
                    if e.scenario.rat.is_empty() { "this radio" } else { e.scenario.rat.as_str() }
                ),
                evidence: bins
                    .iter()
                    .take_while(|(l, _)| *l <= lo + 60.0)
                    .map(|(_, g)| ev(&format!("PRR {} m", g.key), g.value.unwrap_or(0.0),
                                     "ratio", &format!("pdr[dist_bin={}]", g.key)))
                    .collect(),
                location: Location { distance_bins: vec![g.key.clone()], ..Location::default() },
                reference: Some(format!(
                    "range read at PRR {} (3GPP TR 37.885 evaluation methodology)",
                    fmt(th.prr_range_level)
                )),
                links: vec![grouped_link("pdr", "dist_bin")],
            });
        }
        // Shape: reception should not rise with distance by much.
        let mut best_rise: Option<(f64, f64, f64)> = None;
        for w in bins.windows(2) {
            let (a, b) = (w[0].1.value.unwrap_or(0.0), w[1].1.value.unwrap_or(0.0));
            let rise = b - a;
            if rise > 0.1 && best_rise.is_none_or(|(r, _, _)| rise > r) {
                best_rise = Some((rise, w[0].0, w[1].0));
            }
        }
        if let Some((rise, from, to)) = best_rise {
            out.push(Finding {
                id: "delivery.non-monotone".into(),
                layer: Layer::Delivery,
                severity: Severity::Notice,
                title: "Reception rose with distance somewhere along the curve".into(),
                detail: "A reception curve normally falls with distance. A rise this large \
                         usually means few samples in one bin, or buildings shadowing a nearer \
                         band of receivers more than a farther one (street canyons)."
                    .into(),
                evidence: vec![ev(&format!("PRR rise from {} m to {} m", fmt(from), fmt(to)),
                                  rise, "ratio", "pdr[dist_bin]")],
                location: Location {
                    distance_bins: vec![format!("{}", fmt(from)), format!("{}", fmt(to))],
                    ..Location::default()
                },
                reference: Some("a rise above 0.1 between adjacent bins (engineering choice)".into()),
                links: vec![grouped_link("pdr", "dist_bin")],
            });
        }
    }
    if let Some(nar) = e.value("nar").or_else(|| e.value("awareness")) {
        if nar < th.awareness_floor {
            out.push(Finding {
                id: "delivery.awareness".into(),
                layer: Layer::Delivery,
                severity: Severity::Warning,
                title: "Vehicles were not aware of all their neighbours".into(),
                detail: "The neighbour awareness ratio is the share of vehicles truly nearby \
                         that a vehicle had heard from recently enough to know about; a \
                         collision warning cannot fire for a vehicle that is not in the table."
                    .into(),
                evidence: vec![ev("neighbour awareness ratio", nar, "ratio", "nar")],
                location: Location { window_s: peak_window(e, "nar"), ..Location::default() },
                reference: Some(format!("below {} (engineering choice)", fmt(th.awareness_floor))),
                links: vec![link("nar")],
            });
        }
    }
    let stale = th.aoi_intervals * th.generation_interval_ms;
    if let Some(aoi) = e.value("aoi.p95").or_else(|| e.value("aoi")) {
        if aoi > stale {
            out.push(Finding {
                id: "delivery.stale-information".into(),
                layer: Layer::Delivery,
                severity: Severity::Warning,
                title: "Neighbour information went stale".into(),
                detail: "The age of information (time since the newest message heard from a \
                         neighbour) exceeded several generation intervals: consecutive \
                         messages from the same sender were lost."
                    .into(),
                evidence: vec![
                    ev("age of information (p95)", aoi, &e.unit("aoi"), "aoi.p95"),
                    ev("stale above", stale, "ms", "thresholds"),
                ],
                location: Location::default(),
                reference: Some(format!(
                    "{} generation intervals of {} ms (engineering choice; 10 Hz from SAE J2945/1)",
                    fmt(th.aoi_intervals),
                    fmt(th.generation_interval_ms)
                )),
                links: vec![link("aoi")],
            });
        }
    }
}

fn latency(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    let (name, p95) = match (e.value("e2e_latency.p95"), e.value("latency.p95")) {
        (Some(v), _) => ("e2e_latency.p95", Some(v)),
        (None, Some(v)) => ("latency.p95", Some(v)),
        _ => ("", None),
    };
    let stages = e.groups("latency_stage", "stage");
    let total: f64 = stages.iter().filter_map(|g| g.value).sum();
    let top = worst(stages, 1, true).first().copied();
    if let Some(v) = p95 {
        if v > th.latency_budget_ms {
            let mut evidence = vec![
                ev("end-to-end latency (p95)", v, "ms", name),
                ev("budget", th.latency_budget_ms, "ms", "3GPP TS 22.185 §5.1"),
            ];
            if let Some(t) = top {
                evidence.push(ev(&format!("stage `{}` mean", t.key), t.value.unwrap_or(0.0),
                                 "ms", &format!("latency_stage[{}]", t.key)));
            }
            out.push(Finding {
                id: "latency.over-budget".into(),
                layer: Layer::Latency,
                severity: Severity::Critical,
                title: "Latency exceeded the safety-message budget".into(),
                detail: format!(
                    "One message in twenty took longer than {} ms from generation to the \
                     receiving application.{}",
                    fmt(th.latency_budget_ms),
                    top.map(|t| format!(" The largest stage is `{}`.", t.key)).unwrap_or_default()
                ),
                evidence,
                location: Location { window_s: peak_window(e, "e2e_latency.p95"),
                                     ..Location::default() },
                reference: Some(format!("{} ms, 3GPP TS 22.185 §5.1", fmt(th.latency_budget_ms))),
                links: vec![link(name), grouped_link("latency_stage", "stage")],
            });
        }
    }
    if let Some(t) = top {
        if total > 0.0 {
            let share = t.value.unwrap_or(0.0) / total;
            out.push(Finding {
                id: "latency.dominant-stage".into(),
                layer: Layer::Latency,
                severity: if share > 0.6 { Severity::Notice } else { Severity::Info },
                title: format!("`{}` is the largest part of the latency", t.key),
                detail: "Where the time goes between a message being generated and being \
                         used: shortening any other stage changes little while this one \
                         dominates."
                    .into(),
                evidence: stages
                    .iter()
                    .filter_map(|g| g.value.map(|v| ev(&format!("stage `{}` mean", g.key), v,
                                                       "ms", &format!("latency_stage[{}]", g.key))))
                    .chain(std::iter::once(ev("share of the total", share, "ratio",
                                              "latency_stage")))
                    .collect(),
                location: Location::default(),
                reference: None,
                links: vec![grouped_link("latency_stage", "stage")],
            });
        }
    }
}

fn security(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    let _ = th;
    if let Some(pool) = e.value("cert_pool_valid") {
        let min = e
            .metrics
            .get("cert_pool_valid")
            .map(|m| m.series.iter().filter_map(|(_, v)| *v).fold(f64::INFINITY, f64::min))
            .filter(|v| v.is_finite())
            .unwrap_or(pool);
        if min <= 0.0 {
            out.push(Finding {
                id: "security.pool-exhausted".into(),
                layer: Layer::Security,
                severity: Severity::Critical,
                title: "Vehicles ran out of valid pseudonym certificates".into(),
                detail: "A vehicle with no valid certificate cannot sign, so it stops being \
                         heard by every receiver that verifies. Top-ups did not arrive in time."
                    .into(),
                evidence: vec![
                    ev("mean valid certificates per vehicle", pool, "certs", "cert_pool_valid"),
                    ev("lowest bin", min, "certs", "cert_pool_valid (binned)"),
                ],
                location: Location::default(),
                reference: Some("any vehicle at zero valid certificates".into()),
                links: vec![link("cert_pool_valid")],
            });
        }
    }
    if let Some(ee) = backend_entity(e, "ee") {
        let st = &ee["state"];
        let num = |k: &str| st.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let (refused, starved, started, completed) = (
            num("topups_refused"),
            num("vehicles_starved"),
            num("topups_started"),
            num("topups_completed"),
        );
        if refused > 0.0 || starved > 0.0 {
            out.push(Finding {
                id: "security.topups".into(),
                layer: Layer::Security,
                severity: if starved > 0.0 { Severity::Critical } else { Severity::Warning },
                title: "Certificate top-ups failed".into(),
                detail: "Some vehicles asked for new pseudonym certificates and were refused or \
                         left without; a starved vehicle is one whose pool ran dry before a \
                         top-up landed (no coverage, or a backend queue)."
                    .into(),
                evidence: vec![
                    ev("top-ups started", started, "count", "backend:ee.topups_started"),
                    ev("top-ups completed", completed, "count", "backend:ee.topups_completed"),
                    ev("top-ups refused", refused, "count", "backend:ee.topups_refused"),
                    ev("vehicles starved", starved, "count", "backend:ee.vehicles_starved"),
                ],
                location: Location { entities: vec!["ee".into(), "ra".into()],
                                     ..Location::default() },
                reference: None,
                links: vec![],
            });
        }
    }
    if let Some(f) = e.value("false_accusations") {
        if f > 0.0 {
            out.push(Finding {
                id: "security.false-revocations".into(),
                layer: Layer::Security,
                severity: Severity::Critical,
                title: "Honest vehicles were accused".into(),
                detail: "Misbehaviour reports named vehicles the ground truth says were honest. \
                         If the authority acts on them, honest vehicles are revoked and \
                         disappear from everyone's view."
                    .into(),
                evidence: vec![ev("false accusations", f, "count", "false_accusations")],
                location: Location::default(),
                reference: None,
                links: vec![link("false_accusations")],
            });
        }
    }
    let rev = e.groups("revocation_latency_stage", "stage");
    if let Some(t) = worst(rev, 1, true).first() {
        let total: f64 = rev.iter().filter_map(|g| g.value).sum();
        if total > 0.0 {
            out.push(Finding {
                id: "security.revocation-latency".into(),
                layer: Layer::Security,
                severity: Severity::Info,
                title: format!("Revocation took longest in `{}`", t.key),
                detail: "From the first report to the last receiver holding the CRL, by stage."
                    .into(),
                evidence: rev
                    .iter()
                    .filter_map(|g| g.value.map(|v| ev(&format!("stage `{}`", g.key), v,
                                                       &e.unit("revocation_latency_stage"),
                                                       &format!("revocation_latency_stage[{}]", g.key))))
                    .collect(),
                location: Location::default(),
                reference: None,
                links: vec![grouped_link("revocation_latency_stage", "stage")],
            });
        }
    }
}

fn backend_entity<'a>(e: &'a RunEvidence, id: &str) -> Option<&'a Value> {
    e.backend
        .as_ref()?
        .get("entities")?
        .as_array()?
        .iter()
        .find(|x| x.get("id").and_then(Value::as_str) == Some(id))
}

fn backend(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    let Some(view) = &e.backend else { return };
    let t_ns = view.get("t").and_then(Value::as_f64).unwrap_or(e.t_reached_s * 1e9);
    if t_ns <= 0.0 {
        return;
    }
    let mut hot: BTreeMap<String, (f64, f64, f64, f64)> = BTreeMap::new();
    for ent in view.get("entities").and_then(Value::as_array).into_iter().flatten() {
        let Some(q) = ent.get("queue").filter(|q| q.is_object()) else { continue };
        let id = ent.get("id").and_then(Value::as_str).unwrap_or("?").to_string();
        let f = |k: &str| q.get(k).and_then(Value::as_f64).unwrap_or(0.0);
        let servers = f("servers").max(1.0);
        let util = f("busy_ns") / (servers * t_ns);
        let served = f("served");
        let mean_wait_ms = if served > 0.0 { f("waited_ns") / served / 1e6 } else { 0.0 };
        if util > th.entity_utilisation || f("backlog_ns") > 1e9 {
            hot.insert(id, (util, f("depth"), mean_wait_ms, f("backlog_ns") / 1e9));
        }
    }
    for (id, (util, depth, wait, backlog)) in hot {
        out.push(Finding {
            id: format!("backend.saturated.{id}"),
            layer: Layer::Backend,
            severity: if util >= 0.95 { Severity::Critical } else { Severity::Warning },
            title: format!("The `{id}` entity's queue saturated"),
            detail: "Its servers were busy almost all the time, so requests waited and the \
                     wait grows faster than the load. Every certificate or report flowing \
                     through it is delayed by that queue."
                .into(),
            evidence: vec![
                ev("server utilisation", util, "ratio", &format!("backend:{id}.queue.busy_ns")),
                ev("queue depth now", depth, "requests", &format!("backend:{id}.queue.depth")),
                ev("mean wait", wait, "ms", &format!("backend:{id}.queue.waited_ns")),
                ev("backlog", backlog, "s", &format!("backend:{id}.queue.backlog_ns")),
            ],
            location: Location { entities: vec![id.clone()], ..Location::default() },
            reference: Some(format!(
                "utilisation above {} (engineering choice; queueing planning ceiling)",
                fmt(th.entity_utilisation)
            )),
            links: vec![],
        });
    }
}

fn privacy(e: &RunEvidence, th: &Thresholds, out: &mut Vec<Finding>) {
    if let Some(l) = e.value("linkability") {
        if l > th.linkability {
            out.push(Finding {
                id: "privacy.linkable".into(),
                layer: Layer::Privacy,
                severity: Severity::Warning,
                title: "Pseudonym changes did not stop tracking".into(),
                detail: "An observer listening to the air could link a vehicle's old and new \
                         pseudonyms most of the time: a change made in plain sight of \
                         neighbours, or without a silent period, is easy to follow."
                    .into(),
                evidence: vec![ev("linkability", l, "ratio", "linkability")],
                location: Location::default(),
                reference: Some(format!("above {} (engineering choice)", fmt(th.linkability))),
                links: vec![link("linkability")],
            });
        }
    }
    if let (Some(rate), Some(period)) = (e.value("pseudonym_change_rate"), e.scenario.pseudonym_period_s) {
        if period > 0.0 && e.t_reached_s > period {
            let expected = 1.0 / period;
            if rate < 0.5 * expected {
                out.push(Finding {
                    id: "privacy.changes-missing".into(),
                    layer: Layer::Privacy,
                    severity: Severity::Notice,
                    title: "Vehicles changed pseudonym less often than configured".into(),
                    detail: "Fewer changes than the time strategy implies: changes held back \
                             (no fresh certificate, or a change refused while a condition such \
                             as a minimum distance is unmet)."
                        .into(),
                    evidence: vec![
                        ev("change rate", rate, &e.unit("pseudonym_change_rate"),
                           "pseudonym_change_rate"),
                        ev("configured period", period, "s",
                           "scenario security.pseudonym_change.period_s"),
                    ],
                    location: Location::default(),
                    reference: Some("below half of 1/period".into()),
                    links: vec![link("pseudonym_change_rate")],
                });
            }
        }
    }
}

/// One row of a comparison of two runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Delta {
    /// The metric.
    pub metric: String,
    /// Its value in the first run.
    pub a: f64,
    /// In the second.
    pub b: f64,
    /// `b - a`.
    pub change: f64,
}

/// Two analyses side by side: the headline numbers both runs measured, and the findings
/// one has and the other does not.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// Headline metrics both runs have, by name.
    pub deltas: Vec<Delta>,
    /// Finding ids only the first run has (resolved by the change).
    pub only_in_a: Vec<String>,
    /// Finding ids only the second run has (introduced by the change).
    pub only_in_b: Vec<String>,
}

/// Compares two analyses. Pure.
#[must_use]
pub fn compare(a: &Analysis, b: &Analysis) -> Comparison {
    let deltas = a
        .headline
        .iter()
        .filter_map(|(k, va)| {
            b.headline.get(k).map(|vb| Delta {
                metric: k.clone(),
                a: *va,
                b: *vb,
                change: vb - va,
            })
        })
        .collect();
    let ids = |x: &Analysis| -> BTreeSet<String> {
        x.findings
            .iter()
            .filter(|f| f.severity >= Severity::Notice)
            .map(|f| f.id.clone())
            .collect()
    };
    let (ia, ib) = (ids(a), ids(b));
    Comparison {
        deltas,
        only_in_a: ia.difference(&ib).cloned().collect(),
        only_in_b: ib.difference(&ia).cloned().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(value: f64, unit: &str) -> MetricEvidence {
        MetricEvidence {
            unit: unit.into(),
            value: Some(value),
            ..MetricEvidence::default()
        }
    }

    fn groups(dim: &str, rows: &[(&str, f64)], unit: &str) -> MetricEvidence {
        let mut m = MetricEvidence { unit: unit.into(), ..MetricEvidence::default() };
        m.groups.insert(
            dim.to_string(),
            rows.iter()
                .map(|(k, v)| Group { key: (*k).to_string(), value: Some(*v), n: 100 })
                .collect(),
        );
        m
    }

    /// A run with no bottleneck: every detector quiet.
    fn healthy() -> RunEvidence {
        let mut e = RunEvidence {
            run_id: "r1".into(),
            scenario_hash: "h".into(),
            scenario: ScenarioFacts {
                name: "t".into(),
                rat: "dsrc-80211p".into(),
                duration_s: 60.0,
                speed_limit_mps: Some(11.0),
                security_protocol: Some("scms-us".into()),
                pseudonym_period_s: Some(300.0),
                metrics: vec!["all".into()],
                ..ScenarioFacts::default()
            },
            t_reached_s: 60.0,
            t_end_s: 60.0,
            ..RunEvidence::default()
        };
        let m = &mut e.metrics;
        m.insert("mean_speed".into(), metric(9.0, "m/s"));
        m.insert("ttc_conflicts".into(), metric(0.0, "count"));
        m.insert("ttc_min".into(), metric(4.0, "s"));
        m.insert("cbr".into(), metric(0.3, "ratio"));
        m.insert("cbr.p95".into(), metric(0.4, "ratio"));
        m.insert("mac_access_delay.p95".into(), metric(0.4, "ms"));
        m.insert("mac_queue_depth".into(), metric(0.1, "frames"));
        m.insert("mac_drops".into(), metric(0.0, "count"));
        m.insert("unverified_ratio".into(), metric(0.0, "ratio"));
        m.insert("verify_queue_depth.p95".into(), metric(1.0, "messages"));
        m.insert("pdr".into(), groups("dist_bin",
            &[("0-20", 0.99), ("20-40", 0.99), ("40-60", 0.98), ("60-80", 0.97),
              ("80-100", 0.95), ("100-120", 0.93)], "ratio"));
        m.insert("pdr_by_cause".into(), groups("cause",
            &[("collision", 0.2), ("half-duplex", 0.1), ("below-sensitivity", 0.7)], "ratio"));
        m.get_mut("pdr").expect("pdr").value = Some(0.97);
        m.insert("nar".into(), metric(0.97, "ratio"));
        m.insert("aoi.p95".into(), metric(150.0, "ms"));
        m.insert("e2e_latency.p95".into(), metric(12.0, "ms"));
        m.insert("latency_stage".into(), groups("stage",
            &[("generation", 1.0), ("access", 0.5), ("airtime", 0.4), ("verify", 2.0)], "ms"));
        m.insert("cert_pool_valid".into(), metric(20.0, "certs"));
        m.insert("false_accusations".into(), metric(0.0, "count"));
        m.insert("linkability".into(), metric(0.1, "ratio"));
        m.insert("pseudonym_change_rate".into(), metric(0.0033, "1/s"));
        e.backend = Some(json!({"t": 60e9, "entities": [
            {"id": "ra", "queue": {"depth": 0, "servers": 4, "served": 100,
             "busy_ns": 1e9, "waited_ns": 1e8, "backlog_ns": 0, "inbound": 100}},
            {"id": "ee", "state": {"topups_started": 10, "topups_completed": 10,
             "topups_refused": 0, "vehicles_starved": 0}}
        ]}));
        e
    }

    fn ids(a: &Analysis) -> Vec<&str> {
        a.findings
            .iter()
            .filter(|f| f.severity >= Severity::Notice)
            .map(|f| f.id.as_str())
            .collect()
    }

    #[test]
    fn a_healthy_run_has_no_bottleneck() {
        let a = analyse(&healthy(), &Thresholds::default());
        // The dominant loss cause (below sensitivity at 0.7) is reported, as a fact about
        // the channel, not as an overload.
        let found = ids(&a);
        assert_eq!(found, vec!["channel.weak-signal"], "{:#?}", a.findings);
        assert!(a.not_measured.is_empty(), "{:?}", a.not_measured);
    }

    /// Each detector, shown firing on a constructed bottleneck and quiet on the healthy
    /// run above — so each can fail.
    #[test]
    fn every_detector_fires_on_its_constructed_bottleneck() {
        let th = Thresholds::default();
        let cases: Vec<(&str, Box<dyn Fn(&mut RunEvidence)>)> = vec![
            ("traffic.delay", Box::new(|e| e.metrics.get_mut("mean_speed").unwrap().value = Some(3.0))),
            ("traffic.conflicts", Box::new(|e| e.metrics.get_mut("ttc_conflicts").unwrap().value = Some(4.0))),
            ("channel.congested", Box::new(|e| e.metrics.get_mut("cbr").unwrap().value = Some(0.85))),
            ("channel.collisions", Box::new(|e| *e.metrics.get_mut("pdr_by_cause").unwrap()
                .groups.get_mut("cause").unwrap() = vec![
                    Group { key: "collision".into(), value: Some(0.6), n: 10 },
                    Group { key: "below-sensitivity".into(), value: Some(0.4), n: 10 }])),
            ("access.delay", Box::new(|e| e.metrics.get_mut("mac_access_delay.p95").unwrap().value = Some(35.0))),
            ("access.tx-queue", Box::new(|e| e.metrics.get_mut("mac_drops").unwrap().value = Some(12.0))),
            ("access.verification", Box::new(|e| e.metrics.get_mut("unverified_ratio").unwrap().value = Some(0.2))),
            ("delivery.near-range-loss", Box::new(|e| e.metrics.get_mut("pdr").unwrap()
                .groups.get_mut("dist_bin").unwrap()[1].value = Some(0.8))),
            ("delivery.range", Box::new(|e| e.metrics.get_mut("pdr").unwrap()
                .groups.get_mut("dist_bin").unwrap()[5].value = Some(0.7))),
            ("delivery.awareness", Box::new(|e| e.metrics.get_mut("nar").unwrap().value = Some(0.6))),
            ("delivery.stale-information", Box::new(|e| e.metrics.get_mut("aoi.p95").unwrap().value = Some(900.0))),
            ("latency.over-budget", Box::new(|e| e.metrics.get_mut("e2e_latency.p95").unwrap().value = Some(140.0))),
            ("latency.dominant-stage", Box::new(|e| *e.metrics.get_mut("latency_stage").unwrap()
                .groups.get_mut("stage").unwrap() = vec![
                    Group { key: "access".into(), value: Some(30.0), n: 10 },
                    Group { key: "verify".into(), value: Some(2.0), n: 10 }])),
            ("security.pool-exhausted", Box::new(|e| e.metrics.get_mut("cert_pool_valid").unwrap().value = Some(0.0))),
            ("security.topups", Box::new(|e| e.backend.as_mut().unwrap()["entities"][1]["state"]["vehicles_starved"] = json!(3))),
            ("security.false-revocations", Box::new(|e| e.metrics.get_mut("false_accusations").unwrap().value = Some(1.0))),
            ("backend.saturated.ra", Box::new(|e| e.backend.as_mut().unwrap()["entities"][0]["queue"]["busy_ns"] = json!(230e9))),
            ("privacy.linkable", Box::new(|e| e.metrics.get_mut("linkability").unwrap().value = Some(0.8))),
            ("simulation.incomplete", Box::new(|e| e.t_reached_s = 20.0)),
        ];
        let quiet = ids(&analyse(&healthy(), &th)).into_iter().map(str::to_string).collect::<Vec<_>>();
        for (id, inject) in cases {
            assert!(!quiet.contains(&id.to_string()), "{id} fires on the healthy run");
            let mut e = healthy();
            inject(&mut e);
            let a = analyse(&e, &th);
            let f = a.finding(id).unwrap_or_else(|| panic!("{id} did not fire: {:#?}", ids(&a)));
            assert!(f.severity >= Severity::Notice, "{id} fired only as info");
            assert!(!f.evidence.is_empty(), "{id} has no evidence");
        }
    }

    #[test]
    fn a_layer_with_no_metrics_is_not_measured_not_healthy() {
        let mut e = healthy();
        e.metrics.retain(|k, _| k.starts_with("pdr"));
        e.scenario.security_protocol = None;
        e.backend = None;
        let a = analyse(&e, &Thresholds::default());
        for layer in [Layer::Traffic, Layer::Channel, Layer::Latency, Layer::Security,
                      Layer::Backend, Layer::Privacy] {
            assert!(a.not_measured.contains(&layer), "{layer:?} should be not measured");
        }
        assert!(a.findings.iter().any(|f| f.id == "security.not-measured"
            && f.detail.contains("security.protocol")));
    }

    #[test]
    fn findings_name_where_and_link_to_their_chart() {
        let mut e = healthy();
        e.metrics.insert("mac_access_delay".into(), groups("node",
            &[("3", 40.0), ("9", 55.0), ("1", 2.0)], "ms"));
        e.metrics.get_mut("mac_access_delay.p95").unwrap().value = Some(45.0);
        let a = analyse(&e, &Thresholds::default());
        let f = a.finding("access.delay").expect("fires");
        assert_eq!(f.location.nodes[0], "9", "worst node first");
        assert!(f.links.iter().any(|l| l.chart == "mac_access_delay"));
        assert!(f.reference.as_deref().unwrap_or("").contains("22.185"));
    }

    #[test]
    fn the_analysis_is_deterministic() {
        let e = healthy();
        let a = serde_json::to_string(&analyse(&e, &Thresholds::default())).expect("json");
        let b = serde_json::to_string(&analyse(&e, &Thresholds::default())).expect("json");
        assert_eq!(a, b);
    }

    #[test]
    fn a_comparison_names_what_changed() {
        let th = Thresholds::default();
        let a = analyse(&healthy(), &th);
        let mut e = healthy();
        e.metrics.get_mut("cbr").unwrap().value = Some(0.9);
        let b = analyse(&e, &th);
        let c = compare(&a, &b);
        assert!(c.only_in_b.contains(&"channel.congested".to_string()));
        let d = c.deltas.iter().find(|d| d.metric == "cbr").expect("cbr compared");
        assert!((d.change - 0.6).abs() < 1e-9);
    }

    #[test]
    fn numbers_format_one_way() {
        assert_eq!(fmt(0.68), "0.68");
        assert_eq!(fmt(100.0), "100");
        assert_eq!(fmt(12.34), "12.3");
        assert_eq!(fmt(0.0), "0");
        assert_eq!(fmt(0.0033), "0.0033");
    }
}
