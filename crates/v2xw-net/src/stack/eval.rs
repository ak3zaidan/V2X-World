//! The comparison with a published GeoNetworking evaluation.
//!
//! **Reference.** O. Amador, M. Urueña, M. Calderon, I. Soto, *Evaluation and improvement
//! of ETSI ITS Contention-Based Forwarding (CBF) of warning messages in highway
//! scenarios*, Vehicular Communications 34 (2022) 100454; arXiv:2403.05994. Artery +
//! Vanetza (the reference implementations this module follows).
//!
//! **Their setup (Table 1).** 5 km straight highway, 4 lanes per direction; a stationary
//! source near one end; the destination area a rectangle 4 km behind the source plus
//! 100 m ahead, all lanes; 10-50 vehicles/km/lane; 20 mW, maximum range 778 m;
//! `TO_CBF_MIN` 1 ms, `TO_CBF_MAX` 100 ms, `DIST_MAX` 1 000 m; one 301-octet DENM per
//! second for 30 s; LIMERIC-based DCC.
//!
//! **Their results, as published.** Table 2, GN transmissions (ETSI / DPD / GPC):
//! 10 veh/km 8 245.0 / 485.0 / 596.4; 20 veh/km 26 515.0 / 1 095.6 / 1 480.8;
//! 30 veh/km 19 878.4 / 1 229.8 / 1 900.6. Table 3, PDR in the area: 10 veh/km
//! 0.9998 / 0.9500 / 0.9917; 20 veh/km 0.9961 / 0.9639 / 1.0036; 30 veh/km
//! 0.9280 / 0.8504 / 0.9864. Their headline: duplicate detection plus no out-of-area
//! forwarding cuts transmissions "between 9.6 and 24.2 times". Table 2's caption is
//! "Average number of transmissions per density"; the text does not say whether that is
//! per DENM or per 30-DENM run. Its magnitude (8 245 for 400 vehicles) reads as per run,
//! so [`EvalRow::tx_per_denm`] is compared with the published value ÷ 30 — our reading,
//! flagged as such.
//!
//! **Measured here (10 veh/km/lane, 5 DENMs, seed 7; `compare` prints it).** ETSI /
//! DPD / GPC transmissions per DENM = 183.8 / 6.6 / 18.4 against the published ÷ 30 of
//! 275 / 16.2 / 19.9; PDR 0.787 / 0.667 / 1.000 against 0.9998 / 0.9500 / 0.9917. The
//! ETSI-to-DPD ratio is 27.8x (published 17.0x) and GPC agrees within 8 %; DPD's loss is worse here, and
//! ETSI's PDR lower, because with no MAC two near-tied contenders both transmit and the
//! second copy cancels every downstream contender — the failure GPC exists to fix,
//! sharpened by the missing DCC queueing that de-synchronises real forwarders.
//!
//! **With CSMA/CA** ([`compare_with`] with `Some(Csma::AC_VO)`, same seed): ETSI / DPD /
//! GPC transmissions per DENM 200.8 / 6.6 / 17.2 and PDR 1.000 / 0.667 / 1.000. Carrier
//! sense alone takes ETSI's PDR from 0.787 to the published 0.9998, and its transmissions
//! from 183.8 towards the published ÷ 30 of 275; DPD's loss is unchanged (standing down
//! on any duplicate is a protocol property; the published 0.95 includes DCC queueing,
//! which this run does not model). The engine's on-air forwarding has the full MAC and
//! DCC.
//!
//! **What this module reproduces, and what it cannot.** Our run has no MAC contention
//! and no DCC (see [`super::dissem`]), so absolute transmission counts and the
//! density-driven PDR loss of their DCC-limited channel are not comparable. The claims a
//! protocol-level model *must* reproduce, and [`compare`] checks, are the orderings:
//!
//! 1. ETSI-baseline CBF sends an order of magnitude more than DPD (their 9.6-24.2×).
//! 2. DPD with cancellation-on-any-duplicate loses reach against ETSI (their −5 to −19 %).
//! 3. GPC recovers the reach of ETSI at a transmission count near DPD's.

use serde::{Deserialize, Serialize};

use super::area::Area;
use super::dissem::{
    Csma, DenParams, DissemConfig, DissemReport, DisseminationRun, Hazard, MotionNode, UnitDisk,
    highway,
};
use super::router::{CbfVariant, ForwardingConfig};

/// Amador et al. Table 1: maximum transmission range, m.
pub const AMADOR_RANGE_M: f64 = 778.0;
/// Highway length, m.
pub const AMADOR_LENGTH_M: f64 = 5_000.0;
/// Lanes per direction.
pub const AMADOR_LANES_PER_DIR: u32 = 4;
/// DENM size, octets.
pub const AMADOR_DENM_BYTES: u32 = 301;

/// Their published Table 2 and 3 rows: `(veh/km/lane, [tx ETSI, DPD, GPC], [PDR ETSI, DPD, GPC])`.
pub const AMADOR_PUBLISHED: [(u32, [f64; 3], [f64; 3]); 5] = [
    (10, [8_245.0, 485.0, 596.4], [0.9998, 0.9500, 0.9917]),
    (20, [26_515.0, 1_095.6, 1_480.8], [0.9961, 0.9639, 1.0036]),
    (30, [19_878.4, 1_229.8, 1_900.6], [0.9280, 0.8504, 0.9864]),
    (40, [19_063.6, 1_029.2, 2_167.6], [0.9371, 0.8107, 0.9852]),
    (50, [16_542.2, 1_720.6, 2_483.6], [0.9372, 0.8824, 0.9812]),
];

/// One simulated row of the comparison.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvalRow {
    /// Vehicles per km per lane.
    pub density: u32,
    /// Variant.
    pub variant: CbfVariant,
    /// Mean GN transmissions per DENM packet.
    pub tx_per_denm: f64,
    /// Mean per-packet PDR in the area.
    pub pdr: f64,
    /// Median dissemination latency over the area, ms (mean over DENMs).
    pub latency_p50_ms: f64,
    /// 95th percentile, ms (mean over DENMs).
    pub latency_p95_ms: f64,
    /// Most hops.
    pub max_hops: u8,
}

/// Run Amador et al.'s highway for one density and variant, with `denms` DENMs one
/// second apart after a 4 s beacon warm-up.
pub fn amador_highway(
    density: u32,
    variant: CbfVariant,
    denms: u32,
    seed: u64,
) -> (EvalRow, DissemReport) {
    amador_highway_with(density, variant, denms, seed, None)
}

/// [`amador_highway`] with a medium-access model: `Some(Csma::AC_VO)` makes forwarders
/// carrier-sense, back off and collide as 802.11p broadcast does.
pub fn amador_highway_with(
    density: u32,
    variant: CbfVariant,
    denms: u32,
    seed: u64,
    mac: Option<Csma>,
) -> (EvalRow, DissemReport) {
    let mut nodes = highway(
        AMADOR_LENGTH_M,
        AMADOR_LANES_PER_DIR,
        f64::from(density),
        seed,
    );
    // The stationary source on the shoulder, 500 m from the far end.
    let src_x = AMADOR_LENGTH_M - 500.0;
    nodes.push(MotionNode::fixed(src_x, -2.0));
    let src = nodes.len() - 1;
    let width = 3.5 * 2.0 * f64::from(AMADOR_LANES_PER_DIR);
    let area = Area::rect(src_x - 4_000.0, -5.0, src_x + 100.0, width + 1.0);
    let hazards = (0..denms)
        .map(|k| Hazard {
            at_ms: 4_000 + 1_000 * u64::from(k),
            node: src,
            area,
        })
        .collect();
    let improved = variant != CbfVariant::Etsi;
    let cfg = DissemConfig {
        forwarding: ForwardingConfig {
            cbf_variant: variant,
            out_of_area_broadcast_forward: !improved,
            ..Default::default()
        },
        den: DenParams {
            bytes: AMADOR_DENM_BYTES,
            ..Default::default()
        },
        beacons: true,
        end_ms: 4_000 + 1_000 * u64::from(denms) + 2_000,
        processing_us: 0,
        seed,
        mac,
    };
    let m = UnitDisk {
        range_m: AMADOR_RANGE_M,
    };
    let rep = DisseminationRun::new(cfg, &m, nodes, hazards).run();
    let n = rep.actions.len().max(1) as f64;
    let avg = |f: &dyn Fn(&super::dissem::ActionStats) -> f64| {
        v2xw_core::math::q3(v2xw_core::math::sum_ordered(rep.actions.iter().map(f)) / n)
    };
    let row = EvalRow {
        density,
        variant,
        tx_per_denm: rep.tx_per_packet_mean,
        pdr: rep.packet_pdr_mean,
        latency_p50_ms: avg(&|a| a.latency_p50_ms),
        latency_p95_ms: avg(&|a| a.latency_p95_ms),
        max_hops: rep.actions.iter().map(|a| a.max_hops).max().unwrap_or(0),
    };
    (row, rep)
}

/// The three orderings of the module docs, checked on simulated rows for one density.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// Density.
    pub density: u32,
    /// Simulated rows: ETSI, DPD, GPC.
    pub rows: [EvalRow; 3],
    /// Simulated ETSI/DPD transmission ratio.
    pub tx_ratio_etsi_over_dpd: f64,
    /// Published ETSI/DPD transmission ratio at the same density, if tabulated.
    pub published_tx_ratio: Option<f64>,
    /// Claim 1 holds: ETSI sends at least 5× DPD.
    pub overhead_order_of_magnitude: bool,
    /// Claim 2 holds: DPD's PDR is below ETSI's.
    pub dpd_loses_reach: bool,
    /// Claim 3 holds: GPC's PDR is within 0.02 of ETSI's (or above) and its
    /// transmissions below a third of ETSI's.
    pub gpc_recovers: bool,
}

/// Compare at one density.
pub fn compare(density: u32, denms: u32, seed: u64) -> Comparison {
    compare_with(density, denms, seed, None)
}

/// [`compare`] with a medium-access model.
pub fn compare_with(density: u32, denms: u32, seed: u64, mac: Option<Csma>) -> Comparison {
    let e = amador_highway_with(density, CbfVariant::Etsi, denms, seed, mac).0;
    let d = amador_highway_with(density, CbfVariant::Dpd, denms, seed, mac).0;
    let g = amador_highway_with(density, CbfVariant::Gpc, denms, seed, mac).0;
    let ratio = if d.tx_per_denm > 0.0 {
        v2xw_core::math::q3(e.tx_per_denm / d.tx_per_denm)
    } else {
        0.0
    };
    let published = AMADOR_PUBLISHED
        .iter()
        .find(|r| r.0 == density)
        .map(|r| v2xw_core::math::q3(r.1[0] / r.1[1]));
    Comparison {
        density,
        overhead_order_of_magnitude: ratio >= 5.0,
        dpd_loses_reach: d.pdr < e.pdr,
        gpc_recovers: g.pdr >= e.pdr - 0.02 && g.tx_per_denm < e.tx_per_denm / 3.0,
        tx_ratio_etsi_over_dpd: ratio,
        published_tx_ratio: published,
        rows: [e, d, g],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reproduces_amador_orderings_at_10_veh_per_km_lane() {
        let c = compare(10, 5, 7);
        eprintln!("{}", serde_json::to_string_pretty(&c).unwrap());
        assert!(
            c.overhead_order_of_magnitude,
            "ETSI/DPD = {}",
            c.tx_ratio_etsi_over_dpd
        );
        assert!(c.dpd_loses_reach);
        assert!(c.gpc_recovers);
        assert!(c.rows[2].pdr >= 0.98, "GPC pdr {}", c.rows[2].pdr);
        // 4 km at 778 m range needs at least 6 hops.
        assert!(c.rows[2].max_hops >= 5);
    }

    /// With CSMA/CA the forwarders carrier-sense one another, so two near-tied CBF
    /// contenders no longer put the same copy on the air in the same instant: ETSI's
    /// reach recovers to the published PDR (0.9998) and the orderings still hold.
    #[test]
    fn with_csma_etsi_reaches_the_published_pdr_and_the_orderings_hold() {
        let ideal = compare(10, 5, 7);
        let mac = compare_with(10, 5, 7, Some(Csma::AC_VO));
        eprintln!(
            "no MAC: tx {:?} pdr {:?}\nCSMA:   tx {:?} pdr {:?}",
            ideal.rows.iter().map(|r| r.tx_per_denm).collect::<Vec<_>>(),
            ideal.rows.iter().map(|r| r.pdr).collect::<Vec<_>>(),
            mac.rows.iter().map(|r| r.tx_per_denm).collect::<Vec<_>>(),
            mac.rows.iter().map(|r| r.pdr).collect::<Vec<_>>(),
        );
        let (_, rep) = amador_highway_with(10, CbfVariant::Etsi, 5, 7, Some(Csma::AC_VO));
        eprintln!(
            "ETSI with CSMA: collisions {} half-duplex {} access {} us",
            rep.collisions, rep.half_duplex, rep.access_delay_mean_us
        );
        assert!(rep.access_delay_mean_us > 0.0, "the MAC is in the loop");
        assert!(
            mac.rows[0].pdr > ideal.rows[0].pdr && mac.rows[0].pdr >= 0.95,
            "ETSI with a MAC: {mac:?}"
        );
        assert!(mac.overhead_order_of_magnitude, "{mac:?}");
        assert!(mac.gpc_recovers, "{mac:?}");
        assert!(mac.rows[2].pdr >= 0.95, "{mac:?}");
    }
}
