//! The 802.11p arm of the harness with its congestion control in the loop: the models of
//! [`crate::dcc`] reading the busy ratio each station measures and deciding its rate (and,
//! for SAE J2945/1, its power) every `T_CBR`, on the same ring highway as
//! [`super::sweep_dsrc`], so each algorithm can be watched as density rises.
//!
//! # The loop
//!
//! Time advances in control intervals of 100 ms — `T_CBR` in ETSI TS 102 687 §5.2 and
//! `vCBPMeasInt` / `vTxRateCntrlInt` in SAE J2945/1 (Rostami 2018 Table 1). Inside one:
//!
//! 1. every station generates on its nominal 100 ms period, stretched by its DCC: J2945/1's
//!    `MaxITT` from the smoothed neighbour count (Ahmad 2019 Eq. 5-6), the ETSI algorithms'
//!    `T_off` (EN 302 637-2's `T_GenCam_Dcc`);
//! 2. an ETSI station's frame then passes the gatekeeper ([`crate::traits::Dcc::gate`]),
//!    which may hold it or, for a frame the EN 302 571 floor refuses, drop it;
//! 3. the frames contend for the medium exactly as in [`super::sweep_dsrc`] — sequential
//!    CSMA/CA with deferral to every audible committed transmission;
//! 4. at the interval's end every station measures its busy ratio — the time its energy
//!    detector saw the medium above the CCA threshold, its own transmissions included
//!    (J2945/1's CBP counts "frame reception, or frame transmission", Rostami Eq. 7) —
//!    and hands it, with its neighbour count within 100 m, to its DCC.
//!
//! Reception is then evaluated over the committed transmissions after the warm-up, with
//! the same PHY, channel and shadowing as the unregulated sweep, each frame at the power
//! its DCC gave it.

use super::*;
use crate::dcc::{AdaptiveDcc, ReactiveDcc, SaeJ2945Dcc};
use crate::traits::Dcc;
use crate::types::{GateDecision, TxRequest};

/// Which congestion control the stations run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SweepDcc {
    /// None: 10 Hz at full power whatever the load.
    Off,
    /// `dcc/sae/j2945-1-rate-power`.
    SaeJ2945,
    /// `dcc/etsi/adaptive-ts102687`.
    EtsiAdaptive,
    /// `dcc/etsi/reactive-ts102687`, Table A.1.
    EtsiReactive,
}

impl SweepDcc {
    /// Every arm, in a fixed order.
    pub const ALL: [SweepDcc; 4] = [
        SweepDcc::Off,
        SweepDcc::SaeJ2945,
        SweepDcc::EtsiAdaptive,
        SweepDcc::EtsiReactive,
    ];

    /// The label a report prints.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            SweepDcc::Off => "off",
            SweepDcc::SaeJ2945 => "sae-j2945-1",
            SweepDcc::EtsiAdaptive => "etsi-adaptive",
            SweepDcc::EtsiReactive => "etsi-reactive",
        }
    }
}

enum Model {
    Off,
    J(SaeJ2945Dcc),
    A(AdaptiveDcc),
    R(ReactiveDcc),
}

impl Model {
    fn new(dcc: SweepDcc) -> Self {
        match dcc {
            SweepDcc::Off => Model::Off,
            SweepDcc::SaeJ2945 => Model::J(SaeJ2945Dcc::new()),
            SweepDcc::EtsiAdaptive => Model::A(AdaptiveDcc::new()),
            SweepDcc::EtsiReactive => Model::R(ReactiveDcc::new()),
        }
    }

    fn on_cbr(&mut self, ctx: &mut SweepCtx, node: NodeId, cbr: f64) {
        match self {
            Model::Off => {}
            Model::J(d) => Dcc::on_cbr(d, ctx, node, cbr),
            Model::A(d) => Dcc::on_cbr(d, ctx, node, cbr),
            Model::R(d) => Dcc::on_cbr(d, ctx, node, cbr),
        }
    }

    fn on_density(&mut self, ctx: &mut SweepCtx, node: NodeId, n: u32) {
        if let Model::J(d) = self {
            d.on_density(ctx, node, n);
        }
    }

    /// The interval the generator waits between two messages: the nominal period,
    /// stretched to the DCC's inter-transmit time or `T_off`.
    fn generation_interval(&self, node: NodeId, nominal: u64) -> u64 {
        let t_off = match self {
            Model::Off => 0,
            Model::J(d) => d.itt(node).as_nanos(),
            Model::A(d) => <AdaptiveDcc as Dcc<SweepCtx>>::state(d, node)
                .t_off
                .as_nanos(),
            Model::R(d) => <ReactiveDcc as Dcc<SweepCtx>>::state(d, node)
                .t_off
                .as_nanos(),
        };
        nominal.max(t_off)
    }

    fn gate(&mut self, ctx: &mut SweepCtx, node: NodeId, req: &TxRequest) -> GateDecision {
        match self {
            Model::A(d) => Dcc::gate(d, ctx, node, req),
            Model::R(d) => Dcc::gate(d, ctx, node, req),
            _ => GateDecision::Now {
                power_dbm: req.power_dbm,
                mcs: req.mcs,
            },
        }
    }

    /// The radiated power the DCC allows, dBm EIRP; `None` when it does not control power.
    fn radiated_power_dbm(&self, node: NodeId) -> Option<f64> {
        match self {
            Model::J(d) => Some(d.power_dbm(node)),
            _ => None,
        }
    }
}

/// What one closed-loop sweep measured.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DccSweepReport {
    /// The congestion control.
    pub dcc: &'static str,
    /// Vehicle density, veh/km over all lanes.
    pub density_veh_per_km: f64,
    /// Vehicles on the ring.
    pub vehicles: usize,
    /// Mean busy ratio the stations measured, over stations and control intervals after
    /// the warm-up.
    pub mean_cbr: f64,
    /// Largest per-interval mean busy ratio after the warm-up.
    pub peak_cbr: f64,
    /// Mean transmissions per station per second after the warm-up.
    pub rate_hz: f64,
    /// Mean radiated power of the transmissions after the warm-up, dBm EIRP.
    pub mean_eirp_dbm: f64,
    /// Mean number of other stations within 100 m, the J2945/1 density input.
    pub mean_neighbours_100m: f64,
    /// Frames the gatekeeper dropped.
    pub gate_drops: u64,
    /// Delivery per distance bin.
    pub bins: Vec<BinStats>,
}

impl DccSweepReport {
    /// Delivery over the bins whose centre lies in `[lo, hi)`, pooled.
    #[must_use]
    pub fn pdr_between(&self, lo: f64, hi: f64) -> Option<f64> {
        let (mut e, mut r) = (0u64, 0u64);
        for b in &self.bins {
            if b.centre_m >= lo && b.centre_m < hi {
                e += b.evaluated;
                r += b.received;
            }
        }
        (e > 0).then(|| r as f64 / e as f64)
    }
}

/// One committed transmission and the conducted power it went out at.
#[derive(Debug, Clone, Copy)]
struct Sent {
    vehicle: usize,
    start: SimTime,
    end: SimTime,
    bytes: u32,
    power_dbm: f64,
}

/// Runs the 802.11p arm with `dcc` in the loop, reception included; see the module
/// documentation.
///
/// # Panics
///
/// If a frame exceeds the MSDU cap, which is a configuration error.
#[must_use]
pub fn sweep_dsrc_dcc(sweep: &HighwaySweep, cfg: DsrcConfig, dcc: SweepDcc) -> DccSweepReport {
    run(sweep, cfg, dcc, true)
}

/// The same loop without the reception pass: load, rate and power only, and every
/// delivery bin empty. Reception is most of the cost at a saturating density, and the
/// congestion control never reads it.
///
/// # Panics
///
/// As [`sweep_dsrc_dcc`].
#[must_use]
pub fn sweep_dsrc_dcc_load(sweep: &HighwaySweep, cfg: DsrcConfig, dcc: SweepDcc) -> DccSweepReport {
    run(sweep, cfg, dcc, false)
}

fn run(sweep: &HighwaySweep, cfg: DsrcConfig, dcc: SweepDcc, reception: bool) -> DccSweepReport {
    use crate::phy::{Arrival, InterferenceSource, OfdmPhy};
    use crate::traits::Phy;

    let mut ctx = SweepCtx::new(sweep.seed);
    let vehicles = place_vehicles(sweep, &ctx);
    let n = vehicles.len();
    let mut model = Model::new(dcc);
    let gain = sweep.antenna_gain_dbi;
    let gain2 = 2.0 * gain;
    let total_ns = sweep.warmup.as_nanos() + sweep.duration.as_nanos();
    let warm = sweep.warmup.as_nanos();
    let nominal_ns = 1_000_000_000u64 / u64::from(sweep.packets_per_second.max(1));
    let interval_ns = Duration::from_millis(100).as_nanos();
    let slot_ns = crate::types::timing::SLOT_TIME.as_nanos();
    let aifs_ns = cfg.ac.aifs().as_nanos();
    let cw = cfg.ac.cw_min();
    let f_hz = ChannelId::CCH.centre_hz();

    let along_at = |i: usize, t: SimTime| -> f64 {
        let v = &vehicles[i];
        let mut a = (v.start_m + v.velocity_mps * (t as f64 * 1e-9)) % sweep.ring_m;
        if a < 0.0 {
            a += sweep.ring_m;
        }
        a
    };
    let distance_at = |i: usize, j: usize, t: SimTime| -> f64 {
        let dl = ring_delta(along_at(i, t), along_at(j, t), sweep.ring_m);
        let dlat = vehicles[i].lateral_m - vehicles[j].lateral_m;
        math::sqrt(dl * dl + dlat * dlat).max(1.0)
    };
    // Received power at `rx` of a transmission from `tx` at conducted power `p`, without
    // shadowing: what carrier sense and the busy-ratio meter act on.
    let heard = |tx: usize, rx: usize, p: f64, t: SimTime| -> f64 {
        p + gain2 - sweep.channel.path_loss_db(distance_at(tx, rx, t), f_hz)
    };

    let mut next_gen: Vec<u64> = vehicles.iter().map(|v| v.phase_ns).collect();
    let mut cycle: Vec<usize> = vehicles.iter().map(|v| v.cycle).collect();
    let mut sent: Vec<Sent> = Vec::new();
    let mut longest_air = 0u64;
    let mut gate_drops = 0u64;
    let mut cbr_sum = 0.0f64;
    let mut cbr_n = 0u64;
    let mut peak_cbr = 0.0f64;
    let mut neighbour_sum = 0.0f64;
    let mut neighbour_n = 0u64;

    let mut t0 = 0u64;
    while t0 < total_ns {
        let t1 = (t0 + interval_ns).min(total_ns);
        // 1. Generation inside the interval.
        let mut events: Vec<(SimTime, usize)> = Vec::new();
        for i in 0..n {
            while next_gen[i] < t1 {
                events.push((next_gen[i], i));
                next_gen[i] += model.generation_interval(NodeId::new(i as u32), nominal_ns);
            }
        }
        events.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));

        // 2-3. Gatekeeper, then CSMA/CA against what is already committed.
        for (gen_at, i) in events {
            let payload = sweep.payload_cycle[cycle[i] % sweep.payload_cycle.len()];
            cycle[i] += 1;
            let bytes = payload + cfg.overhead_bytes;
            assert!(
                bytes <= crate::types::timing::MAX_MSDU_BYTES,
                "a {bytes} B frame exceeds the MSDU cap"
            );
            let air = crate::phy::air_time(bytes, cfg.mcs);
            let node = NodeId::new(i as u32);
            let conducted = match model.radiated_power_dbm(node) {
                Some(rp) => sweep.tx_power_dbm.min(rp - gain),
                None => sweep.tx_power_dbm,
            };
            let mut ready = gen_at;
            let mut dropped = false;
            for _ in 0..8 {
                ctx.set_now(ready);
                let req = TxRequest {
                    bytes,
                    mcs: cfg.mcs,
                    power_dbm: conducted,
                    ac: cfg.ac,
                    channel: ChannelId::CCH,
                    air_time: air,
                    at: ready,
                };
                match model.gate(&mut ctx, node, &req) {
                    GateDecision::Now { .. } => break,
                    GateDecision::DelayUntil(t) => ready = t.max(ready + 1),
                    GateDecision::Drop => {
                        dropped = true;
                        break;
                    }
                }
            }
            if dropped || ready >= total_ns {
                gate_drops += u64::from(dropped);
                continue;
            }
            let air_ns = air.as_nanos();
            longest_air = longest_air.max(air_ns);
            ctx.set_now(ready);
            let drawn = ctx
                .rng(RngDomain::MacBackoff, EntityRef::Node(node))
                .below(u64::from(cw) + 1) as u32;
            let mut remaining = drawn;
            let mut t = ready + aifs_ns;
            let mut guard = 0;
            loop {
                guard += 1;
                if guard > 4096 {
                    break;
                }
                let mut busy_until: Option<SimTime> = None;
                let mut next_start: Option<SimTime> = None;
                for c in sent.iter().rev() {
                    if c.end + longest_air < t {
                        if c.start + 2 * longest_air < t {
                            break;
                        }
                        continue;
                    }
                    if c.vehicle == i {
                        continue;
                    }
                    if heard(c.vehicle, i, c.power_dbm, t) < cfg.cca_threshold_dbm {
                        continue;
                    }
                    if c.start <= t && t < c.end {
                        busy_until = Some(busy_until.map_or(c.end, |b: SimTime| b.max(c.end)));
                    } else if c.start > t {
                        next_start = Some(next_start.map_or(c.start, |s: SimTime| s.min(c.start)));
                    }
                }
                if let Some(until) = busy_until {
                    t = until + aifs_ns;
                    continue;
                }
                let idle_ns = next_start.map_or(u64::MAX, |s| s.saturating_sub(t));
                let slots_available = idle_ns / slot_ns;
                if u64::from(remaining) <= slots_available {
                    t += u64::from(remaining) * slot_ns;
                    break;
                }
                remaining -= slots_available as u32;
                t = next_start.unwrap_or(t) + 1;
            }
            sent.push(Sent {
                vehicle: i,
                start: t,
                end: t + air_ns,
                bytes,
                power_dbm: conducted,
            });
        }
        sent.sort_by(|a, b| a.start.cmp(&b.start).then(a.vehicle.cmp(&b.vehicle)));

        // 4. Every station's busy ratio over the interval, and its neighbour count.
        let lo = sent.partition_point(|c| c.end <= t0);
        let span = (t1 - t0) as f64;
        let mut interval_cbr = 0.0;
        for rx in 0..n {
            let mut busy = 0u64;
            for c in &sent[lo..] {
                if c.start >= t1 {
                    break;
                }
                let audible = c.vehicle == rx
                    || heard(c.vehicle, rx, c.power_dbm, c.start) >= cfg.cca_threshold_dbm;
                if audible {
                    busy += c.end.min(t1) - c.start.max(t0);
                }
            }
            let cbr = (busy as f64 / span).min(1.0);
            let neighbours = (0..n)
                .filter(|&j| j != rx && distance_at(rx, j, t1) <= 100.0)
                .count() as u32;
            ctx.set_now(t1);
            let node = NodeId::new(rx as u32);
            model.on_cbr(&mut ctx, node, cbr);
            model.on_density(&mut ctx, node, neighbours);
            if t0 >= warm {
                cbr_sum += cbr;
                cbr_n += 1;
                interval_cbr += cbr;
                neighbour_sum += f64::from(neighbours);
                neighbour_n += 1;
            }
        }
        if t0 >= warm && n > 0 {
            peak_cbr = peak_cbr.max(interval_cbr / n as f64);
        }
        t0 = t1;
    }

    // Reception, after the warm-up, with shadowing.
    let mut phy = OfdmPhy::new(v2xw_core::card::Tier::High);
    let mut shadow = ShadowField::new(n, sweep.channel.sigma_db(), sweep.channel.decorrelation_m());
    let mut bins: Vec<BinStats> = (0..sweep.bins())
        .map(|i| BinStats {
            centre_m: sweep.bin_centre_m(i),
            ..BinStats::default()
        })
        .collect();
    let mut counted = 0u64;
    let mut eirp_sum = 0.0;
    for tx in &sent {
        if tx.start >= warm {
            counted += 1;
            eirp_sum += tx.power_dbm + gain;
        }
    }
    for (k, tx) in sent.iter().enumerate() {
        if !reception || tx.start < warm {
            continue;
        }
        let lo = sent.partition_point(|c| c.end <= tx.start);
        let hi = sent.partition_point(|c| c.start < tx.end);
        ctx.set_now(tx.start);
        for rx in 0..n {
            if rx == tx.vehicle {
                continue;
            }
            let d = distance_at(tx.vehicle, rx, tx.start);
            let Some(bin) = sweep.bin_of(d) else {
                continue;
            };
            let mut loss = sweep.channel.path_loss_db(d, f_hz);
            if sweep.shadowing {
                loss -= shadow.update(&ctx, tx.vehicle, rx, d, tx.start / 1_000_000);
            }
            let p = tx.power_dbm + gain2 - loss;
            let mut interferers: Vec<InterferenceSource> = Vec::new();
            for (off, other) in sent[lo..hi].iter().enumerate() {
                let j = lo + off;
                if j == k || other.vehicle == tx.vehicle || other.vehicle == rx {
                    continue;
                }
                let op = heard(other.vehicle, rx, other.power_dbm, other.start);
                let mut src = InterferenceSource::new(
                    NodeId::new(other.vehicle as u32),
                    op,
                    other.start,
                    other.end,
                );
                if heard(other.vehicle, tx.vehicle, other.power_dbm, tx.start)
                    < cfg.cca_threshold_dbm
                {
                    src = src.hidden();
                }
                interferers.push(src);
            }
            let frame = FrameDescriptor {
                tx_power_dbm: tx.power_dbm,
                ..FrameDescriptor::broadcast(
                    tx.bytes,
                    cfg.mcs,
                    SduRef::new(SduId::new(tx.vehicle as u32), FrameSeq::new(k as u32)),
                )
            };
            let handle = phy.register_arrival(Arrival {
                tx_id: k as u64 + 1,
                tx: NodeId::new(tx.vehicle as u32),
                rx: NodeId::new(rx as u32),
                power_dbm: p,
                start: tx.start,
                end: tx.end,
                frame,
                interferers,
            });
            let outcome = Phy::finish_rx(&mut phy, &mut ctx, NodeId::new(rx as u32), handle);
            bins[bin].evaluated += 1;
            if matches!(outcome, RxOutcome::Received { .. }) {
                bins[bin].received += 1;
            }
        }
    }
    let measured_s = sweep.duration.as_secs_f64().max(1e-9);
    DccSweepReport {
        dcc: dcc.label(),
        density_veh_per_km: sweep.density_veh_per_km,
        vehicles: n,
        mean_cbr: if cbr_n > 0 {
            cbr_sum / cbr_n as f64
        } else {
            0.0
        },
        peak_cbr,
        rate_hz: if n > 0 {
            counted as f64 / n as f64 / measured_s
        } else {
            0.0
        },
        mean_eirp_dbm: if counted > 0 {
            eirp_sum / counted as f64
        } else {
            f64::NAN
        },
        mean_neighbours_100m: if neighbour_n > 0 {
            neighbour_sum / neighbour_n as f64
        } else {
            0.0
        },
        gate_drops,
        bins,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dcc::J2945Params;

    /// A 1 km ring, three lanes each way, 300 B frames every 100 ms at 20 dBm radiated
    /// (J2945/1's `vRPMax`), 6 Mbit/s, on the measurement-fitted highway LOS law. The
    /// warm-up is 6 s because J2945/1's `vDensityWeightFactor` of 0.05 closes only 5 %
    /// of the gap to the counted density every 100 ms (95 % after 5.8 s).
    fn ring(density: f64) -> HighwaySweep {
        HighwaySweep {
            ring_m: 1000.0,
            payload_cycle: vec![300],
            density_veh_per_km: density,
            tx_power_dbm: 20.0 - 3.0,
            max_distance_m: 300.0,
            bin_m: 50.0,
            sensing_range_m: 500.0,
            ..HighwaySweep::highway_slow(10)
        }
        .with_duration(Duration::from_secs(6), Duration::from_secs(2))
    }

    fn load(density: f64, dcc: SweepDcc) -> DccSweepReport {
        sweep_dsrc_dcc_load(&ring(density), DsrcConfig::default(), dcc)
    }

    /// With no congestion control the channel fills as density rises; each algorithm then
    /// does what its standard says it does at a load that saturates the unregulated
    /// channel:
    ///
    /// * **ETSI adaptive** (TS 102 687 §5.4): δ is driven down until the smoothed load sits
    ///   at or below `CBR_target` = 0.68, so the measured load is held under the
    ///   saturating one.
    /// * **ETSI reactive** (TS 102 687 Annex A Table A.1): the station's rate is its state's
    ///   — 10, 5, 2.5, 2 or 1 Hz — so the load is held below the saturating one.
    /// * **SAE J2945/1** (Ahmad 2019 Eq. 5-9): `MaxITT` stretches to `100·N/25` ms with
    ///   the neighbour count `N` within 100 m, and the radiated power falls from 20 dBm
    ///   towards 10 dBm as the busy percentage passes `vMinCU` = 0.5. On a straight ring
    ///   at constant speed the neighbours' coasting is exact, so no tracking-error BSM is
    ///   sent and the rate is `MaxITT`'s.
    #[test]
    fn each_dcc_holds_the_load_its_standard_says_as_density_rises() {
        let light = load(40.0, SweepDcc::Off);
        let heavy = load(400.0, SweepDcc::Off);
        eprintln!(
            "off: light cbr {:.3}, heavy cbr {:.3} rate {:.2}",
            light.mean_cbr, heavy.mean_cbr, heavy.rate_hz
        );
        assert!(
            light.mean_cbr < 0.3,
            "light unregulated load {}",
            light.mean_cbr
        );
        assert!(
            heavy.mean_cbr > 0.75,
            "heavy unregulated load {}",
            heavy.mean_cbr
        );
        assert!(
            (heavy.rate_hz - 10.0).abs() < 0.5,
            "unregulated rate {}",
            heavy.rate_hz
        );
        assert!(
            heavy.bins.iter().all(|b| b.evaluated == 0),
            "the load run evaluates no reception"
        );

        let adaptive = load(400.0, SweepDcc::EtsiAdaptive);
        eprintln!(
            "adaptive: cbr {:.3} peak {:.3} rate {:.2}",
            adaptive.mean_cbr, adaptive.peak_cbr, adaptive.rate_hz
        );
        assert!(
            adaptive.mean_cbr < heavy.mean_cbr - 0.05 && adaptive.mean_cbr < 0.75,
            "adaptive load {} against unregulated {}",
            adaptive.mean_cbr,
            heavy.mean_cbr
        );
        assert!(adaptive.rate_hz < 9.0, "adaptive rate {}", adaptive.rate_hz);

        let reactive = load(400.0, SweepDcc::EtsiReactive);
        eprintln!(
            "reactive: cbr {:.3} peak {:.3} rate {:.2}",
            reactive.mean_cbr, reactive.peak_cbr, reactive.rate_hz
        );
        assert!(
            reactive.mean_cbr < heavy.mean_cbr - 0.05,
            "{}",
            reactive.mean_cbr
        );
        assert!(reactive.rate_hz < 5.5, "reactive rate {}", reactive.rate_hz);

        let j2945 = load(400.0, SweepDcc::SaeJ2945);
        let n = j2945.mean_neighbours_100m;
        let itt_ms = (100.0 * n / 25.0).clamp(100.0, 600.0);
        eprintln!(
            "j2945: cbr {:.3} rate {:.2} eirp {:.1} neighbours {:.1} (MaxITT {:.0} ms)",
            j2945.mean_cbr, j2945.rate_hz, j2945.mean_eirp_dbm, n, itt_ms
        );
        // The measured rate is Eq. 6's for the measured density, within 15 %.
        let want_hz = 1000.0 / itt_ms;
        assert!(
            (j2945.rate_hz - want_hz).abs() / want_hz < 0.15,
            "J2945/1 rate {:.2} Hz against MaxITT {:.0} ms ({want_hz:.2} Hz)",
            j2945.rate_hz,
            itt_ms
        );
        assert!(j2945.mean_cbr < heavy.mean_cbr - 0.05, "{}", j2945.mean_cbr);
        // Power follows Eq. 9 for the load it measured: 20 dBm up to 50 %, 10 dBm from 80 %.
        let want_rp =
            J2945Params::J2945_1.rp_max_dbm - 10.0 * ((j2945.mean_cbr - 0.5) / 0.3).clamp(0.0, 1.0);
        assert!(
            (j2945.mean_eirp_dbm - want_rp).abs() < 2.0,
            "J2945/1 power {:.1} dBm against f(CBP) {want_rp:.1} dBm",
            j2945.mean_eirp_dbm
        );

        // At a light load nobody is throttled.
        for dcc in [
            SweepDcc::SaeJ2945,
            SweepDcc::EtsiAdaptive,
            SweepDcc::EtsiReactive,
        ] {
            let r = load(40.0, dcc);
            assert!(
                (r.rate_hz - 10.0).abs() < 0.5,
                "{dcc:?} throttles a light load: {r:?}"
            );
        }
    }

    /// The table of the density sweep, one row per algorithm and density, delivery
    /// included.
    #[test]
    #[ignore = "measurement: prints load, rate, power and delivery against density"]
    fn dcc_against_density_table() {
        println!("dcc            veh/km  N100  cbr    peak   rate_Hz  eirp   pdr100  pdr200");
        for density in [25.0, 50.0, 100.0, 200.0, 300.0, 400.0, 600.0] {
            for dcc in SweepDcc::ALL {
                let r = sweep_dsrc_dcc(&ring(density), DsrcConfig::default(), dcc);
                println!(
                    "{:<14} {:>6.0} {:>5.1} {:>6.3} {:>6.3} {:>7.2} {:>6.1} {:>7.3} {:>7.3}",
                    r.dcc,
                    density,
                    r.mean_neighbours_100m,
                    r.mean_cbr,
                    r.peak_cbr,
                    r.rate_hz,
                    r.mean_eirp_dbm,
                    r.pdr_between(0.0, 100.0).unwrap_or(f64::NAN),
                    r.pdr_between(150.0, 250.0).unwrap_or(f64::NAN),
                );
            }
        }
    }
}
