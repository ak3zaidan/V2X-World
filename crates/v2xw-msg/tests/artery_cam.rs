//! The CAM generation rules against Artery's `CaService` (riebl/artery master,
//! `src/artery/application/CaService.cc`, `checkTriggeringConditions`, `sendCam`,
//! `genCamDcc`; defaults from `CaService.ned`: 0.1 s / 1.0 s, 4°, 4 m, 0.5 m/s,
//! `withDccRestriction = true`), which is the reference ETSI EN 302 637-2 V1.3.2 §6.1.3
//! implementation in OMNeT++.
//!
//! Artery's state machine is transcribed below and both are driven by the same trajectory,
//! sampled at Artery's 100 ms middleware update interval, under several DCC gate intervals.
//! Every check must agree: same instant, same low-frequency container.

use v2xw_core::geom::Vec3;
use v2xw_core::time::{Duration, SimTime};
use v2xw_msg::generator::{CamDynamics, CamGenParams, CamTriggerState, DccState};

const MS: u64 = 1_000_000;

/// Artery's `CaService`, field for field.
struct Artery {
    gen_cam: u64,
    min: u64,
    max: u64,
    low_counter: u32,
    low_limit: u32,
    last_ts: Option<u64>,
    last_low_ts: Option<u64>,
    last: Option<CamDynamics>,
}

impl Artery {
    fn new() -> Self {
        Self {
            gen_cam: 1000 * MS,
            min: 100 * MS,
            max: 1000 * MS,
            low_counter: 0,
            low_limit: 3,
            last_ts: None,
            last_low_ts: None,
            last: None,
        }
    }

    /// `checkTriggeringConditions`; `dcc` is the TRC interval, clamped as `genCamDcc` does.
    /// Returns `Some(low_frequency)` when a CAM is sent.
    fn check(&mut self, now: u64, d: &CamDynamics, dcc: u64) -> Option<bool> {
        let t_gen_cam_dcc = dcc.clamp(self.min, self.max);
        // Artery's first CAM: `mLastCamTimestamp` starts at 0 s and the first check is at
        // least T_GenCamMax later in practice; model "never sent" as always eligible.
        let elapsed = self.last_ts.map_or(u64::MAX, |t| now - t);
        if elapsed < t_gen_cam_dcc {
            return None;
        }
        let Some(last) = self.last.clone() else {
            return Some(self.send(now, d));
        };
        let mut dh = (d.heading_rad - last.heading_rad).to_degrees().abs() % 360.0;
        if dh > 180.0 {
            dh = 360.0 - dh;
        }
        let heading = dh > 4.0;
        let position = d.pos.distance_2d(last.pos) > 4.0;
        let speed = (d.speed_mps - last.speed_mps).abs() > 0.5;
        if heading || position || speed {
            let lf = self.send(now, d);
            self.gen_cam = elapsed.min(self.max);
            self.low_counter = 0;
            return Some(lf);
        }
        if elapsed >= self.gen_cam {
            let lf = self.send(now, d);
            self.low_counter += 1;
            if self.low_counter >= self.low_limit {
                self.gen_cam = self.max;
            }
            return Some(lf);
        }
        None
    }

    fn send(&mut self, now: u64, d: &CamDynamics) -> bool {
        self.last = Some(d.clone());
        self.last_ts = Some(now);
        let lf = self.last_low_ts.is_none_or(|t| now - t >= 500 * MS);
        if lf {
            self.last_low_ts = Some(now);
        }
        lf
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

#[test]
fn cam_generation_matches_artery_ca_service() {
    let traj = trajectory();
    for dcc_ms in [0u64, 100, 250, 500, 1000, 2000] {
        let mut ours = CamTriggerState::new(CamGenParams::default());
        let mut artery = Artery::new();
        let dcc = DccState {
            t_off: Duration::from_millis(dcc_ms),
            cbr: None,
        };
        let (mut n_ours, mut n_artery, mut lf_ours, mut mismatches) = (0, 0, 0, Vec::new());
        for (t, d) in &traj {
            let a = ours.check(*t, d, &dcc);
            let b = artery.check(*t, d, dcc_ms * MS);
            n_ours += u32::from(a.is_some());
            n_artery += u32::from(b.is_some());
            lf_ours += u32::from(a.as_ref().is_some_and(|x| x.include_low_frequency));
            if a.as_ref().map(|x| x.include_low_frequency) != b {
                mismatches.push(*t / MS);
            }
        }
        println!(
            "MEAS artery-cam dcc {dcc_ms:>4} ms: ours {n_ours} CAMs ({lf_ours} with LF), Artery \
             {n_artery}, mismatched checks {}",
            mismatches.len()
        );
        assert!(
            mismatches.is_empty(),
            "dcc {dcc_ms} ms: differs from Artery at {mismatches:?} ms"
        );
    }
}
