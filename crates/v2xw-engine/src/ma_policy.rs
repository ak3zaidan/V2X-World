//! `security.scms.ma_policy`: a declarative, pluggable MA decision policy
//! (docs/design/scms-real/08-settings-ux-and-gaps.md §7), wired into the run.
//!
//! The evaluator is [`v2xw_threat::ma_policy::MaPolicy`] (piece B1). This module reads the
//! scenario's keys into it (piece B2): a policy's preset, or a name from
//! `security.scms.ma_policies`, with every field written beside it overriding the preset's;
//! a provider's own `ma_policy`; the decision mode, carried out by the provider's SCMS run
//! as its `MaMode`; and, at each report, the linkage authorities' device link, issuer and
//! secMark phase that `identity: device` and the population guard read.
//!
//! **Byte-identity of the default.** A policy whose resolved fields are today's MA
//! (`CorroborationParams::default()`, no extension) is hosted as today's MA, so the run
//! takes today's path unchanged; a preset whose decision is the CV-pilot operator review
//! and that writes no `decision` leaves the run's review rule (`ma_review_s`, compressed
//! calendar) as it is today.
//!
//! The keys' menus, ranges, titles and status stay here (§11), read through
//! [`crate::scenario::module_keys`].

use std::collections::BTreeMap;

use serde_json::Value;
use v2xw_threat::ma_policy::{
    Accuracy, Decay, Decision, Evaluator, Identity, MaPolicy, MaPolicyParams, PolicyEvidence,
    PopulationGuard,
};

use crate::error::{EngineError, Result, ScenarioError};
use crate::scenario::Scenario;
use crate::scenario::module_keys::{self, KeyWords, Opt};
use crate::scenario::schema::MaPolicySpec;
use crate::scenario::validate::{Bound, KeyStatus, Status};

/// The model id the policy reports as.
pub const MODEL_ID: &str = v2xw_threat::ma_policy::MODEL_ID;

/// The preset a policy starts from when it names none: today's MA.
pub const DEFAULT_PRESET: &str = v2xw_threat::ma_policy::DEFAULT_PRESET;

/// The family's presets (§7 table).
pub const PRESET_IDS: [&str; 8] = v2xw_threat::ma_policy::PRESET_IDS;

/// `identity`: what one case is about.
pub const IDENTITY_IDS: [&str; 2] = ["pseudonym", "device"];
/// `evidence.accuracy_weighting`.
pub const ACCURACY_WEIGHTING_IDS: [&str; 2] = ["none", "catch"];
/// `decay.mode`.
pub const DECAY_IDS: [&str; 3] = ["none", "daily-multiplier", "half-life"];
/// `population_guard.mode`.
pub const POPULATION_GUARD_IDS: [&str; 2] = ["off", "cluster"];
/// `decision.mode`.
pub const DECISION_IDS: [&str; 2] = ["manual-review", "automatic"];
/// The keys of `evidence.rules`: the SM-MBR v1.0 report subtypes, then every engine check
/// as `detect/<check id>` ([`v2xw_threat::ma_policy::RULE_KEYS`]).
pub const RULE_KEY_IDS: [&str; 21] = v2xw_threat::ma_policy::RULE_KEYS;

/// `decay: daily-multiplier` with no factor: SM-MBR v1.0's store multiplier.
const DAILY_FACTOR: f64 = 0.9;
/// … and its hour, 05:00 Eastern (SM-MBR v1.0 §2.2).
const DAILY_HOUR_ET: u32 = 5;
/// `decay: half-life` with no half-life: one day (ASSUMPTION; a preset of §7).
const HALF_LIFE_S: f64 = 86_400.0;
/// `population_guard: cluster` with no `c` or `m`: `ma-replay`'s grid (3, 3).
const GUARD_C: u32 = 3;
const GUARD_M: u32 = 3;
/// `accuracy_weighting: catch` with no τ: SM-MBR gpsDrift, 1 m.
const TAU_M: f64 = 1.0;

const ROOT: &str = "security.scms.ma_policy";
const FAMILY: &str = "security.scms.ma_policies";
const PROVIDER: &str = "security.scms.deployment.providers[].ma_policy";

/// The keys' implementation status. `validate::status_of` reads these beside its own
/// table, longest match winning.
pub static KEY_STATUS: &[KeyStatus] = &[
    KeyStatus {
        path: ROOT,
        status: Status::Wired,
        note: "The MA's decision rule as one policy (08-settings-ux-and-gaps §7): a preset, \
               with any field written beside it overriding the preset's. The default, \
               cv-pilot-manual, is today's MA (reports corroborated into events, then an \
               operator's review). Refused together with detection.ma.",
    },
    KeyStatus {
        path: FAMILY,
        status: Status::Wired,
        note: "Named MA policies (08 §7): each starts from a preset; a provider's own \
               ma_policy, ma_policy.preset or an experiment sweep over it may name one.",
    },
    KeyStatus {
        path: PROVIDER,
        status: Status::Wired,
        note: "This provider's MA policy (08 §7): a preset id or a name from \
               security.scms.ma_policies. Its MA decides about the certificates this \
               provider issued; other providers keep the deployment's MA.",
    },
];

const fn range(path: &'static str, lo: f64, hi: f64, what: &'static str) -> Bound {
    Bound {
        path,
        lo,
        hi,
        exclusive_lo: false,
        what,
    }
}

/// The ranges, relative to a policy's root (`security.scms.ma_policy` or
/// `security.scms.ma_policies.*`). Sources: 08 §7 "Settings and menus".
static BOUNDS: &[Bound] = &[
    range(
        "evidence.rules.*.score",
        0.0,
        100.0,
        "an evidence rule's score",
    ),
    range("evidence.tau_m", 0.1, 10.0, "the accuracy tolerance"),
    range("evidence.event_window_s", 0.1, 600.0, "the event window"),
    range(
        "evidence.event_min_reporters",
        1.0,
        100.0,
        "the reporters an event needs",
    ),
    range("threshold.min_events", 1.0, 100.0, "the events that revoke"),
    range(
        "threshold.min_score",
        0.0,
        1_000_000.0,
        "the score that revokes",
    ),
    range(
        "threshold.min_reports",
        0.0,
        1_000_000.0,
        "the reports that revoke",
    ),
    range(
        "threshold.min_distinct_reporters",
        1.0,
        1_000.0,
        "the distinct reporters that revoke",
    ),
    range(
        "threshold.window_s",
        1.0,
        2_592_000.0,
        "the evidence window",
    ),
    range("decay.factor", 0.0, 1.0, "the daily evidence multiplier"),
    range(
        "decay.hour_et",
        0.0,
        23.0,
        "the hour of the daily multiplier",
    ),
    // 08 §7 gives presets (1 h, 1 day) and no range; a second to a year spans every
    // published store and review horizon (MBR v1.0's daily decay, a week's i-period).
    range(
        "decay.half_life_s",
        1.0,
        31_536_000.0,
        "the evidence half-life",
    ),
    range(
        "population_guard.c",
        2.0,
        1_000.0,
        "the accused devices that crowd a class",
    ),
    range(
        "population_guard.m",
        1.0,
        10.0,
        "the reporter multiplier in a crowded class",
    ),
    // The legacy defaults are 40 and 30 (`CorroborationParams`); no source bounds them.
    range(
        "defence.reputation_max",
        1.0,
        1_000_000.0,
        "the accusations that make a reporter untrusted",
    ),
    range(
        "defence.report_budget",
        1.0,
        1_000_000.0,
        "the reports one reporter may file per window",
    ),
    Bound {
        path: "decision.review_s",
        lo: 0.0,
        hi: 2_592_000.0,
        exclusive_lo: true,
        what: "the operator's review time",
    },
];

const fn w(path: &'static str, title: &'static str, unit: &'static str, step: f64) -> KeyWords {
    KeyWords {
        path,
        title,
        unit,
        step,
    }
}

/// Titles, units and steps, relative to a policy's root.
static WORDS: &[KeyWords] = &[
    w("preset", "MA policy", "", 0.0),
    w("identity", "What one case is about", "", 0.0),
    w(
        "evidence.rules.*.score",
        "Score one report adds",
        "score",
        0.1,
    ),
    w(
        "evidence.accuracy_weighting",
        "Allow for the sender's stated accuracy",
        "",
        0.0,
    ),
    w("evidence.tau_m", "Accuracy tolerance (CaTch τ)", "m", 0.1),
    w(
        "evidence.event_window_s",
        "Reports this close together are one event",
        "s",
        0.1,
    ),
    w(
        "evidence.event_min_reporters",
        "Reporters one event needs",
        "reporters",
        1.0,
    ),
    w("threshold.min_events", "Events that revoke", "events", 1.0),
    w(
        "threshold.min_score",
        "Score that revokes (0: not used)",
        "score",
        0.1,
    ),
    w(
        "threshold.min_reports",
        "Reports that revoke (0: not used)",
        "reports",
        1.0,
    ),
    w(
        "threshold.min_distinct_reporters",
        "Different reporters that revoke",
        "reporters",
        1.0,
    ),
    w("threshold.window_s", "Evidence window", "s", 1.0),
    w("decay.mode", "How old evidence fades", "", 0.0),
    w(
        "decay.factor",
        "Daily evidence multiplier",
        "× per day",
        0.01,
    ),
    w(
        "decay.hour_et",
        "Hour the daily multiplier applies",
        "h (Eastern)",
        1.0,
    ),
    w("decay.half_life_s", "Evidence half-life", "s", 1.0),
    w("population_guard.mode", "Population guard", "", 0.0),
    w(
        "population_guard.c",
        "Accused devices that make a class crowded",
        "devices",
        1.0,
    ),
    w(
        "population_guard.m",
        "Reporter multiplier in a crowded class",
        "×",
        1.0,
    ),
    w(
        "defence.trusted_reporters",
        "Count trusted reporters only",
        "",
        0.0,
    ),
    w(
        "defence.reputation_max",
        "Accusations that make a reporter untrusted",
        "accusations",
        1.0,
    ),
    w(
        "defence.report_budget",
        "Reports one reporter may file per window",
        "reports",
        1.0,
    ),
    w("decision.mode", "Who decides", "", 0.0),
    w("decision.review_s", "Operator review time", "s", 60.0),
];

/// Titles of the absolute paths.
static ROOT_WORDS: &[KeyWords] = &[
    w(ROOT, "MA decision policy", "", 0.0),
    w(FAMILY, "Named MA policies", "", 0.0),
    w(PROVIDER, "This provider's MA policy", "", 0.0),
];

/// The presets' menu (§7 table).
static PRESETS: &[Opt] = &[
    Opt {
        value: "cv-pilot-manual",
        label: "CV-pilot operator review (US default)",
        description: "Today's MA: reports corroborated into events (2 reporters an event, 3 \
                      events in 60 s, 3 reporters), then an operator's 8 h review, as the CV \
                      pilots ran it (CAMP122 §4.2-4.3); on a compressed calendar the review \
                      is automatic, as today. The evidence rule is an ASSUMPTION.",
        available: true,
    },
    Opt {
        value: "corroborated-auto",
        label: "Corroborated, automatic",
        description: "The same corroborating rule with no operator: it revokes as soon as \
                      the evidence suffices.",
        available: true,
    },
    Opt {
        value: "legacy-window",
        label: "Legacy persistence gate",
        description: "The legacy corpus's rule (3 trusted reporters over 4 distinct seconds \
                      spanning 3 s inside 15 s), kept for parity with its results; automatic.",
        available: true,
    },
    Opt {
        value: "pseudonym-k",
        label: "k reporters per certificate",
        description: "Revokes a pseudonym certificate once k different reporters accused it \
                      (k = 3, no trust gate; v2xw ma-replay's rule); k = 1 lets one report \
                      revoke (unsound). Automatic.",
        available: true,
    },
    Opt {
        value: "device-k",
        label: "k reporters per device",
        description: "Revokes a whole device, linked through the linkage authorities, once \
                      k different reporters accused any of its certificates within 1 h (k = \
                      3; v2xw ma-replay's rule). Automatic.",
        available: true,
    },
    Opt {
        value: "catch",
        label: "Accuracy-aware (CaTch-style)",
        description: "Per device, each reporter counts only the share of the position error \
                      the accused's own stated accuracy cannot explain; revokes at a summed \
                      score of 3 within 1 h (Kamel et al., IEEE WCNC 2019). Automatic.",
        available: true,
    },
    Opt {
        value: "cluster",
        label: "Population guard",
        description: "Per device, 3 reporters within 1 h; when 3 devices of one class are \
                      accused at once, a device of that class needs 3 times the reporters, \
                      so one product line's benign trigger cannot revoke the line. Automatic.",
        available: true,
    },
    Opt {
        value: "custom",
        label: "Custom",
        description: "Starts from the US default's values; every field you write is yours.",
        available: true,
    },
];

static IDENTITY: &[Opt] = &[
    Opt {
        value: "pseudonym",
        label: "Per certificate",
        description: "Each pseudonym certificate is its own case, as an MA sees reports \
                      before the linkage authorities join them.",
        available: true,
    },
    Opt {
        value: "device",
        label: "Per device, through the linkage authorities",
        description: "All certificates of one device are one case, joined through the \
                      linkage authorities' link values (answered at once: the query's \
                      round trips are not modelled).",
        available: true,
    },
];

static ACCURACY: &[Opt] = &[
    Opt {
        value: "none",
        label: "None",
        description: "Every report counts in full, whatever accuracy the accused stated.",
        available: true,
    },
    Opt {
        value: "catch",
        label: "Discount what stated accuracy explains",
        description: "A position report counts only the deviation the accused's own stated \
                      accuracy cannot explain (CaTch); other checks count in full.",
        available: true,
    },
];

static DECAY: &[Opt] = &[
    Opt {
        value: "none",
        label: "None",
        description: "Evidence keeps its score until it leaves the window.",
        available: true,
    },
    Opt {
        value: "daily-multiplier",
        label: "Daily multiplier (SCMS Manager MBR v1.0)",
        description: "Once a day, at the hour set (05:00 Eastern unless set), every \
                      report's score is multiplied by the factor (0.9 unless set), as the \
                      US device's report store does. Acts on the score threshold.",
        available: true,
    },
    Opt {
        value: "half-life",
        label: "Half-life",
        description: "A report's score halves every half-life (1 day unless set), \
                      continuously. Acts on the score threshold.",
        available: true,
    },
];

static GUARD: &[Opt] = &[
    Opt {
        value: "off",
        label: "Off",
        description: "Every device needs the same evidence.",
        available: true,
    },
    Opt {
        value: "cluster",
        label: "Cluster guard",
        description: "A device in a class (secMark phase, accuracy band, issuer) with c or \
                      more accused devices needs m times the reporters (the ma-replay \
                      cluster rule; c = 3, m = 3 unless set).",
        available: true,
    },
];

static DECISION: &[Opt] = &[
    Opt {
        value: "manual-review",
        label: "Operator review (CV pilots)",
        description: "An operator reviews each case for the review time (8 h unless set) \
                      before the device is added to the revocation list (CAMP122 §4.2-4.3).",
        available: true,
    },
    Opt {
        value: "automatic",
        label: "Automatic",
        description: "The rule revokes the moment the evidence suffices.",
        available: true,
    },
];

/// `path` relative to a policy's root, or `None` outside one.
fn rel(path: &str) -> Option<&str> {
    path.strip_prefix("security.scms.ma_policy.")
        .or_else(|| path.strip_prefix("security.scms.ma_policies.*."))
}

/// The range for `path`.
pub fn bound(path: &str) -> Option<&'static Bound> {
    let r = rel(path)?;
    BOUNDS.iter().find(|b| b.path == r)
}

/// The title, unit and step for `path`.
pub fn words(path: &str) -> Option<&'static KeyWords> {
    if let Some(k) = ROOT_WORDS.iter().find(|k| k.path == path) {
        return Some(k);
    }
    let r = rel(path)?;
    WORDS.iter().find(|k| k.path == r)
}

/// The menu for `path`, and whether it is closed.
pub fn options(path: &str) -> Option<(Vec<Value>, bool)> {
    if path == PROVIDER {
        // Open: a provider may name a policy of `ma_policies` as well as a preset.
        return Some((module_keys::rows(PRESETS), false));
    }
    let (opts, closed) = match rel(path)? {
        // Open for the same reason.
        "preset" => (PRESETS, false),
        "identity" => (IDENTITY, true),
        "evidence.accuracy_weighting" => (ACCURACY, true),
        "decay.mode" => (DECAY, true),
        "population_guard.mode" => (GUARD, true),
        "decision.mode" => (DECISION, true),
        _ => return None,
    };
    Some((module_keys::rows(opts), closed))
}

/// Every path these tables name, under both roots.
pub fn table_paths() -> Vec<String> {
    let mut out: Vec<String> = KEY_STATUS.iter().map(|k| k.path.to_string()).collect();
    out.extend(ROOT_WORDS.iter().map(|k| k.path.to_string()));
    for root in [ROOT, "security.scms.ma_policies.*"] {
        for r in BOUNDS
            .iter()
            .map(|b| b.path)
            .chain(WORDS.iter().map(|k| k.path))
        {
            out.push(format!("{root}.{r}"));
        }
    }
    out
}

/// The closed key set of a map inside a policy.
fn map_keys(template: &str) -> Option<&'static [&'static str]> {
    let ids: &'static [&'static str] = &RULE_KEY_IDS;
    template.ends_with(".evidence.rules").then_some(ids)
}

/// A policy resolved from the scenario: its fields, and whether a `decision` was written
/// anywhere along its chain (the policy itself, or the named policy it starts from).
#[derive(Debug, Clone)]
struct Resolved {
    params: MaPolicyParams,
    decision_written: bool,
}

/// The well-formedness pass over one written policy: each leaf in its range and menu.
/// Returns the leaves, as the per-field checks name them.
fn leaves_of(
    spec: &MaPolicySpec,
    at: &str,
    template: &str,
    e: &mut Vec<ScenarioError>,
) -> Vec<module_keys::Leaf> {
    let v = serde_json::to_value(spec).unwrap_or(Value::Null);
    let maps = [format!("{template}.evidence.rules")];
    let maps: Vec<&str> = maps.iter().map(String::as_str).collect();
    let mut ok = Vec::new();
    for leaf in module_keys::leaves(&v, at, template, &maps, &map_keys, e) {
        match module_keys::malformed(&leaf) {
            Some(err) => e.push(err),
            None => ok.push(leaf),
        }
    }
    ok
}

/// Resolves the policy `spec`, written at `at`. `family` is the named policies its preset
/// may name (`None` for a named policy itself, whose preset must be a built-in one).
fn resolve(
    spec: &MaPolicySpec,
    at: &str,
    template: &str,
    family: Option<&BTreeMap<String, MaPolicySpec>>,
    e: &mut Vec<ScenarioError>,
) -> Option<Resolved> {
    let before = e.len();
    let leaves = leaves_of(spec, at, template, e);
    let preset = spec.preset.as_deref().unwrap_or(DEFAULT_PRESET);
    let (mut params, mut decision_written) = if let Some(p) = MaPolicyParams::preset(preset) {
        (p, false)
    } else if let Some(named) = family.and_then(|f| f.get(preset)) {
        let r = resolve(
            named,
            &format!("{FAMILY}.{preset}"),
            &format!("{FAMILY}.*"),
            None,
            e,
        )?;
        (r.params, r.decision_written)
    } else {
        let names = family.map_or(String::new(), |f| {
            if f.is_empty() {
                String::new()
            } else {
                format!(
                    ", or a name from {FAMILY} ({})",
                    f.keys().cloned().collect::<Vec<_>>().join(", ")
                )
            }
        });
        e.push(ScenarioError::conflict(
            format!("{at}.preset"),
            format!(
                "'{preset}' is not a preset{}; allowed: {}{names}",
                if family.is_some() {
                    " or a named policy"
                } else {
                    " (a named policy starts from a built-in preset)"
                },
                PRESET_IDS.join(", ")
            ),
        ));
        return None;
    };
    decision_written |= spec.decision.is_some();
    // Under legacy-window most fields have nothing to act on: refuse them where written.
    for leaf in &leaves {
        let Some(r) = leaf.template.strip_prefix(&format!("{template}.")) else {
            continue;
        };
        let r = if r.starts_with("evidence.rules.") {
            "evidence.rules"
        } else {
            r
        };
        if !params.applies(r) {
            e.push(ScenarioError::conflict(
                &leaf.at,
                format!(
                    "does not apply under the legacy persistence gate (preset {}), which \
                     counts distinct reporters, distinct seconds and a span; remove it or \
                     start from another preset",
                    params.preset
                ),
            ));
        }
    }
    overlay(spec, &mut params, at, e);
    (e.len() == before).then_some(Resolved {
        params,
        decision_written,
    })
}

/// Writes every field `spec` states over `p`, refusing a field its mode does not use.
#[allow(clippy::too_many_lines)]
fn overlay(spec: &MaPolicySpec, p: &mut MaPolicyParams, at: &str, e: &mut Vec<ScenarioError>) {
    let only = |e: &mut Vec<ScenarioError>, field: &str, when: &str| {
        e.push(ScenarioError::conflict(
            format!("{at}.{field}"),
            format!("applies only when {when}; remove it or set that mode"),
        ));
    };
    let count = |n: u32| usize::try_from(n).unwrap_or(usize::MAX);
    if let Some(i) = spec.identity.as_deref() {
        p.identity = if i == "device" {
            Identity::Device
        } else {
            Identity::Pseudonym
        };
    }
    if let Some(ev) = &spec.evidence {
        for (k, r) in &ev.rules {
            if let Some(score) = r.score {
                p.scores.insert(k.clone(), score);
            }
        }
        if let Some(a) = ev.accuracy_weighting.as_deref() {
            p.accuracy = if a == "catch" {
                Accuracy::Catch {
                    tau_m: match p.accuracy {
                        Accuracy::Catch { tau_m } => tau_m,
                        Accuracy::None => TAU_M,
                    },
                }
            } else {
                Accuracy::None
            };
        }
        if let Some(t) = ev.tau_m {
            match &mut p.accuracy {
                Accuracy::Catch { tau_m } => *tau_m = t,
                Accuracy::None => only(e, "evidence.tau_m", "accuracy_weighting is catch"),
            }
        }
        if let Some(w) = ev.event_window_s {
            p.event_window_s = w;
        }
        if let Some(n) = ev.event_min_reporters {
            p.event_min_reporters = count(n);
        }
    }
    if let Some(t) = &spec.threshold {
        if let Some(n) = t.min_events {
            p.min_events = count(n);
        }
        if let Some(x) = t.min_score {
            p.min_score = x;
        }
        if let Some(n) = t.min_reports {
            p.min_reports = count(n);
        }
        if let Some(n) = t.min_distinct_reporters {
            p.min_distinct_reporters = count(n);
        }
        if let Some(w) = t.window_s {
            p.window_s = w;
        }
    }
    if let Some(d) = &spec.decay {
        match d.mode.as_deref() {
            Some("none") => p.decay = Decay::None,
            Some("daily-multiplier") if !matches!(p.decay, Decay::DailyMultiplier { .. }) => {
                p.decay = Decay::DailyMultiplier {
                    factor: DAILY_FACTOR,
                    hour_et: DAILY_HOUR_ET,
                };
            }
            Some("half-life") if !matches!(p.decay, Decay::HalfLife { .. }) => {
                p.decay = Decay::HalfLife {
                    half_life_s: HALF_LIFE_S,
                };
            }
            _ => {}
        }
        match &mut p.decay {
            Decay::DailyMultiplier { factor, hour_et } => {
                if let Some(f) = d.factor {
                    *factor = f;
                }
                if let Some(h) = d.hour_et {
                    *hour_et = h;
                }
            }
            Decay::HalfLife { half_life_s } => {
                if let Some(h) = d.half_life_s {
                    *half_life_s = h;
                }
            }
            Decay::None => {}
        }
        if !matches!(p.decay, Decay::DailyMultiplier { .. }) {
            if d.factor.is_some() {
                only(e, "decay.factor", "decay.mode is daily-multiplier");
            }
            if d.hour_et.is_some() {
                only(e, "decay.hour_et", "decay.mode is daily-multiplier");
            }
        }
        if !matches!(p.decay, Decay::HalfLife { .. }) && d.half_life_s.is_some() {
            only(e, "decay.half_life_s", "decay.mode is half-life");
        }
    }
    if let Some(g) = &spec.population_guard {
        match g.mode.as_deref() {
            Some("off") => p.guard = PopulationGuard::Off,
            Some("cluster") if p.guard == PopulationGuard::Off => {
                p.guard = PopulationGuard::Cluster {
                    c: count(GUARD_C),
                    m: count(GUARD_M),
                };
            }
            _ => {}
        }
        match &mut p.guard {
            PopulationGuard::Cluster { c, m } => {
                if let Some(x) = g.c {
                    *c = count(x);
                }
                if let Some(x) = g.m {
                    *m = count(x);
                }
            }
            PopulationGuard::Off => {
                if g.c.is_some() {
                    only(e, "population_guard.c", "population_guard.mode is cluster");
                }
                if g.m.is_some() {
                    only(e, "population_guard.m", "population_guard.mode is cluster");
                }
            }
        }
    }
    if let Some(d) = &spec.defence {
        if let Some(on) = d.trusted_reporters {
            p.defence = on;
        }
        if let Some(n) = d.reputation_max {
            p.reputation_max = n;
        }
        if let Some(n) = d.report_budget {
            p.report_budget = n;
        }
    }
    if let Some(d) = &spec.decision {
        match d.mode.as_deref() {
            Some("automatic") => p.decision = Decision::Automatic,
            Some("manual-review") if p.decision == Decision::Automatic => {
                p.decision = Decision::ManualReview {
                    review_s: v2xw_threat::ma_policy::CV_PILOT_REVIEW_S,
                };
            }
            _ => {}
        }
        if let Some(r) = d.review_s {
            match &mut p.decision {
                Decision::ManualReview { review_s } => *review_s = r,
                Decision::Automatic => {
                    only(e, "decision.review_s", "decision.mode is manual-review")
                }
            }
        }
    }
    if p.decay != Decay::None && p.min_score <= 0.0 && spec.decay.is_some() {
        e.push(ScenarioError::conflict(
            format!("{at}.decay.mode"),
            "fades a report's score, and threshold.min_score is 0 (not used), so it would \
             change nothing; set a score threshold or leave decay at none"
                .to_string(),
        ));
    }
}

/// Whether `p` is today's MA: the corroborating rule at `CorroborationParams::default()`
/// with no extension (the decision mode is the run's, not the rule's).
fn is_today(p: &MaPolicyParams) -> bool {
    let d = v2xw_threat::CorroborationParams::default();
    p.evaluator == Evaluator::Corroborating
        && p.identity == Identity::Pseudonym
        && p.scores.values().all(|s| (*s - 1.0).abs() < f64::EPSILON)
        && p.accuracy == Accuracy::None
        && p.decay == Decay::None
        && p.guard == PopulationGuard::Off
        && p.min_score <= 0.0
        && p.min_reports == 0
        && (p.event_window_s - d.event_window_s).abs() < f64::EPSILON
        && p.event_min_reporters == d.event_min_reporters
        && p.min_events == d.min_events
        && (p.window_s - d.window_s).abs() < f64::EPSILON
        && p.min_distinct_reporters == d.min_reporters
        && p.defence == d.defence
        && p.reputation_max == d.reputation_max
        && p.report_budget == d.report_budget
}

/// The policy a provider's `ma_policy` names: a preset id or a key of `ma_policies`.
fn named(
    name: &str,
    at: &str,
    family: &BTreeMap<String, MaPolicySpec>,
    e: &mut Vec<ScenarioError>,
) -> Option<Resolved> {
    if let Some(spec) = family.get(name) {
        return resolve(
            spec,
            &format!("{FAMILY}.{name}"),
            &format!("{FAMILY}.*"),
            None,
            &mut Vec::new(),
        );
    }
    if let Some(params) = MaPolicyParams::preset(name) {
        return Some(Resolved {
            params,
            decision_written: false,
        });
    }
    let mut names: Vec<String> = PRESET_IDS.iter().map(ToString::to_string).collect();
    names.extend(family.keys().cloned());
    e.push(ScenarioError::conflict(
        at,
        format!(
            "'{name}' names neither a preset nor a policy of {FAMILY}; allowed: {}",
            names.join(", ")
        ),
    ));
    None
}

/// Every refusal of the MA-policy keys, in document order.
pub fn refusals(s: &Scenario) -> Vec<ScenarioError> {
    let mut e = Vec::new();
    let Some(scms) = s.security.scms.as_ref() else {
        return e;
    };
    let providers = scms
        .deployment
        .as_ref()
        .map(|d| d.providers.as_slice())
        .unwrap_or_default();
    if s.detection.ma.is_some() {
        if scms.ma_policy.is_some() {
            e.push(ScenarioError::conflict(
                ROOT,
                "and detection.ma both choose the MA's decision rule; remove one",
            ));
        }
        for p in providers.iter().filter(|p| p.ma_policy.is_some()) {
            e.push(ScenarioError::conflict(
                format!("security.scms.deployment.providers.{}.ma_policy", p.name),
                "and detection.ma both choose the MA's decision rule; remove one",
            ));
        }
    }
    for (name, spec) in &scms.ma_policies {
        let at = format!("{FAMILY}.{name}");
        if PRESET_IDS.contains(&name.as_str()) {
            e.push(ScenarioError::conflict(
                &at,
                format!("'{name}' is a preset's id; give the named policy another name"),
            ));
            continue;
        }
        resolve(spec, &at, &format!("{FAMILY}.*"), None, &mut e);
    }
    if let Some(spec) = &scms.ma_policy
        && let Some(r) = resolve(spec, ROOT, ROOT, Some(&scms.ma_policies), &mut e)
        && r.decision_written
        && crate::phase2::lifecycle_param(s, "ma_review_s").is_some()
    {
        e.push(ScenarioError::conflict(
            format!("{ROOT}.decision"),
            "and ma_review_s (security.scms.lifecycle or security.protocol.params) both set \
             the operator's review; remove one",
        ));
    }
    for p in providers {
        let Some(m) = &p.ma_policy else {
            continue;
        };
        let at = format!("security.scms.deployment.providers.{}.ma_policy", p.name);
        if let Some(r) = named(m, &at, &scms.ma_policies, &mut e)
            && r.decision_written
            && p.ma_mode.is_some()
        {
            e.push(ScenarioError::conflict(
                &at,
                format!(
                    "names '{m}', whose decision is written, and this provider's ma_mode \
                     sets it too; remove one"
                ),
            ));
        }
    }
    e
}

/// The loader's check: every refusal, once.
pub fn validate(s: &Scenario, e: &mut Vec<ScenarioError>) {
    for err in refusals(s) {
        if !e.contains(&err) {
            e.push(err);
        }
    }
}

/// The run's start in unix seconds, for the daily decay's Eastern clock.
fn t0_unix_s(s: &Scenario) -> i64 {
    v2xw_core::time::WallClock::parse_rfc3339(&s.time.t0).map_or(0, |w| w.t0_unix_s())
}

fn evaluator(params: MaPolicyParams, s: &Scenario) -> PolicyMa {
    PolicyMa {
        inner: MaPolicy::new(params).with_calendar(
            t0_unix_s(s),
            v2xw_proto::scms::calendar::eastern_times_between,
        ),
    }
}

/// The policy the run's MA hosts, for `phase2::MaHost::from_scenario`: `None` runs
/// today's MA, which is what the default preset is.
///
/// # Errors
/// The first refusal, so a run built without the loader is refused the same way.
pub(crate) fn host(s: &Scenario) -> Result<Option<PolicyMa>> {
    if let Some(err) = refusals(s).into_iter().next() {
        return Err(EngineError::Scenario(err));
    }
    let Some(scms) = s.security.scms.as_ref() else {
        return Ok(None);
    };
    let Some(spec) = &scms.ma_policy else {
        return Ok(None);
    };
    let r = resolve(spec, ROOT, ROOT, Some(&scms.ma_policies), &mut Vec::new());
    Ok(r.filter(|r| !is_today(&r.params))
        .map(|r| evaluator(r.params, s)))
}

/// The MA of each provider, in deployment order: a provider with its own `ma_policy`
/// hosts that policy, every other the deployment's (`base`).
///
/// # Errors
/// A refusal, or a deployment that does not resolve.
pub(crate) fn hosts(
    s: &Scenario,
    base: crate::phase2::MaHost,
    providers: usize,
) -> Result<Vec<crate::phase2::MaHost>> {
    let Some(scms) = s.security.scms.as_ref() else {
        return Ok(vec![base; providers]);
    };
    let specs = scms
        .deployment
        .as_ref()
        .map(|d| d.providers.as_slice())
        .unwrap_or_default();
    if specs.iter().all(|p| p.ma_policy.is_none()) {
        return Ok(vec![base; providers]);
    }
    let deployment = crate::phase2::deployment_of(s, true)?;
    let mut out = Vec::with_capacity(providers);
    for k in 0..providers {
        let own = deployment
            .providers
            .get(k)
            .and_then(|q| specs.iter().find(|p| p.name == q.name))
            .and_then(|p| p.ma_policy.as_deref());
        let Some(name) = own else {
            out.push(base.clone());
            continue;
        };
        let mut e = Vec::new();
        let r = named(name, PROVIDER, &scms.ma_policies, &mut e);
        if let Some(err) = e.into_iter().next() {
            return Err(EngineError::Scenario(err));
        }
        out.push(match r {
            Some(r) if !is_today(&r.params) => {
                crate::phase2::MaHost::Policy(evaluator(r.params, s))
            }
            _ => crate::phase2::MaHost::Corroborated(v2xw_threat::CorroboratedMa::defaults()),
        });
    }
    Ok(out)
}

/// The decision mode each provider's SCMS run carries out: the provider's own policy's,
/// else the deployment's. A provider with its own `ma_mode` keeps it; a policy that
/// writes no decision and starts from the CV-pilot operator review leaves the run's
/// review rule as today (`ma_review_s`, or automatic on a compressed calendar).
///
/// # Errors
/// A refusal, or a deployment that does not resolve.
pub(crate) fn apply_decision(
    s: &Scenario,
    runs: &mut [v2xw_proto::scms::run::ScmsRun],
) -> Result<()> {
    use v2xw_proto::scms::params::MaMode;
    let Some(scms) = s.security.scms.as_ref() else {
        return Ok(());
    };
    let specs = scms
        .deployment
        .as_ref()
        .map(|d| d.providers.as_slice())
        .unwrap_or_default();
    let root = scms
        .ma_policy
        .as_ref()
        .and_then(|spec| resolve(spec, ROOT, ROOT, Some(&scms.ma_policies), &mut Vec::new()));
    for (k, run) in runs.iter_mut().enumerate() {
        let name = run
            .state
            .deployment
            .providers
            .get(k)
            .map(|q| q.name.clone());
        let spec = name
            .as_deref()
            .and_then(|n| specs.iter().find(|p| p.name == n));
        if spec.is_some_and(|p| p.ma_mode.is_some()) {
            continue;
        }
        let own = match spec.and_then(|p| p.ma_policy.as_deref()) {
            Some(m) => named(m, PROVIDER, &scms.ma_policies, &mut Vec::new()),
            None => root.clone(),
        };
        let Some(r) = own else {
            continue;
        };
        match r.params.decision {
            Decision::Automatic => run.state.params.ma.mode = MaMode::Automatic,
            Decision::ManualReview { review_s } if r.decision_written => {
                run.state.params.ma.mode = MaMode::ManualReview {
                    review: v2xw_core::time::Duration::from_nanos((review_s * 1e9).round() as u64),
                };
            }
            Decision::ManualReview { .. } => {}
        }
    }
    Ok(())
}

/// The sweep axis that compares MA policies (08 §7, "Sweep mode"): one full run per
/// policy per seed.
pub const SWEEP_AXIS: &str = "security.scms.ma_policy.preset";

/// The MA policy family a scenario names, each policy resolved with the run's calendar:
/// the names its `experiment.sweep` over [`SWEEP_AXIS`] lists, or else every preset and
/// every policy of `security.scms.ma_policies`. `v2xw ma-replay --policies` replays them
/// over one recorded run through the same evaluator the run hosts.
///
/// # Errors
/// A refusal of the scenario's MA keys, or a name that is neither a preset nor a named
/// policy.
pub fn family_of(s: &Scenario) -> Result<Vec<(String, MaPolicy)>> {
    if let Some(err) = refusals(s).into_iter().next() {
        return Err(EngineError::Scenario(err));
    }
    let empty = BTreeMap::new();
    let family = s.security.scms.as_ref().map_or(&empty, |x| &x.ma_policies);
    let swept: Option<Vec<String>> = s
        .experiment
        .as_ref()
        .and_then(|e| e.sweep.get(SWEEP_AXIS))
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        });
    let names = swept.unwrap_or_else(|| {
        PRESET_IDS
            .iter()
            .map(ToString::to_string)
            .chain(family.keys().cloned())
            .collect()
    });
    let at = format!("experiment.sweep.{SWEEP_AXIS}");
    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let mut e = Vec::new();
        let r = named(&name, &at, family, &mut e);
        if let Some(err) = e.into_iter().next() {
            return Err(EngineError::Scenario(err));
        }
        let Some(r) = r else {
            return Err(EngineError::Scenario(ScenarioError::conflict(
                &at,
                format!("'{name}' does not resolve to a policy"),
            )));
        };
        out.push((name, evaluator(r.params, s).inner));
    }
    Ok(out)
}

/// What the deciding MA learns about a report's subject beyond the report: the linkage
/// authorities' device link, the issuing provider and the judged BSM's secMark phase.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SubjectHint<'a> {
    /// The LAs' link token for the accused device.
    pub link: &'a str,
    /// The provider whose ACA issued the accused certificate.
    pub issuer: &'a str,
    /// The judged BSM's secMark modulo 50 ms, when the run kept it.
    pub secmark_phase_ms: Option<u32>,
}

/// The hosted policy.
#[derive(Debug, Clone)]
pub(crate) struct PolicyMa {
    inner: MaPolicy,
}

impl PolicyMa {
    pub(crate) fn ingest_evidence(
        &mut self,
        r: &v2xw_threat::MisbehaviourReport,
        at: v2xw_core::time::SimTime,
    ) -> Option<v2xw_threat::MaAction> {
        self.inner.ingest_evidence(r, at)
    }

    /// [`PolicyMa::ingest_evidence`] with what the MA learns about the subject.
    pub(crate) fn ingest_linked(
        &mut self,
        r: &v2xw_threat::MisbehaviourReport,
        at: v2xw_core::time::SimTime,
        hint: &SubjectHint<'_>,
    ) -> Option<v2xw_threat::MaAction> {
        let mut ev = PolicyEvidence::of_report(r, at);
        ev.device = Some(hint.link);
        ev.issuer = hint.issuer;
        if let Some(p) = hint.secmark_phase_ms {
            ev.secmark_phase_ms = p;
        }
        self.inner
            .ingest(&ev)
            .map(|d| v2xw_threat::MaAction::Revoke { subject: d.subject })
    }

    pub(crate) fn subjects(&self) -> usize {
        self.inner.subjects()
    }

    pub(crate) fn corroborated_events(&self, subject: &str) -> Option<u32> {
        self.inner.corroborated_events(subject)
    }

    pub(crate) fn peak_unrevoked_events(&self) -> Option<u32> {
        self.inner.peak_unrevoked_events()
    }

    pub(crate) fn trust_infrastructure(&mut self, digest: String) {
        self.inner.trust_infrastructure(digest);
    }

    /// The fields it runs.
    #[cfg(test)]
    pub(crate) fn params(&self) -> &MaPolicyParams {
        self.inner.params()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scenario(scms: Value, detection_ma: bool) -> Scenario {
        let mut s = Scenario::minimal();
        s.security.scms = Some(serde_json::from_value(scms).expect("the block parses"));
        if detection_ma {
            s.detection.ma = Some(crate::scenario::ModelChoice {
                id: crate::phase2::MA_CORROBORATED.to_string(),
                params: Value::Null,
            });
        }
        s
    }

    fn refused(scms: Value) -> Vec<String> {
        refusals(&scenario(scms, false))
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    fn hosted(scms: Value) -> Option<MaPolicyParams> {
        host(&scenario(scms, false))
            .expect("hosts")
            .map(|p| p.params().clone())
    }

    /// Every preset and every field loads; today's MA (the default preset, `custom` and
    /// `corroborated-auto`, whose rule is today's and whose decision is the run's) is
    /// hosted as today's MA; anything else as the policy, with the written field over
    /// the preset's.
    #[test]
    fn presets_and_overrides_resolve() {
        for p in PRESET_IDS {
            assert!(
                refused(json!({"ma_policy": {"preset": p}})).is_empty(),
                "{p}"
            );
        }
        for today in [
            json!({}),
            json!({"ma_policy": {}}),
            json!({"ma_policy": {"preset": "cv-pilot-manual"}}),
            json!({"ma_policy": {"preset": "custom"}}),
            json!({"ma_policy": {"preset": "corroborated-auto"}}),
            json!({"ma_policy": {"threshold": {"min_events": 3}}}),
        ] {
            assert!(hosted(today.clone()).is_none(), "{today} is today's MA");
        }
        let k1 = hosted(json!({"ma_policy": {"preset": "pseudonym-k",
            "threshold": {"min_distinct_reporters": 1}}}))
        .expect("a policy");
        assert_eq!(
            (k1.preset.as_str(), k1.min_distinct_reporters, k1.defence),
            ("pseudonym-k", 1, false)
        );
        assert!(!k1.sound());
        let full = hosted(json!({"ma_policy": {
            "preset": "custom",
            "identity": "device",
            "evidence": {"rules": {"MaxSpeed": {"score": 0.0},
                                   "detect/positionJump": {"score": 2.5}},
                         "accuracy_weighting": "catch", "tau_m": 2.0,
                         "event_window_s": 4.0, "event_min_reporters": 3},
            "threshold": {"min_events": 2, "min_score": 4.0, "min_reports": 5,
                          "min_distinct_reporters": 4, "window_s": 3600.0},
            "decay": {"mode": "daily-multiplier", "factor": 0.8, "hour_et": 6},
            "population_guard": {"mode": "cluster", "c": 5, "m": 2},
            "defence": {"trusted_reporters": false, "reputation_max": 10,
                        "report_budget": 20},
            "decision": {"mode": "manual-review", "review_s": 3600.0}}}))
        .expect("a policy");
        assert_eq!(full.identity, Identity::Device);
        assert_eq!(full.scores.get("MaxSpeed"), Some(&0.0));
        assert_eq!(full.accuracy, Accuracy::Catch { tau_m: 2.0 });
        assert_eq!(
            (
                full.event_window_s,
                full.event_min_reporters,
                full.min_events
            ),
            (4.0, 3, 2)
        );
        assert_eq!(
            (
                full.min_score,
                full.min_reports,
                full.min_distinct_reporters
            ),
            (4.0, 5, 4)
        );
        assert_eq!(
            full.decay,
            Decay::DailyMultiplier {
                factor: 0.8,
                hour_et: 6
            }
        );
        assert_eq!(full.guard, PopulationGuard::Cluster { c: 5, m: 2 });
        assert_eq!(
            (full.defence, full.reputation_max, full.report_budget),
            (false, 10, 20)
        );
        assert_eq!(full.decision, Decision::ManualReview { review_s: 3600.0 });
        // A named policy, through the root's preset.
        let named = hosted(json!({
            "ma_policies": {"strict-k5": {"preset": "pseudonym-k",
                                          "threshold": {"min_distinct_reporters": 5}}},
            "ma_policy": {"preset": "strict-k5", "threshold": {"window_s": 600.0}}}))
        .expect("a policy");
        assert_eq!(
            (
                named.preset.as_str(),
                named.min_distinct_reporters,
                named.window_s
            ),
            ("pseudonym-k", 5, 600.0)
        );
    }

    /// 08 §7 accept test 6 at the unit level, and the field-mode refusals: each names its
    /// path.
    #[test]
    fn conflicts_are_refused_at_their_path() {
        let cases = [
            (
                json!({"ma_policy": {"preset": "strict"}}),
                "security.scms.ma_policy.preset",
                "is not a preset",
            ),
            (
                json!({"ma_policies": {"a": {"preset": "b"}, "b": {}}}),
                "security.scms.ma_policies.a.preset",
                "a named policy starts from a built-in preset",
            ),
            (
                json!({"ma_policies": {"catch": {}}}),
                "security.scms.ma_policies.catch",
                "is a preset's id",
            ),
            (
                json!({"deployment": {"providers": [{"name": "provider-b",
                                                      "ma_policy": "nonesuch"}]}}),
                "security.scms.deployment.providers.provider-b.ma_policy",
                "names neither a preset nor a policy",
            ),
            (
                json!({"ma_policy": {"preset": "legacy-window",
                                     "threshold": {"min_events": 2}}}),
                "security.scms.ma_policy.threshold.min_events",
                "does not apply under the legacy persistence gate",
            ),
            (
                json!({"ma_policy": {"evidence": {"tau_m": 2.0}}}),
                "security.scms.ma_policy.evidence.tau_m",
                "applies only when accuracy_weighting is catch",
            ),
            (
                json!({"ma_policy": {"decay": {"mode": "half-life", "factor": 0.5},
                                     "threshold": {"min_score": 2.0}}}),
                "security.scms.ma_policy.decay.factor",
                "applies only when decay.mode is daily-multiplier",
            ),
            (
                json!({"ma_policy": {"decay": {"mode": "half-life"}}}),
                "security.scms.ma_policy.decay.mode",
                "would change nothing",
            ),
            (
                json!({"ma_policy": {"population_guard": {"c": 3}}}),
                "security.scms.ma_policy.population_guard.c",
                "applies only when population_guard.mode is cluster",
            ),
            (
                json!({"ma_policy": {"decision": {"mode": "automatic", "review_s": 60.0}}}),
                "security.scms.ma_policy.decision.review_s",
                "applies only when decision.mode is manual-review",
            ),
            (
                json!({"ma_policy": {"decision": {"mode": "automatic"}},
                       "lifecycle": {"ma_review_s": 60.0}}),
                "security.scms.ma_policy.decision",
                "both set the operator's review",
            ),
            (
                json!({"ma_policies": {"auto": {"decision": {"mode": "automatic"}}},
                       "deployment": {"providers": [{"name": "provider-b",
                           "ma_policy": "auto", "ma_mode": "manual-review"}]}}),
                "security.scms.deployment.providers.provider-b.ma_policy",
                "this provider's ma_mode sets it too",
            ),
        ];
        for (scms, path, why) in cases {
            let got = refused(scms.clone());
            assert!(
                got.iter().any(|m| m.starts_with(path) && m.contains(why)),
                "{scms} must be refused at {path} ({why}); got {got:#?}"
            );
        }
        // A provider naming a preset or a named policy loads.
        assert!(
            refused(
                json!({"ma_policies": {"strict-k5": {"preset": "pseudonym-k"}},
                "deployment": {"providers": [{"name": "provider-a", "ma_policy": "catch"},
                                             {"name": "provider-b", "ma_policy": "strict-k5"}]}})
            )
            .is_empty()
        );
    }

    /// The run refuses without the loader: `phase2::MaHost::from_scenario` asks [`host`]
    /// first; the default preset hosts today's corroborating MA and another preset the
    /// policy.
    #[test]
    fn the_run_hosts_the_policy_and_refuses_without_the_loader() {
        let host_of = |scms: Value| crate::phase2::MaHost::from_scenario(&scenario(scms, false));
        let today = host_of(json!({"ma_policy": {"preset": "cv-pilot-manual"}}))
            .expect("the default preset hosts");
        assert!(
            matches!(today, crate::phase2::MaHost::Corroborated(_)),
            "{today:?}"
        );
        let catch = host_of(json!({"ma_policy": {"preset": "catch"}})).expect("hosts");
        assert!(
            matches!(catch, crate::phase2::MaHost::Policy(_)),
            "{catch:?}"
        );
        for scms in [
            json!({"ma_policy": {"preset": "nonesuch"}}),
            json!({"ma_policy": {"threshold": {"min_distinct_reporters": 0}}}),
        ] {
            let got = host_of(scms.clone());
            assert!(got.is_err(), "{scms}: the run must refuse it; got {got:?}");
        }
    }

    /// Each menu offers exactly its `_IDS` constant, in order, so the loader's set and the
    /// form's cannot drift apart; every value is available now.
    #[test]
    fn menus_offer_exactly_their_ids() {
        let vals = |o: &[Opt]| o.iter().map(|x| x.value).collect::<Vec<_>>();
        assert_eq!(vals(PRESETS), PRESET_IDS);
        assert_eq!(vals(IDENTITY), IDENTITY_IDS);
        assert_eq!(vals(ACCURACY), ACCURACY_WEIGHTING_IDS);
        assert_eq!(vals(DECAY), DECAY_IDS);
        assert_eq!(vals(GUARD), POPULATION_GUARD_IDS);
        assert_eq!(vals(DECISION), DECISION_IDS);
        for o in [PRESETS, IDENTITY, ACCURACY, DECAY, GUARD, DECISION] {
            assert!(o.iter().all(|x| x.available), "{:?}", vals(o));
        }
    }

    /// A malformed value is refused as malformed: an out-of-range number, a value outside
    /// a closed menu, an unknown rule key; and `detection.ma` together with a policy.
    #[test]
    fn malformed_values_are_named_as_such() {
        let range = refused(json!({"ma_policy": {"threshold": {"min_distinct_reporters": 0}}}));
        assert!(
            range
                .iter()
                .any(|m| m.contains("outside the allowed range")),
            "{range:#?}"
        );
        let menu = refused(json!({"ma_policy": {"identity": "vehicle"}}));
        assert!(
            menu.iter()
                .any(|m| m.contains("allowed: pseudonym, device")),
            "{menu:#?}"
        );
        let key = refused(json!({"ma_policy": {"evidence": {"rules": {"Speeding": {}}}}}));
        assert!(
            key.iter().any(|m| m.contains("'Speeding' is not a key")),
            "{key:#?}"
        );
        assert!(
            refused(json!({"ma_policy": {"evidence": {"rules": {
                "detect/mapOffRoad": {"score": 2.0}}}}}))
            .is_empty()
        );
        let both = refusals(&scenario(
            json!({"ma_policy": {"preset": "cv-pilot-manual"}}),
            true,
        ));
        assert!(
            both.iter()
                .any(|m| m.to_string().contains("detection.ma both choose")),
            "{both:#?}"
        );
    }
}
