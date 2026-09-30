//! A hybrid post-quantum `security.signature` changes the credential system, not only the
//! air: every certificate and signed backend message grows, every signature is two and
//! is charged as two, and — because there is no post-quantum butterfly — the device
//! generates and uploads one post-quantum key per pseudonym certificate
//! (`v2xw_proto::hybrid`).

mod common;

use common::{DEVICE_A, provisioned};
use v2xw_core::ids::NodeId;
use v2xw_proto::HybridScheme;
use v2xw_proto::etsi::{EtsiParams, EtsiRun};
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::run::ScmsRun;

const PERIODS: u32 = 2;
const JMAX: u32 = 5;
const CERTS: u64 = (PERIODS * JMAX) as u64;

fn scms(signature: &str) -> ScmsRun {
    let params = ScmsParams {
        hybrid: HybridScheme::from_signature(signature),
        ..ScmsParams::default().quick()
    };
    let mut run = ScmsRun::new(params).expect("certificates encode");
    run.add_device(DEVICE_A);
    provisioned(&mut run, DEVICE_A, 0, PERIODS, JMAX);
    run
}

fn bytes_of(run: &ScmsRun, step: &str) -> u64 {
    run.kernel
        .steps
        .iter()
        .filter(|s| s.step == step)
        .map(|s| u64::from(s.bytes))
        .sum()
}

fn ops(run: &ScmsRun, node: NodeId, primitive: &str, kind: &str) -> u64 {
    run.kernel
        .ops
        .get(&(node, primitive, kind))
        .copied()
        .unwrap_or(0)
}

fn busy_ns(run: &ScmsRun, node: NodeId) -> u64 {
    run.kernel.queue(node).map_or(0, |q| q.busy().as_nanos())
}

#[test]
fn a_hybrid_scms_issues_the_same_certificates_and_pays_for_both_halves() {
    let classic = scms("ecdsa-p256");
    let hybrid = scms("hybrid-mldsa44-ecdsa-p256");
    let h = HybridScheme::from_signature("hybrid-mldsa44-ecdsa-p256").expect("hybrid");
    let pca = hybrid.state.nodes.pca;

    // The same credentials reach the device either way.
    let held = |r: &ScmsRun| r.state.devices[&DEVICE_A].credentials.len() as u64;
    assert_eq!(held(&classic), CERTS);
    assert_eq!(held(&hybrid), CERTS);

    // The request carries one wrapped post-quantum key per certificate, the RA forwards
    // them to the PCA, and every certificate downloaded carries a post-quantum key and
    // the PCA's post-quantum signature.
    let grew = |step: &str| bytes_of(&hybrid, step) - bytes_of(&classic, step);
    let upload = u64::from(h.key_upload_bytes());
    assert!(
        grew("provisioning-request") >= CERTS * upload,
        "request grew {} B, expected at least {}",
        grew("provisioning-request"),
        CERTS * upload
    );
    assert!(grew("cert-request") >= CERTS * upload);
    assert!(grew("batch-download") >= CERTS * u64::from(h.cert_extra_bytes()));

    // Classical: nothing post-quantum anywhere.
    assert!(
        classic
            .kernel
            .ops
            .keys()
            .all(|(_, p, _)| !p.contains("ml-dsa")),
        "a classical deployment charged a post-quantum operation"
    );
    // Hybrid: the device made one post-quantum key per certificate, the PCA signed each
    // one, and the device checked each signature on download.
    let mldsa = "primitive/ml-dsa-44";
    assert_eq!(ops(&hybrid, DEVICE_A, mldsa, "keygen"), CERTS);
    assert_eq!(ops(&hybrid, pca, mldsa, "sign"), CERTS);
    assert!(ops(&hybrid, DEVICE_A, mldsa, "verify") >= CERTS);
    // And the time is charged, not only counted.
    assert!(busy_ns(&hybrid, pca) > busy_ns(&classic, pca));
    assert!(busy_ns(&hybrid, DEVICE_A) > busy_ns(&classic, DEVICE_A));
}

/// Falcon-512's key generation is two orders of magnitude slower than its signing on a
/// small CPU (42 ms on a Cortex-A72), so a Falcon batch costs the device far more time to
/// request than an ML-DSA one, even though its certificates are smaller.
#[test]
fn a_falcon_batch_costs_the_device_its_key_generation() {
    let falcon = scms("hybrid-falcon512-ecdsa-p256");
    let mldsa = scms("hybrid-mldsa44-ecdsa-p256");
    assert!(bytes_of(&falcon, "batch-download") < bytes_of(&mldsa, "batch-download"));
    let f = busy_ns(&falcon, DEVICE_A);
    let m = busy_ns(&mldsa, DEVICE_A);
    let keygen = HybridScheme::from_signature("hybrid-falcon512-ecdsa-p256")
        .and_then(|h| h.op_time(v2xw_sec::primitive::PrimitiveOpKind::KeyGen, true))
        .expect("figure")
        .as_nanos();
    assert!(f > m, "falcon {f} ns vs ml-dsa {m} ns");
    assert!(
        f >= CERTS * keygen,
        "falcon device time {f} ns < {CERTS} key generations"
    );
}

const STATION: NodeId = NodeId::new(2_000);

fn etsi(signature: &str, butterfly: bool) -> EtsiRun {
    let params = EtsiParams {
        hybrid: HybridScheme::from_signature(signature),
        ..EtsiParams::default()
    };
    let mut run = EtsiRun::new(params).expect("encodes");
    run.add_station(STATION);
    run.enrol(STATION);
    run.run().expect("enrols");
    if butterfly {
        run.authorize_butterfly(STATION);
    } else {
        run.authorize(STATION);
    }
    run.run().expect("authorizes");
    run
}

fn etsi_ops(run: &EtsiRun, node: NodeId, primitive: &str, kind: &str) -> u64 {
    run.kernel
        .ops
        .get(&(node, primitive, kind))
        .copied()
        .unwrap_or(0)
}

#[test]
fn a_hybrid_ccms_ticket_costs_the_station_and_the_aa_both_halves() {
    let classic = etsi("ecdsa-p256", false);
    let hybrid = etsi("hybrid-mldsa44-ecdsa-p256", false);
    let aa = hybrid.nodes.aa;
    let mldsa = "primitive/ml-dsa-44";
    assert_eq!(classic.tickets_of(STATION), 1);
    assert_eq!(hybrid.tickets_of(STATION), 1);
    assert_eq!(etsi_ops(&classic, STATION, mldsa, "keygen"), 0);
    assert_eq!(etsi_ops(&hybrid, STATION, mldsa, "keygen"), 1);
    assert!(etsi_ops(&hybrid, aa, mldsa, "sign") >= 1);
    let up = |r: &EtsiRun| -> u64 {
        r.kernel
            .steps
            .iter()
            .filter(|s| s.from == STATION)
            .map(|s| u64::from(s.bytes))
            .sum()
    };
    let h = HybridScheme::from_signature("hybrid-mldsa44-ecdsa-p256").expect("hybrid");
    // The ticket request carries the post-quantum key; every signed upload the
    // post-quantum signature.
    assert!(up(&hybrid) >= up(&classic) + u64::from(h.pk_bytes + h.sig_bytes));
}

#[test]
fn a_hybrid_butterfly_authorization_uploads_one_key_per_ticket() {
    let classic = etsi("ecdsa-p256", true);
    let hybrid = etsi("hybrid-falcon512-ecdsa-p256", true);
    let batch = u64::from(EtsiParams::default().butterfly_batch);
    let h = HybridScheme::from_signature("hybrid-falcon512-ecdsa-p256").expect("hybrid");
    let request = |r: &EtsiRun| -> u64 {
        r.kernel
            .steps
            .iter()
            .filter(|s| s.step == "etsi-butterfly-authorization-request")
            .map(|s| u64::from(s.bytes))
            .sum()
    };
    assert!(request(&hybrid) >= request(&classic) + batch * u64::from(h.key_upload_bytes()));
    assert_eq!(
        etsi_ops(&hybrid, STATION, "primitive/falcon-512", "keygen"),
        batch
    );
}
