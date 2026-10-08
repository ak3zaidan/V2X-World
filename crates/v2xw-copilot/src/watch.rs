//! The always-on watcher: the agent follows every run the engine serves, says in plain
//! words when something notable happens, and writes the run's summary when it ends.
//!
//! [`Watcher`] is a pure state machine. A host (the `v2xw serve` agent host) feeds it the
//! server's `run.status` answers and, every few seconds while a run moves, the evidence
//! [`crate::gather::gather`] reads; the watcher answers with [`Observation`]s and, when the
//! run ends, a [`RunSummary`]. It holds no clock, no thread and no transport, so every
//! decision it makes is a function of what it was shown, and the tests drive it with
//! constructed status answers and constructed evidence.
//!
//! # What it says, and where each number comes from
//!
//! * **A finding appears, gets worse, or clears.** The deterministic analyst
//!   ([`crate::analyst::analyse`]) is run over the evidence read so far; a finding at
//!   warning level or above that was not there before is announced, one that rises a level
//!   is announced again, and one that disappears is announced as cleared. The text is the
//!   finding's own title and its first piece of evidence, so every number is the analyst's.
//! * **A trend.** The analyst judges whole-run values, which move slowly; the latest bin of
//!   the binned channel-load and delivery series is compared against the *same* thresholds
//!   the analyst uses ([`Thresholds::cbr_target`], [`Thresholds::prr_range_level`]), so
//!   "channel load is climbing" can be said before the whole-run mean crosses the line.
//! * **An event.** The credential backend's own counters, read from its entity view: the
//!   misbehaviour authority's revocation decisions and the vehicles left with no valid
//!   pseudonym. A rise is announced with the count the backend reported.
//!
//! Findings about the run itself (`simulation.*`) and the "not measured" facts are left out
//! of the live feed: mid-run, "the run is incomplete" is always true and says nothing. Both
//! are in the summary.
//!
//! # It never slows the run
//!
//! The watcher reads the run through the same JSON-RPC methods the Studio's panels use, at
//! a cadence the host sets ([`WatchCadence`]); it never pauses, steps or reconfigures
//! anything. Nothing it produces reaches a run (see [`crate::boundary`]).

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Value, json};

use crate::analyst::{
    self, Analysis, Evidence, Finding, Layer, RunEvidence, Severity, Thresholds, fmt,
};
use crate::error::Result;
use crate::provider::{ChatMessage, ChatRequest, LlmProvider};

/// How often a host reads the run. The values are the host's to choose; these defaults
/// keep the watcher's load on the engine to one status read a second and one evidence
/// read (about thirty small metric queries) every six seconds of wall time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WatchCadence {
    /// Wall milliseconds between `run.status` reads.
    pub status_ms: u64,
    /// Wall milliseconds between evidence reads while a run moves.
    pub gather_ms: u64,
}

impl Default for WatchCadence {
    fn default() -> Self {
        WatchCadence {
            status_ms: 1_000,
            gather_ms: 6_000,
        }
    }
}

/// What kind of observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationKind {
    /// A run started and is being watched.
    Started,
    /// A finding appeared.
    Finding,
    /// A finding rose a severity level.
    Worse,
    /// A finding is no longer present.
    Cleared,
    /// The latest bin of a series crossed a threshold before the whole-run value did.
    Trend,
    /// A backend counter rose.
    Event,
    /// The run ended and its summary is ready.
    Finished,
    /// The run stopped with an engine error.
    Failed,
}

/// One thing the watcher noticed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Observation {
    /// Its place in the host's feed, from 1.
    pub seq: u64,
    /// The run it is about.
    pub run_id: String,
    /// The server's run generation (how many runs this server has started).
    pub generation: u64,
    /// Simulated seconds when it was noticed.
    pub t_s: f64,
    /// How much it matters.
    pub severity: Severity,
    /// What kind.
    pub kind: ObservationKind,
    /// The analyst finding it is about, when it is about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finding: Option<String>,
    /// The layer, when it is about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<Layer>,
    /// One plain sentence.
    pub text: String,
    /// The numbers behind it, exactly as the analyst or the backend gave them.
    pub evidence: Vec<Evidence>,
}

/// The run's overall state, as the summary states it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Verdict {
    /// No finding at warning level or above.
    Healthy,
    /// At least one warning, no critical finding.
    Attention,
    /// At least one critical finding: a requirement missed or a resource exhausted.
    Unhealthy,
}

/// One layer's line in the summary.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LayerVerdict {
    /// The layer.
    pub layer: Layer,
    /// Its title.
    pub title: String,
    /// `healthy`, `attention`, `unhealthy` or `not-measured`.
    pub state: String,
    /// The findings at warning level or above in it.
    pub findings: Vec<String>,
}

/// What the agent says about a run that ended.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RunSummary {
    /// The run.
    pub run_id: String,
    /// The server's run generation.
    pub generation: u64,
    /// The scenario's name.
    pub scenario: String,
    /// The scenario's content hash.
    pub scenario_hash: String,
    /// Simulated seconds reached.
    pub t_s: f64,
    /// Simulated seconds it was meant to run.
    pub t_end_s: f64,
    /// The overall state.
    pub verdict: Verdict,
    /// One sentence a person reads first.
    pub headline: String,
    /// The overview: the model's when one is attached and every number in it checks out,
    /// otherwise written from the findings.
    pub overview: String,
    /// Who wrote the overview: `analyst`, or the model's name.
    pub overview_by: String,
    /// Why the model's overview was not used, when it was not.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overview_note: Option<String>,
    /// Each layer's state.
    pub layers: Vec<LayerVerdict>,
    /// The findings at warning level or above, most severe first.
    pub problems: Vec<Finding>,
    /// The full analysis.
    pub analysis: Analysis,
    /// What the watcher said while the run moved.
    pub observations: Vec<Observation>,
}

/// What the host should do next, after a status read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Nothing to read.
    Idle,
    /// A run is moving: read the evidence when the cadence allows.
    Gather,
    /// The run ended: read the evidence once more and call [`Watcher::finish`].
    Finish,
}

/// Where the watcher is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Phase {
    /// No run watched yet.
    Idle,
    /// A run is moving and being watched.
    Watching,
    /// The run ended; its summary is being written.
    Analysing,
    /// The run ended and its summary is ready (or it failed).
    Done,
}

/// The live feed's limit per run: a run that keeps flapping is not allowed to fill the
/// page.
pub const MAX_OBSERVATIONS_PER_RUN: usize = 60;

/// The backend counters announced when they rise: `(entity, field, sentence)`.
/// Simulated seconds with no actor on the road before it is called out: long enough for
/// a Poisson demand of a few hundred veh/h to have placed its first vehicles.
const NO_VEHICLES_AFTER_S: f64 = 15.0;
/// Network mean speed under which traffic is called a standstill, m/s (see `alerts`).
const GRIDLOCK_MPS: f64 = 1.0;
/// Simulated seconds before a standstill is called: vehicles start from rest.
const GRIDLOCK_AFTER_S: f64 = 30.0;
/// Detection recall under which attacks are called missed: fewer than half flagged.
const ATTACK_RECALL_LOW: f64 = 0.5;

const COUNTERS: [(&str, &str, &str); 2] = [
    (
        "ma",
        "revocation_decisions",
        "The misbehaviour authority revoked a credential",
    ),
    (
        "ee",
        "vehicles_starved",
        "A vehicle ran out of valid pseudonym certificates",
    ),
];

/// The watcher.
#[derive(Debug, Clone)]
pub struct Watcher {
    thresholds: Thresholds,
    seq: u64,
    key: Option<(String, u64)>,
    phase: Phase,
    t_s: f64,
    t_end_s: f64,
    seen: BTreeMap<String, Severity>,
    trends: BTreeMap<String, bool>,
    counters: BTreeMap<String, f64>,
    observations: Vec<Observation>,
    feed: Vec<Observation>,
    summary: Option<RunSummary>,
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new(Thresholds::default())
    }
}

impl Watcher {
    /// A watcher judging against these thresholds.
    #[must_use]
    pub fn new(thresholds: Thresholds) -> Self {
        Watcher {
            thresholds,
            seq: 0,
            key: None,
            phase: Phase::Idle,
            t_s: 0.0,
            t_end_s: 0.0,
            seen: BTreeMap::new(),
            trends: BTreeMap::new(),
            counters: BTreeMap::new(),
            observations: Vec::new(),
            feed: Vec::new(),
            summary: None,
        }
    }

    /// Where it is.
    #[must_use]
    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// The run being (or last) watched: `(run_id, generation)`.
    #[must_use]
    pub fn run(&self) -> Option<(&str, u64)> {
        self.key.as_ref().map(|(r, g)| (r.as_str(), *g))
    }

    /// Simulated seconds reached, and where the run ends.
    #[must_use]
    pub fn progress(&self) -> (f64, f64) {
        (self.t_s, self.t_end_s)
    }

    /// Every observation since the host started, oldest first. `seq` is the index + 1.
    #[must_use]
    pub fn feed(&self) -> &[Observation] {
        &self.feed
    }

    /// The last finished run's summary.
    #[must_use]
    pub fn summary(&self) -> Option<&RunSummary> {
        self.summary.as_ref()
    }

    /// Takes a `run.status` answer and says what to do next.
    pub fn on_status(&mut self, status: &Value) -> Next {
        let run_id = status
            .get("run_id")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let generation = status
            .get("generation")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let state = status.get("state").and_then(Value::as_str).unwrap_or("");
        let t_ns = status.get("t_ns").and_then(Value::as_u64).unwrap_or(0);
        let end_ns = status.get("t_end_ns").and_then(Value::as_u64).unwrap_or(0);
        let t_s = t_ns as f64 / 1e9;
        let t_end_s = end_ns as f64 / 1e9;
        let ended = state == "finished" || (end_ns > 0 && t_ns >= end_ns);
        let key = (run_id, generation);
        let same = self.key.as_ref() == Some(&key);

        // A run is adopted when it moves (or is found already over): a server started
        // paused at t = 0 has a run that nobody has started yet.
        let rerun =
            same && self.phase == Phase::Done && state == "running" && t_s + 1e-9 < self.t_s;
        if !same || rerun {
            if state != "running" && !ended {
                return Next::Idle;
            }
            self.begin(key, t_s, t_end_s);
        }
        let previous_t = self.t_s;
        self.t_s = t_s;
        self.t_end_s = t_end_s;
        if self.phase == Phase::Watching {
            self.no_vehicles(status, t_s);
        }
        match self.phase {
            Phase::Watching => {
                if state == "error" {
                    self.phase = Phase::Done;
                    let failure = status
                        .pointer("/engine/failure")
                        .map(|f| match f {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        })
                        .unwrap_or_else(|| "no detail given".into());
                    self.push(
                        Severity::Critical,
                        ObservationKind::Failed,
                        None,
                        None,
                        format!(
                            "The engine stopped with an error at {} s: {failure}",
                            fmt(t_s)
                        ),
                        Vec::new(),
                    );
                    return Next::Idle;
                }
                if ended {
                    self.phase = Phase::Analysing;
                    return Next::Finish;
                }
                if state == "running" || t_s > previous_t {
                    Next::Gather
                } else {
                    Next::Idle
                }
            }
            Phase::Analysing => Next::Finish,
            Phase::Idle | Phase::Done => Next::Idle,
        }
    }

    fn begin(&mut self, key: (String, u64), t_s: f64, t_end_s: f64) {
        self.key = Some(key);
        self.phase = Phase::Watching;
        self.t_s = t_s;
        self.t_end_s = t_end_s;
        self.seen.clear();
        self.trends.clear();
        self.counters.clear();
        self.observations.clear();
        let text = if t_end_s > 0.0 {
            format!(
                "Watching this run ({} s to simulate). I'll note anything notable as it happens \
                 and summarise it at the end.",
                fmt(t_end_s)
            )
        } else {
            "Watching this run. I'll note anything notable as it happens and summarise it at \
             the end."
                .to_string()
        };
        self.push(
            Severity::Info,
            ObservationKind::Started,
            None,
            None,
            text,
            Vec::new(),
        );
    }

    fn push(
        &mut self,
        severity: Severity,
        kind: ObservationKind,
        finding: Option<String>,
        layer: Option<Layer>,
        text: String,
        evidence: Vec<Evidence>,
    ) {
        let limited = !matches!(
            kind,
            ObservationKind::Started | ObservationKind::Finished | ObservationKind::Failed
        );
        if limited && self.observations.len() >= MAX_OBSERVATIONS_PER_RUN {
            return;
        }
        self.seq += 1;
        let (run_id, generation) = self.key.clone().unwrap_or_default();
        let o = Observation {
            seq: self.seq,
            run_id,
            generation,
            t_s: self.t_s,
            severity,
            kind,
            finding,
            layer,
            text,
            evidence,
        };
        self.observations.push(o.clone());
        self.feed.push(o);
    }

    /// Takes the evidence read while a run moves and returns what is new.
    pub fn on_evidence(&mut self, e: &RunEvidence) -> Vec<Observation> {
        if self.phase != Phase::Watching || !self.is_current(e) {
            return Vec::new();
        }
        let before = self.feed.len();
        if e.t_reached_s > self.t_s {
            self.t_s = e.t_reached_s;
        }
        let analysis = analyst::analyse(e, &self.thresholds);
        let live: Vec<&Finding> = analysis
            .findings
            .iter()
            .filter(|f| f.severity >= Severity::Warning && live_finding(&f.id))
            .collect();

        for f in &live {
            match self.seen.get(&f.id).copied() {
                None => self.push(
                    f.severity,
                    ObservationKind::Finding,
                    Some(f.id.clone()),
                    Some(f.layer),
                    sentence(f),
                    first_evidence(f),
                ),
                Some(was) if f.severity > was => self.push(
                    f.severity,
                    ObservationKind::Worse,
                    Some(f.id.clone()),
                    Some(f.layer),
                    format!("Worse now: {}", sentence(f)),
                    first_evidence(f),
                ),
                Some(_) => {}
            }
            self.seen.insert(f.id.clone(), f.severity);
        }
        let gone: Vec<String> = self
            .seen
            .keys()
            .filter(|id| !live.iter().any(|f| &f.id == *id))
            .cloned()
            .collect();
        for id in gone {
            self.seen.remove(&id);
            let layer = layer_of(&id);
            self.push(
                Severity::Notice,
                ObservationKind::Cleared,
                Some(id.clone()),
                layer,
                format!("Cleared: the `{id}` problem is no longer present in the numbers so far."),
                Vec::new(),
            );
        }
        self.trends(e);
        self.alerts(e);
        self.events(e);
        self.feed[before..].to_vec()
    }

    /// Nothing on the road a while into a run: the demand placed no vehicle (a cap that
    /// counts trips, a router that refuses every destination) and every other number of
    /// the run is about an empty city.
    fn no_vehicles(&mut self, status: &Value, t_s: f64) {
        let Some(actors) = status.get("actors").and_then(Value::as_u64) else {
            return;
        };
        let flagged = self.trends.get("traffic.no-vehicles").copied().unwrap_or(false);
        if actors == 0 && t_s >= NO_VEHICLES_AFTER_S && !flagged {
            self.push(
                Severity::Critical,
                ObservationKind::Trend,
                Some("traffic.no-vehicles".into()),
                Some(Layer::Traffic),
                format!(
                    "No vehicles on the road after {} s: the demand placed none, so the \
                     network numbers describe an empty city.",
                    fmt(t_s)
                ),
                vec![Evidence {
                    label: "actors on the road".into(),
                    value: 0.0,
                    unit: "count".into(),
                    source: "run.status.actors".into(),
                }],
            );
            self.trends.insert("traffic.no-vehicles".into(), true);
        }
    }

    /// The owner's named problems the analyst's whole-run findings do not cover: the
    /// verifier dropping messages, traffic at a standstill, attacks detected or missed.
    /// Each is said once per run, when it first becomes true.
    fn alerts(&mut self, e: &RunEvidence) {
        let whole = |name: &str| e.metrics.get(name).and_then(|m| m.value);
        let once = |w: &mut Self, key: &str| -> bool {
            let fresh = !w.trends.get(key).copied().unwrap_or(false);
            w.trends.insert(key.into(), true);
            fresh
        };

        // Verification-queue overflow: a message dropped unverified is a message the
        // application never sees (IEEE 1609.2 §5.2: a receiver acts only on verified data).
        if let Some(n) = whole("verify_overflow").filter(|n| *n > 0.0) {
            if once(self, "access.verify-drops") {
                self.push(
                    Severity::Warning,
                    ObservationKind::Trend,
                    Some("access.verify-drops".into()),
                    Some(Layer::Access),
                    format!(
                        "The verification queue is dropping messages ({} so far): receivers \
                         hear more signed messages than they can check.",
                        fmt(n)
                    ),
                    vec![Evidence {
                        label: "messages dropped by a full verify queue".into(),
                        value: n,
                        unit: "count".into(),
                        source: "verify_overflow".into(),
                    }],
                );
            }
        }

        // Gridlock: the latest bin's network mean speed under 1 m/s with vehicles on the
        // road. HCM 6th ed. (Exhibit 16-8) calls an urban street LOS F below 30% of its
        // base free-flow speed; 1 m/s is under 10% of a 25 mph street, a jam rather than
        // slow traffic.
        if let Some(m) = e.metrics.get("mean_speed") {
            let to_mps = if m.unit.contains("km") { 1.0 / 3.6 } else { 1.0 };
            let latest = m.series.iter().rev().find_map(|(t, v)| v.map(|v| (*t, v)));
            if let Some((t, v)) = latest {
                let mps = v * to_mps;
                if mps < GRIDLOCK_MPS && t >= GRIDLOCK_AFTER_S && once(self, "traffic.gridlock") {
                    self.push(
                        Severity::Warning,
                        ObservationKind::Trend,
                        Some("traffic.gridlock".into()),
                        Some(Layer::Traffic),
                        format!(
                            "Traffic is at a standstill: mean speed {} {} in the latest \
                             interval (under 1 m/s is a jam, HCM LOS F).",
                            fmt(v),
                            m.unit
                        ),
                        vec![Evidence {
                            label: "mean speed, latest bin".into(),
                            value: v,
                            unit: m.unit.clone(),
                            source: format!("mean_speed[t={} s]", fmt(t)),
                        }],
                    );
                }
            }
        }

        // Attacks: the detectors' recall over the attacker's messages so far.
        if let Some(recall) = whole("det_recall") {
            let (key, sev, text) = if recall < ATTACK_RECALL_LOW {
                (
                    "security.attacks-missed",
                    Severity::Warning,
                    format!(
                        "Attacks are being missed: the detectors flagged {} of the \
                         attackers' messages.",
                        fmt(recall)
                    ),
                )
            } else {
                (
                    "security.attacks-detected",
                    Severity::Notice,
                    format!(
                        "Attacks detected: the detectors flagged {} of the attackers' messages.",
                        fmt(recall)
                    ),
                )
            };
            if once(self, key) {
                self.push(
                    sev,
                    ObservationKind::Trend,
                    Some(key.into()),
                    Some(Layer::Security),
                    text,
                    vec![Evidence {
                        label: "detection recall".into(),
                        value: recall,
                        unit: "ratio".into(),
                        source: "det_recall".into(),
                    }],
                );
            }
        }
    }

    fn is_current(&self, e: &RunEvidence) -> bool {
        self.key
            .as_ref()
            .is_none_or(|(r, _)| e.run_id.is_empty() || &e.run_id == r)
    }

    /// The latest bin of the delivery series against the analyst's own reliability level.
    /// (Channel load needs no trend of its own: the analyst's `channel.congested` already
    /// fires, as a warning, on the busiest bin before the whole-run mean crosses.)
    fn trends(&mut self, e: &RunEvidence) {
        let latest = |name: &str| -> Option<(f64, f64)> {
            e.metrics
                .get(name)?
                .series
                .iter()
                .rev()
                .find_map(|(t, v)| v.map(|v| (*t, v)))
        };
        let whole = |name: &str| e.metrics.get(name).and_then(|m| m.value);

        // Delivery dropping below the reliability the analyst reads ranges at.
        let level = self.thresholds.prr_range_level;
        if let Some((t, pdr)) = latest("pdr") {
            let low = pdr < level && whole("pdr").is_none_or(|w| w >= level);
            let was = self
                .trends
                .get("delivery.dropping")
                .copied()
                .unwrap_or(false);
            if low && !was {
                self.push(
                    Severity::Warning,
                    ObservationKind::Trend,
                    Some("delivery.dropping".into()),
                    Some(Layer::Delivery),
                    format!(
                        "Delivery is dropping: {} of messages were received in the latest \
                         interval, below the {} reliability level (3GPP TR 37.885).",
                        fmt(pdr),
                        fmt(level)
                    ),
                    vec![Evidence {
                        label: "packet delivery ratio, latest bin".into(),
                        value: pdr,
                        unit: "ratio".into(),
                        source: format!("pdr[t={} s]", fmt(t)),
                    }],
                );
            }
            self.trends.insert("delivery.dropping".into(), low);
        }
    }

    /// Backend counters that rose since the last read.
    fn events(&mut self, e: &RunEvidence) {
        let Some(view) = &e.backend else { return };
        for (entity, field, what) in COUNTERS {
            let Some(now) = backend_counter(view, entity, field) else {
                continue;
            };
            let key = format!("{entity}.{field}");
            let before = self.counters.get(&key).copied().unwrap_or(0.0);
            if now > before {
                let text = format!("{what} ({} so far).", fmt(now));
                self.push(
                    Severity::Notice,
                    ObservationKind::Event,
                    Some(key.clone()),
                    Some(if entity == "ma" {
                        Layer::Backend
                    } else {
                        Layer::Security
                    }),
                    text,
                    vec![Evidence {
                        label: field.replace('_', " "),
                        value: now,
                        unit: "count".into(),
                        source: format!("backend:{entity}.{field}"),
                    }],
                );
            }
            self.counters.insert(key, now);
        }
    }

    /// Takes the evidence read once the run ended and writes the summary, from the
    /// findings alone. A host with a model may then replace the overview with
    /// [`Watcher::set_overview`].
    pub fn finish(&mut self, e: &RunEvidence) -> RunSummary {
        // Counters and trends seen in the last interval still deserve their line.
        if self.phase == Phase::Analysing || self.phase == Phase::Watching {
            self.phase = Phase::Watching;
            let _ = self.on_evidence(e);
        }
        let analysis = analyst::analyse(e, &self.thresholds);
        let problems: Vec<Finding> = analysis
            .findings
            .iter()
            .filter(|f| f.severity >= Severity::Warning)
            .cloned()
            .collect();
        let verdict = if problems.iter().any(|f| f.severity == Severity::Critical) {
            Verdict::Unhealthy
        } else if problems.is_empty() {
            Verdict::Healthy
        } else {
            Verdict::Attention
        };
        let layers = layer_verdicts(&analysis);
        let headline = headline(verdict, &problems, &layers, e);
        let (run_id, generation) = self.key.clone().unwrap_or_else(|| (e.run_id.clone(), 0));
        let mut summary = RunSummary {
            run_id,
            generation,
            scenario: e.scenario.name.clone(),
            scenario_hash: e.scenario_hash.clone(),
            t_s: e.t_reached_s.max(self.t_s),
            t_end_s: if e.t_end_s > 0.0 {
                e.t_end_s
            } else {
                self.t_end_s
            },
            verdict,
            headline: headline.clone(),
            overview: String::new(),
            overview_by: "analyst".into(),
            overview_note: None,
            layers,
            problems,
            analysis,
            observations: Vec::new(),
        };
        summary.overview = written_overview(&summary);
        self.phase = Phase::Done;
        let severity = match verdict {
            Verdict::Healthy => Severity::Info,
            Verdict::Attention => Severity::Warning,
            Verdict::Unhealthy => Severity::Critical,
        };
        self.push(
            severity,
            ObservationKind::Finished,
            None,
            None,
            format!("The run ended. {headline} The run summary is ready."),
            Vec::new(),
        );
        summary.observations = self.observations.clone();
        self.summary = Some(summary.clone());
        summary
    }

    /// Replaces the last summary's overview (the model's, once checked).
    pub fn set_overview(
        &mut self,
        run_id: &str,
        overview: String,
        by: String,
        note: Option<String>,
    ) {
        if let Some(s) = self.summary.as_mut().filter(|s| s.run_id == run_id) {
            if !overview.trim().is_empty() {
                s.overview = overview;
                s.overview_by = by;
            }
            s.overview_note = note;
        }
    }
}

/// Whether a finding belongs in the live feed.
fn live_finding(id: &str) -> bool {
    !id.starts_with("simulation.") && !id.ends_with(".not-measured")
}

fn layer_of(id: &str) -> Option<Layer> {
    let head = id.split('.').next().unwrap_or("");
    serde_json::from_value::<Layer>(json!(head)).ok()
}

/// What a finding means in the words a person watching would use, for the kinds the owner
/// named: congestion building, delivery dropping, a queue saturating, credentials failing.
fn lead(id: &str) -> Option<&'static str> {
    let head = id.split('.').next().unwrap_or("");
    Some(match id {
        "channel.congested" | "channel.collisions" | "access.delay" => "Congestion is building",
        "traffic.delay" => "Traffic is congested",
        "traffic.conflicts" => "Near misses",
        "access.tx-queue" | "access.verification" => "A queue is saturating",
        "latency.over-budget" => "Messages are arriving late",
        "security.pool-exhausted" | "security.topups" => "Vehicles are running out of credentials",
        "security.false-revocations" => "Honest vehicles were revoked",
        "privacy.linkable" => "Vehicles can be tracked",
        _ if id.starts_with("backend.saturated.") => "A queue is saturating",
        _ if head == "delivery" => "Delivery is dropping",
        _ => return None,
    })
}

/// The number that best shows a finding: the busiest bin when only the peak crossed (a
/// warning, not a whole-run breach), else the first.
fn headline_evidence(f: &Finding) -> Option<&Evidence> {
    if f.severity == Severity::Warning {
        if let Some(e) = f.evidence.iter().find(|e| e.label == "busiest bin") {
            return Some(e);
        }
    }
    f.evidence.first()
}

/// A finding as one sentence: what it means, its title and its telling number.
fn sentence(f: &Finding) -> String {
    let title = f.title.trim_end_matches('.');
    let body = match headline_evidence(f) {
        Some(e) => format!(
            "{title} ({} {}{}).",
            e.label,
            fmt(e.value),
            unit_suffix(&e.unit)
        ),
        None => format!("{title}."),
    };
    match lead(&f.id) {
        Some(l) => {
            let mut cs = body.chars();
            let first: String = cs
                .next()
                .map(|c| c.to_lowercase().collect())
                .unwrap_or_default();
            format!("{l}: {first}{}", cs.as_str())
        }
        None => body,
    }
}

fn unit_suffix(unit: &str) -> String {
    match unit {
        "" | "ratio" | "count" => String::new(),
        u => format!(" {u}"),
    }
}

fn first_evidence(f: &Finding) -> Vec<Evidence> {
    f.evidence.iter().take(2).cloned().collect()
}

fn backend_counter(view: &Value, entity: &str, field: &str) -> Option<f64> {
    let ent = view
        .get("entities")?
        .as_array()?
        .iter()
        .find(|x| x.get("id").and_then(Value::as_str) == Some(entity))?;
    ent.pointer(&format!("/state/{field}"))
        .or_else(|| ent.get(field))
        .and_then(Value::as_f64)
}

fn layer_verdicts(a: &Analysis) -> Vec<LayerVerdict> {
    Layer::ALL
        .iter()
        .map(|&layer| {
            let fs: Vec<&Finding> = a
                .findings
                .iter()
                .filter(|f| f.layer == layer && f.severity >= Severity::Warning)
                .collect();
            let state = if a.not_measured.contains(&layer) {
                "not-measured"
            } else if fs.iter().any(|f| f.severity == Severity::Critical) {
                "unhealthy"
            } else if fs.is_empty() {
                "healthy"
            } else {
                "attention"
            };
            LayerVerdict {
                layer,
                title: layer.title().to_string(),
                state: state.to_string(),
                findings: fs.iter().map(|f| f.id.clone()).collect(),
            }
        })
        .collect()
}

fn headline(
    verdict: Verdict,
    problems: &[Finding],
    layers: &[LayerVerdict],
    e: &RunEvidence,
) -> String {
    let measured = layers.iter().filter(|l| l.state != "not-measured").count();
    match verdict {
        Verdict::Healthy if measured < layers.len() => format!(
            "Every measured layer stayed within its threshold, but only {measured} of {} layers \
             were measured: the scenario's `metrics` list leaves the rest out.",
            layers.len()
        ),
        Verdict::Healthy => "Every layer stayed within its threshold.".to_string(),
        Verdict::Attention | Verdict::Unhealthy => {
            let critical = problems
                .iter()
                .filter(|f| f.severity == Severity::Critical)
                .count();
            let worst = problems
                .first()
                .map(|f| f.title.trim_end_matches('.').to_string())
                .unwrap_or_default();
            let incomplete = if e.t_end_s > 0.0 && e.t_reached_s + 1e-9 < e.t_end_s {
                " The run did not reach its end."
            } else {
                ""
            };
            format!(
                "{} problem{} found{}; the most severe: {worst}.{incomplete}",
                problems.len(),
                if problems.len() == 1 { "" } else { "s" },
                if critical > 0 {
                    format!(", {critical} critical")
                } else {
                    String::new()
                },
            )
        }
    }
}

/// The overview written from the findings: what went wrong, what stayed healthy, what was
/// not measured. Every number in it is a finding's.
#[must_use]
pub fn written_overview(s: &RunSummary) -> String {
    let mut out = String::new();
    out.push_str(&s.headline);
    out.push_str("\n\n");
    if !s.problems.is_empty() {
        out.push_str("Problems, most severe first:\n");
        for f in s.problems.iter().take(8) {
            out.push_str(&format!("- [{}] {}\n", f.id, sentence(f)));
        }
        out.push('\n');
    }
    let healthy: Vec<&str> = s
        .layers
        .iter()
        .filter(|l| l.state == "healthy")
        .map(|l| l.title.as_str())
        .collect();
    if !healthy.is_empty() {
        out.push_str(&format!("Within thresholds: {}.\n", healthy.join(", ")));
    }
    let missing: Vec<&str> = s
        .layers
        .iter()
        .filter(|l| l.state == "not-measured")
        .map(|l| l.title.as_str())
        .collect();
    if !missing.is_empty() {
        out.push_str(&format!(
            "Not measured in this run: {}.\n",
            missing.join(", ")
        ));
    }
    out
}

/// The instructions for the model's end-of-run overview. Stable byte for byte, so the
/// prompt cache holds.
#[must_use]
pub fn overview_system_prompt() -> String {
    "You write the end-of-run overview of a V2X world simulation for an engineer who is \
     evaluating V2X protocols. You are given the deterministic analyst's findings for the \
     run as JSON. Write plain language a non-specialist can follow, in Markdown, short:\n\
     1. Two or three sentences: how the run went overall and whether it is fit to draw \
     conclusions from.\n\
     2. The problems, most severe first, one bullet each, naming the finding id in square \
     brackets (e.g. [channel.congested]), what it means in practice, and its numbers.\n\
     3. One line on which layers stayed within their thresholds and which were not \
     measured.\n\
     The one hard rule: never state a number that is not in the JSON, and never compute \
     one (no differences, ratios or percentages of your own). Quote numbers with their \
     units exactly as given. If there are no problems, say so plainly."
        .to_string()
}

/// Asks a model for the overview, and checks it: a number in it that the analysis does not
/// contain discards it. Returns the overview, or `Err` with the reason it was not used.
///
/// # Errors
/// The provider's error.
pub fn model_overview(
    provider: &mut dyn LlmProvider,
    summary: &RunSummary,
) -> Result<core::result::Result<String, String>> {
    let compact = summary.analysis.compact();
    let observations: Vec<Value> = summary
        .observations
        .iter()
        .map(|o| json!({"t_s": fmt(o.t_s), "text": o.text}))
        .collect();
    let payload = json!({
        "scenario": summary.scenario,
        "simulated_s": fmt(summary.t_s),
        "planned_s": fmt(summary.t_end_s),
        "verdict": summary.verdict,
        "layers": summary.layers,
        "analysis": compact,
        "observed_during_the_run": observations,
    });
    let corpus = payload.to_string();
    let completion = provider.complete(&ChatRequest {
        messages: vec![
            ChatMessage::system(overview_system_prompt()),
            ChatMessage::user(format!("The run's analysis:\n{corpus}")),
        ],
        tools: Vec::new(),
    })?;
    let text = completion.message.text().trim().to_string();
    if text.is_empty() {
        return Ok(Err("the model returned no text".into()));
    }
    let ungrounded = crate::agent::ungrounded_numbers(&text, &corpus);
    if ungrounded.is_empty() {
        Ok(Ok(text))
    } else {
        Ok(Err(format!(
            "the model's overview stated numbers the analysis does not contain ({}), so the \
             analyst's own overview is shown",
            ungrounded.join(", ")
        )))
    }
}

/// The summary as Markdown, for `v2xw agent` and for copying out of the page.
#[must_use]
pub fn summary_markdown(s: &RunSummary) -> String {
    let mut md = format!(
        "# Run summary: {}\n\n",
        if s.scenario.is_empty() {
            &s.run_id
        } else {
            &s.scenario
        }
    );
    md.push_str(&format!(
        "Run `{}`, {} of {} s simulated. Verdict: **{:?}**.\n\n",
        s.run_id,
        fmt(s.t_s),
        fmt(s.t_end_s),
        s.verdict
    ));
    md.push_str(s.overview.trim());
    md.push_str("\n\n");
    if let Some(n) = &s.overview_note {
        md.push_str(&format!("> {n}\n\n"));
    }
    md.push_str("| Layer | State | Findings |\n|---|---|---|\n");
    for l in &s.layers {
        md.push_str(&format!(
            "| {} | {} | {} |\n",
            l.title,
            l.state,
            l.findings.join(", ")
        ));
    }
    md.push_str(&format!(
        "\n_Overview by {}; every number is the analyst's._\n",
        s.overview_by
    ));
    md
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyst::{Group, MetricEvidence, ScenarioFacts};
    use crate::provider::{Completion, ScriptedProvider};

    fn status(run: &str, generation: u64, state: &str, t_s: u64, end_s: u64) -> Value {
        json!({"run_id": run, "generation": generation, "state": state,
               "t_ns": t_s * 1_000_000_000, "t_end_ns": end_s * 1_000_000_000})
    }

    fn metric(value: f64, unit: &str) -> MetricEvidence {
        MetricEvidence {
            unit: unit.into(),
            value: Some(value),
            ..MetricEvidence::default()
        }
    }

    /// A run with nothing wrong in the layers the live feed watches.
    fn quiet(t: f64) -> RunEvidence {
        let mut e = RunEvidence {
            run_id: "r1".into(),
            scenario_hash: "h".into(),
            scenario: ScenarioFacts {
                name: "grid".into(),
                rat: "dsrc-80211p".into(),
                duration_s: 60.0,
                speed_limit_mps: Some(11.0),
                metrics: vec!["all".into()],
                ..ScenarioFacts::default()
            },
            t_reached_s: t,
            t_end_s: 60.0,
            ..RunEvidence::default()
        };
        let m = &mut e.metrics;
        m.insert("mean_speed".into(), metric(9.0, "m/s"));
        m.insert("ttc_conflicts".into(), metric(0.0, "count"));
        let mut cbr = metric(0.3, "ratio");
        cbr.series = vec![(0.0, Some(0.3)), (10.0, Some(0.3))];
        m.insert("cbr".into(), cbr);
        m.insert("mac_access_delay.p95".into(), metric(0.4, "ms"));
        m.insert("mac_queue_depth".into(), metric(0.1, "frames"));
        m.insert("mac_drops".into(), metric(0.0, "count"));
        let mut pdr = metric(0.97, "ratio");
        pdr.series = vec![(0.0, Some(0.97)), (10.0, Some(0.97))];
        pdr.groups.insert(
            "dist_bin".into(),
            vec![
                Group {
                    key: "0-20".into(),
                    value: Some(0.99),
                    n: 100,
                },
                Group {
                    key: "20-40".into(),
                    value: Some(0.98),
                    n: 100,
                },
            ],
        );
        m.insert("pdr".into(), pdr);
        m.insert("nar".into(), metric(0.97, "ratio"));
        m.insert("e2e_latency.p95".into(), metric(12.0, "ms"));
        e
    }

    fn kinds(os: &[Observation]) -> Vec<(ObservationKind, Option<&str>)> {
        os.iter().map(|o| (o.kind, o.finding.as_deref())).collect()
    }

    #[test]
    fn a_paused_run_at_zero_is_not_adopted_and_a_moving_one_is() {
        let mut w = Watcher::default();
        assert_eq!(w.on_status(&status("r1", 1, "paused", 0, 60)), Next::Idle);
        assert_eq!(w.phase(), Phase::Idle);
        assert!(w.feed().is_empty());
        assert_eq!(
            w.on_status(&status("r1", 1, "running", 1, 60)),
            Next::Gather
        );
        assert_eq!(w.phase(), Phase::Watching);
        assert_eq!(kinds(w.feed()), vec![(ObservationKind::Started, None)]);
        // A second status of the same run does not restart it.
        assert_eq!(
            w.on_status(&status("r1", 1, "running", 2, 60)),
            Next::Gather
        );
        assert_eq!(w.feed().len(), 1);
    }

    #[test]
    fn a_quiet_run_says_nothing_between_start_and_end() {
        let mut w = Watcher::default();
        w.on_status(&status("r1", 1, "running", 10, 60));
        assert!(w.on_evidence(&quiet(10.0)).is_empty(), "{:#?}", w.feed());
        assert!(w.on_evidence(&quiet(20.0)).is_empty());
        assert_eq!(
            w.on_status(&status("r1", 1, "finished", 60, 60)),
            Next::Finish
        );
        let s = w.finish(&quiet(60.0));
        assert_eq!(s.verdict, Verdict::Healthy, "{:#?}", s.problems);
        assert_eq!(
            kinds(&s.observations).last().map(|k| k.0),
            Some(ObservationKind::Finished)
        );
        assert!(s.overview.contains("Within thresholds"), "{}", s.overview);
        assert!(
            s.layers.iter().any(|l| l.state == "not-measured"),
            "security was not measured"
        );
    }

    #[test]
    fn an_empty_road_verify_drops_and_missed_attacks_are_flagged_once() {
        let mut w = Watcher::default();
        let mut st = status("r1", 1, "running", 5, 60);
        st["actors"] = json!(0);
        w.on_status(&st);
        assert!(
            !w.feed().iter().any(|o| o.finding.as_deref() == Some("traffic.no-vehicles")),
            "not yet: vehicles may still be entering"
        );
        st["t_ns"] = json!(20_000_000_000u64);
        w.on_status(&st);
        w.on_status(&st);
        let empty: Vec<_> = w
            .feed()
            .iter()
            .filter(|o| o.finding.as_deref() == Some("traffic.no-vehicles"))
            .collect();
        assert_eq!(empty.len(), 1, "{:#?}", w.feed());
        assert_eq!(empty[0].severity, Severity::Critical);

        let mut e = quiet(20.0);
        e.metrics.insert("verify_overflow".into(), metric(42.0, "count"));
        e.metrics.insert("det_recall".into(), metric(0.2, "ratio"));
        let new = w.on_evidence(&e);
        let ids: Vec<_> = new.iter().filter_map(|o| o.finding.as_deref()).collect();
        assert!(ids.contains(&"access.verify-drops"), "{ids:?}");
        assert!(ids.contains(&"security.attacks-missed"), "{ids:?}");
        assert!(new.iter().any(|o| o.text.contains("42")));
        assert!(w.on_evidence(&e).is_empty(), "each is said once");
    }

    /// Congestion: the finding is announced once, announced again only when it is worse,
    /// and announced as cleared when it goes. Each step is shown failing on the quiet run.
    #[test]
    fn a_finding_is_announced_once_then_cleared() {
        let mut w = Watcher::default();
        w.on_status(&status("r1", 1, "running", 10, 60));
        let mut congested = quiet(10.0);
        congested.metrics.get_mut("cbr").unwrap().value = Some(0.85);
        let first = w.on_evidence(&congested);
        assert_eq!(
            kinds(&first),
            vec![(ObservationKind::Finding, Some("channel.congested"))],
            "{first:#?}"
        );
        assert!(
            first[0].text.contains("0.85"),
            "the number is the analyst's: {}",
            first[0].text
        );
        assert!(w.on_evidence(&congested).is_empty(), "announced once");
        let cleared = w.on_evidence(&quiet(30.0));
        assert_eq!(
            kinds(&cleared),
            vec![(ObservationKind::Cleared, Some("channel.congested"))]
        );
    }

    #[test]
    fn congestion_and_a_delivery_drop_are_named_before_the_whole_run_value_crosses() {
        let mut w = Watcher::default();
        w.on_status(&status("r1", 1, "running", 20, 60));
        let mut e = quiet(20.0);
        e.metrics
            .get_mut("cbr")
            .unwrap()
            .series
            .push((20.0, Some(0.85)));
        e.metrics
            .get_mut("pdr")
            .unwrap()
            .series
            .push((20.0, Some(0.8)));
        let got = w.on_evidence(&e);
        assert_eq!(
            kinds(&got),
            vec![
                (ObservationKind::Finding, Some("channel.congested")),
                (ObservationKind::Trend, Some("delivery.dropping")),
            ],
            "{got:#?}"
        );
        // The busiest bin crossed, not the mean: the note says so with the bin's number.
        assert!(
            got[0].text.starts_with("Congestion is building:"),
            "{}",
            got[0].text
        );
        assert!(got[0].text.contains("busiest bin 0.85"), "{}", got[0].text);
        assert!(
            got[1].text.contains("0.8 ") && got[1].text.contains("0.9"),
            "{}",
            got[1].text
        );
        // Still over: not repeated.
        assert!(w.on_evidence(&e).is_empty());
        // The quiet series names neither.
        let mut w2 = Watcher::default();
        w2.on_status(&status("r1", 1, "running", 20, 60));
        assert!(w2.on_evidence(&quiet(20.0)).is_empty());
    }

    #[test]
    fn a_revocation_is_announced_with_the_backends_count() {
        let mut w = Watcher::default();
        w.on_status(&status("r1", 1, "running", 20, 60));
        let mut e = quiet(20.0);
        e.backend = Some(json!({"entities": [{"id": "ma", "state": {"revocation_decisions": 0}}]}));
        assert!(w.on_evidence(&e).is_empty(), "no revocation, nothing said");
        e.backend = Some(json!({"entities": [{"id": "ma", "state": {"revocation_decisions": 2}}]}));
        let got = w.on_evidence(&e);
        assert_eq!(
            kinds(&got),
            vec![(ObservationKind::Event, Some("ma.revocation_decisions"))]
        );
        assert!(got[0].text.contains("2 so far"), "{}", got[0].text);
        assert!(w.on_evidence(&e).is_empty(), "the same count is not news");
    }

    #[test]
    fn a_new_generation_starts_a_new_watch_and_keeps_the_last_summary() {
        let mut w = Watcher::default();
        w.on_status(&status("r1", 1, "finished", 60, 60));
        let s = w.finish(&quiet(60.0));
        assert_eq!(s.generation, 1);
        assert_eq!(
            w.on_status(&status("r2", 2, "running", 1, 60)),
            Next::Gather
        );
        assert_eq!(w.run(), Some(("r2", 2)));
        assert_eq!(w.summary().map(|s| s.run_id.as_str()), Some("r1"));
        let seqs: Vec<u64> = w.feed().iter().map(|o| o.seq).collect();
        assert!(
            seqs.windows(2).all(|p| p[1] == p[0] + 1),
            "the feed is numbered without gaps"
        );
    }

    #[test]
    fn evidence_from_another_run_is_ignored() {
        let mut w = Watcher::default();
        w.on_status(&status("r9", 1, "running", 10, 60));
        let mut congested = quiet(10.0);
        congested.metrics.get_mut("cbr").unwrap().value = Some(0.85);
        assert!(
            w.on_evidence(&congested).is_empty(),
            "r1's evidence is not r9's"
        );
    }

    #[test]
    fn a_problem_run_is_called_so_and_the_model_overview_is_checked() {
        let mut w = Watcher::default();
        w.on_status(&status("r1", 1, "finished", 60, 60));
        let mut e = quiet(60.0);
        e.metrics.get_mut("e2e_latency.p95").unwrap().value = Some(140.0);
        let s = w.finish(&e);
        assert_ne!(s.verdict, Verdict::Healthy);
        assert!(s.problems.iter().any(|f| f.id == "latency.over-budget"));
        assert!(
            s.layers
                .iter()
                .any(|l| l.layer == Layer::Latency && l.state != "healthy")
        );

        let reply = |text: &str| Completion {
            message: ChatMessage::assistant(text),
            finish_reason: "stop".into(),
        };
        let mut good = ScriptedProvider::new(vec![reply(
            "Latency missed its budget: 140 ms [latency.over-budget].",
        )]);
        assert_eq!(
            model_overview(&mut good, &s).unwrap().as_deref().ok(),
            Some("Latency missed its budget: 140 ms [latency.over-budget].")
        );
        let mut bad = ScriptedProvider::new(vec![reply("Latency was 187 ms, far over budget.")]);
        let why = model_overview(&mut bad, &s).unwrap().unwrap_err();
        assert!(why.contains("187"), "{why}");
        // The request carries no tools and the stable instructions.
        assert!(good.seen[0].tools.is_empty());
        assert_eq!(good.seen[0].messages[0].text(), overview_system_prompt());

        w.set_overview("r1", "model text".into(), "scripted".into(), None);
        assert_eq!(
            w.summary().map(|s| s.overview_by.as_str()),
            Some("scripted")
        );
        assert!(summary_markdown(w.summary().unwrap()).contains("| Latency |"));
    }

    #[test]
    fn an_engine_error_is_reported_and_ends_the_watch() {
        let mut w = Watcher::default();
        w.on_status(&status("r1", 1, "running", 10, 60));
        let mut st = status("r1", 1, "error", 12, 60);
        st["engine"] = json!({"failure": "world import failed"});
        assert_eq!(w.on_status(&st), Next::Idle);
        assert_eq!(w.phase(), Phase::Done);
        let last = w.feed().last().unwrap();
        assert_eq!(last.kind, ObservationKind::Failed);
        assert!(last.text.contains("world import failed"));
    }
}
