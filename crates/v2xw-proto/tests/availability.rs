//! SC-P12.1: random backend outages, a two-state alternating renewal process per entity
//! (06-node-models §4), off by default.
//!
//! * over 10⁴ simulated hours the down share is within ±10 % of MTTR/(MTBF+MTTR), and the
//!   mean up and down times are within ±10 % of MTBF and MTTR;
//! * the same seed gives byte-identical outages, and an entity's outages do not move
//!   when another entity with a model is hosted beside it (keyed draws, no stream);
//! * the drawn outages are `offline` faults: the entity is down inside each, up outside,
//!   and its `outages` and `downtime_s` counters are the windows'.

use v2xw_core::ids::NodeId;
use v2xw_core::time::{Duration, SimTime};
use v2xw_proto::fault::FaultPlan;
use v2xw_proto::kernel::Kernel;
use v2xw_proto::net::BackendNet;
use v2xw_proto::service::{AvailabilityModel, ServiceModelSpec};

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
