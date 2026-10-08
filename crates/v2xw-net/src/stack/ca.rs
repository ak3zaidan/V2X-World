//! The CA basic service's generation rules [EN 302 637-2 V1.4.1 §6.1.3] as a state
//! machine.
//!
//! A CAM is due when `T_GenCamMax` passed, or when at least `T_GenCam` passed *and* the
//! vehicle's heading changed by more than 4°, its position by more than 4 m, or its speed
//! by more than 0.5 m/s since the last CAM. A CAM triggered by dynamics sets `T_GenCam` to
//! the elapsed time and keeps it for `N_GenCam` CAMs; the low-frequency container goes in
//! the first CAM and then whenever 500 ms or more passed since the last one did.

use serde::{Deserialize, Serialize};

/// `T_GenCamMin`: 100 ms [§6.1.3].
pub const T_GEN_CAM_MIN_MS: u64 = 100;
/// `T_GenCamMax`: 1 000 ms [§6.1.3].
pub const T_GEN_CAM_MAX_MS: u64 = 1_000;
/// `N_GenCam` default: 3 [§6.1.3].
pub const N_GEN_CAM: u32 = 3;
/// Heading threshold, degrees [§6.1.3 condition 1].
pub const HEADING_DELTA_DEG: f64 = 4.0;
/// Position threshold, m [§6.1.3 condition 1].
pub const POSITION_DELTA_M: f64 = 4.0;
/// Speed threshold, m/s [§6.1.3 condition 1].
pub const SPEED_DELTA_MPS: f64 = 0.5;
/// Low-frequency container interval, ms [§6.1.3].
pub const LF_INTERVAL_MS: u64 = 500;

/// The vehicle state the CA service samples every `T_CheckCamGen`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Kinematics {
    /// x, m.
    pub x: f64,
    /// y, m.
    pub y: f64,
    /// Speed, m/s.
    pub speed: f64,
    /// Heading, degrees.
    pub heading_deg: f64,
}

/// Why a CAM was generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CamTrigger {
    /// The first CAM.
    First,
    /// Dynamics (heading, position or speed).
    Dynamics,
    /// `T_GenCam` elapsed without a dynamics trigger.
    Timer,
}

/// A generated CAM's facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CamDecision {
    /// Why.
    pub trigger: CamTrigger,
    /// Whether the low-frequency container is included.
    pub low_frequency: bool,
}

/// The CA basic service state.
#[derive(Debug, Clone, Default)]
pub struct CaService {
    last: Option<(u64, Kinematics)>,
    last_lf_ms: Option<u64>,
    t_gen_cam_ms: u64,
    n_left: u32,
}

impl CaService {
    /// A fresh service.
    pub fn new() -> Self {
        CaService { t_gen_cam_ms: T_GEN_CAM_MAX_MS, ..Default::default() }
    }

    /// The current `T_GenCam`, ms.
    pub fn t_gen_cam_ms(&self) -> u64 {
        self.t_gen_cam_ms
    }

    /// Check at `now_ms` (call every `T_CheckCamGen <= T_GenCamMin`).
    pub fn check(&mut self, now_ms: u64, k: Kinematics) -> Option<CamDecision> {
        let Some((t_last, prev)) = self.last else {
            self.last = Some((now_ms, k));
            self.last_lf_ms = Some(now_ms);
            return Some(CamDecision { trigger: CamTrigger::First, low_frequency: true });
        };
        let elapsed = now_ms.saturating_sub(t_last);
        if elapsed < T_GEN_CAM_MIN_MS {
            return None;
        }
        let mut dh = (k.heading_deg - prev.heading_deg).abs() % 360.0;
        if dh > 180.0 {
            dh = 360.0 - dh;
        }
        let dp = v2xw_core::math::hypot(k.x - prev.x, k.y - prev.y);
        let dv = (k.speed - prev.speed).abs();
        let trigger = if dh > HEADING_DELTA_DEG || dp > POSITION_DELTA_M || dv > SPEED_DELTA_MPS {
            self.t_gen_cam_ms = elapsed.clamp(T_GEN_CAM_MIN_MS, T_GEN_CAM_MAX_MS);
            self.n_left = N_GEN_CAM;
            CamTrigger::Dynamics
        } else if elapsed >= self.t_gen_cam_ms {
            if self.n_left > 0 {
                self.n_left -= 1;
                if self.n_left == 0 {
                    self.t_gen_cam_ms = T_GEN_CAM_MAX_MS;
                }
            }
            CamTrigger::Timer
        } else {
            return None;
        };
        let lf = self.last_lf_ms.is_none_or(|t| now_ms.saturating_sub(t) >= LF_INTERVAL_MS);
        if lf {
            self.last_lf_ms = Some(now_ms);
        }
        self.last = Some((now_ms, k));
        Some(CamDecision { trigger, low_frequency: lf })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standstill_sends_at_t_gen_cam_max_and_a_mover_at_its_pace() {
        let still = Kinematics { x: 0.0, y: 0.0, speed: 0.0, heading_deg: 0.0 };
        let mut s = CaService::new();
        let sent: Vec<u64> = (0..=3_000).step_by(100).filter(|&t| s.check(t, still).is_some()).collect();
        assert_eq!(sent, vec![0, 1_000, 2_000, 3_000]);
        // 25 m/s: more than 4 m every 200 ms -> a CAM every 200 ms.
        let mut m = CaService::new();
        let mut n = 0;
        for t in (0..=2_000u64).step_by(100) {
            let k = Kinematics { x: 25.0 * t as f64 / 1_000.0, y: 0.0, speed: 25.0, heading_deg: 0.0 };
            if m.check(t, k).is_some() {
                n += 1;
            }
        }
        assert_eq!(n, 11);
    }
}
