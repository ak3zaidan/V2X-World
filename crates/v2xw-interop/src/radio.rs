//! Delivery against distance: this engine's 802.11p and C-V2X sidelink, run through the
//! radio crate's highway sweep harness (`v2xw_radio::sweep`) in the configurations of three
//! published simulator studies, against those studies' curves.
//!
//! # The references
//!
//! * **Veins** (802.11p): Sepulcre, Gozalvez et al. (UMH), "Analytical Models of the
//!   Performance of IEEE 802.11p Vehicle to Vehicle Communications", IEEE Trans. Veh.
//!   Technol., accepted October 2021 (arXiv:2104.07923), Fig. 4a, the **Veins simulation**
//!   curve for DR = 6 Mbit/s: 60 veh/km, 10 Hz, 190 B, 23 dBm, 10 MHz, WINNER+ B1, Krauss
//!   mobility in SUMO, 5 km highway with 2 lanes per direction (the paper's Table III and
//!   §V.A). Their analytical model matches that curve with a mean absolute deviation below
//!   1 % (§V.B).
//! * **OpenCV2X / Artery** (LTE-V2X Mode 4): McCarthy, Burbano-Abril, Rangel Licea,
//!   O'Driscoll, "OpenCV2X: Modelling of the V2X Cellular Sidelink and Performance
//!   Evaluation for Aperiodic Traffic", arXiv:2103.13212 (2021), Fig. 3, the **OpenCV2X**
//!   (solid) curves at β = 0.06 and 0.12 veh/m: 20 dBm, 10 Hz, MCS 7 (QPSK 0.5, 2
//!   sub-channels of 12 RB), 190 B, WINNER+ B1, RSRP threshold −126 dBm, 6 lanes over 2 km
//!   (its Tables I and II). OpenCV2X is built on Artery (the paper's Table II lists the
//!   Artery middleware), so this is also the Artery comparison; Artery's own ITS-G5 curves
//!   are not used.
//! * **Molina-Masegosa & Gozalvez 2017** (LTE-V2X Mode 4), the curve already held in
//!   `v2xw_radio::sweep`'s tests (Fig. 3 at 10 pps, highway slow).
//!
//! Every reference value was **digitised from the published figure**, not stated in the
//! text: the Veins curve by colour-tracing the rendered figure at 400 dpi (±0.01), the
//! OpenCV2X curves by reading the plotted markers (±0.02). The tables say so.
//!
//! # What cannot match
//!
//! All three studies used WINNER+ B1. It now ships (`propagation/winner-plus-b1`,
//! `ChannelModel::WinnerPlusB1`, with the effective antenna heights `h' = h − 1 m`); the
//! earlier shaped stand-in (`ChannelModel::WinnerB1ShapedTodoCalibrate`, breakpoint from
//! the real heights) and the TR 37.885 highway-LOS channel are still run beside it so the
//! size of the channel's contribution stays visible. The harness's ring has 3 lanes per direction
//! where Veins used 2.

use v2xw_core::time::Duration;
use v2xw_radio::sidelink::{LTE_QPSK_R050, PoolConfig};
use v2xw_radio::sps::SpsParams;
use v2xw_radio::sweep::{
    ChannelModel, DsrcConfig, HighwaySweep, SweepReport, sweep_dsrc, sweep_sidelink,
};

/// One published curve.
#[derive(Debug, Clone)]
pub struct Reference {
    /// Short name.
    pub name: &'static str,
    /// Where it comes from, and how it was read.
    pub source: &'static str,
    /// `(distance m, PDR)`.
    pub points: &'static [(f64, f64)],
}

/// Veins, Sepulcre et al. 2022 Fig. 4a, simulation, DR = 6 Mbit/s (colour-traced, ±0.01).
pub const VEINS_6MBPS_60VPKM: Reference = Reference {
    name: "Veins 802.11p, 60 veh/km, 10 Hz, 190 B, 6 Mbit/s",
    source: "Sepulcre, Gozalvez et al., IEEE TVT (accepted Oct. 2021), arXiv:2104.07923, Fig. 4a (Veins simulation curve, digitised ±0.01)",
    points: &[
        (0.0, 0.986),
        (50.0, 0.979),
        (100.0, 0.968),
        (150.0, 0.930),
        (200.0, 0.859),
        (250.0, 0.630),
        (300.0, 0.297),
        (350.0, 0.091),
        (400.0, 0.019),
    ],
};

/// OpenCV2X (Artery-based), McCarthy et al. 2021 Fig. 3, β = 0.06 veh/m (markers, ±0.02).
pub const OPENCV2X_006: Reference = Reference {
    name: "OpenCV2X LTE-V2X Mode 4, 60 veh/km, 10 Hz, 190 B, MCS 7",
    source: "McCarthy et al., arXiv:2103.13212, Fig. 3, solid curve β=0.06 veh/m (digitised ±0.02)",
    points: &[
        (0.0, 0.99),
        (50.0, 0.98),
        (100.0, 0.965),
        (150.0, 0.95),
        (200.0, 0.945),
        (250.0, 0.92),
        (300.0, 0.89),
        (350.0, 0.82),
        (400.0, 0.67),
        (450.0, 0.46),
        (500.0, 0.24),
    ],
};

/// OpenCV2X, β = 0.12 veh/m.
pub const OPENCV2X_012: Reference = Reference {
    name: "OpenCV2X LTE-V2X Mode 4, 120 veh/km, 10 Hz, 190 B, MCS 7",
    source: "McCarthy et al., arXiv:2103.13212, Fig. 3, solid curve β=0.12 veh/m (digitised ±0.02)",
    points: &[
        (0.0, 0.99),
        (50.0, 0.96),
        (100.0, 0.93),
        (150.0, 0.92),
        (200.0, 0.895),
        (250.0, 0.835),
        (300.0, 0.77),
        (350.0, 0.68),
        (400.0, 0.53),
        (450.0, 0.35),
        (500.0, 0.18),
    ],
};

/// Molina-Masegosa & Gozalvez 2017, Fig. 3, 10 pps highway slow (as held in
/// `v2xw_radio::sweep` tests).
pub const MOLINA_10PPS: Reference = Reference {
    name: "Molina-Masegosa 2017 LTE-V2X Mode 4, 120 veh/km, 10 pps",
    source: "Molina-Masegosa & Gozalvez, IEEE VT Mag. 12(4) 2017, Fig. 3 (values as transcribed in v2xw-radio sweep.rs)",
    points: &[
        (0.0, 0.97),
        (100.0, 0.95),
        (200.0, 0.90),
        (300.0, 0.79),
        (400.0, 0.66),
        (500.0, 0.45),
    ],
};

/// One comparison row set.
#[derive(Debug, Clone)]
pub struct Comparison {
    /// The reference.
    pub reference: Reference,
    /// Channel used for our run.
    pub channel: &'static str,
    /// `(d, published, ours)`.
    pub rows: Vec<(f64, f64, Option<f64>)>,
    /// Mean absolute deviation over rows with a measurement, PDR points.
    pub mad: Option<f64>,
    /// Our report's summary line.
    pub ours: String,
}

fn compare(reference: Reference, report: &SweepReport) -> Comparison {
    let rows: Vec<(f64, f64, Option<f64>)> = reference
        .points
        .iter()
        .map(|(d, p)| (*d, *p, report.pdr_at(d.max(1.0))))
        .collect();
    let devs: Vec<f64> = rows
        .iter()
        .filter_map(|(_, p, o)| o.map(|o| (o - p).abs()))
        .collect();
    let mad = (!devs.is_empty()).then(|| devs.iter().sum::<f64>() / devs.len() as f64);
    Comparison {
        reference,
        channel: report.channel,
        rows,
        mad,
        ours: format!(
            "{} vehicles, {} transmissions, {} arrivals evaluated",
            report.vehicles, report.transmissions, report.evaluated
        ),
    }
}

/// Runs every comparison. `quick` shortens each sweep to 1 s after 1 s of warm-up.
pub fn run_all(quick: bool) -> Vec<Comparison> {
    run_channels(
        quick,
        &[
            ChannelModel::WinnerPlusB1,
            ChannelModel::WinnerPlusB1ActualHeights,
            ChannelModel::WinnerB1ShapedTodoCalibrate,
            ChannelModel::Tr37885HighwayLos,
        ],
    )
}

/// Runs every comparison on the given channels.
pub fn run_channels(quick: bool, channels: &[ChannelModel]) -> Vec<Comparison> {
    let dur = |s: HighwaySweep| {
        if quick {
            s.with_duration(Duration::from_secs(1), Duration::from_secs(1))
        } else {
            s
        }
    };
    let mut out = Vec::new();
    for &channel in channels {
        // Veins: 60 veh/km, 10 Hz, 190 B, 23 dBm, 6 Mbit/s.
        let mut s = HighwaySweep::highway_fast(10).with_channel(channel);
        s.payload_cycle = vec![190];
        s.tx_power_dbm = 23.0;
        s.mean_speed_mps = 70.0 / 3.6;
        let r = sweep_dsrc(&dur(s), DsrcConfig::default());
        out.push(compare(VEINS_6MBPS_60VPKM, &r));

        // OpenCV2X: 20 dBm, MCS 7 (QPSK r0.5), 190 B, RSRP threshold -126 dBm.
        for (density, reference) in [(60.0, OPENCV2X_006), (120.0, OPENCV2X_012)] {
            let mut s = HighwaySweep::highway_slow(10)
                .with_channel(channel)
                .with_density(density);
            s.payload_cycle = vec![190];
            s.tx_power_dbm = 20.0;
            let mut pool = PoolConfig::molina_masegosa_highway();
            pool.mcs = LTE_QPSK_R050;
            let mut sps = SpsParams::molina_masegosa(10);
            sps.rsrp_threshold_dbm = -126.0;
            let r = sweep_sidelink(&dur(s), pool, sps);
            out.push(compare(reference, &r));
        }

        // Molina-Masegosa 2017, the harness's own configuration.
        let s = HighwaySweep::highway_slow(10).with_channel(channel);
        let r = sweep_sidelink(
            &dur(s),
            PoolConfig::molina_masegosa_highway(),
            SpsParams::molina_masegosa(10),
        );
        out.push(compare(MOLINA_10PPS, &r));
    }
    out
}

/// The report section.
pub fn markdown(rows: &[Comparison]) -> String {
    use std::fmt::Write as _;
    let mut s = String::new();
    for c in rows {
        let _ = writeln!(s, "#### {} — channel `{}`\n", c.reference.name, c.channel);
        let _ = writeln!(
            s,
            "Reference: {}. This engine: {}.\n",
            c.reference.source, c.ours
        );
        let _ = writeln!(
            s,
            "| distance (m) | published PDR | this engine | difference |"
        );
        let _ = writeln!(s, "|---:|---:|---:|---:|");
        for (d, p, o) in &c.rows {
            match o {
                Some(o) => {
                    let _ = writeln!(s, "| {d:.0} | {p:.3} | {o:.3} | {:+.3} |", o - p);
                }
                None => {
                    let _ = writeln!(s, "| {d:.0} | {p:.3} | — | — |");
                }
            }
        }
        if let Some(m) = c.mad {
            let _ = writeln!(
                s,
                "\nMean absolute deviation: **{:.1} PDR points**.\n",
                m * 100.0
            );
        }
    }
    s
}

#[cfg(test)]
mod winner_tests {
    use super::*;

    /// The three studies on the channel they ran on. A measurement, not a gate: run with
    /// `cargo test -p v2xw-interop winner -- --ignored --nocapture` and read the tables.
    #[test]
    #[ignore = "measurement: a few minutes in a debug build"]
    fn the_studies_on_winner_plus_b1() {
        let rows = run_channels(
            false,
            &[
                ChannelModel::WinnerPlusB1,
                ChannelModel::WinnerPlusB1ActualHeights,
            ],
        );
        println!("{}", markdown(&rows));
        for r in &rows {
            assert!(r.mad.is_some(), "{} measured nothing", r.reference.name);
        }
    }
}

/// The OpenCV2X pool as McCarthy et al. 2021 configure it: two sub-channels of 12 RB in
/// the 10 MHz channel (adjacent PSCCH), rather than the harness's four of 12 in 50 PRB.
#[must_use]
pub fn opencv2x_pool() -> PoolConfig {
    let mut pool = PoolConfig::molina_masegosa_highway();
    pool.mcs = LTE_QPSK_R050;
    pool.bandwidth_prb = 24;
    pool
}

/// One OpenCV2X sweep at `density` on WINNER+ B1 actual heights, with `pool`.
pub fn opencv2x_sweep(density: f64, pool: PoolConfig, quick: bool) -> SweepReport {
    let mut s = HighwaySweep::highway_slow(10)
        .with_channel(ChannelModel::WinnerPlusB1ActualHeights)
        .with_density(density);
    if quick {
        s = s.with_duration(Duration::from_secs(1), Duration::from_secs(2));
    }
    s.payload_cycle = vec![190];
    s.tx_power_dbm = 20.0;
    let mut sps = SpsParams::molina_masegosa(10);
    sps.rsrp_threshold_dbm = -126.0;
    sweep_sidelink(&s, pool, sps)
}

#[cfg(test)]
mod opencv2x_tests {
    use super::*;

    /// Sub-channelisation and inter-packet gap against OpenCV2X. A measurement: run with
    /// `cargo test -p v2xw-interop opencv2x_tests -- --ignored --nocapture`.
    #[test]
    #[ignore = "measurement"]
    fn subchannels_and_ipg() {
        let mut harness = PoolConfig::molina_masegosa_highway();
        harness.mcs = LTE_QPSK_R050;
        // OpenCV2X's Table II pool with 190 B in one sub-channel, as its PDR curve implies
        // (one 12-RB sub-channel carries 1,105 bits at QPSK r0.5 here, so the code rate is
        // raised to 0.72, the least that fits 1,520 bits): a bound, since the error curve
        // then belongs to that rate, not to MCS 7.
        let mut one = opencv2x_pool();
        one.mcs = v2xw_radio::sidelink::SlMcsSpec::new("opencv2x-190b-one-subchannel", 2, 740);
        for (name, pool) in [
            ("4x12 RB/50 PRB", harness),
            ("2x12 RB", opencv2x_pool()),
            ("2x12 RB, 190 B in one", one),
        ] {
            println!(
                "VERDICT pool {name}: {} sub-channels, 190 B takes {:?}",
                pool.subchannels(),
                pool.subchannels_for(190)
            );
            for (density, reference) in [(60.0, OPENCV2X_006), (120.0, OPENCV2X_012)] {
                let r = opencv2x_sweep(density, pool.clone(), false);
                let c = compare(reference, &r);
                let pts: Vec<String> = c
                    .rows
                    .iter()
                    .map(|(d, p, o)| format!("{d:.0}:{p:.2}/{:.3}", o.unwrap_or(f64::NAN)))
                    .collect();
                println!(
                    "VERDICT {density} veh/km MAD {:.1} pts: {}",
                    c.mad.unwrap_or(f64::NAN) * 100.0,
                    pts.join(" ")
                );
                let ipg: Vec<String> = r
                    .bins
                    .iter()
                    .step_by(5)
                    .map(|b| {
                        format!(
                            "{:.0}:{:.0}/{:.0}",
                            b.centre_m,
                            b.mean_ipg_ms().unwrap_or(f64::NAN),
                            b.ipg_max_ms
                        )
                    })
                    .collect();
                println!("VERDICT   IPG mean/max ms by m: {}", ipg.join(" "));
            }
        }
    }
}
