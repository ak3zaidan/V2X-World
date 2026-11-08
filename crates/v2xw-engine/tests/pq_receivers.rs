//! Receivers under the post-quantum profiles, on the post-quantum density scenarios
//! (scenarios/tests/pq-density-*.yaml) cut to their first half second.
//!
//! * The 47 `Invalid` verification states of both certificate-cycle runs were the first
//!   PSM each handset sent, judged by roadside units before their first own step: a unit
//!   with no application certificate (the post-quantum profile gives it none) held its CRL
//!   gate at i-period 0 until that step, and refused every linked certificate it heard as
//!   an implausible period. Two things now keep the gate in the current period, and
//!   removing either alone leaves this green: the `sync_credential_periods` call at the
//!   end of `Engine::create_rsus` and the `follow_clock` call in the node's `classify`.
//!   Fault that turns it red (2026-11-05): both removed (34 `Invalid` in this half
//!   second). The test took the units for the receivers that never transmit, and found
//!   none once relaying units sent WSAs; it now names them by `ROADSIDE_ID_BASE`.
//! * A VRU handset signed and verified at the ECDSA P-256 price whatever
//!   `security.signature` said. Fault that turns it red: `hosted::build_vru_device` without
//!   the scenario's `sign_op` and `verify_op`, or the handset profile without the
//!   `crypto_anchors` that price a post-quantum signature.
//! * A relaying unit's WSA was refused by every receiver (`psid-not-permitted`): outside
//!   the certificate cycle a unit signs with its factory certificate, which permitted
//!   only the unit's message services and not the WSA's PSID, so under
//!   `net.rsu_ip.air: sch` no vehicle verified a unit's WSA and none relayed a report
//!   through one (`pq_relay`: 0 relayed uploads). Fault that turns it red: `build_rsu`
//!   without the `rsu_signed_messages` stack, the factory certificate then permitting
//!   what `apply_security_profile` lists.

use std::collections::BTreeSet;
use std::path::Path;

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};

fn stress(name: &str) -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut s = Scenario::load(root.join("scenarios/tests").join(name)).expect("loads");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut s.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    s.time.duration_s = 0.5;
    s.time.warmup_s = None;
    s.time.fast_forward.clear();
    s
}

fn run(s: Scenario) -> (RunReport, MemoryRecorder) {
    let mut engine = Engine::build(s, "").expect("builds");
    let mut rec = MemoryRecorder::new();
    let report = engine.run(&mut rec).expect("runs");
    (report, rec)
}

fn records(rec: &MemoryRecorder, channel: &str) -> Vec<serde_json::Value> {
    rec.records()
        .iter()
        .filter(|(_, r)| r.channel == channel)
        .filter_map(|(_, r)| serde_json::from_slice(&r.json).ok())
        .collect()
}

#[test]
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
fn a_roadside_unit_judges_the_first_frames_it_hears_in_the_current_period() {
    let (report, rec) = run(stress("pq-density-falcon512-certcycle.yaml"));
    let states = &report.phase2.verification_states;
    let psm_tx: BTreeSet<u64> = records(&rec, "node.tx")
        .iter()
        .filter(|t| t["msg_type"] == "psm")
        .filter_map(|t| t["node"].as_u64())
        .collect();
    // The roadside units are the nodes from `ROADSIDE_ID_BASE` up. They are not the nodes
    // that never transmit: a relaying unit broadcasts its WSA (`net.rsu_ip.air: sch`, the
    // default), so a test that took the silent receivers for the units found none.
    let is_unit = |n: u64| n >= u64::from(v2xw_engine::run::ROADSIDE_ID_BASE);
    let unit_tx: BTreeSet<String> = records(&rec, "node.tx")
        .iter()
        .filter(|t| t["node"].as_u64().is_some_and(is_unit))
        .map(|t| t["msg_type"].to_string())
        .collect();
    let unit_psms = records(&rec, "node.verify")
        .iter()
        .filter(|v| v["msg_type"] == "psm")
        .filter(|v| v["node"].as_u64().is_some_and(is_unit))
        .count();
    println!(
        "{} handsets on the air; {unit_psms} PSM verifications at units (which sent \
         {unit_tx:?}); states {states:?}",
        psm_tx.len()
    );
    assert!(
        unit_psms > 0,
        "no unit judged a PSM, so this proves nothing"
    );
    let invalid: Vec<serde_json::Value> = records(&rec, "node.verify")
        .into_iter()
        .filter(|v| v.to_string().contains("nvalid"))
        .take(3)
        .collect();
    assert_eq!(
        states.get("Invalid").copied().unwrap_or(0),
        0,
        "honest frames judged Invalid, e.g. {invalid:?}"
    );
    // Every handset holds its provider's pre-run pool. With provider-b's `aca/1` unable to
    // open the LAs' sealed pre-linkage values, every provider-b pool failed and those
    // handsets signed with the period-0 bootstrap stand-in that every receiver refused;
    // the failure is a backend error, not a quiet one.
    assert_eq!(
        report.phase2.backend_errors, 0,
        "the backend refused: {}",
        report.phase2.first_backend_error
    );
    assert_eq!(report.phase2.vehicles_unprovisioned, 0);
}

#[test]
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
fn a_relaying_units_wsa_is_verified_under_the_post_quantum_profile() {
    let (report, rec) = run(stress("pq-density-falcon512.yaml"));
    let is_unit = |n: u64| n >= u64::from(v2xw_engine::run::ROADSIDE_ID_BASE);
    let wsa_rx: Vec<serde_json::Value> = records(&rec, "node.rx")
        .into_iter()
        .filter(|r| r["msg_type"] == "wsa")
        .filter(|r| r["rx"].as_u64().is_some_and(|n| !is_unit(n)))
        .collect();
    let verified = wsa_rx
        .iter()
        .filter(|r| r["outcome"] == "delivered" && r["verification"] == "verified")
        .count();
    let refused = wsa_rx
        .iter()
        .filter(|r| r["cause"] == "receiver-rule")
        .count();
    let rules = &report.receiver_validation.refused;
    println!(
        "{} WSA receptions at vehicles and handsets: {verified} verified, {refused} refused \
         by a receiver rule; refusals by rule {rules:?}",
        wsa_rx.len()
    );
    assert!(
        !wsa_rx.is_empty(),
        "no WSA reached a vehicle, so this proves nothing"
    );
    assert!(verified > 0, "no vehicle verified a unit's WSA");
    assert_eq!(refused, 0, "a receiver refused a unit's WSA");
    assert!(
        !rules.contains_key("psid-not-permitted"),
        "a certificate did not permit what it signed: {rules:?}"
    );
}

#[test]
#[cfg_attr(
    not(v2xw_map = "manhattan"),
    ignore = "needs worlds/cache/manhattan.osm.xml, which is gitignored"
)]
fn a_vru_handset_signs_and_verifies_at_the_scenario_signature_price() {
    let s = stress("pq-density-falcon512.yaml");
    let (sign_op, verify_op) =
        v2xw_node::profile::signature_ops(v2xw_engine::signature::message_signature(&s));
    let handset = v2xw_node::profiles::get(v2xw_node::profiles::GENERIC_VRU_DEVICE)
        .expect("the shipped handset profile");
    let (sign, _) = handset
        .op_cost(sign_op)
        .expect("the handset prices the scheme");
    let (ecdsa, _) = handset
        .op_cost("ecdsa-p256-sign")
        .expect("the handset prices ECDSA");
    assert!(
        sign > ecdsa,
        "{sign_op} is not dearer than ECDSA on a handset"
    );
    let (_, rec) = run(s);
    let handsets: BTreeSet<u64> = records(&rec, "node.tx")
        .iter()
        .filter(|t| t["msg_type"] == "psm")
        .filter_map(|t| t["node"].as_u64())
        .collect();
    let verify = records(&rec, "node.verify");
    // A task for a signer the handset cannot verify runs no primitive (reason
    // `unknown-signer`), so it names none and is not a verification.
    let at_handsets: Vec<&serde_json::Value> = verify
        .iter()
        .filter(|v| v["reason"].as_str() != Some(v2xw_node::policy::VERIFY_UNKNOWN_SIGNER))
        .filter(|v| v["node"].as_u64().is_some_and(|n| handsets.contains(&n)))
        .collect();
    let wrong: BTreeSet<String> = at_handsets
        .iter()
        .filter(|v| v["primitive"] != verify_op)
        .map(|v| v["primitive"].to_string())
        .collect();
    // A PSM's signing time is the handset's, from its first sign to its signed instant.
    let signing: Vec<u64> = records(&rec, "node.rx")
        .iter()
        .filter(|r| r["msg_type"] == "psm")
        .filter_map(|r| Some(r["t_signed"].as_u64()? - r["t_sign_start"].as_u64()?))
        .collect();
    let shortest = signing.iter().min().copied().unwrap_or(0);
    println!(
        "{} handsets, {} verifications at handsets, primitives other than {verify_op}: \
         {wrong:?}; PSM signing {shortest} ns at least, {sign_op} {} ns",
        handsets.len(),
        at_handsets.len(),
        sign.as_nanos()
    );
    assert!(!at_handsets.is_empty(), "no handset verified anything");
    assert!(wrong.is_empty(), "a handset verified with {wrong:?}");
    assert!(!signing.is_empty(), "no PSM was heard");
    assert!(
        shortest >= sign.as_nanos(),
        "a PSM was signed in {shortest} ns, under the {sign_op} time {} ns",
        sign.as_nanos()
    );
}
