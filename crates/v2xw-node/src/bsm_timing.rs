//! `node/bsm-timing/per-device`: how a real OBU's BSM timestamp relates to the GNSS fix
//! its position comes from (Paper 1 item B, research/synthesis/paper-plan-2026-10.md §1.4).
//!
//! On public CV-pilot BSMs (research/synthesis/check-realdata-and-vesnos.md §2.3) honest
//! vehicles trip SM-MBR RandomPosition because of the **timing** of the message, not its
//! position error: the reported position comes from one GNSS fix epoch and the timestamp
//! says another (an *epoch slip*), or the position is repeated from the previous message
//! while the vehicle moves (a *frozen repeat*), and the 1609.2 `generationTime` jitters
//! against `secMark`. Whether a device does this is a property of the device: IDs whose
//! `secMark` sits on the fix epoch ("locked") trip it about 0 times per ID-minute at speed,
//! IDs that do not ("free") 20–195 times.
//!
//! This module gives each vehicle OBU a **timing class** (keyed draw, its own RNG domain)
//! and applies the class to the **transmitted** BSM only:
//!
//! * fix epochs are every `fix_interval_ms` on the GNSS (simulation) timeline; the
//!   message's base epoch `E` is the latest one at or before the instant its belief is for;
//! * `secMark` is the timestamp epoch plus the class's `secmark_phase_ms` (drawn once per
//!   device, or once per message when the class says `phase_scope: message`);
//! * with `epoch_slip_prob` the position and the timestamp come from different epochs:
//!   a slip of `k < 0` reports the fix `|k|` epochs older than the timestamp says, a slip of
//!   `k > 0` stamps the timestamp `k` epochs older than the fix (both causal: the reported
//!   position is always a fix the receiver already had);
//! * with `frozen_repeat_prob` the previous message's position is sent again while moving;
//! * the 1609.2 `generationTime` is the device's own time plus a normal jitter of
//!   `header_jitter_ms.sd`.
//!
//! Schema `v2xw.bsm_timing.v2` (v1 files still read) adds two per-class blocks:
//!
//! * `bsm_clock`: the device's own BSM clock. A free OBU generates its BSMs on its own
//!   timer, whose period is the fix interval plus a per-device `drift_ms` and a per-BSM
//!   `period_dev_ms`; `secMark` and `generationTime` are the tick, and the position is the
//!   latest fix at or before the tick. The tick's phase therefore walks across the fix grid,
//!   and each wrap skips a fix (two epochs of travel in one secMark period) while secMark
//!   keeps advancing by about one period: CV-pilot free devices advance secMark by 100-136 ms
//!   per BSM and never repeat it (TM-1, testing/paper1-validation.md §5). The node wakes on
//!   its 100 ms schedule; a window with no tick sends nothing, and the BSM leaves at its tick.
//! * `output_noise`: the receiver's per-BSM speed and heading output noise, white, on the
//!   transmitted `speed` and `heading` only (the reported position follows the fix).
//!
//! Schema `v2xw.bsm_timing.v3` adds two more:
//!
//! * `position_noise`: the receiver's reported-position error, a random walk over the fix
//!   epochs whose per-epoch increment is a Gaussian scale mixture (one component drawn per
//!   epoch, along- and cross-track sds per component), mean-reverting with `tau_s`. A BSM
//!   between two epochs carries the error interpolated between them. CV-pilot locked
//!   devices' exactly-100 ms residuals are white (lag-1 about 0) and heavy-tailed (rms 5.7
//!   cm against an IQR-based sd of 1.0 cm on Tampa), which white Gaussian velocity noise
//!   (v2's `output_noise` read alone) does not give (testing/paper1-validation.md §8).
//! * `grid_hop`: a locked device's :00/:50 grid process. Per BSM, a device on its home grid
//!   leaves with `leave_prob` and one away returns with `return_prob`; away, the BSM is
//!   generated half a fix interval after the epoch (secMark, position and the transmission
//!   all at that instant), so a hop shows as secMark Δt 150 then 50 ms with a position
//!   consistent with Δt.
//!
//! A slip `k > 0` reports the position of the fix `k` epochs **later** than the timestamp
//! says (an over-extrapolated, latency-compensated output), with the timestamp unchanged;
//! v1's "older timestamp" form repeated secMark, which no CV-pilot device does.
//!
//! Ground truth, the mobility model and the GNSS model are untouched: the slip is between
//! the reported position and the reported time. The position of an earlier fix epoch is
//! the node's own belief carried back along its own believed velocity, which is exactly the
//! fix when the GNSS model samples on the fix grid (the engine's 100 ms mobility step).
//! It replaces the raw-vs-smoothed GNSS factor as the honest-trigger knob; the GNSS
//! presets stay as they are.
//!
//! Off unless `actors.vehicles.bsm_timing.enabled` is true. The calibration is the shared
//! `v2xw.bsm_timing.v1` file (`data/calibration/bsm-timing-cvpilot.yaml`, written by the
//! real-data pipeline); [`BsmTimingCalibration::provisional`] is the built-in stand-in,
//! marked provisional, taken from the two research checks.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};
use v2xw_core::belief::PositionEstimate;
use v2xw_core::card::{
    Determinism, Equation, Family, ModelCard, Parameter, Source, SourceKind, Tier, Validation,
    ValidationStatus,
};
use v2xw_core::geom::Vec3;
use v2xw_core::rng::{RngDomain, RngStream};
use v2xw_core::time::SimTime;

/// The model id, which is also the name of its RNG domain.
pub const MODEL_ID: &str = "node/bsm-timing/per-device";

/// The first calibration schema this module reads.
pub const SCHEMA: &str = "v2xw.bsm_timing.v1";

/// v1 plus a class's `bsm_clock` and `output_noise`.
pub const SCHEMA_V2: &str = "v2xw.bsm_timing.v2";

/// The current schema: v2 plus a class's `position_noise` and `grid_hop`.
pub const SCHEMA_V3: &str = "v2xw.bsm_timing.v3";

/// The RNG domain every draw of the model comes from: one stream per node.
#[must_use]
pub fn rng_domain() -> RngDomain {
    RngDomain::plugin(MODEL_ID)
}

/// What a BSM says beyond position, speed and heading that a receiver's SM-MBR checks
/// read: its `secMark` and its `accelSet` longitudinal acceleration.
pub use v2xw_core::bsm_fields::BsmExtras;

/// A distribution of `secMark` minus the fix epoch, milliseconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "dist", rename_all = "lowercase")]
pub enum PhaseDist {
    /// Normal, `mean` and `sd` in ms.
    Normal {
        /// Mean, ms.
        mean: f64,
        /// Standard deviation, ms.
        sd: f64,
    },
    /// Uniform on `[lo, hi)`, ms.
    Uniform {
        /// Lower bound, ms.
        lo: f64,
        /// Upper bound, ms.
        hi: f64,
    },
    /// Empirical: `p` over `bins_ms`. With one more bin value than probabilities the
    /// values are bin **edges** and a draw is uniform inside its bin; with as many, they
    /// are point masses.
    Empirical {
        /// Bin edges or point values, ms, ascending.
        bins_ms: Vec<f64>,
        /// The probability of each bin or point; sums to 1.
        p: Vec<f64>,
    },
}

impl PhaseDist {
    /// One draw, ms. Consumes exactly two 64-bit draws whatever the distribution, so a
    /// change of distribution does not shift the device's later draws.
    fn draw(&self, rng: &mut RngStream) -> f64 {
        let n = rng.normal(0.0, 1.0);
        let u = rng.uniform(0.0, 1.0);
        match self {
            Self::Normal { mean, sd } => mean + sd * n,
            Self::Uniform { lo, hi } => lo + (hi - lo) * u,
            Self::Empirical { bins_ms, p } => {
                // The bin comes from `u`, the place inside it from Φ(n), a second uniform.
                let mut acc = 0.0;
                let mut i = p.len().saturating_sub(1);
                for (j, pj) in p.iter().enumerate() {
                    acc += pj;
                    if u < acc {
                        i = j;
                        break;
                    }
                }
                if bins_ms.len() == p.len() + 1 {
                    let inner = 0.5 * (1.0 + erf(n / core::f64::consts::SQRT_2));
                    bins_ms[i] + (bins_ms[i + 1] - bins_ms[i]) * inner.clamp(0.0, 1.0)
                } else {
                    bins_ms.get(i).copied().unwrap_or(0.0)
                }
            }
        }
    }

    fn check(&self, at: &str) -> Result<(), String> {
        let finite = |v: f64| v.is_finite();
        match self {
            Self::Normal { mean, sd } if !finite(*mean) || !finite(*sd) || *sd < 0.0 => Err(
                format!("{at}: a normal phase needs a finite mean and an sd of 0 or more"),
            ),
            Self::Uniform { lo, hi } if !finite(*lo) || !finite(*hi) || lo > hi => {
                Err(format!("{at}: a uniform phase needs finite lo <= hi"))
            }
            Self::Empirical { bins_ms, p } => {
                let sum: f64 = p.iter().sum();
                if p.is_empty()
                    || p.iter().any(|v| !v.is_finite() || *v < 0.0)
                    || (sum - 1.0).abs() > 1e-6
                {
                    return Err(format!(
                        "{at}: empirical p must be non-negative and sum to 1"
                    ));
                }
                if bins_ms.len() != p.len() && bins_ms.len() != p.len() + 1 {
                    return Err(format!(
                        "{at}: empirical bins_ms needs as many values as p (points) or one \
                         more (edges)"
                    ));
                }
                if bins_ms.iter().any(|v| !v.is_finite()) || bins_ms.windows(2).any(|w| w[0] > w[1])
                {
                    return Err(format!(
                        "{at}: empirical bins_ms must be finite and ascending"
                    ));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
}

/// The error function, Abramowitz and Stegun 7.1.26 (|error| < 1.5e-7): only the place of a
/// draw inside an empirical bin uses it.
fn erf(x: f64) -> f64 {
    let t = 1.0 / (1.0 + 0.327_591_1 * x.abs());
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * v2xw_core::math::exp(-x * x);
    if x < 0.0 { -y } else { y }
}

/// Whether a device's `secMark` phase is drawn once per device or once per message.
///
/// A v2xw extension of the shared format (absent = `device`). On Tampa the residual's p50
/// is 0.019 m while 4.5 % of fast pairs exceed 1 m (check-realdata §2.2–2.3): the phase of
/// a free device is steady and its triggers are discrete epoch slips, which a per-message
/// phase draw would not reproduce.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PhaseScope {
    /// One phase per device for the run.
    #[default]
    Device,
    /// A fresh phase on every message.
    Message,
}

/// The `header_jitter_ms` block.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Jitter {
    /// Standard deviation of `generationTime` minus the device's time, ms.
    pub sd: f64,
}

/// The `bsm_clock` block (schema v2): the device's own BSM timer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BsmClock {
    /// The device's period minus the fix interval, ms, drawn once per device.
    pub drift_ms: PhaseDist,
    /// Each BSM's period minus the device's own, ms, drawn per BSM. Its tail is the
    /// clock's resets.
    pub period_dev_ms: PhaseDist,
}

/// The `output_noise` block (schema v2): the receiver's white output noise on the
/// transmitted speed and heading.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OutputNoise {
    /// Standard deviation of the transmitted speed, m/s.
    pub speed_mps: f64,
    /// Standard deviation of the transmitted heading, degrees.
    pub heading_deg: f64,
}

/// The `position_noise` block (schema v3): the reported position's error, a random walk
/// over the fix epochs with a Gaussian scale-mixture increment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PositionNoise {
    /// `normal_mixture` (the only form).
    pub dist: String,
    /// Mean-reversion time of the walk, s: each epoch the error keeps `exp(-T_fix / tau_s)`
    /// of itself before the increment is added.
    pub tau_s: f64,
    /// The weight of each mixture component; sums to 1.
    pub p: Vec<f64>,
    /// Each component's along-track increment sd, m.
    pub along_m: Vec<f64>,
    /// Each component's cross-track increment sd, m.
    pub cross_m: Vec<f64>,
}

/// The `grid_hop` block (schema v3): a locked device's :00/:50 grid process, per BSM.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GridHop {
    /// P(a device on its home grid moves to the other one at this BSM).
    pub leave_prob: f64,
    /// P(a device on the other grid moves back at this BSM).
    pub return_prob: f64,
}

/// One device timing class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimingClass {
    /// The class name (`locked`, `free`, ...).
    pub name: String,
    /// The share of devices in this class, 0..1; shares sum to 1.
    pub share: f64,
    /// `secMark` minus the fix epoch, ms.
    pub secmark_phase_ms: PhaseDist,
    /// Whether the phase is per device or per message (extension; default device).
    #[serde(default)]
    pub phase_scope: PhaseScope,
    /// Per-message P(position from a different fix epoch than the timestamp says).
    pub epoch_slip_prob: f64,
    /// The slip size in fix intervals, given a slip: `{k: p}`.
    #[serde(default, deserialize_with = "slip_map")]
    pub slip_epochs: BTreeMap<i32, f64>,
    /// Per-message P(position repeated from the previous message while moving).
    pub frozen_repeat_prob: f64,
    /// `generationTime` minus true time.
    #[serde(default)]
    pub header_jitter_ms: Jitter,
    /// The device's own BSM clock (schema v2); `None`: the BSM is stamped at the node's wake.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bsm_clock: Option<BsmClock>,
    /// The receiver's output noise on speed and heading (schema v2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_noise: Option<OutputNoise>,
    /// The reported position's error process (schema v3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position_noise: Option<PositionNoise>,
    /// The :00/:50 grid hops of a device without a `bsm_clock` (schema v3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grid_hop: Option<GridHop>,
}

fn slip_map<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<i32, f64>, D::Error> {
    use serde::de::Error;
    // Through a YAML value, whose keys may be integers (`-1:`) or strings (`"-1":`).
    let m = match serde_yml::Value::deserialize(d)? {
        serde_yml::Value::Mapping(m) => m,
        serde_yml::Value::Null => return Ok(BTreeMap::new()),
        _ => {
            return Err(D::Error::custom(
                "slip_epochs must be a map of slip size to probability",
            ));
        }
    };
    let mut out = BTreeMap::new();
    for (k, v) in m {
        let key = k
            .as_i64()
            .or_else(|| k.as_str().and_then(|s| s.trim().parse::<i64>().ok()))
            .and_then(|i| i32::try_from(i).ok())
            .ok_or_else(|| D::Error::custom(format!("slip_epochs key {k:?} is not an integer")))?;
        let p = v
            .as_f64()
            .ok_or_else(|| D::Error::custom(format!("slip_epochs[{key}] is not a number")))?;
        out.insert(key, p);
    }
    Ok(out)
}

/// A `v2xw.bsm_timing.v1` calibration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BsmTimingCalibration {
    /// `v2xw.bsm_timing.v1`.
    pub schema: String,
    /// Where the numbers came from (kept as written; not interpreted).
    #[serde(default)]
    pub provenance: serde_json::Value,
    /// True for the built-in stand-in values (extension; default false).
    #[serde(default)]
    pub provisional: bool,
    /// The GNSS fix interval, ms (100 for a 10 Hz receiver).
    pub fix_interval_ms: f64,
    /// The device classes.
    pub classes: Vec<TimingClass>,
}

impl BsmTimingCalibration {
    /// Parses and checks a calibration file's text (YAML or JSON).
    ///
    /// # Errors
    /// A message naming what is wrong.
    pub fn from_text(text: &str) -> Result<Self, String> {
        let c: Self = serde_yml::from_str(text).map_err(|e| e.to_string())?;
        c.check()?;
        Ok(c)
    }

    /// Checks every field's range.
    ///
    /// # Errors
    /// The first problem, as a sentence.
    pub fn check(&self) -> Result<(), String> {
        if self.schema != SCHEMA && self.schema != SCHEMA_V2 && self.schema != SCHEMA_V3 {
            return Err(format!(
                "schema is {:?}; this build reads {SCHEMA}, {SCHEMA_V2} and {SCHEMA_V3}",
                self.schema
            ));
        }
        if !(self.fix_interval_ms.is_finite() && self.fix_interval_ms >= 1.0) {
            return Err("fix_interval_ms must be at least 1".to_string());
        }
        if self.classes.is_empty() {
            return Err("classes is empty".to_string());
        }
        let prob = |v: f64| v.is_finite() && (0.0..=1.0).contains(&v);
        let mut total = 0.0;
        for (i, c) in self.classes.iter().enumerate() {
            let at = format!("classes[{i}] ({})", c.name);
            if !prob(c.share) || !prob(c.epoch_slip_prob) || !prob(c.frozen_repeat_prob) {
                return Err(format!(
                    "{at}: share, epoch_slip_prob and frozen_repeat_prob must be in 0..1"
                ));
            }
            total += c.share;
            c.secmark_phase_ms.check(&at)?;
            if self.schema == SCHEMA && (c.bsm_clock.is_some() || c.output_noise.is_some()) {
                return Err(format!(
                    "{at}: bsm_clock and output_noise need schema {SCHEMA_V2}"
                ));
            }
            if self.schema != SCHEMA_V3 && (c.position_noise.is_some() || c.grid_hop.is_some()) {
                return Err(format!(
                    "{at}: position_noise and grid_hop need schema {SCHEMA_V3}"
                ));
            }
            if let Some(n) = &c.position_noise {
                let k = n.p.len();
                let psum: f64 = n.p.iter().sum();
                let sd_ok = |v: &f64| v.is_finite() && *v >= 0.0;
                if n.dist != "normal_mixture"
                    || k == 0
                    || n.along_m.len() != k
                    || n.cross_m.len() != k
                    || !n.p.iter().all(|v| prob(*v))
                    || (psum - 1.0).abs() > 1e-6
                    || !n.along_m.iter().all(sd_ok)
                    || !n.cross_m.iter().all(sd_ok)
                    || !(n.tau_s.is_finite() && n.tau_s > 0.0)
                {
                    return Err(format!(
                        "{at}: position_noise is a normal_mixture with p summing to 1, one \
                         along_m and cross_m sd (0 or more) per component, and tau_s > 0"
                    ));
                }
            }
            if let Some(h) = c.grid_hop {
                if !prob(h.leave_prob) || !prob(h.return_prob) {
                    return Err(format!(
                        "{at}: grid_hop leave_prob and return_prob must be in 0..1"
                    ));
                }
                if c.bsm_clock.is_some() {
                    return Err(format!(
                        "{at}: grid_hop is for a device on the fix grid; a bsm_clock has its own phase"
                    ));
                }
            }
            if let Some(k) = &c.bsm_clock {
                k.drift_ms.check(&format!("{at}.bsm_clock.drift_ms"))?;
                k.period_dev_ms
                    .check(&format!("{at}.bsm_clock.period_dev_ms"))?;
                if c.phase_scope == PhaseScope::Message {
                    return Err(format!(
                        "{at}: a bsm_clock moves the device's own phase; phase_scope must be device"
                    ));
                }
            }
            if let Some(n) = c.output_noise
                && !(n.speed_mps.is_finite()
                    && n.speed_mps >= 0.0
                    && n.heading_deg.is_finite()
                    && n.heading_deg >= 0.0)
            {
                return Err(format!(
                    "{at}: output_noise sds must be finite and 0 or more"
                ));
            }
            if !(c.header_jitter_ms.sd.is_finite() && c.header_jitter_ms.sd >= 0.0) {
                return Err(format!("{at}: header_jitter_ms.sd must be 0 or more"));
            }
            let slip_sum: f64 = c.slip_epochs.values().sum();
            if c.slip_epochs
                .iter()
                .any(|(k, p)| *k == 0 || k.abs() > 10 || !prob(*p))
            {
                return Err(format!(
                    "{at}: slip_epochs keys are non-zero fix counts within ±10, values in 0..1"
                ));
            }
            if c.epoch_slip_prob > 0.0 && (slip_sum - 1.0).abs() > 1e-6 {
                return Err(format!(
                    "{at}: slip_epochs must sum to 1 (it sums to {slip_sum})"
                ));
            }
        }
        if (total - 1.0).abs() > 1e-6 {
            return Err(format!("class shares sum to {total}, not 1"));
        }
        Ok(())
    }

    /// The calibration with the `locked` class at `share` and every other class scaled to
    /// fill the rest in proportion to its own share (equally when they are all 0).
    ///
    /// # Errors
    /// When `share` is outside 0..1 or there is no class named `locked`.
    pub fn with_locked_share(mut self, share: f64) -> Result<Self, String> {
        if !(share.is_finite() && (0.0..=1.0).contains(&share)) {
            return Err(format!("locked_share is {share}; it must be in 0..1"));
        }
        let Some(li) = self.classes.iter().position(|c| c.name == "locked") else {
            return Err("locked_share is set and the calibration has no class named locked".into());
        };
        let others: f64 = self
            .classes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != li)
            .map(|(_, c)| c.share)
            .sum();
        let n_others = (self.classes.len() - 1).max(1) as f64;
        for (i, c) in self.classes.iter_mut().enumerate() {
            c.share = if i == li {
                share
            } else if others > 0.0 {
                (1.0 - share) * c.share / others
            } else {
                (1.0 - share) / n_others
            };
        }
        if self.classes.len() == 1 && share < 1.0 {
            return Err("locked_share below 1 needs a second class to fill the rest".into());
        }
        Ok(self)
    }

    /// The built-in **provisional** values, until the real-data pipeline writes
    /// `data/calibration/bsm-timing-cvpilot.yaml`. Sources:
    /// research/synthesis/check-realdata-and-vesnos.md §2.2–2.4 and
    /// check-paper1-preemption.md §2.1.
    ///
    /// * shares: Tampa's classified IDs, 48 locked and 142 free;
    /// * locked phase: `secMark` on the epoch ±1 ms (Wyoming's xx00/xx50 modes);
    /// * locked slips: 1 of 2,803 fast Tampa pairs over 1 m, about two pairs per slip;
    /// * free slips: 4.5 % of fast Tampa pairs over 1 m, the triggers being epoch slips
    ///   (e.g. 3.04 m in 102 ms at 15.16 m/s), two pairs per slip;
    /// * free frozen repeats: Tampa's ConstantPosition share (447 of 32,291 pairs, all
    ///   speeds); Orem's 13 % is the high end;
    /// * header jitter: Wyoming's header Δt minus secMark Δt, p10 −46 / p90 +53 ms per
    ///   pair, i.e. about 27 ms per message.
    #[must_use]
    pub fn provisional() -> Self {
        let slips = BTreeMap::from([(-1, 0.45), (1, 0.45), (2, 0.10)]);
        Self {
            schema: SCHEMA.to_string(),
            provenance: serde_json::json!({
                "status": "provisional",
                "sources": [
                    "research/synthesis/check-realdata-and-vesnos.md §2.2-2.4",
                    "research/synthesis/check-paper1-preemption.md §2.1"
                ],
                "note": "hand-derived from published sample statistics; replace with the \
                         real-data pipeline's fit (data/calibration/bsm-timing-cvpilot.yaml)"
            }),
            provisional: true,
            fix_interval_ms: 100.0,
            classes: vec![
                TimingClass {
                    name: "locked".to_string(),
                    share: 0.25,
                    secmark_phase_ms: PhaseDist::Normal { mean: 0.0, sd: 0.5 },
                    phase_scope: PhaseScope::Device,
                    epoch_slip_prob: 0.000_2,
                    slip_epochs: slips.clone(),
                    frozen_repeat_prob: 0.0,
                    header_jitter_ms: Jitter { sd: 27.0 },
                    bsm_clock: None,
                    output_noise: None,
                    position_noise: None,
                    grid_hop: None,
                },
                TimingClass {
                    name: "free".to_string(),
                    share: 0.75,
                    secmark_phase_ms: PhaseDist::Uniform { lo: 0.0, hi: 100.0 },
                    phase_scope: PhaseScope::Device,
                    epoch_slip_prob: 0.022,
                    slip_epochs: slips,
                    frozen_repeat_prob: 0.005,
                    header_jitter_ms: Jitter { sd: 27.0 },
                    bsm_clock: None,
                    output_noise: None,
                    position_noise: None,
                    grid_hop: None,
                },
            ],
        }
    }

    fn interval_ns(&self) -> u64 {
        (self.fix_interval_ms * 1e6).round().max(1.0) as u64
    }
}

/// What the model did to one message.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BsmReport {
    /// The position the message reports, world ENU metres.
    pub pos: Vec3,
    /// The instant, on the simulation (GNSS) timeline, the message's `secMark` encodes.
    pub sec_mark_at: SimTime,
    /// `generationTime` minus the device's own time, ns.
    pub header_jitter_ns: i64,
    /// The slip in fix intervals (0 for none).
    pub slip: i32,
    /// Whether the position was repeated from the previous message.
    pub frozen: bool,
    /// False when the device's BSM clock has no tick in this wake's window: nothing is
    /// sent (schema v2 `bsm_clock`); always true otherwise.
    pub send: bool,
    /// How long after the belief's instant the message is generated (its tick), ns.
    pub tx_delay_ns: u64,
    /// The transmitted speed when the class has `output_noise`, m/s.
    pub speed_mps: Option<f64>,
    /// The transmitted heading when the class has `output_noise`, radians.
    pub heading_rad: Option<f64>,
}

/// One device's class and memory.
#[derive(Debug, Clone, PartialEq)]
pub struct BsmTimingDevice {
    class: usize,
    phase_ms: f64,
    last_pos: Option<Vec3>,
    /// The device's BSM-clock drift, ms (0 without a `bsm_clock`).
    drift_ms: f64,
    /// The device's next BSM-clock tick on the GNSS timeline, once it has sent.
    next_tick: Option<SimTime>,
    /// The reported position's error at the current fix epoch, ENU metres (schema v3).
    pos_err: (f64, f64),
    /// Whether the device is on the other :00/:50 grid (schema v3 `grid_hop`).
    away: bool,
    /// The previous belief's instant and ground speed (schema v3 `grid_hop`), from which a
    /// hop BSM's speed is carried to its own instant.
    last_speed: Option<(SimTime, f64)>,
}

impl BsmTimingDevice {
    /// Assigns a device its class (one uniform draw against the cumulative shares) and,
    /// for a per-device phase, its phase.
    pub fn assign(cal: &BsmTimingCalibration, rng: &mut RngStream) -> Self {
        let u = rng.uniform(0.0, 1.0);
        // The `locked` class first, then the rest in file order: a higher locked share
        // then only ever moves a device into the locked class.
        let locked = cal.classes.iter().position(|c| c.name == "locked");
        let order = locked
            .into_iter()
            .chain((0..cal.classes.len()).filter(|i| Some(*i) != locked));
        let mut acc = 0.0;
        let mut class = cal.classes.len() - 1;
        for i in order {
            acc += cal.classes[i].share;
            class = i;
            if u < acc {
                break;
            }
        }
        let phase_ms = cal.classes[class].secmark_phase_ms.draw(rng);
        // A v2 clock's drift is a third draw, taken only by a class that has a clock, so a
        // v1 class's sequence is the one it always was.
        let drift_ms = cal.classes[class]
            .bsm_clock
            .as_ref()
            .map_or(0.0, |k| k.drift_ms.draw(rng));
        Self {
            class,
            phase_ms,
            last_pos: None,
            drift_ms,
            next_tick: None,
            pos_err: (0.0, 0.0),
            away: false,
            last_speed: None,
        }
    }

    /// The device's class.
    #[must_use]
    pub fn class(&self) -> usize {
        self.class
    }

    /// Applies the class to one message built from `belief` (on the simulation timeline at
    /// `belief.time_ns`). `None` when the belief has no position: nothing is sent.
    ///
    /// Every call consumes the same seven 64-bit draws, whatever the class's numbers, plus
    /// two for a `bsm_clock`, two for `output_noise`, five for `position_noise` and one for
    /// `grid_hop` when the class has them.
    pub fn report(
        &mut self,
        cal: &BsmTimingCalibration,
        rng: &mut RngStream,
        belief: &PositionEstimate,
    ) -> Option<BsmReport> {
        let c = &cal.classes[self.class];
        let phase_draw = c.secmark_phase_ms.draw(rng); // 2 draws
        let u_slip = rng.uniform(0.0, 1.0);
        let u_size = rng.uniform(0.0, 1.0);
        let u_frozen = rng.uniform(0.0, 1.0);
        let jitter = rng.normal(0.0, 1.0) * c.header_jitter_ms.sd; // 2 draws
        let period_dev_ms = c.bsm_clock.as_ref().map(|k| k.period_dev_ms.draw(rng)); // 2
        let noise = c.output_noise.map(|n| {
            let ns = rng.normal(0.0, 1.0) * n.speed_mps;
            (ns, rng.normal(0.0, 1.0) * n.heading_deg.to_radians())
        }); // 2 draws
        // v3: one mixture component for this epoch's increment and its two parts (5 draws),
        // then the grid process (1 draw).
        let pn_step = c.position_noise.as_ref().map(|n| {
            let u = rng.uniform(0.0, 1.0);
            let za = rng.normal(0.0, 1.0);
            let zc = rng.normal(0.0, 1.0);
            let mut j = n.p.len() - 1;
            let mut acc = 0.0;
            for (i, p) in n.p.iter().enumerate() {
                acc += p;
                if u < acc {
                    j = i;
                    break;
                }
            }
            (za * n.along_m[j], zc * n.cross_m[j], n.tau_s)
        });
        let u_hop = c.grid_hop.map(|_| rng.uniform(0.0, 1.0));
        if !belief.fix.has_position() {
            return None;
        }
        // J2735 Speed LSB: below it the vehicle is not moving and a repeat is no repeat.
        let moving = belief.ground_speed_mps() >= 0.02;
        // The error walk advances one fix epoch per wake while the vehicle moves: (error
        // now, error at the next epoch). Standing, the receiver holds its solution (static
        // hold), so the walk holds too and its error stays on the reported position: left
        // off at a stop, the position jumped by the whole walk (sd about 0.7 m) at every
        // stop and start, which SM-MBR RandomPosition read as a jump on honest locked cars
        // in queues (P1-E7 E7-F0: 32 honest RandomPosition reports at the MA, each one a
        // stop/start toggle; CV-pilot locked devices show 0.00 % below 5 m/s, T4).
        let pn = pn_step.map(|(wa, wc, tau)| {
            if !moving {
                return (self.pos_err, self.pos_err);
            }
            let (sh, ch) = belief.heading_rad.sin_cos();
            // along = (cos h, sin h), left of track = (-sin h, cos h): ENU, h from east.
            let w = (wa * ch - wc * sh, wa * sh + wc * ch);
            let rho = (-cal.fix_interval_ms * 1e-3 / tau).exp();
            let cur = self.pos_err;
            let next = (rho * cur.0 + w.0, rho * cur.1 + w.1);
            self.pos_err = next;
            (cur, next)
        });
        if let (Some(h), Some(u)) = (c.grid_hop, u_hop) {
            self.away = if self.away {
                u >= h.return_prob
            } else {
                u < h.leave_prob
            };
        }
        let phase_ms = match c.phase_scope {
            PhaseScope::Device => self.phase_ms,
            PhaseScope::Message => phase_draw,
        };
        let iv = cal.interval_ns();
        let t = belief.time_ns;
        // With a BSM clock the message is generated at the device's own tick in this wake's
        // window [t, t + T_fix), and the fix it carries is the latest at or before the tick.
        let mut tick = None;
        if let Some(dev) = period_dev_ms {
            let win_end = t.saturating_add(iv);
            let g = match self.next_tick {
                Some(g) => g.max(t),
                None => {
                    let base = t - t % iv;
                    let phase_ns =
                        ((phase_ms * 1e6).round() as i128).rem_euclid(i128::from(iv)) as u64;
                    let g = base + phase_ns;
                    if g < t { g + iv } else { g }
                }
            };
            if g >= win_end {
                // No tick in this window: the device sends nothing until its next one.
                self.next_tick = Some(g);
                return Some(BsmReport {
                    pos: belief.pos,
                    sec_mark_at: g,
                    header_jitter_ns: 0,
                    slip: 0,
                    frozen: false,
                    send: false,
                    tx_delay_ns: 0,
                    speed_mps: None,
                    heading_rad: None,
                });
            }
            let period_ms = cal.fix_interval_ms + self.drift_ms + dev;
            let next = (i128::from(g) + (period_ms * 1e6).round() as i128).max(0) as SimTime;
            // One BSM per wake: a period shorter than what is left of the window waits for
            // the next one.
            self.next_tick = Some(next.max(win_end));
            tick = Some(g);
        }
        let epoch = tick.map_or(t - t % iv, |g| g - g % iv);
        // Away on the other grid, the BSM is generated half a fix interval after the epoch.
        let hop_ns = if tick.is_none() && c.grid_hop.is_some() && self.away {
            iv / 2
        } else {
            0
        };
        let mut slip = 0;
        if u_slip < c.epoch_slip_prob {
            let mut acc = 0.0;
            for (k, p) in &c.slip_epochs {
                acc += p;
                slip = *k;
                if u_size < acc {
                    break;
                }
            }
        }
        // A per-message phase is the generation instant's jitter about the epoch, and the
        // position is the fix carried to that instant: CV-pilot locked devices' along-track
        // residual at secMark Δt 99 / 101 ms is +0.41 / -0.50 ms × v, the 1 ms truncation of a
        // jittered instant, not the ±1 ms of a position left on the epoch
        // (data/calibration/bsm-timing-cvpilot.yaml). The timestamp keeps its epoch; a slip
        // moves the position k fixes (later for k > 0).
        let at_instant = if tick.is_none() && c.phase_scope == PhaseScope::Message {
            (phase_ms * 1e6).round() as i128
        } else {
            0
        };
        let pos_epoch = (i128::from(epoch)
            + i128::from(hop_ns)
            + at_instant
            + i128::from(slip) * i128::from(iv))
        .max(0);
        let age_s = (i128::from(t) - pos_epoch) as f64 * 1e-9;
        let mut pos = Vec3::new(
            belief.pos.x - belief.vel.x * age_s,
            belief.pos.y - belief.vel.y * age_s,
            belief.pos.z - belief.vel.z * age_s,
        );
        // The reported position's error at the generation instant, between this epoch's and
        // the next one's (a standing vehicle's walk is held, so it reports its fix plus the
        // error it stopped with).
        if let Some((cur, next)) = pn {
            let inst = tick.map_or(epoch + hop_ns, |g| g);
            let f = ((inst.saturating_sub(t - t % iv)) as f64 / iv as f64).clamp(0.0, 1.0);
            pos.x += cur.0 + f * (next.0 - cur.0);
            pos.y += cur.1 + f * (next.1 - cur.1);
        }
        let frozen = moving && u_frozen < c.frozen_repeat_prob && self.last_pos.is_some();
        if frozen && let Some(p) = self.last_pos {
            pos = p;
        }
        self.last_pos = Some(pos);
        let sec_mark_at = tick.unwrap_or_else(|| {
            (i128::from(epoch) + i128::from(hop_ns) + (phase_ms * 1e6).round() as i128).max(0)
                as SimTime
        });
        // A hop BSM is generated half an interval after the belief, its position carried
        // there along the believed velocity (above). Its speed is carried there too, with
        // the belief's own recent acceleration: left at the belief's instant, a hop's secMark
        // Δt of 50 ms doubles the acceleration that MaxAcceleration reads (|Δv| / Δt), and a
        // 5 m/s² deceleration fired it (61 honest verdicts on E4U before this fix), where
        // CV-pilot locked devices show 0.0 per ID-minute (p1-realdata-baseline.md T5).
        let v_now = belief.ground_speed_mps();
        let accel = match self.last_speed {
            Some((t0, v0)) if t > t0 && t - t0 <= 2 * iv => (v_now - v0) / ((t - t0) as f64 * 1e-9),
            _ => 0.0,
        };
        if c.grid_hop.is_some() {
            self.last_speed = Some((t, v_now));
        }
        let v_at = (v_now + accel * hop_ns as f64 * 1e-9).max(0.0);
        let (speed_mps, heading_rad) = match noise {
            Some((ns, nh)) if moving => (Some((v_at + ns).max(0.0)), Some(belief.heading_rad + nh)),
            _ if moving && hop_ns > 0 => (Some(v_at), Some(belief.heading_rad)),
            _ => (None, None),
        };
        Some(BsmReport {
            pos,
            sec_mark_at,
            header_jitter_ns: (jitter * 1e6).round() as i64,
            slip,
            frozen,
            send: true,
            tx_delay_ns: tick.map_or((epoch + hop_ns).saturating_sub(t), |g| g.saturating_sub(t)),
            speed_mps,
            heading_rad,
        })
    }
}

/// The model card for a calibration.
#[must_use]
pub fn card(cal: &BsmTimingCalibration) -> ModelCard {
    let mut card = ModelCard::new(
        MODEL_ID,
        Family::Gnss,
        "1.0.0",
        "Per-device BSM timing: each vehicle OBU's timing class sets the transmitted BSM's \
         secMark phase against the GNSS fix epoch, its position-epoch slips, its frozen \
         repeats while moving and its 1609.2 generationTime jitter.",
    );
    card.tier = vec![Tier::Abstract, Tier::Medium, Tier::High];
    card.equations = vec![
        Equation::new("epoch", "E = ⌊t / T_fix⌋·T_fix"),
        Equation::new("secMark", "secMark = E_time + φ_class"),
        Equation::new("position", "p = p̂(t) − v̂·(t − E_pos)"),
        Equation::new("generationTime", "t_gen = t_device + N(0, σ_hdr)"),
    ];
    let src = |r: &str| Source::new(SourceKind::Dataset, r);
    card.parameters = vec![Parameter::new(
        "fix_interval_ms",
        "ms",
        serde_json::json!(cal.fix_interval_ms),
        src("v2xw.bsm_timing.v1 calibration"),
    )];
    for c in &cal.classes {
        let n = &c.name;
        for (name, unit, v) in [
            ("share", "-", c.share),
            ("epoch_slip_prob", "-", c.epoch_slip_prob),
            ("frozen_repeat_prob", "-", c.frozen_repeat_prob),
            ("header_jitter_ms.sd", "ms", c.header_jitter_ms.sd),
        ] {
            card.parameters.push(Parameter::new(
                format!("{n}.{name}"),
                unit,
                serde_json::json!(v),
                src(if cal.provisional {
                    "PROVISIONAL: research/synthesis/check-realdata-and-vesnos.md §2.2-2.4"
                } else {
                    "v2xw.bsm_timing.v1 calibration (provenance in the file)"
                }),
            ));
        }
    }
    card.sources = vec![
        Source::new(
            SourceKind::Dataset,
            "USDOT ITS JPO CV Pilot BSMs (THEA Tampa, Wyoming, Utah Orem), \
             research/synthesis/check-realdata-and-vesnos.md",
        ),
        Source::new(
            SourceKind::Code,
            "research/synthesis/paper-plan-2026-10.md §1.4 item B",
        ),
    ];
    card.assumptions = vec![
        "Fix epochs are on the simulation (GPS) timeline; an earlier epoch's position is the \
         belief carried back along the believed velocity."
            .to_string(),
        "A slip keeps the reported position causal: k < 0 reports an older fix, k > 0 an \
         older timestamp."
            .to_string(),
        "phase_scope device (default) draws the secMark phase once per device.".to_string(),
        "position_noise (v3) is applied to the transmitted position only, while moving; the \
         node's belief and the GNSS model are untouched."
            .to_string(),
        "grid_hop (v3) puts the other grid half a fix interval late; a device that hops early \
         (Δt 50 then 150 ms) is modelled as a late one (150 then 50 ms)."
            .to_string(),
    ];
    card.limitations = vec![
        "The phase does not drift with the device oscillator; a slip stands in for the \
         epoch crossings that drift causes."
            .to_string(),
    ];
    card.determinism = Determinism {
        uses_rng: true,
        rng_domains: vec![rng_domain().as_str().to_string()],
    };
    card.validation = Validation {
        status: ValidationStatus::UnitTested,
        references: vec![src("research/synthesis/check-realdata-and-vesnos.md §2.3")],
        tests: vec!["bsm_timing::tests".to_string()],
    };
    card
}

#[cfg(test)]
mod tests {
    use super::*;
    use v2xw_core::belief::FixQuality;
    use v2xw_core::rng::EntityRef;

    fn belief(t_ms: u64, v: f64) -> PositionEstimate {
        let mut b = PositionEstimate::no_fix(t_ms * 1_000_000);
        b.fix = FixQuality::Rtk;
        b.pos = Vec3::new(v * t_ms as f64 * 1e-3, 0.0, 0.0);
        b.vel = Vec3::new(v, 0.0, 0.0);
        b
    }

    fn rng(n: u32) -> RngStream {
        RngStream::derive(
            7,
            rng_domain(),
            EntityRef::Node(v2xw_core::ids::NodeId::new(n)),
        )
    }

    #[test]
    fn the_provisional_file_round_trips_and_checks() {
        let p = BsmTimingCalibration::provisional();
        p.check().expect("provisional values are valid");
        let text = serde_yml::to_string(&p).expect("serialises");
        assert_eq!(BsmTimingCalibration::from_text(&text).expect("parses"), p);
    }

    #[test]
    fn the_shared_format_parses_with_string_slip_keys() {
        let text = r#"
schema: v2xw.bsm_timing.v1
provenance: {sources: [{dataset: thea, rows: 10, query: q}], manifest_sha256: ab, fitted_on: [thea], held_out: [wy], tool: tools/realdata 0.1}
fix_interval_ms: 100
classes:
  - name: locked
    share: 0.4
    secmark_phase_ms: {dist: normal, mean: 0.2, sd: 0.4}
    epoch_slip_prob: 0.001
    slip_epochs: {"-1": 0.5, "1": 0.4, "2": 0.1}
    frozen_repeat_prob: 0
    header_jitter_ms: {sd: 25}
  - name: free
    share: 0.6
    secmark_phase_ms: {dist: empirical, bins_ms: [0, 50, 100], p: [0.5, 0.5]}
    epoch_slip_prob: 0.02
    slip_epochs: {-1: 0.5, 1: 0.5}
    frozen_repeat_prob: 0.01
    header_jitter_ms: {sd: 25}
"#;
        let c = BsmTimingCalibration::from_text(text).expect("the shared shape parses");
        assert_eq!(c.classes[0].slip_epochs.get(&-1), Some(&0.5));
        assert_eq!(c.classes[1].slip_epochs.get(&1), Some(&0.5));
        // Shown red: shares that do not sum to one are refused.
        let bad = text.replace("share: 0.6", "share: 0.7");
        assert!(BsmTimingCalibration::from_text(&bad).is_err());
    }

    #[test]
    fn locked_share_moves_devices_monotonically() {
        let base = BsmTimingCalibration::provisional();
        let mut prev = 0;
        for share in [0.0, 0.5, 1.0] {
            let cal = base.clone().with_locked_share(share).expect("valid");
            cal.check().expect("rescaled shares check");
            let locked = (0..400)
                .filter(|n| BsmTimingDevice::assign(&cal, &mut rng(*n)).class() == 0)
                .count();
            assert!(locked >= prev, "share {share}: {locked} < {prev}");
            prev = locked;
        }
        assert_eq!(prev, 400);
    }

    #[test]
    fn a_forced_slip_moves_the_position_by_whole_fixes() {
        let mut cal = BsmTimingCalibration::provisional()
            .with_locked_share(1.0)
            .unwrap();
        cal.classes[0].epoch_slip_prob = 1.0;
        cal.classes[0].slip_epochs = BTreeMap::from([(-1, 1.0)]);
        let mut r = rng(1);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let rep = d.report(&cal, &mut r, &belief(1_230, 20.0)).unwrap();
        assert_eq!(rep.slip, -1);
        // The base epoch is 1.2 s; the fix reported is 1.1 s's while the time says 1.2 s.
        assert!((rep.pos.x - 22.0).abs() < 1e-9, "{}", rep.pos.x);
        assert!((rep.sec_mark_at as f64 * 1e-6 - 1_200.0).abs() < 3.0);
    }

    #[test]
    fn a_frozen_repeat_sends_the_previous_position() {
        let mut cal = BsmTimingCalibration::provisional()
            .with_locked_share(1.0)
            .unwrap();
        cal.classes[0].frozen_repeat_prob = 1.0;
        let mut r = rng(2);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let a = d.report(&cal, &mut r, &belief(1_000, 20.0)).unwrap();
        let b = d.report(&cal, &mut r, &belief(1_100, 20.0)).unwrap();
        assert!(!a.frozen && b.frozen);
        assert_eq!(a.pos, b.pos);
    }

    #[test]
    fn the_same_seed_gives_the_same_reports() {
        let cal = BsmTimingCalibration::provisional();
        let run = |seed_node: u32| {
            let mut r = rng(seed_node);
            let mut d = BsmTimingDevice::assign(&cal, &mut r);
            (0..200)
                .map(|i| {
                    d.report(&cal, &mut r, &belief(1_000 + 100 * i, 15.0))
                        .unwrap()
                })
                .map(|x| (x.pos.x.to_bits(), x.sec_mark_at, x.header_jitter_ns))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(3), run(3));
        assert_ne!(run(3), run(4), "a different key gives a different sequence");
    }

    /// TM-1: a slip k > 0 moves the position a fix later and keeps the timestamp, so no two
    /// BSMs share a secMark (the v1 form stamped the message a fix older instead).
    #[test]
    fn a_positive_slip_moves_the_position_forward_and_keeps_the_stamp() {
        let mut cal = BsmTimingCalibration::provisional()
            .with_locked_share(1.0)
            .unwrap();
        cal.classes[0].secmark_phase_ms = PhaseDist::Normal { mean: 0.0, sd: 0.0 };
        cal.classes[0].epoch_slip_prob = 1.0;
        cal.classes[0].slip_epochs = BTreeMap::from([(1, 1.0)]);
        let mut r = rng(1);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let rep = d.report(&cal, &mut r, &belief(1_230, 20.0)).unwrap();
        assert_eq!(rep.slip, 1);
        assert_eq!(rep.sec_mark_at, 1_200 * 1_000_000);
        // The fix of 1.3 s, the device's own belief carried forward along its velocity.
        assert!((rep.pos.x - 26.0).abs() < 1e-9, "{}", rep.pos.x);
    }

    fn clocked(drift_ms: f64, dev_ms: f64) -> BsmTimingCalibration {
        let mut cal = BsmTimingCalibration::provisional()
            .with_locked_share(0.0)
            .unwrap();
        cal.schema = SCHEMA_V2.to_string();
        let c = &mut cal.classes[1];
        c.epoch_slip_prob = 0.0;
        c.frozen_repeat_prob = 0.0;
        c.header_jitter_ms = Jitter { sd: 0.0 };
        c.bsm_clock = Some(BsmClock {
            drift_ms: PhaseDist::Empirical {
                bins_ms: vec![drift_ms],
                p: vec![1.0],
            },
            period_dev_ms: PhaseDist::Empirical {
                bins_ms: vec![dev_ms],
                p: vec![1.0],
            },
        });
        cal.check().expect("a valid v2 calibration");
        cal
    }

    /// TM-1: a free device whose own BSM clock runs 3 ms long per message walks across the
    /// fix grid: secMark always advances by its period (never by 0), a window without a tick
    /// sends nothing, and at each wrap the position jumps two fixes.
    #[test]
    fn a_drifting_bsm_clock_never_repeats_a_secmark_and_skips_a_fix_at_each_wrap() {
        let cal = clocked(3.0, 0.0);
        let mut r = rng(5);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        assert_eq!(d.class(), 1);
        let (mut sent, mut skipped, mut doubles) = (Vec::new(), 0, 0);
        for i in 0..1_030u64 {
            let rep = d
                .report(&cal, &mut r, &belief(10_000 + 100 * i, 20.0))
                .unwrap();
            if !rep.send {
                skipped += 1;
                continue;
            }
            assert!(rep.tx_delay_ns < 100_000_000);
            sent.push((rep.sec_mark_at, rep.pos.x));
        }
        for w in sent.windows(2) {
            assert_eq!(
                w[1].0 - w[0].0,
                103_000_000,
                "secMark advances by the period"
            );
            // 20 m/s × 100 ms = 2 m per fix.
            let fixes = ((w[1].1 - w[0].1) / 2.0).round();
            assert!(fixes == 1.0 || fixes == 2.0, "{fixes}");
            doubles += usize::from(fixes == 2.0);
        }
        // 1,030 windows of 100 ms hold 1,000 ticks 103 ms apart.
        assert!((29..=31).contains(&skipped), "{skipped}");
        assert!((29..=31).contains(&doubles), "{doubles}");
    }

    #[test]
    fn output_noise_reaches_the_speed_and_a_v1_file_refuses_it() {
        let mut cal = clocked(3.0, 0.0);
        cal.classes[1].bsm_clock = None;
        cal.classes[1].output_noise = Some(OutputNoise {
            speed_mps: 0.2,
            heading_deg: 0.5,
        });
        cal.check().expect("valid");
        let mut r = rng(6);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let (mut s2, mut n) = (0.0, 0.0);
        for i in 0..2_000u64 {
            let rep = d
                .report(&cal, &mut r, &belief(10_000 + 100 * i, 20.0))
                .unwrap();
            s2 += (rep.speed_mps.expect("a noisy speed") - 20.0).powi(2);
            n += 1.0;
            assert!(rep.heading_rad.is_some());
            assert!((rep.pos.x - 20.0 * (10.0 + 0.1 * i as f64)).abs() < 1e-6);
        }
        let sd = (s2 / n).sqrt();
        assert!((sd - 0.2).abs() < 0.02, "{sd}");
        let mut v1 = cal.clone();
        v1.schema = SCHEMA.to_string();
        assert!(v1.check().is_err(), "a v1 file may not carry output_noise");
    }

    /// A v3 locked class on the epoch exactly (phase 0, no slips, no jitter).
    fn locked_v3() -> BsmTimingCalibration {
        let mut cal = BsmTimingCalibration::provisional()
            .with_locked_share(1.0)
            .unwrap();
        cal.schema = SCHEMA_V3.to_string();
        let c = &mut cal.classes[0];
        c.secmark_phase_ms = PhaseDist::Normal { mean: 0.0, sd: 0.0 };
        c.epoch_slip_prob = 0.0;
        c.frozen_repeat_prob = 0.0;
        c.header_jitter_ms = Jitter { sd: 0.0 };
        cal
    }

    /// E4-BODY: a locked device that hops to the :50 grid stamps, positions and sends its
    /// BSM half an interval late, so secMark Δt is 150 then 50 ms and the position stays
    /// consistent with the stamp; the hop pairs' share follows leave and return.
    #[test]
    fn a_grid_hop_moves_the_stamp_the_position_and_the_send_together() {
        let mut cal = locked_v3();
        cal.classes[0].grid_hop = Some(GridHop {
            leave_prob: 0.2,
            return_prob: 0.5,
        });
        cal.check().expect("a valid v3 calibration");
        let mut r = rng(8);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let mut sent = Vec::new();
        for i in 0..4_000u64 {
            let b = belief(10_000 + 100 * i, 20.0);
            let rep = d.report(&cal, &mut r, &b).unwrap();
            assert!(rep.send);
            assert_eq!(
                rep.sec_mark_at,
                b.time_ns + rep.tx_delay_ns,
                "sent at its stamp"
            );
            let x = 20.0 * rep.sec_mark_at as f64 * 1e-9;
            assert!(
                (rep.pos.x - x).abs() < 1e-6,
                "position at the stamp: {} vs {x}",
                rep.pos.x
            );
            sent.push(rep.sec_mark_at);
        }
        let (mut n50, mut n150) = (0usize, 0usize);
        for w in sent.windows(2) {
            match w[1] - w[0] {
                50_000_000 => n50 += 1,
                100_000_000 => {}
                150_000_000 => n150 += 1,
                d => panic!("secMark Δt {d} ns"),
            }
        }
        // P(hop pair) = 2·l·r / (l + r) = 0.286.
        let share = (n50 + n150) as f64 / (sent.len() - 1) as f64;
        assert!((0.25..0.32).contains(&share), "{share}");
        assert!(n50.abs_diff(n150) <= 1, "{n50} vs {n150}");
        let mut v2 = cal.clone();
        v2.schema = SCHEMA_V2.to_string();
        assert!(v2.check().is_err(), "a v2 file may not carry grid_hop");
    }

    /// A hop BSM carries the speed of its own instant: under a steady 6 m/s² deceleration
    /// the acceleration implied by consecutive BSMs (|Δv| / secMark Δt, SM-MBR
    /// MaxAcceleration) stays 6 m/s² across the 50 ms and 150 ms hop pairs, not 12 and 4.
    #[test]
    fn a_hop_bsm_carries_the_speed_of_its_own_instant() {
        let mut cal = locked_v3();
        cal.classes[0].grid_hop = Some(GridHop {
            leave_prob: 0.3,
            return_prob: 0.5,
        });
        let (mut hops, mut worst, mut least) = (0, 0.0f64, f64::MAX);
        // Twelve devices, each braking from 30 to 3.6 m/s over 4.4 s.
        for n in 0..12u32 {
            let mut r = rng(10 + n);
            let mut d = BsmTimingDevice::assign(&cal, &mut r);
            let mut sent = Vec::new();
            for i in 0..45u64 {
                let s = 0.1 * i as f64;
                let mut b = belief(10_000 + 100 * i, 0.0);
                b.vel = Vec3::new(30.0 - 6.0 * s, 0.0, 0.0);
                b.pos = Vec3::new(30.0 * s - 3.0 * s * s, 0.0, 0.0);
                let rep = d.report(&cal, &mut r, &b).unwrap();
                sent.push((rep.sec_mark_at, rep.speed_mps.unwrap_or(b.vel.x)));
            }
            // The first BSM has no earlier belief to take an acceleration from.
            for w in sent[1..].windows(2) {
                let dt = (w[1].0 - w[0].0) as f64 * 1e-9;
                hops += usize::from((dt - 0.1).abs() > 0.01);
                let a = (w[1].1 - w[0].1).abs() / dt;
                worst = worst.max(a);
                least = least.min(a);
            }
        }
        assert!(hops > 40, "{hops}");
        assert!(least > 5.5, "implied acceleration {least} m/s²");
        assert!(worst < 6.5, "implied acceleration {worst} m/s²");
    }

    /// E4-BODY: position_noise is a white, heavy-tailed increment of the reported position
    /// (a random walk over the fix epochs): exactly-100 ms residuals have the mixture's rms,
    /// an rms far above their IQR-based sd, and no lag-1 correlation; speed is untouched.
    #[test]
    fn position_noise_is_a_heavy_tailed_random_walk_on_the_reported_position() {
        let mut cal = locked_v3();
        cal.classes[0].position_noise = Some(PositionNoise {
            dist: "normal_mixture".to_string(),
            tau_s: 1.0e4,
            p: vec![0.9, 0.1],
            along_m: vec![0.005, 0.1],
            cross_m: vec![0.002, 0.05],
        });
        cal.check().expect("a valid v3 calibration");
        let mut r = rng(9);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let mut pts = Vec::new();
        for i in 0..20_000u64 {
            let rep = d
                .report(&cal, &mut r, &belief(10_000 + 100 * i, 20.0))
                .unwrap();
            assert!(rep.speed_mps.is_none());
            pts.push((rep.pos.x, rep.pos.y));
        }
        let al: Vec<f64> = pts.windows(2).map(|w| w[1].0 - w[0].0 - 2.0).collect();
        let cr: Vec<f64> = pts.windows(2).map(|w| w[1].1 - w[0].1).collect();
        let rms = |v: &[f64]| (v.iter().map(|x| x * x).sum::<f64>() / v.len() as f64).sqrt();
        let iqr_sd = |v: &[f64]| {
            let mut s = v.to_vec();
            s.sort_by(f64::total_cmp);
            (s[s.len() * 3 / 4] - s[s.len() / 4]) / 1.349
        };
        // sqrt(0.9·0.005² + 0.1·0.1²) = 0.0320; sqrt(0.9·0.002² + 0.1·0.05²) = 0.0159.
        assert!((rms(&al) - 0.0320).abs() < 0.003, "{}", rms(&al));
        assert!((rms(&cr) - 0.0159).abs() < 0.0015, "{}", rms(&cr));
        assert!(
            rms(&al) > 3.0 * iqr_sd(&al),
            "heavy-tailed: {} vs {}",
            rms(&al),
            iqr_sd(&al)
        );
        let lag1 =
            al.windows(2).map(|w| w[0] * w[1]).sum::<f64>() / al.len() as f64 / rms(&al).powi(2);
        assert!(lag1.abs() < 0.05, "white increments: lag-1 {lag1}");
        let mut v2 = cal.clone();
        v2.schema = SCHEMA_V2.to_string();
        assert!(
            v2.check().is_err(),
            "a v2 file may not carry position_noise"
        );
    }

    /// P1-E7: with the walk left off at a stop, a stopping or starting car's reported
    /// position jumped by the whole walk (RandomPosition on honest locked cars in queues).
    #[test]
    fn a_stop_keeps_the_error_walk_on_the_reported_position() {
        let mut cal = locked_v3();
        cal.classes[0].position_noise = Some(PositionNoise {
            dist: "normal_mixture".to_string(),
            tau_s: 1.0e4,
            p: vec![1.0],
            along_m: vec![0.2],
            cross_m: vec![0.2],
        });
        cal.check().expect("a valid v3 calibration");
        let mut r = rng(11);
        let mut d = BsmTimingDevice::assign(&cal, &mut r);
        let mut last: Option<(f64, f64, f64)> = None;
        let mut x = 0.0;
        let mut worst: f64 = 0.0;
        for i in 0..400u64 {
            // 10 m/s for 20 s, standing for 10 s, then 10 m/s again.
            let v = if (200..300).contains(&i) { 0.0 } else { 10.0 };
            x += v * 0.1;
            let mut b = belief(10_000 + 100 * i, v);
            b.pos = Vec3::new(x, 0.0, 0.0);
            let rep = d.report(&cal, &mut r, &b).unwrap();
            if let Some((px, py, pxt)) = last {
                // the reported step less the true step: the error's own change
                let e = ((rep.pos.x - px - (x - pxt)).powi(2) + (rep.pos.y - py).powi(2)).sqrt();
                worst = worst.max(e);
            }
            last = Some((rep.pos.x, rep.pos.y, x));
        }
        // One increment is under ~4.5 sigma (0.9 m per axis), where the walk after 200
        // epochs has sd 0.2 * sqrt(200) = 2.8 m per axis, which a stop used to throw away.
        assert!(worst < 1.3, "a stop or start moved the error by {worst:.2} m");
    }
}
