//! Every provider of the US deployment carries real vehicle flows.
//!
//! The default deployment (`us-production-2025`) is two providers on one CTL. Each runs
//! its own SCMS (one `ScmsRun` per provider): the seeded fleet split assigns every
//! vehicle to one, whose ECA enrolled it, whose RA and ACA provision it, whose MA receives
//! its reports and whose CRL Generator revokes it. A device of one provider verifies the
//! other's through the shared CTL and chain file; a report about another provider's
//! certificate is shared MA to MA with the provider that issued it, because only that
//! provider's ACA and LAs can resolve it (Provider Req. §2.2); and every RA serves
//! every provider's CRL, so each provider's revocations are enforced by everyone.
//!
//! One run of the NYC Manhattan scenario (every third roadside unit, as `tests/report_to_revocation.rs`)
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
    /// Per provider: reports its MA opened, shared reports it holds that came
    /// on the modelled path (nothing to open), and shared reports still on the link.
    opened: Vec<(usize, usize, usize)>,
}

/// Per provider: reports its MA opened, shared reports it holds without an opened report,
/// and shared reports still on the MA-to-MA link at the end.
fn opened_of(runs: &[v2xw_proto::scms::run::ScmsRun]) -> Vec<(usize, usize, usize)> {
    use v2xw_proto::scms::run::SubjectFrom;
    runs.iter()
        .map(|r| {
            let ma = &r.state.ma;
            let modelled = ma
                .report_subjects
                .iter()
                .zip(&ma.report_opened)
                .filter(|(s, o)| s.from == SubjectFrom::Shared && o.is_none())
                .count();
            (ma.opened_reports.len(), modelled, ma.shared_runs.len())
        })
        .collect()
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
        let opened = opened_of(p2.scms_runs());
        Outcome {
            report,
            providers,
            attackers,
            opened,
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
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
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
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
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

/// [CAMP122] §5; Provider Req. §2.2: a report goes to the MA of the provider
/// whose ACA the accused certificate's issuer names, as the receiving MA read it from
/// the report, not to the provider the engine's map gives the accused device. With that
/// map naming a device of the other provider for every certificate
/// (`Phase2::misattribute_providers_for_test`), a provider-b certificate reported
/// through provider-a's RA still reaches provider-b's MA, every report an MA received
/// from another MA names one of its own ACAs, and every report about another provider's
/// certificate was shared with that provider. Fault: route by the map's device
/// (`provider_of_device`) -> the reports are misrouted and the test goes red.
#[test]
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
fn a_report_routes_by_the_issuer_named_in_its_certificate() {
    use std::collections::BTreeSet;
    use v2xw_proto::scms::run::SubjectFrom;
    let mut engine = Engine::build(scenario(), "").expect("builds");
    engine
        .phase2_mut()
        .expect("an SCMS run")
        .misattribute_providers_for_test();
    let report = engine.run(&mut DigestRecorder::new()).expect("runs");
    assert_eq!(
        report.phase2.backend_errors, 0,
        "{}",
        report.phase2.first_backend_error
    );
    let runs = engine.phase2().expect("an SCMS run").scms_runs();
    assert_eq!(runs.len(), 2);
    let owner = |id8: &[u8; 8]| runs.iter().position(|r| r.state.issues_under(id8));
    // Per provider: the certificates it holds a shared report about, and the
    // certificates of other providers its own vehicles reported to it.
    let mut shared_in: Vec<BTreeSet<[u8; 8]>> = vec![BTreeSet::new(); runs.len()];
    let mut foreign: Vec<(usize, [u8; 8])> = Vec::new();
    // A report that reached an MA in the run's last second may still be on the MA-to-MA
    // link when the run ends.
    let last = runs
        .iter()
        .flat_map(|r| r.state.ma.report_arrived.iter().copied())
        .max()
        .unwrap_or(0);
    for (k, r) in runs.iter().enumerate() {
        for (idx, s) in r.state.ma.report_subjects.iter().enumerate() {
            let late = r.state.ma.report_arrived[idx] + 1_000_000_000 > last;
            let (Some(issuer), Some(cert)) = (s.issuer, s.cert) else {
                continue;
            };
            match s.from {
                SubjectFrom::Shared => {
                    assert_eq!(
                        owner(&issuer),
                        Some(k),
                        "provider {k}'s MA was shared a report it cannot resolve"
                    );
                    shared_in[k].insert(cert);
                }
                SubjectFrom::SuspectCertificate if !late => {
                    if let Some(x) = owner(&issuer).filter(|x| *x != k) {
                        foreign.push((x, cert));
                    }
                }
                SubjectFrom::SuspectCertificate => {}
                SubjectFrom::EngineMap => {}
            }
        }
    }
    println!(
        "shared in {:?}; foreign {}",
        shared_in.iter().map(BTreeSet::len).collect::<Vec<_>>(),
        foreign.len()
    );
    assert!(
        foreign.iter().any(|(x, _)| *x == 1),
        "no provider-a vehicle reported a provider-b certificate"
    );
    for (x, cert) in &foreign {
        assert!(
            shared_in[*x].contains(cert),
            "a report about provider {x}'s certificate never reached its MA"
        );
    }
}

/// [SM-MBR] §3 (the MA verifies the evidence it acts on): on the same seed, the
/// MA-to-MA share is the same (every provider's `shared.received` is unchanged from a run
/// that shares without the report's bytes), and each receiving MA now
/// opens every real report shared with it itself: its `opened_reports` grows by
/// `shared.received`, less the shared reports that were modelled and those still on the
/// link at the end. No honest shared report is refused (the backend refused nothing).
/// Fault: keep `secured: None` on the share -> nothing more is opened and the test goes
/// red.
#[test]
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
fn the_receiving_ma_opens_every_report_shared_with_it() {
    let o = outcome();
    let mut engine = Engine::build(scenario(), "").expect("builds");
    engine
        .phase2_mut()
        .expect("an SCMS run")
        .share_unverified_for_test();
    engine.run(&mut DigestRecorder::new()).expect("runs");
    let before = engine.phase2().expect("an SCMS run").scms_runs();
    let was = opened_of(before);
    println!(
        "opened {:?} (unverified share {was:?}); providers {:?}",
        o.opened, o.providers
    );
    let mut grew = 0;
    for (k, run) in before.iter().enumerate() {
        let received = o.providers[k].2;
        assert_eq!(
            received, run.state.shared.received,
            "provider {k}: shared.received moved"
        );
        assert_eq!(o.providers[k].4, 0, "provider {k}'s backend refused SPDUs");
        let (now, modelled, on_link) = o.opened[k];
        let expect = usize::try_from(received).expect("fits") - modelled - on_link;
        assert_eq!(
            now,
            was[k].0 + expect,
            "provider {k}: opened {now}, was {}, shared {received} ({modelled} modelled, \
             {on_link} on the link)",
            was[k].0
        );
        grew += expect;
    }
    assert!(grew > 0, "no real report was shared and re-verified");
}
