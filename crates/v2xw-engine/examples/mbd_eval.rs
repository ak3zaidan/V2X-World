//! `mbd_eval` — one misbehaviour-detection measurement: what the detectors reported, what
//! the Misbehaviour Authority decided, and whether it was right.
//!
//! ```text
//! cargo run -p v2xw-engine --example mbd_eval -- <scenario.yaml>
//!     [--rate VEH_PER_H] [--duration S] [--no-attackers]
//!     [--attack KIND] [--count N | --fraction F] [--from S]
//!     [--ma ID] [--detector ID] [--seed SEED] [--keep-metrics]
//!     [--strategy NAME] [--period S] [--silent MIN,MAX] [--sniffers F [--sniffer-range M]]
//!     [--legacy-accuracy]
//! ```
//!
//! * `--no-attackers` removes every attacker population: the honest-revocation check.
//! * `--attack KIND` replaces the first population's model with
//!   `threat/attacker/legacy/KIND`, keeping its size and schedule unless `--count`,
//!   `--fraction` or `--from` say otherwise.
//! * The metric catalogue is switched off unless `--keep-metrics`: the run report's
//!   Phase 2 counters are what this measures, and the catalogue costs wall time.
//!
//! It prints one JSON object. The precision and recall it reports are the ground-truth
//! joins of the run report (`Phase2Report`): which reports named an armed attacker, which
//! decisions were about one. Nothing in the detection path reads them.

#![recursion_limit = "256"]

use std::time::Instant;

use v2xw_engine::scenario::{Attacker, DilationWindow, ModelChoice};
use v2xw_engine::{Engine, NullRecorder, Scenario};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: mbd_eval <scenario.yaml> [options]");
        std::process::exit(2);
    };
    let flag = |name: &str| args.iter().any(|a| a == name);
    let value = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let num = |name: &str| value(name).map(|v| v.parse::<f64>().expect("a number"));

    let mut s = Scenario::load(path).expect("the scenario loads");
    if let Some(r) = num("--rate") {
        s.actors.vehicles.demand.rate_veh_per_h = Some(r);
    }
    if let Some(d) = num("--duration") {
        s.time.duration_s = d;
    }
    if let Some(seed) = value("--seed") {
        s.seed = seed.parse().expect("a seed");
    }
    if !flag("--keep-metrics") {
        s.metrics.clear();
    }
    if flag("--no-attackers") {
        s.threats.attackers.clear();
        s.events
            .retain(|e| e.kind != v2xw_engine::scenario::TimelineKind::AttackWave);
    }
    if let Some(kind) = value("--attack") {
        let id = format!("threat/attacker/legacy/{kind}");
        if s.threats.attackers.is_empty() {
            s.threats.attackers.push(Attacker {
                id,
                fraction: Some(0.05),
                count: None,
                actor_ids: Vec::new(),
                params: serde_json::json!({}),
                schedule: None,
            });
        } else {
            s.threats.attackers.truncate(1);
            s.threats.attackers[0].id = id;
            s.threats.attackers[0].params = serde_json::json!({});
        }
    }
    if let Some(a) = s.threats.attackers.first_mut() {
        if let Some(n) = num("--count") {
            a.count = Some(n as u32);
            a.fraction = None;
            a.actor_ids.clear();
        }
        if let Some(f) = num("--fraction") {
            a.fraction = Some(f);
            a.count = None;
            a.actor_ids.clear();
        }
        if let Some(from) = num("--from") {
            a.schedule = Some(DilationWindow {
                from_s: from,
                to_s: s.time.duration_s,
            });
        }
    }
    if let Some(ma) = value("--ma") {
        s.detection.ma = Some(ModelChoice {
            id: ma,
            params: serde_json::json!({}),
        });
    }
    if let Some(strategy) = value("--strategy") {
        s.security.pseudonym_change.strategy = strategy;
    }
    if let Some(period) = num("--period") {
        s.security.pseudonym_change.period_s = Some(period);
    }
    if let Some(det) = value("--detector") {
        s.detection.local = vec![ModelChoice {
            id: det,
            params: serde_json::json!({}),
        }];
    }

    if flag("--legacy-accuracy") {
        for d in &mut s.detection.local {
            if !d.params.is_object() {
                d.params = serde_json::json!({});
            }
            d.params["use_stated_accuracy"] = serde_json::json!(0.0);
        }
    }
    if let Some(v) = value("--silent") {
        let parts: Vec<f64> = v.split(',').map(|x| x.parse().expect("a number")).collect();
        s.security.pseudonym_change.silent_period_s = Some(parts);
    }
    if let Some(f) = num("--sniffers") {
        s.threats.eavesdropper = Some(ModelChoice {
            id: "threat/observer/passive-privacy".to_string(),
            params: serde_json::json!({"sniffer_fraction": f, "range_m": num("--sniffer-range").unwrap_or(200.0)}),
        });
    }
    let rate = s.actors.vehicles.demand.rate_veh_per_h;
    let duration = s.time.duration_s;
    let attack = s.threats.attackers.first().map(|a| a.id.clone());
    let start = Instant::now();
    let mut engine = Engine::build(s, "").expect("the scenario builds");
    let mut recorder = NullRecorder::new();
    let report = engine.run(&mut recorder).expect("the run completes");
    let wall = start.elapsed().as_secs_f64();
    let p = &report.phase2;

    let ratio = |a: u64, b: u64| if b == 0 { None } else { Some(a as f64 / b as f64) };
    let stats = |v: &[u64]| {
        if v.is_empty() {
            return serde_json::Value::Null;
        }
        let mut s: Vec<f64> = v.iter().map(|ns| *ns as f64 / 1e9).collect();
        s.sort_by(f64::total_cmp);
        let mean = s.iter().sum::<f64>() / s.len() as f64;
        serde_json::json!({
            "n": s.len(),
            "min_s": s[0],
            "median_s": s[s.len() / 2],
            "mean_s": mean,
            "max_s": s[s.len() - 1],
        })
    };
    let out = serde_json::json!({
        "scenario": path,
        "rate_veh_per_h": rate,
        "duration_s": duration,
        "attack": attack,
        "wall_s": wall,
        "nodes_created": report.nodes_created,
        "attackers": p.attackers,
        "falsified_claims": p.falsified_claims,
        "messages_checked": p.messages_checked,
        "verdicts_fired": p.verdicts_fired,
        "verdicts_by_detector": p.verdicts_by_detector,
        "verification_states": p.verification_states,
        "spdu_signature_failures": p.spdu_signature_failures,
        "spdu_parse_failures": p.spdu_parse_failures,
        "crl_period_refusals": p.crl_period_refusals,
        "vehicles_starved": p.vehicles_starved,
        "vehicles_unprovisioned": p.vehicles_unprovisioned,
        "reports_sent": p.reports_sent,
        "reports_about_attackers": p.reports_about_attackers,
        "reports_about_honest": p.reports_about_honest,
        "report_precision": ratio(p.reports_about_attackers, p.reports_sent),
        "attackers_reported": p.attackers_reported,
        "detection_recall": ratio(p.attackers_reported, p.attackers),
        "honest_reported": p.honest_reported,
        "reports_at_ma": p.reports_at_ma,
        "ma_revoke_decisions": p.ma_revoke_decisions,
        "decisions_honest": p.decisions_honest,
        "revoked_attackers": p.revoked_attackers,
        "revoked_honest": p.revoked_honest,
        "revocation_recall": ratio(p.revoked_attackers, p.attackers),
        "revocation_precision": ratio(p.revoked_attackers, p.revoked_attackers + p.revoked_honest),
        "onset_to_detection": stats(&p.onset_to_detection_ns),
        "detection_to_decision": stats(&p.detection_to_decision_ns),
        "crls_issued": p.crls_issued,
        "crl_past_horizon": p.crl_past_horizon,
        "revocation_latency_s": p.revocation_latency_ns as f64 / 1e9,
        "pseudonym_changes": p.pseudonym_changes,
        "pseudonym_changes_requested": p.pseudonym_changes_requested,
        "pseudonym_silenced_frames": p.pseudonym_silenced_frames,
        "accuracy_stated": p.accuracy_stated,
        "accuracy_unavailable": p.accuracy_unavailable,
        "accuracy_unjoined": p.accuracy_unjoined,
        "privacy": {
            "frames_read": p.privacy_frames_read,
            "link_decisions": p.privacy_link_decisions,
            "links_claimed": p.privacy_links_claimed,
            "links_correct": p.privacy_links_correct,
            "linkability": ratio(p.privacy_links_correct, p.pseudonym_changes),
            "mean_anonymity_set": ratio(p.privacy_anonymity_set_sum, p.privacy_link_decisions),
            "mean_degree_of_anonymity": ratio(p.privacy_degree_micro_sum, p.privacy_link_decisions.saturating_mul(1_000_000)),
            "tracked_vehicles": p.privacy_tracked_vehicles,
            "mean_tracked_s": ratio(p.privacy_tracked_sum_ns, p.privacy_tracked_vehicles.saturating_mul(1_000_000_000)),
            "max_tracked_s": p.privacy_tracked_max_ns as f64 / 1e9,
        },
        "backend_errors": p.backend_errors,
    });
    println!("{}", serde_json::to_string(&out).expect("serialises"));
}
