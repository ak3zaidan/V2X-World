//! `cellular/uu/packet-pf` — a packet-level LTE / NR Uu plug-in beside the three tiers of
//! [`crate::cellular`]: base stations laid out per city, one proportional-fair scheduler
//! per cell and direction, HARQ, RLC acknowledged mode, a measured handover interruption
//! with lossless data forwarding, and the core-network leg.
//!
//! # What it adds over `cellular/uu/cell-capacity-mm1`
//!
//! | Mechanism | Simple model | This model |
//! |---|---|---|
//! | Capacity | one aggregate bit rate per cell, M/M/1 per node | PRBs per TTI shared by a PF scheduler across every UE the cell serves, at each UE's own spectral efficiency |
//! | Link quality | none | SINR from the Uu path loss of the serving site against the neighbour sites, mapped to a spectral efficiency |
//! | Errors | none | HARQ at the 10 % first-transmission BLER the CQI is defined at, then RLC AM retransmission, then loss |
//! | Uplink access | none | scheduling-request wait and the grant chain before the first uplink TTI |
//! | Handover | none (medium) / loss inside the interruption (high) | packets held for the interruption and forwarded to the target cell (RLC AM bearers are lossless, TS 36.300 §10.1.2.3) |
//!
//! # How a delivery instant is computed
//!
//! Each cell keeps its scheduler state lazily: it is advanced to `now` when a packet
//! arrives, the new packet is added, and a *copy* of the cell is rolled forward until that
//! packet is delivered or lost. The packet's instant is therefore the one the scheduler
//! gives it with the backlog known at send time; a packet arriving later shares the TTIs
//! that follow but does not move a delivery already reported. Every HARQ draw is a pure
//! hash of the UE's salt, the packet and the attempt, so the copy and the real cell agree
//! draw for draw and the result is a function of the schedule alone (determinism contract).
//!
//! # Comparison with the simple model
//!
//! Every send also evaluates the M/M/1 model of `cellular/uu/cell-capacity-mm1` on the same
//! arrival, analytically and without a draw, and [`PacketUu::comparison`] says when the
//! two disagree by more than 25 % or 5 ms on the mean, or on loss.

use std::collections::{BTreeMap, VecDeque};

use serde::{Deserialize, Serialize};
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::ctx::Ctx;
use v2xw_core::geom::Vec3;
use v2xw_core::ids::{CellId, NodeId};
use v2xw_core::math;
use v2xw_core::model::Model;
use v2xw_core::rng::{EntityRef, RngDomain};
use v2xw_core::time::{Duration, SimTime};

use crate::cellular::{
    CellCapacityUu, CellPlan, CellQuality, CellView, CellularUu, Direction, MecPlacement, Qos,
    RadioLatencyClass, SendOutcome, uu_path_loss_db,
};
use crate::cv2x::{THERMAL_NOISE_DBM_PER_HZ, UE_NOISE_FIGURE_DB};
use crate::types::DropCause;

// =========================================================================================
// Radio access technology
// =========================================================================================

/// Which radio access technology the cells run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rat {
    /// LTE, 20 MHz FDD, 1 ms TTI.
    Lte,
    /// NR FR1, 100 MHz at 30 kHz subcarrier spacing, 0.5 ms slot.
    Nr,
}

impl Rat {
    /// The label a scenario names.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Rat::Lte => "lte",
            Rat::Nr => "nr",
        }
    }

    /// The RAT a scenario names, by label.
    #[must_use]
    pub fn named(label: &str) -> Option<Rat> {
        match label {
            "lte" | "4g" => Some(Rat::Lte),
            "nr" | "5g" => Some(Rat::Nr),
            _ => None,
        }
    }
}

/// The numbers a RAT fixes. Each constructor cites where its values come from.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct RatConfig {
    /// Which RAT.
    pub rat: Rat,
    /// TTI (LTE subframe, NR slot), ns.
    pub tti_ns: u64,
    /// Physical resource blocks per TTI.
    pub prbs: u32,
    /// Channel bandwidth, Hz, for the noise floor.
    pub bandwidth_hz: f64,
    /// Resource elements per PRB per TTI (12 subcarriers × 14 symbols).
    pub re_per_prb: f64,
    /// Fraction of resource elements spent on control and reference signals, downlink.
    pub overhead_dl: f64,
    /// The same, uplink.
    pub overhead_ul: f64,
    /// Lowest spectral efficiency, bit per resource element (CQI 1).
    pub se_min: f64,
    /// Highest spectral efficiency, bit per resource element (top CQI).
    pub se_max: f64,
    /// HARQ round-trip time, TTIs.
    pub harq_rtt_tti: u64,
    /// HARQ transmissions before RLC takes over.
    pub max_harq_tx: u8,
    /// Block error rate of a first transmission.
    pub bler_first: f64,
    /// Block error rate of a HARQ retransmission (soft-combined).
    pub bler_retx: f64,
    /// Delay before RLC AM retransmits a block HARQ gave up on, ms.
    pub rlc_retx_ms: f64,
    /// RLC AM retransmissions before the SDU is declared lost.
    pub max_rlc_retx: u8,
    /// Scheduling-request period, ms: the uplink wait is uniform in `[0, period)`.
    pub sr_period_ms: f64,
    /// SR transmission, eNB/gNB decode, grant and UE processing before the first uplink
    /// data TTI, ms.
    pub ul_grant_ms: f64,
    /// Base-station processing of downlink data before its first TTI, ms.
    pub dl_processing_ms: f64,
    /// Mean user-plane handover interruption, ms.
    pub ho_interruption_ms: f64,
    /// Its spread, ms.
    pub ho_interruption_sigma_ms: f64,
    /// UE transmit power, dBm.
    pub ue_tx_dbm: f64,
    /// UE noise figure, dB.
    pub ue_nf_db: f64,
    /// Base-station noise figure, dB.
    pub bs_nf_db: f64,
    /// Uplink interference over thermal, dB.
    pub ul_iot_db: f64,
}

impl RatConfig {
    /// LTE, 20 MHz.
    ///
    /// 100 PRB at 20 MHz [TS 36.101 Table 5.6-1]; 1 ms subframe of 14 symbols with normal
    /// cyclic prefix [TS 36.211 §4.1, §6.2.3]; overheads 0.14 DL / 0.08 UL [the FR1 OH
    /// factors of TS 38.306 §4.1.2, used for LTE too, `todo-calibrate`]; CQI 1 and 15
    /// efficiencies 0.1523 and 5.5547 bit/RE and the 10 % BLER the CQI is defined at
    /// [TS 36.213 §7.2.3, Table 7.2.3-1]; 8 ms FDD HARQ RTT [TS 36.213 §8, §10.1]; four
    /// transmissions [ns-3 LTE `LteEnbMac::MaxHarqRetransmissions` = 3]; SR period 10 ms and
    /// the 8 ms SR → grant → data chain, 1.5 ms DL processing, 49.5 ms handover
    /// interruption [TR 36.881 §5.1, §5.2 latency analysis, `todo-calibrate`]; UE 23 dBm
    /// [TS 36.101 §6.2.2 power class 3]; UE NF 9 dB, BS NF 5 dB [TR 37.885 Table 6.1.1-1].
    #[must_use]
    pub const fn lte_20mhz() -> Self {
        Self {
            rat: Rat::Lte,
            tti_ns: 1_000_000,
            prbs: 100,
            bandwidth_hz: 18e6,
            re_per_prb: 168.0,
            overhead_dl: 0.14,
            overhead_ul: 0.08,
            se_min: 0.1523,
            se_max: 5.5547,
            harq_rtt_tti: 8,
            max_harq_tx: 4,
            bler_first: 0.1,
            bler_retx: 0.05,
            rlc_retx_ms: 20.0,
            max_rlc_retx: 4,
            sr_period_ms: 10.0,
            ul_grant_ms: 8.0,
            dl_processing_ms: 1.5,
            ho_interruption_ms: 49.5,
            ho_interruption_sigma_ms: 10.0,
            ue_tx_dbm: 23.0,
            ue_nf_db: UE_NOISE_FIGURE_DB,
            bs_nf_db: 5.0,
            ul_iot_db: 3.0,
        }
    }

    /// NR FR1, 100 MHz at 30 kHz.
    ///
    /// 273 PRB [TS 38.101-1 Table 5.3.2-1]; 0.5 ms slot of 14 symbols [TS 38.211 §4.3.2];
    /// overheads 0.14 / 0.08 [TS 38.306 §4.1.2]; top efficiency 7.4063 bit/RE of 256QAM
    /// [TS 38.214 Table 5.2.2.1-3]; the HARQ RTT, SR period, grant chain and interruption are
    /// `todo-calibrate` (a 4-slot RTT, a 1 ms SR period, a 2 ms grant chain, 0.5 ms DL
    /// processing; 30 ms interruption between the LTE figure and the 0 ms Rel-16 DAPS
    /// target of TS 38.300 §9.2.3.2).
    #[must_use]
    pub const fn nr_100mhz_30khz() -> Self {
        Self {
            rat: Rat::Nr,
            tti_ns: 500_000,
            prbs: 273,
            bandwidth_hz: 98.28e6,
            re_per_prb: 168.0,
            overhead_dl: 0.14,
            overhead_ul: 0.08,
            se_min: 0.1523,
            se_max: 7.4063,
            harq_rtt_tti: 4,
            max_harq_tx: 4,
            bler_first: 0.1,
            bler_retx: 0.05,
            rlc_retx_ms: 10.0,
            max_rlc_retx: 4,
            sr_period_ms: 1.0,
            ul_grant_ms: 2.0,
            dl_processing_ms: 0.5,
            ho_interruption_ms: 30.0,
            ho_interruption_sigma_ms: 8.0,
            ue_tx_dbm: 23.0,
            ue_nf_db: UE_NOISE_FIGURE_DB,
            bs_nf_db: 5.0,
            ul_iot_db: 3.0,
        }
    }

    /// The configuration of a RAT.
    #[must_use]
    pub const fn of(rat: Rat) -> Self {
        match rat {
            Rat::Lte => Self::lte_20mhz(),
            Rat::Nr => Self::nr_100mhz_30khz(),
        }
    }

    /// Information bits one PRB carries in one TTI per bit/RE of spectral efficiency.
    #[must_use]
    pub fn bits_per_prb_per_se(&self, dir: Direction) -> f64 {
        let oh = match dir {
            Direction::Downlink => self.overhead_dl,
            Direction::Uplink => self.overhead_ul,
        };
        self.re_per_prb * (1.0 - oh)
    }

    /// The TTI in seconds.
    #[must_use]
    pub fn tti_s(&self) -> f64 {
        self.tti_ns as f64 * 1e-9
    }

    fn ms_to_tti(&self, ms: f64) -> u64 {
        ((ms * 1e6) / self.tti_ns as f64).ceil().max(0.0) as u64
    }

    /// The spectral efficiency an SINR supports: the attenuated Shannon bound
    /// `0.75·log2(1 + SINR/1.25)` [Mogensen et al., VTC 2007-Spring, "LTE capacity compared
    /// to the Shannon bound"], clamped to the CQI table's range.
    #[must_use]
    pub fn spectral_efficiency(&self, sinr_db: f64) -> f64 {
        let lin = math::pow(10.0, sinr_db / 10.0);
        (SHANNON_ALPHA * math::log2(1.0 + lin / SHANNON_SNR_EFF)).clamp(self.se_min, self.se_max)
    }
}

/// The bandwidth-efficiency factor of the attenuated Shannon bound.
pub const SHANNON_ALPHA: f64 = 0.75;
/// The SNR-efficiency factor of the same bound (linear; about 1 dB).
pub const SHANNON_SNR_EFF: f64 = 1.25;
/// The proportional-fair averaging window, TTIs [Jalali, Padovani, Pankaj, VTC 2000 use
/// 1,000 slots; 100 TTIs is the value ns-3's PF scheduler examples use, `todo-calibrate`].
pub const PF_WINDOW_TTI: f64 = 100.0;
/// How far a copy of a cell is rolled forward before a packet is declared undeliverable.
pub const ROLLOUT_LIMIT: Duration = Duration::from_secs(30);

// =========================================================================================
// Base stations
// =========================================================================================

/// Where the base stations are.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum BaseStationLayout {
    /// The square lattice of a [`CellPlan`]: cell ids are the plan's own.
    Lattice(CellPlan),
    /// An explicit site list — a city's hexagonal deployment from
    /// [`BaseStationLayout::for_city`], or a measured site list such as an OpenCelliD
    /// export — with the plan's power figures. A cell id is the site's index.
    Sites {
        /// Power and coverage figures.
        plan: CellPlan,
        /// Site positions, metres in the world frame.
        sites: Vec<(f64, f64)>,
        /// Site indices by `isd`-sized bucket, so a lookup touches a fixed neighbourhood
        /// and never depends on a hash order.
        buckets: BTreeMap<(i64, i64), Vec<u32>>,
    },
}

impl BaseStationLayout {
    /// The lattice of a plan.
    #[must_use]
    pub fn lattice(plan: CellPlan) -> Self {
        Self::Lattice(plan)
    }

    /// An explicit site list.
    #[must_use]
    pub fn from_sites(plan: CellPlan, sites: Vec<(f64, f64)>) -> Self {
        let mut buckets: BTreeMap<(i64, i64), Vec<u32>> = BTreeMap::new();
        for (i, s) in sites.iter().enumerate() {
            buckets
                .entry(bucket_of(s.0, s.1, plan.isd_m))
                .or_default()
                .push(i as u32);
        }
        Self::Sites {
            plan,
            sites,
            buckets,
        }
    }

    /// A hexagonal deployment covering a city's bounding box at the plan's inter-site
    /// distance, with one ring of sites beyond the box so the edge has neighbours to
    /// interfere with and hand over to.
    #[must_use]
    pub fn for_city(plan: CellPlan, min: (f64, f64), max: (f64, f64)) -> Self {
        let isd = plan.isd_m.max(1.0);
        let row = isd * math::sqrt(3.0) / 2.0;
        let mut sites = Vec::new();
        let rows = ((max.1 - min.1) / row).ceil() as i64 + 2;
        let cols = ((max.0 - min.0) / isd).ceil() as i64 + 2;
        for r in -1..=rows {
            let off = if r.rem_euclid(2) == 1 { isd / 2.0 } else { 0.0 };
            for c in -1..=cols {
                sites.push((min.0 + c as f64 * isd + off, min.1 + r as f64 * row));
            }
        }
        Self::from_sites(plan, sites)
    }

    /// The plan whose power figures the layout uses.
    #[must_use]
    pub const fn plan(&self) -> &CellPlan {
        match self {
            Self::Lattice(p) | Self::Sites { plan: p, .. } => p,
        }
    }

    /// How many sites, when the layout is explicit.
    #[must_use]
    pub fn site_count(&self) -> Option<usize> {
        match self {
            Self::Lattice(_) => None,
            Self::Sites { sites, .. } => Some(sites.len()),
        }
    }

    /// The sites near a point with their distances, nearest first (ties by id).
    #[must_use]
    pub fn nearby(&self, p: Vec3) -> Vec<(u32, f64)> {
        let mut out = Vec::new();
        match self {
            Self::Lattice(plan) => {
                let (si, sj) = plan.nearest_site(p);
                for dj in -2..=2 {
                    for di in -2..=2 {
                        let s = (si + di, sj + dj);
                        if s.0.abs() > plan.extent_sites || s.1.abs() > plan.extent_sites {
                            continue;
                        }
                        let dx = p.x - f64::from(s.0) * plan.isd_m;
                        let dy = p.y - f64::from(s.1) * plan.isd_m;
                        out.push((plan.cell_id(s).index(), math::sqrt(dx * dx + dy * dy)));
                    }
                }
            }
            Self::Sites {
                plan,
                sites,
                buckets,
            } => {
                let (bi, bj) = bucket_of(p.x, p.y, plan.isd_m);
                for dj in -2..=2 {
                    for di in -2..=2 {
                        if let Some(ids) = buckets.get(&(bi + di, bj + dj)) {
                            for &id in ids {
                                let s = sites[id as usize];
                                let (dx, dy) = (p.x - s.0, p.y - s.1);
                                out.push((id, math::sqrt(dx * dx + dy * dy)));
                            }
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        out
    }

    /// The serving cell and its quality, or `None` beyond the sites or below the floor.
    #[must_use]
    pub fn coverage(&self, p: Vec3) -> Option<CellView> {
        if let Self::Lattice(plan) = self {
            return plan.coverage(p);
        }
        let plan = self.plan();
        let &(id, d) = self.nearby(p).first()?;
        let d = d.max(1.0);
        let rsrp = plan.bs_tx_power_dbm + plan.bs_gain_dbi - uu_path_loss_db(d);
        if rsrp < plan.no_coverage_below_dbm {
            return None;
        }
        let quality = if d < plan.isd_m * 0.25 {
            CellQuality::Good
        } else if d < plan.isd_m * 0.5 {
            CellQuality::Fair
        } else {
            CellQuality::Edge
        };
        Some(CellView {
            cell: CellId::new(id),
            distance_m: math::quantize_to(d, 1e-3),
            rsrp_dbm: crate::numeric::q_db(rsrp),
            quality,
        })
    }

    /// The SINR of a UE at `p` served by `cell`, dB.
    ///
    /// Downlink: the serving site's received power against the other nearby sites, each
    /// weighted by `neighbour_load` (the fraction of their PRBs in use), plus the UE's
    /// noise floor. Uplink: the UE's power spread over the whole band against the
    /// base station's noise floor raised by the interference over thermal — a deliberately
    /// pessimistic power-limited uplink, which is what a cell edge is.
    #[must_use]
    pub fn sinr_db(&self, cfg: &RatConfig, p: Vec3, cell: u32, dir: Direction, neighbour_load: f64) -> f64 {
        let plan = self.plan();
        let near = self.nearby(p);
        let d0 = near
            .iter()
            .find(|(id, _)| *id == cell)
            .map_or_else(|| near.first().map_or(plan.isd_m, |n| n.1), |n| n.1)
            .max(1.0);
        let bw_db = 10.0 * math::log10(cfg.bandwidth_hz);
        match dir {
            Direction::Downlink => {
                let tx = plan.bs_tx_power_dbm + plan.bs_gain_dbi;
                let s = dbm_to_mw(tx - uu_path_loss_db(d0));
                let mut i = 0.0;
                for &(id, d) in &near {
                    if id != cell {
                        i += neighbour_load * dbm_to_mw(tx - uu_path_loss_db(d.max(1.0)));
                    }
                }
                let n = dbm_to_mw(THERMAL_NOISE_DBM_PER_HZ + bw_db + cfg.ue_nf_db);
                10.0 * math::log10(s / (i + n))
            }
            Direction::Uplink => {
                let s = cfg.ue_tx_dbm + plan.bs_gain_dbi - uu_path_loss_db(d0);
                let n = THERMAL_NOISE_DBM_PER_HZ + bw_db + cfg.bs_nf_db + cfg.ul_iot_db;
                s - n
            }
        }
    }
}

fn bucket_of(x: f64, y: f64, isd: f64) -> (i64, i64) {
    let b = isd.max(1.0);
    ((x / b).floor() as i64, (y / b).floor() as i64)
}

fn dbm_to_mw(dbm: f64) -> f64 {
    math::pow(10.0, dbm / 10.0)
}

/// A stateless 64-bit mix (SplitMix64's finaliser), for draws that a copy of a cell and
/// the cell itself must agree on.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit(h: u64) -> f64 {
    (h >> 11) as f64 / (1u64 << 53) as f64
}

// =========================================================================================
// The per-cell scheduler
// =========================================================================================

#[derive(Debug, Clone)]
struct Pkt {
    id: u64,
    left: u32,
    inflight: u32,
    rlc_tx: u8,
    ready: u64,
    last_ok: u64,
}

#[derive(Debug, Clone)]
struct UeQ {
    avg_bps: f64,
    se: f64,
    salt: u64,
    q: VecDeque<Pkt>,
}

#[derive(Debug, Clone)]
struct Tb {
    ue: u32,
    pkt: u64,
    bytes: u32,
    prbs: u32,
    attempt: u8,
    first: u64,
    rlc: u8,
    due: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    Delivered(u64),
    Lost,
}

/// The numbers one scheduler step needs.
#[derive(Debug, Clone, Copy)]
struct StepCfg {
    bits_per_prb_se: f64,
    prbs_free: u32,
    harq_rtt: u64,
    max_harq: u8,
    bler_first: f64,
    bler_retx: f64,
    rlc_retx_tti: u64,
    max_rlc: u8,
    tti_s: f64,
}

/// One cell's scheduler in one direction.
#[derive(Debug, Clone, Default)]
struct Sched {
    tti: u64,
    ues: BTreeMap<u32, UeQ>,
    harq: Vec<Tb>,
    pending: BTreeMap<(u64, u64), (u32, f64, u64, Pkt)>,
    /// UEs whose first successful block after a handover is being timed, from this TTI.
    watch: BTreeMap<u32, u64>,
    /// Measured handover gaps, TTIs, drained by the model.
    gaps: Vec<u64>,
    done: Vec<(u64, Fate)>,
    harq_retx: u64,
    rlc_retx: u64,
}

impl Sched {
    fn has_work_now(&self) -> bool {
        let t = self.tti;
        self.harq.iter().any(|tb| tb.due <= t)
            || self
                .ues
                .values()
                .any(|u| u.q.iter().any(|p| p.left > 0 && p.ready <= t))
            || self.pending.keys().next().is_some_and(|k| k.0 <= t)
    }

    fn next_event(&self) -> Option<u64> {
        let mut e: Option<u64> = self.pending.keys().next().map(|k| k.0);
        let mut take = |v: u64| e = Some(e.map_or(v, |x| x.min(v)));
        for tb in &self.harq {
            take(tb.due);
        }
        for u in self.ues.values() {
            for p in &u.q {
                if p.left > 0 {
                    take(p.ready);
                }
            }
        }
        e
    }

    fn decay(&mut self, k: u64) {
        if k == 0 {
            return;
        }
        let f = math::pow(1.0 - 1.0 / PF_WINDOW_TTI, k as f64);
        for u in self.ues.values_mut() {
            u.avg_bps *= f;
        }
        self.ues.retain(|_, u| !u.q.is_empty() || u.avg_bps > 1.0);
        self.tti += k;
    }

    /// Advances the real state to `target` (exclusive), jumping idle stretches.
    fn advance(&mut self, target: u64, c: &StepCfg) {
        while self.tti < target {
            if self.has_work_now() {
                self.step(c);
                continue;
            }
            match self.next_event() {
                Some(e) if e > self.tti => {
                    let to = e.min(target);
                    self.decay(to - self.tti);
                }
                Some(_) => self.step(c),
                None => {
                    self.decay(target - self.tti);
                }
            }
        }
        self.done.clear();
    }

    /// Rolls a copy forward until packet `id` is delivered or lost.
    fn rollout(&self, id: u64, c: &StepCfg, limit_tti: u64) -> Option<Fate> {
        let mut s = self.clone();
        s.done.clear();
        let stop = s.tti + limit_tti;
        loop {
            if let Some(f) = s.done.iter().find(|d| d.0 == id) {
                return Some(f.1);
            }
            if s.tti > stop {
                return None;
            }
            if s.has_work_now() {
                s.step(c);
                continue;
            }
            match s.next_event() {
                Some(e) if e > s.tti => s.decay(e - s.tti),
                Some(_) => s.step(c),
                None => return None,
            }
        }
    }

    fn fails(salt: u64, tb: &Tb, p: f64) -> bool {
        let h = mix(salt ^ mix(tb.pkt ^ (tb.first << 20) ^ (u64::from(tb.attempt) << 56) ^ (u64::from(tb.rlc) << 60)));
        unit(h) < p
    }

    fn step(&mut self, c: &StepCfg) {
        let t = self.tti;
        while let Some(entry) = self.pending.first_entry() {
            if entry.key().0 > t {
                break;
            }
            let (ue, se, salt, pkt) = entry.remove();
            let u = self.ues.entry(ue).or_insert_with(|| UeQ {
                avg_bps: 0.0,
                se,
                salt,
                q: VecDeque::new(),
            });
            u.se = se;
            u.q.push_back(pkt);
        }
        let mut avail = c.prbs_free;
        let mut served: BTreeMap<u32, f64> = BTreeMap::new();
        // HARQ retransmissions first: they hold their PRBs.
        let mut due = Vec::new();
        self.harq.retain(|tb| {
            if tb.due <= t {
                due.push(tb.clone());
                false
            } else {
                true
            }
        });
        let mut resolved: Vec<(Tb, bool)> = Vec::new();
        for mut tb in due {
            if tb.prbs > avail {
                tb.due = t + 1;
                self.harq.push(tb);
                continue;
            }
            avail -= tb.prbs;
            let salt = self.ues.get(&tb.ue).map_or(0, |u| u.salt);
            let ok = !Self::fails(salt, &tb, c.bler_retx);
            self.harq_retx += 1;
            resolved.push((tb, ok));
        }
        // New transmissions, proportional fair: instantaneous rate over average rate.
        let mut order: Vec<(f64, u32)> = self
            .ues
            .iter()
            .filter(|(_, u)| u.q.iter().any(|p| p.left > 0 && p.ready <= t))
            .map(|(id, u)| {
                let r = u.se * c.bits_per_prb_se * f64::from(c.prbs_free) / c.tti_s;
                (r / u.avg_bps.max(1.0), *id)
            })
            .collect();
        order.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, ue) in order {
            if avail == 0 {
                break;
            }
            let Some(u) = self.ues.get_mut(&ue) else { continue };
            let bpp = (u.se * c.bits_per_prb_se).max(1.0);
            for p in u.q.iter_mut() {
                if avail == 0 {
                    break;
                }
                if p.ready > t || p.left == 0 {
                    continue;
                }
                let need = ((f64::from(p.left) * 8.0) / bpp).ceil().max(1.0) as u32;
                let take = need.min(avail);
                let bytes = p.left.min(((f64::from(take) * bpp) / 8.0).floor() as u32);
                if bytes == 0 {
                    break;
                }
                p.left -= bytes;
                p.inflight += bytes;
                avail -= take;
                *served.entry(ue).or_insert(0.0) += f64::from(bytes) * 8.0;
                let tb = Tb {
                    ue,
                    pkt: p.id,
                    bytes,
                    prbs: take,
                    attempt: 1,
                    first: t,
                    rlc: p.rlc_tx,
                    due: t + c.harq_rtt,
                };
                let ok = !Self::fails(u.salt, &tb, c.bler_first);
                resolved.push((tb, ok));
            }
        }
        for (tb, ok) in resolved {
            self.resolve(tb, ok, t, c);
        }
        let a = 1.0 / PF_WINDOW_TTI;
        for (id, u) in &mut self.ues {
            let s = served.get(id).copied().unwrap_or(0.0) / c.tti_s;
            u.avg_bps = (1.0 - a) * u.avg_bps + a * s;
        }
        self.tti += 1;
    }

    fn resolve(&mut self, mut tb: Tb, ok: bool, t: u64, c: &StepCfg) {
        if ok && let Some(start) = self.watch.remove(&tb.ue) {
            self.gaps.push(t + 1 - start.min(t + 1));
        }
        let Some(u) = self.ues.get_mut(&tb.ue) else { return };
        let Some(pos) = u.q.iter().position(|p| p.id == tb.pkt) else {
            return;
        };
        let mut lost = false;
        {
            let p = &mut u.q[pos];
            if ok {
                p.inflight -= tb.bytes;
                p.last_ok = p.last_ok.max(t);
            } else if tb.attempt < c.max_harq {
                tb.attempt += 1;
                tb.due = t + c.harq_rtt;
                self.harq.push(tb);
                return;
            } else {
                p.inflight -= tb.bytes;
                p.left += tb.bytes;
                p.rlc_tx += 1;
                p.ready = t + c.rlc_retx_tti;
                self.rlc_retx += 1;
                lost = p.rlc_tx > c.max_rlc;
            }
        }
        if lost {
            let id = u.q[pos].id;
            u.q.remove(pos);
            self.harq.retain(|h| h.pkt != id);
            self.done.push((id, Fate::Lost));
            return;
        }
        let p = &u.q[pos];
        if p.left == 0 && p.inflight == 0 {
            let (id, at) = (p.id, p.last_ok + 1);
            u.q.remove(pos);
            self.done.push((id, Fate::Delivered(at)));
        }
    }

    /// Takes everything this cell holds for one UE: queued and pending packets, with their
    /// unacknowledged bytes returned to the packet (PDCP forwards them to the target).
    fn extract(&mut self, ue: u32) -> Vec<Pkt> {
        let mut out = Vec::new();
        if let Some(u) = self.ues.remove(&ue) {
            for mut p in u.q {
                p.left += p.inflight;
                p.inflight = 0;
                out.push(p);
            }
        }
        self.harq.retain(|tb| tb.ue != ue);
        let keys: Vec<(u64, u64)> = self
            .pending
            .iter()
            .filter(|(_, v)| v.0 == ue)
            .map(|(k, _)| *k)
            .collect();
        for k in keys {
            if let Some((_, _, _, p)) = self.pending.remove(&k) {
                out.push(p);
            }
        }
        self.watch.remove(&ue);
        out
    }

    fn backlog_of(&self, ue: u32) -> bool {
        self.ues.get(&ue).is_some_and(|u| !u.q.is_empty())
            || self.pending.values().any(|v| v.0 == ue)
    }
}

// =========================================================================================
// The model
// =========================================================================================

#[derive(Debug, Clone)]
struct UeInfo {
    salt: u64,
    serving: Option<u32>,
    pos: Option<Vec3>,
    interrupted_until: SimTime,
}

/// Counters, all integers so a run report can compare them exactly.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PacketUuStats {
    /// Packets offered.
    pub sent: u64,
    /// Packets delivered.
    pub delivered: u64,
    /// Packets RLC AM gave up on.
    pub lost_rlc: u64,
    /// Packets that could not be delivered within the rollout limit.
    pub dropped_overload: u64,
    /// HARQ retransmissions performed.
    pub harq_retx: u64,
    /// RLC AM retransmissions performed.
    pub rlc_retx: u64,
    /// Handovers.
    pub handovers: u64,
    /// Measured handover gaps: count, sum and maximum, µs — from the start of the
    /// interruption to the UE's first successful block on the target cell.
    pub ho_gaps: u64,
    /// Sum of measured gaps, µs.
    pub ho_gap_sum_us: u64,
    /// Longest measured gap, µs.
    pub ho_gap_max_us: u64,
    /// Sum of delivered latencies, µs.
    pub latency_sum_us: u64,
    /// Longest delivered latency, µs.
    pub latency_max_us: u64,
    /// Delivered latencies in 1 ms bins, the last bin open-ended.
    pub latency_hist_ms: Vec<u64>,
    /// Sum of the simple model's mean latency for the same packets, µs.
    pub simple_sum_us: u64,
    /// Packets the simple model would have dropped (λ ≥ µ).
    pub simple_drops: u64,
}

/// Latency histogram bins, 1 ms each.
pub const HIST_BINS: usize = 1000;

/// Packet-level model against the simple one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SimpleModelComparison {
    /// Packets compared.
    pub packets: u64,
    /// Mean latency, packet model, ms.
    pub mean_packet_ms: f64,
    /// 95th-percentile latency, packet model, ms (1 ms resolution).
    pub p95_packet_ms: f64,
    /// Mean latency, simple model, ms.
    pub mean_simple_ms: f64,
    /// Loss fraction, packet model.
    pub loss_packet: f64,
    /// Loss fraction, simple model.
    pub loss_simple: f64,
    /// True when the two differ by more than 25 % and 5 ms on the mean, or 1 point of loss.
    pub differs: bool,
    /// A sentence saying how.
    pub note: String,
}

/// `cellular/uu/packet-pf`.
#[derive(Debug, Clone)]
pub struct PacketUu {
    card: ModelCard,
    cfg: RatConfig,
    layout: BaseStationLayout,
    mec: MecPlacement,
    background_load: f64,
    neighbour_load: f64,
    cells: BTreeMap<u32, [Sched; 2]>,
    ues: BTreeMap<u32, UeInfo>,
    next_id: u64,
    stats: PacketUuStats,
    shadow: CellCapacityUu,
    shadow_arrivals: BTreeMap<(u32, u8), VecDeque<SimTime>>,
}

impl PacketUu {
    /// The model's id.
    pub const ID: &'static str = "cellular/uu/packet-pf";

    /// The model on a layout, with half the PRBs taken by other subscribers' traffic.
    #[must_use]
    pub fn new(rat: Rat, layout: BaseStationLayout) -> Self {
        let shadow = CellCapacityUu::new(layout.plan().clone())
            .with_chain(RadioLatencyClass::LowAutomationLowLoad, MecPlacement::AtCoreNetwork);
        Self {
            card: packet_card(),
            cfg: RatConfig::of(rat),
            layout,
            mec: MecPlacement::AtCoreNetwork,
            background_load: DEFAULT_BACKGROUND_LOAD,
            neighbour_load: DEFAULT_BACKGROUND_LOAD,
            cells: BTreeMap::new(),
            ues: BTreeMap::new(),
            next_id: 0,
            stats: PacketUuStats {
                latency_hist_ms: vec![0; HIST_BINS],
                ..PacketUuStats::default()
            },
            shadow,
            shadow_arrivals: BTreeMap::new(),
        }
    }

    /// The model with caller-chosen RAT figures.
    #[must_use]
    pub fn with_config(mut self, cfg: RatConfig) -> Self {
        self.cfg = cfg;
        self
    }

    /// The fraction of each cell's PRBs other subscribers use, and the load the
    /// neighbouring cells interfere with.
    #[must_use]
    pub fn with_background_load(mut self, own: f64, neighbours: f64) -> Self {
        self.background_load = own.clamp(0.0, 0.99);
        self.neighbour_load = neighbours.clamp(0.0, 1.0);
        self
    }

    /// Where the core network's user plane terminates.
    #[must_use]
    pub fn with_core(mut self, mec: MecPlacement) -> Self {
        self.mec = mec;
        self.shadow = self.shadow.clone().with_chain(RadioLatencyClass::LowAutomationLowLoad, mec);
        self
    }

    /// The simple model's capacities, for the comparison.
    #[must_use]
    pub fn with_simple_capacity_bps(mut self, ul: f64, dl: f64) -> Self {
        self.shadow = self.shadow.clone().with_capacity_bps(ul, dl);
        self
    }

    /// The RAT figures in use.
    #[must_use]
    pub const fn config(&self) -> &RatConfig {
        &self.cfg
    }

    /// The layout.
    #[must_use]
    pub const fn layout(&self) -> &BaseStationLayout {
        &self.layout
    }

    /// The counters.
    #[must_use]
    pub fn stats(&self) -> PacketUuStats {
        let mut s = self.stats.clone();
        for c in self.cells.values() {
            for d in c {
                s.harq_retx += d.harq_retx;
                s.rlc_retx += d.rlc_retx;
            }
        }
        s
    }

    /// The one-way core-network latency, ms: the measured transport network to the
    /// placement plus the server's service time [Coll-Perales et al. 2022 via
    /// 04-models.md §10.1].
    #[must_use]
    pub fn core_ms(&self) -> f64 {
        self.mec.transport_network_ms().0 + self.mec.server_service_ms()
    }

    fn step_cfg(&self, dir: Direction) -> StepCfg {
        let free = (f64::from(self.cfg.prbs) * (1.0 - self.background_load)).floor() as u32;
        StepCfg {
            bits_per_prb_se: self.cfg.bits_per_prb_per_se(dir),
            prbs_free: free.max(1),
            harq_rtt: self.cfg.harq_rtt_tti,
            max_harq: self.cfg.max_harq_tx,
            bler_first: self.cfg.bler_first,
            bler_retx: self.cfg.bler_retx,
            rlc_retx_tti: self.cfg.ms_to_tti(self.cfg.rlc_retx_ms),
            max_rlc: self.cfg.max_rlc_retx,
            tti_s: self.cfg.tti_s(),
        }
    }

    fn ue_info<C: Ctx + ?Sized>(&mut self, ctx: &mut C, ue: NodeId) -> &mut UeInfo {
        self.ues.entry(ue.index()).or_insert_with(|| {
            let salt = (ctx.rng(RngDomain::Backend, EntityRef::Node(ue)).f64() * 9.007_199_254_740_992e15) as u64;
            UeInfo {
                salt: mix(salt ^ u64::from(ue.index())),
                serving: None,
                pos: None,
                interrupted_until: 0,
            }
        })
    }

    fn drain_gaps(&mut self, cell: u32) {
        let tti_us = self.cfg.tti_ns / 1000;
        if let Some(c) = self.cells.get_mut(&cell) {
            for d in c.iter_mut() {
                for g in d.gaps.drain(..) {
                    let us = g * tti_us;
                    self.stats.ho_gaps += 1;
                    self.stats.ho_gap_sum_us += us;
                    self.stats.ho_gap_max_us = self.stats.ho_gap_max_us.max(us);
                }
            }
        }
    }

    fn advance_cell(&mut self, cell: u32, now: SimTime) {
        let target = now / self.cfg.tti_ns;
        let cfgs = [self.step_cfg(Direction::Uplink), self.step_cfg(Direction::Downlink)];
        let c = self.cells.entry(cell).or_default();
        for (i, d) in c.iter_mut().enumerate() {
            if d.tti == 0 && d.ues.is_empty() && d.pending.is_empty() {
                d.tti = target;
            }
            d.advance(target, &cfgs[i]);
        }
        self.drain_gaps(cell);
    }

    /// Notices a UE's position: sets its serving cell and, when the cell changed, performs
    /// the handover — the UE's packets move to the target cell and wait out the
    /// interruption there. Returns the interruption when a handover happened.
    pub fn observe_position<C: Ctx + ?Sized>(&mut self, ctx: &mut C, ue: NodeId, p: Vec3) -> Option<Duration> {
        let view = self.layout.coverage(p);
        let info = self.ue_info(ctx, ue);
        info.pos = Some(p);
        let new = view.map(|v| v.cell.index());
        let old = info.serving;
        info.serving = new.or(old);
        match (old, new) {
            (Some(o), Some(n)) if o != n => {
                Some(CellularUu::<C>::handover(self, ctx, ue, CellId::new(o), CellId::new(n)))
            }
            _ => None,
        }
    }

    /// The delivery of `bytes` offered at `at` (no earlier than now) over the UE's link.
    pub fn send_at<C: Ctx + ?Sized>(
        &mut self,
        ctx: &mut C,
        ue: NodeId,
        dir: Direction,
        bytes: u32,
        at: SimTime,
    ) -> SendOutcome {
        let now = ctx.now();
        let at = at.max(now);
        self.stats.sent += 1;
        let (salt, serving, pos, until) = {
            let info = self.ue_info(ctx, ue);
            (info.salt, info.serving, info.pos, info.interrupted_until)
        };
        let (cell, pos) = match (serving, pos) {
            (Some(c), Some(p)) => (c, p),
            _ => {
                // No position seen: the UE is taken to sit at a quarter of the ISD from the
                // first site, which is the inner edge of `Fair` quality.
                let q = self.layout.plan().isd_m * 0.25;
                let p = Vec3::new(q, 0.0, 0.0);
                match self.layout.coverage(p) {
                    Some(v) => (v.cell.index(), p),
                    None => return SendOutcome::NoCoverage,
                }
            }
        };
        if self.layout.coverage(pos).is_none() {
            return SendOutcome::NoCoverage;
        }
        let sinr = self.layout.sinr_db(&self.cfg, pos, cell, dir, self.neighbour_load);
        let se = self.cfg.spectral_efficiency(sinr);
        self.advance_cell(cell, now);
        let di = dir as usize;
        let core_ns = (self.core_ms() * 1e6) as u64;
        let mut arrive = at;
        match dir {
            Direction::Downlink => {
                arrive += core_ns + (self.cfg.dl_processing_ms * 1e6) as u64;
            }
            Direction::Uplink => {
                let busy = self.cells.get(&cell).is_some_and(|c| c[di].backlog_of(ue.index()));
                if !busy {
                    // A UE with nothing buffered must ask for a grant: the wait for its next
                    // SR occasion, then the SR → grant → data chain.
                    let h = unit(mix(salt ^ mix(self.next_id ^ 0x5352)));
                    arrive += ((h * self.cfg.sr_period_ms + self.cfg.ul_grant_ms) * 1e6) as u64;
                }
            }
        }
        arrive = arrive.max(until);
        let id = self.next_id;
        self.next_id += 1;
        let tti = arrive.div_ceil(self.cfg.tti_ns);
        let pkt = Pkt {
            id,
            left: bytes.max(1),
            inflight: 0,
            rlc_tx: 0,
            ready: tti,
            last_ok: 0,
        };
        let cfg = self.step_cfg(dir);
        let limit = ROLLOUT_LIMIT.as_nanos() / self.cfg.tti_ns;
        let sched = &mut self.cells.entry(cell).or_default()[di];
        sched.pending.insert((tti, id), (ue.index(), se, salt, pkt));
        let fate = sched.rollout(id, &cfg, limit + (tti - sched.tti.min(tti)));
        let simple = self.simple_ms(ue, dir, at, bytes);
        match fate {
            Some(Fate::Delivered(t)) => {
                let mut deliver = t * self.cfg.tti_ns;
                if dir == Direction::Uplink {
                    deliver += core_ns;
                }
                let lat_us = (deliver - at) / 1000;
                self.stats.delivered += 1;
                self.stats.latency_sum_us += lat_us;
                self.stats.latency_max_us = self.stats.latency_max_us.max(lat_us);
                let bin = ((lat_us / 1000) as usize).min(HIST_BINS - 1);
                self.stats.latency_hist_ms[bin] += 1;
                match simple {
                    Some(ms) => self.stats.simple_sum_us += (ms * 1e3) as u64,
                    None => self.stats.simple_drops += 1,
                }
                SendOutcome::Scheduled {
                    deliver_at: deliver,
                    bytes_on_wire: bytes,
                }
            }
            Some(Fate::Lost) => {
                self.stats.lost_rlc += 1;
                if simple.is_none() {
                    self.stats.simple_drops += 1;
                }
                SendOutcome::Dropped(DropCause::QueueFull { ac: "rlc-am", depth: 0 })
            }
            None => {
                let s = &mut self.cells.entry(cell).or_default()[di];
                s.pending.remove(&(tti, id));
                self.stats.dropped_overload += 1;
                if simple.is_none() {
                    self.stats.simple_drops += 1;
                }
                SendOutcome::Dropped(DropCause::QueueFull {
                    ac: "cellular-pf",
                    depth: 0,
                })
            }
        }
    }

    /// The simple model's mean latency for the same arrival: the fixed chain plus the
    /// M/M/1 mean sojourn `1/(µ − λ)`, or `None` where it would drop.
    fn simple_ms(&mut self, ue: NodeId, dir: Direction, at: SimTime, _bytes: u32) -> Option<f64> {
        let w = self.shadow_arrivals.entry((ue.index(), dir as u8)).or_default();
        let from = at.saturating_sub(crate::cellular::ARRIVAL_WINDOW.as_nanos());
        while w.front().is_some_and(|t| *t < from) {
            w.pop_front();
        }
        w.push_back(at);
        let lambda = w.len() as f64 / crate::cellular::ARRIVAL_WINDOW.as_secs_f64();
        let mu = self.shadow.service_rate(dir);
        (lambda < mu).then(|| self.shadow.fixed_latency_ms() + 1e3 / (mu - lambda))
    }

    /// The packet model against the simple one, over every send so far.
    #[must_use]
    pub fn comparison(&self) -> SimpleModelComparison {
        let s = &self.stats;
        let n = s.delivered.max(1) as f64;
        let mean_packet_ms = s.latency_sum_us as f64 / n / 1e3;
        let simple_n = (s.delivered + s.lost_rlc + s.dropped_overload).saturating_sub(s.simple_drops).max(1) as f64;
        let mean_simple_ms = s.simple_sum_us as f64 / simple_n / 1e3;
        let total = s.sent.max(1) as f64;
        let loss_packet = (s.lost_rlc + s.dropped_overload) as f64 / total;
        let loss_simple = s.simple_drops as f64 / total;
        let target = (0.95 * s.delivered as f64).ceil() as u64;
        let mut acc = 0;
        let mut p95 = 0.0;
        for (i, c) in s.latency_hist_ms.iter().enumerate() {
            acc += c;
            if acc >= target && target > 0 {
                p95 = (i + 1) as f64;
                break;
            }
        }
        let diff = mean_packet_ms - mean_simple_ms;
        let lat_differs = diff.abs() > 5.0 && diff.abs() > 0.25 * mean_simple_ms.max(1e-9);
        let loss_differs = (loss_packet - loss_simple).abs() > 0.01;
        let differs = s.delivered > 0 && (lat_differs || loss_differs);
        let note = if s.sent == 0 {
            "no packets were sent".to_string()
        } else if !differs {
            format!(
                "the packet-level and simple models agree: mean {mean_packet_ms:.1} ms against {mean_simple_ms:.1} ms, loss {:.2} % against {:.2} %",
                loss_packet * 100.0,
                loss_simple * 100.0
            )
        } else {
            format!(
                "the packet-level model differs from cellular/uu/cell-capacity-mm1: mean latency {mean_packet_ms:.1} ms (p95 {p95:.0} ms) against {mean_simple_ms:.1} ms, loss {:.2} % against {:.2} %; {} HARQ and {} RLC retransmissions, {} handovers with a measured mean gap of {:.1} ms",
                loss_packet * 100.0,
                loss_simple * 100.0,
                self.stats().harq_retx,
                self.stats().rlc_retx,
                s.handovers,
                if s.ho_gaps > 0 { s.ho_gap_sum_us as f64 / s.ho_gaps as f64 / 1e3 } else { 0.0 }
            )
        };
        SimpleModelComparison {
            packets: s.sent,
            mean_packet_ms,
            p95_packet_ms: p95,
            mean_simple_ms,
            loss_packet,
            loss_simple,
            differs,
            note,
        }
    }
}

/// The share of PRBs other subscribers' traffic occupies by default: half, a mid-loaded
/// macro cell [`todo-calibrate` against operator PRB-utilisation data].
pub const DEFAULT_BACKGROUND_LOAD: f64 = 0.5;

impl Model for PacketUu {
    fn card(&self) -> &ModelCard {
        &self.card
    }
}

impl<C: Ctx + ?Sized> CellularUu<C> for PacketUu {
    fn tier(&self) -> Tier {
        Tier::High
    }

    fn coverage(&self, p: Vec3, _t: SimTime) -> Option<CellView> {
        self.layout.coverage(p)
    }

    fn send(&mut self, ctx: &mut C, ue: NodeId, dir: Direction, bytes: u32, _qos: Qos) -> SendOutcome {
        let now = ctx.now();
        self.send_at(ctx, ue, dir, bytes, now)
    }

    fn handover(&mut self, ctx: &mut C, ue: NodeId, from: CellId, to: CellId) -> Duration {
        let now = ctx.now();
        let (from, to) = (from.index(), to.index());
        self.advance_cell(from, now);
        self.advance_cell(to, now);
        let ms = ctx
            .rng(RngDomain::Backend, EntityRef::Node(ue))
            .normal(self.cfg.ho_interruption_ms, self.cfg.ho_interruption_sigma_ms)
            .max(0.0);
        let d = Duration::from_secs_f64(ms * 1e-3);
        let until = d.after(now);
        let (salt, se_pos) = {
            let info = self.ue_info(ctx, ue);
            info.serving = Some(to);
            info.interrupted_until = until;
            (info.salt, info.pos)
        };
        let ready = until.div_ceil(self.cfg.tti_ns);
        let start = now / self.cfg.tti_ns;
        let ses: [f64; 2] = [Direction::Uplink, Direction::Downlink].map(|dir| {
            se_pos.map_or(self.cfg.se_min, |p| {
                self.cfg
                    .spectral_efficiency(self.layout.sinr_db(&self.cfg, p, to, dir, self.neighbour_load))
            })
        });
        let moved: [Vec<Pkt>; 2] = {
            let c = self.cells.entry(from).or_default();
            [c[0].extract(ue.index()), c[1].extract(ue.index())]
        };
        let target = self.cells.entry(to).or_default();
        for (i, pkts) in moved.into_iter().enumerate() {
            for mut p in pkts {
                p.ready = p.ready.max(ready);
                target[i].pending.insert((p.ready, p.id), (ue.index(), ses[i], salt, p));
            }
            target[i].watch.insert(ue.index(), start);
        }
        self.stats.handovers += 1;
        d
    }
}

// =========================================================================================
// Card
// =========================================================================================

fn todo(reference: &str) -> Source {
    Source {
        kind: SourceKind::TodoCalibrate,
        reference: reference.to_string(),
        accessed: None,
        note: None,
    }
}

fn todo_param(name: &str, unit: &str, v: serde_json::Value, reference: &str, plan: &str) -> Parameter {
    let mut p = Parameter::new(name, unit, v, todo(reference));
    p.calibration = Some(plan.to_string());
    p
}

fn packet_card() -> ModelCard {
    let lte = RatConfig::lte_20mhz();
    let mut card = ModelCard::new(
        PacketUu::ID,
        Family::Cellular,
        "1.0.0",
        "Packet-level LTE/NR Uu: base stations per city, a proportional-fair scheduler per \
         cell and direction, HARQ, RLC acknowledged mode, measured handover interruption \
         with lossless forwarding, and the core-network leg.",
    );
    card.tier = vec![Tier::High];
    card.equations = vec![
        Equation::new(
            "proportional fair",
            "serve UEs in order of r_i(t) / R_i(t); R_i ← (1 − 1/T)·R_i + (1/T)·served_i, T = 100 TTI",
        ),
        Equation::new(
            "spectral efficiency",
            "SE = clamp(0.75·log2(1 + SINR/1.25), CQI1, CQI_max); bits per PRB = SE·168·(1 − OH)",
        ),
        Equation::new(
            "HARQ and RLC AM",
            "first transmission fails with BLER 0.1, a retransmission with 0.05, every HARQ RTT, \
             up to max_harq_tx; then RLC AM retransmits after rlc_retx_ms, up to max_rlc_retx; \
             then the SDU is lost",
        ),
        Equation::new(
            "one-way latency",
            "UL: SR wait U[0, T_SR) + grant chain + scheduling + HARQ + core; DL: core + BS \
             processing + scheduling + HARQ",
        ),
    ];
    card.parameters = vec![
        Parameter::new(
            "lte_prbs",
            "PRB",
            serde_json::json!(lte.prbs),
            Source::new(SourceKind::Standard, "TS 36.101 Table 5.6-1: 100 PRB at 20 MHz"),
        ),
        Parameter::new(
            "nr_prbs",
            "PRB",
            serde_json::json!(RatConfig::nr_100mhz_30khz().prbs),
            Source::new(SourceKind::Standard, "TS 38.101-1 Table 5.3.2-1: 273 PRB at 100 MHz, 30 kHz SCS"),
        ),
        Parameter::new(
            "overhead_dl",
            "-",
            serde_json::json!(lte.overhead_dl),
            Source::new(SourceKind::Standard, "TS 38.306 §4.1.2: OH 0.14 DL, 0.08 UL in FR1"),
        ),
        Parameter::new(
            "bler_first",
            "-",
            serde_json::json!(lte.bler_first),
            Source::new(SourceKind::Standard, "TS 36.213 §7.2.3: CQI chosen for a transport-block error probability of 0.1"),
        ),
        Parameter::new(
            "lte_harq_rtt_ms",
            "ms",
            serde_json::json!(8.0),
            Source::new(SourceKind::Standard, "TS 36.213 §8, §10.1: FDD HARQ timing n+4 / n+8"),
        ),
        Parameter::new(
            "max_harq_tx",
            "-",
            serde_json::json!(lte.max_harq_tx),
            Source::new(SourceKind::Code, "ns-3 LTE module, LteEnbMac MaxHarqRetransmissions = 3"),
        ),
        todo_param(
            "bler_retx",
            "-",
            serde_json::json!(lte.bler_retx),
            "chosen: chase combining makes a retransmission more reliable than the first",
            "Fit to an ns-3 LTE or 5G-LENA HARQ trace of residual BLER per attempt.",
        ),
        todo_param(
            "rlc_retx_ms",
            "ms",
            serde_json::json!(lte.rlc_retx_ms),
            "chosen from the TS 36.331 t-StatusProhibit / t-PollRetransmit option ranges",
            "Take the operator default from a measured RRC configuration log.",
        ),
        todo_param(
            "lte_ul_access_ms",
            "ms",
            serde_json::json!([lte.sr_period_ms, lte.ul_grant_ms]),
            "TR 36.881 §5.1 user-plane latency analysis: SR period 10 ms, SR to data 8 ms",
            "Read TR 36.881 Table 5.1-1 and record each component.",
        ),
        todo_param(
            "ho_interruption_ms",
            "ms",
            serde_json::json!([lte.ho_interruption_ms, RatConfig::nr_100mhz_30khz().ho_interruption_ms]),
            "TR 36.881 §5.2 LTE handover interruption analysis (49.5 ms); NR between it and the \
             0 ms DAPS target of TS 38.300 §9.2.3.2",
            "Fit to a drive-test handover trace (e.g. MobileInsight) per RAT.",
        ),
        todo_param(
            "shannon_fit",
            "-",
            serde_json::json!([SHANNON_ALPHA, SHANNON_SNR_EFF]),
            "Mogensen et al., VTC 2007-Spring: attenuated Shannon bound for LTE",
            "Read the paper's fitted BW_eff and SNR_eff for SISO and replace both.",
        ),
        todo_param(
            "background_load",
            "-",
            serde_json::json!(DEFAULT_BACKGROUND_LOAD),
            "chosen: a mid-loaded macro cell",
            "Use operator PRB-utilisation statistics for the city being modelled.",
        ),
    ];
    card.assumptions = vec![
        "A delivery instant is the scheduler's answer with the backlog known at send time; a \
         later arrival shares the following TTIs but does not move a delivery already reported."
            .to_string(),
        "Wideband scheduling: a UE's channel is flat across PRBs and constant between sends."
            .to_string(),
        "RLC AM bearers are lossless across a handover (TS 36.300 §10.1.2.3): packets wait out \
         the interruption and are forwarded to the target cell."
            .to_string(),
    ];
    card.ignores = vec![
        "Fast fading, frequency-selective scheduling, MIMO layers, beam management.".to_string(),
        "Radio-link failure and re-establishment.".to_string(),
    ];
    card.sources = vec![
        Source::new(SourceKind::Standard, "TS 36.213, TS 36.101, TS 38.101-1, TS 38.306, TS 36.300"),
        Source::new(SourceKind::Paper, "Mogensen et al., VTC 2007-Spring"),
        Source::new(SourceKind::Paper, "Jalali, Padovani, Pankaj, VTC 2000 (proportional fair)"),
    ];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![Source::new(
            SourceKind::Paper,
            "Narayanan et al., WWW'20 Table 2: LTE first-hop RTT 29.2 ± 4.8 ms",
        )],
        tests: vec![
            "an_unloaded_lte_round_trip_lands_near_the_measured_first_hop".to_string(),
            "proportional_fair_shares_a_cell_and_load_slows_it".to_string(),
            "a_handover_holds_packets_and_measures_the_gap".to_string(),
        ],
    };
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![RngDomain::Backend.as_str().to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testctx::TestCtx;

    fn model(rat: Rat) -> PacketUu {
        PacketUu::new(rat, BaseStationLayout::lattice(CellPlan::urban_macro()))
    }

    #[test]
    fn the_card_validates() {
        model(Rat::Lte).card().validate().unwrap();
    }

    #[test]
    fn an_unloaded_lte_round_trip_lands_near_the_measured_first_hop() {
        // Narayanan et al. WWW'20: LTE first-hop RTT 29.2 ± 4.8 ms. A request up and a
        // reply down, one at a time, one UE, no other traffic.
        let mut uu = model(Rat::Lte).with_background_load(0.0, 0.5);
        let mut ctx = TestCtx::new(7);
        let ue = NodeId::new(1);
        uu.observe_position(&mut ctx, ue, Vec3::new(120.0, 40.0, 0.0));
        let mut total = 0.0;
        let n = 200;
        for i in 0..n {
            ctx.set_now(i * 200_000_000);
            let up = uu.send(&mut ctx, ue, Direction::Uplink, 100, Qos::BestEffort);
            let t1 = up.deliver_at().unwrap();
            ctx.set_now(t1);
            let dn = uu.send(&mut ctx, ue, Direction::Downlink, 100, Qos::BestEffort);
            total += (dn.deliver_at().unwrap() - i * 200_000_000) as f64 * 1e-6;
        }
        let rtt = total / n as f64;
        assert!((15.0..40.0).contains(&rtt), "LTE RTT {rtt:.2} ms");
        // NR is faster on the radio leg.
        let mut nr = model(Rat::Nr).with_background_load(0.0, 0.5);
        let mut ctx = TestCtx::new(7);
        nr.observe_position(&mut ctx, ue, Vec3::new(120.0, 40.0, 0.0));
        let up = nr.send(&mut ctx, ue, Direction::Uplink, 100, Qos::BestEffort);
        assert!(up.deliver_at().unwrap() < 10_000_000);
    }

    #[test]
    fn proportional_fair_shares_a_cell_and_load_slows_it() {
        let big = 2_000_000; // 2 MB each
        let run = |ues: u32, bg: f64| -> f64 {
            let mut uu = model(Rat::Lte).with_background_load(bg, bg);
            let mut ctx = TestCtx::new(3);
            let mut worst = 0u64;
            for k in 0..ues {
                let ue = NodeId::new(k + 1);
                uu.observe_position(&mut ctx, ue, Vec3::new(60.0 + 20.0 * f64::from(k), 30.0, 0.0));
            }
            for k in 0..ues {
                let out = uu.send(&mut ctx, NodeId::new(k + 1), Direction::Downlink, big, Qos::BestEffort);
                worst = worst.max(out.deliver_at().expect("delivered"));
            }
            worst as f64 * 1e-9
        };
        let one = run(1, 0.0);
        let four = run(4, 0.0);
        let loaded = run(1, 0.75);
        // 2 MB at near-peak efficiency over 100 PRB takes on the order of 0.2 s.
        assert!(one > 0.1 && one < 1.0, "one UE {one:.3} s");
        assert!(four > 2.5 * one, "four UEs share: {four:.3} s against {one:.3} s");
        assert!(loaded > 2.5 * one, "background load: {loaded:.3} s against {one:.3} s");
    }

    #[test]
    fn a_handover_holds_packets_and_measures_the_gap() {
        let mut uu = model(Rat::Lte);
        let mut ctx = TestCtx::new(11);
        let ue = NodeId::new(5);
        uu.observe_position(&mut ctx, ue, Vec3::new(200.0, 0.0, 0.0));
        ctx.set_now(1_000_000_000);
        let d = uu
            .observe_position(&mut ctx, ue, Vec3::new(300.0, 0.0, 0.0))
            .expect("crossing the midpoint hands over");
        assert!(d.as_secs_f64() > 0.0);
        let out = uu.send(&mut ctx, ue, Direction::Downlink, 500, Qos::BestEffort);
        // Held, not lost: it arrives after the interruption.
        assert!(out.deliver_at().unwrap() >= d.after(ctx.now()));
        ctx.set_now(3_000_000_000);
        let _ = uu.send(&mut ctx, ue, Direction::Downlink, 100, Qos::BestEffort);
        let s = uu.stats();
        assert_eq!(s.handovers, 1);
        assert_eq!(s.ho_gaps, 1, "the gap is measured once the target delivers");
        assert!(s.ho_gap_sum_us as f64 >= d.as_secs_f64() * 1e6 * 0.9);
    }

    #[test]
    fn the_same_schedule_gives_the_same_answers() {
        let go = || {
            let mut uu = model(Rat::Nr);
            let mut ctx = TestCtx::new(5);
            let mut v = Vec::new();
            for i in 0..50u32 {
                let ue = NodeId::new(i % 7);
                ctx.set_now(u64::from(i) * 3_000_000);
                uu.observe_position(&mut ctx, ue, Vec3::new(f64::from(i) * 30.0, 10.0, 0.0));
                v.push(uu.send(&mut ctx, ue, Direction::Uplink, 3000, Qos::BestEffort).deliver_at());
            }
            (v, uu.stats())
        };
        assert_eq!(go(), go());
    }

    #[test]
    fn a_city_layout_covers_its_box_and_the_comparison_reports() {
        let l = BaseStationLayout::for_city(CellPlan::urban_macro(), (0.0, 0.0), (3000.0, 2000.0));
        assert!(l.site_count().unwrap() > 30);
        assert!(l.coverage(Vec3::new(1500.0, 1000.0, 0.0)).is_some());
        let mut uu = PacketUu::new(Rat::Lte, l);
        let mut ctx = TestCtx::new(2);
        for i in 0..40u64 {
            ctx.set_now(i * 50_000_000);
            let ue = NodeId::new((i % 4) as u32);
            uu.observe_position(&mut ctx, ue, Vec3::new(400.0 + i as f64 * 10.0, 900.0, 0.0));
            let _ = uu.send(&mut ctx, ue, Direction::Downlink, 1200, Qos::BestEffort);
        }
        let c = uu.comparison();
        assert_eq!(c.packets, 40);
        assert!(c.mean_packet_ms > 0.0 && c.mean_simple_ms > 0.0);
        assert!(!c.note.is_empty());
    }
}
