//! The path-history rule at run time: what a pseudonym
//! change does to the path history in what the node puts on the air
//! (`NodeConfig::path_history_on_change`; SAE J2945/1 has no clearing rule, and §6.3.6.16
//! `DATAACC-039` asks for 200 m of history), and the random first `msgCnt` after start-up
//! (§6.3.6.2 `DATAACC-002`).
//!
//! # Faults injected to prove these checks can fail
//!
//! * The unset rule in `ObuRuntime::new` made `Clear` for every node ->
//!   `a_pseudonym_change_keeps_the_path_history` and
//!   `unset_keeps_on_a_bsm_sender_and_clears_on_a_cam_station` red.
//! * The start-up draw in `ObuRuntime::step_timed` removed ->
//!   `the_first_msg_count_is_random` red (every node's first BSM carries 1).

use v2xw_core::belief::{FixQuality, PositionEstimate};
use v2xw_core::geom::Vec3;
use v2xw_core::ids::NodeId;
use v2xw_core::rng::RngRegistry;
use v2xw_core::time::{Duration, NS_PER_MS, NS_PER_S, SimTime};
use v2xw_msg::MsgType;
use v2xw_msg::j2735::bsm;
use v2xw_msg::j2945::PathHistoryOnChange;
use v2xw_node::ctx::NodeRuntimeCtx;
use v2xw_node::generate::ServiceSet;
use v2xw_node::policy::VerifyAll;
use v2xw_node::runtime::{NodeConfig, ObuRuntime, Transmission};
use v2xw_node::stores::{CredState, CredentialHandle, RotationPolicy, pseudo_signer};

const BSM_ONLY: ServiceSet = ServiceSet {
    bsm: true,
    ..ServiceSet::NONE
};

/// The SCMS-issued key for `(node, i, j)`, as `runtime_behaviour.rs` makes it.
fn test_key(node: NodeId, i: u32, j: u32) -> v2xw_sec::credential::ImplicitKey {
    static ISSUER: std::sync::OnceLock<v2xw_sec::credential::ImplicitIssuer> =
        std::sync::OnceLock::new();
    let iss = ISSUER.get_or_init(|| {
        v2xw_sec::credential::ImplicitIssuer::standalone(b"node-tests", 0, &[0x20]).expect("issuer")
    });
    let mut secret = [0u8; 32];
    secret[..4].copy_from_slice(&node.index().to_be_bytes());
    let mut lv = [0u8; 9];
    lv[..4].copy_from_slice(&node.index().to_be_bytes());
    lv[4..8].copy_from_slice(&j.to_be_bytes());
    iss.issue_factory(
        v2xw_sec::credential::FactoryProfile::Pseudonym {
            lv: v2xw_sec::linkage::LinkageValue::new(lv),
        },
        &secret,
        i,
        j,
        0,
        &[0x20],
    )
    .expect("issues")
}

fn credential(node: NodeId, j: u32) -> CredentialHandle {
    CredentialHandle {
        digest: pseudo_signer(node, j),
        cert_coer: vec![0u8; 120],
        key: v2xw_sec::KeyId(u64::from(j)),
        i_period: 100,
        j_index: j,
        valid_from: 0,
        valid_until: 10_000 * NS_PER_S,
        state: CredState::Active,
    }
}

/// A fix at `pos`, heading `heading_rad`, `speed` m/s, taken at `t`.
fn fix(t: SimTime, pos: Vec3, heading_rad: f64, speed: f64) -> PositionEstimate {
    let mut p = PositionEstimate::no_fix(t);
    p.time_ns = t;
    p.pos = pos;
    p.vel = Vec3::new(speed * heading_rad.cos(), speed * heading_rad.sin(), 0.0);
    p.heading_rad = heading_rad;
    p.semi_major_m = 1.0;
    p.semi_minor_m = 1.0;
    p.fix = FixQuality::ThreeD;
    p
}

fn node(id: u32, config: NodeConfig, keys: &[v2xw_sec::credential::ImplicitKey]) -> ObuRuntime {
    let node = NodeId::new(id);
    let mut rt = ObuRuntime::new(
        node,
        v2xw_node::profiles::get(v2xw_node::profiles::REFERENCE_OBU)
            .expect("shipped profile")
            .clone(),
        Box::new(VerifyAll::new()),
        config,
        0,
    );
    rt.stores_mut().crl.set_period(100);
    for (j, k) in keys.iter().enumerate() {
        let j = u32::try_from(j).expect("few keys");
        rt.install_credential(credential(node, j), k)
            .expect("installs");
    }
    rt
}

fn crumbs(t: &Transmission) -> usize {
    let frame = t.signed.as_ref().expect("a generated BSM carries bytes");
    let decoded = bsm::decode_message_frame(&frame.payload).expect("decodes");
    decoded
        .part_ii
        .iter()
        .find_map(|p| match &p.value {
            bsm::PartIIValue::VehicleSafety(ext) => ext.path_history.as_ref(),
            _ => None,
        })
        .map_or(0, |h| h.crumb_data.len())
}

/// For each pseudonym change on a 40 s drive round a 50 m circle at 10 m/s (a change
/// every 3 s), the path-history points of the last BSM before it and the first after it.
fn crumbs_across_changes(on_change: Option<PathHistoryOnChange>) -> Vec<(usize, usize)> {
    let id = NodeId::new(1);
    let keys: Vec<_> = (0..4).map(|j| test_key(id, 100, j)).collect();
    let mut rt = node(
        1,
        NodeConfig {
            services: BSM_ONLY,
            path_history_on_change: on_change,
            ..NodeConfig::default()
        },
        &keys,
    );
    let certs = core::mem::take(&mut rt.stores_mut().certs);
    rt.stores_mut().certs = certs.with_policy(RotationPolicy {
        min_age: Duration::from_millis(3_050),
        min_distance_m: f64::INFINITY,
        require_both: false,
    });
    let reg = RngRegistry::new(7);
    let (radius, speed) = (50.0_f64, 10.0_f64);
    let mut bsms: Vec<Transmission> = Vec::new();
    for k in 0..400_u64 {
        let now = k * 100 * NS_PER_MS;
        let theta = speed * (now as f64 / 1e9) / radius;
        rt.set_belief(fix(
            now,
            Vec3::new(radius * theta.cos(), radius * theta.sin(), 0.0),
            theta + std::f64::consts::FRAC_PI_2,
            speed,
        ));
        let mut ctx = NodeRuntimeCtx::new(now, &reg);
        let o = rt.step(&mut ctx, Vec::new(), speed * 0.1);
        bsms.extend(
            o.transmissions
                .into_iter()
                .filter(|t| t.msg_type == MsgType::Bsm),
        );
    }
    let pairs: Vec<_> = bsms
        .windows(2)
        .filter(|w| w[1].signer != w[0].signer)
        .map(|w| (crumbs(&w[0]), crumbs(&w[1])))
        .collect();
    assert!(
        pairs.len() >= 8,
        "the drive changed pseudonym {} times",
        pairs.len()
    );
    pairs
}

/// Under the default (unset on a BSM sender: keep), the first BSM under a new pseudonym
/// carries the history the old one had built: no change loses a point. Under `clear` it
/// starts again from the one point every BSM carries (`BSMCONT-003/004`).
///
/// Fault: the unset rule made `Clear` -> the default loses its points at each change, red.
#[test]
fn a_pseudonym_change_keeps_the_path_history() {
    let kept = crumbs_across_changes(None);
    assert!(
        kept.iter().all(|&(before, after)| after >= before),
        "a change lost path-history points: {kept:?}"
    );
    assert!(
        kept.iter().any(|&(before, _)| before >= 5),
        "the drive never built a history to keep: {kept:?}"
    );
    let cleared = crumbs_across_changes(Some(PathHistoryOnChange::Clear));
    assert!(
        cleared.iter().all(|&(_, after)| after <= 1),
        "clear kept points across a change: {cleared:?}"
    );
    assert!(
        cleared.iter().any(|&(before, _)| before >= 2),
        "the clear run never had a history to lose: {cleared:?}"
    );
}

/// Unset, a node that sends BSMs keeps its history (J2945/1) and a CAM-only station clears
/// it (ETSI); a set value holds for either.
///
/// Fault: the unset rule made `Clear` -> the BSM sender clears, red.
#[test]
fn unset_keeps_on_a_bsm_sender_and_clears_on_a_cam_station() {
    let rule = |services: ServiceSet, set: Option<PathHistoryOnChange>| {
        node(
            1,
            NodeConfig {
                services,
                path_history_on_change: set,
                ..NodeConfig::default()
            },
            &[],
        )
        .own_vehicle()
        .path_history_on_change()
    };
    assert_eq!(rule(BSM_ONLY, None), PathHistoryOnChange::Keep);
    assert_eq!(rule(ServiceSet::BOTH, None), PathHistoryOnChange::Keep);
    let cam_only = ServiceSet {
        cam: true,
        ..ServiceSet::NONE
    };
    assert_eq!(rule(cam_only, None), PathHistoryOnChange::Clear);
    assert_eq!(
        rule(cam_only, Some(PathHistoryOnChange::Keep)),
        PathHistoryOnChange::Keep
    );
    assert_eq!(
        rule(BSM_ONLY, Some(PathHistoryOnChange::Clear)),
        PathHistoryOnChange::Clear
    );
}

/// Each node's first BSM after start-up carries a `msgCnt` drawn from its own keyed stream
/// (`node/msgcnt-startup`): over 1,000 nodes switched on together, at least 100 distinct
/// first values out of 128, where counting from 0 gives every node 1. The count on the air
/// is the count in the frame's extras, and the draw is the same on a second run.
///
/// Fault: no start-up draw -> every first value is 1, red.
#[test]
fn the_first_msg_count_is_random() {
    let key = test_key(NodeId::new(1), 100, 0);
    let first = |id: u32| {
        let mut rt = node(
            id,
            NodeConfig {
                services: BSM_ONLY,
                ..NodeConfig::default()
            },
            std::slice::from_ref(&key),
        );
        rt.set_belief(fix(0, Vec3::ZERO, 0.0, 10.0));
        let reg = RngRegistry::new(7);
        let mut ctx = NodeRuntimeCtx::new(0, &reg);
        let o = rt.step(&mut ctx, Vec::new(), 0.0);
        let t = o
            .transmissions
            .iter()
            .find(|t| t.msg_type == MsgType::Bsm)
            .expect("a BSM at start-up");
        let frame = t.signed.as_ref().expect("bytes");
        let on_air = bsm::decode_message_frame(&frame.payload)
            .expect("decodes")
            .core
            .msg_cnt;
        assert_eq!(t.bsm.msg_cnt, Some(on_air), "node {id}");
        on_air
    };
    let values: Vec<u8> = (1..=1_000).map(first).collect();
    let distinct: std::collections::BTreeSet<u8> = values.iter().copied().collect();
    assert!(
        distinct.len() >= 100,
        "{} distinct first msgCnt values over 1,000 nodes: {distinct:?}",
        distinct.len()
    );
    assert!(values.iter().all(|&v| v < 128), "MsgCount is 0..=127");
    assert_eq!(first(17), values[16], "the same seed draws the same value");
}
