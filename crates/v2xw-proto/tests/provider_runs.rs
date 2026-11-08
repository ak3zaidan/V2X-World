//! One SCMS run per provider. Provider-b's run is built from the home run: the
//! same CTL (byte for byte), its own PKI, components and seed, and an MA that decides on
//! reports another provider's MA shares with it, after the inter-provider latency.

use v2xw_core::ids::NodeId;
use v2xw_proto::ScmsParams;
use v2xw_proto::scms::deployment::{Deployment, Preset, Region};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::StageId;

fn runs() -> (ScmsRun, ScmsRun) {
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[1].region = Region::assumed("us-west").expect("in the catalogue");
    let home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    let peer = ScmsRun::new_peer(&home, 1).expect("provider-b's run builds");
    (home, peer)
}

/// Both runs distribute one CTL; provider-b's run is provider-b's (its nodes, its PKI,
/// its local chain file headed by its own root); a vehicle it provisions holds
/// pseudonyms from provider-b's ACA, not provider-a's.
#[test]
fn a_peer_run_is_its_providers_own_on_the_shared_ctl() {
    let (home, mut peer) = runs();
    let (h, p) = (&home.state, &peer.state);
    assert_eq!(p.provider, 1);
    assert_eq!(h.gov.ctl.spdu, p.gov.ctl.spdu, "one CTL, byte for byte");
    assert_eq!(p.nodes, h.deployment.nodes(1).online);
    assert_ne!(p.nodes.ra, h.nodes.ra);
    assert_eq!(p.gov.config.provider, "provider-b");
    let b_root = h
        .gov
        .peer_cert("provider-b", "root")
        .expect("provider-b's root");
    assert_eq!(p.gov.lccf.certs.first(), Some(b_root), "its own chain file");
    let (b_aca, a_aca) = (
        p.pki.as_ref().unwrap().aca.id8(),
        h.pki.as_ref().unwrap().aca.id8(),
    );
    assert_eq!(b_aca, h.peer_pki[0].1.aca.id8());
    assert_ne!(p.params.master_seed, h.params.master_seed, "its own seed");
    assert!(ScmsRun::new_peer(&home, 0).is_err());
    assert!(ScmsRun::new_peer(&home, 2).is_err());

    // Provider-b runs two ACAs: each pseudonym is one of theirs.
    let b_acas = [b_aca, p.pki.as_ref().unwrap().aca_at(1).id8()];
    let device = NodeId::new(5_000_123);
    peer.preload(device, 0, 1, 2).expect("provisions");
    let dev = &peer.state.devices[&device];
    assert!(!dev.credentials.is_empty());
    for c in dev.credentials.values() {
        let imp = c.implicit.as_ref().expect("an implicit pseudonym");
        assert!(b_acas.contains(&imp.aca_id8), "issued by provider-b's ACAs");
        assert_ne!(imp.aca_id8, a_aca);
    }
}

/// SAESOL's published chain files (2025-07-17, 2025-08-13) carry
/// two ACA certificates, and provider-b places two ACAs. The second (`aca/1`) holds a
/// certificate of the core ACA's form (same ICA, validity, SSP, region, CRACA, an
/// encryption key; its own keys), is hosted and linked to the RA and the MA, takes its
/// share of the RA's shuffled requests (round-robin, ASSUMPTION: the split rule is not
/// published) and signs what it issues, so the device's same-ACA check passes and the MA
/// asks it about its own certificates. Fault (before): `aca/1` placed with no certificate
/// and no traffic, every request to `aca/0`.
#[test]
fn provider_bs_second_aca_is_certified_and_issues_its_share() {
    use v2xw_proto::scms::pdu::SpduKind;
    let (home, peer) = runs();
    assert!(home.state.aca_extra.is_empty(), "provider-a runs one ACA");
    let p = &peer.state;
    assert_eq!(p.aca_extra.len(), 1, "provider-b's aca/1");
    let pki = std::sync::Arc::clone(p.pki.as_ref().unwrap());
    let (a0, a1) = (&pki.aca, pki.aca_at(1));
    assert!(a1.name.starts_with("aca-1."), "{}", a1.name);
    let (t0, t1) = (&a0.cert.0.to_be_signed, &a1.cert.0.to_be_signed);
    assert_eq!(
        a0.cert.0.issuer, a1.cert.0.issuer,
        "both under provider-b's ICA"
    );
    assert_eq!(t0.validity_period, t1.validity_period);
    assert_eq!(t0.app_permissions, t1.app_permissions);
    assert_eq!(t0.cert_issue_permissions, t1.cert_issue_permissions);
    assert_eq!(t0.region, t1.region);
    assert_eq!(t0.craca_id, t1.craca_id);
    assert_eq!(t0.crl_series, t1.crl_series);
    assert_eq!(t0.encryption_key.is_some(), t1.encryption_key.is_some());
    assert_ne!(
        t0.verify_key_indicator, t1.verify_key_indicator,
        "its own key"
    );
    // The records list it with its certificate.
    let mut tracker = v2xw_proto::scms::pki_view::PkiTracker::default();
    let runs = [peer];
    let s = v2xw_proto::scms::pki_view::pki_state(&runs, 0, &mut tracker).expect("a state");
    let acas: Vec<_> = s
        .providers
        .iter()
        .flat_map(|q| q.components.iter())
        .filter(|c| c.provider == "provider-b" && c.role == "aca")
        .collect();
    assert_eq!(acas.len(), 2, "{acas:?}");
    assert!(acas.iter().all(|c| c.cert.is_some()), "{acas:?}");
    let [mut peer] = runs;
    // A provisioning: the requests go to both ACAs, half each, and every certificate the
    // device installs is the receiving ACA's and verifies.
    let device = NodeId::new(5_000_131);
    peer.preload(device, 0, 1, 20).expect("provisions");
    let st = &peer.state;
    let (n0, n1) = (st.nodes.pca.index(), st.aca_extra[0].index());
    let to = |n: u32| {
        st.pdus
            .iter()
            .filter(|x| x.kind == SpduKind::RaAcaCertRequest && x.to == n)
            .count()
    };
    assert_eq!(
        (to(n0), to(n1)),
        (10, 10),
        "round-robin over the shuffled batch"
    );
    let id1 = pki.aca_at(1).id8();
    let by1 = st.devices[&device]
        .credentials
        .values()
        .filter(|c| c.implicit.as_ref().is_some_and(|i| i.aca_id8 == id1))
        .count();
    assert_eq!(by1, 10, "aca/1 issued and signed its ten");
    assert!(st.pdu_rejections.is_empty(), "{:?}", st.pdu_rejections);
    assert_eq!(st.devices[&device].credentials.len(), 20);
}

/// A report provider-a's MA shares about a provider-b certificate reaches provider-b's
/// MA one inter-provider latency (us-east to us-west, ASSUMPTION) after it left, and
/// provider-b's MA revokes the device from it: its own CRL Generator issues the entry.
#[test]
fn a_shared_report_crosses_the_inter_provider_link_and_is_decided_there() {
    let (home, mut peer) = runs();
    let device = NodeId::new(5_000_124);
    peer.preload(device, 0, 1, 2).expect("provisions");
    let (&(i, _), c) = peer.state.devices[&device]
        .credentials
        .iter()
        .next()
        .expect("a pseudonym");
    let lv = c.lv;
    let from = home.state.nodes.ma;
    let sent = 1_000_000_000;
    let run = peer.shared_report_at_ma(from, i, lv, sent, sent, 200, None);
    peer.run_until(sent + 2_000_000_000).expect("runs");
    let at = peer
        .kernel
        .stages
        .at(run, StageId::ReportReceived)
        .expect("arrived at provider-b's MA");
    let lat = home
        .state
        .deployment
        .inter_provider_latency(0, 1)
        .expect("region model");
    // Independent derivation (haversine in Python, R = 6371.0088 km): us-east
    // (39.0438, -77.4874) to us-west (45.8399, -119.7006) is 3,502.6 km; at 2/3 c that is
    // 17,525,023 ns, plus the 1 ms per-link overhead.
    assert_eq!(lat.as_nanos(), 18_525_023, "coast to coast: {lat:?}");
    assert!(at >= sent + lat.as_nanos(), "{at} before {sent} + {lat:?}");
    assert!(
        at < sent + lat.as_nanos() + 5_000_000,
        "{at}: queued too long"
    );
    assert_eq!(peer.state.shared.received, 1);
    assert_eq!(peer.state.ma.reports.len(), 1);
    assert!(peer.request_revocation(0, i, 2).is_some());
    peer.run_until(sent + 7 * 86_400_000_000_000).expect("runs");
    assert!(
        !peer.state.crlg.entries.is_empty(),
        "provider-b's CRL Generator issued nothing"
    );
    assert!(home.state.crlg.entries.is_empty());
}

/// A report another provider's MA shares
/// with this one while this MA is down waits in the sharing MA's queue, retried every
/// `BACKEND_RETRY`, and arrives once the MA is back, as a message between two of one
/// provider's components does. Before, the sender was not one of this run's components,
/// so the report was dropped as a device's request is. Fault: drop it again -> no report
/// at the MA.
#[test]
fn a_report_shared_with_a_down_ma_arrives_when_it_is_back() {
    let (home, mut peer) = runs();
    let device = NodeId::new(5_000_124);
    peer.preload(device, 0, 1, 2).expect("provisions");
    let (&(i, _), c) = peer.state.devices[&device]
        .credentials
        .iter()
        .next()
        .expect("a pseudonym");
    let lv = c.lv;
    let sent = 1_000_000_000;
    let back = sent + 60_000_000_000;
    peer.set_faults(vec![v2xw_proto::fault::Fault {
        node: peer.state.nodes.ma,
        kind: v2xw_proto::fault::FaultKind::Offline,
        from: sent,
        until: back,
    }]);
    let run = peer.shared_report_at_ma(home.state.nodes.ma, i, lv, sent, sent, 200, None);
    peer.run_until(back + 30_000_000_000).expect("runs");
    assert!(
        peer.state.faults.held >= 1,
        "the shared report was not held: {:?}",
        peer.state.faults
    );
    assert_eq!(
        peer.state.ma.reports.len(),
        1,
        "the report never reached the MA"
    );
    let at = peer
        .kernel
        .stages
        .at(run, StageId::ReportReceived)
        .expect("arrived at provider-b's MA");
    assert!(at >= back, "{at}: received while the MA was down");
}

/// A home root rollover re-certifies the home provider's ICA and components under the new
/// root; each peer's copy of that hierarchy follows (`sync_peer_from`), so the peer's
/// local chain file lists the re-certified ICA, and still lists the home provider once
/// the old root leaves the CTL.
#[test]
fn a_home_root_rollover_reaches_the_peers_chain_file() {
    let (mut home, mut peer) = runs();
    let label = home.state.gov.config.provider.clone();
    let old_ica = home.state.gov.cert("ica").unwrap().clone();
    let st = &mut home.state;
    let (old, _new, publish) = st.gov.rollover_root(&st.rng, &st.nodes, 1_000);
    st.gov.publish_due(publish);
    let new_ica = home.state.gov.cert("ica").unwrap().clone();
    assert_ne!(old_ica, new_ica);
    peer.state.gov.sync_trust_from(&home.state.gov);
    assert!(peer.state.gov.sync_peer_from(&home.state.gov));
    assert_eq!(peer.state.gov.peer_cert(&label, "ica"), Some(&new_ica));
    assert!(peer.state.gov.lccf.certs.contains(&new_ica));
    assert!(
        !peer.state.gov.sync_peer_from(&home.state.gov),
        "idempotent"
    );
    let at = home
        .state
        .gov
        .remove_root(old, publish + 1)
        .expect("decided");
    home.state.gov.publish_due(at);
    peer.state.gov.sync_trust_from(&home.state.gov);
    peer.state.gov.sync_peer_from(&home.state.gov);
    assert!(
        peer.state.gov.lccf.certs.contains(&new_ica),
        "the home provider stays in the peer's chain file under its new root"
    );
    assert!(!peer.state.gov.lccf.certs.contains(&old_ica));
}

/// Edge-case package, item 8: the EAC removes provider-b's root from the shared CTL.
/// Provider-b's own view of that CTL (it keeps operating under the removed root) must
/// take the CTL, re-sign its local chain file and report its root off the CTL; before
/// 2026-10-19 it panicked ("the home provider has an approved root").
#[test]
fn a_provider_whose_root_the_eac_removed_takes_the_ctl_and_keeps_running() {
    let (mut home, mut peer) = runs();
    let peer_label = peer.state.gov.config.provider.clone();
    let root = home
        .state
        .gov
        .root_of(&peer_label)
        .expect("provider-b on the CTL");
    assert!(home.state.gov.ctl.roots.contains(&root));
    let at = home.state.gov.remove_root(root, 1_000).expect("decided");
    assert_eq!(home.state.gov.publish_due(at), 1);
    assert!(!home.state.gov.ctl.roots.contains(&root));
    assert!(home.state.gov.root_of(&peer_label).is_none());
    assert!(peer.state.gov.sync_trust_from(&home.state.gov));
    assert_eq!(peer.state.gov.ctl.sequence, home.state.gov.ctl.sequence);
    assert!(!peer.state.gov.ctl.roots.contains(&root));
    // Provider-b's own components keep running under the removed root: its view still
    // names that root as its own, by certificate and by key.
    assert!(peer.state.gov.cert("root").is_some());
    assert!(peer.state.gov.signing_key("root").is_some());
}

/// The PKI view names the sharing MA by its own provider (SCMS settings audit). Before,
/// the peer run's id map held only its own components, so the record read
/// `ee/<n> -> provider-b/ma/0`: a device talking to the MA, which no device does.
#[test]
fn the_records_name_the_sharing_ma_as_the_other_providers() {
    use v2xw_proto::scms::pki_view::{self, PkiTracker};
    let (home, mut peer) = runs();
    let device = NodeId::new(5_000_124);
    peer.preload(device, 0, 1, 2).expect("provisions");
    let (&(i, _), c) = peer.state.devices[&device]
        .credentials
        .iter()
        .next()
        .expect("a pseudonym");
    let lv = c.lv;
    let sent = 1_000_000_000;
    peer.shared_report_at_ma(home.state.nodes.ma, i, lv, sent, sent, 200, None);
    peer.run_until(sent + 2_000_000_000).expect("runs");
    let flows = pki_view::drain_flows(std::slice::from_ref(&peer), &mut PkiTracker::default());
    let into_ma: Vec<(Option<&str>, &str)> = flows
        .iter()
        .filter(|f| f.kind == "msg" && f.to.as_deref() == Some("provider-b/ma/0"))
        .map(|f| (f.from.as_deref(), f.name.as_str()))
        .collect();
    assert!(
        into_ma.iter().any(|(f, _)| *f == Some("provider-a/ma/0")),
        "{into_ma:?}"
    );
    assert!(
        into_ma
            .iter()
            .all(|(f, _)| !f.unwrap_or("").starts_with("ee/")),
        "a device talks to the MA: {into_ma:?}"
    );
}

/// The PKI view records a
/// shared report as sent by the forwarding provider's MA. It was labelled as a device
/// (`ee/<node>` of provider-a's MA node), so the recorded topology showed devices talking
/// to the other provider's MA.
#[test]
fn a_shared_report_is_recorded_as_sent_by_the_other_providers_ma() {
    use v2xw_proto::scms::pki_view::{self, PkiTracker};
    let (home, mut peer) = runs();
    let device = NodeId::new(5_000_125);
    peer.preload(device, 0, 1, 2).expect("provisions");
    let (&(i, _), c) = peer.state.devices[&device]
        .credentials
        .iter()
        .next()
        .expect("a pseudonym");
    let lv = c.lv;
    let sent = 1_000_000_000;
    peer.shared_report_at_ma(home.state.nodes.ma, i, lv, sent, sent, 200, None);
    peer.run_until(sent + 2_000_000_000).expect("runs");
    let mut tracker = PkiTracker::default();
    let flows = pki_view::drain_flows(std::slice::from_ref(&peer), &mut tracker);
    let shared: Vec<_> = flows.iter().filter(|f| f.name == "report-shared").collect();
    assert_eq!(shared.len(), 1, "{flows:?}");
    assert_eq!(shared[0].from.as_deref(), Some("provider-a/ma/0"));
    assert_eq!(shared[0].to.as_deref(), Some("provider-b/ma/0"));
}

/// A provider whose root the
/// electors have not approved (`on_ctl: false`, a rogue SCMS or a test-CTL root) still
/// runs under its own root: it answers for its root and provisions its devices, whose
/// certificates nobody else trusts. The first use of its root panicked ("the home
/// provider has an approved root"), so a run with such a provider crashed in seconds.
#[test]
fn a_provider_off_the_ctl_still_runs_under_its_own_root() {
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[1].on_ctl = false;
    let home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    let mut peer = ScmsRun::new_peer(&home, 1).expect("provider-b's run builds");
    assert!(peer.state.gov.cert("root").is_some(), "its own root");
    assert!(peer.state.gov.signing_key("root").is_some());
    let device = NodeId::new(5_000_126);
    peer.preload(device, 0, 1, 2).expect("provisions");
    assert!(!peer.state.devices[&device].credentials.is_empty());
}

/// An enrolment certificate names the provider whose ECA issued it
/// ([SMP] §6.3.1, a unique name). Fault (the first form): every name said `provider-a`,
/// including the ones provider-b's ECA certified.
#[test]
fn an_enrolment_certificate_names_the_provider_whose_eca_issued_it() {
    let (mut home, mut peer) = runs();
    let name = |run: &ScmsRun, d: NodeId| {
        let e = run.state.devices[&d]
            .enrolment_credential
            .as_ref()
            .expect("enrolled");
        v2xw_proto::scms::enrolment::tbs_name(&e.cert.0.to_be_signed)
            .expect("id.name")
            .to_string()
    };
    let (a, b) = (NodeId::new(5_000_201), NodeId::new(5_000_202));
    home.preload(a, 0, 1, 2).expect("provisions");
    peer.preload(b, 0, 1, 2).expect("provisions");
    assert_eq!(name(&home, a), format!("ee-{:08x}.provider-a", a.index()));
    assert_eq!(name(&peer, b), format!("ee-{:08x}.provider-b", b.index()));
}

/// Each provider's Root CA and ICA count the certificates they
/// signed, not the other providers'. Provider-a (ISS form): self, ICA, PG, MA and the
/// root-issued CRL signer; provider-b (SAESOL S2X production form): self, ICA, PG, MA, and
/// its ICA the ECA, PCA, RA, both LAs, the DCM and the ICA-issued CRL signer. Fault (the
/// first form): the home view counted every provider's (9 and 14 at t0) and provider-b's
/// none (0). Provider-b's ICA count was 6 while its records showed 7: the DCM that signs
/// its enrolment traffic had a PDU-layer certificate only. It was then 7
/// against 8: its second ACA (`aca-1`, SAESOL's two published ACA certificates)
/// is issued by the PDU layer under the same ICA and was not counted. Fault: the count in
/// `ScmsRun::new_peer` removed makes the ICA assertion read 7.
#[test]
fn each_providers_root_and_ica_count_their_own_certificates() {
    let (home, peer) = runs();
    let (h, p) = (&home.state.gov.counters, &peer.state.gov.counters);
    assert_eq!(
        h.root_certs_issued, 5,
        "provider-a's root: itself, ICA, PG, MA, CRLG"
    );
    assert_eq!(
        p.root_certs_issued, 4,
        "provider-b's root: itself, ICA, PG, MA"
    );
    let extra = peer.state.pki.as_ref().map_or(0, |k| k.aca_instances());
    assert_eq!(
        extra, 1,
        "provider-b runs a second ACA with its own certificate"
    );
    assert_eq!(
        p.ica_certs_issued, 8,
        "provider-b's ICA: ECA, PCA, RA, LA1, LA2, DCM, CRLG and the second ACA"
    );
    // The home provider runs one ACA: nothing beside its governance set.
    assert_eq!(home.state.pki.as_ref().map_or(0, |k| k.aca_instances()), 0);
}

/// The PDU-layer components of `run`'s provider, by governance role.
fn pdu_components(
    run: &ScmsRun,
) -> Vec<(
    &'static str,
    v2xw_msg::sec_types::ieee1609_dot2::Certificate,
)> {
    let pki = run.state.pki.as_ref().expect("the PDU layer is on");
    vec![
        ("root", pki.root.cert.clone()),
        ("ica", pki.ica.cert.clone()),
        ("eca", pki.eca.cert.clone()),
        ("ra", pki.ra.cert.clone()),
        ("pca", pki.aca.cert.clone()),
        ("ma", pki.ma.cert.clone()),
        ("la1", pki.la1.cert.clone()),
        ("la2", pki.la2.cert.clone()),
        ("dcm", pki.dcm.cert.clone()),
        ("crlg", pki.crlg.cert.clone()),
    ]
}

/// Each SCMS component holds one certificate, the one its
/// provider's chain file lists and the one that signs on the wire ([SMP] §6.1). For both
/// providers every PDU-layer component certificate is the governance's (the chain file's),
/// and an enrolment certificate names that ECA as issuer. Fault (the first form): the PDU
/// layer drew its own ECA, RA, ACA, MA, LAs, DCM and CRL signer under the governance ICA,
/// so `pki.state` named one certificate and the entity's counters another.
#[test]
fn every_component_signs_with_the_certificate_its_chain_file_lists() {
    let (mut home, mut peer) = runs();
    for run in [&home, &peer] {
        let gov = &run.state.gov;
        for (role, cert) in pdu_components(run) {
            assert_eq!(
                gov.cert(role),
                Some(&cert),
                "{}: the {role} signs with its chain-file certificate",
                gov.config.provider
            );
            assert!(
                gov.lccf.certs.contains(&cert),
                "{}: the {role} certificate is in the local chain file",
                gov.config.provider
            );
        }
    }
    let (a, b) = (NodeId::new(5_000_211), NodeId::new(5_000_212));
    home.preload(a, 0, 1, 2).expect("provisions");
    peer.preload(b, 0, 1, 2).expect("provisions");
    for (run, d) in [(&home, a), (&peer, b)] {
        let e = run.state.devices[&d]
            .enrolment_credential
            .as_ref()
            .expect("enrolled");
        let issuer = match &e.cert.0.issuer {
            v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha256AndDigest(h) => {
                h.0.to_vec()
            }
            other => panic!("an enrolment certificate is issued by digest: {other:?}"),
        };
        assert_eq!(
            Some(issuer),
            run.state.gov.cert_digest("eca").map(|h| h.to_vec()),
            "the enrolment certificate's issuer is the chain file's ECA"
        );
    }
}

/// A root rollover reaches the wire. Once the CTL approving the new
/// root is published, the provider's PDU PKI follows the governance (`follow_pki`): a
/// device enrolled after the changeover holds an enrolment certificate and pseudonyms that
/// chain to the new root; a device enrolled before keeps certificates that still verify
/// ([SM-PROV] §3.5.4); and the CRL is signed under both CRACAs, so the old hierarchy's
/// pseudonyms can still be revoked. Fault (the first form): the PDU PKI was fixed at run
/// start, so the joiner's certificates chained to the old root.
#[test]
fn after_a_rollovers_changeover_new_certificates_chain_to_the_new_root() {
    let (mut home, _) = runs();
    let label = home.state.gov.config.provider.clone();
    let before = NodeId::new(5_000_221);
    home.preload(before, 0, 1, 2).expect("provisions");
    let old = std::sync::Arc::clone(home.state.pki.as_ref().unwrap());
    let st = &mut home.state;
    let (_old_root, new_root, publish) = st.gov.rollover_root(&st.rng, &st.nodes, 1_000);
    st.gov.publish_due(publish);
    assert!(st.gov.ctl.roots.contains(&new_root), "the CTL is published");
    let next = v2xw_proto::scms::run::pdu_pki_of(&st.gov, &label).expect("builds");
    assert!(st.follow_pki(&label, std::sync::Arc::new(old.superseded_by(next))));
    let pki = std::sync::Arc::clone(home.state.pki.as_ref().unwrap());
    assert_eq!(
        v2xw_sec::sig1609::cert_hashed_id32(&pki.root.cert).ok(),
        Some(new_root),
        "the PDU layer signs under the new root"
    );
    assert_ne!(pki.eca.cert, old.eca.cert);
    let issuer8 = |c: &v2xw_msg::sec_types::ieee1609_dot2::Certificate| match &c.0.issuer {
        v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha256AndDigest(h)
        | v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier::sha384AndDigest(h) => {
            let mut b = [0u8; 8];
            b.copy_from_slice(&h.0[..]);
            b
        }
        _ => [0; 8],
    };
    let joiner = NodeId::new(5_000_222);
    home.preload(joiner, 0, 1, 2).expect("provisions");
    let dev = &home.state.devices[&joiner];
    let enrol = &dev.enrolment_credential.as_ref().expect("enrolled").cert;
    assert_eq!(issuer8(enrol), pki.eca.id8(), "enrolled by the new ECA");
    assert!(pki.signer_key(enrol).is_ok());
    let ps: Vec<_> = dev
        .credentials
        .values()
        .filter_map(|c| c.implicit.as_ref())
        .collect();
    assert!(!ps.is_empty(), "the joiner holds pseudonyms");
    for c in &ps {
        assert_eq!(c.aca_id8, pki.aca.id8(), "issued by the new ACA");
        assert!(pki.signer_key(&c.certificate).is_ok());
    }
    assert_eq!(issuer8(&pki.aca.cert), pki.ica.id8());
    assert_eq!(
        issuer8(&pki.ica.cert),
        pki.root.id8(),
        "the chain ends at the new root"
    );
    // The device enrolled before the changeover keeps certificates that still verify.
    let old_dev = &home.state.devices[&before];
    let old_enrol = &old_dev.enrolment_credential.as_ref().unwrap().cert;
    assert_eq!(issuer8(old_enrol), old.eca.id8());
    assert!(
        pki.signer_key(old_enrol).is_ok(),
        "the old enrolment still verifies"
    );
    for c in old_dev
        .credentials
        .values()
        .filter_map(|c| c.implicit.as_ref())
    {
        assert!(
            pki.signer_key(&c.certificate).is_ok(),
            "an old pseudonym still verifies"
        );
    }
    // One CRL per CRACA: the current one, and the superseded hierarchy's.
    home.seed_standing_crl(1);
    let crlg = &home.state.crlg;
    let full = v2xw_sec::crl::decode_crl(crlg.secured.as_ref().unwrap().bytes()).unwrap();
    assert_eq!(full.contents.crl_craca.0[..], pki.craca_id8()[..]);
    assert_eq!(
        crlg.secured_retired.len(),
        1,
        "the old CRL signer signs too"
    );
    let retired = v2xw_sec::crl::decode_crl(crlg.secured_retired[0].bytes()).unwrap();
    assert_eq!(retired.contents.crl_craca.0[..], old.craca_id8()[..]);
    assert_ne!(old.craca_id8(), pki.craca_id8());
}

/// Each provider's component certificates carry the
/// `SecurityMgmtSsp` version of its published chain files. ISS pre-production (provider-a)
/// carries version 1 on the root, ICA, ACA, RA, MA, LAs and CRL signer and version 2 on
/// its 1609.2.1 ECA (`dot1.eca.preprod`); SAESOL (provider-b) version 2 throughout. Fault
/// (the first form): version 2 for every provider.
#[test]
fn component_ssp_versions_follow_each_providers_published_form() {
    let (home, peer) = runs();
    let version = |c: &v2xw_msg::sec_types::ieee1609_dot2::Certificate| {
        let ssp = v2xw_sec::ctl::app_ssp(c, 35)
            .flatten()
            .expect("PSID 35 SSP");
        // choice tag, SEQUENCE preamble, then the `version` Uint8.
        ssp[2]
    };
    for (role, cert) in pdu_components(&home) {
        let want = if role == "eca" { 2 } else { 1 };
        assert_eq!(version(&cert), want, "ISS {role}");
    }
    for (role, cert) in pdu_components(&peer) {
        assert_eq!(version(&cert), 2, "S2X {role}");
    }
}

/// The enrolment request's DCM->ECA hop is on `pki.flow`. CAMP's
/// bootstrap is EE -> DCM -> ECA, and the DCM forwards the device's own
/// `EeEcaCertRequestSpdu`, so each provider's records show as many DCM->ECA requests as
/// EE->DCM ones, each forward no earlier than its request. Fault (the first form): only
/// EE->DCM, ECA->DCM and DCM->EE were recorded and the ECA showed 0 requests received.
#[test]
fn every_enrolment_request_is_recorded_on_the_dcm_to_eca_hop() {
    let (mut home, mut peer) = runs();
    let (a, b) = (NodeId::new(5_000_231), NodeId::new(5_000_232));
    home.preload(a, 0, 1, 2).expect("provisions");
    peer.preload(b, 0, 1, 2).expect("provisions");
    for run in [&home, &peer] {
        let n = run.state.nodes;
        let (dcm, eca) = (n.dcm.index(), n.eca.index());
        let req = |from: Option<u32>, to: u32| -> Vec<u64> {
            run.state
                .pdus
                .iter()
                .filter(|q| {
                    q.kind == v2xw_proto::scms::pdu::SpduKind::EeEcaCertRequest
                        && q.to == to
                        && from.is_none_or(|f| q.from == f)
                })
                .map(|q| q.at)
                .collect()
        };
        let to_dcm = req(None, dcm);
        let to_eca = req(Some(dcm), eca);
        assert!(!to_dcm.is_empty(), "a device asked its DCM");
        assert_eq!(to_eca.len(), to_dcm.len(), "one forward per request");
        for (r, f) in to_dcm.iter().zip(&to_eca) {
            assert!(f >= r, "forwarded no earlier than requested");
        }
    }
}

/// A second ACA on an ISS-pattern provider (provider-a, or any provider a `custom`
/// deployment adds, which copies provider-a's form) builds: its certificate carries the
/// same PSID-35 SSP as the core ACA in the protocol PKI. Fault (before): the extra ACA took
/// the ISS chain-file version (1), which the 2022 `AcaSsp` type refuses at encode time, so
/// a run with `components: {aca: 2}` on such a provider failed to start
/// ("provider-b aca/1: COER: Value constraint not satisfied: expected: 2; actual: 1").
#[test]
fn a_second_aca_on_an_iss_pattern_provider_builds_and_signs() {
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[0].components.insert("aca".to_string(), 2);
    let home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    let pki = home.state.pki.as_ref().unwrap();
    let (a0, a1) = (&pki.aca, pki.aca_at(1));
    assert_ne!(a0.id8(), a1.id8(), "aca/1 holds its own certificate");
    // The same PSID-35 permission and SSP choice (0x85, AcaSsp) as the core ACA. The
    // version byte may differ: ISS's chain files carry version 1 on their one PCA, and
    // ISS publishes no second PCA, so an extra ISS-pattern ACA takes the 2022 module's
    // version 2 (documented deviation; the bodies are identical).
    let ssp = |a: &v2xw_proto::scms::pdu::Authority| {
        let perms = a
            .cert
            .0
            .to_be_signed
            .app_permissions
            .as_ref()
            .expect("appPermissions");
        let p = &perms.0[0];
        let bytes = match p.ssp.as_ref().expect("an SSP") {
            v2xw_msg::sec_types::ieee1609_dot2_base_types::ServiceSpecificPermissions::opaque(
                o,
            ) => o.to_vec(),
            other => panic!("not opaque: {other:?}"),
        };
        (p.psid.clone(), bytes[0], bytes.len())
    };
    assert_eq!(
        ssp(a0),
        ssp(a1),
        "the same permission and SSP choice as the core ACA"
    );
}

/// [CAMP122] §5; [BRECHT] §VI-D: the MA sends its lookup to the ACA whose
/// certificate the accused certificate's `issuer` names, read from the opened report,
/// not to the instance the issuance records name. A pseudonym `aca/1` issued is looked
/// up at `aca/1` with those records cleared. Fault: route by the issuance records only
/// (drop the issuer) -> the lookup goes to the core ACA and the test goes red.
#[test]
fn the_ma_asks_the_aca_its_accused_certificates_issuer_names() {
    use v2xw_proto::scms::mbr::{self, Evidence, MbrType, MisbehaviorReport, SignedBsms, Subtype};
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[0].components.insert("aca".to_string(), 2);
    let mut home = ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).expect("builds");
    let (accused, reporter) = (NodeId::new(5_000_201), NodeId::new(5_000_202));
    home.preload(accused, 0, 1, 2).expect("provisions");
    home.preload(reporter, 0, 1, 2).expect("provisions");
    let id1 = home.state.pki.as_ref().unwrap().aca_at(1).id8();
    let (cert, key) = {
        let imp = home.state.devices[&accused]
            .credentials
            .values()
            .filter_map(|c| c.implicit.as_ref())
            .find(|i| i.aca_id8 == id1)
            .expect("aca/1 issued one of the pair");
        (
            (*imp.certificate).clone(),
            v2xw_sec::sig1609::Signer1609::p256(&v2xw_sec::ec::scalar_to_be32(&imp.private))
                .expect("a P-256 key"),
        )
    };
    let bsm = mbr::sign_bsm(&[0x5a; 40], 1_000_000, &cert, &key).expect("signs");
    let report = MisbehaviorReport {
        generation_time: 600_000,
        location: v2xw_sec::envelope::GenerationLocation {
            lat_tenth_microdeg: 407_440_000,
            lon_tenth_microdeg: -739_900_000,
            elevation: 4_096,
        },
        mbr_type: MbrType::Implausible,
        subtypes: vec![Subtype::MaxSpeed { threshold_mps: 90 }],
        evidence: Evidence::Suspect(SignedBsms {
            bsms: vec![bsm],
            cert,
        }),
    };
    home.submit_mbr(reporter, &report).expect("a real report");
    home.run().expect("runs");
    assert!(
        home.state.pdu_rejections.is_empty(),
        "{:?}",
        home.state.pdu_rejections
    );
    assert_eq!(home.state.ma.subject(0).and_then(|s| s.issuer), Some(id1));
    // The issuance records no longer say which instance issued it.
    home.state.pca.issued_by.clear();
    let (res, _) = home.revoke(0, 0, 2).expect("held");
    home.run().expect("runs");
    let (ma, aca1) = (home.state.nodes.ma, home.state.aca_extra[0]);
    let asked: Vec<NodeId> = home
        .kernel
        .steps
        .iter()
        .filter(|s| s.run == res && s.step == "pca-lookup-request" && s.from == ma)
        .map(|s| s.to)
        .collect();
    assert_eq!(asked, vec![aca1], "the ACA the issuer names");
    assert!(home.state.ra.blocklist.contains(&accused));
}

/// `bsm` as `crypto_mode: modeled` airs it: the signature's r and s (the SPDU's last 64
/// octets, after the point's choice tag) replaced by the token of the holder's modelled
/// backend, which also records the key relying parties reconstruct.
fn modelled_token_on(
    bsm: &v2xw_msg::sec_types::ieee1609_dot2::Ieee1609Dot2Data,
    imp: &v2xw_proto::scms::msg::ImplicitCredential,
    issuer: &v2xw_msg::sec_types::ieee1609_dot2::Certificate,
) -> v2xw_msg::sec_types::ieee1609_dot2::Ieee1609Dot2Data {
    use v2xw_sec::crypto::{CryptoBackend, Modeled};
    use v2xw_sec::primitive::PrimitiveId;
    use v2xw_sec::sig1609;
    let key = v2xw_sec::credential::ImplicitKey::new(
        imp.certificate.clone(),
        std::sync::Arc::new(issuer.clone()),
        imp.private,
    );
    let mut m = Modeled::new();
    let handle = m
        .import_implicit(PrimitiveId::ECDSA_P256_SHA256, NodeId::new(1), &key)
        .expect("the modelled backend imports it");
    let o = sig1609::open_signed(bsm).expect("signed");
    let input = sig1609::canonical_cert_coer(&imp.certificate).expect("encodes");
    let digest: [u8; 32] = sig1609::digest_1609(o.hash, &o.tbs_coer, &input)
        .try_into()
        .expect("SHA-256");
    let mut ctx = v2xw_sec::testctx::TestCtx::new(1);
    let token = m
        .sign_prehashed(&mut ctx, &handle, &digest)
        .expect("signs")
        .bytes;
    let mut b = rasn::coer::encode(bsm).expect("encodes");
    let n = b.len();
    b[n - 64..n - 32].copy_from_slice(&token[..32]);
    b[n - 32..].copy_from_slice(&token[32..]);
    v2xw_proto::scms::mbr::decode_signed_bsm(&b).expect("still decodes")
}

/// A real report provider-a's MA opened about a provider-b certificate, shared with
/// provider-b's MA as the MA-to-MA share sends it (the opened report re-encrypted to the peer MA).
/// With `corrupt`, one byte of the evidence BSM's signature is flipped before the device
/// files it, and provider-a's MA forwards without its own check (it would refuse it);
/// with `modelled`, the BSM signature is the modelled backend's token and both MAs are told
/// so (the engine's `crypto_mode: modeled`). Returns the peer run, the share's run and the
/// re-encrypted length.
fn share_a_real_report(
    corrupt: bool,
    modelled: bool,
) -> (ScmsRun, v2xw_proto::stage::FlowRun, u32) {
    use v2xw_proto::scms::mbr::{self, Evidence, MbrType, MisbehaviorReport, SignedBsms, Subtype};
    let (mut home, mut peer) = runs();
    peer.set_evidence_signatures_modelled(modelled);
    home.set_evidence_signatures_modelled(modelled);
    home.state.ma.skip_evidence_check_for_test = corrupt;
    let (accused, reporter) = (NodeId::new(5_000_301), NodeId::new(5_000_302));
    peer.preload(accused, 0, 1, 2)
        .expect("provider-b provisions the accused");
    home.preload(reporter, 0, 1, 2)
        .expect("provider-a provisions the reporter");
    let (cert, key, imp) = {
        let imp = peer.state.devices[&accused]
            .credentials
            .values()
            .find_map(|c| c.implicit.as_ref())
            .expect("a pseudonym certificate");
        (
            (*imp.certificate).clone(),
            v2xw_sec::sig1609::Signer1609::p256(&v2xw_sec::ec::scalar_to_be32(&imp.private))
                .expect("a P-256 key"),
            imp.clone(),
        )
    };
    let mut bsm = mbr::sign_bsm(&[0x5a; 40], 1_000_000, &cert, &key).expect("signs");
    if modelled {
        // What a holder's modelled backend airs: the same SPDU with its token as r || s.
        let pki = peer.state.pki.clone().expect("the PDU layer");
        let issuer = pki.by_id8(&imp.aca_id8).expect("the ACA").cert.clone();
        bsm = modelled_token_on(&bsm, &imp, &issuer);
    }
    if corrupt {
        // The signature is the SignedData's last field: flip its last octet.
        let mut b = rasn::coer::encode(&bsm).expect("encodes");
        *b.last_mut().expect("bytes") ^= 0x01;
        bsm = mbr::decode_signed_bsm(&b).expect("still decodes");
    }
    let report = MisbehaviorReport {
        generation_time: 600_000,
        location: v2xw_sec::envelope::GenerationLocation {
            lat_tenth_microdeg: 407_440_000,
            lon_tenth_microdeg: -739_900_000,
            elevation: 4_096,
        },
        mbr_type: MbrType::Implausible,
        subtypes: vec![Subtype::MaxSpeed { threshold_mps: 90 }],
        evidence: Evidence::Suspect(SignedBsms {
            bsms: vec![bsm],
            cert,
        }),
    };
    home.submit_mbr(reporter, &report).expect("a real report");
    home.run().expect("runs");
    assert!(
        home.state.pdu_rejections.is_empty(),
        "{:?}",
        home.state.pdu_rejections
    );
    assert_eq!(
        home.state.ma.opened_reports.len(),
        1,
        "provider-a's MA opened it"
    );
    assert_eq!(home.state.ma.opened_reports[0].evidence_verified, !corrupt);
    let subj = *home.state.ma.subject(0).expect("a subject");
    let target = peer.ma_share_target().expect("provider-b's MA certificate");
    let secured = home
        .reencrypt_report_for(0, &target)
        .expect("the opened report re-encrypts");
    assert!(
        secured.header.is_none(),
        "no Download-Req on the MA-to-MA leg"
    );
    let len = secured.report.len();
    let sent = home.state.ma.report_arrived[0];
    let run = peer.shared_report_at_ma(
        home.state.nodes.ma,
        subj.subject_i,
        subj.subject_lv,
        sent,
        sent,
        200,
        Some(secured),
    );
    peer.run_until(sent + 2_000_000_000).expect("runs");
    (peer, run, len)
}

/// [SM-MBR] §3 (the MA verifies the evidence it acts on): a report another
/// provider's MA shares crosses the MA-to-MA link as the report it opened, re-encrypted
/// to the receiving MA, which decrypts it and verifies the reporter's signature and the
/// evidence itself. A report whose evidence BSM's signature was corrupted is refused
/// there (a `SecuredMisbehaviorReport` rejection) and no case opens; the intact one is
/// opened, read from its own certificate (labelled shared) and decided. Under modelled
/// BSM signatures the MA checks the token as a modelled receiver does (decided): the
/// intact one verifies and the corrupt one is refused the same way. Fault: keep `secured:
/// None` on the share -> the corrupt report is acted on and the test goes red; the old
/// `!evidence_signatures_modelled` guard -> the modelled corrupt report is held, red;
/// `verify_modelled` false -> the modelled intact report is refused, red.
#[test]
fn a_shared_report_is_reverified_by_the_receiving_ma() {
    use v2xw_proto::scms::pdu::SpduKind;
    let (mut bad, run, _) = share_a_real_report(true, false);
    assert!(
        bad.state
            .pdu_rejections
            .iter()
            .any(|r| matches!(r.kind, SpduKind::SecuredMisbehaviorReport)),
        "the corrupt shared report was not refused: {:?}",
        bad.state.pdu_rejections
    );
    assert!(
        bad.state.ma.reports.is_empty(),
        "the corrupt report was held"
    );
    assert!(bad.state.ma.opened_reports.is_empty());
    assert!(bad.kernel.stages.at(run, StageId::ReportReceived).is_none());
    assert!(
        bad.request_revocation(0, 0, 2).is_none(),
        "a case opened on it"
    );
    assert_eq!(bad.state.shared.received, 1);

    let (mut good, run, len) = share_a_real_report(false, false);
    assert!(
        good.state.pdu_rejections.is_empty(),
        "{:?}",
        good.state.pdu_rejections
    );
    assert_eq!(good.state.ma.reports.len(), 1);
    assert_eq!(
        good.state.ma.opened_reports.len(),
        1,
        "provider-b's MA opened the shared report itself"
    );
    assert!(
        good.kernel
            .stages
            .at(run, StageId::ReportReceived)
            .is_some()
    );
    let s = *good.state.ma.subject(0).expect("a subject");
    assert_eq!(s.from, v2xw_proto::scms::run::SubjectFrom::Shared);
    assert!(s.issuer.is_some_and(|id8| good.state.issues_under(&id8)));
    let e = good.state.ma.report_evidence[0].clone().expect("evidence");
    assert!(e.shared);
    assert_eq!(e.verified, Some(true));
    // The bytes on the link are the re-encrypted report's.
    let step = good
        .kernel
        .steps
        .iter()
        .find(|s| s.run == run && s.step == "report-shared")
        .expect("the sharing leg");
    assert_eq!(step.bytes, len);
    assert!(good.request_revocation(0, s.subject_i, 2).is_some());

    let (modelled, _, _) = share_a_real_report(false, true);
    assert!(
        modelled.state.pdu_rejections.is_empty(),
        "{:?}",
        modelled.state.pdu_rejections
    );
    let e = modelled.state.ma.report_evidence[0]
        .clone()
        .expect("evidence");
    assert_eq!((e.shared, e.verified), (true, Some(true)));

    let (forged, _, _) = share_a_real_report(true, true);
    assert!(
        forged
            .state
            .pdu_rejections
            .iter()
            .any(|r| matches!(r.kind, SpduKind::SecuredMisbehaviorReport)),
        "a modelled report with a bad token was held"
    );
    assert!(forged.state.ma.reports.is_empty());
}

/// The same check on the MA's own path: a report from its own RA whose evidence does not verify is
/// refused as a shared one is, under real crypto. Fault: the check back to `shared_in &&` ->
/// the report is held with `verified: false` and a case can open, red.
#[test]
fn a_report_on_the_own_path_with_failed_evidence_is_refused() {
    use v2xw_proto::scms::mbr::{self, Evidence, MbrType, MisbehaviorReport, SignedBsms, Subtype};
    use v2xw_proto::scms::pdu::SpduKind;
    let (mut home, _peer) = runs();
    let (accused, reporter) = (NodeId::new(5_000_401), NodeId::new(5_000_402));
    home.preload(accused, 0, 1, 2).expect("the accused");
    home.preload(reporter, 0, 1, 2).expect("the reporter");
    let (cert, key) = {
        let imp = home.state.devices[&accused]
            .credentials
            .values()
            .find_map(|c| c.implicit.as_ref())
            .expect("a pseudonym certificate");
        (
            (*imp.certificate).clone(),
            v2xw_sec::sig1609::Signer1609::p256(&v2xw_sec::ec::scalar_to_be32(&imp.private))
                .expect("a P-256 key"),
        )
    };
    let bsm = mbr::sign_bsm(&[0x5a; 40], 1_000_000, &cert, &key).expect("signs");
    let mut b = rasn::coer::encode(&bsm).expect("encodes");
    *b.last_mut().expect("bytes") ^= 0x01;
    let bsm = mbr::decode_signed_bsm(&b).expect("still decodes");
    let report = MisbehaviorReport {
        generation_time: 600_000,
        location: v2xw_sec::envelope::GenerationLocation {
            lat_tenth_microdeg: 407_440_000,
            lon_tenth_microdeg: -739_900_000,
            elevation: 4_096,
        },
        mbr_type: MbrType::Implausible,
        subtypes: vec![Subtype::MaxSpeed { threshold_mps: 90 }],
        evidence: Evidence::Suspect(SignedBsms {
            bsms: vec![bsm],
            cert,
        }),
    };
    home.submit_mbr(reporter, &report).expect("a real report");
    home.run().expect("runs");
    assert!(
        home.state
            .pdu_rejections
            .iter()
            .any(|r| matches!(r.kind, SpduKind::SecuredMisbehaviorReport)),
        "failed evidence on the own path was not refused"
    );
    assert!(home.state.ma.reports.is_empty(), "the report was held");
}

/// A device's misbehaviour report uploaded while its RA is down is kept in the device's
/// store and uploaded again ([SM-MBR] §2.2: a report leaves the store once it has been
/// uploaded), so it reaches the MA after the RA is back. Under the US deployment there is
/// no Location Obscurer Proxy, and the proxy hop that stands for the device's access
/// network is a component, so the report is not dropped as a lost request (otherwise an
/// RA outage of 50 s loses 1,483 of 3,178 reports). Fault: the
/// `device_report` arm of `ScmsRun::step` removed -> the report is dropped and never
/// reaches the MA.
#[test]
fn a_report_uploaded_to_a_down_ra_arrives_when_it_is_back() {
    let (mut home, _) = runs();
    let reporter = NodeId::new(5_000_200);
    let accused = NodeId::new(5_000_201);
    home.preload(reporter, 0, 1, 2).expect("provisions");
    home.preload(accused, 0, 1, 2).expect("provisions");
    let (&(i, _), c) = home.state.devices[&accused]
        .credentials
        .iter()
        .next()
        .expect("a pseudonym");
    let lv = c.lv;
    let sent = 1_000_000_000;
    let back = sent + 60_000_000_000;
    home.set_faults(vec![v2xw_proto::fault::Fault {
        node: home.state.nodes.ra,
        kind: v2xw_proto::fault::FaultKind::Offline,
        from: sent,
        until: back,
    }]);
    let run = home.report_at_proxy(
        reporter,
        i,
        lv,
        sent,
        sent,
        sent + 50_000_000,
        v2xw_proto::net::Transport::CellularUu,
        200,
    );
    home.run_until(back + 2 * 86_400_000_000_000).expect("runs");
    assert_eq!(home.state.faults.dropped, 0, "{:?}", home.state.faults);
    assert!(home.state.faults.held >= 1, "{:?}", home.state.faults);
    assert_eq!(
        home.state.ma.reports.len(),
        1,
        "the report never reached the MA"
    );
    let at = home
        .kernel
        .stages
        .at(run, StageId::ReportReceived)
        .expect("arrived at the MA");
    assert!(at >= back, "{at}: received while the RA was down");
}

/// `pki.flow` opens a PDU as its recipient does: a request a device sealed to the RA's
/// certificate before a root rollover's changeover is opened under the retired certificate
/// (`ScmsPki::decrypt_as`), as the RA itself did. The conformance oracle found 108 reports in
/// the RA's day-long shuffle across a changeover that the record said could not be opened.
/// Fault: `open_pdu` back to the current certificate only -> this test red.
#[test]
fn the_flow_record_opens_what_was_sealed_before_a_rollover() {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[1].region = Region::assumed("us-west").expect("in the catalogue");
    let mut home = ScmsRun::new_deployed(p, 0, d).expect("builds");
    let label = home.state.gov.config.provider.clone();
    home.preload(NodeId::new(5_000_231), 0, 1, 2)
        .expect("provisions");
    let old = std::sync::Arc::clone(home.state.pki.as_ref().unwrap());
    let st = &mut home.state;
    let (_old, _new, publish) = st.gov.rollover_root(&st.rng, &st.nodes, 1_000);
    st.gov.publish_due(publish);
    let next = v2xw_proto::scms::run::pdu_pki_of(&st.gov, &label).expect("builds");
    assert!(st.follow_pki(&label, std::sync::Arc::new(old.superseded_by(next))));
    let mut tracker = v2xw_proto::scms::pki_view::PkiTracker::default();
    let flows = v2xw_proto::scms::pki_view::drain_flows(std::slice::from_ref(&home), &mut tracker);
    let req = flows
        .iter()
        .flat_map(|f| &f.pdus)
        .find(|p| p.kind == "EeRaCertRequestSpdu")
        .expect("the request sealed before the rollover");
    let opened = req.opened.as_ref().expect("the RA opens it");
    assert!(opened.error.is_none(), "{:?}", opened.error);
    assert!(opened.hex.is_some());
}
