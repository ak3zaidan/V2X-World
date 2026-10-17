//! WP-D1: a run built from a deployment preset (00 §2.1; gap rows 41–42).

use v2xw_core::time::Duration;
use v2xw_proto::ScmsParams;
use v2xw_proto::scms::deployment::{Deployment, Preset};
use v2xw_proto::scms::run::ScmsRun;

fn us() -> ScmsRun {
    ScmsRun::new_deployed(
        ScmsParams::default().quick(),
        0,
        Deployment::preset(Preset::UsProduction2025),
    )
    .expect("the US deployment builds")
}

/// The preset's component counts are what the run hosts, and the CTL lists exactly the
/// preset's roots.
#[test]
fn us_production_hosts_both_providers_on_one_ctl() {
    let run = us();
    let st = &run.state;
    let d = &st.deployment;
    assert_eq!(st.gov.ctl.roots.len(), 2);
    for p in &d.providers {
        assert!(
            st.gov.root_of(&p.name).is_some(),
            "{} root on the CTL",
            p.name
        );
    }
    let hosted = run.kernel.hosted();
    let b = d.nodes(1);
    let online: Vec<_> = b
        .instances
        .iter()
        .filter(|(r, _, _)| r != "root" && r != "ica")
        .collect();
    // ECA, RA, ACA ×2, LA1, LA2, MA, CRLG, PG.
    assert_eq!(online.len(), 9);
    for (role, i, node) in &online {
        assert!(hosted.contains(node), "provider-b {role}#{i} hosted");
    }
    for (role, _, node) in b
        .instances
        .iter()
        .filter(|(r, _, _)| r == "root" || r == "ica")
    {
        assert!(!hosted.contains(node), "provider-b {role} is offline");
    }
    // Both of provider-b's ACAs sit behind its RA; the two MAs share reports.
    for (_, _, aca) in b.instances.iter().filter(|(r, _, _)| r == "aca") {
        assert!(run.kernel.net().link(b.online.ra, *aca).is_some());
    }
    let mm = run
        .kernel
        .net()
        .link(d.nodes(0).online.ma, b.online.ma)
        .expect("MA ↔ MA");
    assert_eq!(
        mm.latency,
        Duration::from_millis(1),
        "same assumed region: one link"
    );
    // The intra-region model replaces the flat 10 ms backend link.
    assert_eq!(st.params.backend_link_latency, Duration::from_millis(1));
    let home = d.nodes(0).online;
    assert_eq!(
        run.kernel.net().link(home.ra, home.pca).unwrap().latency,
        Duration::from_millis(1)
    );
    // No LOP is deployed: its hop is transparent.
    assert_eq!(
        run.kernel.net().link(home.lop, home.ra).unwrap().latency,
        Duration::ZERO
    );
    // Provider-b's PDU PKI is under its own governance root and ICA.
    let (label, pki) = &st.peer_pki[0];
    assert_eq!(label, "provider-b");
    assert_eq!(Some(&pki.root.cert), st.gov.peer_cert("provider-b", "root"));
    assert_eq!(Some(&pki.ica.cert), st.gov.peer_cert("provider-b", "ica"));
    let home_pki = st.pki.as_ref().unwrap();
    assert_ne!(pki.aca.id8(), home_pki.aca.id8());
}

/// `camp-reference` is today's run: `new_at` builds it, with one root, the flat 10 ms
/// links and the LOP hop.
#[test]
fn camp_reference_is_todays_run() {
    let run = ScmsRun::new(ScmsParams::default().quick()).unwrap();
    let st = &run.state;
    assert_eq!(st.deployment.preset, Preset::CampReference);
    assert_eq!(st.gov.ctl.roots.len(), 1);
    assert!(st.peer_pki.is_empty());
    let n = st.nodes;
    assert_eq!(
        run.kernel.net().link(n.lop, n.ra).unwrap().latency,
        ScmsParams::default().backend_link_latency
    );
}

/// A deployment that does not validate is refused.
#[test]
fn an_invalid_deployment_is_refused() {
    let mut d = Deployment::preset(Preset::UsProduction2025);
    d.providers[1].fleet_share = 0.7;
    assert!(ScmsRun::new_deployed(ScmsParams::default().quick(), 0, d).is_err());
}
