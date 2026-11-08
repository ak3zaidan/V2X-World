//! ECDSA P-384 is charged its own published figure where a profile has one, and P-256
//! (flagged) where it does not. Before, `signature_ops` kept the P-256 operations for P-384 on
//! every
//! profile, so even `obu/generic-automotive-soc-no-hsm`, which publishes 8,104 us sign and
//! 5,721 us verify, verified P-384 in P-256's 645 us.

use v2xw_node::profile::signature_ops;
use v2xw_node::profiles;

#[test]
fn p384_signs_and_verifies_with_its_own_operations() {
    assert_eq!(
        signature_ops("ecdsa-p384"),
        ("ecdsa-p384-sign", "ecdsa-p384-verify")
    );
    assert_eq!(
        signature_ops("ecdsa-p256"),
        ("ecdsa-p256-sign", "ecdsa-p256-verify")
    );
}

#[test]
fn a_profile_that_publishes_p384_charges_it() {
    let p = profiles::get("obu/generic-automotive-soc-no-hsm").expect("shipped");
    let (_, verify) = signature_ops("ecdsa-p384");
    let (sign, _) = signature_ops("ecdsa-p384");
    let us = |op: &str| p.op_cost(op).map(|c| c.0.as_secs_f64() * 1e6);
    assert!(
        us(verify).is_some_and(|t| (t - 5_721.0).abs() < 1.0),
        "{:?}",
        us(verify)
    );
    assert!(
        us(sign).is_some_and(|t| (t - 8_104.0).abs() < 1.0),
        "{:?}",
        us(sign)
    );
    assert!(!p.op_cost_is_curve_proxy(verify));
    let p256 = p.op_cost("ecdsa-p256-verify").expect("priced").0;
    assert!(p.op_cost(verify).expect("priced").0 > p256 * 5);
}

/// A profile that publishes no P-384 figure no longer
/// charges P-384 exactly as P-256. It scales its own P-256 figure by the P-384 / P-256
/// cycle ratio on the catalogue reference that publishes both (the CRATON2's Pi 3B
/// Cortex-A53 anchor: verify 775.3 / 94.1 per s = 8.24x, sign 1,631.1 / 358.4 = 4.55x),
/// and its cost facts say so. Before, this test pinned the P-256 cost itself.
///
/// Fault injected (and reverted): `curve_ratio` returning 1.0 -> this test red.
#[test]
fn a_profile_without_p384_scales_p256_by_the_reference_ratio_and_says_so() {
    let p = profiles::get(profiles::REFERENCE_OBU).expect("shipped");
    let (sign, verify) = signature_ops("ecdsa-p384");
    for (op, p256, want) in [
        (sign, "ecdsa-p256-sign", 1_631.1 / 358.4),
        (verify, "ecdsa-p256-verify", 775.3 / 94.1),
    ] {
        assert_eq!(p.priced_op(op), p256);
        assert!(p.op_cost_is_curve_proxy(op));
        assert!(p.op_cost_is_derived(op), "{op}");
        let k = p.curve_ratio(op);
        assert!((k - want).abs() < 0.01, "{op}: ratio {k}, want {want}");
        let t = p.op_cost(op).expect("priced").0.as_nanos() as f64;
        let base = p.op_cost(p256).expect("priced").0.as_nanos() as f64;
        assert!(
            (t / base - k).abs() < 1e-6,
            "{op}: {t} ns against {base} ns"
        );
        let facts = p.op_cost_facts(op).expect("priced");
        assert!(
            facts
                .source
                .as_deref()
                .is_some_and(|s| s.contains("charged as P-256 x")),
            "{facts:?}"
        );
    }
    // Every other operation, and P-384 on a profile that prices it, is unscaled.
    assert_eq!(p.curve_ratio("ecdsa-p256-verify"), 1.0);
    assert!(!p.op_cost_is_curve_proxy("ecdsa-p256-verify"));
    let g = profiles::get("obu/generic-automotive-soc-no-hsm").expect("shipped");
    assert_eq!(g.curve_ratio(verify), 1.0);
}
