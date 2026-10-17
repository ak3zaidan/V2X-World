//! WP-D1, finished: every provider of the US deployment carries real vehicle flows.
//!
//! The default deployment (`us-production-2025`) is two providers on one CTL. Each runs
//! its own SCMS (one `ScmsRun` per provider): the seeded fleet split assigns every
//! vehicle to one, whose ECA enrolled it, whose RA and ACA provision it, whose MA receives
//! its reports and whose CRL Generator revokes it. A device of one provider verifies the
//! other's through the shared CTL and chain file; a report about another provider's
//! certificate is shared MA to MA with the provider that issued it, because only that
//! provider's ACA and LAs can resolve it (Provider Req. §2.2; 01 §9); and every RA serves
//! every provider's CRL, so each provider's revocations are enforced by everyone.
//!
//! One run of the NYC Manhattan scenario (every third roadside unit, as `tests/phase2.rs`)
//! with the first six vehicles armed as constant-position attackers, so both providers
//! have liars to revoke.

use std::path::Path;
use std::sync::OnceLock;

use v2xw_engine::{DigestRecorder, Engine, RunReport, Scenario};

fn scenario() -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    let mut s = Scenario::load(root.join("scenarios").join("phase2-manhattan.yaml"))
        .expect("the shipped scenario loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    s.actors.rsus = s
        .actors
        .rsus
        .into_iter()
        .enumerate()
        .filter(|(i, _)| i % 3 == 0)
        .map(|(_, r)| r)
        .collect();
    s.threats.attackers[0].count = Some(6);
    s
}

struct Outcome {
    report: RunReport,
    /// Per provider: devices enrolled, CRL entries its Generator issued, reports its MA
    /// received from another MA, reports it forwarded, SPDUs its backend refused.
    providers: Vec<(usize, usize, u64, u64, usize)>,
    /// Per attacker: its provider and whether the authority decided on it.
    attackers: Vec<(usize, bool)>,
}

fn outcome() -> &'static Outcome {
    static RUN: OnceLock<Outcome> = OnceLock::new();
    RUN.get_or_init(|| {
        let mut engine = Engine::build(scenario(), "").expect("builds");
        let report = engine.run(&mut DigestRecorder::new()).expect("runs");
        let p2 = engine.phase2().expect("an SCMS run");
        let providers = p2
            .scms_runs()
            .iter()
            .map(|r| {
                (
                    r.state.devices.len(),
                    r.state.crlg.entries.len(),
                    r.state.shared.received,
                    r.state.shared.forwarded,
                    r.state.pdu_rejections.len(),
                )
            })
            .collect();
        let attackers = report
            .phase2
            .attacker_outcomes
            .iter()
            .map(|a| {
                let node = v2xw_core::ids::NodeId::new(a.node);
                (
                    p2.provider_of_node(node)
                        .expect("an attacker is provisioned"),
                    a.decided_ns.is_some(),
                )
            })
            .collect();
        Outcome {
            report,
            providers,
            attackers,
        }
    })
}

fn get(m: &std::collections::BTreeMap<String, u64>, k: &str) -> u64 {
    m.get(k).copied().unwrap_or(0)
}

/// The fleet split and cross-provider verification: both providers enrolled vehicles,
/// and a vehicle of each provider verified the other provider's vehicles' messages
/// through the CTL (from the receivers' own verdicts).
#[test]
fn both_providers_serve_vehicles_that_verify_each_other() {
    let o = outcome();
    let p = &o.report.phase2;
    println!(
        "providers {:?} attackers {:?}\nverified {:?}\nrevoked {:?}\nreports {:?}\nrevocations {:?} shared {} unresolvable {}",
        o.providers,
        o.attackers,
        p.verified_by_provider,
        p.revoked_by_provider,
        p.reports_by_provider,
        p.revocations_by_provider,
        p.reports_shared,
        p.reports_foreign_unresolvable
    );
    assert_eq!(p.backend_errors, 0, "{}", p.first_backend_error);
    assert_eq!(o.providers.len(), 2, "one SCMS run per provider");
    for (k, prov) in o.providers.iter().enumerate() {
        assert!(prov.0 > 0, "provider {k} enrolled no vehicle");
        assert_eq!(prov.4, 0, "provider {k}'s backend refused SPDUs");
    }
    assert_eq!(
        p.devices_by_provider.values().sum::<u64>(),
        o.providers.iter().map(|x| x.0 as u64).sum::<u64>()
    );
    for pair in ["provider-a<-provider-b", "provider-b<-provider-a"] {
        assert!(
            get(&p.verified_by_provider, pair) > 0,
            "no {pair} message verified: {:?}",
            p.verified_by_provider
        );
    }
}

/// A provider-b liar reported by provider-a vehicles reaches provider-b's MA by report
/// sharing and is revoked by provider-b's CRL Generator; every provider's CRL is
/// enforced by every provider's vehicles; no honest vehicle is revoked.
#[test]
fn a_shared_report_is_revoked_by_the_issuing_provider_and_enforced_by_everyone() {
    let o = outcome();
    let p = &o.report.phase2;
    assert!(
        get(&p.reports_by_provider, "provider-a<-provider-b") > 0,
        "no provider-a vehicle reported a provider-b certificate: {:?}",
        p.reports_by_provider
    );
    assert!(p.reports_shared > 0, "no MA shared a report");
    assert!(
        o.providers[1].2 > 0,
        "provider-b's MA received no shared report"
    );
    assert!(o.providers[0].3 > 0, "provider-a's MA forwarded nothing");
    assert_eq!(p.reports_foreign_unresolvable, 0);
    for k in 0..2 {
        assert!(
            o.attackers.iter().any(|(a, decided)| *a == k && *decided),
            "no provider-{k} attacker was decided: {:?}",
            o.attackers
        );
        assert!(
            o.providers[k].1 > 0,
            "provider {k}'s CRL Generator issued nothing"
        );
    }
    assert!(get(&p.revocations_by_provider, "provider-a") > 0);
    assert!(get(&p.revocations_by_provider, "provider-b") > 0);
    assert_eq!(p.revoked_honest, 0, "an honest vehicle was revoked");
    for pair in [
        "provider-a<-provider-a",
        "provider-a<-provider-b",
        "provider-b<-provider-a",
        "provider-b<-provider-b",
    ] {
        assert!(
            get(&p.revoked_by_provider, pair) > 0,
            "no {pair} reception was refused by a CRL: {:?}",
            p.revoked_by_provider
        );
    }
}
