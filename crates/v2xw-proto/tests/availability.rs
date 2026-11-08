//! Random backend outages, a two-state alternating renewal process per entity, off by
//! default.
//!
//! * over 10⁴ simulated hours the down share is within ±10 % of MTTR/(MTBF+MTTR), and the
//!   mean up and down times are within ±10 % of MTBF and MTTR;
//! * the same seed gives byte-identical outages, and an entity's outages do not move
//!   when another entity with a model is hosted beside it (keyed draws, no stream);
//! * the drawn outages are `offline` faults: the entity is down inside each, up outside,
//!   and its `outages` and `downtime_s` counters are the windows'.
//!
//! Log-normal service times, off by default.
//!
//! * over 10⁵ admits the multiplier's median is within 1 % of 1 and its sample cv within
//!   5 % of the setting; the deterministic model is the cost table's time exactly;
//! * the same seed gives byte-identical service times, and an entity's draws are its own.

use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};
use v2xw_proto::fault::FaultPlan;
use v2xw_proto::kernel::{Kernel, Outbox};
use v2xw_proto::net::BackendNet;
use v2xw_proto::service::{AvailabilityModel, ServiceModelSpec, ServiceQueue, ServiceTimeModel};

const HOUR: u64 = 3_600_000_000_000;

fn model(mtbf_h: u64, mttr_min: u64) -> AvailabilityModel {
    AvailabilityModel {
        mtbf: Duration::from_secs(mtbf_h * 3_600),
        mttr: Duration::from_secs(mttr_min * 60),
    }
}

/// A kernel hosting `nodes`, each with `m`.
fn kernel(nodes: &[u32], m: AvailabilityModel) -> Kernel<()> {
    let mut k: Kernel<()> = Kernel::new(BackendNet::new());
    for &n in nodes {
        k.host(
            NodeId::new(n),
            &ServiceModelSpec::new(4, Duration::from_millis(1)).available(Some(m)),
            "test",
        );
    }
    k
}

fn bytes(w: &std::collections::BTreeMap<NodeId, Vec<(SimTime, SimTime)>>) -> Vec<u8> {
    let mut b = Vec::new();
    for (n, xs) in w {
        b.extend_from_slice(&n.index().to_le_bytes());
        for &(f, u) in xs {
            b.extend_from_slice(&f.to_le_bytes());
            b.extend_from_slice(&u.to_le_bytes());
        }
    }
    b
}

#[test]
fn outages_follow_the_mtbf_and_mttr() {
    // 24 h between failures, 60 min to repair: a down share of 1/25.
    let m = model(24, 60);
    let horizon: SimTime = 10_000 * HOUR;
    // Ten entities, about 4,000 cycles: the ±10 % band is several standard errors wide.
    let nodes: Vec<u32> = (10..20).collect();
    let w = kernel(&nodes, m).availability_windows(7, horizon);
    assert_eq!(w.len(), nodes.len(), "every entity fails at least once");
    let (mut down, mut ups, mut n) = (0u64, 0u64, 0u64);
    for xs in w.values() {
        let mut t = 0;
        for &(f, u) in xs {
            assert!(f > t, "windows are disjoint and ordered");
            ups += f - t;
            down += u - f;
            n += 1;
            t = u;
        }
    }
    let total = (horizon * nodes.len() as u64) as f64;
    let share = down as f64 / total;
    let want = m.down_share();
    assert!(
        (share - want).abs() <= 0.1 * want,
        "down share {share:.5} against {want:.5}"
    );
    let mean_down = down as f64 / n as f64;
    let mean_up = ups as f64 / n as f64;
    let (b, r) = (m.mtbf.as_nanos() as f64, m.mttr.as_nanos() as f64);
    assert!(
        (mean_down - r).abs() <= 0.1 * r,
        "mean down {mean_down} against {r}"
    );
    assert!(
        (mean_up - b).abs() <= 0.1 * b,
        "mean up {mean_up} against {b}"
    );
    // Always up without a model.
    let mut k: Kernel<()> = Kernel::new(BackendNet::new());
    k.host(
        NodeId::new(1),
        &ServiceModelSpec::new(1, Duration::ZERO),
        "test",
    );
    assert!(k.availability_windows(7, horizon).is_empty());
}

#[test]
fn the_same_seed_gives_the_same_outages() {
    let m = model(168, 30);
    let horizon: SimTime = 2_000 * HOUR;
    let a = kernel(&[5, 9], m).availability_windows(42, horizon);
    let b = kernel(&[5, 9], m).availability_windows(42, horizon);
    assert!(!a.is_empty());
    assert_eq!(bytes(&a), bytes(&b), "byte-identical");
    // Another entity with a model, hosted before them: theirs do not move.
    let c = kernel(&[2, 5, 9], m).availability_windows(42, horizon);
    for n in [5, 9] {
        assert_eq!(
            a[&NodeId::new(n)],
            c[&NodeId::new(n)],
            "entity {n} moved when entity 2 was added"
        );
    }
    // Another seed, other outages.
    let d = kernel(&[5, 9], m).availability_windows(43, horizon);
    assert_ne!(bytes(&a), bytes(&d));
}

#[test]
fn drawn_outages_are_offline_faults_and_counted() {
    let m = model(24, 30);
    let horizon: SimTime = 500 * HOUR;
    let k = kernel(&[3], m);
    let ra = NodeId::new(3);
    let w = k.availability_windows(1, horizon)[&ra].clone();
    let mut plan = FaultPlan::default();
    assert_eq!(k.apply_availability(&mut plan, 1, horizon), w.len());
    assert_eq!(plan.outages(ra), w.len() as u64);
    let down: u64 = w.iter().map(|(f, u)| u - f).sum();
    assert_eq!(plan.downtime(ra).as_nanos(), down);
    for &(f, u) in &w {
        assert!(plan.offline(ra, f) && plan.offline(ra, u - 1));
        assert!(!plan.offline(ra, f - 1) && !plan.offline(ra, u));
        assert_eq!(plan.back_online(ra, f), Some(u));
        assert!(plan.retry_reaches(ra, f).is_some_and(|t| t >= u));
    }
    assert!(
        !plan.offline(NodeId::new(4), w[0].0),
        "another entity stays up"
    );
}

// -------------------------------------------------------------------------------------
// Log-normal service times
// -------------------------------------------------------------------------------------

/// The service times of `n` requests, each arriving at an idle single-server queue
/// (1000 s apart, so none waits), with 9 ms of work and 1 ms of overhead under `model`.
fn service_times(model: ServiceTimeModel, seed: u64, entity: u64, n: u64) -> Vec<u64> {
    let spec = ServiceModelSpec::new(1, Duration::from_millis(1)).varied(model);
    let mut q = ServiceQueue::new(&spec).keyed(seed, entity);
    let gap = 1_000 * 1_000_000_000u64;
    let out: Vec<u64> = (0..n)
        .map(|i| {
            let at = i * gap;
            q.admit(at, Duration::from_millis(9)) - at
        })
        .collect();
    assert_eq!(q.waiting(), Duration::ZERO, "no request waited");
    out
}

/// `(median, coefficient of variation)` of the multipliers `t / 10 ms`.
fn median_and_cv(times: &[u64]) -> (f64, f64) {
    let mut m: Vec<f64> = times.iter().map(|&t| t as f64 / 1.0e7).collect();
    let n = m.len() as f64;
    let mean = m.iter().sum::<f64>() / n;
    let var = m.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / (n - 1.0);
    m.sort_by(f64::total_cmp);
    let median = m[m.len() / 2];
    (median, var.sqrt() / mean)
}

/// Over 10⁵ admits the multiplier's median is within 1 % of 1 (the deterministic time is
/// the median) and its sample coefficient of variation within 5 % of the setting, at
/// cv 0.25 and 1.0; the deterministic model gives the cost table's time exactly.
///
/// Fault injected (and reverted): `σ = cv` instead of `sqrt(ln(1 + cv²))` fails the cv
/// check at cv 1.0 (sample cv 1.31).
#[test]
fn service_times_are_lognormal_with_median_one() {
    for cv_milli in [250u32, 1_000] {
        let times = service_times(ServiceTimeModel::LogNormal { cv_milli }, 7, 3, 100_000);
        let (median, cv) = median_and_cv(&times);
        let want = f64::from(cv_milli) / 1000.0;
        assert!(
            (median - 1.0).abs() <= 0.01,
            "cv {want}: median multiplier {median}"
        );
        assert!((cv / want - 1.0).abs() <= 0.05, "cv {want}: sample cv {cv}");
    }
    let fixed = service_times(ServiceTimeModel::Deterministic, 7, 3, 1_000);
    assert!(
        fixed.iter().all(|&t| t == 10_000_000),
        "the cost table's time"
    );
    assert_eq!(ServiceTimeModel::default(), ServiceTimeModel::Deterministic);
    assert_eq!(
        ServiceModelSpec::new(1, Duration::ZERO).service_time,
        ServiceTimeModel::Deterministic,
        "off by default"
    );
}

/// A kernel hosting `nodes`, each a two-server entity with 1 ms of overhead and cv 0.5,
/// seeded with `seed` (before or after hosting), serving `n` requests each at 1 s
/// intervals, interleaved in node order. Returns each node's finish instants.
fn kernel_service(
    nodes: &[u32],
    seed: u64,
    seed_first: bool,
    n: u64,
) -> std::collections::BTreeMap<u32, Vec<SimTime>> {
    let mut k: Kernel<()> = Kernel::new(BackendNet::new());
    if seed_first {
        k.seed_service_times(seed);
    }
    let spec = ServiceModelSpec::new(2, Duration::from_millis(1))
        .varied(ServiceTimeModel::LogNormal { cv_milli: 500 });
    for &node in nodes {
        k.host(NodeId::new(node), &spec, "test");
    }
    if !seed_first {
        k.seed_service_times(seed);
    }
    let mut out = std::collections::BTreeMap::new();
    for i in 0..n {
        for &node in nodes {
            let at = i * 1_000_000_000;
            let done = k
                .dispatch(at, NodeId::new(node), Outbox::new("test"))
                .expect("hosted");
            out.entry(node).or_insert_with(Vec::new).push(done);
        }
    }
    out
}

/// The same seed gives byte-identical service times; a different seed different ones; an
/// entity's draws are its own, unchanged when another entity is hosted and served beside
/// it, and seeding before or after hosting is the same; two entities do not share draws.
///
/// Fault injected (and reverted): `Kernel::host` keying every queue as entity 0 fails the
/// "two entities do not share draws" assert.
#[test]
fn the_same_seed_gives_the_same_service_times() {
    let a = kernel_service(&[2, 5], 11, true, 200);
    let b = kernel_service(&[2, 5], 11, true, 200);
    assert_eq!(a, b, "byte-identical");
    let other = kernel_service(&[2, 5], 12, true, 200);
    assert_ne!(a[&5], other[&5], "the seed matters");
    let alone = kernel_service(&[5], 11, false, 200);
    assert_eq!(
        alone[&5], a[&5],
        "entity 5 did not move when entity 2 was added"
    );
    let spans = |v: &Vec<SimTime>| -> Vec<u64> {
        v.iter()
            .enumerate()
            .map(|(i, &d)| d - i as u64 * 1_000_000_000)
            .collect()
    };
    assert_ne!(
        spans(&a[&2]),
        spans(&a[&5]),
        "two entities do not share draws"
    );
}
