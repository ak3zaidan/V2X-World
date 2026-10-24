//! The CAM generation rules of ETSI EN 302 637-2 V1.3.2 §6.1.3, written out a second time
//! as an independent oracle and driven, check by check, against `CamTriggerState`.
//!
//! The oracle is written from the clause, not from any implementation:
//!
//! * The generation check runs every `T_CheckCamGen` ≤ `T_GenCamMin`; here every 100 ms.
//! * `T_GenCamMin` = 100 ms, `T_GenCamMax` = 1000 ms. DCC supplies `T_GenCam_DCC`, which
//!   is kept within `[T_GenCamMin, T_GenCamMax]`.
//! * **Condition 1**: at least `T_GenCam_DCC` since the last CAM, and the heading has
//!   changed by more than 4°, or the position by more than 4 m, or the speed by more than
//!   0.5 m/s since it. The CAM is generated at once, `T_GenCam` becomes the time since the
//!   last CAM (bounded by `T_GenCamMax`) and holds for the next `N_GenCam` = 3 CAMs.
//! * **Condition 2**: at least `T_GenCam` and at least `T_GenCam_DCC` since the last CAM.
//!   Once `N_GenCam` such CAMs have gone out, `T_GenCam` returns to `T_GenCamMax`.
//! * The first CAM carries the low-frequency container, and so does every CAM generated
//!   500 ms or more after the last CAM that carried it.
//!
//! The expected CAM and low-frequency counts per DCC interval are held as data
//! ([`EXPECTED`]). They were first obtained by running Artery's `CaService` on the same
//! trajectory (behaviour only: its output counts are the cross-check value, and none of
//! its code is used here). `the_oracle_can_tell_a_wrong_rule` shows the comparison goes
//! red when a rule of the oracle is changed.

use v2xw_core::geom::Vec3;
use v2xw_core::time::{Duration, SimTime};
use v2xw_msg::generator::{CamDynamics, CamGenParams, CamTriggerState, DccState};

const MS: u64 = 1_000_000;

/// `(T_GenCam_DCC ms, CAMs generated, CAMs with the low-frequency container)` over the
/// 60 s trajectory below.
const EXPECTED: [(u64, u32, u32); 6] = [
    (0, 181, 89),
    (100, 181, 89),
    (250, 159, 89),
    (500, 104, 104),
    (1000, 60, 60),
    (2000, 60, 60),
];

/// The §6.1.3 constants, so that a test can perturb one of them.
#[derive(Clone, Copy)]
struct Rules {
    t_gen_cam_min: u64,
    t_gen_cam_max: u64,
    n_gen_cam: u32,
    heading_deg: f64,
    position_m: f64,
    speed_mps: f64,
    low_frequency_interval: u64,
}

impl Rules {
    const STANDARD: Rules = Rules {
        t_gen_cam_min: 100 * MS,
        t_gen_cam_max: 1000 * MS,
        n_gen_cam: 3,
        heading_deg: 4.0,
        position_m: 4.0,
        speed_mps: 0.5,
        low_frequency_interval: 500 * MS,
    };
}

/// What the previous CAM said, and when.
#[derive(Clone)]
struct LastCam {
    at: u64,
    dynamics: CamDynamics,
}

/// The originating station's CAM generation state under §6.1.3.
struct Clause6_1_3 {
    rules: Rules,
    /// `T_GenCam`, ns.
    t_gen_cam: u64,
    /// CAMs still to be generated at the shortened `T_GenCam` before it returns to
    /// `T_GenCamMax`.
    shortened_left: u32,
    last: Option<LastCam>,
    last_low_frequency_at: Option<u64>,
}

impl Clause6_1_3 {
    fn new(rules: Rules) -> Self {
        Self {
            rules,
            t_gen_cam: rules.t_gen_cam_max,
            shortened_left: 0,
            last: None,
            last_low_frequency_at: None,
        }
    }

    /// Whether the dynamics moved enough since the last CAM for condition 1.
    fn dynamics_changed(&self, then: &CamDynamics, now: &CamDynamics) -> bool {
        let turned = (now.heading_rad - then.heading_rad)
            .to_degrees()
            .rem_euclid(360.0);
        let turned = turned.min(360.0 - turned);
        turned > self.rules.heading_deg
            || now.pos.distance_2d(then.pos) > self.rules.position_m
            || (now.speed_mps - then.speed_mps).abs() > self.rules.speed_mps
    }

    /// One generation check at `now` with DCC's interval `dcc`. Returns whether the CAM
    /// generated, if one is, carries the low-frequency container.
    fn check(&mut self, now: u64, d: &CamDynamics, dcc: u64) -> Option<bool> {
        let t_gen_cam_dcc = dcc.clamp(self.rules.t_gen_cam_min, self.rules.t_gen_cam_max);
        let Some(last) = self.last.clone() else {
            return Some(self.generate(now, d));
        };
        let since = now - last.at;
        if since < t_gen_cam_dcc {
            return None;
        }
        if self.dynamics_changed(&last.dynamics, d) {
            // Condition 1.
            self.t_gen_cam = since.min(self.rules.t_gen_cam_max);
            self.shortened_left = self.rules.n_gen_cam;
            return Some(self.generate(now, d));
        }
        if since < self.t_gen_cam {
            return None;
        }
        // Condition 2.
        self.shortened_left = self.shortened_left.saturating_sub(1);
        if self.shortened_left == 0 {
            self.t_gen_cam = self.rules.t_gen_cam_max;
        }
        Some(self.generate(now, d))
    }

    fn generate(&mut self, now: u64, d: &CamDynamics) -> bool {
        self.last = Some(LastCam {
            at: now,
            dynamics: d.clone(),
        });
        let low_frequency = match self.last_low_frequency_at {
            None => true,
            Some(t) => now - t >= self.rules.low_frequency_interval,
        };
        if low_frequency {
            self.last_low_frequency_at = Some(now);
        }
        low_frequency
    }
}

/// A 60 s drive: cruise, a slow crawl, a stop, a turn through a junction, hard
/// acceleration, and a long gentle curve.
fn trajectory() -> Vec<(SimTime, CamDynamics)> {
    let mut out = Vec::new();
    let (mut x, mut y, mut h, mut v) = (0.0_f64, 0.0_f64, 0.0_f64, 13.9_f64);
    for k in 0..600u64 {
        let t = k as f64 * 0.1;
        let (a, w) = match t {
            t if t < 10.0 => (0.0, 0.0),   // cruise at 50 km/h
            t if t < 15.0 => (-2.38, 0.0), // brake to a crawl
            t if t < 25.0 => (0.0, 0.0),   // crawl at ~2 m/s
            t if t < 26.0 => (-2.0, 0.0),  // stop
            t if t < 35.0 => (0.0, 0.0),   // stationary
            t if t < 41.0 => (1.0, 0.26),  // pull away turning ~90°
            t if t < 47.0 => (2.5, 0.0),   // hard acceleration
            _ => (0.0, 0.012),             // long gentle curve
        };
        v = (v + a * 0.1).max(0.0);
        h += w * 0.1;
        x += v * h.cos() * 0.1;
        y += v * h.sin() * 0.1;
        out.push((
            k * 100 * MS,
            CamDynamics {
                pos: Vec3::new(x, y, 0.0),
                heading_rad: h,
                speed_mps: v,
            },
        ));
    }
    out
}

/// Per DCC interval: the engine's counts, the oracle's counts and the checks at which the
/// two disagree (ms).
fn compare(rules: Rules, dcc_ms: u64) -> ((u32, u32), (u32, u32), Vec<u64>) {
    let mut ours = CamTriggerState::new(CamGenParams::default());
    let mut oracle = Clause6_1_3::new(rules);
    let dcc = DccState {
        t_off: Duration::from_millis(dcc_ms),
        cbr: None,
    };
    let (mut a_n, mut a_lf, mut b_n, mut b_lf, mut mismatches) = (0, 0, 0, 0, Vec::new());
    for (t, d) in &trajectory() {
        let a = ours.check(*t, d, &dcc).map(|x| x.include_low_frequency);
        let b = oracle.check(*t, d, dcc_ms * MS);
        a_n += u32::from(a.is_some());
        a_lf += u32::from(a == Some(true));
        b_n += u32::from(b.is_some());
        b_lf += u32::from(b == Some(true));
        if a != b {
            mismatches.push(*t / MS);
        }
    }
    ((a_n, a_lf), (b_n, b_lf), mismatches)
}

#[test]
fn cam_generation_follows_en_302_637_2_clause_6_1_3() {
    for (dcc_ms, n, lf) in EXPECTED {
        let (ours, oracle, mismatches) = compare(Rules::STANDARD, dcc_ms);
        println!(
            "MEAS cam-6.1.3 dcc {dcc_ms:>4} ms: ours {} CAMs ({} with LF), oracle {} ({}), \
             expected {n} ({lf}), mismatched checks {}",
            ours.0,
            ours.1,
            oracle.0,
            oracle.1,
            mismatches.len()
        );
        assert!(
            mismatches.is_empty(),
            "dcc {dcc_ms} ms: differs from §6.1.3 at {mismatches:?} ms"
        );
        assert_eq!(ours, (n, lf), "dcc {dcc_ms} ms: CAM counts");
        assert_eq!(oracle, (n, lf), "dcc {dcc_ms} ms: oracle counts");
    }
}

/// The comparison is not vacuous: change any one rule of the oracle and it disagrees with
/// the engine on this trajectory.
#[test]
fn the_oracle_can_tell_a_wrong_rule() {
    let s = Rules::STANDARD;
    let wrong = [
        ("N_GenCam 2", Rules { n_gen_cam: 2, ..s }),
        (
            "heading 8°",
            Rules {
                heading_deg: 8.0,
                ..s
            },
        ),
        (
            "position 8 m",
            Rules {
                position_m: 8.0,
                ..s
            },
        ),
        (
            "speed 1 m/s",
            Rules {
                speed_mps: 1.0,
                ..s
            },
        ),
        (
            "LF 400 ms",
            Rules {
                low_frequency_interval: 400 * MS,
                ..s
            },
        ),
        (
            "T_GenCamMax 2 s",
            Rules {
                t_gen_cam_max: 2000 * MS,
                ..s
            },
        ),
    ];
    for (what, rules) in wrong {
        let caught = EXPECTED
            .iter()
            .any(|(dcc_ms, ..)| !compare(rules, *dcc_ms).2.is_empty());
        assert!(caught, "{what}: the oracle did not notice");
    }
}
