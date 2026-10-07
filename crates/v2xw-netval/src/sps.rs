//! Layer 2, sidelink: the semi-persistent scheduler's selection window and its S-RSSI
//! ranking against the 3GPP procedures (ACC-09 to ACC-11 in `access` hold the
//! reselection counter, the keep probability and the RSRP step-up).

use v2xw_core::card::Tier;
use v2xw_core::ids::NodeId;
use v2xw_radio::sidelink::{Numerology, PoolConfig};
use v2xw_radio::sps::{SelectionReason, SpsEngine, SpsParams};

use crate::ctx::TestCtx;
use crate::{Check, Cost, Layer, Mode, Outcome};

/// The checks of this module.
#[must_use]
pub fn checks() -> Vec<Check> {
    vec![
        Check {
            id: "ACC-12",
            layer: Layer::Access,
            title: "Sidelink selection window and candidate set: T1, T2 and the sensing window per profile, and every candidate single-subframe resource inside [n + T1, n + T2]",
            reference: "LTE: 3GPP TS 36.213 §14.1.1.6 steps 1-2 (sensing over the 1,000 preceding subframes; T1 ≤ 4; T2min ≤ T2 ≤ 100, and T2 within the packet's latency budget, 1000/pps ms — Molina-Masegosa & Gozalvez, IEEE VT Mag. 12(4) 2017); M_total = (T2 − T1 + 1)·(N_subCH − L + 1). NR: TS 38.214 §8.1.4 (T1 ≤ T_proc,1 = 5 slots at µ = 1; T2 ≤ the 100 ms packet delay budget; sensing window 100·2^µ slots)",
            tolerance: "exact",
            fault: "The selection window closed one slot after the latency deadline (T2 = 1000/pps + 1)",
            cost: Cost::Fast,
            run: selection_window,
        },
        Check {
            id: "ACC-13",
            layer: Layer::Access,
            title: "S-RSSI ranking: the pick is uniform over the 20 % of all candidates with the lowest average S-RSSI",
            reference: "3GPP TS 36.213 §14.1.1.6 step 8 (move the resource with the smallest E_x,y from S_A to S_B until |S_B| ≥ 0.2·M_total; E_x,y the S-RSSI averaged over subframes y − P_step·j, TS 36.214 §5.1.28) and step 9 (report S_B; the MAC picks uniformly, TS 36.321 §5.14.1.1)",
            tolerance: "every one of 120 picks inside S_B; the picks spread over at least 40 distinct slots (S_B holds 80 resources over 100 slots)",
            fault: "S-RSSI ranking switched off (the uniform pick made over S_A)",
            cost: Cost::Fast,
            run: srssi_ranking,
        },
    ]
}

fn selection_window(mode: Mode) -> Outcome {
    let mut parts = Vec::new();
    let pool = PoolConfig::molina_masegosa_highway();
    let s = u64::from(pool.subchannels());
    for pps in [10u32, 20, 50] {
        let mut params = SpsParams::molina_masegosa(pps);
        if mode.faulted() {
            params.t2_slots += 1;
        }
        let (t1, t2, window) = (params.t1_slots, params.t2_slots, params.sensing_window_slots);
        let latency = u64::from(1000 / pps);
        let engine = SpsEngine::new(Tier::High, pool.clone(), params);
        let n = 5_000u64;
        let c = engine.candidates(n, 1);
        let inside = c.iter().all(|r| r.slot >= n + t1 && r.slot <= n + t2 && r.subch < s as u32);
        let ok = t1 <= 4 && (20..=100).contains(&t2) && t2 <= latency && window == 1_000 && inside && c.len() as u64 == (t2 - t1 + 1) * s;
        parts.push((ok, format!("LTE {pps} pps: T1 {t1}, T2 {t2} (budget {latency}), sensing {window}, M_total {} = ({t2} − {t1} + 1)·{s}", c.len())));
    }
    // NR Mode 2 at µ = 1 (30 kHz): T_proc,1 = 5 slots, PDB 100 ms = 200 slots.
    let params = SpsParams::ali_todisco(Numerology::Mu1, true);
    let (t1, t2, window) = (params.t1_slots, params.t2_slots, params.sensing_window_slots);
    let engine = SpsEngine::new(Tier::High, pool.clone(), params);
    let c = engine.candidates(5_000, 1);
    let inside = c.iter().all(|r| r.slot >= 5_000 + t1 && r.slot <= 5_000 + t2);
    let ok = t1 <= 5 && t2 <= 200 && window == 200 && inside && c.len() as u64 == (t2 - t1 + 1) * s;
    parts.push((ok, format!("NR µ=1: T1 {t1}, T2 {t2}, sensing {window} slots, M_total {}", c.len())));
    Outcome::all(parts)
}

fn srssi_ranking(mode: Mode) -> Outcome {
    let pool = PoolConfig::molina_masegosa_highway();
    let s = pool.subchannels();
    let mut params = SpsParams::molina_masegosa(10);
    params.srssi_ranking = !mode.faulted();
    let mut engine = SpsEngine::new(Tier::High, pool, params);
    let mut ctx = TestCtx::new(17);
    let trigger = 2_000u64;
    let picks = 120u32;
    let mut outside = 0u32;
    let mut slots = std::collections::BTreeSet::new();
    // S-RSSI rises 5 dB per sub-channel, the same in every slot: sub-channel 0 is the
    // quietest and holds 100 of the 400 candidates, so S_B (80 resources) lies on it.
    for k in 0..picks {
        let node = NodeId::new(k);
        for slot in trigger - 1_000..trigger {
            for sc in 0..s {
                engine.note_energy(node, slot, sc, -100.0 + 5.0 * f64::from(sc));
            }
        }
        let Some(out) = engine.select(&mut ctx, node, trigger, 1, SelectionReason::NoReservation) else {
            return Outcome::judge(false, format!("node {k}: no selection"));
        };
        if out.resource.subch != 0 {
            outside += 1;
        }
        slots.insert(out.resource.slot);
    }
    Outcome::judge(
        outside == 0 && slots.len() >= 40,
        format!("{} of {picks} picks outside S_B; {} distinct slots", outside, slots.len()),
    )
}
