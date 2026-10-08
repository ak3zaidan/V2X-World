//! Traffic control: what the gridlock resolution and the actuated signals did.
//!
//! The engine logs both on `traffic.control` (`v2xw_engine::actuated`), cumulatively; this
//! provider reports the last values:
//!
//! | Metric | What | Unit |
//! |---|---|---|
//! | `traffic_jammed` | vehicles that stood still for the whole gridlock timeout | count |
//! | `traffic_teleports` | of those, moved on along their route (SUMO's teleport) | count |
//! | `traffic_removed` | of those, removed: no room anywhere on the rest of the route | count |
//! | `signal_gap_outs` | actuated greens ended early because the detectors went quiet | count |
//! | `signal_max_outs` | actuated greens ended at their maximum with traffic still coming | count |
//!
//! A teleport is a jam the network did not clear by itself: SUMO prints a warning for
//! every one, and a run with many of them has demand its network cannot carry, or a
//! network fault. The health band says so.
//!
//! # Visibility
//!
//! Public: a traffic management centre's own log.

use v2xw_core::card::ModelCard;
use v2xw_core::ctx::{ChannelName, EventRecord, Visibility};
use v2xw_core::model::Model;
use v2xw_core::time::SimTime;

use crate::cards;
use crate::def::{Agg, Dim, Dims, MetricDef, MetricSample, SampleValue};
use crate::health::{Basis, Better, HealthBand};
use crate::provider::MetricProvider;
use crate::quant::Quantum;

/// The metric group name a scenario's `metrics` list may give to ask for these.
pub const GROUP: &str = "traffic";

/// The record channel read.
pub const CHANNEL: &str = "traffic.control";

/// The names, in report order, with the record field each reads.
const FIELDS: [(&str, &str); 5] = [
    ("traffic_jammed", "jammed"),
    ("traffic_teleports", "teleported"),
    ("traffic_removed", "removed"),
    ("signal_gap_outs", "gap_outs"),
    ("signal_max_outs", "max_outs"),
];

/// The traffic-control provider.
pub struct TrafficControlProvider {
    card: ModelCard,
    last: Option<[u64; 5]>,
    rejected: u64,
}

impl Default for TrafficControlProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl TrafficControlProvider {
    /// A new provider.
    #[must_use]
    pub fn new() -> Self {
        let mut card = cards::provider_card(
            "metric/traffic/control",
            "1.0.0",
            "Gridlock resolution (jams, teleports, removals) and actuated signal decisions \
             (gap-outs, max-outs), from the engine's traffic-control log.",
        );
        card.sources = vec![cards::design(
            "SUMO user documentation, \"Why Vehicles are teleporting\"; FHWA Traffic Signal \
             Timing Manual (2008) ch. 5",
        )];
        card.validation.tests = vec!["traffic::tests::the_last_record_is_reported".to_string()];
        Self {
            card,
            last: None,
            rejected: 0,
        }
    }

    /// The definitions, in [`FIELDS`] order.
    #[must_use]
    pub fn definitions() -> Vec<MetricDef> {
        let src = cards::design("v2xw_engine::actuated (traffic.control)");
        let what = [
            "Vehicles that stood still for the whole gridlock timeout: each one is a jam the \
             network did not clear by itself.",
            "Vehicles the gridlock resolution moved on along their route, as SUMO's \
             teleport does.",
            "Vehicles the gridlock resolution removed, because no lane on the rest of their \
             route had room.",
            "Actuated greens ended early because no vehicle reached the detectors within \
             the passage time while another approach waited.",
            "Actuated greens ended at their maximum while vehicles were still arriving.",
        ];
        FIELDS
            .iter()
            .zip(what)
            .map(|((name, _), about)| {
                let d = MetricDef::new(
                    *name,
                    "count",
                    Agg::Count,
                    Visibility::Public,
                    Quantum::COUNT,
                    about,
                )
                .with_dims([Dim::T])
                .with_source(src.clone())
                .with_min_samples(1)
                .not_accounting_for(
                    "a jam shorter than the gridlock timeout, and a vehicle the timeout is \
                     off for (0 s); a pedestrian's signal call, which the detectors do not see",
                );
                if *name == "traffic_teleports" {
                    d.with_health(HealthBand {
                        better: Better::Lower,
                        judged: String::new(),
                        caution: 0.0,
                        unhealthy: 10.0,
                        healthy_note: "no vehicle was stuck long enough to be moved on".into(),
                        caution_note: "a few vehicles were stuck past the gridlock timeout: \
                                       look for a blocked junction"
                            .into(),
                        unhealthy_note: "more than 10 vehicles were stuck past the gridlock \
                                         timeout: the demand is more than this network \
                                         carries, or the network has a fault"
                            .into(),
                        source: "engineering choice: SUMO warns of every teleport; a \
                                 calibrated run should have none"
                            .into(),
                        basis: Basis::Engineering,
                    })
                } else {
                    d
                }
            })
            .collect()
    }
}

impl Model for TrafficControlProvider {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl MetricProvider for TrafficControlProvider {
    fn defs(&self) -> Vec<MetricDef> {
        Self::definitions()
    }

    fn subscribe(&self) -> Vec<ChannelName> {
        vec![ChannelName(CHANNEL)]
    }

    fn on_event(&mut self, ev: &EventRecord) {
        if ev.channel != CHANNEL {
            return;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&ev.json) else {
            self.rejected += 1;
            return;
        };
        let mut row = [0u64; 5];
        for (k, (_, field)) in FIELDS.iter().enumerate() {
            match v[*field].as_u64() {
                Some(x) => row[k] = x,
                None => {
                    self.rejected += 1;
                    return;
                }
            }
        }
        self.last = Some(row);
    }

    fn flush(&mut self, at: SimTime) -> Vec<MetricSample> {
        let Some(row) = self.last else {
            return Vec::new();
        };
        let defs = Self::definitions();
        defs.iter()
            .zip(row)
            .map(|(d, count)| MetricSample::new(d, at, Dims::new(), SampleValue::Count { count }))
            .collect()
    }

    fn rejected(&self) -> u64 {
        self.rejected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::ctx::OwnedRecord;

    #[test]
    fn the_last_record_is_reported() {
        let mut p = TrafficControlProvider::new();
        for (j, t) in [(1u64, 0u64), (3, 2)] {
            p.on_event(&OwnedRecord {
                channel: CHANNEL,
                visibility: Visibility::Public,
                json: serde_json::to_vec(&serde_json::json!({
                    "jammed": j, "teleported": t, "removed": 0, "gap_outs": 5, "max_outs": 1
                }))
                .expect("json"),
            });
        }
        let out = p.flush(10);
        let get = |n: &str| match out.iter().find(|s| s.metric == n).map(|s| &s.value) {
            Some(SampleValue::Count { count }) => *count,
            _ => u64::MAX,
        };
        assert_eq!(get("traffic_jammed"), 3);
        assert_eq!(get("traffic_teleports"), 2);
        assert_eq!(get("signal_gap_outs"), 5);
        assert_eq!(p.rejected(), 0);
    }

    #[test]
    fn every_definition_validates() {
        for d in TrafficControlProvider::definitions() {
            d.validate().unwrap_or_else(|e| panic!("{}: {e}", d.name));
        }
    }
}
