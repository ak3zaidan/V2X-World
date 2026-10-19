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
