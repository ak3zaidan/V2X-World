//! WP-34: BSM generation aligned to the LTE-V2X SPS reservation
//! (`radio.sidelink.align_generation`).
//!
//! On an LTE-V2X sidelink the BSM is carried on a semi-persistent reservation made
//! `T1..T2` subframes after a packet arrives (TS 36.213 V14.4.0 §14.1.1.6), and a BSM
//! generated on its own 100 ms clock then waits for it: on average half the reservation
//! period (Molina-Masegosa & Gozalvez 2017), which put the Midtown matrix's LTE p95 at
//! 113 ms against J3161/1's 100 ms budget (`scms-matrix.yaml` row `lte-latency-budget`).
//! SAE J3161/1 has the application generate the BSM just before its reserved subframe
//! [second-hand: Abrar et al. 2026, arXiv 2608.05087]. These tests hold the engine to that:
//!
//! * aligned, a BSM goes on the air its signing bound plus the margin after it was
//!   generated, give or take one subframe, and off it waits tens of milliseconds;
//! * a reselection moves the alignment to the new reservation;
//! * no interval the alignment changes is shorter than J2945/1's `vMinITT`;
//! * the same seed gives the same run.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use v2xw_engine::{Engine, MemoryRecorder, RunReport, Scenario};
use v2xw_metrics::channels::{ChannelView, NodeTxView};

fn scenarios() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("scenarios")
}

fn rooted(mut scenario: Scenario) -> Scenario {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..");
    if let v2xw_world::WorldSourceSpec::OsmXml { path, .. } = &mut scenario.world.source
        && Path::new(path).is_relative()
    {
        *path = root.join(&*path).to_string_lossy().into_owned();
    }
    scenario
}

/// Thirty vehicles on Manhattan for three seconds, LTE-V2X under SAE J3161/1, buildings
/// off so the medium is shared.
fn lte_fleet(aligned: bool) -> Scenario {
    let mut s = rooted(
        Scenario::load(scenarios().join("scale/base.yaml")).expect("the shipped scenario loads"),
    );
    s.time.duration_s = 3.0;
    s.actors.vehicles.demand.params = serde_json::json!({ "max_total_vehicles": 30 });
    s.world.buildings.enabled = false;
    s.radio.rat = serde_json::from_value(serde_json::json!("lte-v2x-pc5")).expect("a rat");
    if aligned {
        s.radio.sidelink.align_generation = Some(true);
    }
    s
}

fn run(scenario: Scenario) -> (RunReport, MemoryRecorder) {
    let errors = v2xw_engine::scenario::validate::validate(&scenario);
    assert!(errors.is_empty(), "the scenario validates: {errors:?}");
    let mut engine = Engine::build(scenario, "").expect("builds");
    let mut recorder = MemoryRecorder::new();
    let report = engine.run(&mut recorder).expect("runs");
    (report, recorder)
}

/// Every BSM's initial transmission: `(node, generated, on the air)`, ns.
fn bsm_initials(recorder: &MemoryRecorder) -> Vec<(u32, u64, u64)> {
    recorder
        .records()
        .iter()
        .filter(|(_, r)| r.channel == NodeTxView::CHANNEL)
        .map(|(_, r)| v2xw_metrics::channels::decode::<NodeTxView>(r).expect("decodes"))
        .filter(|v| v.msg_type.as_deref() == Some("bsm"))
        .filter(|v| v.radio.as_ref().and_then(|r| r.attempt).unwrap_or(1) == 1)
        .filter_map(|v| Some((v.node.index(), v.t_generated?, v.t)))
        .collect()
}

/// The lead the engine budgets: the default OBU's longest signing time plus the margin
/// (unset: the J3161/1 pool's `T1`, one 1 ms subframe). The test reads the bound from the
/// profile, as the engine does, rather than writing a number down.
fn lead_ns(s: &Scenario) -> u64 {
    let profile = v2xw_node::profiles::get(&s.nodes.default_obu).expect("a shipped OBU");
    let sign = v2xw_node::ProfileServiceModel::new(profile.clone())
        .service_time_bound(&v2xw_node::OpDescriptor::sign("ecdsa-p256-sign", 0))
        .expect("the OBU costs a signature");
    sign.as_nanos() + 1_000_000
}

const SUBFRAME_NS: u64 = 1_000_000;

/// The share of BSMs that went on the air within the lead plus one subframe of their
/// generation, and the mean generation-to-air time, ms.
fn on_time(recorder: &MemoryRecorder, lead: u64) -> (f64, f64, usize) {
    let tx = bsm_initials(recorder);
    let n = tx.len().max(1) as f64;
    let ok = tx
        .iter()
        .filter(|(_, g, t)| t.saturating_sub(*g) <= lead + SUBFRAME_NS)
        .count() as f64;
    let mean = tx
        .iter()
        .map(|(_, g, t)| t.saturating_sub(*g) as f64)
        .sum::<f64>()
        / n
        / 1e6;
    (ok / n, mean, tx.len())
}

/// Aligned, nearly every BSM is on the air its signing bound plus the margin after it was
/// generated (one subframe of rounding); off, almost none is, because the BSM waits for a
/// reservation that sits anywhere in the next 100 ms.
///
/// fault: alignment off (`sidelink_align_bsm` returns at once) -> red (the aligned run is
/// the off run).
#[test]
fn aligned_bsm_waits_at_most_the_margin() {
    let aligned = lte_fleet(true);
    let lead = lead_ns(&aligned);
    let (on, on_rec) = run(aligned);
    let (off, off_rec) = run(lte_fleet(false));
    let (share_on, mean_on, n_on) = on_time(&on_rec, lead);
    let (share_off, mean_off, n_off) = on_time(&off_rec, lead);
    eprintln!(
        "aligned: {n_on} BSMs, {share_on:.3} on time, mean {mean_on:.2} ms; off: {n_off}, \
         {share_off:.3}, mean {mean_off:.2} ms; lead {:.2} ms",
        lead as f64 / 1e6
    );
    assert!(n_on > 300 && n_off > 300, "{n_on} and {n_off} BSMs");
    assert!(
        share_on >= 0.85,
        "aligned, only {share_on:.3} of BSMs are on the air within lead + 1 subframe \
         (lead {} ms); mean generation-to-air {mean_on:.1} ms",
        lead as f64 / 1e6
    );
    assert!(
        share_off <= 0.2 && mean_off > 3.0 * mean_on,
        "off: {share_off:.3} on time, mean {mean_off:.1} ms against aligned {mean_on:.1} ms"
    );
    let sl = on
        .sidelink
        .as_ref()
        .expect("an LTE run reports its sidelink");
    let counted = sl.bsm_aligned + sl.bsm_unaligned;
    assert!(
        counted > 300 && sl.bsm_aligned as f64 >= 0.85 * counted as f64,
        "aligned {} of {counted}",
        sl.bsm_aligned
    );
    let off_sl = off
        .sidelink
        .as_ref()
        .expect("an LTE run reports its sidelink");
    assert_eq!(
        (
            off_sl.bsm_aligned,
            off_sl.bsm_unaligned,
            off_sl.bsm_realigned_on_reselection
        ),
        (0, 0, 0),
        "off counts nothing"
    );
}

/// With `probResourceKeep` 0 every counter expiry reselects (every 5 to 15 BSMs), and the
/// alignment follows each new reservation: the reselections are counted, and the BSMs
/// stay on time between them.
///
/// fault: the phase kept after a reselection (no realignment when the selection count
/// changes) -> red.
#[test]
fn alignment_follows_a_reselection() {
    let mut s = lte_fleet(true);
    s.radio.sidelink.resource_keep_probability = Some(0.0);
    let lead = lead_ns(&s);
    let (report, rec) = run(s);
    let sl = report
        .sidelink
        .as_ref()
        .expect("an LTE run reports its sidelink");
    assert!(
        sl.bsm_realigned_on_reselection >= 20,
        "only {} realignments after reselection",
        sl.bsm_realigned_on_reselection
    );
    let (share, mean, n) = on_time(&rec, lead);
    eprintln!(
        "keep 0: {n} BSMs, {share:.3} on time, mean {mean:.2} ms, {} realignments",
        sl.bsm_realigned_on_reselection
    );
    assert!(
        share >= 0.75,
        "with a reselection every 5-15 BSMs only {share:.3} of {n} BSMs are on time \
         (mean {mean:.1} ms)"
    );
}

/// No interval the alignment sets is shorter than J2945/1's `vMinITT` (100 ms): on a 50 ms
/// reservation interval (TS 36.331 `sl-ResourceReservationPeriod` allows 20, 50 and 100)
/// the next reserved subframe after a 10 Hz BSM is only 50 ms away, and the BSM waits for
/// the one 100 ms out instead. On the default 100 ms reservation the engine cannot shorten
/// an interval at all (a reselection happens when a block arrives, so the new reservation
/// is always at least a period after the last BSM), which is why the check runs at 50 ms.
///
/// fault: the lower clamp removed from `MessageSchedule::align_to` (`due = at`) -> red
/// (BSMs 50 ms apart).
#[test]
fn realigned_intervals_stay_inside_the_j2945_bounds() {
    let mut s = lte_fleet(true);
    s.radio.sidelink.reservation_interval_ms = Some(50);
    s.time.mobility_step_ms = 50;
    let (report, rec) = run(s);
    let mut by_node: BTreeMap<u32, Vec<u64>> = BTreeMap::new();
    for (node, g, _) in bsm_initials(&rec) {
        by_node.entry(node).or_default().push(g);
    }
    let tol = v2xw_node::generate::ALIGN_TOLERANCE.as_nanos();
    let mut intervals = 0;
    for (node, mut g) in by_node {
        g.sort_unstable();
        for w in g.windows(2) {
            intervals += 1;
            let d = w[1] - w[0];
            assert!(
                d + tol >= 100_000_000,
                "node {node}: BSMs {:.3} ms apart on a 50 ms reservation",
                d as f64 / 1e6
            );
        }
    }
    assert!(intervals > 300, "{intervals} intervals");
    let sl = report
        .sidelink
        .as_ref()
        .expect("an LTE run reports its sidelink");
    let counted = sl.bsm_aligned + sl.bsm_unaligned;
    assert!(
        sl.bsm_aligned as f64 >= 0.8 * counted as f64,
        "on a 50 ms reservation only {} of {counted} BSMs aligned",
        sl.bsm_aligned
    );
}

/// The same seed gives the same aligned run.
#[test]
fn an_aligned_run_is_deterministic() {
    let (a, ra) = run(lte_fleet(true));
    let (b, rb) = run(lte_fleet(true));
    assert_eq!(a.sidelink, b.sidelink);
    assert_eq!(bsm_initials(&ra), bsm_initials(&rb));
}

/// The loader refuses an alignment that would do nothing: on 802.11p (no reservation), with
/// every node generating at one instant (no phase to move), with a mobility step that does
/// not divide the reservation interval, and a margin without the alignment.
///
/// fault: the synchronised-timing refusal removed from `validate::sidelink_alignment` -> red.
#[test]
fn the_loader_refuses_an_alignment_that_would_do_nothing() {
    let refused = |s: &Scenario, path: &str| {
        v2xw_engine::scenario::validate::validate(s)
            .iter()
            .any(|e| format!("{e:?}").contains(path))
    };
    assert!(
        !refused(&lte_fleet(true), "align_"),
        "the aligned LTE fleet loads"
    );
    let mut dsrc = lte_fleet(true);
    dsrc.radio.rat = serde_json::from_value(serde_json::json!("dsrc-80211p")).expect("a rat");
    assert!(
        refused(&dsrc, "radio.sidelink"),
        "802.11p has no reservation"
    );
    let mut sync = lte_fleet(true);
    sync.messages.generator = Some(v2xw_engine::scenario::schema::ModelChoice {
        id: v2xw_msg::GENERATION_TIMING_ID.to_string(),
        params: serde_json::json!({ "phase_window_ms": 0, "max_jitter_ms": 0 }),
    });
    assert!(
        refused(&sync, "radio.sidelink.align_generation"),
        "synchronised timing"
    );
    let mut step = lte_fleet(true);
    step.radio.sidelink.reservation_interval_ms = Some(50);
    step.time.mobility_step_ms = 100;
    assert!(
        refused(&step, "radio.sidelink.align_generation"),
        "step 100 ms, RRI 50 ms"
    );
    let mut margin = lte_fleet(false);
    margin.radio.sidelink.align_margin_ms = Some(2.0);
    assert!(
        refused(&margin, "radio.sidelink.align_margin_ms"),
        "margin without alignment"
    );
}
