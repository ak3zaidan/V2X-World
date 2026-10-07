//! Findings beyond the analyst's nine layers, from the extra whole-run metrics the agent
//! reads after a run: misbehaviour detection, security overhead, pedestrians and the V2X
//! applications' warnings.
//!
//! The analyst ([`crate::analyst`]) judges the layers every run has. An adversarial or
//! application scenario has numbers it does not read — how many attackers' messages the
//! detectors flagged, what share of the air bytes the certificates are, how many
//! pedestrian conflicts there were — and a comparison of "baseline against an attack" with
//! no finding about the attack cannot explain itself. These findings are built the same
//! way the analyst's are: each number is the server's, each threshold is named with its
//! basis, and nothing here computes a value that goes into a run.

use std::collections::BTreeMap;

use serde_json::json;

use crate::analyst::{Evidence, Finding, Layer, Link, Location, Severity};

/// A detection recall below this means most attacker messages pass unflagged: 0.5, an
/// engineering choice.
pub const RECALL_FLOOR: f64 = 0.5;
/// A false-positive rate above this means honest vehicles are being accused: 0.01, an
/// engineering choice (one honest message in a hundred).
pub const FPR_CEILING: f64 = 0.01;

fn ev(metric: &str, v: f64, units: &BTreeMap<String, String>) -> Evidence {
    Evidence {
        label: metric.replace('_', " "),
        value: v,
        unit: units.get(metric).cloned().unwrap_or_default(),
        source: metric.to_string(),
    }
}

fn link(metric: &str) -> Link {
    Link {
        metric: metric.to_string(),
        chart: metric.split(['.', '[']).next().unwrap_or(metric).to_string(),
        query: json!({"method": "metrics.query", "params": {"metrics": [metric]}}),
    }
}

/// The findings these metrics support. `attackers` is whether the scenario declares any.
#[must_use]
pub fn findings(
    extra: &BTreeMap<String, f64>,
    headline: &BTreeMap<String, f64>,
    units: &BTreeMap<String, String>,
    attackers: bool,
) -> Vec<Finding> {
    let get = |k: &str| extra.get(k).or_else(|| headline.get(k)).copied();
    let mut out = Vec::new();

    // A run in which nothing reached the air: every radio result is then empty, and a
    // comparison against it reads as "100 % less load" unless this is said first.
    let silent: Vec<&str> = ["bytes_air", "offered_load"]
        .into_iter()
        .filter(|k| get(k) == Some(0.0))
        .collect();
    if !silent.is_empty() {
        out.push(Finding {
            id: "simulation.silent".into(),
            layer: Layer::Simulation,
            severity: Severity::Critical,
            title: "No frame reached the air".into(),
            detail: "The run transmitted nothing, so its channel, delivery, latency and \
                     security results are empty rather than good. Look for what stops a node \
                     sending: a signature or certificate its hardware cannot produce, a message \
                     too large for the radio without fragmentation, no equipped vehicle."
                .into(),
            evidence: silent.iter().map(|k| ev(k, 0.0, units)).collect(),
            location: Location::default(),
            reference: None,
            links: silent.iter().map(|k| link(k)).collect(),
        });
    }

    // Detection.
    let det: Vec<&str> = ["det_recall", "det_precision", "det_fpr", "det_tp", "det_fp", "det_fn",
                          "time_to_detect", "time_to_decision"]
        .into_iter()
        .filter(|k| get(k).is_some())
        .collect();
    if !det.is_empty() && (attackers || get("det_fp").is_some_and(|v| v > 0.0)) {
        let recall = get("det_recall");
        let fpr = get("det_fpr");
        let missed = recall.is_some_and(|r| r < RECALL_FLOOR);
        let accusing = fpr.is_some_and(|f| f > FPR_CEILING);
        let severity = if missed || accusing { Severity::Warning } else { Severity::Notice };
        let title = match (missed, accusing) {
            (true, true) => "Detection misses most attacker messages and flags honest ones",
            (true, false) => "Detection misses most attacker messages",
            (false, true) => "Detection flags honest vehicles' messages",
            _ => "Detection caught the attackers' messages",
        };
        out.push(Finding {
            id: "security.detection".into(),
            layer: Layer::Security,
            severity,
            title: title.into(),
            detail: format!(
                "The misbehaviour detectors' score against ground truth: recall is the share of \
                 attacker messages flagged, precision the share of flags that were attackers, \
                 the false-positive rate the share of honest messages flagged. Judged against \
                 recall {RECALL_FLOOR} and false-positive rate {FPR_CEILING} (engineering choices)."
            ),
            evidence: det.iter().filter_map(|k| get(k).map(|v| ev(k, v, units))).collect(),
            location: Location::default(),
            reference: Some(format!("recall below {RECALL_FLOOR}, false-positive rate above {FPR_CEILING} (engineering choices)")),
            links: det.iter().map(|k| link(k)).collect(),
        });
    } else if attackers && det.is_empty() {
        out.push(Finding {
            id: "security.detection-off".into(),
            layer: Layer::Security,
            severity: Severity::Notice,
            title: "Attackers run, but no detector scored them".into(),
            detail: "The scenario declares attackers and the run reports no detection metric: \
                     add detection.local (detect/legacy-12) to measure whether they are caught."
                .into(),
            evidence: Vec::new(),
            location: Location::default(),
            reference: None,
            links: Vec::new(),
        });
    }

    // Jamming: the analyst names the jammer only when it is the largest loss cause; with a
    // jammer in the scenario its share is reported whatever its rank.
    let jammed = get("lost_frames[jammed]");
    if let Some(share) = get("pdr_by_cause[jammed]").filter(|v| *v > 0.0) {
        let mut evidence = vec![ev("pdr_by_cause[jammed]", share, units)];
        if let Some(n) = jammed {
            evidence.push(Evidence {
                label: "receptions lost to jamming".into(),
                value: n,
                unit: "frames".into(),
                source: "pdr_by_cause{cause=jammed}.n".into(),
            });
        }
        out.push(Finding {
            id: "channel.jamming".into(),
            layer: Layer::Channel,
            severity: if share >= 0.3 { Severity::Warning } else { Severity::Notice },
            title: "Frames were lost to a jammer".into(),
            detail: "The share of lost receptions the run attributes to jamming energy (a \
                     threat in the scenario, not load), whether or not it is the largest \
                     loss cause."
                .into(),
            evidence,
            location: Location::default(),
            reference: Some("0.3 of losses, the analyst's dominant-cause share (engineering choice)".into()),
            links: vec![link("pdr_by_cause[jammed]")],
        });
    }

    // Security overhead.
    let over: Vec<&str> = ["envelope_overhead", "cert_bytes_share", "full_cert_share",
                           "air_bytes_per_payload_byte", "security_overhead"]
        .into_iter()
        .filter(|k| get(k).is_some())
        .collect();
    if !over.is_empty() {
        out.push(Finding {
            id: "security.overhead".into(),
            layer: Layer::Security,
            severity: Severity::Notice,
            title: "What security adds to every message".into(),
            detail: "The signed envelope's bytes against the payload's: the certificate's share \
                     of the air bytes and how often a full certificate rides along."
                .into(),
            evidence: over.iter().filter_map(|k| get(k).map(|v| ev(k, v, units))).collect(),
            location: Location::default(),
            reference: None,
            links: over.iter().map(|k| link(k)).collect(),
        });
    }

    // Pedestrians.
    let ped: Vec<&String> = extra.keys().filter(|k| k.starts_with("ped_")).take(6).collect();
    if !ped.is_empty() {
        let conflicts = ped.iter().filter(|k| k.contains("conflict") || k.contains("near_miss"))
            .any(|k| extra.get(*k).is_some_and(|v| *v > 0.0));
        out.push(Finding {
            id: "traffic.pedestrians".into(),
            layer: Layer::Traffic,
            severity: if conflicts { Severity::Warning } else { Severity::Notice },
            title: if conflicts { "Pedestrians and vehicles came into conflict" } else { "Pedestrians crossed without measured conflicts" }.into(),
            detail: "The pedestrian metrics of the run (crossings, conflicts, near misses, \
                     mid-block crossings), as the server measured them."
                .into(),
            evidence: ped.iter().filter_map(|k| extra.get(*k).map(|v| ev(k, *v, units))).collect(),
            location: Location::default(),
            reference: None,
            links: ped.iter().map(|k| link(k)).collect(),
        });
    }

    // Application warnings.
    let apps: Vec<&String> = extra.keys().filter(|k| k.starts_with("apps_") || k.as_str() == "warnings").take(6).collect();
    if !apps.is_empty() {
        out.push(Finding {
            id: "apps.warnings".into(),
            layer: Layer::Delivery,
            severity: Severity::Notice,
            title: "The V2X applications' warnings".into(),
            detail: "Warnings the equipped vehicles' applications raised (forward collision, \
                     electronic brake light, …) and how they scored against ground truth."
                .into(),
            evidence: apps.iter().filter_map(|k| extra.get(*k).map(|v| ev(k, *v, units))).collect(),
            location: Location::default(),
            reference: None,
            links: apps.iter().map(|k| link(k)).collect(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(rows: &[(&str, f64)]) -> BTreeMap<String, f64> {
        rows.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
    }

    #[test]
    fn a_detector_that_misses_is_a_warning_and_one_that_catches_is_a_notice() {
        let none: BTreeMap<String, f64> = BTreeMap::new();
        let units: BTreeMap<String, String> = BTreeMap::new();
        let miss = findings(&map(&[("det_recall", 0.2), ("det_fpr", 0.0)]), &none, &units, true);
        assert_eq!(miss[0].id, "security.detection");
        assert_eq!(miss[0].severity, Severity::Warning);
        let catch = findings(&map(&[("det_recall", 0.9), ("det_fpr", 0.001)]), &none, &units, true);
        assert_eq!(catch[0].severity, Severity::Notice);
        let off = findings(&none, &none, &units, true);
        assert_eq!(off[0].id, "security.detection-off");
        assert!(findings(&map(&[("det_recall", 0.9)]), &none, &units, false).is_empty(),
                "no attackers and no false positives: nothing to say about detection");
    }

    #[test]
    fn a_silent_run_is_critical_and_a_jammer_reports_its_kill_count() {
        let units = BTreeMap::new();
        let none = BTreeMap::new();
        let f = findings(&map(&[("bytes_air", 0.0)]), &none, &units, false);
        assert_eq!(f[0].id, "simulation.silent");
        assert_eq!(f[0].severity, Severity::Critical);
        assert!(findings(&map(&[("bytes_air", 10.0)]), &none, &units, false).is_empty());
        let j = findings(&map(&[("pdr_by_cause[jammed]", 0.0066), ("lost_frames[jammed]", 305.0)]), &none, &units, false);
        assert_eq!(j[0].id, "channel.jamming");
        assert!(j[0].evidence.iter().any(|e| e.value == 305.0 && e.unit == "frames"));
    }

    #[test]
    fn pedestrian_conflicts_warn() {
        let none: BTreeMap<String, f64> = BTreeMap::new();
        let units: BTreeMap<String, String> = BTreeMap::new();
        let f = findings(&map(&[("ped_conflicts", 3.0)]), &none, &units, false);
        assert_eq!(f[0].id, "traffic.pedestrians");
        assert_eq!(f[0].severity, Severity::Warning);
    }
}
