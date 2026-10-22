//! WP-D1 acceptance on a built engine: the default deployment is the two US providers on
//! one CTL, every node's trust reaches the other provider's ACA through its root, and a
//! provider kept off the CTL is untrusted (gap rows 41–42).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, Scenario};
use v2xw_msg::sec_types::Certificate;
use v2xw_sec::chain::{ChainPolicy, validate};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

fn load(deployment: Option<serde_json::Value>) -> Scenario {
    // Four signalised intersections with roadside units (a procedural grid: no map file),
    // on an SCMS backend.
    let mut s = Scenario::load(
        root()
            .join("scenarios")
            .join("connected-intersections.yaml"),
    )
    .expect("loads");
    s.time.duration_s = 2.0;
    s.threats.attackers.clear();
    s.security.protocol = Some(
        serde_json::from_value(serde_json::json!({"id": "protocol/scms/camp"})).expect("choice"),
    );
    s.security.scms = deployment.map(|d| {
        serde_json::from_value(serde_json::json!({"deployment": d})).expect("a deployment")
    });
    s
}

struct Trust {
    roots: Vec<Certificate>,
    cas: Vec<Certificate>,
    acas: Vec<Certificate>,
    warnings: Vec<String>,
}

fn trust(s: Scenario) -> Trust {
    let engine = Engine::build(s, "").expect("builds");
    let (roots, cas) = engine.phase2().expect("an SCMS run").trust_bundle();
    Trust {
        roots: roots.iter().map(|c| (**c).clone()).collect(),
        cas: cas.iter().map(|c| (**c).clone()).collect(),
        acas: engine
            .credential_issuers()
            .iter()
            .map(|c| (**c).clone())
            .collect(),
        warnings: engine.manifest().warnings.clone(),
    }
}

fn id8(c: &Certificate) -> [u8; 8] {
    v2xw_sec::hashedid::id8_bytes(&v2xw_sec::cert::cert_id8(c).expect("hashes"))
}

/// The path from `leaf` to a self-signed certificate through `pool`, by issuer digest.
fn path(leaf: &Certificate, pool: &[Certificate]) -> Vec<Certificate> {
    use v2xw_msg::sec_types::ieee1609_dot2::IssuerIdentifier;
    let mut out = vec![leaf.clone()];
    loop {
        let d = match &out.last().unwrap().0.issuer {
            IssuerIdentifier::sha256AndDigest(d) | IssuerIdentifier::sha384AndDigest(d) => {
                let mut b = [0u8; 8];
                b.copy_from_slice(&d.0[..]);
                b
            }
            _ => return out,
        };
        match pool.iter().find(|c| id8(c) == d) {
            Some(c) if out.len() < 8 => out.push(c.clone()),
            _ => return out,
        }
    }
}

/// Validates provider `k`'s ACA as a node of the run would: its path through the chain
/// file, under the 1609.2 chain rules, to a root the node installed.
fn aca_verifies(t: &Trust, k: usize) -> bool {
    let anchors: BTreeSet<[u8; 32]> = t
        .roots
        .iter()
        .map(|r| v2xw_sec::cert::hashed_id32(r).expect("hashes"))
        .collect();
    let mut pool = t.cas.clone();
    pool.extend(t.roots.iter().cloned());
    let chain = path(&t.acas[k], &pool);
    let start = t.acas[k].0.to_be_signed.validity_period.start.0.0;
    validate(&chain, &ChainPolicy::us_device(anchors, start + 3_600)).is_ok()
}

/// The default deployment is the US system: two providers, each with its own ACA whose
/// path every node validates to that provider's root on the CTL, and the manifest says
/// which assumptions the topology rests on. With provider-b's root off the CTL, every
/// node still trusts provider-a and no longer reaches provider-b's ACA.
#[test]
fn a_mixed_fleet_trusts_both_providers_and_only_through_the_ctl() {
    let both = trust(load(None));
    assert_eq!(both.acas.len(), 2, "one ACA per provider");
    assert_ne!(id8(&both.acas[0]), id8(&both.acas[1]));
    assert_eq!(both.roots.len(), 2, "both roots installed from the CTL");
    assert!(aca_verifies(&both, 0), "provider-a's ACA");
    assert!(aca_verifies(&both, 1), "provider-b's ACA, across providers");
    assert!(
        both.warnings
            .iter()
            .any(|w| w.starts_with("SCMS deployment us-production-2025")),
        "{:?}",
        both.warnings
    );
    assert!(
        both.warnings
            .iter()
            .any(|w| w.contains("ASSUMPTION: provider-b runs in region"))
    );

    let rogue = trust(load(Some(serde_json::json!({
        "providers": [{"name": "provider-b", "on_ctl": false}]
    }))));
    assert_eq!(rogue.acas.len(), 2, "the rogue provider still issues");
    assert_eq!(rogue.roots.len(), 1);
    assert!(aca_verifies(&rogue, 0));
    assert!(
        !aca_verifies(&rogue, 1),
        "provider-b is untrusted off the CTL"
    );
    assert!(rogue.warnings.iter().any(|w| w.contains("NOT on the CTL")));

    let camp = trust(load(Some(serde_json::json!({"preset": "camp-reference"}))));
    assert_eq!(
        (camp.acas.len(), camp.roots.len()),
        (1, 1),
        "today's single SCMS"
    );
}

/// `security.scms` is refused where it would do nothing, and a refused butterfly or an
/// unknown provider is refused at load.
#[test]
fn the_deployment_settings_are_refused_where_they_cannot_apply() {
    let refused = |d: serde_json::Value| {
        let s = load(Some(d));
        Engine::build(s, "").err().map(|e| e.to_string())
    };
    let e = refused(serde_json::json!({"options": {"butterfly": "unified"}})).expect("refused");
    assert!(e.contains("butterfly"), "{e}");
    let e = refused(serde_json::json!({"providers": [{"name": "provider-z"}]})).expect("refused");
    assert!(e.contains("provider-z"), "{e}");
    let e = refused(serde_json::json!({"providers": [{"name": "provider-b", "fleet_share": 0.9}]}))
        .expect("shares that do not sum to 1");
    assert!(e.contains("sum"), "{e}");
    let mut s = load(Some(serde_json::json!({"preset": "camp-reference"})));
    s.security.protocol = None;
    s.actors.backend.protocol = None;
    let e = Engine::build(s, "")
        .err()
        .expect("no SCMS: refused")
        .to_string();
    assert!(e.contains("security.scms"), "{e}");
}

/// R-NOCRL (testing/stress.md): every provider's CRL Generator publishes at `t0`, an
/// empty list when nothing stands on it, and renews it a week later, as a CRACA of a
/// deployment that has run for years does. Before, a Generator published nothing until
/// its first revocation, so the two-week overdue rule could never act on a CRACA that
/// had not revoked in the run.
#[test]
fn every_provider_publishes_its_crl_at_the_start() {
    let mut s = load(None);
    s.time.duration_s = 10.0;
    let mut engine = Engine::build(s, "").expect("builds");
    let runs = engine.phase2().expect("an SCMS run").scms_runs();
    assert_eq!(runs.len(), 2, "two providers");
    for (k, r) in runs.iter().enumerate() {
        let st = &r.state;
        assert!(st.crlg.entries.is_empty(), "provider {k}: nothing stands");
        let first = st
            .crlg
            .issued
            .iter()
            .find(|c| c.series == 1)
            .unwrap_or_else(|| panic!("provider {k} published no CRL at t0"));
        assert_eq!((first.at, first.entries, first.delta), (0, 0, false));
        assert_eq!(first.next_crl - first.issue_date, 7 * 86_400);
        assert!(
            st.crl_store.secured.as_ref().map(|p| p.bytes()) == Some(first.pdu.bytes()),
            "provider {k}: the store serves it from t0"
        );
    }
    let report = engine
        .run(&mut v2xw_engine::DigestRecorder::new())
        .expect("runs");
    // Every device joins holding both providers' lists, verified by its own trust (it
    // was on the road before the run), so the overdue rule has a list to act on.
    let devices = engine.phase2().unwrap().device_nodes();
    assert!(!devices.is_empty(), "devices joined");
    for d in devices {
        assert_eq!(engine.node_crls_held(d), 2, "{d:?} holds both t0 lists");
    }
    assert_eq!(report.phase2.crl_frames_rejected, 0);
}

/// R-TRUSTLEAK (testing/realism.md, matrix.md): with provider-b off the CTL, nothing it
/// certifies verifies at provider-a's receivers. Its roadside units' CRL frames (and the
/// reports relayed over the air) are sized, not encoded, and verified on the engine's
/// word alone: 7, then 280, provider-b messages verified at provider-a in the realism
/// runs. The engine now attaches the signer's certificate to every sized frame and each
/// receiver holds it to its own installed roots: here, no CRL frame is accepted
/// unchecked. The refusal under a dropped root is proven at the node
/// (`v2xw-node` tests/rx_validation.rs, the sized-frame tests).
#[test]
fn nothing_a_provider_off_the_ctl_certifies_verifies() {
    let mut s = load(Some(serde_json::json!({
        "providers": [{"name": "provider-b", "on_ctl": false}]
    })));
    s.time.duration_s = 30.0;
    // A standing list from t0 and every unit broadcasting it, so units of both providers
    // put sized CRL frames on the air from the start.
    s.security.protocol = Some(
        serde_json::from_value(serde_json::json!({
            "id": "protocol/scms/camp",
            "params": {"crl_background_entries": 3}
        }))
        .expect("choice"),
    );
    for r in &mut s.actors.rsus {
        r.roles.push("crl".into());
    }
    let mut engine = Engine::build(s, "").expect("builds");
    let report = engine
        .run(&mut v2xw_engine::DigestRecorder::new())
        .expect("runs");
    let p = &report.phase2;
    let get = |k: &str| p.verified_by_provider.get(k).copied().unwrap_or(0);
    assert!(p.crl_broadcasts > 0, "the units broadcast the list");
    // Every sized CRL frame a vehicle took off the air carried its signer's certificate
    // and was held to the vehicle's roots: none was accepted on the engine's word.
    assert_eq!(
        report.receiver_validation.chain_unchecked, 0,
        "{:?}",
        report.receiver_validation
    );
    assert!(
        get("provider-a<-provider-a") > 0,
        "{:?}",
        p.verified_by_provider
    );
    assert_eq!(
        get("provider-a<-provider-b"),
        0,
        "{:?}",
        p.verified_by_provider
    );
}
