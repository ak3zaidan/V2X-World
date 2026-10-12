//! Roadside units price post-quantum signatures from a flagged software proxy, and
//! nothing classical changes.
//!
//! Before the proxy no RSU profile priced any post-quantum operation, so under a
//! post-quantum or hybrid scheme every RSU dropped every message it heard as
//! `verify_unpriced`.

use v2xw_node::profile::signature_ops;
use v2xw_node::profiles;

const RSUS: [&str; 2] = ["rsu/commsignia-its-rs4", "rsu/cohda-mk5-rsu"];

#[test]
fn every_rsu_prices_every_post_quantum_scheme_from_the_proxy() {
    for id in RSUS {
        let p = profiles::get(id).expect(id);
        // The MK5 RSU publishes no ECDSA cost, so it cannot do a hybrid's classical half.
        let hybrid: &[&str] = if id == "rsu/commsignia-its-rs4" {
            &["hybrid-mldsa44-ecdsa-p256"]
        } else {
            &[]
        };
        for scheme in ["ml-dsa-44", "ml-dsa-65", "falcon-512"]
            .iter()
            .chain(hybrid)
        {
            let (sign, verify) = signature_ops(scheme);
            for op in [sign, verify] {
                let (t, _) = p
                    .op_cost(op)
                    .unwrap_or_else(|| panic!("{id} prices no {op} under {scheme}"));
                assert!(t.as_nanos() > 0, "{id} {op}");
                assert!(
                    p.op_cost_is_pq_proxy(op),
                    "{id} {op} is not flagged as the proxy"
                );
                let facts = p.op_cost_facts(op).expect("facts");
                if v2xw_sec::scheme::op_components(op).is_none() {
                    assert_eq!(
                        facts.basis,
                        v2xw_node::profile::CostBasis::SoftwareProxy,
                        "{id} {op}"
                    );
                }
            }
        }
    }
}

#[test]
fn the_proxy_prices_nothing_classical() {
    // The MK5 RSU publishes no ECDSA figure at all; the proxy must not invent one.
    let mk5 = profiles::get("rsu/cohda-mk5-rsu").unwrap();
    assert!(mk5.op_cost("ecdsa-p256-sign").is_none());
    assert!(mk5.op_cost("ecdsa-p256-verify").is_none());
    assert!(!mk5.op_cost_is_pq_proxy("ecdsa-p256-verify"));
    // The RS4's ECDSA stays on its secure element.
    let rs4 = profiles::get("rsu/commsignia-its-rs4").unwrap();
    assert!(!rs4.op_cost_is_pq_proxy("ecdsa-p256-verify"));
    assert!(rs4.op_cost("ecdsa-p256-verify").is_some());
}

#[test]
fn ml_dsa_44_verify_on_the_proxy_is_the_cited_mk6_figure() {
    // NDSS 2024 Table V, Cohda MK6, Botan: verify 189 us (the only MK6 ML-DSA-44 row).
    let p = profiles::get("rsu/commsignia-its-rs4").unwrap();
    let (t, _) = p.op_cost("ml-dsa-44-verify").unwrap();
    assert_eq!(t.as_nanos(), 189_000);
}
