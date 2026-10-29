//! `security.scms.ma_policy`: a declarative, pluggable MA decision policy
//! (docs/design/scms-real/08-settings-ux-and-gaps.md §7). **Scaffolding (piece A0).**
//!
//! What is here: the keys' menus, ranges, titles and status, and the refusal. Every value
//! but the default is refused with "not built yet", so nothing half-works: the default,
//! `preset: cv-pilot-manual`, is by definition today's MA (`CorroborationParams::default()`
//! under the run's 8 h manual review), and the run then takes today's path unchanged.
//!
//! Pieces B1–B3 build the policy (`v2xw_threat::ma_policy`), wire it through
//! [`crate::phase2`]'s `MaHost::Policy` arm (the call is already in place:
//! [`host`]), and make each key live by flipping its row in [`KEY_STATUS`] and its
//! refusal in [`refusals`], never by editing the scenario tables (§11).

use serde_json::{Value, json};

use crate::error::{EngineError, Result, ScenarioError};
use crate::scenario::Scenario;
use crate::scenario::module_keys::{self, KeyWords, Opt};
use crate::scenario::validate::{Bound, KeyStatus, Status};

/// The model id the policy reports as, once built.
pub const MODEL_ID: &str = "threat/ma/policy";

/// The preset a policy starts from when it names none: today's MA.
pub const DEFAULT_PRESET: &str = "cv-pilot-manual";

/// The family's presets (§7 table). The last five come from `v2xw ma-replay`'s rules.
pub const PRESET_IDS: [&str; 8] = [
    "cv-pilot-manual",
    "corroborated-auto",
    "legacy-window",
    "pseudonym-k",
    "device-k",
    "catch",
    "cluster",
    "custom",
];

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
/// The keys of `evidence.rules`: the SM-MBR v1.0 report subtypes the US device files
/// (`v2xw_threat::us_mbr`).
pub const RULE_SUBTYPE_IDS: [&str; 5] = [
    "RandomPosition",
    "ConstantPosition",
    "MaxSpeed",
    "MaxAcceleration",
    "BrakeAcceleration",
];

/// Where the refusal points the reader.
const PIECE: &str = "docs/design/scms-real/08-settings-ux-and-gaps.md §7, pieces B1-B2";

const ROOT: &str = "security.scms.ma_policy";
const FAMILY: &str = "security.scms.ma_policies";
const PROVIDER: &str = "security.scms.deployment.providers[].ma_policy";

/// The keys' implementation status. `validate::status_of` reads these beside its own
/// table, longest match winning.
pub static KEY_STATUS: &[KeyStatus] = &[
    KeyStatus {
        path: ROOT,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §7, pieces B1-B2). Only preset \
               cv-pilot-manual loads, and it is today's default MA (reports corroborated \
               into events, then an operator's review); every other value is refused.",
    },
    KeyStatus {
        path: FAMILY,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §7, piece B2): any named policy is \
               refused.",
    },
    KeyStatus {
        path: PROVIDER,
        status: Status::Refused,
        note: "Not built yet (08-settings-ux-and-gaps §7, piece B2): a provider's own MA \
               policy is refused; every provider runs the deployment's MA.",
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

/// The presets' menu. Only today's MA is available until B1-B2 build the rest.
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
                      the evidence suffices. Not built yet (08 §7, piece B1).",
        available: false,
    },
    Opt {
        value: "legacy-window",
        label: "Legacy persistence gate",
        description: "The legacy corpus's rule, kept for parity with its results. Not built \
                      yet as a policy (08 §7, piece B1); detection.ma offers it today.",
        available: false,
    },
    Opt {
        value: "pseudonym-k",
        label: "k reporters per certificate",
        description: "Revokes a pseudonym certificate once k different reporters accused \
                      it; k = 1 lets one report revoke (unsound). Not built yet (08 §7, \
                      piece B1).",
        available: false,
    },
    Opt {
        value: "device-k",
        label: "k reporters per device",
        description: "Revokes a whole device, linked through the linkage authorities, once \
                      k different reporters accused any of its certificates within a \
                      window. Not built yet (08 §7, piece B1).",
        available: false,
    },
    Opt {
        value: "catch",
        label: "Accuracy-aware (CaTch-style)",
        description: "Each report counts only the share of its position error the accused's \
                      own stated accuracy cannot explain (Kamel et al., IEEE WCNC 2019). Not \
                      built yet (08 §7, piece B1).",
        available: false,
    },
    Opt {
        value: "cluster",
        label: "Population guard",
        description: "When many devices of one class are accused at once, a device of that \
                      class needs several times the reporters, so one product line's benign \
                      trigger cannot revoke the line. Not built yet (08 §7, piece B1).",
        available: false,
    },
    Opt {
        value: "custom",
        label: "Custom",
        description: "Every field is yours; nothing comes from a preset. Not built yet (08 \
                      §7, piece B1).",
        available: false,
    },
];

static IDENTITY: &[Opt] = &[
    Opt {
        value: "pseudonym",
        label: "Per certificate",
        description: "Each pseudonym certificate is its own case, as an MA sees reports \
                      before the linkage authorities join them. Not built yet (08 §7, piece \
                      B1).",
        available: false,
    },
    Opt {
        value: "device",
        label: "Per device, through the linkage authorities",
        description: "All certificates of one device are one case, joined through the \
                      linkage authorities' link values. Not built yet (08 §7, piece B1).",
        available: false,
    },
];

static ACCURACY: &[Opt] = &[
    Opt {
        value: "none",
        label: "None",
        description: "Every report counts in full, whatever accuracy the accused stated. \
                      Not built yet (08 §7, piece B1).",
        available: false,
    },
    Opt {
        value: "catch",
        label: "Discount what stated accuracy explains",
        description: "A position report counts only the deviation the accused's own stated \
                      accuracy cannot explain (CaTch). Not built yet (08 §7, piece B1).",
        available: false,
    },
];

static DECAY: &[Opt] = &[
    Opt {
        value: "none",
        label: "None",
        description: "Evidence keeps its weight until it leaves the window. Not built yet \
                      (08 §7, piece B1).",
        available: false,
    },
    Opt {
        value: "daily-multiplier",
        label: "Daily multiplier (SCMS Manager MBR v1.0)",
        description: "Once a day, at the hour set, every piece of evidence is multiplied by \
                      the factor, as the US device's report store does. Not built yet (08 \
                      §7, piece B1).",
        available: false,
    },
    Opt {
        value: "half-life",
        label: "Half-life",
        description: "Evidence loses half its weight every half-life, continuously. Not \
                      built yet (08 §7, piece B1).",
        available: false,
    },
];

static GUARD: &[Opt] = &[
    Opt {
        value: "off",
        label: "Off",
        description: "Every device needs the same evidence. Not built yet (08 §7, piece B1).",
        available: false,
    },
    Opt {
        value: "cluster",
        label: "Cluster guard",
        description: "A device in a class with many accused devices needs m times the \
                      reporters (the ma-replay cluster rule). Not built yet (08 §7, piece \
                      B1).",
        available: false,
    },
];

static DECISION: &[Opt] = &[
    Opt {
        value: "manual-review",
        label: "Operator review (CV pilots)",
        description: "An operator reviews each case before the device is added to the \
                      revocation list (CAMP122 §4.2-4.3). Not built yet as a policy field \
                      (08 §7, piece B2); providers[].ma_mode offers it today.",
        available: false,
    },
    Opt {
        value: "automatic",
        label: "Automatic",
        description: "The rule revokes the moment the evidence suffices. Not built yet as a \
                      policy field (08 §7, piece B2); providers[].ma_mode offers it today.",
        available: false,
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
    let ids: &'static [&'static str] = &RULE_SUBTYPE_IDS;
    template.ends_with(".evidence.rules").then_some(ids)
}

/// Every refusal of the MA-policy keys, in document order.
pub fn refusals(s: &Scenario) -> Vec<ScenarioError> {
    let mut e = Vec::new();
    let Some(scms) = s.security.scms.as_ref() else {
        return e;
    };
    if let Some(p) = &scms.ma_policy {
        if s.detection.ma.is_some() {
            e.push(ScenarioError::conflict(
                ROOT,
                "and detection.ma both choose the MA's decision rule; remove one",
            ));
        }
        let v = serde_json::to_value(p).unwrap_or(Value::Null);
        let maps = [format!("{ROOT}.evidence.rules")];
        let maps: Vec<&str> = maps.iter().map(String::as_str).collect();
        for leaf in module_keys::leaves(&v, ROOT, ROOT, &maps, &map_keys, &mut e) {
            if let Some(err) = module_keys::malformed(&leaf) {
                e.push(err);
            } else if !(leaf.template == format!("{ROOT}.preset") && leaf.value == DEFAULT_PRESET) {
                e.push(module_keys::not_built(
                    &leaf,
                    PIECE,
                    "leave it unset; preset cv-pilot-manual alone is today's MA",
                ));
            }
        }
    }
    for (name, p) in &scms.ma_policies {
        let at = format!("{FAMILY}.{name}");
        let template = format!("{FAMILY}.*");
        let v = serde_json::to_value(p).unwrap_or(Value::Null);
        let maps = [format!("{template}.evidence.rules")];
        let maps: Vec<&str> = maps.iter().map(String::as_str).collect();
        let before = e.len();
        for leaf in module_keys::leaves(&v, &at, &template, &maps, &map_keys, &mut e) {
            e.push(module_keys::malformed(&leaf).unwrap_or_else(|| {
                module_keys::not_built(&leaf, PIECE, "remove the named policy")
            }));
        }
        if e.len() == before {
            e.push(ScenarioError::conflict(
                &at,
                format!("is a named MA policy: not built yet ({PIECE}); remove it"),
            ));
        }
    }
    if let Some(d) = &scms.deployment {
        for p in &d.providers {
            if let Some(m) = &p.ma_policy {
                let leaf = module_keys::Leaf {
                    at: format!("security.scms.deployment.providers.{}.ma_policy", p.name),
                    template: PROVIDER.to_string(),
                    value: json!(m),
                };
                e.push(module_keys::not_built(
                    &leaf,
                    PIECE,
                    "leave it unset; every provider runs the deployment's MA",
                ));
            }
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

/// The policy the run's MA hosts, for `phase2::MaHost::from_scenario`: `None` runs
/// today's MA, which is what the default preset is.
///
/// # Errors
/// The first refusal, so a run built without the loader is refused the same way.
pub(crate) fn host(s: &Scenario) -> Result<Option<PolicyMa>> {
    match refusals(s).into_iter().next() {
        Some(err) => Err(EngineError::Scenario(err)),
        None => Ok(None),
    }
}

/// The hosted policy. It has no value until B2 builds one, so every arm of
/// `phase2::MaHost::Policy` is unreachable by construction rather than by a check.
#[derive(Debug, Clone)]
pub(crate) enum PolicyMa {}

impl PolicyMa {
    pub(crate) fn ingest_evidence(
        &mut self,
        _r: &v2xw_threat::MisbehaviourReport,
        _at: v2xw_core::time::SimTime,
    ) -> Option<v2xw_threat::MaAction> {
        match *self {}
    }

    pub(crate) fn subjects(&self) -> usize {
        match *self {}
    }

    pub(crate) fn corroborated_events(&self, _subject: &str) -> Option<u32> {
        match *self {}
    }

    pub(crate) fn peak_unrevoked_events(&self) -> Option<u32> {
        match *self {}
    }

    pub(crate) fn trust_infrastructure(&mut self, _digest: String) {
        match *self {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Every MA-policy key refuses any value but the default, naming its path, and the
    /// default loads with no refusal and runs today's MA.
    #[test]
    fn every_non_default_value_is_refused() {
        assert!(refused(json!({})).is_empty());
        assert!(refused(json!({"ma_policy": {}})).is_empty());
        assert!(refused(json!({"ma_policy": {"preset": "cv-pilot-manual"}})).is_empty());
        let cases = [
            (
                json!({"ma_policy": {"preset": "pseudonym-k"}}),
                "security.scms.ma_policy.preset",
            ),
            (
                json!({"ma_policy": {"identity": "device"}}),
                "security.scms.ma_policy.identity",
            ),
            (
                json!({"ma_policy": {"evidence": {"rules": {"MaxSpeed": {"score": 2.0}}}}}),
                "security.scms.ma_policy.evidence.rules.MaxSpeed.score",
            ),
            (
                json!({"ma_policy": {"threshold": {"min_distinct_reporters": 3}}}),
                "security.scms.ma_policy.threshold.min_distinct_reporters",
            ),
            (
                json!({"ma_policy": {"decay": {"mode": "none"}}}),
                "security.scms.ma_policy.decay.mode",
            ),
            (
                json!({"ma_policy": {"population_guard": {"c": 3}}}),
                "security.scms.ma_policy.population_guard.c",
            ),
            (
                json!({"ma_policy": {"defence": {"trusted_reporters": true}}}),
                "security.scms.ma_policy.defence.trusted_reporters",
            ),
            (
                json!({"ma_policy": {"decision": {"mode": "automatic"}}}),
                "security.scms.ma_policy.decision.mode",
            ),
            (
                json!({"ma_policies": {"strict-k5": {}}}),
                "security.scms.ma_policies.strict-k5",
            ),
            (
                json!({"ma_policies": {"strict-k5": {"preset": "cv-pilot-manual"}}}),
                "security.scms.ma_policies.strict-k5.preset",
            ),
            (
                json!({"deployment": {"providers": [{"name": "provider-b", "ma_policy": "catch"}]}}),
                "security.scms.deployment.providers.provider-b.ma_policy",
            ),
        ];
        for (scms, path) in cases {
            let got = refused(scms.clone());
            assert!(
                got.iter()
                    .any(|m| m.starts_with(path) && m.contains("not built yet")),
                "{scms} must be refused at {path} as not built yet; got {got:#?}"
            );
        }
    }

    /// The run refuses them too, without the loader: `phase2::MaHost::from_scenario`
    /// asks [`host`] first, and the default preset hosts today's corroborating MA.
    #[test]
    fn the_run_refuses_without_the_loader() {
        let host_of = |scms: Value| crate::phase2::MaHost::from_scenario(&scenario(scms, false));
        let today = host_of(json!({"ma_policy": {"preset": "cv-pilot-manual"}}))
            .expect("the default preset hosts");
        assert!(
            matches!(today, crate::phase2::MaHost::Corroborated(_)),
            "{today:?}"
        );
        for scms in [
            json!({"ma_policy": {"preset": "catch"}}),
            json!({"ma_policies": {"strict-k5": {}}}),
        ] {
            let got = host_of(scms.clone());
            assert!(
                got.as_ref()
                    .is_err_and(|e| e.to_string().contains("not built yet")),
                "{scms}: the run must refuse it; got {got:?}"
            );
        }
    }

    /// Each menu offers exactly its `_IDS` constant, in order, so the loader's set and the
    /// form's cannot drift apart.
    #[test]
    fn menus_offer_exactly_their_ids() {
        let vals = |o: &[Opt]| o.iter().map(|x| x.value).collect::<Vec<_>>();
        assert_eq!(vals(PRESETS), PRESET_IDS);
        assert_eq!(vals(IDENTITY), IDENTITY_IDS);
        assert_eq!(vals(ACCURACY), ACCURACY_WEIGHTING_IDS);
        assert_eq!(vals(DECAY), DECAY_IDS);
        assert_eq!(vals(GUARD), POPULATION_GUARD_IDS);
        assert_eq!(vals(DECISION), DECISION_IDS);
    }

    /// A malformed value is refused as malformed, not as unbuilt: an out-of-range number,
    /// a value outside a closed menu, an unknown report subtype; and `detection.ma`
    /// together with a policy.
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
