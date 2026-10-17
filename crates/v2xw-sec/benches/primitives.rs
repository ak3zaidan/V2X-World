//! WP-C4 host benchmark: the crate's own implementations of every primitive the cost
//! tables price, timed on the machine that runs the simulator.
//!
//! These numbers are **not** device costs and never enter a cost table: the host is not
//! the OBU being simulated. They exist for two things the tables cannot give:
//!
//! 1. **Ratios inside one implementation**, which check the recipes in
//!    `primitive.rs` — above all that an ECQV reconstruction (decompress `P_U`, one
//!    variable-base multiplication, one addition) costs about one ECDH, not one ECDSA
//!    verification, and how much the decompression the recipe leaves out adds.
//! 2. **The simulator's own throughput ceiling** in `crypto: real` mode.
//!
//! Run: `cargo bench -p v2xw-sec --bench primitives`. It prints a machine line (OS,
//! architecture, CPU brand — no host name or user name) and one line per operation:
//! the median of five timed runs, each at least 200 ms.

use std::hint::black_box;
use std::time::{Duration, Instant};

use p256::ecdsa::signature::hazmat::{PrehashSigner, PrehashVerifier};
use sha2::{Digest, Sha256, Sha384};
use v2xw_sec::{aes128, butterfly, ccm, ec, ecies, linkage};

fn median_ns(mut f: impl FnMut()) -> f64 {
    let mut runs = Vec::with_capacity(5);
    for _ in 0..5 {
        let mut n: u64 = 0;
        let t0 = Instant::now();
        while t0.elapsed() < Duration::from_millis(200) {
            for _ in 0..16 {
                f();
            }
            n += 16;
        }
        runs.push(t0.elapsed().as_nanos() as f64 / n as f64);
    }
    runs.sort_by(f64::total_cmp);
    runs[2]
}

fn cpu_brand() -> String {
    let out = std::process::Command::new("sysctl")
        .args(["-n", "machdep.cpu.brand_string"])
        .output();
    if let Ok(o) = out
        && o.status.success()
    {
        return String::from_utf8_lossy(&o.stdout).trim().to_string();
    }
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split(':').nth(1))
                .map(|v| v.trim().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn row(name: &str, ns: f64) {
    println!("{name:<44} {:>12.1} ns", ns);
}

fn main() {
    println!(
        "machine: os={} arch={} cpu=\"{}\" profile=bench (optimised)",
        std::env::consts::OS,
        std::env::consts::ARCH,
        cpu_brand()
    );

    // Hashes, at the 64-byte size class the openssl-speed rows use, and a 1 KB input.
    for n in [64usize, 1_024] {
        let m = vec![0xa5u8; n];
        row(
            &format!("sha-256 {n} B"),
            median_ns(|| {
                black_box(Sha256::digest(black_box(&m)));
            }),
        );
        row(
            &format!("sha-384 {n} B"),
            median_ns(|| {
                black_box(Sha384::digest(black_box(&m)));
            }),
        );
    }

    // AES: one block (the linkage / butterfly pattern), and CCM at two lengths.
    let key = [0x2bu8; 16];
    let block = [0x11u8; 16];
    row(
        "aes-128 one block",
        median_ns(|| {
            black_box(aes128::encrypt_block(black_box(&key), black_box(&block)));
        }),
    );
    let nonce = [0x5au8; 12];
    for n in [100usize, 1_000] {
        let pt = vec![0x33u8; n];
        row(
            &format!("aes-128-ccm encrypt {n} B"),
            median_ns(|| {
                black_box(ccm::encrypt(&key, &nonce, black_box(&pt)).expect("ccm"));
            }),
        );
    }

    // P-256: ECDSA, ECDH (variable-base multiplication), fixed-base multiplication, and
    // the ECQV reconstruction as a relying party performs it.
    let d = ec::scalar_from_be32(&[0x42u8; 32]).expect("scalar");
    let e = ec::scalar_from_be32(&[0x17u8; 32]).expect("scalar");
    let q = ec::Point::mul_base(&d);
    let q_ca = ec::Point::mul_base(&e);
    let pu_bytes = q.compressed().expect("finite");
    let sk = p256::ecdsa::SigningKey::from_bytes(&[0x42u8; 32].into()).expect("key");
    let vk = *sk.verifying_key();
    let digest = Sha256::digest(b"tbs");
    let sig: p256::ecdsa::Signature = sk.sign_prehash(&digest).expect("sign");
    let sign_ns = median_ns(|| {
        let s: p256::ecdsa::Signature = sk.sign_prehash(black_box(&digest)).expect("sign");
        black_box(s);
    });
    let verify_ns = median_ns(|| {
        black_box(vk.verify_prehash(black_box(&digest), &sig).is_ok());
    });
    let mul_ns = median_ns(|| {
        black_box(q.mul(black_box(&e)));
    });
    let mul_base_ns = median_ns(|| {
        black_box(ec::Point::mul_base(black_box(&e)));
    });
    let decompress_ns = median_ns(|| {
        black_box(ec::Point::from_sec1(black_box(&pu_bytes)).expect("point"));
    });
    let reconstruct_ns = median_ns(|| {
        let pu = ec::Point::from_sec1(black_box(&pu_bytes)).expect("point");
        black_box(pu.mul(&e).add(&q_ca));
    });
    row("ecdsa-p256 sign (prehashed)", sign_ns);
    row("ecdsa-p256 verify (prehashed)", verify_ns);
    row("p256 variable-base mul (ECDH)", mul_ns);
    row("p256 fixed-base mul (keygen)", mul_base_ns);
    row("p256 point decompression", decompress_ns);
    row("ecqv reconstruct (decompress+mul+add)", reconstruct_ns);
    println!(
        "ratio reconstruct/ecdh = {:.3}; reconstruct/ecdsa-verify = {:.3}; \
         decompress/ecdh = {:.3}",
        reconstruct_ns / mul_ns,
        reconstruct_ns / verify_ns,
        decompress_ns / mul_ns
    );

    // ECIES (1609.2 §5.3.5) both ways.
    let p1 = ecies::p1(ecies::RecipientInfoKind::RekRecipInfo, &[]);
    let wrapped = ecies::encrypt(&q, &key, &p1, &e).expect("ecies");
    row(
        "ecies-p256 encrypt",
        median_ns(|| {
            black_box(ecies::encrypt(&q, black_box(&key), &p1, &e).expect("ecies"));
        }),
    );
    row(
        "ecies-p256 decrypt",
        median_ns(|| {
            black_box(ecies::decrypt(&d, black_box(&wrapped), &p1).expect("ecies"));
        }),
    );

    // P-384 (elector / root curve).
    let sk384 = p384::ecdsa::SigningKey::from_bytes(&[0x42u8; 48].into()).expect("key");
    let vk384 = *sk384.verifying_key();
    let d384 = Sha384::digest(b"tbs");
    let sig384: p384::ecdsa::Signature = sk384.sign_prehash(&d384).expect("sign");
    row(
        "ecdsa-p384 sign (prehashed)",
        median_ns(|| {
            let s: p384::ecdsa::Signature = sk384.sign_prehash(black_box(&d384)).expect("sign");
            black_box(s);
        }),
    );
    row(
        "ecdsa-p384 verify (prehashed)",
        median_ns(|| {
            black_box(vk384.verify_prehash(black_box(&d384), &sig384).is_ok());
        }),
    );

    // SCMS constructions.
    let ck: butterfly::ExpansionKey = [0x01u8; 16];
    row(
        "butterfly f1(i,j) (3 AES)",
        median_ns(|| {
            black_box(butterfly::f1(black_box(&ck), 7, 3));
        }),
    );
    let seed = linkage::LinkageSeed::new([0x99u8; 16]);
    let la = linkage::LaId(0x1234);
    row(
        "linkage seed step (1 SHA-256)",
        median_ns(|| {
            black_box(linkage::linkage_seed_next(la, black_box(seed)));
        }),
    );
    row(
        "pre-linkage value (1 AES, rekeyed)",
        median_ns(|| {
            black_box(linkage::pre_linkage_value(la, black_box(seed), 5));
        }),
    );
}
