//! What post-quantum cryptography and thresholding cost the credential system, from runs
//! of the SCMS deployment (not from formulas): each table row is a fresh `ScmsRun`.
//!
//! ```sh
//! cargo run -p v2xw-proto --example crypto_costs
//! ```
//!
//! Part 1 lists every registered signature scheme's sizes and the per-operation times
//! the credential system charges. Part 2 provisions one device with one CAMP i-period
//! (20 certificates) under each scheme and KEM and reports the bytes and time it took.
//! Part 3 does the same with the PCA as a 3-of-5 committee under each registered
//! protocol, healthy and with members failing. Part 4 revokes a device with two-of-two
//! and 3-of-5 linkage, healthy and with servers down.

use v2xw_core::ids::NodeId;
use v2xw_core::time::Duration;
use v2xw_proto::fault::{Fault, FaultKind};
use v2xw_proto::hybrid::{HybridScheme, KemSpec};
use v2xw_proto::net::Transport;
use v2xw_proto::scms::params::{ScmsParams, ThresholdDeployment};
use v2xw_proto::scms::run::ScmsRun;
use v2xw_proto::stage::FlowId;
use v2xw_proto::threshold::ProtocolParams;
use v2xw_sec::primitive::PrimitiveOpKind;
use v2xw_sec::scheme::SIGNATURE_SCHEMES;

const DEVICE: NodeId = NodeId::new(1_000);
const REPORTER: NodeId = NodeId::new(1_001);
/// CAMP's certificates per i-period [CAMP-EE Table 2.1.2.6.2].
const JMAX: u32 = 20;

fn ms(d: Duration) -> f64 {
    d.as_nanos() as f64 / 1e6
}

fn us(d: Option<Duration>) -> String {
    d.map_or("—".to_string(), |d| format!("{:.0}", d.as_nanos() as f64 / 1e3))
}

struct Provisioned {
    run: ScmsRun,
    latency: Duration,
}

fn provision(params: ScmsParams, faults: impl Fn(&ScmsRun) -> Vec<Fault>) -> Provisioned {
    let mut run = ScmsRun::new(params.quick()).expect("certificates encode");
    run.add_device(DEVICE);
    run.add_device(REPORTER);
    let f = faults(&run);
    run.set_faults(f);
    run.enrol(DEVICE);
    run.run().expect("enrolment");
    let t0 = run.kernel.now();
    run.provision(DEVICE, 0, 1, JMAX);
    run.run().expect("provisioning");
    // Installed: the device's last stage of the flow.
    let installed = run
        .kernel
        .stages
        .stamps()
        .iter()
        .filter(|s| s.flow == FlowId::Provisioning)
        .map(|s| s.t)
        .max()
        .unwrap_or(t0);
    Provisioned {
        latency: Duration::from_nanos(installed.saturating_sub(t0)),
        run,
    }
}

fn step_bytes(run: &ScmsRun, step: &str) -> u64 {
    run.kernel
        .steps
        .iter()
        .filter(|s| s.step == step)
        .map(|s| u64::from(s.bytes))
        .sum()
}

fn busy(run: &ScmsRun, node: NodeId) -> Duration {
    run.kernel.queue(node).map_or(Duration::ZERO, |q| q.busy())
}

fn part1() {
    println!("## 1. Every registered signature scheme\n");
    println!(
        "| scheme | mode | quantum level | sig B | key B | +B/signature | +B/certificate | \
         authority sign µs | authority verify µs | device sign µs | device verify µs | device keygen µs |"
    );
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    for s in SIGNATURE_SCHEMES {
        let (per_sig, per_cert) = s.extra_octets();
        let h = HybridScheme::from_signature(s.id);
        let t = |k, device| h.and_then(|h| h.op_time(k, device));
        println!(
            "| `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            s.id,
            s.mode.as_str(),
            s.quantum_level(),
            s.sig_octets(),
            s.key_octets(),
            per_sig,
            per_cert,
            us(t(PrimitiveOpKind::Sign, false)),
            us(t(PrimitiveOpKind::Verify, false)),
            us(t(PrimitiveOpKind::Sign, true)),
            us(t(PrimitiveOpKind::Verify, true)),
            us(t(PrimitiveOpKind::KeyGen, true)),
        );
    }
    println!(
        "\nPost-quantum times only (the ECDSA half of a hybrid is charged on top). Authority = \
         Raspberry Pi 5 liboqs (an upper bound for a server); device = Cohda MK6, key generation \
         Raspberry Pi 4 liboqs. Parameter sets with no published figure are derived by cycle \
         ratio (v2xw_sec::primitive::CostRatio).\n"
    );
}

fn part2() {
    println!("## 2. Provisioning one device with one i-period (20 certificates)\n");
    println!(
        "| signature | KEM | request B (device→RA) | RA→PCA B | batch download B | backend B total | \
         uplink+downlink B | PCA busy ms | device busy ms | latency ms |"
    );
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|");
    let cells = [
        ("ecdsa-p256", "ecies-p256"),
        ("hybrid-falcon512-ecdsa-p256", "ecies-p256"),
        ("falcon-512", "ecies-p256"),
        ("hybrid-mldsa44-ecdsa-p256", "ecies-p256"),
        ("hybrid-mldsa44-ecdsa-p256", "hybrid-ecies-p256-ml-kem-768"),
        ("ml-dsa-44", "ecies-p256"),
        ("ml-dsa-44", "ml-kem-768"),
        ("hybrid-mldsa65-ecdsa-p256", "hybrid-ecies-p256-ml-kem-768"),
        ("ml-dsa-65", "ml-kem-768"),
        ("ml-dsa-87", "ml-kem-1024"),
        ("falcon-1024", "ml-kem-1024"),
        ("slh-dsa-sha2-128s", "ml-kem-768"),
        ("slh-dsa-sha2-128f", "ml-kem-768"),
    ];
    for (sig, kem) in cells {
        let k = KemSpec::from_kem(kem);
        let params = ScmsParams {
            hybrid: HybridScheme::from_signature(sig).map(|h| h.with_kem(k.as_ref())),
            kem: k,
            ..ScmsParams::default()
        };
        let p = provision(params, |_| Vec::new());
        let r = &p.run;
        let by = r.kernel.bytes_by_transport();
        let backend = by.get(&Transport::BackendNet).copied().unwrap_or(0);
        let access: u64 = by
            .iter()
            .filter(|(t, _)| **t != Transport::BackendNet)
            .map(|(_, b)| *b)
            .sum();
        assert_eq!(
            r.state.devices[&DEVICE].credentials.len(),
            JMAX as usize,
            "{sig}/{kem}: the batch was installed"
        );
        println!(
            "| `{sig}` | `{kem}` | {} | {} | {} | {backend} | {access} | {:.1} | {:.1} | {:.0} |",
            step_bytes(r, "provisioning-request"),
            step_bytes(r, "cert-request"),
            step_bytes(r, "batch-download"),
            ms(busy(r, r.state.nodes.pca)),
            ms(busy(r, DEVICE)),
            ms(p.latency),
        );
    }
    println!(
        "\nLatency includes the test deployment's one-minute shuffle window \
         (`ScmsParams::quick`), the same in every row; the differences are the scheme's.\n"
    );
}

fn committee(protocol: &'static str, preprocessed: bool) -> ThresholdDeployment {
    ThresholdDeployment {
        protocol,
        protocol_params: ProtocolParams {
            preprocessed,
            umbrella_leaves: JMAX,
            ..ProtocolParams::default()
        },
        crl: false,
        linkage: None,
        ..ThresholdDeployment::default()
    }
}

fn part3() {
    println!("## 3. A 3-of-5 PCA committee issuing the same batch\n");
    println!(
        "| protocol | members failing | issued | sessions | restarts | timeouts | invalid shares | \
         committee messages | committee B | session ms | provisioning ms |"
    );
    println!("|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let single = provision(ScmsParams::default(), |_| Vec::new());
    println!(
        "| single PCA (CAMP) | — | {} | — | — | — | — | — | — | — | {:.0} |",
        single.run.state.devices[&DEVICE].credentials.len(),
        ms(single.latency)
    );
    type Faults = fn(&ScmsRun) -> Vec<Fault>;
    let none: Faults = |_| Vec::new();
    let failures: [(&str, Faults); 4] = [
        ("none", none),
        ("1 offline", |r| {
            vec![Fault {
                node: r.committee_members("pca")[0],
                kind: FaultKind::Offline,
                from: 0,
                until: u64::MAX,
            }]
        }),
        ("1 malicious", |r| {
            vec![Fault {
                node: r.committee_members("pca")[0],
                kind: FaultKind::Malicious,
                from: 0,
                until: u64::MAX,
            }]
        }),
        ("3 offline", |r| {
            r.committee_members("pca")[..3]
                .iter()
                .map(|n| Fault {
                    node: *n,
                    kind: FaultKind::Offline,
                    from: 0,
                    until: u64::MAX,
                })
                .collect()
        }),
    ];
    for (protocol, pre, label) in [
        ("frost-p256", false, "frost-p256"),
        ("frost-p256", true, "frost-p256, preprocessed"),
        ("cggmp21-ecdsa", false, "cggmp21-ecdsa"),
        ("cggmp21-ecdsa", true, "cggmp21-ecdsa, presigned"),
        ("umbrella", false, "umbrella (k = 20, FROST)"),
    ] {
        for (fname, f) in failures {
            if fname == "3 offline" && protocol != "frost-p256" {
                continue;
            }
            let mut t = committee(protocol, pre);
            // Asking exactly a quorum makes an offline member cost a timeout.
            t.fanout_all = false;
            t.max_attempts = 2;
            t.retry_after = Duration::from_secs(5);
            let params = ScmsParams {
                threshold: Some(t),
                ..ScmsParams::default()
            };
            let p = provision(params, f);
            let s = p
                .run
                .state
                .committees
                .stats
                .get("pca")
                .copied()
                .unwrap_or_default();
            println!(
                "| {label} | {fname} | {} | {} | {} | {} | {} | {} | {} | {:.1} | {:.0} |",
                p.run.state.devices[&DEVICE].credentials.len(),
                s.sessions,
                s.restarts,
                s.timeouts,
                s.invalid_shares,
                s.messages,
                s.bytes,
                s.mean_latency_ns() as f64 / 1e6,
                ms(p.latency),
            );
        }
    }
    println!(
        "\nThe committee asks exactly a quorum (so an offline member costs a 500 ms timeout); \
         members are 20 ms one way from the PCA. 'session ms' is the committee's own time \
         inside the PCA's step.\n"
    );
}

fn part4() {
    println!("## 4. Revoking a device: two-of-two against 3-of-5 linkage\n");
    println!(
        "| linkage | servers down | CRL entry issued | resolution→published ms | \
         resolution bytes | linkage committee messages |"
    );
    println!("|---|---|---:|---:|---:|---:|");
    let cases: [(&str, Option<(u16, u16)>, u8); 5] = [
        ("two-of-two (CAMP)", None, 0),
        ("two-of-two (CAMP)", None, 1),
        ("3-of-5", Some((5, 3)), 0),
        ("3-of-5", Some((5, 3)), 2),
        ("3-of-5", Some((5, 3)), 3),
    ];
    for (label, linkage, down) in cases {
        let threshold = linkage.map(|l| ThresholdDeployment {
            pca: false,
            crl: false,
            linkage: Some(l),
            max_attempts: 2,
            retry_after: Duration::from_secs(5),
            ..ThresholdDeployment::default()
        });
        let mut run = ScmsRun::new(
            ScmsParams {
                threshold,
                ..ScmsParams::default()
            }
            .quick(),
        )
        .expect("encodes");
        run.add_device(DEVICE);
        run.add_device(REPORTER);
        run.enrol(DEVICE);
        run.run().expect("enrol");
        run.provision(DEVICE, 0, 1, 2);
        run.run().expect("provision");
        let now = run.kernel.now();
        let down_nodes: Vec<NodeId> = if linkage.is_some() {
            run.committee_members("linkage")[..usize::from(down)].to_vec()
        } else if down > 0 {
            vec![run.state.nodes.la2]
        } else {
            Vec::new()
        };
        let what = if linkage.is_some() {
            format!("{down} of 5 members")
        } else if down > 0 {
            "LA2".to_string()
        } else {
            "none".to_string()
        };
        run.set_faults(
            down_nodes
                .iter()
                .map(|n| Fault {
                    node: *n,
                    kind: FaultKind::Offline,
                    from: now,
                    until: u64::MAX,
                })
                .collect(),
        );
        let lv = run.state.devices[&DEVICE].credentials[&(0, 0)].lv;
        run.submit_report(REPORTER, 0, lv);
        run.run().expect("report");
        let t0 = run.kernel.now();
        let Some((resolution, _issuance)) = run.revoke(0, 0, 2) else {
            println!("| {label} | {what} | no case | — | — | — |");
            continue;
        };
        run.run().expect("revocation");
        let published = run
            .kernel
            .stages
            .stamps()
            .iter()
            .filter(|s| s.stage == v2xw_proto::stage::StageId::Published)
            .map(|s| s.t)
            .min();
        let bytes: u64 = run
            .kernel
            .steps
            .iter()
            .filter(|s| s.run == resolution)
            .map(|s| u64::from(s.bytes))
            .sum();
        let msgs = run
            .state
            .committees
            .stats
            .get("linkage")
            .map_or(0, |s| s.messages);
        println!(
            "| {label} | {what} | {} | {} | {bytes} | {msgs} |",
            if run.state.crlg.entries.is_empty() { "no" } else { "yes" },
            published.map_or("—".to_string(), |t| format!("{:.0}", (t - t0) as f64 / 1e6)),
        );
    }
    println!();
}

fn main() {
    part1();
    part2();
    part3();
    part4();
}
