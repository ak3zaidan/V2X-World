//! Layer 2: 802.11p EDCA, the channel busy ratio, ETSI and SAE congestion control, and the
//! LTE-V2X / NR-V2X sensing-based schedulers.

use v2xw_core::card::Tier;
use v2xw_core::ids::{FrameSeq, NodeId, SduId};
use v2xw_core::time::SimTime;
use v2xw_radio::dcc::{AdaptiveDcc, AdaptiveParams, J2945Params, ReactiveDcc, ReactiveTable, SaeJ2945Dcc};
use v2xw_radio::mac::{CbrMeter, EdcaOcbMac};
use v2xw_radio::phy::air_time;
use v2xw_radio::sidelink::{PoolConfig, ProbResourceKeep, Rri, SlRat, SlResource};
use v2xw_radio::sps::{SelectionReason, SpsEngine, SpsParams};
use v2xw_radio::traits::{Dcc, Mac};
use v2xw_radio::types::{
    AccessCategory, CcaState, ChannelId, FrameDescriptor, MacSdu, Mcs, ReactiveState, SduRef,
    timing,
};

use crate::ctx::TestCtx;
use crate::stats;
use crate::{Check, Cost, Layer, Mode, Outcome};

const CH: ChannelId = ChannelId::CCH;
const BUSY: CcaState = CcaState::Busy { energy_dbm: -70.0 };

/// The checks of this layer.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "ACC-01",
            layer: Layer::Access,
            title: "EDCA parameter set in OCB mode: slot, SIFS, CWmin/CWmax/AIFSN per access category, resulting AIFS, and the UP → AC mapping",
            reference: "ETSI EN 302 663 V1.3.1 Tables C.3-C.6 (from IEEE 802.11-2016 Tables 9-138 and 17-21): slot 13 µs, SIFS 32 µs; VO 3/7/2 → 58 µs, VI 7/15/3 → 71 µs, BE 15/1023/6 → 110 µs, BK 15/1023/9 → 149 µs",
            tolerance: "exact",
            fault: "AC_VI's AIFSN 2 instead of 3 (VI would contend with VO's AIFS)",
            cost: Cost::Fast,
            run: edca_parameters,
        },
        Check {
            id: "ACC-02",
            layer: Layer::Access,
            title: "The backoff draw is uniform on the integers [0, CWmin]",
            reference: "ETSI EN 302 663 V1.3.1 Annex C.4.2 i) ('draw an integer from a uniform distribution [0, CW] ... the total number of integers to draw from is CW+1'); χ² test over 16,000 AC_BE draws and 8,000 AC_VO draws",
            tolerance: "χ² p-value > 0.001 for each category",
            fault: "Off-by-one draw over [0, CW − 1] (the value CW never drawn)",
            cost: Cost::Fast,
            run: backoff_uniform,
        },
        Check {
            id: "ACC-03",
            layer: Layer::Access,
            title: "A busy medium suspends the backoff countdown; after AIFS it resumes from the slots still owed",
            reference: "ETSI EN 302 663 V1.3.1 Annex C.4.2 ('if the channel becomes busy during the decrease of the backoff value ... the node has to suspend the countdown ... after every busy channel period the node will first wait an AIFS before the decrementation resumes'); IEEE 802.11-2016 §10.3.4.3",
            tolerance: "the grant instant exact to the nanosecond, for 30 draws",
            fault: "The busy medium reported to the MAC late, at the instant its countdown would have expired",
            cost: Cost::Fast,
            run: backoff_freeze,
        },
        Check {
            id: "ACC-04",
            layer: Layer::Access,
            title: "Saturated single-hop broadcast: the collision probability of the EDCA state machine against the analytical broadcast model, for 2 to 20 stations",
            reference: "Bianchi, IEEE JSAC 18(3) 2000, Eq. 7 with m = 0 (no contention-window doubling in broadcast): τ = 2/(W + 1), W = CWmin + 1; p_coll = 1 − (1 − τ)^(N−1). Ma and Chen, 'Saturation performance of IEEE 802.11 broadcast networks', IEEE Commun. Lett. 11(8) 2007",
            tolerance: "|simulated − analytical| < 0.03 absolute with 4,000 transmissions per density",
            fault: "Frames queued on AC_VO (CWmin 3) where the analysis and configuration say AC_BE (CWmin 15)",
            cost: Cost::Fast,
            run: broadcast_collisions,
        },
        Check {
            id: "ACC-05",
            layer: Layer::Access,
            title: "Channel busy ratio: busy time over the last 100 ms, own transmissions included, overlapping intervals counted once",
            reference: "ETSI EN 302 571 V2.1.1 §4.2.10.1 / ETSI TS 102 687 V1.2.1 Table 3 (T_CBR = 100 ms)",
            tolerance: "1e-9",
            fault: "The measurement window taken as 200 ms",
            cost: Cost::Fast,
            run: cbr_meter,
        },
        Check {
            id: "ACC-06",
            layer: Layer::Access,
            title: "Reactive DCC: the CBR → state map and the packet-rate / T_off limits of both informative tables, and state changes only to a neighbouring state",
            reference: "ETSI TS 102 687 V1.2.1 (2018-04) §5.3 ('One state can only be reached by a neighbouring state') and Annex A Tables A.1 (T_on ≤ 1 ms) and A.2 (T_on ≤ 500 µs), read from the published PDF",
            tolerance: "exact",
            fault: "The state machine allowed to jump straight to the target state",
            cost: Cost::Fast,
            run: reactive_dcc,
        },
        Check {
            id: "ACC-07",
            layer: Layer::Access,
            title: "Adaptive DCC (LIMERIC): the update equations, and convergence of N identical stations to the analytical fixed point δ* = β·CBR_target/(α + N·β)",
            reference: "ETSI TS 102 687 V1.2.1 §5.4 Eqs. (1)-(6) and Table 3 (α 0.016, β 0.0012, CBR_target 0.68, δ 0.0006-0.03, G± 0.0005/−0.00025); Bansal, Kenney and Rohrs, IEEE TVT 62(9) 2013 (LIMERIC fixed point)",
            tolerance: "equations 1e-15; |δ − δ*| < 2 % of δ* after 2,000 evaluations for N = 50, 100, 200",
            fault: "α = 0.16 (a decimal slip) in the model's parameter table",
            cost: Cost::Fast,
            run: adaptive_dcc,
        },
        Check {
            id: "ACC-08",
            layer: Layer::Access,
            title: "SAE J2945/1 rate and power control: MaxITT from the smoothed density, the SUPRA power target and step, the density filter",
            reference: "SAE J2945/1 (2016) §6.3.8 via Ahmad et al., 'V2V System Congestion Control Validation and Performance', IEEE TVT 68(3) 2019, Table I and Eqs. 5-9 (B = 25, vMaxITT 600 ms, vRP 10-20 dBm over CBP 50-80 %, vSUPRAGain 0.5, λ 0.05)",
            tolerance: "1e-9 (ms, dBm)",
            fault: "vDensityCoefficient B = 20 instead of 25",
            cost: Cost::Fast,
            run: j2945,
        },
        Check {
            id: "ACC-09",
            layer: Layer::Access,
            title: "Sidelink reselection counter: the interval for every LTE and NR reservation period, and a uniform draw over it",
            reference: "3GPP TS 36.321 §5.14.1.1 (LTE: [5,15] for RRI ≥ 100 ms, [10,30] at 50 ms, [25,75] at 20 ms); TS 38.321 §5.22.1.1 (NR: [5,15] for RRI ≥ 100 ms, else [5·⌈100/max(20,RRI)⌉, 15·⌈100/max(20,RRI)⌉], ceiling as printed, read 2026-10-06); χ² over 5,500 selections",
            tolerance: "intervals exact for every RRI 1-1000 ms; χ² p > 0.001",
            fault: "NR's multiplier computed with a floor instead of the ceiling",
            cost: Cost::Fast,
            run: reselection_counter,
        },
        Check {
            id: "ACC-10",
            layer: Layer::Access,
            title: "probResourceKeep: at counter expiry the UE keeps its resource with the configured probability",
            reference: "3GPP TS 36.321 §5.14.1.1 / TS 38.321 §5.22.1.1 (keep if a uniform draw on [0,1] ≤ probResourceKeep); TS 36.331 probResourceKeep-r14 {0, 0.2, 0.4, 0.6, 0.8}",
            tolerance: "observed keep fraction within 4 binomial standard errors of 0.8 over ≥ 1,500 expiries",
            fault: "The model configured with probResourceKeep 0.4 where 0.8 is configured",
            cost: Cost::Fast,
            run: resource_keep,
        },
        Check {
            id: "ACC-11",
            layer: Layer::Access,
            title: "Sensing exclusion: the RSRP threshold steps up by 3 dB until at least 20 % of the candidate resources survive",
            reference: "3GPP TS 36.213 §14.1.1.6 steps 4-7 (repeat with Th increased by 3 dB while |S_A| < 0.2·M_total)",
            tolerance: "exact: from −110 dBm with every resource sensed at −100 dBm, four step-ups to −98 dBm and every candidate surviving",
            fault: "The initial threshold configured as −112 dBm",
            cost: Cost::Fast,
            run: sensing_exclusion,
        },
    ]
}

fn sdu(bytes: u32, at: SimTime) -> MacSdu {
    MacSdu {
        frame: FrameDescriptor::broadcast(
            bytes,
            Mcs::R6Qpsk12,
            SduRef::new(SduId::new(1), FrameSeq::new(1)),
        ),
        enqueued_at: at,
    }
}

fn edca_parameters(mode: Mode) -> Outcome {
    let want = [
        (AccessCategory::Vo, 3, 7, 2, 58_000u64),
        (AccessCategory::Vi, 7, 15, 3, 71_000),
        (AccessCategory::Be, 15, 1023, 6, 110_000),
        (AccessCategory::Bk, 15, 1023, 9, 149_000),
    ];
    let mut bad = Vec::new();
    if timing::SLOT_TIME.as_nanos() != 13_000 || timing::SIFS.as_nanos() != 32_000 {
        bad.push("slot or SIFS".to_string());
    }
    for (ac, cwmin, cwmax, aifsn, aifs) in want {
        let model_aifsn = if mode.faulted() && ac == AccessCategory::Vi { 2 } else { ac.aifsn() };
        let model_aifs = u64::from(model_aifsn) * timing::SLOT_TIME.as_nanos() + timing::SIFS.as_nanos();
        let shipped_aifs_ok = ac.aifs().as_nanos() == aifs;
        if ac.cw_min() != cwmin || ac.cw_max() != cwmax || model_aifsn != aifsn || model_aifs != aifs || !shipped_aifs_ok {
            bad.push(format!("{}: CW {}/{} AIFSN {model_aifsn} AIFS {model_aifs} ns", ac.label(), ac.cw_min(), ac.cw_max()));
        }
    }
    let up = [(0u8, AccessCategory::Be), (1, AccessCategory::Bk), (2, AccessCategory::Bk), (3, AccessCategory::Be), (4, AccessCategory::Vi), (5, AccessCategory::Vi), (6, AccessCategory::Vo), (7, AccessCategory::Vo)];
    for (p, ac) in up {
        if AccessCategory::from_user_priority(p) != ac {
            bad.push(format!("UP {p}"));
        }
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { "4 categories, 8 user priorities, slot and SIFS match".to_string() } else { bad.join("; ") })
}

/// The slots one enqueue on a busy medium draws, for `nodes` nodes.
fn draws(ac: AccessCategory, nodes: u32, seed: u64) -> Vec<u32> {
    let mut ctx = TestCtx::new(seed);
    let mut mac = EdcaOcbMac::new();
    (0..nodes)
        .map(|n| {
            let node = NodeId::new(n);
            Mac::on_cca(&mut mac, &mut ctx, node, CH, BUSY);
            Mac::enqueue(&mut mac, &mut ctx, node, sdu(200, 0), ac).expect("queued");
            mac.backoff(node, CH, ac).expect("armed").drawn
        })
        .collect()
}

fn backoff_uniform(mode: Mode) -> Outcome {
    let mut parts = Vec::new();
    for (ac, n) in [(AccessCategory::Be, 16_000u32), (AccessCategory::Vo, 8_000)] {
        let cw = ac.cw_min();
        let mut counts = vec![0u64; cw as usize + 1];
        for d in draws(ac, n, 0xBAC0) {
            let d = if mode.faulted() { d % cw } else { d };
            counts[d as usize] += 1;
        }
        let expected = vec![f64::from(n) / f64::from(cw + 1); cw as usize + 1];
        let (chi2, p) = stats::chi_square(&counts, &expected, 0);
        parts.push((p > 1e-3, format!("{}: χ²={chi2:.1} over {} bins, p={p:.3}", ac.label(), cw + 1)));
    }
    Outcome::all(parts)
}

fn backoff_freeze(mode: Mode) -> Outcome {
    let slot = timing::SLOT_TIME.as_nanos();
    let aifs = AccessCategory::Be.aifs().as_nanos();
    let mut ctx = TestCtx::new(0xF4EE_2E);
    let mut mac = EdcaOcbMac::new();
    let mut checked = 0;
    let mut wrong = Vec::new();
    let elapsed_slots = 3u64;
    for n in 0..200u32 {
        if checked >= 30 {
            break;
        }
        let node = NodeId::new(n);
        ctx.set_now(0);
        Mac::on_cca(&mut mac, &mut ctx, node, CH, BUSY);
        Mac::enqueue(&mut mac, &mut ctx, node, sdu(200, 0), AccessCategory::Be).expect("queued");
        let drawn = u64::from(mac.backoff(node, CH, AccessCategory::Be).expect("armed").drawn);
        let t1 = 1_000_000;
        ctx.set_now(t1);
        Mac::on_cca(&mut mac, &mut ctx, node, CH, CcaState::Idle);
        if drawn <= elapsed_slots {
            // Too short a countdown to interrupt; drain it and move on.
            ctx.set_now(t1 + aifs + drawn * slot);
            let _ = Mac::poll(&mut mac, &mut ctx, node, CH);
            continue;
        }
        // The medium goes busy part-way through slot 4 of the countdown.
        let busy_at = if mode.faulted() {
            t1 + aifs + drawn * slot
        } else {
            t1 + aifs + elapsed_slots * slot + 5_000
        };
        ctx.set_now(busy_at);
        Mac::on_cca(&mut mac, &mut ctx, node, CH, BUSY);
        let t3 = 3_000_000;
        ctx.set_now(t3);
        Mac::on_cca(&mut mac, &mut ctx, node, CH, CcaState::Idle);
        let expected = t3 + aifs + (drawn - elapsed_slots) * slot;
        let due = Mac::<TestCtx>::next_poll_at(&mac, node, CH);
        ctx.set_now(expected);
        let grant = Mac::poll(&mut mac, &mut ctx, node, CH);
        let at = grant.as_ref().map(|g| g.at);
        if due != Some(expected) || at != Some(expected) {
            wrong.push(format!("drawn {drawn}: due {due:?}, granted {at:?}, expected {expected}"));
        }
        checked += 1;
    }
    Outcome::judge(
        wrong.is_empty() && checked >= 30,
        if wrong.is_empty() {
            format!("{checked} interrupted countdowns resumed with drawn − 3 slots after AIFS")
        } else {
            format!("{} of {checked} wrong; first: {}", wrong.len(), wrong[0])
        },
    )
}

/// Simulates `n` saturated stations in one collision domain, driving the EDCA state
/// machine the way an event-driven medium does: a CCA report at every busy and idle
/// transition. Returns (transmissions, transmissions that overlapped another).
fn saturated(n: u32, ac: AccessCategory, target_tx: u64, seed: u64) -> (u64, u64) {
    let mut ctx = TestCtx::new(seed);
    let mut mac = EdcaOcbMac::new();
    let nodes: Vec<NodeId> = (0..n).map(NodeId::new).collect();
    let air = air_time(200, Mcs::R6Qpsk12).as_nanos();
    for &node in &nodes {
        Mac::on_cca(&mut mac, &mut ctx, node, CH, BUSY);
        Mac::enqueue(&mut mac, &mut ctx, node, sdu(200, 0), ac).expect("queued");
        Mac::enqueue(&mut mac, &mut ctx, node, sdu(200, 0), ac).expect("queued");
    }
    ctx.set_now(1_000);
    for &node in &nodes {
        Mac::on_cca(&mut mac, &mut ctx, node, CH, CcaState::Idle);
    }
    let (mut tx, mut collided) = (0u64, 0u64);
    let mut guard = 0;
    while tx < target_tx && guard < 10 * target_tx {
        guard += 1;
        let Some(next) = nodes.iter().filter_map(|&nd| Mac::<TestCtx>::next_poll_at(&mac, nd, CH)).min() else {
            break;
        };
        ctx.set_now(next);
        let mut winners = Vec::new();
        for &node in &nodes {
            if let Some(g) = Mac::poll(&mut mac, &mut ctx, node, CH) {
                debug_assert_eq!(g.at, next);
                winners.push(node);
            }
        }
        if winners.is_empty() {
            continue;
        }
        tx += winners.len() as u64;
        if winners.len() > 1 {
            collided += winners.len() as u64;
        }
        for &node in &winners {
            Mac::enqueue(&mut mac, &mut ctx, node, sdu(200, next), ac).expect("queued");
        }
        for &node in &nodes {
            Mac::on_cca(&mut mac, &mut ctx, node, CH, BUSY);
        }
        ctx.set_now(next + air);
        for &node in &nodes {
            Mac::on_cca(&mut mac, &mut ctx, node, CH, CcaState::Idle);
        }
    }
    (tx, collided)
}

fn broadcast_collisions(mode: Mode) -> Outcome {
    let configured = AccessCategory::Be;
    let queued_on = if mode.faulted() { AccessCategory::Vo } else { configured };
    let w = f64::from(configured.cw_min() + 1);
    let tau = 2.0 / (w + 1.0);
    let mut parts = Vec::new();
    for n in [2u32, 5, 10, 20] {
        let (tx, coll) = saturated(n, queued_on, 4_000, 0xB1A2 + u64::from(n));
        let sim = coll as f64 / tx.max(1) as f64;
        let analytic = 1.0 - (1.0 - tau).powi(n as i32 - 1);
        parts.push(((sim - analytic).abs() < 0.03, format!("N={n}: {sim:.3} simulated vs {analytic:.3} analytical")));
    }
    Outcome::all(parts)
}

fn cbr_meter(mode: Mode) -> Outcome {
    let window = if mode.faulted() { 200_000_000 } else { 100_000_000 };
    let mut m = CbrMeter::with_window(v2xw_core::time::Duration::from_nanos(window));
    // 10 ms busy long ago (outside the window), then 20 ms and an overlapping 15 ms
    // (10 ms new), then 5 ms: 35 ms busy in the last 100 ms at t = 300 ms.
    m.note_busy(50_000_000, 60_000_000);
    m.note_busy(210_000_000, 230_000_000);
    m.note_busy(220_000_000, 240_000_000);
    m.note_busy(280_000_000, 285_000_000);
    let cbr = m.cbr(300_000_000);
    Outcome::judge((cbr - 0.35).abs() < 1e-9, format!("CBR {cbr:.6} against 0.350000"))
}

fn reactive_dcc(mode: Mode) -> Outcome {
    let mut bad = Vec::new();
    // Annex A as printed.
    let a1 = [(ReactiveState::Relaxed, 10.0, 100), (ReactiveState::Active1, 5.0, 200), (ReactiveState::Active2, 2.5, 400), (ReactiveState::Active3, 2.0, 500), (ReactiveState::Restrictive, 1.0, 1_000)];
    let a2 = [(ReactiveState::Relaxed, 20.0, 50), (ReactiveState::Active1, 10.0, 100), (ReactiveState::Active2, 5.0, 200), (ReactiveState::Active3, 4.0, 250), (ReactiveState::Restrictive, 1.0, 1_000)];
    for (table, rows) in [(ReactiveTable::A1TonUpTo1Ms, a1), (ReactiveTable::A2TonUpTo500Us, a2)] {
        for (state, rate, toff_ms) in rows {
            let (r, t) = table.limits(state);
            if r != rate || t.as_nanos() != toff_ms * 1_000_000 {
                bad.push(format!("{table:?} {}: {r} Hz {} ms", state.label(), t.as_nanos() / 1_000_000));
            }
        }
    }
    let bands = [(0.29, ReactiveState::Relaxed), (0.30, ReactiveState::Active1), (0.39, ReactiveState::Active1), (0.40, ReactiveState::Active2), (0.49, ReactiveState::Active2), (0.50, ReactiveState::Active3), (0.60, ReactiveState::Active3), (0.61, ReactiveState::Restrictive)];
    for (cbr, want) in bands {
        if ReactiveTable::A1TonUpTo1Ms.state_for(cbr) != want {
            bad.push(format!("A.1 CBR {cbr}"));
        }
    }
    for (cbr, want) in [(0.60, ReactiveState::Active3), (0.65, ReactiveState::Active3), (0.66, ReactiveState::Restrictive)] {
        if ReactiveTable::A2TonUpTo500Us.state_for(cbr) != want {
            bad.push(format!("A.2 CBR {cbr}"));
        }
    }
    // Neighbour-only transitions: a jump from 10 % to 90 % walks up one state per
    // evaluation, and back down the same way.
    let mut dcc = ReactiveDcc::new();
    let mut ctx = TestCtx::new(1);
    let node = NodeId::new(0);
    let mut path = Vec::new();
    for (i, cbr) in [0.1, 0.9, 0.9, 0.9, 0.9, 0.9, 0.1, 0.1, 0.1, 0.1, 0.1].iter().enumerate() {
        ctx.set_now(i as u64 * 100_000_000);
        if mode.faulted() {
            // The fault: the target state applied directly.
            path.push(ReactiveTable::A1TonUpTo1Ms.state_for(*cbr));
            continue;
        }
        Dcc::on_cbr(&mut dcc, &mut ctx, node, *cbr);
        path.push(dcc.state_of(node));
    }
    let jumps = path.windows(2).filter(|w| (w[0] as i32 - w[1] as i32).abs() > 1).count();
    let reached = path.contains(&ReactiveState::Restrictive) && path.last() == Some(&ReactiveState::Relaxed);
    if jumps > 0 || !reached {
        bad.push(format!("{jumps} non-neighbour transitions in {:?}", path.iter().map(|s| s.label()).collect::<Vec<_>>()));
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { "10 table rows, 11 band edges and the neighbour-only walk 10 % → 90 % → 10 % match".to_string() } else { bad.join("; ") })
}

fn adaptive_dcc(mode: Mode) -> Outcome {
    let p = AdaptiveParams::TS102687;
    let model_p = if mode.faulted() { AdaptiveParams { alpha: 0.16, ..p } } else { p };
    let mut bad = Vec::new();
    // Eqs. (2)-(6) by hand, on a grid of states.
    for &delta in &[0.0006, 0.004, 0.012, 0.03] {
        for &cbr in &[0.0, 0.3, 0.6, 0.68, 0.75, 0.95] {
            let raw = p.beta * (p.cbr_target - cbr);
            let off = if raw > 0.0 { raw.min(p.g_max_plus) } else { raw.max(p.g_max_minus) };
            let want = ((1.0 - p.alpha) * delta + off).clamp(p.delta_min, p.delta_max);
            let got = AdaptiveDcc::update_delta(model_p, delta, cbr);
            if (got - want).abs() > 1e-15 {
                bad.push(format!("δ({delta}, {cbr}) = {got} vs {want}"));
                break;
            }
        }
    }
    // Eq. (1).
    let s = AdaptiveDcc::smooth(0.4, 0.6, 0.2);
    if (s - (0.5 * 0.4 + 0.5 * (0.6 + 0.2) / 2.0)).abs() > 1e-15 {
        bad.push(format!("smoothing {s}"));
    }
    // N identical stations, each transmitting its full allowance, so CBR = N·δ.
    let mut fixed = Vec::new();
    for n in [50.0, 100.0, 200.0] {
        let mut delta = p.delta_max;
        let (mut smoothed, mut prev) = (0.0, 0.0);
        for _ in 0..2_000 {
            let now = (n * delta).min(1.0);
            smoothed = AdaptiveDcc::smooth(smoothed, now, prev);
            prev = now;
            delta = AdaptiveDcc::update_delta(model_p, delta, smoothed);
        }
        let star = p.beta * p.cbr_target / (p.alpha + n * p.beta);
        let ok = crate::rel(delta, star) < 0.02;
        fixed.push(format!("N={n}: δ {delta:.5} vs δ* {star:.5}"));
        if !ok {
            bad.push(format!("N={n} converged to {delta:.5}, δ* = {star:.5}"));
        }
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { format!("equations exact on 24 states; {}", fixed.join(", ")) } else { bad.join("; ") })
}

fn j2945(mode: Mode) -> Outcome {
    let p = J2945Params::J2945_1;
    let model_p = if mode.faulted() { J2945Params { b_density: 20.0, ..p } } else { p };
    let mut bad = Vec::new();
    // MaxITT: 100 ms to B, 100·N/B ms above, capped at 600 ms (reached at 150 vehicles).
    for n in [0.0f64, 10.0, 25.0, 30.0, 50.0, 100.0, 149.0, 150.0, 400.0] {
        let want = (100.0 * n / 25.0).clamp(100.0, 600.0);
        let got = SaeJ2945Dcc::max_itt_for(model_p, n).as_secs_f64() * 1_000.0;
        if (got - want).abs() > 1e-6 {
            bad.push(format!("MaxITT({n}) {got:.3} vs {want} ms"));
        }
    }
    // f(CBP): 20 dBm to 50 %, linear to 10 dBm at 80 %.
    for cbp in [0.0, 0.5, 0.6, 0.65, 0.8, 0.95] {
        let want = if cbp <= 0.5 { 20.0 } else if cbp >= 0.8 { 10.0 } else { 20.0 - (cbp - 0.5) / 0.3 * 10.0 };
        let got = SaeJ2945Dcc::power_target_dbm(model_p, cbp);
        if (got - want).abs() > 1e-9 {
            bad.push(format!("f({cbp}) {got} vs {want}"));
        }
        let step = SaeJ2945Dcc::supra_step(model_p, 15.0, cbp);
        if (step - (15.0 + 0.5 * (want - 15.0))).abs() > 1e-9 {
            bad.push(format!("SUPRA step at {cbp}: {step}"));
        }
    }
    let smoothed = SaeJ2945Dcc::smooth_density(model_p, 20.0, 40.0);
    if (smoothed - (0.05 * 40.0 + 0.95 * 20.0)).abs() > 1e-12 {
        bad.push(format!("density filter {smoothed}"));
    }
    Outcome::judge(bad.is_empty(), if bad.is_empty() { "9 MaxITT points, 6 power targets and SUPRA steps, and the density filter match".to_string() } else { bad.join("; ") })
}

/// The reselection-counter interval the standards print.
fn spec_counter_range(rri_ms: u32, rat: SlRat) -> (u32, u32) {
    if rri_ms == 0 {
        return (1, 1);
    }
    match rat {
        SlRat::LteMode4 => match rri_ms {
            20 => (25, 75),
            50 => (10, 30),
            _ => (5, 15),
        },
        SlRat::NrMode2 => {
            if rri_ms >= 100 {
                (5, 15)
            } else {
                let c = 100u32.div_ceil(rri_ms.max(20));
                (5 * c, 15 * c)
            }
        }
    }
}

fn reselection_counter(mode: Mode) -> Outcome {
    let mut bad = Vec::new();
    let mut lte_n = 0;
    for &rri in &Rri::LTE_VALUES {
        lte_n += 1;
        let got = Rri(rri).c_resel_range(SlRat::LteMode4);
        if got != spec_counter_range(rri, SlRat::LteMode4) {
            bad.push(format!("LTE {rri} ms: {got:?}"));
        }
    }
    let mut nr_n = 0;
    for rri in (1..=99).chain((100..=1000).step_by(100)) {
        nr_n += 1;
        let got = if mode.faulted() && rri < 100 {
            let c = 100 / rri.max(20);
            (5 * c, 15 * c)
        } else {
            Rri(rri).c_resel_range(SlRat::NrMode2)
        };
        let want = spec_counter_range(rri, SlRat::NrMode2);
        if got != want {
            bad.push(format!("NR {rri} ms: {got:?} vs {want:?}"));
        }
    }
    // The draw: uniform over [5, 15] at RRI 100 ms.
    let mut counts = vec![0u64; 11];
    let mut ctx = TestCtx::new(0xC0FFEE);
    let mut e = SpsEngine::new(Tier::High, PoolConfig::molina_masegosa_highway(), SpsParams::molina_masegosa(10));
    let draws = 5_500u32;
    for n in 0..draws {
        let out = e.select(&mut ctx, NodeId::new(n), 500, 1, SelectionReason::NoReservation).expect("selects");
        if (5..=15).contains(&out.c_resel) {
            counts[(out.c_resel - 5) as usize] += 1;
        } else {
            bad.push(format!("C_resel {} outside [5,15]", out.c_resel));
            break;
        }
    }
    let (chi2, p) = stats::chi_square(&counts, &[f64::from(draws) / 11.0; 11], 0);
    if p <= 1e-3 {
        bad.push(format!("draw not uniform: χ²={chi2:.1}, p={p:.4}"));
    }
    Outcome::judge(
        bad.is_empty(),
        if bad.is_empty() {
            format!("{lte_n} LTE and {nr_n} NR periods match; draw χ²={chi2:.1}, p={p:.3}")
        } else {
            format!("{} wrong: {}", bad.len(), bad.iter().take(4).cloned().collect::<Vec<_>>().join("; "))
        },
    )
}

fn resource_keep(mode: Mode) -> Outcome {
    let mut params = SpsParams::molina_masegosa(10);
    params.prob_keep = if mode.faulted() { ProbResourceKeep::P040 } else { ProbResourceKeep::P080 };
    let mut e = SpsEngine::new(Tier::High, PoolConfig::molina_masegosa_highway(), params);
    let mut ctx = TestCtx::new(0x4B45_4550);
    let (mut kept, mut expiries) = (0u64, 0u64);
    for n in 0..60u32 {
        let node = NodeId::new(n);
        let mut slot = 1_000u64;
        e.select(&mut ctx, node, slot, 1, SelectionReason::NoReservation).expect("selects");
        for _ in 0..500 {
            let before = e.reservation(node).map(|r| r.c_resel);
            e.on_transmitted(&mut ctx, node);
            slot += 100;
            if before == Some(1) {
                expiries += 1;
                if e.reservation(node).is_some() {
                    kept += 1;
                } else {
                    e.select(&mut ctx, node, slot, 1, SelectionReason::CounterExpired).expect("selects");
                }
            }
        }
    }
    let frac = kept as f64 / expiries as f64;
    let se = (0.8 * 0.2 / expiries as f64).sqrt();
    Outcome::judge(
        expiries >= 1_500 && (frac - 0.8).abs() < 4.0 * se,
        format!("kept {kept} of {expiries} expiries = {frac:.3} (0.8 ± {:.3})", 4.0 * se),
    )
}

fn sensing_exclusion(mode: Mode) -> Outcome {
    let mut params = SpsParams::molina_masegosa(10);
    params.rsrp_threshold_dbm = if mode.faulted() { -112.0 } else { -110.0 };
    params.srssi_ranking = false;
    let mut e = SpsEngine::new(Tier::High, PoolConfig::molina_masegosa_highway(), params);
    let mut ctx = TestCtx::new(9);
    let node = NodeId::new(0);
    for slot in 900..=1_100u64 {
        for sc in 0..64u32 {
            e.note_sensed(node, SlResource::new(slot, sc, 1), -100.0, 100);
        }
    }
    let out = e.select(&mut ctx, node, 1_100, 1, SelectionReason::NoReservation).expect("selects");
    let ok = out.step_ups == 4
        && (out.final_threshold_dbm - -98.0).abs() < 1e-9
        && out.candidates_surviving * 5 >= out.candidates_total;
    Outcome::judge(
        ok,
        format!(
            "{} step-ups to {} dBm; {} of {} candidates survive",
            out.step_ups, out.final_threshold_dbm, out.candidates_surviving, out.candidates_total
        ),
    )
}
