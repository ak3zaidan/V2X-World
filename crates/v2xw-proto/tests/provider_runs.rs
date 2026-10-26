//! WP-D1: one SCMS run per provider. Provider-b's run is built from the home run: the
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

    let device = NodeId::new(5_000_123);
    peer.preload(device, 0, 1, 2).expect("provisions");
    let dev = &peer.state.devices[&device];
    assert!(!dev.credentials.is_empty());
    for c in dev.credentials.values() {
        let imp = c.implicit.as_ref().expect("an implicit pseudonym");
        assert_eq!(imp.aca_id8, b_aca, "issued by provider-b's ACA");
        assert_ne!(imp.aca_id8, a_aca);
    }
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
    let run = peer.shared_report_at_ma(from, i, lv, sent, sent, 200);
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
    peer.shared_report_at_ma(home.state.nodes.ma, i, lv, sent, sent, 200);
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

/// Realism R-SHARE (docs/design/scms-real/testing/realism.md): the PKI view records a
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
    peer.shared_report_at_ma(home.state.nodes.ma, i, lv, sent, sent, 200);
    peer.run_until(sent + 2_000_000_000).expect("runs");
    let mut tracker = PkiTracker::default();
    let flows = pki_view::drain_flows(std::slice::from_ref(&peer), &mut tracker);
    let shared: Vec<_> = flows.iter().filter(|f| f.name == "report-shared").collect();
    assert_eq!(shared.len(), 1, "{flows:?}");
    assert_eq!(shared[0].from.as_deref(), Some("provider-a/ma/0"));
    assert_eq!(shared[0].to.as_deref(), Some("provider-b/ma/0"));
}

/// Realism R-OFFCTL (docs/design/scms-real/testing/realism.md): a provider whose root the
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

/// Audit-gov GOV.E3.name: an enrolment certificate names the provider whose ECA issued it
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

/// Audit-gov GOV.L.counters: each provider's Root CA and ICA count the certificates they
/// signed, not the other providers'. Provider-a (ISS form): self, ICA, PG, MA and the
/// root-issued CRL signer; provider-b (SAESOL S2X production form): self, ICA, PG, MA, and
/// its ICA the ECA, PCA, RA, both LAs, the DCM and the ICA-issued CRL signer. Fault (the
/// first form): the home view counted every provider's (9 and 14 at t0) and provider-b's
/// none (0). Provider-b's ICA count was 6 while its records showed 7: the DCM that signs
/// its enrolment traffic had a PDU-layer certificate only (GOV.L.onecert).
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
    assert_eq!(
        p.ica_certs_issued, 7,
        "provider-b's ICA: ECA, PCA, RA, LA1, LA2, DCM, CRLG"
    );
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

/// Audit-gov GOV.L.onecert: each SCMS component holds one certificate, the one its
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

/// Audit-gov §4 item 2: a root rollover reaches the wire. Once the CTL approving the new
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

/// Audit-gov §4 item 5: each provider's component certificates carry the
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

/// Audit-gov §4 item 4: the enrolment request's DCM->ECA hop is on `pki.flow`. CAMP's
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
