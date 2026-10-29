//! `transport/80211p-sch`: the service-channel hop between a vehicle and the roadside unit
//! whose WSA it joined, as a [`PacketPath`] under the TCP/TLS model of
//! [`crate::ip_transport`] (WP-19 P19.3).
//!
//! IEEE 1609.3 carries IP on a service channel the WSA names (its ChannelInfo and WAVE
//! Routing Advertisement), and IEEE 1609.2.1 §6.3 runs the SCMS REST interfaces over HTTPS,
//! so a certificate download, a CRL fetch or a report upload through an RSU is TCP segments
//! on that channel. Each IP packet offered to this path is judged at offer time:
//!
//! 1. **Contact window.** The vehicle's distance to the unit is extrapolated at constant
//!    velocity from the session start. A packet whose frame would end past
//!    [`RsuSchPath::range_m`] is lost: the vehicle has left coverage, which is how a
//!    top-off over an RSU fails in the field (NYC CV pilot, FHWA-JPO-20-804 p. 4: "OBUs
//!    sometimes left coverage before a top-off finished").
//! 2. **Channel access.** EDCA on AC_BE: `AIFS + CWmin/2 · aSlotTime` (IEEE 802.11-2020
//!    Table 9-155, OCB: AIFSN 6, CWmin 15, so 110 µs + 97.5 µs), plus the wait for a busy
//!    channel. With the channel busy a fraction `ρ` of the time (the WP-13 meter, read at
//!    session start), the mean wait is `ρ / (1 − ρ) · T_air` (**ASSUMPTION**: an M/D/1
//!    mean wait with the other stations' frames as long as this one; the SCH's frame mix
//!    is not measured).
//! 3. **Alternating access.** A one-radio unit in 1609.4 alternating mode transmits on the
//!    SCH only inside an SCH interval and never across its end
//!    ([`ChannelSchedule::fits`], WP-16); a frame that does not fit waits for the next SCH
//!    interval and runs AIFS again.
//! 4. **One medium.** The session's frames, both directions, are sent one after another
//!    (802.11 is half-duplex on one channel): a TCP window of segments takes its air time,
//!    not one segment's.
//! 5. **Loss.** The packet error rate at the extrapolated distance, from the link budget
//!    at the SCH power and MCS ([`RsuSchPath::per_curve`], a reference-size curve the
//!    engine computes, scaled to the frame's length as `1 − (1 − p)^(L / L_ref)`), drawn
//!    with a keyed stream (`RngDomain::plugin(ID)`, one word per offered packet), so a
//!    run is byte-identical for a seed.
//!
//! The limit with an infinite range, zero PER, no EDCA wait, an idle channel and
//! continuous access is a [`crate::ip_transport::ConstantPath`] at the MCS's rate plus the
//! PHY's preamble and symbol rounding; `the_limits_are_the_constant_path` holds that.

use v2xw_core::ids::NodeId;
use v2xw_core::rng::{EntityRef, RngDomain, RngStream};
use v2xw_core::time::{Duration, SimTime};

use crate::cellular::Direction;
use crate::ip_transport::PacketPath;
use crate::mac::EdcaParams;
use crate::types::{AccessCategory, Mcs, timing};
use crate::wave_schedule::{ChannelAccess, ChannelSchedule};

/// The model id.
pub const ID: &str = "transport/80211p-sch";

/// Octets an IP packet gains on the SCH: the 802.11 data header (24), LLC/SNAP with the
/// IPv6 EtherType (8; RFC 8691 §4.2, IPv6 over 802.11-OCB) and the FCS (4).
pub const SCH_FRAMING_BYTES: u32 = 36;

/// The highest busy ratio the wait formula is evaluated at; above it the M/D/1 mean
/// diverges, and a channel that busy is saturated whatever the exact figure.
const MAX_BUSY: f64 = 0.95;

/// The service-channel hop of one vehicle's session with one roadside unit.
#[derive(Debug, Clone, PartialEq)]
pub struct RsuSchPath {
    /// The session start, when the positions below were taken.
    pub start: SimTime,
    /// The vehicle's position at `start`, local ENU metres (x, y).
    pub vehicle_m: [f64; 2],
    /// Its velocity at `start`, m/s (x, y), held constant over the session.
    pub velocity_mps: [f64; 2],
    /// The unit's antenna, local ENU metres (x, y).
    pub rsu_m: [f64; 2],
    /// The SCH range, metres: a frame that ends farther away is lost.
    pub range_m: f64,
    /// PER of a [`RsuSchPath::per_ref_bytes`] frame against distance, `(m, per)` with
    /// ascending distances, linear between points and flat beyond the ends. Empty: no
    /// loss inside the range.
    pub per_curve: Vec<(f64, f64)>,
    /// The frame length the curve is for, octets.
    pub per_ref_bytes: u32,
    /// The SCH data rate.
    pub mcs: Mcs,
    /// The SCH busy ratio at session start, `0..1`.
    pub cbr: f64,
    /// The EDCA set the frames contend with (AC_BE by default); `None` contends for
    /// nothing, which is only the [`crate::ip_transport::ConstantPath`] limit.
    pub edca: Option<EdcaParams>,
    /// The sending unit's 1609.4 channel access: alternating gates the SCH to its
    /// interval.
    pub schedule: ChannelSchedule,
    /// Octets added to each IP packet on the air ([`SCH_FRAMING_BYTES`]).
    pub framing_bytes: u32,
    /// The run's master seed.
    pub seed: u64,
    /// Packets offered so far: the loss draw's word position.
    pub offered: u64,
    /// When the session's last frame left the air. The channel is one half-duplex
    /// medium, so the session's frames, both directions, go one after another.
    pub free_at: SimTime,
    /// Packets lost to leaving coverage.
    pub lost_contact: u64,
    /// Packets lost to the PER draw.
    pub lost_per: u64,
}

impl RsuSchPath {
    /// A session at `start` on the defaults: AC_BE, 6 Mb/s, continuous access, IPv6 over
    /// OCB framing, no loss curve.
    #[must_use]
    pub fn new(
        start: SimTime,
        vehicle_m: [f64; 2],
        velocity_mps: [f64; 2],
        rsu_m: [f64; 2],
        range_m: f64,
        seed: u64,
    ) -> Self {
        Self {
            start,
            vehicle_m,
            velocity_mps,
            rsu_m,
            range_m,
            per_curve: Vec::new(),
            per_ref_bytes: 1_500,
            mcs: Mcs::R6Qpsk12,
            cbr: 0.0,
            edca: Some(EdcaParams::standard(AccessCategory::Be)),
            schedule: ChannelSchedule {
                access: ChannelAccess::Continuous,
            },
            framing_bytes: SCH_FRAMING_BYTES,
            seed,
            offered: 0,
            free_at: 0,
            lost_contact: 0,
            lost_per: 0,
        }
    }

    /// The vehicle's distance to the unit at `t`, by constant velocity from `start`.
    #[must_use]
    pub fn distance_at(&self, t: SimTime) -> f64 {
        let dt = t.saturating_sub(self.start) as f64 / 1e9;
        let x = self.vehicle_m[0] + self.velocity_mps[0] * dt - self.rsu_m[0];
        let y = self.vehicle_m[1] + self.velocity_mps[1] * dt - self.rsu_m[1];
        v2xw_core::math::hypot(x, y)
    }

    /// The PER of a frame of `bytes` octets at `d_m` metres.
    #[must_use]
    pub fn per(&self, d_m: f64, bytes: u32) -> f64 {
        let p_ref = match self.per_curve.as_slice() {
            [] => 0.0,
            [only] => only.1,
            curve => {
                let first = curve[0];
                let last = curve[curve.len() - 1];
                if d_m <= first.0 {
                    first.1
                } else if d_m >= last.0 {
                    last.1
                } else {
                    let i = curve.partition_point(|(d, _)| *d <= d_m);
                    let (d0, p0) = curve[i - 1];
                    let (d1, p1) = curve[i];
                    p0 + (p1 - p0) * (d_m - d0) / (d1 - d0).max(1e-9)
                }
            }
        }
        .clamp(0.0, 1.0);
        if p_ref <= 0.0 || p_ref >= 1.0 {
            return p_ref;
        }
        let ratio = f64::from(bytes) / f64::from(self.per_ref_bytes.max(1));
        1.0 - (1.0 - p_ref).powf(ratio)
    }

    /// The channel-access delay before a frame of `air` starts, on an idle or a busy SCH.
    #[must_use]
    pub fn access_delay(&self, air: Duration) -> Duration {
        let Some(edca) = self.edca else {
            return Duration::ZERO;
        };
        let backoff_ns = timing::SLOT_TIME.as_nanos() * u64::from(edca.cw_min) / 2;
        let rho = self.cbr.clamp(0.0, MAX_BUSY);
        let busy_ns = (rho / (1.0 - rho) * air.as_nanos() as f64).round() as u64;
        Duration::from_nanos(edca.aifs().as_nanos() + backoff_ns + busy_ns)
    }

    fn aifs(&self) -> Duration {
        self.edca.map_or(Duration::ZERO, |e| e.aifs())
    }

    fn draw(&mut self, ue: NodeId) -> f64 {
        let mut s = RngStream::derive(self.seed, RngDomain::plugin(ID), EntityRef::Node(ue));
        s.set_word_pos(u128::from(self.offered) * 2);
        self.offered += 1;
        s.f64()
    }
}

impl<C: ?Sized> PacketPath<C> for RsuSchPath {
    fn deliver(
        &mut self,
        _ctx: &mut C,
        ue: NodeId,
        _dir: Direction,
        at: SimTime,
        bytes: u32,
    ) -> Option<SimTime> {
        let frame = bytes.saturating_add(self.framing_bytes);
        let air = crate::phy::air_time(frame, self.mcs);
        let u = self.draw(ue);
        let mut t = self.access_delay(air).after(at.max(self.free_at));
        // 1609.4: on the SCH only inside its interval and never across the end (WP-16).
        let mut guard = 0;
        while let Err(next) = self.schedule.fits(t, air.as_nanos(), false) {
            t = self.aifs().after(next);
            guard += 1;
            if guard > 4 {
                break;
            }
        }
        let end = air.after(t);
        self.free_at = end;
        let d = self.distance_at(end);
        if d > self.range_m {
            self.lost_contact += 1;
            return None;
        }
        if u < self.per(d, frame) {
            self.lost_per += 1;
            return None;
        }
        Some(crate::phy::propagation_delay(d).after(end))
    }
}

/// The model card of `transport/80211p-sch`.
#[must_use]
pub fn card() -> v2xw_core::card::ModelCard {
    use v2xw_core::card::{
        Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
        ValidationStatus,
    };
    let mut card = ModelCard::new(
        ID,
        Family::Backhaul,
        "1.0.0",
        "The IP hop on the service channel a roadside unit's WSA advertises: contact \
         window, EDCA access on a busy channel, 1609.4 intervals and link-budget loss.",
    );
    card.tier = vec![Tier::High];
    card.equations = vec![
        Equation::new(
            "access",
            "t0 = at + AIFS(AC_BE) + CWmin/2 · slot + ρ/(1−ρ) · T_air; held to the SCH interval",
        ),
        Equation::new(
            "loss",
            "lost if d(t0 + T_air) > range, else with p = 1 − (1 − PER_ref(d))^(L/L_ref)",
        ),
    ];
    card.parameters = vec![
        Parameter::new(
            "framing_bytes",
            "B",
            serde_json::json!(SCH_FRAMING_BYTES),
            Source::new(
                SourceKind::Standard,
                "802.11 data header 24 + LLC/SNAP 8 (RFC 8691 §4.2) + FCS 4",
            ),
        ),
        Parameter::new(
            "access_category",
            "-",
            serde_json::json!("AC_BE"),
            Source::new(
                SourceKind::Standard,
                "IEEE 802.11-2020 Table 9-155 (OCB): AIFSN 6, CWmin 15",
            ),
        ),
    ];
    card.assumptions = vec![
        "The vehicle's track over a session is a straight line at its start velocity.".to_string(),
        "The busy-channel wait is an M/D/1 mean with the other frames as long as this one."
            .to_string(),
    ];
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![],
        tests: vec![
            "the_limits_are_the_constant_path".to_string(),
            "leaving_coverage_loses_the_packet".to_string(),
        ],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ip_transport::ConstantPath;

    const UE: NodeId = NodeId::new(7);

    fn send(p: &mut RsuSchPath, at: SimTime, bytes: u32) -> Option<SimTime> {
        PacketPath::<()>::deliver(p, &mut (), UE, Direction::Uplink, at, bytes)
    }

    fn still(range_m: f64) -> RsuSchPath {
        RsuSchPath::new(0, [0.0, 0.0], [0.0, 0.0], [0.0, 0.0], range_m, 11)
    }

    /// With no loss, no contention, an idle channel, continuous access and the vehicle
    /// under the mast, the path is a constant link: the PHY's air time of the framed
    /// packet, which is `ConstantPath` at the MCS's rate plus preamble, SIGNAL and the
    /// symbol rounding.
    #[test]
    fn the_limits_are_the_constant_path() {
        let mut p = still(f64::INFINITY);
        p.edca = None;
        p.framing_bytes = 0;
        for (at, bytes) in [(0, 1_500), (5_000_000, 40), (1_000_000_000, 576)] {
            let air = crate::phy::air_time(bytes, Mcs::R6Qpsk12);
            let mut c = ConstantPath {
                one_way: Duration::ZERO,
                bps: 6e6,
            };
            let constant =
                PacketPath::<()>::deliver(&mut c, &mut (), UE, Direction::Uplink, at, bytes)
                    .expect("constant");
            let got = send(&mut p, at, bytes).expect("delivered");
            assert_eq!(got, air.after(at));
            // The PHY adds 40 µs of preamble and SIGNAL and at most one symbol (8 µs)
            // of rounding, besides 22 SERVICE and tail bits, to the line rate's time.
            let extra = got - constant;
            assert!((40_000..=56_000).contains(&extra), "{bytes} B: {extra} ns");
        }
        assert_eq!((p.lost_contact, p.lost_per), (0, 0));
    }

    /// A vehicle at 20 m/s leaving a 300 m cell from the mast: packets go through until it
    /// is 300 m out (15 s) and are lost after.
    #[test]
    fn leaving_coverage_loses_the_packet() {
        let mut p = RsuSchPath::new(0, [0.0, 0.0], [20.0, 0.0], [0.0, 0.0], 300.0, 11);
        assert!(send(&mut p, 14_000_000_000, 1_500).is_some());
        assert!(send(&mut p, 15_100_000_000, 1_500).is_none());
        assert_eq!(p.lost_contact, 1);
    }

    /// The busier the SCH, the later the frame, and an idle channel pays only AIFS and
    /// the mean backoff (110 + 97.5 µs on AC_BE).
    #[test]
    fn a_busy_channel_delays_the_frame() {
        let air = crate::phy::air_time(1_536, Mcs::R6Qpsk12);
        let mut last = 0;
        for (i, cbr) in [0.0, 0.1, 0.3, 0.5, 0.9].into_iter().enumerate() {
            let mut p = still(300.0);
            p.cbr = cbr;
            let t = send(&mut p, 0, 1_500).expect("delivered");
            if i == 0 {
                assert_eq!(t - air.as_nanos(), 110_000 + 97_500);
            }
            assert!(t > last, "cbr {cbr}: {t} after {last}");
            last = t;
        }
    }

    /// One-radio alternating access: a frame offered in the CCH interval waits for the SCH
    /// interval (50-100 ms, after its 4 ms guard) and runs AIFS there.
    #[test]
    fn alternating_access_waits_for_the_sch_interval() {
        let mut p = still(300.0);
        p.schedule = ChannelSchedule {
            access: ChannelAccess::Alternating,
        };
        let t = send(&mut p, 10_000_000, 200).expect("delivered");
        assert!(t > 54_000_000 && t < 60_000_000, "{t}");
        let mut c = still(300.0);
        assert!(send(&mut c, 10_000_000, 200).expect("delivered") < 11_000_000);
    }

    /// Loss follows the curve, the frame's length scales it, and the draws repeat for a
    /// seed.
    #[test]
    fn loss_follows_distance_and_repeats_for_a_seed() {
        let curve = vec![(0.0, 0.0), (100.0, 0.05), (300.0, 0.8)];
        let losses = |d: f64, seed: u64| {
            let mut p = RsuSchPath::new(0, [d, 0.0], [0.0, 0.0], [0.0, 0.0], 400.0, seed);
            p.per_curve = curve.clone();
            (0..2_000)
                .filter(|k| send(&mut p, k * 1_000_000, 1_500).is_none())
                .count()
        };
        let near = losses(50.0, 3);
        let far = losses(250.0, 3);
        assert!(near < far, "{near} near, {far} far");
        assert_eq!(losses(250.0, 3), far, "a seed repeats");
        let p = RsuSchPath {
            per_curve: curve,
            ..still(400.0)
        };
        assert!((p.per(100.0, 1_500) - 0.05).abs() < 1e-12);
        assert!(
            p.per(100.0, 3_000) > 0.05,
            "a longer frame is lost more often"
        );
        assert!((p.per(200.0, 1_500) - 0.425).abs() < 1e-12);
        assert_eq!(p.per(500.0, 1_500), 0.8);
    }

    /// The share of HTTPS downloads of `body` octets that finish, out of 40 vehicles
    /// passing the unit 10 m off its mast at `speed` m/s, each starting at an even step
    /// along the 600 m it is in range, over TCP/TLS 1.3 (`ip_transport::https_exchange`).
    fn completed(speed: f64, cbr: f64, body: u32) -> usize {
        use crate::ip_transport::{TcpConfig, TcpConn, https_exchange};
        let cfg = TcpConfig::default();
        (0..40)
            .filter(|k| {
                let x0 = -295.0 + f64::from(*k) * 15.0;
                let mut p = RsuSchPath::new(0, [x0, 10.0], [speed, 0.0], [0.0, 0.0], 300.0, 5);
                p.cbr = cbr;
                let mut conn = TcpConn::new(&cfg);
                https_exchange(&mut (), &mut p, UE, &mut conn, &cfg, 0, 300, body)
                    .done_at
                    .is_some()
            })
            .count()
    }

    /// WP-19 accept: download success rises with contact time. At 5, 10, 20 and 30 m/s a
    /// 4 MB download completes for fewer and fewer of the passes, because the faster
    /// vehicle leaves the 300 m cell before the transfer ends (FHWA-JPO-20-804 p. 4).
    #[test]
    fn download_success_falls_with_speed() {
        let shares: Vec<usize> = [5.0, 10.0, 20.0, 30.0]
            .into_iter()
            .map(|v| completed(v, 0.0, 4_000_000))
            .collect();
        println!("SCH speed shares {shares:?} of 40");
        assert!(shares.windows(2).all(|w| w[0] >= w[1]), "{shares:?}");
        assert!(shares[3] + 4 <= shares[0], "{shares:?}");
    }

    /// WP-19 accept: download success falls with SCH load. At busy ratios 0.1, 0.3 and 0.5
    /// each frame waits longer for the channel, the transfer takes longer, and fewer passes
    /// at 20 m/s finish in range.
    #[test]
    fn download_success_falls_with_sch_load() {
        let shares: Vec<usize> = [0.1, 0.3, 0.5]
            .into_iter()
            .map(|cbr| completed(20.0, cbr, 4_000_000))
            .collect();
        println!("SCH load shares {shares:?} of 40");
        assert!(shares.windows(2).all(|w| w[0] >= w[1]), "{shares:?}");
        assert!(shares[2] + 3 <= shares[0], "{shares:?}");
    }

    #[test]
    fn the_card_validates() {
        let c = card();
        assert_eq!(c.id, ID);
        assert!(c.validate().is_ok(), "{:?}", c.validate());
    }
}
