//! `phy/nr-v2x/bler-lut-lusvarghi-2024` — the NR-V2X sidelink link-level lookups of
//! Lusvarghi, Coll-Perales, Gozalvez and Merani, "Link Level Analysis of NR V2X Sidelink
//! Communications", IEEE Internet of Things Journal, 2024 (DOI 10.1109/JIOT.2024.3402551),
//! transcribed from the paper's own figures.
//!
//! # What the source is
//!
//! A MATLAB link-level simulator (openly released: github.com/uwicore/NR-V2X-SL-
//! LinkLevelSimulator) run over the 3GPP TR 37.885 V2V CDL channel models with the ETSI
//! EN 303 798 NR-V2X configuration (the paper's Table I): 5.9 GHz, 2 Tx and 4 Rx isotropic
//! antennas, one layer, 20 MHz at 30 kHz sub-carrier spacing (51 PRB), 14 symbols a slot,
//! sub-channels of 12 PRB (4 in the channel), a 12-PRB, 3-symbol PSCCH, the 2-symbol
//! PSSCH-DMRS pattern, MCS from TS 38.214 Table 5.1.3.1-2 (the 256-QAM table). Every
//! point is a measured BLER at an *average* SNR over 10^4 transport blocks, with a
//! relative 95 % margin of error below 0.007 (§VI). The curves therefore include the
//! channel's small-scale fading: a caller that also draws a fast-fading sample per packet
//! counts it twice.
//!
//! # How the points were transcribed
//!
//! The authors' full dataset (6,188 curves as CSV) sits behind a registration form, which
//! this build did not fill in. The paper's figures are vector drawings: every plotted
//! point is a marker path, and every grid line an embedded image. The points below are
//! the markers' centres mapped through the figures' own grid lines (the SNR axis through
//! its labelled vertical lines, the BLER axis through its decade lines). They land on
//! integer SNRs to within 0.03 dB, which is the simulator's 1 dB SNR step; the BLER is
//! read to three significant figures. A value the source plots as BLER = 1 (all
//! transport blocks lost) is carried as the curve's first point. The cross-checks in the
//! tests hold the transcription to the paper's own text: the 1st-stage SCI curve is the
//! same in all four panels of Fig. 4, QPSK-308's 1 % point moves "from −1.8 dB to 5.5 dB"
//! between 0 and 280 km/h (§VI.B), 16QAM-658 needs "approximately 7 dB" more than
//! QPSK-602 at 1 % (§VI.D), and urban NLOSv and NLOS cost 16QAM-490 "2 dB" and "4 dB"
//! at 1 % (§VI.C).
//!
//! # What is covered
//!
//! * **Transport block, highway LOS, 0 km/h, one sub-channel:** twelve MCS — the smallest
//!   and largest code rate of each modulation (Fig. 8) and one in between (Fig. 4).
//! * **Relative speed, highway LOS:** 70, 140 and 280 km/h for the four MCS of Fig. 5.
//! * **Channel state:** highway NLOSv, and urban LOS, NLOSv and NLOS (Fig. 7), at 0 km/h,
//!   for the same four MCS.
//! * **The 1st-stage SCI** (PSCCH, fixed QPSK, Fig. 4), which the paper shows barely
//!   moves with speed (Fig. 6b).
//!
//! An MCS between two transcribed ones of the same modulation is placed by interpolating
//! the 10 % SNR linearly in code rate, and takes the shape of the transcribed curve of
//! its modulation for the condition asked for; [`NrCurveSource`] says which of the three
//! a curve is.

use serde::Serialize;

use crate::bler::BlerCurve;
use crate::sidelink::SlMcsSpec;

/// The propagation environment of a link, as TR 37.885 §6.2.3 names its CDL models.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NrEnvironment {
    /// Highway (TR 37.885 Tables 6.2.3.1-1 and -2).
    Highway,
    /// Urban grid (TR 37.885 Tables 6.2.3.1-3 to -5).
    Urban,
}

/// The state of a link: line of sight, blocked by vehicles, or blocked by buildings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NrLinkState {
    /// Line of sight.
    Los,
    /// The line of sight is blocked by vehicles.
    NlosV,
    /// The line of sight is blocked by buildings (urban only).
    Nlos,
}

/// What a sidelink transport block's link looked like: the inputs a link-level curve is
/// indexed by.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct NrLinkCondition {
    /// The environment.
    pub environment: NrEnvironment,
    /// The link state.
    pub state: NrLinkState,
    /// The transmitter-receiver relative speed, km/h.
    pub relative_speed_kmh: f64,
}

impl NrLinkCondition {
    /// The condition every figure of the paper that does not say otherwise is drawn at:
    /// highway, line of sight, 0 km/h.
    pub const REFERENCE: NrLinkCondition = NrLinkCondition {
        environment: NrEnvironment::Highway,
        state: NrLinkState::Los,
        relative_speed_kmh: 0.0,
    };

    /// The speed the paper simulated that this condition is read at.
    ///
    /// Highway: 0, 70, 140 or 280 km/h (TR 37.885's 70 and 140 km/h vehicles, same or
    /// opposite direction), the nearest one. Urban: 0 km/h, the only urban speed the paper
    /// prints; its dataset has 60 and 120 km/h, and this build does not carry them.
    #[must_use]
    pub fn speed_bucket_kmh(&self) -> u16 {
        match self.environment {
            NrEnvironment::Urban => 0,
            NrEnvironment::Highway => {
                let v = self.relative_speed_kmh.abs();
                if v < 35.0 {
                    0
                } else if v < 105.0 {
                    70
                } else if v < 210.0 {
                    140
                } else {
                    280
                }
            }
        }
    }
}

/// Where a curve [`tb_curve`] returns came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum NrCurveSource {
    /// Every point transcribed from the paper for exactly this MCS and condition.
    Transcribed,
    /// The MCS's own 10 % point is transcribed (highway LOS, 0 km/h); the condition's
    /// effect is the transcribed curve of the same modulation in that condition, moved
    /// onto it.
    TranscribedShifted,
    /// The MCS was not plotted: its 10 % point is interpolated in code rate between two
    /// transcribed MCS of the same modulation, and the shape is the modulation's
    /// transcribed curve for the condition.
    Interpolated,
}

impl NrCurveSource {
    /// The label a report prints.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            NrCurveSource::Transcribed => "transcribed",
            NrCurveSource::TranscribedShifted => "transcribed-shifted",
            NrCurveSource::Interpolated => "interpolated",
        }
    }
}

/// One transcribed curve: `(SNR dB, BLER)` points in ascending SNR.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NrLut {
    /// The TB MCS: modulation order and code rate × 1024 (Table 5.1.3.1-2's rounding of
    /// 682.5 and 916.5 to the source's own labels, 683 and 917).
    pub qm: u8,
    /// Code rate × 1024.
    pub r_1024: u16,
    /// The environment.
    pub environment: NrEnvironment,
    /// The link state.
    pub state: NrLinkState,
    /// The relative speed, km/h.
    pub speed_kmh: u16,
    /// Which figure of the paper the points are read from.
    pub figure: &'static str,
    /// The points. BLER 1 below the first.
    pub points: &'static [(f64, f64)],
}

use NrEnvironment::{Highway, Urban};
use NrLinkState::{Los, Nlos, NlosV};

macro_rules! lut {
    ($qm:expr, $r:expr, $env:expr, $state:expr, $v:expr, $fig:expr, [$(($s:expr, $b:expr)),* $(,)?]) => {
        NrLut {
            qm: $qm,
            r_1024: $r,
            environment: $env,
            state: $state,
            speed_kmh: $v,
            figure: $fig,
            points: &[$(($s as f64, $b)),*],
        }
    };
}

/// Every transport-block curve transcribed from the paper.
///
/// The first point of each curve is the last SNR the source plots at BLER 1 (or just
/// below it); BLER is 1 at every SNR below it.
pub const NR_TB_LUTS: &[NrLut] = &[
    // --- Fig. 8: highway LOS, 0 km/h, N_sub 1, smallest and largest rate per modulation.
    lut!(
        2,
        120,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (-10, 1.0),
            (-9, 0.952),
            (-8, 0.703),
            (-7, 0.272),
            (-6, 0.0412),
            (-5, 0.0032),
            (-4, 0.0002)
        ]
    ),
    lut!(
        2,
        602,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (-3, 1.0),
            (-2, 0.985),
            (-1, 0.801),
            (0, 0.318),
            (1, 0.0466),
            (2, 0.0030)
        ]
    ),
    lut!(
        4,
        378,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (0, 1.0),
            (1, 0.952),
            (2, 0.657),
            (3, 0.211),
            (4, 0.0248),
            (5, 0.000901)
        ]
    ),
    lut!(
        4,
        658,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (4, 1.0),
            (5, 0.980),
            (6, 0.777),
            (7, 0.301),
            (8, 0.0403),
            (9, 0.0023)
        ]
    ),
    lut!(
        6,
        466,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (5, 1.0),
            (6, 0.992),
            (7, 0.850),
            (8, 0.396),
            (9, 0.0746),
            (10, 0.0048)
        ]
    ),
    lut!(
        6,
        873,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (12, 1.0),
            (13, 0.985),
            (14, 0.822),
            (15, 0.353),
            (16, 0.0561),
            (17, 0.0039)
        ]
    ),
    lut!(
        8,
        683,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (14, 1.0),
            (15, 0.945),
            (16, 0.685),
            (17, 0.276),
            (18, 0.0468),
            (19, 0.0027),
            (20, 0.0001)
        ]
    ),
    lut!(
        8,
        948,
        Highway,
        Los,
        0,
        "Fig. 8",
        [
            (20, 1.0),
            (21, 0.985),
            (22, 0.857),
            (23, 0.488),
            (24, 0.156),
            (25, 0.0248),
            (26, 0.00319),
            (27, 0.000499)
        ]
    ),
    // --- Fig. 4 (TB) and Fig. 5 (v_rel 0): highway LOS, the four representative MCS.
    lut!(
        2,
        308,
        Highway,
        Los,
        0,
        "Fig. 4a, 5a",
        [
            (-6, 1.0),
            (-5, 0.948),
            (-4, 0.644),
            (-3, 0.197),
            (-2, 0.0206),
            (-1, 0.0010)
        ]
    ),
    lut!(
        4,
        490,
        Highway,
        Los,
        0,
        "Fig. 4b, 5b",
        [
            (2, 1.0),
            (3, 0.944),
            (4, 0.620),
            (5, 0.170),
            (6, 0.0149),
            (7, 0.0003)
        ]
    ),
    lut!(
        6,
        616,
        Highway,
        Los,
        0,
        "Fig. 4c, 5c",
        [
            (8, 1.0),
            (9, 0.938),
            (10, 0.591),
            (11, 0.159),
            (12, 0.0123),
            (13, 0.0004)
        ]
    ),
    lut!(
        8,
        797,
        Highway,
        Los,
        0,
        "Fig. 4d, 5d",
        [
            (17, 1.0),
            (18, 0.940),
            (19, 0.624),
            (20, 0.203),
            (21, 0.0247),
            (22, 0.00141)
        ]
    ),
    // --- Fig. 5: relative speed, highway LOS.
    lut!(
        2,
        308,
        Highway,
        Los,
        70,
        "Fig. 5a",
        [
            (-6, 1.0),
            (-5, 0.981),
            (-4, 0.881),
            (-3, 0.562),
            (-2, 0.182),
            (-1, 0.0235),
            (0, 0.000904)
        ]
    ),
    lut!(
        2,
        308,
        Highway,
        Los,
        140,
        "Fig. 5a",
        [
            (-5, 1.0),
            (-4, 0.963),
            (-3, 0.812),
            (-2, 0.511),
            (-1, 0.201),
            (0, 0.0451),
            (1, 0.00492),
            (2, 0.000402)
        ]
    ),
    lut!(
        2,
        308,
        Highway,
        Los,
        280,
        "Fig. 5a",
        [
            (-4, 1.0),
            (-3, 0.987),
            (-2, 0.957),
            (-1, 0.881),
            (0, 0.767),
            (1, 0.622),
            (2, 0.492),
            (3, 0.380),
            (4, 0.219),
            (5, 0.0454),
            (6, 0.00191)
        ]
    ),
    lut!(
        4,
        490,
        Highway,
        Los,
        70,
        "Fig. 5b",
        [
            (2, 1.0),
            (3, 0.975),
            (4, 0.812),
            (5, 0.420),
            (6, 0.0865),
            (7, 0.0041)
        ]
    ),
    lut!(
        4,
        490,
        Highway,
        Los,
        140,
        "Fig. 5b",
        [
            (2, 1.0),
            (3, 0.987),
            (4, 0.854),
            (5, 0.483),
            (6, 0.106),
            (7, 0.00752)
        ]
    ),
    // 280 km/h: an error floor near 2e-3 that no SNR removes (§VI.B: "settles at 0.002").
    lut!(
        4,
        490,
        Highway,
        Los,
        280,
        "Fig. 5b",
        [
            (4, 1.0),
            (5, 0.963),
            (6, 0.876),
            (7, 0.724),
            (8, 0.528),
            (9, 0.339),
            (10, 0.196),
            (11, 0.109),
            (12, 0.0577),
            (13, 0.0316),
            (14, 0.0194),
            (15, 0.0127),
            (16, 0.0087),
            (17, 0.00642),
            (18, 0.00502),
            (19, 0.00451),
            (20, 0.00431),
            (21, 0.0038),
            (22, 0.00361),
            (23, 0.0031),
            (24, 0.00291),
            (25, 0.0026),
            (26, 0.0026),
            (27, 0.0023),
            (28, 0.00211),
            (30, 0.00211)
        ]
    ),
    lut!(
        6,
        616,
        Highway,
        Los,
        70,
        "Fig. 5c",
        [
            (8, 1.0),
            (9, 0.975),
            (10, 0.816),
            (11, 0.416),
            (12, 0.0831),
            (13, 0.00542)
        ]
    ),
    lut!(
        6,
        616,
        Highway,
        Los,
        140,
        "Fig. 5c",
        [
            (9, 1.0),
            (10, 0.927),
            (11, 0.701),
            (12, 0.323),
            (13, 0.0691),
            (14, 0.00694)
        ]
    ),
    // 280 km/h: "the BLER is equal to 1" at every SNR (§VI.B).
    lut!(6, 616, Highway, Los, 280, "Fig. 5c", [(30, 1.0)]),
    lut!(
        8,
        797,
        Highway,
        Los,
        70,
        "Fig. 5d",
        [
            (18, 1.0),
            (19, 0.903),
            (20, 0.645),
            (21, 0.283),
            (22, 0.0632),
            (23, 0.00604),
            (24, 0.000201)
        ]
    ),
    // 140 and 280 km/h: "the BLER is equal to 1 starting from relative speeds greater or
    // equal than 140 km/h" (§VI.B).
    lut!(8, 797, Highway, Los, 140, "Fig. 5d", [(30, 1.0)]),
    lut!(8, 797, Highway, Los, 280, "Fig. 5d", [(30, 1.0)]),
    // --- Fig. 7a: highway NLOSv, 0 km/h.
    lut!(
        2,
        308,
        Highway,
        NlosV,
        0,
        "Fig. 7a",
        [
            (-7, 1.0),
            (-6, 0.960),
            (-5, 0.846),
            (-4, 0.645),
            (-3, 0.375),
            (-2, 0.144),
            (-1, 0.0569),
            (0, 0.0210),
            (1, 0.00759),
            (2, 0.00211)
        ]
    ),
    lut!(
        4,
        490,
        Highway,
        NlosV,
        0,
        "Fig. 7a",
        [
            (1, 1.0),
            (2, 0.951),
            (3, 0.836),
            (4, 0.635),
            (5, 0.364),
            (6, 0.134),
            (7, 0.0546),
            (8, 0.0192),
            (9, 0.00692),
            (10, 0.00171)
        ]
    ),
    lut!(
        6,
        616,
        Highway,
        NlosV,
        0,
        "Fig. 7a",
        [
            (7, 1.0),
            (8, 0.946),
            (9, 0.827),
            (10, 0.621),
            (11, 0.352),
            (12, 0.129),
            (13, 0.0534),
            (14, 0.0175),
            (15, 0.00672),
            (16, 0.0023)
        ]
    ),
    lut!(
        8,
        797,
        Highway,
        NlosV,
        0,
        "Fig. 7a",
        [
            (16, 1.0),
            (17, 0.949),
            (18, 0.829),
            (19, 0.630),
            (20, 0.367),
            (21, 0.144),
            (22, 0.0571),
            (23, 0.0204),
            (24, 0.00851),
            (25, 0.00331),
            (26, 0.0006)
        ]
    ),
    // --- Fig. 7b: urban LOS, NLOSv and NLOS, 0 km/h.
    lut!(
        2,
        308,
        Urban,
        Los,
        0,
        "Fig. 7b",
        [
            (-6, 1.0),
            (-5, 0.943),
            (-4, 0.693),
            (-3, 0.282),
            (-2, 0.0607),
            (-1, 0.0076),
            (0, 0.000999)
        ]
    ),
    lut!(
        4,
        490,
        Urban,
        Los,
        0,
        "Fig. 7b",
        [
            (2, 1.0),
            (3, 0.938),
            (4, 0.679),
            (5, 0.260),
            (6, 0.0534),
            (7, 0.0055),
            (8, 0.0006)
        ]
    ),
    lut!(
        6,
        616,
        Urban,
        Los,
        0,
        "Fig. 7b",
        [
            (8, 1.0),
            (9, 0.935),
            (10, 0.671),
            (11, 0.248),
            (12, 0.0499),
            (13, 0.0050),
            (14, 0.000999),
            (15, 0.0001)
        ]
    ),
    lut!(
        8,
        797,
        Urban,
        Los,
        0,
        "Fig. 7b",
        [
            (17, 1.0),
            (18, 0.941),
            (19, 0.701),
            (20, 0.296),
            (21, 0.0721),
            (22, 0.00968),
            (23, 0.0019),
            (24, 0.0001)
        ]
    ),
    lut!(
        2,
        308,
        Urban,
        NlosV,
        0,
        "Fig. 7b",
        [
            (-7, 1.0),
            (-6, 0.974),
            (-5, 0.882),
            (-4, 0.715),
            (-3, 0.479),
            (-2, 0.239),
            (-1, 0.0964),
            (0, 0.0340),
            (1, 0.00681),
            (2, 0.000501)
        ]
    ),
    lut!(
        4,
        490,
        Urban,
        NlosV,
        0,
        "Fig. 7b",
        [
            (1, 1.0),
            (2, 0.974),
            (3, 0.872),
            (4, 0.701),
            (5, 0.462),
            (6, 0.221),
            (7, 0.0897),
            (8, 0.0284),
            (9, 0.0044),
            (10, 0.0003)
        ]
    ),
    lut!(
        6,
        616,
        Urban,
        NlosV,
        0,
        "Fig. 7b",
        [
            (7, 1.0),
            (8, 0.974),
            (9, 0.864),
            (10, 0.695),
            (11, 0.451),
            (12, 0.217),
            (13, 0.0889),
            (14, 0.0290),
            (15, 0.0043),
            (16, 0.0002)
        ]
    ),
    lut!(
        8,
        797,
        Urban,
        NlosV,
        0,
        "Fig. 7b",
        [
            (16, 1.0),
            (17, 0.974),
            (18, 0.872),
            (19, 0.703),
            (20, 0.468),
            (21, 0.235),
            (22, 0.0959),
            (23, 0.0354),
            (24, 0.0049),
            (25, 0.0002)
        ]
    ),
    lut!(
        2,
        308,
        Urban,
        Nlos,
        0,
        "Fig. 7b",
        [
            (-8, 1.0),
            (-7, 0.974),
            (-6, 0.914),
            (-5, 0.772),
            (-4, 0.590),
            (-3, 0.416),
            (-2, 0.263),
            (-1, 0.147),
            (0, 0.0819),
            (1, 0.0432),
            (2, 0.0235)
        ]
    ),
    lut!(
        4,
        490,
        Urban,
        Nlos,
        0,
        "Fig. 7b",
        [
            (0, 1.0),
            (1, 0.971),
            (2, 0.908),
            (3, 0.763),
            (4, 0.580),
            (5, 0.407),
            (6, 0.260),
            (7, 0.145),
            (8, 0.0810),
            (9, 0.0419),
            (10, 0.0237),
            (11, 0.0100),
            (12, 0.0022),
            (13, 0.0002)
        ]
    ),
    lut!(
        6,
        616,
        Urban,
        Nlos,
        0,
        "Fig. 7b",
        [
            (6, 1.0),
            (7, 0.968),
            (8, 0.908),
            (9, 0.758),
            (10, 0.573),
            (11, 0.404),
            (12, 0.251),
            (13, 0.141),
            (14, 0.0791),
            (15, 0.0419),
            (16, 0.0237),
            (17, 0.00949),
            (18, 0.0026),
            (19, 0.0002)
        ]
    ),
    lut!(
        8,
        797,
        Urban,
        Nlos,
        0,
        "Fig. 7b",
        [
            (15, 1.0),
            (16, 0.963),
            (17, 0.898),
            (18, 0.745),
            (19, 0.563),
            (20, 0.397),
            (21, 0.249),
            (22, 0.135),
            (23, 0.0791),
            (24, 0.0429),
            (25, 0.0238),
            (26, 0.0107),
            (27, 0.0026),
            (28, 0.0001)
        ]
    ),
];

/// The 1st-stage SCI's curve (PSCCH, QPSK, fixed code rate): highway LOS, 0 km/h, the
/// median of the four panels of Fig. 4 at each SNR (they agree to the source's Monte
/// Carlo resolution; they differ only below 1e-3, at 1e-4 to 6e-4 at −5 dB).
pub const NR_SCI1_LUT: &[(f64, f64)] = &[
    (-12.0, 1.0),
    (-11.0, 0.930),
    (-10.0, 0.745),
    (-9.0, 0.436),
    (-8.0, 0.165),
    (-7.0, 0.035),
    (-6.0, 0.0040),
    (-5.0, 0.0004),
];

/// The four MCS the paper prints in every condition, one per modulation: the shape an
/// untranscribed (MCS, condition) pair borrows.
const REPRESENTATIVE: [(u8, u16); 4] = [(2, 308), (4, 490), (6, 616), (8, 797)];

/// TS 38.214 Table 5.1.3.1-2 (the 256-QAM MCS table ETSI EN 303 798 configures for the
/// sidelink): `(Qm, R·1024)` for MCS 0 to 27. Rows 20 and 26 are 682.5 and 916.5 in the
/// table, carried as the source dataset labels them (683, 917); the spectral-efficiency
/// error is 0.004 bit per resource element.
pub const NR_MCS_TABLE2: [(u8, u16); 28] = [
    (2, 120),
    (2, 193),
    (2, 308),
    (2, 449),
    (2, 602),
    (4, 378),
    (4, 434),
    (4, 490),
    (4, 553),
    (4, 616),
    (4, 658),
    (6, 466),
    (6, 517),
    (6, 567),
    (6, 616),
    (6, 666),
    (6, 719),
    (6, 772),
    (6, 822),
    (6, 873),
    (8, 683),
    (8, 711),
    (8, 754),
    (8, 797),
    (8, 841),
    (8, 885),
    (8, 917),
    (8, 948),
];

const NR_MCS_TABLE2_LABELS: [&str; 28] = [
    "nr-t2-mcs0",
    "nr-t2-mcs1",
    "nr-t2-mcs2",
    "nr-t2-mcs3",
    "nr-t2-mcs4",
    "nr-t2-mcs5",
    "nr-t2-mcs6",
    "nr-t2-mcs7",
    "nr-t2-mcs8",
    "nr-t2-mcs9",
    "nr-t2-mcs10",
    "nr-t2-mcs11",
    "nr-t2-mcs12",
    "nr-t2-mcs13",
    "nr-t2-mcs14",
    "nr-t2-mcs15",
    "nr-t2-mcs16",
    "nr-t2-mcs17",
    "nr-t2-mcs18",
    "nr-t2-mcs19",
    "nr-t2-mcs20",
    "nr-t2-mcs21",
    "nr-t2-mcs22",
    "nr-t2-mcs23",
    "nr-t2-mcs24",
    "nr-t2-mcs25",
    "nr-t2-mcs26",
    "nr-t2-mcs27",
];

/// One row of TS 38.214 Table 5.1.3.1-2, or `None` above 27.
#[must_use]
pub fn nr_mcs_table2(index: u8) -> Option<SlMcsSpec> {
    let i = usize::from(index);
    let (qm, r) = *NR_MCS_TABLE2.get(i)?;
    Some(SlMcsSpec::new(NR_MCS_TABLE2_LABELS[i], qm, r))
}

fn find(qm: u8, r: u16, env: NrEnvironment, state: NrLinkState, v: u16) -> Option<&'static NrLut> {
    NR_TB_LUTS.iter().find(|l| {
        l.qm == qm && l.r_1024 == r && l.environment == env && l.state == state && l.speed_kmh == v
    })
}

fn curve_of(l: &NrLut, label: String) -> BlerCurve {
    BlerCurve::new(label, l.points.to_vec())
}

/// The SNR at which a transcribed curve crosses 10 %.
fn ten_pc(l: &NrLut) -> Option<f64> {
    curve_of(l, String::new()).snr_at_bler(0.1)
}

/// The transcribed reference-condition curves of one modulation, `(rate, 10 % SNR)`, in
/// ascending rate.
fn anchors(qm: u8) -> Vec<(u16, f64)> {
    let mut v: Vec<(u16, f64)> = NR_TB_LUTS
        .iter()
        .filter(|l| l.qm == qm && l.environment == Highway && l.state == Los && l.speed_kmh == 0)
        .filter_map(|l| ten_pc(l).map(|s| (l.r_1024, s)))
        .collect();
    v.sort_by_key(|a| a.0);
    v
}

/// The reference-condition 10 % SNR of an MCS: transcribed if it was plotted, otherwise
/// interpolated linearly in code rate between the nearest transcribed rates of its
/// modulation (extrapolated from the nearest two outside them).
fn reference_ten_pc(qm: u8, r: u16) -> Option<(f64, bool)> {
    let a = anchors(qm);
    if let Some(&(_, s)) = a.iter().find(|x| x.0 == r) {
        return Some((s, true));
    }
    if a.len() < 2 {
        return None;
    }
    let i = a.iter().position(|x| x.0 > r).unwrap_or(a.len() - 1).max(1);
    let (r0, s0) = a[i - 1];
    let (r1, s1) = a[i];
    let t = (f64::from(r) - f64::from(r0)) / (f64::from(r1) - f64::from(r0));
    Some((s0 + t * (s1 - s0), false))
}

/// The transport block's BLER curve for an MCS under a link condition, and where it came
/// from. `None` when the MCS is not a TS 38.214 Table 5.1.3.1-2 row.
#[must_use]
pub fn tb_curve(mcs: SlMcsSpec, cond: &NrLinkCondition) -> Option<(BlerCurve, NrCurveSource)> {
    let (qm, r) = (mcs.qm, mcs.r_1024);
    if !NR_MCS_TABLE2.contains(&(qm, r)) {
        return None;
    }
    // Highway has no building-blocked CDL; a building-blocked highway link reads the
    // urban NLOS curve, the only one the source has for it.
    let (env, state) = match (cond.environment, cond.state) {
        (Highway, Nlos) => (Urban, Nlos),
        other => other,
    };
    let v = NrLinkCondition {
        environment: env,
        state,
        relative_speed_kmh: cond.relative_speed_kmh,
    }
    .speed_bucket_kmh();
    let label = |src: NrCurveSource| {
        format!(
            "lusvarghi-2024/q{qm}-r{r}/{env:?}-{state:?}-{v}kmh/{}",
            src.label()
        )
        .to_lowercase()
    };
    if let Some(l) = find(qm, r, env, state, v) {
        return Some((
            curve_of(l, label(NrCurveSource::Transcribed)),
            NrCurveSource::Transcribed,
        ));
    }
    // The modulation's representative curve in this condition gives the shape; it is
    // moved so that the representative's own reference-condition 10 % point lands on
    // this MCS's reference-condition 10 % point.
    let rep = REPRESENTATIVE.iter().find(|x| x.0 == qm)?;
    let shape = find(qm, rep.1, env, state, v)?;
    let rep_ref = ten_pc(find(qm, rep.1, Highway, Los, 0)?)?;
    let (own_ref, transcribed) = reference_ten_pc(qm, r)?;
    let src = if transcribed {
        NrCurveSource::TranscribedShifted
    } else {
        NrCurveSource::Interpolated
    };
    let moved = curve_of(shape, String::new()).shifted(own_ref - rep_ref);
    Some((BlerCurve::new(label(src), moved.points().to_vec()), src))
}

/// The 1st-stage SCI's curve. It is independent of the TB MCS (a fixed QPSK code on the
/// PSCCH) and, in the paper's Fig. 6b, of the relative speed.
#[must_use]
pub fn sci1_curve() -> BlerCurve {
    BlerCurve::new("lusvarghi-2024/sci1/highway-los-0kmh", NR_SCI1_LUT.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(l: &NrLut, pct: f64) -> f64 {
        curve_of(l, String::new())
            .snr_at_bler(pct)
            .expect("crosses")
    }

    #[test]
    fn every_transcribed_curve_falls_and_stays_a_probability() {
        for l in NR_TB_LUTS {
            for w in l.points.windows(2) {
                assert!(w[1].0 > w[0].0, "{l:?}: SNR not ascending");
                assert!(w[1].1 <= w[0].1, "{l:?}: BLER rises from {w:?}");
            }
            assert!(l.points.iter().all(|p| p.1 > 0.0 && p.1 <= 1.0));
            assert!(
                NR_MCS_TABLE2.contains(&(l.qm, l.r_1024)),
                "{l:?} is not a Table 2 row"
            );
        }
    }

    /// §VI.B: "the SNR corresponding to BLER = 0.01 increases from −1.8 dB to 5.5 dB when
    /// the relative speed augments from 0 km/h to 280 km/h" (QPSK-308).
    #[test]
    fn qpsk_308_moves_as_the_paper_says_with_speed() {
        let v0 = at(find(2, 308, Highway, Los, 0).unwrap(), 0.01);
        let v280 = at(find(2, 308, Highway, Los, 280).unwrap(), 0.01);
        assert!((v0 - -1.8).abs() < 0.15, "{v0}");
        assert!((v280 - 5.5).abs() < 0.15, "{v280}");
    }

    /// §VI.D: "16QAM-658 requires an approximately 7 dB larger SNR to get the same
    /// BLER = 0.01 performance compared to a TB transmitted with QPSK-602".
    #[test]
    fn sixteen_qam_658_needs_seven_db_more_than_qpsk_602() {
        let d = at(find(4, 658, Highway, Los, 0).unwrap(), 0.01)
            - at(find(2, 602, Highway, Los, 0).unwrap(), 0.01);
        assert!((d - 7.0).abs() < 0.5, "{d}");
    }

    /// §VI.C: for 16QAM-490 in the urban environment "the SNR difference for a BLER =
    /// 0.01 grows from 2 dB to 4 dB when comparing LOS curves with their NLOSv and NLOS
    /// counterparts", and highway NLOSv costs QPSK-308 2.6 dB at 1 %.
    #[test]
    fn the_channel_state_costs_what_the_paper_says() {
        let los = at(find(4, 490, Urban, Los, 0).unwrap(), 0.01);
        let nlosv = at(find(4, 490, Urban, NlosV, 0).unwrap(), 0.01);
        let nlos = at(find(4, 490, Urban, Nlos, 0).unwrap(), 0.01);
        assert!((nlosv - los - 2.0).abs() < 0.6, "NLOSv {}", nlosv - los);
        assert!((nlos - los - 4.0).abs() < 0.6, "NLOS {}", nlos - los);
        let h = at(find(2, 308, Highway, NlosV, 0).unwrap(), 0.01)
            - at(find(2, 308, Highway, Los, 0).unwrap(), 0.01);
        assert!((h - 2.6).abs() < 0.3, "highway NLOSv {h}");
    }

    /// The 16QAM-490 curve at 280 km/h floors near 0.002 (§VI.B: "settles at 0.002 when
    /// v_rel = 280 km/h and the SNR is higher than 25 dB"), and 64QAM-616 at 280 km/h and
    /// 256QAM-797 from 140 km/h never decode.
    #[test]
    fn the_high_speed_error_floors_are_kept() {
        let (c, _) = tb_curve(
            nr_mcs_table2(7).unwrap(),
            &NrLinkCondition {
                relative_speed_kmh: 280.0,
                ..NrLinkCondition::REFERENCE
            },
        )
        .unwrap();
        assert!((c.bler(40.0) - 0.00211).abs() < 1e-4);
        for (mcs, v) in [(14u8, 280.0), (23, 140.0), (23, 280.0)] {
            let (c, _) = tb_curve(
                nr_mcs_table2(mcs).unwrap(),
                &NrLinkCondition {
                    relative_speed_kmh: v,
                    ..NrLinkCondition::REFERENCE
                },
            )
            .unwrap();
            assert_eq!(c.bler(60.0), 1.0, "mcs {mcs} at {v} km/h decodes");
        }
    }

    #[test]
    fn the_sci1_curve_is_more_robust_than_every_tb_curve() {
        let sci = sci1_curve();
        let s = sci.snr_at_bler(0.1).unwrap();
        for l in NR_TB_LUTS {
            if let Some(t) = ten_pc(l) {
                assert!(s < t, "{l:?}: {t} vs SCI {s}");
            }
        }
    }

    #[test]
    fn a_transcribed_mcs_reads_its_own_points_and_an_untranscribed_one_is_placed_between() {
        let (c, src) = tb_curve(nr_mcs_table2(4).unwrap(), &NrLinkCondition::REFERENCE).unwrap();
        assert_eq!(src, NrCurveSource::Transcribed);
        assert_eq!(c.bler(0.0), 0.318);
        // QPSK-449 lies between QPSK-308 and QPSK-602.
        let (c, src) = tb_curve(nr_mcs_table2(3).unwrap(), &NrLinkCondition::REFERENCE).unwrap();
        assert_eq!(src, NrCurveSource::Interpolated);
        let s = c.snr_at_bler(0.1).unwrap();
        let lo = at(find(2, 308, Highway, Los, 0).unwrap(), 0.1);
        let hi = at(find(2, 602, Highway, Los, 0).unwrap(), 0.1);
        assert!(lo < s && s < hi, "{lo} < {s} < {hi}");
        // QPSK-602 in urban NLOS borrows QPSK-308's urban NLOS shape.
        let (_, src) = tb_curve(
            nr_mcs_table2(4).unwrap(),
            &NrLinkCondition {
                environment: Urban,
                state: Nlos,
                relative_speed_kmh: 0.0,
            },
        )
        .unwrap();
        assert_eq!(src, NrCurveSource::TranscribedShifted);
        // A Table 5.1.3.1-1 row that is not in Table 2 has no curve here.
        assert!(tb_curve(SlMcsSpec::new("t1", 2, 679), &NrLinkCondition::REFERENCE).is_none());
    }

    #[test]
    fn higher_rates_need_more_snr_within_each_modulation() {
        for qm in [2u8, 4, 6, 8] {
            let a = anchors(qm);
            for w in a.windows(2) {
                assert!(w[1].1 > w[0].1, "q{qm}: {a:?}");
            }
        }
        let mut last = f64::NEG_INFINITY;
        for i in 0..28u8 {
            let (c, _) = tb_curve(nr_mcs_table2(i).unwrap(), &NrLinkCondition::REFERENCE).unwrap();
            let s = c.snr_at_bler(0.1).unwrap();
            // Table 2's rows are ordered by spectral efficiency except at the modulation
            // switches, where a higher-order row can need slightly less SNR; allow 1.5 dB.
            assert!(s > last - 1.5, "mcs {i}: {s} after {last}");
            last = s;
        }
    }
}
