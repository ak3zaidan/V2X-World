//! The weekly pre-generation's cryptography runs on rayon's pool (honest-density.md §6:
//! the rollover was one thread of P-256 work), and must not change a byte: the ACA's
//! issuance draws from the run's counter generator, the RA's request signing draws a dummy
//! linkage value per certificate, and every other step is a pure function of its input.
//!
//! * `par_map_drbg` equals the sequential loop for items that draw a fixed number of blocks
//!   and for items that draw a varying number (the guessed starts are wrong and the tail is
//!   recomputed). Fault that turns it red: the guessed start without the `(k - 1)·stride`
//!   offset, or the misprediction check removed.
//! * A provisioning and two weekly top-offs of two devices with real bytes give the same
//!   SPDUs and credentials on one thread and on four, and the bytes the sequential code gave
//!   (pinned from `8becf72a`). Fault that turns it red: any reordering or a wrong generator
//!   block in the parallel loops of `run/provision.rs`.

mod common;

use common::{DEVICE_A, DEVICE_B, provisioned};
use v2xw_proto::scms::params::ScmsParams;
use v2xw_proto::scms::pdu::{Drbg, par_map_drbg};
use v2xw_proto::scms::run::ScmsRun;

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("pool")
}

/// Item `k` draws `draws(k)` blocks and returns them.
fn sequential(n: usize, draws: fn(usize) -> usize) -> (Vec<Vec<[u8; 32]>>, u64) {
    let mut g = Drbg::new(7, b"par");
    let _ = g.next32();
    let out = (0..n)
        .map(|k| (0..draws(k)).map(|_| g.next32()).collect())
        .collect();
    (out, g.position())
}

#[test]
fn the_parallel_map_draws_exactly_what_the_loop_draws() {
    fn fixed(_: usize) -> usize {
        3
    }
    fn varying(k: usize) -> usize {
        if k == 5 || k == 11 { 1 } else { 3 }
    }
    fn first_odd(k: usize) -> usize {
        if k == 0 { 2 } else { 4 }
    }
    for (name, draws) in [
        ("fixed", fixed as fn(usize) -> usize),
        ("varying", varying),
        ("first-odd", first_odd),
    ] {
        let want = sequential(20, draws);
        for threads in [1, 4] {
            let got = pool(threads).install(|| {
                let mut g = Drbg::new(7, b"par");
                let _ = g.next32();
                let v = par_map_drbg(&mut g, 20, |k, h| {
                    (0..draws(k)).map(|_| h.next32()).collect::<Vec<_>>()
                });
                (v, g.position())
            });
            assert_eq!(got, want, "{name} on {threads} thread(s)");
        }
    }
}

/// SHA-256 over every SPDU (kind, ends, time, bytes) and every credential the devices hold.
fn digest(run: &ScmsRun) -> String {
    let mut b = Vec::new();
    for p in &run.state.pdus {
        b.extend_from_slice(format!("{:?}|{}|{}|{}|", p.kind, p.from, p.to, p.at).as_bytes());
        if let Some(x) = &p.bytes {
            b.extend_from_slice(x.bytes());
        }
    }
    for d in [DEVICE_A, DEVICE_B] {
        for ((i, j), c) in &run.state.devices[&d].credentials {
            b.extend_from_slice(format!("{i}/{j}/{:?}|", c.lv).as_bytes());
        }
    }
    v2xw_core::hash::sha256_hex(&b)
}

fn provision_and_top_off() -> (String, usize) {
    let mut p = ScmsParams::default().quick();
    p.pdu.keep_bytes = true;
    let mut run = ScmsRun::new(p).expect("certificates encode");
    run.add_device(DEVICE_A);
    run.add_device(DEVICE_B);
    provisioned(&mut run, DEVICE_A, 614, 1, 20);
    provisioned(&mut run, DEVICE_B, 614, 1, 20);
    for week in 1..=2u32 {
        run.topup(DEVICE_A, 614 + week, 20);
        run.topup(DEVICE_B, 614 + week, 20);
        run.run().expect("runs");
    }
    assert!(
        run.state.pdu_rejections.is_empty(),
        "{:?}",
        run.state.pdu_rejections
    );
    let held = run.state.devices[&DEVICE_A].credentials.len()
        + run.state.devices[&DEVICE_B].credentials.len();
    (digest(&run), held)
}

#[test]
fn provisioning_gives_the_same_bytes_on_any_number_of_threads() {
    let one = pool(1).install(provision_and_top_off);
    let four = pool(4).install(provision_and_top_off);
    println!(
        "one thread {} ({} credentials); four {}",
        one.0, one.1, four.0
    );
    assert_eq!(one.1, 120, "two devices, three weeks of 20");
    assert_eq!(one, four);
    assert_eq!(one.0, PINNED, "the sequential code's bytes");
}

/// `provision_and_top_off`'s digest. Re-pinned for the RA shuffle (audit-prov
/// PV.aca-cannot-group: one RA→ACA request per message in a seeded order across devices,
/// and the weekly job for every account) and for one certificate per component (audit-gov
/// §4 item 1: every component certificate byte moved); one thread = four still. Before
/// both, `99f8481a…` on `8becf72a`. Re-pinned 2026-10-27 for the RA's past-due weeks
/// (`ScmsRun::generate_past_due`: the top-offs' weeks are generated at their due instant
/// on a scratch kernel, so their SPDUs come first, as pre-run ones, and the top-offs are
/// .info + download); `783c2b49…` before, one thread = four still.
const PINNED: &str = "c6e1e2bb1107fc07862ad5807bc0a7829cbc54fce891cebf8913b940e1ca813d";
