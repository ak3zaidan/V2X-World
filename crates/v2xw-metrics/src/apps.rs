//! The V2X applications' performance: how many warnings each issued, how many were right,
//! how many were missed, and how much warning the driver got.
//!
//! Every warning a vehicle issues is on `app.warning` (what the vehicle decided, from what
//! it heard); the engine labels each one against ground truth on `app.outcome` — `true`,
//! `false`, or a `missed` truth episode no warning covered (`v2xw_engine::app_truth`).
//! Four metrics reduce them, per application:
//!
//! | Metric | Formula | Unit |
//! |---|---|---|
//! | `app_warnings` | warnings issued | count |
//! | `app_precision` | true / (true + false) | ratio (Wilson interval) |
//! | `app_miss_ratio` | missed / (true + missed) | ratio (Wilson interval) |
//! | `app_lead_time_s` | the true time to collision left when a true warning issued | s, distribution |
//!
//! # Visibility
//!
//! Ground truth: the labels read both vehicles' true states.

use std::collections::BTreeMap;

use v2xw_core::card::ModelCard;
use v2xw_core::ctx::{ChannelName, EventRecord, Visibility};
use v2xw_core::model::Model;
use v2xw_core::time::SimTime;

use crate::cards;
use crate::def::{Agg, DEFAULT_LEVEL, Dim, DimValue, Dims, MetricDef, MetricSample, SampleValue};
use crate::provider::MetricProvider;
use crate::quant::Quantum;
use crate::stats::{ConfidenceLevel, Distribution, Proportion};

#[derive(Debug, Default, Clone)]
struct Sums {
    issued: u64,
    precision: Proportion,
    missed: Proportion,
    lead: Distribution,
}

/// The applications provider.
pub struct AppsProvider {
    card: ModelCard,
    level: ConfidenceLevel,
    sums: BTreeMap<String, Sums>,
    rejected: u64,
}

impl Default for AppsProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl AppsProvider {
    /// A provider at the crate's 95 % level.
    #[must_use]
    pub fn new() -> Self {
        let mut card = cards::provider_card(
            "metric/apps/warnings",
            "1.0.0",
            "The V2X applications' warnings issued, their precision and miss ratio against \
             ground truth, and the lead time a true warning gave.",
        );
        card.parameters = cards::statistics_params();
        card.sources = vec![cards::design(
            "07-threats-and-detection.md §5 (false and missed warnings must be measured)",
        )];
        card.limitations = vec![
            "A warning about an object another station reported in a CPM is counted as \
             issued and not labelled."
                .to_string(),
            "Blind-spot and lane-change advisories (bsw, lcw) are counted and neither \
             matched nor missed. rlvw is labelled against the signal the ego's approach \
             truly showed and the ego's true speed, not against whether it then entered \
             on red."
                .to_string(),
        ];
        card.validation.tests = vec!["apps::tests::labels_become_ratios".to_string()];
        Self {
            card,
            level: DEFAULT_LEVEL,
            sums: BTreeMap::new(),
            rejected: 0,
        }
    }

    fn definitions() -> Vec<MetricDef> {
        let src = cards::design("07-threats-and-detection.md §5");
        vec![
            MetricDef::new(
                "app_warnings",
                "count",
                Agg::Count,
                Visibility::Node,
                Quantum::COUNT,
                "Warnings issued, per application: the rising edges a driver would have \
                 seen, not the updates of a standing warning.",
            )
            .with_dims([Dim::T, Dim::App])
            .with_source(src.clone())
            .with_min_samples(1)
            .not_accounting_for(
                "whether the driver noticed or acted on the warning: an issued warning is \
                 the HMI's output, not the driver's response",
            ),
            MetricDef::new(
                "app_precision",
                "ratio",
                Agg::ratio("true warnings", "true + false warnings"),
                Visibility::Gt,
                Quantum::RATIO,
                "The share of labelled warnings that ground truth agrees with: the same \
                 decision function, run over the two vehicles' true states, fired within \
                 1 s of the warning.",
            )
            .with_dims([Dim::T, Dim::App])
            .with_source(src.clone())
            .with_min_samples(1)
            .with_range(0.0, 1.0)
            .not_accounting_for("warnings about objects reported in CPMs"),
            MetricDef::new(
                "app_miss_ratio",
                "ratio",
                Agg::ratio("missed episodes", "true warnings + missed episodes"),
                Visibility::Gt,
                Quantum::RATIO,
                "The share of ground-truth conflict episodes (at least 300 ms long) the \
                 vehicle never warned of.",
            )
            .with_dims([Dim::T, Dim::App])
            .with_source(src.clone())
            .with_min_samples(1)
            .with_range(0.0, 1.0)
            .not_accounting_for(
                "advisory applications (blind spot, lane change) and episodes shorter than \
                 300 ms, which are neither matched nor counted as missed",
            ),
            MetricDef::new(
                "app_lead_time_s",
                "s",
                Agg::Distribution,
                Visibility::Gt,
                Quantum::TIME_S,
                "The true time to collision left when a true warning issued: how much time \
                 the driver had.",
            )
            .with_dims([Dim::T, Dim::App])
            .with_source(src)
            .with_min_samples(1)
            .not_accounting_for(
                "applications without a time to collision (EEBL, RLVW report no lead time)",
            ),
        ]
    }

    fn on_json(&mut self, channel: &str, json: &[u8]) {
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(json) else {
            self.rejected += 1;
            return;
        };
        let Some(app) = v["app"].as_str() else {
            self.rejected += 1;
            return;
        };
        let s = self.sums.entry(app.to_string()).or_default();
        match channel {
            "app.warning" => {
                if v["kind"].as_str() == Some("issue") {
                    s.issued += 1;
                }
            }
            _ => match v["outcome"].as_str() {
                Some("true") => {
                    s.precision.observe(true);
                    s.missed.observe(false);
                    if let Some(x) = v["ttc_truth_s"].as_f64() {
                        s.lead.observe(x);
                    }
                }
                Some("false") => s.precision.observe(false),
                Some("missed") => s.missed.observe(true),
                _ => self.rejected += 1,
            },
        }
    }
}

impl Model for AppsProvider {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MetricProvider for AppsProvider {
    fn defs(&self) -> Vec<MetricDef> {
        Self::definitions()
    }

    fn subscribe(&self) -> Vec<ChannelName> {
        vec![ChannelName("app.warning"), ChannelName("app.outcome")]
    }

    fn on_event(&mut self, ev: &EventRecord) {
        if ev.channel == "app.warning" || ev.channel == "app.outcome" {
            self.on_json(ev.channel, &ev.json);
        }
    }

    fn flush(&mut self, at: SimTime) -> Vec<MetricSample> {
        let defs = Self::definitions();
        let mut out = Vec::new();
        for (app, s) in core::mem::take(&mut self.sums) {
            let mut dims = Dims::new();
            dims.insert(Dim::App, DimValue::label(app));
            out.push(MetricSample::new(
                &defs[0],
                at,
                dims.clone(),
                SampleValue::Count { count: s.issued },
            ));
            if s.precision.trials() > 0 {
                out.push(MetricSample::new(
                    &defs[1],
                    at,
                    dims.clone(),
                    SampleValue::Ratio(s.precision.estimate(1, self.level)),
                ));
            }
            if s.missed.trials() > 0 {
                out.push(MetricSample::new(
                    &defs[2],
                    at,
                    dims.clone(),
                    SampleValue::Ratio(s.missed.estimate(1, self.level)),
                ));
            }
            out.push(MetricSample::new(
                &defs[3],
                at,
                dims,
                SampleValue::Distribution(s.lead.summary(1)),
            ));
        }
        out
    }

    fn rejected(&self) -> u64 {
        self.rejected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::ctx::OwnedRecord;

    fn rec(channel: &'static str, v: serde_json::Value) -> OwnedRecord {
        OwnedRecord {
            channel,
            visibility: Visibility::NodeAndGt,
            json: serde_json::to_vec(&v).expect("json"),
        }
    }

    #[test]
    fn labels_become_ratios() {
        let mut p = AppsProvider::new();
        for _ in 0..3 {
            p.on_event(&rec(
                "app.warning",
                serde_json::json!({"app": "fcw", "kind": "issue"}),
            ));
        }
        p.on_event(&rec(
            "app.outcome",
            serde_json::json!({"app": "fcw", "outcome": "true", "ttc_truth_s": 2.1}),
        ));
        p.on_event(&rec(
            "app.outcome",
            serde_json::json!({"app": "fcw", "outcome": "false"}),
        ));
        p.on_event(&rec(
            "app.outcome",
            serde_json::json!({"app": "fcw", "outcome": "missed"}),
        ));
        let out = p.flush(1);
        let count = out.iter().find(|s| s.metric == "app_warnings").expect("count");
        assert!(matches!(count.value, SampleValue::Count { count: 3 }));
        assert!(out.iter().any(|s| s.metric == "app_precision"));
        assert!(out.iter().any(|s| s.metric == "app_miss_ratio"));
        assert_eq!(p.rejected(), 0);
    }

    /// Every definition passes the registry's own validation; one that did not stopped
    /// every scenario with `metrics: [all]` from building (the golden case found it).
    #[test]
    fn every_definition_validates() {
        for d in AppsProvider::definitions() {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.name));
        }
    }
}
