//! ECDSA P-384 is charged its own published figure where a profile has one, and P-256
//! (flagged) where it does not (live-scms S384.verify, docs/validation/live-scms-2026-10-25.md
//! finding 3). Before, `signature_ops` kept the P-256 operations for P-384 on every
//! profile, so even `obu/generic-automotive-soc-no-hsm`, which publishes 8,104 us sign and
//! 5,721 us verify (R5 B.5), verified P-384 in P-256's 645 us.

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
    assert!(us(verify).is_some_and(|t| (t - 5_721.0).abs() < 1.0), "{:?}", us(verify));
    assert!(us(sign).is_some_and(|t| (t - 8_104.0).abs() < 1.0), "{:?}", us(sign));
    assert!(!p.op_cost_is_curve_proxy(verify));
    let p256 = p.op_cost("ecdsa-p256-verify").expect("priced").0;
    assert!(p.op_cost(verify).expect("priced").0 > p256 * 5);
}

#[test]
fn a_profile_without_p384_charges_p256_and_says_so() {
    let p = profiles::get(profiles::REFERENCE_OBU).expect("shipped");
    let (sign, verify) = signature_ops("ecdsa-p384");
    for (op, p256) in [(sign, "ecdsa-p256-sign"), (verify, "ecdsa-p256-verify")] {
        assert_eq!(p.priced_op(op), p256);
        assert!(p.op_cost_is_curve_proxy(op));
        assert_eq!(
            p.op_cost(op).map(|c| c.0),
            p.op_cost(p256).map(|c| c.0),
            "{op}"
        );
        let facts = p.op_cost_facts(op).expect("priced");
        assert!(
            facts.source.as_deref().is_some_and(|s| s.contains("charged as P-256")),
            "{facts:?}"
        );
    }
    assert!(!p.op_cost_is_curve_proxy("ecdsa-p256-verify"));
}
