//! The ETSI TS 102 941 skeleton: the two built flows, and passive revocation.

use v2xw_core::ids::NodeId;
use v2xw_proto::etsi::ts102941::{EtsiParams, EtsiRun};
use v2xw_proto::stage::StageId;

const STATION: NodeId = NodeId::new(2_000);

fn deployment() -> EtsiRun {
    let mut run = EtsiRun::new(EtsiParams::default()).expect("encodes");
    run.add_station(STATION);
    run
}

#[test]
fn authorization_takes_the_aa_to_ea_round_trip() {
    let mut run = deployment();
    run.enrol(STATION);
    run.run().expect("runs");
    let auth = run.authorize(STATION);
    run.run().expect("runs");

    // The station never talks to the EA during authorization; the AA does, and the EA
    // never sees the ticket keys [TS 102 941 §6.1.4 NOTE 1].
    let steps: Vec<_> = run.kernel.steps.iter().map(|s| s.step).collect();
    assert!(steps.contains(&"etsi-validation-request"));
    assert!(steps.contains(&"etsi-validation-response"));
    let station_to_ea = run.kernel.steps.iter().any(|s| {
        s.from == STATION
            && s.to == run.nodes.ea
            && s.flow == v2xw_proto::stage::FlowId::EtsiAuthorization
    });
    assert!(
        !station_to_ea,
        "the station must reach the EA only through the AA"
    );
    assert_eq!(run.tickets.get(&STATION), Some(&1));
    assert!(run.kernel.stages.at(auth, StageId::Certified).is_some());
}

#[test]
fn a_blocklisted_station_is_starved_rather_than_revoked() {
    // Passive revocation: no CRL, no list, no broadcast — the EA simply refuses the next
    // authorization request [TS 102 941 §6.1.6; EUCP §7.3.2].
    let mut run = deployment();
    run.enrol(STATION);
    run.run().expect("runs");
    run.authorize(STATION);
    run.run().expect("runs");
    assert_eq!(run.tickets.get(&STATION), Some(&1));

    run.blocklist(STATION);
    let refused_run = run.authorize(STATION);
    run.run().expect("runs");

    assert_eq!(run.refused, 1);
    assert_eq!(
        run.tickets.get(&STATION),
        Some(&1),
        "no further ticket may be issued"
    );
    assert!(
        run.kernel
            .stages
            .at(refused_run, StageId::Certified)
            .is_none(),
        "a refused authorization must not stamp `certified`"
    );
    assert!(
        run.kernel
            .stages
            .at(refused_run, StageId::Requested)
            .is_some(),
        "but the attempt itself is still on the record"
    );
}

#[test]
fn a_station_that_never_enrolled_cannot_be_authorized() {
    let mut run = deployment();
    run.authorize(STATION);
    run.run().expect("runs");
    assert_eq!(run.refused, 1);
    assert!(run.tickets.is_empty());
}

#[test]
fn the_pool_size_and_preload_are_the_certificate_policys() {
    let run = deployment();
    assert_eq!(run.params.at_concurrent, 100);
    assert_eq!(
        run.params.at_preload.as_nanos(),
        90 * 86_400 * 1_000_000_000
    );
    assert_eq!(
        run.params.at_validity.as_nanos(),
        7 * 86_400 * 1_000_000_000
    );
}

// =========================================================================================
// The flows build decision D5 deferred
// =========================================================================================

use v2xw_proto::etsi::ts102941::{DEFERRED_FLOWS, all_flows};
use v2xw_proto::spec::RevocationMechanism;
use v2xw_proto::stage::FlowId;

const SUBJECT: NodeId = NodeId::new(2_001);

fn enrolled() -> EtsiRun {
    let mut run = deployment();
    run.enrol(STATION);
    run.run().expect("runs");
    run
}

fn uplink_bytes(run: &EtsiRun, flow: FlowId, from: NodeId) -> u64 {
    run.kernel
        .steps
        .iter()
        .filter(|s| s.flow == flow && s.from == from)
        .map(|s| u64::from(s.bytes))
        .sum()
}

/// Every deferred flow is declared, and every flow the plug-in declares is one of the
/// seven the module documents.
#[test]
fn every_deferred_flow_is_declared_once() {
    assert_eq!(DEFERRED_FLOWS.len(), 7);
    let all = all_flows();
    assert_eq!(all.len(), 9, "two original plus seven deferred");
    let mut ids: Vec<&str> = all.iter().map(|f| f.id.as_str()).collect();
    let before = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), before, "no flow is declared twice");
    for f in &all {
        assert!(!f.participants.is_empty(), "{} has no participants", f.id);
        assert!(!f.stages.is_empty(), "{} declares no stages", f.id);
    }
}

/// The butterfly variant's whole point: one uplink request buys a whole batch, where the
/// standard variant needs one request per ticket.
///
/// This is the number 05-protocols.md §4.2 is describing when it says the variant is
/// "based on IEEE 1609.2.1", and it is the comparison a study of provisioning cost makes.
/// What the variant does *not* save is signatures: the AA still certifies each ticket, and
/// the second half of this test pins that so the saving cannot be over-claimed.
#[test]
fn the_butterfly_variant_costs_one_uplink_request_instead_of_twenty() {
    let batch = EtsiParams::default().butterfly_batch;
    assert_eq!(batch, 20);

    // The standard variant, twenty times over.
    let mut standard = enrolled();
    for _ in 0..batch {
        standard.authorize(STATION);
        standard.run().expect("runs");
    }
    assert_eq!(standard.tickets_of(STATION), batch);
    let standard_uplink = uplink_bytes(&standard, FlowId::EtsiAuthorization, STATION);

    // The butterfly variant: one request, then one download.
    let mut butterfly = enrolled();
    let auth = butterfly.authorize_butterfly(STATION);
    butterfly.run().expect("runs");
    let current_i = butterfly.current_i;
    let download = butterfly.download_ats(STATION, current_i);
    butterfly.run().expect("runs");
    assert_eq!(
        butterfly.tickets_of(STATION),
        batch,
        "one expansion fills the pool"
    );
    let butterfly_uplink = uplink_bytes(&butterfly, FlowId::EtsiButterflyAuthorization, STATION)
        + uplink_bytes(&butterfly, FlowId::EtsiAtDownload, STATION);

    assert!(
        butterfly_uplink * 4 < standard_uplink,
        "the butterfly variant must be far cheaper uplink: {butterfly_uplink} against \
         {standard_uplink}"
    );

    // The station sent exactly two messages for twenty tickets: one request and one
    // collection. The enrolment it did first is a different flow and is not counted.
    let station_steps = butterfly
        .kernel
        .steps
        .iter()
        .filter(|s| {
            s.from == STATION
                && matches!(
                    s.flow,
                    FlowId::EtsiButterflyAuthorization | FlowId::EtsiAtDownload
                )
        })
        .count();
    assert_eq!(station_steps, 2);
    // Against twenty for the standard variant.
    assert_eq!(
        standard
            .kernel
            .steps
            .iter()
            .filter(|s| s.from == STATION && s.flow == FlowId::EtsiAuthorization)
            .count(),
        batch as usize
    );

    // Both flows emitted their declared stages, in order.
    let declared = |id: FlowId| {
        all_flows()
            .into_iter()
            .find(|f| f.id == id)
            .map(|f| f.stages)
            .expect("declared")
    };
    assert_eq!(
        butterfly.kernel.stages.stages(auth),
        declared(FlowId::EtsiButterflyAuthorization)
    );
    assert!(butterfly.kernel.stages.is_ordered(auth));
    assert_eq!(
        butterfly.kernel.stages.stages(download),
        declared(FlowId::EtsiAtDownload)
    );
    assert!(butterfly.kernel.stages.is_ordered(download));

    // And the signature count is *not* saved: the AA signs once per ticket either way.
    let aa = butterfly.nodes.aa;
    let butterfly_signs = butterfly
        .kernel
        .ops
        .iter()
        .filter(|((node, _, kind), _)| *node == aa && *kind == "sign")
        .map(|(_, count)| *count)
        .sum::<u64>();
    assert!(
        butterfly_signs >= u64::from(batch),
        "the AA certifies each cocoon key: {butterfly_signs}"
    );
}

/// A station that was never enrolled cannot start a butterfly request, and one that is
/// blocklisted cannot download a batch that had already been certified for it.
///
/// The second half is where passive revocation bites on this variant, and it bites *late*:
/// the backend has already done all the work.
#[test]
fn a_blocklisted_station_cannot_download_a_batch_it_was_already_certified() {
    let mut run = deployment();
    // Never enrolled: refused at the first hop, and no batch is ever certified.
    run.authorize_butterfly(STATION);
    run.run().expect("runs");
    assert_eq!(run.refused, 1);
    assert!(run.pending_batches.is_empty());

    // Enrolled, expanded and certified — and then blocklisted before it collects.
    let mut run = enrolled();
    run.authorize_butterfly(STATION);
    run.run().expect("runs");
    assert!(
        run.pending_batches.contains_key(&STATION),
        "the AA certified a batch the station has not collected"
    );
    run.blocklist(STATION);

    let download = run.download_ats(STATION, run.current_i);
    run.run().expect("runs");
    assert_eq!(run.tickets_of(STATION), 0, "no ticket may be handed over");
    assert!(
        run.kernel
            .stages
            .at(download, v2xw_proto::stage::StageId::Downloaded)
            .is_none(),
        "a refused download must not stamp `downloaded`"
    );
    assert!(
        run.kernel.stages.at(download, StageId::Requested).is_some(),
        "but the attempt is still on the record"
    );
    assert!(run.refused >= 1);
}

/// A trust list's size is dominated by the certificates in it, and those come from the real
/// COER encoder — so the list grows with the certificate profile and not with a constant.
#[test]
fn a_trust_list_grows_with_the_certificates_in_it() {
    let run = deployment();
    let one = run.sizes.ectl(1).bytes();
    let nine = run.sizes.ectl(9).bytes();
    let per_entry = (nine - one) / 8;
    let certs = v2xw_proto::sizes::CertificateSizes::measured().expect("encodes");
    assert_eq!(
        per_entry,
        certs.authority.bytes() + 8,
        "each entry is a real CA certificate plus a link-certificate HashedId8"
    );

    // The CA-only CRL is the other way round: an entry is twelve bytes, because it revokes
    // an authority by identifier rather than carrying it.
    let crl_one = run.sizes.ca_crl(1).bytes();
    let crl_nine = run.sizes.ca_crl(9).bytes();
    assert_eq!((crl_nine - crl_one) / 8, 12);
    assert!(
        crl_nine < nine,
        "a list of nine revoked authorities is smaller than a list of nine trusted ones"
    );
}

/// The trust list reaches a station, and the station records which sequence it installed.
#[test]
fn a_trust_list_reaches_the_station_and_is_installed() {
    let mut run = deployment();
    assert_eq!(run.installed_ctl_of(STATION), None);
    let flow = run.publish_ectl(STATION);
    run.run().expect("runs");
    assert_eq!(run.installed_ctl_of(STATION), Some(1));
    assert_eq!(
        run.kernel.stages.stages(flow),
        DEFERRED_FLOWS
            .iter()
            .find(|f| f.id == FlowId::EtsiTrustList)
            .map(|f| f.stages)
            .expect("declared")
    );
    assert!(run.kernel.stages.is_ordered(flow));

    // A second publication carries the next sequence number.
    let flow = run.publish_ectl(STATION);
    run.run().expect("runs");
    assert_eq!(run.installed_ctl_of(STATION), Some(2));
    assert!(run.kernel.stages.at(flow, StageId::Issued).is_some());
}

/// The Distribution Centre: the TLM and the Root CA each sign a list once, every station
/// fetches it, and a station that already holds it is answered "current" without the list.
#[test]
fn a_list_is_signed_once_and_every_station_fetches_it() {
    let mut run = deployment();
    let other = NodeId::new(2_050);
    run.add_station(other);
    // Before anything is issued the Distribution Centre has nothing to send.
    let early = run.fetch_trust_lists(STATION, 0);
    run.run().expect("runs");
    assert_eq!(run.installed_ctl_of(STATION), None);
    assert_eq!(run.dc_not_modified, 1);
    assert!(run.kernel.stages.at(early, StageId::Processed).is_some());

    let (ectl, crl) = run.issue_trust_lists(run.kernel.now());
    run.run().expect("runs");
    let declared = |id: FlowId| {
        DEFERRED_FLOWS
            .iter()
            .find(|f| f.id == id)
            .map(|f| f.stages)
            .expect("declared")
    };
    for r in [ectl, crl] {
        assert_eq!(
            run.kernel.stages.stages(r),
            declared(FlowId::EtsiTrustIssue)
        );
    }
    let signs_before = run
        .kernel
        .steps
        .iter()
        .filter(|s| s.flow == FlowId::EtsiTrustIssue)
        .count();
    let mut fetches = Vec::new();
    for station in [STATION, other] {
        fetches.push(run.fetch_trust_lists(station, run.kernel.now()));
    }
    run.run().expect("runs");
    for (station, f) in [STATION, other].into_iter().zip(&fetches) {
        assert_eq!(run.installed_ctl_of(station), Some(1));
        assert_eq!(run.installed_ca_crl_seq.get(&station), Some(&1));
        assert_eq!(
            run.kernel.stages.stages(*f),
            declared(FlowId::EtsiTrustFetch)
        );
        assert!(run.kernel.stages.is_ordered(*f));
    }
    // Two stations served, and the TLM and the Root CA signed nothing more for them.
    assert_eq!(
        run.kernel
            .steps
            .iter()
            .filter(|s| s.flow == FlowId::EtsiTrustIssue)
            .count(),
        signs_before
    );
    let full: Vec<u32> = run
        .kernel
        .steps
        .iter()
        .filter(|s| s.step == "etsi-trust-list-response")
        .map(|s| s.bytes)
        .collect();
    assert_eq!(full.len(), 2);

    // Fetching again: nothing newer, so the answer is the few-byte "current".
    let again = run.fetch_trust_lists(STATION, run.kernel.now());
    run.run().expect("runs");
    let current: Vec<u32> = run
        .kernel
        .steps
        .iter()
        .filter(|s| s.run == again && s.step == "etsi-trust-list-current")
        .map(|s| s.bytes)
        .collect();
    assert_eq!(current.len(), 1);
    assert!(current[0] * 10 < full[0], "{current:?} vs {full:?}");
    assert_eq!(run.dc_not_modified, 2);

    // A new ECTL and CA-CRL reach a station on its next fetch.
    run.issue_trust_lists(run.kernel.now());
    run.run().expect("runs");
    run.fetch_trust_lists(other, run.kernel.now());
    run.run().expect("runs");
    assert_eq!(run.installed_ctl_of(other), Some(2));
    assert_eq!(run.installed_ctl_of(STATION), Some(1));
}

/// The CA-only CRL is the protocol's one active list, and it reaches `enforced`.
#[test]
fn the_ca_only_crl_is_the_one_active_list() {
    let mut run = deployment();
    let flow = run.publish_ca_crl(STATION);
    run.run().expect("runs");
    assert_eq!(run.installed_ca_crl.get(&STATION), Some(&1));
    assert!(
        run.kernel
            .stages
            .at_node(flow, StageId::Enforced, STATION)
            .is_some(),
        "a CA-CRL is enforced at the station, unlike a blocklist entry"
    );

    // And the declared mechanism is now both halves, as 05-protocols.md §4.3 binds it.
    let plugin = v2xw_proto::etsi::EtsiTs102941::default();
    assert!(matches!(
        plugin.revocation(),
        RevocationMechanism::Both(_, _)
    ));
}

/// A report travels the whole TS 103 759 path and ends with the subject's enrolment
/// credential on the EA's internal blocklist — which is never published, so there is
/// nothing to distribute and the subject keeps transmitting until its pool runs out.
#[test]
fn a_report_blocks_the_subjects_enrolment_credential() {
    let mut run = deployment();
    run.add_station(SUBJECT);
    run.enrol(STATION);
    run.enrol(SUBJECT);
    run.run().expect("runs");
    run.authorize(SUBJECT);
    run.run().expect("runs");
    assert_eq!(run.tickets_of(SUBJECT), 1, "the subject holds a ticket");

    let flow = run.report(STATION, SUBJECT);
    run.run().expect("runs");

    assert_eq!(
        run.kernel.stages.stages(flow),
        DEFERRED_FLOWS
            .iter()
            .find(|f| f.id == FlowId::EtsiMisbehaviourReport)
            .map(|f| f.stages)
            .expect("declared")
    );
    assert!(run.kernel.stages.is_ordered(flow));
    assert!(run.blocklist.contains(&SUBJECT));
    assert_eq!(run.reports.get(&SUBJECT), Some(&1));
    assert_eq!(run.pre_processed, 1, "pre-processing is on by default");

    // The ticket it already holds is not taken away: passive revocation is starvation.
    assert_eq!(run.tickets_of(SUBJECT), 1);
    // And the next authorization is refused, which is the eviction mechanism.
    let refused_before = run.refused;
    run.authorize(SUBJECT);
    run.run().expect("runs");
    assert_eq!(run.refused, refused_before + 1);
    assert_eq!(run.tickets_of(SUBJECT), 1);
}

/// Pre-processing is optional, and turning it off removes a charged step without changing
/// the stages the flow emits.
#[test]
fn pre_processing_is_optional_and_charged_when_it_runs() {
    fn go(pre: bool) -> (u64, Vec<StageId>) {
        let params = EtsiParams {
            report_pre_processing: pre,
            ..EtsiParams::default()
        };
        let mut run = EtsiRun::new(params).expect("encodes");
        run.add_station(STATION);
        run.add_station(SUBJECT);
        run.enrol(STATION);
        run.run().expect("runs");
        let flow = run.report(STATION, SUBJECT);
        run.run().expect("runs");
        let ma = run.nodes.ma;
        let verifies = run
            .kernel
            .ops
            .iter()
            .filter(|((node, _, kind), _)| *node == ma && *kind == "verify")
            .map(|(_, c)| *c)
            .sum::<u64>();
        (verifies, run.kernel.stages.stages(flow))
    }
    let (with, stages_with) = go(true);
    let (without, stages_without) = go(false);
    assert!(
        with > without,
        "pre-processing must cost verifications: {with} against {without}"
    );
    assert_eq!(
        stages_with, stages_without,
        "and it must not change the stage vocabulary"
    );
}

/// Every hop of every deferred flow put bytes on a link that the size table knows about.
///
/// Invariant I-P8 for the new flows: the SCMS side has the same test, and without it a new
/// step could put an unattributed byte count on the wire.
#[test]
fn every_deferred_step_is_in_the_size_table() {
    let mut run = deployment();
    run.add_station(SUBJECT);
    run.enrol(STATION);
    run.run().expect("runs");
    run.authorize_butterfly(STATION);
    run.run().expect("runs");
    run.download_ats(STATION, run.current_i);
    run.run().expect("runs");
    run.publish_ectl(STATION);
    run.run().expect("runs");
    run.publish_ca_crl(STATION);
    run.run().expect("runs");
    run.report(STATION, SUBJECT);
    run.run().expect("runs");

    let known: std::collections::BTreeSet<&'static str> = run
        .sizes
        .table()
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    let mut seen = std::collections::BTreeSet::new();
    for step in &run.kernel.steps {
        assert!(step.bytes > 0, "{} carried no bytes", step.step);
        assert!(
            known.contains(step.step),
            "step `{}` put {} bytes on a link with no entry in the size table",
            step.step,
            step.bytes
        );
        seen.insert(step.step);
    }
    assert!(
        seen.len() >= 14,
        "only {} of {} message kinds were exercised",
        seen.len(),
        known.len()
    );
}

/// TS 102 941's privacy separation: the Authorization Authority certifies tickets for
/// stations the EA vouches for, and never learns which enrolment credential it vouched
/// for. The identity crosses the AA sealed for the EA, and what the AA keeps is counts.
#[test]
fn the_aa_cannot_learn_the_enrolment_identity() {
    use v2xw_proto::etsi::{AaState, SealedForEa};
    const OTHER: NodeId = NodeId::new(2_001);
    let mut run = deployment();
    run.add_station(OTHER);
    // The seal: only the EA opens it.
    let sealed = SealedForEa::seal(STATION);
    assert_eq!(sealed.open(run.nodes.aa, run.nodes.ea), None);
    assert_eq!(sealed.open(run.nodes.ma, run.nodes.ea), None);
    assert_eq!(sealed.open(run.nodes.ea, run.nodes.ea), Some(STATION));

    run.enrol(STATION);
    run.enrol(OTHER);
    run.run().expect("runs");
    run.blocklist(OTHER);
    run.authorize(STATION);
    run.authorize(OTHER);
    run.run().expect("runs");
    // The EA told them apart — one ticket, one refusal — and the AA holds only counts.
    // An exhaustive literal: a field naming a station would not compile here.
    assert_eq!(
        run.aa,
        AaState {
            tickets_issued: 1,
            validations_requested: 2,
            refused: 1,
            butterfly_batches: 0,
        }
    );
    assert_eq!(run.tickets.get(&STATION), Some(&1));
    assert_eq!(run.tickets.get(&OTHER), None);

    // The butterfly variant hands the AA a sealed handle it passes back unopened.
    run.authorize_butterfly(STATION);
    run.run().expect("runs");
    assert_eq!(run.aa.butterfly_batches, 1);
    assert!(run.pending_batches.contains_key(&STATION));
}

/// The CCMS's entities as the Backend view shows them.
#[test]
fn the_ccms_view_names_every_authority_and_its_flows() {
    let mut run = deployment();
    run.enrol(STATION);
    run.run().expect("runs");
    run.authorize(STATION);
    run.publish_ectl(STATION);
    run.run().expect("runs");
    let mut tracker = v2xw_proto::view::EdgeTracker::default();
    let view = run.backend_view(run.kernel.now(), &mut tracker);
    let ids: Vec<&str> = view.entities.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["tlm", "cpoc", "rca", "ea", "aa", "ma", "ee"]);
    let edge = |a: &str, b: &str| view.edges.iter().any(|e| e.from == a && e.to == b);
    assert!(edge("ee", "aa") && edge("aa", "ea") && edge("ea", "aa") && edge("aa", "ee"));
    assert!(edge("tlm", "cpoc") && edge("cpoc", "ee"));
    let aa = view.entities.iter().find(|e| e.id == "aa").expect("aa");
    assert_eq!(aa.state["tickets_issued"], 1);
}
